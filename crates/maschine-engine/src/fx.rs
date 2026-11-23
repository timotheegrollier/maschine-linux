//! Effets d'insert des Sounds, Groups et du Master.
//!
//! - Filter : SVF « TPT » résonant (LP / HP / BP), coupure lissée
//! - Distortion : saturation douce (tanh) ou dure, anti-repliement ADAA,
//!   tonalité et niveau de sortie, gain compensé
//! - Compressor : stéréo lié, genou doux, gain de compensation automatique
//! - Delay : stéréo / ping-pong jusqu'à 2 s, temps lissé (pas de clic),
//!   tonalité dans la réinjection
//! - Reverb : réseau de 8 retards rebouclés (Hadamard) après 4 diffuseurs,
//!   retards modulés, amortissement des aigus, pré-délai et largeur
//! - EQ : 3 bandes (plateau grave, cloche médium réglable, plateau aigu)
//! - Chorus : double ligne modulée en quadrature (chorus, flanger, ensemble)
//! - Lo-Fi : réduction de résolution et d'échantillonnage, tonalité
//!
//! Chaque slot d'une chaîne a son propre état DSP : deux filtres dans la même
//! chaîne ne partagent plus leurs mémoires. [`FxChainProcessor::prepare`]
//! alloue ces états hors du thread audio ; [`FxChainProcessor::process`]
//! n'alloue jamais.

use serde::{Deserialize, Serialize};
use std::f32::consts::{FRAC_PI_2, LN_2, PI};

/// Type d'effet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxType {
    Reverb,
    Delay,
    Distortion,
    Filter,
    Compressor,
    Eq,
    Chorus,
    LoFi,
}

impl FxType {
    /// Nom court (écrans, statut).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Reverb => "REVERB",
            Self::Delay => "DELAY",
            Self::Distortion => "DISTO",
            Self::Filter => "FILTER",
            Self::Compressor => "COMP",
            Self::Eq => "EQ",
            Self::Chorus => "CHORUS",
            Self::LoFi => "LO-FI",
        }
    }

    /// Nom lisible (liste de la chaîne, GUI).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Reverb => "Reverb",
            Self::Delay => "Delay",
            Self::Distortion => "Distortion",
            Self::Filter => "Filter",
            Self::Compressor => "Compressor",
            Self::Eq => "EQ",
            Self::Chorus => "Chorus",
            Self::LoFi => "Lo-Fi",
        }
    }

    /// Tous les effets, dans l'ordre de la chaîne de signal usuelle (et des
    /// huit boutons de l'emplacement « + effet »).
    pub const ALL: [Self; 8] = [
        Self::Filter,
        Self::Eq,
        Self::Distortion,
        Self::Compressor,
        Self::LoFi,
        Self::Chorus,
        Self::Delay,
        Self::Reverb,
    ];
}

/// Slot d'effet d'une chaîne (Sound, Group ou Master).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxSlot {
    pub fx_type: FxType,
    pub enabled: bool,
    /// Dry/Wet (0 = 100 % dry, 1 = 100 % wet).
    pub mix: f32,
    /// Reverb Size 0..1 / Delay Time ms / Drive 0..1 / Cutoff Hz / Threshold dB
    /// / EQ Low dB / Chorus Rate Hz / Lo-Fi Bits
    pub p1: f32,
    /// Reverb Damp 0..1 / Delay Feedback 0..0.95 / Tone 0..1 / Reso 0..1 / Ratio
    /// / EQ Mid dB / Chorus Depth 0..1 / Lo-Fi Rate Hz
    pub p2: f32,
    /// Reverb Pre-delay ms / Delay Tone Hz / Type (0 soft, 1 hard) / Mode (0 LP,
    /// 1 HP, 2 BP) / Attack ms / EQ Mid Hz / Chorus Delay ms / Lo-Fi Tone Hz
    pub p3: f32,
    /// Reverb Width 0..1 / Ping-pong (0/1) / Output dB / — / Release ms
    /// / EQ High dB / Chorus Feedback 0..0.9 / —
    pub p4: f32,
}

impl FxSlot {
    /// Slot par défaut d'un type d'effet.
    #[must_use]
    pub fn default_for(fx_type: FxType) -> Self {
        let (mix, p1, p2, p3, p4) = match fx_type {
            FxType::Reverb => (0.30, 0.60, 0.40, 12.0, 0.90),
            FxType::Delay => (0.30, 375.0, 0.40, 6000.0, 1.0),
            FxType::Distortion => (1.0, 0.35, 0.60, 0.0, 0.0),
            FxType::Filter => (1.0, 1800.0, 0.25, 0.0, 0.0),
            FxType::Compressor => (1.0, -18.0, 4.0, 10.0, 120.0),
            FxType::Eq => (1.0, 0.0, 0.0, 1000.0, 0.0),
            FxType::Chorus => (0.5, 0.8, 0.5, 12.0, 0.0),
            FxType::LoFi => (1.0, 10.0, 12_000.0, 9000.0, 0.0),
        };
        Self {
            fx_type,
            enabled: true,
            mix,
            p1,
            p2,
            p3,
            p4,
        }
    }
}

/// Processeur d'une chaîne : un état DSP par slot.
#[derive(Debug, Clone)]
pub struct FxChainProcessor {
    sample_rate: f32,
    slots: Vec<SlotDsp>,
}

impl FxChainProcessor {
    /// Processeur vide : appeler [`Self::prepare`] avant de traiter une chaîne.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate.max(8000) as f32,
            slots: Vec::new(),
        }
    }

    /// L'état DSP correspond-il slot pour slot à cette chaîne ?
    #[must_use]
    pub fn is_prepared_for(&self, chain: &[FxSlot]) -> bool {
        self.slots.len() == chain.len()
            && self
                .slots
                .iter()
                .zip(chain)
                .all(|(dsp, slot)| dsp.kind() == slot.fx_type)
    }

    /// Alloue les états manquants (hors thread audio). Un slot dont le type
    /// n'a pas changé garde son état (queues de réverbe, échos en cours).
    pub fn prepare(&mut self, chain: &[FxSlot]) {
        let sr = self.sample_rate;
        let mut old = std::mem::take(&mut self.slots);
        self.slots = chain
            .iter()
            .enumerate()
            .map(|(i, slot)| match old.get_mut(i) {
                Some(dsp) if dsp.kind() == slot.fx_type => {
                    std::mem::replace(dsp, SlotDsp::new(FxType::Distortion, sr))
                }
                _ => SlotDsp::new(slot.fx_type, sr),
            })
            .collect();
    }

    /// Traite un bloc stéréo entrelacé `[L, R, L, R, ...]`. Les slots non
    /// préparés (chaîne modifiée depuis le dernier `prepare`) sont ignorés.
    pub fn process(&mut self, buffer: &mut [f32], chain: &[FxSlot]) {
        let sr = self.sample_rate;
        for (slot, dsp) in chain.iter().zip(self.slots.iter_mut()) {
            if dsp.kind() != slot.fx_type || !slot.enabled {
                continue;
            }
            match dsp {
                SlotDsp::Filter(f) => f.process(buffer, slot, sr),
                SlotDsp::Distortion(d) => d.process(buffer, slot, sr),
                SlotDsp::Compressor(c) => c.process(buffer, slot, sr),
                SlotDsp::Delay(d) => d.process(buffer, slot, sr),
                SlotDsp::Reverb(r) => r.process(buffer, slot),
                SlotDsp::Eq(e) => e.process(buffer, slot, sr),
                SlotDsp::Chorus(c) => c.process(buffer, slot, sr),
                SlotDsp::LoFi(l) => l.process(buffer, slot, sr),
            }
        }
    }
}

