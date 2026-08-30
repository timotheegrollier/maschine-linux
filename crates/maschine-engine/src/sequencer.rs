//! Sequencer — quantize, nudge, swing, step.

use serde::{Deserialize, Serialize};

use crate::project::{Pattern, PatternEvent};

/// Grille de quantification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Grid {
    /// 1/4
    Quarter,
    /// 1/8
    Eighth,
    /// 1/8T
    EighthTriplet,
    /// 1/16
    #[default]
    Sixteenth,
    /// 1/16T
    SixteenthTriplet,
    /// 1/32
    ThirtySecond,
    /// Off (pas de quantize)
    Off,
}

impl Grid {
    /// Ticks pour la grille à PPQ 960.
    #[must_use]
    pub fn ticks(self) -> u32 {
        match self {
            Self::Quarter => 960,
            Self::Eighth => 480,
            Self::EighthTriplet => 320,
            Self::Sixteenth => 240,
            Self::SixteenthTriplet => 160,
            Self::ThirtySecond => 120,
            Self::Off => 1,
        }
    }

    /// Quantize un tick.
    #[must_use]
    pub fn quantize(self, tick: u32) -> u32 {
        if self == Self::Off {
            return tick;
        }
        let step = self.ticks();
        ((tick + step / 2) / step) * step
    }
}

/// Sequencer — opère sur Patterns.
#[derive(Debug, Default)]
pub struct Sequencer {
    /// Grille courante
    pub grid: Grid,
    /// Swing 0..1
    pub swing: f32,
}

impl Sequencer {
    /// Nouveau.
    #[must_use]
    pub fn new(grid: Grid, swing: f32) -> Self {
        Self { grid, swing }
    }

    /// Quantize tous les events d'un pattern.
    pub fn quantize_pattern(&self, pat: &mut Pattern) {
        for ev in &mut pat.events {
            ev.tick = self.grid.quantize(ev.tick);
        }
        pat.events.sort_by_key(|e| e.tick);
    }

    /// Nudge (décale) un event.
    #[must_use]
    pub fn nudge(&self, tick: u32, delta: i32) -> u32 {
        tick.saturating_add_signed(delta)
    }

    /// Applique le swing aux events (retard des 16e paires).
    pub fn apply_swing(&self, events: &mut [PatternEvent]) {
        if self.swing <= 0.0 {
            return;
        }
        let step = 240; // 16e
        for ev in events.iter_mut() {
            let step_idx = (ev.tick / step) % 16;
            if step_idx % 2 == 1 {
                let offset = (self.swing * 0.5 * step as f32) as u32;
                ev.tick += offset;
            }
        }
        events.sort_by_key(|e| e.tick);
    }

    /// Génère un pattern 16-step à partir de slices (pour slicing).
    #[must_use]
    pub fn pattern_from_slices(&self, num_slices: usize, length_ticks: u32) -> Pattern {
        let mut pat = Pattern::new(
            "Sliced",
            crate::project::BarsBeatsTicks::from_ticks(length_ticks),
        );
        let step = length_ticks / num_slices.max(1) as u32;
        for i in 0..num_slices {
            pat.events.push(PatternEvent {
                pad: (i % 16) as u8,
                tick: i as u32 * step,
                velocity: 100,
                length_ticks: step / 2,
                pitch: 36 + (i % 16) as u8,
            });
        }
        pat
    }

    /// Humanize : variation légère vélocité/timing.
    pub fn humanize(&self, events: &mut [PatternEvent], vel_range: u8, tick_range: i32) {
        // déterministe pour tests — utilise un LCG simple
        let mut seed = 0x12345678u32;
        for ev in events.iter_mut() {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let vel_delta = (seed % (vel_range as u32 * 2 + 1)) as i16 - vel_range as i16;
            ev.velocity = (ev.velocity as i16 + vel_delta).clamp(1, 127) as u8;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let tick_delta = (seed % (tick_range as u32 * 2 + 1)) as i32 - tick_range;
            ev.tick = ev.tick.saturating_add_signed(tick_delta);
        }
        events.sort_by_key(|e| e.tick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_quantize() {
        assert_eq!(Grid::Sixteenth.quantize(100), 0);
        assert_eq!(Grid::Sixteenth.quantize(130), 240);
        assert_eq!(Grid::Quarter.quantize(500), 960);
        assert_eq!(Grid::Off.quantize(123), 123);
    }

    #[test]
    fn swing_offsets() {
        let seq = Sequencer::new(Grid::Sixteenth, 0.5);
        let mut evs = vec![
            PatternEvent {
                pad: 0,
                tick: 0,
                velocity: 100,
                length_ticks: 120,
                pitch: 36,
            },
            PatternEvent {
                pad: 0,
                tick: 240,
                velocity: 100,
                length_ticks: 120,
                pitch: 36,
            },
        ];
        seq.apply_swing(&mut evs);
        assert_eq!(evs[0].tick, 0);
        assert!(evs[1].tick > 240);
    }

    #[test]
    fn slices_pattern() {
        let seq = Sequencer::default();
        let pat = seq.pattern_from_slices(8, 4 * 960);
        assert_eq!(pat.events.len(), 8);
        assert_eq!(pat.events[0].tick, 0);
        assert!(pat.events[7].tick > 0);
    }
}
