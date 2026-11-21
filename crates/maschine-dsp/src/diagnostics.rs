//! Real-time diagnostics: CPU load, block timing, clipping and latency.
//!
//! These helpers are allocation-free and safe to use from the audio thread.
//! The GUI reads an immutable `RtStats` snapshot published by the audio
//! thread through an atomic or a lock-free channel.

use std::time::Instant;

use crate::analysis::ClipStatus;

/// Immutable snapshot of audio-thread statistics, suitable for display.
#[derive(Debug, Clone, Copy, Default)]
pub struct RtStats {
    /// Smoothed fraction of the available block budget used by DSP (0..1).
    pub cpu_load: f32,
    /// Average time spent in the audio callback, in microseconds.
    pub avg_block_us: f32,
    /// Worst observed callback time, in microseconds.
    pub max_block_us: f32,
    /// Estimated end-to-end latency in milliseconds.
    pub latency_ms: f32,
    /// Peak sample level in the most recent block.
    pub peak: f32,
    /// Clipped samples in the most recent block.
    pub clipped_samples: u32,
    /// Cumulative clipped samples.
    pub total_clipped: u64,
    /// Number of processed blocks.
    pub blocks: u64,
    /// Number of reported buffer under/overruns.
    pub xruns: u32,
}

/// Collector running on the audio thread.
pub struct RtDiagnostics {
    sample_rate: f32,
    block_size: usize,
    cpu_ema: f32,
    max_us: f32,
    sum_us: f64,
    blocks: u64,
    latency_ms: f32,
    stats: RtStats,
}

impl RtDiagnostics {
    pub fn new(sample_rate: f32, block_size: usize) -> Self {
        Self {
            sample_rate,
            block_size: block_size.max(1),
            cpu_ema: 0.0,
            max_us: 0.0,
            sum_us: 0.0,
            blocks: 0,
            latency_ms: 0.0,
            stats: RtStats::default(),
        }
    }

    /// Mark the beginning of a callback. Pair with `end_block`.
    #[inline]
    pub fn begin_block(&self) -> Instant {
        Instant::now()
    }

    /// Finish timing a callback and fold in clipping information.
    #[inline]
    pub fn end_block(&mut self, start: Instant, clip: ClipStatus) {
        let elapsed_us = start.elapsed().as_secs_f64() * 1_000_000.0;
        let budget_us = self.block_size as f64 * 1_000_000.0 / self.sample_rate as f64;
        let load = (elapsed_us / budget_us) as f32;

        // Slow EMA (~1 s at 48 kHz / 128 frames) so the display is readable.
        const EMA: f32 = 0.02;
        self.cpu_ema += (load - self.cpu_ema) * EMA;
        if elapsed_us as f32 > self.max_us {
            self.max_us = elapsed_us as f32;
        }
        self.sum_us += elapsed_us;
        self.blocks += 1;

        self.stats.cpu_load = self.cpu_ema.clamp(0.0, 4.0);
        self.stats.avg_block_us = (self.sum_us / self.blocks as f64) as f32;
        self.stats.max_block_us = self.max_us;
        self.stats.blocks = self.blocks;
        self.stats.peak = clip.peak;
        self.stats.clipped_samples = clip.clipped_samples;
        self.stats.total_clipped = clip.total_clipped;
        self.stats.latency_ms = self.latency_ms;
    }

    /// Set the estimated end-to-end latency (from the audio backend).
    pub fn set_latency_ms(&mut self, latency_ms: f32) {
        self.latency_ms = latency_ms;
        self.stats.latency_ms = latency_ms;
    }

    /// Record a buffer underrun / overrun.
    pub fn report_xrun(&mut self) {
        self.stats.xruns += 1;
    }

    /// Read the current snapshot.
    pub fn snapshot(&self) -> RtStats {
        self.stats
    }

    pub fn reset(&mut self) {
        self.cpu_ema = 0.0;
        self.max_us = 0.0;
        self.sum_us = 0.0;
        self.blocks = 0;
        self.stats = RtStats {
            latency_ms: self.latency_ms,
            ..RtStats::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_start_empty() {
        let d = RtDiagnostics::new(48000.0, 128);
        let s = d.snapshot();
        assert_eq!(s.blocks, 0);
        assert_eq!(s.cpu_load, 0.0);
    }

    #[test]
    fn records_clipping_and_timing() {
        let mut d = RtDiagnostics::new(48000.0, 128);
        let start = d.begin_block();
        let clip = ClipStatus {
            clipped_samples: 3,
            peak: 1.2,
            total_clipped: 3,
        };
        d.end_block(start, clip);
        let s = d.snapshot();
        assert_eq!(s.blocks, 1);
        assert_eq!(s.clipped_samples, 3);
        assert!((s.peak - 1.2).abs() < 1e-6);
        assert!(s.avg_block_us >= 0.0);
    }
}
