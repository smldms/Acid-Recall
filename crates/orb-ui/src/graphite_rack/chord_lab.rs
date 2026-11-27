//! Step selection, voicing construction and manual polyphonic note entry.
use super::{egui, App, Ui, ACID};
use super::acid_synth::pitch_name;

const CHORDS: &[(&str, &[i16])] = &[
    ("Major", &[0, 4, 7]), ("Minor", &[0, 3, 7]),
    ("Sus2", &[0, 2, 7]), ("Sus4", &[0, 5, 7]),
    ("Diminished", &[0, 3, 6]), ("Augmented", &[0, 4, 8]),
    ("Major 7", &[0, 4, 7, 11]), ("Minor 7", &[0, 3, 7, 10]),
    ("Dominant 7", &[0, 4, 7, 10]), ("Half-diminished", &[0, 3, 6, 10]),
    ("Diminished 7", &[0, 3, 6, 9]), ("Minor-major 7", &[0, 3, 7, 11]),
    ("Major 6", &[0, 4, 7, 9]), ("Minor 6", &[0, 3, 7, 9]),
    ("Add9", &[0, 4, 7, 14]), ("Minor add9", &[0, 3, 7, 14]),
    ("Quartal", &[0, 5, 10, 15]), ("Power fifth", &[0, 7]),
];

#[derive(Clone, Copy, Default)]
enum Spread { #[default] Close, Open, Drop2 }

const SPREADS: [(&str, Spread); 3] = [
    ("Close", Spread::Close), ("Open", Spread::Open), ("Drop 2", Spread::Drop2),
];

#[derive(Clone, Copy)]
struct Builder { root: u8, quality: usize, inversion: usize, spread: usize, octave: u8 }

impl Default for Builder {
    fn default() -> Self { Self { root: 48, quality: 1, inversion: 0, spread: 0, octave: 4 } }
}

fn voicing(root: u8, intervals: &[i16], inversion: usize, spread: Spread) -> Option<[u8; 4]> {
    let mut notes = [0i16; 4];
    let count = intervals.len();
    for (note, interval) in notes.iter_mut().zip(intervals) { *note = i16::from(root) + interval; }
    for _ in 0..inversion.min(count - 1) {
        while notes[0] <= notes[count - 1] { notes[0] += 12; }
        notes[..count].rotate_left(1);
    }
    match spread {
        Spread::Close => {}
        Spread::Open => notes[1] += 12,
        Spread::Drop2 => notes[count - 2] -= 12,
    }
    notes[..count].sort_unstable();
    if notes[..count].iter().any(|n| !(1..=127).contains(n)) { return None; }
    Some(notes.map(|n| n as u8))
}

fn toggle_note(notes: &mut [u8; 4], pitch: u8) -> bool {
    if pitch == 0 || pitch > 127 { return false; }
    if notes.contains(&pitch) {
        for note in notes.iter_mut().filter(|n| **n == pitch) { *note = 0; }
    } else if let Some(space) = notes.iter_mut().find(|n| **n == 0) {
        *space = pitch;
    } else {
        return false;
    }
    notes.sort_by_key(|n| if *n == 0 { u8::MAX } else { *n });
    true
}

fn pitches(notes: &[u8; 4]) -> String {
    let names = notes.iter().copied().filter(|n| *n != 0).map(pitch_name).collect::<Vec<_>>();
    if names.is_empty() { "Rest".to_owned() } else { names.join("  ") }
}

fn combo(ui: &mut Ui, id: &str, selected: &mut usize, labels: impl Iterator<Item = &'static str> + Clone) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(labels.clone().nth(*selected).unwrap_or_default())
        .show_ui(ui, |ui| {
            for (index, label) in labels.enumerate() { ui.selectable_value(selected, index, label); }
        });
}

/// White keys receive the full height, with shorter black keys above them.
fn keyboard(ui: &mut Ui, octave: u8, notes: &mut [u8; 4]) -> bool {
    let (area, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 74.0), egui::Sense::click());
    let key_width = area.width() / 14.0;
    let pointer = response.interact_pointer_pos();
    let mut hit = None;
    for black in [false, true] {
        let mut whites = 0;
        for semitone in 0..24 {
            let pitch = octave * 12 + semitone;
            let is_black = matches!(semitone % 12, 1 | 3 | 6 | 8 | 10);
            let x = whites as f32;
            if !is_black { whites += 1; }
            if black != is_black { continue; }
            let (left, width, height) = if black { (x - 0.31, 0.62, 0.62) } else { (x, 1.0, 1.0) };
            let key = egui::Rect::from_min_size(
                area.min + egui::vec2(left * key_width, 0.0),
                egui::vec2((width * key_width - 1.0).max(1.0), area.height() * height),
            );
            let available = (1..=127).contains(&pitch);
            let active = available && notes.contains(&pitch);
            let fill = if !available { egui::Color32::from_gray(62) } else if active { ACID } else if black { egui::Color32::from_gray(46) } else { egui::Color32::from_gray(194) };
            ui.painter().rect_filled(key, 2.0, fill);
            if available && (pitch % 12 == 0 || active) {
                let ink = if black && !active { egui::Color32::WHITE } else { egui::Color32::BLACK };
                ui.painter().text(key.center_bottom() - egui::vec2(0.0, 3.0), egui::Align2::CENTER_BOTTOM,
                    pitch_name(pitch), egui::FontId::monospace(10.0), ink);
            }
            if pointer.is_some_and(|p| key.contains(p)) { hit = available.then_some(pitch); }
        }
    }
    response.clicked() && hit.is_some_and(|pitch| toggle_note(notes, pitch))
}

