//! Synthétiseur de modélisation de grosse caisse (Kick Synth) analogique & techno.
//!
//! Architecture DSP étendue :
//! - Oscillateur à balayage de fréquence exponentiel (Pitch Envelope)
//! - Formes d'onde : Sinusoïde, Triangle, Sinusoïde saturée, Sub-carré rave, Bruit percussif
//! - Transitoire d'attaque (Click generator + bruit blanc percussif filtré)
//! - Corps sub-harmonique riche avec ratio harmonique ajustable (sub-octave, fondamentale, 2e harmonique)
//! - Étage de distorsion multi-types : Tanh analogique, Hard Clip, Wavefolder non-linéaire, Tube Drive
//! - Filtre résonant multimode intégré (Off, LowPass, HighPass, BandPass) avec cutoff et résonance
//! - Banques de presets de pointe : Techno Berlin, 909 Punch, Hardcore Wavefolder, Sub 808, Minimal Click, Industrial Tube, Acid Gabber, Deep Dub.

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
    /// Sous-oscillateur carré percutant style rave / gabber
    SquareSub,
    /// Transitoire avec bruit percussif intégré
    NoiseTransient,
    /// Onde parabolique chaude avec harmoniques paires style 909 analogique
    Parabolic,
    /// Dent de scie inverse / rampe agressive pour kicks industriels & hard techno
    Ramp,
}

/// Type de saturation / distorsion non-linéaire pour le kick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum KickDriveType {
    /// Saturation progressive analogique tanh douce et chaude
    #[default]
    AnalogTanh,
    /// Écrêtage dur (Hard Clip) pour kicks industriels percutants
    HardClip,
    /// Repliement d'onde Buchla / Eurorack (Wavefolder) pour kicks métalliques modulaires
    Wavefolder,
    /// Saturation asymétrique à lampes (Tube Drive) avec harmoniques paires
    TubeDrive,
    /// Quantification lo-fi / réduction de bits pour kicks numériques & chiptune
    Bitcrush,
    /// Fuzz extrême asymétrique au germanium pour hardcore / frenchcore
    Fuzz,
    /// Redressement / repliement d'onde pour gabber & uptempo
    Rectifier,
}

/// Mode de filtre résonant multimode pour le kick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum KickFilterMode {
    /// Pas de filtre / bypass
    #[default]
    Off,
    /// Passe-bas résonant (pour assombrir ou arrondir les sub-kicks)
    LowPass,
    /// Passe-haut résonant (pour top-kicks et rolling basslines)
    HighPass,
    /// Passe-bande percutant (sculptage d'impact médium)
    BandPass,
    /// Coupe-bande / Réjecteur (Notch) pour creuser les fréquences boueuses
    Notch,
    /// Filtre en cloche résonant (Peak) pour sculpter une bosse d'énergie précise
    Peak,
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
    /// Type de saturation non-linéaire
    #[serde(default)]
    pub drive_type: KickDriveType,
    /// Brillance / couleur spectrale post-saturation (0.0 .. 1.0)
    #[serde(default = "default_drive_tone")]
    pub drive_tone: f32,
    /// Renfort d'infrabasse / sub (0.0 .. 1.0)
    pub sub: f32,
    /// Ratio fréquentiel du sub (0.5 = sub-octave, 1.0 = fondamentale, 2.0 = harmonique 2)
    #[serde(default = "default_sub_ratio")]
    pub sub_ratio: f32,
    /// Niveau du bruit de batte percussif (0.0 .. 1.0)
    #[serde(default)]
    pub noise_mix: f32,
    /// Durée du déclin du bruit de batte (ms, 2.0 .. 80.0)
    #[serde(default = "default_noise_decay")]
    pub noise_decay_ms: f32,
    /// Mode du filtre multimode
    #[serde(default)]
    pub filter_mode: KickFilterMode,
    /// Fréquence de coupure du filtre (Hz, 20.0 .. 18000.0)
    #[serde(default = "default_filter_cutoff")]
    pub filter_cutoff_hz: f32,
    /// Résonance du filtre (0.0 .. 0.95)
    #[serde(default)]
    pub filter_reso: f32,
    /// Volume de sortie en dB (-60.0 .. +12.0)
    pub volume_db: f32,
    /// Index ou nom de preset courant
    pub preset_name: String,
    /// Modulation de fréquence linéaire (FM) pour claquant métallique (0.0 .. 1.0)
    #[serde(default = "default_fm_amount")]
    pub fm_amount: f32,
    /// Ratio fréquentiel du modulateur FM (0.25 .. 8.0)
    #[serde(default = "default_fm_ratio")]
    pub fm_ratio: f32,
    /// Durée de maintien (hold) avant le déclin en ms (0.0 .. 60.0)
    #[serde(default = "default_hold_ms")]
    pub hold_ms: f32,
}

