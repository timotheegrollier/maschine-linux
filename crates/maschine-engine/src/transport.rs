//! Transport — horloge, BPM, play/rec/loop.

use serde::{Deserialize, Serialize};

/// État transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TransportState {
    /// Arrêté
    #[default]
    Stopped,
    /// Play
    Playing,
    /// Enregistrement
    Recording,
}

/// Transport — maître du temps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transport {
    /// BPM
    pub bpm: f32,
    /// Sample rate (pour conversion tick <-> samples)
    pub sample_rate: u32,
    /// PPQ
    pub ppq: u32,
    /// État
    pub state: TransportState,
    /// Position ticks globale (timeline Song)
    pub tick: u64,
    /// Loop activé
    pub loop_enabled: bool,
    /// Début loop ticks
    pub loop_start: u64,
    /// Fin loop ticks
    pub loop_end: u64,
    /// Swing 0..1 (retard des doubles croches paires)
    pub swing: f32,
    /// Metronome
    pub metronome: bool,
    /// Count-in bars
    pub count_in_bars: u8,
    /// Fraction de tick résiduelle (accumulateur sub-sample pour précision exacte)
    #[serde(default)]
    pub tick_fraction: f64,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            sample_rate: 48000,
            ppq: 960,
            state: TransportState::Stopped,
            tick: 0,
            loop_enabled: false,
            loop_start: 0,
            loop_end: 4 * 960,
            swing: 0.0,
            metronome: false,
            count_in_bars: 0,
            tick_fraction: 0.0,
        }
    }
}

impl Transport {
    /// Nouveau.
    #[must_use]
    pub fn new(bpm: f32, sample_rate: u32) -> Self {
        Self {
            bpm,
            sample_rate,
            ..Default::default()
        }
    }

    /// Tick → secondes.
    #[must_use]
    pub fn tick_to_secs(&self, tick: u64) -> f64 {
        let beats = tick as f64 / self.ppq as f64;
        beats * 60.0 / self.bpm as f64
    }

    /// Secondes → tick.
    #[must_use]
    pub fn secs_to_tick(&self, secs: f64) -> u64 {
        let beats = secs * self.bpm as f64 / 60.0;
        (beats * self.ppq as f64) as u64
    }

    /// Tick → samples.
    #[must_use]
    pub fn tick_to_samples(&self, tick: u64) -> u64 {
        (self.tick_to_secs(tick) * self.sample_rate as f64) as u64
    }

    /// Samples → tick.
    #[must_use]
    pub fn samples_to_tick(&self, samples: u64) -> u64 {
        self.secs_to_tick(samples as f64 / self.sample_rate as f64)
    }

    /// Avance de `samples` — gère loop avec accumulateur haute précision (0 drift).
    pub fn advance_samples(&mut self, samples: u64) {
        if self.state == TransportState::Stopped || self.sample_rate == 0 {
            return;
        }
        let delta_ticks = (samples as f64 / self.sample_rate as f64) * (self.bpm as f64 / 60.0) * (self.ppq as f64);
        self.tick_fraction += delta_ticks;
        let whole_ticks = self.tick_fraction.floor() as u64;
        self.tick_fraction -= whole_ticks as f64;
        self.tick += whole_ticks;

        if self.loop_enabled && self.tick >= self.loop_end {
            let loop_len = (self.loop_end - self.loop_start).max(1);
            self.tick = self.loop_start + ((self.tick - self.loop_end) % loop_len);
        }
    }

    /// Swing : retarde les 16e paires de `swing * 0.5 * step_ticks`.
    #[must_use]
    pub fn swing_offset(&self, tick: u64) -> i32 {
        if self.swing <= 0.0 {
            return 0;
        }
        // 16e = ppq/4 = 240 ticks
        let step = self.ppq / 4;
        let pos_in_bar = tick % (4 * 4 * step as u64); // 16 steps par 4/4
        let step_idx = (pos_in_bar / step as u64) % 16;
        if step_idx % 2 == 1 {
            // paire -> retard
            (self.swing * 0.5 * step as f32) as i32
        } else {
            0
        }
    }

    /// Position en bars:beats:ticks.
    #[must_use]
    pub fn position_bbt(&self) -> crate::project::BarsBeatsTicks {
        crate::project::BarsBeatsTicks::from_ticks(self.tick as u32)
    }

    /// Play.
    pub fn play(&mut self) {
        self.state = TransportState::Playing;
    }

    /// Stop.
    pub fn stop(&mut self) {
        self.state = TransportState::Stopped;
        self.tick_fraction = 0.0;
    }

    /// Record.
    pub fn record(&mut self) {
        self.state = TransportState::Recording;
    }

    /// Toggle play/stop.
    pub fn toggle_play(&mut self) {
        match self.state {
            TransportState::Playing | TransportState::Recording => self.stop(),
            TransportState::Stopped => self.play(),
        }
    }

    /// Est en lecture ?
    #[must_use]
    pub fn is_playing(&self) -> bool {
        matches!(
            self.state,
            TransportState::Playing | TransportState::Recording
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_secs_roundtrip() {
        let t = Transport::new(120.0, 48000);
        let tick = 960; // 1 beat
        let secs = t.tick_to_secs(tick);
        assert!((secs - 0.5).abs() < 1e-6);
        let tick2 = t.secs_to_tick(secs);
        assert_eq!(tick, tick2);
    }

    #[test]
    fn swing() {
        let mut t = Transport::new(120.0, 48000);
        t.swing = 0.6;
        // step 0 pair -> 0, step 1 impair -> retard
        assert_eq!(t.swing_offset(0), 0);
        assert!(t.swing_offset(240) > 0);
        assert_eq!(t.swing_offset(480), 0);
    }

    #[test]
    fn loop_wrap() {
        let mut t = Transport::new(120.0, 48000);
        t.loop_enabled = true;
        t.loop_start = 0;
        t.loop_end = 4 * 960;
        t.state = TransportState::Playing;
        t.tick = 4 * 960 - 10;
        t.advance_samples(t.tick_to_samples(20));
        assert!(t.tick < 4 * 960);
    }
}
