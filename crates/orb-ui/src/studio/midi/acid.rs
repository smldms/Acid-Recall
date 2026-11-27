//! The editor and MIDI router share stable IDs for each AcidSynth slot.
use orb_engine::studio::{acid_presets::PRESETS, acid_synth::Slot};

#[derive(Clone, Copy)]
pub(super) enum Domain {
    Unit,
    Ratio,
    Bipolar,
    Semitones,
    Choice(u8),
    Switch,
    Action,
}

impl Domain {
    fn decode(self, raw: u8) -> f32 {
        let n = f32::from(raw) / 127.0;
        match self {
            Self::Unit => n,
            Self::Ratio => 0.25 + n * 11.75,
            Self::Bipolar => if raw < 64 { (f32::from(raw) - 64.0) / 64.0 } else { (f32::from(raw) - 64.0) / 63.0 },
            Self::Semitones => (48.0 * n).round() - 24.0,
            Self::Choice(max) => (f32::from(max) * n).round(),
            Self::Switch => f32::from(u8::from(raw >= 64)),
            Self::Action => 0.0,
        }
    }

    fn encode(self, value: f32) -> u8 {
        let scaled = match self {
            Self::Unit | Self::Switch => 127.0 * value,
            Self::Ratio => (value - 0.25) * 127.0 / 11.75,
            Self::Bipolar => 64.0 + value * if value < 0.0 { 64.0 } else { 63.0 },
            Self::Semitones => (value + 24.0) * 127.0 / 48.0,
            Self::Choice(max) => value * 127.0 / f32::from(max.max(1)),
            Self::Action => 0.0,
        };
        scaled.round().clamp(0.0, 127.0) as u8
    }
}

pub(super) struct Field {
    pub id: &'static str,
    pub label: &'static str,
    pub domain: Domain,
    get: fn(&Slot) -> f32,
    set: fn(&mut Slot, f32),
}

impl Field {
    pub fn cc(&self, slot: &Slot) -> u8 {
        self.domain.encode((self.get)(slot))
    }

    pub fn represents(&self, slot: &Slot, raw: u8) -> bool {
        (self.get)(slot) == self.domain.decode(raw)
    }

    pub fn apply(&self, slot: &mut Slot, raw: u8) {
        let value = self.domain.decode(raw);
        if (self.get)(slot) != value {
            (self.set)(slot, value);
        }
    }
}

macro_rules! field {
    ($id:literal, $label:literal, $domain:expr, $($path:tt)+) => {
        Field { id: $id, label: $label, domain: $domain,
            get: |slot| slot.$($path)+ as f32,
            set: |slot, value| slot.$($path)+ = value as _,
        }
    };
}
use Domain::{Bipolar, Choice, Ratio, Semitones, Unit};

pub(super) static FIELDS: &[Field] = &[
    field!("timbre", "Timbre", Unit, macros[0]),
    field!("movement", "Movement", Unit, macros[1]),
    field!("width", "Width", Unit, macros[2]),
    field!("space", "Space", Unit, macros[3]),
    field!("wave_a", "Oscillator A table", Unit, patch.wave_a),
    field!("wave_b", "Oscillator B table", Unit, patch.wave_b),
    field!("mix", "Oscillator balance", Unit, patch.mix),
    field!("detune", "Detune", Unit, patch.detune),
    field!("attack", "Attack", Unit, patch.attack),
    field!("decay", "Decay", Unit, patch.decay),
    field!("sustain", "Sustain", Unit, patch.sustain),
    field!("release", "Release", Unit, patch.release),
    field!("cutoff", "Filter cutoff", Unit, patch.cutoff),
    field!("resonance", "Filter resonance", Unit, patch.resonance),
    field!("filter_env", "Filter envelope depth", Unit, patch.filter_env),
    field!("sub", "Sub oscillator", Unit, patch.sub),
    field!("drive", "Drive", Unit, patch.drive),
    field!("chorus", "Chorus", Unit, patch.chorus),
    field!("lfo_rate", "LFO rate", Unit, patch.lfo_rate),
    field!("lfo_depth", "LFO depth", Unit, patch.lfo_depth),
    field!("fm_index", "FM index", Unit, patch.fm_index),
    field!("fm_feedback", "FM feedback", Unit, patch.fm_feedback),
    field!("fold", "Wavefold amount", Unit, patch.fold),
    field!("sync", "Sync amount", Unit, patch.sync),
    field!("osc_mode", "Oscillator engine", Choice(4), patch.osc_mode),
    field!("filter_mode", "Filter mode", Choice(3), patch.filter_mode),
    field!("lfo_target", "LFO destination", Choice(3), patch.lfo_target),
    field!("lfo_shape", "LFO shape", Choice(3), patch.lfo_shape),
    field!("fm_algorithm", "FM algorithm", Choice(3), patch.fm_algorithm),
    field!("transpose_b", "Oscillator B transpose", Semitones, patch.transpose_b),
    field!("fm_ratio2", "FM operator 2 ratio", Ratio, patch.fm_ratios[0]),
    field!("fm_ratio3", "FM operator 3 ratio", Ratio, patch.fm_ratios[1]),
    field!("fm_ratio4", "FM operator 4 ratio", Ratio, patch.fm_ratios[2]),
    field!("route1.source", "Route 1 source", Choice(6), patch.routes[0].source),
    field!("route1.dest", "Route 1 destination", Choice(5), patch.routes[0].dest),
    field!("route1.amount", "Route 1 amount", Bipolar, patch.routes[0].amount),
    field!("route2.source", "Route 2 source", Choice(6), patch.routes[1].source),
    field!("route2.dest", "Route 2 destination", Choice(5), patch.routes[1].dest),
    field!("route2.amount", "Route 2 amount", Bipolar, patch.routes[1].amount),
    field!("route3.source", "Route 3 source", Choice(6), patch.routes[2].source),
    field!("route3.dest", "Route 3 destination", Choice(5), patch.routes[2].dest),
    field!("route3.amount", "Route 3 amount", Bipolar, patch.routes[2].amount),
    field!("route4.source", "Route 4 source", Choice(6), patch.routes[3].source),
    field!("route4.dest", "Route 4 destination", Choice(5), patch.routes[3].dest),
    field!("route4.amount", "Route 4 amount", Bipolar, patch.routes[3].amount),
    Field { id: "preset", label: "Factory preset", domain: Choice((PRESETS.len() - 1) as u8),
        get: |slot| f32::from(slot.preset), set: |slot, value| slot.select_factory_preset(value as usize) },
    Field { id: "sequence_enabled", label: "Sequence + keys", domain: Domain::Switch,
        get: |slot| f32::from(u8::from(slot.sequence_enabled)), set: |slot, value| slot.sequence_enabled = value != 0.0 },
    Field { id: "audition", label: "Audition sound", domain: Domain::Action,
        get: |_| 0.0, set: |_, _| {} },
];

pub(super) fn target(id: &str) -> Option<(usize, &'static Field)> {
    let (slot, field) = id.split_once('.')?;
    let row = match slot { "acidsynth1" => 0, "acidsynth2" => 1, _ => return None };
    Some((row, FIELDS.iter().find(|parameter| parameter.id == field)?))
}
