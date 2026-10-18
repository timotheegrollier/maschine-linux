//! `maschine-linux` — binaire principal.
//!
//! CLI : `probe`, `hid-dump`, `daemon`, `project`...

use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
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
use maschine_hardware::mapper::{HardwareMapper, MapperAction, MapperConfig, SequencerLedInfo};

#[cfg(feature = "audio")]
mod audio;
#[cfg(feature = "usb")]
mod display_thread;
#[cfg(feature = "gui")]
mod gui;
mod hw_output;
mod settings;

/// Statistiques latence HID (mode `--debug-latency`) : événement brut →
/// action appliquée dans l'Engine.
#[derive(Default)]
struct LatencyStats {
    /// Événements traités (par intervalle de log).
    hid_events: AtomicU64,
    /// Durée du dernier événement (µs).
    hid_last_us: AtomicU64,
    /// Durée max de l'intervalle (µs).
    hid_max_us: AtomicU64,
}

/// Boucle de log des stats latence (toutes les 2 s).
fn spawn_latency_logger(
    stats: Arc<LatencyStats>,
    #[cfg(feature = "audio")] audio_stats: Option<Arc<audio::AudioStats>>,
) {
    std::thread::Builder::new()
        .name("latency-logger".to_string())
        .spawn(move || loop {
            std::thread::sleep(Duration::from_secs(2));
            let ev = stats.hid_events.swap(0, Ordering::Relaxed);
            let last = stats.hid_last_us.load(Ordering::Relaxed);
            let max = stats.hid_max_us.swap(0, Ordering::Relaxed);
            #[cfg(feature = "audio")]
            let audio_txt = match &audio_stats {
                Some(a) => {
                    let frames = a.buffer_frames();
                    let ms = if frames > 0 { frames as f64 * 1000.0 / 48000.0 } else { 0.0 };
                    format!(
                        "audio: {} cb (buffer {} frames ≈ {:.1} ms), {} miss, {} err, max {} µs",
                        a.callbacks.swap(0, Ordering::Relaxed),
                        frames,
                        ms,
                        a.lock_misses.swap(0, Ordering::Relaxed),
                        a.errors.swap(0, Ordering::Relaxed),
                        a.max_callback_us.swap(0, Ordering::Relaxed),
                    )
                }
                None => "audio: —".to_string(),
            };
            #[cfg(not(feature = "audio"))]
            let audio_txt = "audio: —";
            info!(
                "⏱ LATENCE — HID→Engine: {ev} évts/2s, dernier {last} µs, max {max} µs • {audio_txt}"
            );
        })
        .ok();
}

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
        /// Instrumentation latence : log HID→note_on et stats callback audio
        #[arg(long)]
        debug_latency: bool,
        /// Diagnostic audio : démarre la lecture et simule une charge UI/HID
        /// (verrou moteur + snapshots en continu) pour exposer les manques du
        /// callback audio (craquements). À combiner avec --debug-latency.
        #[arg(long)]
        stress: bool,
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
    /// Allume les pads 1..16 un par un dans l'ordre logique pour vérifier le mapping
    PadTest {
        /// Pause entre chaque pad (ms)
        #[arg(long, default_value = "350")]
        step_ms: u64,
    },
    /// Calibration interactive : appuyez chaque pad 1..16 pour révéler son index brut
    PadCalibrate,
    /// Moniteur live des pads : une ligne par appui/release traité (pipeline
    /// complet), avec numéros physique + logique — pour débugger en manipulant
    /// le hardware. Ajoutez `-v` pour voir aussi les suppressions
    /// (diaphonie, rebond, continuation de rafale, capteur bloqué).
    PadMonitor,
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
        /// Instrumentation latence : stats audio (buffer réel, misses, erreurs)
        #[arg(long)]
        debug_latency: bool,
        /// Diagnostic audio : démarre la lecture + contention verrou moteur
        #[arg(long)]
        stress: bool,
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

