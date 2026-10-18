//! Sampler — voix polyphonique, zones, choke, ADSR simplifié.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Mapping zone → sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneMapping {
    /// Sample data (f32 mono, 48k) — Arc pour partage voix
    #[serde(skip)]
    pub sample: Option<Arc<Vec<f32>>>,
    /// Chemin (pour persistance)
    pub sample_path: Option<String>,
    /// Sample rate original
    pub sample_rate: u32,
    /// Root key
    pub root_key: u8,
    /// Lokey/hikey
    pub lokey: u8,
    /// Hikey
    pub hikey: u8,
    /// Lovel/hivel
    pub lovel: u8,
    /// Hivel
    pub hivel: u8,
    /// Volume dB
    pub volume_db: f32,
    /// Pan
    pub pan: f32,
    /// Tune cents
    pub tune_cents: i16,
    /// Choke group
    pub choke_group: Option<u8>,
    /// Mode One-Shot (ne coupe pas le son au relâchement du pad)
    #[serde(default)]
    pub is_one_shot: bool,
}

impl Default for ZoneMapping {
    fn default() -> Self {
        Self {
            sample: None,
            sample_path: None,
            sample_rate: 48000,
            root_key: 60,
            lokey: 0,
            hikey: 127,
            lovel: 0,
            hivel: 127,
            volume_db: 0.0,
            pan: 0.0,
            tune_cents: 0,
            choke_group: None,
            is_one_shot: true,
        }
    }
}

/// Paramètres de voix lus depuis le `Sound` du projet (volume/pan déjà appliqués
/// via gain, ici enveloppe + filtre). Construit par l'`Engine` à chaque trigger
/// pour que les réglages GUI/écrans/knobs soient audibles immédiatement.
#[derive(Debug, Clone, Copy)]
pub struct VoiceParams {
    /// Attack ms
    pub attack_ms: f32,
    /// Decay ms
    pub decay_ms: f32,
    /// Sustain 0..1
    pub sustain: f32,
    /// Release ms
    pub release_ms: f32,
    /// Cutoff lowpass Hz (20000+ = off)
    pub cutoff_hz: f32,
    /// Resonance 0..1
    pub resonance: f32,
}

impl Default for VoiceParams {
    fn default() -> Self {
        Self {
            attack_ms: 2.0,
            decay_ms: 100.0,
            sustain: 0.8,
            release_ms: 150.0,
            cutoff_hz: 20000.0,
            resonance: 0.0,
        }
    }
}

/// Voix active.
#[derive(Debug, Clone)]
pub struct SamplerVoice {
    /// Sample
    pub sample: Arc<Vec<f32>>,
    /// Position lecture (float pour interpolation)
    pub pos: f64,
    /// Incrément (pitch + tune)
    pub increment: f64,
    /// Gain linéaire (vélocité + volume zone)
    pub gain: f32,
    /// Pan -1..1
    pub pan: f32,
    /// ADSR état
    pub adsr_phase: AdsrPhase,
    /// Temps dans phase (samples)
    pub phase_samples: u32,
    /// Attack en samples
    pub attack_samples: u32,
    /// Decay en samples
    pub decay_samples: u32,
    /// Niveau sustain 0..1
    pub sustain_level: f32,
    /// Release en samples
    pub release_samples: u32,
    /// Niveau d'enveloppe courant (pour les transitions)
    pub env_level: f32,
    /// Niveau au début du Release
    pub release_from: f32,
    /// Décalage de départ dans le bloc courant (échantillons) — permet de
    /// placer les notes au sample près dans un buffer audio, au lieu de les
    /// quantifier à la taille du buffer.
    pub start_frame: usize,
    /// Longueur (en positions de lecture) du fondu anti-clic en fin de sample.
    pub end_fade: f64,
    /// Resonance 0..1
    pub resonance: f32,
    /// Filtre activé
    pub filter_enabled: bool,
    /// Coefficients filtre biquad
    pub filter_b0: f32,
    pub filter_b1: f32,
    pub filter_b2: f32,
    pub filter_a1: f32,
    pub filter_a2: f32,
    /// États filtre biquad (DF2-T)
    pub filter_s1: f32,
    pub filter_s2: f32,
    /// Note
    pub note: u8,
    /// Velocity
    pub velocity: u8,
    /// Choke group
    pub choke_group: Option<u8>,
    /// Âge (pour voice stealing)
    pub age: u64,
    /// Mode One-Shot
    pub is_one_shot: bool,
    /// Index du Sound source (pour mise à jour filtre/ADSR en temps réel)
    pub sound: u8,
    /// Groupe mixer 0..8 attribué par l'Engine (0xFF = hors groupe : clic, preview)
    pub group: u8,
}