#[derive(Debug, Clone)]
enum SlotDsp {
    Filter(Svf),
    Distortion(Drive),
    Compressor(Comp),
    Delay(Box<StereoDelay>),
    Reverb(Box<Reverb>),
    Eq(Eq3),
    Chorus(Box<Chorus>),
    LoFi(LoFi),
}

impl SlotDsp {
    fn new(kind: FxType, sr: f32) -> Self {
        match kind {
            FxType::Filter => Self::Filter(Svf::default()),
            FxType::Distortion => Self::Distortion(Drive::default()),
            FxType::Compressor => Self::Compressor(Comp::default()),
            FxType::Delay => Self::Delay(Box::new(StereoDelay::new(sr))),
            FxType::Reverb => Self::Reverb(Box::new(Reverb::new(sr))),
            FxType::Eq => Self::Eq(Eq3::default()),
            FxType::Chorus => Self::Chorus(Box::new(Chorus::new(sr))),
            FxType::LoFi => Self::LoFi(LoFi::default()),
        }
    }

    fn kind(&self) -> FxType {
        match self {
            Self::Filter(_) => FxType::Filter,
            Self::Distortion(_) => FxType::Distortion,
            Self::Compressor(_) => FxType::Compressor,
            Self::Delay(_) => FxType::Delay,
            Self::Reverb(_) => FxType::Reverb,
            Self::Eq(_) => FxType::Eq,
            Self::Chorus(_) => FxType::Chorus,
            Self::LoFi(_) => FxType::LoFi,
        }
    }
}

/// Valeur lissée sur un bloc : rampe linéaire de la valeur précédente à la
/// nouvelle (un knob qui tourne ne produit pas de « zipper »).
#[derive(Debug, Clone, Copy, Default)]
struct Ramp {
    value: f32,
    ready: bool,
}

impl Ramp {
    /// Départ et pas par trame pour atteindre `target` en `frames` trames.
    fn begin(&mut self, target: f32, frames: usize) -> (f32, f32) {
        if !self.ready {
            self.value = target;
            self.ready = true;
        }
        let from = self.value;
        self.value = target;
        (from, (target - from) / frames.max(1) as f32)
    }
}

/// Gains dry / wet à puissance constante (effets d'espace).
fn equal_power(mix: f32) -> (f32, f32) {
    let a = mix.clamp(0.0, 1.0) * FRAC_PI_2;
    (a.cos(), a.sin())
}

/// Évite les nombres dénormalisés dans les boucles de réinjection.
#[inline]
fn flush(x: f32) -> f32 {
    if x.abs() < 1e-20 {
        0.0
    } else {
        x
    }
}

// ── Filter ──────────────────────────────────────────────────────────────

/// Filtre à variable d'état (topologie TPT, Zavalishin) : stable même
/// modulé rapidement, résonance propre jusqu'à l'auto-oscillation douce.
#[derive(Debug, Clone, Copy, Default)]
struct Svf {
    ic1: [f32; 2],
    ic2: [f32; 2],
    cutoff: f32,
    mix: Ramp,
    /// Mode du dernier bloc (0 LP, 1 HP) : la strip y lit sa bascule.
    mode: f32,
}

impl Svf {
    fn process(&mut self, buf: &mut [f32], slot: &FxSlot, sr: f32) {
        let target = slot.p1.clamp(20.0, sr * 0.45);
        if self.cutoff <= 0.0 {
            self.cutoff = target;
        }
        let k = std::f32::consts::SQRT_2 * (1.0 - 0.95 * slot.p2.clamp(0.0, 1.0));
        let mode = slot.p3.round() as i32;
        let frames = buf.len() / 2;
        let (mut mix, dmix) = self.mix.begin(slot.mix.clamp(0.0, 1.0), frames);
        // Coefficients recalculés toutes les 16 trames, coupure lissée en log.
        let glide = 1.0 - (-16.0 / (0.02 * sr)).exp();
        for block in buf.chunks_mut(32) {
            self.cutoff *= (target / self.cutoff).powf(glide);
            let g = (PI * self.cutoff / sr).tan();
            let a1 = 1.0 / (1.0 + g * (g + k));
            let a2 = g * a1;
            let a3 = g * a2;
            for frame in block.as_chunks_mut::<2>().0 {
                for (c, x) in frame.iter_mut().enumerate() {
                    let v3 = *x - self.ic2[c];
                    let v1 = a1 * self.ic1[c] + a2 * v3;
                    let v2 = self.ic2[c] + a2 * self.ic1[c] + a3 * v3;
                    self.ic1[c] = flush(2.0 * v1 - self.ic1[c]);
                    self.ic2[c] = flush(2.0 * v2 - self.ic2[c]);
                    let y = match mode {
                        1 => *x - k * v1 - v2,
                        2 => v1 * k,
                        _ => v2,
                    };
                    *x += (y - *x) * mix;
                }
                mix += dmix;
            }
        }
    }
}

// ── Distortion ──────────────────────────────────────────────────────────

/// Saturation avec anti-repliement du premier ordre (ADAA) : la non-linéarité
/// est intégrée entre deux échantillons, ce qui supprime l'essentiel du
/// « crépitement » numérique des distorsions naïves.
#[derive(Debug, Clone, Copy, Default)]
struct Drive {
    x1: [f32; 2],
    f1: [f32; 2],
    tone: [f32; 2],
    mix: Ramp,
    gain: Ramp,
}

/// Courbe de saturation (ou sa primitive, pour l'ADAA).
type Shaper = fn(f32) -> f32;

fn soft_clip(x: f32) -> f32 {
    x.tanh()
}

/// Primitive de tanh : ln(cosh(x)), stable pour |x| grand.
fn soft_clip_ad(x: f32) -> f32 {
    let a = x.abs();
    a + (-2.0 * a).exp().ln_1p() - LN_2
}

fn hard_clip(x: f32) -> f32 {
    x.clamp(-1.0, 1.0)
}

fn hard_clip_ad(x: f32) -> f32 {
    if x.abs() <= 1.0 {
        0.5 * x * x
    } else {
        x.abs() - 0.5
    }
}

impl Drive {
    fn process(&mut self, buf: &mut [f32], slot: &FxSlot, sr: f32) {
        let pre = 10f32.powf(slot.p1.clamp(0.0, 1.0) * 36.0 / 20.0);
        // Gain compensé : plus de drive = plus de grain, pas plus de volume.
        let out_gain = 10f32.powf(slot.p4.clamp(-24.0, 24.0) / 20.0) / pre.sqrt();
        let hard = slot.p3 > 0.5;
        let (f, fa): (Shaper, Shaper) = if hard {
            (hard_clip, hard_clip_ad)
        } else {
            (soft_clip, soft_clip_ad)
        };
        let fc = 800.0 * 20f32.powf(slot.p2.clamp(0.0, 1.0));
        let g = (PI * fc.min(sr * 0.45) / sr).tan();
        let tone_g = g / (1.0 + g);
        let frames = buf.len() / 2;
        let (mut mix, dmix) = self.mix.begin(slot.mix.clamp(0.0, 1.0), frames);
        let (mut gain, dgain) = self.gain.begin(out_gain, frames);
        for frame in buf.as_chunks_mut::<2>().0 {
            for (c, s) in frame.iter_mut().enumerate() {
                let x = *s * pre;
                let fx = fa(x);
                let dx = x - self.x1[c];
                let y = if dx.abs() > 1e-4 {
                    (fx - self.f1[c]) / dx
                } else {
                    f(0.5 * (x + self.x1[c]))
                };
                self.x1[c] = x;
                self.f1[c] = fx;
                // Tonalité : passe-bas un pôle (TPT) après la saturation.
                let v = (y - self.tone[c]) * tone_g;
                let lp = v + self.tone[c];
                self.tone[c] = flush(lp + v);
                *s += (lp * gain - *s) * mix;
            }
            mix += dmix;
            gain += dgain;
        }
    }
}

