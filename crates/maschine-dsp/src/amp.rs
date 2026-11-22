//! Multi-model tube amplifier simulation.
//!
//! Signal path (all internal processing runs at 4x the sample rate):
//!
//! ```text
//! input -> [coupling HP] -> gain stage 1 -> gain stage 2 -> gain stage 3
//!       -> tone stack -> power amp (sag + saturation) -> output
//! ```
//!
//! Each gain stage uses an asymmetric soft-clipping curve (a corrected,
//! DC-free `tanh`) which generates the even harmonics characteristic of
//! single-ended tube stages. Oversampling keeps those harmonics from
//! aliasing back into the audible band, which is what makes high-gain
//! settings sound like an amp instead of a digital fuzz.

use crate::biquad::Biquad;

/// Oversampling factor used by the nonlinear stages.
const OVERSAMPLE: usize = 4;
/// Total FIR length (must be a multiple of OVERSAMPLE).
const FIR_TAPS: usize = 64;
const TAPS_PER_PHASE: usize = FIR_TAPS / OVERSAMPLE;

/// Available amplifier voices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmpModel {
    /// Vox AC30 Top Boost — chimey clean, sag-heavy British crunch.
    VoxAc30,
    /// Fender Blackface — scooped mids, tight clean, firm low end.
    FenderBlackface,
    /// Fender Tweed — warm, mid-forward, early breakup.
    FenderTweed,
    /// Modern high-gain — tight, compressed, singing lead sustain.
    HighGain,
}

impl AmpModel {
    pub const ALL: [AmpModel; 4] = [
        AmpModel::VoxAc30,
        AmpModel::FenderBlackface,
        AmpModel::FenderTweed,
        AmpModel::HighGain,
    ];

    pub fn name(self) -> &'static str {
        match self {
            AmpModel::VoxAc30 => "Vox AC30",
            AmpModel::FenderBlackface => "Fender Blackface",
            AmpModel::FenderTweed => "Fender Tweed",
            AmpModel::HighGain => "High Gain",
        }
    }

    pub fn from_index(index: usize) -> Self {
        Self::ALL[index % Self::ALL.len()]
    }
}

/// User-facing amp controls, all normalized 0.0..=1.0 (0.5 = flat EQ).
#[derive(Debug, Clone, Copy)]
pub struct AmpSettings {
    pub gain: f32,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub presence: f32,
    pub sag: f32,
    pub master: f32,
}

impl Default for AmpSettings {
    fn default() -> Self {
        Self {
            // Gain staging is calibrated for a line-level input: 0.30 gives a
            // clean, usable level; higher values progressively drive the tubes.
            gain: 0.30,
            bass: 0.5,
            mid: 0.5,
            treble: 0.5,
            presence: 0.5,
            sag: 0.5,
            master: 0.9,
        }
    }
}

/// Niveau de sortie : un signal guitare typique (-16 dBFS crête) ressort
/// vers -8 dBFS au réglage par défaut, sans jamais saturer la sortie.
const OUTPUT_TRIM: f32 = 1.2;

/// Plage du GAIN (dB de préampli) : 0 = -6 dB (clair), 1 = +36 dB.
const DRIVE_DB: (f32, f32) = (-6.0, 36.0);
/// Crête d'un jeu de guitare typique : référence de la compensation.
const NOMINAL_INPUT: f32 = 0.15;

/// Garde-fou de sortie : linéaire jusqu'à 0,8, arrondi au-delà, jamais > 1.
/// Avec le gain compensé, un jeu normal ne l'atteint pas.
#[inline]
fn output_guard(x: f32) -> f32 {
    let a = x.abs();
    if a <= 0.8 {
        x
    } else {
        x.signum() * (0.8 + 0.2 * ((a - 0.8) / 0.2).tanh())
    }
}

