//! 3-band Baxandall-style tone stack using simple IIR filters

/// One-pole lowpass filter
#[derive(Clone, Copy)]
struct OnePoleLP {
    z1: f32,
    coeff: f32,
}

impl OnePoleLP {
    fn new() -> Self {
        Self { z1: 0.0, coeff: 0.0 }
    }
    fn set_cutoff(&mut self, freq_hz: f32, sample_rate: f32) {
        self.coeff = 1.0 - (-2.0 * std::f32::consts::PI * freq_hz / sample_rate).exp();
    }
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let out = input * self.coeff + self.z1 * (1.0 - self.coeff);
        self.z1 = out;
        out
    }
}

/// Biquad filter (Direct Form I)
#[derive(Clone, Copy)]
struct Biquad {
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    z1: f32, z2: f32,
}

impl Biquad {
    fn new() -> Self {
        Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, z1: 0.0, z2: 0.0 }
    }
    
    /// Low shelf filter
    fn low_shelf(&mut self, freq_hz: f32, gain_db: f32, sample_rate: f32) {
        let a = 10.0_f32.powf(gain_db / 40.0); // sqrt(10^(gain/20))
        let w0 = 2.0 * std::f32::consts::PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / 2.0; // Q = 1/sqrt(2) for shelf
        
        let a1 = 2.0 * ((a - 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * alpha * a.sqrt();
        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * alpha * a.sqrt());
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * alpha * a.sqrt());
        let norm = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * alpha * a.sqrt();
        
        self.b0 = b0 / norm;
        self.b1 = b1 / norm;
        self.b2 = b2 / norm;
        self.a1 = a1 / norm;
        self.a2 = a2 / norm;
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    /// High shelf filter
    fn high_shelf(&mut self, freq_hz: f32, gain_db: f32, sample_rate: f32) {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / 2.0;
        
        let a1 = -2.0 * ((a - 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * alpha * a.sqrt();
        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * alpha * a.sqrt());
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * alpha * a.sqrt());
        let norm = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * alpha * a.sqrt();
        
        self.b0 = b0 / norm;
        self.b1 = b1 / norm;
        self.b2 = b2 / norm;
        self.a1 = a1 / norm;
        self.a2 = a2 / norm;
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
            - self.a1 * self.z1 - self.a2 * self.z2;
        self.z2 = self.z1;
        self.z1 = input;
        out
    }
}

/// 3-band tone control (Bass, Mid, Treble)
pub struct ToneStack {
    bass: Biquad,
    mid: Biquad,
    treble: Biquad,
    sample_rate: f32,
}

impl ToneStack {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            bass: Biquad::new(),
            mid: Biquad::new(),
            treble: Biquad::new(),
            sample_rate,
        }
    }

    /// Update all three bands from dB gains
    pub fn set_gains(&mut self, bass_db: f32, mid_db: f32, treble_db: f32) {
        self.bass.low_shelf(200.0, bass_db, self.sample_rate);
        self.mid.peaking_eq(800.0, mid_db, 1.0, self.sample_rate);
        self.treble.high_shelf(3000.0, treble_db, self.sample_rate);
    }

    /// Process one sample through all three bands
    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let x = self.bass.process(input);
        let x = self.mid.process(x);
        self.treble.process(x)
    }

    /// Process block (mono)
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        for (in_sample, out_sample) in input.iter().zip(output.iter_mut()) {
            *out_sample = self.process_sample(*in_sample);
        }
    }
}