// ── Compressor ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
struct Comp {
    /// Réduction de gain lissée (dB, ≤ 0).
    gr_db: f32,
    mix: Ramp,
}

impl Comp {
    fn process(&mut self, buf: &mut [f32], slot: &FxSlot, sr: f32) {
        const KNEE: f32 = 6.0;
        let thresh = slot.p1.clamp(-60.0, 0.0);
        let ratio = slot.p2.clamp(1.0, 20.0);
        let slope = 1.0 / ratio - 1.0;
        let att = (-1.0 / (slot.p3.clamp(0.1, 200.0) * 0.001 * sr)).exp();
        let rel = (-1.0 / (slot.p4.clamp(5.0, 2000.0) * 0.001 * sr)).exp();
        // Compensation automatique : la moitié de la réduction à 0 dBFS.
        let makeup_db = -0.5 * slope * -thresh;
        let frames = buf.len() / 2;
        let (mut mix, dmix) = self.mix.begin(slot.mix.clamp(0.0, 1.0), frames);
        for frame in buf.as_chunks_mut::<2>().0 {
            let level = frame[0].abs().max(frame[1].abs());
            let level_db = 20.0 * (level + 1e-9).log10();
            let over = level_db - thresh;
            let target = if 2.0 * over < -KNEE {
                0.0
            } else if 2.0 * over.abs() <= KNEE {
                slope * (over + KNEE / 2.0).powi(2) / (2.0 * KNEE)
            } else {
                slope * over
            };
            let coef = if target < self.gr_db { att } else { rel };
            self.gr_db = target + coef * (self.gr_db - target);
            let gain = 10f32.powf((self.gr_db + makeup_db) / 20.0);
            for s in frame.iter_mut() {
                *s += (*s * gain - *s) * mix;
            }
            mix += dmix;
        }
    }
}

// ── Delay ───────────────────────────────────────────────────────────────

/// Delay stéréo : lecture fractionnaire d'un temps lissé (tourner TIME
/// donne un glissement « bande », jamais un clic), réinjection filtrée.
#[derive(Debug, Clone)]
struct StereoDelay {
    buf: [Vec<f32>; 2],
    pos: usize,
    /// Temps courant (échantillons), lissé vers la consigne.
    len: f32,
    tone: [f32; 2],
    low: [f32; 2],
    mix: Ramp,
}

impl StereoDelay {
    fn new(sr: f32) -> Self {
        let n = (sr * 2.0) as usize + 4;
        Self {
            buf: [vec![0.0; n], vec![0.0; n]],
            pos: 0,
            len: 0.0,
            tone: [0.0; 2],
            low: [0.0; 2],
            mix: Ramp::default(),
        }
    }

    fn read(&self, c: usize, delay: f32) -> f32 {
        let n = self.buf[c].len();
        let rp = self.pos as f32 + n as f32 - delay;
        let i = rp as usize;
        let frac = rp - i as f32;
        let a = self.buf[c][i % n];
        let b = self.buf[c][(i + 1) % n];
        a + (b - a) * frac
    }

    fn process(&mut self, buf: &mut [f32], slot: &FxSlot, sr: f32) {
        let n = self.buf[0].len();
        let target = (slot.p1.clamp(1.0, 2000.0) * 0.001 * sr).clamp(1.0, n as f32 - 3.0);
        if self.len <= 0.0 {
            self.len = target;
        }
        let glide = 1.0 - (-1.0 / (0.06 * sr)).exp();
        let feedback = slot.p2.clamp(0.0, 0.95);
        let g = (PI * slot.p3.clamp(200.0, sr * 0.45) / sr).tan();
        let tone_g = g / (1.0 + g);
        // Coupe-bas fixe (~90 Hz) dans la boucle : les basses ne s'accumulent pas.
        let low_g = {
            let g = (PI * 90.0 / sr).tan();
            g / (1.0 + g)
        };
        let ping_pong = slot.p4 > 0.5;
        let frames = buf.len() / 2;
        let (mut mix, dmix) = self.mix.begin(slot.mix.clamp(0.0, 1.0), frames);
        for frame in buf.as_chunks_mut::<2>().0 {
            self.len += (target - self.len) * glide;
            let wet = [self.read(0, self.len), self.read(1, self.len)];
            let mut fb = [0.0f32; 2];
            for c in 0..2 {
                let v = (wet[c] - self.tone[c]) * tone_g;
                let lp = v + self.tone[c];
                self.tone[c] = flush(lp + v);
                let v = (lp - self.low[c]) * low_g;
                let low = v + self.low[c];
                self.low[c] = flush(low + v);
                // Saturation douce : une réinjection forte reste bornée.
                fb[c] = soft_clip((lp - low) * feedback);
            }
            let (dry_l, dry_r) = (frame[0], frame[1]);
            let (w0, w1) = if ping_pong {
                (0.5 * (dry_l + dry_r) + fb[1], fb[0])
            } else {
                (dry_l + fb[0], dry_r + fb[1])
            };
            self.buf[0][self.pos] = w0;
            self.buf[1][self.pos] = w1;
            self.pos = (self.pos + 1) % n;
            let (dg, wg) = equal_power(mix);
            frame[0] = dry_l * dg + wet[0] * wg;
            frame[1] = dry_r * dg + wet[1] * wg;
            mix += dmix;
        }
    }
}

// ── Reverb (réseau de retards rebouclés) ────────────────────────────────

/// Durée de la queue (RT60, secondes) selon SIZE : de la petite pièce
/// (0,3 s) au grand hall (3,6 s). Écran et DSP lisent la même courbe.
#[must_use]
pub fn reverb_decay_secs(size: f32) -> f32 {
    0.3 * 12f32.powf(size.clamp(0.0, 1.0))
}

/// Passe-tout de Schroeder : densifie l'entrée sans la colorer.
#[derive(Debug, Clone)]
struct Diffuser {
    buf: Vec<f32>,
    idx: usize,
    g: f32,
}

impl Diffuser {
    fn new(len: usize, g: f32) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            idx: 0,
            g,
        }
    }

    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        let delayed = self.buf[self.idx];
        let w = flush(x + self.g * delayed);
        self.buf[self.idx] = w;
        self.idx = (self.idx + 1) % self.buf.len();
        delayed - self.g * w
    }
}

/// Hadamard 8×8 normalisée (sans perte) : chaque retard nourrit tous les autres.
#[inline]
fn hadamard8(x: &mut [f32; 8]) {
    for h in [1, 2, 4] {
        for i in (0..8).step_by(h * 2) {
            for j in i..i + h {
                let (a, b) = (x[j], x[j + h]);
                x[j] = a + b;
                x[j + h] = a - b;
            }
        }
    }
    for v in x.iter_mut() {
        *v *= std::f32::consts::FRAC_1_SQRT_2 * 0.5;
    }
}

/// Réverbe à 8 retards rebouclés par une matrice de Hadamard (FDN), après
/// quatre diffuseurs : queue dense et lisse, sans le « métal » des peignes
/// parallèles. Les retards ondulent lentement (quelques échantillons) pour
/// casser les résonances fixes ; les aigus s'amortissent à chaque passage.
#[derive(Debug, Clone)]
struct Reverb {
    diffusers: [Diffuser; 4],
    lines: [Vec<f32>; 8],
    idx: [usize; 8],
    lp: [f32; 8],
    pre: Vec<f32>,
    pre_pos: usize,
    /// LFO de modulation : phaseur (cos, sin) tourné à chaque échantillon.
    lfo: (f32, f32),
    sr: f32,
    mix: Ramp,
}

