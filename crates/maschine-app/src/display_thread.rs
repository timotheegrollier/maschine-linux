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

/// Lance le thread de rafraîchissement des 2 écrans MK3 via USB Bulk (IF5).
///
/// `activity` est un drapeau *sans verrou* posé par la boucle HID à chaque action
/// utilisateur : le thread détecte l'activité en ~3 ms mais ne verrouille le
/// moteur qu'au plus toutes les 33 ms (le callback audio prend ce même verrou
/// en `try_lock` — on ne veut pas l'augmenter).
#[cfg(feature = "usb")]
pub fn spawn_display_thread(
    engine: Arc<Mutex<Engine>>,
    activity: Arc<AtomicBool>,
) -> anyhow::Result<DisplayThreadHandle> {
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
            let mut last_revision = u64::MAX;
            let mut last_check = std::time::Instant::now() - Duration::from_secs(10);
            let mut last_send = last_check;

            while !stop_clone.load(Ordering::Relaxed) {
                // Réveil court (3 ms) : réactivité immédiate aux actions utilisateur
                // sans marteler le verrou du moteur (partagé avec l'audio).
                let active = activity.swap(false, Ordering::Relaxed);
                if last_check.elapsed() < Duration::from_millis(33) {
                    // Plafond ~30 FPS (2 x 261 Ko par frame en USB Bulk).
                    std::thread::sleep(Duration::from_millis(if active { 2 } else { 3 }));
                    continue;
                }
                last_check = std::time::Instant::now();

                let (snap, revision) = {
                    let eng = engine.lock();
                    (eng.snapshot(), eng.revision())
                };

                if revision == last_revision && last_send.elapsed() < Duration::from_secs(2) {
                    // Rien de neuf : aucun envoi (garde-fou 2 s pour absorber
                    // toute modification qui aurait manqué l'invalidation).
                    std::thread::sleep(Duration::from_millis(3));
                    continue;
                }
                last_revision = revision;
                last_send = std::time::Instant::now();

                let (left_frame, right_frame) =
                    maschine_engine::display_render::render_screens(&snap);

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
