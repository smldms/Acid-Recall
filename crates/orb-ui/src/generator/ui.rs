use eframe::egui::{self, Align2, Color32, Context, FontId, Rect, Sense, Ui};

use super::{Change, DrumStyle, Rng, Scale, Undo, drum_pattern, tb_pair, tb_pair_lengths};
use super::acid::{PadChange, Riff, linked_phrase};
use super::harmony::{fit_to_chords, joint_phrases};
use orb_engine::studio::acid_synth::Slot;
use crate::app::{App, View};

const MACHINES: [&str; 4] = ["TB-303 A", "TB-303 B", "TR-808", "TR-909"];
const NOTES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

fn rack_button(
    ui: &mut Ui,
    rect: Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    text: &str,
    on: bool,
    scale: f32,
) -> egui::Response {
    let response = ui.interact(rect, egui::Id::unique(id), Sense::click());
    let color = if on {
        Color32::from_rgb(0x4a, 0x53, 0x32)
    } else if response.hovered() {
        Color32::from_rgb(0x50, 0x50, 0x56)
    } else {
        Color32::from_rgb(0x30, 0x30, 0x34)
    };
    ui.painter().rect_filled(rect, 1.5 * scale, color);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(6.5 * scale),
        Color32::WHITE,
    );
    response
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Destination {
    LocalPad,
    Joint,
    SelectedPad,
    BothTbsFromSelectedPad,
}

