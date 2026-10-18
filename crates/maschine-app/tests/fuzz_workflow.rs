// Test d'intégration : enchaîne tous les boutons/pads/modes comme un utilisateur
// qui maltraite la MK3, applique au moteur, sync LEDs, rend tous les écrans.
use maschine_engine::{Engine, EngineEvent};
use maschine_hardware::events::{HardwareEvent, PadState};
use maschine_hardware::mapper::{HardwareMapper, MapperAction, MapperConfig, SequencerLedInfo};
use maschine_proto::ButtonId;

fn pad(p: u8, pressed: bool) -> HardwareEvent {
    HardwareEvent::Pad(PadState {
        pad: p,
        pressed,
        velocity: 100,
        pressure: 0x4080,
        ts_ns: 0,
    })
}
fn btn(id: ButtonId, pressed: bool) -> HardwareEvent {
    HardwareEvent::Button {
        id,
        pressed,
        ts_ns: 0,
    }
}

#[test]
fn fuzz_full_workflow_no_panic() {
    let mut eng = Engine::new_demo(48000);
    let mut mapper = HardwareMapper::new(MapperConfig::default());

    // Tous les boutons press + release
    let buttons = vec![
        ButtonId::Play,
        ButtonId::Stop,
        ButtonId::Rec,
        ButtonId::Restart,
        ButtonId::Tap,
        ButtonId::PadMode,
        ButtonId::Keyboard,
        ButtonId::Chords,
        ButtonId::Step,
        ButtonId::Tempo,
        ButtonId::Volume,
        ButtonId::Swing,
        ButtonId::Pitch,
        ButtonId::Mod,
        ButtonId::Perform,
        ButtonId::Notes,
        ButtonId::FixedVel,
        ButtonId::Follow,
        ButtonId::NoteRepeat,
        ButtonId::Variation,
        ButtonId::Auto,
        ButtonId::FileSave,
        ButtonId::Arranger,
        ButtonId::Mixer,
        ButtonId::Sampling,
        ButtonId::Browser,
        ButtonId::Settings,
        ButtonId::Shift,
        ButtonId::Mute,
        ButtonId::Solo,
        ButtonId::Erase,
        ButtonId::Duplicate,
        ButtonId::Scene,
        ButtonId::Pattern,
        ButtonId::Events,
        ButtonId::Select,
        ButtonId::Macro,
        ButtonId::Plugin,
        ButtonId::ChannelMidi,
        ButtonId::Lock,
        ButtonId::Pedal,
        ButtonId::EncoderPush,
        ButtonId::FourWayUp,
        ButtonId::FourWayDown,
        ButtonId::FourWayLeft,
        ButtonId::FourWayRight,
        ButtonId::Left,
        ButtonId::Right,
        ButtonId::DisplayBtn(1),
        ButtonId::DisplayBtn(2),
        ButtonId::DisplayBtn(3),
        ButtonId::DisplayBtn(4),
        ButtonId::DisplayBtn(5),
        ButtonId::DisplayBtn(6),
        ButtonId::DisplayBtn(7),
        ButtonId::DisplayBtn(8),
        ButtonId::Group(0),
        ButtonId::Group(7),
        ButtonId::Group(3),
    ];
    let apply = |eng: &mut Engine, mapper: &mut HardwareMapper, actions: Vec<MapperAction>| {
        for a in actions {
            match a {
                MapperAction::PadTrigger {
                    pad,
                    velocity,
                    pressed,
                } => {
                    let _ = eng.handle_event(EngineEvent::SelectSound(pad));
                    let _ = eng.handle_event(EngineEvent::Pad {
                        pad,
                        velocity,
                        pressed,
                    });
                }
                MapperAction::SceneTrigger(i) => {
                    let _ = eng.handle_event(EngineEvent::Scene(i));
                }
                MapperAction::PatternTrigger { group, pattern } => {
                    let _ = eng.handle_event(EngineEvent::PatternSelect { group, pattern });
                }
                MapperAction::EraseSound { group, sound } => {
                    let _ = eng.handle_event(EngineEvent::EraseSound { group, sound });
                }
                MapperAction::SelectSound(s) => {
                    mapper.active_sound = s;
                    let _ = eng.handle_event(EngineEvent::SelectSound(s));
                }
                MapperAction::GroupFocus(g) => {
                    let _ = eng.handle_event(EngineEvent::GroupFocus(g));
                }
                MapperAction::SetLeftTab(t) => {
                    let _ = eng.handle_event(EngineEvent::SetLeftTab(t));
                }
                MapperAction::SetRightTab(t) => {
                    let _ = eng.handle_event(EngineEvent::SetRightTab(t));
                }
                MapperAction::SetSceneMode(v) => {
                    let _ = eng.handle_event(EngineEvent::SetSceneMode(v));
                }
                MapperAction::SetPatternMode(v) => {
                    let _ = eng.handle_event(EngineEvent::SetPatternMode(v));
                }
                MapperAction::SetSelectMode(v) => {
                    let _ = eng.handle_event(EngineEvent::SetSelectMode(v));
                }
                MapperAction::SetEraseMode(v) => {
                    let _ = eng.handle_event(EngineEvent::SetEraseMode(v));
                }
                MapperAction::SetControllerMode(v) => {
                    let _ = eng.handle_event(EngineEvent::SetControllerMode(v));
                }
                MapperAction::ToggleLoop => {
                    let _ = eng.handle_event(EngineEvent::ToggleLoop);
                }
                MapperAction::ToggleMetronome => {
                    let _ = eng.handle_event(EngineEvent::ToggleMetronome);
                }
                MapperAction::ToggleSongMode => {
                    let _ = eng.handle_event(EngineEvent::ToggleSongMode);
                }
                MapperAction::PlayPauseToggle => {
                    let _ = eng.handle_event(EngineEvent::Transport {
                        play: true,
                        rec: false,
                        stop: false,
                    });
                }
                MapperAction::Stop => {
                    let _ = eng.handle_event(EngineEvent::Transport {
                        play: false,
                        rec: false,
                        stop: true,
                    });
                }
                MapperAction::RecordToggle => {
                    let _ = eng.handle_event(EngineEvent::Transport {
                        play: true,
                        rec: true,
                        stop: false,
                    });
                }
                MapperAction::Restart => {
                    eng.transport.tick = 0;
                }
                _ => {}
            }
        }
    };

    for b in &buttons {
        let acts = mapper.handle_hardware_event(btn(*b, true));
        apply(&mut eng, &mut mapper, acts);
        let acts = mapper.handle_hardware_event(btn(*b, false));
        apply(&mut eng, &mut mapper, acts);
    }
    // Pads dans tous les modes workflow x modes de jeu
    for mode_btn in [
        ButtonId::PadMode,
        ButtonId::Keyboard,
        ButtonId::Chords,
        ButtonId::Step,
    ] {
        let acts = mapper.handle_hardware_event(btn(mode_btn, true));
        apply(&mut eng, &mut mapper, acts);
        for latch in [
            ButtonId::Scene,
            ButtonId::Pattern,
            ButtonId::Select,
            ButtonId::Erase,
        ] {
            let acts = mapper.handle_hardware_event(btn(latch, true));
            apply(&mut eng, &mut mapper, acts);
            for p in 0..16u8 {
                let acts = mapper.handle_hardware_event(pad(p, true));
                apply(&mut eng, &mut mapper, acts);
                let acts = mapper.handle_hardware_event(pad(p, false));
                apply(&mut eng, &mut mapper, acts);
            }
            let acts = mapper.handle_hardware_event(btn(latch, true));
            apply(&mut eng, &mut mapper, acts);
        }
    }
    // Encodeurs : balaye les 8 knobs sur les 4 onglets
    for tab in [1u8, 2, 3, 0] {
        let acts = mapper.handle_hardware_event(btn(ButtonId::DisplayBtn(tab + 1), true));
        apply(&mut eng, &mut mapper, acts);
        for k in 1..=8u8 {
            for v in (0..1024u16).step_by(37) {
                let acts = mapper.handle_hardware_event(HardwareEvent::Encoder(
                    maschine_proto::EncoderEvent {
                        id: k,
                        value: v,
                        touched: false,
                    },
                ));
                apply(&mut eng, &mut mapper, acts);
            }
        }
    }
    // Transport qui tourne + avance + snapshot + LEDs + tous les écrans
    let _ = eng.handle_event(EngineEvent::Transport {
        play: true,
        rec: false,
        stop: false,
    });
    for _ in 0..50 {
        eng.advance(480);
        let snap = eng.snapshot();
        mapper.group_focus = snap.group_focus;
        mapper.active_sound = snap.active_sound;
        mapper.left_tab = snap.left_tab;
        mapper.right_tab = snap.right_tab;
        let info = SequencerLedInfo {
            is_playing: true,
            is_recording: false,
            is_stopped: false,
            current_bar: 0,
            current_step: 3,
            is_quarter_beat: true,
            active_steps: [true; 16],
            triggered_pads: snap.triggered_pads,
            master_vol: 0.8,
            muted_groups: 1,
            soloed_groups: 2,
            left_tab: snap.left_tab,
            right_tab: snap.right_tab,
            scene_mode: mapper.scene_select_mode,
            pattern_mode: mapper.pattern_select_mode,
            select_mode: mapper.select_mode,
            erase_mode: mapper.erase_mode,
            active_sound: snap.active_sound,
            active_scene: snap.active_scene,
            num_scenes: 4,
            group_num_patterns: 3,
            scene_pattern: Some(1),
            loop_enabled: true,
            metronome: true,
            selected_bar: 0,
            pattern_bars: 2,
        };
        mapper.update_leds(&info);
        let ls = mapper.led_state().clone();
        let _ = maschine_proto::hid_output::encode_report_80(&ls);
        let _ = maschine_proto::hid_output::encode_report_81(&ls);
        let _ = maschine_engine::display_render::render_daw_left(&snap);
        let _ = maschine_engine::display_render::render_daw_right(&snap);
        let _ = maschine_engine::display_render::render_arranger_left(&snap);
        let _ = maschine_engine::display_render::render_arranger_right(&snap);
        let _ = maschine_engine::display_render::render_sampler_left(&snap);
        let _ = maschine_engine::display_render::render_sampler_right(&snap);
        let _ = maschine_engine::display_render::render_mixer_left(&snap);
        let _ = maschine_engine::display_render::render_browser_left(&snap);
        let _ = maschine_engine::display_render::render_events_right(&snap);
        let _ = maschine_engine::display_render::render_macro_right(&snap);
        let _ = maschine_engine::display_render::render_settings_right(&snap);
    }
}
