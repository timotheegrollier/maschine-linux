//! Noise Gate with smooth attack/release


/// Simple noise gate with lookahead-style envelope follower
pub struct NoiseGate {
    envelope: f32,
    sample_rate: f32,
    // Pre-computed coefficients for attack/release
    attack_coeff: f32,
    release_coeff: f32,
}

impl NoiseGate {
    /// Create new gate at sample rate
    pub fn new(sample_rate: f32) -> Self {
        Self {
            envelope: 0.0,
            sample_rate,
            attack_coeff: 0.0,
            release_coeff: 0.0,
        }
    }

    /// Update coefficients from params (call when params change)
    pub fn update_params(&mut self, attack_ms: f32, release_ms: f32) {
        self.attack_coeff = 1.0 - (-1.0 / (attack_ms * 0.001 * self.sample_rate)).exp();
        self.release_coeff = 1.0 - (-1.0 / (release_ms * 0.001 * self.sample_rate)).exp();
    }

    /// Process one sample
    #[inline]
    pub fn process_sample(&mut self, input: f32, threshold_db: f32) -> f32 {
        let input_abs = input.abs();

        // Envelope follower with separate attack/release
        if input_abs > self.envelope {
            self.envelope += (input_abs - self.envelope) * self.attack_coeff;
        } else {
            self.envelope += (input_abs - self.envelope) * self.release_coeff;
        }

        // Gate threshold in linear scale
        let threshold = 10.0_f32.powf(threshold_db * 0.05); // dB to linear

        // Smooth gate transition
        let gain = if self.envelope > threshold {
            1.0
        } else {
            (self.envelope / threshold.max(1e-10)).clamp(0.0, 1.0)
        };

        input * gain
    }

    /// Process block (mono)
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32], threshold_db: f32) {
        for (in_sample, out_sample) in input.iter().zip(output.iter_mut()) {
            *out_sample = self.process_sample(*in_sample, threshold_db);
        }
    }
}
