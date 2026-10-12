//! `maschine-linux` — binaire principal.
//!
//! CLI : `probe`, `hid-dump`, `daemon`, `project`...

use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossbeam_channel::unbounded;
use log::{error, info, warn};
use parking_lot::Mutex;

use maschine_engine::{Engine, EngineEvent, Project};
use maschine_hardware::device::find_maschine_devices;
use maschine_hardware::events::HardwareEvent;
use maschine_hardware::hidraw::{spawn_hidraw_thread, HidrawConfig};
use maschine_hardware::mapper::{HardwareMapper, MapperConfig, MapperAction, SequencerLedInfo};

#[cfg(feature = "audio")]
mod audio;
#[cfg(feature = "usb")]
mod display_thread;
mod hw_output;
mod settings;

#[derive(Parser, Debug)]
#[command(name = "maschine-linux", version, about = "Maschine MK3 sur Fedora — daemon + engine", long_about = None)]
struct Cli {
    /// Verbosité (-v, -vv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Liste les devices Maschine détectés (USB + hidraw)
    Probe,
    /// Dump HID brut pendant N secondes
    HidDump {
        /// Durée en secondes
        #[arg(long, default_value = "5")]
        duration: u64,
        /// Chemin hidraw (auto si absent)
        #[arg(long)]
        hidraw: Option<PathBuf>,
    },
    /// Lance le daemon HID → MIDI + Engine (headless)
    Daemon {
        /// Projet à charger (JSON)
        #[arg(long)]
        project: Option<PathBuf>,
        /// Port MIDI virtuel (ALSA) — placeholder
        #[arg(long, default_value = "Maschine MK3 Virtual")]
        midi_port: String,
        /// Device audio explicite (ex: "Maschine", sinon auto MK3)
        #[arg(long)]
        audio_device: Option<String>,
        /// Pas d'audio (silence, test HID seul)
        #[arg(long)]
        no_audio: bool,
        /// Pas de LEDs (test audio seul)
        #[arg(long)]
        no_led: bool,
        /// Canaux headphone (défaut: dupliquer main sur ch 2,3)
        #[arg(long, default_value = "duplicate")]
        headphone_mode: String,
    },
    /// Gestion projet (new/save/load)
    Project {
        #[command(subcommand)]
        cmd: ProjectCmd,
    },
    /// Envoie un test LED + écran (nécessite --features usb pour bulk)
    LedTest {
        /// Pad 0..15
        #[arg(long, default_value = "0")]
        pad: u8,
        /// Couleur index 0..78
        #[arg(long, default_value = "6")]
        color: u8,
    },
    /// Cycle LEDs (chenillard)
    LedCycle {
        /// Durée totale ms
        #[arg(long, default_value = "3000")]
        duration: u64,
    },
    /// Test écrans bulk (couleur solide)
    DisplayTest {
        /// Couleur R
        #[arg(long, default_value = "255")]
        r: u8,
        /// Couleur G
        #[arg(long, default_value = "0")]
        g: u8,
        /// Couleur B
        #[arg(long, default_value = "0")]
        b: u8,
        /// Durée ms
        #[arg(long, default_value = "2000")]
        duration: u64,
    },
    /// Diagnostic approfondi des écrans USB Bulk (IF5 EP 0x04)
    #[command(name = "display-probe")]
    DisplayProbe,
    /// Liste les devices audio (cpal)
    AudioList,
    /// Joue un test tone sur la MK3
    AudioTest {
        /// Fréquence Hz
        #[arg(long, default_value = "440")]
        freq: f32,
        /// Durée secs
        #[arg(long, default_value = "2")]
        duration: f32,
        /// Device explicite
        #[arg(long)]
        device: Option<String>,
    },
    /// Lance la GUI (nécessite --features gui)
    Gui {
        /// Projet à charger
        #[arg(long)]
        project: Option<PathBuf>,
        /// Device audio explicite (ex: "default", "pc", "mk3", ou nom cpal)
        #[arg(long)]
        audio_device: Option<String>,
        /// Pas d'audio
        #[arg(long)]
        no_audio: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ProjectCmd {
    /// Crée un projet démo
    New {
        /// Chemin sortie
        #[arg(long, default_value = "demo.machine-linux.json")]
        out: PathBuf,
    },
    /// Affiche un projet JSON
    Show {
        /// Chemin
        path: PathBuf,
    },
    /// Valide un projet
    Validate {
        /// Chemin
        path: PathBuf,
    },
}

fn init_logger(verbose: u8) {
    let level = match verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level))
        .format_timestamp_millis()
        .try_init();
}

fn main() -> anyhow::Result<()> {
    init_ctrlc();
    let cli = Cli::parse();
    init_logger(cli.verbose);

    match cli.command.unwrap_or(Commands::Probe) {
        Commands::Probe => cmd_probe()?,
        Commands::HidDump { duration, hidraw } => cmd_hid_dump(duration, hidraw)?,
        Commands::Daemon {
            project,
            midi_port,
            audio_device,
            no_audio,
            no_led,
            headphone_mode,
        } => cmd_daemon(project, midi_port, audio_device, no_audio, no_led, headphone_mode, false)?,
        Commands::Project { cmd } => cmd_project(cmd)?,
        Commands::LedTest { pad, color } => cmd_led_test(pad, color)?,
        Commands::LedCycle { duration } => cmd_led_cycle(duration)?,
        Commands::DisplayTest { r, g, b, duration } => cmd_display_test(r, g, b, duration)?,
        Commands::DisplayProbe => cmd_display_probe()?,
        Commands::AudioList => cmd_audio_list()?,
        Commands::AudioTest {
            freq,
            duration,
            device,
        } => cmd_audio_test(freq, duration, device)?,
        Commands::Gui {
            project,
            audio_device,
            no_audio,
        } => cmd_daemon(
            project,
            "Maschine MK3 Virtual".to_string(),
            audio_device,
            no_audio,
            false,
            "duplicate".to_string(),
            true,
        )?,
    }
    Ok(())
}

fn cmd_probe() -> anyhow::Result<()> {
    println!("# maschine-linux probe");
    println!("# PipeWire: {}", pipewire_info());
    let devs = find_maschine_devices()?;
    if devs.is_empty() {
        println!("Aucun device Maschine trouvé (17cc:1600 / 17cc:1700)");
        println!("- Vérifie lsusb | grep 17cc");
        println!("- Vérifie /dev/hidraw* + udev 99-maschine-mk3.rules");
        return Ok(());
    }
    for (i, d) in devs.iter().enumerate() {
        println!("Device #{i}:");
        println!(
            "  VID:PID {:04x}:{:04x}  serial={:?}  product={:?}",
            d.vendor_id, d.product_id, d.serial, d.product
        );
        println!("  hidraw: {:?}", d.hidraw_path);
        println!("  bus_port: {:?}", d.bus_port);
        if let Some(ref p) = d.hidraw_path {
            let perms = std::fs::metadata(p)
                .map(|m| format!("{:o}", m.permissions().mode() & 0o777))
                .unwrap_or_else(|_| "?".to_string());
            println!("  perms: {perms}  (doit être r/w via TAG+=\"uaccess\" ou plugdev)");
            // test ouverture
            match std::fs::OpenOptions::new().read(true).open(p) {
                Ok(_) => println!("  hidraw open: OK"),
                Err(e) => println!("  hidraw open: ERR {e} — sudo udevadm trigger + relogin ?"),
            }
        }
    }
    // ALSA
    println!("\n# ALSA cards:");
    if let Ok(cards) = std::fs::read_to_string("/proc/asound/cards") {
        print!("{cards}");
    }
    println!("\n# aconnect -l (MIDI):");
    let _ = std::process::Command::new("aconnect")
        .arg("-l")
        .status()
        .map(|_| {});
    Ok(())
}