/// Configuration of a single preamp gain stage.
#[derive(Clone, Copy)]
struct StageConfig {
    /// Gain multiplier applied before clipping.
    gain: f32,
    /// Asymmetry of the clipping curve (0 = symmetric, 0.6 = very even-harmonic).
    bias: f32,
    /// Coupling high-pass frequency in Hz (interstage capacitor).
    hp_hz: f32,
    /// Interstage low-pass frequency in Hz (tube Miller capacitance).
    lp_hz: f32,
}

/// Parameters describing a model's preamp, tone stack and power amp.
struct ModelConfig {
    stages: [StageConfig; 3],
    bass_hz: f32,
    mid_hz: f32,
    treble_hz: f32,
    presence_hz: f32,
    /// Overall tone-stack mid cut, in dB at the center position.
    mid_cut_db: f32,
    /// Power-amp input headroom (larger = cleaner).
    power_headroom: f32,
}

fn model_config(model: AmpModel) -> ModelConfig {
    // Stage gains are deliberately modest: the user's GAIN control provides
    // the remaining drive on top of this fixed voicing. The old values (up to
    // 9x per stage) multiplied into a ~200x chain and squared every note,
    // even at the lowest GAIN setting.
    match model {
        AmpModel::VoxAc30 => ModelConfig {
            stages: [
                StageConfig {
                    gain: 1.2,
                    bias: 0.16,
                    hp_hz: 70.0,
                    lp_hz: 12000.0,
                },
                StageConfig {
                    gain: 1.4,
                    bias: 0.13,
                    hp_hz: 90.0,
                    lp_hz: 9000.0,
                },
                StageConfig {
                    gain: 1.1,
                    bias: 0.10,
                    hp_hz: 120.0,
                    lp_hz: 7000.0,
                },
            ],
            bass_hz: 110.0,
            mid_hz: 650.0,
            treble_hz: 3800.0,
            presence_hz: 6000.0,
            mid_cut_db: -3.0,
            power_headroom: 1.4,
        },
        AmpModel::FenderBlackface => ModelConfig {
            stages: [
                StageConfig {
                    gain: 1.0,
                    bias: 0.11,
                    hp_hz: 80.0,
                    lp_hz: 14000.0,
                },
                StageConfig {
                    gain: 1.0,
                    bias: 0.09,
                    hp_hz: 100.0,
                    lp_hz: 11000.0,
                },
                StageConfig {
                    gain: 1.2,
                    bias: 0.07,
                    hp_hz: 130.0,
                    lp_hz: 8500.0,
                },
            ],
            bass_hz: 180.0,
            mid_hz: 800.0,
            treble_hz: 3200.0,
            presence_hz: 5500.0,
            mid_cut_db: -4.0,
            power_headroom: 1.5,
        },
        AmpModel::FenderTweed => ModelConfig {
            stages: [
                StageConfig {
                    gain: 1.2,
                    bias: 0.18,
                    hp_hz: 90.0,
                    lp_hz: 10000.0,
                },
                StageConfig {
                    gain: 1.4,
                    bias: 0.15,
                    hp_hz: 110.0,
                    lp_hz: 8000.0,
                },
                StageConfig {
                    gain: 1.1,
                    bias: 0.12,
                    hp_hz: 140.0,
                    lp_hz: 6500.0,
                },
            ],
            bass_hz: 140.0,
            mid_hz: 1000.0,
            treble_hz: 2800.0,
            presence_hz: 4500.0,
            mid_cut_db: 1.0,
            power_headroom: 1.3,
        },
        AmpModel::HighGain => ModelConfig {
            stages: [
                StageConfig {
                    gain: 1.6,
                    bias: 0.09,
                    hp_hz: 35.0,
                    lp_hz: 16000.0,
                },
                StageConfig {
                    gain: 2.2,
                    bias: 0.07,
                    hp_hz: 45.0,
                    lp_hz: 13000.0,
                },
                StageConfig {
                    gain: 1.8,
                    bias: 0.06,
                    hp_hz: 60.0,
                    lp_hz: 10000.0,
                },
            ],
            bass_hz: 120.0,
            mid_hz: 720.0,
            treble_hz: 4200.0,
            presence_hz: 7000.0,
            mid_cut_db: -2.0,
            power_headroom: 1.6,
        },
    }
}

