//! Prepare the held-chord lookup before transferring a replacement to the audio thread.
use super::{Slot, VOICES};
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub(super) struct Prepared(Option<Arc<Index>>);

// This derived playback cache is neither saved project data nor an editable setting.
impl PartialEq for Prepared {
    fn eq(&self, _other: &Self) -> bool { true }
}

#[derive(Debug)]
struct Index {
    fingerprint: u64,
    /// Distances to prior active chord onsets, newest first. Zero terminates each entry.
    prior: Box<[[u16; VOICES]]>,
}

impl Prepared {
    pub fn build(slot: &Slot) -> Self {
        let count = slot.step_count();
        let mut hash = DefaultHasher::new();
        count.hash(&mut hash);
        slot.ticks_per_step().hash(&mut hash);
        for i in 0..count {
            let step = slot.step(i);
            if step.notes.iter().any(|&n| n != 0) {
                (i, step.notes, usize::from(step.gate_steps).min(count)).hash(&mut hash);
            }
        }
        let fingerprint = hash.finish();
        if slot.prepared.fingerprint() == Some(fingerprint) { return slot.prepared.clone(); }

        let mut prior = vec![[0; VOICES]; count].into_boxed_slice();
        let mut active = BTreeSet::new();
        let mut expiry = BinaryHeap::new();
        // The first cycle supplies wrapped gates for the second. During the first
        // transport pass the reader rejects any onset before absolute tick zero.
        for n in 0..2 * count {
            while let Some(&Reverse((end, onset))) = expiry.peek() {
                if end > n { break; }
                expiry.pop();
                active.remove(&onset);
            }
            if n >= count {
                for (age, &onset) in prior[n-count].iter_mut().zip(active.iter().rev()) {
                    *age = (n - onset) as u16;
                }
            }
            let step = slot.step(n % count);
            let gate = usize::from(step.gate_steps).min(count);
            if gate > 1 && step.notes.iter().any(|&note| note != 0) {
                active.insert(n);
                expiry.push(Reverse((n + gate, n)));
            }
        }
        Self(Some(Arc::new(Index { fingerprint, prior })))
    }

    pub fn fingerprint(&self) -> Option<u64> {
        self.0.as_ref().map(|data| data.fingerprint)
    }

    pub fn prior(&self, at: usize) -> Option<&[u16; VOICES]> {
        self.0.as_ref()?.prior.get(at)
    }
}
