//! Voice pad phrases against the sounding TB line, including its native tuning.
use orb_engine::studio::acid_synth::{PhraseLength, Slot, Step};

use super::{Options, Rng, pitch, steps303};
use crate::model::Step303;

pub(super) struct PadChange {
    pub row: usize,
    pub length: PhraseLength,
    pub ticks: u16,
    pub before: Vec<Step>,
    pub after: Vec<Step>,
}

impl PadChange {
    pub fn matches(&self, slot: &Slot) -> bool {
        self.length == slot.length && self.ticks == slot.ticks_per_step()
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

/// A source keeps its own note grid and tuning even when the synth faces the rack.
pub(super) struct Riff {
    steps: [Step303; 16],
    length: usize,
    transpose: i16,
    weight: i32,
}

impl Riff {
    pub fn new(bytes: &[u8], transpose: i16, weight: i32) -> Self {
        Self { steps: steps303(bytes), length: usize::from(bytes[1].clamp(1, 16)), transpose, weight }
    }

    fn pitch_class(&self, step: Step303) -> usize {
        (pitch(step) + i32::from(self.transpose)).rem_euclid(12) as usize
    }

    pub fn silent(&self) -> bool {
        self.steps[..self.length].iter().all(|s| s.rest)
    }

    fn evidence(&self, start: usize, end: usize, ticks: usize, cycle: usize, weights: &mut [i32; 12]) -> Option<usize> {
        let period = self.length * 48;
        let mut local = [0; 12];
        let mut first = None;
        let mut phase = 0;
        let mut phases = 0;
        loop {
            let from = start * ticks + phase;
            let to = end * ticks + phase;
            for beat in from / 48..to.div_ceil(48) {
                let n = self.steps[beat % self.length];
                if n.rest { continue; }
                first.get_or_insert(start + (beat * 48).saturating_sub(from) / ticks);
                local[self.pitch_class(n)] += 1 + 3 * i32::from(n.accent) + i32::from(n.slide)
                    + 2 * i32::from(beat % 4 == 0);
            }
            phases += 1;
            phase = (phase + cycle) % period;
            if phase == 0 { break; }
        }
        // Unequal loop lengths need different numbers of phases; normalize before combining A and B.
        for (total, count) in weights.iter_mut().zip(local) {
            *total += (count * self.weight * 16 + phases / 2) / phases;
        }
        first
    }
}

pub(super) fn linked_phrase(options: &Options, riffs: &[Riff], slot: &Slot, seed: u64, upper: bool) -> Vec<Step> {
    let mut out = vec![Step::default(); slot.step_count()];
    if options.density == 0 { return out; }
    let mut key_fit = [0; 12];
    for riff in riffs {
        for note in &riff.steps[..riff.length] {
            if !note.rest {
                key_fit[riff.pitch_class(*note)] += riff.weight * if note.accent { 3 } else { 1 };
            }
        }
    }
    let scale = options.scale.intervals();
    let transpose = riffs.iter().filter(|riff| !riff.silent())
        .max_by_key(|riff| riff.weight).map_or(0, |riff| riff.transpose);
    let requested = (i32::from(options.root) + i32::from(transpose)).rem_euclid(12) as usize;
    let root = (0..12).max_by_key(|&root| {
        let fit: i32 = scale.iter().map(|&degree| key_fit[(root + usize::from(degree)) % 12]).sum();
        (fit, root == requested)
    }).unwrap();
    let mut harmony = super::harmony::Voicer::new(root as u8, scale, seed, upper);
    let mut rng = Rng(seed);
    let ticks = usize::from(slot.ticks_per_step());
    let cycle = ticks * out.len();
    let mut start = 0;
    while start < out.len() {
        let span = if cycle <= 768 && options.variation > 0 {
            if rng.chance(options.variation) { 192 } else { 384 }
        } else if rng.chance(options.variation.min(90)) { 1536 } else { 768 };
        let end = (start + (span / ticks).max(1)).min(out.len());
        let mut weights = [0; 12];
        let first = riffs.iter().filter_map(|riff| riff.evidence(start, end, ticks, cycle, &mut weights)).min();
        if let Some(onset) = first {
            out[onset] = Step { notes: harmony.next(&weights, options.variation), gate_steps: (end - onset) as u16 };
        }
        start = end;
    }
    if options.variation > 0 { super::harmony::legato(&mut out); }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{Scale, bytes303, note};
    use crate::model::Step303;

    #[test]
    fn hears_both_riffs_with_independent_tuning_and_odd_loop_lengths() {
        let mut a = [Step303::default(); 16];
        let mut b = a;
        a[0] = note(0, true);
        b[2] = note(7, true);
        let a = Riff::new(&bytes303(&a, 7), 1, 4);
        let b = Riff::new(&bytes303(&b, 13), -1, 1);
        let mut weights = [0; 12];
        for riff in [a, b] {
            assert!(riff.evidence(0, 16, 48, 768, &mut weights).is_some());
        }
        assert!(weights[1] > weights[6] && weights[6] > 0);
        assert_eq!(weights.iter().filter(|weight| **weight != 0).count(), 2);
    }

    #[test]
    fn follows_the_riffs_attacks_accents_and_native_tuning_at_any_phrase_length() {
        let mut bass = [Step303::default(); 16];
        bass[3] = note(2, true);
        let options = Options { variation: 0, ..Default::default() };
        let source = bytes303(&bass, 16);
        for bars in [16, 32, 128] {
            let mut slot = Slot::default();
            slot.length = PhraseLength::Bars(bars);
            slot.set_step_rate(48);
            let riffs = [Riff::new(&source, 1, 1)];
            let steps = linked_phrase(&options, &riffs, &slot, 901, false);
            assert_eq!(steps, linked_phrase(&options, &riffs, &slot, 901, false));
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
        assert_eq!(linked_phrase(&Options::default(), &[Riff::new(&bytes, 0, 1)], &slot, 7, false), vec![Step::default(); 32]);
    }
}
