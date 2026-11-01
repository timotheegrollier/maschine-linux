//! Synthétiseur analogique virtuel TB-303 Acid.
//!
//! Architecture :
//! - Oscillateur à onde en dent de scie (Saw) anti-repliée via PolyBLEP (+ Square optionnel)
//! - Glissando / Portamento (Slide) analogique
//! - Filtre passe-bas résonant à échelle non-linéaire (saturation interne tanh façon 303)
//! - Enveloppe d'accent et de modulation de filtre à décroissance exponentielle rapide
//! - Étage de distorsion / overdrive non-linéaire avec réglage de tonalité
//! - Ligne à retard (Delay) stéréo avec réinjection filtrée et mixage wet/dry

use serde::{Deserialize, Serialize};

/// Type d'onde de l'oscillateur principal 303.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AcidWaveform {
    /// Onde en dent de scie classique (Sawtooth PolyBLEP anti-repliée)
    #[default]
    Saw,
    /// Onde carrée / impulsionnelle (Square PolyBLEP)
    Square,
}

/// Paramètres de synthèse de l'Acid Synth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcidParams {
    /// Module actif ou non
    pub enabled: bool,
    /// Type d'onde (Saw ou Square)
    pub waveform: AcidWaveform,
    /// Fréquence de coupure de base du filtre passe-bas (Hz, 20..18000)
    pub cutoff_hz: f32,
    /// Résonance du filtre (0.0..1.0, auto-oscillation acid au-delà de 0.8)
    pub resonance: f32,
    /// Profondeur de modulation du filtre par l'enveloppe (0.0..1.0)
    pub env_mod: f32,
    /// Temps de déclin de l'enveloppe de filtre (ms, 30..3000)
    pub decay_ms: f32,
    /// Quantité d'accent (renforce le cutoff et le volume sur vélocités élevées, 0.0..1.0)
    pub accent: f32,
    /// Temps de portamento / slide (ms, 0..250)
    pub slide_time_ms: f32,
    /// Accordage en demi-tons (-24..+24)
    pub tuning_semitones: f32,
    /// Décalage d'octave (-2..+2)
    pub octave: i8,
    /// Taux de distorsion / drive (0.0..1.0, 0 dB à +36 dB)
    pub drive: f32,
    /// Tonalité du filtre post-distorsion (Hz, 500..16000)
    pub drive_tone: f32,
    /// Mixage distorsion (0.0 = clean, 1.0 = full fuzz)
    pub drive_mix: f32,
    /// Temps du delay (ms, 20..1000)
    pub delay_time_ms: f32,
    /// Réinjection du delay (0.0..0.95)
    pub delay_feedback: f32,
    /// Mixage delay (0.0 = dry, 1.0 = wet)
    pub delay_mix: f32,
    /// Volume de sortie en dB (-60..+12)
    pub volume_db: f32,
}

impl Default for AcidParams {
    fn default() -> Self {
        Self {
            enabled: true,
            waveform: AcidWaveform::Saw,
            cutoff_hz: 850.0,
            resonance: 0.75,
            env_mod: 0.65,
            decay_ms: 220.0,
            accent: 0.50,
            slide_time_ms: 60.0,
            tuning_semitones: 0.0,
            octave: 0,
            drive: 0.40,
            drive_tone: 6000.0,
            drive_mix: 0.45,
            delay_time_ms: 375.0, // ~croche pointée à 120 BPM
            delay_feedback: 0.45,
            delay_mix: 0.30,
            volume_db: 0.0,
        }
    }
}

impl AcidParams {
    /// Preset Acid 303 classique : basse saw râpeuse et résonante.
    #[must_use]
    pub fn preset_classic_acid() -> Self {
        Self {
            waveform: AcidWaveform::Saw,
            cutoff_hz: 650.0,
            resonance: 0.82,
            env_mod: 0.70,
            decay_ms: 200.0,
            accent: 0.60,
            slide_time_ms: 55.0,
            tuning_semitones: 0.0,
            octave: -1,
            drive: 0.35,
            drive_tone: 7500.0,
            drive_mix: 0.40,
            delay_time_ms: 300.0,
            delay_feedback: 0.35,
            delay_mix: 0.20,
            volume_db: 0.0,
            ..Default::default()
        }
    }

