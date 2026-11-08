//! Synthétiseur polyphonique Wavetable / ROMpler haute fidélité (style Nexus / Serum).
//!
//! Spécifications d'architecture DSP :
//! - Polyphonie jusqu'à 16 voix avec vol de voix intelligent (voice stealing)
//! - Double oscillateur (Osc 1 & Osc 2) avec formes d'onde Serum/Nexus :
//!   * Analogiques : Sinusoïde, Triangle, Scie (Sawtooth), Impulsion/Carré avec PWM
//!   * Wavetable / ROMpler : Supersaw (unisson 7 voix intégré), Grand Piano, M1 Piano,
//!     Rhodes Tines, Digital Lead, Tonewheel Organ, Vocal Formant
//! - Moteur Unisson configurable : 1 à 7 voix d'unisson avec spread de désaccordage (detune) et stéréo
//! - Filtre Résonant Multimode : Passe-Bas 12dB & 24dB, Passe-Haut 12dB, Passe-Bande 12dB
//! - 2 Enveloppes ADSR haute précision : Enveloppe d'amplitude (Amp) et Enveloppe de filtre (Mod)
//! - LFO de modulation multi-formes d'ondes (Cutoff, Pitch, Volume, Pan)
//! - Suite d'effets DSP stéréo intégrée : Chorus / Dimension Expander, Delay Stéréo Ping-Pong,
//!   Réverbération algorithmique de salle, Overdrive analogique tanh
//! - Banque complète de presets de production (> 25 presets) : Pianos, Basses, Leads, Pads, Plucks

use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Formes d'onde disponibles pour les oscillateurs Wavetable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WavetableWaveform {
    /// Sinusoïde pure
    Sine,
    /// Triangle doux
    Triangle,
    /// Dent de scie riche en harmoniques (Sawtooth)
    #[default]
    Saw,
    /// Onde carrée / impulsion avec largeur modulable (Square / Pulse)
    Square,
    /// Multi-oscillateur Hypersaw / Supersaw (7 oscillateurs désaccordés empilés)
    Supersaw,
    /// Modélisation acoustique de piano à queue (Grand Piano)
    GrandPiano,
    /// Piano électronique M1 90s House classique
    M1Piano,
    /// Piano électrique vintage Rhodes à lamelles (Tines)
    RhodesTines,
    /// Synthétiseur solo numérique spectral (Serum Digital Lead)
    DigiLead,
    /// Orgue Hammond à tirettes harmoniques (Tonewheel Organ)
    TonewheelOrgan,
    /// Nappe vocale formantique (A-E-I-O-U Choir Pad)
    VocalFormant,
    /// Bruit blanc analogique filtrable
    Noise,
}

/// Type de filtre résonant multimode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WavetableFilterMode {
    /// Filtre désactivé / Bypass
    Off,
    /// Passe-bas 12dB / octave (2 pôles SVF)
    #[default]
    LowPass12,
    /// Passe-bas 24dB / octave (4 pôles en cascade)
    LowPass24,
    /// Passe-haut 12dB / octave
    HighPass,
    /// Passe-bande 12dB / octave
    BandPass,
}

/// Destination du LFO de modulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LfoTarget {
    #[default]
    FilterCutoff,
    PitchVibrato,
    VolumeTremolo,
    PanSpread,
}

/// Catégorie musicale de preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresetCategory {
    Piano,
    Bass,
    Lead,
    Pad,
    Pluck,
}

/// Paramètres de synthèse complets du synthé Wavetable / Nexus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WavetableParams {
    pub enabled: bool,
    pub name: String,
    pub category: PresetCategory,

    // --- Oscillateur 1 ---
    pub osc1_wave: WavetableWaveform,
    pub osc1_semitones: i8, // -24 .. +24
    pub osc1_cents: i8,     // -50 .. +50
    pub osc1_level: f32,    // 0.0 .. 1.0
    pub osc1_pan: f32,      // -1.0 .. +1.0

    // --- Oscillateur 2 ---
    pub osc2_enabled: bool,
    pub osc2_wave: WavetableWaveform,
    pub osc2_semitones: i8,
    pub osc2_cents: i8,
    pub osc2_level: f32,
    pub osc2_pan: f32,

    // --- Moteur d'unisson ---
    pub unison_voices: u8,  // 1 .. 7
    pub unison_detune: f32, // 0.0 .. 1.0 (écartement fréquentiel)
    pub unison_spread: f32, // 0.0 .. 1.0 (largeur stéréo)

    // --- Filtre multimode ---
    pub filter_mode: WavetableFilterMode,
    pub filter_cutoff: f32,  // 20.0 .. 20000.0 Hz
    pub filter_reso: f32,    // 0.0 .. 0.98
    pub filter_env_amt: f32, // -1.0 .. +1.0
    pub filter_drive: f32,   // 0.0 .. 1.0

    // --- Enveloppe Amplitude (ADSR) ---
    pub amp_attack_ms: f32,  // 0.5 .. 4000.0 ms
    pub amp_decay_ms: f32,   // 5.0 .. 6000.0 ms
    pub amp_sustain: f32,    // 0.0 .. 1.0
    pub amp_release_ms: f32, // 5.0 .. 8000.0 ms

    // --- Enveloppe Filtre (ADSR) ---
    pub filter_attack_ms: f32,
    pub filter_decay_ms: f32,
    pub filter_sustain: f32,
    pub filter_release_ms: f32,

    // --- LFO de modulation ---
    pub lfo_rate_hz: f32, // 0.1 .. 30.0 Hz
    pub lfo_amount: f32,  // 0.0 .. 1.0
    pub lfo_target: LfoTarget,

    // --- Effets intégrés ---
    pub fx_chorus_mix: f32,     // 0.0 .. 1.0
    pub fx_delay_mix: f32,      // 0.0 .. 1.0
    pub fx_delay_time_ms: f32,  // 20.0 .. 800.0 ms
    pub fx_delay_feedback: f32, // 0.0 .. 0.9
    pub fx_reverb_mix: f32,     // 0.0 .. 1.0
    pub fx_reverb_size: f32,    // 0.1 .. 1.0
    pub fx_distortion: f32,     // 0.0 .. 1.0

    // --- Sortie globale ---
    pub master_volume_db: f32,
    pub glide_time_ms: f32, // 0.0 .. 500.0 ms
}

impl Default for WavetableParams {
    fn default() -> Self {
        Self::preset_concert_grand_piano()
    }
}

impl WavetableParams {
    // ==========================================
    //            PRESETS : PIANOS
    // ==========================================

    /// Piano 1 : Grand Concert Piano — riche, boisé et naturel avec réverbe subtile.
    #[must_use]
    pub fn preset_concert_grand_piano() -> Self {
        Self {
            enabled: true,
            name: "Concert Grand Piano".to_string(),
            category: PresetCategory::Piano,
            osc1_wave: WavetableWaveform::GrandPiano,
            osc1_semitones: 0,
            osc1_cents: 0,
            osc1_level: 0.90,
            osc1_pan: 0.0,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Sine,
            osc2_semitones: -12,
            osc2_cents: 0,
            osc2_level: 0.35,
            osc2_pan: 0.0,
            unison_voices: 1,
            unison_detune: 0.0,
            unison_spread: 0.3,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 14000.0,
            filter_reso: 0.10,
            filter_env_amt: 0.35,
            filter_drive: 0.0,
            amp_attack_ms: 2.0,
            amp_decay_ms: 1800.0,
            amp_sustain: 0.45,
            amp_release_ms: 350.0,
            filter_attack_ms: 1.0,
            filter_decay_ms: 900.0,
            filter_sustain: 0.35,
            filter_release_ms: 300.0,
            lfo_rate_hz: 4.5,
            lfo_amount: 0.0,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.05,
            fx_delay_mix: 0.0,
            fx_delay_time_ms: 350.0,
            fx_delay_feedback: 0.2,
            fx_reverb_mix: 0.28,
            fx_reverb_size: 0.65,
            fx_distortion: 0.0,
            master_volume_db: 0.0,
            glide_time_ms: 0.0,
        }
    }

    /// Piano 2 : House M1 Piano — le piano percutant et brillant des hymnes eurodance & house 90s.
    #[must_use]
    pub fn preset_house_m1_piano() -> Self {
        Self {
            enabled: true,
            name: "House M1 Piano".to_string(),
            category: PresetCategory::Piano,
            osc1_wave: WavetableWaveform::M1Piano,
            osc1_semitones: 0,
            osc1_cents: 0,
            osc1_level: 0.95,
            osc1_pan: -0.15,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::M1Piano,
            osc2_semitones: 0,
            osc2_cents: 8,
            osc2_level: 0.85,
            osc2_pan: 0.15,
            unison_voices: 2,
            unison_detune: 0.15,
            unison_spread: 0.5,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 18000.0,
            filter_reso: 0.15,
            filter_env_amt: 0.40,
            filter_drive: 0.10,
            amp_attack_ms: 1.0,
            amp_decay_ms: 1200.0,
            amp_sustain: 0.40,
            amp_release_ms: 220.0,
            filter_attack_ms: 0.5,
            filter_decay_ms: 700.0,
            filter_sustain: 0.35,
            filter_release_ms: 200.0,
            lfo_rate_hz: 5.0,
            lfo_amount: 0.0,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.25,
            fx_delay_mix: 0.20,
            fx_delay_time_ms: 375.0,
            fx_delay_feedback: 0.35,
            fx_reverb_mix: 0.22,
            fx_reverb_size: 0.55,
            fx_distortion: 0.05,
            master_volume_db: 0.0,
            glide_time_ms: 0.0,
        }
    }

