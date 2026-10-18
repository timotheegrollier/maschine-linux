//! Synthétiseur de batterie DSP — kit 808/909 analogique modélisé.
//!
//! Les 16 instruments sont synthétisés à taux plein (aucun sample externe) puis :
//! - filtrés par biquads RBJ (pente 12 dB/oct) ou oscillateurs bande-limitée
//!   (somme d'harmoniques) : pas de repliement audible ;
//! - normalisés à une crête de kit homogène (`normalize`) ;
//! - terminés par un fondu court (`declick`) : aucune chute d'amplitude en fin
//!   de sample (le clic de fin était audible sur les queues longues).

use std::f32::consts::PI;

/// Générateur pseudo-aléatoire déterministe (xorshift32) — bruit blanc.
struct NoiseGen {
    state: u32,
}

impl NoiseGen {
    fn new(seed: u32) -> Self {
        Self { state: seed | 1 }
    }

    fn next_f32(&mut self) -> f32 {
        // xorshift32 : période 2^32-1, bien plus « blanc » que l'LCG.
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        // [-1, 1)
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Biquad RBJ (Direct Form II transposé) — LP / HP / BP, Q contrôlé.
#[derive(Debug, Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    s1: f32,
    s2: f32,
}

impl Biquad {
    fn from_coeffs(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            s1: 0.0,
            s2: 0.0,
        }
    }

