//! Independent, non-destructive previews of arpeggios, applied as ordinary TB patterns.

mod ui;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    #[default]
    Chord,
    Pattern,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Chord {
    Major,
    #[default]
    Minor,
    Dominant7,
    Major7,
    Minor7,
    Sus2,
    Sus4,
    Diminished,
    Augmented,
}

impl Chord {
    pub const ALL: [Self; 9] = [
        Self::Major,
        Self::Minor,
        Self::Dominant7,
        Self::Major7,
        Self::Minor7,
        Self::Sus2,
        Self::Sus4,
        Self::Diminished,
        Self::Augmented,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Major => "Major",
            Self::Minor => "Minor",
            Self::Dominant7 => "Dominant 7",
            Self::Major7 => "Major 7",
            Self::Minor7 => "Minor 7",
            Self::Sus2 => "Sus2",
            Self::Sus4 => "Sus4",
            Self::Diminished => "Diminished",
            Self::Augmented => "Augmented",
        }
    }

    pub fn intervals(self) -> &'static [i32] {
        match self {
            Self::Major => &[0, 4, 7],
            Self::Minor => &[0, 3, 7],
            Self::Dominant7 => &[0, 4, 7, 10],
            Self::Major7 => &[0, 4, 7, 11],
            Self::Minor7 => &[0, 3, 7, 10],
            Self::Sus2 => &[0, 2, 7],
            Self::Sus4 => &[0, 5, 7],
            Self::Diminished => &[0, 3, 6],
            Self::Augmented => &[0, 4, 8],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    Up,
    Down,
    UpDown,
    DownUp,
    AsWritten,
    Random,
}

impl Order {
    pub const ALL: [Self; 6] = [
        Self::Up,
        Self::Down,
        Self::UpDown,
        Self::DownUp,
        Self::AsWritten,
        Self::Random,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Up => "Up",
            Self::Down => "Down",
            Self::UpDown => "Up / Down",
            Self::DownUp => "Down / Up",
            Self::AsWritten => "Source order",
            Self::Random => "Random",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub source: Source,
    pub root: u8,
    pub chord: Chord,
    pub order: Order,
    pub octaves: u8,
    pub length: u8,
    pub every: u8,
    pub offset: u8,
    pub accent_every: u8,
    pub slides: bool,
    pub seed: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            source: Source::Chord,
            root: 2,
            chord: Chord::Minor,
            order: Order::Up,
            octaves: 1,
            length: 16,
            every: 1,
            offset: 0,
            accent_every: 4,
            slides: false,
            seed: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    pub bytes: Vec<u8>,
    pub notes: Vec<i32>,
}

fn unique_push(notes: &mut Vec<i32>, pitch: i32) {
    if !notes.contains(&pitch) {
        notes.push(pitch);
    }
}

fn source_notes(options: &Options, pattern: &[u8]) -> Result<Vec<i32>, String> {
    if options.source == Source::Chord {
        return Ok(options
            .chord
            .intervals()
            .iter()
            .map(|&n| i32::from(options.root % 12) + n)
            .collect());
    }
    if pattern.len() != 98 {
        return Err("No TB pattern is available.".into());
    }
    let mut notes = Vec::new();
    for b in pattern[2..]
        .as_chunks::<6>()
        .0
        .iter()
        .take(usize::from(pattern[1].clamp(1, 16)))
    {
        if b[5] != 0 {
            continue;
        }
        if b[0] > 12 {
            return Err("The source pattern contains an out-of-range note.".into());
        }
        let pitch = i32::from(b[0]) + 12 * i32::from(b[3] != 0) - 12 * i32::from(b[4] != 0);
        unique_push(&mut notes, pitch);
    }
    if notes.is_empty() {
        return Err("The pattern has no active notes. Add notes or choose Chord source.".into());
    }
    Ok(notes)
}

/// Endpoint-free bouncing: three notes give 0,1,2,1 rather than repeating 2 and 0.
fn index(order: Order, step: usize, count: usize) -> usize {
    if count < 2 {
        return 0;
    }
    match order {
        Order::Down => count - 1 - step % count,
        Order::UpDown | Order::DownUp => {
            let p = step % (2 * count - 2);
            let p = if p < count { p } else { 2 * count - 2 - p };
            if order == Order::DownUp {
                count - 1 - p
            } else {
                p
            }
        }
        _ => step % count,
    }
}

fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut n = *seed;
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    n ^ (n >> 31)
}

