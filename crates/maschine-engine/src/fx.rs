//! Chaîne d'effets DSP pour pads et groupes.
//!
//! Effets supportés :
//! - Reverb : réverbération algorithmique stéréo (diffusion, damping, décroissance)
//! - Delay : écho stéréo synchronisé avec feedback et filtre
//! - Distortion : overdrive analogique / saturation douce ou agressive
//! - Filter : filtre multimode résonant (LowPass / HighPass / BandPass)
//! - Compressor : compresseur de dynamique avec attaque, déclin, seuil et gain

use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Type d'effet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxType {
    Reverb,
    Delay,
    Distortion,
    Filter,
    Compressor,
}

impl FxType {
    /// Nom d'affichage de l'effet.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Reverb => "REVERB",
            Self::Delay => "DELAY",
            Self::Distortion => "DISTO",
            Self::Filter => "FILTER",
            Self::Compressor => "COMP",
        }
    }

    /// Liste de tous les types d'effets disponibles.
    pub const ALL: [Self; 5] = [
        Self::Reverb,
        Self::Delay,
        Self::Distortion,
        Self::Filter,
        Self::Compressor,
    ];
}

/// Slot d'effet configurable sur un pad ou un groupe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FxSlot {
    pub fx_type: FxType,
    pub enabled: bool,
    /// Mixage Wet/Dry (0.0 = 100% dry, 1.0 = 100% wet)
    pub mix: f32,
    /// Paramètre 1 (Reverb Size / Delay Time / Drive / Cutoff / Threshold dB)
    pub p1: f32,
    /// Paramètre 2 (Reverb Damp / Delay Feedback / Tone / Reso / Ratio)
    pub p2: f32,
    /// Paramètre 3 (Reverb Decay / Delay Tone / Sat Type / Filter Kind / Attack ms)
    pub p3: f32,
    /// Paramètre 4 (Reverb Width / PingPong / Post-Gain dB / Filter Slope / Release ms)
    pub p4: f32,
}

impl FxSlot {
    /// Crée un slot d'effet par défaut selon son type.
    #[must_use]
    pub fn default_for(fx_type: FxType) -> Self {
        match fx_type {
            FxType::Reverb => Self {
                fx_type,
                enabled: true,
                mix: 0.30,
                p1: 0.60, // Room size (0..1)
                p2: 0.40, // Damping (0..1)
                p3: 0.70, // Decay time (0..1)
                p4: 0.80, // Stereo width (0..1)
            },
            FxType::Delay => Self {
                fx_type,
                enabled: true,
                mix: 0.35,
                p1: 375.0,  // Time ms (20..1000)
                p2: 0.45,   // Feedback (0..0.95)
                p3: 5000.0, // High cut Tone Hz (500..18000)
                p4: 1.0,    // Ping-pong (0 = mono, 1 = ping-pong)
            },
            FxType::Distortion => Self {
                fx_type,
                enabled: true,
                mix: 0.50,
                p1: 0.50, // Drive (0..1)
                p2: 0.60, // Tone (0..1)
                p3: 0.0,  // Type (0 = soft tube, 1 = hard clip)
                p4: 0.0,  // Output gain dB (-12..+12)
            },
            FxType::Filter => Self {
                fx_type,
                enabled: true,
                mix: 1.0,
                p1: 1800.0, // Cutoff Hz (20..20000)
                p2: 0.40,   // Resonance (0..1)
                p3: 0.0,    // Mode (0 = LowPass, 1 = HighPass, 2 = BandPass)
                p4: 0.0,
            },
            FxType::Compressor => Self {
                fx_type,
                enabled: true,
                mix: 1.0,
                p1: -14.0, // Threshold dB (-40..0)
                p2: 4.0,   // Ratio (1..20)
                p3: 15.0,  // Attack ms (1..100)
                p4: 120.0, // Release ms (10..500)
            },
        }
    }
}

/// Processeur DSP interne pour une chaîne d'effets stéréo.
#[derive(Debug, Clone)]
pub struct FxChainProcessor {
    sample_rate: f32,
    // Delay lines
    delay_buf_l: Vec<f32>,
    delay_buf_r: Vec<f32>,
    delay_write_pos: usize,
    // Reverb comb & allpass states
    comb_bufs: [Vec<f32>; 4],
    comb_pos: [usize; 4],
    comb_filter: [f32; 4],
    allpass_bufs: [Vec<f32>; 2],
    allpass_pos: [usize; 2],
    // Filter biquad states
    filt_s1_l: f32,
    filt_s2_l: f32,
    filt_s1_r: f32,
    filt_s2_r: f32,
    // Compressor envelope
    comp_env: f32,
}

impl FxChainProcessor {
    /// Crée un nouveau processeur d'effets.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(8000) as f32;
        let max_delay_samples = (sr * 1.5) as usize;

        // Comb sizes (primes for diffuse reverb)
        let comb_lens = [
            (sr * 0.0297) as usize,
            (sr * 0.0371) as usize,
            (sr * 0.0411) as usize,
            (sr * 0.0437) as usize,
        ];
        let ap_lens = [(sr * 0.005) as usize, (sr * 0.0017) as usize];

