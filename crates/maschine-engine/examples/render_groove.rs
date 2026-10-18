//! Rendu hors-ligne du groove démo (diagnostic qualité audio).
//!
//! Usage : `cargo run -p maschine-engine --example render_groove -- out.wav [secondes] [rate]`
//!
//! Écrit un WAV 16 bits stéréo et affiche les statistiques qui comptent pour
//! traquer les craquements : crête, RMS, échantillons écrêtés (>= 0,999) et
//! plus grand saut d'un échantillon à l'autre (clic/distorsion).

use std::io::Write;

use maschine_engine::{Engine, EngineEvent};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/tmp/opencode/demo_groove.wav".to_string());
    let secs: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(12.0);
    let rate: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(48000);
    let master: f32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.85);

    let mut eng = Engine::new_demo(rate);
    // Kit complet + pattern démo (comme le daemon).
    eng.load_default_808_kit();
    eng.handle_event(EngineEvent::MasterVolume(master)).unwrap();
    eng.handle_event(EngineEvent::Transport {
        play: true,
        rec: false,
        stop: false,
    })
    .unwrap();

    const BLOCK: usize = 512;
    let blocks = (secs * rate as f64 / BLOCK as f64) as usize;
    let mut mono = Vec::with_capacity(blocks * BLOCK);
    let mut stereo = vec![0.0f32; BLOCK * 2];
    let mut peak = 0.0f32;
    let mut clipped = 0usize;
    let mut max_jump = 0.0f32;
    let mut sum_sq = 0.0f64;
    let mut n = 0usize;
    let mut prev = 0.0f32;

    for _ in 0..blocks {
        eng.advance(BLOCK as u64);
        eng.render_audio(&mut stereo);
        for f in 0..BLOCK {
            let l = stereo[f * 2];
            let r = stereo[f * 2 + 1];
            let s = (l + r) * 0.5;
            peak = peak.max(l.abs()).max(r.abs());
            if l.abs() >= 0.999 || r.abs() >= 0.999 {
                clipped += 1;
            }
            max_jump = max_jump.max((s - prev).abs());
            prev = s;
            sum_sq += (s * s) as f64;
            n += 1;
            mono.push(s);
        }
    }
    let rms = (sum_sq / n as f64).sqrt();

    // WAV 16 bits stéréo (duplication mono pour l'écoute).
    let mut w = std::fs::File::create(&path).expect("create wav");
    let data_len = (mono.len() * 2 * 2) as u32; // 2 canaux * i16
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data_len).to_le_bytes());
    header.extend_from_slice(b"WAVEfmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes()); // PCM
    header.extend_from_slice(&2u16.to_le_bytes()); // stéréo
    header.extend_from_slice(&rate.to_le_bytes());
    header.extend_from_slice(&(rate * 4).to_le_bytes());
    header.extend_from_slice(&4u16.to_le_bytes());
    header.extend_from_slice(&16u16.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_len.to_le_bytes());
    w.write_all(&header).unwrap();
    let mut pcm = Vec::with_capacity(mono.len() * 4);
    for s in &mono {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        pcm.extend_from_slice(&v.to_le_bytes());
        pcm.extend_from_slice(&v.to_le_bytes());
    }
    w.write_all(&pcm).unwrap();

    println!("WAV: {path} ({secs} s @ {rate} Hz)");
    println!("crête      : {peak:.4}");
    println!("RMS        : {rms:.4} ({:.1} dB)", 20.0 * rms.log10());
    println!("écrêtés    : {clipped} échantillons (>= 0,999)");
    println!("max saut   : {max_jump:.4} (delta échantillon à échantillon)");
}
