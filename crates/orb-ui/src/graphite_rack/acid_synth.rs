//! AcidSynth's live controls and polyphonic step editor for the two instrument slots.

use super::{ACID, App, DIM, INK, NOTES, Rack, Ui, button, egui, fitted_button_font, mix_knob, paint_mix_knob};
use crate::midi::MidiMenu;
use crate::studio::midi::acid::{Domain, Field};
use orb_engine::studio::acid_presets::{EXPERIMENTAL_START, PRESETS};
use orb_engine::studio::acid_synth::{PhraseLength, Slot};

pub(super) const RATES: [(u16, &str); 5] = [
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

/// Both editors change the same saved length, including the historical 128-bar option.
pub(super) fn phrase_length_picker(ui: &mut Ui, slot: &mut Slot, width: f32) -> bool {
    let initial = slot.length;
    let mut selected = initial;
    let count = match initial {
        PhraseLength::Steps(n) => format!("{n} steps"),
        PhraseLength::Bars(1) => "1 bar".into(),
        PhraseLength::Bars(n) => format!("{n} bars"),
    };
    egui::ComboBox::from_id_salt("length").width(width).selected_text(count).show_ui(ui, |ui| {
        for n in [1, 2, 4, 8, 16, 32, 64] {
            let label = if n == 1 { "1 bar".into() } else { format!("{n} bars") };
            ui.selectable_value(&mut selected, PhraseLength::Bars(n), label);
        }
        if slot.legacy_128_bars || initial == PhraseLength::Bars(128) {
            ui.selectable_value(&mut selected, PhraseLength::Bars(128), "128 bars (legacy)");
        }
        ui.separator();
        for n in [16, 32, 64, 128] {
            ui.selectable_value(&mut selected, PhraseLength::Steps(n), format!("{n} steps"));
        }
    });
    if selected == initial { return false; }
    slot.set_length(selected);
    true
}

fn pinned_control(ui: &mut Ui, rack: Rack, row: usize, field: &Field, slot: &mut Slot, x: f32, y: f32) -> (bool, bool) {
    let id = format!("acidsynth{}.{name}", row+1, name=field.id);
    let rect = rack.rect(x, y, 43.0, 41.0);
    let widget = egui::Id::unique(("acid-main-pin", row, field.id));
    let response = ui.interact(rect, widget, egui::Sense::click_and_drag()).midi_menu(ui, &id);
    let old = field.position(slot);
    let mut next = old;
    let mut audition = false;
    if response.clicked() {
        response.request_focus();
        match field.domain {
            Domain::Action => audition = true,
            Domain::Switch => next = 1.0-old,
            Domain::Choice(max) => next = (((old*f32::from(max)).round() as u8 + 1) % (max+1)) as f32 / f32::from(max),
            _ => {}
        }
    }
    if response.dragged_by(egui::PointerButton::Primary) && !matches!(field.domain, Domain::Action) {
        let base = widget.with("start");
        if response.drag_started_by(egui::PointerButton::Primary) { ui.ctx().data_mut(|d| d.insert_temp(base, old)); }
        let starting = ui.ctx().data(|d| d.get_temp::<f32>(base)).unwrap_or(old);
        let distance = ui.input(|i| i.pointer.press_origin().zip(i.pointer.interact_pos())
            .map_or(0.0, |(origin, current)| (origin.y-current.y) / if i.modifiers.shift { 800.0 } else { 145.0 }));
        next = (starting + distance).clamp(0.0, 1.0);
    }
    let changed = next != old && field.set_position(slot, next);
    let value = field.display(slot);
    let ink = if response.hovered() { ACID } else { INK };
    let dial = rack.rect(x+2.0, y+11.0, 20.0, 25.0);
    let vrect = rack.rect(x+22.0, y+13.0, 20.0, 20.0);
    let name_rect = rack.rect(x, y, 43.0, 14.0);
    ui.painter().rect_filled(rect, rack.scale*2.0, egui::Color32::from_rgb(23,26,26));
    let label = match field.id {
        "wave_a" => "Table A", "wave_b" => "Table B", "mix" => "Balance",
        "filter_env" => "Filter env", "transpose_b" => "B transpose", "cutoff" => "Cutoff",
        "lfo_division" => "LFO div", "fm_ratio2" => "OP2 ratio", "fm_ratio3" => "OP3 ratio",
        "fm_ratio4" => "OP4 ratio", _ => field.label,
    };
    ui.painter().text(name_rect.center(), egui::Align2::CENTER_CENTER, label,
        fitted_button_font(ui, name_rect, label, 10.0*rack.scale, rack.scale*0.45), ink);
    if matches!(field.domain, Domain::Action) {
        ui.painter().text(rect.center_bottom()-egui::vec2(0.0,12.0*rack.scale), egui::Align2::CENTER_CENTER,
            "PLAY", egui::FontId::monospace(9.0*rack.scale), ACID);
    } else {
        paint_mix_knob(ui, rack.scale, dial, field.position(slot));
        ui.painter().text(vrect.center(), egui::Align2::CENTER_CENTER, &value,
            fitted_button_font(ui, vrect, &value, 9.0*rack.scale, rack.scale*0.3), ACID);
    }
    response.on_hover_text(format!("{}: {}\nRight-click for MIDI or to remove from Main Panel.", field.label, value));
    (changed, audition)
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
                    .selected_text(if slot.user_preset.is_empty() { PRESETS[preset].0 } else { &slot.user_preset })
                    .show_ui(ui, |ui| {
                        for (index, (name, _)) in PRESETS.iter().enumerate() {
                            if index == 0 {
                                ui.strong("Factory");
                            } else if index == EXPERIMENTAL_START {
                                ui.separator();
                                ui.strong("Experimental");
                            }
                            if ui.selectable_label(index == preset && slot.user_preset.is_empty(), *name).clicked() {
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
        let compact = self.studio.main_panel.count(row) > 0;
        for (i, &(key, label)) in MACROS.iter().enumerate() {
            let mut value = (slot.macros[i] * 1000.0).round() as i32;
            let response = mix_knob(
                ui, rack.scale, &format!("acidsynth{}.{key}", row + 1), label,
                if compact { rack.rect(224.0 + i as f32 * 60.0, y + 40.0, 55.0, 69.0) }
                else { rack.rect(247.0 + i as f32 * 152.0, y + 38.0, 60.0, 71.0) },
                &mut value,
            );
            if response.changed() {
                slot.macros[i] = value as f32 / 1000.0;
                changed = true;
            }
        }
        let mut audition_pin = false;
        for (index, field) in self.studio.main_panel.fields(row).enumerate() {
            let x = 470.0 + (index % 8) as f32 * 45.0;
            let top = y + 33.0 + (index / 8) as f32 * 43.0;
            let (edited, play) = pinned_control(ui, rack, row, field, slot, x, top);
            changed |= edited;
            audition_pin |= play;
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
        ui.scope_builder(
            egui::UiBuilder::new().id_salt(("acid-length", row))
                .max_rect(rack.rect(405.0, y + 8.0, 85.0, 18.0)),
            |ui| {
                let width = ui.available_width();
                changed |= phrase_length_picker(ui, slot, width);
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
            .on_hover_text("Generate related patterns for both TBs and the participating AcidSynth parts together.").clicked() {
            self.generator.options.link_tb = !linked;
        }
        if audition_pin { self.audition_acid_patch(row); }
        self.rack_random_button(ui, rack, row, rack.rect(313.0, y + 9.0, 72.0, 19.0));
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
    fn linked_random_from_either_rack_button_keeps_the_selected_phrase_grid() {
        for length in [PhraseLength::Steps(32), PhraseLength::Bars(4)] {
            let ctx = Context::default();
            let mut app = App::new(Panel::new(), None);
            app.random_pattern_with_seed(0, 31);
            app.generator.clear_undo();
            let slot = &mut app.studio.settings.synth_slots[1];
            slot.enabled = true;
            slot.set_length(length);
            slot.set_step_rate(96);
            app.generator.options.link_tb = true;
            let canvas = Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDTH, HEIGHT));
            let rack = Rack::fit(canvas);
            for button in [rack.rect(313.0, ROWS[1].0 + 9.0, 72.0, 19.0),
                           rack.rect(740.0, ROWS[0].0 + 9.0, 43.0, 19.0)] {
                click(&ctx, &mut app, canvas, button.center());
                assert_eq!(app.studio.settings.synth_slots[1].length, length);
                assert_eq!(app.studio.settings.synth_slots[1].ticks_per_step(), 96);
                assert!(app.undo_random(), "rack click must produce an undoable change");
            }
        }
    }

    #[test]
    fn pinned_choice_and_midi_update_the_same_visible_parameter() {
        let ctx = Context::default();
        let mut app = App::new(Panel::new(), None);
        let id = "acidsynth2.filter_mode";
        let (row, field) = crate::studio::midi::acid::target(id).unwrap();
        let slot = &mut app.studio.settings.synth_slots[row];
        slot.enabled = true;
        slot.edit_synth = Some(true);
        slot.patch.filter_mode = 0;
        app.studio.main_panel.update(row, field, true);
        let canvas = Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDTH, HEIGHT));
        let rack = Rack::fit(canvas);
        click(&ctx, &mut app, canvas, rack.rect(470.0, ROWS[row].0+33.0, 43.0, 41.0).center());
        assert_eq!(app.studio.settings.synth_slots[row].patch.filter_mode, 1);
        app.studio_mirror();
        assert_eq!(app.panel.value(id), Some(42));
        app.panel.set_knob(id, 1000);
        app.studio_midi();
        assert_eq!(field.display(&app.studio.settings.synth_slots[row]), "Notch");
        assert!(app.studio.main_panel.has(row, field));
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
        let switch = rack.rect(150.0, y + 8.0, 66.0, 22.0).center();
        let activate = rack.rect(84.0, y + 8.0, 63.0, 22.0).center();
        let tb_toggle = rack.rect(26.0, y + 8.0, 55.0, 22.0).center();
        let tb_pattern = app.panel.pattern_bytes(1);
        let slot_a = app.studio.settings.synth_slots[0].clone();
        click(&ctx, &mut app, canvas, switch);
        assert!(!app.studio.settings.synth_slots[1].enabled);
        assert!(app.studio.settings.synth_slots[1].editing_synth());
        click(&ctx, &mut app, canvas, activate);
        assert!(app.studio.settings.synth_slots[1].enabled);
        assert!(app.studio.settings.synth_slots[1].tb_active());
        assert!(app.studio.content && app.doc.modified);
        click(&ctx, &mut app, canvas, rack.rect(28.0, y + 123.0, 34.0, 21.0).center());
        assert_eq!(app.studio.settings.synth_slots[1].step_ticks, 48);
        let before = app.studio.settings.synth_slots[1].clone();
        click(&ctx, &mut app, canvas, switch);
        assert!(app.studio.settings.synth_slots[1].enabled);
        assert!(!app.studio.settings.synth_slots[1].editing_synth());
        click(&ctx, &mut app, canvas, tb_toggle);
        assert!(!app.studio.settings.synth_slots[1].tb_active());
        click(&ctx, &mut app, canvas, activate);
        assert!(!app.studio.settings.synth_slots[1].enabled);
        click(&ctx, &mut app, canvas, tb_toggle);
        click(&ctx, &mut app, canvas, activate);
        click(&ctx, &mut app, canvas, switch);
        assert_eq!(app.studio.settings.synth_slots[1], before);
        assert_eq!(app.studio.settings.synth_slots[0], slot_a);
        assert_eq!(app.panel.pattern_bytes(1), tb_pattern);
    }
}
