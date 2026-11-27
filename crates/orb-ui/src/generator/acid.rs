//! Voice pad phrases against the sounding TB line, including its native tuning.
use orb_engine::studio::acid_synth::{PhraseLength, Slot, Step};

use super::{Options, Rng, pitch, steps303};

pub(super) struct PadChange {
    pub row: usize,
    pub length: PhraseLength,
    pub ticks: u16,
    pub before: Vec<Step>,
    pub after: Vec<Step>,
}

impl PadChange {
    pub fn matches(&self, slot: &Slot) -> bool {
        slot.enabled && self.length == slot.length && self.ticks == slot.ticks_per_step()
            && self.after.iter().enumerate().all(|(i, step)| slot.step(i) == *step)
    }

    pub fn write(slot: &mut Slot, steps: &[Step]) {
        for (index, step) in steps.iter().enumerate() {
            if slot.step(index) != *step {
                *slot.step_mut(index) = *step;
            }
        }
    }
}

/// Generate only the notes and gates. Timing follows the destination's current grid.
pub(super) fn phrase(options: &Options, bytes: &[u8], transpose: i16, slot: &Slot, seed: u64, upper: bool) -> Vec<Step> {
    let tb = steps303(bytes);
    let tb = &tb[..usize::from(bytes[1].clamp(1, 16))];
    let mut out = vec![Step::default(); slot.step_count()];
    if options.density == 0 || tb.iter().all(|note| note.rest) {
        return out;
    }
    let pc = |n| (pitch(n) + i32::from(transpose)).rem_euclid(12) as usize;
    let scale = options.scale.intervals();
    let requested = (i32::from(options.root) + i32::from(transpose)).rem_euclid(12) as usize;
    // Prefer the configured key on a tie, but follow an existing transposed or hand-written riff.
    let root = (0..12).max_by_key(|&root| {
        let fit: usize = tb.iter().filter(|n| !n.rest && scale.contains(&(((pc(**n) + 12 - root) % 12) as u8)))
            .map(|n| if n.accent { 3 } else { 1 }).sum();
        (fit, root == requested)
    }).unwrap();
    let mut triads = Vec::new();
    for &degree in scale {
        for third in [3, 4] {
            let intervals = [degree, (degree + third) % 12, (degree + 7) % 12];
            if intervals.iter().all(|i| scale.contains(i)) {
                triads.push(intervals.map(|i| (root + usize::from(i)) % 12));
            }
        }
    }
    let ticks = usize::from(slot.ticks_per_step()).max(1);
    let mut rng = Rng(seed ^ 0x8b2a_763f_43e5_12df);
    let mut start = 0;
    while start < out.len() {
        let bars = if rng.chance(options.variation.min(90)) { 2 } else { 1 };
        let end = (start + (768 * bars / ticks).max(1)).min(out.len());
        let mut weights = [0i32; 12];
        let mut first = None;
        // Score all alignments of unequal loop lengths. An odd-length TB meets
        // a different portion of the same pad every time the pad loops.
        let period = tb.len() * 48;
        let mut phase = 0;
        loop {
            let from = start * ticks + phase;
            let to = end * ticks + phase;
            for beat in from / 48..to.div_ceil(48) {
                let n = tb[beat % tb.len()];
                if !n.rest {
                    let offset = (beat * 48).saturating_sub(from) / ticks;
                    first.get_or_insert((start + offset).min(end - 1));
                    weights[pc(n)] += 1 + 3 * i32::from(n.accent) + i32::from(n.slide)
                        + 2 * i32::from(beat % 4 == 0);
                }
            }
            phase = (phase + out.len() * ticks) % period;
            if phase == 0 { break; }
        }
        if let Some(onset) = first {
            let score = |chord: [usize; 3]| -> i32 {
                weights.iter().enumerate().map(|(bass, &weight)| {
                    let fit: i32 = chord.iter().map(|note| match (note + 12 - bass) % 12 {
                        0 => 7, 3 | 4 | 5 | 7 | 8 | 9 => 2, 1 | 11 => -4, 6 => -3, _ => -1,
                    }).sum();
                    weight * fit
                }).sum()
            };
            let triad = *triads.iter().max_by_key(|&&chord| (score(chord), rng.next())).unwrap();
            let mut notes = [0; 4];
            for (i, tone) in triad.into_iter().enumerate() {
                notes[i] = (if upper { 67 } else { 55 }) + ((tone + 5) % 12) as u8;
            }
            notes[..3].sort_unstable();
            if rng.chance(options.variation.min(100)) {
                notes[0] += 12;
                notes[..3].sort_unstable();
            }
            if rng.chance(options.variation / 2) {
                for seventh in [10, 11] {
                    let tone = (triad[0] + seventh) % 12;
                    if scale.contains(&(((tone + 12 - root) % 12) as u8)) {
                        let top = notes[2];
                        notes[3] = top + ((tone + 12 - usize::from(top % 12)) % 12) as u8;
                        break;
                    }
                }
            }
            out[onset] = Step { notes, gate_steps: (end - onset) as u16 };
        }
        start = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{Scale, bytes303, note};
    use crate::model::Step303;

    #[test]
    fn follows_the_riffs_attacks_accents_and_native_tuning_at_any_phrase_length() {
        let mut bass = [Step303::default(); 16];
        bass[3] = note(2, true);
        let options = Options { variation: 0, ..Default::default() };
        let source = bytes303(&bass, 16);
        for bars in [16, 32, 128] {
            let mut slot = Slot::default();
            slot.length = PhraseLength::Bars(bars);
            let steps = phrase(&options, &source, 1, &slot, 901, false);
            assert_eq!(steps, phrase(&options, &source, 1, &slot, 901, false));
            for bar in steps.chunks_exact(16) {
                assert_eq!(bar.iter().filter(|s| s.notes[0] != 0).count(), 1);
                let chord = &bar[3];
                assert_eq!(chord.gate_steps, 13);
                let voiced = &chord.notes[..3];
                // The riff is D played one semitone up. Every chord supports its sounding E-flat.
                assert!(voiced.iter().any(|n| n % 12 == 3));
                assert!(voiced.iter().all(|n| Scale::Minor.intervals().contains(&((n + 9) % 12))));
            }
        }
    }

    #[test]
    fn empty_tb_yields_rests_and_the_selected_pad_grid_stays_intact() {
        let mut slot = Slot::default();
        slot.length = PhraseLength::Steps(32);
        slot.step_ticks = 384;
        let bytes = bytes303(&[Step303::default(); 16], 13);
        assert_eq!(phrase(&Options::default(), &bytes, 0, &slot, 7, false), vec![Step::default(); 32]);
    }
}
