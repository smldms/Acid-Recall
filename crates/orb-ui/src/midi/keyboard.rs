//! Performance keyboard routing, independent of the controller's CC assignments.
use eframe::egui;
use orb_engine::studio::acid_live::Event;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scale {
    #[default]
    Chromatic,
    Minor,
    Major,
    Dorian,
    Phrygian,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Keyboard {
    /// One-based MIDI channel, zero disconnects the slot.
    pub channels: [u8; 2],
    pub root: u8,
    pub scale: Scale,
    pub grid: u16,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self { channels: [1, 2], root: 0, scale: Scale::Chromatic, grid: 0 }
    }
}

impl Keyboard {
    pub fn sanitize(&mut self) {
        self.channels = self.channels.map(|ch| ch.min(16));
        self.root %= 12;
        if !matches!(self.grid, 0 | 24 | 48 | 96 | 192) { self.grid = 0; }
    }

    fn pitch(&self, note: u8) -> u8 {
        let intervals: &[u8] = match self.scale {
            Scale::Chromatic => return note,
            Scale::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Scale::Major => &[0, 2, 4, 5, 7, 9, 11],
            Scale::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Scale::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
        };
        let allowed = |n: u8| intervals.contains(&((n % 12 + 12 - self.root % 12) % 12));
        (note.saturating_sub(2)..=note.saturating_add(2).min(127))
            .filter(|&n| allowed(n)).min_by_key(|&n| n.abs_diff(note)).unwrap_or(note)
    }

    pub fn decode(&self, input: u8, word: u32) -> Option<(u8, Event)> {
        let [status, note, velocity, _] = word.to_le_bytes();
        if note > 127 || velocity > 127 || input >= 16 { return None; }
        let channel = status & 15;
        let key = u16::from(input) << 11 | u16::from(channel) << 7 | u16::from(note);
        let event = match status >> 4 {
            8 => Event::Off { key },
            9 if velocity == 0 => Event::Off { key },
            9 => Event::On { key, pitch: self.pitch(note), velocity, grid: self.grid },
            11 if matches!(note, 120 | 123) => Event::Panic,
            _ => return None,
        };
        Some((channel + 1, event))
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) -> (bool, bool) {
        let old = self.clone();
        ui.strong("AcidSynth keyboard");
        ui.horizontal(|ui| {
            for (i, channel) in self.channels.iter_mut().enumerate() {
                ui.label(if i == 0 { "Synth A channel" } else { "Synth B channel" });
                ui.add(egui::DragValue::new(channel).range(0..=16)).on_hover_text("0 disables this slot");
            }
        });
        ui.horizontal(|ui| {
            let names = ["C","C#","D","D#","E","F","F#","G","G#","A","A#","B"];
            egui::ComboBox::from_label("Root").selected_text(names[usize::from(self.root % 12)])
                .show_ui(ui, |ui| {
                    for (n, &name) in names.iter().enumerate() { ui.selectable_value(&mut self.root,n as u8,name); }
                });
            egui::ComboBox::from_label("Scale").selected_text(format!("{:?}", self.scale))
                .show_ui(ui, |ui| {
                    for s in [Scale::Chromatic, Scale::Minor, Scale::Major, Scale::Dorian, Scale::Phrygian] {
                        ui.selectable_value(&mut self.scale, s, format!("{s:?}"));
                    }
                });
        });
        let options = [(0,"Off"),(24,"1/32"),(48,"1/16"),(96,"1/8"),(192,"1/4")];
        let mut panic = false;
        ui.horizontal(|ui| {
            egui::ComboBox::from_label("Live quantize").selected_text(options.iter()
                .find(|(n,_)| *n == self.grid).map_or("Off", |(_,s)| *s)).show_ui(ui, |ui| {
                    for (n,s) in options { ui.selectable_value(&mut self.grid,n,s); }
                });
            panic = ui.button("Panic / release keys").clicked();
        });
        (old != *self, panic || old.channels != self.channels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scale_lock_preserves_physical_key_identity_and_velocity_zero_releases() {
        let k = Keyboard { scale: Scale::Minor, root: 2, grid: 48, ..Default::default() };
        let word = |s,n,v| u32::from_le_bytes([s,n,v,0]);
        let (_, Event::On { key, pitch, velocity, grid }) = k.decode(2,word(0x91,61,99)).unwrap() else { panic!() };
        assert_eq!((pitch,velocity,grid),(60,99,48));
        assert_eq!(k.decode(2,word(0x91,61,0)),Some((2,Event::Off { key })));
        assert_eq!(k.decode(2,word(0x81,61,64)),Some((2,Event::Off { key })));
        assert_ne!(k.decode(1,word(0x81,61,64)),Some((2,Event::Off { key })));
        assert_eq!(k.decode(2,word(0xb1,123,0)),Some((2,Event::Panic)));
    }
}