/// Met le mapper matériel au même contexte que le moteur avant un nouvel input.
/// En GUI, les onglets peuvent changer sans passer par le mapper ; sans cette
/// synchro pré-événement, le premier cran de knob part vers l'ancienne vue.
fn sync_mapper_context(
    mapper: &mut HardwareMapper,
    snap: &maschine_engine::engine::HardwareSnapshot,
) {
    mapper.group_focus = snap.group_focus;
    mapper.active_sound = snap.active_sound;
    mapper.left_tab = snap.left_tab;
    mapper.right_tab = snap.right_tab;
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
            debug_latency,
            stress,
        } => cmd_daemon(
            project,
            midi_port,
            audio_device,
            no_audio,
            no_led,
            headphone_mode,
            false,
            debug_latency,
            stress,
        )?,
        Commands::Project { cmd } => cmd_project(cmd)?,
        Commands::LedTest { pad, color } => cmd_led_test(pad, color)?,
        Commands::LedCycle { duration } => cmd_led_cycle(duration)?,
        Commands::PadTest { step_ms } => cmd_pad_test(step_ms)?,
        Commands::PadCalibrate => cmd_pad_calibrate()?,
        Commands::PadMonitor => cmd_pad_monitor()?,
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
            debug_latency,
            stress,
        } => cmd_daemon(
            project,
            "Maschine MK3 Virtual".to_string(),
            audio_device,
            no_audio,
            false,
            "duplicate".to_string(),
            true,
            debug_latency,
            stress,
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

#[allow(clippy::too_many_arguments)] // handler CLI : 1 paramètre par option
fn cmd_daemon(
    project: Option<PathBuf>,
    _midi_port: String,
    audio_device: Option<String>,
    no_audio: bool,
    no_led: bool,
    headphone_mode: String,
    with_gui: bool,
    debug_latency: bool,
    stress: bool,
) -> anyhow::Result<()> {
    println!("# maschine-linux daemon — Ctrl+C pour quitter");
    let project_path = project.clone();
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
        engine.lock().load_default_808_kit();
    }
    if let Some(path) = project_path.as_deref() {
        engine.lock().restore_project_samples(path)?;
    }

    // Audio — cpal
    let settings = settings::AppSettings::load();
    let initial_audio_dev = audio_device.or(settings.audio_device);
    let hp_dup = headphone_mode == "duplicate";

    #[cfg(feature = "audio")]
    // cpal::Stream n'est pas Send/Sync sur ALSA : l'AudioOutput vit dans ce
    // mutex partagé avec le thread GUI (même thread d'accès en pratique).
    #[allow(clippy::arc_with_non_send_sync)]
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
                log::error!(
                    "Audio démarrage échec: {e} — continue sans son (modifiable via paramètres)"
                );
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

    // Diagnostic : lecture + charge UI/HID simulée (verrou moteur et snapshots
    // pris en continu) pour vérifier que le callback audio ne manque jamais son
    // verrou, même sous contention.
    if stress {
        {
            let mut eng = engine.lock();
            let _ = eng.handle_event(EngineEvent::Transport {
                play: true,
                rec: false,
                stop: false,
            });
        }
        info!("⚠ Mode stress : lecture + contention verrou moteur (diagnostic)");
        let eng_stress = engine.clone();
        std::thread::Builder::new()
            .name("stress-lock".to_string())
            .spawn(move || {
                let mut i = 0u64;
                loop {
                    {
                        let mut eng = eng_stress.lock();
                        match i % 6 {
                            0 => {
                                let pad = (i % 16) as u8;
                                let _ = eng.handle_event(EngineEvent::Pad {
                                    pad,
                                    velocity: 110,
                                    pressed: true,
                                });
                            }
                            1 => {
                                let pad = (i % 16) as u8;
                                let _ = eng.handle_event(EngineEvent::Pad {
                                    pad,
                                    velocity: 0,
                                    pressed: false,
                                });
                            }
                            2 => {
                                let _ = eng.handle_event(EngineEvent::SelectSound((i % 16) as u8));
                            }
                            3 => {
                                let _ = eng.handle_event(EngineEvent::GroupFocus((i % 8) as u8));
                            }
                            _ => {}
                        }
                        // Comme le thread écrans : copie complète de l'état.
                        let _ = eng.snapshot();
                    }
                    i += 1;
                    std::thread::sleep(Duration::from_millis(1));
                }
            })
            .ok();
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
                let palette = [
                    6u8, 22, 30, 42, 50, 58, 10, 14, 26, 31, 34, 38, 11, 27, 33, 78,
                ];
                for (i, &col) in palette.iter().enumerate().take(16) {
                    init.set_pad(
                        i,
                        maschine_proto::color::Color::from_index(col)
                            .unwrap_or(maschine_proto::color::Color::OFF),
                    );
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

    // Drapeau d'activité utilisateur pour le thread d'écrans : posé sans verrou
    // par la boucle HID, consommé par le thread d'affichage (réactivité ~3 ms).
    let display_activity = Arc::new(std::sync::atomic::AtomicBool::new(true));

    // Display bulk (optionnel, feature usb)
    #[cfg(feature = "usb")]
    let mut display_handle =
        match display_thread::spawn_display_thread(engine.clone(), display_activity.clone()) {
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
                                let _ =
                                    eng.handle_event(EngineEvent::Transport { play, rec, stop });
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

    /// Vitesses des knobs d'écran, par tick brut (1024 ticks = 1 tour de knob,
    /// soit ~32 ticks par cran mécanique). Calibré pour qu'un cran ait le grain
    /// du logiciel officiel (~1 dB, ~3%, ~1 demi-ton de cutoff, ~1.5 BPM).
    mod knob_rate {
        pub const BPM: f32 = 0.05;
        pub const SWING: f32 = 0.001;
        pub const VOL_DB: f32 = 0.03;
        pub const PAN: f32 = 0.001;
        /// Octaves de cutoff par tick : ~2.7 octaves par tour, ~1 demi-ton par cran.
        pub const CUTOFF_OCT: f32 = 1.0 / 384.0;
        pub const RESO: f32 = 0.001;
        pub const MASTER: f32 = 0.001;
        pub const ATTACK_MS: f32 = 0.5;
        pub const DECAY_MS: f32 = 1.0;
        pub const SUSTAIN: f32 = 0.001;
        pub const RELEASE_MS: f32 = 1.0;
    }

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
                MapperAction::PadTrigger {
                    pad,
                    velocity,
                    pressed,
                } => {
                    log::debug!("PAD trigger pad={pad} vel={velocity} pressed={pressed}");
                    if pressed {
                        // La sélection suit le jeu (comme Maschine) : l'écran SAMPLER
                        // et l'édition GUI ciblent toujours le dernier pad touché.
                        mapper.active_sound = pad;
                        let mut eng = engine.lock();
                        let _ = eng.handle_event(EngineEvent::SelectSound(pad));
                        let _ = eng.handle_event(EngineEvent::Pad {
                            pad,
                            velocity,
                            pressed,
                        });
                    } else {
                        let _ = engine.lock().handle_event(EngineEvent::Pad {
                            pad,
                            velocity,
                            pressed,
                        });
                    }
                }
                MapperAction::ToggleStep { sound, bar, step } => {
                    log::debug!("STEP toggle sound={sound} bar={bar} step={step}");
                    let tick = (bar as u32 * 4 * 960) + (step as u32 * 240);
                    engine.lock().toggle_step(sound, tick);
                }
                MapperAction::StepVelocityDelta(delta) => {
                    let _ = engine
                        .lock()
                        .handle_event(EngineEvent::StepVelocityDelta(delta));
                }
                MapperAction::StepNudgeTicks(delta) => {
                    let _ = engine
                        .lock()
                        .handle_event(EngineEvent::StepNudgeTicks(delta));
                }
                MapperAction::StepLengthDelta(delta) => {
                    let _ = engine
                        .lock()
                        .handle_event(EngineEvent::StepLengthDelta(delta));
                }
                MapperAction::StepPitchDelta(delta) => {
                    let _ = engine
                        .lock()
                        .handle_event(EngineEvent::StepPitchDelta(delta));
                }
                MapperAction::StepScroll(delta) => {
                    let _ = engine.lock().handle_event(EngineEvent::StepScroll(delta));
                }
                MapperAction::PlayPauseToggle => {
                    let mut eng = engine.lock();
                    let is_playing = eng.transport.is_playing();
                    if is_playing {
                        let _ = eng.handle_event(EngineEvent::Transport {
                            play: false,
                            rec: false,
                            stop: true,
                        });
                    } else {
                        let _ = eng.handle_event(EngineEvent::Transport {
                            play: true,
                            rec: false,
                            stop: false,
                        });
                    }
                }
                MapperAction::Stop => {
                    let _ = engine.lock().handle_event(EngineEvent::Transport {
                        play: false,
                        rec: false,
                        stop: true,
                    });
                }
                MapperAction::RecordToggle => {
                    let mut eng = engine.lock();
                    let is_rec = eng.transport.state
                        == maschine_engine::transport::TransportState::Recording;
                    let _ = eng.handle_event(EngineEvent::Transport {
                        play: true,
                        rec: !is_rec,
                        stop: false,
                    });
                }
                MapperAction::Restart => {
                    // Passe par l'Engine : remise à zéro du transport + invalidation
                    // des écrans (le BBT affiché suit immédiatement).
                    let _ = engine.lock().handle_event(EngineEvent::Restart);
                }
                MapperAction::BpmDelta(delta) => {
                    let mut eng = engine.lock();
                    let new_bpm = (eng.transport.bpm + delta * knob_rate::BPM).clamp(20.0, 999.0);
                    let _ = eng.handle_event(EngineEvent::Bpm(new_bpm));
                }
                MapperAction::BpmSet(bpm) => {
                    let _ = engine.lock().handle_event(EngineEvent::Bpm(bpm));
                }
                MapperAction::SwingDelta(delta) => {
                    let mut eng = engine.lock();
                    let current = eng.project_handle().read().swing;
                    let new_sw = (current + delta * knob_rate::SWING).clamp(0.0, 1.0);
                    let _ = eng.handle_event(EngineEvent::Swing(new_sw));
                }
                MapperAction::MasterVolumeDelta(delta) => {
                    let mut eng = engine.lock();
                    let current = eng.snapshot().master_volume;
                    let new_vol = (current + delta * knob_rate::MASTER).clamp(0.0, 1.0);
                    let _ = eng.handle_event(EngineEvent::MasterVolume(new_vol));
                }
                MapperAction::GroupVolumeDelta(delta) => {
                    let gf = engine.lock().group_focus();
                    let proj = engine.lock().project_handle();
                    let current = proj
                        .read()
                        .groups
                        .get(gf as usize)
                        .map(|g| g.volume_db)
                        .unwrap_or(0.0);
                    let new_db = (current + delta * knob_rate::VOL_DB).clamp(-60.0, 12.0);
                    let _ = engine.lock().handle_event(EngineEvent::GroupVolume {
                        group: gf,
                        volume_db: new_db,
                    });
                }
                MapperAction::GroupPanDelta(delta) => {
                    let gf = engine.lock().group_focus();
                    let proj = engine.lock().project_handle();
                    let current = proj
                        .read()
                        .groups
                        .get(gf as usize)
                        .map(|g| g.pan)
                        .unwrap_or(0.0);
                    let new_pan = (current + delta * knob_rate::PAN).clamp(-1.0, 1.0);
                    let _ = engine.lock().handle_event(EngineEvent::GroupPan {
                        group: gf,
                        pan: new_pan,
                    });
                }
                MapperAction::SoundVolumeDelta(delta) => {
                    let gf = engine.lock().group_focus();
                    let snd = mapper.active_sound;
                    let proj = engine.lock().project_handle();
                    let current = proj
                        .read()
                        .groups
                        .get(gf as usize)
                        .and_then(|g| g.sounds.get(snd as usize).and_then(|s| s.as_ref()))
                        .map(|s| s.volume_db)
                        .unwrap_or(0.0);
                    let new_db = (current + delta * knob_rate::VOL_DB).clamp(-60.0, 12.0);
                    let _ = engine.lock().handle_event(EngineEvent::SoundVolume {
                        group: gf,
                        sound: snd,
                        volume_db: new_db,
                    });
                }
                MapperAction::SoundPanDelta(delta) => {
                    let gf = engine.lock().group_focus();
                    let snd = mapper.active_sound;
                    let proj = engine.lock().project_handle();
                    let current = proj
                        .read()
                        .groups
                        .get(gf as usize)
                        .and_then(|g| g.sounds.get(snd as usize).and_then(|s| s.as_ref()))
                        .map(|s| s.pan)
                        .unwrap_or(0.0);
                    let new_pan = (current + delta * knob_rate::PAN).clamp(-1.0, 1.0);
                    let _ = engine.lock().handle_event(EngineEvent::SoundPan {
                        group: gf,
                        sound: snd,
                        pan: new_pan,
                    });
                }
                MapperAction::SoundFilterCutoffDelta(delta) => {
                    let gf = engine.lock().group_focus();
                    let snd = mapper.active_sound;
                    let proj = engine.lock().project_handle();
                    let current = proj
                        .read()
                        .groups
                        .get(gf as usize)
                        .and_then(|g| g.sounds.get(snd as usize).and_then(|s| s.as_ref()))
                        .map(|s| s.filter.cutoff_hz)
                        .unwrap_or(20000.0);
                    // Échelle exponentielle musicale : ~2.7 octaves par tour de knob,
                    // soit ~6% par cran de 32 ticks (20 Hz -> 20 kHz en ~3.5 tours).
                    let base = if current <= 20.0 { 20000.0 } else { current };
                    let factor = 2.0f32.powf(delta * knob_rate::CUTOFF_OCT);
                    let new_cutoff = (base * factor).clamp(20.0, 20000.0);
                    let _ = engine.lock().handle_event(EngineEvent::SoundFilterCutoff {
                        group: gf,
                        sound: snd,
                        cutoff_hz: new_cutoff,
                    });
                }
                MapperAction::SoundFilterResoDelta(delta) => {
                    let gf = engine.lock().group_focus();
                    let snd = mapper.active_sound;
                    let proj = engine.lock().project_handle();
                    let current = proj
                        .read()
                        .groups
                        .get(gf as usize)
                        .and_then(|g| g.sounds.get(snd as usize).and_then(|s| s.as_ref()))
                        .map(|s| s.filter.resonance)
                        .unwrap_or(0.0);
                    let new_reso = (current + delta * knob_rate::RESO).clamp(0.0, 1.0);
                    let _ = engine.lock().handle_event(EngineEvent::SoundFilterReso {
                        group: gf,
                        sound: snd,
                        resonance: new_reso,
                    });
                }
                MapperAction::SoundAttackDelta(delta) => {
                    let gf = engine.lock().group_focus();
                    let snd = mapper.active_sound;
                    let proj = engine.lock().project_handle();
                    let current = proj
                        .read()
                        .groups
                        .get(gf as usize)
                        .and_then(|g| g.sounds.get(snd as usize).and_then(|s| s.as_ref()))
                        .map(|s| {
                            (
                                s.adsr.attack_ms,
                                s.adsr.decay_ms,
                                s.adsr.sustain,
                                s.adsr.release_ms,
                            )
                        });
                    if let Some((a, d, su, r)) = current {
                        let _ = engine.lock().handle_event(EngineEvent::SoundAdsr {
                            group: gf,
                            sound: snd,
                            attack_ms: (a + delta * knob_rate::ATTACK_MS).clamp(0.1, 5000.0),
                            decay_ms: d,
                            sustain: su,
                            release_ms: r,
                        });
                    }
                }
                MapperAction::SoundDecayDelta(delta) => {
                    let gf = engine.lock().group_focus();
                    let snd = mapper.active_sound;
                    let proj = engine.lock().project_handle();
                    let current = proj
                        .read()
                        .groups
                        .get(gf as usize)
                        .and_then(|g| g.sounds.get(snd as usize).and_then(|s| s.as_ref()))
                        .map(|s| {
                            (
                                s.adsr.attack_ms,
                                s.adsr.decay_ms,
                                s.adsr.sustain,
                                s.adsr.release_ms,
                            )
                        });
                    if let Some((a, d, su, r)) = current {
                        let _ = engine.lock().handle_event(EngineEvent::SoundAdsr {
                            group: gf,
                            sound: snd,
                            attack_ms: a,
                            decay_ms: (d + delta * knob_rate::DECAY_MS).clamp(0.1, 5000.0),
                            sustain: su,
                            release_ms: r,
                        });
                    }
                }
                MapperAction::SoundSustainDelta(delta) => {
                    let gf = engine.lock().group_focus();
                    let snd = mapper.active_sound;
                    let proj = engine.lock().project_handle();
                    let current = proj
                        .read()
                        .groups
                        .get(gf as usize)
                        .and_then(|g| g.sounds.get(snd as usize).and_then(|s| s.as_ref()))
                        .map(|s| {
                            (
                                s.adsr.attack_ms,
                                s.adsr.decay_ms,
                                s.adsr.sustain,
                                s.adsr.release_ms,
                            )
                        });
                    if let Some((a, d, su, r)) = current {
                        let _ = engine.lock().handle_event(EngineEvent::SoundAdsr {
                            group: gf,
                            sound: snd,
                            attack_ms: a,
                            decay_ms: d,
                            sustain: (su + delta * knob_rate::SUSTAIN).clamp(0.0, 1.0),
                            release_ms: r,
                        });
                    }
                }
                MapperAction::SoundReleaseDelta(delta) => {
                    let gf = engine.lock().group_focus();
                    let snd = mapper.active_sound;
                    let proj = engine.lock().project_handle();
                    let current = proj
                        .read()
                        .groups
                        .get(gf as usize)
                        .and_then(|g| g.sounds.get(snd as usize).and_then(|s| s.as_ref()))
                        .map(|s| {
                            (
                                s.adsr.attack_ms,
                                s.adsr.decay_ms,
                                s.adsr.sustain,
                                s.adsr.release_ms,
                            )
                        });
                    if let Some((a, d, su, r)) = current {
                        let _ = engine.lock().handle_event(EngineEvent::SoundAdsr {
                            group: gf,
                            sound: snd,
                            attack_ms: a,
                            decay_ms: d,
                            sustain: su,
                            release_ms: (r + delta * knob_rate::RELEASE_MS).clamp(0.1, 5000.0),
                        });
                    }
                }
                MapperAction::GroupVolumeDeltaFor { group, delta } => {
                    let proj = engine.lock().project_handle();
                    let current = proj
                        .read()
                        .groups
                        .get(group as usize)
                        .map(|g| g.volume_db)
                        .unwrap_or(0.0);
                    let new_db = (current + delta * knob_rate::VOL_DB).clamp(-60.0, 12.0);
                    let _ = engine.lock().handle_event(EngineEvent::GroupVolume {
                        group,
                        volume_db: new_db,
                    });
                }
                MapperAction::BrowseSoundStep(step) => {
                    let cur = mapper.active_sound as i32;
                    let next = cur + step;
                    let next = next.rem_euclid(16) as u8;
                    mapper.active_sound = next;
                    let _ = engine.lock().handle_event(EngineEvent::SelectSound(next));
                }
                MapperAction::BrowsePresetStep(step) => {
                    let _ = engine
                        .lock()
                        .handle_event(EngineEvent::BrowsePresetStep(step));
                }
                MapperAction::LoadSelectedPreset => {
                    let mut eng = engine.lock();
                    let gf = eng.group_focus();
                    let snd = mapper.active_sound;
                    let preset = eng.browser_preset_idx();
                    let _ = eng.handle_event(EngineEvent::LoadDrumPreset {
                        group: gf,
                        sound: snd,
                        preset_idx: preset,
                    });
                    let _ = eng.handle_event(EngineEvent::Pad {
                        pad: snd,
                        velocity: 115,
                        pressed: true,
                    });
                }
                MapperAction::BrowseGroupStep(step) => {
                    let cur = mapper.group_focus as i32;
                    let next = (cur + step).rem_euclid(8) as u8;
                    mapper.group_focus = next;
                    let _ = engine.lock().handle_event(EngineEvent::GroupFocus(next));
                }
                MapperAction::GroupFocus(g) => {
                    let _ = engine.lock().handle_event(EngineEvent::GroupFocus(g));
                }
                MapperAction::SelectBar(b) => {
                    if engine.lock().snapshot().song_mode {
                        if b == 0 {
                            let _ = engine.lock().handle_event(EngineEvent::PrevSection);
                        } else {
                            let _ = engine.lock().handle_event(EngineEvent::NextSection);
                        }
                    } else {
                        mapper.selected_bar = b;
                        let _ = engine.lock().handle_event(EngineEvent::SelectBar(b));
                    }
                }
                MapperAction::SetPatternBars(bars) => {
                    mapper.pattern_bars = bars;
                    let _ = engine
                        .lock()
                        .handle_event(EngineEvent::SetPatternBars(bars));
                }
                MapperAction::SelectSound(s) => {
                    mapper.active_sound = s;
                    let _ = engine.lock().handle_event(EngineEvent::SelectSound(s));
                }
                MapperAction::SceneTrigger(i) => {
                    let n = engine.lock().snapshot().project.scenes.len();
                    if i < n {
                        let _ = engine.lock().handle_event(EngineEvent::Scene(i));
                    }
                }
                MapperAction::PatternTrigger { group, pattern } => {
                    // Ne sélectionne que les patterns existants du groupe.
                    let ok = engine
                        .lock()
                        .snapshot()
                        .project
                        .groups
                        .get(group as usize)
                        .map(|g| pattern < g.patterns.len())
                        .unwrap_or(false);
                    if ok {
                        let _ = engine
                            .lock()
                            .handle_event(EngineEvent::PatternSelect { group, pattern });
                    }
                }
                MapperAction::EraseSound { group, sound } => {
                    let _ = engine
                        .lock()
                        .handle_event(EngineEvent::EraseSound { group, sound });
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
                    let (g, scene, proj) = {
                        let eng = engine.lock();
                        let snap = eng.snapshot();
                        (
                            snap.group_focus as usize,
                            snap.active_scene,
                            eng.project_handle(),
                        )
                    };
                    let mut guard = proj.write();
                    let pat_idx = guard.active_pattern_index(g, scene);
                    if let Some(group) = guard.groups.get_mut(g) {
                        if let Some(pi) = pat_idx {
                            if let Some(pat) = group.patterns.get_mut(pi) {
                                let bar1_events: Vec<_> = pat
                                    .events
                                    .iter()
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
                    drop(guard);
                    engine.lock().invalidate_display();
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
                MapperAction::ToggleSongMode => {
                    let _ = engine.lock().handle_event(EngineEvent::ToggleSongMode);
                }
                MapperAction::Quantize => {
                    engine.lock().quantize_active_pattern();
                }
                MapperAction::SetSceneMode(v) => {
                    let _ = engine.lock().handle_event(EngineEvent::SetSceneMode(v));
                }
                MapperAction::SetPatternMode(v) => {
                    let _ = engine.lock().handle_event(EngineEvent::SetPatternMode(v));
                }
                MapperAction::SetSelectMode(v) => {
                    let _ = engine.lock().handle_event(EngineEvent::SetSelectMode(v));
                }
                MapperAction::SetEraseMode(v) => {
                    let _ = engine.lock().handle_event(EngineEvent::SetEraseMode(v));
                }
                MapperAction::SetControllerMode(v) => {
                    let _ = engine
                        .lock()
                        .handle_event(EngineEvent::SetControllerMode(v));
                }
                MapperAction::ToggleLoop => {
                    let _ = engine.lock().handle_event(EngineEvent::ToggleLoop);
                }
                MapperAction::ToggleMetronome => {
                    let _ = engine.lock().handle_event(EngineEvent::ToggleMetronome);
                }
                MapperAction::SoftKey(n) => {
                    // Action contextuelle décidée par l'Engine : exactement ce
                    // que le label affiché sous l'écran annonce.
                    let _ = engine.lock().handle_event(EngineEvent::SoftKey(n));
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
                    if let Ok(json) = serde_json::to_string_pretty(&*snap.project) {
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
        let snap = engine.lock().hardware_snapshot();
        // Le mapper suit l'engine (la GUI peut aussi changer groupe/son/onglets).
        sync_mapper_context(mapper, &snap);
        // Couleurs des pads = couleurs des Sounds du groupe focus (comme Maschine).
        for (index, color_index) in snap.pad_colors.iter().enumerate() {
            if let Some(color) = color_index.and_then(maschine_proto::color::Color::from_index) {
                mapper.set_pad_color(index, color);
            }
        }
        let led_info = SequencerLedInfo {
            is_playing: snap.is_playing,
            is_recording: snap.is_recording,
            is_stopped: snap.is_stopped,
            current_bar: snap.current_bar,
            current_step: snap.current_step,
            is_quarter_beat: snap.is_quarter_beat,
            active_steps: snap.active_steps,
            step_cursor: snap.step_cursor,
            triggered_pads: snap.triggered_pads,
            master_vol: snap.master_volume,
            muted_groups: snap.muted_groups,
            soloed_groups: snap.soloed_groups,
            left_tab: snap.left_tab,
            right_tab: snap.right_tab,
            scene_mode: mapper.scene_select_mode,
            pattern_mode: mapper.pattern_select_mode,
            select_mode: mapper.select_mode,
            erase_mode: mapper.erase_mode,
            active_sound: snap.active_sound,
            active_scene: snap.active_scene,
            num_scenes: snap.num_scenes,
            group_num_patterns: snap.group_num_patterns,
            scene_pattern: snap.scene_pattern,
            loop_enabled: snap.loop_enabled,
            metronome: snap.metronome,
            selected_bar: snap.selected_bar,
            pattern_bars: snap.pattern_bars,
        };
        mapper.update_leds(&led_info);
        let _ = led_tx.send(mapper.led_state().clone());
    }

    // Latence (--debug-latency) + repaint temps réel (GUI).
    let latency_stats = Arc::new(LatencyStats::default());
    if debug_latency {
        #[cfg(feature = "audio")]
        spawn_latency_logger(
            latency_stats.clone(),
            audio_shared.lock().as_ref().map(|o| Arc::clone(&o.stats)),
        );
        #[cfg(not(feature = "audio"))]
        spawn_latency_logger(latency_stats.clone());
    }
    #[cfg(feature = "gui")]
    let repaint = gui::RepaintSignal::new();

    // ----- GUI mode: run event loop in background, GUI on main thread -----
    #[cfg(feature = "gui")]
    if with_gui {
        // Spawn HID event loop in background thread
        let engine_bg = engine.clone();
        let led_tx_bg = led_tx.clone();
        let latency_bg = latency_stats.clone();
        let display_activity_bg = display_activity.clone();
        #[cfg(feature = "gui")]
        let repaint_bg = repaint.clone();
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
                    if !no_led && last_led_sync.elapsed() >= Duration::from_millis(16) {
                        sync_hardware_leds(&engine_bg, &mut mapper, &led_tx_bg);
                        last_led_sync = std::time::Instant::now();
                    }

                    match rx.recv_timeout(Duration::from_millis(5)) {
                        Ok(ev) => {
                            let t0 = std::time::Instant::now();
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
                                            let _ = engine_bg
                                                .lock()
                                                .handle_event(EngineEvent::MasterVolume(vol));
                                        }
                                        ButtonId::Unknown(99, 1) => {
                                            let vol = *value as f32 / 1023.0;
                                            let _ = engine_bg
                                                .lock()
                                                .handle_event(EngineEvent::HeadphoneVolume(vol));
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }

                            // La GUI peut avoir changé de vue depuis le dernier
                            // événement HID : router ce knob avec le contexte
                            // moteur courant, pas celui mémorisé par le mapper.
                            let context = engine_bg.lock().hardware_snapshot();
                            sync_mapper_context(&mut mapper, &context);
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
                            display_activity_bg.store(true, Ordering::Relaxed);

                            if !no_led {
                                sync_hardware_leds(&engine_bg, &mut mapper, &led_tx_bg);
                                last_led_sync = std::time::Instant::now();
                            }

                            // Latence HID→Engine + réveil immédiat de la GUI.
                            let us = t0.elapsed().as_micros() as u64;
                            latency_bg.hid_events.fetch_add(1, Ordering::Relaxed);
                            latency_bg.hid_last_us.store(us, Ordering::Relaxed);
                            latency_bg.hid_max_us.fetch_max(us, Ordering::Relaxed);
                            repaint_bg.notify();
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
        let gui_res = gui::run_with_engine(engine, audio_shared, repaint);
        #[cfg(not(feature = "audio"))]
        let gui_res = gui::run_with_engine(engine, repaint);
        #[cfg(feature = "usb")]
        if let Some(h) = display_handle.take() {
            h.join();
        }
        return gui_res;
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
                let t0 = std::time::Instant::now();
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
                                let _ = engine
                                    .lock()
                                    .handle_event(EngineEvent::HeadphoneVolume(vol));
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
                display_activity.store(true, Ordering::Relaxed);

                if !no_led {
                    sync_hardware_leds(&engine, &mut mapper, &led_tx);
                    last_led_sync = std::time::Instant::now();
                }

                let us = t0.elapsed().as_micros() as u64;
                latency_stats.hid_events.fetch_add(1, Ordering::Relaxed);
                latency_stats.hid_last_us.store(us, Ordering::Relaxed);
                latency_stats.hid_max_us.fetch_max(us, Ordering::Relaxed);
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

    // Arrête le thread d'affichage (qui envoie l'image par défaut)
    #[cfg(feature = "usb")]
    if let Some(h) = display_handle.take() {
        h.join();
    }

    // Réinitialise les écrans à leur image par défaut à la fermeture
    #[cfg(feature = "usb")]
    {
        use maschine_hardware::usb::BulkHandle;
        if let Ok(mut bulk) = BulkHandle::open() {
            let left = maschine_proto::display::DisplayFrame::solid(
                maschine_proto::display::DisplayId::Left,
                maschine_proto::display::DisplayPixel::from_rgb(14, 15, 20),
            );
            let right = maschine_proto::display::DisplayFrame::solid(
                maschine_proto::display::DisplayId::Right,
                maschine_proto::display::DisplayPixel::from_rgb(14, 15, 20),
            );
            let _ = bulk.send_frame(&left);
            let _ = bulk.send_frame(&right);
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
    use maschine_proto::hid_output::{encode_report_80, encode_report_81_mapped, LedState};

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
    let r81 = encode_report_81_mapped(&state, maschine_hardware::pad_output_hw_order())?;
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
                    let r81off =
                        encode_report_81_mapped(&off, maschine_hardware::pad_output_hw_order())?;
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

/// Allume les pads 1..16 un par un : si le pad éclairé n'est pas celui annoncé,
/// le mapping hardware/logique doit être ajusté (voir HW_PAD_TO_LOGICAL).
fn cmd_pad_test(step_ms: u64) -> anyhow::Result<()> {
    use maschine_proto::color::Color;
    use maschine_proto::hid_output::LedState;
    let devs = find_maschine_devices()?;
    let path = devs
        .iter()
        .find_map(|d| d.hidraw_path.clone())
        .ok_or_else(|| anyhow::anyhow!("pas de hidraw trouvé"))?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)?;
    use std::os::fd::AsRawFd;
    let fd = file.as_raw_fd();
    println!(
        "Pad test sur {path:?} : pads 1..16 dans l'ordre logique (bas-gauche -> haut-droite)."
    );
    println!("Vérifiez que chaque numéro annoncé correspond au pad physique éclairé.");
    for pad in 0..16u8 {
        let mut s = LedState::new();
        s.set_pad(pad as usize, Color::from_index(6).unwrap_or(Color::OFF));
        hw_output::send_leds(fd, &s)?;
        println!(
            "  ... pad logique {} (physique Pad {}) allumé",
            pad,
            pad + 1
        );
        std::thread::sleep(std::time::Duration::from_millis(step_ms));
    }
    hw_output::send_leds(fd, &LedState::new())?;
    println!("✓ pad-test terminé");
    Ok(())
}

/// Moniteur live des pads : affiche chaque événement traité par le pipeline
/// (baseline, hystérésis, anti-rebond, anti-diaphonie, rafales) avec les
/// numéros physique (sérigraphie 1..16) et logique (0..15).
/// Les suppressions (fantômes, rebonds, continuations) n'apparaissent qu'avec
/// `-v` (logs debug du thread HID).
fn cmd_pad_monitor() -> anyhow::Result<()> {
    let devs = find_maschine_devices()?;
    let info = devs
        .into_iter()
        .find(|d| d.hidraw_path.is_some())
        .ok_or_else(|| anyhow::anyhow!("pas de MK3 hidraw trouvée — lance probe"))?;
    let path = info.hidraw_path.clone().unwrap();
    println!("Pad monitor sur {path:?} (Ctrl+C pour arrêter).");
    println!("Appuyez les pads : une ligne par événement traité (mapping logique = physique - 1).");
    let (tx, rx) = unbounded::<HardwareEvent>();
    let cfg = HidrawConfig::default();
    let _handle = spawn_hidraw_thread(info, cfg, tx)?;
    let start = std::time::Instant::now();
    let mut counts = [0u32; 16];
    loop {
        if arc_ctrlc_flag() {
            println!("Interrompu.");
            break;
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(HardwareEvent::Pad(p)) => {
                let dt = start.elapsed().as_secs_f64();
                if p.pressed {
                    counts[p.pad as usize % 16] += 1;
                    println!(
                        "[+{:7.2}s] Pad {:>2} (log {:>2}) PRESS vel={:<3} #{:<4}",
                        dt,
                        p.pad + 1,
                        p.pad,
                        p.velocity,
                        counts[p.pad as usize % 16]
                    );
                } else {
                    println!(
                        "[+{:7.2}s] Pad {:>2} (log {:>2}) release",
                        dt,
                        p.pad + 1,
                        p.pad
                    );
                }
            }
            Ok(HardwareEvent::Connected { serial }) => {
                println!("✓ MK3 connectée serial={serial:?}");
            }
            Ok(HardwareEvent::Error(e)) => {
                println!("! erreur hardware : {e}");
            }
            Ok(HardwareEvent::Disconnected) => {
                println!("MK3 déconnectée.");
                break;
            }
            Ok(_) => {}
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}

/// Calibration : demande d'appuyer les pads 1..16 et affiche l'index brut vu.
/// Si Pad N (physique, sérigraphie) donne raw == N-1, le firmware numérote en
/// logique : lancez avec `MASCHINE_PAD_MAP=identity`.
/// Si Pad 1 donne raw 12, Pad 13 donne raw 0, etc. (scan haut->bas) : défaut `hw` OK.
fn cmd_pad_calibrate() -> anyhow::Result<()> {
    use maschine_proto::hid_input::decode_report_02;
    let devs = find_maschine_devices()?;
    let path = devs
        .iter()
        .find_map(|d| d.hidraw_path.clone())
        .ok_or_else(|| anyhow::anyhow!("pas de hidraw trouvé"))?;
    println!("Pad calibrate sur {path:?} (Ctrl+C pour interrompre).");
    println!("Appuyez chaque pad demandé (un appui franc), 6s max par pad.");
    let file = std::fs::OpenOptions::new().read(true).open(&path)?;
    use std::os::fd::AsRawFd;
    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    unsafe {
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    let mut table: Vec<(u8, Option<u8>)> = Vec::new();
    let mut buf = [0u8; 128];
    for pad_n in 1..=16u8 {
        println!("--- Appuyez sur le Pad {pad_n} (physique, UNE frappe franche puis relâchez) ...");
        let start = std::time::Instant::now();
        // Pression max vue par raw sur la fenêtre : le vrai pad frappé a le pic
        // le plus fort, les voisins déclenchés par vibration sont plus faibles.
        // (L'ancien code prenait le PREMIER déclenché et se faisait berner.)
        let mut best: [u16; 16] = [0; 16];
        let mut first_hit: Option<std::time::Instant> = None;
        while start.elapsed() < std::time::Duration::from_secs(6) {
            let mut pfd = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let ret = unsafe { libc::poll(&mut pfd as *mut _, 1, 100) };
            if ret <= 0 {
                if arc_ctrlc_flag() {
                    println!("Interrompu.");
                    return Ok(());
                }
                // Fenêtre de collecte écoulée depuis la 1re détection : on tranche.
                if first_hit.is_some_and(|t| t.elapsed() > std::time::Duration::from_millis(600)) {
                    break;
                }
                continue;
            }
            let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n <= 0 {
                continue;
            }
            let data = &buf[..n as usize];
            if data.is_empty() || data[0] != 0x02 {
                continue;
            }
            if let Ok(pads) = decode_report_02(data) {
                // decode renvoie l'index déjà mappé hw->logique : on retrouve le brut
                // via la table auto-inverse (raw = T[logique]).
                for p in pads {
                    if p.pressure >= 0x4000 {
                        let raw = maschine_proto::consts::hw_pad_to_logical(p.pad);
                        if (raw as usize) < 16 {
                            best[raw as usize] = best[raw as usize].max(p.pressure);
                            if p.touched && first_hit.is_none() {
                                first_hit = Some(std::time::Instant::now());
                            }
                        }
                    }
                }
                // 600ms après la 1re détection : pic collecté, on tranche.
                if first_hit.is_some_and(|t| t.elapsed() > std::time::Duration::from_millis(600)) {
                    break;
                }
            }
        }
        // Gagnant = raw avec la pression max ; affiche aussi les voisins
        // (diaphonie) pour diagnostiquer un capteur trop sensible.
        let mut ranked: Vec<(u8, u16)> = best
            .iter()
            .enumerate()
            .filter(|(_, &pr)| pr > 0)
            .map(|(raw, &pr)| (raw as u8, pr))
            .collect();
        ranked.sort_by_key(|&(_, pr)| std::cmp::Reverse(pr));
        let found = ranked.first().map(|&(raw, _)| raw);
        match found {
            Some(raw) => {
                let ghosts: Vec<String> = ranked
                    .iter()
                    .skip(1)
                    .take(3)
                    .map(|&(r, pr)| format!("raw {r} (0x{pr:04X})"))
                    .collect();
                if ghosts.is_empty() {
                    println!(
                        "  Pad {pad_n} -> raw {raw} (pic 0x{:04X})",
                        best[raw as usize]
                    );
                } else {
                    println!(
                        "  Pad {pad_n} -> raw {raw} (pic 0x{:04X}) — voisins: {}",
                        best[raw as usize],
                        ghosts.join(", ")
                    );
                }
            }
            None => println!("  Pad {pad_n} -> (pas de détection, timeout)"),
        }
        table.push((pad_n, found));
        // anti-rebond : vide le buffer (borné à 300ms pour ne jamais bloquer).
        let drain_start = std::time::Instant::now();
        while drain_start.elapsed() < std::time::Duration::from_millis(300) {
            let mut pfd = libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let ret = unsafe { libc::poll(&mut pfd as *mut _, 1, 20) };
            if ret > 0 {
                let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                if n <= 0 {
                    break;
                }
            } else {
                break;
            }
        }
    }
    println!("\n# Résultat (pad physique -> raw) :");
    for (n, raw) in &table {
        println!(
            "  Pad {n} -> {}",
            raw.map(|r| r.to_string()).as_deref().unwrap_or("?")
        );
    }
    let identity = table
        .iter()
        .filter_map(|(n, r)| r.map(|raw| raw == n - 1))
        .filter(|&b| b)
        .count();
    if identity >= 12 {
        println!("Conclusion : raw == logique → relancez avec MASCHINE_PAD_MAP=identity");
    } else {
        println!("Conclusion : scan hardware haut->bas → mapping par défaut (hw) OK.");
        println!("Astuce : MASCHINE_PAD_OUTPUT=identity si `pad-test` s'allume à l'envers.");
    }
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
        let dev = devices
            .iter()
            .find(|d| {
                if let Ok(desc) = d.device_descriptor() {
                    desc.vendor_id() == 0x17cc && desc.product_id() == 0x1600
                } else {
                    false
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Maschine MK3 (17cc:1600) non trouvée en USB"))?;

        let handle = dev.open()?;
        let _ = handle.set_auto_detach_kernel_driver(true);
        handle.claim_interface(5)?;
        println!("   ✓ Interface 5 (Maschine MK3 BD) réclamée !");

        // Helper: construit et envoie une frame complète selon le protocole de référence
        let _send_solid = |handle: &rusb::DeviceHandle<rusb::Context>,
                           display_id: u8,
                           r: u8,
                           g: u8,
                           b: u8|
         -> anyhow::Result<()> {
            let color = maschine_proto::display::DisplayPixel::from_rgb(r, g, b);
            let did = if display_id == 0 {
                maschine_proto::display::DisplayId::Left
            } else {
                maschine_proto::display::DisplayId::Right
            };
            let frame = maschine_proto::display::DisplayFrame::solid(did, color);
            let data = maschine_proto::display::encode_bulk_transfer(&frame)?;

            println!(
                "   Frame {:5} octets — header[0..20]: {:02x?}",
                data.len(),
                &data[0..20]
            );
            println!("   footer[-8..]: {:02x?}", &data[data.len() - 8..]);

            let n = handle.write_bulk(0x04, &data, std::time::Duration::from_millis(2000))?;
            if n != data.len() {
                anyhow::bail!("Short write: {n}/{}", data.len());
            }
            println!("   ✓ Écran {} : {n} octets envoyés OK", display_id);
            Ok(())
        };

        // Helper: split 4-write (ni-display)
        let _send_split = |handle: &rusb::DeviceHandle<rusb::Context>,
                           display_id: u8,
                           r: u8,
                           g: u8,
                           b: u8|
         -> anyhow::Result<()> {
            let color = maschine_proto::display::DisplayPixel::from_rgb(r, g, b);
            let did = if display_id == 0 {
                maschine_proto::display::DisplayId::Left
            } else {
                maschine_proto::display::DisplayId::Right
            };
            let frame = maschine_proto::display::DisplayFrame::solid(did, color);
            let data = maschine_proto::display::encode_bulk_transfer(&frame)?;
            let timeout = std::time::Duration::from_millis(1000);
            handle.write_bulk(0x04, &data[0..16], timeout)?;
            handle.write_bulk(0x04, &data[16..20], timeout)?;
            handle.write_bulk(0x04, &data[20..20 + 261120], timeout)?;
            handle.write_bulk(0x04, &data[20 + 261120..], timeout)?;
            Ok(())
        };

        // Helper: row-by-row (cabl)
        let _send_row_by_row = |handle: &rusb::DeviceHandle<rusb::Context>,
                                display_id: u8,
                                r: u8,
                                g: u8,
                                b: u8|
         -> anyhow::Result<()> {
            let color = maschine_proto::display::DisplayPixel::from_rgb(r, g, b);
            let px_bytes = color.to_be_bytes();
            let timeout = std::time::Duration::from_millis(500);
            for row in 0..272u16 {
                let mut pkt = Vec::with_capacity(20 + 480 * 2 + 8);
                pkt.extend_from_slice(&[
                    0x84,
                    0x00,
                    display_id,
                    0x60,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                    (row >> 8) as u8,
                    (row & 0xFF) as u8,
                    0x01,
                    0xE0,
                    0x00,
                    0x01,
                ]);
                pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0xF0]);
                for _ in 0..480 {
                    pkt.extend_from_slice(&px_bytes);
                }
                pkt.extend_from_slice(&[0x03, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00]);
                handle.write_bulk(0x04, &pkt, timeout)?;
            }
            Ok(())
        };

        println!("\n2. AFFICHAGE DE L'INTERFACE DAW OFFICIELLE SUR LES 2 ÉCRANS...");
        let mut snap = maschine_engine::EngineSnapshot {
            project: std::sync::Arc::new(maschine_engine::project::Project::new_demo()),
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
            song_mode: false,
            active_section: None,
            triggered_pads: [false; 16],
            pad_activity: [0.0; 16],
            scene_mode: false,
            pattern_mode: false,
            select_mode: false,
            erase_mode: false,
            controller_mode: 0,
            browser_preset_idx: 0,
            selected_bar: 0,
            step_cursor: 0,
            pattern_bars: 2,
            group_levels: [0.0; 8],
            group_peaks: [0.0; 8],
            master_level: 0.0,
            master_peak: 0.0,
            soft_labels: ["1", "2", "3", "4", "5", "6", "7", "8"],
            waveform: None,
            sample_duration: 0.0,
            revision: 0,
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

        println!(
            "   ✓ {frame_idx} frames de l'interface DAW rendues avec succès sur les 2 écrans."
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_sampler_navigation_routes_next_hardware_knob_to_sound() {
        let mut engine = Engine::new_demo(48000);
        engine.handle_event(EngineEvent::SetRightTab(1)).unwrap();
        engine.handle_event(EngineEvent::SetLeftTab(2)).unwrap();

        let mut mapper = HardwareMapper::with_default();
        mapper.left_tab = 0;
        mapper.right_tab = 1;
        let context = engine.hardware_snapshot();
        sync_mapper_context(&mut mapper, &context);

        let event = |value| {
            HardwareEvent::Encoder(maschine_proto::hid_input::EncoderEvent {
                id: 1,
                value,
                touched: true,
            })
        };
        let _ = mapper.handle_hardware_event(event(100)); // baseline absolu
        let actions = mapper.handle_hardware_event(event(101));
        assert!(actions.contains(&MapperAction::SoundVolumeDelta(1.0)));
        assert_eq!(mapper.left_tab, 2);
        assert_eq!(mapper.right_tab, 0);
    }
}

// ctrlc est initialisé en début de main via init_ctrlc()
