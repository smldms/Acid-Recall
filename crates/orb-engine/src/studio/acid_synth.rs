//! Two optional wavetable instruments following the native 192 PPQN transport.
use serde::{Deserialize, Serialize};

use crate::frame::Block;
use std::f32::consts::TAU;
#[path = "acid_models.rs"]
mod models;
#[path = "acid_phrase.rs"]
mod phrase;

const RATE: f32 = 44100.0;
const TABLE: usize = 512;
const LEVELS: usize = 7;
pub(super) const VOICES: usize = 12;

/// One LFO cycle in native sequencer ticks (192 per quarter). Dotted/triplet values stay exact.
pub const LFO_DIVISIONS: &[(&str, u16)] = &[
    ("16 bars", 12288), ("8 bars", 6144), ("4 bars", 3072), ("2 bars", 1536),
    ("1 bar", 768), ("1/2 dotted", 576), ("1/2", 384), ("1/4 dotted", 288),
    ("1/2 triplet", 256), ("1/4", 192), ("1/8 dotted", 144), ("1/4 triplet", 128),
    ("1/8", 96), ("1/16 dotted", 72), ("1/8 triplet", 64), ("1/16", 48),
    ("1/32 dotted", 36), ("1/16 triplet", 32), ("1/32", 24), ("1/32 triplet", 16),
];

/// One bipolar patch cable. Source zero disconnects the cable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModRoute {
    pub source: u8,
    pub dest: u8,
    pub amount: f32,
}

impl ModRoute {
    pub const OFF: Self = Self { source: 0, dest: 0, amount: 0.0 };
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
    /// 0 = free Hz, 1 = one cycle per selected musical division.
    pub lfo_sync: u8,
    pub lfo_division: u8,
    pub lfo_depth: f32,
    pub decay: f32,
    pub sustain: f32,
    /// Positive filter sweep following the loudest voice's amplitude envelope.
    pub filter_env: f32,
    pub sub: f32,
    pub drive: f32,
    pub chorus: f32,
    /// Blend, ring modulation, two-oscillator phase modulation, FM4 or Sync Fold.
    pub osc_mode: u8,
    /// Cascade, parallel pairs, fan-in or split stack.
    pub fm_algorithm: u8,
    /// Frequency ratios of the second, third and fourth operators.
    pub fm_ratios: [f32; 3],
    pub fm_index: f32,
    pub fm_feedback: f32,
    pub fold: f32,
    pub sync: f32,
    /// Low-pass, band-pass, high-pass or notch.
    pub filter_mode: u8,
    /// Colour, cutoff, pitch or oscillator mix.
    pub lfo_target: u8,
    /// Sine, triangle, rising saw or square.
    pub lfo_shape: u8,
    pub transpose_b: i8,
    pub routes: [ModRoute; 4],
}

impl Patch {
    pub fn lfo_hz(&self) -> f32 {
        0.04 + 6.0 * self.lfo_rate.powi(2)
    }

    pub fn lfo_period(&self) -> (&'static str, u16) {
        LFO_DIVISIONS[usize::from(self.lfo_division).min(LFO_DIVISIONS.len() - 1)]
    }

    /// Defaults for old projects, leaving the additional stages bypassed.
    pub const INIT: Self = Self {
        wave_a: 0.0, wave_b: 0.5, detune: 0.0, mix: 0.5,
        attack: 0.05, release: 0.25, cutoff: 0.75, resonance: 0.0,
        lfo_rate: 0.25, lfo_sync: 0, lfo_division: 9, lfo_depth: 0.0, decay: 0.0, sustain: 1.0,
        filter_env: 0.0, sub: 0.0, drive: 0.0, chorus: 0.0,
        fm_algorithm: 0, fm_ratios: [2.0, 3.0, 4.0],
        fm_index: 0.45, fm_feedback: 0.1, fold: 0.42, sync: 0.45,
        osc_mode: 0, filter_mode: 0, lfo_target: 0, lfo_shape: 0, transpose_b: 0, routes: [ModRoute::OFF; 4],
    };
}

impl Default for Patch {
    fn default() -> Self { Self::INIT }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    /// MIDI pitches, with zero denoting an empty chord voice.
    pub notes: [u8; 4],
    pub gate_steps: u16,
}

impl Default for Step {
    fn default() -> Self { Self { notes: [0; 4], gate_steps: 1 } }
}

pub const MAX_STEPS: usize = 128 * 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhraseLength {
    Steps(u16),
    Bars(u16),
}

impl Default for PhraseLength {
    fn default() -> Self { Self::Steps(16) }
}

