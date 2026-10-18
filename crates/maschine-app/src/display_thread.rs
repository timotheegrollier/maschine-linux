//! Thread de rafraîchissement des 2 écrans MK3 via USB Bulk (IF5).
//!
//! Stratégie « diff incrémental » (0.6.0) :
//!
//! - Une frame pleine pèse **261 148 octets** (480×272 RGB565 + header). Envoyer
//!   les 2 écrans à 30 FPS = ~15,7 Mo/s : le device sature, les écritures bulk
//!   finissent en timeout et les écrans se figent.
//! - Le protocole accepte un **header de fenêtre** (`x/y/w/h`) : on n'envoie que
//!   les régions modifiées ([`FrameDiffer`]), typiquement quelques Ko.
//! - Un **budget d'octets** (~2,5 Mo/s) lisse les rafraîchissements complets
//!   (changement de vue) sans jamais bloquer les petites mises à jour.
//! - Toute erreur invalide le diff et force une frame complète ; après plusieurs
//!   échecs : `clear_halt` + backoff, puis ré-acquisition de l'interface.

#[cfg(feature = "usb")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "usb")]
use std::sync::Arc;
#[cfg(feature = "usb")]
use std::time::{Duration, Instant};

#[cfg(feature = "usb")]
use maschine_engine::Engine;
#[cfg(feature = "usb")]
use maschine_hardware::usb::BulkHandle;
#[cfg(feature = "usb")]
use maschine_proto::display::{transfer_len, DisplayFrame, DisplayId, DisplayPixel, FrameDiffer};
#[cfg(feature = "usb")]
use parking_lot::Mutex;

/// Plafond de rafraîchissement (~30 FPS) : laisse du temps au device.
#[cfg(feature = "usb")]
const MIN_INTERVAL: Duration = Duration::from_millis(33);
/// Débit moyen autorisé pour les écrans (octets/s).
///
/// Calibré **sous le mur matériel du MCU** : au-dessus de ~2 Mo/s soutenus,
/// la file RX du MCU déborde et l'endpoint cesse d'accepter (écritures en
/// timeout, écran figé). Mesuré sain : 1,6 Mo/s tient indéfiniment ; à 2,4 Mo/s
/// le wedge apparaît après ~20-30 s.
#[cfg(feature = "usb")]
const BYTE_RATE: f64 = 1_600_000.0;
/// Rafale maximale (une paire pleine écran ≈ 522 Ko tient dedans).
#[cfg(feature = "usb")]
const BYTE_BURST: f64 = 560_000.0;
/// Seuil « transfert lent » (au-delà, on log + compteur).
#[cfg(feature = "usb")]
const SLOW_TRANSFER: Duration = Duration::from_millis(120);
/// Intervalle du log de statistiques écrans.
#[cfg(feature = "usb")]
const STATS_INTERVAL: Duration = Duration::from_secs(10);
/// Backoff après erreur (min, max) — assez long pour laisser la file RX du
/// MCU se vider avant de repousser des centaines de Ko.
#[cfg(feature = "usb")]
const BACKOFF_MIN: Duration = Duration::from_millis(250);
#[cfg(feature = "usb")]
const BACKOFF_MAX: Duration = Duration::from_secs(4);
/// Nombre d'erreurs consécutives sans succès avant ré-acquisition douce de
/// l'interface (jamais de reset USB en cours de route : il tuerait le flux
/// audio qui partage le même device).
#[cfg(feature = "usb")]
const ERRORS_BEFORE_REACQUIRE: u32 = 8;

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

/// État d'un écran : diff + besoin éventuel d'une frame pleine.
#[cfg(feature = "usb")]
struct ScreenState {
    differ: FrameDiffer,
    pending_full: bool,
}

#[cfg(feature = "usb")]
impl ScreenState {
    fn new(display: DisplayId) -> Self {
        Self {
            differ: FrameDiffer::new(display),
            pending_full: true,
        }
    }

