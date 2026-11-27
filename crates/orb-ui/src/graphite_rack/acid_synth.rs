//! AcidSynth's live controls and polyphonic step editor for the two instrument slots.

use super::{ACID, App, DIM, INK, NOTES, Rack, Ui, button, egui, mix_knob};
use orb_engine::studio::acid_presets::PRESETS;
use orb_engine::studio::acid_synth::{Slot, Step};

const RATES: [(u16, &str); 5] = [
    (48, "1/16"), (96, "1/8"), (192, "1/4"), (384, "1/2"), (768, "1 bar"),
];
const MACROS: [(&str, &str); 4] = [
    ("timbre", "TIMBRE"), ("movement", "MOVEMENT"), ("width", "WIDTH"), ("space", "SPACE"),
];

fn pitch_name(note: u8) -> String {
    if note == 0 {
        return "Rest".into();
    }
    format!("{}{}", NOTES[usize::from(note % 12)], i16::from(note / 12) - 1)
}

/// Accept the pitch names shown by the editor, as well as a numeric MIDI pitch.
fn parse_pitch(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("rest") {
        return Some(0.0);
    }
    if let Ok(note @ 0..=127) = text.parse::<u8>() {
        return Some(f64::from(note));
    }
    let mut chars = text.chars();
    let pitch = match chars.next()?.to_ascii_uppercase() {
        'C' => 0, 'D' => 2, 'E' => 4, 'F' => 5,
        'G' => 7, 'A' => 9, 'B' => 11, _ => return None,
    };
    let suffix = chars.as_str();
    let (accidental, octave) = if let Some(octave) = suffix.strip_prefix('#') {
        (1, octave)
    } else if let Some(octave) = suffix.strip_prefix('b') {
        (-1, octave)
    } else {
        (0, suffix)
    };
    let octave = octave.parse::<i8>().ok()?;
    let note = (i16::from(octave) + 1) * 12 + pitch + accidental;
    (1..=127).contains(&note).then(|| f64::from(note))
}

fn step_id(row: usize) -> egui::Id {
    egui::Id::unique(("acid-selected-step", row))
}

fn apply_preset(slot: &mut Slot, index: usize) {
    slot.preset = index as u8;
    slot.patch = PRESETS[index].1;
}