    /// Preset Squelch Lead : résonance hurlante et filtre ouvert.
    #[must_use]
    pub fn preset_squelch_lead() -> Self {
        Self {
            waveform: AcidWaveform::Saw,
            cutoff_hz: 1200.0,
            resonance: 0.94,
            env_mod: 0.85,
            decay_ms: 350.0,
            accent: 0.75,
            slide_time_ms: 70.0,
            tuning_semitones: 0.0,
            octave: 0,
            drive: 0.55,
            drive_tone: 9000.0,
            drive_mix: 0.50,
            delay_time_ms: 250.0,
            delay_feedback: 0.50,
            delay_mix: 0.35,
            volume_db: -1.0,
            ..Default::default()
        }
    }

    /// Preset Distorted Hard Acid : overdrive saturé façon rave 90s.
    #[must_use]
    pub fn preset_distorted() -> Self {
        Self {
            waveform: AcidWaveform::Saw,
            cutoff_hz: 900.0,
            resonance: 0.88,
            env_mod: 0.75,
            decay_ms: 180.0,
            accent: 0.80,
            slide_time_ms: 50.0,
            tuning_semitones: 0.0,
            octave: -1,
            drive: 0.85,
            drive_tone: 4500.0,
            drive_mix: 0.80,
            delay_time_ms: 375.0,
            delay_feedback: 0.40,
            delay_mix: 0.25,
            volume_db: -2.0,
            ..Default::default()
        }
    }

    /// Preset Dub Echo 303 : delay profond et modulation douce.
    #[must_use]
    pub fn preset_dub_echo() -> Self {
        Self {
            waveform: AcidWaveform::Square,
            cutoff_hz: 500.0,
            resonance: 0.65,
            env_mod: 0.50,
            decay_ms: 400.0,
            accent: 0.40,
            slide_time_ms: 80.0,
            tuning_semitones: 0.0,
            octave: -1,
            drive: 0.20,
            drive_tone: 4000.0,
            drive_mix: 0.25,
            delay_time_ms: 500.0,
            delay_feedback: 0.70,
            delay_mix: 0.55,
            volume_db: 0.0,
            ..Default::default()
        }
    }
}

/// Correction PolyBLEP pour adoucir les discontinuités et éliminer le repliement de spectre.
#[inline(always)]
fn poly_blep(t: f64, dt: f64) -> f64 {
    if t < dt {
        let t_norm = t / dt;
        2.0 * t_norm - t_norm * t_norm - 1.0
    } else if t > 1.0 - dt {
        let t_norm = (t - 1.0) / dt;
        2.0 * t_norm + t_norm * t_norm + 1.0
    } else {
        0.0
    }
}

/// Moteur de synthèse complet Acid TB-303.
#[derive(Debug)]
pub struct AcidSynth {
    /// Paramètres utilisateur
    pub params: AcidParams,
    /// Fréquence d'échantillonnage de l'engine
    pub sample_rate: f32,

    // --- Oscillateur ---
    phase: f64,
    current_freq: f32,
    target_freq: f32,

    // --- Enveloppes & Note ---
    active_note: Option<u8>,
    velocity: u8,
    gate_open: bool,
    filter_env: f32,
    amp_env: f32,
    accent_val: f32,
    sample_offset: usize,

    // --- Filtre 4 pôles à saturation non-linéaire ---
    y1: f32,
    y2: f32,
    y3: f32,
    y4: f32,

    // --- Distorsion & Tonalité ---
    dist_tone_state: f32,