/// Preview and apply use exactly the same deterministic renderer. Source bytes are never edited.
pub fn render(options: &Options, pattern: &[u8]) -> Result<Preview, String> {
    let source = source_notes(options, pattern)?;
    let mut expanded = Vec::new();
    for octave in 0..options.octaves.clamp(1, 3) {
        for &pitch in &source {
            unique_push(&mut expanded, pitch + 12 * i32::from(octave));
        }
    }
    // Fit the expanded set into the original TB's note/down/up register. Never clamp a
    // semitone (which would change the harmony); omit only notes outside the register.
    let mut shift = 0;
    let highest = *expanded.iter().max().unwrap();
    while highest + shift > 24 {
        shift -= 12;
    }
    let mut notes: Vec<_> = expanded
        .into_iter()
        .map(|n| n + shift)
        .filter(|n| (-12..=24).contains(n))
        .collect();
    if options.order != Order::AsWritten {
        notes.sort_unstable();
    }
    if notes.is_empty() {
        return Err("The note set does not fit the TB register.".into());
    }
    let length = usize::from(options.length.clamp(1, 16));
    let every = usize::from(options.every.clamp(1, 16));
    let mut bytes = vec![0; 98];
    bytes[0] = u8::from(pattern.first().is_some_and(|&b| b != 0));
    bytes[1] = length as u8;
    for b in bytes[2..].as_chunks_mut::<6>().0 {
        b[5] = 1;
    }
    let mut seed = options.seed;
    let mut previous = None;
    for (pulse, step) in (0..length).step_by(every).enumerate() {
        let at = if options.order == Order::Random {
            if previous.is_none() {
                (random(&mut seed) % notes.len() as u64) as usize
            } else if notes.len() > 1 {
                let p = (random(&mut seed) % (notes.len() - 1) as u64) as usize;
                if previous.is_some_and(|last| p >= last) {
                    p + 1
                } else {
                    p
                }
            } else {
                0
            }
        } else {
            index(
                options.order,
                pulse + usize::from(options.offset),
                notes.len(),
            )
        };
        previous = Some(at);
        let pitch = notes[at];
        let b = &mut bytes[2 + step * 6..8 + step * 6];
        b[0] = if pitch == 24 {
            12
        } else {
            pitch.rem_euclid(12) as u8
        };
        b[2] = u8::from(options.accent_every > 0 && pulse % usize::from(options.accent_every) == 0);
        b[3] = u8::from(pitch >= 12);
        b[4] = u8::from(pitch < 0);
        b[5] = 0;
    }
    if options.slides && length > 1 {
        for s in 0..length {
            let next = (s + 1) % length;
            bytes[2 + 6 * s + 1] =
                u8::from(bytes[2 + 6 * s + 5] == 0 && bytes[2 + 6 * next + 5] == 0);
        }
    }
    Ok(Preview { bytes, notes })
}

pub fn note_name(pitch: i32) -> String {
    let name = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ][pitch.rem_euclid(12) as usize];
    match pitch.div_euclid(12) {
        0 => name.into(),
        octave => format!("{name}({octave:+})"),
    }
}

struct Undo {
    pattern: u8,
    before: Vec<u8>,
    after: Vec<u8>,
}

pub struct Arpeggiator {
    pub open: [bool; 2],
    pub options: [Options; 2],
    undo: [Option<Undo>; 2],
    notice: [Option<String>; 2],
}

impl Default for Arpeggiator {
    fn default() -> Self {
        Self {
            open: [false; 2],
            options: std::array::from_fn(|i| Options {
                seed: i as u64 + 1,
                ..Options::default()
            }),
            undo: [None, None],
            notice: [None, None],
        }
    }
}

impl Arpeggiator {
    pub fn clear_undo(&mut self) {
        self.undo = [None, None];
        self.notice = [None, None];
    }
}
