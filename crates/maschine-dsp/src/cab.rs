//! Guitar cabinet simulation using speaker-response filtering.
//!
//! The speaker is modeled as a band-limited chain of resonant peaks
//! (cone breakup, basket resonance) followed by a high-cut. An optional
//! convolution mode with real impulse responses can be enabled with the
//! `ir` feature.

use crate::biquad::Biquad;

/// Cabinet voicing presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CabModel {
    /// Vox AC30 — bright, chimey, aggressive cone breakup.
    Vox,
    /// Fender Blackface — warm, rounded, smooth top end.
    Fender,
    /// Modern high-gain — tight and focused.
    Modern,
    /// Flat — no coloration, only the high-cut.
    Flat,
}

impl CabModel {
    pub const ALL: [CabModel; 4] = [
        CabModel::Vox,
        CabModel::Fender,
        CabModel::Modern,
        CabModel::Flat,
    ];
    pub fn name(self) -> &'static str {
        match self {
            CabModel::Vox => "Vox 12\"",
            CabModel::Fender => "Fender 12\"",
            CabModel::Modern => "Modern 12\"",
            CabModel::Flat => "Flat",
        }
    }
    pub fn from_index(i: usize) -> Self {
        Self::ALL[i % Self::ALL.len()]
    }
}

/// Cabinet simulator: resonant speaker peaks + high-cut.
pub struct CabSim {
    /// Coupe-haut du HP : deux biquads = Butterworth 4ᵉ ordre (24 dB/oct),
    /// la pente d'un vrai baffle ; à 12 dB/oct la distorsion « grésille ».
    highcut: Biquad,
    highcut2: Biquad,
    lowcut: Biquad,
    resonance: Biquad,
    presence: Biquad,
    sample_rate: f32,
    enabled: bool,
    model: CabModel,
}

impl CabSim {
    pub fn new(sample_rate: f32) -> Self {
        let mut sim = Self {
            highcut: Biquad::new(sample_rate),
            highcut2: Biquad::new(sample_rate),
            lowcut: Biquad::new(sample_rate),
            resonance: Biquad::new(sample_rate),
            presence: Biquad::new(sample_rate),
            sample_rate,
            enabled: true,
            model: CabModel::Vox,
        };
        sim.configure();
        sim
    }

    pub fn model(&self) -> CabModel {
        self.model
    }

    pub fn set_model(&mut self, model: CabModel) {
        if model != self.model {
            self.model = model;
            self.configure();
        }
    }

    /// Update the high-cut and resonance controls.
    ///
    /// `highcut_hz` is the speaker rolloff, `resonance_q` controls the
    /// height of the cone peak (0.5 = gentle, 5.0 = aggressive).
    pub fn set_params(&mut self, highcut_hz: f32, resonance_q: f32) {
        let peak_freq = match self.model {
            CabModel::Vox => 2400.0,
            CabModel::Fender => 1900.0,
            CabModel::Modern => 2900.0,
            CabModel::Flat => 2500.0,
        };
        let peak_db = match self.model {
            CabModel::Vox => 3.0 + resonance_q * 0.4,
            CabModel::Fender => 1.5 + resonance_q * 0.3,
            CabModel::Modern => 2.0 + resonance_q * 0.3,
            CabModel::Flat => resonance_q * 0.2,
        };
        self.highcut.lowpass(highcut_hz, 0.541, self.sample_rate);
        self.highcut2.lowpass(highcut_hz, 1.307, self.sample_rate);
        self.resonance
            .peaking(peak_freq, peak_db, 1.2, self.sample_rate);
    }

    fn configure(&mut self) {
        let lowcut_hz = match self.model {
            CabModel::Vox => 90.0,
            CabModel::Fender => 75.0,
            CabModel::Modern => 55.0,
            CabModel::Flat => 20.0,
        };
        self.lowcut.highpass(lowcut_hz, 0.707, self.sample_rate);
        self.presence.peaking(
            match self.model {
                CabModel::Vox => 5200.0,
                CabModel::Fender => 4000.0,
                CabModel::Modern => 6500.0,
                CabModel::Flat => 6000.0,
            },
            match self.model {
                CabModel::Vox => 2.0,
                CabModel::Fender => -1.0,
                CabModel::Modern => 1.0,
                CabModel::Flat => 0.0,
            },
            0.8,
            self.sample_rate,
        );
        self.set_params(5000.0, 1.5);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }
        let x = self.lowcut.process(input);
        let x = self.resonance.process(x);
        let x = self.presence.process(x);
        self.highcut2.process(self.highcut.process(x))
    }

    #[inline]
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        if !self.enabled {
            output.copy_from_slice(input);
            return;
        }
        for (x, y) in input.iter().zip(output.iter_mut()) {
            *y = self.process_sample(*x);
        }
    }

    pub fn reset(&mut self) {
        self.highcut.reset_state();
        self.highcut2.reset_state();
        self.lowcut.reset_state();
        self.resonance.reset_state();
        self.presence.reset_state();
    }
}