impl App {
    pub(super) fn acid_chord_editor(&mut self, ui: &mut Ui, row: usize, id: egui::Id) {
        let slot = &mut self.studio.settings.synth_slots[row];
        let selected_id = egui::Id::unique(("acid-selected-step", row));
        let page_id = egui::Id::unique(("acid-page", row));
        let builder_id = egui::Id::unique(("acid-chord-builder", row));
        let total = slot.step_count();
        let mut index = ui.ctx().data(|d| d.get_temp::<usize>(selected_id)).unwrap_or(0).min(total - 1);
        let mut builder = ui.ctx().data(|d| d.get_temp::<Builder>(builder_id)).unwrap_or_default();
        let mut changed = false;
        let mut audition = None;
        let mut open = true;
        egui::Window::new(format!("AcidSynth {} / Chord", ["A", "B"][row]))
            .id(egui::Id::unique(("chord-window", row))).open(&mut open).vscroll(true)
            .default_width(520.0).min_width(350.0).show(ui.ctx(), |ui| {
                let pages = total.div_ceil(16);
                let mut page = index / 16;
                ui.horizontal(|ui| {
                    if ui.add_enabled(page > 0, egui::Button::new("‹")).clicked() { page -= 1; }
                    ui.label("Page");
                    let mut page_number = page + 1;
                    if ui.add(egui::DragValue::new(&mut page_number).range(1..=pages)).changed() { page = page_number - 1; }
                    ui.label(format!("/ {pages}"));
                    if ui.add_enabled(page + 1 < pages, egui::Button::new("›")).clicked() { page += 1; }
                    ui.separator();
                    ui.label(format!("Step {} / {}", index + 1, total));
                });
                index = (page * 16 + index % 16).min(total - 1);
                let playhead = self.panel.transport.playing.then(|| {
                    self.panel.transport.position.max(0) as usize / usize::from(slot.ticks_per_step()) % total
                });
                egui::Grid::new(("chord-grid", row)).num_columns(8).spacing([3.0, 3.0]).show(ui, |ui| {
                    for i in page * 16..((page + 1) * 16).min(total) {
                        let notes = slot.step(i).notes;
                        let root = notes.iter().copied().find(|&n| n != 0);
                        let label = format!("{:02}\n{}", i + 1, root.map(pitch_name).unwrap_or_else(|| "·".into()));
                        let text = if playhead == Some(i) { egui::RichText::new(label).color(ACID) } else { label.into() };
                        if ui.selectable_label(i == index, text).on_hover_text(pitches(&notes)).clicked() { index = i; }
                        if i % 8 == 7 { ui.end_row(); }
                    }
                });
                let original = slot.step(index);
                let mut step = original;
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.label("Root");
                    ui.add(egui::DragValue::new(&mut builder.root).range(1..=127)
                        .custom_formatter(|v, _| pitch_name(v as u8)).custom_parser(super::acid_synth::parse_pitch));
                    combo(ui, "quality", &mut builder.quality, CHORDS.iter().map(|x| x.0));
                    let intervals = CHORDS[builder.quality].1;
                    builder.inversion = builder.inversion.min(intervals.len() - 1);
                    combo(ui, "inversion", &mut builder.inversion, ["Root position", "1st inversion", "2nd inversion", "3rd inversion"].into_iter().take(intervals.len()));
                    combo(ui, "spread", &mut builder.spread, SPREADS.iter().map(|x| x.0));
                });
                let chord = voicing(builder.root, CHORDS[builder.quality].1, builder.inversion, SPREADS[builder.spread].1);
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(chord.is_some(), egui::Button::new("Apply chord")).clicked() && let Some(notes) = chord {
                        step.notes = notes;
                        audition = Some(notes);
                        builder.octave = (notes[0] / 12).min(9);
                    }
                    if ui.add_enabled(chord.is_some(), egui::Button::new("Preview")).clicked() { audition = chord; }
                    ui.label(chord.as_ref().map(pitches).unwrap_or_else(|| "Outside MIDI range".into()));
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!("Step {:02}   {}", index + 1, pitches(&step.notes)));
                    if ui.add_enabled(step.notes.iter().any(|n| *n != 0), egui::Button::new("Play")).clicked() { audition = Some(step.notes); }
                    if ui.button("Clear notes").clicked() { step.notes = [0; 4]; }
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(builder.octave > 0, egui::Button::new("−")).clicked() { builder.octave -= 1; }
                    ui.label(format!("Keyboard octave {}", i16::from(builder.octave) - 1));
                    if ui.add_enabled(builder.octave < 9, egui::Button::new("+")).clicked() { builder.octave += 1; }
                });
                if keyboard(ui, builder.octave, &mut step.notes) { audition = Some(step.notes); }
                ui.horizontal_wrapped(|ui| {
                    ui.label("Gate");
                    ui.add(egui::DragValue::new(&mut step.gate_steps).range(1..=2048).suffix(" steps"));
                    if usize::from(step.gate_steps) > total { ui.label(format!("Plays for {total} steps in this phrase")); }
                    for length in [1, 2, 4, 8, 16] {
                        if ui.small_button(length.to_string()).clicked() { step.gate_steps = length; }
                    }
                });
                if original != step { *slot.step_mut(index) = step; changed = true; }
            });
        ui.ctx().data_mut(|d| {
            d.insert_temp(builder_id, builder);
            d.insert_temp(selected_id, index);
            d.insert_temp(page_id, index / 16);
            if !open { d.insert_temp(id, None::<usize>); }
        });
        if changed { self.studio.changed(); self.doc.modified = true; }
        if let (Some(notes), Some(link)) = (audition, &mut self.link) {
            link.effect_command(orb_player::Command::AcidLive {
                slot: row, event: orb_engine::studio::acid_live::Event::Audition { notes, frames: 44_100 },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inversions_preserve_pitch_classes_and_ascending_order() {
        assert_eq!(voicing(48, CHORDS[7].1, 1, Spread::Close), Some([51,55,58,60]));
        assert_eq!(voicing(48, &[0,4,7,14], 1, Spread::Close), Some([52,55,62,72]));
        assert_eq!(voicing(48, &[0,4,7,11], 0, Spread::Drop2), Some([43,48,52,59]));
        assert_eq!(voicing(124, &[0,3,7], 0, Spread::Close), None);
    }

    #[test]
    fn manual_edit_supports_removal_and_enforces_four_voices() {
        let mut notes = [60,64,67,71];
        assert!(!toggle_note(&mut notes, 74));
        assert!(toggle_note(&mut notes, 64));
        assert!(toggle_note(&mut notes, 74));
        assert_eq!(notes, [60,67,71,74]);
        assert!(!toggle_note(&mut notes, 0));
    }
}