    /// Régions à envoyer pour amener l'écran à jour.
    fn plan(&self, frame: &DisplayFrame) -> Vec<DisplayFrame> {
        if self.pending_full {
            vec![frame.to_full()]
        } else {
            self.differ.diff(frame)
        }
    }

    /// Marque l'écran comme synchronisé avec `frame`.
    fn commit(&mut self, frame: &DisplayFrame) {
        self.differ.commit(frame);
        self.pending_full = false;
    }

    /// Invalide (prochain envoi = frame pleine).
    fn invalidate(&mut self) {
        self.differ.invalidate();
        self.pending_full = true;
    }
}

/// Statistiques écrans (log périodique).
#[cfg(feature = "usb")]
#[derive(Default)]
struct DisplayStats {
    frames: u64,
    bytes: u64,
    rects: u64,
    errors: u64,
    slow: u64,
    since: Option<Instant>,
}

#[cfg(feature = "usb")]
impl DisplayStats {
    fn log(&mut self) {
        let now = Instant::now();
        let Some(since) = self.since else {
            self.since = Some(now);
            return;
        };
        let dt = now.duration_since(since).as_secs_f64();
        if dt < STATS_INTERVAL.as_secs_f64() {
            return;
        }
        if self.frames > 0 {
            log::debug!(
                "Écrans: {:.1} FPS, {:.0} Ko/s, {:.1} régions/frame, {} err, {} lents",
                self.frames as f64 / dt,
                self.bytes as f64 / dt / 1024.0,
                self.rects as f64 / self.frames as f64,
                self.errors,
                self.slow,
            );
        }
        self.frames = 0;
        self.bytes = 0;
        self.rects = 0;
        self.errors = 0;
        self.slow = 0;
        self.since = Some(now);
    }
}

/// Sonde de vie du MCU : fenêtre 1×1 px dans le coin bas-droit (30 octets).
///
/// Après une erreur, on ne renvoie les vraies frames que si cette
/// micro-écriture passe : inutile de noyer une file RX saturée. Le pixel noir
/// écrasé se resynchronise tout seul au diff suivant.
#[cfg(feature = "usb")]
fn probe_frame() -> DisplayFrame {
    DisplayFrame {
        display: DisplayId::Right,
        x: 479,
        y: 271,
        w: 1,
        h: 1,
        pixels: vec![DisplayPixel::BLACK],
    }
}

/// Durée d'attente après le `n`-ième échec consécutif ; `None` ⇒ escalade
/// (ré-acquisition douce de l'interface) nécessaire.
#[cfg(feature = "usb")]
fn backoff_for(n: u32) -> Option<Duration> {
    if n >= ERRORS_BEFORE_REACQUIRE {
        None
    } else {
        let exp = n.min(4).saturating_sub(1);
        Some((BACKOFF_MIN * 2u32.saturating_pow(exp)).min(BACKOFF_MAX))
    }
}

/// Envoie un plan (régions) puis commite ; en cas d'erreur invalide l'écran.
#[cfg(feature = "usb")]
fn flush_screen(
    bulk: &mut BulkHandle,
    state: &mut ScreenState,
    frame: &DisplayFrame,
    plan: Vec<DisplayFrame>,
    stats: &mut DisplayStats,
) -> Result<(), String> {
    for region in &plan {
        let started = Instant::now();
        let bytes = bulk
            .send_frame(region)
            .map_err(|e| format!("bulk write: {e}"))?;
        let elapsed = started.elapsed();
        stats.bytes += bytes as u64;
        stats.rects += 1;
        if elapsed > SLOW_TRANSFER {
            stats.slow += 1;
            log::debug!(
                "Écran {:?}: transfert lent {} ms ({} octets)",
                region.display,
                elapsed.as_millis(),
                bytes
            );
        }
    }
    state.commit(frame);
    Ok(())
}

