//! Portable, versioned Studio container. Samples and IR are embedded, never external paths.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use orb_engine::studio::audio_file::{MAX_AUDIO_BYTES, MAX_FRAMES, Wave};
use orb_engine::studio::{Settings, Studio, convolution::Convolver};
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 8] = b"ORB338S2";
const SYNTH_MAGIC: &[u8; 8] = b"ORB338S3";
const EXTENDED_SYNTH_MAGIC: &[u8; 8] = b"ORB338S4";
const HYBRID_SYNTH_MAGIC: &[u8; 8] = b"ORB338S5";
const BAR_RATE_MAGIC: &[u8; 8] = b"ORB338S6";
const MAX_BYTES: usize = 256 * 1024 * 1024;
const MAX_METADATA_BYTES: usize = 4 * 1024 * 1024;

/// New projects use .recall; the original Studio extension remains fully supported.
pub fn is_project_path(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("recall") || extension.eq_ignore_ascii_case("orb338")
    })
}

pub fn save_path(path: &Path) -> PathBuf {
    if is_project_path(path) {
        path.to_path_buf()
    } else {
        path.with_extension("recall")
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub names: [String; 8],
    pub ir_name: String,
    pub closed_hat: Option<u8>,
    pub sidechain: crate::sidechain::Settings,
    pub settings: Settings,
}

#[derive(Clone)]
pub struct Project {
    pub metadata: Metadata,
    pub song: Vec<u8>,
    pub waves: [Option<Arc<Wave>>; 8],
    pub ir: Arc<Wave>,
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, &x| {
        (h ^ u64::from(x)).wrapping_mul(0x100000001b3)
    })
}

fn put(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), String> {
    if out.len() + bytes.len() + 12 > MAX_BYTES {
        return Err("Studio project exceeds 256 MiB".into());
    }
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&checksum(bytes).to_le_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

fn take<'a>(bytes: &mut &'a [u8], limit: usize) -> Result<&'a [u8], String> {
    let header = bytes.get(..12).ok_or("Truncated Studio project")?;
    let len = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
    let hash = u64::from_le_bytes(header[4..12].try_into().unwrap());
    if len > limit {
        return Err("Studio project section exceeds its size limit".into());
    }
    let payload = bytes
        .get(12..12 + len)
        .ok_or("Truncated Studio project section")?;
    if checksum(payload) != hash {
        return Err("Studio project checksum mismatch".into());
    }
    *bytes = &bytes[12 + len..];
    Ok(payload)
}

