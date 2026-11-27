//! AcidSynth's live controls and polyphonic step editor for the two instrument slots.

use super::{ACID, App, DIM, NOTES, Rack, Ui, button, egui, mix_knob};
use orb_engine::studio::acid_presets::{EXPERIMENTAL_START, PRESETS};
use orb_engine::studio::acid_synth::{PhraseLength, Slot};

const RATES: [(u16, &str); 5] = [
    (48, "1/16"), (96, "1/8"), (192, "1/4"), (384, "1/2"), (768, "1 bar"),
];
const MACROS: [(&str, &str); 4] = [
    ("timbre", "TIMBRE"), ("movement", "MOVEMENT"), ("width", "WIDTH"), ("space", "SPACE"),
];

pub(super) fn pitch_name(note: u8) -> String {
    if note == 0 {
        return "Rest".into();
    }
    format!("{}{}", NOTES[usize::from(note % 12)], i16::from(note / 12) - 1)
}

/// Accept the pitch names shown by the editor, as well as a numeric MIDI pitch.
pub(super) fn parse_pitch(text: &str) -> Option<f64> {
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

pub(super) fn apply_preset(slot: &mut Slot, index: usize) {
    slot.select_factory_preset(index);
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
                            if index == 0 {
                                ui.strong("Factory");
                            } else if index == EXPERIMENTAL_START {
                                ui.separator();
                                ui.strong("Experimental");
                            }
                            if ui.selectable_label(index == preset, *name).clicked() {
                                apply_preset(slot, index);
                                changed = true;
                            }
                        }
                    });
            },
        );
        if button(ui, rack, rack.rect(101.0, y + 36.0, 104.0, 22.0),
            (row, "acid-keys-only"),
            if slot.sequence_enabled { "SEQUENCE + KEYS" } else { "KEYS ONLY" },
            !slot.sequence_enabled,
        ).on_hover_text("Switch between sequencer + MIDI keyboard and MIDI keyboard only.").clicked() {
            slot.sequence_enabled = !slot.sequence_enabled;
            changed = true;
        }
        rack.text(ui, 30.0, y + 105.0, 9.0, "STEP RATE", DIM);
        for (n, &(ticks, label)) in RATES.iter().enumerate() {
            if button(
                ui, rack, rack.rect(28.0 + n as f32 * 36.0, y + 123.0, 34.0, 21.0),
                (row, "acid-rate", ticks), label, slot.ticks_per_step() == ticks,
            ).clicked() && slot.ticks_per_step() != ticks {
                slot.set_step_rate(ticks);
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
        let chord_id = egui::Id::unique("acid-chord-editor");
        let page_id = egui::Id::unique(("acid-page", row));
        let mut page = ui.ctx().data(|d| d.get_temp::<usize>(page_id)).unwrap_or(0);
        let count = match slot.length {
            PhraseLength::Steps(n) => format!("{n} steps"),
            PhraseLength::Bars(1) => "1 bar".into(),
            PhraseLength::Bars(n) => format!("{n} bars"),
        };
        ui.scope_builder(
            egui::UiBuilder::new().id_salt(("acid-length", row))
                .max_rect(rack.rect(405.0, y + 8.0, 85.0, 18.0)),
            |ui| {
                egui::ComboBox::from_id_salt("length").width(ui.available_width()).selected_text(count).show_ui(ui, |ui| {
                    for n in [1, 2, 4, 8, 16, 32, 64] {
                        let length = PhraseLength::Bars(n);
                        let label = if n == 1 { "1 bar".into() } else { format!("{n} bars") };
                        if ui.selectable_label(slot.length == length, label).clicked() && slot.length != length {
                            slot.set_length(length);
                            changed = true;
                        }
                    }
                    if slot.legacy_128_bars || slot.length == PhraseLength::Bars(128) {
                        let length = PhraseLength::Bars(128);
                        if ui.selectable_label(slot.length == length, "128 bars (legacy)").clicked() && slot.length != length {
                            slot.set_length(length);
                            changed = true;
                        }
                    }
                    ui.separator();
                    for n in [16, 32, 64, 128] {
                        let length = PhraseLength::Steps(n);
                        if ui.selectable_label(slot.length == length, format!("{n} steps")).clicked() && slot.length != length {
                            slot.set_length(length);
                            changed = true;
                        }
                    }
                });
            },
        );
        let pages = slot.step_count().div_ceil(16);
        page = page.min(pages - 1);
        if button(ui, rack, rack.rect(512.0, y + 9.0, 20.0, 19.0), (row, "page-prev"), "<", false).clicked() {
            page = page.saturating_sub(1);
        }
        button(ui, rack, rack.rect(536.0, y + 9.0, 60.0, 19.0), (row, "page-indicator"),
            &format!("{}/{}", page + 1, pages), false).on_hover_text("Right-click to jump to a page")
            .context_menu(|ui| {
                let mut page_number = page + 1;
                if ui.add(egui::DragValue::new(&mut page_number).range(1..=pages).prefix("Page ")).changed() {
                    page = page_number - 1;
                }
            });
        if button(ui, rack, rack.rect(600.0, y + 9.0, 20.0, 19.0), (row, "page-next"), ">", false).clicked() {
            page = (page + 1).min(pages - 1);
        }
        ui.ctx().data_mut(|d| d.insert_temp(page_id, page));
        if button(ui, rack, rack.rect(630.0, y + 9.0, 116.0, 19.0),
            (row, "acid-chord"), "CHORD LAB", false).clicked() {
            let selected = ui.ctx().data(|d| d.get_temp::<usize>(step_id(row))).unwrap_or(0);
            ui.ctx().data_mut(|d| {
                d.insert_temp(step_id(row), (page * 16 + selected % 16).min(slot.step_count() - 1));
                d.insert_temp(chord_id, Some(row));
            });
            self.panel.focus = row;
        }
        let cursor = self.panel.transport.position.max(0) as usize
            / usize::from(slot.ticks_per_step().max(1)) % slot.step_count();
        for column in 0..(slot.step_count() - page * 16).min(16) {
            let index = page * 16 + column;
            let step = slot.step(index);
            let root = step.notes.iter().copied().find(|&n| n != 0);
            let label = root.map(pitch_name).unwrap_or_else(|| "–".into());
            let response = button(
                ui, rack, rack.rect(224.0 + column as f32 * 38.0, y + 126.0, 34.0, 27.0),
                (row, "acid-step", index), &label, root.is_some(),
            ).on_hover_text(format!("Step {}: click to edit notes and duration", index + 1));
            if response.clicked() {
                self.panel.focus = row;
                ui.ctx().data_mut(|d| {
                    d.insert_temp(step_id(row), index);
                    d.insert_temp(chord_id, Some(row));
                });
            }
            if slot.sequence_enabled && self.panel.transport.playing && cursor == index {
                ui.painter().rect_filled(
                    rack.rect(225.0 + column as f32 * 38.0, y + 120.0, 32.0, 2.0), 0.0, ACID,
                );
            }
        }
        let linked = self.generator.options.link_tb;
        if button(ui, rack, rack.rect(228.0, y + 9.0, 76.0, 19.0),
            (row, "acid-link"), "LINK A/B", linked)
            .on_hover_text("Generate harmonically linked synth and TB patterns.").clicked() {
            self.generator.options.link_tb = !linked;
        }
        if button(ui, rack, rack.rect(313.0, y + 9.0, 72.0, 19.0),
            (row, "acid-rnd"), "RND", false).on_hover_text("Generate a new chord sequence").clicked() {
            self.random_pattern(row);
        }
        if changed {
            self.studio.changed();
            self.doc.modified = true;
        }
    }

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
    fn one_bar_rate_buttons_resize_the_editable_grid_without_erasing_notes() {
        let ctx = Context::default();
        let mut app = App::new(Panel::new(), None);
        let canvas = Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDTH, HEIGHT));
        let rack = Rack::fit(canvas);
        let y = ROWS[0].0;
        let slot = &mut app.studio.settings.synth_slots[0];
        slot.enabled = true;
        slot.set_length(PhraseLength::Bars(1));
        slot.set_step_rate(48);
        slot.step_mut(15).notes = [60, 67, 72, 0];
        click(&ctx, &mut app, canvas, rack.rect(172.0, y + 123.0, 34.0, 21.0).center());
        assert_eq!(app.studio.settings.synth_slots[0].step_count(), 1);
        click(&ctx, &mut app, canvas, rack.rect(224.0 + 15.0 * 38.0, y + 126.0, 34.0, 27.0).center());
        ctx.data(|d| assert!(d.get_temp::<Option<usize>>(egui::Id::unique("acid-chord-editor")).flatten().is_none()));
        click(&ctx, &mut app, canvas, rack.rect(28.0, y + 123.0, 34.0, 21.0).center());
        let slot = &app.studio.settings.synth_slots[0];
        assert_eq!(slot.step_count(), 16);
        assert_eq!(slot.step(15).notes, [60, 67, 72, 0]);
    }

    #[test]
    fn late_page_click_opens_chord_lab_on_the_absolute_step() {
        let ctx = Context::default();
        let mut app = App::new(Panel::new(), None);
        let canvas = Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDTH, HEIGHT));
        let rack = Rack::fit(canvas);
        let y = ROWS[0].0;
        let slot = &mut app.studio.settings.synth_slots[0];
        slot.enabled = true;
        slot.length = PhraseLength::Bars(128);
        slot.set_step_rate(48);
        ctx.data_mut(|data| data.insert_temp(egui::Id::unique(("acid-page", 0usize)), 127usize));
        click(&ctx, &mut app, canvas, rack.rect(224.0 + 15.0 * 38.0, y + 126.0, 34.0, 27.0).center());
        ctx.data(|data| {
            assert_eq!(data.get_temp::<usize>(step_id(0)), Some(2047));
            assert_eq!(data.get_temp::<Option<usize>>(egui::Id::unique("acid-chord-editor")), Some(Some(0)));
            assert_eq!(data.get_temp::<Option<usize>>(egui::Id::unique("graphite-edit")).flatten(), None);
        });
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