    fn lowpass(fc: f32, q: f32, sr: f32) -> Self {
        let w0 = 2.0 * PI * (fc / sr).clamp(0.0005, 0.49);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.05));
        Self::from_coeffs(
            (1.0 - cos_w0) * 0.5,
            1.0 - cos_w0,
            (1.0 - cos_w0) * 0.5,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        )
    }

    fn highpass(fc: f32, q: f32, sr: f32) -> Self {
        let w0 = 2.0 * PI * (fc / sr).clamp(0.0005, 0.49);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.05));
        Self::from_coeffs(
            (1.0 + cos_w0) * 0.5,
            -(1.0 + cos_w0),
            (1.0 + cos_w0) * 0.5,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        )
    }

    fn bandpass(fc: f32, q: f32, sr: f32) -> Self {
        let w0 = 2.0 * PI * (fc / sr).clamp(0.0005, 0.49);
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q.max(0.05));
        Self::from_coeffs(alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.s1;
        self.s1 = self.b1 * x - self.a1 * y + self.s2;
        self.s2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// Saturation douce (tanh) : ajoute des harmoniques sans jamais écrêter dur.
fn saturate(x: f32, drive: f32) -> f32 {
    (x * drive).tanh()
}

/// Onde carrée bande-limitée : somme des harmoniques impaires jusqu'à
/// `max_odd` — évite le repliement spectral des carrés numériques.
fn bandlimited_square(phase: f32, max_odd: usize) -> f32 {
    let mut s = 0.0;
    let mut n = 1usize;
    while n <= max_odd.max(1) {
        s += (phase * n as f32).sin() / n as f32;
        n += 2;
    }
    s * 0.85
}

/// Cluster métallique 808 : 6 partiels inharmoniques (ratios d'origine).
struct MetalCluster {
    phases: [f32; 6],
}

/// Ratios inharmoniques du hi-hat/crash 808.
const METAL_RATIOS: [f32; 6] = [1.0, 1.342, 1.752, 2.208, 3.447, 4.996];

impl MetalCluster {
    fn new() -> Self {
        Self { phases: [0.0; 6] }
    }

    /// `square_mix` 0 = sinusoïdes pures, 1 = carrés bande-limitée.
    /// Le nombre d'harmoniques suit la fréquence du partiel : jamais au-delà
    /// de ~45 % du taux d'échantillonnage (zéro repliement).
    fn process(&mut self, base_hz: f32, sr: f32, square_mix: f32) -> f32 {
        let mut mix = 0.0;
        for (p, ratio) in self.phases.iter_mut().zip(METAL_RATIOS.iter()) {
            let f = base_hz * ratio;
            let inc = 2.0 * PI * f / sr;
            *p = (*p + inc) % (2.0 * PI);
            let max_odd = ((0.45 * sr / f) as usize) | 1;
            let sq = bandlimited_square(*p, max_odd.min(31));
            mix += sq * square_mix + p.sin() * (1.0 - square_mix);
        }
        mix / 6.0
    }
}

/// Marge de niveau du kit (≈ -2,5 dB) : plusieurs voix s'additionnent dans le
/// mix (kick + snare + clap) et le limiteur master n'est là que pour les pics —
/// une frappe seule doit rester parfaitement linéaire.
const KIT_HEADROOM: f32 = 0.75;

/// Normalise à `target` de crête (le kit garde des niveaux cohérents).
fn normalize(out: &mut [f32], target: f32) {
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if peak > 1e-6 {
        let g = target / peak;
        for s in out.iter_mut() {
            *s *= g;
        }
    }
}

/// Fondu raised-cosine en fin de sample + micro fade-in : jamais de marche
/// d'amplitude (clic d'attaque ou de fin).
fn declick(out: &mut [f32], sr: f32, fade_ms: f32) {
    let n = out.len();
    if n == 0 {
        return;
    }
    // fade-out final
    let f = ((fade_ms / 1000.0 * sr) as usize).clamp(1, (n / 3).max(1));
    let start = n - f;
    for (i, s) in out[start..].iter_mut().enumerate() {
        let t = i as f32 / f as f32;
        let w = 0.5 + 0.5 * (PI * t).cos(); // 1 → 0
        *s *= w;
    }
    // fade-in ultra court (0,4 ms) : évite la marche à l'échantillon 0
    let fi = ((0.0004 * sr) as usize).clamp(1, (n / 8).max(1));
    for (i, s) in out.iter_mut().take(fi).enumerate() {
        let t = i as f32 / fi as f32;
        *s *= 0.5 - 0.5 * (PI * t).cos(); // 0 → 1
    }
}

/// Contexte de génération (taux + position).
struct Gen {
    sr: f32,
    t: f32,
}

impl Gen {
    fn new(sample_rate: u32) -> Self {
        Self {
            sr: sample_rate.max(8000) as f32,
            t: 0.0,
        }
    }

    fn advance(&mut self) {
        self.t += 1.0 / self.sr;
    }

    fn samples(&self, secs: f32) -> usize {
        (secs.max(0.005) * self.sr) as usize
    }
}

/// 1. Kick 808 lourd et percutant —
///
/// corps sub 52 Hz + glissando rapide, click de transitoire filtré,
/// saturation douce : punch dans le grave et attaque nette.
pub fn generate_kick(sample_rate: u32) -> Vec<f32> {
    let mut g = Gen::new(sample_rate);
    let sr = g.sr;
    let mut out = Vec::with_capacity(g.samples(0.90));
    let mut phase = 0.0f32;
    let mut noise = NoiseGen::new(12345);
    let mut click_bp = Biquad::bandpass(2200.0, 0.9, sr);

    for _ in 0..g.samples(0.90) {
        let t = g.t;
        // Glissando 262 → 52 Hz (le « boing » du 808).
        let f_env = 52.0 + 210.0 * (-t / 0.030).exp();
        phase = (phase + 2.0 * PI * f_env / sr) % (2.0 * PI);

        // Corps : fondamentale + 2e harmonique (chaleur) qui s'éteint plus vite.
        let body = phase.sin() + (2.0 * phase).sin() * 0.16 * (-t / 0.05).exp();

        // Enveloppe d'amplitude : corps long, queue inaudible en fin de sample.
        let amp_env = (-t / 0.150).exp();
        // Micro-retard d'amplitude (2 ms) : garde l'attaque sans « pop » numérique.
        let attack = (t / 0.002).min(1.0);

        // Click de transitoire : bruit passe-bande + burst 1,6 kHz (5 ms).
        let click = if t < 0.006 {
            let env = (1.0 - t / 0.006).powi(2);
            (click_bp.process(noise.next_f32()) * 0.7 + (2.0 * PI * 1600.0 * t).sin() * 0.5) * env
        } else {
            0.0
        };

        let raw = (body * 0.95 * amp_env * attack + click * 0.55) * 1.1;
        out.push(saturate(raw, 1.15));
        g.advance();
    }
    normalize(&mut out, 0.95 * KIT_HEADROOM);
    declick(&mut out, sr, 20.0);
    out
}

/// 2. Snare 808 tranchante —
///
/// double tonalité (~190/290 Hz) + bruit passé bande 1,4–7,5 kHz, snap 2 ms.
pub fn generate_snare(sample_rate: u32) -> Vec<f32> {
    let mut g = Gen::new(sample_rate);
    let sr = g.sr;
    let mut out = Vec::with_capacity(g.samples(0.40));
    let mut p1 = 0.0f32;
    let mut p2 = 0.0f32;
    let mut noise = NoiseGen::new(54321);
    let mut hp = Biquad::highpass(1400.0, 0.7, sr);
    let mut lp = Biquad::lowpass(7500.0, 0.7, sr);

    for _ in 0..g.samples(0.40) {
        let t = g.t;
        // Fût : deux partiels avec léger glissando.
        let f1 = 190.0 + 70.0 * (-t / 0.018).exp();
        let f2 = 290.0 + 110.0 * (-t / 0.018).exp();
        p1 = (p1 + 2.0 * PI * f1 / sr) % (2.0 * PI);
        p2 = (p2 + 2.0 * PI * f2 / sr) % (2.0 * PI);
        let tone = (p1.sin() * 0.62 + p2.sin() * 0.38) * (-t / 0.075).exp();

        // Timbre : bruit bande 1,4–7,5 kHz, queue plus longue que le fût.
        let n = noise.next_f32();
        let snap = lp.process(hp.process(n)) * (-t / 0.130).exp();

        // Attaque 2 ms (transitoire large bande).
        let pop = if t < 0.002 {
            (1.0 - t / 0.002) * noise.next_f32() * 0.8
        } else {
            0.0
        };

        out.push(saturate(tone * 0.5 + snap * 0.85 + pop * 0.35, 1.15));
        g.advance();
    }
    normalize(&mut out, 0.92 * KIT_HEADROOM);
    declick(&mut out, sr, 25.0);
    out
}

/// 3. Closed Hi-Hat 808 métallique et net.
pub fn generate_closed_hh(sample_rate: u32) -> Vec<f32> {
    let mut g = Gen::new(sample_rate);
    let sr = g.sr;
    let mut out = Vec::with_capacity(g.samples(0.13));
    let mut metal = MetalCluster::new();
    let mut noise = NoiseGen::new(9999);
    let mut hp = Biquad::highpass(7200.0, 0.7, sr);
    let mut hp2 = Biquad::highpass(5200.0, 0.7, sr);

    for _ in 0..g.samples(0.13) {
        let t = g.t;
        let cluster = hp.process(metal.process(318.0, sr, 0.75));
        let hiss = hp2.process(noise.next_f32()) * 0.55;
        // Attaque 0,4 ms puis décroissance 34 ms.
        let env = (t / 0.0004).min(1.0) * (-t / 0.034).exp();
        out.push((cluster + hiss) * env * 0.8);
        g.advance();
    }
    normalize(&mut out, 0.86 * KIT_HEADROOM);
    declick(&mut out, sr, 12.0);
    out
}

/// 4. Open Hi-Hat 808 brillant — queue 320 ms.
pub fn generate_open_hh(sample_rate: u32) -> Vec<f32> {
    let mut g = Gen::new(sample_rate);
    let sr = g.sr;
    let mut out = Vec::with_capacity(g.samples(0.50));
    let mut metal = MetalCluster::new();
    let mut noise = NoiseGen::new(8888);
    let mut hp = Biquad::highpass(6500.0, 0.7, sr);
    let mut hp2 = Biquad::highpass(4800.0, 0.7, sr);

    for _ in 0..g.samples(0.50) {
        let t = g.t;
        let cluster = hp.process(metal.process(318.0, sr, 0.7));
        let hiss = hp2.process(noise.next_f32()) * 0.5;
        let env = (t / 0.0004).min(1.0) * (-t / 0.160).exp();
        out.push((cluster + hiss) * env * 0.8);
        g.advance();
    }
    normalize(&mut out, 0.84 * KIT_HEADROOM);
    declick(&mut out, sr, 25.0);
    out
}

/// 5. Hand Clap 808 authentique — 3 frappes (0/10/20 ms) + queue diffuse.
pub fn generate_clap(sample_rate: u32) -> Vec<f32> {
    let mut g = Gen::new(sample_rate);
    let sr = g.sr;
    let mut out = Vec::with_capacity(g.samples(0.42));
    let mut noise = NoiseGen::new(7777);
    let mut bp1 = Biquad::bandpass(1150.0, 0.85, sr);
    let mut bp2 = Biquad::bandpass(2900.0, 0.9, sr);
    let mut lp = Biquad::lowpass(4200.0, 0.7, sr);

    for _ in 0..g.samples(0.42) {
        let t = g.t;
        let n = noise.next_f32();
        let filtered = lp.process(bp1.process(n) * 0.8 + bp2.process(n) * 0.7 + n * 0.12);

        // 3 rafales serrées puis la queue de salle.
        let env = if t < 0.010 {
            (-t / 0.0035).exp()
        } else if t < 0.020 {
            (-(t - 0.010) / 0.0035).exp()
        } else if t < 0.030 {
            (-(t - 0.020) / 0.0035).exp()
        } else {
            (-(t - 0.030) / 0.115).exp() * 0.85
        };
        out.push(filtered * env * 1.25);
        g.advance();
    }
    normalize(&mut out, 0.90 * KIT_HEADROOM);
    declick(&mut out, sr, 25.0);
    out
}

/// 6. Tom 808 résonant — glissando + attaque de baguette.
pub fn generate_tom(sample_rate: u32, base_freq: f32) -> Vec<f32> {
    let mut g = Gen::new(sample_rate);
    let sr = g.sr;
    let mut out = Vec::with_capacity(g.samples(0.55));
    let mut phase = 0.0f32;
    let mut noise = NoiseGen::new(4321);
    let mut stick_bp = Biquad::bandpass(3200.0, 1.1, sr);

    for _ in 0..g.samples(0.55) {
        let t = g.t;
        let f = base_freq + (base_freq * 1.15) * (-t / 0.032).exp();
        phase = (phase + 2.0 * PI * f / sr) % (2.0 * PI);

        let tone = (phase.sin() + (2.0 * phase).sin() * 0.12 * (-t / 0.06).exp())
            * (-t / 0.135).exp()
            * (t / 0.002).min(1.0);
        // Baguette : bruit passe-bande très court.
        let stick = if t < 0.005 {
            stick_bp.process(noise.next_f32()) * (1.0 - t / 0.005).powi(2) * 0.45
        } else {
            0.0
        };

        out.push(saturate(tone * 0.95 + stick, 1.1));
        g.advance();
    }
    normalize(&mut out, 0.88 * KIT_HEADROOM);
    declick(&mut out, sr, 20.0);
    out
}

/// 7. Rimshot 808.
pub fn generate_rimshot(sample_rate: u32) -> Vec<f32> {
    let mut g = Gen::new(sample_rate);
    let sr = g.sr;
    let mut out = Vec::with_capacity(g.samples(0.10));
    let mut p1 = 0.0f32;
    let mut p2 = 0.0f32;
    let mut noise = NoiseGen::new(6543);
    let mut click_bp = Biquad::bandpass(2400.0, 0.9, sr);

    for _ in 0..g.samples(0.10) {
        let t = g.t;
        p1 = (p1 + 2.0 * PI * 1680.0 / sr) % (2.0 * PI);
        p2 = (p2 + 2.0 * PI * 2240.0 / sr) % (2.0 * PI);

        let tone = (p1.sin() * 0.6 + p2.sin() * 0.4) * (-t / 0.022).exp();
        let click = if t < 0.004 {
            click_bp.process(noise.next_f32()) * (1.0 - t / 0.004).powi(2) * 0.9
        } else {
            0.0
        };
        out.push(saturate(tone * 0.75 + click * 0.55, 1.2));
        g.advance();
    }
    normalize(&mut out, 0.88 * KIT_HEADROOM);
    declick(&mut out, sr, 10.0);
    out
}

/// 8. Shaker 808.
pub fn generate_shaker(sample_rate: u32) -> Vec<f32> {
    let mut g = Gen::new(sample_rate);
    let sr = g.sr;
    let mut out = Vec::with_capacity(g.samples(0.22));
    let mut noise = NoiseGen::new(3333);
    let mut hp = Biquad::highpass(6200.0, 0.7, sr);
    let mut body = Biquad::bandpass(2600.0, 1.0, sr);

    for _ in 0..g.samples(0.22) {
        let t = g.t;
        let n = noise.next_f32();
        let filtered = hp.process(n) + body.process(n) * 0.35;
        // Attaque 8 ms (mouvement de main) puis double décroissance.
        let env = (t / 0.008).min(1.0) * ((-t / 0.055).exp() + 0.35 * (-t / 0.16).exp());
        out.push(filtered * env * 0.7);
        g.advance();
    }
    normalize(&mut out, 0.80 * KIT_HEADROOM);
    declick(&mut out, sr, 18.0);
    out
}

/// 9. Cowbell 808 mythique — deux « carrés » 540/800 Hz dans un passe-bande.
pub fn generate_cowbell(sample_rate: u32) -> Vec<f32> {
    let mut g = Gen::new(sample_rate);
    let sr = g.sr;
    let mut out = Vec::with_capacity(g.samples(0.45));
    let mut p1 = 0.0f32;
    let mut p2 = 0.0f32;
    let mut bp = Biquad::bandpass(720.0, 1.1, sr);
    let mut lp = Biquad::lowpass(4200.0, 0.7, sr);

    for _ in 0..g.samples(0.45) {
        let t = g.t;
        p1 = (p1 + 2.0 * PI * 540.0 / sr) % (2.0 * PI);
        p2 = (p2 + 2.0 * PI * 800.0 / sr) % (2.0 * PI);

        let mix = (bandlimited_square(p1, 11) + bandlimited_square(p2, 11)) * 0.5;
        let filtered = lp.process(bp.process(mix));
        // Décroissance : partiel aigu plus court (caractère du 808).
        let env = (-t / 0.30).exp() * 0.7 + (-t / 0.045).exp() * 0.45;
        out.push(saturate(filtered * env * 1.1, 1.05));
        g.advance();
    }
    normalize(&mut out, 0.86 * KIT_HEADROOM);
    declick(&mut out, sr, 22.0);
    out
}

/// 10. Crash Cymbal 808 — cluster métallique + bruit, longue queue.
pub fn generate_crash(sample_rate: u32) -> Vec<f32> {
    let mut g = Gen::new(sample_rate);
    let sr = g.sr;
    let mut out = Vec::with_capacity(g.samples(1.60));
    let mut noise = NoiseGen::new(2222);
    let mut metal = MetalCluster::new();
    let mut hp = Biquad::highpass(4200.0, 0.7, sr);
    let mut hp2 = Biquad::highpass(2800.0, 0.7, sr);

    for _ in 0..g.samples(1.60) {
        let t = g.t;
        let cluster = metal.process(384.0, sr, 0.55);
        let metal_mix = hp.process(cluster) * 0.85 + hp2.process(noise.next_f32()) * 0.65;
        // Attaque 0,5 ms + double queue (corps + traîne).
        let env = (t / 0.0005).min(1.0) * ((-t / 0.42).exp() * 0.8 + (-t / 0.10).exp() * 0.35);
        out.push(metal_mix * env * 0.75);
        g.advance();
    }
    normalize(&mut out, 0.82 * KIT_HEADROOM);
    declick(&mut out, sr, 40.0);
    out
}

/// 11. Perc / Woodblock résonant.
pub fn generate_perc(sample_rate: u32, freq: f32) -> Vec<f32> {
    let mut g = Gen::new(sample_rate);
    let sr = g.sr;
    let mut out = Vec::with_capacity(g.samples(0.16));
    let mut phase = 0.0f32;
    let mut noise = NoiseGen::new(1234);
    let mut stick = Biquad::bandpass(4000.0, 1.2, sr);

    for _ in 0..g.samples(0.16) {
        let t = g.t;
        let f = freq + (freq * 0.7) * (-t / 0.007).exp();
        phase = (phase + 2.0 * PI * f / sr) % (2.0 * PI);
        let tone = (phase.sin() + (2.0 * phase).sin() * 0.25) * (-t / 0.035).exp();
        let click = if t < 0.003 {
            stick.process(noise.next_f32()) * (1.0 - t / 0.003).powi(2) * 0.6
        } else {
            0.0
        };
        out.push(tone * 0.85 + click);
        g.advance();
    }
    normalize(&mut out, 0.86 * KIT_HEADROOM);
    declick(&mut out, sr, 15.0);
    out
}

/// 12. Synth Stab (accord mineur FM vintage).
pub fn generate_synth_stab(sample_rate: u32) -> Vec<f32> {
    let mut g = Gen::new(sample_rate);
    let sr = g.sr;
    let mut out = Vec::with_capacity(g.samples(0.55));
    let mut p: [f32; 6] = [0.0; 6];
    // Accord mineur F# / A / C# avec léger détune (largeur).
    let freqs = [185.0, 186.4, 220.0, 221.6, 277.18, 278.8];
    let mut lp = Biquad::lowpass(5200.0, 0.7, sr);

    for _ in 0..g.samples(0.55) {
        let t = g.t;
        let mut s = 0.0f32;
        for (ph, f) in p.iter_mut().zip(freqs.iter()) {
            *ph = (*ph + 2.0 * PI * f / sr) % (2.0 * PI);
            s += ph.sin() + (2.0 * *ph).sin() * 0.15;
        }
        s /= 6.0;
        let env = (t / 0.004).min(1.0) * (-t / 0.170).exp();
        out.push(lp.process(saturate(s * env * 1.3, 1.1)));
        g.advance();
    }
    normalize(&mut out, 0.86 * KIT_HEADROOM);
    declick(&mut out, sr, 25.0);
    out
}

/// 13. Sub Bass 808 profond.
pub fn generate_sub_bass(sample_rate: u32) -> Vec<f32> {
    let mut g = Gen::new(sample_rate);
    let sr = g.sr;
    let mut out = Vec::with_capacity(g.samples(0.95));
    let mut phase = 0.0f32;
    let mut noise = NoiseGen::new(2468);
    let mut attack_bp = Biquad::bandpass(1800.0, 0.9, sr);
    let mut lp = Biquad::lowpass(900.0, 0.7, sr);

    for _ in 0..g.samples(0.95) {
        let t = g.t;
        let f = 44.0 + 36.0 * (-t / 0.022).exp();
        phase = (phase + 2.0 * PI * f / sr) % (2.0 * PI);

        // Sinus + 2e harmonique (audible sur petits HP) — LP pour la rondeur.
        let s = lp.process(phase.sin() + (2.0 * phase).sin() * 0.18);
        let env = (t / 0.003).min(1.0) * (-t / 0.280).exp();
        // Petit click d'attaque (précision rythmique).
        let click = if t < 0.004 {
            attack_bp.process(noise.next_f32()) * (1.0 - t / 0.004).powi(2) * 0.5
        } else {
            0.0
        };
        out.push(saturate(s * env * 1.15 + click, 1.1));
        g.advance();
    }
    normalize(&mut out, 0.90 * KIT_HEADROOM);
    declick(&mut out, sr, 30.0);
    out
}

/// Génère le kit complet de 16 instruments 808.
pub fn generate_complete_808_kit(sample_rate: u32) -> Vec<(u8, &'static str, Vec<f32>)> {
    vec![
        (0, "Kick 808", generate_kick(sample_rate)),
        (1, "Snare 808", generate_snare(sample_rate)),
        (2, "Closed HH", generate_closed_hh(sample_rate)),
        (3, "Open HH", generate_open_hh(sample_rate)),
        (4, "Tom Low", generate_tom(sample_rate, 85.0)),
        (5, "Tom Mid", generate_tom(sample_rate, 130.0)),
        (6, "Tom High", generate_tom(sample_rate, 185.0)),
        (7, "Clap 808", generate_clap(sample_rate)),
        (8, "Rimshot", generate_rimshot(sample_rate)),
        (9, "Shaker", generate_shaker(sample_rate)),
        (10, "Cowbell", generate_cowbell(sample_rate)),
        (11, "Crash", generate_crash(sample_rate)),
        (12, "Perc 1", generate_perc(sample_rate, 880.0)),
        (13, "Perc 2", generate_perc(sample_rate, 1250.0)),
        (14, "Synth Stab", generate_synth_stab(sample_rate)),
        (15, "Sub Bass 808", generate_sub_bass(sample_rate)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_16_drums_generated() {
        let kit = generate_complete_808_kit(48000);
        assert_eq!(kit.len(), 16);
        for (pad, name, samples) in kit {
            assert!(!samples.is_empty(), "Sound {name} has no samples");
            assert!(samples.len() > 1000, "Sound {name} is too short");
            // Vérifier absence de NaN ou Infini
            for &s in &samples {
                assert!(s.is_finite(), "Sound {name} contains NaN or Inf");
                assert!(s.abs() <= 1.05, "Sound {name} clips above limit: {s}");
            }
            // Vérifier que le son a de l'énergie
            let max_amp = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            assert!(
                max_amp > 0.1,
                "Sound {name} has almost no energy ({max_amp})"
            );
            assert_eq!(pad as usize, pad as usize);
        }
    }

    /// La fin de chaque sample doit être silencieuse : une coupure nette à
    /// amplitude non nulle produisait un clic audible (surtout en fin de boucle).
    #[test]
    fn drum_tails_end_silent() {
        for (pad, name, samples) in generate_complete_808_kit(44100) {
            let tail = &samples[samples.len().saturating_sub(64)..];
            let peak = tail.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(
                peak < 0.02,
                "Sound {name} (pad {pad}) finit à {peak:.4} — clic de fin de sample"
            );
            // Début proche de zéro (fade-in anti-marche).
            assert!(
                samples[0].abs() < 0.05,
                "Sound {name} (pad {pad}) commence à {:.4}",
                samples[0]
            );
        }
    }

    /// Pas de composante continue (le biquad + saturation n'en créent pas).
    #[test]
    fn drum_has_no_dc_offset() {
        for (_pad, name, samples) in generate_complete_808_kit(48000) {
            let mean = samples.iter().sum::<f32>() / samples.len() as f32;
            assert!(mean.abs() < 0.02, "Sound {name} DC offset {mean:.4}");
        }
    }
}
