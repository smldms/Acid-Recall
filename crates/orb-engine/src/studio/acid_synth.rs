//! Two optional wavetable instruments following the native 192 PPQN transport.
use serde::{Deserialize, Serialize};

use crate::frame::Block;
use std::f32::consts::TAU;

const RATE: f32 = 44100.0;
const TABLE: usize = 512;
const LEVELS: usize = 7;
const VOICES: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    pub wave_a: f32,
    pub wave_b: f32,
    pub detune: f32,
    pub mix: f32,
    pub attack: f32,
    pub release: f32,
    pub cutoff: f32,
    pub resonance: f32,
    pub lfo_rate: f32,
    pub lfo_depth: f32,
}

fn unit(value: f32) -> f32 {
    if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    /// MIDI pitches, with zero denoting an empty chord voice.
    pub notes: [u8; 4],
    pub gate_steps: u8,
}

impl Default for Step {
    fn default() -> Self { Self { notes: [0; 4], gate_steps: 1 } }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Slot {
    pub enabled: bool,
    pub preset: u8,
    pub patch: Patch,
    pub macros: [f32; 4],
    pub steps: [Step; 16],
    pub step_ticks: u16,
}

impl Default for Slot {
    fn default() -> Self {
        let mut steps = [Step::default(); 16];
        steps[0] = Step { notes: [50, 57, 60, 65], gate_steps: 8 };
        steps[8] = Step { notes: [46, 53, 57, 62], gate_steps: 8 };
        Self {
            enabled: false, preset: 0, patch: super::acid_presets::PRESETS[0].1,
            macros: [0.5, 0.35, 0.45, 0.25], steps, step_ticks: 192,
        }
    }
}

impl Slot {
    pub fn sanitize(&mut self) {
        self.macros = self.macros.map(unit);
        self.preset = self.preset.min((super::acid_presets::PRESETS.len() - 1) as u8);
        self.step_ticks = self.step_ticks.clamp(1, 768);
        for step in &mut self.steps {
            step.notes = step.notes.map(|n| n.min(127));
            step.gate_steps = step.gate_steps.clamp(1, 16);
        }
        let p = &mut self.patch;
        for v in [&mut p.wave_a, &mut p.wave_b, &mut p.detune, &mut p.mix,
                  &mut p.attack, &mut p.release, &mut p.cutoff, &mut p.resonance,
                  &mut p.lfo_rate, &mut p.lfo_depth] { *v = unit(*v); }
    }
}

/// Octave-banded additive tables. Allocated and generated only when preparing the Studio.
pub(super) struct Bank(Box<[[[f32; TABLE + 1]; 4]]>);

impl Bank {
    pub fn new() -> Self {
        let mut levels = vec![[[0.0; TABLE + 1]; 4]; LEVELS].into_boxed_slice();
        for (level, shapes) in levels.iter_mut().enumerate() {
            for i in 0..TABLE {
                let phase = TAU * i as f32 / TABLE as f32;
                shapes[0][i] = phase.sin();
                for h in 1..=(1 << level) {
                    let n = h as f32;
                    let sine = (n * phase).sin();
                    if h % 2 == 1 {
                        let sign = if h % 4 == 1 { 1.0 } else { -1.0 };
                        shapes[1][i] += sine * sign * 0.81 / (n * n);
                    }
                    shapes[2][i] += sine * 0.57 / n;
                    let weight = match h { 1 => 0.45, 3 => 0.15, 7 => 0.24,
                        11 => 0.16, 17 => 0.12, _ => 0.0 };
                    shapes[3][i] += sine * weight;
                }
            }
            for shape in shapes { shape[TABLE] = shape[0]; }
        }
        Self(levels)
    }

