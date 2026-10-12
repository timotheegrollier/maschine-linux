//! Synthétiseur de batterie DSP haute fidélité (Kit 808/909 analogique modélisé).
//! Génère les 16 instruments percussifs complets : Kick, Snare, Hi-Hats, Clap, Toms, etc.

use std::f32::consts::PI;

/// Générateur pseudo-aléatoire déterministe (LCG) pour le bruit blanc/rose.
struct NoiseGen {
    state: u32,
}

impl NoiseGen {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next_f32(&mut self) -> f32 {
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.state as f32 / u32::MAX as f32) * 2.0) - 1.0
    }
}

/// Filtre passe-bas à un pôle
struct OnePoleLP {
    alpha: f32,
    prev: f32,
}

impl OnePoleLP {
    fn new(cutoff_hz: f32, sample_rate: f32) -> Self {
        let dt = 1.0 / sample_rate;
        let rc = 1.0 / (2.0 * PI * cutoff_hz);
        let alpha = dt / (rc + dt);
        Self { alpha: alpha.clamp(0.0, 1.0), prev: 0.0 }
    }

    fn process(&mut self, input: f32) -> f32 {
        self.prev += self.alpha * (input - self.prev);
        self.prev
    }
}

/// Filtre passe-haut à un pôle
struct OnePoleHP {
    alpha: f32,
    prev_in: f32,
    prev_out: f32,
}

impl OnePoleHP {
    fn new(cutoff_hz: f32, sample_rate: f32) -> Self {
        let dt = 1.0 / sample_rate;
        let rc = 1.0 / (2.0 * PI * cutoff_hz);
        let alpha = rc / (rc + dt);
        Self { alpha: alpha.clamp(0.0, 1.0), prev_in: 0.0, prev_out: 0.0 }
    }

    fn process(&mut self, input: f32) -> f32 {
        self.prev_out = self.alpha * (self.prev_out + input - self.prev_in);
        self.prev_in = input;
        self.prev_out
    }
}

/// Saturation analogique douce (soft clipper)
fn soft_clip(x: f32) -> f32 {
    if x > 1.0 {
        1.0
    } else if x < -1.0 {
        -1.0
    } else {
        1.5 * x - 0.5 * x * x * x
    }
}

/// 1. Kick 808 lourd et percutant
/// Pitch drop rapide (220 Hz -> 50 Hz), transitoire click + sous-basse résonante
pub fn generate_kick(sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (0.50 * sr) as usize; // 500 ms
    let mut out = Vec::with_capacity(num_samples);
    let mut phase = 0.0f32;
    let mut noise = NoiseGen::new(12345);

    for i in 0..num_samples {
        let t = i as f32 / sr;

        // Enveloppe de pitch : descente exponentielle rapide de 240Hz à 52Hz
        let f_env = 52.0 + 190.0 * (-t / 0.038).exp();
        phase += 2.0 * PI * f_env / sr;
        if phase > 2.0 * PI {
            phase -= 2.0 * PI;
        }

        // Corps sinusoïdal
        let body = phase.sin();

        // Enveloppe d'amplitude (attaque 0ms, sustain et decay sous-basse)
        let amp_env = (-t / 0.16).exp();

        // Transitoire de click dans les 4 premières millisecondes
        let click = if t < 0.005 {
            let ct = t / 0.005;
            let click_env = 1.0 - ct;
            (noise.next_f32() * 0.4 + (2.0 * PI * 1800.0 * t).sin() * 0.6) * click_env
        } else {
            0.0
        };

        let raw = (body * 0.95 + click * 0.45) * amp_env;
        out.push(soft_clip(raw * 1.3) * 0.88);
    }
    out
}

