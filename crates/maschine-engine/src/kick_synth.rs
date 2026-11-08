//! Synthétiseur de modélisation de grosse caisse (Kick Synth) analogique & techno.
//!
//! Architecture DSP :
//! - Oscillateur à balayage de fréquence exponentiel (Pitch Envelope)
//! - Transitoire d'attaque (Click generator : impulsion + bruit filtré passe-bande)
//! - Corps sub-harmonique riche (fondamentale sinusoïdale + 2e harmonique dosée)
//! - Enveloppe d'amplitude exponentielle à déclin contrôlé
//! - Étage de saturation non-linéaire analogique / overdrive techno (asymmetric drive / tanh)
//! - Banques de presets : Techno Berlin, 909 Punch, Hardcore Drive, Sub 808, Minimal Click, Industrial.

use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Type de forme d'onde pour le corps du kick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum KickWaveform {
    /// Sinusoïde pure avec résonance sub
    #[default]
    Sine,
    /// Triangle adouci (plus d'harmoniques impaires)
    Triangle,
    /// Sinusoïde saturée (techno punch)
    SatSine,
}

/// Paramètres de synthèse de la modélisation de Kick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KickParams {
    /// Module actif
    pub enabled: bool,
    /// Forme d'onde de base
    pub waveform: KickWaveform,
    /// Fréquence fondamentale de repos (Hz, 30.0 .. 90.0, typique 45..55 Hz)
    pub tune_hz: f32,
    /// Accordage fin en demi-tons (-12 .. +12)
    pub tuning_semitones: f32,
    /// Déclin d'amplitude du corps (ms, 40.0 .. 2000.0)
    pub decay_ms: f32,
    /// Durée du glissando de pitch / drop (ms, 5.0 .. 120.0)
    pub sweep_time_ms: f32,
    /// Intensité du saut de pitch / punch à l'attaque (0.0 .. 1.0, ajoute jusqu'à +650 Hz)
    pub snap: f32,
    /// Niveau du transitoire de clic d'impact (0.0 .. 1.0)
    pub click: f32,
    /// Saturation / overdrive / distorsion analogique (0.0 .. 1.0)
    pub drive: f32,
    /// Renfort d'infrabasse / sub (0.0 .. 1.0)
    pub sub: f32,
    /// Volume de sortie en dB (-60.0 .. +12.0)
    pub volume_db: f32,
    /// Index ou nom de preset courant
    pub preset_name: String,
}

impl Default for KickParams {
    fn default() -> Self {
        Self::preset_techno_berlin()
    }
}

impl KickParams {
    /// Preset 1 : Techno Berlin — sub profond, pitch drop rapide, drive chaud et percutant.
    #[must_use]
    pub fn preset_techno_berlin() -> Self {
        Self {
            enabled: true,
            waveform: KickWaveform::SatSine,
            tune_hz: 48.0,
            tuning_semitones: 0.0,
            decay_ms: 320.0,
            sweep_time_ms: 28.0,
            snap: 0.72,
            click: 0.55,
            drive: 0.42,
            sub: 0.65,
            volume_db: 0.0,
            preset_name: "Techno Berlin".to_string(),
        }
    }

    /// Preset 2 : 909 Punch — kick percutant typé boîte à rythme légendaire.
    #[must_use]
    pub fn preset_punchy_909() -> Self {
        Self {
            enabled: true,
            waveform: KickWaveform::Sine,
            tune_hz: 52.0,
            tuning_semitones: 0.0,
            decay_ms: 240.0,
            sweep_time_ms: 22.0,
            snap: 0.82,
            click: 0.68,
            drive: 0.20,
            sub: 0.40,
            volume_db: 0.0,
            preset_name: "909 Punch".to_string(),
        }
    }

    /// Preset 3 : Hardcore Drive — distorsion abrasive, drop violent, présence maximale.
    #[must_use]
    pub fn preset_hardcore() -> Self {
        Self {
            enabled: true,
            waveform: KickWaveform::SatSine,
            tune_hz: 55.0,
            tuning_semitones: 0.0,
            decay_ms: 290.0,
            sweep_time_ms: 35.0,
            snap: 0.88,
            click: 0.80,
            drive: 0.85,
            sub: 0.50,
            volume_db: -1.5,
            preset_name: "Hardcore Drive".to_string(),
        }
    }