    fn read(&self, level: usize, phase: f32, shape: f32) -> f32 {
        let x = phase * TABLE as f32;
        let i = x as usize;
        let y = shape.clamp(0.0, 1.0) * 3.0;
        let s = (y as usize).min(2);
        let table = &self.0[level];
        let interp = |shape: usize| {
            table[shape][i] + (table[shape][i + 1] - table[shape][i]) * (x - i as f32)
        };
        let a = interp(s);
        a + (interp(s + 1) - a) * (y - s as f32)
    }
}

#[derive(Default, Clone, Copy)]
struct Voice {
    phase_a: f32,
    phase_b: f32,
    increment: f32,
    level: usize,
    gain: f32,
    end_tick: f64,
}

pub(super) struct Instrument {
    voices: [Voice; VOICES],
    last_step: Option<i64>,
    last_tick: Option<f64>,
    blend: f32,
    lfo: f32,
    low: f32,
    band: f32,
    history: [f32; 512],
    write: usize,
}

impl Default for Instrument {
    fn default() -> Self {
        Self {
            voices: [Voice::default(); VOICES], last_step: None, last_tick: None, blend: 0.0,
            lfo: 0.0, low: 0.0, band: 0.0, history: [0.0; 512], write: 0,
        }
    }
}

impl Instrument {
    fn trigger(&mut self, step: Step, end_tick: f64) {
        let mut reserved = [false; VOICES];
        for (i, &note) in step.notes.iter().enumerate() {
            if note == 0 || step.notes[..i].contains(&note) { continue; }
            let index = self.voices.iter().position(|v| v.increment == 0.0)
                .unwrap_or_else(|| self.voices.iter().enumerate()
                    .filter(|(i, _)| !reserved[*i])
                    .min_by(|(_, a), (_, b)| a.gain.total_cmp(&b.gain)).unwrap().0);
            reserved[index] = true;
            let increment = 440.0 * 2.0_f32.powf((f32::from(note) - 69.0) / 12.0) / RATE;
            let level = ((0.47 / increment).log2().floor() as usize).min(LEVELS - 1);
            self.voices[index] = Voice { increment, level, end_tick, ..Voice::default() };
        }
    }

    /// Return before touching the legacy bus whenever AcidSynth is fully bypassed.
    pub fn render(&mut self, bank: &Bank, slot: &Slot, bus: &mut Block, tick: f64, advance: f64, play: bool) {
        if !slot.enabled && self.blend <= 0.0 { return; }
        if !play || !slot.enabled {
            for v in &mut self.voices { v.end_tick = f64::NEG_INFINITY; }
            self.last_step = None;
            self.last_tick = None;
        } else if self.last_tick.is_some_and(|last| (tick-last).abs() > 2.0 * advance + 0.001) {
            self.voices.fill(Voice::default());
            self.last_step = None;
        }
        let p = slot.patch;
        let [timbre, motion, width, _] = slot.macros;
        let lfo_speed = (0.04 + 6.0 * p.lfo_rate.powi(2)) / RATE;
        let vibrato = (self.lfo * TAU).sin() * (p.lfo_depth * (0.15 + 0.85 * motion));
        let color = (timbre - 0.5) * 0.5 + vibrato * 0.22;
        let detune = 2.0_f32.powf(p.detune * (0.15 + 0.85 * width) * 30.0 / 1200.0);
        let attack = 1.0 / ((0.004 + 3.0 * p.attack.powi(2)) * RATE);
        let release = (-6.91 / ((0.025 + 5.0 * p.release.powi(2)) * RATE)).exp();
        let hz = 70.0 * 2.0_f32.powf(7.0 * (p.cutoff + color * 0.8).clamp(0.0, 1.0));
        // TPT state-variable low-pass; bounded Q at every cutoff.
        let g = (std::f32::consts::PI * hz / RATE).tan();
        let damp = 1.9 - 1.65 * p.resonance;
        let a = 1.0 / (1.0 + g * (g + damp));
        let span = f64::from(slot.step_ticks.max(1));
        for (i, sample) in bus.iter_mut().enumerate() {
            let position = tick + i as f64 * advance;
            if play && slot.enabled {
                let n = (position.max(0.0) / span).floor() as i64;
                if self.last_step.is_none() {
                    // Locate or enable mid-pattern: recover chords whose gates still span this step.
                    for previous in n.saturating_sub(15).max(0)..n {
                        let step = slot.steps[previous as usize % 16];
                        let end = (previous as f64 + f64::from(step.gate_steps)) * span;
                        if end > position { self.trigger(step, end); }
                    }
                }
                if self.last_step != Some(n) {
                    let step = slot.steps[n as usize % 16];
                    self.trigger(step, (n as f64 + f64::from(step.gate_steps)) * span);
                    self.last_step = Some(n);
                }
            }
            let mut sum = 0.0;
            for v in &mut self.voices {
                if v.increment == 0.0 { continue; }
                if position < v.end_tick {
                    v.gain = (v.gain + attack).min(1.0);
                } else {
                    v.gain *= release;
                    if v.gain < 0.00001 { v.increment = 0.0; continue; }
                }
                let a = bank.read(v.level, v.phase_a, p.wave_a + color);
                let b = bank.read(v.level, v.phase_b, p.wave_b - 0.6 * color);
                sum += (a + (b-a) * p.mix) * v.gain * 0.14;
                v.phase_a = (v.phase_a + v.increment).fract();
                v.phase_b = (v.phase_b + v.increment * detune).fract();
            }
            let band = a * (self.band + g * (sum - self.low));
            let low = self.low + g * band;
            self.low = 2.0 * low - self.low;
            self.band = 2.0 * band - self.band;
            let target = if slot.enabled { 1.0 } else { 0.0 };
            self.blend += (target - self.blend).clamp(-1.0/256.0, 1.0/256.0);
            *sample += (low - *sample) * self.blend;
        }
        if play && slot.enabled { self.last_tick = Some(tick + bus.len() as f64 * advance); }
        self.lfo = (self.lfo + lfo_speed * bus.len() as f32).fract();
        if !slot.enabled && self.blend == 0.0 { *self = Self::default(); }
    }