/// 2. Snare 808 tranchante
/// Double tonalité harmonique (~185Hz et ~330Hz) + burst de bruit blanc filtré passe-haut
pub fn generate_snare(sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (0.32 * sr) as usize; // 320 ms
    let mut out = Vec::with_capacity(num_samples);
    let mut phase1 = 0.0f32;
    let mut phase2 = 0.0f32;
    let mut noise = NoiseGen::new(54321);
    let mut hp = OnePoleHP::new(1400.0, sr);
    let mut lp = OnePoleLP::new(7500.0, sr);

    for i in 0..num_samples {
        let t = i as f32 / sr;

        // Tonalité de fût avec léger pitch drop
        let f1 = 185.0 + 80.0 * (-t / 0.015).exp();
        let f2 = 330.0 + 120.0 * (-t / 0.015).exp();
        phase1 = (phase1 + 2.0 * PI * f1 / sr) % (2.0 * PI);
        phase2 = (phase2 + 2.0 * PI * f2 / sr) % (2.0 * PI);

        let tone = (phase1.sin() * 0.65 + phase2.sin() * 0.35) * (-t / 0.075).exp();

        // Timbre de timbre métallique (noise filtré passe-haut)
        let n = noise.next_f32();
        let filtered_noise = lp.process(hp.process(n));
        let noise_env = (-t / 0.16).exp();
        let snap = filtered_noise * noise_env;

        // Attaque percutante dans le premier ms
        let pop = if t < 0.003 { (1.0 - t / 0.003) * 0.6 } else { 0.0 };

        let mixed = tone * 0.45 + snap * 0.65 + pop * 0.3;
        out.push(soft_clip(mixed * 1.25) * 0.85);
    }
    out
}

/// 3. Closed Hi-Hat 808 métallique et net
/// Cluster inharmonique 808 + passe-haut à 7 kHz, decay ultra rapide (45 ms)
pub fn generate_closed_hh(sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (0.09 * sr) as usize; // 90 ms
    let mut out = Vec::with_capacity(num_samples);
    let mut phases = [0.0f32; 6];
    let freqs = [245.0, 306.0, 384.0, 523.0, 845.0, 1230.0];
    let mut hp = OnePoleHP::new(6800.0, sr);
    let mut noise = NoiseGen::new(9999);

    for i in 0..num_samples {
        let t = i as f32 / sr;

        // 6 oscillateurs carrés inharmoniques typiques 808
        let mut cluster = 0.0f32;
        for (p, &f) in phases.iter_mut().zip(freqs.iter()) {
            *p = (*p + 2.0 * PI * f / sr) % (2.0 * PI);
            cluster += if p.sin() > 0.0 { 1.0 } else { -1.0 };
        }
        cluster /= 6.0;

        let n = noise.next_f32();
        let metal = cluster * 0.55 + n * 0.45;
        let filtered = hp.process(metal);

        let env = (-t / 0.038).exp();
        out.push(soft_clip(filtered * 0.9) * env * 0.85);
    }
    out
}

/// 4. Open Hi-Hat 808 brillant
/// Même cluster métallique avec sustain et decay prolongé (350 ms)
pub fn generate_open_hh(sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (0.40 * sr) as usize; // 400 ms
    let mut out = Vec::with_capacity(num_samples);
    let mut phases = [0.0f32; 6];
    let freqs = [245.0, 306.0, 384.0, 523.0, 845.0, 1230.0];
    let mut hp = OnePoleHP::new(6200.0, sr);
    let mut noise = NoiseGen::new(8888);

    for i in 0..num_samples {
        let t = i as f32 / sr;

        let mut cluster = 0.0f32;
        for (p, &f) in phases.iter_mut().zip(freqs.iter()) {
            *p = (*p + 2.0 * PI * f / sr) % (2.0 * PI);
            cluster += if p.sin() > 0.0 { 1.0 } else { -1.0 };
        }
        cluster /= 6.0;

        let n = noise.next_f32();
        let metal = cluster * 0.6 + n * 0.4;
        let filtered = hp.process(metal);

        let env = (-t / 0.18).exp();
        out.push(soft_clip(filtered * 0.9) * env * 0.80);
    }
    out
}