impl PhraseLength {
    fn valid(self) -> Self {
        match self {
            Self::Bars(1 | 2 | 4 | 8 | 16 | 32 | 64 | 128)
            | Self::Steps(16 | 32 | 64 | 128) => self,
            _ => Self::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Slot {
    pub enabled: bool,
    /// Missing in projects where choosing AcidSynth replaced the native TB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tb_enabled: Option<bool>,
    /// Which panel is visible. Editing a muted instrument does not enable it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_synth: Option<bool>,
    /// While off, this slot responds only to live MIDI and audition events.
    pub sequence_enabled: bool,
    pub preset: u8,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub user_preset: String,
    pub patch: Patch,
    pub macros: [f32; 4],
    pub steps: [Step; 16],
    pub step_ticks: u16,
    pub length: PhraseLength,
    /// Original bar patterns always played at 48 ticks, even if their Steps rate was different.
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy_bar_ticks: Option<u16>,
    /// Keep 128 bars accessible when revisiting an old phrase after temporarily shortening it.
    pub legacy_128_bars: bool,
    /// Steps after the first page. Only edited pages consume space in a saved project.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tail: Vec<Step>,
    #[serde(skip)]
    prepared: phrase::Prepared,
}

impl Default for Slot {
    fn default() -> Self {
        let mut steps = [Step::default(); 16];
        steps[0] = Step { notes: [50, 57, 60, 65], gate_steps: 8 };
        steps[8] = Step { notes: [46, 53, 57, 62], gate_steps: 8 };
        Self {
            enabled: false, tb_enabled: None, edit_synth: None, sequence_enabled: true,
            preset: 0, user_preset: String::new(), patch: super::acid_presets::PRESETS[0].1,
            macros: [0.5, 0.35, 0.45, 0.25], steps, step_ticks: 192,
            length: PhraseLength::default(), legacy_bar_ticks: None, legacy_128_bars: false,
            tail: Vec::new(), prepared: Default::default(),
        }
    }
}

impl Slot {
    pub(super) fn prepare_phrase(&mut self) { self.prepared = phrase::Prepared::build(self); }

    pub(super) fn phrase_fingerprint(&self) -> Option<u64> { self.prepared.fingerprint() }

    pub fn tb_active(&self) -> bool { self.tb_enabled.unwrap_or(!self.enabled) }

    pub fn editing_synth(&self) -> bool { self.edit_synth.unwrap_or(self.enabled) }

    pub fn set_synth_active(&mut self, enabled: bool) {
        self.tb_enabled = Some(self.tb_active());
        self.edit_synth = Some(self.editing_synth());
        self.enabled = enabled;
    }

    pub fn step_count(&self) -> usize {
        match self.length.valid() {
            PhraseLength::Steps(n) => usize::from(n),
            PhraseLength::Bars(n) => usize::from(n) * 768 / usize::from(self.ticks_per_step()),
        }
    }

    pub fn ticks_per_step(&self) -> u16 {
        match self.length.valid() {
            PhraseLength::Steps(_) => self.step_ticks.clamp(1, 768),
            PhraseLength::Bars(_) => {
                let ticks = self.legacy_bar_ticks.unwrap_or(self.step_ticks);
                [48u16, 96, 192, 384, 768]
                    .into_iter()
                    .min_by_key(|rate| rate.abs_diff(ticks))
                    .unwrap_or(48)
            }
        }
    }

    pub fn migrate_legacy_bar_grid(&mut self) {
        self.legacy_128_bars |= self.length == PhraseLength::Bars(128)
            || self.tail.len() + 16 > 64 * 16;
        if matches!(self.length, PhraseLength::Bars(_)) && self.step_ticks != 48 {
            self.legacy_bar_ticks = Some(48);
        }
    }

    pub fn set_step_rate(&mut self, ticks: u16) {
        self.prepared = Default::default();
        self.step_ticks = ticks;
        self.legacy_bar_ticks = None;
    }

    pub fn set_length(&mut self, length: PhraseLength) {
        self.prepared = Default::default();
        self.legacy_128_bars |= self.length == PhraseLength::Bars(128);
        if matches!(length, PhraseLength::Steps(_)) {
            self.legacy_bar_ticks = None;
        }
        self.length = length;
    }

    pub fn step(&self, index: usize) -> Step {
        if index < 16 {
            self.steps[index]
        } else {
            self.tail.get(index - 16).copied().unwrap_or_default()
        }
    }

    /// Editor-side only. The audio thread exclusively uses the non-allocating reader.
    pub fn step_mut(&mut self, index: usize) -> &mut Step {
        self.prepared = Default::default();
        assert!(index < MAX_STEPS);
        if index < 16 {
            return &mut self.steps[index];
        }
        let offset = index - 16;
        if self.tail.len() <= offset {
            self.tail.resize(offset + 1, Step::default());
        }
        &mut self.tail[offset]
    }

    pub fn sanitize(&mut self) {
        self.sanitize_sound();
        self.step_ticks = self.step_ticks.clamp(1, 768);
        self.length = self.length.valid();
        self.tail.truncate(MAX_STEPS - 16);
        for step in self.steps.iter_mut().chain(&mut self.tail) {
            step.notes = step.notes.map(|n| n.min(127));
            step.gate_steps = step.gate_steps.clamp(1, MAX_STEPS as u16);
        }
    }

    /// Validate patch controls without traversing the stored phrase during live sound edits.
    pub fn sanitize_sound(&mut self) {
        self.macros = self.macros.map(unit);
        self.preset = self.preset.min((super::acid_presets::PRESETS.len() - 1) as u8);
        let p = &mut self.patch;
        for v in [&mut p.wave_a, &mut p.wave_b, &mut p.detune, &mut p.mix,
                  &mut p.attack, &mut p.release, &mut p.cutoff, &mut p.resonance,
                  &mut p.lfo_rate, &mut p.lfo_depth, &mut p.decay, &mut p.sustain,
                  &mut p.filter_env, &mut p.sub, &mut p.drive, &mut p.chorus] { *v = unit(*v); }
        for route in &mut p.routes {
            if route.source > 6 || route.dest > 5 || !route.amount.is_finite() {
                *route = ModRoute::OFF;
            } else {
                route.amount = route.amount.clamp(-1.0, 1.0);
            }
        }
        p.transpose_b = p.transpose_b.clamp(-24, 24);
        p.osc_mode = p.osc_mode.min(4);
        p.fm_algorithm = p.fm_algorithm.min(3);
        for ratio in &mut p.fm_ratios {
            *ratio = if ratio.is_finite() { ratio.clamp(0.25, 12.0) } else { 1.0 };
        }
        p.fm_index = unit(p.fm_index);
        p.fm_feedback = unit(p.fm_feedback);
        p.fold = unit(p.fold);
        p.sync = unit(p.sync);
        p.filter_mode = p.filter_mode.min(3);
        p.lfo_target = p.lfo_target.min(3);
        p.lfo_sync = p.lfo_sync.min(1);
        p.lfo_division = p.lfo_division.min((LFO_DIVISIONS.len() - 1) as u8);
        p.lfo_shape = p.lfo_shape.min(3);
    }
}

/// Octave-banded additive tables. Allocated and generated only when preparing the Studio.
pub(super) struct Bank {
    tables: Box<[[[f32; TABLE + 1]; 4]]>,
    pitches: [(f32, usize); 128],
}

#[derive(Clone, Copy)]
struct Shape {
    index: usize,
    blend: f32,
}

impl Shape {
    fn new(value: f32) -> Self {
        let y = value.clamp(0.0, 1.0) * 3.0;
        let index = (y as usize).min(2);
        Self { index, blend: y - index as f32 }
    }
}

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
        let pitches = std::array::from_fn(|note| {
            let increment = 440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0) / RATE;
            let level = ((0.47 / increment).log2().floor() as usize).min(LEVELS - 1);
            (increment, level)
        });
        Self { tables: levels, pitches }
    }

    fn sine(&self, phase: f32) -> f32 {
        // rem_euclid can round a tiny negative input to exactly 1.0.
        let x = phase * TABLE as f32;
        let whole = x as i32;
        let i = whole as usize & (TABLE - 1);
        let sine = &self.tables[0][0];
        sine[i] + (sine[i + 1] - sine[i]) * (x - whole as f32)
    }

