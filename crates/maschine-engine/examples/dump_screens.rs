//! Exporte les rendus des écrans MK3 en PPM — debug visuel hors matériel.
//!
//! ```bash
//! cargo run -p maschine-engine --example dump_screens -- /tmp/opencode/screens
//! ```

use std::io::Write;

use maschine_engine::{display_render, Engine};
use maschine_proto::display::DisplayFrame;

/// Écrit une frame en PPM binaire (RGB888).
fn write_ppm(path: &std::path::Path, frame: &DisplayFrame) -> std::io::Result<()> {
    let mut out = Vec::with_capacity(frame.pixels.len() * 3 + 32);
    write!(out, "P6\n{} {}\n255\n", frame.w, frame.h)?;
    for px in &frame.pixels {
        let v = px.0;
        let r5 = ((v >> 11) & 0x1F) as u8;
        let g6 = ((v >> 5) & 0x3F) as u8;
        let b5 = (v & 0x1F) as u8;
        out.push((r5 << 3) | (r5 >> 2));
        out.push((g6 << 2) | (g6 >> 4));
        out.push((b5 << 3) | (b5 >> 2));
    }
    std::fs::write(path, out)
}

fn main() -> std::io::Result<()> {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/opencode/screens".to_string());
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir)?;

    let mut eng = Engine::new_demo(48000);
    eng.advance(4800); // ~0.1 s : BBT et VU non nuls

    let dump = |eng: &Engine, name: &str| -> std::io::Result<()> {
        let snap = eng.snapshot();
        let (left, right) = display_render::render_screens(&snap);
        write_ppm(&dir.join(format!("{name}_left.ppm")), &left)?;
        write_ppm(&dir.join(format!("{name}_right.ppm")), &right)?;
        Ok(())
    };

    // ARRANGE / PATTERN (vue par défaut)
    dump(&eng, "arrange")?;

    // Mode STEP (pads = pas, écran gauche = grille du pattern, droit = mesure)
    eng.handle_event(maschine_engine::EngineEvent::SetControllerMode(3))
        .unwrap();
    eng.handle_event(maschine_engine::EngineEvent::SelectSound(3))
        .unwrap();
    eng.toggle_step(3, 240);
    eng.handle_event(maschine_engine::EngineEvent::StepScroll(17))
        .unwrap();
    eng.handle_event(maschine_engine::EngineEvent::StepVelocityDelta(20))
        .unwrap();
    dump(&eng, "step")?;

    // Pattern 4 mesures en STEP : grille complète sur 4 lignes
    eng.handle_event(maschine_engine::EngineEvent::SoftKey(7))
        .unwrap(); // 4 MES
    eng.handle_event(maschine_engine::EngineEvent::StepScroll(3))
        .unwrap();
    dump(&eng, "step4bars")?;

    // MIXER / SAMPLER / BROWSER
    eng.handle_event(maschine_engine::EngineEvent::SetControllerMode(0))
        .unwrap();
    eng.handle_event(maschine_engine::EngineEvent::SetLeftTab(1))
        .unwrap();
    dump(&eng, "mixer")?;
    eng.handle_event(maschine_engine::EngineEvent::SetLeftTab(2))
        .unwrap();
    dump(&eng, "sampler")?;
    eng.handle_event(maschine_engine::EngineEvent::SetLeftTab(3))
        .unwrap();
    dump(&eng, "browser")?;

    // Onglets droits
    eng.handle_event(maschine_engine::EngineEvent::SetLeftTab(0))
        .unwrap();
    eng.handle_event(maschine_engine::EngineEvent::SetRightTab(1))
        .unwrap();
    dump(&eng, "events")?;
    eng.handle_event(maschine_engine::EngineEvent::SetRightTab(2))
        .unwrap();
    dump(&eng, "macro")?;
    eng.handle_event(maschine_engine::EngineEvent::SetRightTab(3))
        .unwrap();
    dump(&eng, "settings")?;
    eng.handle_event(maschine_engine::EngineEvent::SetRightTab(0))
        .unwrap();

    // SONG et SCENES
    eng.handle_event(maschine_engine::EngineEvent::ToggleSongMode)
        .unwrap();
    dump(&eng, "song")?;
    eng.handle_event(maschine_engine::EngineEvent::ToggleSongMode)
        .unwrap();
    eng.handle_event(maschine_engine::EngineEvent::SetSceneMode(true))
        .unwrap();
    dump(&eng, "scenes")?;

    println!("écrans exportés dans {}", dir.display());
    Ok(())
}