/// 5. Hand Clap 808 authentique
/// 3 micro-frappes espacées de 11ms + réverbération/queue diffuse
pub fn generate_clap(sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (0.35 * sr) as usize;
    let mut out = Vec::with_capacity(num_samples);
    let mut noise = NoiseGen::new(7777);
    let mut bp_lp = OnePoleLP::new(2600.0, sr);
    let mut bp_hp = OnePoleHP::new(1000.0, sr);

    for i in 0..num_samples {
        let t = i as f32 / sr;
        let n = noise.next_f32();
        let filtered = bp_hp.process(bp_lp.process(n));

        // Claps initiaux à t=0ms, t=11ms, t=22ms, puis queue diffuse à t=33ms
        let env = if t < 0.011 {
            (-(t) / 0.004).exp()
        } else if t < 0.022 {
            (-(t - 0.011) / 0.004).exp()
        } else if t < 0.033 {
            (-(t - 0.022) / 0.004).exp()
        } else {
            (-(t - 0.033) / 0.11).exp()
        };

        out.push(soft_clip(filtered * env * 1.6) * 0.85);
    }
    out
}

/// 6. Tom Low 808 résonant
pub fn generate_tom(sample_rate: u32, base_freq: f32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (0.40 * sr) as usize;
    let mut out = Vec::with_capacity(num_samples);
    let mut phase = 0.0f32;
    let mut noise = NoiseGen::new(4321);

    for i in 0..num_samples {
        let t = i as f32 / sr;
        let f = base_freq + (base_freq * 1.2) * (-t / 0.035).exp();
        phase = (phase + 2.0 * PI * f / sr) % (2.0 * PI);

        let tone = phase.sin();
        let amp_env = (-t / 0.14).exp();
        let stick = if t < 0.005 { noise.next_f32() * (1.0 - t / 0.005) * 0.3 } else { 0.0 };

        let raw = (tone * 0.9 + stick) * amp_env;
        out.push(soft_clip(raw * 1.2) * 0.85);
    }
    out
}

/// 7. Rimshot 808
pub fn generate_rimshot(sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (0.09 * sr) as usize;
    let mut out = Vec::with_capacity(num_samples);
    let mut phase1 = 0.0f32;
    let mut phase2 = 0.0f32;
    let mut noise = NoiseGen::new(6543);

    for i in 0..num_samples {
        let t = i as f32 / sr;
        phase1 = (phase1 + 2.0 * PI * 1680.0 / sr) % (2.0 * PI);
        phase2 = (phase2 + 2.0 * PI * 2240.0 / sr) % (2.0 * PI);

        let tone = (phase1.sin() * 0.6 + phase2.sin() * 0.4) * (-t / 0.025).exp();
        let click = if t < 0.004 { noise.next_f32() * 0.8 * (1.0 - t / 0.004) } else { 0.0 };

        out.push((tone * 0.7 + click * 0.5) * 0.85);
    }
    out
}

/// 8. Shaker 808
pub fn generate_shaker(sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (0.16 * sr) as usize;
    let mut out = Vec::with_capacity(num_samples);
    let mut noise = NoiseGen::new(3333);
    let mut hp = OnePoleHP::new(5500.0, sr);

    for i in 0..num_samples {
        let t = i as f32 / sr;
        let n = noise.next_f32();
        let filtered = hp.process(n);

        // Attaque douce (12ms) puis decay
        let env = if t < 0.015 {
            t / 0.015
        } else {
            (-(t - 0.015) / 0.055).exp()
        };

        out.push(filtered * env * 0.75);
    }
    out
}

/// 9. Cowbell 808 mythique
/// Double onde carrée 540 Hz + 800 Hz passée dans un filtre résonant
pub fn generate_cowbell(sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (0.35 * sr) as usize;
    let mut out = Vec::with_capacity(num_samples);
    let mut p1 = 0.0f32;
    let mut p2 = 0.0f32;
    let mut bp_lp = OnePoleLP::new(1200.0, sr);
    let mut bp_hp = OnePoleHP::new(650.0, sr);

    for i in 0..num_samples {
        let t = i as f32 / sr;
        p1 = (p1 + 2.0 * PI * 540.0 / sr) % (2.0 * PI);
        p2 = (p2 + 2.0 * PI * 800.0 / sr) % (2.0 * PI);

        let sq1 = if p1.sin() > 0.0 { 1.0 } else { -1.0 };
        let sq2 = if p2.sin() > 0.0 { 1.0 } else { -1.0 };
        let mix = (sq1 + sq2) * 0.5;

        let filtered = bp_hp.process(bp_lp.process(mix));
        let env = (-t / 0.11).exp();

        out.push(soft_clip(filtered * env * 1.5) * 0.85);
    }
    out
}

