//! Overdrive / Distortion using tanh waveshaping with pre/post EQ

/// Overdrive processor with gain, tone (lowpass), and mix
pub struct Overdrive {
    /// Pre-emphasis highpass to tighten low end before distortion
    pre_hp_z1: f32,
    /// Post lowpass for tone control
    post_lp_z1: f32,
    sample_rate: f32,
    tone_coeff: f32,
    pre_hp_coeff: f32,
}

impl Overdrive {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            pre_hp_z1: 0.0,
            post_lp_z1: 0.0,
            sample_rate,
            tone_coeff: 0.0,
            pre_hp_coeff: 0.0,
        }
    }

    /// Update tone coefficient (cutoff frequency for post lowpass)
    pub fn set_tone(&mut self, cutoff_hz: f32) {
        // One-pole lowpass
        self.tone_coeff = 1.0 - (-2.0 * std::f32::consts::PI * cutoff_hz / self.sample_rate).exp();
        // Pre-emphasis highpass at ~80Hz
        self.pre_hp_coeff = (-2.0 * std::f32::consts::PI * 80.0 / self.sample_rate).exp();
    }

    /// Process one sample with tanh waveshaping
    #[inline]
    pub fn process_sample(&mut self, input: f32, gain_db: f32, mix: f32) -> f32 {
        let gain = 10.0_f32.powf(gain_db * 0.05); // dB to linear

        // Pre-emphasis highpass
        let hp_out = input - self.pre_hp_z1 + self.pre_hp_coeff * self.pre_hp_z1;
        self.pre_hp_z1 = input;

        // Drive stage
        let driven = (hp_out * gain).tanh();

        // Post lowpass (tone)
        let lp_out = driven * self.tone_coeff + self.post_lp_z1 * (1.0 - self.tone_coeff);
        self.post_lp_z1 = lp_out;

        // Dry/wet mix
        input * (1.0 - mix) + lp_out * mix
    }

    /// Process block (mono)
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32], gain_db: f32, mix: f32) {
        for (in_sample, out_sample) in input.iter().zip(output.iter_mut()) {
            *out_sample = self.process_sample(*in_sample, gain_db, mix);
        }
    }
}