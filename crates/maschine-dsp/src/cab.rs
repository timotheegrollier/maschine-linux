//! Cabinet simulation using IIR filters (lightweight, no IR convolution)

/// Biquad filter (Direct Form I)
#[derive(Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn new() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Lowpass filter
    fn lowpass(&mut self, freq_hz: f32, q: f32, sample_rate: f32) {
        let w0 = 2.0 * std::f32::consts::PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 - cos_w0) / 2.0;
        let b1 = 1.0 - cos_w0;
        let b2 = (1.0 - cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    /// Peaking EQ filter
    fn peaking_eq(&mut self, freq_hz: f32, gain_db: f32, q: f32, sample_rate: f32) {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let out = self.b0 * input + self.b1 * self.z1 + self.b2 * self.z2
            - self.a1 * self.z1
            - self.a2 * self.z2;
        self.z2 = self.z1;
        self.z1 = input;
        out
    }
}

/// Cab sim: high-cut lowpass + resonant peak for "speaker" character
pub struct CabSim {
    highcut: Biquad,
    resonance: Biquad,
    sample_rate: f32,
    enabled: bool,
}

impl CabSim {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            highcut: Biquad::new(),
            resonance: Biquad::new(),
            sample_rate,
            enabled: true,
        }
    }

    /// Update filter parameters
    pub fn set_params(&mut self, highcut_hz: f32, resonance_q: f32) {
        // High-cut lowpass (simulates speaker rolloff)
        self.highcut.lowpass(highcut_hz, 0.707, self.sample_rate);

        // Resonant peak around 2.5kHz (speaker cone resonance)
        self.resonance
            .peaking_eq(2500.0, 3.0, resonance_q, self.sample_rate);
    }

    /// Enable/disable cab sim
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Process one sample
    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }
        let x = self.highcut.process(input);
        self.resonance.process(x)
    }

    /// Process block (mono)
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        if !self.enabled {
            output.copy_from_slice(input);
            return;
        }
        for (in_sample, out_sample) in input.iter().zip(output.iter_mut()) {
            *out_sample = self.process_sample(*in_sample);
        }
    }
}