impl Reverb {
    /// Retards du réseau (ms), sans rapport simple entre eux.
    const LINES_MS: [f32; 8] = [31.7, 37.1, 41.1, 43.7, 53.0, 59.9, 67.7, 73.3];
    /// Diffuseurs d'entrée : (ms, gain).
    const DIFFUSERS: [(f32, f32); 4] =
        [(4.771, 0.75), (3.595, 0.75), (12.73, 0.625), (9.307, 0.625)];
    /// Profondeur (échantillons à 48 kHz) et vitesse de la modulation.
    const MOD_DEPTH: f32 = 12.0;
    const MOD_HZ: f32 = 0.6;
    /// Signes d'injection et des deux sorties : trois lignes de Hadamard,
    /// gauche et droite décorrélées.
    const IN: [f32; 8] = [1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0];
    const OUT_L: [f32; 8] = [1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0];
    const OUT_R: [f32; 8] = [1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0];
    /// Niveau de sortie : même volume perçu que l'ancienne réverbe.
    const OUT_GAIN: f32 = 1.0;

    fn new(sr: f32) -> Self {
        let samples = |ms: f32| (ms * 0.001 * sr) as usize;
        let depth = (Self::MOD_DEPTH * sr / 48_000.0) as usize;
        Self {
            diffusers: Self::DIFFUSERS.map(|(ms, g)| Diffuser::new(samples(ms), g)),
            lines: Self::LINES_MS.map(|ms| vec![0.0; samples(ms) + depth + 4]),
            idx: [0; 8],
            lp: [0.0; 8],
            pre: vec![0.0; (sr * 0.1) as usize + 2],
            pre_pos: 0,
            lfo: (1.0, 0.0),
            sr,
            mix: Ramp::default(),
        }
    }

    fn process(&mut self, buf: &mut [f32], slot: &FxSlot) {
        let sr = self.sr;
        let rt60 = reverb_decay_secs(slot.p1);
        let gains = Self::LINES_MS.map(|ms| 10f32.powf(-3.0 * ms * 0.001 / rt60));
        // Passe-bas un pôle dans chaque boucle : 0 = brillant, 1 = sombre.
        let damp = 0.45 * slot.p2.clamp(0.0, 1.0);
        let n = self.pre.len();
        let pre = ((slot.p3.clamp(0.0, 100.0) * 0.001 * sr) as usize).min(n - 1);
        let width = slot.p4.clamp(0.0, 1.0);
        let wet1 = width * 0.5 + 0.5;
        let wet2 = (1.0 - width) * 0.5;
        let depth = Self::MOD_DEPTH * sr / 48_000.0;
        let (sw, cw) = (2.0 * PI * Self::MOD_HZ / sr).sin_cos();
        let frames = buf.len() / 2;
        let (mut mix, dmix) = self.mix.begin(slot.mix.clamp(0.0, 1.0), frames);
        for frame in buf.as_chunks_mut::<2>().0 {
            let (dry_l, dry_r) = (frame[0], frame[1]);
            self.pre[self.pre_pos] = (dry_l + dry_r) * 0.5;
            let mut x = self.pre[(self.pre_pos + n - pre) % n];
            self.pre_pos = (self.pre_pos + 1) % n;
            for d in &mut self.diffusers {
                x = d.tick(x);
            }
            let (c, s) = self.lfo;
            self.lfo = (c * cw - s * sw, s * cw + c * sw);
            let mut y = [0.0f32; 8];
            for (i, yi) in y.iter_mut().enumerate() {
                let line = &self.lines[i];
                let len = line.len();
                // Quatre phases du LFO (cos, sin, −cos, −sin) sur les 8 retards.
                let m = if i % 2 == 0 { c } else { s } * if i & 2 == 0 { 1.0 } else { -1.0 };
                let delay = (len - 4) as f32 - depth * 0.5 * (1.0 - m);
                let rp = self.idx[i] as f32 + len as f32 - delay;
                let i0 = rp as usize;
                let frac = rp - i0 as f32;
                let (a, b) = (line[i0 % len], line[(i0 + 1) % len]);
                let v = a + (b - a) * frac;
                self.lp[i] = flush(v + (self.lp[i] - v) * damp);
                *yi = self.lp[i] * gains[i];
            }
            let out_l: f32 = y.iter().zip(Self::OUT_L).map(|(v, k)| v * k).sum();
            let out_r: f32 = y.iter().zip(Self::OUT_R).map(|(v, k)| v * k).sum();
            hadamard8(&mut y);
            for (i, v) in y.iter().enumerate() {
                let len = self.lines[i].len();
                self.lines[i][self.idx[i]] = flush(v + x * Self::IN[i]);
                self.idx[i] = (self.idx[i] + 1) % len;
            }
            let (out_l, out_r) = (out_l * Self::OUT_GAIN, out_r * Self::OUT_GAIN);
            let (dg, wg) = equal_power(mix);
            frame[0] = dry_l * dg + (out_l * wet1 + out_r * wet2) * wg;
            frame[1] = dry_r * dg + (out_r * wet1 + out_l * wet2) * wg;
            mix += dmix;
        }
        // Le phaseur garde son module (les arrondis le feraient dériver).
        let (c, s) = self.lfo;
        let norm = (c * c + s * s).sqrt().max(1e-6);
        self.lfo = (c / norm, s / norm);
    }
}

// ── EQ 3 bandes ─────────────────────────────────────────────────────────

/// Coefficients normalisés d'un biquad (a0 = 1).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Biquad {
    pub b: [f32; 3],
    pub a: [f32; 2],
}

impl Biquad {
    /// Plateau grave / aigu (RBJ, pente S = 1) ou cloche (Q donné), gain en dB.
    #[must_use]
    pub fn shelf_or_peak(kind: u8, freq: f32, gain_db: f32, q: f32, sr: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq.clamp(10.0, sr * 0.45) / sr;
        let (sn, cs) = w0.sin_cos();
        let (b0, b1, b2, a0, a1, a2) = match kind {
            // Plateau grave
            0 => {
                let alpha = sn / 2.0 * std::f32::consts::SQRT_2;
                let k = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cs + k),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cs),
                    a * ((a + 1.0) - (a - 1.0) * cs - k),
                    (a + 1.0) + (a - 1.0) * cs + k,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cs),
                    (a + 1.0) + (a - 1.0) * cs - k,
                )
            }
            // Plateau aigu
            2 => {
                let alpha = sn / 2.0 * std::f32::consts::SQRT_2;
                let k = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cs + k),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
                    a * ((a + 1.0) + (a - 1.0) * cs - k),
                    (a + 1.0) - (a - 1.0) * cs + k,
                    2.0 * ((a - 1.0) - (a + 1.0) * cs),
                    (a + 1.0) - (a - 1.0) * cs - k,
                )
            }
            // Cloche
            _ => {
                let alpha = sn / (2.0 * q.max(0.1));
                (
                    1.0 + alpha * a,
                    -2.0 * cs,
                    1.0 - alpha * a,
                    1.0 + alpha / a,
                    -2.0 * cs,
                    1.0 - alpha / a,
                )
            }
        };
        Self {
            b: [b0 / a0, b1 / a0, b2 / a0],
            a: [a1 / a0, a2 / a0],
        }
    }

    /// Gain (linéaire) à la fréquence `freq` (courbes des écrans).
    #[must_use]
    pub fn magnitude(&self, freq: f32, sr: f32) -> f32 {
        let w = 2.0 * PI * freq / sr;
        let (s1, c1) = w.sin_cos();
        let (s2, c2) = (2.0 * w).sin_cos();
        let nr = self.b[0] + self.b[1] * c1 + self.b[2] * c2;
        let ni = -(self.b[1] * s1 + self.b[2] * s2);
        let dr = 1.0 + self.a[0] * c1 + self.a[1] * c2;
        let di = -(self.a[0] * s1 + self.a[1] * s2);
        ((nr * nr + ni * ni) / (dr * dr + di * di).max(1e-12)).sqrt()
    }
}

