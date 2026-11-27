//! MIDI performance events. A key identifies the physical port, channel and original note,
//! so two notes mapped to the same scale degree still release independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    On { key: u16, pitch: u8, velocity: u8, grid: u16 },
    Off { key: u16 },
    Audition { notes: [u8; 4], frames: u32 },
    Panic,
}

#[derive(Clone, Copy)]
pub(super) struct Pending {
    pub key: u16,
    pub pitch: u8,
    pub velocity: u8,
    pub onset: u64,
    pub delay: u64,
    pub release: Option<u64>,
}

/// Bounded by the instrument's voice count. Due events use a sample counter, so a loop or
/// transport locate between a note-on and the next grid boundary cannot strand a key.
#[derive(Default)]
pub(super) struct Queue {
    pub sample: u64,
    pub notes: [Option<Pending>; super::acid_synth::VOICES],
    pub next: Option<u64>,
}

impl Queue {
    pub fn refresh(&mut self) {
        self.next = self.notes.iter().flatten().map(|n| n.onset).min();
    }

    pub fn push(&mut self, note: Pending) {
        let index = self.notes.iter().position(Option::is_none)
            .unwrap_or_else(|| self.notes.iter().enumerate()
                .min_by_key(|(_, p)| p.map(|p| p.onset)).unwrap().0);
        self.notes[index] = Some(note);
        self.refresh();
    }
}