fn pipewire_info() -> String {
    std::process::Command::new("pw-cli")
        .arg("info")
        .arg("0")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            s.lines()
                .find(|l| l.contains("version"))
                .map(|l| l.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn cmd_hid_dump(duration: u64, hidraw: Option<PathBuf>) -> anyhow::Result<()> {
    let devs = find_maschine_devices()?;
    let path = if let Some(p) = hidraw {
        p
    } else {
        devs.iter()
            .find_map(|d| d.hidraw_path.clone())
            .ok_or_else(|| {
                anyhow::anyhow!("pas de hidraw trouvé — branche la MK3 et lance probe")
            })?
    };
    println!("Dump {path:?} pendant {duration}s — appuie sur pads/boutons...");
    maschine_hardware::hidraw::dump_hidraw_sync(path, Duration::from_secs(duration))?;
    Ok(())
}

fn cmd_daemon(
    project: Option<PathBuf>,
    _midi_port: String,
    audio_device: Option<String>,
    no_audio: bool,
    no_led: bool,
    headphone_mode: String,
    with_gui: bool,
) -> anyhow::Result<()> {
    println!("# maschine-linux daemon — Ctrl+C pour quitter");
    let proj = if let Some(p) = project {
        println!("Chargement {p:?}");
        Project::load_json(&p)?
    } else {
        println!("Projet démo (16 pads, kit 808, pattern 2 bars)");
        Project::new_demo()
    };
    proj.validate()?;
    println!(
        "Project '{}' — {} groups, {} scenes, {} sections, bpm {}",
        proj.name,
        proj.groups.len(),
        proj.scenes.len(),
        proj.song.len(),
        proj.bpm
    );

    let devs = find_maschine_devices()?;
    let info = devs
        .into_iter()
        .find(|d| d.hidraw_path.is_some())
        .ok_or_else(|| anyhow::anyhow!("pas de MK3 hidraw trouvée — lance probe"))?;
    let hidraw_path = info.hidraw_path.clone().unwrap();
    println!(
        "Device: VID:PID {:04x}:{:04x} hidraw={:?} serial={:?}",
        info.vendor_id, info.product_id, hidraw_path, info.serial
    );

    let (tx, rx) = unbounded::<HardwareEvent>();
    let cfg = HidrawConfig::default();
    let _hid_handle = spawn_hidraw_thread(info, cfg, tx)?;

    // Engine partagé (pour audio thread + main loop)
    let engine = Arc::new(Mutex::new(Engine::new(proj, 48000)));
    // Ajoute le kit complet 808 DSP (16 instruments analogiques authentiques)
    {
        let mut eng = engine.lock();
        let kit = maschine_engine::generate_complete_808_kit(48000);
        for (pad, _name, samp) in kit {
            eng.sampler_mut().add_zone(
                pad,
                maschine_engine::sampler::ZoneMapping {
                    sample: Some(std::sync::Arc::new(samp)),
                    sample_path: None,
                    sample_rate: 48000,
                    root_key: 36 + pad,
                    lokey: 0,
                    hikey: 127,
                    lovel: 0,
                    hivel: 127,
                    volume_db: 0.0,
                    pan: 0.0,
                    tune_cents: 0,
                    choke_group: if pad == 2 || pad == 3 { Some(1) } else { None }, // Choke Closed HH <-> Open HH
                    is_one_shot: true,
                },
            );
        }
        // Activer la boucle de transport par défaut (2 mesures = 7680 ticks à 960 PPQ)
        eng.transport.loop_enabled = true;
        eng.transport.loop_start = 0;
        eng.transport.loop_end = 2 * 4 * 960;
    }

    // Audio — cpal
    let settings = settings::AppSettings::load();
    let initial_audio_dev = audio_device.or(settings.audio_device);
    let hp_dup = headphone_mode == "duplicate";

    #[cfg(feature = "audio")]
    let audio_shared: Arc<Mutex<Option<audio::AudioOutput>>> = Arc::new(Mutex::new(None));
    #[cfg(feature = "audio")]
    if no_audio {
        log::warn!("--no-audio : pas de sortie son");
    } else {
        match audio::AudioOutput::open(engine.clone(), initial_audio_dev.as_deref(), hp_dup) {
            Ok(a) => {
                info!(
                    "✓ Audio démarré: '{}' (cible {:?})",
                    a.device_name(),
                    initial_audio_dev
                );
                *audio_shared.lock() = Some(a);
            }
            Err(e) => {
                log::error!("Audio démarrage échec: {e} — continue sans son (modifiable via paramètres)");
                let _ = audio::AudioOutput::list_devices();
            }
        }
    }
    #[cfg(not(feature = "audio"))]
    {
        let _ = initial_audio_dev;
        if !no_audio {
            log::warn!("feature `audio` non activée — rebuild avec --features audio pour du son");
        }
    }

    // LED writer
    let (led_tx, led_rx) = unbounded::<maschine_proto::hid_output::LedState>();
    let _led_handle = if no_led {
        log::warn!("--no-led : LEDs désactivées");
        None
    } else {
        match hw_output::spawn_led_writer(hidraw_path.clone(), led_rx) {
            Ok(h) => {
                info!("✓ LED writer démarré sur {}", hidraw_path.display());
                // envoie état initial (arc-en-ciel)
                let mut init = maschine_proto::hid_output::LedState::new();
                let palette = [6u8, 22, 30, 42, 50, 58, 10, 14, 26, 31, 34, 38, 11, 27, 33, 78];
                for (i, &col) in palette.iter().enumerate().take(16) {
                    init.set_pad(i, maschine_proto::color::Color::from_index(col).unwrap_or(maschine_proto::color::Color::OFF));
                }
                let _ = led_tx.send(init);
                Some(h)
            }
            Err(e) => {
                log::error!("LED writer échec: {e}");
                None
            }
        }
    };

    // Display bulk (optionnel, feature usb)
    #[cfg(feature = "usb")]
    let _display_handle = match display_thread::spawn_display_thread(engine.clone()) {
        Ok(h) => {
            log::info!("✓ Thread affichage écran MK3 démarré");
            Some(h)
        }
        Err(e) => {
            log::warn!("Thread affichage écran échec: {e}");
            None
        }
    };

    // MIDI ports (ALSA Seq)
    #[cfg(feature = "midi-alsa")]
    let midi_ports = {
        use maschine_hardware::mapper::MidiEvent;
        match maschine_hardware::midi_io::MidiPorts::open(&_midi_port) {
            Ok((ports, midi_rx)) => {
                info!("✓ Port MIDI virtuel ALSA ouvert: '{}'", _midi_port);
                let engine_midi = engine.clone();
                std::thread::spawn(move || {
                    while let Ok(ev) = midi_rx.recv() {
                        let mut eng = engine_midi.lock();
                        match ev {
                            MidiEvent::NoteOn { note, velocity, .. } => {
                                let pad = note.saturating_sub(36).min(15);
                                let _ = eng.handle_event(EngineEvent::Pad {
                                    pad,
                                    velocity,
                                    pressed: true,
                                });
                            }
                            MidiEvent::NoteOff { note, .. } => {
                                let pad = note.saturating_sub(36).min(15);
                                let _ = eng.handle_event(EngineEvent::Pad {
                                    pad,
                                    velocity: 0,
                                    pressed: false,
                                });
                            }
                            MidiEvent::Transport { play, rec, stop } => {
                                let _ = eng.handle_event(EngineEvent::Transport { play, rec, stop });
                            }
                            _ => {}
                        }
                    }
                });
                Some(ports)
            }
            Err(e) => {
                warn!("MIDI ports échec: {e}");
                None
            }
        }
    };

    // Helper : applique les actions du HardwareMapper à l'Engine
    #[allow(unused_variables)]
    fn apply_mapper_actions(
        engine: &Arc<Mutex<Engine>>,
        mapper: &mut HardwareMapper,
        actions: Vec<MapperAction>,
    ) {
        for action in actions {
            match action {
                MapperAction::Midi(m) => {
                    log::debug!("MIDI {:?}", m);
                }
                MapperAction::PadTrigger { pad, velocity, pressed } => {
                    let _ = engine.lock().handle_event(EngineEvent::Pad {
                        pad,
                        velocity,
                        pressed,
                    });
                }
                MapperAction::ToggleStep { sound, bar, step } => {
                    let tick = (bar as u32 * 4 * 960) + (step as u32 * 240);
                    engine.lock().toggle_step(sound, tick);
                }
                MapperAction::PlayPauseToggle => {
                    let mut eng = engine.lock();
                    let is_playing = eng.transport.is_playing();
                    if is_playing {
                        let _ = eng.handle_event(EngineEvent::Transport { play: false, rec: false, stop: true });
                    } else {
                        let _ = eng.handle_event(EngineEvent::Transport { play: true, rec: false, stop: false });
                    }
                }
                MapperAction::Stop => {
                    let _ = engine.lock().handle_event(EngineEvent::Transport { play: false, rec: false, stop: true });
                }
                MapperAction::RecordToggle => {
                    let mut eng = engine.lock();
                    let is_rec = eng.transport.state == maschine_engine::transport::TransportState::Recording;
                    let _ = eng.handle_event(EngineEvent::Transport { play: true, rec: !is_rec, stop: false });
                }
                MapperAction::Restart => {
                    let mut eng = engine.lock();
                    eng.transport.tick = 0;
                }
                MapperAction::BpmDelta(delta) => {
                    let mut eng = engine.lock();
                    let new_bpm = (eng.transport.bpm + delta).clamp(20.0, 999.0);
                    let _ = eng.handle_event(EngineEvent::Bpm(new_bpm));
                }
                MapperAction::BpmSet(bpm) => {
                    let _ = engine.lock().handle_event(EngineEvent::Bpm(bpm));
                }
                MapperAction::SwingDelta(delta) => {
                    let eng = engine.lock();
                    let proj = eng.project_handle();
                    let new_sw = (proj.read().swing + delta).clamp(0.0, 1.0);
                    proj.write().swing = new_sw;
                }
                MapperAction::MasterVolumeDelta(delta) => {
                    let mut eng = engine.lock();
                    let current = eng.snapshot().master_volume;
                    let new_vol = (current + delta).clamp(0.0, 1.0);
                    let _ = eng.handle_event(EngineEvent::MasterVolume(new_vol));
                }
                MapperAction::GroupVolumeDelta(delta) => {
                    let mut eng = engine.lock();
                    let current = eng.snapshot().master_volume;
                    let new_vol = (current + delta).clamp(0.0, 1.0);
                    let _ = eng.handle_event(EngineEvent::MasterVolume(new_vol));
                }
                MapperAction::GroupPanDelta(_) => {}
                MapperAction::GroupFocus(g) => {
                    let _ = engine.lock().handle_event(EngineEvent::GroupFocus(g));
                }
                MapperAction::SelectBar(b) => {
                    mapper.selected_bar = b;
                }
                MapperAction::SelectSound(s) => {
                    mapper.active_sound = s;
                }
                MapperAction::ToggleMute(g) => {
                    let _ = engine.lock().handle_event(EngineEvent::Mute(g));
                }
                MapperAction::ToggleSolo(g) => {
                    let _ = engine.lock().handle_event(EngineEvent::Solo(g));
                }
                MapperAction::ClearBar(b) => {
                    engine.lock().clear_bar(b);
                }
                MapperAction::DuplicateBar => {
                    let eng = engine.lock();
                    let g = eng.snapshot().group_focus as usize;
                    let proj = eng.project_handle();
                    let mut guard = proj.write();
                    if let Some(group) = guard.groups.get_mut(g) {
                        if let Some(pat) = group.patterns.first_mut() {
                            let bar1_events: Vec<_> = pat.events.iter()
                                .filter(|e| e.tick < 4 * 960)
                                .cloned()
                                .collect();
                            pat.events.retain(|e| e.tick < 4 * 960);
                            for mut ev in bar1_events {
                                ev.tick += 4 * 960;
                                pat.events.push(ev);
                            }
                        }
                    }
                }
                MapperAction::ModeSwitch(mode) => {
                    mapper.mode = mode;
                }
                MapperAction::ResetDemoGroove => {
                    engine.lock().reset_demo_groove();
                }
                MapperAction::SetLeftTab(tab) => {
                    let _ = engine.lock().handle_event(EngineEvent::SetLeftTab(tab));
                }
                MapperAction::SetRightTab(tab) => {
                    let _ = engine.lock().handle_event(EngineEvent::SetRightTab(tab));
                }
                MapperAction::Variation => {
                    engine.lock().humanize_active_pattern();
                }
                MapperAction::Quantize => {
                    engine.lock().quantize_active_pattern();
                }
                MapperAction::SaveProject => {
                    let base = std::env::var("XDG_DATA_HOME")
                        .map(std::path::PathBuf::from)
                        .unwrap_or_else(|_| {
                            std::env::var("HOME")
                                .map(|h| std::path::PathBuf::from(h).join(".local/share"))
                                .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
                        });
                    let autosave_dir = base.join("maschine-linux");
                    let _ = std::fs::create_dir_all(&autosave_dir);
                    let autosave = autosave_dir.join("autosave.json");
                    let eng = engine.lock();
                    let snap = eng.snapshot();
                    if let Ok(json) = serde_json::to_string_pretty(&snap.project) {
                        let _ = std::fs::write(&autosave, json);
                        log::info!("Autosave → {:?}", autosave);
                    }
                }
            }
        }
    }

    // Helper : synchronise l'affichage physique de toutes les LEDs (30-60 Hz)
    fn sync_hardware_leds(
        engine: &Arc<Mutex<Engine>>,
        mapper: &mut HardwareMapper,
        led_tx: &crossbeam_channel::Sender<maschine_proto::hid_output::LedState>,
    ) {
        let snap = engine.lock().snapshot();
        let is_p = snap.transport.is_playing();
        let is_rec = snap.transport.state == maschine_engine::transport::TransportState::Recording;
        let is_stopped = !is_p && !is_rec;
        let current_bar = ((snap.transport.tick / (4 * 960)) % 2) as usize;
        let current_step = ((snap.transport.tick / 240) % 16) as usize;
        let is_quarter_beat = (snap.transport.tick % 960) < 240 && is_p;

        let mut active_steps = [false; 16];
        if let Some(group) = snap.project.groups.get(snap.group_focus as usize) {
            if let Some(pat) = group.patterns.first() {
                let bar_offset = mapper.selected_bar as u32 * 4 * 960;
                for e in &pat.events {
                    if e.pad == mapper.active_sound && e.tick >= bar_offset && e.tick < bar_offset + (4 * 960) {
                        let step_idx = ((e.tick - bar_offset) / 240) as usize;
                        if step_idx < 16 {
                            active_steps[step_idx] = true;
                        }
                    }
                }
            }
        }

        let led_info = SequencerLedInfo {
            is_playing: is_p,
            is_recording: is_rec,
            is_stopped,
            current_bar,
            current_step,
            is_quarter_beat,
            active_steps,
            triggered_pads: [false; 16],
            master_vol: snap.master_volume,
            muted_groups: snap.muted_groups,
            soloed_groups: snap.soloed_groups,
            left_tab: snap.left_tab,
            right_tab: snap.right_tab,
        };
        mapper.update_leds(&led_info);
        let _ = led_tx.send(mapper.led_state().clone());
    }

    // ----- GUI mode: run event loop in background, GUI on main thread -----
    #[cfg(feature = "gui")]
    if with_gui {
        // Spawn HID event loop in background thread
        let engine_bg = engine.clone();
        let led_tx_bg = led_tx.clone();
        std::thread::Builder::new()
            .name("maschine-hid-loop".to_string())
            .spawn(move || {
                let mut mapper = HardwareMapper::new(MapperConfig::default());
                if !no_led {
                    sync_hardware_leds(&engine_bg, &mut mapper, &led_tx_bg);
                }
                let mut last_advance = std::time::Instant::now();
                let mut last_led_sync = std::time::Instant::now();
                let use_manual_advance = no_audio || cfg!(not(feature = "audio"));
                loop {
                    if use_manual_advance {
                        let elapsed = last_advance.elapsed();
                        if elapsed >= Duration::from_millis(10) {
                            let samples = (elapsed.as_secs_f64() * 48000.0) as u64;
                            let mut eng = engine_bg.lock();
                            eng.advance(samples);
                            let mut dummy = vec![0.0; 480 * 2];
                            eng.render_audio(&mut dummy);
                            drop(eng);
                            last_advance = std::time::Instant::now();
                        }
                    }

                    // Rafraîchissement régulier des LEDs physiques (30 FPS)
                    if !no_led && last_led_sync.elapsed() >= Duration::from_millis(33) {
                        sync_hardware_leds(&engine_bg, &mut mapper, &led_tx_bg);
                        last_led_sync = std::time::Instant::now();
                    }

                    match rx.recv_timeout(Duration::from_millis(5)) {
                        Ok(ev) => {
                            match &ev {
                                HardwareEvent::Connected { serial } => {
                                    info!("✓ MK3 connectée serial={:?}", serial);
                                }
                                HardwareEvent::Error(e) => error!("hardware error: {e}"),
                                HardwareEvent::Disconnected => {
                                    warn!("MK3 déconnectée");
                                    return;
                                }
                                HardwareEvent::Pot { id, value } => {
                                    use maschine_proto::ButtonId;
                                    match id {
                                        ButtonId::Unknown(99, 2) => {
                                            let vol = *value as f32 / 1023.0;
                                            let _ = engine_bg.lock().handle_event(EngineEvent::MasterVolume(vol));
                                        }
                                        ButtonId::Unknown(99, 1) => {
                                            let vol = *value as f32 / 1023.0;
                                            let _ = engine_bg.lock().handle_event(EngineEvent::HeadphoneVolume(vol));
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }

                            let actions = mapper.handle_hardware_event(ev);
                            #[cfg(feature = "midi-alsa")]
                            if let Some(ref ports) = midi_ports {
                                for a in &actions {
                                    if let MapperAction::Midi(m) = a {
                                        ports.send(m);
                                    }
                                }
                            }
                            apply_mapper_actions(&engine_bg, &mut mapper, actions);

                            if !no_led {
                                sync_hardware_leds(&engine_bg, &mut mapper, &led_tx_bg);
                                last_led_sync = std::time::Instant::now();
                            }
                        }
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                            warn!("channel disconnected");
                            return;
                        }
                    }
                }
            })?;

        // Run GUI on main thread (required by egui/winit)
        info!("Lancement GUI egui avec hardware connecté…");
        #[cfg(feature = "audio")]
        return gui::run_with_engine(engine, audio_shared);
        #[cfg(not(feature = "audio"))]
        return gui::run_with_engine(engine);
    }

    // ----- Daemon mode: blocking event loop on main thread -----
    let mut mapper = HardwareMapper::new(MapperConfig::default());
    if !no_led {
        sync_hardware_leds(&engine, &mut mapper, &led_tx);
    }

    // heartbeat snapshot
    let engine_clone = engine.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(2));
        let snap = engine_clone.lock().snapshot();
        log::info!(
            "♥ heartbeat project='{}' groups={} bpm={} tick={} voices={} group_focus={}",
            snap.project.name,
            snap.project.groups.len(),
            snap.transport.bpm,
            snap.transport.tick,
            snap.num_voices,
            snap.group_focus
        );
    });

    let mut last_advance = std::time::Instant::now();
    let mut last_led_sync = std::time::Instant::now();
    let use_manual_advance = no_audio || cfg!(not(feature = "audio"));
    loop {
        if use_manual_advance {
            let elapsed = last_advance.elapsed();
            if elapsed >= Duration::from_millis(10) {
                let samples = (elapsed.as_secs_f64() * 48000.0) as u64;
                let mut eng = engine.lock();
                eng.advance(samples);
                let mut dummy = vec![0.0; 480 * 2];
                eng.render_audio(&mut dummy);
                drop(eng);
                last_advance = std::time::Instant::now();
            }
        }

        // Rafraîchissement régulier des LEDs physiques (30 FPS)
        if !no_led && last_led_sync.elapsed() >= Duration::from_millis(33) {
            sync_hardware_leds(&engine, &mut mapper, &led_tx);
            last_led_sync = std::time::Instant::now();
        }

        // poll events avec timeout 5ms
        match rx.recv_timeout(Duration::from_millis(5)) {
            Ok(ev) => {
                match &ev {
                    HardwareEvent::Connected { serial } => {
                        info!("✓ MK3 connectée serial={:?}", serial);
                    }
                    HardwareEvent::Error(e) => error!("hardware error: {e}"),
                    HardwareEvent::Disconnected => {
                        warn!("MK3 déconnectée");
                        break;
                    }
                    HardwareEvent::Pot { id, value } => {
                        use maschine_proto::ButtonId;
                        match id {
                            ButtonId::Unknown(99, 2) => {
                                let vol = *value as f32 / 1023.0;
                                let _ = engine.lock().handle_event(EngineEvent::MasterVolume(vol));
                            }
                            ButtonId::Unknown(99, 1) => {
                                let vol = *value as f32 / 1023.0;
                                let _ = engine.lock().handle_event(EngineEvent::HeadphoneVolume(vol));
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }

                let actions = mapper.handle_hardware_event(ev);
                #[cfg(feature = "midi-alsa")]
                if let Some(ref ports) = midi_ports {
                    for a in &actions {
                        if let MapperAction::Midi(m) = a {
                            ports.send(m);
                        }
                    }
                }
                apply_mapper_actions(&engine, &mut mapper, actions);

                if !no_led {
                    sync_hardware_leds(&engine, &mut mapper, &led_tx);
                    last_led_sync = std::time::Instant::now();
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                warn!("channel disconnected");
                break;
            }
        }

        if arc_ctrlc_flag() {
            println!("\nCtrl+C — arrêt");
            break;
        }
    }

    // save autosave
    let autosave = dirs_or_tmp().join("maschine-linux/autosave.json");
    {
        let eng = engine.lock();
        if let Err(e) = eng.snapshot().project.save_json(&autosave) {
            warn!("autosave failed {autosave:?}: {e}");
        } else {
            println!("Autosave → {autosave:?}");
        }
    }

    Ok(())
}

static CTRLC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn arc_ctrlc_flag() -> bool {
    CTRLC.load(std::sync::atomic::Ordering::Relaxed)
}
fn init_ctrlc() {
    let _ = ctrlc::set_handler(|| CTRLC.store(true, std::sync::atomic::Ordering::Relaxed));
}
// On l'appelle lazy
fn dirs_or_tmp() -> PathBuf {
    std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".local/share")
        })
}

fn cmd_project(cmd: ProjectCmd) -> anyhow::Result<()> {
    match cmd {
        ProjectCmd::New { out } => {
            let p = Project::new_demo();
            p.save_json(&out)?;
            println!("Projet démo → {out:?}");
            println!("{} groups, {} scenes", p.groups.len(), p.scenes.len());
        }
        ProjectCmd::Show { path } => {
            let p = Project::load_json(&path)?;
            println!("{}", serde_json::to_string_pretty(&p)?);
        }
        ProjectCmd::Validate { path } => {
            let p = Project::load_json(&path)?;
            p.validate()?;
            println!(
                "✓ {path:?} valide (schema {}, bpm {})",
                p.schema_version, p.bpm
            );
        }
    }
    Ok(())
}

fn cmd_led_test(pad: u8, color: u8) -> anyhow::Result<()> {
    use maschine_proto::color::Color;
    use maschine_proto::hid_output::{encode_report_80, encode_report_81, LedState};

    if pad >= 16 {
        anyhow::bail!("pad doit être 0..15");
    }
    let col = Color::from_index(color).ok_or_else(|| {
        anyhow::anyhow!("couleur index invalide {color} — voir PaletteIndex::ALL")
    })?;
    let mut state = LedState::new();
    state.set_pad(pad as usize, col);
    state.set_button_raw(maschine_proto::hid_output::offsets::SHIFT, 7);
    let r80 = encode_report_80(&state)?;
    let r81 = encode_report_81(&state)?;
    println!("LED test pad {pad} color {color} ({}):", col.0 as u8);
    println!(
        "  0x80 ({}B): {:02x?}...",
        r80.len(),
        &r80[..16.min(r80.len())]
    );
    println!(
        "  0x81 ({}B): {:02x?}...",
        r81.len(),
        &r81[..16.min(r81.len())]
    );

    // Si hidraw dispo, tente d'envoyer via hidraw OUT (nécessite perms + O_WRONLY)
    let devs = find_maschine_devices()?;
    if let Some(info) = devs.iter().find(|d| d.hidraw_path.is_some()) {
        if let Some(ref p) = info.hidraw_path {
            println!("Envoi vers {p:?}...");
            // hidraw OUT se fait via write() sur le même fd, mais il faut ouvrir en RDWR et gérer report ID
            // Certains kernels exigent 62/43 bytes avec ID inclus.
            match std::fs::OpenOptions::new().read(true).write(true).open(p) {
                Ok(mut f) => {
                    use std::io::Write;
                    // Note : hidraw attend le report sans ID sur certains kernels, avec ID sur d'autres
                    // On tente avec ID d'abord
                    match f.write_all(&r81) {
                        Ok(_) => println!("✓ write 0x81 OK (avec ID)"),
                        Err(e) => {
                            println!("write avec ID échoué: {e} — tentative sans ID");
                            let _ = f.write_all(&r81[1..]);
                            println!("write sans ID tenté");
                        }
                    }
                    let _ = f.write_all(&r80);
                    println!("✓ write 0x80 tenté");
                    std::thread::sleep(Duration::from_millis(800));
                    // éteint
                    let off = LedState::new();
                    let r81off = encode_report_81(&off)?;
                    let _ = f.write_all(&r81off);
                    println!("LEDs éteintes après 800ms");
                }
                Err(e) => println!("open hidraw write échoué: {e} — besoin udev + plugdev ?"),
            }
        }
    } else {
        println!("(pas de hidraw — juste encode test, pas d'envoi hardware)");
    }
    Ok(())
}

fn cmd_led_cycle(duration: u64) -> anyhow::Result<()> {
    let devs = find_maschine_devices()?;
    let path = devs
        .iter()
        .find_map(|d| d.hidraw_path.clone())
        .ok_or_else(|| anyhow::anyhow!("pas de hidraw trouvé"))?;
    println!("LED cycle sur {path:?} pendant {duration}ms");
    hw_output::led_test_cycle(&path, duration)?;
    println!("✓ cycle terminé");
    Ok(())
}

fn cmd_display_test(r: u8, g: u8, b: u8, duration: u64) -> anyhow::Result<()> {
    #[cfg(feature = "usb")]
    {
        let color = maschine_proto::display::DisplayPixel::from_rgb(r, g, b);
        println!("Display test RGB({r},{g},{b}) {duration}ms");
        hw_output::display::show_solid(color, duration)?;
        println!("✓ display solid OK");
        Ok(())
    }
    #[cfg(not(feature = "usb"))]
    {
        let _ = (r, g, b, duration);
        anyhow::bail!("feature `usb` non activée — rebuild avec --features usb,display (sudo dnf install libusb1-devel)");
    }
}

fn cmd_display_probe() -> anyhow::Result<()> {
    #[cfg(feature = "usb")]
    {
        use rusb::UsbContext;
        println!("# Display Probe — protocole ni-controllers-lib (IF5, EP 0x04)");
        println!("  Format: 16B header + 4B cmd + 261120B pixels + 4B blit + 4B end = 261148B");

        println!("\n1. Ouverture USB context...");
        let ctx = rusb::Context::new()?;
        let devices = ctx.devices()?;
        let dev = devices.iter().find(|d| {
            if let Ok(desc) = d.device_descriptor() {
                desc.vendor_id() == 0x17cc && desc.product_id() == 0x1600
            } else {
                false
            }
        }).ok_or_else(|| anyhow::anyhow!("Maschine MK3 (17cc:1600) non trouvée en USB"))?;

        let handle = dev.open()?;
        let _ = handle.set_auto_detach_kernel_driver(true);
        handle.claim_interface(5)?;
        println!("   ✓ Interface 5 (Maschine MK3 BD) réclamée !");

        // Helper: construit et envoie une frame complète selon le protocole de référence
        let _send_solid = |handle: &rusb::DeviceHandle<rusb::Context>, display_id: u8, r: u8, g: u8, b: u8| -> anyhow::Result<()> {
            let color = maschine_proto::display::DisplayPixel::from_rgb(r, g, b);
            let did = if display_id == 0 { maschine_proto::display::DisplayId::Left } else { maschine_proto::display::DisplayId::Right };
            let frame = maschine_proto::display::DisplayFrame::solid(did, color);
            let data = maschine_proto::display::encode_bulk_transfer(&frame)?;

            println!("   Frame {:5} octets — header[0..20]: {:02x?}",
                data.len(), &data[0..20]);
            println!("   footer[-8..]: {:02x?}", &data[data.len()-8..]);

            let n = handle.write_bulk(0x04, &data, std::time::Duration::from_millis(2000))?;
            if n != data.len() {
                anyhow::bail!("Short write: {n}/{}", data.len());
            }
            println!("   ✓ Écran {} : {n} octets envoyés OK", display_id);
            Ok(())
        };

        // Helper: split 4-write (ni-display)
        let _send_split = |handle: &rusb::DeviceHandle<rusb::Context>, display_id: u8, r: u8, g: u8, b: u8| -> anyhow::Result<()> {
            let color = maschine_proto::display::DisplayPixel::from_rgb(r, g, b);
            let did = if display_id == 0 { maschine_proto::display::DisplayId::Left } else { maschine_proto::display::DisplayId::Right };
            let frame = maschine_proto::display::DisplayFrame::solid(did, color);
            let data = maschine_proto::display::encode_bulk_transfer(&frame)?;
            let timeout = std::time::Duration::from_millis(1000);
            handle.write_bulk(0x04, &data[0..16], timeout)?;
            handle.write_bulk(0x04, &data[16..20], timeout)?;
            handle.write_bulk(0x04, &data[20..20+261120], timeout)?;
            handle.write_bulk(0x04, &data[20+261120..], timeout)?;
            Ok(())
        };

        // Helper: row-by-row (cabl)
        let _send_row_by_row = |handle: &rusb::DeviceHandle<rusb::Context>, display_id: u8, r: u8, g: u8, b: u8| -> anyhow::Result<()> {
            let color = maschine_proto::display::DisplayPixel::from_rgb(r, g, b);
            let px_bytes = color.to_be_bytes();
            let timeout = std::time::Duration::from_millis(500);
            for row in 0..272u16 {
                let mut pkt = Vec::with_capacity(20 + 480 * 2 + 8);
                pkt.extend_from_slice(&[
                    0x84, 0x00, display_id, 0x60,
                    0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, (row >> 8) as u8, (row & 0xFF) as u8,
                    0x01, 0xE0, 0x00, 0x01,
                ]);
                pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0xF0]);
                for _ in 0..480 {
                    pkt.extend_from_slice(&px_bytes);
                }
                pkt.extend_from_slice(&[
                    0x03, 0x00, 0x00, 0x00,
                    0x40, 0x00, 0x00, 0x00,
                ]);
                handle.write_bulk(0x04, &pkt, timeout)?;
            }
            Ok(())
        };

        println!("\n2. AFFICHAGE DE L'INTERFACE DAW OFFICIELLE SUR LES 2 ÉCRANS...");
        let mut snap = maschine_engine::EngineSnapshot {
            project: maschine_engine::project::Project::new_demo(),
            transport: maschine_engine::transport::Transport::new(120.0, 48000),
            group_focus: 0,
            active_scene: Some(0),
            bbt: maschine_engine::project::BarsBeatsTicks::new(1, 0, 0),
            num_voices: 4,
            master_volume: 0.85,
            headphone_volume: 0.7,
            muted_groups: 0,
            soloed_groups: 0,
            left_tab: 0,
            right_tab: 0,
            active_sound: 0,
        };
        snap.transport.state = maschine_engine::transport::TransportState::Playing;

        println!("   Boucle active temps-réel (15 FPS pendant 10s) :");
        println!("   - Écran Gauche : [ARRANGE/MIXER/SAMPLER/BROWSER] + Groupes A..H + Grille 16 Pads Kit 808 + Encodeurs 1..4");
        println!("   - Écran Droit  : [PATTERN/EVENTS/MACRO/SETTINGS] + Transport BBT + Step-Sequencer 16 pas + VU-Mètres Stéréo L/R + Encodeurs 5..8");

        let start = std::time::Instant::now();
        let mut frame_idx = 0u64;

        while start.elapsed() < std::time::Duration::from_secs(12) {
            // Avance le BBT en simulation de lecture 120 BPM
            let total_ticks = (frame_idx * 64) as u32;
            let beats = ((total_ticks / 960) % 4) as u8;
            let bars = ((total_ticks / 3840) + 1) as u16;
            let ticks = (total_ticks % 960) as u16;
            snap.bbt = maschine_engine::project::BarsBeatsTicks::new(bars, beats, ticks);

            // Dynamique des VU-mètres
            let l_dyn = 0.5 + 0.4 * (((frame_idx as f32 * 0.4).sin() + 1.0) / 2.0);
            snap.master_volume = l_dyn;

            let left_frame = maschine_engine::display_render::render_daw_left(&snap);
            let right_frame = maschine_engine::display_render::render_daw_right(&snap);

            let data_l = maschine_proto::display::encode_bulk_transfer(&left_frame)?;
            let data_r = maschine_proto::display::encode_bulk_transfer(&right_frame)?;

            handle.write_bulk(0x04, &data_l, std::time::Duration::from_millis(500))?;
            handle.write_bulk(0x04, &data_r, std::time::Duration::from_millis(500))?;

            frame_idx += 1;
            std::thread::sleep(std::time::Duration::from_millis(66));
        }

        println!("   ✓ {frame_idx} frames de l'interface DAW rendues avec succès sur les 2 écrans.");
        let _ = handle.release_interface(5);
        Ok(())
    }
    #[cfg(not(feature = "usb"))]
    {
        anyhow::bail!("feature usb non activée");
    }
}

