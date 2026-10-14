//! Thread de rafraîchissement des 2 écrans MK3 via USB Bulk (IF5).

#[cfg(feature = "usb")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "usb")]
use std::sync::Arc;
#[cfg(feature = "usb")]
use std::time::Duration;

#[cfg(feature = "usb")]
use maschine_engine::Engine;
#[cfg(feature = "usb")]
use maschine_hardware::usb::BulkHandle;
#[cfg(feature = "usb")]
use parking_lot::Mutex;

/// Handle du thread display
#[cfg(feature = "usb")]
#[allow(dead_code)]
pub struct DisplayThreadHandle {
    join: std::thread::JoinHandle<()>,
    stop: Arc<AtomicBool>,
}

#[cfg(feature = "usb")]
impl DisplayThreadHandle {
    /// Demande l'arrêt du thread
    #[allow(dead_code)]
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Attend la fin du thread (envoie d'abord l'image par défaut)
    #[allow(dead_code)]
    pub fn join(self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.join.join();
    }
}

/// Lance le thread de rafraîchissement des 2 écrans MK3 via USB Bulk (IF5)
#[cfg(feature = "usb")]
pub fn spawn_display_thread(engine: Arc<Mutex<Engine>>) -> anyhow::Result<DisplayThreadHandle> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    let join = std::thread::Builder::new()
        .name("maschine-display".to_string())
        .spawn(move || {
            let mut bulk = match BulkHandle::open() {
                Ok(b) => {
                    log::info!("✓ Écrans USB Bulk ouvert (IF5)");
                    b
                }
                Err(e) => {
                    log::warn!("Impossible d'ouvrir l'accès USB Bulk pour les écrans: {e}");
                    return;
                }
            };

            let mut frame_count = 0u64;

            while !stop_clone.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(66)); // ~15 FPS

                let snap = engine.lock().snapshot();

                let left_frame = if snap.song_mode {
                    maschine_engine::display_render::render_arranger_left(&snap)
                } else {
                    match snap.left_tab {
                        1 => maschine_engine::display_render::render_mixer_left(&snap),
                        2 => maschine_engine::display_render::render_sampler_left(&snap),
                        3 => maschine_engine::display_render::render_browser_left(&snap),
                        _ => {
                            if snap.song_mode {
                                maschine_engine::display_render::render_arranger_left(&snap)
                            } else {
                                maschine_engine::display_render::render_daw_left(&snap)
                            }
                        }
                    }
                };
                let right_frame = if snap.song_mode {
                    maschine_engine::display_render::render_arranger_right(&snap)
                } else if snap.left_tab == 2 && snap.right_tab == 0 {
                    // SAMPLER + PATTERN : paire édition (VOL/PAN/CUT/RESO + ADSR).
                    maschine_engine::display_render::render_sampler_right(&snap)
                } else if snap.left_tab == 3 && snap.right_tab == 0 {
                    // BROWSER : validation et chargement preset.
                    maschine_engine::display_render::render_browser_right(&snap)
                } else {
                    match snap.right_tab {
                        1 => maschine_engine::display_render::render_events_right(&snap),
                        2 => maschine_engine::display_render::render_macro_right(&snap),
                        3 => maschine_engine::display_render::render_settings_right(&snap),
                        _ => maschine_engine::display_render::render_daw_right(&snap),
                    }
                };

                if let Err(e) = bulk.send_frame(&left_frame) {
                    log::warn!("Écran gauche send err: {e}");
                }
                if let Err(e) = bulk.send_frame(&right_frame) {
                    log::warn!("Écran droit send err: {e}");
                }

                frame_count += 1;
                if frame_count == 1 {
                    log::info!("✓ Écrans MK3 rafraîchis (première frame OK)");
                }
            }

            // Ne pas envoyer de frame noire : on libère l'interface pour que le firmware
            // de la MK3 réaffiche son écran par défaut (logo Maschine).
            std::thread::sleep(Duration::from_millis(200));
            log::info!("✓ Interface écrans libérée (logo MK3 par défaut)");
        })
        .map_err(|e| anyhow::anyhow!("spawn display thread: {e}"))?;

    Ok(DisplayThreadHandle { join, stop })
}