/// Les trois bandes d'un réglage d'EQ (grave 120 Hz, médium, aigu 8 kHz).
#[must_use]
pub fn eq_bands(slot: &FxSlot, sr: f32) -> [Biquad; 3] {
    [
        Biquad::shelf_or_peak(0, 120.0, slot.p1.clamp(-18.0, 18.0), 0.7, sr),
        Biquad::shelf_or_peak(
            1,
            slot.p3.clamp(80.0, 12_000.0),
            slot.p2.clamp(-18.0, 18.0),
            0.9,
            sr,
        ),
        Biquad::shelf_or_peak(2, 8000.0, slot.p4.clamp(-18.0, 18.0), 0.7, sr),
    ]
}

/// EQ 3 bandes en série (forme directe II transposée par canal).
#[derive(Debug, Clone, Copy, Default)]
struct Eq3 {
    z: [[[f32; 2]; 3]; 2],
    mix: Ramp,
}

impl Eq3 {
    fn process(&mut self, buf: &mut [f32], slot: &FxSlot, sr: f32) {
        let bands = eq_bands(slot, sr);
        let frames = buf.len() / 2;
        let (mut mix, dmix) = self.mix.begin(slot.mix.clamp(0.0, 1.0), frames);
        for frame in buf.as_chunks_mut::<2>().0 {
            for (c, s) in frame.iter_mut().enumerate() {
                let mut x = *s;
                for (k, q) in bands.iter().enumerate() {
                    let z = &mut self.z[c][k];
                    let y = q.b[0] * x + z[0];
                    z[0] = flush(q.b[1] * x - q.a[0] * y + z[1]);
                    z[1] = flush(q.b[2] * x - q.a[1] * y);
                    x = y;
                }
                *s += (x - *s) * mix;
            }
            mix += dmix;
        }
    }
}

// ── Chorus / flanger ────────────────────────────────────────────────────

/// Ligne à retard modulée par deux LFO en quadrature (gauche / droite) :
/// chorus (quelques ms, sans réinjection) jusqu'au flanger (retard court,
/// réinjection forte).
#[derive(Debug, Clone)]
struct Chorus {
    buf: [Vec<f32>; 2],
    pos: usize,
    phase: f32,
    mix: Ramp,
}

impl Chorus {
    fn new(sr: f32) -> Self {
        let n = (sr * 0.06) as usize + 4;
        Self {
            buf: [vec![0.0; n], vec![0.0; n]],
            pos: 0,
            phase: 0.0,
            mix: Ramp::default(),
        }
    }

    fn process(&mut self, buf: &mut [f32], slot: &FxSlot, sr: f32) {
        let n = self.buf[0].len();
        let rate = slot.p1.clamp(0.05, 10.0);
        let depth_ms = slot.p2.clamp(0.0, 1.0) * 6.0;
        let base_ms = slot.p3.clamp(0.5, 30.0);
        let feedback = slot.p4.clamp(0.0, 0.9);
        let inc = rate / sr;
        let frames = buf.len() / 2;
        let (mut mix, dmix) = self.mix.begin(slot.mix.clamp(0.0, 1.0), frames);
        for frame in buf.as_chunks_mut::<2>().0 {
            let mut wet = [0.0f32; 2];
            for (c, w) in wet.iter_mut().enumerate() {
                let lfo = (2.0 * PI * (self.phase + c as f32 * 0.25)).sin();
                let delay = ((base_ms + depth_ms * 0.5 * (1.0 + lfo)) * 0.001 * sr)
                    .clamp(1.0, n as f32 - 3.0);
                let rp = self.pos as f32 + n as f32 - delay;
                let i = rp as usize;
                let frac = rp - i as f32;
                let a = self.buf[c][i % n];
                let b = self.buf[c][(i + 1) % n];
                *w = a + (b - a) * frac;
            }
            for c in 0..2 {
                self.buf[c][self.pos] = flush(frame[c] + soft_clip(wet[c] * feedback));
            }
            self.pos = (self.pos + 1) % n;
            self.phase = (self.phase + inc).fract();
            // MIX 50 % = chorus classique, 100 % = vibrato.
            for (s, w) in frame.iter_mut().zip(wet) {
                *s += (w - *s) * mix;
            }
            mix += dmix;
        }
    }
}

// ── Lo-Fi ───────────────────────────────────────────────────────────────

/// Réduction de résolution (bits) et d'échantillonnage (échantillonneur-
/// bloqueur), puis tonalité : le grain des boîtes à rythmes 8/12 bits.
#[derive(Debug, Clone, Copy, Default)]
struct LoFi {
    held: [f32; 2],
    acc: f32,
    tone: [f32; 2],
    mix: Ramp,
}

impl LoFi {
    fn process(&mut self, buf: &mut [f32], slot: &FxSlot, sr: f32) {
        let levels = 2f32.powf(slot.p1.clamp(2.0, 16.0).round() - 1.0);
        let step = (slot.p2.clamp(500.0, sr) / sr).min(1.0);
        let g = (PI * slot.p3.clamp(200.0, sr * 0.45) / sr).tan();
        let tone_g = g / (1.0 + g);
        let frames = buf.len() / 2;
        let (mut mix, dmix) = self.mix.begin(slot.mix.clamp(0.0, 1.0), frames);
        for frame in buf.as_chunks_mut::<2>().0 {
            self.acc += step;
            if self.acc >= 1.0 {
                self.acc -= 1.0;
                for (held, f) in self.held.iter_mut().zip(frame.iter()) {
                    *held = (f.clamp(-1.0, 1.0) * levels).round() / levels;
                }
            }
            for (c, s) in frame.iter_mut().enumerate() {
                let v = (self.held[c] - self.tone[c]) * tone_g;
                let y = v + self.tone[c];
                self.tone[c] = flush(y + v);
                *s += (y - *s) * mix;
            }
            mix += dmix;
        }
    }
}

// ── Smart Strip : MOD FX et PERFORM FX ─────────────────────────────────

/// Effet joué à la Smart Strip sur le Master : balayages en MOD, effets
/// rythmés en PERFORM (comme les Perform FX de la MK3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StripFx {
    Filter,
    Flanger,
    Stutter,
    Echo,
    Tremolo,
}

impl StripFx {
    /// Effets du mode MOD (SHIFT + MOD : suivant).
    pub const MOD: [Self; 2] = [Self::Filter, Self::Flanger];
    /// Effets du mode PERFORM (SHIFT + PERFORM, « FX Select » : suivant).
    pub const PERFORM: [Self; 3] = [Self::Stutter, Self::Echo, Self::Tremolo];

    /// Nom (écrans, statut).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Filter => "FILTRE",
            Self::Flanger => "FLANGER",
            Self::Stutter => "STUTTER",
            Self::Echo => "ÉCHO",
            Self::Tremolo => "TRÉMOLO",
        }
    }

    /// Réglage donné par la position du doigt (0 = gauche, 1 = droite).
    #[must_use]
    pub fn setting(self, x: f32) -> String {
        match self {
            Self::Filter if x < 0.5 => format!("passe-bas {}", hz_label(strip_cutoff(x).0)),
            Self::Filter => format!("passe-haut {}", hz_label(strip_cutoff(x).0)),
            Self::Flanger => format!("{:.1} ms", flanger_ms(x)),
            Self::Stutter | Self::Tremolo => strip_division(x).1.to_string(),
            Self::Echo => echo_division(x).1.to_string(),
        }
    }
}