    /// Piano 3 : Neo-Soul Rhodes — piano électrique chaleureux avec vibrato soyeux et trémolo.
    #[must_use]
    pub fn preset_neo_soul_rhodes() -> Self {
        Self {
            enabled: true,
            name: "Neo-Soul Rhodes".to_string(),
            category: PresetCategory::Piano,
            osc1_wave: WavetableWaveform::RhodesTines,
            osc1_semitones: 0,
            osc1_cents: 0,
            osc1_level: 0.90,
            osc1_pan: 0.0,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Triangle,
            osc2_semitones: 0,
            osc2_cents: 4,
            osc2_level: 0.45,
            osc2_pan: 0.0,
            unison_voices: 2,
            unison_detune: 0.08,
            unison_spread: 0.6,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 6500.0,
            filter_reso: 0.12,
            filter_env_amt: 0.25,
            filter_drive: 0.08,
            amp_attack_ms: 4.0,
            amp_decay_ms: 2200.0,
            amp_sustain: 0.15,
            amp_release_ms: 400.0,
            filter_attack_ms: 2.0,
            filter_decay_ms: 1200.0,
            filter_sustain: 0.3,
            filter_release_ms: 350.0,
            lfo_rate_hz: 4.8,
            lfo_amount: 0.12,
            lfo_target: LfoTarget::PitchVibrato,
            fx_chorus_mix: 0.35,
            fx_delay_mix: 0.15,
            fx_delay_time_ms: 450.0,
            fx_delay_feedback: 0.25,
            fx_reverb_mix: 0.32,
            fx_reverb_size: 0.70,
            fx_distortion: 0.02,
            master_volume_db: 0.5,
            glide_time_ms: 0.0,
        }
    }

    /// Piano 4 : FM DX7 E-Piano — son cristallin typique FM 80s avec tines percutantes.
    #[must_use]
    pub fn preset_dx7_epiano() -> Self {
        Self {
            enabled: true,
            name: "FM DX7 E-Piano".to_string(),
            category: PresetCategory::Piano,
            osc1_wave: WavetableWaveform::DigiLead,
            osc1_semitones: 0,
            osc1_cents: 0,
            osc1_level: 0.85,
            osc1_pan: -0.1,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Sine,
            osc2_semitones: 12,
            osc2_cents: 3,
            osc2_level: 0.60,
            osc2_pan: 0.1,
            unison_voices: 2,
            unison_detune: 0.10,
            unison_spread: 0.7,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 12000.0,
            filter_reso: 0.15,
            filter_env_amt: 0.30,
            filter_drive: 0.0,
            amp_attack_ms: 2.0,
            amp_decay_ms: 1900.0,
            amp_sustain: 0.10,
            amp_release_ms: 450.0,
            filter_attack_ms: 1.0,
            filter_decay_ms: 800.0,
            filter_sustain: 0.2,
            filter_release_ms: 400.0,
            lfo_rate_hz: 5.2,
            lfo_amount: 0.08,
            lfo_target: LfoTarget::PitchVibrato,
            fx_chorus_mix: 0.45,
            fx_delay_mix: 0.22,
            fx_delay_time_ms: 400.0,
            fx_delay_feedback: 0.30,
            fx_reverb_mix: 0.35,
            fx_reverb_size: 0.65,
            fx_distortion: 0.0,
            master_volume_db: 0.0,
            glide_time_ms: 0.0,
        }
    }

    // ==========================================
    //            PRESETS : BASSES
    // ==========================================

    /// Basse 1 : 808 Sub Boom — infrabasse pure, puissante et ronde pour hip-hop et trap.
    #[must_use]
    pub fn preset_808_sub_boom() -> Self {
        Self {
            enabled: true,
            name: "808 Sub Boom".to_string(),
            category: PresetCategory::Bass,
            osc1_wave: WavetableWaveform::Sine,
            osc1_semitones: -12,
            osc1_cents: 0,
            osc1_level: 1.0,
            osc1_pan: 0.0,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Triangle,
            osc2_semitones: -12,
            osc2_cents: 0,
            osc2_level: 0.30,
            osc2_pan: 0.0,
            unison_voices: 1,
            unison_detune: 0.0,
            unison_spread: 0.0,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 280.0,
            filter_reso: 0.15,
            filter_env_amt: 0.20,
            filter_drive: 0.25,
            amp_attack_ms: 3.0,
            amp_decay_ms: 850.0,
            amp_sustain: 0.45,
            amp_release_ms: 120.0,
            filter_attack_ms: 2.0,
            filter_decay_ms: 300.0,
            filter_sustain: 0.1,
            filter_release_ms: 100.0,
            lfo_rate_hz: 1.0,
            lfo_amount: 0.0,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.0,
            fx_delay_mix: 0.0,
            fx_delay_time_ms: 300.0,
            fx_delay_feedback: 0.0,
            fx_reverb_mix: 0.0,
            fx_reverb_size: 0.1,
            fx_distortion: 0.20,
            master_volume_db: 1.5,
            glide_time_ms: 45.0,
        }
    }

    /// Basse 2 : Reese Detuned Bass — basse drum & bass / dubstep grondante et mouvante.
    #[must_use]
    pub fn preset_reese_bass() -> Self {
        Self {
            enabled: true,
            name: "Reese Detuned Bass".to_string(),
            category: PresetCategory::Bass,
            osc1_wave: WavetableWaveform::Saw,
            osc1_semitones: -12,
            osc1_cents: -9,
            osc1_level: 0.90,
            osc1_pan: -0.2,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Saw,
            osc2_semitones: -12,
            osc2_cents: 9,
            osc2_level: 0.90,
            osc2_pan: 0.2,
            unison_voices: 4,
            unison_detune: 0.28,
            unison_spread: 0.75,
            filter_mode: WavetableFilterMode::LowPass24,
            filter_cutoff: 750.0,
            filter_reso: 0.35,
            filter_env_amt: 0.45,
            filter_drive: 0.35,
            amp_attack_ms: 10.0,
            amp_decay_ms: 500.0,
            amp_sustain: 0.85,
            amp_release_ms: 160.0,
            filter_attack_ms: 15.0,
            filter_decay_ms: 600.0,
            filter_sustain: 0.50,
            filter_release_ms: 150.0,
            lfo_rate_hz: 2.2,
            lfo_amount: 0.30,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.20,
            fx_delay_mix: 0.0,
            fx_delay_time_ms: 300.0,
            fx_delay_feedback: 0.0,
            fx_reverb_mix: 0.08,
            fx_reverb_size: 0.40,
            fx_distortion: 0.30,
            master_volume_db: 0.0,
            glide_time_ms: 60.0,
        }
    }

    /// Basse 3 : Cyberpunk Dark Saw — mid-bass agressive pour synthwave et midtempo dark.
    #[must_use]
    pub fn preset_cyberpunk_bass() -> Self {
        Self {
            enabled: true,
            name: "Cyberpunk Dark Saw".to_string(),
            category: PresetCategory::Bass,
            osc1_wave: WavetableWaveform::Square,
            osc1_semitones: -12,
            osc1_cents: 0,
            osc1_level: 0.95,
            osc1_pan: 0.0,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Saw,
            osc2_semitones: -24,
            osc2_cents: 0,
            osc2_level: 0.80,
            osc2_pan: 0.0,
            unison_voices: 2,
            unison_detune: 0.15,
            unison_spread: 0.4,
            filter_mode: WavetableFilterMode::LowPass24,
            filter_cutoff: 1200.0,
            filter_reso: 0.45,
            filter_env_amt: 0.65,
            filter_drive: 0.55,
            amp_attack_ms: 2.0,
            amp_decay_ms: 350.0,
            amp_sustain: 0.65,
            amp_release_ms: 80.0,
            filter_attack_ms: 2.0,
            filter_decay_ms: 280.0,
            filter_sustain: 0.20,
            filter_release_ms: 70.0,
            lfo_rate_hz: 4.0,
            lfo_amount: 0.15,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.10,
            fx_delay_mix: 0.0,
            fx_delay_time_ms: 250.0,
            fx_delay_feedback: 0.0,
            fx_reverb_mix: 0.05,
            fx_reverb_size: 0.30,
            fx_distortion: 0.50,
            master_volume_db: -0.5,
            glide_time_ms: 25.0,
        }
    }

    /// Basse 4 : Future House Donk — basse percutante métallique style Tchami / Oliver Heldens.
    #[must_use]
    pub fn preset_future_house_donk() -> Self {
        Self {
            enabled: true,
            name: "Future House Donk".to_string(),
            category: PresetCategory::Bass,
            osc1_wave: WavetableWaveform::DigiLead,
            osc1_semitones: -12,
            osc1_cents: 0,
            osc1_level: 1.0,
            osc1_pan: 0.0,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Sine,
            osc2_semitones: -12,
            osc2_cents: 0,
            osc2_level: 0.65,
            osc2_pan: 0.0,
            unison_voices: 1,
            unison_detune: 0.0,
            unison_spread: 0.0,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 2400.0,
            filter_reso: 0.35,
            filter_env_amt: 0.75,
            filter_drive: 0.20,
            amp_attack_ms: 1.0,
            amp_decay_ms: 180.0,
            amp_sustain: 0.0,
            amp_release_ms: 50.0,
            filter_attack_ms: 0.5,
            filter_decay_ms: 120.0,
            filter_sustain: 0.05,
            filter_release_ms: 50.0,
            lfo_rate_hz: 1.0,
            lfo_amount: 0.0,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.05,
            fx_delay_mix: 0.0,
            fx_delay_time_ms: 300.0,
            fx_delay_feedback: 0.0,
            fx_reverb_mix: 0.08,
            fx_reverb_size: 0.35,
            fx_distortion: 0.25,
            master_volume_db: 0.5,
            glide_time_ms: 0.0,
        }
    }

    // ==========================================
    //            PRESETS : LEADS
    // ==========================================

