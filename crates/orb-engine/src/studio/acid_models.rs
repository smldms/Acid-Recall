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

#[derive(Clone, Copy, Default)]
pub(super) struct State {
    phase: [f32; 4],
    feedback: f32,
    slave: f32,
    history: [f32; 7],
    head: usize,
}

impl State {
    /// One FM operator. Attenuate partials approaching the internal Nyquist frequency.
    fn operator(bank: &Bank, phase: f32, offset: f32, increment: f32) -> f32 {
        let gain = ((0.49 - increment) * 12.5).clamp(0.0, 1.0);
        gain * bank.sine((phase + offset).rem_euclid(1.0))
    }

    fn fm(&mut self, bank: &Bank, c: &Controls, delta: f32) -> f32 {
        let increments = [delta, delta * c.ratios[0], delta * c.ratios[1], delta * c.ratios[2]];
        let phase = self.phase;
        let op = |i: usize, pm: f32| Self::operator(bank, phase[i], pm, increments[i]);
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
            *phase = (*phase + inc).fract();
        }
        output
    }

    fn sync_fold(&mut self, bank: &Bank, c: &Controls, delta: f32) -> f32 {
        let master = bank.sine(self.phase[0]);
        let slave = Self::operator(bank, self.slave, 0.0, delta * c.sync_ratio);
        let input = slave * c.fold;
        let folded = 1.0 - ((input + 1.0).rem_euclid(4.0) - 2.0).abs();
        let out = master + (folded - master) * c.mix;

        let next = self.phase[0] + delta;
        self.phase[0] = next.fract();
        self.slave = if next >= 1.0 {
            // Fractional reset retains the part of this sub-sample after master wrap.
            (self.phase[0] * c.sync_ratio).fract()
        } else {
            (self.slave + delta * c.sync_ratio).fract()
        };
        out
    }

    pub fn sample(&mut self, bank: &Bank, c: &Controls, increment: f32) -> f32 {
        let delta = increment * 0.5;
        for _ in 0..2 {
            let x = if c.mode == 3 { self.fm(bank, c, delta) } else { self.sync_fold(bank, c, delta) };
            self.history[self.head] = x;
            self.head = (self.head + 1) % 7;
        }
        // Seven-tap halfband decimator, unity DC gain. The centre tap is three sub-samples old.
        let delayed = |n: usize| self.history[(self.head + 6 - n) % 7];
        0.5 * delayed(3) + 0.2956 * (delayed(2) + delayed(4))
            - 0.0456 * (delayed(0) + delayed(6))
    }
}