/// Lance le thread de rafraîchissement des 2 écrans MK3 via USB Bulk (IF5).
///
/// `activity` est un drapeau *sans verrou* posé par la boucle HID à chaque action
/// utilisateur : le thread détecte l'activité en ~3 ms mais ne verrouille le
/// moteur qu'au plus toutes les 33 ms (le callback audio prend ce même verrou).
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
            // ── Cycle de vie : (ré)ouverture du Bulk, puis boucle de refresh ──
            // Une autre instance (gui/daemon) peut tenir IF5 au démarrage, et la MK3
            // peut être débranchée pendant l'exécution : au lieu d'abandonner
            // (écrans figés définitivement), on réessaie toutes les 3 s puis on
            // rouvre l'interface à chaud quand elle est disponible à nouveau.
            'lifecycle: loop {
                if stop_clone.load(Ordering::Relaxed) {
                    return;
                }
                let mut open_fails = 0u32;
                let mut bulk = 'open: loop {
                    if stop_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    match BulkHandle::open() {
                        Ok(b) => break 'open b,
                        Err(e) => {
                            open_fails += 1;
                            if open_fails == 1 || open_fails % 10 == 0 {
                                log::warn!(
                                    "Impossible d'ouvrir l'accès USB Bulk pour les écrans: {e} \
                                     (nouvel essai dans 3 s)"
                                );
                            }
                        }
                    }
                    for _ in 0..30 {
                        if stop_clone.load(Ordering::Relaxed) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                };
                log::info!("✓ Écrans USB Bulk ouvert (IF5)");

                let mut left = ScreenState::new(DisplayId::Left);
                let mut right = ScreenState::new(DisplayId::Right);
                let mut stats = DisplayStats {
                    since: Some(Instant::now()),
                    ..DisplayStats::default()
                };
                let mut budget = BYTE_BURST;
                let mut last_refill = Instant::now();
                let mut last_revision = u64::MAX;
                let mut last_check = Instant::now() - Duration::from_secs(10);
                let mut last_send = last_check;
                let mut pending = true;
                let mut backoff_until: Option<Instant> = None;
                let mut consecutive_errors = 0u32;
                let mut soft_retries = 0u32;
                let mut first_frame = true;

                while !stop_clone.load(Ordering::Relaxed) {
                    let now = Instant::now();

                    // Backoff après erreur : on laisse le device respirer.
                    if let Some(until) = backoff_until {
                        if now < until {
                            std::thread::sleep(Duration::from_millis(20));
                            continue;
                        }
                        backoff_until = None;
                    }

                    // Réveil court (2-3 ms) : réactivité immédiate aux actions
                    // utilisateur sans marteler le verrou du moteur (partagé audio).
                    let active = activity.swap(false, Ordering::Relaxed);
                    if now.duration_since(last_check) < MIN_INTERVAL {
                        std::thread::sleep(Duration::from_millis(if active { 2 } else { 3 }));
                        continue;
                    }
                    last_check = now;

                    // Budget octets : recharge linéaire, plafonnée à la rafale.
                    let dt = now.duration_since(last_refill).as_secs_f64();
                    last_refill = now;
                    budget = (budget + dt * BYTE_RATE).min(BYTE_BURST);

                    let (snap, revision) = {
                        let eng = engine.lock();
                        (eng.snapshot(), eng.revision())
                    };

                    let dirty = revision != last_revision
                        || pending
                        || last_send.elapsed() > Duration::from_secs(2);
                    if !dirty {
                        std::thread::sleep(Duration::from_millis(3));
                        continue;
                    }
                    last_revision = revision;

                    let mut failed = false;

                    // Sonde en reprise : après une erreur, vérifier d'abord que le
                    // MCU re-accepte une micro-écriture (fenêtre 1×1 px ≈ 30 o),
                    // avant de renvoyer de vraies frames — inutile de noyer une
                    // file RX saturée avec des centaines de Ko.
                    if consecutive_errors > 0 {
                        match bulk.send_frame(&probe_frame()) {
                            Ok(n) => stats.bytes += n as u64,
                            Err(e) => {
                                stats.errors += 1;
                                left.invalidate();
                                right.invalidate();
                                bulk.clear_stall();
                                log::warn!("Écrans: sonde KO ({e})");
                                failed = true;
                            }
                        }
                    }

                    if !failed {
                        let (left_frame, right_frame) =
                            maschine_engine::display_render::render_screens(&snap);
                        let left_plan = left.plan(&left_frame);
                        let right_plan = right.plan(&right_frame);
                        let bytes: usize = left_plan
                            .iter()
                            .chain(right_plan.iter())
                            .map(|f| transfer_len(f.pixels.len()))
                            .sum();

                        // Pas assez de budget : on réessaiera au prochain tick. Les
                        // diffs ne sont pas commités → rien n'est perdu (coalescing).
                        if bytes as f64 > budget {
                            pending = true;
                            std::thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                        budget -= bytes as f64;
                        stats.frames += 1;

                        let left_result =
                            flush_screen(&mut bulk, &mut left, &left_frame, left_plan, &mut stats);
                        let right_result =
                            flush_screen(&mut bulk, &mut right, &right_frame, right_plan, &mut stats);
                        if let Err(e) = left_result {
                            failed = true;
                            stats.errors += 1;
                            left.invalidate();
                            log::warn!("Écran gauche send err: {e}");
                        }
                        if let Err(e) = right_result {
                            failed = true;
                            stats.errors += 1;
                            right.invalidate();
                            log::warn!("Écran droit send err: {e}");
                        }
                    }

                    if failed {
                        consecutive_errors += 1;
                        pending = true;
                        if let Some(delay) = backoff_for(consecutive_errors) {
                            // Le pipe peut être bloqué : halt + backoff exponentiel
                            // (assez long pour laisser la file RX du MCU se vider).
                            bulk.clear_stall();
                            backoff_until = Some(now + delay);
                        } else {
                            // ── Escalade : ré-acquisition DOUCE de l'interface
                            // (release + claim + clear halt). Jamais de reset USB en
                            // cours de route : il tuerait le flux audio qui partage
                            // ce device. Cooldown croissant pour laisser la file se
                            // vider puis repartir sur des frames pleines.
                            match bulk.reacquire() {
                                Ok(()) => {
                                    soft_retries += 1;
                                    let cool = 3u64 * (1u64 << soft_retries.min(3));
                                    log::warn!(
                                        "Écrans: ré-acquisition douce n°{soft_retries} après \
                                         {consecutive_errors} erreurs — reprise dans {cool} s"
                                    );
                                    left.invalidate();
                                    right.invalidate();
                                    consecutive_errors = 0;
                                    backoff_until = Some(now + Duration::from_secs(cool));
                                }
                                Err(e) => {
                                    // Device probablement parti (débranché) :
                                    // réouverture complète via le cycle de vie.
                                    log::warn!(
                                        "Écrans: ré-acquisition échouée ({e}) — réouverture complète"
                                    );
                                    break;
                                }
                            }
                        }
                    } else {
                        consecutive_errors = 0;
                        soft_retries = 0;
                        pending = false;
                        last_send = Instant::now();
                        if first_frame {
                            first_frame = false;
                            log::info!("✓ Écrans MK3 rafraîchis (première frame OK)");
                        }
                    }

                    stats.log();
                }

                if stop_clone.load(Ordering::Relaxed) {
                    // Ne pas envoyer de frame noire : on libère l'interface pour que le
                    // firmware de la MK3 réaffiche son écran par défaut (logo Maschine).
                    std::thread::sleep(Duration::from_millis(200));
                    log::info!("✓ Interface écrans libérée (logo MK3 par défaut)");
                    return;
                }
                // Interface perdue (device débranché, ré-énumération, ImportError) :
                // on libère tout et on repasse par la boucle d'ouverture.
                drop(bulk);
                log::warn!("Écrans: interface perdue — réouverture dans 3 s");
                std::thread::sleep(Duration::from_secs(3));
                continue 'lifecycle;
            }
        })
        .map_err(|e| anyhow::anyhow!("spawn display thread: {e}"))?;

    Ok(DisplayThreadHandle { join, stop })
}
