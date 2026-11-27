//! The editor and MIDI router share stable IDs for each AcidSynth slot.
use orb_engine::studio::{acid_presets::PRESETS, acid_synth::{LFO_DIVISIONS, Slot}};

#[derive(Clone, Copy)]
pub(crate) enum Domain {
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

pub(crate) struct Field {
    pub id: &'static str,
    pub label: &'static str,
    pub domain: Domain,
    get: fn(&Slot) -> f32,
    set: fn(&mut Slot, f32),
}

impl Field {
    pub fn position(&self, slot: &Slot) -> f32 {
        let value = (self.get)(slot);
        match self.domain {
            Domain::Unit | Domain::Switch => value,
            Domain::Ratio => (value - 0.25) / 11.75,
            Domain::Bipolar => (value + 1.0) / 2.0,
            Domain::Semitones => (value + 24.0) / 48.0,
            Domain::Choice(max) => value / f32::from(max.max(1)),
            Domain::Action => 0.0,
        }
    }

    pub fn set_position(&self, slot: &mut Slot, position: f32) -> bool {
        let n = position.clamp(0.0, 1.0);
        let value = match self.domain {
            Domain::Unit => n,
            Domain::Ratio => 0.25 + n * 11.75,
            Domain::Bipolar => n * 2.0 - 1.0,
            Domain::Semitones => (n * 48.0).round() - 24.0,
            Domain::Choice(max) => (n * f32::from(max)).round(),
            Domain::Switch => f32::from(u8::from(n >= 0.5)),
            Domain::Action => return false,
        };
        if (self.get)(slot) == value { return false; }
        (self.set)(slot, value);
        true
    }

    pub fn display(&self, slot: &Slot) -> String {
        let value = (self.get)(slot);
        let index = value.max(0.0) as usize;
        if self.id == "lfo_division" { return LFO_DIVISIONS[index.min(LFO_DIVISIONS.len()-1)].0.into(); }
        if self.id == "preset" {
            if !slot.user_preset.is_empty() { return slot.user_preset.clone(); }
            return PRESETS.get(index).map_or("?", |preset| preset.0).into();
        }
        let choices: &[&str] = match self.id {
            "osc_mode" => &["Blend", "Ring", "Phase", "FM4", "Sync Fold"],
            "filter_mode" => &["LP", "BP", "HP", "Notch"],
            "lfo_target" => &["Colour", "Filter", "Pitch", "Mix"],
            "lfo_shape" => &["Sine", "Tri", "Ramp", "Square"],
            "fm_algorithm" => &["Cascade", "Pairs", "Fan", "Branch"],
            key if key.ends_with(".source") => &["Off", "LFO", "Amp env", "Timbre", "Motion", "Width", "Space"],
            key if key.ends_with(".dest") => match slot.patch.osc_mode {
                3 => &["FM index", "Feedback", "Cutoff", "Mix", "Pitch", "Drive"],
                4 => &["Fold", "Sync", "Cutoff", "Mix", "Pitch", "Drive"],
                _ => &["Wave A", "Wave B", "Cutoff", "Mix", "Pitch", "Drive"],
            },
            _ => &[],
        };
        if let Some(label) = choices.get(index) { return (*label).into(); }
        match self.domain {
            Domain::Unit => format!("{:.0}%", value * 100.0),
            Domain::Ratio => format!("{value:.2}×"),
            Domain::Bipolar => format!("{:+.0}%", value * 100.0),
            Domain::Semitones => format!("{value:+.0} st"),
            Domain::Choice(_) => format!("{}", index + 1),
            Domain::Switch => if value == 0.0 { "Off" } else { "On" }.into(),
            Domain::Action => "Play".into(),
        }
    }

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
    field!("lfo_sync", "LFO tempo sync", Domain::Switch, patch.lfo_sync),
    field!("lfo_division", "LFO tempo division", Choice((LFO_DIVISIONS.len() - 1) as u8), patch.lfo_division),
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

pub(crate) fn target(id: &str) -> Option<(usize, &'static Field)> {
    let (slot, field) = id.split_once('.')?;
    let row = match slot { "acidsynth1" => 0, "acidsynth2" => 1, _ => return None };
    Some((row, FIELDS.iter().find(|parameter| parameter.id == field)?))
}

/// Project UI state, never sent across the real-time audio boundary.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct MainPanel([Vec<String>; 2]);

pub(crate) const MAX_PINS: usize = 16;

impl MainPanel {
    pub fn is_default(&self) -> bool { self.0.iter().all(Vec::is_empty) }

    pub fn fields(&self, row: usize) -> impl Iterator<Item = &'static Field> + '_ {
        self.0[row].iter().filter_map(|key| FIELDS.iter().find(|f| f.id == key))
    }

    pub fn count(&self, row: usize) -> usize { self.0[row].len() }

    pub fn has(&self, row: usize, field: &Field) -> bool {
        self.0[row].iter().any(|key| key == field.id)
    }

    pub fn update(&mut self, row: usize, field: &Field, add: bool) -> bool {
        let entries = &mut self.0[row];
        let at = entries.iter().position(|key| key == field.id);
        match (add, at) {
            (true, None) if entries.len() < MAX_PINS => entries.push(field.id.into()),
            (false, Some(i)) => { entries.remove(i); }
            _ => return false,
        }
        true
    }

    pub fn sanitize(&mut self) {
        for row in &mut self.0 {
            let mut seen = Vec::with_capacity(MAX_PINS);
            for id in row.drain(..) {
                if seen.len() == MAX_PINS { break; }
                if !seen.contains(&id) && FIELDS.iter().any(|f| f.id == id) { seen.push(id); }
            }
            *row = seen;
        }
    }
}

#[cfg(test)]
mod pin_tests {
    use super::*;

    #[test]
    fn sixteen_unique_pins_per_slot_survive_duplicate_and_excess_input() {
        let mut panel = MainPanel::default();
        for field in FIELDS.iter().take(MAX_PINS) {
            assert!(panel.update(0, field, true));
            assert!(!panel.update(0, field, true));
        }
        assert!(!panel.update(0, &FIELDS[MAX_PINS], true));
        assert!(panel.update(1, &FIELDS[MAX_PINS], true));
        assert!(panel.update(0, &FIELDS[1], false));
        assert!(panel.update(0, &FIELDS[MAX_PINS], true));
        panel.0[0].insert(0, "unrecognised".into());
        panel.0[0].insert(2, FIELDS[0].id.into());
        panel.0[0].push(FIELDS[1].id.into());
        panel.sanitize();
        assert_eq!(panel.count(0), MAX_PINS);
        assert_eq!(panel.fields(0).last().unwrap().id, FIELDS[MAX_PINS].id);
        assert_eq!(panel.count(1), 1);
    }
}