fn cmd_audio_list() -> anyhow::Result<()> {
    #[cfg(feature = "audio")]
    {
        audio::AudioOutput::list_devices()
    }
    #[cfg(not(feature = "audio"))]
    {
        anyhow::bail!("feature `audio` non activée — rebuild avec --features audio");
    }
}

fn cmd_audio_test(freq: f32, duration: f32, device: Option<String>) -> anyhow::Result<()> {
    #[cfg(feature = "audio")]
    {
        let target = device.as_deref();
        println!(
            "Audio test tone {freq}Hz {duration}s (cible: {})",
            target.unwrap_or("défaut système / PC")
        );
        audio::play_test_tone(duration, freq, target)?;
        println!("✓ Test tone terminé avec succès.");
        Ok(())
    }
    #[cfg(not(feature = "audio"))]
    {
        let _ = (freq, duration, device);
        anyhow::bail!("feature `audio` non activée — rebuild avec --features audio");
    }
}

#[cfg(feature = "gui")]
mod gui {
    use super::*;
    use eframe::egui;

    #[allow(dead_code)]
    pub fn run(project: Option<PathBuf>) -> anyhow::Result<()> {
        cmd_daemon(
            project,
            "Maschine MK3 Virtual".to_string(),
            None,
            false,
            false,
            "duplicate".to_string(),
            true,
        )
    }

