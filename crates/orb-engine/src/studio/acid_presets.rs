//! Factory patches for AcidSynth. Patch values use the same normalized range as the editor.
use super::acid_synth::{ModRoute, Patch};

/// First index in the experimental bank; older project and MIDI preset numbers stay stable.
pub const EXPERIMENTAL_START: usize = 24;

/// Stable display order for the patch browser.
pub const PRESETS: &[(&str, Patch)] = &[
    (
        "Pad / Low Orbit",
        Patch {
            wave_a: 0.18,
            wave_b: 0.49,
            detune: 0.30,
            mix: 0.44,
            attack: 0.54,
            release: 0.67,
            cutoff: 0.42,
            resonance: 0.19,
            lfo_rate: 0.19,
            lfo_depth: 0.22,
            ..Patch::INIT
        },
    ),
    (
        "Pad / Tidal Glass",
        Patch {
            wave_a: 0.70,
            wave_b: 0.13,
            detune: 0.41,
            mix: 0.64,
            attack: 0.61,
            release: 0.78,
            cutoff: 0.62,
            resonance: 0.26,
            lfo_rate: 0.14,
            lfo_depth: 0.37,
            ..Patch::INIT
        },
    ),
    (
        "Pad / Carbon Veil",
        Patch {
            wave_a: 0.86,
            wave_b: 0.61,
            detune: 0.15,
            mix: 0.28,
            attack: 0.72,
            release: 0.59,
            cutoff: 0.28,
            resonance: 0.32,
            lfo_rate: 0.22,
            lfo_depth: 0.19,
            ..Patch::INIT
        },
    ),
    (
        "Pad / First Light",
        Patch {
            wave_a: 0.36,
            wave_b: 0.80,
            detune: 0.48,
            mix: 0.35,
            attack: 0.44,
            release: 0.71,
            cutoff: 0.67,
            resonance: 0.17,
            lfo_rate: 0.28,
            lfo_depth: 0.24,
            ..Patch::INIT
        },
    ),
    (
        "Pad / Meltwater",
        Patch {
            wave_a: 0.10,
            wave_b: 0.57,
            detune: 0.56,
            mix: 0.57,
            attack: 0.66,
            release: 0.88,
            cutoff: 0.53,
            resonance: 0.35,
            lfo_rate: 0.08,
            lfo_depth: 0.51,
            ..Patch::INIT
        },
    ),
    (
        "Dub / Tape Return",
        Patch {
            wave_a: 0.59,
            wave_b: 0.91,
            detune: 0.20,
            mix: 0.32,
            attack: 0.06,
            release: 0.45,
            cutoff: 0.32,
            resonance: 0.46,
            lfo_rate: 0.19,
            lfo_depth: 0.13,
            ..Patch::INIT
        },
    ),
    (
        "Dub / Transit Chord",
        Patch {
            wave_a: 0.77,
            wave_b: 0.40,
            detune: 0.28,
            mix: 0.51,
            attack: 0.03,
            release: 0.27,
            cutoff: 0.47,
            resonance: 0.34,
            lfo_rate: 0.30,
            lfo_depth: 0.18,
            ..Patch::INIT
        },
    ),
    (
        "Dub / Cold Room",
        Patch {
            wave_a: 0.29,
            wave_b: 0.88,
            detune: 0.37,
            mix: 0.60,
            attack: 0.12,
            release: 0.60,
            cutoff: 0.40,
            resonance: 0.56,
            lfo_rate: 0.11,
            lfo_depth: 0.29,
            ..Patch::INIT
        },
    ),
    (
        "Dub / Shutter",
        Patch {
            wave_a: 0.93,
            wave_b: 0.58,
            detune: 0.10,
            mix: 0.40,
            attack: 0.01,
            release: 0.18,
            cutoff: 0.60,
            resonance: 0.49,
            lfo_rate: 0.65,
            lfo_depth: 0.42,
            ..Patch::INIT
        },
    ),
    (
        "Drone / Iron Sleep",
        Patch {
            wave_a: 0.82,
            wave_b: 0.97,
            detune: 0.62,
            mix: 0.46,
            attack: 0.82,
            release: 0.74,
            cutoff: 0.30,
            resonance: 0.39,
            lfo_rate: 0.09,
            lfo_depth: 0.46,
            ..Patch::INIT
        },
    ),
    (
        "Drone / Undertow",
        Patch {
            wave_a: 0.07,
            wave_b: 0.37,
            detune: 0.23,
            mix: 0.39,
            attack: 0.90,
            release: 0.85,
            cutoff: 0.25,
            resonance: 0.24,
            lfo_rate: 0.06,
            lfo_depth: 0.50,
            ..Patch::INIT
        },
    ),
    (
        "Drone / Wire Choir",
        Patch {
            wave_a: 0.96,
            wave_b: 0.44,
            detune: 0.72,
            mix: 0.56,
            attack: 0.52,
            release: 0.69,
            cutoff: 0.59,
            resonance: 0.43,
            lfo_rate: 0.17,
            lfo_depth: 0.34,
            ..Patch::INIT
        },
    ),
    (
        "Drone / Relay Tower",
        Patch {
            wave_a: 0.44,
            wave_b: 0.72,
            detune: 0.17,
            mix: 0.67,
            attack: 0.37,
            release: 0.58,
            cutoff: 0.46,
            resonance: 0.57,
            lfo_rate: 0.47,
            lfo_depth: 0.67,
            ..Patch::INIT
        },
    ),
    (
        "Stab / Flash Memory",
        Patch {
            wave_a: 0.80,
            wave_b: 0.51,
            detune: 0.36,
            mix: 0.35,
            attack: 0.00,
            release: 0.14,
            cutoff: 0.71,
            resonance: 0.36,
            lfo_rate: 0.29,
            lfo_depth: 0.15,
            ..Patch::INIT
        },
    ),
    (
        "Stab / Rivet",
        Patch {
            wave_a: 0.96,
            wave_b: 0.24,
            detune: 0.09,
            mix: 0.24,
            attack: 0.01,
            release: 0.07,
            cutoff: 0.53,
            resonance: 0.54,
            lfo_rate: 0.56,
            lfo_depth: 0.21,
            ..Patch::INIT
        },
    ),
    (
        "Stab / Afterimage",
        Patch {
            wave_a: 0.54,
            wave_b: 0.83,
            detune: 0.45,
            mix: 0.61,
            attack: 0.04,
            release: 0.32,
            cutoff: 0.65,
            resonance: 0.25,
            lfo_rate: 0.33,
            lfo_depth: 0.27,
            ..Patch::INIT
        },
    ),
    (
        "Keys / Frost Line",
        Patch {
            wave_a: 0.03,
            wave_b: 0.42,
            detune: 0.21,
            mix: 0.27,
            attack: 0.03,
            release: 0.36,
            cutoff: 0.76,
            resonance: 0.15,
            lfo_rate: 0.20,
            lfo_depth: 0.10,
            ..Patch::INIT
        },
    ),
    (
        "Keys / Blue Relay",
        Patch {
            wave_a: 0.31,
            wave_b: 0.71,
            detune: 0.34,
            mix: 0.33,
            attack: 0.10,
            release: 0.48,
            cutoff: 0.55,
            resonance: 0.30,
            lfo_rate: 0.39,
            lfo_depth: 0.24,
            ..Patch::INIT
        },
    ),
    (
        "Bass / Pressure Drop",
        Patch {
            wave_a: 0.17,
            wave_b: 0.68,
            detune: 0.05,
            mix: 0.24,
            attack: 0.01,
            release: 0.09,
            cutoff: 0.34,
            resonance: 0.29,
            lfo_rate: 0.13,
            lfo_depth: 0.07,
            ..Patch::INIT
        },
    ),
    (
        "Bass / Moving Floor",
        Patch {
            wave_a: 0.67,
            wave_b: 0.92,
            detune: 0.13,
            mix: 0.41,
            attack: 0.02,
            release: 0.16,
            cutoff: 0.43,
            resonance: 0.50,
            lfo_rate: 0.46,
            lfo_depth: 0.34,
            ..Patch::INIT
        },
    ),
    ("Pad / Ion Cathedral", Patch {
        wave_a: 0.63, wave_b: 0.91, osc_mode: 2, transpose_b: 7, mix: 0.32,
        attack: 0.51, decay: 0.75, sustain: 0.76, release: 0.72,
        cutoff: 0.56, filter_env: 0.38, chorus: 0.77,
        routes: [
            ModRoute { source: 1, dest: 1, amount: 0.16 },
            ModRoute { source: 2, dest: 3, amount: -0.14 },
            ModRoute::OFF, ModRoute::OFF,
        ],
        ..Patch::INIT
    }),
    ("Stab / Copper Contacts", Patch {
        wave_a: 0.69, wave_b: 0.41, osc_mode: 1, transpose_b: 12, mix: 0.57,
        attack: 0.01, decay: 0.32, sustain: 0.25, release: 0.24,
        cutoff: 0.32, filter_env: 0.82, resonance: 0.58, drive: 0.27, ..Patch::INIT
    }),
    ("Drone / Polar Current", Patch {
        wave_a: 0.64, wave_b: 0.89, detune: 0.47, mix: 0.54, transpose_b: -12,
        attack: 0.56, release: 0.77, cutoff: 0.49, filter_mode: 3,
        lfo_target: 1, lfo_shape: 1, lfo_rate: 0.11, lfo_depth: 0.88, chorus: 0.42,
        ..Patch::INIT
    }),
    ("Bass / Relay Shunt", Patch {
        wave_a: 0.64, wave_b: 0.99, sub: 0.43, mix: 0.2, drive: 0.5,
        attack: 0.01, decay: 0.26, sustain: 0.42, release: 0.19,
        cutoff: 0.23, filter_env: 0.86, resonance: 0.34, ..Patch::INIT
    }),
    ("Experimental / Porcelain Rotor", Patch {
        osc_mode: 3,
        fm_algorithm: 0,
        fm_ratios: [2.0, 3.02, 7.0],
        fm_index: 0.2,
        fm_feedback: 0.06,
        mix: 0.58,
        attack: 0.01,
        decay: 0.3,
        sustain: 0.13,
        release: 0.39,
        cutoff: 0.63,
        filter_env: 0.22,
        lfo_rate: 0.2,
        routes: [
            ModRoute { source: 2, dest: 0, amount: 0.27 },
            ModRoute { source: 3, dest: 0, amount: 0.16 },
            ModRoute { source: 1, dest: 1, amount: 0.04 },
            ModRoute::OFF,
        ],
        ..Patch::INIT
    }),
    ("Experimental / Glass Repeater", Patch {
        osc_mode: 3,
        fm_algorithm: 1,
        fm_ratios: [1.5, 2.0, 4.01],
        fm_index: 0.24,
        fm_feedback: 0.09,
        mix: 0.62,
        attack: 0.04,
        decay: 0.26,
        sustain: 0.38,
        release: 0.35,
        cutoff: 0.49,
        chorus: 0.35,
        lfo_rate: 0.44,
        routes: [
            ModRoute { source: 1, dest: 0, amount: 0.13 },
            ModRoute { source: 2, dest: 2, amount: 0.16 },
            ModRoute { source: 4, dest: 1, amount: 0.12 },
            ModRoute::OFF,
        ],
        ..Patch::INIT
    }),
    ("Experimental / Bent Antenna", Patch {
        osc_mode: 3,
        fm_algorithm: 2,
        fm_ratios: [1.37, 2.71, 4.19],
        fm_index: 0.22,
        fm_feedback: 0.11,
        mix: 0.53,
        attack: 0.48,
        decay: 0.65,
        sustain: 0.72,
        release: 0.61,
        cutoff: 0.53,
        filter_mode: 3,
        lfo_rate: 0.18,
        chorus: 0.28,
        routes: [
            ModRoute { source: 1, dest: 0, amount: 0.11 },
            ModRoute { source: 4, dest: 4, amount: 0.07 },
            ModRoute { source: 6, dest: 1, amount: 0.14 },
            ModRoute::OFF,
        ],
        ..Patch::INIT
    }),
    ("Experimental / Split Transformer", Patch {
        osc_mode: 3,
        fm_algorithm: 3,
        fm_ratios: [0.5, 1.0, 3.0],
        fm_index: 0.27,
        fm_feedback: 0.12,
        mix: 0.48,
        attack: 0.01,
        decay: 0.24,
        sustain: 0.4,
        release: 0.12,
        cutoff: 0.38,
        sub: 0.21,
        drive: 0.12,
        lfo_rate: 0.23,
        routes: [
            ModRoute { source: 2, dest: 0, amount: 0.32 },
            ModRoute { source: 3, dest: 1, amount: 0.14 },
            ModRoute { source: 4, dest: 2, amount: 0.19 },
            ModRoute::OFF,
        ],
        ..Patch::INIT
    }),
    ("Experimental / Lithium Choir", Patch {
        osc_mode: 3,
        fm_algorithm: 1,
        fm_ratios: [1.002, 2.005, 3.99],
        fm_index: 0.13,
        fm_feedback: 0.04,
        mix: 0.58,
        attack: 0.53,
        decay: 0.61,
        sustain: 0.8,
        release: 0.73,
        cutoff: 0.58,
        chorus: 0.49,
        lfo_rate: 0.12,
        routes: [
            ModRoute { source: 1, dest: 0, amount: 0.09 },
            ModRoute { source: 5, dest: 1, amount: 0.15 },
            ModRoute { source: 6, dest: 2, amount: -0.1 },
            ModRoute::OFF,
        ],
        ..Patch::INIT
    }),
    ("Experimental / Broken Escapement", Patch {
        osc_mode: 3,
        fm_algorithm: 0,
        fm_ratios: [2.49, 4.0, 9.03],
        fm_index: 0.32,
        fm_feedback: 0.13,
        mix: 0.64,
        attack: 0.01,
        decay: 0.19,
        sustain: 0.09,
        release: 0.12,
        cutoff: 0.57,
        filter_env: 0.2,
        lfo_rate: 0.48,
        lfo_shape: 3,
        routes: [
            ModRoute { source: 2, dest: 0, amount: 0.38 },
            ModRoute { source: 1, dest: 1, amount: 0.07 },
            ModRoute { source: 3, dest: 5, amount: 0.13 },
            ModRoute::OFF,
        ],
        ..Patch::INIT
    }),
    ("Experimental / Sheared Ribbon", Patch {
        osc_mode: 4,
        sync: 0.25,
        fold: 0.26,
        wave_a: 0.3,
        wave_b: 0.7,
        mix: 0.57,
        attack: 0.33,
        decay: 0.57,
        sustain: 0.69,
        release: 0.51,
        cutoff: 0.49,
        chorus: 0.38,
        lfo_rate: 0.19,
        routes: [
            ModRoute { source: 1, dest: 0, amount: 0.16 },
            ModRoute { source: 3, dest: 1, amount: 0.17 },
            ModRoute { source: 4, dest: 2, amount: 0.11 },
            ModRoute::OFF,
        ],
        ..Patch::INIT
    }),
    ("Experimental / Coil Spring", Patch {
        osc_mode: 4,
        sync: 0.34,
        fold: 0.2,
        wave_a: 0.59,
        wave_b: 0.31,
        mix: 0.64,
        attack: 0.01,
        decay: 0.27,
        sustain: 0.19,
        release: 0.24,
        cutoff: 0.47,
        filter_env: 0.42,
        resonance: 0.32,
        lfo_rate: 0.34,
        routes: [
            ModRoute { source: 2, dest: 1, amount: 0.33 },
            ModRoute { source: 2, dest: 0, amount: 0.15 },
            ModRoute { source: 3, dest: 5, amount: 0.13 },
            ModRoute::OFF,
        ],
        ..Patch::INIT
    }),
    ("Experimental / Magnetic Teeth", Patch {
        osc_mode: 4,
        sync: 0.41,
        fold: 0.31,
        wave_a: 0.69,
        wave_b: 0.9,
        mix: 0.53,
        attack: 0.06,
        decay: 0.35,
        sustain: 0.48,
        release: 0.28,
        cutoff: 0.4,
        filter_mode: 1,
        resonance: 0.2,
        lfo_rate: 0.45,
        lfo_shape: 2,
        routes: [
            ModRoute { source: 1, dest: 1, amount: 0.14 },
            ModRoute { source: 4, dest: 0, amount: 0.22 },
            ModRoute { source: 3, dest: 2, amount: 0.11 },
            ModRoute::OFF,
        ],
        ..Patch::INIT
    }),
    ("Experimental / Tectonic Plate", Patch {
        osc_mode: 4,
        sync: 0.16,
        fold: 0.23,
        wave_a: 0.36,
        wave_b: 0.68,
        mix: 0.45,
        attack: 0.68,
        decay: 0.81,
        sustain: 0.73,
        release: 0.74,
        cutoff: 0.32,
        sub: 0.18,
        chorus: 0.18,
        lfo_rate: 0.08,
        routes: [
            ModRoute { source: 1, dest: 1, amount: 0.1 },
            ModRoute { source: 1, dest: 0, amount: -0.12 },
            ModRoute { source: 6, dest: 2, amount: 0.12 },
            ModRoute::OFF,
        ],
        ..Patch::INIT
    }),
    ("Experimental / Fault Current", Patch {
        osc_mode: 4,
        sync: 0.29,
        fold: 0.35,
        wave_a: 0.75,
        wave_b: 0.43,
        mix: 0.47,
        attack: 0.01,
        decay: 0.29,
        sustain: 0.46,
        release: 0.13,
        cutoff: 0.3,
        sub: 0.24,
        drive: 0.12,
        lfo_rate: 0.32,
        routes: [
            ModRoute { source: 2, dest: 1, amount: 0.24 },
            ModRoute { source: 3, dest: 0, amount: 0.2 },
            ModRoute { source: 4, dest: 5, amount: 0.13 },
            ModRoute::OFF,
        ],
        ..Patch::INIT
    }),
    ("Experimental / Phosphor Gate", Patch {
        osc_mode: 4,
        sync: 0.38,
        fold: 0.22,
        wave_a: 0.41,
        wave_b: 0.79,
        mix: 0.62,
        attack: 0.13,
        decay: 0.45,
        sustain: 0.62,
        release: 0.42,
        cutoff: 0.51,
        filter_mode: 3,
        lfo_rate: 0.52,
        lfo_shape: 3,
        chorus: 0.28,
        routes: [
            ModRoute { source: 1, dest: 1, amount: 0.12 },
            ModRoute { source: 4, dest: 0, amount: 0.19 },
            ModRoute { source: 5, dest: 2, amount: -0.1 },
            ModRoute::OFF,
        ],
        ..Patch::INIT
    }),
];