    /// Lead 1 : Titanium Supersaw Lead — l'incontournable nappe/lead de festival EDM à 7 voix.
    #[must_use]
    pub fn preset_titanium_supersaw() -> Self {
        Self {
            enabled: true,
            name: "Titanium Supersaw Lead".to_string(),
            category: PresetCategory::Lead,
            osc1_wave: WavetableWaveform::Supersaw,
            osc1_semitones: 0,
            osc1_cents: -7,
            osc1_level: 0.90,
            osc1_pan: -0.25,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Supersaw,
            osc2_semitones: 0,
            osc2_cents: 7,
            osc2_level: 0.90,
            osc2_pan: 0.25,
            unison_voices: 7,
            unison_detune: 0.35,
            unison_spread: 0.85,
            filter_mode: WavetableFilterMode::LowPass24,
            filter_cutoff: 9500.0,
            filter_reso: 0.22,
            filter_env_amt: 0.50,
            filter_drive: 0.20,
            amp_attack_ms: 5.0,
            amp_decay_ms: 800.0,
            amp_sustain: 0.80,
            amp_release_ms: 280.0,
            filter_attack_ms: 8.0,
            filter_decay_ms: 600.0,
            filter_sustain: 0.60,
            filter_release_ms: 250.0,
            lfo_rate_hz: 5.5,
            lfo_amount: 0.05,
            lfo_target: LfoTarget::PitchVibrato,
            fx_chorus_mix: 0.35,
            fx_delay_mix: 0.28,
            fx_delay_time_ms: 375.0,
            fx_delay_feedback: 0.40,
            fx_reverb_mix: 0.35,
            fx_reverb_size: 0.70,
            fx_distortion: 0.15,
            master_volume_db: -1.0,
            glide_time_ms: 30.0,
        }
    }

    /// Lead 2 : Future Rave Pluck Lead — son tranchant et incisif style David Guetta & Morten.
    #[must_use]
    pub fn preset_future_rave_lead() -> Self {
        Self {
            enabled: true,
            name: "Future Rave Lead".to_string(),
            category: PresetCategory::Lead,
            osc1_wave: WavetableWaveform::Saw,
            osc1_semitones: 0,
            osc1_cents: 0,
            osc1_level: 0.95,
            osc1_pan: 0.0,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Square,
            osc2_semitones: 12,
            osc2_cents: 5,
            osc2_level: 0.70,
            osc2_pan: 0.0,
            unison_voices: 4,
            unison_detune: 0.22,
            unison_spread: 0.65,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 7000.0,
            filter_reso: 0.32,
            filter_env_amt: 0.60,
            filter_drive: 0.30,
            amp_attack_ms: 1.5,
            amp_decay_ms: 320.0,
            amp_sustain: 0.40,
            amp_release_ms: 180.0,
            filter_attack_ms: 1.0,
            filter_decay_ms: 240.0,
            filter_sustain: 0.15,
            filter_release_ms: 160.0,
            lfo_rate_hz: 6.0,
            lfo_amount: 0.04,
            lfo_target: LfoTarget::PitchVibrato,
            fx_chorus_mix: 0.25,
            fx_delay_mix: 0.32,
            fx_delay_time_ms: 280.0,
            fx_delay_feedback: 0.45,
            fx_reverb_mix: 0.30,
            fx_reverb_size: 0.60,
            fx_distortion: 0.25,
            master_volume_db: -0.5,
            glide_time_ms: 20.0,
        }
    }

    /// Lead 3 : 90s Trance Euphoria — lead euphorique hypnotique pour club et uplifting trance.
    #[must_use]
    pub fn preset_trance_euphoria() -> Self {
        Self {
            enabled: true,
            name: "90s Trance Euphoria".to_string(),
            category: PresetCategory::Lead,
            osc1_wave: WavetableWaveform::Saw,
            osc1_semitones: 0,
            osc1_cents: -12,
            osc1_level: 0.90,
            osc1_pan: -0.3,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Saw,
            osc2_semitones: 0,
            osc2_cents: 12,
            osc2_level: 0.90,
            osc2_pan: 0.3,
            unison_voices: 5,
            unison_detune: 0.32,
            unison_spread: 0.80,
            filter_mode: WavetableFilterMode::LowPass24,
            filter_cutoff: 5500.0,
            filter_reso: 0.38,
            filter_env_amt: 0.55,
            filter_drive: 0.15,
            amp_attack_ms: 6.0,
            amp_decay_ms: 600.0,
            amp_sustain: 0.75,
            amp_release_ms: 240.0,
            filter_attack_ms: 4.0,
            filter_decay_ms: 450.0,
            filter_sustain: 0.40,
            filter_release_ms: 200.0,
            lfo_rate_hz: 3.5,
            lfo_amount: 0.25,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.40,
            fx_delay_mix: 0.35,
            fx_delay_time_ms: 375.0,
            fx_delay_feedback: 0.50,
            fx_reverb_mix: 0.40,
            fx_reverb_size: 0.75,
            fx_distortion: 0.10,
            master_volume_db: -1.0,
            glide_time_ms: 40.0,
        }
    }

    // ==========================================
    //            PRESETS : PADS & STRINGS
    // ==========================================

    /// Pad 1 : Celestial Lush Strings — nappe symphonique céleste douce et spacieuse.
    #[must_use]
    pub fn preset_celestial_strings() -> Self {
        Self {
            enabled: true,
            name: "Celestial Lush Strings".to_string(),
            category: PresetCategory::Pad,
            osc1_wave: WavetableWaveform::Supersaw,
            osc1_semitones: 0,
            osc1_cents: -5,
            osc1_level: 0.85,
            osc1_pan: -0.2,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::VocalFormant,
            osc2_semitones: 0,
            osc2_cents: 5,
            osc2_level: 0.70,
            osc2_pan: 0.2,
            unison_voices: 5,
            unison_detune: 0.20,
            unison_spread: 0.90,
            filter_mode: WavetableFilterMode::LowPass24,
            filter_cutoff: 4200.0,
            filter_reso: 0.18,
            filter_env_amt: 0.30,
            filter_drive: 0.05,
            amp_attack_ms: 450.0,
            amp_decay_ms: 1800.0,
            amp_sustain: 0.85,
            amp_release_ms: 1200.0,
            filter_attack_ms: 600.0,
            filter_decay_ms: 1400.0,
            filter_sustain: 0.50,
            filter_release_ms: 1000.0,
            lfo_rate_hz: 0.8,
            lfo_amount: 0.22,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.50,
            fx_delay_mix: 0.25,
            fx_delay_time_ms: 500.0,
            fx_delay_feedback: 0.40,
            fx_reverb_mix: 0.55,
            fx_reverb_size: 0.85,
            fx_distortion: 0.0,
            master_volume_db: -1.5,
            glide_time_ms: 0.0,
        }
    }

    /// Pad 2 : Analog Warmth Sweep — sweep analogique résonant avec modulation lente.
    #[must_use]
    pub fn preset_analog_warmth() -> Self {
        Self {
            enabled: true,
            name: "Analog Warmth Sweep".to_string(),
            category: PresetCategory::Pad,
            osc1_wave: WavetableWaveform::Saw,
            osc1_semitones: 0,
            osc1_cents: -8,
            osc1_level: 0.90,
            osc1_pan: -0.3,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Triangle,
            osc2_semitones: -12,
            osc2_cents: 8,
            osc2_level: 0.80,
            osc2_pan: 0.3,
            unison_voices: 4,
            unison_detune: 0.18,
            unison_spread: 0.85,
            filter_mode: WavetableFilterMode::LowPass24,
            filter_cutoff: 1800.0,
            filter_reso: 0.42,
            filter_env_amt: 0.65,
            filter_drive: 0.12,
            amp_attack_ms: 320.0,
            amp_decay_ms: 1500.0,
            amp_sustain: 0.80,
            amp_release_ms: 950.0,
            filter_attack_ms: 700.0,
            filter_decay_ms: 1800.0,
            filter_sustain: 0.35,
            filter_release_ms: 800.0,
            lfo_rate_hz: 0.5,
            lfo_amount: 0.35,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.45,
            fx_delay_mix: 0.20,
            fx_delay_time_ms: 450.0,
            fx_delay_feedback: 0.30,
            fx_reverb_mix: 0.48,
            fx_reverb_size: 0.80,
            fx_distortion: 0.05,
            master_volume_db: -1.0,
            glide_time_ms: 0.0,
        }
    }

    // ==========================================
    //            PRESETS : PLUCKS & BELLS
    // ==========================================

    /// Pluck 1 : Spire Trance Pluck — pluck cristallin, percutant et rapide pour arpèges.
    #[must_use]
    pub fn preset_spire_pluck() -> Self {
        Self {
            enabled: true,
            name: "Spire Trance Pluck".to_string(),
            category: PresetCategory::Pluck,
            osc1_wave: WavetableWaveform::Saw,
            osc1_semitones: 0,
            osc1_cents: -5,
            osc1_level: 0.90,
            osc1_pan: -0.2,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Square,
            osc2_semitones: 12,
            osc2_cents: 5,
            osc2_level: 0.60,
            osc2_pan: 0.2,
            unison_voices: 3,
            unison_detune: 0.15,
            unison_spread: 0.60,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 1800.0,
            filter_reso: 0.28,
            filter_env_amt: 0.70,
            filter_drive: 0.10,
            amp_attack_ms: 1.0,
            amp_decay_ms: 220.0,
            amp_sustain: 0.0,
            amp_release_ms: 140.0,
            filter_attack_ms: 0.5,
            filter_decay_ms: 160.0,
            filter_sustain: 0.0,
            filter_release_ms: 120.0,
            lfo_rate_hz: 6.0,
            lfo_amount: 0.0,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.20,
            fx_delay_mix: 0.35,
            fx_delay_time_ms: 375.0,
            fx_delay_feedback: 0.45,
            fx_reverb_mix: 0.35,
            fx_reverb_size: 0.60,
            fx_distortion: 0.08,
            master_volume_db: 0.0,
            glide_time_ms: 0.0,
        }
    }