impl App {
    fn generator_machine(&self, row: usize) -> &'static str {
        if row < 2 && self.studio.settings.synth_slots[row].editing_synth() {
            ["AcidSynth A", "AcidSynth B"][row]
        } else {
            MACHINES[row]
        }
    }

    fn generate_with_pad(&mut self, row: usize, seed: u64, target: Destination) {
        let options = &self.generator.options;
        let slots = &self.studio.settings.synth_slots;
        let patterns = [self.panel.rows[0].pattern, self.panel.rows[1].pattern];
        let original: [Vec<u8>; 2] = std::array::from_fn(|i| self.generator_before(i, patterns[i]));
        let tuning: [i16; 2] = std::array::from_fn(|i| {
            let knob = self.panel.knob(["tb303_1.tune", "tb303_2.tune"][i]);
            orb_engine::tb303::tune_semitones(crate::behavior::knob_send(knob) as u8)
        });
        let reverse = target == Destination::BothTbsFromSelectedPad;
        let joint = target == Destination::Joint;
        let linked = target != Destination::LocalPad;
        let mut riffs = Vec::new();
        if target == Destination::SelectedPad {
            riffs.extend((0..2).map(|i| {
                Riff::new(&original[i], tuning[i], if slots[i].tb_active() { 4 } else { 1 })
            }));
        }
        if !joint && !reverse && riffs.iter().all(Riff::silent) {
            let [anchor, _] = tb_pair(options, 16, seed);
            riffs = vec![Riff::new(&anchor, 0, 1)];
        }

        let active = std::array::from_fn(|i| slots[i].enabled || (i == row && slots[i].editing_synth()));
        let mut composition = joint.then(|| joint_phrases(options, slots, active, seed));
        let mut pads = Vec::new();
        if !reverse {
            for (index, slot) in slots.iter().enumerate() {
                let participating = if joint { active[index] } else { index == row };
                if !participating { continue; }
                let upper = linked && index == 1 && slots[0].enabled;
                let after = match &mut composition {
                    Some(parts) => std::mem::take(&mut parts[index]),
                    None => linked_phrase(options, &riffs, slot, seed, upper),
                };
                pads.push(PadChange {
                    row: index, length: slot.length, ticks: slot.ticks_per_step(),
                    before: (0..slot.step_count()).map(|i| slot.step(i)).collect(),
                    after,
                });
            }
        }

        for pad in &pads {
            PadChange::write(&mut self.studio.settings.synth_slots[pad.row], &pad.after);
        }
        let slots = &self.studio.settings.synth_slots;
        let fallback;
        let mut sources: Vec<&Slot> = if target == Destination::BothTbsFromSelectedPad {
            vec![&slots[row]]
        } else {
            slots.iter().enumerate().filter(|(i, slot)| {
                slot.enabled || pads.iter().any(|pad| pad.row == *i)
            }).map(|(_, slot)| slot).collect()
        };
        let mut changes = Vec::new();
        if linked {
            // An empty keyboard or sequence can still seed a riff in the chosen key. The
            // temporary harmony is never written over the performer's empty synth phrase.
            if options.density > 0 && sources.iter().all(|slot| {
                (0..slot.step_count()).all(|i| slot.step(i).notes == [0; 4])
            }) {
                let mut key_phrase = Slot::default();
                key_phrase.steps.fill(Default::default());
                let root = 48 + options.root % 12;
                let third = if options.scale.intervals().contains(&3) { 3 } else { 4 };
                key_phrase.steps[0].notes = [root, root + third, root + 7, 0];
                key_phrase.steps[0].gate_steps = 16;
                fallback = key_phrase;
                sources = vec![&fallback];
            }
            let lengths = [original[0][1], original[1][1]];
            let pair = tb_pair_lengths(options, lengths, seed);
            for (i, mut after) in pair.into_iter().enumerate() {
                if !joint && !reverse && slots[i].tb_active() { continue; }
                fit_to_chords(&mut after, tuning[i], &sources, seed.wrapping_add(i as u64));
                // Off-grid notes are dormant when the TB pattern is shorter than sixteen.
                let active_end = 2 + usize::from(lengths[i].clamp(1, 16)) * 6;
                after[active_end..].copy_from_slice(&original[i][active_end..]);
                after[0] = original[i][0];
                changes.push(Change { row: i, pattern: patterns[i], before: original[i].clone(), after });
            }
        }
        for change in &changes {
            self.panel.replace_pattern(change.row, change.pattern, change.after.clone());
        }
        if !pads.is_empty() { self.studio.changed(); }
        self.doc.modified = true;
        self.generator.undo = Some(Undo { changes, pads, tuning: None });
        self.generator.notice = Some(if options.density == 0 {
            "Density is zero. Generated rests.".into()
        } else if joint {
            "Composed both TB lines and the synth parts together.".into()
        } else if reverse {
            "Generated TB A and TB B from the chord phrase.".into()
        } else if linked {
            "Generated chords from the TB patterns and updated the hidden TB lines.".into()
        } else { "Generated AcidSynth chords.".into() });
        self.flush_generator_edits();
    }

    pub(crate) fn linked_tbs_from_chords(&mut self, row: usize) {
        if row >= 2 { return; }
        self.flush_generator_edits();
        let seed = self.next_generator_seed();
        self.generate_with_pad(row, seed, Destination::BothTbsFromSelectedPad);
    }

    pub(crate) fn linked_chords_from_tbs(&mut self, row: usize) {
        if row >= 2 { return; }
        self.flush_generator_edits();
        let seed = self.next_generator_seed();
        self.generate_with_pad(row, seed, Destination::SelectedPad);
    }

    fn flush_generator_edits(&mut self) {
        self.note_edits();
        if let Some(link) = &mut self.link {
            link.forward(&mut self.panel);
        }
    }

    fn generator_before(&self, row: usize, pattern: u8) -> Vec<u8> {
        if row >= 2 {
            return self.drum_pattern_bytes(row, pattern);
        }
        if let Some(link) = &self.link {
            let size = 98;
            let at = usize::from(pattern) * size;
            if let Some(pattern) = link.patterns()[row].get(at..at + size) {
                return pattern.to_vec();
            }
        }
        self.panel.pattern_bytes(row)
    }

    /// Generate the selected slot(s). Explicit seeds are also used by regression tests.
    pub fn random_pattern_with_seed(&mut self, row: usize, seed: u64) {
        if row >= 4 {
            return;
        }
        self.flush_generator_edits();
        if row < 2 {
            let slots = &self.studio.settings.synth_slots;
            let editing_synth = slots[row].editing_synth();
            let joint = self.generator.options.link_tb && (editing_synth || slots.iter().any(|slot| slot.enabled));
            if joint || editing_synth {
                self.generate_with_pad(row, seed, if joint { Destination::Joint } else { Destination::LocalPad });
                return;
            }
        }
        let pattern_size = if row < 2 { 98 } else { 194 };
        if self
            .link
            .as_ref()
            .is_some_and(|l| l.patterns()[row].len() < 32 * pattern_size)
        {
            self.generator.notice = Some("Load a drum kit before generating its patterns.".into());
            return;
        }
        let options = &self.generator.options;
        let length = if row >= 2 {
            self.drum_length(row) as u8
        } else {
            self.panel.rows[row].len[self.panel.rows[row].pattern as usize].clamp(1, 16)
        };
        let linked = row < 2 && options.link_tb;
        let tb_lengths = std::array::from_fn(|r| {
            self.panel.rows[r].len[usize::from(self.panel.rows[r].pattern)].clamp(1, 16)
        });
        let shuffle = self.panel.rows[row].shuffle[self.panel.rows[row].pattern as usize];
        let generated: Vec<(usize, Vec<u8>)> = if row < 2 {
            if linked {
                let [bass, answer] = tb_pair_lengths(options, tb_lengths, seed);
                vec![(0, bass), (1, answer)]
            } else {
                let [bass, _] = tb_pair(options, length, seed);
                vec![(row, bass)]
            }
        } else {
            vec![(row, drum_pattern(options, row == 2, length, seed))]
        };
        let tuning = if linked {
            let before = [
                self.panel.knob("tb303_1.tune"),
                self.panel.knob("tb303_2.tune"),
            ];
            let value = before[row];
            self.panel.set_knob("tb303_1.tune", value);
            self.panel.set_knob("tb303_2.tune", value);
            Some((before, [value; 2]))
        } else {
            None
        };
        let mut changes = Vec::new();
        for (r, mut bytes) in generated {
            let pattern = self.panel.rows[r].pattern;
            let before = self.generator_before(r, pattern);
            let pattern_shuffle = if linked {
                shuffle
            } else {
                self.panel.rows[r].shuffle[usize::from(pattern)]
            };
            bytes[0] = u8::from(pattern_shuffle);
            if r >= 2 {
                // Preserve inactive pages when regenerating a shortened pattern.
                let mut full = before.clone();
                full[..bytes.len()].copy_from_slice(&bytes);
                bytes = full;
                self.drum_replace_pattern(r, pattern, bytes.clone());
            } else {
                self.panel.replace_pattern(r, pattern, bytes.clone());
            }
            changes.push(Change {
                row: r,
                pattern,
                before,
                after: bytes,
            });
        }
        self.generator.undo = Some(Undo { changes, tuning, pads: Vec::new() });
        self.generator.notice = Some(if linked {
            format!(
                "Generated linked TBs: {} {}, {} / {} steps.",
                NOTES[usize::from(self.generator.options.root % 12)],
                self.generator.options.scale.name(),
                tb_lengths[0],
                tb_lengths[1],
            )
        } else {
            format!("Generated {}: {length} steps.", MACHINES[row])
        });
        self.flush_generator_edits();
    }

    fn next_generator_seed(&mut self) -> u64 {
        let mut rng = Rng(self.generator.seed);
        let seed = rng.next();
        self.generator.seed = seed;
        seed
    }

    pub fn random_pattern(&mut self, row: usize) {
        let seed = self.next_generator_seed();
        self.random_pattern_with_seed(row, seed);
    }

    /// Undo is offered only while the generated notes, length and linked tuning still match.
    /// Later manual/MIDI edits are never silently overwritten by this one-action undo.
    pub fn can_undo_random(&self) -> bool {
        let Some(undo) = &self.generator.undo else {
            return false;
        };
        if !undo.pads.iter().all(|pad| pad.matches(&self.studio.settings.synth_slots[pad.row])) {
            return false;
        }
        if let Some((_, after)) = undo.tuning {
            let now = [
                self.panel.knob("tb303_1.tune"),
                self.panel.knob("tb303_2.tune"),
            ];
            if now != after {
                return false;
            }
        }
        undo.changes.iter().all(|c| {
            if c.row >= 2 {
                self.drum_pattern_bytes(c.row, c.pattern) == c.after
            } else if self.panel.rows[c.row].pattern == c.pattern {
                self.panel.pattern_bytes(c.row) == c.after
            } else if let Some(link) = &self.link {
                let size = c.after.len();
                let at = usize::from(c.pattern) * size;
                link.patterns()[c.row][at..at + size] == c.after
            } else {
                false
            }
        })
    }

    pub fn undo_random(&mut self) -> bool {
        if !self.can_undo_random() {
            return false;
        }
        self.flush_generator_edits();
        let undo = self.generator.undo.take().unwrap();
        if !undo.pads.is_empty() {
            for pad in undo.pads {
                PadChange::write(&mut self.studio.settings.synth_slots[pad.row], &pad.before);
            }
            self.studio.changed();
            self.doc.modified = true;
        }
        for c in undo.changes {
            if c.row >= 2 {
                self.drum_replace_pattern(c.row, c.pattern, c.before);
            } else {
                self.panel.replace_pattern(c.row, c.pattern, c.before);
            }
        }
        if let Some((before, _)) = undo.tuning {
            self.panel.set_knob("tb303_1.tune", before[0]);
            self.panel.set_knob("tb303_2.tune", before[1]);
        }
        self.generator.notice = Some("Restored the patterns before the last Random.".into());
        self.flush_generator_edits();
        true
    }

    pub(crate) fn generator_buttons(&mut self, ui: &mut Ui, view: &View) {
        if self.browsing() || self.prompting() || self.error_open() {
            return;
        }
        let mut generate = None;
        let mut undo = false;
        for row in 0..MACHINES.len() {
            let name = self.generator_machine(row);
            let y = 131.0 + row as f32 * 115.0;
            let response = rack_button(
                ui,
                view.rect(20.0, y, 30.0, 12.0),
                ("random-pattern", row),
                "RND",
                false,
                view.scale,
            )
            .on_hover_text(format!(
                "Random {name}. Right-click for generator settings and Undo Random."
            ));
            if response.clicked() {
                generate = Some(row);
            }
            response.context_menu(|ui| {
                if ui.button("Generator settings...").clicked() {
                    self.generator.open = true;
                    ui.close();
                }
                if ui
                    .add_enabled(self.can_undo_random(), egui::Button::new("Undo Random"))
                    .clicked()
                {
                    undo = true;
                    ui.close();
                }
            });
            if row < 2 {
                let response = rack_button(
                    ui,
                    view.rect(54.0, y - 1.0, 28.0, 8.0),
                    ("link-tb", row),
                    "LINK",
                    self.generator.options.link_tb,
                    view.scale,
                )
                .on_hover_text("Link the TB and AcidSynth patterns, including TB lines behind an active synth.");
                if response.clicked() {
                    self.generator.options.link_tb = !self.generator.options.link_tb;
                }
            }
        }
        if let Some(row) = generate {
            self.random_pattern(row);
            ui.ctx().request_repaint();
        }
        if undo {
            self.undo_random();
            ui.ctx().request_repaint();
        }
    }

    pub(crate) fn generator_ui(&mut self, ctx: &Context) {
        if !self.generator.open {
            return;
        }
        let mut open = true;
        let mut generate = None;
        let mut undo = false;
        let can_undo = self.can_undo_random();
        egui::Window::new("Pattern Generator")
            .open(&mut open)
            .default_width(430.0)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.heading("Musical Random");
                ui.label("Generate a pattern for the selected instrument.");
                let o = &mut self.generator.options;
                ui.horizontal(|ui| {
                    ui.label("Key");
                    egui::ComboBox::from_id_salt("generator-key")
                        .selected_text(NOTES[usize::from(o.root % 12)])
                        .show_ui(ui, |ui| {
                            for (root, name) in NOTES.iter().enumerate() {
                                ui.selectable_value(&mut o.root, root as u8, *name);
                            }
                        });
                    egui::ComboBox::from_id_salt("generator-scale")
                        .selected_text(o.scale.name())
                        .show_ui(ui, |ui| {
                            for scale in Scale::ALL {
                                ui.selectable_value(&mut o.scale, scale, scale.name());
                            }
                        });
                });
                ui.checkbox(&mut o.link_tb, "LINK instrument slots");
                ui.small("Linked Random composes both TBs and the active synth phrases together. Use Chord Lab to generate in just one direction.");
                ui.add(egui::Slider::new(&mut o.density, 0..=100).text("Notes (%)"));
                ui.add(egui::Slider::new(&mut o.accents, 0..=100).text("Accents (%)"));
                ui.add(egui::Slider::new(&mut o.slides, 0..=100).text("Slides (%)"));
                ui.add(egui::Slider::new(&mut o.variation, 0..=100).text("Variation (%)"));
                ui.separator();
                ui.label("Style / musical grammar");
                ui.horizontal(|ui| {
                    for style in DrumStyle::ALL {
                        ui.selectable_value(&mut o.drum_style, style, style.name());
                    }
                });
                ui.small(o.drum_style.description());
                ui.add(egui::Slider::new(&mut o.drum_density, 0..=100).text("Drum density (%)"));
                ui.separator();
                for row in 0..MACHINES.len() {
                    ui.horizontal(|ui| {
                        ui.label(self.generator_machine(row));
                        if row >= 2 {
                            let before = self.drum_length(row);
                            let mut length = before;
                            egui::ComboBox::from_id_salt(("generator-drum-length", row))
                                .selected_text(format!("{length} steps"))
                                .show_ui(ui, |ui| {
                                    for value in [16, 32, 64] {
                                        ui.selectable_value(
                                            &mut length,
                                            value,
                                            format!("{value} steps"),
                                        );
                                    }
                                });
                            if length != before {
                                self.drum_set_length(row, length);
                            }
                        } else if self.studio.settings.synth_slots[row].editing_synth() {
                            ui.label(format!("{} steps", self.studio.settings.synth_slots[row].step_count()));
                        } else {
                            let pattern = usize::from(self.panel.rows[row].pattern);
                            let mut length = self.panel.rows[row].len[pattern].clamp(1, 16);
                            if ui
                                .add(
                                    egui::DragValue::new(&mut length)
                                        .range(1..=16)
                                        .suffix(" steps"),
                                )
                                .changed()
                            {
                                self.panel.set_pattern_length(row, length);
                            }
                        }
                        if ui.button("Random").clicked() {
                            generate = Some(row);
                        }
                    });
                }
                ui.separator();
                if ui
                    .add_enabled(can_undo, egui::Button::new("Undo Random"))
                    .clicked()
                {
                    undo = true;
                }
                if let Some(notice) = &self.generator.notice {
                    ui.label(notice);
                }
            });
        self.generator.open = open;
        if let Some(row) = generate {
            self.random_pattern(row);
            ctx.request_repaint();
        }
        if undo {
            self.undo_random();
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod pad_tests {
    use crate::{app::App, model::Panel};
    use orb_engine::studio::acid_synth::{PhraseLength, Step};

    #[test]
    fn switching_editor_preserves_independent_layer_power_and_selects_the_unlinked_target() {
        let mut app = App::new(Panel::new(), None);
        let slot = &mut app.studio.settings.synth_slots[0];
        slot.set_synth_active(true);
        slot.edit_synth = Some(false);
        let synth_before = slot.clone();
        let tb_before = app.panel.pattern_bytes(0);
        app.random_pattern_with_seed(0, 32);
        assert_ne!(app.panel.pattern_bytes(0), tb_before);
        assert_eq!(app.studio.settings.synth_slots[0], synth_before);
        app.studio.settings.synth_slots[0].edit_synth = Some(true);
        let new_tb = app.panel.pattern_bytes(0);
        app.random_pattern_with_seed(0, 44);
        assert_eq!(app.panel.pattern_bytes(0), new_tb);
        assert_ne!(app.studio.settings.synth_slots[0].steps, synth_before.steps);
        assert!(app.studio.settings.synth_slots[0].tb_active());
        assert!(app.studio.settings.synth_slots[0].enabled);
    }

    #[test]
    fn both_synths_write_their_hidden_tbs_and_undo_restores_all_four_patterns() {
        let mut app = App::new(Panel::new(), None);
        for (i, slot) in app.studio.settings.synth_slots.iter_mut().enumerate() {
            slot.enabled = true;
            slot.set_length(PhraseLength::Bars(if i == 0 { 1 } else { 2 }));
            slot.set_step_rate(48);
        }
        let before_slots = app.studio.settings.synth_slots.clone();
        let before_tbs = [app.panel.pattern_bytes(0), app.panel.pattern_bytes(1)];
        app.generator.options.link_tb = true;
        app.random_pattern_with_seed(0, 720);
        let undo = app.generator.undo.as_ref().unwrap();
        assert_eq!((undo.pads.len(), undo.changes.len()), (2, 2));
        for (i, before) in before_tbs.iter().enumerate() {
            assert_ne!(&app.panel.pattern_bytes(i), before);
            let slot = &app.studio.settings.synth_slots[i];
            assert_eq!(slot.length, before_slots[i].length);
            assert_eq!(slot.ticks_per_step(), before_slots[i].ticks_per_step());
            assert!((0..slot.step_count()).any(|index| slot.step(index).notes != [0; 4]));
        }
        assert!(app.undo_random());
        for (i, old) in before_slots.iter().enumerate() {
            assert_eq!(app.panel.pattern_bytes(i), before_tbs[i]);
            let slot = &app.studio.settings.synth_slots[i];
            assert!((0..slot.step_count()).all(|n| slot.step(n) == old.step(n)));
        }
    }

    #[test]
    fn explicit_reverse_reads_selected_stored_chord_even_with_link_off() {
        let mut app = App::new(Panel::new(), None);
        let slot = &mut app.studio.settings.synth_slots[1];
        slot.enabled = false;
        slot.sequence_enabled = false;
        slot.steps.fill(Step::default());
        slot.steps[0] = Step { notes: [61, 61, 61, 61], gate_steps: 16 };
        let original_slot = slot.clone();
        app.linked_tbs_from_chords(1);
        assert_eq!(app.studio.settings.synth_slots[1], original_slot);
        for row in 0..2 {
            let knob = app.panel.knob(["tb303_1.tune", "tb303_2.tune"][row]);
            let tuning = orb_engine::tb303::tune_semitones(crate::behavior::knob_send(knob) as u8);
            let steps = super::super::steps303(&app.panel.pattern_bytes(row));
            let notes: Vec<_> = steps.iter().filter(|s| !s.rest).map(|s| {
                (super::super::pitch(*s) + i32::from(tuning)).rem_euclid(12)
            }).collect();
            assert!(!notes.is_empty());
            assert!(notes.iter().all(|&n| n == 1));
        }
        assert!(app.undo_random());
    }

    #[test]
    fn unlinked_pad_random_only_changes_the_clicked_pad_and_zero_density_stays_silent() {
        let mut app = App::new(Panel::new(), None);
        for slot in &mut app.studio.settings.synth_slots { slot.enabled = true; }
        let other = app.studio.settings.synth_slots[1].clone();
        let tbs = [app.panel.pattern_bytes(0), app.panel.pattern_bytes(1)];
        app.random_pattern_with_seed(0, 19);
        assert!(app.studio.settings.synth_slots[0].steps.iter().any(|s| s.notes[3] > 0));
        assert_eq!(app.studio.settings.synth_slots[1], other);
        for (row, before) in tbs.iter().enumerate() { assert_eq!(&app.panel.pattern_bytes(row), before); }
        app.generator.options.link_tb = true;
        app.generator.options.density = 0;
        app.random_pattern_with_seed(0, 20);
        for (row, slot) in app.studio.settings.synth_slots.iter().enumerate() {
            assert!((0..slot.step_count()).all(|i| slot.step(i).notes == [0; 4]));
            assert!(super::super::steps303(&app.panel.pattern_bytes(row)).iter().all(|s| s.rest));
        }
    }

    #[test]
    fn mixed_rack_linked_rnd_composes_both_tbs_and_the_pad_from_either_row() {
        for pad in 0..2 {
            let tb = 1 - pad;
            let mut app = App::new(Panel::new(), None);
            app.random_pattern_with_seed(tb, 1);
            let original_tb = app.panel.pattern_bytes(tb);
            let hidden_tb = app.panel.pattern_bytes(pad);
            app.studio.settings.synth_slots[pad].enabled = true;
            app.studio.settings.synth_slots[pad].length = PhraseLength::Steps(64);
            let slot = &app.studio.settings.synth_slots[pad];
            let original: Vec<_> = (0..64).map(|i| slot.step(i)).collect();
            let patch = slot.patch;
            app.generator.options.link_tb = true;
            for initiator in [pad, tb] {
                app.random_pattern_with_seed(initiator, 405);
                assert_ne!(app.panel.pattern_bytes(pad), hidden_tb);
                assert_ne!(app.panel.pattern_bytes(tb), original_tb);
                let slot = &app.studio.settings.synth_slots[pad];
                assert_eq!(slot.length, PhraseLength::Steps(64));
                assert_eq!(slot.patch, patch);
                assert!((16..64).any(|i| slot.step(i).notes[0] > 0));
                assert!(app.undo_random());
                assert_eq!(app.panel.pattern_bytes(tb), original_tb);
                assert_eq!(app.panel.pattern_bytes(pad), hidden_tb);
                let slot = &app.studio.settings.synth_slots[pad];
                assert!(original.iter().enumerate().all(|(i, step)| slot.step(i) == *step));
            }
            app.random_pattern_with_seed(pad, 405);
            app.studio.settings.synth_slots[pad].step_mut(63).notes = [70; 4];
            assert!(!app.can_undo_random());
        }
    }
}