    #[cfg(feature = "audio")]
    pub fn run_with_engine(
        engine: Arc<Mutex<Engine>>,
        audio: Arc<Mutex<Option<audio::AudioOutput>>>,
    ) -> anyhow::Result<()> {
        run_native_gui(engine, Some(audio))
    }

    #[cfg(not(feature = "audio"))]
    pub fn run_with_engine(
        engine: Arc<Mutex<Engine>>,
    ) -> anyhow::Result<()> {
        run_native_gui(engine)
    }

    fn run_native_gui(
        engine: Arc<Mutex<Engine>>,
        #[cfg(feature = "audio")]
        audio: Option<Arc<Mutex<Option<audio::AudioOutput>>>>,
    ) -> anyhow::Result<()> {
        let opts = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1280.0, 820.0])
                .with_min_inner_size([960.0, 600.0]),
            ..Default::default()
        };
        eframe::run_native(
            "Maschine MK3 Linux",
            opts,
            Box::new(move |cc| {
                let mut visuals = egui::Visuals::dark();
                visuals.panel_fill = egui::Color32::from_rgb(22, 22, 30);
                visuals.window_fill = egui::Color32::from_rgb(28, 28, 38);
                visuals.extreme_bg_color = egui::Color32::from_rgb(12, 12, 18);
                visuals.faint_bg_color = egui::Color32::from_rgb(32, 34, 44);
                visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(40, 42, 55);
                visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 60, 80);
                visuals.widgets.active.bg_fill = egui::Color32::from_rgb(70, 80, 110);
                visuals.selection.bg_fill = egui::Color32::from_rgb(0, 140, 220);
                cc.egui_ctx.set_visuals(visuals);

                #[cfg(feature = "audio")]
                let app = GuiApp::new(engine, audio);
                #[cfg(not(feature = "audio"))]
                let app = GuiApp::new(engine);

                Ok(Box::new(app))
            }),
        )
        .map_err(|e| anyhow::anyhow!("eframe: {e}"))?;
        Ok(())
    }

    const PAD_COLORS: [egui::Color32; 16] = [
        egui::Color32::from_rgb(230, 50, 50),
        egui::Color32::from_rgb(230, 120, 30),
        egui::Color32::from_rgb(230, 200, 0),
        egui::Color32::from_rgb(120, 220, 0),
        egui::Color32::from_rgb(0, 200, 80),
        egui::Color32::from_rgb(0, 200, 180),
        egui::Color32::from_rgb(0, 160, 230),
        egui::Color32::from_rgb(50, 100, 230),
        egui::Color32::from_rgb(120, 60, 230),
        egui::Color32::from_rgb(180, 50, 220),
        egui::Color32::from_rgb(220, 50, 150),
        egui::Color32::from_rgb(200, 80, 80),
        egui::Color32::from_rgb(160, 180, 0),
        egui::Color32::from_rgb(0, 180, 130),
        egui::Color32::from_rgb(80, 80, 200),
        egui::Color32::from_rgb(200, 200, 210),
    ];

    const SOUND_NAMES: [&str; 16] = [
        "Kick 808",
        "Snare 808",
        "Closed HH",
        "Open HH",
        "Tom Low",
        "Tom Mid",
        "Tom High",
        "Clap 808",
        "Rimshot",
        "Shaker",
        "Cowbell",
        "Crash",
        "Perc 1",
        "Perc 2",
        "Synth Stab",
        "Sub Bass",
    ];

    struct GuiApp {
        engine: Arc<Mutex<Engine>>,
        #[cfg(feature = "audio")]
        audio: Option<Arc<Mutex<Option<audio::AudioOutput>>>>,
        show_settings: bool,
        #[cfg(feature = "audio")]
        available_devices: Vec<audio::AudioDeviceInfo>,
        selected_device: String,
        status_message: Option<(String, bool)>,
        selected_bar: usize,
    }

    impl GuiApp {
        #[cfg(feature = "audio")]
        fn new(
            engine: Arc<Mutex<Engine>>,
            audio: Option<Arc<Mutex<Option<audio::AudioOutput>>>>,
        ) -> Self {
            let devices = audio::list_output_devices();
            let current_dev = audio
                .as_ref()
                .and_then(|a| a.lock().as_ref().map(|o| o.device_name().to_string()))
                .unwrap_or_else(|| "default".to_string());
            let settings = settings::AppSettings::load();
            let selected = settings.audio_device.unwrap_or(current_dev);
            Self {
                engine,
                audio,
                show_settings: false,
                available_devices: devices,
                selected_device: selected,
                status_message: None,
                selected_bar: 0,
            }
        }

        #[cfg(not(feature = "audio"))]
        fn new(engine: Arc<Mutex<Engine>>) -> Self {
            Self {
                engine,
                show_settings: false,
                selected_device: "disabled".to_string(),
                status_message: None,
                selected_bar: 0,
            }
        }

        fn toggle_step(&self, pad: u8, tick: u32) {
            self.engine.lock().toggle_step(pad, tick);
        }

        fn fill_four_on_the_floor(&self, pad: u8, bar: usize) {
            self.engine.lock().fill_four_on_the_floor(pad, bar);
        }

        fn fill_eighths(&self, pad: u8, bar: usize) {
            self.engine.lock().fill_eighths(pad, bar);
        }

        fn clear_bar(&self, bar: usize) {
            self.engine.lock().clear_bar(bar);
        }

        fn reset_demo_groove(&self) {
            self.engine.lock().reset_demo_groove();
        }

        #[cfg(feature = "audio")]
        fn refresh_devices(&mut self) {
            self.available_devices = audio::list_output_devices();
        }

        #[cfg(feature = "audio")]
        fn switch_audio_device(&mut self, target: &str) {
            if let Some(ref audio_holder) = self.audio {
                // 1. Fermer et libérer explicitement l'ancien flux audio ALSA/PipeWire
                {
                    let mut guard = audio_holder.lock();
                    *guard = None;
                }
                std::thread::sleep(std::time::Duration::from_millis(30));

                let hp_dup = true;
                match audio::AudioOutput::open(self.engine.clone(), Some(target), hp_dup) {
                    Ok(new_out) => {
                        let dev_name = new_out.device_name().to_string();
                        let rate = new_out.sample_rate();
                        *audio_holder.lock() = Some(new_out);
                        self.selected_device = dev_name.clone();
                        self.status_message = Some((
                            format!("✓ Connecté à '{dev_name}' ({rate} Hz)"),
                            false,
                        ));
                        let mut s = settings::AppSettings::load();
                        s.audio_device = Some(target.to_string());
                        let _ = s.save();
                    }
                    Err(e) => {
                        self.status_message = Some((
                            format!("Erreur lors de l'ouverture de '{target}': {e}"),
                            true,
                        ));
                    }
                }
            }
        }
    }

    impl eframe::App for GuiApp {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            // Raccourci barre d'espace pour Play / Pause
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                let mut eng = self.engine.lock();
                let is_playing = eng.transport.is_playing();
                if is_playing {
                    eng.handle_event(EngineEvent::Transport { play: false, rec: false, stop: true }).ok();
                } else {
                    eng.handle_event(EngineEvent::Transport { play: true, rec: false, stop: false }).ok();
                }
            }

            let snap = self.engine.lock().snapshot();

            let cyan = egui::Color32::from_rgb(0, 200, 255);
            let green = egui::Color32::from_rgb(80, 220, 100);
            let red = egui::Color32::from_rgb(230, 60, 60);
            let yellow = egui::Color32::from_rgb(255, 200, 0);
            let dim = egui::Color32::from_rgb(140, 150, 170);

            let is_playing = snap.transport.is_playing();
            let current_step = ((snap.transport.tick / 240) % 16) as usize;
            let current_bar = ((snap.transport.tick / (4 * 960)) % 2) as usize;

            // ======== SINGLE TOP BAR (Title + Transport + BPM + Volume) ========
            egui::TopBottomPanel::top("header").show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;

                    // Logo & Status
                    ui.colored_label(cyan, egui::RichText::new("⬣ MASCHINE LINUX").size(16.0).strong());
                    ui.colored_label(green, egui::RichText::new("● HW OK").size(11.0));

                    ui.separator();

                    // Transport Buttons avec Play bien visible
                    if is_playing {
                        if ui.add(
                            egui::Button::new(egui::RichText::new("■ STOP (Espace)").color(egui::Color32::WHITE).strong().size(13.0))
                                .fill(red)
                                .min_size(egui::vec2(120.0, 26.0)),
                        ).clicked() {
                            self.engine.lock().handle_event(EngineEvent::Transport { play: false, rec: false, stop: true }).ok();
                        }
                    } else {
                        if ui.add(
                            egui::Button::new(egui::RichText::new("▶ PLAY (Espace)").color(egui::Color32::BLACK).strong().size(13.0))
                                .fill(green)
                                .min_size(egui::vec2(120.0, 26.0)),
                        ).clicked() {
                            self.engine.lock().handle_event(EngineEvent::Transport { play: true, rec: false, stop: false }).ok();
                        }
                    }

                    if ui.button(egui::RichText::new("⟲ Restart").strong()).clicked() {
                        self.engine.lock().transport.tick = 0;
                    }

                    // Bouton boucle
                    let loop_bg = if snap.transport.loop_enabled { egui::Color32::from_rgb(20, 60, 40) } else { egui::Color32::TRANSPARENT };
                    let loop_txt_col = if snap.transport.loop_enabled { green } else { dim };
                    if ui.add(
                        egui::Button::new(egui::RichText::new("🔁 Boucle 2 Bars").color(loop_txt_col).strong())
                            .fill(loop_bg)
                    ).clicked() {
                        let mut eng = self.engine.lock();
                        eng.transport.loop_enabled = !eng.transport.loop_enabled;
                    }

                    ui.separator();

                    // BPM
                    ui.colored_label(yellow, egui::RichText::new(format!("{:.1} BPM", snap.transport.bpm)).strong());
                    let mut bpm = snap.transport.bpm;
                    if ui.add(egui::Slider::new(&mut bpm, 20.0..=300.0).show_value(false)).changed() {
                        self.engine.lock().handle_event(EngineEvent::Bpm(bpm)).ok();
                    }

                    ui.separator();

                    // BBT Counter
                    ui.colored_label(egui::Color32::WHITE,
                        egui::RichText::new(format!("{:03}:{:02}:{:03}", snap.bbt.bars, snap.bbt.beats, snap.bbt.ticks)).monospace().strong().size(13.0)
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        #[cfg(feature = "audio")]
                        {
                            let (btn_txt, btn_col) = if self.show_settings {
                                ("⚙ Fermer Paramètres", yellow)
                            } else {
                                ("⚙ Paramètres Audio", egui::Color32::WHITE)
                            };
                            if ui.button(egui::RichText::new(btn_txt).color(btn_col).strong()).clicked() {
                                self.show_settings = !self.show_settings;
                                if self.show_settings {
                                    self.refresh_devices();
                                }
                            }

                            ui.separator();

                            let audio_opt = self.audio.as_ref().and_then(|a| {
                                a.lock().as_ref().map(|o| (o.device_name().to_string(), o.sample_rate()))
                            });
                            if let Some((dev, rate)) = audio_opt {
                                ui.colored_label(green, egui::RichText::new(format!("🔊 {dev} ({rate}Hz)")).size(12.0));
                            } else {
                                ui.colored_label(red, egui::RichText::new("🔇 Pas d'audio").size(12.0));
                            }

                            ui.separator();
                        }

                        ui.colored_label(dim, format!("Master: {:.0}%", snap.master_volume * 100.0));
                        ui.separator();
                        ui.colored_label(dim, format!("Voices: {}", snap.num_voices));
                    });
                });
                ui.add_space(6.0);
            });

            // ======== LEFT PANEL — Groups & Scenes ========
            egui::SidePanel::left("groups_panel").default_width(170.0).min_width(140.0).show(ctx, |ui| {
                ui.add_space(6.0);
                ui.colored_label(cyan, egui::RichText::new("GROUPS").size(12.0).strong());
                ui.add_space(4.0);
                for (i, g) in snap.project.groups.iter().enumerate() {
                    let focused = i as u8 == snap.group_focus;
                    let group_char = (b'A' + i as u8) as char;
                    let label = format!("{}: {} ({} pat)", group_char, g.name, g.patterns.len());
                    let color = if focused { cyan } else { egui::Color32::from_rgb(180, 185, 200) };
                    let bg = if focused { egui::Color32::from_rgb(30, 50, 80) } else { egui::Color32::TRANSPARENT };
                    if ui.add(egui::Button::new(egui::RichText::new(&label).color(color).size(12.0)).fill(bg).min_size(egui::vec2(ui.available_width(), 26.0))).clicked() {
                        self.engine.lock().handle_event(EngineEvent::GroupFocus(i as u8)).ok();
                    }
                }
                ui.add_space(10.0);
                ui.colored_label(yellow, egui::RichText::new("SCENES").size(12.0).strong());
                ui.add_space(4.0);
                for (i, s) in snap.project.scenes.iter().enumerate() {
                    let active = Some(i) == snap.active_scene;
                    let color = if active { yellow } else { egui::Color32::from_rgb(180, 185, 200) };
                    let bg = if active { egui::Color32::from_rgb(60, 50, 20) } else { egui::Color32::TRANSPARENT };
                    if ui.add(egui::Button::new(egui::RichText::new(&s.name).color(color).size(12.0)).fill(bg).min_size(egui::vec2(ui.available_width(), 24.0))).clicked() {
                        self.engine.lock().handle_event(EngineEvent::Scene(i)).ok();
                    }
                }
                ui.add_space(10.0);
                ui.separator();
                ui.colored_label(dim, egui::RichText::new(format!("Projet: {}", snap.project.name)).size(11.0));
            });

            // ======== CENTRAL PANEL — Step Sequencer & Pad Matrix ========
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(4.0);

                    // ── 1. SÉQUENCEUR PAS-À-PAS 16 PAS INTERACTIF ────────────────────────
                    ui.horizontal(|ui| {
                        ui.colored_label(cyan, egui::RichText::new("🎹 SÉQUENCEUR PAS-À-PAS (STEP SEQUENCER)").size(14.0).strong());
                        ui.add_space(15.0);

                        // Sélecteur de Mesure (Bar 1 ou Bar 2)
                        let bar1_txt = if is_playing && current_bar == 0 { "► Mesure 1 (1..16)" } else { "Mesure 1 (1..16)" };
                        let bar2_txt = if is_playing && current_bar == 1 { "► Mesure 2 (17..32)" } else { "Mesure 2 (17..32)" };

                        ui.selectable_value(&mut self.selected_bar, 0, egui::RichText::new(bar1_txt).strong());
                        ui.selectable_value(&mut self.selected_bar, 1, egui::RichText::new(bar2_txt).strong());

                        ui.separator();

                        // Presets et Actions rapides
                        if ui.button("🥁 4-on-Floor").on_hover_text("Met des Kicks sur tous les temps (1, 5, 9, 13)").clicked() {
                            self.fill_four_on_the_floor(0, self.selected_bar);
                        }
                        if ui.button("🎩 8ths Hats").on_hover_text("Remplit les Charlestons fermés en croches").clicked() {
                            self.fill_eighths(2, self.selected_bar);
                        }
                        if ui.button("✨ Reset 808").on_hover_text("Restaure le groove démo 808 d'origine").clicked() {
                            self.reset_demo_groove();
                        }
                        if ui.button("🗑 Vider Mesure").on_hover_text("Supprime toutes les notes de la mesure active").clicked() {
                            self.clear_bar(self.selected_bar);
                        }
                    });

                    ui.add_space(6.0);

                    // Récupère les events du pattern actif du groupe courant
                    let active_events: Vec<maschine_engine::project::PatternEvent> = snap
                        .project
                        .groups
                        .get(snap.group_focus as usize)
                        .and_then(|g| g.patterns.get(0))
                        .map(|p| p.events.clone())
                        .unwrap_or_default();

                    // Timeline Step Header (1..16 avec découpage par temps)
                    ui.horizontal(|ui| {
                        ui.add_space(114.0); // Alignement avec le nom de l'instrument
                        for step in 0..16 {
                            let is_current = is_playing && (current_bar == self.selected_bar) && (current_step == step);
                            let col = if is_current {
                                cyan
                            } else if step % 4 == 0 {
                                egui::Color32::WHITE
                            } else {
                                dim
                            };
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 18.0), egui::Sense::hover());
                            let painter = ui.painter_at(rect);
                            if is_current {
                                painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(0, 70, 90));
                            }
                            painter.text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                format!("{}", step + 1),
                                egui::FontId::proportional(11.0),
                                col,
                            );
                            if step % 4 == 3 && step < 15 {
                                ui.add_space(4.0); // Séparateur de temps
                            }
                        }
                    });

                    // 8 Pistes principales de percussion dans le séquenceur
                    const SEQUENCER_PADS: [u8; 8] = [0, 1, 2, 3, 7, 4, 10, 15];

                    for &pad_idx in &SEQUENCER_PADS {
                        let pad_name = SOUND_NAMES[pad_idx as usize];
                        let pad_color = PAD_COLORS[pad_idx as usize];

                        ui.horizontal(|ui| {
                            // Bouton d'écoute / nom d'instrument
                            let btn_txt = format!("▶ {}", pad_name);
                            if ui.add(
                                egui::Button::new(egui::RichText::new(&btn_txt).color(pad_color).size(12.0).strong())
                                    .fill(egui::Color32::from_rgb(25, 28, 36))
                                    .min_size(egui::vec2(110.0, 26.0)),
                            ).clicked() {
                                let mut eng = self.engine.lock();
                                let _ = eng.handle_event(EngineEvent::Pad { pad: pad_idx, velocity: 115, pressed: true });
                            }

                            // 16 Pas
                            for step in 0..16 {
                                let tick = (self.selected_bar as u32 * 3840) + (step as u32 * 240);
                                let is_active = active_events.iter().any(|e| e.pad == pad_idx && e.tick == tick);
                                let is_current = is_playing && (current_bar == self.selected_bar) && (current_step == step);

                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(28.0, 26.0), egui::Sense::click());
                                let painter = ui.painter_at(rect);

                                let bg = if is_active {
                                    if is_current {
                                        egui::Color32::from_rgb(
                                            (pad_color.r() as u16 * 140 / 100).min(255) as u8,
                                            (pad_color.g() as u16 * 140 / 100).min(255) as u8,
                                            (pad_color.b() as u16 * 140 / 100).min(255) as u8,
                                        )
                                    } else {
                                        pad_color
                                    }
                                } else if is_current {
                                    egui::Color32::from_rgb(45, 65, 80)
                                } else if resp.hovered() {
                                    egui::Color32::from_rgb(40, 44, 56)
                                } else if (step / 4) % 2 == 1 {
                                    egui::Color32::from_rgb(26, 28, 36)
                                } else {
                                    egui::Color32::from_rgb(20, 22, 28)
                                };

                                painter.rect_filled(rect, 4.0, bg);

                                let stroke_color = if is_current {
                                    egui::Color32::WHITE
                                } else if is_active {
                                    egui::Color32::from_rgb(220, 220, 240)
                                } else {
                                    egui::Color32::from_rgb(45, 50, 65)
                                };
                                painter.rect_stroke(rect, 4.0, egui::Stroke::new(if is_current { 1.8_f32 } else { 1.0_f32 }, stroke_color), egui::StrokeKind::Outside);

                                if is_active {
                                    painter.circle_filled(rect.center(), 3.5, egui::Color32::BLACK);
                                }

                                if resp.clicked() {
                                    self.toggle_step(pad_idx, tick);
                                }

                                if step % 4 == 3 && step < 15 {
                                    ui.add_space(4.0);
                                }
                            }
                        });
                    }

                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // ── 2. GRILLE DE PADS 4x4 (JOUER EN DIRECT) ─────────────────────────
                    ui.horizontal(|ui| {
                        ui.colored_label(yellow, egui::RichText::new("🎛 GRILLE DE PADS 4x4 (JEU EN DIRECT & APERÇU)").size(14.0).strong());
                        ui.colored_label(dim, "— Cliquez sur un pad pour jouer le sample 808 pleine dynamique");
                    });
                    ui.add_space(6.0);

                    let pad_layout: [[u8; 4]; 4] = [
                        [12, 13, 14, 15],
                        [8,  9,  10, 11],
                        [4,  5,  6,  7],
                        [0,  1,  2,  3],
                    ];

                    for row in &pad_layout {
                        ui.horizontal(|ui| {
                            for &pad_idx in row {
                                let color = PAD_COLORS[pad_idx as usize];
                                let name = SOUND_NAMES[pad_idx as usize];

                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(142.0, 48.0), egui::Sense::click());
                                let painter = ui.painter_at(rect);

                                let bg = if resp.is_pointer_button_down_on() {
                                    color
                                } else if resp.hovered() {
                                    egui::Color32::from_rgb(color.r() / 2, color.g() / 2, color.b() / 2)
                                } else {
                                    egui::Color32::from_rgb(color.r() / 4, color.g() / 4, color.b() / 4)
                                };

                                painter.rect_filled(rect, 6.0, bg);
                                painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.8_f32, color), egui::StrokeKind::Outside);

                                // Numéro de Pad
                                painter.text(
                                    egui::pos2(rect.left() + 8.0, rect.top() + 8.0),
                                    egui::Align2::LEFT_TOP,
                                    format!("{}", pad_idx + 1),
                                    egui::FontId::proportional(12.0),
                                    color,
                                );

                                // Nom de l'instrument
                                painter.text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    name,
                                    egui::FontId::proportional(13.0),
                                    egui::Color32::WHITE,
                                );

                                if resp.clicked() {
                                    let mut eng = self.engine.lock();
                                    let _ = eng.handle_event(EngineEvent::Pad {
                                        pad: pad_idx,
                                        velocity: 115,
                                        pressed: true,
                                    });
                                }
                            }
                        });
                        ui.add_space(2.0);
                    }

                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // ── 3. LISTE DES PATTERNS ───────────────────────────────────────────
                    ui.colored_label(yellow, egui::RichText::new(format!("PATTERNS DU GROUPE {}", (b'A' + snap.group_focus) as char)).size(13.0).strong());
                    ui.add_space(4.0);

                    if let Some(group) = snap.project.groups.get(snap.group_focus as usize) {
                        for (pi, pat) in group.patterns.iter().enumerate() {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.colored_label(cyan, egui::RichText::new(format!("[P{}]", pi + 1)).strong());
                                    ui.colored_label(egui::Color32::WHITE, egui::RichText::new(&pat.name).strong().size(13.0));
                                    ui.colored_label(dim, format!("{} notes programmées  |  Longueur : {} mesures ({} ticks)", pat.events.len(), pat.length.bars, pat.length));
                                });
                            });
                        }
                    } else {
                        ui.colored_label(dim, "Aucun pattern dans le groupe actif");
                    }
                });
            });

            // ======== MODAL WINDOW: SETTINGS AUDIO ========
            #[cfg(feature = "audio")]
            if self.show_settings {
                let mut settings_open = self.show_settings;
                let mut switch_target: Option<String> = None;

                egui::Window::new("⚙ Paramètres Audio — Choix de la Sortie Son")
                    .open(&mut settings_open)
                    .default_width(540.0)
                    .min_width(460.0)
                    .resizable(true)
                    .collapsible(false)
                    .show(ctx, |ui| {
                        ui.add_space(4.0);

                        let active_info = self.audio.as_ref().and_then(|a| {
                            a.lock().as_ref().map(|o| (o.device_name().to_string(), o.sample_rate()))
                        });

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                if let Some((ref dev, rate)) = active_info {
                                    ui.colored_label(green, egui::RichText::new("●").size(18.0));
                                    ui.vertical(|ui| {
                                        ui.colored_label(
                                            egui::Color32::WHITE,
                                            egui::RichText::new(format!("Sortie active : {dev}")).strong().size(13.0),
                                        );
                                        ui.colored_label(dim, format!("Fréquence : {rate} Hz  |  Stéréo  |  Buffer actif"));
                                    });
                                } else {
                                    ui.colored_label(red, egui::RichText::new("●").size(18.0));
                                    ui.colored_label(red, egui::RichText::new("Aucun flux audio actif (choisissez une sortie ci-dessous)").strong());
                                }
                            });
                        });

                        ui.add_space(10.0);
                        ui.colored_label(cyan, egui::RichText::new("SÉLECTION RAPIDE DE LA SORTIE").strong().size(12.0));
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            let is_pc_active = active_info
                                .as_ref()
                                .map(|(n, _)| n == "default" || n == "pipewire" || n.contains("PCH") || n.contains("analog"))
                                .unwrap_or(false);
                            let pc_bg = if is_pc_active {
                                egui::Color32::from_rgb(25, 70, 45)
                            } else {
                                egui::Color32::from_rgb(35, 40, 55)
                            };

                            if ui.add(
                                egui::Button::new(
                                    egui::RichText::new("🔊 Haut-parleurs PC (Défaut Système)")
                                        .color(if is_pc_active { green } else { egui::Color32::WHITE })
                                        .size(13.0)
                                        .strong(),
                                )
                                .fill(pc_bg)
                                .min_size(egui::vec2(250.0, 38.0)),
                            ).clicked() {
                                switch_target = Some("default".to_string());
                            }

                            let is_mk3_active = active_info
                                .as_ref()
                                .map(|(n, _)| n.to_lowercase().contains("mk3") || n.to_lowercase().contains("maschine"))
                                .unwrap_or(false);
                            let mk3_bg = if is_mk3_active {
                                egui::Color32::from_rgb(25, 70, 45)
                            } else {
                                egui::Color32::from_rgb(35, 40, 55)
                            };

                            if ui.add(
                                egui::Button::new(
                                    egui::RichText::new("🎛 Maschine MK3 (Prises Hardware)")
                                        .color(if is_mk3_active { green } else { egui::Color32::WHITE })
                                        .size(13.0)
                                        .strong(),
                                )
                                .fill(mk3_bg)
                                .min_size(egui::vec2(250.0, 38.0)),
                            ).clicked() {
                                switch_target = Some("mk3".to_string());
                            }
                        });

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            ui.colored_label(yellow, egui::RichText::new("TOUS LES PÉRIPHÉRIQUES DÉTECTÉS").strong().size(12.0));
                            if ui.button("⟲ Actualiser la liste").clicked() {
                                self.refresh_devices();
                            }
                        });
                        ui.add_space(4.0);

                        egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                            for dev in &self.available_devices {
                                let is_active = active_info.as_ref().map(|(n, _)| n == &dev.name).unwrap_or(false);
                                let is_selected = self.selected_device == dev.name;

                                let mut title = dev.name.clone();
                                if dev.is_default {
                                    title.push_str("  [Défaut Système]");
                                }
                                if dev.is_mk3 {
                                    title.push_str("  [Maschine MK3]");
                                }

                                let text_col = if is_active {
                                    green
                                } else if is_selected {
                                    cyan
                                } else {
                                    egui::Color32::from_rgb(200, 205, 220)
                                };

                                if ui.selectable_label(is_selected, egui::RichText::new(&title).color(text_col)).clicked() {
                                    self.selected_device = dev.name.clone();
                                }
                            }
                        });

                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button(egui::RichText::new("✔ Activer le périphérique sélectionné").strong()).clicked() {
                                switch_target = Some(self.selected_device.clone());
                            }
                            ui.colored_label(dim, format!("Sélectionné : {}", self.selected_device));
                        });

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(6.0);

                        ui.colored_label(cyan, egui::RichText::new("TEST SONORE IMMÉDIAT & VOLUMES").strong().size(12.0));
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            if ui.button(egui::RichText::new("▶ Tester le Kick 808 (Pad 1)").color(green).strong().size(13.0)).clicked() {
                                let mut eng = self.engine.lock();
                                let _ = eng.handle_event(EngineEvent::Pad { pad: 0, velocity: 115, pressed: true });
                            }
                            if ui.button(egui::RichText::new("▶ Tester la Snare 808 (Pad 2)").strong().size(13.0)).clicked() {
                                let mut eng = self.engine.lock();
                                let _ = eng.handle_event(EngineEvent::Pad { pad: 1, velocity: 115, pressed: true });
                            }
                            if ui.button(egui::RichText::new("▶ Tester le Hi-Hat (Pad 3)").strong().size(13.0)).clicked() {
                                let mut eng = self.engine.lock();
                                let _ = eng.handle_event(EngineEvent::Pad { pad: 2, velocity: 110, pressed: true });
                            }
                            if ui.button(egui::RichText::new("▶ Tester le Clap (Pad 8)").strong().size(13.0)).clicked() {
                                let mut eng = self.engine.lock();
                                let _ = eng.handle_event(EngineEvent::Pad { pad: 7, velocity: 110, pressed: true });
                            }
                        });

                        ui.add_space(6.0);
                        let mut master_vol = snap.master_volume;
                        let master_text = format!("{:.0}%", master_vol * 100.0);
                        ui.horizontal(|ui| {
                            ui.label("Volume Master :");
                            if ui.add(egui::Slider::new(&mut master_vol, 0.0..=1.0).text(&master_text)).changed() {
                                self.engine.lock().handle_event(EngineEvent::MasterVolume(master_vol)).ok();
                            }
                        });

                        let mut hp_vol = snap.headphone_volume;
                        let hp_text = format!("{:.0}%", hp_vol * 100.0);
                        ui.horizontal(|ui| {
                            ui.label("Volume Casque :");
                            if ui.add(egui::Slider::new(&mut hp_vol, 0.0..=1.0).text(&hp_text)).changed() {
                                self.engine.lock().handle_event(EngineEvent::HeadphoneVolume(hp_vol)).ok();
                            }
                        });

                        if let Some((ref msg, is_err)) = self.status_message {
                            ui.add_space(8.0);
                            let col = if is_err { red } else { green };
                            ui.colored_label(col, egui::RichText::new(msg).strong());
                        }
                    });

                self.show_settings = settings_open;
                if let Some(target) = switch_target {
                    self.switch_audio_device(&target);
                }
            }

            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }
}

// ctrlc est initialisé en début de main via init_ctrlc()

