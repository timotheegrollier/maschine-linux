//! Real-time analysis tools: pitch detection (tuner), spectrum, clipping.
//!
//! All state is preallocated. No allocation or locking happens while
//! processing audio samples.

#[cfg(feature = "analysis")]
use crate::utils::lin_to_db;

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Result of a pitch estimate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TunerReading {
    pub frequency: f32,
    /// Note name without octave, e.g. "A".
    pub note: &'static str,
    pub octave: i32,
    /// Deviation from the equal-tempered note, in cents (-50..=50).
    pub cents: f32,
    /// Confidence of the estimate (0..1, higher is better).
    pub confidence: f32,
}

impl TunerReading {
    /// Human-readable label such as `A4 +3c`.
    pub fn label(&self) -> String {
        let sign = if self.cents >= 0.0 { '+' } else { '-' };
        format!(
            "{}{} {}{:.0}c",
            self.note,
            self.octave,
            sign,
            self.cents.abs()
        )
    }
}

/// Convert a frequency in Hz to a note name, octave and cent deviation.
pub fn frequency_to_note(frequency: f32) -> (&'static str, i32, f32) {
    let midi = 69.0 + 12.0 * (frequency / 440.0).log2();
    let nearest = midi.round() as i32;
    let cents = (midi - nearest as f32) * 100.0;
    let note = NOTE_NAMES[(nearest.rem_euclid(12)) as usize];
    let octave = nearest.div_euclid(12) - 1;
    (note, octave, cents)
}

/// Monophonic pitch tracker using the YIN algorithm.
///
/// Feed samples with `process`, then call `estimate` at the GUI refresh
/// rate (e.g. every 30–60 ms).
pub struct Tuner {
    sample_rate: f32,
    size: usize,
    buffer: Vec<f32>,
    scratch: Vec<f32>,
    write_pos: usize,
    filled: usize,
    diff: Vec<f32>,
}

impl Tuner {
    /// Create a tuner with a window of roughly `window_ms` milliseconds.
    pub fn new(sample_rate: f32, window_ms: f32) -> Self {
        let samples = (sample_rate * window_ms * 0.001) as usize;
        let size = samples.max(1024).next_power_of_two().min(16384);
        Self {
            sample_rate,
            size,
            buffer: vec![0.0; size],
            scratch: vec![0.0; size],
            write_pos: 0,
            filled: 0,
            diff: vec![0.0; size / 2],
        }
    }

    /// Push one sample into the analysis window.
    #[inline]
    pub fn process(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.size;
        self.filled = (self.filled + 1).min(self.size);
    }

    /// Estimate the current pitch, or `None` if the signal is too quiet or
    /// too noisy to produce a reliable estimate.
    #[allow(clippy::needless_range_loop)]
    pub fn estimate(&mut self) -> Option<TunerReading> {
        if self.filled < self.size / 2 {
            return None;
        }
        let n = self.size;
        // Copy from the ring buffer into a chronological scratch slice.
        for i in 0..n {
            self.scratch[i] = self.buffer[(self.write_pos + i) % n];
        }
        let scratch = &self.scratch;

        let mut rms = 0.0;
        for i in 0..n {
            rms += scratch[i] * scratch[i];
        }
        rms = (rms / n as f32).sqrt();
        if rms < 0.002 {
            return None;
        }

        // YIN difference function, limited to the useful guitar range.
        let min_tau = (self.sample_rate / 1200.0) as usize; // up to ~C6
        let max_tau = ((self.sample_rate / 60.0) as usize).min(n / 2); // down to ~C2
        if max_tau <= min_tau {
            return None;
        }

        let d = &mut self.diff;
        for tau in min_tau..max_tau {
            let mut sum = 0.0;
            for j in 0..(n - tau) {
                let delta = scratch[j] - scratch[j + tau];
                sum += delta * delta;
            }
            d[tau] = sum;
        }

        // Cumulative mean normalized difference.
        let mut running = 0.0;
        let mut best_tau = 0usize;
        let mut best_value = f32::INFINITY;
        for tau in min_tau..max_tau {
            running += d[tau];
            let cmnd = if running > 1e-12 {
                d[tau] * tau as f32 / running
            } else {
                1.0
            };
            d[tau] = cmnd;
            if cmnd < best_value {
                best_value = cmnd;
                best_tau = tau;
            }
        }
        if best_value > 0.25 || best_tau == 0 {
            return None;
        }

        // Parabolic interpolation around the minimum for sub-sample period.
        let left = d[best_tau.saturating_sub(1)];
        let right = d[(best_tau + 1).min(max_tau - 1)];
        let denom = 2.0 * (2.0 * best_value - left - right);
        let offset = if denom.abs() > 1e-9 {
            (right - left) / denom
        } else {
            0.0
        };
        let period = best_tau as f32 + offset.clamp(-1.0, 1.0);
        if period <= 0.0 {
            return None;
        }
        let frequency = self.sample_rate / period;
        let (note, octave, cents) = frequency_to_note(frequency);
        Some(TunerReading {
            frequency,
            note,
            octave,
            cents,
            confidence: (1.0 - best_value / 0.25).clamp(0.0, 1.0),
        })
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
        self.filled = 0;
    }