/// A DC-free asymmetric soft clipper.
#[inline]
fn tube_clip(x: f32, bias: f32) -> f32 {
    // tanh(x + b) - tanh(b) has unity slope at x=0 after normalization,
    // and introduces the even-order harmonics of a biased tube.
    let tb = bias.tanh();
    let norm = (1.0 - tb * tb).max(1e-4);
    (((x + bias).tanh() - tb) / norm).clamp(-1.5, 1.5)
}

/// Polyphase FIR oversampler (4x) with preallocated buffers.
struct Oversampler {
    up_taps: [f32; FIR_TAPS],
    down_taps: [f32; FIR_TAPS],
    up_buf: [f32; FIR_TAPS],
    down_buf: [f32; FIR_TAPS],
    up_pos: usize,
    down_pos: usize,
}

impl Oversampler {
    fn new(sample_rate: f32) -> Self {
        let mut up_taps = [0.0; FIR_TAPS];
        let mut down_taps = [0.0; FIR_TAPS];
        let center = (FIR_TAPS - 1) as f32 * 0.5;
        let mut up_sum = 0.0;
        let mut down_sum = 0.0;
        for n in 0..FIR_TAPS {
            let d = n as f32 - center;
            // sinc(n / R) is the ideal lowpass with cutoff at the old Nyquist.
            let sinc = if d.abs() < 1e-6 {
                1.0
            } else {
                let x = std::f32::consts::PI * d / OVERSAMPLE as f32;
                x.sin() / x
            };
            // Lanczos window.
            let window = if d.abs() < center {
                let w = std::f32::consts::PI * d / center;
                w.sin() / w
            } else {
                0.0
            };
            let h = sinc * window;
            up_taps[n] = h * OVERSAMPLE as f32;
            down_taps[n] = h;
            up_sum += up_taps[n];
            down_sum += down_taps[n];
        }
        // Chaque phase de l'interpolateur doit avoir un gain unité : le
        // zéro-bourrage divise l'énergie par OVERSAMPLE (-12 dB sinon).
        for tap in &mut up_taps {
            *tap *= OVERSAMPLE as f32 / up_sum;
        }
        for tap in &mut down_taps {
            *tap /= down_sum;
        }
        debug_assert!(sample_rate > 0.0);
        Self {
            up_taps,
            down_taps,
            up_buf: [0.0; FIR_TAPS],
            down_buf: [0.0; FIR_TAPS],
            up_pos: 0,
            down_pos: 0,
        }
    }

    #[inline]
    fn upsample(&mut self, x: f32) -> [f32; OVERSAMPLE] {
        self.up_buf[self.up_pos] = x;
        let mut out = [0.0; OVERSAMPLE];
        for (r, out_r) in out.iter_mut().enumerate() {
            let mut acc = 0.0;
            // Polyphase : la phase r applique h[r + k·L] à x[n − k] (k
            // échantillons d'entrée en arrière, pas k·L).
            for k in 0..TAPS_PER_PHASE {
                let idx = (self.up_pos + FIR_TAPS - k) % FIR_TAPS;
                acc += self.up_taps[r + k * OVERSAMPLE] * self.up_buf[idx];
            }
            *out_r = acc;
        }
        self.up_pos = (self.up_pos + 1) % FIR_TAPS;
        out
    }

    #[inline]
    fn downsample(&mut self, x: f32) -> Option<f32> {
        self.down_buf[self.down_pos] = x;
        self.down_pos = (self.down_pos + 1) % FIR_TAPS;
        if !self.down_pos.is_multiple_of(OVERSAMPLE) {
            return None;
        }
        let mut acc = 0.0;
        for k in 0..FIR_TAPS {
            let idx = (self.down_pos + FIR_TAPS - 1 - k) % FIR_TAPS;
            acc += self.down_taps[k] * self.down_buf[idx];
        }
        Some(acc)
    }