    /// Preset 4 : Sub 808 — infrabasse pure et longue, attaque douce, queue grondante.
    #[must_use]
    pub fn preset_sub_808() -> Self {
        Self {
            enabled: true,
            waveform: KickWaveform::Sine,
            tune_hz: 42.0,
            tuning_semitones: 0.0,
            decay_ms: 750.0,
            sweep_time_ms: 45.0,
            snap: 0.45,
            click: 0.25,
            drive: 0.12,
            sub: 0.90,
            volume_db: 0.0,
            preset_name: "Sub 808".to_string(),
        }
    }

    /// Preset 5 : Minimal Click — kick court et précis pour minimal techno / microhouse.
    #[must_use]
    pub fn preset_minimal() -> Self {
        Self {
            enabled: true,
            waveform: KickWaveform::Sine,
            tune_hz: 50.0,
            tuning_semitones: 0.0,
            decay_ms: 170.0,
            sweep_time_ms: 16.0,
            snap: 0.60,
            click: 0.70,
            drive: 0.15,
            sub: 0.30,
            volume_db: 0.0,
            preset_name: "Minimal Click".to_string(),
        }
    }

    /// Preset 6 : Industrial Rumble — kick sombre, texturé et dense.
    #[must_use]
    pub fn preset_industrial() -> Self {
        Self {
            enabled: true,
            waveform: KickWaveform::Triangle,
            tune_hz: 44.0,
            tuning_semitones: 0.0,
            decay_ms: 400.0,
            sweep_time_ms: 32.0,
            snap: 0.75,
            click: 0.60,
            drive: 0.58,
            sub: 0.75,
            volume_db: -0.5,
            preset_name: "Industrial".to_string(),
        }
    }

    /// Retourne la liste des presets disponibles.
    #[must_use]
    pub fn all_presets() -> [Self; 6] {
        [
            Self::preset_techno_berlin(),
            Self::preset_punchy_909(),
            Self::preset_hardcore(),
            Self::preset_sub_808(),
            Self::preset_minimal(),
            Self::preset_industrial(),
        ]
    }
}

/// Moteur de synthèse temps réel pour Kick.
#[derive(Debug, Clone)]
pub struct KickSynth {
    pub params: KickParams,
    sample_rate: f32,
    /// Phase de l'oscillateur (0 .. 2*PI)
    phase: f32,
    /// Temps écoulé depuis le déclenchement en secondes
    time: f32,
    /// Durée totale active calculée
    duration: f32,
    /// Vélocité du dernier déclenchement (0..1)
    velocity_gain: f32,
    /// La voix est-elle active
    is_playing: bool,
    /// Générateur pseudo-aléatoire pour le clic
    noise_seed: u32,
}

impl Default for KickSynth {
    fn default() -> Self {
        Self::new(48000)
    }
}