impl Project {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        validate_bank(self.waves.iter().flatten().map(|w| w.frames.len()))?;
        if self.ir.frames.len() > 6 * 44100 {
            return Err("Impulse response exceeds six seconds".into());
        }
        // Older readers ignore unknown TOML fields. Refuse that lossy round trip
        // once either instrument slot contains music or a customised dormant patch.
        let has_synth_data = self
            .metadata
            .settings
            .synth_slots
            .iter()
            .any(|slot| *slot != Default::default());
        let needs_hybrid = self.metadata.settings.synth_slots.iter().any(|slot| {
            use orb_engine::studio::{acid_presets::EXPERIMENTAL_START, acid_synth::Patch};
            let patch = &slot.patch;
            usize::from(slot.preset) >= EXPERIMENTAL_START || patch.osc_mode > 2
                || patch.fm_algorithm != Patch::INIT.fm_algorithm || patch.fm_ratios != Patch::INIT.fm_ratios
                || patch.fm_index != Patch::INIT.fm_index || patch.fm_feedback != Patch::INIT.fm_feedback
                || patch.fold != Patch::INIT.fold || patch.sync != Patch::INIT.sync
        });
        let uses_bar_grid = self.metadata.settings.synth_slots.iter().any(|slot| {
            slot.legacy_128_bars || matches!(slot.length, orb_engine::studio::acid_synth::PhraseLength::Bars(_))
        });
        let version = if uses_bar_grid { BAR_RATE_MAGIC } else if needs_hybrid { HYBRID_SYNTH_MAGIC } else if has_synth_data { EXTENDED_SYNTH_MAGIC } else { MAGIC };
        let mut out = version.to_vec();
        let metadata = toml::to_string(&self.metadata).map_err(|e| e.to_string())?;
        if metadata.len() > MAX_METADATA_BYTES {
            return Err("Studio project metadata exceeds its size limit".into());
        }
        put(&mut out, metadata.as_bytes())?;
        put(&mut out, &self.song)?;
        for wave in self
            .waves
            .iter()
            .map(Option::as_ref)
            .chain([Some(&self.ir)])
        {
            let mut bytes = Vec::new();
            if let Some(wave) = wave {
                bytes.reserve(wave.frames.len() * 8);
                for &x in wave.frames.iter().flatten() {
                    bytes.extend_from_slice(&x.to_le_bytes());
                }
            }
            put(&mut out, &bytes)?;
        }
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES || ![MAGIC, SYNTH_MAGIC, EXTENDED_SYNTH_MAGIC, HYBRID_SYNTH_MAGIC, BAR_RATE_MAGIC]
            .iter().any(|magic| bytes.starts_with(&magic[..]))
        {
            return Err("Not a supported Studio project (expected ORB338S2 through ORB338S6)".into());
        }
        let legacy_bar_grid = !bytes.starts_with(BAR_RATE_MAGIC);
        let mut bytes = &bytes[8..];
        let mut metadata: Metadata = toml::from_str(
            std::str::from_utf8(take(&mut bytes, MAX_METADATA_BYTES)?).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if legacy_bar_grid {
            for slot in &mut metadata.settings.synth_slots {
                slot.migrate_legacy_bar_grid();
            }
        }
        if metadata
            .settings
            .lanes
            .iter()
            .any(|l| l.steps.len() != 1024)
        {
            return Err("Each Studio lane must contain exactly 1024 steps".into());
        }
        metadata.settings.sanitize();
        for bank in &mut metadata.settings.drum_patterns {
            Arc::make_mut(bank).sanitize();
        }
        metadata.sidechain.sanitize();
        metadata.closed_hat = metadata.closed_hat.map(|x| x.min(127));
        let song = take(&mut bytes, 16 * 1024 * 1024)?.to_vec();
        rbs::Song::parse(&song).map_err(|e| format!("Invalid embedded song: {e}"))?;
        let mut assets: [Option<Arc<Wave>>; 9] = std::array::from_fn(|_| None);
        let mut bank_frames = 0;
        for (i, slot) in assets.iter_mut().enumerate() {
            let raw = take(
                &mut bytes,
                if i == 8 {
                    6 * 44100 * 8
                } else {
                    MAX_FRAMES * 8
                },
            )?;
            if !raw.len().is_multiple_of(8) {
                return Err("Invalid embedded audio length".into());
            }
            if raw.is_empty() {
                continue;
            }
            if i < 8 {
                bank_frames += raw.len() / 8;
                validate_bank([bank_frames])?;
            }
            let mut frames = Vec::with_capacity(raw.len() / 8);
            for chunk in raw.as_chunks::<8>().0 {
                let frame = [
                    f32::from_le_bytes(chunk[..4].try_into().unwrap()),
                    f32::from_le_bytes(chunk[4..].try_into().unwrap()),
                ];
                if frame.iter().any(|x| !x.is_finite() || x.abs() > 8.0) {
                    return Err("Invalid embedded audio value".into());
                }
                frames.push(frame.map(|x| if x.abs() < 1e-20 { 0.0 } else { x }));
            }
            *slot = Some(Arc::new(Wave { frames }));
        }
        if !bytes.is_empty() {
            return Err("Unexpected trailing Studio project data".into());
        }
        let ir = assets[8].take().ok_or("Project has no impulse response")?;
        Ok(Self {
            metadata,
            song,
            waves: std::array::from_fn(|i| assets[i].take()),
            ir,
        })
    }

    pub fn prepare(&self) -> Result<Box<Studio>, String> {
        let mut studio = Box::new(Studio::new(Box::new(Convolver::new(&self.ir.frames)?)));
        studio.settings = Box::new(self.metadata.settings.clone());
        for i in 0..8 {
            studio.sampler.waves[i] = self.waves[i].as_ref().map(|w| Box::new((**w).clone()));
        }
        Ok(studio)
    }
}

