//! Harmony shared by pad generation and the reverse TB-to-pad pitch fit.
use super::Rng;
use orb_engine::studio::acid_synth::{Slot, Step};

const PROGRESSIONS: [[usize; 8]; 5] = [
    [0, 5, 2, 6, 0, 3, 5, 6],
    [0, 3, 5, 4, 2, 3, 4, 0],
    [0, 6, 3, 5, 0, 2, 3, 6],
    [5, 3, 0, 4, 5, 2, 3, 4],
    [0, 2, 5, 3, 0, 6, 4, 0],
];

// Relative pitch classes. Ninth chords omit the fifth so four independent voices suffice.
const COLORS: [[u8; 4]; 9] = [
    [0, 3, 7, 10], [0, 4, 7, 11], [0, 4, 7, 10],
    [0, 2, 3, 10], [0, 2, 4, 11], [0, 2, 4, 10],
    [0, 2, 5, 7], [0, 5, 7, 10], [0, 2, 4, 9],
];

pub(super) struct Voicer<'a> {
    root: u8,
    scale: &'a [u8],
    upper: bool,
    plan: usize,
    event: usize,
    previous: Option<[u8; 4]>,
    rng: Rng,
}

impl<'a> Voicer<'a> {
    pub(super) fn new(root: u8, scale: &'a [u8], seed: u64, upper: bool) -> Self {
        let mut rng = Rng(seed);
        let plan = rng.pick(PROGRESSIONS.len());
        Self { root: root % 12, scale, upper, plan, event: 0, previous: None, rng }
    }

    pub(super) fn next(&mut self, weights: &[i32; 12], variation: u8) -> [u8; 4] {
        let scale = if self.scale.is_empty() { &[0, 2, 3, 5, 7, 8, 10][..] } else { self.scale };
        let stage = PROGRESSIONS[self.plan][self.event % 8] * scale.len() / 7;
        let color = (self.event + self.rng.pick(3)) % 3;
        let floor = if self.upper { 55 } else { 43 };
        let target = self.previous.unwrap_or([floor + 9, floor + 16, floor + 21, floor + 27]);
        let total: i64 = weights.iter().map(|w| i64::from((*w).max(0))).sum();
        let pedal = weights.iter().position(|w| *w > 0)
            .filter(|_| weights.iter().filter(|w| **w > 0).count() == 1);
        let mut best = (i32::MIN, 0_u64, target);
        for (degree, &interval) in scale.iter().enumerate() {
            let chord_root = (self.root + interval) % 12;
            for (quality, shape) in COLORS.iter().enumerate() {
                if !shape.iter().all(|d| scale.contains(&((interval + d) % 12))) { continue; }
                let pcs = shape.map(|d| (chord_root + d) % 12);
                let support: i64 = weights.iter().enumerate().map(|(bass, weight)| {
                    let consonance = if pcs.contains(&(bass as u8)) { 110 } else {
                        // Guide tones of two basslines can compete. Prefer consonant intervals
                        // when their combined notes cannot all belong to one chord.
                        pcs.iter().map(|tone| match (usize::from(*tone) + 12 - bass) % 12 {
                            3 | 4 | 5 | 7 | 8 | 9 => 12, 1 | 11 => -5, 6 => -4, _ => 2,
                        }).sum::<i64>()
                    };
                    consonance * i64::from((*weight).max(0))
                }).sum();
                let mut score = (support / total.max(1)) as i32 * 3;
                if degree == stage { score += 79; }
                if quality / 3 == color { score += 14 + i32::from(variation.min(100)) / 5; }
                if self.previous.is_some_and(|prev| pcs.iter().all(|pc| prev.iter().any(|n| n % 12 == *pc))) {
                    score -= 62;
                }
                for inversion in 0..4 {
                    let mut closed = [0_u8; 4];
                    closed[0] = floor + (pcs[inversion] + 12 - floor % 12) % 12;
                    for i in 1..4 {
                        let previous = closed[i-1];
                        closed[i] = previous + (pcs[(inversion+i) % 4] + 12 - previous % 12) % 12;
                    }
                    for spread in [None, Some(1), Some(3)] {
                        let mut notes = closed;
                        if let Some(i) = spread { notes[i] += 12; notes.sort_unstable(); }
                        let motion: i32 = notes.iter().zip(target).map(|(a,b)| i32::from(a.abs_diff(b))).sum();
                        let crowded = notes.windows(2).filter(|p| p[0] < 66 && p[1] - p[0] < 3).count() as i32;
                        let centre = target[0].abs_diff(notes[0]) as i32;
                        let mut rank = score - 2 * motion - 7 * crowded - centre;
                        if pedal.is_some_and(|pc| !notes[..3].iter().any(|n| usize::from(n % 12) == pc)) {
                            rank -= 105;
                        }
                        let tie = self.rng.next();
                        if (rank, tie) > (best.0, best.1) { best = (rank, tie, notes); }
                    }
                }
            }
        }
        self.previous = Some(best.2);
        self.event += 1;
        best.2
    }
}

