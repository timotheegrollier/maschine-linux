//! Effets d'insert des Sounds, Groups et du Master.
//!
//! - Filter : SVF « TPT » résonant (LP / HP / BP), coupure lissée
//! - Distortion : saturation douce (tanh) ou dure, anti-repliement ADAA,
//!   tonalité et niveau de sortie, gain compensé
//! - Compressor : stéréo lié, genou doux, gain de compensation automatique
//! - Delay : stéréo / ping-pong jusqu'à 2 s, temps lissé (pas de clic),
//!   tonalité dans la réinjection
//! - Reverb : Freeverb stéréo (8 peignes + 4 passe-tout par canal),
//!   pré-délai et largeur
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
        }
    }

    /// Tous les effets, dans l'ordre de la chaîne de signal usuelle.
    pub const ALL: [Self; 5] = [
        Self::Filter,
        Self::Distortion,
        Self::Compressor,
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
    pub p1: f32,
    /// Reverb Damp 0..1 / Delay Feedback 0..0.95 / Tone 0..1 / Reso 0..1 / Ratio
    pub p2: f32,
    /// Reverb Pre-delay ms / Delay Tone Hz / Type (0 soft, 1 hard) / Mode (0 LP, 1 HP, 2 BP) / Attack ms
    pub p3: f32,
    /// Reverb Width 0..1 / Ping-pong (0/1) / Output dB / — / Release ms
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
}

impl SlotDsp {
    fn new(kind: FxType, sr: f32) -> Self {
        match kind {
            FxType::Filter => Self::Filter(Svf::default()),
            FxType::Distortion => Self::Distortion(Drive::default()),
            FxType::Compressor => Self::Compressor(Comp::default()),
            FxType::Delay => Self::Delay(Box::new(StereoDelay::new(sr))),
            FxType::Reverb => Self::Reverb(Box::new(Reverb::new(sr))),
        }
    }

    fn kind(&self) -> FxType {
        match self {
            Self::Filter(_) => FxType::Filter,
            Self::Distortion(_) => FxType::Distortion,
            Self::Compressor(_) => FxType::Compressor,
            Self::Delay(_) => FxType::Delay,
            Self::Reverb(_) => FxType::Reverb,
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

// ── Reverb (Freeverb) ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Comb {
    buf: Vec<f32>,
    idx: usize,
    store: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            idx: 0,
            store: 0.0,
        }
    }

    #[inline]
    fn tick(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let out = self.buf[self.idx];
        self.store = flush(out * (1.0 - damp) + self.store * damp);
        self.buf[self.idx] = input + self.store * feedback;
        self.idx = (self.idx + 1) % self.buf.len();
        out
    }
}

#[derive(Debug, Clone)]
struct Allpass {
    buf: Vec<f32>,
    idx: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            idx: 0,
        }
    }

    #[inline]
    fn tick(&mut self, input: f32) -> f32 {
        let b = self.buf[self.idx];
        self.buf[self.idx] = flush(input + b * 0.5);
        self.idx = (self.idx + 1) % self.buf.len();
        b - input
    }
}

#[derive(Debug, Clone)]
struct Reverb {
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<Allpass>; 2],
    pre: Vec<f32>,
    pre_pos: usize,
    sr: f32,
    mix: Ramp,
}

impl Reverb {
    /// Longueurs de Freeverb (à 44,1 kHz), mises à l'échelle du taux réel.
    const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
    const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
    const SPREAD: usize = 23;

    fn new(sr: f32) -> Self {
        let scale = |n: usize| ((n as f32) * sr / 44_100.0) as usize;
        let side = |spread: usize| {
            (
                Self::COMBS.map(|n| Comb::new(scale(n + spread))).to_vec(),
                Self::ALLPASSES
                    .map(|n| Allpass::new(scale(n + spread)))
                    .to_vec(),
            )
        };
        let (cl, al) = side(0);
        let (cr, ar) = side(Self::SPREAD);
        Self {
            combs: [cl, cr],
            allpasses: [al, ar],
            pre: vec![0.0; (sr * 0.1) as usize + 2],
            pre_pos: 0,
            sr,
            mix: Ramp::default(),
        }
    }

    fn process(&mut self, buf: &mut [f32], slot: &FxSlot) {
        let feedback = 0.70 + 0.28 * slot.p1.clamp(0.0, 1.0);
        let damp = 0.4 * slot.p2.clamp(0.0, 1.0);
        let n = self.pre.len();
        let pre = ((slot.p3.clamp(0.0, 100.0) * 0.001 * self.sr) as usize).min(n - 1);
        let width = slot.p4.clamp(0.0, 1.0);
        let wet1 = width * 0.5 + 0.5;
        let wet2 = (1.0 - width) * 0.5;
        let frames = buf.len() / 2;
        let (mut mix, dmix) = self.mix.begin(slot.mix.clamp(0.0, 1.0), frames);
        for frame in buf.as_chunks_mut::<2>().0 {
            let (dry_l, dry_r) = (frame[0], frame[1]);
            self.pre[self.pre_pos] = (dry_l + dry_r) * 0.015;
            let input = self.pre[(self.pre_pos + n - pre) % n];
            self.pre_pos = (self.pre_pos + 1) % n;
            let mut out = [0.0f32; 2];
            for (c, o) in out.iter_mut().enumerate() {
                let mut acc = 0.0;
                for comb in &mut self.combs[c] {
                    acc += comb.tick(input, feedback, damp);
                }
                for ap in &mut self.allpasses[c] {
                    acc = ap.tick(acc);
                }
                *o = acc * 3.0;
            }
            let (dg, wg) = equal_power(mix);
            frame[0] = dry_l * dg + (out[0] * wet1 + out[1] * wet2) * wg;
            frame[1] = dry_r * dg + (out[1] * wet1 + out[0] * wet2) * wg;
            mix += dmix;
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
}