    fn reset(&mut self) {
        self.up_buf.fill(0.0);
        self.down_buf.fill(0.0);
        self.up_pos = 0;
        self.down_pos = 0;
    }
}

/// Single tube gain stage: coupling capacitor, gain, asymmetric clipping,
/// and interstage low-pass.
struct TubeStage {
    hp: Biquad,
    lp: Biquad,
    config: StageConfig,
}

impl TubeStage {
    fn new(config: StageConfig, sample_rate: f32) -> Self {
        let hi_rate = sample_rate * OVERSAMPLE as f32;
        let mut hp = Biquad::new(hi_rate);
        hp.highpass(config.hp_hz, 0.707, hi_rate);
        let mut lp = Biquad::new(hi_rate);
        lp.lowpass(config.lp_hz, 0.707, hi_rate);
        hp.reset_state();
        lp.reset_state();
        Self { hp, lp, config }
    }

    #[inline]
    fn process(&mut self, x: f32, gain_scale: f32) -> f32 {
        let x = self.hp.process(x);
        let driven = x * self.config.gain * gain_scale;
        let clipped = tube_clip(driven, self.config.bias);
        self.lp.process(clipped)
    }

    fn reset(&mut self) {
        self.hp.reset_state();
        self.lp.reset_state();
    }
}

/// Complete multi-model tube amplifier.
pub struct TubeAmp {
    model: AmpModel,
    sample_rate: f32,
    oversampler: Oversampler,
    stages: [TubeStage; 3],
    tone_bass: Biquad,
    tone_mid: Biquad,
    tone_treble: Biquad,
    presence: Biquad,
    power_lp: Biquad,
    /// Power-supply sag envelope (tracks recent output level).
    sag_env: f32,
    sag_attack: f32,
    sag_release: f32,
    settings: AmpSettings,
    config: ModelConfig,
}

impl TubeAmp {
    pub fn new(model: AmpModel, sample_rate: f32) -> Self {
        let config = model_config(model);
        let hi_rate = sample_rate * OVERSAMPLE as f32;
        let stages = [
            TubeStage::new(config.stages[0], sample_rate),
            TubeStage::new(config.stages[1], sample_rate),
            TubeStage::new(config.stages[2], sample_rate),
        ];
        let tone_bass = Biquad::new(hi_rate);
        let tone_mid = Biquad::new(hi_rate);
        let tone_treble = Biquad::new(hi_rate);
        let presence = Biquad::new(hi_rate);
        let mut power_lp = Biquad::new(hi_rate);
        power_lp.lowpass(12000.0, 0.707, hi_rate);
        power_lp.reset_state();
        let sag_attack = 1.0 - (-1.0 / (0.005 * hi_rate)).exp();
        let sag_release = 1.0 - (-1.0 / (0.15 * hi_rate)).exp();
        let mut amp = Self {
            model,
            sample_rate,
            oversampler: Oversampler::new(sample_rate),
            stages,
            tone_bass,
            tone_mid,
            tone_treble,
            presence,
            power_lp,
            sag_env: 0.0,
            sag_attack,
            sag_release,
            settings: AmpSettings::default(),
            config,
        };
        amp.apply_settings();
        amp
    }

    pub fn model(&self) -> AmpModel {
        self.model
    }

    pub fn settings(&self) -> &AmpSettings {
        &self.settings
    }

    /// Switch amp voice. Internal filters are reconfigured and cleared.
    pub fn set_model(&mut self, model: AmpModel) {
        if model == self.model {
            return;
        }
        self.model = model;
        self.config = model_config(model);
        let hi_rate = self.sample_rate * OVERSAMPLE as f32;
        for (stage, cfg) in self.stages.iter_mut().zip(self.config.stages) {
            stage.config = cfg;
            let mut hp = Biquad::new(hi_rate);
            hp.highpass(cfg.hp_hz, 0.707, hi_rate);
            hp.reset_state();
            let mut lp = Biquad::new(hi_rate);
            lp.lowpass(cfg.lp_hz, 0.707, hi_rate);
            lp.reset_state();
            stage.hp = hp;
            stage.lp = lp;
        }
        self.config_tone();
        self.reset();
    }

