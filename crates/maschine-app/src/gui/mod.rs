//! Module GUI `egui` — interface "Maschine MK3 Studio".
//!
//! Le moteur reste le maître de l'état (`Engine` verrouillé brièvement par
//! action). La UI lit des instantanés (`EngineSnapshot`, O(1) grâce à
//! `Arc<Project>`) pour rester réactive sans bloquer l'audio.
//!
//! Le panneau MK3 à droite affiche un miroir temps réel des deux écrans
//! 480×272 (rendus par `maschine_engine::display_render::render_screens`) :
//! tout ce que fait le hardware est visible et pilotable depuis le PC.

use parking_lot::Mutex;
use std::sync::Arc;

pub mod app;
pub mod theme;
pub mod transport;
pub mod views;
pub mod widgets;

/// Signal de repaint partagé : le thread HID (hardware) réveille la GUI
/// immédiatement quand un événement MK3 arrive ("tac-au-tac").
#[derive(Clone, Default)]
pub struct RepaintSignal(Arc<Mutex<Option<eframe::egui::Context>>>);

impl RepaintSignal {
    /// Crée un signal vide (la GUI s'y attache à la création).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attache le contexte egui (appelé une fois par la GUI).
    pub fn attach(&self, ctx: &eframe::egui::Context) {
        let mut guard = self.0.lock();
        if guard.is_none() {
            *guard = Some(ctx.clone());
        }
    }

    /// Réveille la GUI depuis n'importe quel thread (thread HID, audio…).
    pub fn notify(&self) {
        if let Some(ctx) = self.0.lock().as_ref() {
            ctx.request_repaint();
        }
    }
}

/// Lance le GUI avec un engine, une sortie audio partagée et un signal de repaint.
#[cfg(feature = "audio")]
pub fn run_with_engine(
    engine: Arc<Mutex<maschine_engine::Engine>>,
    audio: Arc<Mutex<Option<crate::audio::AudioOutput>>>,
    repaint: RepaintSignal,
) -> anyhow::Result<()> {
    app::run_native_gui(engine, audio, repaint)
}

/// Lance le GUI avec un engine seul (pas d'audio).
#[cfg(not(feature = "audio"))]
pub fn run_with_engine(
    engine: Arc<Mutex<maschine_engine::Engine>>,
    repaint: RepaintSignal,
) -> anyhow::Result<()> {
    app::run_native_gui(engine, repaint)
}
