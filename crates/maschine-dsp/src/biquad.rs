//! Correct, click-free biquad filters (RBJ cookbook) for real-time use.
//!
//! The previous implementation used a single pair of delays for both the
//! input and output history of a Direct Form I filter, which produced
//! incorrect frequency responses and clicks when coefficients changed.
//! This implementation uses Direct Form II transposed and crossfades
//! coefficients sample by sample so parameter changes stay smooth.

const COEFF_COUNT: usize = 5;

/// A biquad filter with smooth coefficient interpolation.
#[derive(Clone)]
pub struct Biquad {
    /// b0, b1, b2, a1, a2 currently in effect
    cur: [f32; COEFF_COUNT],
    /// Coefficients at the start of the current crossfade
    start: [f32; COEFF_COUNT],
    /// Coefficients targeted by `set_coefficients`
    target: [f32; COEFF_COUNT],
    /// Crossfade position, 0.0..=1.0
    blend: f32,
    /// Crossfade increment per sample
    inc: f32,
    /// DF2T state
    z1: f32,
    z2: f32,
}

impl Biquad {
    /// Create a transparent biquad (identity transfer function).
    pub fn new(sample_rate: f32) -> Self {
        let inc = (1.0 / (0.01 * sample_rate)).min(1.0); // ~10 ms crossfade
        Self {
            cur: [1.0, 0.0, 0.0, 0.0, 0.0],
            start: [1.0, 0.0, 0.0, 0.0, 0.0],
            target: [1.0, 0.0, 0.0, 0.0, 0.0],
            blend: 1.0,
            inc,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Set new coefficients. The currently effective coefficients become the
    /// starting point, so the transition is always continuous.
    pub fn set_coefficients(&mut self, b0: f32, b1: f32, b2: f32, a1: f32, a2: f32) {
        // Freeze the effective coefficients as the crossfade starting point.
        for i in 0..COEFF_COUNT {
            self.start[i] = self.cur[i];
        }
        self.target = [b0, b1, b2, a1, a2];
        self.blend = 0.0;
    }

    /// Low-pass filter (RBJ cookbook).
    pub fn lowpass(&mut self, freq_hz: f32, q: f32, sample_rate: f32) {
        let (b0, b1, b2, a1, a2) = lowpass_coeffs(freq_hz, q, sample_rate);
        self.set_coefficients(b0, b1, b2, a1, a2);
    }

    /// High-pass filter (RBJ cookbook).
    pub fn highpass(&mut self, freq_hz: f32, q: f32, sample_rate: f32) {
        let (b0, b1, b2, a1, a2) = highpass_coeffs(freq_hz, q, sample_rate);
        self.set_coefficients(b0, b1, b2, a1, a2);
    }

    /// Peaking EQ (RBJ cookbook).
    pub fn peaking(&mut self, freq_hz: f32, gain_db: f32, q: f32, sample_rate: f32) {
        let (b0, b1, b2, a1, a2) = peaking_coeffs(freq_hz, gain_db, q, sample_rate);
        self.set_coefficients(b0, b1, b2, a1, a2);
    }

    /// Low shelf (RBJ cookbook).
    pub fn low_shelf(&mut self, freq_hz: f32, gain_db: f32, sample_rate: f32) {
        let (b0, b1, b2, a1, a2) = shelf_coeffs(freq_hz, gain_db, sample_rate, true);
        self.set_coefficients(b0, b1, b2, a1, a2);
    }

    /// High shelf (RBJ cookbook).
    pub fn high_shelf(&mut self, freq_hz: f32, gain_db: f32, sample_rate: f32) {
        let (b0, b1, b2, a1, a2) = shelf_coeffs(freq_hz, gain_db, sample_rate, false);
        self.set_coefficients(b0, b1, b2, a1, a2);
    }

    /// Process one sample.
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        if self.blend < 1.0 {
            for i in 0..COEFF_COUNT {
                self.cur[i] = self.start[i] + (self.target[i] - self.start[i]) * self.blend;
            }
            self.blend = (self.blend + self.inc).min(1.0);
        }
        let [b0, b1, b2, a1, a2] = self.cur;
        let y = b0 * x + self.z1;
        self.z1 = b1 * x - a1 * y + self.z2;
        self.z2 = b2 * x - a2 * y;
        y
    }

    /// Process a block of samples.
    #[inline]
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        for (x, y) in input.iter().zip(output.iter_mut()) {
            *y = self.process(*x);
        }
    }

    /// Reset filter state (keeps coefficients).
    pub fn reset_state(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
        self.blend = 1.0;
        self.cur = self.target;
        self.start = self.target;
    }
}

#[inline]
fn clamp_freq(freq: f32, sample_rate: f32) -> f32 {
    freq.max(10.0).min(sample_rate * 0.45)
}