fn default_drive_tone() -> f32 {
    0.65
}
fn default_sub_ratio() -> f32 {
    1.0
}
fn default_noise_decay() -> f32 {
    15.0
}
fn default_filter_cutoff() -> f32 {
    12000.0
}
fn default_fm_amount() -> f32 {
    0.0
}
fn default_fm_ratio() -> f32 {
    1.0
}
fn default_hold_ms() -> f32 {
    0.0
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
            drive_type: KickDriveType::AnalogTanh,
            drive_tone: 0.70,
            sub: 0.65,
            sub_ratio: 1.0,
            noise_mix: 0.15,
            noise_decay_ms: 18.0,
            filter_mode: KickFilterMode::LowPass,
            filter_cutoff_hz: 8500.0,
            filter_reso: 0.15,
            volume_db: 0.0,
            preset_name: "Techno Berlin".to_string(),
            fm_amount: 0.0,
            fm_ratio: 1.0,
            hold_ms: 0.0,
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
            drive_type: KickDriveType::AnalogTanh,
            drive_tone: 0.85,
            sub: 0.40,
            sub_ratio: 1.0,
            noise_mix: 0.25,
            noise_decay_ms: 12.0,
            filter_mode: KickFilterMode::Off,
            filter_cutoff_hz: 14000.0,
            filter_reso: 0.0,
            volume_db: 0.0,
            preset_name: "909 Punch".to_string(),
            fm_amount: 0.0,
            fm_ratio: 1.0,
            hold_ms: 0.0,
        }
    }

    /// Preset 3 : Hardcore Wavefolder — repliement d'onde agressif et percutant.
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
            drive_type: KickDriveType::Wavefolder,
            drive_tone: 0.60,
            sub: 0.50,
            sub_ratio: 1.0,
            noise_mix: 0.35,
            noise_decay_ms: 24.0,
            filter_mode: KickFilterMode::LowPass,
            filter_cutoff_hz: 6500.0,
            filter_reso: 0.35,
            volume_db: -1.5,
            preset_name: "Hardcore Wavefolder".to_string(),
            fm_amount: 0.0,
            fm_ratio: 1.0,
            hold_ms: 0.0,
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
            drive_type: KickDriveType::TubeDrive,
            drive_tone: 0.50,
            sub: 0.90,
            sub_ratio: 0.5,
            noise_mix: 0.05,
            noise_decay_ms: 10.0,
            filter_mode: KickFilterMode::LowPass,
            filter_cutoff_hz: 1800.0,
            filter_reso: 0.10,
            volume_db: 0.0,
            preset_name: "Sub 808".to_string(),
            fm_amount: 0.0,
            fm_ratio: 1.0,
            hold_ms: 0.0,
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
            drive_type: KickDriveType::AnalogTanh,
            drive_tone: 0.80,
            sub: 0.30,
            sub_ratio: 1.0,
            noise_mix: 0.20,
            noise_decay_ms: 8.0,
            filter_mode: KickFilterMode::BandPass,
            filter_cutoff_hz: 1200.0,
            filter_reso: 0.25,
            volume_db: 0.0,
            preset_name: "Minimal Click".to_string(),
            fm_amount: 0.0,
            fm_ratio: 1.0,
            hold_ms: 0.0,
        }
    }

    /// Preset 6 : Industrial Tube — kick sombre, texturé avec saturation à lampes et filtre résonant.
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
            drive_type: KickDriveType::TubeDrive,
            drive_tone: 0.65,
            sub: 0.75,
            sub_ratio: 1.0,
            noise_mix: 0.30,
            noise_decay_ms: 22.0,
            filter_mode: KickFilterMode::LowPass,
            filter_cutoff_hz: 4200.0,
            filter_reso: 0.40,
            volume_db: -0.5,
            preset_name: "Industrial Tube".to_string(),
            fm_amount: 0.0,
            fm_ratio: 1.0,
            hold_ms: 0.0,
        }
    }

    /// Preset 7 : Acid Gabber — distorsion hard clipping extrême et sous-oscillateur carré.
    #[must_use]
    pub fn preset_acid_gabber() -> Self {
        Self {
            enabled: true,
            waveform: KickWaveform::SquareSub,
            tune_hz: 56.0,
            tuning_semitones: 0.0,
            decay_ms: 260.0,
            sweep_time_ms: 26.0,
            snap: 0.92,
            click: 0.75,
            drive: 0.90,
            drive_type: KickDriveType::HardClip,
            drive_tone: 0.80,
            sub: 0.60,
            sub_ratio: 1.0,
            noise_mix: 0.40,
            noise_decay_ms: 20.0,
            filter_mode: KickFilterMode::LowPass,
            filter_cutoff_hz: 7500.0,
            filter_reso: 0.50,
            volume_db: -2.0,
            preset_name: "Acid Gabber".to_string(),
            fm_amount: 0.0,
            fm_ratio: 1.0,
            hold_ms: 0.0,
        }
    }

    /// Preset 8 : Deep Dub Rumble — kick dub profond avec filtre passe-bas doux et sub 50Hz.
    #[must_use]
    pub fn preset_deep_dub() -> Self {
        Self {
            enabled: true,
            waveform: KickWaveform::Sine,
            tune_hz: 40.0,
            tuning_semitones: 0.0,
            decay_ms: 550.0,
            sweep_time_ms: 38.0,
            snap: 0.55,
            click: 0.35,
            drive: 0.25,
            drive_type: KickDriveType::AnalogTanh,
            drive_tone: 0.45,
            sub: 0.95,
            sub_ratio: 0.5,
            noise_mix: 0.10,
            noise_decay_ms: 14.0,
            filter_mode: KickFilterMode::LowPass,
            filter_cutoff_hz: 900.0,
            filter_reso: 0.20,
            volume_db: 0.5,
            preset_name: "Deep Dub Rumble".to_string(),
            fm_amount: 0.0,
            fm_ratio: 1.0,
            hold_ms: 0.0,
        }
    }

    /// Preset 9 : Frenchcore Fuzz — saturation fuzz destructrice, attaque percutante et sub soutenu.
    #[must_use]
    pub fn preset_frenchcore_fuzz() -> Self {
        Self {
            enabled: true,
            waveform: KickWaveform::SatSine,
            tune_hz: 52.0,
            tuning_semitones: 0.0,
            decay_ms: 280.0,
            sweep_time_ms: 22.0,
            snap: 0.85,
            click: 0.70,
            drive: 0.82,
            drive_type: KickDriveType::Fuzz,
            drive_tone: 0.75,
            sub: 0.80,
            sub_ratio: 1.0,
            noise_mix: 0.15,
            noise_decay_ms: 20.0,
            filter_mode: KickFilterMode::LowPass,
            filter_cutoff_hz: 6500.0,
            filter_reso: 0.35,
            volume_db: 0.0,
            preset_name: "Frenchcore Fuzz".to_string(),
            fm_amount: 0.15,
            fm_ratio: 2.0,
            hold_ms: 6.0,
        }
    }

    /// Preset 10 : Dark Psy FM Kick — modulation de fréquence percutante, onde parabolique et sweep rapide.
    #[must_use]
    pub fn preset_dark_psy_fm() -> Self {
        Self {
            enabled: true,
            waveform: KickWaveform::Parabolic,
            tune_hz: 56.0,
            tuning_semitones: 0.0,
            decay_ms: 210.0,
            sweep_time_ms: 18.0,
            snap: 0.90,
            click: 0.80,
            drive: 0.35,
            drive_type: KickDriveType::AnalogTanh,
            drive_tone: 0.80,
            sub: 0.60,
            sub_ratio: 1.0,
            noise_mix: 0.08,
            noise_decay_ms: 10.0,
            filter_mode: KickFilterMode::Peak,
            filter_cutoff_hz: 4200.0,
            filter_reso: 0.45,
            volume_db: 0.5,
            preset_name: "Dark Psy FM Kick".to_string(),
            fm_amount: 0.65,
            fm_ratio: 3.0,
            hold_ms: 2.0,
        }
    }

    /// Preset 11 : Glitch Bitcrush — texture numérique industrielle lo-fi avec transitoire bruitée.
    #[must_use]
    pub fn preset_glitch_bitcrush() -> Self {
        Self {
            enabled: true,
            waveform: KickWaveform::NoiseTransient,
            tune_hz: 48.0,
            tuning_semitones: 0.0,
            decay_ms: 310.0,
            sweep_time_ms: 25.0,
            snap: 0.75,
            click: 0.85,
            drive: 0.70,
            drive_type: KickDriveType::Bitcrush,
            drive_tone: 0.60,
            sub: 0.75,
            sub_ratio: 0.5,
            noise_mix: 0.30,
            noise_decay_ms: 25.0,
            filter_mode: KickFilterMode::BandPass,
            filter_cutoff_hz: 2800.0,
            filter_reso: 0.40,
            volume_db: -0.5,
            preset_name: "Glitch Bitcrush".to_string(),
            fm_amount: 0.0,
            fm_ratio: 1.0,
            hold_ms: 0.0,
        }
    }

    /// Preset 12 : 909 Parabolic Punch — attaque parabolique punchy, résonance de batte et hold analogique.
    #[must_use]
    pub fn preset_909_parabolic_punch() -> Self {
        Self {
            enabled: true,
            waveform: KickWaveform::Parabolic,
            tune_hz: 50.0,
            tuning_semitones: 0.0,
            decay_ms: 360.0,
            sweep_time_ms: 30.0,
            snap: 0.80,
            click: 0.65,
            drive: 0.40,
            drive_type: KickDriveType::AnalogTanh,
            drive_tone: 0.70,
            sub: 0.85,
            sub_ratio: 1.0,
            noise_mix: 0.12,
            noise_decay_ms: 18.0,
            filter_mode: KickFilterMode::LowPass,
            filter_cutoff_hz: 5500.0,
            filter_reso: 0.25,
            volume_db: 0.8,
            preset_name: "909 Parabolic Punch".to_string(),
            fm_amount: 0.0,
            fm_ratio: 1.0,
            hold_ms: 8.0,
        }
    }

    /// Preset 13 : Trap 808 Long Boom — onde sub pure avec long sustain, saturation à lampes et coupe-bande anti-boue.
    #[must_use]
    pub fn preset_trap_808_long_boom() -> Self {
        Self {
            enabled: true,
            waveform: KickWaveform::Sine,
            tune_hz: 38.0,
            tuning_semitones: 0.0,
            decay_ms: 1400.0,
            sweep_time_ms: 45.0,
            snap: 0.45,
            click: 0.30,
            drive: 0.45,
            drive_type: KickDriveType::TubeDrive,
            drive_tone: 0.50,
            sub: 1.0,
            sub_ratio: 0.5,
            noise_mix: 0.05,
            noise_decay_ms: 12.0,
            filter_mode: KickFilterMode::Notch,
            filter_cutoff_hz: 240.0,
            filter_reso: 0.30,
            volume_db: 1.0,
            preset_name: "Trap 808 Long Boom".to_string(),
            fm_amount: 0.0,
            fm_ratio: 1.0,
            hold_ms: 15.0,
        }
    }

    /// Retourne la liste complète des presets disponibles.
    #[must_use]
    pub fn all_presets() -> Vec<Self> {
        vec![
            Self::preset_techno_berlin(),
            Self::preset_punchy_909(),
            Self::preset_hardcore(),
            Self::preset_sub_808(),
            Self::preset_minimal(),
            Self::preset_industrial(),
            Self::preset_acid_gabber(),
            Self::preset_deep_dub(),
            Self::preset_frenchcore_fuzz(),
            Self::preset_dark_psy_fm(),
            Self::preset_glitch_bitcrush(),
            Self::preset_909_parabolic_punch(),
            Self::preset_trap_808_long_boom(),
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
    /// Phase du sub-oscillateur
    sub_phase: f32,
    /// Phase du modulateur FM
    fm_phase: f32,
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
    /// Filtre résonant SVF
    filter_ic1: f32,
    filter_ic2: f32,
    /// Filtre one-pole de tone
    tone_state: f32,
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
            sub_phase: 0.0,
            fm_phase: 0.0,
            time: 0.0,
            duration: 0.5,
            velocity_gain: 1.0,
            is_playing: false,
            noise_seed: 54321,
            filter_ic1: 0.0,
            filter_ic2: 0.0,
            tone_state: 0.0,
        }
    }

    /// Déclenche le kick avec une vélocité (1..127) et note MIDI optionnelle.
    pub fn trigger(&mut self, _note: u8, velocity: u8) {
        if !self.params.enabled {
            return;
        }
        self.phase = 0.0;
        self.sub_phase = 0.0;
        self.fm_phase = 0.0;
        self.time = 0.0;
        let v = (velocity.clamp(1, 127) as f32 / 127.0).powf(0.85);
        self.velocity_gain = v;
        let hold_sec = (self.params.hold_ms * 0.001).clamp(0.0, 0.08);
        self.duration = (hold_sec + self.params.decay_ms / 1000.0 * 2.2).clamp(0.08, 3.5);
        self.is_playing = true;
        self.filter_ic1 = 0.0;
        self.filter_ic2 = 0.0;
        self.tone_state = 0.0;
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
        let hold_sec = (p.hold_ms * 0.001).clamp(0.0, 0.08);
        let drive = p.drive;
        let click_level = p.click;
        let sub = p.sub;
        let sub_ratio = p.sub_ratio.clamp(0.25, 4.0);
        let vol_linear = 10.0f32.powf(p.volume_db / 20.0) * self.velocity_gain;
        let noise_mix = p.noise_mix;
        let noise_tau = (p.noise_decay_ms / 1000.0).max(0.002);
        let fm_amt = p.fm_amount.clamp(0.0, 1.0);
        let fm_ratio = p.fm_ratio.clamp(0.25, 8.0);

        // Paramètres du filtre SVF
        let f_mode = p.filter_mode;
        let f_cut = p.filter_cutoff_hz.clamp(20.0, sr * 0.48);
        let f_reso = p.filter_reso.clamp(0.0, 0.95);
        let g = (PI * f_cut / sr).tan();
        let k = 2.0 * (1.0 - f_reso);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        let frames = out.len() / 2;

        for f in 0..frames {
            if self.time >= self.duration {
                self.is_playing = false;
                break;
            }

            let t = self.time;
            let decay_t = (t - hold_sec).max(0.0);

            // Enveloppe de fréquence exponentielle : f(t) = base + snap * exp(-decay_t / tau)
            let cur_freq = base_freq + snap_freq * (-decay_t / sweep_tau).exp();

            // Incréments de phase (oscillateur principal, modulateur FM et sub)
            let phase_inc = 2.0 * PI * cur_freq / sr;
            self.phase = (self.phase + phase_inc) % (2.0 * PI);
            let sub_inc = phase_inc * sub_ratio;
            self.sub_phase = (self.sub_phase + sub_inc) % (2.0 * PI);

            let fm_inc = 2.0 * PI * cur_freq * fm_ratio / sr;
            self.fm_phase = (self.fm_phase + fm_inc) % (2.0 * PI);
            let fm_mod = self.fm_phase.sin() * fm_amt * (-decay_t / sweep_tau).exp();
            let osc_phase = (self.phase + fm_mod * PI).rem_euclid(2.0 * PI);

            // Génération de la forme d'onde principale
            let raw_osc = match p.waveform {
                KickWaveform::Sine => osc_phase.sin(),
                KickWaveform::Triangle => (2.0 / PI) * osc_phase.sin().asin(),
                KickWaveform::SatSine => {
                    let s = osc_phase.sin();
                    (s * 1.6).tanh()
                }
                KickWaveform::SquareSub => {
                    let s = osc_phase.sin();
                    let sq = if self.sub_phase < PI { 0.7 } else { -0.7 };
                    s * 0.6 + sq * 0.4
                }
                KickWaveform::NoiseTransient => {
                    let s = osc_phase.sin();
                    let click_impulse = (osc_phase * 4.0).sin() * 0.3;
                    s * 0.7 + click_impulse
                }
                KickWaveform::Parabolic => {
                    let norm = osc_phase / PI - 1.0;
                    (norm * norm) * 2.0 - 1.0
                }
                KickWaveform::Ramp => 1.0 - (osc_phase / PI),
            };

            // Renfort de sub selon sub_ratio
            let sub_osc = self.sub_phase.sin() * sub * (-decay_t / (decay_tau * 1.5)).exp();
            let body = raw_osc * (1.0 - sub * 0.25) + sub_osc * 0.4;

            // Enveloppe d'amplitude exponentielle + micro-attaque anti-clic
            let attack = (t / 0.0018).min(1.0);
            let amp_env = (-decay_t / decay_tau).exp() * attack;

            // Générateur de bruit pseudo-aléatoire xorshift32
            self.noise_seed ^= self.noise_seed << 13;
            self.noise_seed ^= self.noise_seed >> 17;
            self.noise_seed ^= self.noise_seed << 5;
            let rand_val = (self.noise_seed as f32 / u32::MAX as f32) * 2.0 - 1.0;

            // Transitoire de clic d'attaque (0..7 ms)
            let click = if t < 0.007 {
                let env = (1.0 - t / 0.007).powi(2);
                let burst = (2.0 * PI * 2400.0 * t).sin();
                (burst * 0.65 + rand_val * 0.35) * env * click_level
            } else {
                0.0
            };

            // Couche de bruit de batte (noise transient)
            let noise_layer = if noise_mix > 0.001 {
                let n_env = (-t / noise_tau).exp();
                rand_val * n_env * noise_mix * 0.4
            } else {
                0.0
            };

            // Somme brute
            let mixed = body * amp_env + click + noise_layer;

            // Étage de saturation non-linéaire selon le type de distorsion
            let saturated = if drive > 0.01 {
                let gain = 1.0 + drive * 5.0;
                let pushed = mixed * gain;

                match p.drive_type {
                    KickDriveType::AnalogTanh => {
                        let shaped = (pushed + 0.12 * pushed * pushed).tanh();
                        shaped * (1.0 / (1.0 + drive * 0.5))
                    }
                    KickDriveType::HardClip => {
                        let clipped = pushed.clamp(-1.0, 1.0);
                        clipped * (1.0 / (1.0 + drive * 0.4))
                    }
                    KickDriveType::Wavefolder => {
                        // Wavefolding non-linéaire récursif style Buchla
                        let mut x = pushed;
                        for _ in 0..2 {
                            if x > 1.0 {
                                x = 2.0 - x;
                            } else if x < -1.0 {
                                x = -2.0 - x;
                            }
                        }
                        x.clamp(-1.2, 1.2) * 0.85
                    }
                    KickDriveType::TubeDrive => {
                        let x = pushed;
                        let tube = (x + 0.22 * x * x) / (1.0 + x.abs());
                        tube * 0.90
                    }
                    KickDriveType::Bitcrush => {
                        let bits = (16.0 - drive * 12.0).clamp(2.0, 16.0);
                        let steps = 2.0_f32.powf(bits);
                        (pushed * steps).round() / steps * (1.0 / (1.0 + drive * 0.3))
                    }
                    KickDriveType::Fuzz => {
                        let sign = pushed.signum();
                        let fuzzed = sign * (1.0 - (-pushed.abs() * (2.0 + drive * 4.0)).exp());
                        fuzzed * 0.85
                    }
                    KickDriveType::Rectifier => {
                        let rect = if pushed > 0.0 {
                            pushed
                        } else {
                            -pushed * (1.0 - drive * 0.7)
                        };
                        (rect - 0.3 * drive).tanh() * 0.90
                    }
                }
            } else {
                mixed
            };

            // Filtre de brillance / tonalité post-saturation (one-pole)
            let tone_coeff = (p.drive_tone * 0.85 + 0.1).clamp(0.05, 0.95);
            self.tone_state += tone_coeff * (saturated - self.tone_state);
            let post_tone = self.tone_state;

            // Étage de Filtre Résonant Multimode (SVF)
            let filtered = if f_mode != KickFilterMode::Off {
                let v3 = post_tone - self.filter_ic2;
                let v1 = a1 * self.filter_ic1 + a2 * v3;
                let v2 = self.filter_ic2 + a2 * self.filter_ic1 + a3 * v3;
                self.filter_ic1 = 2.0 * v1 - self.filter_ic1;
                self.filter_ic2 = 2.0 * v2 - self.filter_ic2;

                match f_mode {
                    KickFilterMode::LowPass => v2,
                    KickFilterMode::HighPass => post_tone - k * v1 - v2,
                    KickFilterMode::BandPass => v1,
                    KickFilterMode::Notch => post_tone - k * v1,
                    KickFilterMode::Peak => (2.0 * v2 + k * v1 - post_tone).clamp(-2.0, 2.0),
                    KickFilterMode::Off => post_tone,
                }
            } else {
                post_tone
            };

            // Sortie finale amplifiée
            let out_sample = filtered * vol_linear;

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
        assert!(presets.len() >= 13, "Doit contenir au moins 13 presets");
        for p in presets {
            let mut kick = KickSynth::new(48000);
            kick.params = p;
            kick.trigger(36, 110);
            let mut buf = vec![0.0f32; 512];
            kick.render(&mut buf);
            let amp = buf.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            assert!(
                amp > 0.02,
                "Preset {} doit générer du son",
                kick.params.preset_name
            );
        }
    }

    #[test]
    fn test_kick_all_drive_types() {
        for drive_type in [
            KickDriveType::AnalogTanh,
            KickDriveType::HardClip,
            KickDriveType::Wavefolder,
            KickDriveType::TubeDrive,
            KickDriveType::Bitcrush,
            KickDriveType::Fuzz,
            KickDriveType::Rectifier,
        ] {
            let mut kick = KickSynth::new(48000);
            kick.params.drive = 0.8;
            kick.params.drive_type = drive_type;
            kick.trigger(36, 120);
            let mut buf = vec![0.0f32; 512];
            kick.render(&mut buf);
            let amp = buf.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            assert!(amp > 0.05);
        }
    }

    #[test]
    fn test_kick_all_filter_modes() {
        for mode in [
            KickFilterMode::Off,
            KickFilterMode::LowPass,
            KickFilterMode::HighPass,
            KickFilterMode::BandPass,
            KickFilterMode::Notch,
            KickFilterMode::Peak,
        ] {
            let mut kick = KickSynth::new(48000);
            kick.params.filter_mode = mode;
            kick.params.filter_cutoff_hz = 1500.0;
            kick.params.filter_reso = 0.5;
            kick.trigger(36, 120);
            let mut buf = vec![0.0f32; 512];
            kick.render(&mut buf);
            let amp = buf.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            assert!(amp > 0.01);
        }
    }

    #[test]
    fn test_kick_all_waveforms_and_fm_hold() {
        for wf in [
            KickWaveform::Sine,
            KickWaveform::Triangle,
            KickWaveform::SatSine,
            KickWaveform::SquareSub,
            KickWaveform::NoiseTransient,
            KickWaveform::Parabolic,
            KickWaveform::Ramp,
        ] {
            let mut kick = KickSynth::new(48000);
            kick.params.waveform = wf;
            kick.params.fm_amount = 0.5;
            kick.params.fm_ratio = 2.0;
            kick.params.hold_ms = 10.0;
            kick.trigger(36, 120);
            let mut buf = vec![0.0f32; 512];
            kick.render(&mut buf);
            let amp = buf.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            assert!(amp > 0.05);
        }
    }
}