        Self {
            sample_rate: sr,
            delay_buf_l: vec![0.0; max_delay_samples],
            delay_buf_r: vec![0.0; max_delay_samples],
            delay_write_pos: 0,
            comb_bufs: [
                vec![0.0; comb_lens[0].max(10)],
                vec![0.0; comb_lens[1].max(10)],
                vec![0.0; comb_lens[2].max(10)],
                vec![0.0; comb_lens[3].max(10)],
            ],
            comb_pos: [0; 4],
            comb_filter: [0.0; 4],
            allpass_bufs: [vec![0.0; ap_lens[0].max(10)], vec![0.0; ap_lens[1].max(10)]],
            allpass_pos: [0; 2],
            filt_s1_l: 0.0,
            filt_s2_l: 0.0,
            filt_s1_r: 0.0,
            filt_s2_r: 0.0,
            comp_env: 0.0,
        }
    }

    /// Traite un buffer audio stéréo entrelacé `[L, R, L, R, ...]` à travers la chaîne d'effets.
    pub fn process(&mut self, buffer: &mut [f32], chain: &[FxSlot]) {
        for slot in chain {
            if !slot.enabled || slot.mix <= 0.001 {
                continue;
            }
            match slot.fx_type {
                FxType::Distortion => self.process_distortion(buffer, slot),
                FxType::Filter => self.process_filter(buffer, slot),
                FxType::Compressor => self.process_compressor(buffer, slot),
                FxType::Delay => self.process_delay(buffer, slot),
                FxType::Reverb => self.process_reverb(buffer, slot),
            }
        }
    }

    fn process_distortion(&mut self, buffer: &mut [f32], slot: &FxSlot) {
        let drive = 1.0 + slot.p1 * 6.0;
        let mix = slot.mix.clamp(0.0, 1.0);
        let hard = slot.p3 > 0.5;

        for frame in buffer.chunks_mut(2) {
            let (dry_l, dry_r) = (frame[0], frame[1]);

            let wet_l = if hard {
                (dry_l * drive).clamp(-1.0, 1.0)
            } else {
                (dry_l * drive).tanh()
            };
            let wet_r = if hard {
                (dry_r * drive).clamp(-1.0, 1.0)
            } else {
                (dry_r * drive).tanh()
            };

            frame[0] = dry_l * (1.0 - mix) + wet_l * mix;
            frame[1] = dry_r * (1.0 - mix) + wet_r * mix;
        }
    }

    fn process_filter(&mut self, buffer: &mut [f32], slot: &FxSlot) {
        let sr = self.sample_rate;
        let fc = slot.p1.clamp(30.0, sr * 0.45);
        let q = (0.5 + slot.p2 * 6.0).clamp(0.1, 15.0);
        let mode = slot.p3 as usize; // 0 = LP, 1 = HP, 2 = BP
        let mix = slot.mix.clamp(0.0, 1.0);

        let w0 = 2.0 * PI * (fc / sr);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q);

        let (b0, b1, b2, a0, a1, a2) = match mode {
            1 => {
                // Highpass
                (
                    (1.0 + cos_w0) * 0.5,
                    -(1.0 + cos_w0),
                    (1.0 + cos_w0) * 0.5,
                    1.0 + alpha,
                    -2.0 * cos_w0,
                    1.0 - alpha,
                )
            }
            2 => {
                // Bandpass
                (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
            }
            _ => {
                // Lowpass
                (
                    (1.0 - cos_w0) * 0.5,
                    1.0 - cos_w0,
                    (1.0 - cos_w0) * 0.5,
                    1.0 + alpha,
                    -2.0 * cos_w0,
                    1.0 - alpha,
                )
            }
        };

        let norm = 1.0 / a0;
        let nb0 = b0 * norm;
        let nb1 = b1 * norm;
        let nb2 = b2 * norm;
        let na1 = a1 * norm;
        let na2 = a2 * norm;

        for frame in buffer.chunks_mut(2) {
            let xl = frame[0];
            let yl = nb0 * xl + self.filt_s1_l;
            self.filt_s1_l = nb1 * xl - na1 * yl + self.filt_s2_l;
            self.filt_s2_l = nb2 * xl - na2 * yl;

            let xr = frame[1];
            let yr = nb0 * xr + self.filt_s1_r;
            self.filt_s1_r = nb1 * xr - na1 * yr + self.filt_s2_r;
            self.filt_s2_r = nb2 * xr - na2 * yr;

            frame[0] = xl * (1.0 - mix) + yl * mix;
            frame[1] = xr * (1.0 - mix) + yr * mix;
        }
    }

    fn process_compressor(&mut self, buffer: &mut [f32], slot: &FxSlot) {
        let thresh_db = slot.p1.clamp(-50.0, 0.0);
        let ratio = slot.p2.clamp(1.0, 20.0);
        let attack_ms = slot.p3.clamp(1.0, 200.0);
        let release_ms = slot.p4.clamp(10.0, 1000.0);
        let mix = slot.mix.clamp(0.0, 1.0);

        let sr = self.sample_rate;
        let att_coeff = (-1.0 / (attack_ms * 0.001 * sr)).exp();
        let rel_coeff = (-1.0 / (release_ms * 0.001 * sr)).exp();

        for frame in buffer.chunks_mut(2) {
            let dry_l = frame[0];
            let dry_r = frame[1];
            let peak = dry_l.abs().max(dry_r.abs());

            let target = peak;
            if target > self.comp_env {
                self.comp_env = target + att_coeff * (self.comp_env - target);
            } else {
                self.comp_env = target + rel_coeff * (self.comp_env - target);
            }

            let env_db = if self.comp_env > 1e-5 {
                20.0 * self.comp_env.log10()
            } else {
                -100.0
            };

            let gain = if env_db > thresh_db {
                let over_db = env_db - thresh_db;
                let gr_db = over_db * (1.0 - 1.0 / ratio);
                10.0f32.powf(-gr_db / 20.0)
            } else {
                1.0
            };

            frame[0] = dry_l * (1.0 - mix) + (dry_l * gain) * mix;
            frame[1] = dry_r * (1.0 - mix) + (dry_r * gain) * mix;
        }
    }

    fn process_delay(&mut self, buffer: &mut [f32], slot: &FxSlot) {
        let time_ms = slot.p1.clamp(20.0, 1200.0);
        let feedback = slot.p2.clamp(0.0, 0.92);
        let is_pingpong = slot.p4 > 0.5;
        let mix = slot.mix.clamp(0.0, 1.0);

        let delay_len =
            ((time_ms * 0.001 * self.sample_rate) as usize).clamp(10, self.delay_buf_l.len() - 1);
        let buf_len = self.delay_buf_l.len();

        for frame in buffer.chunks_mut(2) {
            let dry_l = frame[0];
            let dry_r = frame[1];

            let read_pos = (self.delay_write_pos + buf_len - delay_len) % buf_len;
            let wet_l = self.delay_buf_l[read_pos];
            let wet_r = self.delay_buf_r[read_pos];

            if is_pingpong {
                self.delay_buf_l[self.delay_write_pos] = dry_l + wet_r * feedback;
                self.delay_buf_r[self.delay_write_pos] = dry_r + wet_l * feedback;
            } else {
                self.delay_buf_l[self.delay_write_pos] = dry_l + wet_l * feedback;
                self.delay_buf_r[self.delay_write_pos] = dry_r + wet_r * feedback;
            }

            self.delay_write_pos = (self.delay_write_pos + 1) % buf_len;

            frame[0] = dry_l * (1.0 - mix) + wet_l * mix;
            frame[1] = dry_r * (1.0 - mix) + wet_r * mix;
        }
    }

    fn process_reverb(&mut self, buffer: &mut [f32], slot: &FxSlot) {
        let room_size = slot.p1.clamp(0.2, 0.95);
        let damp = slot.p2.clamp(0.0, 0.8);
        let mix = slot.mix.clamp(0.0, 1.0);

        for frame in buffer.chunks_mut(2) {
            let dry_l = frame[0];
            let dry_r = frame[1];
            let input = (dry_l + dry_r) * 0.5;

            // 4 peignes en parallèle
            let mut comb_sum_l = 0.0f32;
            let mut comb_sum_r = 0.0f32;

            for i in 0..4 {
                let pos = self.comb_pos[i];
                let len = self.comb_bufs[i].len();
                let out = self.comb_bufs[i][pos];

                // Damping lowpass
                self.comb_filter[i] = out * (1.0 - damp) + self.comb_filter[i] * damp;
                self.comb_bufs[i][pos] = input + self.comb_filter[i] * room_size;
                self.comb_pos[i] = (pos + 1) % len;

                if i % 2 == 0 {
                    comb_sum_l += out;
                } else {
                    comb_sum_r += out;
                }
            }

            // Allpass filter de diffusion
            let ap_out_l = {
                let pos = self.allpass_pos[0];
                let len = self.allpass_bufs[0].len();
                let buf_val = self.allpass_bufs[0][pos];
                let out = -comb_sum_l + buf_val;
                self.allpass_bufs[0][pos] = comb_sum_l + buf_val * 0.5;
                self.allpass_pos[0] = (pos + 1) % len;
                out
            };

            let ap_out_r = {
                let pos = self.allpass_pos[1];
                let len = self.allpass_bufs[1].len();
                let buf_val = self.allpass_bufs[1][pos];
                let out = -comb_sum_r + buf_val;
                self.allpass_bufs[1][pos] = comb_sum_r + buf_val * 0.5;
                self.allpass_pos[1] = (pos + 1) % len;
                out
            };

            frame[0] = dry_l * (1.0 - mix) + ap_out_l * mix * 0.6;
            frame[1] = dry_r * (1.0 - mix) + ap_out_r * mix * 0.6;
        }
    }
}