/// 10. Crash Cymbal 808
pub fn generate_crash(sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (0.90 * sr) as usize;
    let mut out = Vec::with_capacity(num_samples);
    let mut noise = NoiseGen::new(2222);
    let mut hp = OnePoleHP::new(4500.0, sr);
    let mut phases = [0.0f32; 4];
    let freqs = [315.0, 480.0, 780.0, 1120.0];

    for i in 0..num_samples {
        let t = i as f32 / sr;
        let mut cluster = 0.0f32;
        for (p, &f) in phases.iter_mut().zip(freqs.iter()) {
            *p = (*p + 2.0 * PI * f / sr) % (2.0 * PI);
            cluster += if p.sin() > 0.0 { 1.0 } else { -1.0 };
        }
        let metal = cluster * 0.35 + noise.next_f32() * 0.65;
        let filtered = hp.process(metal);
        let env = (-t / 0.38).exp();

        out.push(soft_clip(filtered * 0.85) * env * 0.80);
    }
    out
}

/// 11. Perc / Woodblock résonant
pub fn generate_perc(sample_rate: u32, freq: f32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (0.12 * sr) as usize;
    let mut out = Vec::with_capacity(num_samples);
    let mut phase = 0.0f32;

    for i in 0..num_samples {
        let t = i as f32 / sr;
        let f = freq + (freq * 0.8) * (-t / 0.008).exp();
        phase = (phase + 2.0 * PI * f / sr) % (2.0 * PI);
        let env = (-t / 0.035).exp();
        out.push(phase.sin() * env * 0.85);
    }
    out
}

/// 12. Synth Stab (Accords FM vintage)
pub fn generate_synth_stab(sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (0.45 * sr) as usize;
    let mut out = Vec::with_capacity(num_samples);
    let mut p1 = 0.0f32;
    let mut p2 = 0.0f32;
    let mut p3 = 0.0f32;

    // Accord mineur F# / A / C#
    let f1 = 185.0;
    let f2 = 220.0;
    let f3 = 277.18;

    for i in 0..num_samples {
        let t = i as f32 / sr;
        p1 = (p1 + 2.0 * PI * f1 / sr) % (2.0 * PI);
        p2 = (p2 + 2.0 * PI * f2 / sr) % (2.0 * PI);
        p3 = (p3 + 2.0 * PI * f3 / sr) % (2.0 * PI);

        let s = p1.sin() * 0.4 + p2.sin() * 0.35 + p3.sin() * 0.35;
        let env = (-t / 0.16).exp();
        out.push(soft_clip(s * 1.2) * env * 0.80);
    }
    out
}

/// 13. Sub Bass 808 profond
pub fn generate_sub_bass(sample_rate: u32) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (0.65 * sr) as usize;
    let mut out = Vec::with_capacity(num_samples);
    let mut phase = 0.0f32;

    for i in 0..num_samples {
        let t = i as f32 / sr;
        let f = 46.0 + 40.0 * (-t / 0.025).exp(); // pitch drop doux
        phase = (phase + 2.0 * PI * f / sr) % (2.0 * PI);

        // Sinus avec 2ème harmonique pour chaleur audible
        let s = phase.sin() + (2.0 * phase).sin() * 0.18;
        let env = (-t / 0.28).exp();
        out.push(soft_clip(s * 1.3) * env * 0.85);
    }
    out
}

/// Génère le kit complet de 16 instruments 808
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
            assert!(max_amp > 0.1, "Sound {name} has almost no energy ({max_amp})");
            assert_eq!(pad as usize, pad as usize);
        }
    }
}
