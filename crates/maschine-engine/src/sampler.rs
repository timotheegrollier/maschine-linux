//! Sampler — voix polyphonique, zones, choke, ADSR simplifié.

use std::collections::HashMap;
#[cfg(feature = "sampler")]
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Identifiant d'un Sound runtime, encodé par [`sound_id`].
pub type SoundId = u16;

/// Clé distincte par Group et slot Sound (chaque slot va de 0 à 15).
#[must_use]
pub const fn sound_id(group: u8, sound: u8) -> SoundId {
    ((group as u16) << 8) | sound as u16
}

/// Mapping zone → sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneMapping {
    /// Sample data (f32 mono, 48k) — Arc pour partage voix
    #[serde(skip)]
    pub sample: Option<Arc<Vec<f32>>>,
    /// Enveloppe de prévisualisation calculée hors du verrou audio à l'import.
    #[serde(skip)]
    pub waveform_cache: Option<Arc<[(f32, f32)]>>,
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
            waveform_cache: None,
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
    /// Pan effectif après addition des trois niveaux de canal.
    pub pan: f32,
    /// Pan de la zone sample.
    pub zone_pan: f32,
    /// Pan du Sound.
    pub sound_pan: f32,
    /// Pan du Group.
    pub group_pan: f32,
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
    pub sound: u16,
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
    /// Zones par identifiant unique (groupe × sound) → Vec<ZoneMapping>.
    zones: HashMap<u16, Vec<ZoneMapping>>,
    /// Enveloppes pré-calculées pour éviter de rescanner tout le sample à
    /// chaque snapshot GUI.
    waveform_cache: HashMap<u16, Arc<[(f32, f32)]>>,
    /// Voix actives
    voices: Vec<SamplerVoice>,
    /// Compteur âge
    age_counter: u64,
    /// Round-robin counters par Sound
    rr_counters: std::collections::HashMap<u16, u8>,
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
    pub fn add_zone(&mut self, sound: u16, zone: ZoneMapping) {
        let cache = zone.waveform_cache.clone().or_else(|| {
            zone.sample
                .as_deref()
                .filter(|sample| !sample.is_empty())
                .map(|sample| Arc::<[(f32, f32)]>::from(make_waveform_envelope(sample, 512)))
        });
        self.zones.entry(sound).or_default().push(zone);
        if let Some(cache) = cache {
            self.waveform_cache.entry(sound).or_insert(cache);
        }
    }

    /// Supprime toutes les zones d'un Sound.
    pub fn clear_sound(&mut self, sound: u16) {
        self.zones.remove(&sound);
        self.waveform_cache.remove(&sound);
    }

    /// Supprime tous les sons et leurs enveloppes pré-calculées.
    pub fn clear(&mut self) {
        self.zones.clear();
        self.waveform_cache.clear();
        self.rr_counters.clear();
        self.voices.clear();
    }

    /// Note on — choisit la zone par key/vel, crée une voix.
    pub fn note_on(&mut self, sound: u16, note: u8, velocity: u8, pitch: u8) -> Option<usize> {
        self.note_on_at(sound, note, velocity, pitch, 0)
    }

    /// Note on avec départ décalé dans le bloc audio courant (`start_frame`
    /// échantillons de silence) : placement sample-accurate des notes.
    pub fn note_on_at(
        &mut self,
        sound: u16,
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
            zone_pan: pan,
            sound_pan: 0.0,
            group_pan: 0.0,
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
            zone_pan: 0.0,
            sound_pan: 0.0,
            group_pan: 0.0,
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
    pub fn note_off(&mut self, sound: u16, note: u8) {
        for v in &mut self.voices {
            // Les percussions en mode One-Shot ne sont pas coupées au relâchement
            if v.sound == sound
                && v.note == note
                && !v.is_one_shot
                && v.adsr_phase != AdsrPhase::Off
                && v.adsr_phase != AdsrPhase::Release
            {
                v.adsr_phase = AdsrPhase::Release;
                v.phase_samples = 0;
                v.release_from = v.env_level.max(0.05);
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
        // Suppression en place : pas de Vec temporaire ni de tri dans le
        // callback audio. `swap_remove` est sûr ici, l'ordre des voix n'est pas
        // un invariant (le voice stealing compare explicitement leur âge).
        let mut idx = 0;
        while idx < self.voices.len() {
            let (remove_voice, group_peak) = {
                let voice = &mut self.voices[idx];
                // Départ décalé dans le bloc (timing sample-accurate) : si le
                // bloc rendu est plus court que le décalage, la voix démarre au
                // bloc suivant (le compteur est consommé bloc par bloc).
                if voice.start_frame >= frames {
                    voice.start_frame -= frames;
                    (false, None)
                } else {
                    let first = voice.start_frame;
                    voice.start_frame = 0;
                    let mut pos = voice.pos;
                    let inc = voice.increment;
                    let sample = &voice.sample;
                    let sample_len = sample.len() as f64;
                    let end_fade = voice.end_fade;
                    let mut voice_peak = 0.0f32;
                    let mut finished = false;
                    for frame in first..frames {
                        if pos >= sample_len - 1.0 {
                            finished = true;
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
                                    let t =
                                        voice.phase_samples as f32 / voice.release_samples as f32;
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
                            finished = true;
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
                    let peak = (voice.group < 8).then_some((voice.group as usize, voice_peak));
                    (finished || voice.adsr_phase == AdsrPhase::Off, peak)
                }
            };
            if let Some((group, peak)) = group_peak {
                if peak > self.group_peak[group] {
                    self.group_peak[group] = peak;
                }
            }
            if remove_voice {
                self.voices.swap_remove(idx);
            } else {
                idx += 1;
            }
        }
    }

    /// Multipie le gain d'une voix (pour volume de groupe).
    pub fn set_voice_gain(&mut self, idx: usize, mul: f32) {
        if let Some(v) = self.voices.get_mut(idx) {
            v.gain *= mul;
        }
    }

    /// Ajuste le gain Sound des voix déjà actives lors d'une automation UI.
    pub fn scale_sound_gain(&mut self, sound: u16, factor: f32) {
        for voice in &mut self.voices {
            if voice.sound == sound {
                voice.gain *= factor.max(0.0);
            }
        }
    }

    /// Ajuste le gain Group des voix déjà actives.
    pub fn scale_group_gain(&mut self, group: u8, factor: f32) {
        for voice in &mut self.voices {
            if voice.group == group {
                voice.gain *= factor.max(0.0);
            }
        }
    }

    /// Définit le panoramique Group des voix actives.
    pub fn update_group_pan(&mut self, group: u8, pan: f32) {
        for voice in &mut self.voices {
            if voice.group == group {
                voice.group_pan = pan.clamp(-1.0, 1.0);
                voice.pan = (voice.zone_pan + voice.sound_pan + voice.group_pan).clamp(-1.0, 1.0);
            }
        }
    }

    /// Définit le pan d'une voix.
    pub fn set_voice_pan(&mut self, idx: usize, pan: f32) {
        if let Some(v) = self.voices.get_mut(idx) {
            v.sound_pan = pan.clamp(-1.0, 1.0);
            v.pan = (v.zone_pan + v.sound_pan + v.group_pan).clamp(-1.0, 1.0);
        }
    }

    /// Définit le pan Group d'une voix nouvellement créée.
    pub fn set_voice_group_pan(&mut self, idx: usize, pan: f32) {
        if let Some(v) = self.voices.get_mut(idx) {
            v.group_pan = pan.clamp(-1.0, 1.0);
            v.pan = (v.zone_pan + v.sound_pan + v.group_pan).clamp(-1.0, 1.0);
        }
    }

    /// Repointe les zones d'un Sound vers un nouveau sample (changement de taux
    /// d'échantillonnage du moteur). Seules les zones qui pointaient encore sur
    /// l'ancien sample sont modifiées : les samples importés sont préservés.
    pub fn replace_zone_sample(
        &mut self,
        sound: u16,
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
        self.rebuild_waveform_cache(sound);
    }

    fn rebuild_waveform_cache(&mut self, sound: u16) {
        let waveform = self
            .zones
            .get(&sound)
            .and_then(|zones| zones.iter().find_map(|zone| zone.sample.as_deref()))
            .filter(|sample| !sample.is_empty())
            .map(|sample| Arc::<[(f32, f32)]>::from(make_waveform_envelope(sample, 512)));
        if let Some(waveform) = waveform {
            self.waveform_cache.insert(sound, waveform);
        } else {
            self.waveform_cache.remove(&sound);
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
    pub fn waveform_envelope(&self, sound: u16, buckets: usize) -> Option<Arc<[(f32, f32)]>> {
        if buckets == 0 {
            return None;
        }
        let cached = self.waveform_cache.get(&sound)?;
        if buckets == cached.len() {
            return Some(cached.clone());
        }
        Some(Arc::from(resample_envelope(cached, buckets)))
    }

    /// Durée en secondes du sample du Sound (0 si aucun).
    #[must_use]
    pub fn sample_duration_secs(&self, sound: u16) -> f32 {
        self.zones
            .get(&sound)
            .and_then(|zones| zones.iter().find(|zone| zone.sample.is_some()))
            .and_then(|zone| {
                zone.sample
                    .as_ref()
                    .map(|sample| (sample.len(), zone.sample_rate))
            })
            .map(|(frames, rate)| frames as f32 / rate.max(1) as f32)
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
    pub fn update_sound_filter(&mut self, sound: u16, cutoff_hz: f32, resonance: f32) {
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
        sound: u16,
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

    /// Définit le pan des voix actives d'un Sound sans effacer celui du Group.
    pub fn update_sound_pan(&mut self, sound: u16, pan: f32) {
        for v in &mut self.voices {
            if v.sound == sound {
                v.sound_pan = pan.clamp(-1.0, 1.0);
                v.pan = (v.zone_pan + v.sound_pan + v.group_pan).clamp(-1.0, 1.0);
            }
        }
    }

    /// Nombre de voix.
    #[must_use]
    pub fn num_voices(&self) -> usize {
        self.voices.len()
    }

    #[cfg(test)]
    pub(crate) fn voice_mix(&self, idx: usize) -> Option<(f32, f32)> {
        self.voices.get(idx).map(|voice| (voice.gain, voice.pan))
    }

    /// Charge un WAV mono/stéréo (via `hound`) et le convertit en mono f32.
    #[cfg(feature = "sampler")]
    pub fn prepare_wav(path: &Path, root_key: u8) -> crate::Result<ZoneMapping> {
        let mut reader = hound::WavReader::open(path).map_err(|e| {
            crate::EngineError::Sampler(format!("hound open {}: {e}", path.display()))
        })?;
        let spec = reader.spec();
        if spec.channels == 0 || spec.sample_rate == 0 || spec.bits_per_sample == 0 {
            return Err(crate::EngineError::Sampler(format!(
                "invalid WAV format: {} channels, {} Hz, {} bits",
                spec.channels, spec.sample_rate, spec.bits_per_sample
            )));
        }
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => {
                if spec.bits_per_sample != 32 {
                    return Err(crate::EngineError::Sampler(format!(
                        "unsupported float WAV width: {} bits",
                        spec.bits_per_sample
                    )));
                }
                reader
                    .samples::<f32>()
                    .map(|sample| {
                        sample.map_err(|e| {
                            crate::EngineError::Sampler(format!("invalid WAV sample: {e}"))
                        })
                    })
                    .collect::<crate::Result<Vec<_>>>()?
                    .into_iter()
                    .map(|sample| {
                        if sample.is_finite() {
                            sample.clamp(-1.0, 1.0)
                        } else {
                            0.0
                        }
                    })
                    .collect()
            }
            hound::SampleFormat::Int => {
                if spec.bits_per_sample > 32 {
                    return Err(crate::EngineError::Sampler(format!(
                        "unsupported integer WAV width: {} bits",
                        spec.bits_per_sample
                    )));
                }
                let scale = 2.0_f64.powi(i32::from(spec.bits_per_sample) - 1);
                reader
                    .samples::<i32>()
                    .map(|sample| {
                        sample
                            .map(|value| (f64::from(value) / scale) as f32)
                            .map_err(|e| {
                                crate::EngineError::Sampler(format!("invalid WAV sample: {e}"))
                            })
                    })
                    .collect::<crate::Result<Vec<_>>>()?
            }
        };
        if samples.len() % spec.channels as usize != 0 {
            return Err(crate::EngineError::Sampler(
                "WAV sample count is not divisible by channel count".to_string(),
            ));
        }
        if samples.is_empty() {
            return Err(crate::EngineError::Sampler(
                "WAV contains no audio frames".to_string(),
            ));
        }
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
        let mono = Arc::new(mono);
        let stored_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        Ok(ZoneMapping {
            sample: Some(mono.clone()),
            waveform_cache: Some(Arc::from(make_waveform_envelope(&mono, 512))),
            sample_path: Some(stored_path.to_string_lossy().to_string()),
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
        })
    }

    /// Prépare un WAV en réappliquant le mapping sauvegardé dans le projet.
    #[cfg(feature = "sampler")]
    pub fn prepare_wav_for_zone(
        path: &Path,
        mapping: &crate::project::Zone,
    ) -> crate::Result<ZoneMapping> {
        let mut zone = Self::prepare_wav(path, mapping.root_key)?;
        zone.lokey = mapping.lokey;
        zone.hikey = mapping.hikey;
        zone.lovel = mapping.lovel;
        zone.hivel = mapping.hivel;
        zone.pan = mapping.pan;
        zone.volume_db = mapping.volume_db;
        zone.tune_cents = mapping.tune_cents;
        Ok(zone)
    }

    /// Remplace un Sound par un WAV. Le décodage a lieu avant mutation, donc
    /// une erreur conserve les zones actuellement jouables.
    #[cfg(feature = "sampler")]
    pub fn load_wav(&mut self, sound: u16, path: &Path, root_key: u8) -> crate::Result<()> {
        let zone = Self::prepare_wav(path, root_key)?;
        self.clear_sound(sound);
        self.add_zone(sound, zone);
        Ok(())
    }
}

fn make_waveform_envelope(sample: &[f32], buckets: usize) -> Vec<(f32, f32)> {
    if buckets == 0 || sample.is_empty() {
        return Vec::new();
    }
    (0..buckets)
        .map(|bucket| {
            let start = bucket * sample.len() / buckets;
            let end = ((bucket + 1) * sample.len() / buckets)
                .max(start + 1)
                .min(sample.len());
            let mut min = f32::INFINITY;
            let mut max = f32::NEG_INFINITY;
            for &value in &sample[start..end] {
                min = min.min(value);
                max = max.max(value);
            }
            (min, max)
        })
        .collect()
}

fn resample_envelope(source: &[(f32, f32)], buckets: usize) -> Vec<(f32, f32)> {
    if source.is_empty() || buckets == 0 {
        return Vec::new();
    }
    (0..buckets)
        .map(|bucket| {
            let start = bucket * source.len() / buckets;
            let end = ((bucket + 1) * source.len() / buckets)
                .max(start + 1)
                .min(source.len());
            source[start..end]
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |acc, point| {
                    (acc.0.min(point.0), acc.1.max(point.1))
                })
        })
        .collect()
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
                waveform_cache: None,
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
    fn note_off_only_releases_the_matching_group_sound() {
        let mut sampler = Sampler::new(48000);
        let mut zone = ZoneMapping {
            sample: Some(dummy_sample()),
            sample_rate: 48000,
            root_key: 60,
            is_one_shot: false,
            ..Default::default()
        };
        zone.sample_path = None;
        sampler.add_zone(0x0000, zone.clone());
        sampler.add_zone(0x0100, zone);

        sampler.note_on(0x0000, 60, 100, 60);
        sampler.note_on(0x0100, 60, 100, 60);
        sampler.note_off(0x0000, 60);

        assert_eq!(sampler.voices[0].adsr_phase, AdsrPhase::Release);
        assert_eq!(sampler.voices[1].adsr_phase, AdsrPhase::Attack);
    }

    #[test]
    fn waveform_and_duration_use_cached_sample_metadata() {
        let mut sampler = Sampler::new(48000);
        sampler.add_zone(
            0x0002,
            ZoneMapping {
                sample: Some(Arc::new([-1.0, 0.5, 0.0, 1.0].repeat(6000))),
                sample_rate: 24000,
                root_key: 60,
                ..Default::default()
            },
        );

        let first = sampler.waveform_envelope(0x0002, 512).unwrap();
        let second = sampler.waveform_envelope(0x0002, 512).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(sampler.waveform_envelope(0x0002, 120).unwrap().len(), 120);
        assert!((sampler.sample_duration_secs(0x0002) - 1.0).abs() < 0.001);
    }

    #[cfg(feature = "sampler")]
    #[test]
    fn decode_32_bit_integer_wav_keeps_signed_full_scale() {
        static NEXT_FILE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = NEXT_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("maschine-wav-{}-{id}.wav", std::process::id()));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 24000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        writer.write_sample(i32::MIN).unwrap();
        writer.write_sample(i32::MAX).unwrap();
        writer.finalize().unwrap();

        let zone = Sampler::prepare_wav(&path, 60).unwrap();
        let sample = zone.sample.as_ref().unwrap();
        assert_eq!(sample[0], -1.0);
        assert!(sample[1] > 0.999_999 && sample[1] <= 1.0);
        assert_eq!(zone.waveform_cache.as_ref().unwrap().len(), 512);
        std::fs::remove_file(path).unwrap();
    }

    #[cfg(feature = "sampler")]
    #[test]
    fn decode_float_wav_sanitizes_non_finite_and_out_of_range_samples() {
        static NEXT_FILE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = NEXT_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "maschine-float-wav-{}-{id}.wav",
            std::process::id()
        ));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 24000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for sample in [f32::NAN, f32::INFINITY, 1.5, -1.5, 0.25] {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();

        let zone = Sampler::prepare_wav(&path, 60).unwrap();
        assert_eq!(
            zone.sample.unwrap().as_slice(),
            &[0.0, 0.0, 1.0, -1.0, 0.25]
        );
        std::fs::remove_file(path).unwrap();
    }

    #[cfg(feature = "sampler")]
    #[test]
    fn empty_wav_is_rejected() {
        static NEXT_FILE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = NEXT_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "maschine-empty-wav-{}-{id}.wav",
            std::process::id()
        ));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 24000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        hound::WavWriter::create(&path, spec)
            .unwrap()
            .finalize()
            .unwrap();

        assert!(Sampler::prepare_wav(&path, 60).is_err());
        std::fs::remove_file(path).unwrap();
    }

    #[cfg(feature = "sampler")]
    #[test]
    fn failed_wav_load_preserves_the_current_sound() {
        static NEXT_FILE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = NEXT_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let missing =
            std::env::temp_dir().join(format!("maschine-missing-{}-{id}.wav", std::process::id()));
        let mut sampler = Sampler::new(48000);
        sampler.add_zone(
            0x0000,
            ZoneMapping {
                sample: Some(dummy_sample()),
                sample_rate: 48000,
                root_key: 60,
                ..Default::default()
            },
        );
        let before = sampler.waveform_envelope(0x0000, 512).unwrap();

        assert!(sampler.load_wav(0x0000, &missing, 60).is_err());

        assert!(Arc::ptr_eq(
            &before,
            &sampler.waveform_envelope(0x0000, 512).unwrap()
        ));
        assert!(sampler.note_on(0x0000, 60, 100, 60).is_some());
    }

    #[test]
    fn choke() {
        let mut s = Sampler::new(48000);
        for i in 0..2 {
            s.add_zone(
                i,
                ZoneMapping {
                    sample: Some(dummy_sample()),
                    waveform_cache: None,
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
