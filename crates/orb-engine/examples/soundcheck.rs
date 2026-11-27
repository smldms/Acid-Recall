//! Render five factory patches through the desktop engine and stereo Studio return.
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::sync::Arc;

use orb_engine::frame::{BLOCK, Devices, Engine};
use orb_engine::studio::acid_presets::PRESETS;
use orb_engine::studio::acid_synth::{PhraseLength, Slot, Step};
use orb_engine::studio::convolution::{Convolver, synthetic};
use orb_engine::studio::Studio;
use orb_engine::tables::Tables;

const RATE: usize = 44_100;
const SECTION: usize = RATE * 8;
const NAMES: [&str; 5] = [
    "Pad / Low Orbit",
    "Experimental / Porcelain Rotor",
    "Experimental / Split Transformer",
    "Experimental / Magnetic Teeth",
    "Experimental / Tectonic Plate",
];

fn score(slot: &mut Slot, cue: usize) {
    slot.length = PhraseLength::Steps(64);
    slot.step_ticks = 48;
    slot.steps = [Step::default(); 16];
    let mut chord = |at, notes, gate_steps| {
        *slot.step_mut(at) = Step { notes, gate_steps };
    };
    match cue {
        0 | 4 => {
            chord(0, [50, 57, 60, 65], 24);
            chord(24, [46, 53, 57, 64], 24);
        }
        1 => {
            for (at, pitch) in [(0, 62), (6, 69), (12, 72), (18, 77), (27, 69), (36, 65), (43, 76)] {
                chord(at, [pitch, 0, 0, 0], 2);
            }
        }
        2 => {
            for bar in [0, 16, 32] {
                for (step, pitch) in [(0, 38), (3, 38), (6, 45), (10, 41), (14, 50)] {
                    chord(bar + step, [pitch, 0, 0, 0], 2);
                }
            }
        }
        _ => {
            for (at, root) in [(0, 50), (6, 50), (14, 53), (22, 48), (30, 53), (38, 50), (45, 57)] {
                chord(at, [root, root + 7, root + 12, 0], 3);
            }
        }
    }
}

fn render(tables: Arc<Tables>, cue: usize) -> io::Result<Vec<f32>> {
    let (index, (_, patch)) = PRESETS.iter().enumerate().find(|(_, (name, _))| *name == NAMES[cue])
        .ok_or_else(|| io::Error::other(format!("Missing factory patch {}", NAMES[cue])))?;
    let convolution = Convolver::new(&synthetic(1, 1.25)).map_err(io::Error::other)?;
    let mut studio = Studio::new(Box::new(convolution));
    let slot = &mut studio.settings.synth_slots[0];
    slot.enabled = true;
    slot.preset = index as u8;
    slot.patch = *patch;
    score(slot, cue);

    let mut engine = Engine::new(tables, Devices::implemented(None));
    engine.frame.studio = Some(Box::new(studio));
    engine.set_tempo(120_000);
    engine.mixer_global(0, 100);
    for row in 0..4 {
        engine.mixer_channel(row, 0, u8::from(row == 0));
    }
    engine.mixer_channel(0, 1, 100);
    engine.mixer_channel(0, 2, 64);
    engine.play();

    let mut samples = Vec::with_capacity(SECTION * 2);
    let (mut left, mut right) = ([0.0; BLOCK], [0.0; BLOCK]);
    for start in (0..SECTION).step_by(BLOCK) {
        engine.frame.block(&mut left, &mut right).map_err(|e| io::Error::other(format!("{e:?}")))?;
        for i in 0..BLOCK.min(SECTION - start) {
            // Let the last bar ring out, then fade the room before the next cue.
            let fade = ((SECTION - 1 - start - i) as f32 / (RATE as f32 * 0.12)).min(1.0);
            samples.extend([left[i] * fade, right[i] * fade]);
        }
    }
    if samples.iter().any(|s| !s.is_finite()) {
        return Err(io::Error::other(format!("Nonfinite output in {}", NAMES[cue])));
    }
    let peak = samples.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
    let rms = (samples.iter().map(|&s| f64::from(s).powi(2)).sum::<f64>() / samples.len() as f64).sqrt();
    if rms < 0.0001 {
        return Err(io::Error::other(format!("Silent cue: {}", NAMES[cue])));
    }
    // Comparable audition levels with peak headroom. Preserve each patch's internal dynamics.
    let gain = (0.11 / rms as f32).min(0.84 / peak);
    for s in &mut samples { *s *= gain; }
    println!("{:02}s  {}  peak {:.3}, RMS {:.3}", cue * 8, NAMES[cue], peak * gain, rms * f64::from(gain));
    Ok(samples)
}

fn write_wav(path: &Path, samples: &[i16]) -> io::Result<()> {
    let bytes = (samples.len() * 2) as u32;
    let mut out = BufWriter::new(File::create(path)?);
    out.write_all(b"RIFF")?;
    out.write_all(&(bytes + 36).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16_u32.to_le_bytes())?;
    out.write_all(&1_u16.to_le_bytes())?;
    out.write_all(&2_u16.to_le_bytes())?;
    out.write_all(&(RATE as u32).to_le_bytes())?;
    out.write_all(&(4 * RATE as u32).to_le_bytes())?;
    out.write_all(&4_u16.to_le_bytes())?;
    out.write_all(&16_u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&bytes.to_le_bytes())?;
    for sample in samples { out.write_all(&sample.to_le_bytes())?; }
    out.flush()
}

fn main() -> io::Result<()> {
    let path = std::env::args_os().nth(1).unwrap_or_else(|| "AcidSynth-Soundcheck.wav".into());
    let tables = Arc::new(Tables::new());
    let mut samples = Vec::with_capacity(SECTION * 2 * NAMES.len());
    for cue in 0..NAMES.len() {
        samples.extend(render(Arc::clone(&tables), cue)?.into_iter().map(|s| (s * 32_767.0).round() as i16));
    }
    write_wav(Path::new(&path), &samples)
}