impl App {
    pub(super) fn rack_acid_synth(&mut self, ui: &mut Ui, rack: Rack, row: usize, y: f32) {
        let slot = &mut self.studio.settings.synth_slots[row];
        let mut changed = false;
        rack.text(ui, 30.0, y + 48.0, 9.0, "SOUND", DIM);
        ui.scope_builder(
            egui::UiBuilder::new()
                .id_salt(("acid-preset", row))
                .max_rect(rack.rect(28.0, y + 60.0, 176.0, 24.0)),
            |ui| {
                let preset = usize::from(slot.preset).min(PRESETS.len() - 1);
                egui::ComboBox::from_id_salt("preset")
                    .width(ui.available_width())
                    .selected_text(PRESETS[preset].0)
                    .show_ui(ui, |ui| {
                        for (index, (name, _)) in PRESETS.iter().enumerate() {
                            if ui.selectable_label(index == preset, *name).clicked() {
                                apply_preset(slot, index);
                                changed = true;
                            }
                        }
                    });
            },
        );
        rack.text(ui, 30.0, y + 105.0, 9.0, "STEP RATE", DIM);
        for (n, &(ticks, label)) in RATES.iter().enumerate() {
            if button(
                ui, rack, rack.rect(28.0 + n as f32 * 36.0, y + 123.0, 34.0, 21.0),
                (row, "acid-rate", ticks), label, slot.step_ticks == ticks,
            ).clicked() && slot.step_ticks != ticks {
                slot.step_ticks = ticks;
                changed = true;
            }
        }
        for (i, &(key, label)) in MACROS.iter().enumerate() {
            let mut value = (slot.macros[i] * 1000.0).round() as i32;
            let response = mix_knob(
                ui, rack.scale, &format!("acidsynth{}.{key}", row + 1), label,
                rack.rect(247.0 + i as f32 * 152.0, y + 38.0, 60.0, 71.0), &mut value,
            );
            if response.changed() {
                slot.macros[i] = value as f32 / 1000.0;
                changed = true;
            }
        }
        let editor_id = egui::Id::unique("graphite-edit");
        if button(
            ui, rack, rack.rect(755.0, y + 9.0, 77.0, 19.0),
            (row, "acid-edit"), "EDIT SOUND", false,
        ).clicked() {
            self.panel.focus = row;
            ui.ctx().data_mut(|d| d.insert_temp(editor_id, Some(row)));
        }
        let cursor = self.panel.transport.position.max(0) as usize
            / usize::from(slot.step_ticks.max(1)) % slot.steps.len();
        for (index, step) in slot.steps.iter().enumerate() {
            let root = step.notes.iter().copied().find(|&n| n != 0);
            let label = root.map(pitch_name).unwrap_or_else(|| "–".into());
            let response = button(
                ui, rack, rack.rect(224.0 + index as f32 * 38.0, y + 126.0, 34.0, 27.0),
                (row, "acid-step", index), &label, root.is_some(),
            ).on_hover_text(format!("Step {} · Click to edit notes and duration", index + 1));
            if response.clicked() {
                self.panel.focus = row;
                ui.ctx().data_mut(|d| {
                    d.insert_temp(step_id(row), index);
                    d.insert_temp(editor_id, Some(row));
                });
            }
            if self.panel.transport.playing && cursor == index {
                ui.painter().rect_filled(
                    rack.rect(225.0 + index as f32 * 38.0, y + 120.0, 32.0, 2.0), 0.0, ACID,
                );
            }
        }
        rack.text(ui, 231.0, y + 19.0, 8.0, "POLYPHONIC WAVETABLE", INK);
        if changed {
            self.studio.changed();
            self.doc.modified = true;
        }
    }

    pub(super) fn acid_editor(&mut self, ui: &mut Ui, row: usize, id: egui::Id) {
        let slot = &mut self.studio.settings.synth_slots[row];
        let mut changed = false;
        let mut open = true;
        let title = format!("AcidSynth {} / Editor", ["A", "B"][row]);
        egui::Window::new(title).open(&mut open).resizable(true).vscroll(true)
            .default_width(490.0).show(ui.ctx(), |ui| {
                ui.heading("Sound");
                let patch = &mut slot.patch;
                egui::Grid::new(("acid-patch", row)).num_columns(2).show(ui, |ui| {
                    for (label, param) in [
                        ("Oscillator A / Table position", &mut patch.wave_a),
                        ("Oscillator B / Table position", &mut patch.wave_b),
                        ("Oscillator balance", &mut patch.mix),
                        ("Detune", &mut patch.detune),
                        ("Filter cutoff", &mut patch.cutoff),
                        ("Filter resonance", &mut patch.resonance),
                        ("Attack", &mut patch.attack),
                        ("Release", &mut patch.release),
                        ("LFO rate", &mut patch.lfo_rate),
                        ("LFO depth", &mut patch.lfo_depth),
                    ] {
                        ui.label(label);
                        changed |= ui.add(egui::Slider::new(param, 0.0..=1.0)).changed();
                        ui.end_row();
                    }
                });
                ui.separator();
                let mut index = ui.ctx().data(|d| d.get_temp::<usize>(step_id(row))).unwrap_or(0).min(15);
                ui.horizontal(|ui| {
                    ui.heading("Chord");
                    if ui.button("‹").clicked() { index = (index + 15) % 16; }
                    ui.label(format!("Step {:02}", index + 1));
                    if ui.button("›").clicked() { index = (index + 1) % 16; }
                });
                ui.ctx().data_mut(|d| d.insert_temp(step_id(row), index));
                changed |= edit_chord(ui, &mut slot.steps[index]);
            });
        if changed {
            self.studio.changed();
            self.doc.modified = true;
        }
        if !open {
            ui.ctx().data_mut(|d| d.insert_temp(id, None::<usize>));
        }
    }
}