pub fn lowpass_coeffs(freq_hz: f32, q: f32, sample_rate: f32) -> (f32, f32, f32, f32, f32) {
    let f = clamp_freq(freq_hz, sample_rate);
    let w0 = 2.0 * std::f32::consts::PI * f / sample_rate;
    let (cos_w0, sin_w0) = (w0.cos(), w0.sin());
    let alpha = sin_w0 / (2.0 * q.max(0.01));
    let a0 = 1.0 + alpha;
    let b0 = ((1.0 - cos_w0) * 0.5) / a0;
    let b1 = (1.0 - cos_w0) / a0;
    let b2 = b0;
    let a1 = (-2.0 * cos_w0) / a0;
    let a2 = (1.0 - alpha) / a0;
    (b0, b1, b2, a1, a2)
}

pub fn highpass_coeffs(freq_hz: f32, q: f32, sample_rate: f32) -> (f32, f32, f32, f32, f32) {
    let f = clamp_freq(freq_hz, sample_rate);
    let w0 = 2.0 * std::f32::consts::PI * f / sample_rate;
    let (cos_w0, sin_w0) = (w0.cos(), w0.sin());
    let alpha = sin_w0 / (2.0 * q.max(0.01));
    let a0 = 1.0 + alpha;
    let b0 = ((1.0 + cos_w0) * 0.5) / a0;
    let b1 = -(1.0 + cos_w0) / a0;
    let b2 = b0;
    let a1 = (-2.0 * cos_w0) / a0;
    let a2 = (1.0 - alpha) / a0;
    (b0, b1, b2, a1, a2)
}

pub fn peaking_coeffs(
    freq_hz: f32,
    gain_db: f32,
    q: f32,
    sample_rate: f32,
) -> (f32, f32, f32, f32, f32) {
    let f = clamp_freq(freq_hz, sample_rate);
    let w0 = 2.0 * std::f32::consts::PI * f / sample_rate;
    let (cos_w0, sin_w0) = (w0.cos(), w0.sin());
    let alpha = sin_w0 / (2.0 * q.max(0.01));
    let a = 10.0_f32.powf(gain_db / 40.0);
    let a0 = 1.0 + alpha / a;
    let b0 = (1.0 + alpha * a) / a0;
    let b1 = (-2.0 * cos_w0) / a0;
    let b2 = (1.0 - alpha * a) / a0;
    let a1 = (-2.0 * cos_w0) / a0;
    let a2 = (1.0 - alpha / a) / a0;
    (b0, b1, b2, a1, a2)
}

pub fn shelf_coeffs(
    freq_hz: f32,
    gain_db: f32,
    sample_rate: f32,
    low: bool,
) -> (f32, f32, f32, f32, f32) {
    let f = clamp_freq(freq_hz, sample_rate);
    let w0 = 2.0 * std::f32::consts::PI * f / sample_rate;
    let (cos_w0, sin_w0) = (w0.cos(), w0.sin());
    let alpha = sin_w0 / 2.0_f32.sqrt();
    let a = 10.0_f32.powf(gain_db / 40.0);
    let sqrt_a = 10.0_f32.powf(gain_db / 80.0);
    let two_sqrt_a_alpha = 2.0 * sqrt_a * alpha;
    let (b0, b1, b2, a0, a1, a2) = if low {
        (
            a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0),
            a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha),
            (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0),
            (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha,
        )
    } else {
        (
            a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0),
            a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha),
            (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha,
            2.0 * ((a - 1.0) - (a + 1.0) * cos_w0),
            (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha,
        )
    };
    (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_transparent() {
        let mut b = Biquad::new(48000.0);
        for x in [-1.0, -0.5, 0.25, 0.9] {
            assert!((b.process(x) - x).abs() < 1e-6);
        }
    }

    #[test]
    fn lowpass_attenuates_high_frequency() {
        let measure = |freq: f32| {
            let mut b = Biquad::new(48000.0);
            b.lowpass(1000.0, 0.707, 48000.0);
            b.reset_state();
            let mut peak: f32 = 0.0;
            for i in 0..4000 {
                let t = i as f32 / 48000.0;
                let x = (2.0 * std::f32::consts::PI * freq * t).sin();
                // Ignore the transient of the first 1000 samples.
                let y = b.process(x);
                if i > 1000 {
                    peak = peak.max(y.abs());
                }
            }
            peak
        };
        let low = measure(200.0);
        let high = measure(10000.0);
        assert!(low > 0.8, "low frequency was not passed: {low}");
        assert!(high < low * 0.05, "low={low}, high={high}");
    }

    #[test]
    fn coefficient_change_does_not_reset_state() {
        let mut b = Biquad::new(48000.0);
        b.lowpass(1000.0, 0.707, 48000.0);
        for _ in 0..100 {
            let _ = b.process(1.0);
        }
        let before = b.z1;
        b.lowpass(4000.0, 0.707, 48000.0);
        assert_eq!(b.z1, before);
    }
}
