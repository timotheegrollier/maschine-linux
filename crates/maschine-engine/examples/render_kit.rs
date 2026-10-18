//! Exporte les 16 instruments du kit DSP en WAV (contrôle qualité à l'oreille).
//!
//! Usage : `cargo run -p maschine-engine --example render_kit -- /tmp/opencode/kit [rate]`

use std::io::Write;

use maschine_engine::drum_synth::generate_complete_808_kit;

fn write_wav(path: &str, samples: &[f32], rate: u32) {
    let mut w = std::fs::File::create(path).expect("create wav");
    let data_len = (samples.len() * 2) as u32;
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data_len).to_le_bytes());
    header.extend_from_slice(b"WAVEfmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes()); // PCM
    header.extend_from_slice(&1u16.to_le_bytes()); // mono
    header.extend_from_slice(&rate.to_le_bytes());
    header.extend_from_slice(&(rate * 2).to_le_bytes());
    header.extend_from_slice(&2u16.to_le_bytes());
    header.extend_from_slice(&16u16.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_len.to_le_bytes());
    w.write_all(&header).unwrap();
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        w.write_all(&v.to_le_bytes()).unwrap();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "/tmp/opencode/kit".to_string());
    let rate: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(48000);
    std::fs::create_dir_all(&dir).unwrap();

    for (pad, name, samples) in generate_complete_808_kit(rate) {
        let slug: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        let path = format!("{dir}/{pad:02}_{slug}.wav");
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let dur = samples.len() as f32 / rate as f32;
        println!("{path}  {dur:.3} s  crête {peak:.3}");
        write_wav(&path, &samples, rate);
    }
}
