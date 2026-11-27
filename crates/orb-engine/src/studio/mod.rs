//! Optional Studio instruments and convolution send. Legacy frames have no Studio instance.

pub mod acid_synth;
pub mod acid_live;
pub mod acid_presets;
pub mod audio_file;
mod bus;
pub use bus::SamplerBusSettings;
pub mod convolution;
pub mod drums;
mod fft;
pub mod meter;
mod mp3;
pub mod sampler;
#[cfg(test)]
mod tests;

use convolution::Convolver;
use sampler::{LANES, Lane, Sampler};

/// One block's logical modulation events, captured around the native sequencer step.
pub struct Modulation {
    pub gates: [bool; 2],
    /// New hits in this block, by machine and native pattern column (accent is unused).
    pub drum_triggers: [[u64; 12]; 2],
    /// Cumulative native counters, used to follow sidechain instrument changes without replay.
    pub drum_counts: [[u64; 12]; 2],
    pub audible: [bool; 2],
    pub config: crate::sidechain::Config,
    pub ticks_per_frame: f64,
}

pub(super) fn smooth(value: &mut f64, target: f64, factor: f64) {
    let delta = target - *value;
    if delta.abs() < 1e-12 {
        *value = target;
    } else {
        *value += delta * factor;
    }
}

fn clean(value: f64) -> f64 {
    if value.abs() < 1e-20 { 0.0 } else { value }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub synth_slots: [acid_synth::Slot; 2],
    /// Shared immutable banks; replacements retain the old bank until UI garbage collection.
    #[serde(default)]
    pub drum_patterns: [std::sync::Arc<drums::Bank>; 2],
    /// Per-machine calibration. Missing fields in older projects preserve the legacy mix.
    #[serde(default)]
    pub drum_gain_db: [f64; 2],
    #[serde(default)]
    pub sampler_bus: SamplerBusSettings,
    pub sends: [f64; 4],
    pub enabled: bool,
    pub return_gain: f64,
    pub predelay_ms: f64,
    pub low_cut: f64,
    pub high_cut: f64,
    pub width: f64,
    pub lanes: [Lane; LANES],
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            synth_slots: Default::default(),
            drum_patterns: std::array::from_fn(|_| std::sync::Arc::default()),
            drum_gain_db: [0.0; 2],
            sampler_bus: SamplerBusSettings::default(),
            lanes: std::array::from_fn(|_| Lane::default()),
            sends: [0.0; 4],
            enabled: true,
            return_gain: 0.25,
            predelay_ms: 18.0,
            low_cut: 140.0,
            high_cut: 10000.0,
            width: 1.0,
        }
    }
}

impl Settings {
    pub fn sanitize(&mut self) {
        for slot in &mut self.synth_slots {
            slot.sanitize();
            slot.prepare_phrase();
        }
        self.sampler_bus.sanitize();
        fn finite(v: f64, lo: f64, hi: f64) -> f64 {
            if v.is_finite() { v.clamp(lo, hi) } else { lo }
        }
        self.drum_gain_db = self.drum_gain_db.map(|v| {
            if v.is_finite() {
                v.clamp(-12.0, 12.0)
            } else {
                0.0
            }
        });
        for lane in &mut self.lanes {
            lane.sanitize();
        }
        self.sends = self.sends.map(|x| finite(x, 0.0, 1.0));
        self.return_gain = finite(self.return_gain, 0.0, 1.0);
        self.predelay_ms = finite(self.predelay_ms, 0.0, 200.0);
        self.low_cut = finite(self.low_cut, 20.0, 2000.0);
        self.high_cut = finite(self.high_cut, 2000.0, 20000.0);
        self.width = finite(self.width, 0.0, 2.0);
    }
}

pub struct Studio {
    synth_bank: acid_synth::Bank,
    synths: [acid_synth::Instrument; 2],
    meters: [meter::Meter; 8],
    drum_gain: [f64; 2],
    pub settings: Box<Settings>,
    pub sampler: Sampler,
    sampler_bus: bus::Bus,
    convolver: Box<Convolver>,
    previous: Option<Box<Convolver>>,
    pending: Option<Box<Convolver>>,
    fade: usize,
    send: [[f64; 64]; 2],
    predelay: Vec<[f64; 2]>,
    delay_position: usize,
    smooth_delay: f64,
    high_state: [f64; 2],
    low_state: [f64; 2],
    smooth_sends: [f64; 4],
    layered_base_send: [f64; 2],
    smooth_return: f64,
    wet_remaining: usize,
    ducker: crate::sidechain::Ducker,
    lane_duckers: [crate::sidechain::Ducker; LANES],
}

/// A replacement validated before it reaches the real-time thread.
pub struct PreparedSettings(Box<Settings>);

