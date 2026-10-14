// Régression : step-sequencer façon Maschine.
// - En mode Step, l'appui d'un pad ne produit AUCUN son (ni preview, ni MIDI).
// - Un appui (même maintenu / répété par le HID) = une seule bascule de pas.
// - La bascule est silencieuse côté engine et tolère humanize/nudge.
use maschine_engine::Engine;
use maschine_hardware::events::{HardwareEvent, PadState};
use maschine_hardware::mapper::{HardwareMapper, MapperAction};
use maschine_proto::ButtonId;

fn press(pad: u8) -> HardwareEvent {
    HardwareEvent::Pad(PadState {
        pad,
        pressed: true,
        velocity: 100,
        pressure: 0x4080,
        ts_ns: 0,
    })
}

fn release(pad: u8) -> HardwareEvent {
    HardwareEvent::Pad(PadState {
        pad,
        pressed: false,
        velocity: 0,
        pressure: 0x4000,
        ts_ns: 0,
    })
}

/// Applique les actions comme `apply_mapper_actions` dans main.rs.
fn apply(eng: &mut Engine, mapper: &mut HardwareMapper, actions: Vec<MapperAction>) {
    for a in actions {
        match a {
            MapperAction::ToggleStep { sound, bar, step } => {
                let tick = (bar as u32 * 4 * 960) + (step as u32 * 240);
                eng.toggle_step(sound, tick);
            }
            MapperAction::PadTrigger {
                pad,
                velocity,
                pressed,
            } => {
                let _ = eng.handle_event(maschine_engine::EngineEvent::Pad {
                    pad,
                    velocity,
                    pressed,
                });
            }
            MapperAction::SelectSound(s) => {
                mapper.active_sound = s;
                let _ = eng.handle_event(maschine_engine::EngineEvent::SelectSound(s));
            }
            _ => {}
        }
    }
}

fn cell_hits(eng: &Engine, pad: u8, tick: u32) -> usize {
    let h = eng.project_handle();
    let proj = h.read();
    proj.groups[0].patterns[0]
        .events
        .iter()
        .filter(|e| e.pad == pad && maschine_engine::project::same_step_cell(e.tick, tick))
        .count()
}

#[test]
fn step_sequencer_press_is_silent_and_toggles_once() {
    let mut eng = Engine::new_demo(48000);
    let mut mapper = HardwareMapper::with_default();
    // Passe en mode Step (bouton physique).
    let acts = mapper.handle_hardware_event(HardwareEvent::Button {
        id: ButtonId::Step,
        pressed: true,
        ts_ns: 0,
    });
    apply(&mut eng, &mut mapper, acts);

    // Son actif 15, pas 0 : case (15, tick 0), vide dans la démo.
    // (En mode Step, le pad pressé désigne le PAS, pas le son.)
    mapper.active_sound = 15;
    let _ = eng.handle_event(maschine_engine::EngineEvent::SelectSound(15));
    assert_eq!(cell_hits(&eng, 15, 0), 0);

    // Appui + répétitions HID d'un doigt maintenu + release.
    for ev in [press(0), press(0), press(0), release(0)] {
        let acts = mapper.handle_hardware_event(ev);
        apply(&mut eng, &mut mapper, acts);
    }

    // Exactement UN pas posé, et silence total.
    assert_eq!(eng.snapshot().num_voices, 0);
    assert_eq!(
        cell_hits(&eng, 15, 0),
        1,
        "un appui maintenu doit poser un seul pas"
    );

    // Second appui franc : le pas disparaît en une fois, toujours sans son.
    for ev in [press(0), release(0)] {
        let acts = mapper.handle_hardware_event(ev);
        apply(&mut eng, &mut mapper, acts);
    }
    assert_eq!(cell_hits(&eng, 15, 0), 0);
    assert_eq!(eng.snapshot().num_voices, 0);
}

#[test]
fn step_sequencer_survives_humanize_without_multi_press() {
    let mut eng = Engine::new_demo(48000);
    let mut mapper = HardwareMapper::with_default();
    let acts = mapper.handle_hardware_event(HardwareEvent::Button {
        id: ButtonId::Step,
        pressed: true,
        ts_ns: 0,
    });
    apply(&mut eng, &mut mapper, acts);

    mapper.active_sound = 15;
    let _ = eng.handle_event(maschine_engine::EngineEvent::SelectSound(15));
    eng.clear_bar(0);
    eng.clear_bar(1);
    for ev in [press(0), release(0)] {
        let acts = mapper.handle_hardware_event(ev);
        apply(&mut eng, &mut mapper, acts);
    }
    assert_eq!(cell_hits(&eng, 15, 0), 1);

    // Humanize décale les ticks : le retrait doit quand même marcher en 1 appui.
    eng.humanize_active_pattern();
    for ev in [press(0), release(0)] {
        let acts = mapper.handle_hardware_event(ev);
        apply(&mut eng, &mut mapper, acts);
    }
    assert_eq!(cell_hits(&eng, 15, 0), 0);
    assert_eq!(eng.snapshot().num_voices, 0);
}
