//! Standalone guitar FX rack
//!
//! Audio: JACK (PipeWire) duplex + CPAL fallback
//! GUI: egui/eframe with parameter knobs, VU meters, device selection
// NOTE : crate hors périmètre maschine-linux — dead_code pré-existant toléré
// (bridges GUI/audio conservés pour évolutions futures).
#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use log::{error, info};
use parking_lot::Mutex;

use maschine_dsp::{GuitarChain, ParamId};

// Audio backend modules
// #[cfg(feature = "jack")]
// mod audio_jack;
#[cfg(feature = "cpal")]
mod audio_cpal;

// GUI module
#[cfg(feature = "gui")]
mod gui;

// Shared types
mod config;
mod params_bridge;

use config::AppConfig;
use params_bridge::ParamsBridge;

/// Audio message from audio thread to GUI
#[derive(Debug, Clone)]
pub enum AudioToGui {
    InputLevel(f32),
    OutputLevel(f32),
    DeviceList(Vec<String>, Vec<String>), // inputs, outputs
}

/// GUI message to audio thread
#[derive(Debug, Clone)]
pub enum GuiToAudio {
    SetParam(ParamId, f32),
    SetInputDevice(usize),
    SetOutputDevice(usize),
    SetBypass(bool),
    RequestDeviceList,
    Quit,
}

/// Main application state
#[derive(Clone)]
#[allow(dead_code)] // champs conservés pour le metering/paramètres futurs
struct App {
    config: AppConfig,
    params_bridge: Arc<ParamsBridge>,
    chain: Arc<Mutex<GuitarChain>>,
    /// Audio thread sends metering (AudioToGui) to GUI
    audio_to_gui_tx: Sender<AudioToGui>,
    /// GUI receives metering (AudioToGui) from audio
    audio_to_gui_rx: Receiver<AudioToGui>,
    /// GUI sends param changes (GuiToAudio) to audio
    gui_to_audio_tx: Sender<GuiToAudio>,
    /// Audio thread receives param changes (GuiToAudio) from GUI
    gui_to_audio_rx: Receiver<GuiToAudio>,
    running: Arc<AtomicBool>,
}

impl App {
    fn new(config: AppConfig) -> Result<Self> {
        let sample_rate = config.audio.sample_rate as f32;
        let chain = Arc::new(Mutex::new(GuitarChain::new(sample_rate)));
        let params_bridge = Arc::new(ParamsBridge::new(chain.clone()));

        let (audio_to_gui_tx, audio_to_gui_rx) = bounded::<AudioToGui>(64);
        let (gui_to_audio_tx, gui_to_audio_rx) = bounded::<GuiToAudio>(64);

        let running = Arc::new(AtomicBool::new(true));

        Ok(Self {
            config,
            params_bridge,
            chain,
            audio_to_gui_tx,
            audio_to_gui_rx,
            gui_to_audio_tx,
            gui_to_audio_rx,
            running,
        })
    }

    #[cfg(feature = "cpal")]
    fn run_audio_thread(&self) -> Result<()> {
        audio_cpal::run_audio_thread(
            self.config.clone(),
            self.chain.clone(),
            self.params_bridge.clone(),
            self.audio_to_gui_tx.clone(),
            self.gui_to_audio_rx.clone(),
            self.running.clone(),
        )
    }

    #[cfg(not(feature = "cpal"))]
    fn run_audio_thread(&self) -> Result<()> {
        anyhow::bail!("No audio backend enabled (need 'cpal' feature)");
    }

    fn run_gui(&mut self) -> Result<()> {
        #[cfg(feature = "gui")]
        {
            gui::run_gui(
                self.config.clone(),
                self.params_bridge.clone(),
                self.gui_to_audio_tx.clone(),
                self.audio_to_gui_rx.clone(),
                self.running.clone(),
            )
            .map_err(|e| anyhow::anyhow!("GUI error: {}", e))
        }

        #[cfg(not(feature = "gui"))]
        {
            // Headless mode - just wait for Ctrl+C
            info!("Running in headless mode. Press Ctrl+C to quit.");
            ctrlc::set_handler({
                let running = self.running.clone();
                move || running.store(false, Ordering::Relaxed)
            })?;

            while self.running.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(100));
            }
            Ok(())
        }
    }
}

fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let config = AppConfig::load_or_default();
    info!(
        "Starting Maschine Guitar Standalone v{}",
        env!("CARGO_PKG_VERSION")
    );
    info!("Config: {:?}", config);

    let mut app = App::new(config)?;

    // Spawn audio thread
    let audio_handle = thread::Builder::new().name("audio".to_string()).spawn({
        let app = app.clone();
        move || {
            if let Err(e) = app.run_audio_thread() {
                error!("Audio thread error: {}", e);
            }
        }
    })?;

    // Run GUI on main thread (required by egui/winit)
    if let Err(e) = app.run_gui() {
        error!("GUI error: {}", e);
    }

    // Shutdown
    app.running.store(false, Ordering::Relaxed);
    audio_handle.join().ok();
    info!("Shutdown complete");
    Ok(())
}