/// Validated sound controls only. No notes, drum banks or sampler data travel with this update.
#[derive(Clone, Copy, PartialEq)]
pub struct AcidSound {
    patch: acid_synth::Patch,
    macros: [f32; 4],
    preset: u8,
}

impl AcidSound {
    pub fn prepare(slot: &mut acid_synth::Slot) -> Self {
        slot.sanitize_sound();
        Self { patch: slot.patch, macros: slot.macros, preset: slot.preset }
    }

    pub fn apply(&self, slot: &mut acid_synth::Slot) {
        slot.patch = self.patch;
        slot.macros = self.macros;
        slot.preset = self.preset;
    }
}

impl PreparedSettings {
    pub fn new(mut settings: Box<Settings>) -> Self {
        settings.sanitize();
        Self(settings)
    }

    pub fn get(&self) -> &Settings { &self.0 }

    pub fn into_inner(self) -> Box<Settings> { self.0 }
}

impl Studio {
    /// Prepare on a worker/UI thread, then transfer the box to the player.
    pub fn new(convolver: Box<Convolver>) -> Self {
        Self {
            synth_bank: acid_synth::Bank::new(),
            synths: Default::default(),
            drum_gain: [1.0; 2],
            meters: Default::default(),
            settings: Box::default(),
            sampler: Sampler::default(),
            sampler_bus: bus::Bus::default(),
            convolver,
            previous: None,
            pending: None,
            fade: 0,
            send: [[0.0; 64]; 2],
            predelay: vec![[0.0; 2]; 16384],
            delay_position: 0,
            smooth_delay: 18.0 * 44.1,
            high_state: [0.0; 2],
            low_state: [0.0; 2],
            smooth_sends: [0.0; 4],
            layered_base_send: [0.0; 2],
            smooth_return: 0.0,
            wet_remaining: 0,
            ducker: crate::sidechain::Ducker::default(),
            lane_duckers: std::array::from_fn(|_| crate::sidechain::Ducker::default()),
        }
    }

    pub fn replace_settings(&mut self, settings: Box<Settings>) -> Box<Settings> {
        self.replace_prepared_settings(PreparedSettings::new(settings))
    }

    pub fn replace_prepared_settings(&mut self, prepared: PreparedSettings) -> Box<Settings> {
        let settings = prepared.into_inner();
        for (index, next) in settings.synth_slots.iter().enumerate() {
            let old = &self.settings.synth_slots[index];
            if old.phrase_fingerprint() != next.phrase_fingerprint()
                || old.sequence_enabled != next.sequence_enabled {
                self.synths[index].phrase_changed();
            }
            if old.enabled && !next.enabled {
                self.synths[index].live_event(acid_live::Event::Panic, 0.0, 0.0);
            }
        }
        std::mem::replace(&mut self.settings, settings)
    }

    /// Returns any already-retiring processor to be destroyed on the UI thread.
    pub fn replace_reverb(&mut self, next: Box<Convolver>) -> Option<Box<Convolver>> {
        if self.fade > 0 || self.pending.is_some() {
            return self.pending.replace(next);
        }
        self.fade = 4410;
        self.previous
            .replace(std::mem::replace(&mut self.convolver, next))
    }

    pub fn retired(&mut self) -> Option<Box<Convolver>> {
        if self.fade > 0 {
            return None;
        }
        let old = self.previous.take();
        if let Some(next) = self.pending.take() {
            self.previous = Some(std::mem::replace(&mut self.convolver, next));
            self.fade = 4410;
        }
        old
    }

    pub fn live_note(&mut self, slot: usize, event: acid_live::Event, tick: f64, advance: f64) {
        if let Some(instrument) = self.synths.get_mut(slot)
            && self.settings.synth_slots[slot].enabled
        {
            instrument.live_event(event, tick, advance);
        }
    }

    pub(crate) fn synth_channel(&mut self, c: usize, bus: &mut crate::frame::Block, tick: f64, advance: f64, play: bool) {
        if c < 2 {
            self.synths[c].render(&self.synth_bank, &self.settings.synth_slots[c], bus, tick, advance, play);
        }
    }

    pub(crate) fn layered_synth_tap(&mut self, c: usize) -> Option<&mut crate::frame::Block> {
        let slot = self.settings.synth_slots.get(c)?;
        (slot.enabled && slot.tb_active()).then(|| &mut self.synths[c].dry)
    }