fn hz_label(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1} kHz", hz / 1000.0)
    } else {
        format!("{hz:.0} Hz")
    }
}

/// Filtre de la strip : passe-bas qui se ferme vers la gauche (18 kHz →
/// 120 Hz), passe-haut qui monte vers la droite (20 Hz → 5 kHz). Le centre
/// est transparent. Renvoie (coupure, mode 0 LP / 1 HP).
fn strip_cutoff(x: f32) -> (f32, f32) {
    let x = x.clamp(0.0, 1.0);
    if x < 0.5 {
        (120.0 * 150f32.powf(x * 2.0), 0.0)
    } else {
        (20.0 * 250f32.powf((x - 0.5) * 2.0), 1.0)
    }
}

fn flanger_ms(x: f32) -> f32 {
    0.5 + 7.5 * x.clamp(0.0, 1.0)
}

/// Division (en noires) des effets rythmés : 1/4 à gauche … 1/32 à droite.
fn strip_division(x: f32) -> (f32, &'static str) {
    const DIVS: [(f32, &str); 4] = [(1.0, "1/4"), (0.5, "1/8"), (0.25, "1/16"), (0.125, "1/32")];
    DIVS[((x.clamp(0.0, 1.0) * 4.0) as usize).min(3)]
}

/// Temps d'écho : 1/4, 1/8 pointée, 1/8, 1/16.
fn echo_division(x: f32) -> (f32, &'static str) {
    const DIVS: [(f32, &str); 4] = [(1.0, "1/4"), (0.75, "1/8 P"), (0.5, "1/8"), (0.25, "1/16")];
    DIVS[((x.clamp(0.0, 1.0) * 4.0) as usize).min(3)]
}

/// Effets de la Smart Strip sur le Master : actifs tant qu'un doigt est
/// posé (la position règle l'effet), ils s'effacent en quelques ms au
/// relâchement — l'écho, lui, garde sa queue. Aucune allocation dans
/// `process`.
#[derive(Debug, Clone)]
pub struct PerformFx {
    sr: f32,
    filter: Svf,
    flanger: Box<Chorus>,
    echo: Box<StereoDelay>,
    /// Tranche de travail : effet d'insert, puis envoi d'écho.
    scratch: [f32; 64],
    send: [f32; 64],
    /// Mémoire du stutter : ce qui joue depuis le toucher (2 s stéréo).
    stutter: Vec<f32>,
    /// Effet en cours (le précédent finit de s'effacer avant le suivant).
    active: Option<StripFx>,
    touching: bool,
    /// Trames depuis le toucher (stutter) et phase du trémolo (0..1).
    t: usize,
    phase: f32,
    /// Position lissée du doigt, niveau d'effet (0 = sec), queue d'écho.
    x: f32,
    wet: f32,
    echo_tail: usize,
}