    pub fn set_settings(&mut self, settings: AmpSettings) {
        self.settings = settings;
        self.apply_settings();
    }

    fn apply_settings(&mut self) {
        self.config_tone();
    }

    fn config_tone(&mut self) {
        let s = self.settings;
        let hi_rate = self.sample_rate * OVERSAMPLE as f32;
        // Normalized 0..1 -> dB around the flat point (0.5).
        let to_db = |v: f32| (v - 0.5) * 24.0;
        self.tone_bass
            .low_shelf(self.config.bass_hz, to_db(s.bass), hi_rate);
        let mid_db = to_db(s.mid) + self.config.mid_cut_db;
        self.tone_mid
            .peaking(self.config.mid_hz, mid_db, 0.9, hi_rate);
        self.tone_treble
            .high_shelf(self.config.treble_hz, to_db(s.treble), hi_rate);
        self.presence.peaking(
            self.config.presence_hz,
            to_db(s.presence) * 0.6,
            0.7,
            hi_rate,
        );
    }

    /// Process a block of samples at the base sample rate.
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) {
        // GAIN en dB comme un vrai préampli ; la compensation garde le même
        // volume perçu : le GAIN change le grain, MASTER règle le niveau.
        let gain = self.settings.gain.clamp(0.0, 1.0);
        let pre_gain = 10f32.powf((DRIVE_DB.0 + gain * (DRIVE_DB.1 - DRIVE_DB.0)) / 20.0);
        // Niveau du préampli pour un jeu nominal : linéaire tant qu'il ne
        // sature pas, plafonné ensuite ; on le ramène toujours à ~0,5.
        let stages: f32 = self.config.stages.iter().map(|s| s.gain).product();
        let makeup = 0.5 / (NOMINAL_INPUT * pre_gain * stages).tanh().max(1e-3);
        let headroom = self.config.power_headroom;

        for (x, y) in input.iter().zip(output.iter_mut()) {
            let up = self.oversampler.upsample(*x * pre_gain);
            let mut acc = 0.0;
            for hi in up {
                let mut s = hi;
                for stage in &mut self.stages {
                    s = stage.process(s, 1.0);
                }
                s = self.tone_bass.process(s);
                s = self.tone_mid.process(s);
                s = self.tone_treble.process(s);
                s = self.presence.process(s);

                // Power amp sag: loud passages momentarily reduce headroom.
                let level = s.abs();
                let coeff = if level > self.sag_env {
                    self.sag_attack
                } else {
                    self.sag_release
                };
                self.sag_env += (level - self.sag_env) * coeff;
                // Gentle supply sag: it adds dynamics without swallowing the
                // signal (the old 0.6 / 2.0 factors cut the level in half).
                let sag_amount = self.settings.sag * 0.15;
                let supply = 1.0 / (1.0 + sag_amount * self.sag_env);

                let driven = s * supply;
                let power = tube_clip(driven / headroom, 0.03) * headroom;
                let smooth = self.power_lp.process(power);
                acc += self.oversampler.downsample(smooth).unwrap_or(0.0);
            }
            *y = output_guard(acc * makeup * self.settings.master * OUTPUT_TRIM);
        }
    }

    pub fn reset(&mut self) {
        self.oversampler.reset();
        for stage in &mut self.stages {
            stage.reset();
        }
        self.tone_bass.reset_state();
        self.tone_mid.reset_state();
        self.tone_treble.reset_state();
        self.presence.reset_state();
        self.power_lp.reset_state();
        self.sag_env = 0.0;
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversampler_round_trip_is_transparent() {
        let mut os = Oversampler::new(48000.0);
        let mut out = Vec::new();
        for i in 0..4800 {
            let x = 0.5 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin();
            for hi in os.upsample(x) {
                if let Some(y) = os.downsample(hi) {
                    out.push(y);
                }
            }
        }
        let peak = out[2400..].iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((peak - 0.5).abs() < 0.02, "gain aller-retour : {peak}");
    }

    #[test]
    fn gain_changes_the_grain_not_the_volume_and_never_clips() {
        // Guitare chaude (-12 dBFS crête) : chaque modèle, du clair au saturé.
        let input: Vec<f32> = (0..9600)
            .map(|i| 0.25 * (2.0 * std::f32::consts::PI * 196.0 * i as f32 / 48000.0).sin())
            .collect();
        for model in AmpModel::ALL {
            let peaks: Vec<f32> = [0.0, 0.3, 0.6, 1.0]
                .iter()
                .map(|&gain| {
                    let mut amp = TubeAmp::new(model, 48000.0);
                    amp.set_settings(AmpSettings {
                        gain,
                        ..AmpSettings::default()
                    });
                    let mut out = vec![0.0; input.len()];
                    amp.process(&input, &mut out);
                    out[4800..].iter().fold(0.0f32, |m, s| m.max(s.abs()))
                })
                .collect();
            let (lo, hi) = peaks
                .iter()
                .fold((f32::MAX, 0.0f32), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
            assert!(hi <= 1.0, "{model:?} sature la sortie : {peaks:?}");
            assert!(lo > 0.05, "{model:?} trop faible : {peaks:?}");
            assert!(
                hi / lo < 4.0,
                "{model:?} : écart de volume > 12 dB {peaks:?}"
            );
        }
    }

    #[test]
    fn all_models_run_without_nan() {
        for model in AmpModel::ALL {
            let mut amp = TubeAmp::new(model, 48000.0);
            let input: Vec<f32> = (0..2048)
                .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 48000.0).sin())
                .collect();
            let mut output = vec![0.0; input.len()];
            amp.process(&input, &mut output);
            assert!(
                output.iter().all(|s| s.is_finite()),
                "{:?} produced NaN",
                model
            );
            assert!(
                output.iter().any(|s| s.abs() > 1e-4),
                "{:?} produced silence",
                model
            );
        }
    }

    #[test]
    fn more_gain_produces_more_harmonics() {
        let analyze = |gain: f32| {
            let mut amp = TubeAmp::new(AmpModel::VoxAc30, 48000.0);
            amp.set_settings(AmpSettings {
                gain,
                ..AmpSettings::default()
            });
            let n = 8192;
            let input: Vec<f32> = (0..n)
                .map(|i| 0.1 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48000.0).sin())
                .collect();
            let mut output = vec![0.0; n];
            amp.process(&input, &mut output);
            // Energy above the fundamental (2nd..8th harmonic).
            let mut high_energy = 0.0;
            for i in n / 2..n {
                let d = output[i] - output[i - 1];
                high_energy += d * d;
            }
            high_energy
        };
        assert!(analyze(0.9) > analyze(0.1) * 1.5);
    }

    #[test]
    fn default_settings_are_clean_and_audible() {
        let mut amp = TubeAmp::new(AmpModel::VoxAc30, 48000.0);
        let n = 16384;
        let input: Vec<f32> = (0..n)
            .map(|i| 0.05 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 48000.0).sin())
            .collect();
        let mut output = vec![0.0; n];
        amp.process(&input, &mut output);
        let tail = &output[4096..];
        let rms = (tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32).sqrt();
        assert!(rms > 0.05 && rms < 0.65, "output RMS out of range: {rms}");
        let near_clip = tail.iter().filter(|s| s.abs() > 0.95).count();
        assert!(
            near_clip < tail.len() / 100,
            "default settings clip too often: {near_clip}/{}",
            tail.len()
        );
    }

    #[test]
    fn model_switch_is_seamless() {
        let mut amp = TubeAmp::new(AmpModel::VoxAc30, 48000.0);
        amp.set_model(AmpModel::HighGain);
        assert_eq!(amp.model(), AmpModel::HighGain);
        let input = vec![0.1; 64];
        let mut output = vec![0.0; 64];
        amp.process(&input, &mut output);
        assert!(output.iter().all(|s| s.is_finite()));
    }
}