fn edit_chord(ui: &mut Ui, step: &mut Step) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for (label, offsets) in [
            ("Minor", &[0, 3, 7][..]), ("Major", &[0, 4, 7][..]),
            ("Min 7", &[0, 3, 7, 10][..]), ("Sus 2", &[0, 2, 7][..]),
        ] {
            if ui.button(label).clicked() {
                let root = step.notes.iter().copied().find(|&n| n != 0).unwrap_or(48)
                    .min(127 - offsets[offsets.len() - 1]);
                step.notes = [0; 4];
                for (note, &offset) in step.notes.iter_mut().zip(offsets) {
                    *note = root + offset;
                }
                changed = true;
            }
        }
        if ui.button("Clear").clicked() {
            step.notes = [0; 4];
            changed = true;
        }
    });
    ui.horizontal_wrapped(|ui| {
        for note in &mut step.notes {
            changed |= ui.add(egui::DragValue::new(note).range(0..=127).speed(0.15)
                .custom_formatter(|v, _| pitch_name(v.round() as u8)).custom_parser(parse_pitch))
                .on_hover_text("Drag to transpose, or click and type a note such as D3 or F#4. Type 0 for a rest.").changed();
        }
    });
    changed |= ui.add(egui::Slider::new(&mut step.gate_steps, 1..=16).text("Duration in steps")).changed();
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphite_rack::{HEIGHT, ROWS, WIDTH};
    use crate::model::Panel;
    use egui::{Context, Pos2, Rect, Vec2};

    #[test]
    fn editable_pitch_names_round_trip() {
        for note in 0..=127 {
            assert_eq!(parse_pitch(&pitch_name(note)), Some(f64::from(note)));
        }
        assert_eq!(parse_pitch("Db4"), Some(61.0));
        assert_eq!(parse_pitch("128"), None);
        assert_eq!(parse_pitch("G20"), None);
    }

    fn click(ctx: &Context, app: &mut App, canvas: Rect, pos: Pos2) {
        for down in [None, Some(true), Some(false)] {
            let mut events = vec![egui::Event::PointerMoved(pos)];
            if let Some(pressed) = down {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                });
            }
            let mut output = ctx.run_ui(
                egui::RawInput { screen_rect: Some(canvas), events, ..Default::default() },
                |ui| { app.graphite_rack(ui, canvas); },
            );
            output.textures_delta.clear();
        }
    }

    #[test]
    fn rack_switch_preserves_each_slots_sequence_and_the_tb_pattern() {
        let ctx = Context::default();
        let mut app = App::new(Panel::new(), None);
        let canvas = Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDTH, HEIGHT));
        let rack = Rack::fit(canvas);
        let y = ROWS[1].0;
        let switch = rack.rect(140.0, y + 9.0, 73.0, 19.0).center();
        let tb_pattern = app.panel.pattern_bytes(1);
        let slot_a = app.studio.settings.synth_slots[0].clone();
        click(&ctx, &mut app, canvas, switch);
        assert!(app.studio.settings.synth_slots[1].enabled);
        assert!(app.studio.content && app.doc.modified);
        click(&ctx, &mut app, canvas, rack.rect(28.0, y + 123.0, 34.0, 21.0).center());
        assert_eq!(app.studio.settings.synth_slots[1].step_ticks, 48);
        let before = app.studio.settings.synth_slots[1].clone();
        click(&ctx, &mut app, canvas, switch);
        assert!(!app.studio.settings.synth_slots[1].enabled);
        click(&ctx, &mut app, canvas, switch);
        assert_eq!(app.studio.settings.synth_slots[1], before);
        assert_eq!(app.studio.settings.synth_slots[0], slot_a);
        assert_eq!(app.panel.pattern_bytes(1), tb_pattern);
    }
}