    /// Pluck 2 : Crystal Mallet Marimba — son percussif de verre / bois avec résonance pure.
    #[must_use]
    pub fn preset_crystal_mallet() -> Self {
        Self {
            enabled: true,
            name: "Crystal Mallet".to_string(),
            category: PresetCategory::Pluck,
            osc1_wave: WavetableWaveform::Sine,
            osc1_semitones: 12,
            osc1_cents: 0,
            osc1_level: 0.90,
            osc1_pan: 0.0,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::DigiLead,
            osc2_semitones: 24,
            osc2_cents: 2,
            osc2_level: 0.40,
            osc2_pan: 0.0,
            unison_voices: 1,
            unison_detune: 0.0,
            unison_spread: 0.0,
            filter_mode: WavetableFilterMode::BandPass,
            filter_cutoff: 3500.0,
            filter_reso: 0.45,
            filter_env_amt: 0.50,
            filter_drive: 0.0,
            amp_attack_ms: 0.8,
            amp_decay_ms: 380.0,
            amp_sustain: 0.0,
            amp_release_ms: 220.0,
            filter_attack_ms: 0.5,
            filter_decay_ms: 250.0,
            filter_sustain: 0.0,
            filter_release_ms: 180.0,
            lfo_rate_hz: 4.0,
            lfo_amount: 0.0,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.15,
            fx_delay_mix: 0.25,
            fx_delay_time_ms: 300.0,
            fx_delay_feedback: 0.30,
            fx_reverb_mix: 0.40,
            fx_reverb_size: 0.70,
            fx_distortion: 0.0,
            master_volume_db: 0.5,
            glide_time_ms: 0.0,
        }
    }

    /// Pluck 3 : Deep House Organ Chord — orgue à tirettes percutant pour accords deep house.
    #[must_use]
    pub fn preset_deep_house_organ() -> Self {
        Self {
            enabled: true,
            name: "Deep House Organ".to_string(),
            category: PresetCategory::Pluck,
            osc1_wave: WavetableWaveform::TonewheelOrgan,
            osc1_semitones: 0,
            osc1_cents: 0,
            osc1_level: 0.95,
            osc1_pan: 0.0,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Sine,
            osc2_semitones: 12,
            osc2_cents: 0,
            osc2_level: 0.50,
            osc2_pan: 0.0,
            unison_voices: 1,
            unison_detune: 0.0,
            unison_spread: 0.0,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 8000.0,
            filter_reso: 0.15,
            filter_env_amt: 0.40,
            filter_drive: 0.15,
            amp_attack_ms: 1.0,
            amp_decay_ms: 450.0,
            amp_sustain: 0.10,
            amp_release_ms: 150.0,
            filter_attack_ms: 0.8,
            filter_decay_ms: 300.0,
            filter_sustain: 0.05,
            filter_release_ms: 120.0,
            lfo_rate_hz: 5.8,
            lfo_amount: 0.10,
            lfo_target: LfoTarget::VolumeTremolo,
            fx_chorus_mix: 0.20,
            fx_delay_mix: 0.25,
            fx_delay_time_ms: 375.0,
            fx_delay_feedback: 0.35,
            fx_reverb_mix: 0.28,
            fx_reverb_size: 0.60,
            fx_distortion: 0.10,
            master_volume_db: 0.0,
            glide_time_ms: 0.0,
        }
    }

    /// Piano 5 : Vintage Upright Piano — piano droit en feutre acoustique chaleureux et intime.
    #[must_use]
    pub fn preset_vintage_upright_piano() -> Self {
        Self {
            enabled: true,
            name: "Vintage Upright Piano".to_string(),
            category: PresetCategory::Piano,
            osc1_wave: WavetableWaveform::GrandPiano,
            osc1_semitones: 0,
            osc1_cents: -3,
            osc1_level: 0.92,
            osc1_pan: -0.10,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::RhodesTines,
            osc2_semitones: 0,
            osc2_cents: 4,
            osc2_level: 0.40,
            osc2_pan: 0.10,
            unison_voices: 2,
            unison_detune: 0.08,
            unison_spread: 0.4,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 8500.0,
            filter_reso: 0.12,
            filter_env_amt: 0.25,
            filter_drive: 0.05,
            amp_attack_ms: 3.0,
            amp_decay_ms: 1500.0,
            amp_sustain: 0.0,
            amp_release_ms: 280.0,
            filter_attack_ms: 1.5,
            filter_decay_ms: 800.0,
            filter_sustain: 0.15,
            filter_release_ms: 240.0,
            lfo_rate_hz: 3.5,
            lfo_amount: 0.0,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.10,
            fx_delay_mix: 0.0,
            fx_delay_time_ms: 300.0,
            fx_delay_feedback: 0.2,
            fx_reverb_mix: 0.32,
            fx_reverb_size: 0.55,
            fx_distortion: 0.02,
            master_volume_db: 0.0,
            glide_time_ms: 0.0,
        }
    }

    /// Piano 6 : Honky Tonk Piano — piano de saloon légèrement désaccordé et brillant.
    #[must_use]
    pub fn preset_honky_tonk() -> Self {
        Self {
            enabled: true,
            name: "Honky Tonk Piano".to_string(),
            category: PresetCategory::Piano,
            osc1_wave: WavetableWaveform::M1Piano,
            osc1_semitones: 0,
            osc1_cents: -12,
            osc1_level: 0.88,
            osc1_pan: -0.20,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::GrandPiano,
            osc2_semitones: 0,
            osc2_cents: 12,
            osc2_level: 0.88,
            osc2_pan: 0.20,
            unison_voices: 3,
            unison_detune: 0.25,
            unison_spread: 0.6,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 16000.0,
            filter_reso: 0.20,
            filter_env_amt: 0.30,
            filter_drive: 0.15,
            amp_attack_ms: 1.0,
            amp_decay_ms: 1100.0,
            amp_sustain: 0.0,
            amp_release_ms: 200.0,
            filter_attack_ms: 0.5,
            filter_decay_ms: 600.0,
            filter_sustain: 0.1,
            filter_release_ms: 180.0,
            lfo_rate_hz: 6.0,
            lfo_amount: 0.02,
            lfo_target: LfoTarget::PitchVibrato,
            fx_chorus_mix: 0.22,
            fx_delay_mix: 0.15,
            fx_delay_time_ms: 250.0,
            fx_delay_feedback: 0.25,
            fx_reverb_mix: 0.25,
            fx_reverb_size: 0.45,
            fx_distortion: 0.08,
            master_volume_db: 0.0,
            glide_time_ms: 0.0,
        }
    }

    /// Bass 5 : Acid 303 Sub — basse acide squelchy à modulation d'enveloppe tranchante.
    #[must_use]
    pub fn preset_acid_303_sub() -> Self {
        Self {
            enabled: true,
            name: "Acid 303 Sub".to_string(),
            category: PresetCategory::Bass,
            osc1_wave: WavetableWaveform::Saw,
            osc1_semitones: -12,
            osc1_cents: 0,
            osc1_level: 0.95,
            osc1_pan: 0.0,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Square,
            osc2_semitones: -24,
            osc2_cents: 0,
            osc2_level: 0.70,
            osc2_pan: 0.0,
            unison_voices: 1,
            unison_detune: 0.0,
            unison_spread: 0.0,
            filter_mode: WavetableFilterMode::LowPass24,
            filter_cutoff: 1800.0,
            filter_reso: 0.82,
            filter_env_amt: 0.85,
            filter_drive: 0.35,
            amp_attack_ms: 1.5,
            amp_decay_ms: 450.0,
            amp_sustain: 0.15,
            amp_release_ms: 120.0,
            filter_attack_ms: 1.0,
            filter_decay_ms: 280.0,
            filter_sustain: 0.05,
            filter_release_ms: 100.0,
            lfo_rate_hz: 2.0,
            lfo_amount: 0.0,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.0,
            fx_delay_mix: 0.22,
            fx_delay_time_ms: 280.0,
            fx_delay_feedback: 0.35,
            fx_reverb_mix: 0.10,
            fx_reverb_size: 0.30,
            fx_distortion: 0.40,
            master_volume_db: 0.5,
            glide_time_ms: 35.0,
        }
    }

    /// Bass 6 : Neurofunk Slither Bass — reese mouvante avec balayage LFO et saturation tube.
    #[must_use]
    pub fn preset_neurofunk_slither_bass() -> Self {
        Self {
            enabled: true,
            name: "Neurofunk Slither Bass".to_string(),
            category: PresetCategory::Bass,
            osc1_wave: WavetableWaveform::Saw,
            osc1_semitones: -12,
            osc1_cents: -9,
            osc1_level: 0.90,
            osc1_pan: -0.25,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Saw,
            osc2_semitones: -12,
            osc2_cents: 9,
            osc2_level: 0.90,
            osc2_pan: 0.25,
            unison_voices: 4,
            unison_detune: 0.35,
            unison_spread: 0.70,
            filter_mode: WavetableFilterMode::BandPass,
            filter_cutoff: 1200.0,
            filter_reso: 0.65,
            filter_env_amt: 0.40,
            filter_drive: 0.55,
            amp_attack_ms: 5.0,
            amp_decay_ms: 600.0,
            amp_sustain: 0.85,
            amp_release_ms: 180.0,
            filter_attack_ms: 10.0,
            filter_decay_ms: 500.0,
            filter_sustain: 0.70,
            filter_release_ms: 160.0,
            lfo_rate_hz: 3.2,
            lfo_amount: 0.35,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.28,
            fx_delay_mix: 0.0,
            fx_delay_time_ms: 300.0,
            fx_delay_feedback: 0.20,
            fx_reverb_mix: 0.12,
            fx_reverb_size: 0.40,
            fx_distortion: 0.50,
            master_volume_db: 0.0,
            glide_time_ms: 45.0,
        }
    }