fn validate_bank(lengths: impl IntoIterator<Item = usize>) -> Result<(), String> {
    let frames = lengths.into_iter().try_fold(0usize, usize::checked_add);
    if frames.is_none_or(|n| n > MAX_AUDIO_BYTES / 8) {
        return Err("Sampler bank exceeds the 240 MiB decoded-audio memory budget".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_orbit_demo_loads_its_four_bar_chord_into_the_runtime() {
        let project = Project::decode(include_bytes!("../../../../assets/demos/Low-Orbit.recall"))
            .unwrap();
        let runtime = project.prepare().unwrap();
        let [bass, pad] = &runtime.settings.synth_slots;
        assert!(!bass.enabled && pad.enabled);
        assert_eq!(pad.steps[0].notes, [50, 57, 65, 76]);
        assert_eq!(pad.steps[0].gate_steps * pad.step_ticks, 4 * 768);
        assert_eq!(pad.step_count(), 16);
    }

    #[test]
    fn experimental_patch_survives_a_project_round_trip_while_its_slot_is_dormant() {
        use orb_engine::studio::acid_presets::EXPERIMENTAL_START;
        let mut project = Project::decode(include_bytes!("../../../../assets/demos/Low-Orbit.recall")).unwrap();
        project.metadata.settings.synth_slots[0].select_factory_preset(EXPERIMENTAL_START);
        let encoded = project.encode().unwrap();
        assert!(encoded.starts_with(HYBRID_SYNTH_MAGIC));
        let decoded = Project::decode(&encoded).unwrap();
        assert_eq!(decoded.metadata.settings.synth_slots, project.metadata.settings.synth_slots);
    }

    #[test]
    fn old_bar_projects_reopen_on_the_original_grid_and_keep_all_hidden_steps() {
        use orb_engine::studio::acid_synth::PhraseLength;
        let mut project = Project::decode(include_bytes!("../../../../assets/demos/Low-Orbit.recall")).unwrap();
        let slot = &mut project.metadata.settings.synth_slots[1];
        slot.set_length(PhraseLength::Bars(128));
        slot.set_step_rate(384); // Older releases stored this Steps-mode rate but played bars at 48.
        slot.step_mut(2047).notes = [50, 57, 60, 65];
        let mut bytes = project.encode().unwrap();
        for magic in [EXTENDED_SYNTH_MAGIC, HYBRID_SYNTH_MAGIC] {
            bytes[..8].copy_from_slice(magic);
            let legacy = Project::decode(&bytes).unwrap();
            let mut restored = Project::decode(&legacy.encode().unwrap()).unwrap();
            let slot = &mut restored.metadata.settings.synth_slots[1];
            assert_eq!((slot.step_count(), slot.ticks_per_step()), (2048, 48));
            assert_eq!(slot.step(2047).notes, [50, 57, 60, 65]);
            slot.set_length(PhraseLength::Steps(16));
            assert_eq!(slot.ticks_per_step(), 384);
            slot.set_length(PhraseLength::Bars(2));
            assert_eq!((slot.step_count(), slot.ticks_per_step()), (4, 384));
            assert!(slot.legacy_128_bars);
            let encoded = restored.encode().unwrap();
            assert!(encoded.starts_with(BAR_RATE_MAGIC));
            let new = Project::decode(&encoded).unwrap();
            assert_eq!(new.metadata.settings.synth_slots, restored.metadata.settings.synth_slots);
        }
    }

    #[test]
    fn bank_budget_counts_all_lanes_without_imposing_a_duration_limit() {
        assert!(validate_bank([44100 * 181, 44100 * 181]).is_ok());
        assert!(validate_bank([MAX_FRAMES]).is_ok());
        assert!(validate_bank([MAX_FRAMES, 1]).is_err());
        assert!(validate_bank([usize::MAX, 1]).is_err());
    }

    #[test]
    fn portable_roundtrip_retains_last_step_and_embedded_audio_and_rejects_corruption() {
        let mut settings = Settings::default();
        settings.lanes[7].bars = 64;
        settings.lanes[7].steps[1023] = 109;
        for (i, bank) in settings.drum_patterns.iter_mut().enumerate() {
            let bank = Arc::make_mut(bank);
            bank.patterns[31].length = if i == 0 { 32 } else { 64 };
            bank.patterns[31].steps[47][11] = 4;
        }
        let project = Project {
            metadata: Metadata {
                settings,
                names: std::array::from_fn(|i| format!("Lane {i}")),
                ir_name: "Test".into(),
                sidechain: Default::default(),
                closed_hat: Some(37),
            },
            song: rbs::Song::empty().write().unwrap(),
            waves: std::array::from_fn(|_| None),
            ir: Arc::new(Wave {
                frames: vec![[0.5, -0.25]; 64],
            }),
        };
        let bytes = project.encode().unwrap();
        assert!(bytes.starts_with(MAGIC));
        let loaded = Project::decode(&bytes).unwrap();
        assert_eq!(loaded.metadata.settings, project.metadata.settings);
        assert_eq!(loaded.ir.frames, project.ir.frames);
        assert_eq!(loaded.metadata.closed_hat, Some(37));
        // Reconstruct the older checksummed container, whose settings predate rack slots.
        let mut sections = &bytes[MAGIC.len()..];
        let mut old_metadata: toml::Table =
            toml::from_str(std::str::from_utf8(take(&mut sections, MAX_BYTES).unwrap()).unwrap())
                .unwrap();
        let old_settings = old_metadata
            .get_mut("settings")
            .unwrap()
            .as_table_mut()
            .unwrap();
        old_settings.remove("synth_slots");
        let mut legacy_bytes = MAGIC.to_vec();
        put(
            &mut legacy_bytes,
            toml::to_string(&old_metadata).unwrap().as_bytes(),
        )
        .unwrap();
        legacy_bytes.extend_from_slice(sections);
        let legacy = Project::decode(&legacy_bytes).unwrap();
        assert!(
            legacy
                .metadata
                .settings
                .synth_slots
                .iter()
                .all(|slot| !slot.enabled)
        );
        assert_eq!(legacy.metadata.settings, project.metadata.settings);
        let mut broken = bytes.clone();
        *broken.last_mut().unwrap() ^= 1;
        assert!(Project::decode(&broken).is_err());
        for len in [0, 7, 8, 19, bytes.len() - 1] {
            assert!(Project::decode(&bytes[..len]).is_err());
        }
    }
}
