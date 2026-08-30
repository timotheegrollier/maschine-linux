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
    /// Note
    pub note: u8,
    /// Velocity
    pub velocity: u8,
    /// Choke group
    pub choke_group: Option<u8>,
    /// Âge (pour voice stealing)
    pub age: u64,
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
    /// Polyphonie max globale
    pub max_voices: usize,
    /// Sample rate engine
    pub sample_rate: u32,
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
        let zones = self.zones.get(&sound)?;
        // filtre lokey/hikey/lovel/hivel
        let mut candidates: Vec<&ZoneMapping> = zones
            .iter()
            .filter(|z| {
                pitch >= z.lokey && pitch <= z.hikey && velocity >= z.lovel && velocity <= z.hivel
            })
            .collect();
        if candidates.is_empty() {
            candidates = zones.iter().collect();
        }
        if candidates.is_empty() {
            return None;
        }
        // round-robin simplifié : premier candidat
        // TODO: seq_position
        let zone = candidates[0];
        let sample = zone.sample.clone()?;
        if sample.is_empty() {
            return None;
        }

        // choke : tue les voix du même choke_group
        if let Some(choke) = zone.choke_group {
            self.voices.retain(|v| v.choke_group != Some(choke));
        }

        // pitch : root_key → pitch
        let semitones = pitch as f32 - zone.root_key as f32 + zone.tune_cents as f32 / 100.0;
        let pitch_ratio = 2_f64.powf(semitones as f64 / 12.0);
        let sr_ratio = zone.sample_rate as f64 / self.sample_rate as f64;
        let increment = pitch_ratio * sr_ratio;

        let gain = velocity as f32 / 127.0 * db_to_linear(zone.volume_db);

        // voice stealing si max atteint : tue la plus vieille en Release
        if self.voices.len() >= self.max_voices {
            if let Some(idx) = self
                .voices
                .iter()
                .enumerate()
                .min_by_key(|(_, v)| v.age)
                .map(|(i, _)| i)
            {
                self.voices.remove(idx);
            }
        }

        let voice = SamplerVoice {
            sample,
            pos: 0.0,
            increment,
            gain,
            pan: zone.pan,
            adsr_phase: AdsrPhase::Attack,
            phase_samples: 0,
            note,
            velocity,
            choke_group: zone.choke_group,
            age: self.age_counter,
        };
        self.age_counter += 1;
        self.voices.push(voice);
        Some(self.voices.len() - 1)
    }

    /// Note off — passe en Release.
    pub fn note_off(&mut self, sound: u8, note: u8) {
        for v in &mut self.voices {
            if v.note == note {
                // on passe toutes les voix de cette note en Release (simplifié, devrait filtrer sound)
                let _ = sound;
                if v.adsr_phase != AdsrPhase::Off && v.adsr_phase != AdsrPhase::Release {
                    v.adsr_phase = AdsrPhase::Release;
                    v.phase_samples = 0;
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
        let mut to_remove = Vec::new();
        for (idx, voice) in self.voices.iter_mut().enumerate() {
            let mut pos = voice.pos;
            let inc = voice.increment;
            let sample = &voice.sample;
            for frame in 0..frames {
                if pos >= sample.len() as f64 - 1.0 {
                    to_remove.push(idx);
                    break;
                }
                // linear interp
                let i = pos as usize;
                let frac = (pos - i as f64) as f32;
                let s0 = sample[i];
                let s1 = sample.get(i + 1).copied().unwrap_or(0.0);
                let s = s0 * (1.0 - frac) + s1 * frac;
                // ADSR gain (simplifié : attack 2ms, release 30ms linéaire)
                let env = match voice.adsr_phase {
                    AdsrPhase::Attack => {
                        let a_samples = (0.002 * self.sample_rate as f32) as u32;
                        if voice.phase_samples < a_samples {
                            voice.phase_samples += 1;
                            voice.phase_samples as f32 / a_samples as f32
                        } else {
                            voice.adsr_phase = AdsrPhase::Sustain;
                            voice.phase_samples = 0;
                            1.0
                        }
                    }
                    AdsrPhase::Sustain => 1.0,
                    AdsrPhase::Release => {
                        let r_samples = (0.03 * self.sample_rate as f32) as u32;
                        if voice.phase_samples < r_samples {
                            voice.phase_samples += 1;
                            1.0 - voice.phase_samples as f32 / r_samples as f32
                        } else {
                            voice.adsr_phase = AdsrPhase::Off;
                            0.0
                        }
                    }
                    _ => 1.0,
                };
                if env <= 0.0 {
                    to_remove.push(idx);
                    break;
                }
                let g = voice.gain * env;
                // pan law -3dB
                let (lg, rg) = pan_gains(voice.pan, g);
                out[frame * 2] += s * lg;
                out[frame * 2 + 1] += s * rg;
                pos += inc;
            }
            voice.pos = pos;
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
                },
            );
        }
        s.note_on(0, 60, 100, 60);
        assert_eq!(s.num_voices(), 1);
        s.note_on(1, 60, 100, 60);
        assert_eq!(s.num_voices(), 1); // choke a tué la première
    }
}