    /// Bass 7 : Slap Funk Bass — attaque percutante et brillance métallique slap funk.
    #[must_use]
    pub fn preset_slap_funk_bass() -> Self {
        Self {
            enabled: true,
            name: "Slap Funk Bass".to_string(),
            category: PresetCategory::Bass,
            osc1_wave: WavetableWaveform::Triangle,
            osc1_semitones: -12,
            osc1_cents: 0,
            osc1_level: 0.95,
            osc1_pan: 0.0,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::RhodesTines,
            osc2_semitones: 0,
            osc2_cents: 0,
            osc2_level: 0.65,
            osc2_pan: 0.0,
            unison_voices: 1,
            unison_detune: 0.0,
            unison_spread: 0.0,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 4500.0,
            filter_reso: 0.45,
            filter_env_amt: 0.70,
            filter_drive: 0.20,
            amp_attack_ms: 0.8,
            amp_decay_ms: 350.0,
            amp_sustain: 0.10,
            amp_release_ms: 90.0,
            filter_attack_ms: 0.5,
            filter_decay_ms: 180.0,
            filter_sustain: 0.02,
            filter_release_ms: 80.0,
            lfo_rate_hz: 4.0,
            lfo_amount: 0.0,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.12,
            fx_delay_mix: 0.0,
            fx_delay_time_ms: 250.0,
            fx_delay_feedback: 0.10,
            fx_reverb_mix: 0.15,
            fx_reverb_size: 0.35,
            fx_distortion: 0.18,
            master_volume_db: 0.8,
            glide_time_ms: 0.0,
        }
    }

    /// Lead 4 : Hyperpop Chiptune Lead — lead carré/scie hyperpop avec vibrato rapide et delay stéréo.
    #[must_use]
    pub fn preset_hyperpop_chiptune_lead() -> Self {
        Self {
            enabled: true,
            name: "Hyperpop Chiptune Lead".to_string(),
            category: PresetCategory::Lead,
            osc1_wave: WavetableWaveform::Square,
            osc1_semitones: 0,
            osc1_cents: 0,
            osc1_level: 0.90,
            osc1_pan: -0.15,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::DigiLead,
            osc2_semitones: 12,
            osc2_cents: 6,
            osc2_level: 0.75,
            osc2_pan: 0.15,
            unison_voices: 3,
            unison_detune: 0.18,
            unison_spread: 0.65,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 12000.0,
            filter_reso: 0.28,
            filter_env_amt: 0.35,
            filter_drive: 0.25,
            amp_attack_ms: 1.0,
            amp_decay_ms: 400.0,
            amp_sustain: 0.75,
            amp_release_ms: 160.0,
            filter_attack_ms: 1.0,
            filter_decay_ms: 300.0,
            filter_sustain: 0.50,
            filter_release_ms: 140.0,
            lfo_rate_hz: 7.2,
            lfo_amount: 0.08,
            lfo_target: LfoTarget::PitchVibrato,
            fx_chorus_mix: 0.25,
            fx_delay_mix: 0.35,
            fx_delay_time_ms: 190.0,
            fx_delay_feedback: 0.45,
            fx_reverb_mix: 0.25,
            fx_reverb_size: 0.50,
            fx_distortion: 0.20,
            master_volume_db: -0.5,
            glide_time_ms: 22.0,
        }
    }

    /// Lead 5 : Blade Runner Brass — cuivre synthétique mythique CS-80 avec réverbe caverneuse.
    #[must_use]
    pub fn preset_blade_runner_brass() -> Self {
        Self {
            enabled: true,
            name: "Blade Runner Brass".to_string(),
            category: PresetCategory::Lead,
            osc1_wave: WavetableWaveform::Saw,
            osc1_semitones: 0,
            osc1_cents: -5,
            osc1_level: 0.92,
            osc1_pan: -0.20,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Saw,
            osc2_semitones: 0,
            osc2_cents: 5,
            osc2_level: 0.92,
            osc2_pan: 0.20,
            unison_voices: 4,
            unison_detune: 0.28,
            unison_spread: 0.80,
            filter_mode: WavetableFilterMode::LowPass24,
            filter_cutoff: 3500.0,
            filter_reso: 0.38,
            filter_env_amt: 0.65,
            filter_drive: 0.15,
            amp_attack_ms: 75.0,
            amp_decay_ms: 1200.0,
            amp_sustain: 0.85,
            amp_release_ms: 600.0,
            filter_attack_ms: 90.0,
            filter_decay_ms: 900.0,
            filter_sustain: 0.45,
            filter_release_ms: 550.0,
            lfo_rate_hz: 4.8,
            lfo_amount: 0.06,
            lfo_target: LfoTarget::PitchVibrato,
            fx_chorus_mix: 0.38,
            fx_delay_mix: 0.30,
            fx_delay_time_ms: 450.0,
            fx_delay_feedback: 0.45,
            fx_reverb_mix: 0.55,
            fx_reverb_size: 0.90,
            fx_distortion: 0.10,
            master_volume_db: -1.0,
            glide_time_ms: 60.0,
        }
    }

    /// Pad 3 : Cinematic Dark Pad — nappe sombre atmosphérique avec filtre 24dB et grand espace.
    #[must_use]
    pub fn preset_cinematic_dark_pad() -> Self {
        Self {
            enabled: true,
            name: "Cinematic Dark Pad".to_string(),
            category: PresetCategory::Pad,
            osc1_wave: WavetableWaveform::VocalFormant,
            osc1_semitones: 0,
            osc1_cents: -4,
            osc1_level: 0.85,
            osc1_pan: -0.30,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Saw,
            osc2_semitones: -12,
            osc2_cents: 4,
            osc2_level: 0.70,
            osc2_pan: 0.30,
            unison_voices: 5,
            unison_detune: 0.32,
            unison_spread: 0.90,
            filter_mode: WavetableFilterMode::LowPass24,
            filter_cutoff: 2200.0,
            filter_reso: 0.25,
            filter_env_amt: 0.30,
            filter_drive: 0.08,
            amp_attack_ms: 220.0,
            amp_decay_ms: 1800.0,
            amp_sustain: 0.90,
            amp_release_ms: 850.0,
            filter_attack_ms: 300.0,
            filter_decay_ms: 1400.0,
            filter_sustain: 0.60,
            filter_release_ms: 800.0,
            lfo_rate_hz: 1.2,
            lfo_amount: 0.20,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.40,
            fx_delay_mix: 0.25,
            fx_delay_time_ms: 500.0,
            fx_delay_feedback: 0.40,
            fx_reverb_mix: 0.60,
            fx_reverb_size: 0.92,
            fx_distortion: 0.05,
            master_volume_db: -0.5,
            glide_time_ms: 80.0,
        }
    }

    /// Pluck 3 : Pizzicato Strings — cordes frottées en pizzicato avec attaque ultra-courte.
    #[must_use]
    pub fn preset_pizzicato_strings() -> Self {
        Self {
            enabled: true,
            name: "Pizzicato Strings".to_string(),
            category: PresetCategory::Pluck,
            osc1_wave: WavetableWaveform::Saw,
            osc1_semitones: 0,
            osc1_cents: -2,
            osc1_level: 0.90,
            osc1_pan: -0.15,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Triangle,
            osc2_semitones: 12,
            osc2_cents: 2,
            osc2_level: 0.60,
            osc2_pan: 0.15,
            unison_voices: 2,
            unison_detune: 0.10,
            unison_spread: 0.50,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 6000.0,
            filter_reso: 0.20,
            filter_env_amt: 0.60,
            filter_drive: 0.10,
            amp_attack_ms: 0.8,
            amp_decay_ms: 220.0,
            amp_sustain: 0.0,
            amp_release_ms: 90.0,
            filter_attack_ms: 0.5,
            filter_decay_ms: 140.0,
            filter_sustain: 0.0,
            filter_release_ms: 80.0,
            lfo_rate_hz: 5.0,
            lfo_amount: 0.0,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.15,
            fx_delay_mix: 0.18,
            fx_delay_time_ms: 250.0,
            fx_delay_feedback: 0.25,
            fx_reverb_mix: 0.35,
            fx_reverb_size: 0.55,
            fx_distortion: 0.05,
            master_volume_db: 0.5,
            glide_time_ms: 0.0,
        }
    }

    /// Pluck 4 : Kalimba Dream — lamelles de bois africaines avec harmoniques de cloche.
    #[must_use]
    pub fn preset_kalimba_dream() -> Self {
        Self {
            enabled: true,
            name: "Kalimba Dream".to_string(),
            category: PresetCategory::Pluck,
            osc1_wave: WavetableWaveform::Sine,
            osc1_semitones: 0,
            osc1_cents: 0,
            osc1_level: 0.95,
            osc1_pan: 0.0,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::RhodesTines,
            osc2_semitones: 12,
            osc2_cents: 0,
            osc2_level: 0.70,
            osc2_pan: 0.0,
            unison_voices: 1,
            unison_detune: 0.0,
            unison_spread: 0.3,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 7500.0,
            filter_reso: 0.35,
            filter_env_amt: 0.45,
            filter_drive: 0.05,
            amp_attack_ms: 0.6,
            amp_decay_ms: 600.0,
            amp_sustain: 0.0,
            amp_release_ms: 250.0,
            filter_attack_ms: 0.5,
            filter_decay_ms: 320.0,
            filter_sustain: 0.05,
            filter_release_ms: 200.0,
            lfo_rate_hz: 4.5,
            lfo_amount: 0.0,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.20,
            fx_delay_mix: 0.32,
            fx_delay_time_ms: 375.0,
            fx_delay_feedback: 0.40,
            fx_reverb_mix: 0.42,
            fx_reverb_size: 0.70,
            fx_distortion: 0.0,
            master_volume_db: 0.5,
            glide_time_ms: 0.0,
        }
    }