impl super::acid_synth::Slot {
    /// Replace only the sound. Macro positions and the playing phrase belong to the slot.
    pub fn select_factory_preset(&mut self, index: usize) {
        if let Some((_, patch)) = PRESETS.get(index) {
            self.preset = index as u8;
            self.patch = *patch;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::acid_synth::Slot;

    #[test]
    fn every_experimental_sound_survives_project_validation() {
        assert_eq!(PRESETS[EXPERIMENTAL_START - 1].0, "Bass / Relay Shunt");
        assert_eq!(PRESETS.len() - EXPERIMENTAL_START, 12);
        for (index, (name, patch)) in PRESETS.iter().enumerate().skip(EXPERIMENTAL_START) {
            assert!(name.starts_with("Experimental / "));
            assert!(PRESETS[..index].iter().all(|(n, p)| n != name && p != patch));
            let mut slot = Slot::default();
            let macros = slot.macros;
            let step = slot.step(0);
            slot.select_factory_preset(index);
            slot.sanitize();
            assert_eq!(&slot.patch, patch, "{name}");
            assert_eq!(slot.preset as usize, index);
            assert_eq!(slot.macros, macros);
            assert_eq!(slot.step(0), step);
            assert!(patch.routes.iter().filter(|r| r.source != 0).count() >= 2);
        }
    }

    #[test]
    fn experimental_chords_render_with_headroom() {
        use crate::studio::acid_synth::{Bank, Instrument};
        let bank = Bank::new();
        for (name, patch) in &PRESETS[EXPERIMENTAL_START..] {
            let mut slot = Slot::default();
            slot.enabled = true;
            slot.patch = *patch;
            let mut synth = Instrument::default();
            let mut peak = 0.0f32;
            for block in 0..384 {
                let mut output = [0.0; 64];
                synth.render(&bank, &slot, &mut output, f64::from(block) * 0.64, 0.01, true);
                for sample in output {
                    assert!(sample.is_finite(), "{name}");
                    peak = peak.max(sample.abs());
                }
            }
            assert!((0.001..1.0).contains(&peak), "{name}: peak {peak}");
        }
    }

    #[test]
    fn missing_factory_sound_does_not_change_slot() {
        let mut slot = Slot::default();
        let original = slot.clone();
        slot.select_factory_preset(usize::MAX);
        assert_eq!(slot, original);
    }
}
