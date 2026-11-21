//! 3-band tone stack built on the click-free biquad filters.

use crate::biquad::Biquad;

/// 3-band tone control (Bass, Mid, Treble) with independent Q settings.
pub struct ToneStack {
    bass: Biquad,
    mid: Biquad,
    treble: Biquad,
    sample_rate: f32,
}

impl ToneStack {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            bass: Biquad::new(sample_rate),
            mid: Biquad::new(sample_rate),
            treble: Biquad::new(sample_rate),
            sample_rate,
        }
    }

    /// Update all three bands from dB gains.
    pub fn set_gains(&mut self, bass_db: f32, mid_db: f32, treble_db: f32) {
        self.bass.low_shelf(200.0, bass_db, self.sample_rate);
        self.mid.peaking(800.0, mid_db, 0.9, self.sample_rate);
        self.treble.high_shelf(3000.0, treble_db, self.sample_rate);
    }

    /// Process one sample through all three bands.
    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let x = self.bass.process(input);
        let x = self.mid.process(x);
        self.treble.process(x)
    }

    /// Process block (mono).
    #[inline]
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        for (x, y) in input.iter().zip(output.iter_mut()) {
            *y = self.process_sample(*x);
        }
    }

    /// Reset filter state.
    pub fn reset(&mut self) {
        self.bass.reset_state();
        self.mid.reset_state();
        self.treble.reset_state();
    }
}