    // --- Ligne de delay stéréo ---
    delay_buf_l: Vec<f32>,
    delay_buf_r: Vec<f32>,
    delay_write_pos: usize,
    delay_lp_l: f32,
    delay_lp_r: f32,
}

impl Default for AcidSynth {
    fn default() -> Self {
        Self::new(48000)
    }
}

impl AcidSynth {
    /// Crée une nouvelle instance avec la fréquence d'échantillonnage donnée.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(8000) as f32;
        // Buffer de delay de 2 secondes max
        let buf_size = (sr * 2.0) as usize;
        Self {
            params: AcidParams::default(),
            sample_rate: sr,
            phase: 0.0,
            current_freq: 110.0,
            target_freq: 110.0,
            active_note: None,
            velocity: 0,
            gate_open: false,
            filter_env: 0.0,
            amp_env: 0.0,
            accent_val: 0.0,
            sample_offset: 0,
            y1: 0.0,
            y2: 0.0,
            y3: 0.0,
            y4: 0.0,
            dist_tone_state: 0.0,
            delay_buf_l: vec![0.0; buf_size],
            delay_buf_r: vec![0.0; buf_size],
            delay_write_pos: 0,
            delay_lp_l: 0.0,
            delay_lp_r: 0.0,
        }
    }

    /// Réinitialise l'état interne (silence les voix et vide les buffers de délai).
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.gate_open = false;
        self.active_note = None;
        self.filter_env = 0.0;
        self.amp_env = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
        self.y3 = 0.0;
        self.y4 = 0.0;
        self.dist_tone_state = 0.0;
        self.delay_buf_l.fill(0.0);
        self.delay_buf_r.fill(0.0);
        self.delay_write_pos = 0;
        self.delay_lp_l = 0.0;
        self.delay_lp_r = 0.0;
    }

    /// Calcule la fréquence en Hz pour un numéro de note MIDI.
    fn midi_to_freq(&self, note: u8) -> f32 {
        let effective_note =
            note as f32 + self.params.tuning_semitones + (self.params.octave as f32 * 12.0);
        440.0 * 2.0f32.powf((effective_note - 69.0) / 12.0)
    }

    /// Déclenchement d'une note (Note On).
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        self.note_on_at(note, velocity, 0);
    }

    /// Déclenchement d'une note avec placement sample-accurate dans le bloc audio courant.
    pub fn note_on_at(&mut self, note: u8, velocity: u8, start_frame: usize) {
        self.sample_offset = start_frame;
        let new_freq = self.midi_to_freq(note);

        // Détection de glissando (slide legato) façon TB-303 :
        // Si une note précédente sonne déjà, la cible est mise à jour et la fréquence glisse.
        let is_legato = self.gate_open && self.active_note.is_some();
        self.active_note = Some(note);
        self.velocity = velocity;
        self.gate_open = true;
        self.target_freq = new_freq;

        if !is_legato || self.params.slide_time_ms <= 1.0 {
            self.current_freq = new_freq;
        }

        // Déclenchement d'enveloppe
        self.filter_env = 1.0;
        self.amp_env = 1.0;

        // Détection d'accent (vélocité > 96 ou accent activé)
        let is_acc = velocity > 96 || self.params.accent > 0.7;
        self.accent_val = if is_acc {
            (velocity as f32 / 127.0) * self.params.accent
        } else {
            0.0
        };
    }

    /// Relâchement d'une note (Note Off).
    pub fn note_off(&mut self, note: u8) {
        if self.active_note == Some(note) {
            self.gate_open = false;
            self.active_note = None;
        }
    }

    /// Traite et rend un bloc stéréo entrelacé `out = [L, R, L, R, ...]`.
    pub fn render(&mut self, out: &mut [f32]) {
        if !self.params.enabled {
            return;
        }

        let num_frames = out.len() / 2;
        let sr = self.sample_rate;
        let dt = 1.0 / sr;

        // Facteur de glissando par échantillon
        let slide_alpha = if self.params.slide_time_ms > 1.0 {
            let tau = self.params.slide_time_ms * 0.001;
            (-dt / tau).exp()
        } else {
            0.0
        };

        // Facteurs de déclin d'enveloppe
        let decay_sec = (self.params.decay_ms * 0.001).clamp(0.015, 4.0);
        // L'accent raccourcit légèrement le déclin du filtre pour plus de punch
        let eff_decay = if self.accent_val > 0.1 {
            decay_sec * (1.0 - self.accent_val * 0.3)
        } else {
            decay_sec
        };
        let filter_decay_rate = (-dt / eff_decay).exp();

        // Déclin de l'ampli (mode gate avec queue de release courte)
        let amp_release_rate = (-dt / 0.040).exp();

        // Paramètres de distorsion
        let drive_gain = 1.0 + self.params.drive * 15.0; // gain jusqu'à +24dB
        let tone_coeff = (1.0
            - (-2.0 * std::f32::consts::PI * self.params.drive_tone.clamp(200.0, sr * 0.45) / sr)
                .exp())
        .clamp(0.001, 1.0);

        // Paramètres de delay
        let delay_samples = ((self.params.delay_time_ms * 0.001 * sr) as usize)
            .clamp(1, self.delay_buf_l.len() - 1);
        let delay_fb = self.params.delay_feedback.clamp(0.0, 0.95);
        let delay_mix = self.params.delay_mix.clamp(0.0, 1.0);

        let master_gain = 10.0f32.powf(self.params.volume_db / 20.0);

        for frame in 0..num_frames {
            if self.sample_offset > 0 {
                self.sample_offset -= 1;
                continue;
            }

            // 1. Portamento / Fréquence
            if slide_alpha > 0.0 {
                self.current_freq =
                    self.target_freq + (self.current_freq - self.target_freq) * slide_alpha;
            } else {
                self.current_freq = self.target_freq;
            }

            let freq = self.current_freq.clamp(10.0, sr * 0.45);
            let phase_inc = (freq / sr) as f64;

            // 2. Oscillateur PolyBLEP (Saw ou Square)
            let osc_out = match self.params.waveform {
                AcidWaveform::Saw => {
                    // Saw naïve : 2 * phase - 1
                    let raw_saw = 2.0 * self.phase - 1.0;
                    // Soustraction de la correction BLEP sur le saut de phase
                    let blep = poly_blep(self.phase, phase_inc);
                    (raw_saw - blep) as f32
                }
                AcidWaveform::Square => {
                    let raw_sqr = if self.phase < 0.5 { 1.0 } else { -1.0 };
                    let blep1 = poly_blep(self.phase, phase_inc);
                    let phase2 = (self.phase + 0.5).fract();
                    let blep2 = poly_blep(phase2, phase_inc);
                    (raw_sqr + blep1 - blep2) as f32
                }
            };

            // Avancement de phase
            self.phase = (self.phase + phase_inc).fract();

            // 3. Enveloppe de filtre et d'amplitude
            self.filter_env *= filter_decay_rate;
            if !self.gate_open {
                self.amp_env *= amp_release_rate;
            }

            if self.amp_env < 1e-4 && !self.gate_open {
                self.amp_env = 0.0;
            }

            // 4. Calcul de coupure modulée façon TB-303
            // La modulation par l'enveloppe ouvre la coupure jusqu'à 6-7 octaves au-dessus
            let env_octaves = (self.params.env_mod * self.filter_env + self.accent_val * 0.4) * 6.0;
            let mod_cutoff = self.params.cutoff_hz * 2.0f32.powf(env_octaves);
            let cutoff = mod_cutoff.clamp(25.0, sr * 0.42);

            // Modèle résonant 4 pôles avec rétroaction non-linéaire saturante (tanh)
            let f = (std::f32::consts::PI * cutoff / sr)
                .tan()
                .clamp(0.0001, 0.99);
            let res = self.params.resonance.clamp(0.0, 0.985) * 3.95;

            // Saturation de la rétroaction pour auto-oscillation propre (le cri acid)
            let feedback = (res * self.y4).tanh();
            let input_signal = osc_out - feedback;

            // Cascades 4 pôles
            self.y1 += f * (input_signal.tanh() - self.y1);
            self.y2 += f * (self.y1 - self.y2);
            self.y3 += f * (self.y2 - self.y3);
            self.y4 += f * (self.y3 - self.y4);

            let filtered = self.y4 * self.amp_env;

            // 5. Étage de distorsion / Overdrive
            let dry_synth = filtered;
            let driven = (dry_synth * drive_gain).tanh();

            // Filtre de tonalité post-saturation (passe-bas 1-pôle)
            self.dist_tone_state += tone_coeff * (driven - self.dist_tone_state);
            let wet_dist = self.dist_tone_state;
            let post_dist =
                dry_synth * (1.0 - self.params.drive_mix) + wet_dist * self.params.drive_mix;

            // 6. Ligne de délai stéréo avec réinjection filtrée (ping-pong subtil)
            let buf_len = self.delay_buf_l.len();
            let read_pos_l = (self.delay_write_pos + buf_len - delay_samples) % buf_len;
            // Décalage pour stéréo ping-pong (5ms plus court sur canal droit)
            let right_offset = (delay_samples * 9 / 10).max(1);
            let read_pos_r = (self.delay_write_pos + buf_len - right_offset) % buf_len;

            let delayed_l = self.delay_buf_l[read_pos_l];
            let delayed_r = self.delay_buf_r[read_pos_r];

            // Filtre coupe-haut dans la boucle de feedback (~4.5 kHz pour delay analogique chaud)
            let damp_coeff =
                (1.0 - (-2.0 * std::f32::consts::PI * 4500.0 / sr).exp()).clamp(0.01, 1.0);
            self.delay_lp_l += damp_coeff * (delayed_l - self.delay_lp_l);
            self.delay_lp_r += damp_coeff * (delayed_r - self.delay_lp_r);

            // Écriture dans le buffer avec croisement stéréo
            self.delay_buf_l[self.delay_write_pos] = post_dist + self.delay_lp_r * delay_fb;
            self.delay_buf_r[self.delay_write_pos] = post_dist + self.delay_lp_l * delay_fb;
            self.delay_write_pos = (self.delay_write_pos + 1) % buf_len;

            let out_l = (post_dist * (1.0 - delay_mix) + delayed_l * delay_mix) * master_gain;
            let out_r = (post_dist * (1.0 - delay_mix) + delayed_r * delay_mix) * master_gain;

            // Accumulation dans le mixeur principal
            out[frame * 2] += out_l;
            out[frame * 2 + 1] += out_r;
        }
    }

    /// Indique si une note est actuellement active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.gate_open || self.amp_env > 1e-4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acid_synth_creation_and_render() {
        let mut synth = AcidSynth::new(48000);
        synth.note_on(36, 100); // C2

        let mut buf = vec![0.0f32; 512];
        synth.render(&mut buf);

        // Le buffer ne doit pas être vide
        let peak = buf.iter().fold(0.0f32, |acc, &x| acc.max(x.abs()));
        assert!(peak > 0.01, "Le synthé acid doit produire du son");
        assert!(peak <= 2.0, "Le signal ne doit pas exploser");
    }

    #[test]
    fn test_acid_presets_dont_panic() {
        let mut synth = AcidSynth::new(48000);
        synth.params = AcidParams::preset_classic_acid();
        synth.note_on(48, 120);

        let mut buf = vec![0.0f32; 1024];
        synth.render(&mut buf);

        synth.params = AcidParams::preset_squelch_lead();
        synth.render(&mut buf);

        synth.params = AcidParams::preset_distorted();
        synth.render(&mut buf);

        synth.params = AcidParams::preset_dub_echo();
        synth.render(&mut buf);

        synth.note_off(48);
        synth.render(&mut buf);
    }
}