/// Keep a pad under the groove until the next actual chord attacks, including across loop wrap.
pub(super) fn legato(steps: &mut [Step]) {
    let Some(first) = steps.iter().position(|s| s.notes.iter().any(|n| *n != 0)) else { return; };
    let mut previous = first;
    for i in first + 1..steps.len() {
        if steps[i].notes.iter().any(|n| *n != 0) {
            steps[previous].gate_steps = (i - previous) as u16;
            previous = i;
        }
    }
    steps[previous].gate_steps = (steps.len() - previous + first) as u16;
}

/// At each pad step, collect pitch classes sustaining across it, including a gate across the loop.
/// Difference arrays keep preparation linear even with thousands of overlapping long gates.
fn chord_masks(slot: &Slot) -> Vec<u16> {
    let count = slot.step_count();
    let mut changes = vec![[0_i32; 12]; count + 1];
    for i in 0..count {
        let step = slot.step(i);
        let end = i + usize::from(step.gate_steps).min(count);
        for &note in &step.notes {
            if note == 0 { continue; }
            let pc = usize::from(note % 12);
            changes[i][pc] += 1;
            changes[end.min(count)][pc] -= 1;
            if end > count {
                changes[0][pc] += 1;
                changes[end-count][pc] -= 1;
            }
        }
    }
    let mut active = [0_i32; 12];
    changes.iter().take(count).map(|deltas| {
        let mut mask = 0;
        for (pc, value) in active.iter_mut().enumerate() {
            *value += deltas[pc];
            if *value > 0 { mask |= 1 << pc; }
        }
        mask
    }).collect()
}