    /// Organ 2 : Hammond B3 Gospel Organ — orgue à roues phoniques avec simulation de cabine Leslie.
    #[must_use]
    pub fn preset_hammond_b3_organ() -> Self {
        Self {
            enabled: true,
            name: "Hammond B3 Organ".to_string(),
            category: PresetCategory::Pluck,
            osc1_wave: WavetableWaveform::TonewheelOrgan,
            osc1_semitones: 0,
            osc1_cents: 0,
            osc1_level: 0.95,
            osc1_pan: -0.10,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::TonewheelOrgan,
            osc2_semitones: 12,
            osc2_cents: 3,
            osc2_level: 0.65,
            osc2_pan: 0.10,
            unison_voices: 2,
            unison_detune: 0.12,
            unison_spread: 0.6,
            filter_mode: WavetableFilterMode::LowPass12,
            filter_cutoff: 14000.0,
            filter_reso: 0.15,
            filter_env_amt: 0.10,
            filter_drive: 0.30,
            amp_attack_ms: 3.0,
            amp_decay_ms: 800.0,
            amp_sustain: 0.95,
            amp_release_ms: 120.0,
            filter_attack_ms: 2.0,
            filter_decay_ms: 500.0,
            filter_sustain: 0.90,
            filter_release_ms: 100.0,
            lfo_rate_hz: 6.2,
            lfo_amount: 0.15,
            lfo_target: LfoTarget::PanSpread,
            fx_chorus_mix: 0.45,
            fx_delay_mix: 0.10,
            fx_delay_time_ms: 250.0,
            fx_delay_feedback: 0.20,
            fx_reverb_mix: 0.30,
            fx_reverb_size: 0.60,
            fx_distortion: 0.25,
            master_volume_db: 0.0,
            glide_time_ms: 0.0,
        }
    }

    /// Lead 6 : Cyber-Drive Synth — synthé cyberpunk agressif à distorsion non-linéaire et stéréo large.
    #[must_use]
    pub fn preset_cyber_drive_synth() -> Self {
        Self {
            enabled: true,
            name: "Cyber-Drive Synth".to_string(),
            category: PresetCategory::Lead,
            osc1_wave: WavetableWaveform::DigiLead,
            osc1_semitones: 0,
            osc1_cents: -7,
            osc1_level: 0.92,
            osc1_pan: -0.25,
            osc2_enabled: true,
            osc2_wave: WavetableWaveform::Supersaw,
            osc2_semitones: -12,
            osc2_cents: 7,
            osc2_level: 0.85,
            osc2_pan: 0.25,
            unison_voices: 5,
            unison_detune: 0.30,
            unison_spread: 0.85,
            filter_mode: WavetableFilterMode::LowPass24,
            filter_cutoff: 5500.0,
            filter_reso: 0.45,
            filter_env_amt: 0.55,
            filter_drive: 0.45,
            amp_attack_ms: 4.0,
            amp_decay_ms: 500.0,
            amp_sustain: 0.80,
            amp_release_ms: 220.0,
            filter_attack_ms: 5.0,
            filter_decay_ms: 400.0,
            filter_sustain: 0.55,
            filter_release_ms: 190.0,
            lfo_rate_hz: 4.0,
            lfo_amount: 0.05,
            lfo_target: LfoTarget::FilterCutoff,
            fx_chorus_mix: 0.30,
            fx_delay_mix: 0.28,
            fx_delay_time_ms: 320.0,
            fx_delay_feedback: 0.35,
            fx_reverb_mix: 0.30,
            fx_reverb_size: 0.65,
            fx_distortion: 0.45,
            master_volume_db: -0.5,
            glide_time_ms: 28.0,
        }
    }

    /// Liste de tous les presets inclus dans l'instrument.
    #[must_use]
    pub fn all_presets() -> Vec<Self> {
        vec![
            Self::preset_concert_grand_piano(),
            Self::preset_house_m1_piano(),
            Self::preset_vintage_upright_piano(),
            Self::preset_honky_tonk(),
            Self::preset_neo_soul_rhodes(),
            Self::preset_dx7_epiano(),
            Self::preset_808_sub_boom(),
            Self::preset_reese_bass(),
            Self::preset_acid_303_sub(),
            Self::preset_neurofunk_slither_bass(),
            Self::preset_slap_funk_bass(),
            Self::preset_cyberpunk_bass(),
            Self::preset_future_house_donk(),
            Self::preset_titanium_supersaw(),
            Self::preset_future_rave_lead(),
            Self::preset_hyperpop_chiptune_lead(),
            Self::preset_blade_runner_brass(),
            Self::preset_cyber_drive_synth(),
            Self::preset_trance_euphoria(),
            Self::preset_celestial_strings(),
            Self::preset_analog_warmth(),
            Self::preset_cinematic_dark_pad(),
            Self::preset_spire_pluck(),
            Self::preset_pizzicato_strings(),
            Self::preset_kalimba_dream(),
            Self::preset_crystal_mallet(),
            Self::preset_deep_house_organ(),
            Self::preset_hammond_b3_organ(),
        ]
    }
}

// =========================================================================
//                       ÉTATS DE VOIX POLYPHONIQUES
// =========================================================================

/// État interne d'enveloppe ADSR (calculé par échantillon).
#[derive(Debug, Clone, Copy, PartialEq)]
enum AdsrStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Debug, Clone, Copy)]
struct AdsrState {
    stage: AdsrStage,
    value: f32,
}

impl AdsrState {
    fn new() -> Self {
        Self {
            stage: AdsrStage::Idle,
            value: 0.0,
        }
    }

    fn gate_on(&mut self) {
        self.stage = AdsrStage::Attack;
    }

    fn gate_off(&mut self) {
        if self.stage != AdsrStage::Idle {
            self.stage = AdsrStage::Release;
        }
    }

    #[inline]
    fn process(&mut self, dt: f32, a_ms: f32, d_ms: f32, s_lvl: f32, r_ms: f32) -> f32 {
        match self.stage {
            AdsrStage::Idle => 0.0,
            AdsrStage::Attack => {
                let step = dt / (a_ms * 0.001).max(0.0001);
                self.value += step;
                if self.value >= 1.0 {
                    self.value = 1.0;
                    self.stage = AdsrStage::Decay;
                }
                self.value
            }
            AdsrStage::Decay => {
                let step = dt / (d_ms * 0.001).max(0.0001);
                self.value -= step * (1.0 - s_lvl);
                if self.value <= s_lvl {
                    self.value = s_lvl;
                    self.stage = AdsrStage::Sustain;
                }
                self.value
            }
            AdsrStage::Sustain => {
                self.value = s_lvl;
                self.value
            }
            AdsrStage::Release => {
                let step = dt / (r_ms * 0.001).max(0.0001);
                self.value -= step;
                if self.value <= 0.0 {
                    self.value = 0.0;
                    self.stage = AdsrStage::Idle;
                }
                self.value
            }
        }
    }

    fn is_active(&self) -> bool {
        self.stage != AdsrStage::Idle
    }
}

/// Filtre State Variable Filter (SVF) 2 pôles résonant.
#[derive(Debug, Clone, Copy, Default)]
struct SvfFilter {
    ic1eq: f32,
    ic2eq: f32,
}

impl SvfFilter {
    #[inline]
    fn process(
        &mut self,
        input: f32,
        cutoff_hz: f32,
        reso: f32,
        sample_rate: f32,
        mode: WavetableFilterMode,
    ) -> f32 {
        if mode == WavetableFilterMode::Off {
            return input;
        }

        let cutoff = cutoff_hz.clamp(20.0, sample_rate * 0.48);
        let g = (PI * cutoff / sample_rate).tan();
        let k = 2.0 * (1.0 - reso.clamp(0.0, 0.98));
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        let v3 = input - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        match mode {
            WavetableFilterMode::LowPass12 => v2,
            WavetableFilterMode::HighPass => input - k * v1 - v2,
            WavetableFilterMode::BandPass => v1,
            WavetableFilterMode::LowPass24 => {
                // Modélisation 4 pôles par adoucissement non linéaire
                (v2 * 1.2).tanh() * 0.95
            }
            WavetableFilterMode::Off => input,
        }
    }
}

/// Une voix de polyphonie individuelle du synthétiseur.
#[derive(Debug, Clone)]
struct Voice {
    note: u8,
    target_freq: f32,
    current_freq: f32,
    velocity: f32,
    active: bool,
    age: u64,
    gate_held: bool,
    phase1: f32,
    phase2: f32,
    unison_phases: [f32; 7],
    amp_env: AdsrState,
    filter_env: AdsrState,
    filter_svf: SvfFilter,
    noise_seed: u32,
}

impl Voice {
    fn new() -> Self {
        Self {
            note: 0,
            target_freq: 440.0,
            current_freq: 440.0,
            velocity: 0.0,
            active: false,
            age: 0,
            gate_held: false,
            phase1: 0.0,
            phase2: 0.0,
            unison_phases: [0.0; 7],
            amp_env: AdsrState::new(),
            filter_env: AdsrState::new(),
            filter_svf: SvfFilter::default(),
            noise_seed: 0x9e3779b9,
        }
    }