    pub(crate) fn strip_sample(&mut self, c: usize, j: usize, mono: f32, gl: f64, gr: f64) -> (f64, f64) {
        let (mut left, mut right) = (f64::from(mono) * gl, f64::from(mono) * gr);
        let mut isolated = None;
        if c < 2 && self.settings.synth_slots[c].enabled {
            let slot = &self.settings.synth_slots[c];
            let layered = slot.tb_active();
            let synth = if layered { self.synths[c].dry[j] } else { mono };
            let side = self.synths[c].spread(synth, slot.macros[2]);
            left += side * gl;
            right -= side * gr;
            if layered {
                isolated = Some(((f64::from(synth) + side) * gl, (f64::from(synth) - side) * gr));
            }
        }
        self.route_channel(c, j, left, right, isolated);
        (left, right)
    }

    pub(crate) fn synth_gate(&self, c: usize, tick: f64, native: bool) -> bool {
        let slot = &self.settings.synth_slots[c];
        (slot.tb_active() && native) || (slot.enabled && self.synths[c].gate(tick))
    }

    pub fn begin(&mut self) {
        self.send = [[0.0; 64]; 2];
    }

    /// Smoothed output trim after inserts, before meters and sends; legacy frames skip it.
    pub fn trim_drum(&mut self, channel: usize, audio: &mut crate::frame::Block) {
        let Some(index) = channel.checked_sub(2).filter(|&i| i < 2) else {
            return;
        };
        let db = self.settings.drum_gain_db[index];
        let target = if db.is_finite() {
            10.0_f64.powf(db.clamp(-12.0, 12.0) / 20.0)
        } else {
            1.0
        };
        for sample in audio {
            smooth(&mut self.drum_gain[index], target, 1.0 / 220.5);
            if self.drum_gain[index] != 1.0 {
                *sample = (*sample as f64 * self.drum_gain[index]) as f32;
            }
        }
    }

    pub fn modulation(&mut self, mut data: Modulation) {
        // GLOBAL lanes preserve the shared source and release; explicit lane routes override
        // only their machine and voice. TB depth controls stay independent from sampler depth.
        data.config.depth = [65; 2];
        self.ducker.set_config(data.config);
        let duck = self.ducker.gains_for_voices(data.drum_counts, data.audible);
        self.sampler.set_modulation(data.gates, duck[0]);
        for (i, ducker) in self.lane_duckers.iter_mut().enumerate() {
            let lane = &self.settings.lanes[i];
            let mut config = data.config;
            config.enabled = lane.sidechain && lane.sidechain_source != 0;
            config.source = match lane.sidechain_source {
                1 => crate::sidechain::Source::Tr808,
                2 => crate::sidechain::Source::Tr909,
                _ => crate::sidechain::Source::Both,
            };
            config.instrument = lane.sidechain_instrument;
            ducker.set_config(config);
            // Track all counters even in GLOBAL mode: choosing a new source must not replay hits.
            let gains = ducker.gains_for_voices(data.drum_counts, data.audible);
            if lane.sidechain_source != 0 {
                self.sampler.set_lane_duck(i, gains[0]);
            }
        }
        self.sampler
            .set_drum_modulation(data.drum_triggers, data.ticks_per_frame);
    }

    pub fn channel(&mut self, channel: usize, sample: usize, left: f64, right: f64) {
        self.route_channel(channel, sample, left, right, None);
    }

    fn route_channel(&mut self, channel: usize, sample: usize, left: f64, right: f64,
        isolated_synth: Option<(f64, f64)>) {
        self.meters[channel].sample(left);
        self.meters[channel].sample(right);
        let target = if self.settings.enabled {
            let send = self.settings.sends[channel];
            if channel < 2 && self.settings.synth_slots[channel].enabled {
                let space = f64::from(self.settings.synth_slots[channel].macros[3]);
                send + (1.0 - send) * space
            } else { send }
        } else {
            0.0
        };
        smooth(&mut self.smooth_sends[channel], target, 0.002);
        let full = self.smooth_sends[channel];
        if let Some((synth_left, synth_right)) = isolated_synth {
            // Strip send includes both instruments after inserts. Only AcidSynth contributes
            // the extra send supplied by the patch's Space macro, via its pre-insert tap.
            let base = if self.settings.enabled { self.settings.sends[channel] } else { 0.0 };
            smooth(&mut self.layered_base_send[channel], base, 0.002);
            let shared = self.layered_base_send[channel];
            self.send[0][sample] += left * shared + synth_left * (full - shared);
            self.send[1][sample] += right * shared + synth_right * (full - shared);
        } else {
            self.send[0][sample] += left * full;
            self.send[1][sample] += right * full;
            if channel < 2 {
                self.layered_base_send[channel] = if self.settings.enabled {
                    self.settings.sends[channel].min(full)
                } else { 0.0 };
            }
        }
    }

    pub fn process(
        &mut self,
        tick: f64,
        ticks_per_frame: f64,
        playing: bool,
        left: &mut [f32; 64],
        right: &mut [f32; 64],
    ) {
        self.prepare_sampler(tick, ticks_per_frame, playing);
        self.route_sampler(None, &mut [0.0; 64]);
        self.mix(left, right);
    }

