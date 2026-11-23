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
    // Éditeur de sample : les quatre pages (boutons 1–4).
    for (button, name) in [(1u8, "record"), (3, "slice"), (4, "zone"), (2, "edit")] {
        eng.handle_event(maschine_engine::EngineEvent::SoftKey(button))
            .unwrap();
        dump(&eng, &format!("sampling_{name}"))?;
    }
    eng.handle_event(maschine_engine::EngineEvent::SetLeftTab(3))
        .unwrap();
    dump(&eng, "browser")?;

    // Onglets droits
    eng.handle_event(maschine_engine::EngineEvent::SetLeftTab(0))
        .unwrap();
    eng.handle_event(maschine_engine::EngineEvent::SetRightTab(1))
        .unwrap();
    dump(&eng, "events")?;
    // Knob 6 : toutes les notes du Sound sélectionnées.
    eng.handle_event(maschine_engine::EngineEvent::EventKnob { knob: 5, delta: 32 })
        .unwrap();
    dump(&eng, "events_all_notes")?;
    eng.handle_event(maschine_engine::EngineEvent::EventKnob {
        knob: 5,
        delta: -32,
    })
    .unwrap();
    eng.handle_event(maschine_engine::EngineEvent::SetRightTab(2))
        .unwrap();
    dump(&eng, "macro")?;
    eng.handle_event(maschine_engine::EngineEvent::SetRightTab(3))
        .unwrap();
    dump(&eng, "settings")?;
    eng.handle_event(maschine_engine::EngineEvent::SetRightTab(0))
        .unwrap();

    // AMPLI GUITARE (page PLUGIN 2) — valeurs réalistes pour vérifier les
    // libellés au-dessus des 8 Knobs et les soft keys des 8 boutons.
    eng.handle_event(maschine_engine::EngineEvent::SetLeftTab(4))
        .unwrap();
    eng.handle_event(maschine_engine::EngineEvent::SetPluginPage(2))
        .unwrap();
    {
        let snap = eng.snapshot();
        let mut g = display_render::GuitarScreenData {
            model: 0,
            cab: 0,
            enabled: true,
            monitor: true,
            bypass: false,
            input_db: -14.5,
            output_db: -3.2,
            cpu_load: 0.28,
            latency_ms: 6.4,
            xruns: 0,
            tuner_label: Some("E2 +2c".to_string()),
            tuner_freq: 82.6,
            ..display_render::GuitarScreenData::default()
        };
        for (i, v) in [0.55, 0.6, 0.45, 0.65, 0.5, 0.4, 0.8, 0.75]
            .into_iter()
            .enumerate()
        {
            g.params[i] = v;
        }
        for (i, v) in g.spectrum.iter_mut().enumerate() {
            *v = -90.0 + (i as f32 * 0.6).sin() * 12.0 - i as f32 * 0.4;
        }
        let (left, right) = display_render::render_screens_with_guitar(&snap, Some(&g));
        write_ppm(&dir.join("guitar_left.ppm"), &left)?;
        write_ppm(&dir.join("guitar_right.ppm"), &right)?;
    }

    // SONG et SCENES
    eng.handle_event(maschine_engine::EngineEvent::ToggleSongMode)
        .unwrap();
    dump(&eng, "song")?;
    eng.handle_event(maschine_engine::EngineEvent::ToggleSongMode)
        .unwrap();
    eng.handle_event(maschine_engine::EngineEvent::SetSceneMode(true))
        .unwrap();
    dump(&eng, "scenes")?;

    // PLUGIN : toutes les pages de chaque instrument (Page ◄ ►).
    use maschine_engine::EngineEvent as E;
    eng.handle_event(E::SetSceneMode(false)).unwrap();
    eng.handle_event(E::SetLeftTab(4)).unwrap();
    eng.handle_event(E::SetPluginPage(1)).unwrap();
    eng.handle_event(E::GroupFocus(0)).unwrap();
    for (name, pad) in [("sampler", 1u8), ("kick", 0), ("wavetable", 5), ("acid", 6)] {
        eng.handle_event(E::SelectSound(pad)).unwrap();
        match name {
            "kick" => eng.handle_event(E::KickSetPreset(0)).unwrap(),
            "wavetable" => eng.handle_event(E::WavetableSetPreset(0)).unwrap(),
            "acid" => {
                // Le TB-303 du projet démo (Groupe C « Synth »).
                eng.handle_event(E::GroupFocus(2)).unwrap();
                for pad in 0..16 {
                    eng.handle_event(E::SelectSound(pad)).unwrap();
                    if eng.snapshot().focus_sound_acid.is_some() {
                        break;
                    }
                }
            }
            _ => {}
        }
        let pages = eng.plugin_instrument().map_or(1, |i| i.page_count());
        for page in 0..pages {
            eng.handle_event(E::SetSynthParamPage(page as u8)).unwrap();
            let mut snap = eng.snapshot();
            snap.page_flash = 0.0;
            let (left, right) = display_render::render_screens(&snap);
            write_ppm(&dir.join(format!("plugin_{name}_{page}_left.ppm")), &left)?;
            write_ppm(&dir.join(format!("plugin_{name}_{page}_right.ppm")), &right)?;
        }
    }
    // Transition : bandeau juste après un changement de page.
    eng.handle_event(E::PluginPageStep(-1)).unwrap();
    dump(&eng, "plugin_transition")?;

    // Chaîne de plugins (4D) : emplacement libre, puis un effet de chaque
    // type inséré sur le Sound, un sur le groupe, et un effet contourné.
    eng.handle_event(E::GroupFocus(0)).unwrap();
    eng.handle_event(E::SelectSound(0)).unwrap();
    eng.handle_event(E::PluginSlotStep(1)).unwrap();
    let still = |eng: &maschine_engine::Engine| {
        let mut snap = eng.snapshot();
        snap.page_flash = 0.0;
        snap.status_message = None;
        display_render::render_screens(&snap)
    };
    let (left, right) = still(&eng);
    write_ppm(&dir.join("plugin_add_left.ppm"), &left)?;
    write_ppm(&dir.join("plugin_add_right.ppm"), &right)?;
    for (button, name) in [
        (1u8, "filter"),
        (2, "eq"),
        (3, "disto"),
        (4, "comp"),
        (5, "lofi"),
        (6, "chorus"),
        (7, "delay"),
        (8, "reverb"),
    ] {
        // Emplacement libre du Sound → bouton de l'effet → l'effet est sélectionné.
        while !eng.snapshot().plugin_slots[eng.snapshot().plugin_slot].empty {
            eng.handle_event(E::PluginSlotStep(1)).unwrap();
        }
        eng.handle_event(E::SoftKey(button)).unwrap();
        let (left, right) = still(&eng);
        write_ppm(&dir.join(format!("plugin_fx_{name}_left.ppm")), &left)?;
        write_ppm(&dir.join(format!("plugin_fx_{name}_right.ppm")), &right)?;
    }
    eng.handle_event(E::SoftKey(3)).unwrap(); // BYPASS de la réverbe
    let (left, right) = still(&eng);
    write_ppm(&dir.join("plugin_fx_bypass_left.ppm"), &left)?;
    write_ppm(&dir.join("plugin_fx_bypass_right.ppm"), &right)?;

    // KEYBOARD et CHORDS (gamme majeure : accords en gamme).
    eng.handle_event(E::SetLeftTab(0)).unwrap();
    eng.handle_event(E::SetControllerMode(1)).unwrap();
    dump(&eng, "keyboard")?;
    eng.handle_event(E::SetControllerMode(2)).unwrap();
    eng.handle_event(E::KeyboardScaleDelta(1)).unwrap();
    dump(&eng, "chords")?;
    eng.handle_event(E::SetControllerMode(0)).unwrap();

    // Lancements en attente (lecture) : pattern puis scène « SUIVANT ».
    eng.handle_event(E::DuplicatePattern {
        group: 0,
        pattern: 0,
    })
    .unwrap();
    eng.handle_event(E::PatternSelect {
        group: 0,
        pattern: 0,
    })
    .unwrap();
    eng.handle_event(E::AddScene).unwrap();
    eng.handle_event(E::Scene(0)).unwrap();
    eng.handle_event(E::Transport {
        play: true,
        rec: false,
        stop: false,
    })
    .unwrap();
    eng.advance(4800);
    eng.handle_event(E::SetPatternMode(true)).unwrap();
    eng.handle_event(E::PatternSelect {
        group: 0,
        pattern: 1,
    })
    .unwrap();
    dump(&eng, "pattern_pending")?;
    eng.handle_event(E::SetPatternMode(false)).unwrap();
    eng.handle_event(E::SetSceneMode(true)).unwrap();
    eng.handle_event(E::Scene(1)).unwrap();
    dump(&eng, "scenes_pending")?;

    // AUTOMATION : TB-303 sur le pad 1, AUTO + REC, un knob tourné en lecture.
    let mut eng = Engine::new_demo(48000);
    eng.load_plugin_to_sound(0, 0, 1).unwrap();
    eng.handle_event(E::SelectSound(0)).unwrap();
    eng.handle_event(E::SetLeftTab(4)).unwrap();
    dump(&eng, "automation_empty")?;
    eng.handle_event(E::ToggleAutomation).unwrap();
    eng.handle_event(E::Transport {
        play: true,
        rec: true,
        stop: false,
    })
    .unwrap();
    for step in 0..24 {
        eng.advance(6000);
        let delta = if step < 12 { -40 } else { 60 };
        eng.handle_event(E::PluginKnob { knob: 0, delta }).unwrap();
    }
    // Une seconde automation (volume du groupe) et le 9e point sélectionné.
    eng.handle_event(E::GroupVolume {
        group: 0,
        volume_db: -6.0,
    })
    .unwrap();
    for _ in 0..8 {
        eng.handle_event(E::SoftKey(4)).unwrap(); // PT >
    }
    let (left, right) = still(&eng);
    write_ppm(&dir.join("automation_left.ppm"), &left)?;
    write_ppm(&dir.join("automation_right.ppm"), &right)?;

    // Démarrage (étape et progression) et veille (fermeture / plantage).
    let (left, right) = display_render::render_boot_screens("Synthèse du kit 808", 0.35);
    write_ppm(&dir.join("boot_left.ppm"), &left)?;
    write_ppm(&dir.join("boot_right.ppm"), &right)?;
    let (left, right) = display_render::render_shutdown_screens();
    write_ppm(&dir.join("standby_left.ppm"), &left)?;
    write_ppm(&dir.join("standby_right.ppm"), &right)?;

    println!("écrans exportés dans {}", dir.display());
    Ok(())
}
