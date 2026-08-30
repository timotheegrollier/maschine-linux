//! `maschine-linux` — binaire principal.
//!
//! CLI : `probe`, `hid-dump`, `daemon`, `project`...

use std::os::unix::fs::PermissionsExt as _;
use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossbeam_channel::unbounded;
use log::{error, info, warn};

use maschine_engine::{Engine, EngineEvent, Project};
use maschine_hardware::device::find_maschine_devices;
use maschine_hardware::events::HardwareEvent;
use maschine_hardware::hidraw::{spawn_hidraw_thread, HidrawConfig};
use maschine_hardware::mapper::{HardwareMapper, MapperConfig};

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
    /// Lance la GUI (nécessite --features gui)
    Gui {
        /// Projet à charger
        #[arg(long)]
        project: Option<PathBuf>,
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
        Commands::Daemon { project, midi_port } => cmd_daemon(project, midi_port)?,
        Commands::Project { cmd } => cmd_project(cmd)?,
        Commands::LedTest { pad, color } => cmd_led_test(pad, color)?,
        Commands::Gui { project } => cmd_gui(project)?,
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

fn cmd_daemon(project: Option<PathBuf>, _midi_port: String) -> anyhow::Result<()> {
    println!("# maschine-linux daemon — Ctrl+C pour quitter");
    let proj = if let Some(p) = project {
        println!("Chargement {p:?}");
        Project::load_json(&p)?
    } else {
        println!("Projet démo");
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
    println!(
        "Device: VID:PID {:04x}:{:04x} hidraw={:?} serial={:?}",
        info.vendor_id, info.product_id, info.hidraw_path, info.serial
    );

    let (tx, rx) = unbounded::<HardwareEvent>();
    let cfg = HidrawConfig::default();
    let _handle = spawn_hidraw_thread(info, cfg, tx)?;

    let mut mapper = HardwareMapper::new(MapperConfig::default());
    let mut engine = Engine::new(proj, 48000);
    // donne un sample dummy au pad 0 pour avoir du son (silence sinon)
    // on génère 100ms de sine pour demo
    {
        let samp: Vec<f32> = (0..4800)
            .map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 48000.0).sin() * 0.3)
            .collect();
        engine.sampler_mut().add_zone(
            0,
            maschine_engine::sampler::ZoneMapping {
                sample: Some(std::sync::Arc::new(samp)),
                sample_path: None,
                sample_rate: 48000,
                root_key: 36,
                lokey: 0,
                hikey: 127,
                lovel: 0,
                hivel: 127,
                volume_db: 0.0,
                pan: 0.0,
                tune_cents: 0,
                choke_group: None,
            },
        );
    }

    let engine_proj = engine.project_handle();
    // thread pour afficher snapshot toutes les 2s
    let proj_clone = engine_proj.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(2));
        let p = proj_clone.read();
        log::info!(
            "heartbeat project='{}' groups={} bpm={}",
            p.name,
            p.groups.len(),
            p.bpm
        );
    });

    // boucle principale : HID -> mapper -> engine
    // On bloque sur rx avec timeout pour aussi avancer le transport
    let mut last_advance = std::time::Instant::now();
    loop {
        // avance transport 10ms de temps si playing
        let elapsed = last_advance.elapsed();
        if elapsed >= Duration::from_millis(10) {
            let samples = (elapsed.as_secs_f64() * 48000.0) as u64;
            engine.advance(samples);
            // render dummy (pour faire tourner le sampler)
            let mut dummy = vec![0.0; 480 * 2]; // 10ms stereo
            engine.render_audio(&mut dummy);
            last_advance = std::time::Instant::now();
        }

        // poll events avec timeout 5ms
        match rx.recv_timeout(Duration::from_millis(5)) {
            Ok(ev) => {
                match &ev {
                    HardwareEvent::Connected { serial } => {
                        info!("✓ MK3 connectée serial={:?}", serial)
                    }
                    HardwareEvent::Pad(p) if p.pressed => {
                        info!(
                            "PAD {} vel={} pressure=0x{:04x}",
                            p.pad, p.velocity, p.pressure
                        );
                    }
                    HardwareEvent::Button { id, pressed, .. } if *pressed => {
                        info!("BTN {:?} pressed", id);
                    }
                    HardwareEvent::Error(e) => error!("hardware error: {e}"),
                    HardwareEvent::Disconnected => {
                        warn!("MK3 déconnectée");
                        break;
                    }
                    _ => {}
                }
                let midi = mapper.handle(ev.clone());
                for m in &midi {
                    log::debug!("MIDI {:?}", m);
                }
                // convertit HardwareEvent::Pad/Button -> EngineEvent
                match ev {
                    HardwareEvent::Pad(p) => {
                        let _ = engine.handle_event(EngineEvent::Pad {
                            pad: p.pad,
                            velocity: p.velocity,
                            pressed: p.pressed,
                        });
                        // LED feedback — pour l'instant on log, l'envoi hid_output viendra avec bulk thread
                        let leds = mapper.led_state();
                        // on pourrait envoyer via hidraw OUT ici (pas encore implémenté en full)
                        let _ = leds;
                    }
                    HardwareEvent::Button { id, pressed, .. } if pressed => {
                        use maschine_proto::ButtonId;
                        match id {
                            ButtonId::Group(g) => {
                                let _ = engine.handle_event(EngineEvent::GroupFocus(g));
                            }
                            ButtonId::Play => {
                                let _ = engine.handle_event(EngineEvent::Transport {
                                    play: true,
                                    rec: false,
                                    stop: false,
                                });
                                info!("transport: PLAY tick={}", engine.snapshot().transport.tick);
                            }
                            ButtonId::Stop => {
                                let _ = engine.handle_event(EngineEvent::Transport {
                                    play: false,
                                    rec: false,
                                    stop: true,
                                });
                                info!("transport: STOP");
                            }
                            ButtonId::Rec => {
                                let _ = engine.handle_event(EngineEvent::Transport {
                                    play: false,
                                    rec: true,
                                    stop: false,
                                });
                                info!("transport: REC");
                            }
                            _ => {}
                        }
                    }
                    _ => {}
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
    if let Err(e) = engine.snapshot().project.save_json(&autosave) {
        warn!("autosave failed {autosave:?}: {e}");
    } else {
        println!("Autosave → {autosave:?}");
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
    state.set_button_raw(maschine_proto::hid_output::offsets::SHIFT + 1, 7);
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

fn cmd_gui(project: Option<PathBuf>) -> anyhow::Result<()> {
    #[cfg(feature = "gui")]
    {
        return gui::run(project);
    }
    #[cfg(not(feature = "gui"))]
    {
        let _ = project;
        anyhow::bail!(
            "GUI non compilée — relance avec: cargo run -p maschine-app --features gui -- gui"
        );
    }
}

#[cfg(feature = "gui")]
mod gui {
    use super::*;
    use eframe::egui;

    pub fn run(project: Option<PathBuf>) -> anyhow::Result<()> {
        let proj = if let Some(p) = project {
            Project::load_json(&p)?
        } else {
            Project::new_demo()
        };
        let engine = Arc::new(RwLock::new(Engine::new(proj, 48000)));
        let opts = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
            ..Default::default()
        };
        eframe::run_native(
            "maschine-linux",
            opts,
            Box::new(|_| Ok(Box::new(GuiApp { engine }))),
        )
        .map_err(|e| anyhow::anyhow!("eframe: {e}"))?;
        Ok(())
    }

    struct GuiApp {
        engine: Arc<RwLock<Engine>>,
    }

    impl eframe::App for GuiApp {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            let snap = self.engine.read().snapshot();
            egui::TopBottomPanel::top("top").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(format!(
                        "maschine-linux — {} — {:.0} BPM",
                        snap.project.name, snap.transport.bpm
                    ));
                    ui.separator();
                    let state = match snap.transport.state {
                        maschine_engine::transport::TransportState::Playing => "▶ PLAY",
                        maschine_engine::transport::TransportState::Recording => "● REC",
                        maschine_engine::transport::TransportState::Stopped => "■ STOP",
                    };
                    ui.label(state);
                    ui.label(format!("{}  voices={}", snap.bbt, snap.num_voices));
                    ui.label(format!("Group {}", (b'A' + snap.group_focus) as char));
                });
            });
            egui::SidePanel::left("groups").show(ctx, |ui| {
                ui.heading("Groups");
                for (i, g) in snap.project.groups.iter().enumerate() {
                    let focused = i as u8 == snap.group_focus;
                    let label = format!(
                        "{}: {} ({} pat)",
                        (b'A' + i as u8) as char,
                        g.name,
                        g.patterns.len()
                    );
                    if ui.selectable_label(focused, label).clicked() {
                        self.engine
                            .write()
                            .handle_event(EngineEvent::GroupFocus(i as u8))
                            .ok();
                    }
                }
                ui.separator();
                ui.heading("Scenes");
                for (i, s) in snap.project.scenes.iter().enumerate() {
                    let active = Some(i) == snap.active_scene;
                    if ui.selectable_label(active, &s.name).clicked() {
                        self.engine.write().handle_event(EngineEvent::Scene(i)).ok();
                    }
                }
                ui.separator();
                if ui.button("Play/Stop").clicked() {
                    let playing = snap.transport.is_playing();
                    self.engine
                        .write()
                        .handle_event(EngineEvent::Transport {
                            play: !playing,
                            rec: false,
                            stop: playing,
                        })
                        .ok();
                }
            });
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Patterns (Group focus)");
                if let Some(group) = snap.project.groups.get(snap.group_focus as usize) {
                    for (pi, pat) in group.patterns.iter().enumerate() {
                        ui.collapsing(format!("{} — {} events — len {}", pat.name, pat.events.len(), pat.length), |ui| {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                for ev in &pat.events {
                                    ui.label(format!("pad {} tick {} vel {} len {}", ev.pad, ev.tick, ev.velocity, ev.length_ticks));
                                }
                            });
                            let _ = pi;
                        });
                    }
                } else {
                    ui.label("Pas de Group");
                }
                ui.separator();
                ui.label("GUI MVP — Ideas/Song grid, Pattern editor piano-roll et sampler viendront en Phase 2.");
            });
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}

// ctrlc est initialisé en début de main via init_ctrlc()
