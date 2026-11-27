//! Alternative oscillators, evaluated at twice the engine sample rate.
use super::{Bank, Patch};

#[derive(Clone, Copy)]
pub(super) struct Controls {
    mode: u8,
    algorithm: u8,
    ratios: [f32; 3],
    index: f32,
    feedback: f32,
    fold: f32,
    sync_ratio: f32,
    mix: f32,
}

impl Controls {
    pub fn new(p: &Patch, color: f32, offsets: [f32; 2], mix: f32) -> Self {
        let index = (p.fm_index + color * 0.45 + offsets[0]).clamp(0.0, 1.0);
        let feedback = (p.fm_feedback + offsets[1]).clamp(0.0, 1.0);
        let fold = (p.fold + color * 0.45 + offsets[0]).clamp(0.0, 1.0);
        let sync = (p.sync + offsets[1]).clamp(0.0, 1.0);
        Self {
            mode: p.osc_mode,
            algorithm: p.fm_algorithm,
            ratios: p.fm_ratios,
            index: 1.25 * index * index,
            feedback: 0.6 * feedback * feedback,
            fold: 1.0 + 8.0 * fold * fold,
            sync_ratio: 1.0 + 11.0 * sync * sync,
            mix,
        }
    }
}

/// Phase increments and Nyquist rolloff stay constant until a note or block control changes.
#[derive(Clone, Copy, Default)]
pub(super) struct Rates {
    increments: [f32; 4],
    gains: [f32; 4],
}

impl Controls {
    pub fn rates(&self, increment: f32) -> Rates {
        let delta = increment * 0.5;
        let increments = match self.mode {
            3 => [delta, delta * self.ratios[0], delta * self.ratios[1], delta * self.ratios[2]],
            4 => [delta, delta * self.sync_ratio, 0.0, 0.0],
            _ => return Rates::default(),
        };
        Rates { increments, gains: increments.map(rate_gain) }
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct State {
    phase: [f32; 4],
    feedback: f32,
    slave: f32,
    previous: [f32; 5],
}

/// Arguments here are bounded by sanitized ratios, phase modulation depth and MIDI pitch.
/// Integer truncation avoids general-purpose floating remainder in every operator.
#[inline]
fn wrap_cycle(x: f32) -> f32 {
    let fraction = x - (x as i32) as f32;
    if fraction < 0.0 { fraction + 1.0 } else { fraction }
}

#[inline]
fn rate_gain(increment: f32) -> f32 {
    ((0.49 - increment) * 12.5).clamp(0.0, 1.0)
}

impl State {
    /// One FM operator. Attenuate partials approaching the internal Nyquist frequency.
    #[inline]
    fn fm(&mut self, bank: &Bank, c: &Controls, increments: [f32; 4], gains: [f32; 4]) -> f32 {
        let phase = self.phase;
        let op = |i: usize, pm: f32| gains[i] * bank.sine(wrap_cycle(phase[i] + pm));
        let four = op(3, self.feedback * c.feedback);
        self.feedback = (self.feedback + four) * 0.5;
        let index = c.index;
        let output = match c.algorithm {
            1 => {
                let bass = op(0, op(1, 0.0) * index);
                let bell = op(2, four * index);
                bass + (bell - bass) * c.mix
            }
            2 => {
                // A mix of independent modulators feeds one carrier.
                let two = op(1, 0.0);
                let middle = two + (op(2, 0.0) - two) * c.mix;
                op(0, (middle + four * 0.5) * index)
            }
            3 => {
                let fork = four * index;
                let a = op(1, fork);
                let b = op(2, fork);
                op(0, (a + (b - a) * c.mix) * index)
            }
            _ => {
                let three = op(2, four * index);
                let two = op(1, three * index * (0.25 + 1.5 * c.mix));
                op(0, two * index)
            }
        };
        for (phase, inc) in self.phase.iter_mut().zip(increments) {
            *phase = wrap_cycle(*phase + inc);
        }
        output
    }

    #[inline]
    fn sync_fold(&mut self, bank: &Bank, c: &Controls, delta: f32, slave_delta: f32, gain: f32) -> f32 {
        let master = bank.sine(self.phase[0]);
        let slave = gain * bank.sine(self.slave);
        let input = slave * c.fold;
        let folded = 1.0 - (4.0 * wrap_cycle((input + 1.0) * 0.25) - 2.0).abs();
        let out = master + (folded - master) * c.mix;

        let next = self.phase[0] + delta;
        self.phase[0] = wrap_cycle(next);
        self.slave = if next >= 1.0 {
            // Fractional reset retains the part of this sub-sample after master wrap.
            wrap_cycle(self.phase[0] * c.sync_ratio)
        } else {
            wrap_cycle(self.slave + slave_delta)
        };
        out
    }

    pub fn sample(&mut self, bank: &Bank, c: &Controls, rates: &Rates) -> f32 {
        let (even, odd) = if c.mode == 3 {
            let Rates { increments, gains } = *rates;
            let even = self.fm(bank, c, increments, gains);
            (even, self.fm(bank, c, increments, gains))
        } else {
            let [delta, slave_delta, _, _] = rates.increments;
            let gain = rates.gains[1];
            let even = self.sync_fold(bank, c, delta, slave_delta, gain);
            (even, self.sync_fold(bank, c, delta, slave_delta, gain))
        };
        // The zero-valued halfband taps drop out. Keep only the five previous
        // sub-samples needed after both new sub-samples have been calculated.
        let [p0, p1, p2, _, p4] = self.previous;
        self.previous = [odd, even, p0, p1, p2];
        0.5 * p1 + 0.2956 * (p0 + p2) - 0.0456 * (odd + p4)
    }
}