    fn read(&self, level: usize, phase: f32, shape: Shape) -> f32 {
        let x = phase * TABLE as f32;
        let i = x as usize;
        let table = &self.tables[level];
        let interp = |shape: usize| {
            table[shape][i] + (table[shape][i + 1] - table[shape][i]) * (x - i as f32)
        };
        let a = interp(shape.index);
        a + (interp(shape.index + 1) - a) * shape.blend
    }
}

struct Tuning {
    pitch: f32,
    detune: f32,
    transpose: f32,
    shift_a: usize,
    shift_b: usize,
    model: models::Controls,
}

#[derive(Default, Clone, Copy)]
struct VoicePitch {
    a: f32,
    b: f32,
    level_a: usize,
    level_b: usize,
    rates: models::Rates,
}

impl Tuning {
    fn prepare(&self, increment: f32, level: usize) -> VoicePitch {
        let a = increment * self.pitch;
        VoicePitch {
            a, b: a * self.detune * self.transpose,
            level_a: level.saturating_sub(self.shift_a),
            level_b: level.saturating_sub(self.shift_b),
            rates: self.model.rates(a),
        }
    }
}

#[derive(Default, Clone, Copy)]
struct Voice {
    live_key: Option<u16>,
    sequenced: bool,
    retiring: bool,
    live_release: Option<u64>,
    live_delay: u64,
    velocity: f32,
    phase_a: f32,
    phase_b: f32,
    phase_sub: f32,
    model: models::State,
    tuned: VoicePitch,
    decaying: bool,
    increment: f32,
    level: usize,
    gain: f32,
    end_tick: f64,
}

pub(super) struct Instrument {
    live: super::acid_live::Queue,
    voices: [Voice; VOICES],
    last_step: Option<i64>,
    last_tick: Option<f64>,
    blend: f32,
    tb_gain: f32,
    pub(super) dry: Block,
    lfo: f32,
    low: f32,
    band: f32,
    // Modulated short delay for the patch chorus, separate from the stereo width control.
    chorus_line: [f32; 1024],
    chorus_index: usize,
    chorus_phase: f32,
    history: [f32; 512],
    write: usize,
}

impl Default for Instrument {
    fn default() -> Self {
        Self {
            live: super::acid_live::Queue::default(),
            voices: [Voice::default(); VOICES], last_step: None, last_tick: None, blend: 0.0,
            tb_gain: 1.0, dry: [0.0; crate::frame::BLOCK],
            lfo: 0.0, low: 0.0, band: 0.0, chorus_line: [0.0; 1024], chorus_index: 0, chorus_phase: 0.0, history: [0.0; 512], write: 0,
        }
    }
}

impl Instrument {
    pub(super) fn live_event(&mut self, event: super::acid_live::Event, tick: f64, advance: f64) {
        use super::acid_live::{Event, Pending};
        let now = self.live.sample;
        match event {
            Event::Audition { notes, frames } => {
                for (i, pitch) in notes.into_iter().enumerate() {
                    if pitch == 0 { continue; }
                    // Physical inputs use only the low 15 bits.
                    let key = 0x8000 | i as u16;
                    self.live_event(Event::On { key, pitch, velocity: 102, grid: 0 }, tick, advance);
                    if let Some(n) = self.live.notes.iter_mut().flatten().find(|n| n.key == key) {
                        n.release = Some(now + u64::from(frames.clamp(64, 441_000)));
                    }
                }
            }
            Event::Panic => {
                self.live.notes.fill(None);
                self.live.refresh();
                for voice in &mut self.voices {
                    if voice.live_key.is_some() { *voice = Voice::default(); }
                }
            }
            Event::Off { key } => {
                for note in self.live.notes.iter_mut().flatten().filter(|n| n.key == key) {
                    note.release = Some((now + note.delay).max(note.onset + 64));
                }
                for v in self.voices.iter_mut().filter(|v| v.live_key == Some(key)) {
                    v.live_release = Some(now + v.live_delay);
                }
            }
            Event::On { key, pitch, velocity, grid } => {
                if velocity == 0 { self.live_event(Event::Off { key }, tick, advance); return; }
                for pending in &mut self.live.notes {
                    if pending.is_some_and(|n| n.key == key) { *pending = None; }
                }
                for v in self.voices.iter_mut().filter(|v| v.live_key == Some(key)) {
                    v.live_key = None;
                    v.end_tick = f64::NEG_INFINITY;
                }
                let delay = if grid > 0 && advance > 0.0 {
                    let grid = f64::from(grid);
                    let wait = ((tick / grid).ceil() * grid - tick).max(0.0);
                    (wait / advance).round() as u64
                } else { 0 };
                self.live.push(Pending { key, pitch: pitch.min(127), velocity: velocity.min(127),
                    onset: now + delay, delay, release: None });
            }
        }
    }

    fn start_live_notes(&mut self, bank: &Bank, tuning: &Tuning) {
        let mut reserved = [false; VOICES];
        for pending in &mut self.live.notes {
            if !pending.is_some_and(|n| n.onset <= self.live.sample) { continue; }
            let n = pending.take().unwrap();
            let index = self.voices.iter().position(|v| v.increment == 0.0).unwrap_or_else(||
                self.voices.iter().enumerate().filter(|(i, _)| !reserved[*i]).min_by(|(_, a), (_, b)|
                    a.gain.total_cmp(&b.gain)).unwrap().0);
            reserved[index] = true;
            let mut v = Self::voice(bank, tuning, n.pitch, f64::INFINITY);
            v.velocity = f32::from(n.velocity) / 127.0;
            v.live_key = Some(n.key);
            v.live_delay = n.delay;
            v.live_release = n.release;
            self.voices[index] = v;
        }
        self.live.refresh();
    }

    fn voice(bank: &Bank, tuning: &Tuning, note: u8, end_tick: f64) -> Voice {
        let (increment, level) = bank.pitches[usize::from(note.min(127))];
        let tuned = tuning.prepare(increment, level);
        Voice { increment, level, tuned, end_tick, velocity: 1.0, ..Voice::default() }
    }