    /// Render the sampler before the shared native delay, retaining per-lane SPACE sends.
    pub fn prepare_sampler(&mut self, tick: f64, ticks_per_frame: f64, playing: bool) {
        self.sampler_bus.clear();
        self.sampler.process(
            &self.settings.lanes,
            tick,
            ticks_per_frame,
            playing,
            &mut self.sampler_bus.dry,
            &mut self.sampler_bus.reverb,
        );
    }

    pub fn route_sampler(
        &mut self,
        distortion: Option<(
            &dyn crate::frame::InsertEffect,
            &crate::frame::BlockContext<'_>,
        )>,
        delay: &mut crate::frame::Block,
    ) {
        self.sampler_bus
            .route(self.settings.sampler_bus, distortion, delay, &mut self.send);
    }

    pub fn sampler_meter(&self) -> [u8; 2] {
        self.sampler_bus.meter()
    }

    pub fn meter_readings(&self) -> [meter::Reading; 8] {
        self.meters.map(|m| m.reading())
    }

    /// Every sample after native master compression/fader, immediately before int16 conversion.
    pub fn meter_output(&mut self, left: &[f32; 64], right: &[f32; 64]) {
        for (c, bus) in [left, right].into_iter().enumerate() {
            for &sample in bus {
                self.meters[6 + c].sample(f64::from(sample));
            }
            for &sample in &self.sampler_bus.dry[c] {
                self.meters[4 + c].sample(sample);
            }
        }
        for meter in &mut self.meters {
            meter.finish();
        }
    }

    /// Add the prepared sampler and convolution return after the shared native delay.
    pub fn mix(&mut self, left: &mut [f32; 64], right: &mut [f32; 64]) {
        let delay = self.settings.predelay_ms * 44.1;
        let hp = (-std::f64::consts::TAU * self.settings.low_cut / 44100.0).exp();
        let lp = (-std::f64::consts::TAU * self.settings.high_cut / 44100.0).exp();
        for j in 0..64 {
            self.predelay[self.delay_position] = [self.send[0][j], self.send[1][j]];
            smooth(&mut self.smooth_delay, delay, 0.001);
            let whole = self.smooth_delay.floor() as usize;
            let fraction = self.smooth_delay - whole as f64;
            let a = self.predelay[(self.delay_position + 16384 - whole) & 16383];
            let b = self.predelay[(self.delay_position + 16383 - whole) & 16383];
            let input: [f64; 2] =
                std::array::from_fn(|c| a[c] * (1.0 - fraction) + b[c] * fraction);
            self.delay_position = (self.delay_position + 1) & 16383;
            for (c, &sample) in input.iter().enumerate() {
                self.high_state[c] = clean(hp * self.high_state[c] + (1.0 - hp) * sample);
                let high = sample - self.high_state[c];
                self.low_state[c] = clean(lp * self.low_state[c] + (1.0 - lp) * high);
                self.send[c][j] = self.low_state[c];
            }
        }
        let mut wet = [[0.0; 64]; 2];
        if self.send.iter().flatten().any(|&x| x != 0.0) {
            // Include the maximum supported IR and partition padding before sleeping.
            self.wet_remaining = 6 * 44100 + 4096;
        }
        if self.wet_remaining > 0 {
            self.convolver.process(&self.send, &mut wet);
            self.wet_remaining = self.wet_remaining.saturating_sub(64);
        }
        if self.fade > 0 {
            let mut old = [[0.0; 64]; 2];
            if let Some(previous) = &mut self.previous {
                previous.process(&self.send, &mut old);
            }
            for j in 0..64 {
                let mix = self.fade as f64 / 4410.0;
                for c in 0..2 {
                    wet[c][j] = wet[c][j] * (1.0 - mix) + old[c][j] * mix;
                }
                self.fade = self.fade.saturating_sub(1);
            }
        }
        let target = if self.settings.enabled {
            self.settings.return_gain
        } else {
            0.0
        };
        for j in 0..64 {
            smooth(&mut self.smooth_return, target, 0.002);
            let mid = (wet[0][j] + wet[1][j]) * 0.5;
            let side = (wet[0][j] - wet[1][j]) * 0.5 * self.settings.width;
            let add = [
                (mid + side) * self.smooth_return + self.sampler_bus.dry[0][j],
                (mid - side) * self.smooth_return + self.sampler_bus.dry[1][j],
            ];
            if add[0] != 0.0 {
                left[j] = (f64::from(left[j]) + add[0]) as f32;
            }
            if add[1] != 0.0 {
                right[j] = (f64::from(right[j]) + add[1]) as f32;
            }
        }
    }
}
