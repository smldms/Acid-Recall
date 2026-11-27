//! Factory patches for AcidSynth. Patch values use the same normalized range as the editor.
use super::acid_synth::{ModRoute, Patch};

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
];