/// Phase ADSR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdsrPhase {
    /// Attack
    Attack,
    /// Decay
    Decay,
    /// Sustain
    Sustain,
    /// Release
    Release,
    /// Off
    Off,
}

/// Sampler — gère les voix.
#[derive(Debug, Default)]
pub struct Sampler {
    /// Zones par Sound (index Sound 0..15) → Vec<ZoneMapping>
    zones: HashMap<u8, Vec<ZoneMapping>>,
    /// Voix actives
    voices: Vec<SamplerVoice>,
    /// Compteur âge
    age_counter: u64,
    /// Round-robin counters par Sound
    rr_counters: std::collections::HashMap<u8, u8>,
    /// Polyphonie max globale
    pub max_voices: usize,
    /// Sample rate engine
    pub sample_rate: u32,
    /// Crête par groupe mixer (0..8) mise à jour à chaque `render` (avant release).
    pub group_peak: [f32; 8],
}

impl Sampler {
    /// Nouveau.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            max_voices: 64,
            ..Default::default()
        }
    }

    /// Ajoute une zone à un Sound.
    pub fn add_zone(&mut self, sound: u8, zone: ZoneMapping) {
        self.zones.entry(sound).or_default().push(zone);
    }

    /// Supprime toutes les zones d'un Sound.
    pub fn clear_sound(&mut self, sound: u8) {
        self.zones.remove(&sound);
    }

    /// Note on — choisit la zone par key/vel, crée une voix.
    pub fn note_on(&mut self, sound: u8, note: u8, velocity: u8, pitch: u8) -> Option<usize> {
        self.note_on_at(sound, note, velocity, pitch, 0)
    }

    /// Note on avec départ décalé dans le bloc audio courant (`start_frame`
    /// échantillons de silence) : placement sample-accurate des notes.
    pub fn note_on_at(
        &mut self,
        sound: u8,
        note: u8,
        velocity: u8,
        pitch: u8,
        start_frame: usize,
    ) -> Option<usize> {
        let zones = self.zones.get(&sound)?;
        // filtre lokey/hikey/lovel/hivel — sans allocation : on compte puis on
        // indexe la n-ième zone éligible (le callback audio ne doit pas allouer).
        let eligible = |z: &ZoneMapping| {
            pitch >= z.lokey && pitch <= z.hikey && velocity >= z.lovel && velocity <= z.hivel
        };
        let n_match = zones.iter().filter(|z| eligible(z)).count();
        let (use_all, n_cand) = if n_match == 0 {
            (true, zones.len())
        } else {
            (false, n_match)
        };
        if n_cand == 0 {
            return None;
        }
        // Round-robin : alterne les zones candidates pour le même Sound
        let rr = self.rr_counters.entry(sound).or_insert(0);
        let rr_idx = (*rr as usize) % n_cand;
        *rr = rr.wrapping_add(1);
        // Copie locale : le vol de voix ci-dessous a besoin de `&mut self`.
        let (sample, pan, choke_group, root_key, tune_cents, volume_db, is_one_shot, zone_rate) = {
            let zone = zones
                .iter()
                .filter(|z| use_all || eligible(z))
                .nth(rr_idx)?;
            (
                zone.sample.clone()?,
                zone.pan,
                zone.choke_group,
                zone.root_key,
                zone.tune_cents,
                zone.volume_db,
                zone.is_one_shot,
                zone.sample_rate,
            )
        };
        if sample.is_empty() {
            return None;
        }

        // choke : les voix du même groupe fondent en Release (5ms, pas de clic)
        // au lieu d'être coupées net.
        if let Some(choke) = choke_group {
            for v in self.voices.iter_mut() {
                if v.choke_group == Some(choke)
                    && v.adsr_phase != AdsrPhase::Off
                    && v.adsr_phase != AdsrPhase::Release
                {
                    v.adsr_phase = AdsrPhase::Release;
                    v.phase_samples = 0;
                    v.release_from = v.env_level.max(0.05);
                    // fade choke court : ~5ms, en resserrant le release existant
                    let choke_rel = ((0.005 * self.sample_rate as f32) as u32).max(1);
                    if v.release_samples > choke_rel {
                        v.release_samples = choke_rel;
                    }
                }
            }
        }

        // pitch : root_key → pitch
        let semitones = pitch as f32 - root_key as f32 + tune_cents as f32 / 100.0;
        let pitch_ratio = 2_f64.powf(semitones as f64 / 12.0);
        let sr_ratio = zone_rate as f64 / self.sample_rate as f64;
        let increment = pitch_ratio * sr_ratio;

        let gain = velocity as f32 / 127.0 * db_to_linear(volume_db);

        // Polyphonie pleine : on ne coupe plus la plus vieille voix d'un coup
        // (clic) — elle part en fondu de 5 ms (le rendu la supprime ensuite).
        if self.voices.len() >= self.max_voices {
            if let Some(idx) = self
                .voices
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    v.adsr_phase != AdsrPhase::Release && v.adsr_phase != AdsrPhase::Off
                })
                .min_by_key(|(_, v)| v.age)
                .map(|(i, _)| i)
            {
                self.fade_out_voice(idx, 0.005);
            }
        }

        let voice = SamplerVoice {
            sample,
            pos: 0.0,
            increment,
            gain,
            pan,
            adsr_phase: AdsrPhase::Attack,
            phase_samples: 0,
            attack_samples: ((0.002 * self.sample_rate as f32) as u32).max(1),
            decay_samples: (0.100 * self.sample_rate as f32) as u32,
            sustain_level: 0.8,
            release_samples: ((0.150 * self.sample_rate as f32) as u32).max(1),
            resonance: 0.0,
            filter_enabled: false,
            filter_b0: 1.0,
            filter_b1: 0.0,
            filter_b2: 0.0,
            filter_a1: 0.0,
            filter_a2: 0.0,
            filter_s1: 0.0,
            filter_s2: 0.0,
            env_level: 0.0,
            release_from: 1.0,
            start_frame,
            // Fondu anti-clic de 3 ms en fin de sample (positions de lecture).
            end_fade: (0.003 * self.sample_rate as f64 / increment.max(0.01)).max(1.0),
            note,
            velocity,
            choke_group,
            age: self.age_counter,
            is_one_shot,
            sound,
            group: u8::MAX,
        };
        self.age_counter += 1;
        self.voices.push(voice);
        Some(self.voices.len() - 1)
    }

    /// Fait fondre une voix sur `secs` puis la laisse s'éteindre (anti-clic).
    fn fade_out_voice(&mut self, idx: usize, secs: f32) {
        if let Some(v) = self.voices.get_mut(idx) {
            if v.adsr_phase == AdsrPhase::Release || v.adsr_phase == AdsrPhase::Off {
                return;
            }
            v.adsr_phase = AdsrPhase::Release;
            v.phase_samples = 0;
            v.release_from = v.env_level.max(0.05);
            v.release_samples = ((secs * self.sample_rate as f32) as u32).max(1);
        }
    }

    /// Joue un buffer brut en one-shot (clic métronome), sans zone.
    pub fn play_raw(&mut self, sample: &Arc<Vec<f32>>, gain: f32) -> Option<usize> {
        self.play_raw_at(sample, gain, 0)
    }

    /// `play_raw` avec départ décalé dans le bloc courant.
    pub fn play_raw_at(
        &mut self,
        sample: &Arc<Vec<f32>>,
        gain: f32,
        start_frame: usize,
    ) -> Option<usize> {
        if sample.is_empty() {
            return None;
        }
        if self.voices.len() >= self.max_voices {
            if let Some(idx) = self
                .voices
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    v.adsr_phase != AdsrPhase::Release && v.adsr_phase != AdsrPhase::Off
                })
                .min_by_key(|(_, v)| v.age)
                .map(|(i, _)| i)
            {
                self.fade_out_voice(idx, 0.005);
            }
        }
        let voice = SamplerVoice {
            sample: sample.clone(),
            pos: 0.0,
            increment: 1.0,
            gain,
            pan: 0.0,
            adsr_phase: AdsrPhase::Sustain,
            phase_samples: 0,
            attack_samples: 1,
            decay_samples: 0,
            sustain_level: 1.0,
            release_samples: ((0.030 * self.sample_rate as f32) as u32).max(1),
            resonance: 0.0,
            filter_enabled: false,
            filter_b0: 1.0,
            filter_b1: 0.0,
            filter_b2: 0.0,
            filter_a1: 0.0,
            filter_a2: 0.0,
            filter_s1: 0.0,
            filter_s2: 0.0,
            env_level: 1.0,
            release_from: 1.0,
            start_frame,
            end_fade: (0.003 * self.sample_rate as f64).max(1.0),
            note: 255,
            velocity: 127,
            choke_group: None,
            age: self.age_counter,
            is_one_shot: true,
            sound: 255,
            group: u8::MAX,
        };
        self.age_counter += 1;
        self.voices.push(voice);
        Some(self.voices.len() - 1)
    }

    /// Note off — passe en Release si non one-shot.
    pub fn note_off(&mut self, sound: u8, note: u8) {
        for v in &mut self.voices {
            // Les percussions en mode One-Shot ne sont pas coupées au relâchement
            if v.note == note && !v.is_one_shot {
                let _ = sound;
                if v.adsr_phase != AdsrPhase::Off && v.adsr_phase != AdsrPhase::Release {
                    v.adsr_phase = AdsrPhase::Release;
                    v.phase_samples = 0;
                    v.release_from = v.env_level.max(0.05);
                }
            }
        }
    }

    /// Render `frames` samples stéréo interleaved dans `out` (out.len() == frames*2).
    /// Retourne le nombre de voix actives après render.
    pub fn render(&mut self, out: &mut [f32]) {
        // out = [L,R,L,R,...]
        let frames = out.len() / 2;
        // zero
        for s in out.iter_mut() {
            *s = 0.0;
        }
        for p in self.group_peak.iter_mut() {
            *p = 0.0;
        }
        let mut to_remove = Vec::new();
        for (idx, voice) in self.voices.iter_mut().enumerate() {
            // Départ décalé dans le bloc (timing sample-accurate) : si le bloc
            // rendu est plus court que le décalage, la voix démarre au bloc
            // suivant (le compteur est consommé bloc par bloc).
            if voice.start_frame >= frames {
                voice.start_frame -= frames;
                continue;
            }
            let first = voice.start_frame;
            voice.start_frame = 0;
            let mut pos = voice.pos;
            let inc = voice.increment;
            let sample = &voice.sample;
            let sample_len = sample.len() as f64;
            let end_fade = voice.end_fade;
            let mut voice_peak = 0.0f32;
            for frame in first..frames {
                if pos >= sample_len - 1.0 {
                    to_remove.push(idx);
                    break;
                }
                // linear interp
                let i = pos as usize;
                let frac = (pos - i as f64) as f32;
                let s0 = sample[i];
                let s1 = sample.get(i + 1).copied().unwrap_or(0.0);
                let mut s = s0 * (1.0 - frac) + s1 * frac;
                // Fondu anti-clic sur la fin du sample (queue tronquée).
                if sample_len - pos < end_fade {
                    s *= ((sample_len - pos) / end_fade) as f32;
                }
                // Filtre lowpass résonant biquad (Direct Form II Transposed)
                if voice.filter_enabled {
                    let out_f = voice.filter_b0 * s + voice.filter_s1;
                    voice.filter_s1 =
                        voice.filter_b1 * s - voice.filter_a1 * out_f + voice.filter_s2;
                    voice.filter_s2 = voice.filter_b2 * s - voice.filter_a2 * out_f;
                    s = out_f;
                }
                // ADSR complète (A/D/S/R du Sound)
                let env = match voice.adsr_phase {
                    AdsrPhase::Attack => {
                        if voice.attack_samples <= 1 {
                            voice.adsr_phase = AdsrPhase::Decay;
                            voice.phase_samples = 0;
                            voice.env_level = 1.0;
                        } else if voice.phase_samples < voice.attack_samples {
                            voice.phase_samples += 1;
                            voice.env_level =
                                voice.phase_samples as f32 / voice.attack_samples as f32;
                        } else {
                            voice.adsr_phase = AdsrPhase::Decay;
                            voice.phase_samples = 0;
                            voice.env_level = 1.0;
                        }
                        voice.env_level
                    }
                    AdsrPhase::Decay => {
                        if voice.decay_samples == 0 {
                            voice.adsr_phase = AdsrPhase::Sustain;
                            voice.env_level = voice.sustain_level;
                        } else if voice.phase_samples < voice.decay_samples {
                            voice.phase_samples += 1;
                            let t = voice.phase_samples as f32 / voice.decay_samples as f32;
                            voice.env_level = 1.0 + (voice.sustain_level - 1.0) * t;
                        } else {
                            voice.adsr_phase = AdsrPhase::Sustain;
                            voice.env_level = voice.sustain_level;
                        }
                        voice.env_level
                    }
                    AdsrPhase::Sustain => {
                        voice.env_level = voice.sustain_level;
                        voice.env_level
                    }
                    AdsrPhase::Release => {
                        if voice.phase_samples < voice.release_samples {
                            voice.phase_samples += 1;
                            let t = voice.phase_samples as f32 / voice.release_samples as f32;
                            voice.env_level = voice.release_from * (1.0 - t);
                        } else {
                            voice.adsr_phase = AdsrPhase::Off;
                            voice.env_level = 0.0;
                        }
                        voice.env_level
                    }
                    AdsrPhase::Off => 0.0,
                };
                if env <= 0.0 {
                    to_remove.push(idx);
                    break;
                }
                let g = voice.gain * env;
                // pan law -3dB
                let (lg, rg) = pan_gains(voice.pan, g);
                let l = s * lg;
                let r = s * rg;
                out[frame * 2] += l;
                out[frame * 2 + 1] += r;
                let mag = l.abs().max(r.abs());
                if mag > voice_peak {
                    voice_peak = mag;
                }
                pos += inc;
            }
            voice.pos = pos;
            if voice.group < 8 && voice_peak > self.group_peak[voice.group as usize] {
                self.group_peak[voice.group as usize] = voice_peak;
            }
            if voice.adsr_phase == AdsrPhase::Off {
                to_remove.push(idx);
            }
        }
        // remove en ordre inverse
        to_remove.sort_unstable();
        to_remove.dedup();
        for &idx in to_remove.iter().rev() {
            if idx < self.voices.len() {
                self.voices.remove(idx);
            }
        }
    }

    /// Multipie le gain d'une voix (pour volume de groupe).
    pub fn set_voice_gain(&mut self, idx: usize, mul: f32) {
        if let Some(v) = self.voices.get_mut(idx) {
            v.gain *= mul;
        }
    }

    /// Définit le pan d'une voix.
    pub fn set_voice_pan(&mut self, idx: usize, pan: f32) {
        if let Some(v) = self.voices.get_mut(idx) {
            v.pan = pan.clamp(-1.0, 1.0);
        }
    }

    /// Repointe les zones d'un Sound vers un nouveau sample (changement de taux
    /// d'échantillonnage du moteur). Seules les zones qui pointaient encore sur
    /// l'ancien sample sont modifiées : les samples importés sont préservés.
    pub fn replace_zone_sample(
        &mut self,
        sound: u8,
        old: &Arc<Vec<f32>>,
        new: Arc<Vec<f32>>,
        sample_rate: u32,
    ) {
        if let Some(zones) = self.zones.get_mut(&sound) {
            for z in zones.iter_mut() {
                if let Some(s) = &z.sample {
                    if Arc::ptr_eq(s, old) {
                        z.sample = Some(new.clone());
                        z.sample_rate = sample_rate;
                    }
                }
            }
        }
    }

    /// Attribue une voix à un groupe mixer (0..8) pour le VU-mètre.
    pub fn set_voice_group(&mut self, idx: usize, group: u8) {
        if let Some(v) = self.voices.get_mut(idx) {
            v.group = group;
        }
    }

    /// Crêtes par groupe du dernier `render` (0..8).
    #[must_use]
    pub fn group_peaks(&self) -> [f32; 8] {
        self.group_peak
    }

    /// Enveloppe (min,max) par bucket du premier sample du Sound — pour la
    /// waveform de l'UI/écrans. `buckets` ≈ nombre de colonnes de pixels.
    #[must_use]
    pub fn waveform_envelope(&self, sound: u8, buckets: usize) -> Option<Vec<(f32, f32)>> {
        if buckets == 0 {
            return None;
        }
        let zones = self.zones.get(&sound)?;
        let sample = zones.iter().find_map(|z| z.sample.as_ref())?;
        if sample.is_empty() {
            return None;
        }
        let n = sample.len();
        let step = n as f32 / buckets as f32;
        let mut out = Vec::with_capacity(buckets);
        for b in 0..buckets {
            let s = (b as f32 * step) as usize;
            let e = (((b + 1) as f32 * step) as usize).min(n);
            if s >= e {
                out.push((0.0, 0.0));
                continue;
            }
            let mut mn = f32::MAX;
            let mut mx = f32::MIN;
            for &v in &sample[s..e] {
                if v < mn {
                    mn = v;
                }
                if v > mx {
                    mx = v;
                }
            }
            out.push((mn, mx));
        }
        Some(out)
    }

    /// Durée en secondes du sample du Sound (0 si aucun).
    #[must_use]
    pub fn sample_duration_secs(&self, sound: u8) -> f32 {
        self.zones
            .get(&sound)
            .and_then(|z| z.iter().find_map(|z| z.sample.as_ref()))
            .map(|s| s.len() as f32 / self.sample_rate.max(1) as f32)
            .unwrap_or(0.0)
    }

    /// Calcule les coefficients du filtre lowpass résonant biquad (RBJ).
    fn calc_biquad(sr: f32, cutoff_hz: f32, resonance: f32, v: &mut SamplerVoice) {
        v.resonance = resonance.clamp(0.0, 1.0);
        if cutoff_hz < 19500.0 || v.resonance > 0.05 {
            let fc = (cutoff_hz / sr).clamp(0.0005, 0.49);
            let w0 = 2.0 * std::f32::consts::PI * fc;
            let cos_w0 = w0.cos();
            let sin_w0 = w0.sin();
            let q = std::f32::consts::FRAC_1_SQRT_2 + v.resonance * 8.0;
            let alpha = sin_w0 / (2.0 * q);
            let a0 = 1.0 + alpha;
            v.filter_enabled = true;
            v.filter_b1 = (1.0 - cos_w0) / a0;
            v.filter_b0 = v.filter_b1 * 0.5;
            v.filter_b2 = v.filter_b0;
            v.filter_a1 = (-2.0 * cos_w0) / a0;
            v.filter_a2 = (1.0 - alpha) / a0;
        } else {
            v.filter_enabled = false;
        }
    }

    /// Applique les paramètres du Sound (ADSR + filtre) à une voix.
    /// Appelé par l'`Engine` juste après `note_on` pour que les réglages
    /// GUI/écrans/knobs soient audibles.
    pub fn set_voice_params(&mut self, idx: usize, params: &VoiceParams) {
        if let Some(v) = self.voices.get_mut(idx) {
            let sr = self.sample_rate.max(1) as f32;
            v.attack_samples = ((params.attack_ms.max(0.1) / 1000.0 * sr) as u32).max(1);
            v.decay_samples = (params.decay_ms.max(0.0) / 1000.0 * sr) as u32;
            v.sustain_level = params.sustain.clamp(0.0, 1.0);
            v.release_samples = ((params.release_ms.max(1.0) / 1000.0 * sr) as u32).max(1);
            Self::calc_biquad(sr, params.cutoff_hz, params.resonance, v);
        }
    }

    /// Met à jour en temps réel le filtre (cutoff + resonance) pour toutes les voix d'un son donné.
    pub fn update_sound_filter(&mut self, sound: u8, cutoff_hz: f32, resonance: f32) {
        let sr = self.sample_rate.max(1) as f32;
        for v in &mut self.voices {
            if v.sound == sound {
                Self::calc_biquad(sr, cutoff_hz, resonance, v);
            }
        }
    }

    /// Met à jour en temps réel l'enveloppe ADSR pour toutes les voix d'un son donné.
    pub fn update_sound_adsr(
        &mut self,
        sound: u8,
        attack_ms: f32,
        decay_ms: f32,
        sustain: f32,
        release_ms: f32,
    ) {
        let sr = self.sample_rate.max(1) as f32;
        for v in &mut self.voices {
            if v.sound == sound {
                v.attack_samples = ((attack_ms.max(0.1) / 1000.0 * sr) as u32).max(1);
                v.decay_samples = (decay_ms.max(0.0) / 1000.0 * sr) as u32;
                v.sustain_level = sustain.clamp(0.0, 1.0);
                v.release_samples = ((release_ms.max(1.0) / 1000.0 * sr) as u32).max(1);
            }
        }
    }

    /// Met à jour en temps réel le pan des voix d'un son.
    pub fn update_sound_pan(&mut self, sound: u8, pan: f32) {
        for v in &mut self.voices {
            if v.sound == sound {
                v.pan = pan.clamp(-1.0, 1.0);
            }
        }
    }

    /// Nombre de voix.
    #[must_use]
    pub fn num_voices(&self) -> usize {
        self.voices.len()
    }

    /// Charge un WAV mono/stéréo (via `hound`) et le convertit en mono f32.
    #[cfg(feature = "sampler")]
    pub fn load_wav(
        &mut self,
        sound: u8,
        path: &std::path::Path,
        root_key: u8,
    ) -> crate::Result<()> {
        let mut reader = hound::WavReader::open(path).map_err(|e| {
            crate::EngineError::Sampler(format!("hound open {}: {e}", path.display()))
        })?;
        let spec = reader.spec();
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => {
                reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect()
            }
            hound::SampleFormat::Int => {
                let max = (1 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i32>()
                    .map(|s| s.unwrap_or(0) as f32 / max)
                    .collect()
            }
        };
        // si stéréo, mix down mono
        let mono: Vec<f32> = if spec.channels == 2 {
            samples.chunks(2).map(|c| (c[0] + c[1]) * 0.5).collect()
        } else if spec.channels == 1 {
            samples
        } else {
            // multi-channel : prend premier canal
            samples
                .chunks(spec.channels as usize)
                .map(|c| c[0])
                .collect()
        };
        let zone = ZoneMapping {
            sample: Some(Arc::new(mono)),
            sample_path: Some(path.to_string_lossy().to_string()),
            sample_rate: spec.sample_rate,
            root_key,
            lokey: 0,
            hikey: 127,
            lovel: 0,
            hivel: 127,
            volume_db: 0.0,
            pan: 0.0,
            tune_cents: 0,
            choke_group: None,
            is_one_shot: true,
        };
        self.add_zone(sound, zone);
        Ok(())
    }
}