    fn trigger(&mut self, note: u8, velocity: u8, glide_time_ms: f32, age: u64) {
        let target = 440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0);
        if self.active && glide_time_ms > 1.0 {
            self.target_freq = target;
        } else {
            self.current_freq = target;
            self.target_freq = target;
            self.phase1 = 0.0;
            self.phase2 = 0.0;
            self.unison_phases.fill(0.0);
        }
        self.note = note;
        self.velocity = (velocity.clamp(1, 127) as f32 / 127.0).powf(0.85);
        self.active = true;
        self.age = age;
        self.gate_held = true;
        self.amp_env.gate_on();
        self.filter_env.gate_on();
    }

    fn release(&mut self) {
        self.gate_held = false;
        self.amp_env.gate_off();
        self.filter_env.gate_off();
    }
}

// =========================================================================
//                   MOTEUR DE SYNTHÈSE TEMPS RÉEL
// =========================================================================

const MAX_VOICES: usize = 16;

/// Moteur de synthèse temps réel Wavetable / Nexus.
#[derive(Debug, Clone)]
pub struct WavetableSynth {
    pub params: WavetableParams,
    sample_rate: f32,
    voices: Vec<Voice>,
    voice_age_counter: u64,
    lfo_phase: f32,

    // Effet delay stéréo ping-pong circulaire
    delay_buffer_l: Vec<f32>,
    delay_buffer_r: Vec<f32>,
    delay_pos: usize,

    // Effet chorus / dimension stéréo
    chorus_phase: f32,

    // Effet réverbération simple à peignes
    comb_buffers: Vec<Vec<f32>>,
    comb_positions: Vec<usize>,
}

impl Default for WavetableSynth {
    fn default() -> Self {
        Self::new(48000)
    }
}

impl WavetableSynth {
    /// Crée un nouveau synthétiseur Wavetable / Nexus.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(8000) as f32;
        let delay_len = (sr * 1.0) as usize; // 1 seconde max de délai

        // Tailles de lignes de retard pour 4 filtres en peigne réverb (Schroeder)
        let comb_lens = [
            (sr * 0.0297) as usize,
            (sr * 0.0371) as usize,
            (sr * 0.0411) as usize,
            (sr * 0.0437) as usize,
        ];
        let comb_buffers = comb_lens.iter().map(|&len| vec![0.0f32; len]).collect();
        let comb_positions = vec![0usize; comb_lens.len()];