impl PerformFx {
    /// Tout est réservé ici (hors thread audio).
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(8000) as f32;
        Self {
            sr,
            filter: Svf::default(),
            flanger: Box::new(Chorus::new(sr)),
            echo: Box::new(StereoDelay::new(sr)),
            scratch: [0.0; 64],
            send: [0.0; 64],
            stutter: vec![0.0; (sr * 2.0) as usize * 2],
            active: None,
            touching: false,
            t: 0,
            phase: 0.0,
            x: 0.5,
            wet: 0.0,
            echo_tail: 0,
        }
    }

    /// Traite un bloc stéréo entrelacé : `fx` = effet du mode de la strip
    /// (None hors MOD / PERFORM), `touch` = position du doigt (0..1),
    /// `bpm` pour les effets rythmés.
    pub fn process(&mut self, buf: &mut [f32], fx: Option<StripFx>, touch: Option<f32>, bpm: f32) {
        if fx != self.active && self.wet < 1e-4 {
            self.active = fx;
            self.wet = 0.0;
        }
        let touch = touch.filter(|_| fx == self.active);
        if let Some(x) = touch {
            if !self.touching {
                // Nouveau toucher : le stutter enregistre à partir d'ici.
                self.t = 0;
                self.phase = 0.0;
                self.x = x;
            }
        }
        self.touching = touch.is_some();
        let sr = self.sr;
        let echo_on = self.active == Some(StripFx::Echo);
        if echo_on && self.touching {
            self.echo_tail = (sr * 8.0) as usize;
        }
        if !self.touching && self.wet < 1e-4 && self.echo_tail == 0 {
            self.wet = 0.0;
            return;
        }
        let target = f32::from(u8::from(self.touching));
        let beat = sr * 60.0 / bpm.clamp(20.0, 400.0);
        // Fondus : niveau ~8 ms, position ~15 ms (par tranche de 32 trames).
        let k_wet = 1.0 - (-1.0 / (0.008 * sr)).exp();
        let k_x = 1.0 - (-32.0 / (0.015 * sr)).exp();
        for chunk in buf.chunks_mut(64) {
            if let Some(x) = touch {
                self.x += (x - self.x) * k_x;
            }
            let x = self.x;
            let n = chunk.len();
            if let Some(fx) = self.active.filter(|&f| f != StripFx::Echo) {
                let wet = &mut self.scratch[..n];
                wet.copy_from_slice(chunk);
                // Filtre : sec au centre, où il bascule de passe-bas à passe-haut.
                let mut presence = 1.0;
                match fx {
                    StripFx::Filter => {
                        let (cutoff, mode) = strip_cutoff(x);
                        if mode != self.filter.mode {
                            self.filter = Svf {
                                cutoff,
                                mode,
                                ..Svf::default()
                            };
                        }
                        let slot = FxSlot {
                            p1: cutoff,
                            p2: 0.55,
                            p3: mode,
                            ..FxSlot::default_for(FxType::Filter)
                        };
                        self.filter.process(wet, &slot, sr);
                        presence = ((x - 0.5).abs() / 0.04).min(1.0);
                    }
                    StripFx::Flanger => {
                        let slot = FxSlot {
                            mix: 0.5,
                            p1: 0.05,
                            p2: 0.0,
                            p3: flanger_ms(x),
                            p4: 0.75,
                            ..FxSlot::default_for(FxType::Chorus)
                        };
                        self.flanger.process(wet, &slot, sr);
                    }
                    StripFx::Stutter => {
                        let cap = self.stutter.len() / 2;
                        let slice = ((strip_division(x).0 * beat) as usize).clamp(64, cap);
                        let fade = (sr * 0.002) as usize;
                        for s in wet.as_chunks_mut::<2>().0 {
                            let t = self.t;
                            if t < cap {
                                self.stutter[t * 2] = s[0];
                                self.stutter[t * 2 + 1] = s[1];
                            }
                            if t >= slice {
                                // Répète la tranche, fondu de 2 ms aux raccords.
                                let p = t % slice;
                                let g = (p.min(slice - p) as f32 / fade as f32).min(1.0);
                                s[0] = self.stutter[p * 2] * g;
                                s[1] = self.stutter[p * 2 + 1] * g;
                            }
                            self.t = t.saturating_add(1);
                        }
                    }
                    StripFx::Tremolo => {
                        let inc = 1.0 / (strip_division(x).0 * beat);
                        for s in wet.as_chunks_mut::<2>().0 {
                            let g = 1.0 - 0.9 * (0.5 - 0.5 * (2.0 * PI * self.phase).cos());
                            s[0] *= g;
                            s[1] *= g;
                            self.phase = (self.phase + inc).fract();
                        }
                    }
                    StripFx::Echo => {}
                }
                for (d, w) in chunk
                    .as_chunks_mut::<2>()
                    .0
                    .iter_mut()
                    .zip(self.scratch[..n].as_chunks::<2>().0)
                {
                    self.wet += (target - self.wet) * k_wet;
                    let g = self.wet * presence;
                    d[0] += (w[0] - d[0]) * g;
                    d[1] += (w[1] - d[1]) * g;
                }
            }
            if self.echo_tail > 0 {
                // Écho : envoi tant que le doigt est posé, la queue continue.
                let send = &mut self.send[..n];
                for (o, i) in send
                    .as_chunks_mut::<2>()
                    .0
                    .iter_mut()
                    .zip(chunk.as_chunks::<2>().0)
                {
                    if echo_on {
                        self.wet += (target - self.wet) * k_wet;
                    }
                    let level = if echo_on { self.wet } else { 0.0 };
                    o[0] = i[0] * level;
                    o[1] = i[1] * level;
                }
                let slot = FxSlot {
                    mix: 1.0,
                    p1: echo_division(x).0 * beat / sr * 1000.0,
                    p2: 0.55,
                    p3: 5000.0,
                    p4: 1.0,
                    ..FxSlot::default_for(FxType::Delay)
                };
                self.echo.process(send, &slot, sr);
                for (d, w) in chunk.iter_mut().zip(send.iter()) {
                    *d += *w;
                }
            }
        }
        if !(echo_on && self.touching) {
            self.echo_tail = self.echo_tail.saturating_sub(buf.len() / 2);
        }
        if !self.touching && self.wet < 1e-4 {
            self.wet = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn sine(freq: f32, amp: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let s = amp * (2.0 * PI * freq * i as f32 / SR as f32).sin();
                [s, s]
            })
            .collect()
    }

    fn peak(buf: &[f32]) -> f32 {
        buf.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    fn run(chain: &[FxSlot], input: &[f32]) -> Vec<f32> {
        let mut p = FxChainProcessor::new(SR);
        p.prepare(chain);
        let mut out = input.to_vec();
        for block in out.chunks_mut(512) {
            p.process(block, chain);
        }
        out
    }

    #[test]
    fn every_effect_stays_finite_and_bounded_at_extremes() {
        let loud = sine(110.0, 1.0, 24_000);
        for t in FxType::ALL {
            for extreme in [0.0f32, 1.0] {
                let mut slot = FxSlot::default_for(t);
                slot.mix = 1.0;
                slot.p1 = if extreme > 0.5 { 1e6 } else { -1e6 };
                slot.p2 = slot.p1;
                slot.p3 = slot.p1;
                slot.p4 = slot.p1;
                let out = run(&[slot], &loud);
                assert!(out.iter().all(|s| s.is_finite()), "{t:?} NaN/inf");
                assert!(peak(&out) < 16.0, "{t:?} explose : {}", peak(&out));
            }
        }
    }

    #[test]
    fn bypassed_or_unprepared_slots_leave_the_signal_untouched() {
        let input = sine(440.0, 0.5, 4096);
        let mut slot = FxSlot::default_for(FxType::Distortion);
        slot.enabled = false;
        assert_eq!(run(&[slot.clone()], &input), input);
        // Chaîne modifiée sans `prepare` : rien n'est traité, rien ne casse.
        let mut p = FxChainProcessor::new(SR);
        let mut out = input.clone();
        slot.enabled = true;
        p.process(&mut out, &[slot]);
        assert_eq!(out, input);
    }

    #[test]
    fn two_filters_in_one_chain_have_independent_state() {
        let input = sine(4000.0, 0.5, 9600);
        let lp = FxSlot {
            p1: 500.0,
            ..FxSlot::default_for(FxType::Filter)
        };
        let one = peak(&run(std::slice::from_ref(&lp), &input)[4800..]);
        let two = peak(&run(&[lp.clone(), lp], &input)[4800..]);
        assert!(one < 0.1, "un passe-bas à 500 Hz atténue 4 kHz : {one}");
        assert!(two < one * 0.5, "deux filtres cascadés atténuent plus");
    }

    #[test]
    fn distortion_adds_harmonics_without_jumping_in_level() {
        let input = sine(220.0, 0.25, 9600);
        let mut slot = FxSlot::default_for(FxType::Distortion);
        slot.p1 = 1.0;
        let out = run(&[slot], &input);
        let p = peak(&out[4800..]);
        assert!(p > 0.05 && p < 0.6, "niveau compensé : {p}");
    }

    #[test]
    fn compressor_reduces_loud_peaks_and_keeps_quiet_ones() {
        let mut slot = FxSlot::default_for(FxType::Compressor);
        slot.p1 = -20.0;
        slot.p2 = 10.0;
        let loud = run(&[slot.clone()], &sine(100.0, 1.0, 24_000));
        let quiet = run(&[slot], &sine(100.0, 0.01, 24_000));
        // Makeup : +9 dB à -20 dB / 10:1 ; le fort est réduit bien en dessous.
        assert!(peak(&loud[12_000..]) < 0.7);
        assert!((peak(&quiet[12_000..]) / 0.01 - 2.8).abs() < 0.6);
    }

    #[test]
    fn reverb_tail_grows_with_size_level_stays_even_and_sides_differ() {
        let mut seed = 1u32;
        let noise: Vec<f32> = (0..SR as usize * 3)
            .flat_map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let v = (seed as f32 / u32::MAX as f32) * 0.5 - 0.25;
                [v, v]
            })
            .collect();
        let rms = |b: &[f32]| (b.iter().map(|v| v * v).sum::<f32>() / b.len() as f32).sqrt();
        let mut impulse = vec![0.0f32; SR as usize * 4];
        impulse[0] = 1.0;
        impulse[1] = 1.0;
        let mut last_t60 = 0.0;
        for size in [0.2f32, 0.6, 0.9] {
            let slot = FxSlot {
                mix: 1.0,
                p1: size,
                ..FxSlot::default_for(FxType::Reverb)
            };
            // Bruit : le niveau mouillé reste du même ordre que le sec.
            let out = run(std::slice::from_ref(&slot), &noise);
            let gain = rms(&out[SR as usize..]) / rms(&noise);
            assert!((0.8..3.0).contains(&gain), "SIZE {size} : gain {gain}");
            // Impulsion : temps pour que l'énergie restante passe sous -60 dB.
            let tail = run(&[slot], &impulse);
            let total: f32 = tail.iter().map(|v| v * v).sum();
            let mut rest = total;
            let t60 = tail
                .chunks(2)
                .position(|f| {
                    rest -= f[0] * f[0] + f[1] * f[1];
                    rest < total * 1e-6
                })
                .unwrap_or(0) as f32
                / SR as f32;
            assert!(t60 > last_t60, "SIZE {size} : queue {t60} s");
            last_t60 = t60;
            // Largeur 100 % : gauche et droite décorrélées (espace stéréo).
            let (l, r): (Vec<f32>, Vec<f32>) =
                out[SR as usize..].chunks(2).map(|f| (f[0], f[1])).unzip();
            let corr = l.iter().zip(&r).map(|(a, b)| a * b).sum::<f32>()
                / (rms(&l) * rms(&r) * l.len() as f32);
            assert!(corr.abs() < 0.3, "SIZE {size} : corrélation {corr}");
        }
        assert!((reverb_decay_secs(0.0) - 0.3).abs() < 1e-6);
        assert!((reverb_decay_secs(1.0) - 3.6).abs() < 1e-4);
    }

    #[test]
    fn delay_repeats_after_its_time_and_reverb_leaves_a_tail() {
        let mut impulse = vec![0.0f32; SR as usize * 2];
        impulse[0] = 1.0;
        impulse[1] = 1.0;
        let mut delay = FxSlot::default_for(FxType::Delay);
        delay.p1 = 100.0;
        delay.p4 = 0.0;
        delay.mix = 1.0;
        let out = run(&[delay], &impulse);
        let echo = (4600..5000).map(|i| out[i * 2].abs()).fold(0.0, f32::max);
        assert!(echo > 0.3, "écho attendu vers 100 ms : {echo}");

        let reverb = FxSlot::default_for(FxType::Reverb);
        let out = run(&[reverb], &impulse);
        let tail = peak(&out[SR as usize / 2..SR as usize]);
        assert!(tail > 1e-4, "queue de réverbe");
        assert!(peak(&out) < 2.0);
    }

    #[test]
    fn flat_eq_is_transparent_and_bands_move_their_region() {
        let flat = FxSlot::default_for(FxType::Eq);
        let input = sine(1000.0, 0.5, 9600);
        let out = run(std::slice::from_ref(&flat), &input);
        assert!(
            (peak(&out[4800..]) - 0.5).abs() < 0.01,
            "EQ à plat : transparent"
        );
        let bass = FxSlot {
            p1: 12.0,
            ..flat.clone()
        };
        let bands = eq_bands(&bass, SR as f32);
        let gain = |f: f32| {
            bands
                .iter()
                .map(|b| b.magnitude(f, SR as f32))
                .product::<f32>()
        };
        assert!(gain(40.0) > 3.0, "plateau grave +12 dB");
        assert!((gain(5000.0) - 1.0).abs() < 0.1, "aigus intacts");
    }

    #[test]
    fn lofi_quantizes_and_chorus_modulates() {
        let input = sine(200.0, 0.9, 4800);
        let error = |bits: f32| {
            let lofi = FxSlot {
                p1: bits,
                p2: SR as f32,
                p3: 20_000.0,
                ..FxSlot::default_for(FxType::LoFi)
            };
            let out = run(&[lofi], &input);
            out[2000..]
                .iter()
                .zip(&input[2000..])
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f32::max)
        };
        assert!(error(3.0) > 0.1, "3 bits : marches audibles");
        assert!(
            error(16.0) < 0.05,
            "16 bits pleine fréquence : quasi transparent"
        );
        let chorus = FxSlot::default_for(FxType::Chorus);
        let input = sine(440.0, 0.5, 48_000);
        let out = run(&[chorus], &input);
        let diff = out
            .iter()
            .zip(&input)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(diff > 0.05, "le chorus modifie le signal");
        assert!(peak(&out) < 1.0);
    }

    #[test]
    fn prepare_keeps_the_state_of_unchanged_slots() {
        let chain = [FxSlot::default_for(FxType::Reverb)];
        let mut p = FxChainProcessor::new(SR);
        p.prepare(&chain);
        let mut buf = sine(300.0, 0.5, 2048);
        p.process(&mut buf, &chain);
        let longer = [chain[0].clone(), FxSlot::default_for(FxType::Filter)];
        p.prepare(&longer);
        assert!(p.is_prepared_for(&longer));
        // La réverbe continue de sonner sur un bloc silencieux.
        let mut silence = vec![0.0f32; 2048];
        p.process(&mut silence, &longer);
        assert!(peak(&silence) > 1e-4);
    }

    /// Rend `input` par blocs de 256 trames avec un doigt posé sur
    /// `[from, to)` (trames) à la position `x`.
    fn strip(fx: StripFx, x: f32, input: &[f32], (from, to): (usize, usize)) -> Vec<f32> {
        let mut p = PerformFx::new(SR);
        let mut out = input.to_vec();
        for (k, block) in out.chunks_mut(512).enumerate() {
            let at = k * 256;
            let touch = (at >= from && at < to).then_some(x);
            p.process(block, Some(fx), touch, 120.0);
        }
        out
    }

    #[test]
    fn strip_filter_sweeps_while_touched_and_lets_go_cleanly() {
        let input = sine(4000.0, 0.5, 48_000);
        // Gauche : passe-bas presque fermé, 4 kHz disparaît.
        let out = strip(StripFx::Filter, 0.05, &input, (0, 24_000));
        assert!(peak(&out[2 * 12_000..2 * 24_000]) < 0.05, "passe-bas fermé");
        // Relâché : retour au son sec en quelques ms.
        assert!((peak(&out[2 * 30_000..]) - 0.5).abs() < 0.01, "sec après");
        // Centre : transparent.
        let out = strip(StripFx::Filter, 0.5, &input, (0, 48_000));
        assert!(
            (peak(&out[2 * 12_000..]) - 0.5).abs() < 0.02,
            "centre neutre"
        );
        // Droite : passe-haut, une basse disparaît.
        let bass = sine(60.0, 0.5, 48_000);
        let out = strip(StripFx::Filter, 0.98, &bass, (0, 48_000));
        assert!(peak(&out[2 * 12_000..]) < 0.05, "passe-haut");
    }

    #[test]
    fn strip_stutter_repeats_the_slice_played_at_the_touch() {
        // Signal qui ne se répète jamais : une rampe lente.
        let input: Vec<f32> = (0..48_000)
            .flat_map(|i| {
                let v = i as f32 / 48_000.0;
                [v, v]
            })
            .collect();
        // Droite = 1/32 à 120 BPM : 3000 trames.
        let out = strip(StripFx::Stutter, 0.95, &input, (0, 48_000));
        for t in [3500usize, 7000, 20_000] {
            let a = out[t * 2];
            let b = out[(t % 3000) * 2];
            assert!(
                (a - b).abs() < 1e-4,
                "trame {t} = trame {} : {a} / {b}",
                t % 3000
            );
        }
    }

    #[test]
    fn strip_echo_repeats_while_touched_and_keeps_its_tail() {
        let mut input = vec![0.0f32; 2 * 48_000];
        input[2 * 1000] = 1.0;
        input[2 * 1000 + 1] = 1.0;
        // Droite = 1/16 à 120 BPM : 6000 trames ; doigt levé à 2000.
        let out = strip(StripFx::Echo, 0.95, &input, (0, 2000));
        // Ping-pong : les échos alternent gauche / droite.
        let around = |at: usize| {
            (at - 400..at + 400)
                .map(|i| out[i * 2].abs().max(out[i * 2 + 1].abs()))
                .fold(0.0, f32::max)
        };
        assert!(around(7000) > 0.1, "premier écho : {}", around(7000));
        assert!(around(13_000) > 0.03, "la queue continue après le relâché");
        // Non touché : aucun écho.
        let dry = strip(StripFx::Echo, 0.95, &input, (0, 0));
        assert!(peak(&dry[2 * 2000..]) < 1e-6);
    }

    #[test]
    fn strip_tremolo_and_flanger_stay_bounded_and_finite() {
        let loud = sine(110.0, 1.0, 24_000);
        for fx in StripFx::MOD.into_iter().chain(StripFx::PERFORM) {
            for x in [0.0, 0.5, 1.0] {
                let out = strip(fx, x, &loud, (0, 24_000));
                assert!(out.iter().all(|v| v.is_finite()), "{fx:?} NaN");
                assert!(peak(&out) < 4.0, "{fx:?} {x} : {}", peak(&out));
            }
        }
        // Trémolo 1/4 à 120 BPM : creux toutes les 24000 trames.
        let steady = sine(1000.0, 0.5, 48_000);
        let out = strip(StripFx::Tremolo, 0.1, &steady, (0, 48_000));
        let level = |at: usize| (at..at + 200).map(|i| out[i * 2].abs()).fold(0.0, f32::max);
        assert!(
            level(12_000 - 100) < 0.1,
            "creux à mi-période : {}",
            level(11_900)
        );
        assert!(level(23_900) > 0.4, "plein en fin de période");
    }
}