    fn trigger(&mut self, bank: &Bank, tuning: &Tuning, step: Step, end_tick: f64) {
        let mut reserved = [false; VOICES];
        for (i, &note) in step.notes.iter().enumerate() {
            if note == 0 || step.notes[..i].contains(&note) { continue; }
            let index = self.voices.iter().position(|v| v.increment == 0.0).or_else(||
                self.voices.iter().enumerate()
                    .filter(|(i, v)| !reserved[*i] && v.live_key.is_none())
                    .min_by(|(_, a), (_, b)| a.gain.total_cmp(&b.gain)).map(|(i, _)| i));
            let Some(index) = index else { break };
            reserved[index] = true;
            let mut voice = Self::voice(bank, tuning, note, end_tick);
            voice.sequenced = true;
            self.voices[index] = voice;
        }
    }

    /// Release only old pattern voices. Physical keys, pending keys and their release envelopes survive edits.
    pub(super) fn phrase_changed(&mut self) {
        for voice in &mut self.voices {
            if voice.sequenced && voice.increment != 0.0 {
                voice.end_tick = f64::NEG_INFINITY;
                voice.retiring = true;
            }
        }
        self.last_step = None;
        self.last_tick = None;
    }

    /// Return before touching the legacy bus whenever AcidSynth is fully bypassed.
    pub fn render(&mut self, bank: &Bank, slot: &Slot, bus: &mut Block, tick: f64, advance: f64, transport_playing: bool) {
        let tb_target = if slot.tb_active() { 1.0 } else { 0.0 };
        if !slot.enabled && self.blend <= 0.0 {
            if self.tb_gain == tb_target {
                if tb_target == 0.0 { bus.fill(0.0); }
            } else {
                for sample in bus {
                    self.tb_gain += (tb_target - self.tb_gain).clamp(-1.0/256.0, 1.0/256.0);
                    *sample *= self.tb_gain;
                }
            }
            return;
        }
        let play = transport_playing && slot.sequence_enabled;
        if !play || !slot.enabled {
            for v in &mut self.voices {
                if !slot.enabled || v.live_key.is_none() { v.end_tick = f64::NEG_INFINITY; }
            }
            self.last_step = None;
            self.last_tick = None;
        } else if self.last_tick.is_some_and(|last| (tick-last).abs() > 2.0 * advance + 0.001) {
            for v in &mut self.voices {
                if v.live_key.is_none() { *v = Voice::default(); }
            }
            self.last_step = None;
        }
        let p = slot.patch;
        let [timbre, motion, width, _] = slot.macros;
        let next_lfo = if p.lfo_sync == 0 {
            (self.lfo + (p.lfo_hz() / RATE) * bus.len() as f32).fract()
        } else {
            let period = f64::from(p.lfo_period().1);
            let elapsed_ticks = advance * bus.len() as f64;
            if transport_playing {
                // Recover phase directly from the transport after seeks, loops and tempo changes.
                self.lfo = (tick.rem_euclid(period) / period) as f32;
                ((tick + elapsed_ticks).rem_euclid(period) / period) as f32
            } else {
                // The transport is parked but live keys still modulate at the project tempo.
                (f64::from(self.lfo) + elapsed_ticks / period).fract() as f32
            }
        };
        let waveform = match p.lfo_shape {
            1 => 1.0 - 4.0 * (self.lfo - 0.5).abs(),
            2 => 2.0 * self.lfo - 1.0,
            3 if self.lfo < 0.5 => 1.0,
            3 => -1.0,
            _ => (self.lfo * TAU).sin(),
        };
        let lfo = waveform * p.lfo_depth * (0.15 + 0.85 * motion);
        // Matrix depth belongs to each cable; fixed LFO routing retains its own depth.
        let env = self.voices.iter().map(|v| v.gain).fold(0.0_f32, f32::max);
        let sources = [0.0, waveform, env, timbre, motion, width, slot.macros[3]];
        let mut offsets = [0.0_f32; 6];
        for route in p.routes {
            if let (Some(&value), Some(offset)) = (sources.get(usize::from(route.source)),
                offsets.get_mut(usize::from(route.dest))) {
                *offset += value * route.amount;
            }
        }
        let [wave_a_offset, wave_b_offset, cutoff_offset, mix_offset, pitch_offset, drive_offset] =
            offsets.map(|v| v.clamp(-1.0, 1.0));
        let color = (timbre - 0.5) * 0.5 + if p.lfo_target == 0 { lfo * 0.22 } else { 0.0 };
        let lfo_cut = if p.lfo_target == 1 { lfo * 0.42 } else { 0.0 };
        let semitones = 12.0 * pitch_offset + if p.lfo_target == 2 { lfo } else { 0.0 };
        let pitch = if semitones == 0.0 { 1.0 } else { 2.0_f32.powf(semitones / 12.0) };
        let mix = (p.mix + mix_offset + if p.lfo_target == 3 { 0.5 * lfo } else { 0.0 }).clamp(0.0, 1.0);
        let model = models::Controls::new(&p, color, [wave_a_offset, wave_b_offset], mix);
        let wave_a = Shape::new(p.wave_a + color + wave_a_offset);
        let wave_b = Shape::new(p.wave_b - 0.6 * color + wave_b_offset);
        let drive = (p.drive + drive_offset).clamp(0.0, 1.0);
        let transpose = 2.0_f32.powf(f32::from(p.transpose_b) / 12.0);
        let shift_a = (semitones.max(0.0) / 12.0).ceil() as usize;
        let shift_b = ((f32::from(p.transpose_b) + semitones).max(0.0) / 12.0).ceil() as usize;
        let decay = (-6.91 / ((0.012 + 2.5 * p.decay.powi(2)) * RATE)).exp();
        let detune = 2.0_f32.powf(p.detune * (0.15 + 0.85 * width) * 30.0 / 1200.0);
        let attack = 1.0 / ((0.004 + 3.0 * p.attack.powi(2)) * RATE);
        let release = (-6.91 / ((0.025 + 5.0 * p.release.powi(2)) * RATE)).exp();
        let hz = 70.0 * 2.0_f32.powf(7.0 * (p.cutoff + cutoff_offset + color * 0.8 + lfo_cut + env * p.filter_env * 0.65).clamp(0.0, 1.0));
        // TPT state-variable low-pass; bounded Q at every cutoff.
        let g = (std::f32::consts::PI * hz / RATE).tan();
        let damp = 1.9 - 1.65 * p.resonance;
        let a = 1.0 / (1.0 + g * (g + damp));
        let span = f64::from(slot.ticks_per_step());
        let count = slot.step_count();
        let tuning = Tuning { pitch, detune, transpose, shift_a, shift_b, model };
        for voice in &mut self.voices {
            if voice.increment != 0.0 {
                voice.tuned = tuning.prepare(voice.increment, voice.level);
            }
        }
        let mut next_step_tick = tick;
        for (i, sample) in bus.iter_mut().enumerate() {
            let position = tick + i as f64 * advance;
            if play && slot.enabled && position >= next_step_tick {
                let n = (position.max(0.0) / span).floor() as i64;
                next_step_tick = (n + 1) as f64 * span;
                if self.last_step.is_none() {
                    let mut chase = |previous: i64| {
                        let step = slot.step(previous as usize % count);
                        let end = (previous as f64 + f64::from(step.gate_steps.min(count as u16))) * span;
                        if end > position { self.trigger(bank, &tuning, step, end); }
                    };
                    if let Some(ages) = slot.prepared.prior(n as usize % count) {
                        for &age in ages.iter().rev() {
                            if age != 0 && i64::from(age) <= n { chase(n - i64::from(age)); }
                        }
                    } else {
                        // Direct/offline users may freely edit the public Slot without preparing it.
                        // Native full-settings messages always carry the prebuilt lookup.
                        for previous in n.saturating_sub(count as i64 - 1).max(0)..n { chase(previous); }
                    }
                }
                if self.last_step != Some(n) {
                    let step = slot.step(n as usize % count);
                    self.trigger(bank, &tuning, step, (n as f64 + f64::from(step.gate_steps.min(count as u16))) * span);
                    self.last_step = Some(n);
                }
            }
            if self.live.next.is_some_and(|next| next <= self.live.sample) {
                self.start_live_notes(bank, &tuning);
            }
            let now = self.live.sample;
            self.live.sample += 1;
            let mut sum = 0.0;
            for v in &mut self.voices {
                if v.increment == 0.0 { continue; }
                if v.live_release.is_some_and(|end| end <= now) {
                    v.live_key = None;
                    v.live_release = None;
                    v.end_tick = f64::NEG_INFINITY;
                }
                if position < v.end_tick {
                    if v.decaying {
                        v.gain = p.sustain + (v.gain - p.sustain) * decay;
                    } else {
                        v.gain = (v.gain + attack).min(1.0);
                        v.decaying = v.gain >= 1.0;
                    }
                } else {
                    // At most a few milliseconds of the outgoing patch on a reroll.
                    v.gain *= if v.retiring { release.min(0.94) } else { release };
                    if v.gain < 0.00001 { v.increment = 0.0; continue; }
                }
                let inc_a = v.tuned.a;
                let inc_b = v.tuned.b;
                let osc = if p.osc_mode >= 3 {
                    v.model.sample(bank, &model, &v.tuned.rates)
                } else {
                    let level_b = v.tuned.level_b;
                    let b = if inc_b < 0.49 { bank.read(level_b, v.phase_b, wave_b) } else { 0.0 };
                    let phase = if p.osc_mode == 2 {
                        (v.phase_a + b * mix * 0.22 + 1.0).fract()
                    } else { v.phase_a };
                    let a = if inc_a < 0.49 { bank.read(v.tuned.level_a, phase, wave_a) } else { 0.0 };
                    match p.osc_mode {
                        1 => a + (1.6 * a * b - a) * mix,
                        2 => a,
                        _ => a + (b-a) * mix,
                    }
                };
                let sub = if p.sub > 0.0 { bank.sine(v.phase_sub) * p.sub * 0.7 } else { 0.0 };
                sum += (osc + sub) * v.gain * v.velocity * 0.14;
                v.phase_a = (v.phase_a + inc_a).fract();
                v.phase_b = (v.phase_b + inc_b).fract();
                v.phase_sub = (v.phase_sub + inc_a * 0.5).fract();
            }
            let band = a * (self.band + g * (sum - self.low));
            let low = self.low + g * band;
            self.low = 2.0 * low - self.low;
            self.band = 2.0 * band - self.band;
            let target = if slot.enabled { 1.0 } else { 0.0 };
            self.blend += (target - self.blend).clamp(-1.0/256.0, 1.0/256.0);
            let high = sum - damp * band - low;
            let mut out = match p.filter_mode {
                1 => band, 2 => high, 3 => high + low, _ => low,
            };
            if drive > 0.0 {
                let gain = 1.0 + drive * 6.0;
                let input = out * gain;
                out += (input / (1.0 + input.abs()) - out) * drive;
            }
            self.chorus_line[self.chorus_index] = out;
            if p.chorus > 0.0 {
                let delay = 520.0 + 150.0 * bank.sine(self.chorus_phase);
                let whole = delay as usize;
                let head = (self.chorus_index + 1024 - whole) & 1023;
                let a = self.chorus_line[head];
                let b = self.chorus_line[(head + 1023) & 1023];
                let delayed = a + (b - a) * (delay - whole as f32);
                out += (delayed - out) * (0.45 * p.chorus);
            }
            self.chorus_index = (self.chorus_index + 1) & 1023;
            self.chorus_phase = (self.chorus_phase + 0.38 / RATE).fract();
            if slot.tb_enabled.is_none() {
                *sample += (out - *sample) * self.blend;
                self.tb_gain = 1.0 - self.blend;
            } else {
                self.tb_gain += (tb_target - self.tb_gain).clamp(-1.0/256.0, 1.0/256.0);
                self.dry[i] = out * self.blend;
                *sample = *sample * self.tb_gain + self.dry[i];
            }
        }
        if play && slot.enabled { self.last_tick = Some(tick + bus.len() as f64 * advance); }
        self.lfo = next_lfo;
        if !slot.enabled && self.blend == 0.0 {
            *self = Self { tb_gain: self.tb_gain, ..Self::default() };
        }
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
    fn tempo_lfo_tracks_transport_across_rate_changes_seeks_and_keys_only_mode() {
        let bank = Bank::new();
        let mut synth = Instrument::default();
        let mut slot = Slot { enabled: true, sequence_enabled: false, ..Slot::default() };
        slot.patch.lfo_sync = 1;
        let mut block = [0.0; 64];
        for (division, &(_, period_ticks)) in LFO_DIVISIONS.iter().enumerate() {
            slot.patch.lfo_division = division as u8;
            let period = f64::from(period_ticks);
            for (tick, bpm) in [(period * 2.25, 90.0), (period * 0.125, 165.0)] {
                let advance = bpm * 192.0 / (60.0 * 44100.0);
                synth.render(&bank, &slot, &mut block, tick, advance, true);
                let expected = ((tick + advance * 64.0).rem_euclid(period) / period) as f32;
                assert_eq!(synth.lfo, expected);
            }
            // Stopped keys follow that same tempo even though the transport tick stays parked.
            let before = synth.lfo;
            synth.render(&bank, &slot, &mut block, 0.0, 0.01, false);
            assert_eq!(synth.lfo, (f64::from(before) + 64.0 * 0.01 / period).fract() as f32);
        }
        assert!(LFO_DIVISIONS.contains(&("1/8 triplet", 64)));
        assert!(LFO_DIVISIONS.contains(&("1/8 dotted", 144)));
    }

    #[test]
    fn old_patch_keeps_free_rate_and_synced_patch_round_trips() {
        let old: Patch = serde_json::from_str(r#"{"lfo_rate":0.625,"lfo_depth":0.75}"#).unwrap();
        assert_eq!(old.lfo_sync, 0);
        assert_eq!(old.lfo_period(), ("1/4", 192));
        let bank = Bank::new();
        let mut synth = Instrument::default();
        let mut slot = Slot { enabled: true, patch: old, ..Slot::default() };
        let mut block = [0.0; 64];
        let mut reference = 0.0_f32;
        for (tick, advance) in [(0.0, 0.01), (64.0, 0.02), (0.0, 0.02)] {
            synth.render(&bank, &slot, &mut block, tick, advance, true);
            reference = (reference + (0.04 + 6.0 * old.lfo_rate.powi(2)) / RATE * 64.0).fract();
            assert_eq!(synth.lfo, reference);
        }
        slot.patch.lfo_sync = 1;
        slot.patch.lfo_division = 14;
        let encoded = serde_json::to_string(&slot.patch).unwrap();
        assert_eq!(serde_json::from_str::<Patch>(&encoded).unwrap(), slot.patch);
        assert_eq!(slot.patch.lfo_period(), ("1/8 triplet", 64));
    }

    #[test]
    fn replacing_a_playing_phrase_chases_mid_gate_without_releasing_live_keys() {
        use crate::studio::{PreparedSettings, Studio, acid_live::Event, convolution::Convolver};
        let mut studio = Studio::new(Box::new(Convolver::new(&[[1.0; 2]]).unwrap()));
        let mut settings = studio.settings.clone();
        let slot = &mut settings.synth_slots[0];
        slot.enabled = true;
        slot.steps = [Step::default(); 16];
        slot.steps[0] = Step { notes: [48, 0, 0, 0], gate_steps: 12 };
        slot.patch.attack = 0.0;
        studio.replace_prepared_settings(PreparedSettings::new(settings));
        let mut output = [0.0; 64];
        let advance = 1.0 / 64.0;
        studio.live_note(0, Event::On { key: 1, pitch: 82, velocity: 120, grid: 0 }, 960.0, advance);
        studio.synth_channel(0, &mut output, 960.0, advance, true);
        assert_eq!(studio.synths[0].last_step, Some(5));
        assert!(studio.synths[0].voices.iter().any(|v| v.sequenced && v.end_tick == 2304.0));
        studio.live_note(0, Event::On { key: 2, pitch: 83, velocity: 118, grid: 48 }, 961.0, advance);

        let mut unchanged = studio.settings.clone();
        unchanged.synth_slots[0].macros[0] = 0.9;
        studio.replace_prepared_settings(PreparedSettings::new(unchanged));
        assert_eq!(studio.synths[0].last_step, Some(5));

        let mut next = studio.settings.clone();
        let slot = &mut next.synth_slots[0];
        slot.set_step_rate(96);
        slot.steps[0] = Step { notes: [55, 62, 0, 0], gate_steps: 16 };
        studio.replace_prepared_settings(PreparedSettings::new(next));
        assert!(studio.synths[0].last_step.is_none());
        for i in 0..4 {
            studio.synth_channel(0, &mut output, 961.0 + f64::from(i), advance, true);
        }
        let synth = &studio.synths[0];
        assert_eq!(synth.last_step, Some(10));
        assert_eq!(synth.voices.iter().filter(|v| v.increment != 0.0 && v.sequenced && v.end_tick == 1536.0).count(), 2);
        assert!(!synth.voices.iter().any(|v| v.increment != 0.0 && v.retiring));
        assert!(synth.voices.iter().any(|v| v.live_key == Some(1)));
        assert!(synth.live.notes.iter().flatten().any(|event| event.key == 2));
    }

    #[test]
    fn prepared_chase_wraps_only_after_the_first_transport_pass() {
        let mut slot = Slot::default();
        slot.enabled = true;
        slot.steps = [Step::default(); 16];
        slot.steps[14] = Step { notes: [64, 0, 0, 0], gate_steps: 5 };
        slot.prepare_phrase();
        let bank = Bank::new();
        let mut synth = Instrument::default();
        let mut block = [0.0; 64];
        synth.render(&bank, &slot, &mut block, 192.0, 0.01, true);
        assert!(!synth.gate(192.0));
        synth.render(&bank, &slot, &mut block, 17.0 * 192.0, 0.01, true);
        assert!(synth.gate(17.0 * 192.0));
        synth.render(&bank, &slot, &mut block, 19.0 * 192.0, 0.01, true);
        assert!(!synth.gate(19.0 * 192.0));
    }

    #[test]
    fn live_keys_are_polyphonic_velocity_sensitive_and_release_while_stopped() {
        use super::super::acid_live::Event;
        let bank = Bank::new();
        let slot = Slot { enabled: true, ..Slot::default() };
        let mut synth = Instrument::default();
        for (key, pitch, velocity) in [(1, 60, 32), (2, 67, 120)] {
            synth.live_event(Event::On { key, pitch, velocity, grid: 48 }, 0.0, 0.0);
        }
        let mut block = [0.0; 64];
        for _ in 0..20 {
            block.fill(0.0);
            synth.render(&bank, &slot, &mut block, 0.0, 0.01, false);
        }
        assert!(block.iter().any(|v| v.abs() > 1.0e-5));
        assert_eq!(synth.voices.iter().filter(|v| v.live_key.is_some()).count(), 2);
        assert!(synth.voices[0].velocity < synth.voices[1].velocity);
        synth.live_event(Event::Off { key: 1 }, 0.0, 0.0);
        synth.render(&bank, &slot, &mut block, 0.0, 0.01, false);
        assert!(synth.voices.iter().all(|v| v.live_key != Some(1)));
        assert!(synth.voices.iter().any(|v| v.live_key == Some(2)));
        synth.live_event(Event::Panic, 0.0, 0.0);
        assert!(synth.voices.iter().all(|v| v.live_key.is_none()));
    }

    #[test]
    fn a_short_quantized_key_starts_on_the_grid_and_releases_even_after_a_seek() {
        use super::super::acid_live::Event;
        let bank = Bank::new();
        let slot = Slot { enabled: true, ..Slot::default() };
        let mut synth = Instrument::default();
        synth.live_event(Event::On { key: 5, pitch: 64, velocity: 90, grid: 48 }, 40.0, 0.125);
        synth.live_event(Event::Off { key: 5 }, 40.0, 0.125);
        let mut block = [0.0; 64];
        synth.render(&bank, &slot, &mut block, 40.0, 0.125, true);
        assert!(synth.voices.iter().all(|v| v.live_key.is_none()));
        synth.render(&bank, &slot, &mut block, 0.0, 0.125, true);
        assert!(synth.voices.iter().any(|v| v.live_key == Some(5)));
        synth.render(&bank, &slot, &mut block, 8.0, 0.125, true);
        assert!(synth.voices.iter().all(|v| v.live_key.is_none()));
    }

    #[test]
    fn literal_bars_keep_their_duration_at_every_rate_and_retain_hidden_steps() {
        let mut slot = Slot::default();
        for ticks in [48, 96, 192, 384, 768] {
            slot.set_step_rate(ticks);
            for n in [1, 2, 4, 8, 16, 32, 64, 128] {
                slot.set_length(PhraseLength::Bars(n));
                assert_eq!(slot.ticks_per_step(), ticks);
                assert_eq!(slot.step_count() * usize::from(ticks), usize::from(n) * 768);
            }
        }
        let chord = Step { notes: [47, 54, 59, 62], gate_steps: 512 };
        *slot.step_mut(2047) = chord;
        slot.set_length(PhraseLength::Bars(1));
        assert_eq!(slot.step_count(), 1);
        slot.sanitize();
        slot.set_length(PhraseLength::Steps(16));
        assert_eq!(slot.step_count(), 16);
        slot.set_length(PhraseLength::Bars(128));
        slot.set_step_rate(48);
        assert_eq!(slot.step_count(), 2048);
        assert_eq!(slot.step(2047), chord);
        assert_eq!(slot.step(2046), Step::default());
    }

    #[test]
    fn long_phrase_chases_earlier_bars_and_wraps_at_its_actual_end() {
        let bank = Bank::new();
        let mut instrument = Instrument::default();
        let mut slot = Slot::default();
        slot.enabled = true;
        slot.length = PhraseLength::Bars(128);
        slot.set_step_rate(48);
        slot.steps = [Step::default(); 16];
        *slot.step_mut(1024) = Step { notes: [61, 66, 71, 76], gate_steps: 512 };
        let mut bus = [0.0; 64];
        let middle = 1280.0 * 48.0;
        instrument.render(&bank, &slot, &mut bus, middle, 0.01, true);
        assert!(instrument.gate(middle));
        let end = 2048.0 * 48.0;
        instrument.render(&bank, &slot, &mut bus, end, 0.01, true);
        assert!(!instrument.gate(end));
        let next = end + middle;
        instrument.render(&bank, &slot, &mut bus, next, 0.01, true);
        assert!(instrument.gate(next));
    }

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
    fn retune_applies_to_held_and_mid_block_live_voices() {
        use super::super::acid_live::Event;
        let bank = Bank::new();
        let mut synth = Instrument::default();
        let mut slot = Slot { enabled: true, sequence_enabled: false, patch: Patch::INIT, ..Slot::default() };
        let mut block = [0.0; 64];
        synth.live_event(Event::On { key: 1, pitch: 60, velocity: 127, grid: 0 }, 0.0, 0.0);
        synth.render(&bank, &slot, &mut block, 0.0, 0.25, false);
        assert_eq!(synth.voices[0].tuned.b, bank.pitches[60].0);
        slot.patch.transpose_b = 12;
        // Four frames before the next grid: the second key starts inside this block.
        synth.live_event(Event::On { key: 2, pitch: 67, velocity: 127, grid: 48 }, 47.0, 0.25);
        synth.render(&bank, &slot, &mut block, 47.0, 0.25, false);
        for (voice, note) in synth.voices[..2].iter().zip([60, 67]) {
            assert_eq!(voice.tuned.b, bank.pitches[note].0 * 2.0);
        }
        let mut expected_phase = 0.0_f32;
        for _ in 0..60 { expected_phase = (expected_phase + bank.pitches[67].0 * 2.0).fract(); }
        assert_eq!(synth.voices[1].phase_b, expected_phase);
    }

    #[test]
    fn stealing_voices_keeps_every_note_of_the_new_chord() {
        let mut synth = Instrument::default();
        for voice in &mut synth.voices {
            voice.increment = 0.01;
            voice.gain = 0.8;
        }
        let bank = Bank::new();
        let tuning = Tuning { pitch: 1.0, detune: 1.0, transpose: 1.0, shift_a: 0, shift_b: 0,
            model: models::Controls::new(&Patch::INIT, 0.0, [0.0; 2], 0.5) };
        synth.trigger(&bank, &tuning, Step { notes: [48, 52, 55, 59], gate_steps: 1 }, 192.0);
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

/// Draw two periods of the selected alternative oscillator at A3, before filter/envelope/FX.
pub fn model_preview(patch: &Patch, macros: [f32; 4], samples: &mut [f32]) {
    if samples.is_empty() { return; }
    if patch.osc_mode < 3 {
        samples.fill(0.0);
        return;
    }
    thread_local! { static PREVIEW_BANK: Bank = Bank::new(); }
    PREVIEW_BANK.with(|bank| draw_preview(bank, patch, macros, samples));
}

fn draw_preview(bank: &Bank, patch: &Patch, macros: [f32; 4], samples: &mut [f32]) {
    let mut state = models::State::default();
    let mut offsets = [0.0_f32; 2];
    let mut mix_offset = 0.0;
    let sources = [0.0, 0.0, patch.sustain, macros[0], macros[1], macros[2], macros[3]];
    for route in patch.routes {
        let amount = sources.get(usize::from(route.source)).copied().unwrap_or(0.0) * route.amount;
        if route.dest < 2 { offsets[usize::from(route.dest)] += amount; }
        if route.dest == 3 { mix_offset += amount; }
    }
    let mix = (patch.mix + mix_offset).clamp(0.0, 1.0);
    let controls = models::Controls::new(patch, (macros[0] - 0.5) * 0.5, offsets, mix);
    const FRAMES: usize = 401;
    let mut cycle = [0.0; FRAMES];
    let rates = controls.rates(220.0 / RATE);
    for _ in 0..201 { state.sample(bank, &controls, &rates); }
    for value in &mut cycle { *value = state.sample(bank, &controls, &rates); }
    let stride = (FRAMES - 1) as f32 / samples.len().saturating_sub(1).max(1) as f32;
    for (i, value) in samples.iter_mut().enumerate() {
        let x = (i as f32 * stride).min((FRAMES - 1) as f32);
        let a = x as usize;
        *value = cycle[a] + (cycle[(a + 1).min(FRAMES - 1)] - cycle[a]) * x.fract();
    }
}

#[cfg(test)]
mod patch_tests {
    use super::*;

    fn audio(bank: &Bank, patch: Patch) -> Vec<f32> {
        let mut instrument = Instrument::default();
        let mut slot = Slot::default();
        slot.enabled = true;
        slot.patch = patch;
        let mut result = Vec::new();
        for block in 0..80 {
            let mut output = [0.0; 64];
            instrument.render(bank, &slot, &mut output, f64::from(block) * 0.64, 0.01, true);
            result.extend_from_slice(&output);
        }
        result
    }

    fn rms_difference(a: &[f32], b: &[f32]) -> f32 {
        (a.iter().zip(b).map(|(x,y)| (x-y).powi(2)).sum::<f32>() / a.len() as f32).sqrt()
    }

    #[test]
    fn four_operator_algorithms_and_sync_fold_render_distinct_audible_chords() {
        let bank = Bank::new();
        let base = Patch { attack: 0.0, cutoff: 0.86, fm_index: 0.67, fm_feedback: 0.42,
            fm_ratios: [1.5, 2.73, 4.1], mix: 0.71, ..Patch::INIT };
        let legacy = audio(&bank, base);
        let mut sounds = vec![legacy.clone()];
        for (mode, algorithm) in [(3,0), (3,1), (3,2), (3,3), (4,0)] {
            let patch = Patch { osc_mode: mode, fm_algorithm: algorithm, ..base };
            let out = audio(&bank, patch);
            assert!(out.iter().all(|s| s.is_finite() && s.abs() < 2.0));
            assert!(out.iter().any(|s| s.abs() > 0.02));
            for previous in &sounds { assert!(rms_difference(&out, previous) > 0.003); }
            sounds.push(out);
        }
        // The extra oscillator controls are strictly bypassed by the original engines.
        for mode in 0..=2 {
            let old = Patch { osc_mode: mode, ..base };
            let changed = Patch { fm_ratios: [12.0; 3], fm_feedback: 1.0, fm_index: 1.0,
                fm_algorithm: 3, fold: 1.0, sync: 1.0, ..old };
            assert_eq!(audio(&bank, old), audio(&bank, changed));
        }
        for (mode, changes) in [
            (3, vec![Patch { fm_feedback: 0.93, ..base }, Patch { fm_index: 0.1, ..base },
                Patch { fm_ratios: [2.0, 2.73, 4.1], ..base },
                Patch { fm_ratios: [1.5, 3.5, 4.1], ..base },
                Patch { fm_ratios: [1.5, 2.73, 2.0], ..base }]),
            (4, vec![Patch { sync: 0.87, ..base }, Patch { fold: 0.87, ..base }]),
        ] {
            let original = audio(&bank, Patch { osc_mode: mode, ..base });
            for edit in changes {
                assert!(rms_difference(&original, &audio(&bank, Patch { osc_mode: mode, ..edit })) > 0.001);
            }
        }
    }

    #[test]
    fn two_bipolar_matrix_cables_act_independently_and_disconnect_cleanly() {
        let bank = Bank::new();
        let mut patch = Patch { attack: 0.0, ..Patch::INIT };
        let dry = audio(&bank, patch);
        let filter = ModRoute { source: 3, dest: 2, amount: -0.9 };
        let morph = ModRoute { source: 2, dest: 0, amount: 0.9 };
        patch.routes[0] = filter;
        let only_filter = audio(&bank, patch);
        patch.routes[1] = morph;
        let both = audio(&bank, patch);
        patch.routes[0] = ModRoute::OFF;
        let only_morph = audio(&bank, patch);
        for other in [&dry, &only_filter, &only_morph] {
            assert!(both.iter().zip(other).any(|(x, y)| (x-y).abs() > 0.001));
        }
        patch.routes[1].amount = 0.0;
        assert_eq!(audio(&bank, patch), dry);
    }

    #[test]
    fn patch_routes_change_the_audio_and_keep_levels_finite() {
        let bank = Bank::new();
        let base = Patch { attack: 0.0, lfo_rate: 0.9, lfo_depth: 0.8, ..Patch::INIT };
        let reference = audio(&bank, base);
        for patch in [
            Patch { osc_mode: 1, ..base }, Patch { osc_mode: 2, ..base },
            Patch { filter_mode: 1, ..base }, Patch { filter_mode: 2, ..base },
            Patch { filter_mode: 3, ..base }, Patch { lfo_target: 2, ..base },
            Patch { lfo_target: 3, lfo_shape: 1, ..base }, Patch { chorus: 0.8, ..base },
            Patch { drive: 0.7, ..base }, Patch { sub: 1.0, ..base },
            Patch { filter_env: 0.65, ..base }, Patch { transpose_b: 7, ..base },
            Patch { decay: 0.2, sustain: 0.2, ..base },
        ] {
            let out = audio(&bank, patch);
            assert!(out.iter().all(|x| x.is_finite() && x.abs() < 2.0));
            assert!(out.iter().zip(&reference).any(|(x,y)| (x-y).abs() > 0.001), "{patch:?}");
        }
    }
}
