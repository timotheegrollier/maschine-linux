//! Limiteur de sortie du Master.
//!
//! Remplace l'ancien waveshaper (tanh au-delà de 0,7) qui saturait tout ce
//! qui dépassait -3 dBFS : ici le signal est intact sous le plafond
//! (-1 dBFS) et les crêtes sont réduites par un gain lissé, calculé avec
//! 1 ms d'anticipation (le gain a fini de descendre quand la crête sort).
//! Aucune allocation dans `process`.

/// Plafond de sortie (≈ -1 dBFS).
const CEILING: f32 = 0.891;
/// Anticipation (s) : latence ajoutée, durée de l'attaque.
const LOOKAHEAD_S: f32 = 0.001;
/// Retour au gain unité (s).
const RELEASE_S: f32 = 0.12;

/// Limiteur stéréo lié, à anticipation.
#[derive(Debug, Clone)]
pub struct MasterLimiter {
    delay: Vec<[f32; 2]>,
    pos: usize,
    /// File monotone (instant, gain requis) : minimum glissant en O(1).
    queue: Vec<(u64, f32)>,
    head: usize,
    len: usize,
    t: u64,
    gain: f32,
    attack: f32,
    release: f32,
    /// Réduction de gain la plus forte du dernier bloc (dB, ≤ 0).
    reduction_db: f32,
}

impl MasterLimiter {
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(8000) as f32;
        let look = ((sr * LOOKAHEAD_S) as usize).max(1);
        Self {
            delay: vec![[0.0; 2]; look],
            pos: 0,
            queue: vec![(0, 1.0); look + 2],
            head: 0,
            len: 0,
            t: 0,
            gain: 1.0,
            // L'attaque atteint ~98 % de sa cible pendant l'anticipation.
            attack: (-4.0 / look as f32).exp(),
            release: (-1.0 / (RELEASE_S * sr)).exp(),
            reduction_db: 0.0,
        }
    }

    /// Latence ajoutée (trames).
    #[must_use]
    pub fn latency_frames(&self) -> usize {
        self.delay.len()
    }

    /// Réduction de gain maximale du dernier bloc (dB, ≤ 0).
    #[must_use]
    pub fn reduction_db(&self) -> f32 {
        self.reduction_db
    }

    /// Limite un bloc stéréo entrelacé `[L, R, L, R, ...]` en place.
    pub fn process(&mut self, buf: &mut [f32]) {
        let look = self.delay.len() as u64;
        let cap = self.queue.len();
        let mut min_gain = 1.0f32;
        for frame in buf.as_chunks_mut::<2>().0 {
            let (l, r) = (frame[0], frame[1]);
            let peak = l.abs().max(r.abs());
            let need = if peak > CEILING && peak.is_finite() {
                CEILING / peak
            } else {
                1.0
            };
            // Minimum glissant sur la fenêtre [t - look, t].
            while self.len > 0 && self.queue[(self.head + self.len - 1) % cap].1 >= need {
                self.len -= 1;
            }
            self.queue[(self.head + self.len) % cap] = (self.t, need);
            self.len += 1;
            while self.queue[self.head].0 + look < self.t {
                self.head = (self.head + 1) % cap;
                self.len -= 1;
            }
            let target = self.queue[self.head].1;
            let coef = if target < self.gain {
                self.attack
            } else {
                self.release
            };
            self.gain = target + coef * (self.gain - target);
            min_gain = min_gain.min(self.gain);
            self.t += 1;

            let [dl, dr] = self.delay[self.pos];
            self.delay[self.pos] = [l, r];
            self.pos = (self.pos + 1) % self.delay.len();
            // Filet de sécurité : résidu d'attaque ou valeur non finie.
            let safe = |x: f32| {
                if x.is_finite() {
                    x.clamp(-CEILING, CEILING)
                } else {
                    0.0
                }
            };
            frame[0] = safe(dl * self.gain);
            frame[1] = safe(dr * self.gain);
        }
        self.reduction_db = 20.0 * min_gain.max(1e-6).log10();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(amp: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let s = amp * (i as f32 * 0.05).sin();
                [s, s]
            })
            .collect()
    }

    #[test]
    fn quiet_signal_passes_unchanged_after_the_lookahead() {
        let mut lim = MasterLimiter::new(48_000);
        let input = sine(0.5, 4800);
        let mut out = input.clone();
        for block in out.chunks_mut(256) {
            lim.process(block);
        }
        let d = lim.latency_frames() * 2;
        for (o, i) in out[d..].iter().zip(&input) {
            assert!((o - i).abs() < 1e-6);
        }
        assert!(lim.reduction_db() > -0.01);
    }

    #[test]
    fn loud_signal_never_exceeds_the_ceiling_and_recovers() {
        let mut lim = MasterLimiter::new(48_000);
        let mut loud = sine(4.0, 9600);
        loud[1000] = f32::NAN;
        for block in loud.chunks_mut(512) {
            lim.process(block);
        }
        assert!(loud
            .iter()
            .all(|s| s.is_finite() && s.abs() <= CEILING + 1e-6));
        assert!(lim.reduction_db() < -6.0);
        // Retour au gain unité ~1 s après la fin des crêtes.
        let mut quiet = sine(0.25, 48_000);
        for block in quiet.chunks_mut(512) {
            lim.process(block);
        }
        let tail = quiet[90_000..].iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((tail - 0.25).abs() < 0.01, "gain revenu à 1 : {tail}");
    }
}