impl KickSynth {
    /// Crée un nouveau moteur KickSynth.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            params: KickParams::default(),
            sample_rate: sample_rate.max(8000) as f32,
            phase: 0.0,
            time: 0.0,
            duration: 0.5,
            velocity_gain: 1.0,
            is_playing: false,
            noise_seed: 54321,
        }
    }

    /// Déclenche le kick avec une vélocité (1..127) et note MIDI optionnelle.
    pub fn trigger(&mut self, _note: u8, velocity: u8) {
        if !self.params.enabled {
            return;
        }
        self.phase = 0.0;
        self.time = 0.0;
        let v = (velocity.clamp(1, 127) as f32 / 127.0).powf(0.85);
        self.velocity_gain = v;
        self.duration = (self.params.decay_ms / 1000.0 * 2.2).clamp(0.08, 3.5);
        self.is_playing = true;
    }

    /// Rend un bloc audio stéréo entrelacé `[L, R, L, R, ...]`.
    pub fn render(&mut self, out: &mut [f32]) {
        if !self.is_playing || !self.params.enabled {
            return;
        }

        let sr = self.sample_rate;
        let dt = 1.0 / sr;
        let p = &self.params;

        // Accordage fondamental effectif
        let semitones = p.tuning_semitones;
        let pitch_mult = 2.0f32.powf(semitones / 12.0);
        let base_freq = (p.tune_hz * pitch_mult).clamp(20.0, 200.0);
        let snap_freq = p.snap * 580.0;
        let sweep_tau = (p.sweep_time_ms / 1000.0).max(0.003);
        let decay_tau = (p.decay_ms / 1000.0 * 0.45).max(0.015);
        let drive = p.drive;
        let click_level = p.click;
        let sub = p.sub;
        let vol_linear = 10.0f32.powf(p.volume_db / 20.0) * self.velocity_gain;

        let frames = out.len() / 2;

        for f in 0..frames {
            if self.time >= self.duration {
                self.is_playing = false;
                break;
            }

            let t = self.time;

            // Enveloppe de fréquence exponentielle : f(t) = base + snap * exp(-t / tau)
            let cur_freq = base_freq + snap_freq * (-t / sweep_tau).exp();

            // Incrément de phase
            let phase_inc = 2.0 * PI * cur_freq / sr;
            self.phase = (self.phase + phase_inc) % (2.0 * PI);

            // Génération de la forme d'onde principale
            let raw_osc = match p.waveform {
                KickWaveform::Sine => self.phase.sin(),
                KickWaveform::Triangle => {
                    // Triangle bande passante continue
                    (2.0 / PI) * (self.phase - PI).asin()
                }
                KickWaveform::SatSine => {
                    let s = self.phase.sin();
                    (s * 1.5).tanh()
                }
            };

            // Renfort de sub / 2e harmonique
            let body = raw_osc * (1.0 - sub * 0.25)
                + (2.0 * self.phase).sin() * (sub * 0.22) * (-t / (decay_tau * 1.5)).exp();

            // Enveloppe d'amplitude exponentielle + micro-attaque pour éviter les pops
            let attack = (t / 0.0018).min(1.0);
            let amp_env = (-t / decay_tau).exp() * attack;

            // Transitoire de clic d'attaque (0..7 ms)
            let click = if t < 0.007 {
                let env = (1.0 - t / 0.007).powi(2);
                // Bruit blanc xorshift32 + impulsion haute fréquence
                self.noise_seed ^= self.noise_seed << 13;
                self.noise_seed ^= self.noise_seed >> 17;
                self.noise_seed ^= self.noise_seed << 5;
                let noise = (self.noise_seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
                let burst = (2.0 * PI * 2400.0 * t).sin();
                (burst * 0.65 + noise * 0.35) * env * click_level
            } else {
                0.0
            };

            // Somme brute
            let mixed = body * amp_env + click;

            // Étage de distorsion / saturation analogique non-linéaire
            let saturated = if drive > 0.01 {
                let gain = 1.0 + drive * 4.5;
                let pushed = mixed * gain;
                // Saturation asymétrique typique des préamplis analogiques
                let shaped = (pushed + 0.1 * pushed * pushed).tanh();
                shaped * (1.0 / (1.0 + drive * 0.5))
            } else {
                mixed
            };

            // Sortie finale amplifiée
            let out_sample = saturated * vol_linear;

            out[f * 2] += out_sample;
            out[f * 2 + 1] += out_sample;

            self.time += dt;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kick_synth_generates_audio() {
        let mut kick = KickSynth::new(48000);
        kick.trigger(36, 100);
        let mut buffer = vec![0.0f32; 1024];
        kick.render(&mut buffer);

        let max_amp = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(max_amp > 0.05, "Le kick doit produire du son audible");
    }

    #[test]
    fn test_kick_presets_valid() {
        let presets = KickParams::all_presets();
        for p in presets {
            assert!(p.tune_hz >= 30.0 && p.tune_hz <= 90.0);
            assert!(p.decay_ms >= 40.0);
        }
    }
}