fn db_to_linear(db: f32) -> f32 {
    10_f32.powf(db / 20.0)
}

fn pan_gains(pan: f32, gain: f32) -> (f32, f32) {
    // equal power
    let p = (pan + 1.0) * 0.5; // 0..1
    let angle = p * std::f32::consts::FRAC_PI_2;
    (gain * angle.cos(), gain * angle.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_sample() -> Arc<Vec<f32>> {
        // 100ms sine 440Hz @48k
        let sr = 48000;
        let n = sr / 10;
        let v: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin() * 0.5)
            .collect();
        Arc::new(v)
    }

    #[test]
    fn note_on_off_render() {
        let mut s = Sampler::new(48000);
        s.add_zone(
            0,
            ZoneMapping {
                sample: Some(dummy_sample()),
                sample_path: None,
                sample_rate: 48000,
                root_key: 60,
                lokey: 0,
                hikey: 127,
                lovel: 0,
                hivel: 127,
                volume_db: 0.0,
                pan: 0.0,
                tune_cents: 0,
                choke_group: None,
                is_one_shot: false,
            },
        );
        s.note_on(0, 60, 100, 60);
        assert_eq!(s.num_voices(), 1);
        let mut out = vec![0.0; 512 * 2];
        s.render(&mut out);
        assert!(out.iter().any(|&x| x.abs() > 1e-5));
        s.note_off(0, 60);
        // render release
        for _ in 0..10 {
            s.render(&mut out);
        }
        // après release, voix éteinte
        // peut rester un peu selon taille sample
    }

    #[test]
    fn choke() {
        let mut s = Sampler::new(48000);
        for i in 0..2 {
            s.add_zone(
                i,
                ZoneMapping {
                    sample: Some(dummy_sample()),
                    sample_path: None,
                    sample_rate: 48000,
                    root_key: 60,
                    lokey: 0,
                    hikey: 127,
                    lovel: 0,
                    hivel: 127,
                    volume_db: 0.0,
                    pan: 0.0,
                    tune_cents: 0,
                    choke_group: Some(1),
                    is_one_shot: true,
                },
            );
        }
        s.note_on(0, 60, 100, 60);
        assert_eq!(s.num_voices(), 1);
        s.note_on(1, 60, 100, 60);
        // choke : l'ancienne voix fond en Release (5ms) au lieu d'être coupée net
        assert_eq!(s.num_voices(), 2);
        assert!(s.voices[0].adsr_phase == AdsrPhase::Release);
        // après le fade, elle disparaît
        let mut out = vec![0.0; 4096 * 2];
        s.render(&mut out);
        assert_eq!(s.num_voices(), 1);
    }
}