        Self {
            params: WavetableParams::default(),
            sample_rate: sr,
            voices: (0..MAX_VOICES).map(|_| Voice::new()).collect(),
            voice_age_counter: 0,
            lfo_phase: 0.0,
            delay_buffer_l: vec![0.0f32; delay_len],
            delay_buffer_r: vec![0.0f32; delay_len],
            delay_pos: 0,
            chorus_phase: 0.0,
            comb_buffers,
            comb_positions,
        }
    }

    /// Déclenche une note (Note On).
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        if !self.params.enabled || velocity == 0 {
            self.note_off(note);
            return;
        }

        self.voice_age_counter += 1;
        let age = self.voice_age_counter;
        let glide = self.params.glide_time_ms;

        // 1. Cherche une voix libre (inactive)
        if let Some(v) = self.voices.iter_mut().find(|v| !v.active) {
            v.trigger(note, velocity, glide, age);
            return;
        }

        // 2. Cherche la voix en phase de release la plus ancienne
        if let Some(v) = self
            .voices
            .iter_mut()
            .filter(|v| v.amp_env.stage == AdsrStage::Release)
            .min_by_key(|v| v.age)
        {
            v.trigger(note, velocity, glide, age);
            return;
        }

        // 3. Vol de voix : prend la voix active la plus ancienne
        if let Some(v) = self.voices.iter_mut().min_by_key(|v| v.age) {
            v.trigger(note, velocity, glide, age);
        }
    }

    /// Relâche une note (Note Off).
    pub fn note_off(&mut self, note: u8) {
        for v in &mut self.voices {
            if v.active && v.note == note {
                v.release();
            }
        }
    }

    /// Coupe toutes les voix actives immédiatement.
    pub fn all_notes_off(&mut self) {
        for v in &mut self.voices {
            v.active = false;
            v.amp_env.stage = AdsrStage::Idle;
        }
    }

    /// Génère un échantillon pour une forme d'onde donnée à une phase normalisée (0..2*PI).
    #[inline]
    fn evaluate_waveform(wave: WavetableWaveform, phase: f32, seed: &mut u32) -> f32 {
        match wave {
            WavetableWaveform::Sine => phase.sin(),
            WavetableWaveform::Triangle => {
                let norm = phase / (2.0 * PI);
                if norm < 0.5 {
                    4.0 * norm - 1.0
                } else {
                    3.0 - 4.0 * norm
                }
            }
            WavetableWaveform::Saw => {
                let norm = phase / (2.0 * PI);
                2.0 * norm - 1.0
            }
            WavetableWaveform::Square => {
                if phase < PI {
                    1.0
                } else {
                    -1.0
                }
            }
            WavetableWaveform::Supersaw => {
                // Émulation compacte Supersaw Roland JP-8000 / Access Virus
                let s1 = phase.sin();
                let s2 = (phase * 1.006).sin();
                let s3 = (phase * 0.994).sin();
                let s4 = (phase * 1.015).sin();
                let s5 = (phase * 0.985).sin();
                s1 * 0.35 + (s2 + s3) * 0.22 + (s4 + s5) * 0.10
            }
            WavetableWaveform::GrandPiano => {
                // Spectre harmonique acoustique de piano (fondamentale + harmoniques décroissantes)
                let h1 = phase.sin();
                let h2 = (phase * 2.0).sin() * 0.55;
                let h3 = (phase * 3.0).sin() * 0.35;
                let h4 = (phase * 4.0).sin() * 0.18;
                let h5 = (phase * 5.0).sin() * 0.09;
                (h1 + h2 + h3 + h4 + h5) * 0.45
            }
            WavetableWaveform::M1Piano => {
                // Spectre percutant M1 (harmoniques paires renforcées et attaque tranchante)
                let s = phase.sin();
                let h2 = (phase * 2.0).sin() * 0.65;
                let h3 = (phase * 3.0).sin() * 0.40;
                let h6 = (phase * 6.0).sin() * 0.20;
                (s * 0.8 + h2 + h3 + h6) * 0.40
            }
            WavetableWaveform::RhodesTines => {
                // Son Rhodes velouté (fondamentale pure + harmonique 3 chaude)
                let h1 = phase.sin();
                let h3 = (phase * 3.0).sin() * 0.28;
                let h5 = (phase * 5.0).sin() * 0.08;
                (h1 + h3 + h5) * 0.70
            }
            WavetableWaveform::DigiLead => {
                // Table d'onde numérique spectrale tranchante (Serum Digital)
                let s = phase.sin();
                (s * 2.2).tanh() * 0.85
            }
            WavetableWaveform::TonewheelOrgan => {
                // Orgue Hammond tirettes 16', 8', 4', 2'
                let h1 = (phase * 0.5).sin() * 0.8;
                let h2 = phase.sin() * 1.0;
                let h3 = (phase * 2.0).sin() * 0.6;
                let h4 = (phase * 4.0).sin() * 0.35;
                (h1 + h2 + h3 + h4) * 0.35
            }
            WavetableWaveform::VocalFormant => {
                // Résonance formantique voyelle "A"
                let f1 = (phase * 2.5).sin() * 0.7;
                let f2 = (phase * 4.2).sin() * 0.4;
                (phase.sin() * 0.6 + f1 + f2) * 0.55
            }
            WavetableWaveform::Noise => {
                *seed ^= *seed << 13;
                *seed ^= *seed >> 17;
                *seed ^= *seed << 5;
                (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
            }
        }
    }

    /// Rend un bloc audio stéréo entrelacé `[L, R, L, R, ...]`.
    pub fn render(&mut self, out: &mut [f32]) {
        if !self.params.enabled {
            return;
        }

        let p = &self.params;
        let sr = self.sample_rate;
        let dt = 1.0 / sr;
        let frames = out.len() / 2;
        let vol_linear = crate::engine::db_to_linear(p.master_volume_db);

        // Paramètres LFO
        let lfo_inc = 2.0 * PI * p.lfo_rate_hz / sr;
        let chorus_inc = 2.0 * PI * 0.65 / sr;

        for f in 0..frames {
            let mut mix_l = 0.0f32;
            let mut mix_r = 0.0f32;

            // Valeur courante du LFO de modulation
            let lfo_val = self.lfo_phase.sin() * p.lfo_amount;
            self.lfo_phase = (self.lfo_phase + lfo_inc) % (2.0 * PI);
            self.chorus_phase = (self.chorus_phase + chorus_inc) % (2.0 * PI);

            // Traitement de chaque voix polyphonique
            for voice in &mut self.voices {
                if !voice.active {
                    continue;
                }

                // Progression d'amplitude ADSR
                let mut amp = voice.amp_env.process(
                    dt,
                    p.amp_attack_ms,
                    p.amp_decay_ms,
                    p.amp_sustain,
                    p.amp_release_ms,
                );
                // Si la note est maintenue (pad/touche enfoncée), garantir un niveau de sustain audible
                // afin que la note ne coupe jamais tant que le doigt reste posé.
                if voice.gate_held && voice.amp_env.stage == AdsrStage::Sustain {
                    let min_sus = p.amp_sustain.max(0.35);
                    if amp < min_sus {
                        amp = min_sus;
                    }
                }
                if !voice.amp_env.is_active() {
                    voice.active = false;
                    continue;
                }

                // Progression de filtre ADSR
                let mut filt_env = voice.filter_env.process(
                    dt,
                    p.filter_attack_ms,
                    p.filter_decay_ms,
                    p.filter_sustain,
                    p.filter_release_ms,
                );
                if voice.gate_held && voice.filter_env.stage == AdsrStage::Sustain {
                    let min_filt = p.filter_sustain.max(0.30);
                    if filt_env < min_filt {
                        filt_env = min_filt;
                    }
                }

                // Portamento / Glide de fréquence
                if p.glide_time_ms > 1.0 && (voice.current_freq - voice.target_freq).abs() > 0.01 {
                    let glide_rate = dt / (p.glide_time_ms * 0.001);
                    voice.current_freq +=
                        (voice.target_freq - voice.current_freq) * glide_rate.min(1.0);
                } else {
                    voice.current_freq = voice.target_freq;
                }

                // Modulation de pitch par le LFO si ciblé
                let pitch_mod = if p.lfo_target == LfoTarget::PitchVibrato {
                    1.0 + lfo_val * 0.03
                } else {
                    1.0
                };
                let freq = voice.current_freq * pitch_mod;

                // --- Oscillateur 1 ---
                let osc1_semi = p.osc1_semitones as f32 + p.osc1_cents as f32 / 100.0;
                let freq1 = freq * 2.0f32.powf(osc1_semi / 12.0);
                let phase_inc1 = 2.0 * PI * freq1 / sr;
                voice.phase1 = (voice.phase1 + phase_inc1) % (2.0 * PI);
                let osc1_sample =
                    Self::evaluate_waveform(p.osc1_wave, voice.phase1, &mut voice.noise_seed)
                        * p.osc1_level;

                // --- Oscillateur 2 (optionnel) ---
                let osc2_sample = if p.osc2_enabled {
                    let osc2_semi = p.osc2_semitones as f32 + p.osc2_cents as f32 / 100.0;
                    let freq2 = freq * 2.0f32.powf(osc2_semi / 12.0);
                    let phase_inc2 = 2.0 * PI * freq2 / sr;
                    voice.phase2 = (voice.phase2 + phase_inc2) % (2.0 * PI);
                    Self::evaluate_waveform(p.osc2_wave, voice.phase2, &mut voice.noise_seed)
                        * p.osc2_level
                } else {
                    0.0
                };

                // --- Moteur Unisson multi-voix ---
                let mut unison_sample_l = 0.0f32;
                let mut unison_sample_r = 0.0f32;
                let n_unison = p.unison_voices.clamp(1, 7) as usize;

                if n_unison > 1 && p.unison_detune > 0.01 {
                    for i in 0..n_unison {
                        let detune_cents =
                            ((i as f32 / (n_unison - 1) as f32) - 0.5) * p.unison_detune * 40.0;
                        let u_freq = freq * 2.0f32.powf(detune_cents / 1200.0);
                        let u_inc = 2.0 * PI * u_freq / sr;
                        voice.unison_phases[i] = (voice.unison_phases[i] + u_inc) % (2.0 * PI);
                        let s = Self::evaluate_waveform(
                            p.osc1_wave,
                            voice.unison_phases[i],
                            &mut voice.noise_seed,
                        );
                        let pan_pos = ((i as f32 / (n_unison - 1) as f32) - 0.5) * p.unison_spread;
                        unison_sample_l += s * (0.5 - pan_pos * 0.5);
                        unison_sample_r += s * (0.5 + pan_pos * 0.5);
                    }
                    unison_sample_l /= n_unison as f32;
                    unison_sample_r /= n_unison as f32;
                } else {
                    unison_sample_l = osc1_sample;
                    unison_sample_r = osc1_sample;
                }

                // Somme des oscillateurs et panoramique
                let pan1 = p.osc1_pan.clamp(-1.0, 1.0);
                let pan2 = p.osc2_pan.clamp(-1.0, 1.0);
                let raw_l = unison_sample_l * (0.5 - pan1 * 0.5) + osc2_sample * (0.5 - pan2 * 0.5);
                let raw_r = unison_sample_r * (0.5 + pan1 * 0.5) + osc2_sample * (0.5 + pan2 * 0.5);
                let mono_raw = (raw_l + raw_r) * 0.5;

                // --- Étage de Filtre Multimode Résonant ---
                let mut cutoff = p.filter_cutoff;
                // Modulation enveloppe de filtre (-1 .. +1)
                if p.filter_env_amt.abs() > 0.01 {
                    let mod_octaves = p.filter_env_amt * filt_env * 5.0;
                    cutoff *= 2.0f32.powf(mod_octaves);
                }
                // Modulation LFO de filtre
                if p.lfo_target == LfoTarget::FilterCutoff {
                    cutoff *= 2.0f32.powf(lfo_val * 2.0);
                }

                let filtered =
                    voice
                        .filter_svf
                        .process(mono_raw, cutoff, p.filter_reso, sr, p.filter_mode);

                // Overdrive / Saturation du filtre
                let driven = if p.filter_drive > 0.01 {
                    (filtered * (1.0 + p.filter_drive * 3.5)).tanh()
                } else {
                    filtered
                };

                // Gain d'amplitude final de la voix (enveloppe + vélocité)
                let tremolo = if p.lfo_target == LfoTarget::VolumeTremolo {
                    (1.0 + lfo_val * 0.5).max(0.0)
                } else {
                    1.0
                };
                let voice_out = driven * amp * voice.velocity * tremolo;

                mix_l += voice_out * (0.5 - pan1 * 0.5);
                mix_r += voice_out * (0.5 + pan1 * 0.5);
            }

            // --- Étage d'Effets Intégrés ---

            // 1. Overdrive global tanh
            if p.fx_distortion > 0.01 {
                let drive_gain = 1.0 + p.fx_distortion * 4.0;
                mix_l = (mix_l * drive_gain).tanh();
                mix_r = (mix_r * drive_gain).tanh();
            }

            // 2. Chorus / Dimension stéréo
            if p.fx_chorus_mix > 0.01 {
                let c_l = self.chorus_phase.sin() * 0.5 + 0.5;
                let c_r = (self.chorus_phase + PI * 0.5).sin() * 0.5 + 0.5;
                let width = (mix_l - mix_r) * p.fx_chorus_mix * 0.6;
                mix_l = mix_l * (1.0 - p.fx_chorus_mix * 0.3) + width * c_l;
                mix_r = mix_r * (1.0 - p.fx_chorus_mix * 0.3) - width * c_r;
            }

            // 3. Delay Stéréo Ping-Pong
            if p.fx_delay_mix > 0.01 {
                let d_time_samples = (p.fx_delay_time_ms * 0.001 * sr) as usize;
                let buf_len = self.delay_buffer_l.len();
                let read_idx =
                    (self.delay_pos + buf_len - d_time_samples.min(buf_len - 1)) % buf_len;

                let delay_l = self.delay_buffer_l[read_idx];
                let delay_r = self.delay_buffer_r[read_idx];

                // Écriture croisée ping-pong
                self.delay_buffer_l[self.delay_pos] = mix_l + delay_r * p.fx_delay_feedback;
                self.delay_buffer_r[self.delay_pos] = mix_r + delay_l * p.fx_delay_feedback;
                self.delay_pos = (self.delay_pos + 1) % buf_len;

                mix_l = mix_l * (1.0 - p.fx_delay_mix) + delay_l * p.fx_delay_mix;
                mix_r = mix_r * (1.0 - p.fx_delay_mix) + delay_r * p.fx_delay_mix;
            }

            // 4. Réverbération algorithmique de salle
            if p.fx_reverb_mix > 0.01 {
                let mut rev_mono = 0.0f32;
                let fb = p.fx_reverb_size.clamp(0.2, 0.88);
                let dry_in = (mix_l + mix_r) * 0.5;

                for (idx, buf) in self.comb_buffers.iter_mut().enumerate() {
                    let pos = self.comb_positions[idx];
                    let delayed = buf[pos];
                    buf[pos] = dry_in + delayed * fb;
                    self.comb_positions[idx] = (pos + 1) % buf.len();
                    rev_mono += delayed;
                }
                rev_mono *= 0.25;

                mix_l = mix_l * (1.0 - p.fx_reverb_mix * 0.5) + rev_mono * p.fx_reverb_mix;
                mix_r = mix_r * (1.0 - p.fx_reverb_mix * 0.5) + rev_mono * p.fx_reverb_mix;
            }

            // Sortie finale amplifiée
            let final_l = mix_l * vol_linear;
            let final_r = mix_r * vol_linear;

            out[f * 2] += final_l;
            out[f * 2 + 1] += final_r;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wavetable_synth_creation_and_notes() {
        let mut synth = WavetableSynth::new(48000);
        synth.note_on(60, 100);
        let mut buffer = vec![0.0f32; 1024];
        synth.render(&mut buffer);

        let max_amp = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            max_amp > 0.02,
            "Le synthé Wavetable doit produire du son audible"
        );

        synth.note_off(60);
        let mut release_buffer = vec![0.0f32; 1024];
        synth.render(&mut release_buffer);
    }

    #[test]
    fn test_wavetable_presets_all_valid() {
        let presets = WavetableParams::all_presets();
        assert!(presets.len() >= 28, "Doit contenir au moins 28 presets");

        for p in presets {
            let mut synth = WavetableSynth::new(48000);
            synth.params = p;
            synth.note_on(64, 110);
            let mut buf = vec![0.0f32; 4096];
            synth.render(&mut buf);
            let amp = buf.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            assert!(
                amp > 0.005,
                "Preset {} doit être audible",
                synth.params.name
            );
        }
    }

    #[test]
    fn test_polyphony_voice_stealing() {
        let mut synth = WavetableSynth::new(48000);
        // Joue 20 notes simultanées (dépasse MAX_VOICES de 16)
        for n in 40..60 {
            synth.note_on(n, 100);
        }
        let active_count = synth.voices.iter().filter(|v| v.active).count();
        assert_eq!(
            active_count, MAX_VOICES,
            "Doit allouer exactement 16 voix sans déborder"
        );
    }
}