    pub fn window_seconds(&self) -> f32 {
        self.size as f32 / self.sample_rate
    }
}

#[cfg(feature = "analysis")]
mod spectrum_impl {
    use super::*;
    use realfft::num_complex::Complex;
    use realfft::RealFftPlanner;

    /// FFT-based spectrum analyser with Hann window and temporal smoothing.
    pub struct SpectrumAnalyser {
        size: usize,
        input: Vec<f32>,
        freq: Vec<Complex<f32>>,
        window: Vec<f32>,
        magnitudes: Vec<f32>,
        scratch: Vec<f32>,
        write_pos: usize,
        filled: usize,
        planner: RealFftPlanner<f32>,
    }

    impl SpectrumAnalyser {
        /// `size` must be a power of two. 2048 is a good default.
        pub fn new(size: usize) -> Self {
            let size = size.max(64).next_power_of_two();
            let mut planner = RealFftPlanner::<f32>::new();
            let _fft = planner.plan_fft_forward(size);
            let n_freq = size / 2 + 1;
            let mut window = vec![0.0; size];
            for (i, w) in window.iter_mut().enumerate() {
                *w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / size as f32).cos();
            }
            Self {
                size,
                input: vec![0.0; size],
                freq: vec![Complex::new(0.0, 0.0); n_freq],
                window,
                magnitudes: vec![-120.0; n_freq],
                scratch: vec![0.0; size],
                write_pos: 0,
                filled: 0,
                planner,
            }
        }

        /// Push one sample.
        #[inline]
        pub fn process(&mut self, sample: f32) {
            self.input[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % self.size;
            self.filled = (self.filled + 1).min(self.size);
        }

        /// Compute the current spectrum in dBFS. Length is `size / 2 + 1`.
        pub fn analyse(&mut self) -> &[f32] {
            if self.filled < self.size {
                return &self.magnitudes;
            }
            for i in 0..self.size {
                self.scratch[i] = self.input[(self.write_pos + i) % self.size] * self.window[i];
            }
            let fft = self.planner.plan_fft_forward(self.size);
            let norm = 2.0 / self.size as f32;
            if fft.process(&mut self.scratch, &mut self.freq).is_ok() {
                for (bin, value) in self.magnitudes.iter_mut().enumerate() {
                    let mag = self.freq[bin].norm() * norm;
                    let db = lin_to_db(mag);
                    // Decaying peak hold for a stable, readable display.
                    *value = db.max(*value - 1.5);
                }
            }
            &self.magnitudes
        }

        pub fn bins(&self) -> usize {
            self.magnitudes.len()
        }

        pub fn reset(&mut self) {
            self.input.fill(0.0);
            self.scratch.fill(0.0);
            self.magnitudes.fill(-120.0);
            self.write_pos = 0;
            self.filled = 0;
        }
    }
}

#[cfg(feature = "analysis")]
pub use spectrum_impl::SpectrumAnalyser;

/// Simple clipping / overload detection over a block.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClipStatus {
    /// Number of samples that exceeded the threshold in the last block.
    pub clipped_samples: u32,
    /// Peak absolute sample value in the last block.
    pub peak: f32,
    /// Running total of clipped samples.
    pub total_clipped: u64,
}

/// Lightweight per-block clipping detector.
pub struct ClipDetector {
    threshold: f32,
    total: u64,
}

impl ClipDetector {
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold: threshold.max(0.001),
            total: 0,
        }
    }

    pub fn process(&mut self, samples: &[f32]) -> ClipStatus {
        let mut peak = 0.0;
        let mut clipped = 0;
        for &s in samples {
            let a = s.abs();
            if a > peak {
                peak = a;
            }
            if a >= self.threshold {
                clipped += 1;
            }
        }
        self.total += clipped as u64;
        ClipStatus {
            clipped_samples: clipped,
            peak,
            total_clipped: self.total,
        }
    }

    pub fn reset(&mut self) {
        self.total = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_conversion() {
        let (note, octave, cents) = frequency_to_note(440.0);
        assert_eq!(note, "A");
        assert_eq!(octave, 4);
        assert!(cents.abs() < 0.01);
        let (note, octave, _) = frequency_to_note(261.63);
        assert_eq!(note, "C");
        assert_eq!(octave, 4);
    }

    #[test]
    fn tuner_detects_a440() {
        let sr = 48000.0;
        let mut tuner = Tuner::new(sr, 100.0);
        for i in 0..20000 {
            let t = i as f32 / sr;
            tuner.process(0.5 * (2.0 * std::f32::consts::PI * 440.0 * t).sin());
        }
        let reading = tuner.estimate().expect("no pitch detected");
        assert_eq!(reading.note, "A");
        assert_eq!(reading.octave, 4);
        assert!(
            (reading.frequency - 440.0).abs() < 5.0,
            "f={}",
            reading.frequency
        );
    }

    #[test]
    fn tuner_ignores_silence() {
        let mut tuner = Tuner::new(48000.0, 100.0);
        for _ in 0..20000 {
            tuner.process(0.0);
        }
        assert!(tuner.estimate().is_none());
    }
}