    /// A delayed difference leaves the mid (mono sum) intact, after the channel inserts.
    pub fn spread(&mut self, sample: f32, width: f32) -> f64 {
        let old = self.history[(self.write + 512 - 389) & 511];
        let recent = self.history[(self.write + 512 - 173) & 511];
        self.history[self.write] = sample;
        self.write = (self.write + 1) & 511;
        f64::from((recent-old) * width * self.blend * 0.22)
    }

    pub fn gate(&self, tick: f64) -> bool {
        self.voices.iter().any(|v| v.increment != 0.0 && tick < v.end_tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bypass_keeps_native_bus_then_chases_a_held_chord_on_enable() {
        let bank = Bank::new();
        let mut instrument = Instrument::default();
        let mut slot = Slot::default();
        let mut bus = [0.125; 64];
        instrument.render(&bank, &slot, &mut bus, 384.0, 0.01, true);
        assert_eq!(bus, [0.125; 64]);
        slot.enabled = true;
        slot.patch.attack = 0.0;
        for i in 0..16 {
            instrument.render(&bank, &slot, &mut bus, 384.0 + f64::from(i)*0.64, 0.01, true);
        }
        assert_eq!(instrument.voices.iter().filter(|v| v.increment > 0.0).count(), 4);
        assert!(bus.iter().all(|v| v.is_finite()));
        assert!(bus.iter().any(|v| v.abs() > 0.001));
        instrument.render(&bank, &slot, &mut bus, 394.24, 0.01, false);
        assert!(!instrument.gate(394.24));
    }

    #[test]
    fn stealing_voices_keeps_every_note_of_the_new_chord() {
        let mut synth = Instrument::default();
        for voice in &mut synth.voices {
            voice.increment = 0.01;
            voice.gain = 0.8;
        }
        synth.trigger(Step { notes: [48, 52, 55, 59], gate_steps: 1 }, 192.0);
        assert_eq!(synth.voices.iter().filter(|v| v.end_tick == 192.0).count(), 4);
    }

    #[test]
    fn sanitizes_untrusted_project_values() {
        let mut slot = Slot::default();
        slot.patch.detune = f32::NAN;
        slot.macros = [2.0; 4];
        slot.steps[0] = Step { notes: [255; 4], gate_steps: 0 };
        slot.step_ticks = 0;
        slot.sanitize();
        assert_eq!(slot.patch.detune, 0.0);
        assert_eq!(slot.macros, [1.0; 4]);
        assert_eq!(slot.steps[0].notes, [127; 4]);
        assert_eq!(slot.steps[0].gate_steps, 1);
        assert_eq!(slot.step_ticks, 1);
    }
}
