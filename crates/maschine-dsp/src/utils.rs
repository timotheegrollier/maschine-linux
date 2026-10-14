//! Utility functions: metering, dB conversion, etc.

/// Convert linear amplitude to dBFS
pub fn lin_to_db(linear: f32) -> f32 {
    20.0 * linear.max(1e-10).log10()
}

/// Convert dBFS to linear amplitude
pub fn db_to_lin(db: f32) -> f32 {
    10.0_f32.powf(db * 0.05)
}

/// Peak meter with attack/release ballistics
pub struct PeakMeter {
    level: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

impl PeakMeter {
    pub fn new(sample_rate: f32) -> Self {
        // Fast attack (1ms), slow release (300ms) for peak meter
        let attack_coeff = 1.0 - (-1.0 / (0.001 * sample_rate)).exp();
        let release_coeff = 1.0 - (-1.0 / (0.3 * sample_rate)).exp();
        Self {
            level: 0.0,
            attack_coeff,
            release_coeff,
        }
    }

    /// Update meter with new sample
    #[inline]
    pub fn update(&mut self, sample: f32) {
        let abs_sample = sample.abs();
        if abs_sample > self.level {
            self.level += (abs_sample - self.level) * self.attack_coeff;
        } else {
            self.level += (abs_sample - self.level) * self.release_coeff;
        }
    }

    /// Get current level in dBFS
    pub fn get_db(&self) -> f32 {
        lin_to_db(self.level)
    }

    /// Get current linear level
    pub fn get_linear(&self) -> f32 {
        self.level
    }

    /// Reset to zero
    pub fn reset(&mut self) {
        self.level = 0.0;
    }
}

/// Stereo peak meter
pub struct StereoMeter {
    left: PeakMeter,
    right: PeakMeter,
}

impl StereoMeter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            left: PeakMeter::new(sample_rate),
            right: PeakMeter::new(sample_rate),
        }
    }

    pub fn update(&mut self, left: f32, right: f32) {
        self.left.update(left);
        self.right.update(right);
    }

    pub fn get_db(&self) -> (f32, f32) {
        (self.left.get_db(), self.right.get_db())
    }
}