/// Fit only sounding pitches. Native accent, timing, slide, rest and shuffle bytes stay intact.
pub(super) fn fit_to_chords(bytes: &mut [u8], tune: i16, slots: &[&Slot], seed: u64) {
    if bytes.len() < 8 { return; }
    let len = usize::from(bytes[1].clamp(1, 16)).min((bytes.len()-2)/6);
    let period = len * 48;
    let mut votes = [[0_i64; 12]; 16];
    let mut totals = [0_i64; 16];
    for &slot in slots {
        let masks = chord_masks(slot);
        if masks.iter().all(|&m| m == 0) { continue; }
        let ticks = usize::from(slot.ticks_per_step());
        let span = masks.len() * ticks;
        for i in 0..len {
            let initial = (i * 48) % span;
            let mut pos = initial;
            let mut hits = [0_i64; 12];
            let mut observed = 0;
            loop {
                let mut mask = masks[pos / ticks];
                if mask != 0 { observed += 1; }
                while mask != 0 {
                    hits[mask.trailing_zeros() as usize] += 1;
                    mask &= mask - 1;
                }
                pos = (pos + period) % span;
                if pos == initial { break; }
            }
            if observed > 0 {
                // Each running synth has the same vote irrespective of its phrase length.
                for (vote, hits) in votes[i].iter_mut().zip(hits) { *vote += hits * 1000 / observed; }
                totals[i] += 1;
            }
        }
    }
    let mut rng = Rng(seed);
    for (i, data) in bytes[2..].chunks_exact_mut(6).take(len).enumerate() {
        if data[5] != 0 || totals[i] == 0 { continue; }
        let old = i32::from(data[0]) + 12 * i32::from(data[3] != 0) - 12 * i32::from(data[4] != 0);
        let pitch = (-12_i32..=23).max_by_key(|&p| {
            let pc = (p + i32::from(tune)).rem_euclid(12) as usize;
            let distance = i64::from((old-p).abs());
            (votes[i][pc] / totals[i] - distance * 28, rng.next())
        }).unwrap();
        data[0] = pitch.rem_euclid(12) as u8;
        data[3] = u8::from(pitch >= 12);
        data[4] = u8::from(pitch < 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{Scale, bytes303, note, pitch, steps303};

    #[test]
    fn eight_bars_over_a_d_pedal_reharmonize_with_voice_leading_and_stay_in_key() {
        let scale = Scale::Minor.intervals();
        let mut voicer = Voicer::new(2, scale, 338, false);
        let mut duplicate = Voicer::new(2, scale, 338, false);
        let mut weights = [0; 12];
        weights[2] = 7;
        let mut chords = std::collections::BTreeSet::new();
        for _ in 0..8 {
            let notes = voicer.next(&weights, 65);
            assert_eq!(notes, duplicate.next(&weights, 65));
            assert!(notes.windows(2).all(|w| w[0] < w[1]));
            assert!(notes[..3].iter().any(|n| n % 12 == 2));
            assert!(notes.iter().all(|n| scale.contains(&((n + 10) % 12))));
            let mut classes = notes.map(|n| n % 12);
            classes.sort_unstable();
            chords.insert(classes);
        }
        assert!(chords.len() >= 2, "a sustained bass note still needs harmonic movement");
    }

    #[test]
    fn low_and_upper_voicings_honor_pentatonic_scale() {
        let scale = Scale::Pentatonic.intervals();
        let mut low = Voicer::new(11, scale, 78, false);
        let mut high = Voicer::new(11, scale, 78, true);
        for _ in 0..12 {
            let a = low.next(&[0; 12], 80);
            let b = high.next(&[0; 12], 80);
            assert!(a.iter().chain(&b).all(|n| scale.contains(&((n+1) % 12))));
            assert!(b.iter().map(|&n| u16::from(n)).sum::<u16>()
                > a.iter().map(|&n| u16::from(n)).sum::<u16>());
        }
    }

    #[test]
    fn legato_connects_offbeats_across_the_phrase_boundary() {
        let mut steps = [Step::default(); 16];
        steps[3].notes[0] = 55;
        steps[10].notes[0] = 60;
        legato(&mut steps);
        assert_eq!(steps[3].gate_steps, 7);
        assert_eq!(steps[10].gate_steps, 9);
        assert_eq!(steps[4], Step::default());
    }

    #[test]
    fn reverse_link_preserves_rhythm_and_accounts_for_live_tuning_and_wrapped_gates() {
        let mut slot = Slot::default();
        slot.enabled = true;
        slot.step_ticks = 48;
        slot.steps = [Step::default(); 16];
        slot.steps[14] = Step { notes: [50, 53, 57, 60], gate_steps: 4 };
        let mut source = [note(1, true); 16];
        source[0].slide = true;
        source[1].rest = true;
        let original = bytes303(&source, 16);
        let mut bytes = original.clone();
        fit_to_chords(&mut bytes, 2, &[&slot], 3);
        let result = steps303(&bytes);
        for i in [0, 14, 15] {
            let sounding = (pitch(result[i]) + 2).rem_euclid(12);
            assert!([2, 5, 9, 0].contains(&sounding));
        }
        assert_eq!(result[0].slide, source[0].slide);
        assert_eq!(result[0].accent, source[0].accent);
        assert_eq!(&bytes[8..86], &original[8..86]);
        slot.enabled = false;
        slot.sequence_enabled = false;
        let mut recalled = original.clone();
        fit_to_chords(&mut recalled, 2, &[&slot], 3);
        assert_eq!(recalled, bytes);
        let mut unlinked = original.clone();
        fit_to_chords(&mut unlinked, 2, &[], 3);
        assert_eq!(unlinked, original);
    }
}
