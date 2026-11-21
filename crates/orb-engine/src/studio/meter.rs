//! Display-only sample peaks. No signal processing, allocation or callback-rate dependence.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Reading {
    pub peak: f32,
    pub hold: f32,
    pub clip: bool,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Meter {
    peak: f64,
    level: f64,
    hold: f64,
    hold_frames: usize,
    clip_frames: usize,
}

impl Meter {
    pub fn sample(&mut self, sample: f64) {
        // Invalid audio must light the overload indicator rather than disappear from metering.
        let value = if sample.is_finite() {
            sample.abs()
        } else {
            2.0
        };
        self.peak = self.peak.max(value);
    }

    pub fn finish(&mut self) {
        // 24 dB/sec release; one-second peak hold and three-second overload latch.
        const RELEASE: f64 = 0.995_998_084_982_693;
        self.level = self.peak.max(self.level * RELEASE);
        if self.peak >= self.hold {
            self.hold = self.peak;
            self.hold_frames = 44100;
        } else if self.hold_frames > 0 {
            self.hold_frames = self.hold_frames.saturating_sub(64);
        } else {
            self.hold = self.level.max(self.hold * RELEASE);
        }
        if self.peak >= 1.0 {
            self.clip_frames = 3 * 44100;
        } else {
            self.clip_frames = self.clip_frames.saturating_sub(64);
        }
        self.peak = 0.0;
        if self.level < 1e-9 {
            self.level = 0.0;
        }
        if self.hold < 1e-9 {
            self.hold = 0.0;
        }
    }

    pub fn reading(&self) -> Reading {
        Reading {
            peak: self.level as f32,
            hold: self.hold as f32,
            clip: self.clip_frames > 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_sample_overload_survives_gui_polling_and_releases_in_audio_time() {
        let mut meter = Meter::default();
        for i in 0..64 {
            meter.sample(if i == 37 { -1.25 } else { 0.0 });
        }
        meter.finish();
        assert_eq!(meter.reading().peak, 1.25);
        assert!(meter.reading().clip);
        for _ in 0..(44100 / 64) {
            meter.finish();
        }
        assert!(meter.reading().clip);
        assert_eq!(meter.reading().hold, 1.25);
        assert!(meter.reading().peak < 0.09);
        for _ in 0..(44100 * 3 / 64) {
            meter.finish();
        }
        assert!(!meter.reading().clip);
        assert!(meter.reading().hold < 0.001);
        meter.sample(f64::NAN);
        meter.finish();
        assert!(meter.reading().clip);
    }
}
