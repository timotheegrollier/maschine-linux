//! Poll hidraw `/dev/hidraw*` — thread temps-réel, 1ms.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use maschine_proto::hid_input::{decode_report_01, decode_report_02};

use crate::device::DeviceInfo;
use crate::events::{pad_to_state, HardwareEvent};
use crate::{HardwareError, Result};

/// Config hidraw.
#[derive(Debug, Clone)]
pub struct HidrawConfig {
    /// Poll interval (défaut 1ms → bInterval HS)
    pub poll_interval: Duration,
    /// Debounce boutons (défaut 4ms)
    pub debounce: Duration,
    /// Taille ring buffer channel
    pub channel_cap: usize,
    /// Mapping pads (défaut Hw, surchargeable via `MASCHINE_PAD_MAP`)
    pub pad_map: crate::PadMap,
}

impl Default for HidrawConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(1),
            debounce: Duration::from_millis(4),
            channel_cap: 1024,
            pad_map: crate::PadMap::from_env(),
        }
    }
}

/// Handle du thread hidraw — `stop()` pour arrêter.
pub struct HidrawHandle {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl HidrawHandle {
    /// Arrête le thread.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for HidrawHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // on ne join pas en Drop pour éviter blocage — caller doit appeler stop()
    }
}

/// Démarre le poll hidraw en arrière-plan. Envoie `HardwareEvent` sur `tx`.
pub fn spawn_hidraw_thread(
    info: DeviceInfo,
    cfg: HidrawConfig,
    tx: Sender<HardwareEvent>,
) -> Result<HidrawHandle> {
    let path = info.hidraw_path.clone().ok_or_else(|| {
        HardwareError::NotFound("pas de hidraw_path — device non HID ou permission".to_string())
    })?;
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let serial = info.serial.clone();

    let join = std::thread::Builder::new()
        .name("maschine-hidraw".to_string())
        .spawn(move || {
            if let Err(e) = hidraw_loop(path, cfg, tx.clone(), stop2, serial) {
                let _ = tx.send(HardwareEvent::Error(format!("hidraw loop: {e}")));
            }
            let _ = tx.send(HardwareEvent::Disconnected);
        })
        .map_err(|e| HardwareError::Other(format!("spawn hidraw: {e}")))?;

    // petit délai pour laisser le thread envoyer Connected ou Error
    std::thread::sleep(Duration::from_millis(10));

    Ok(HidrawHandle {
        stop,
        join: Some(join),
    })
}

/// Fenêtre d'analyse du pic de vélocité après un front montant de pad.
/// La pression d'une frappe monte en ~5-15ms : on accumule le pic pendant ce
/// délai puis on émet UN SEUL événement press (one-shot propre, pas de boucle
/// de re-déclenchements). Un tap éclair (< fenêtre) émet press+release au relâchement.
const PAD_ARM_WINDOW: Duration = Duration::from_millis(12);

/// TTL de la mémoire de rivaux co-armés (voir `PadTracker::rival_mem`) : couvre
/// l'écart de flush entre deux pads d'une même rafale, sans déborder sur les
/// frappes volontaires suivantes.
const RIVAL_MEM_MS: u64 = 30;

/// Config anti-diaphonie + anti-rebond + rafales (mise en cache) :
/// `(ratio, debounce_ms, gap_ms, gap_rise)`.
/// - Quand plusieurs pads s'arment DANS LA MÊME RAFALE (vibration), seul le
///   plus fort joue : un pad dont le pic est < `ratio` × pic max des rivaux
///   co-armés est supprimé. `MASCHINE_PAD_XTALK_RATIO=0` désactive.
///   (L'historique des frappes ne sert jamais de rival : une note émise avant
///   ne doit pas faire taire une vraie frappe.)
/// - Après une frappe émise, un nouveau front montant du MÊME pad dans
///   `debounce_ms` est ignoré (rebond capteur / oscillation autour du seuil).
///   `MASCHINE_PAD_DEBOUNCE_MS` (défaut 35ms, 0 = désactive).
/// - Le firmware envoie des RAFALES de 0x02, pas un flux continu : entre deux
///   rafales d'un appui maintenu, le timeout 30ms relâche à tort puis la rafale
///   suivante re-déclenche (boucle). Si un front survient dans `gap_ms` après
///   un release sur timeout ET que la pression reste PLATE pendant l'armement
///   (montée < `gap_rise` : capteur bloqué, rebond ou rafale suivante — une
///   vraie frappe monte toujours de centaines de counts), c'est la continuation
///   de la même tenue : pas de nouveau press. `MASCHINE_PAD_GAP_MS` (défaut 100),
///   `MASCHINE_PAD_GAP_RISE` (défaut 48 counts de pression).
fn xtalk_cfg() -> (f32, u64, u64, u16) {
    static CACHED: std::sync::OnceLock<(f32, u64, u64, u16)> = std::sync::OnceLock::new();
    *CACHED.get_or_init(|| {
        let ratio = std::env::var("MASCHINE_PAD_XTALK_RATIO")
            .ok()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);
        let debounce = std::env::var("MASCHINE_PAD_DEBOUNCE_MS")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(35);
        let gap_ms = std::env::var("MASCHINE_PAD_GAP_MS")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(100);
        let gap_rise = std::env::var("MASCHINE_PAD_GAP_RISE")
            .ok()
            .and_then(|s| s.trim().parse::<u16>().ok())
            .unwrap_or(48);
        (ratio, debounce, gap_ms, gap_rise)
    })
}

/// Pic rival le plus fort pour le pad `idx` : pics courants des autres pads
/// armés DANS LA MÊME RAFALE **et dont la pression monte** (`rise >= rise_min`).
/// Seule la simultanéité + une vraie montée prouve la diaphonie (vibration) :
/// un voisin plat (fantôme, capteur bloqué) n'est jamais rival, sinon il
/// ferait taire les vraies frappes douces (cf. log : fantôme rival 9 vs vraie
/// note pic 1). Une frappe émise quelques ms plus tôt ne doit JAMAIS faire
/// taire une vraie frappe. `last_hit` reste utilisé par l'anti-rebond.
/// Indépendant de l'ordre de flush (les deux sens marchent).
fn xtalk_rival_peak(
    idx: usize,
    peaks: &[u8; 16],
    armed: &[bool; 16],
    pmin: &[u16; 16],
    pmax: &[u16; 16],
    rise_min: u16,
) -> u8 {
    let mut best = 0u8;
    for (j, armed_j) in armed.iter().enumerate() {
        if j != idx && *armed_j && pmax[j].saturating_sub(pmin[j]) >= rise_min {
            best = best.max(peaks[j]);
        }
    }
    best
}

/// Vrai si le pic `peak` doit être supprimé face au rival `rival`.
fn xtalk_suppressed(peak: u8, rival: u8, ratio: f32) -> bool {
    ratio > 0.0 && rival > 0 && (peak as f32) < ratio * (rival as f32)
}

/// Action pad normalisée produite par le [`PadTracker`] (émission sur fronts
/// uniquement : un appui maintenu = un seul press, même avec un firmware en
/// rafales et des capteurs qui vibrent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PadAction {
    Press {
        pad: u8,
        velocity: u8,
        pressure: u16,
    },
    Release {
        pad: u8,
        pressure: u16,
    },
}

/// État press/pic/trajectoire par pad + mémoire inter-rafales.
/// Toute la politique "un appui = un press" vit ici — pilotable sans hardware
/// (voir tests : rejeu de rafales plates, frappe fraîche, rebond, diaphonie).
/// Base du délai de confirmation d'un release (défaut 15ms,
/// `MASCHINE_PAD_RELEASE_CONFIRM_MS`, 0 = immédiat).
/// Le délai réel est adaptatif (voir `confirm_delay_ms`).
fn release_confirm_ms() -> u64 {
    static CACHED: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("MASCHINE_PAD_RELEASE_CONFIRM_MS")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(15)
    })
}

/// Nombre de lectures brutes IDENTIQUES consécutives qui déclarent un capteur
/// bloqué (défaut 32, `MASCHINE_PAD_STUCK_N`, 0 = désactive). Un doigt vivant
/// fait toujours jitter les LSB de l'ADC ; 32 lectures bit-exactes d'affilée
/// = capteur bloqué (cf. pad 2 à 0x413C constant sur 82 fronts).
fn stuck_run_threshold() -> u32 {
    static CACHED: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("MASCHINE_PAD_STUCK_N")
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(32)
    })
}

struct PadTracker {
    prev_pressed: [bool; 16],
    peak_vel: [u8; 16],
    arming: [Option<Instant>; 16],
    arm_pmin: [u16; 16],
    arm_pmax: [u16; 16],
    gap_since: [Option<Instant>; 16],
    last_hit: [(u8, Option<Instant>); 16],
    last_seen: [Instant; 16],
    /// Baseline adaptative par pad (zéro capteur). Initialisée à 0x4000 puis
    /// calée sur toute valeur brute stable (capteur bloqué) : la pression
    /// effective devient `0x4000 + (raw - base)`, le fantôme constant tombe
    /// sous les seuils et ne déclenche plus rien — sans affecter les vraies
    /// frappes (pics bien au-dessus).
    base: [u16; 16],
    /// Dernière valeur brute vue (détection de stabilité).
    last_raw: [u16; 16],
    /// Compteur de lectures brutes identiques consécutives.
    same_run: [u32; 16],
    /// Release en attente de confirmation (instant, pression) : une chute doit
    /// persister pour valoir levée (voir `confirm_delay_ms`).
    rel_pending: [Option<(Instant, u16)>; 16],
    /// Début de la tenue en cours par pad (front montant hors release).
    hold_since: [Option<Instant>; 16],
    /// Mémoire de rivaux co-armés (pic, instant) : quand un pad qui MONTE
    /// flushe, il laisse son pic aux pads armés au même moment — pour que la
    /// suppression ne dépende pas de l'ordre de flush. TTL court
    /// (`RIVAL_MEM_MS`) : seules les rafales simultanées comptent, jamais
    /// l'historique des frappes (qui ferait taire les vraies notes).
    /// Effacée à chaque (ré)armement : un nouveau cycle décide seul.
    rival_mem: [(u8, Option<Instant>); 16],
}

impl PadTracker {
    fn new(now: Instant) -> Self {
        Self {
            prev_pressed: [false; 16],
            peak_vel: [0; 16],
            arming: [None; 16],
            arm_pmin: [0; 16],
            arm_pmax: [0; 16],
            gap_since: [None; 16],
            last_hit: [(0, None); 16],
            last_seen: [now; 16],
            base: [0x4000; 16],
            last_raw: [0xFFFF; 16],
            same_run: [0; 16],
            rel_pending: [None; 16],
            hold_since: [None; 16],
            rival_mem: [(0, None); 16],
        }
    }

    /// État pressé courant (pour l'hystérésis `pad_to_state` côté appelant).
    fn is_pressed(&self, idx: usize) -> bool {
        self.prev_pressed[idx]
    }

    /// Pression effective d'une lecture brute : écart à la baseline du pad.
    /// Met à jour la détection de capteur bloqué (valeur brute stable :
    /// la baseline se cale dessus, le fantôme constant disparaît).
    /// Retourne la pression ajustée (0x4000 = repos).
    fn effective_pressure(&mut self, idx: usize, raw: u16) -> u16 {
        if raw == self.last_raw[idx] {
            self.same_run[idx] = self.same_run[idx].saturating_add(1);
        } else {
            self.same_run[idx] = 1;
            self.last_raw[idx] = raw;
        }
        let stuck_n = stuck_run_threshold();
        if stuck_n > 0 && self.same_run[idx] >= stuck_n && self.base[idx] != raw {
            log::warn!(
                "pad {idx} capteur bloqué suspecté (0x{raw:04X} constant) — baseline adaptée"
            );
            self.base[idx] = raw;
        }
        0x4000 + raw.saturating_sub(self.base[idx])
    }

    /// Délai de confirmation de release pour un pad : base + la moitié de la
    /// durée de tenue (plafonné à base + 65ms). Plus un appui dure, plus on
    /// ponte les creux : une oscillation à 20Hz pendant une tenue de 400ms ne
    /// doit jamais relâcher, tandis qu'un tap éclair (tenue ~8ms) libère en
    /// ~20ms. Sans effet audible sur les one-shots (déjà joués).
    fn confirm_delay_ms(&self, idx: usize, now: Instant) -> u64 {
        let base = release_confirm_ms();
        if base == 0 {
            return 0;
        }
        let held = self.hold_since[idx]
            .map(|t| now.duration_since(t).as_millis().min(130) as u64)
            .unwrap_or(0);
        base + (held / 2).min(65)
    }

    /// Traite un pad PRÉSENT dans un report 0x02 (`pressed` après hystérésis).
    fn present(
        &mut self,
        idx: usize,
        pressed: bool,
        velocity: u8,
        pressure: u16,
        now: Instant,
    ) -> Vec<PadAction> {
        let mut out = Vec::new();
        self.last_seen[idx] = now;
        let prev = self.prev_pressed[idx];
        if pressed {
            // Pression revenue/confimée : annule toute confirmation de release
            // en cours (micro-coupure pontée).
            self.rel_pending[idx] = None;
        }
        if pressed && !prev {
            // Front montant : arme l'analyse du pic, sans émettre
            // (la vélocité du premier report est quasi nulle).
            let (_, debounce_ms, _, _) = xtalk_cfg();
            // Anti-rebond même pad : un front qui suit de trop près la frappe
            // émise précédente = rebond/oscillation, pas une nouvelle frappe.
            // (La continuation de rafale plate est tranchée au flush via la
            // trajectoire, voir `flush`.)
            let bounced = self.last_hit[idx].1.is_some_and(|t| {
                debounce_ms > 0 && now.duration_since(t) <= Duration::from_millis(debounce_ms)
            });
            self.prev_pressed[idx] = true;
            // Début (ou poursuite) de tenue : sert le délai adaptatif.
            self.hold_since[idx] = Some(now);
            if bounced {
                log::debug!(
                    "pad {idx} front ignoré (rebond <{debounce_ms}ms, pression=0x{pressure:04X})"
                );
            } else {
                // (Re)armement : la décision continuation / nouvelle frappe se
                // prend au flush, une fois la trajectoire connue (le gap doit
                // survivre jusque-là : effacé au flush). Un nouveau cycle
                // oublie la mémoire de rivaux du cycle précédent.
                log::debug!("pad {idx} front montant (pression=0x{pressure:04X} vel={velocity})");
                self.peak_vel[idx] = velocity.max(1);
                self.arm_pmin[idx] = pressure;
                self.arm_pmax[idx] = pressure;
                self.arming[idx] = Some(now);
                self.rival_mem[idx] = (0, None);
            }
        } else if pressed {
            // Appui maintenu : accumule pic + trajectoire, émission unique.
            self.peak_vel[idx] = self.peak_vel[idx].max(velocity);
            self.arm_pmin[idx] = self.arm_pmin[idx].min(pressure);
            self.arm_pmax[idx] = self.arm_pmax[idx].max(pressure);
            if let Some(since) = self.arming[idx] {
                if now.duration_since(since) >= PAD_ARM_WINDOW {
                    out.extend(self.flush(idx, pressure, now));
                }
            }
            // sinon déjà émis (held) : silence radio.
        } else if prev {
            // Chute sous le seuil alors que le pad est présent : PAS de release
            // immédiat — armée d'une confirmation (pontage des micro-coupures
            // d'une pression oscillante : sinon chaque oscillation relâche puis
            // re-déclenche). Le release ne part que si la chute persiste
            // (voir `sweep`).
            if self.rel_pending[idx].is_none() {
                log::debug!(
                    "pad {idx} chute sous le seuil (pression=0x{pressure:04X}) — confirmation en cours"
                );
                self.rel_pending[idx] = Some((now, pressure));
            }
        }
        out
    }

    /// Traite un pad ABSENT d'un report 0x02 (slot 0xFF/invalide = relâché).
    /// Ne fait rien si le pad n'était pas pressé.
    fn absent(&mut self, idx: usize, now: Instant) -> Vec<PadAction> {
        let out = Vec::new();
        if !self.prev_pressed[idx] {
            return out;
        }
        // Disparition du report : armée d'une confirmation comme une chute
        // (le release ne part que si l'absence persiste).
        if self.rel_pending[idx].is_none() {
            log::debug!("pad {idx} absent du report — confirmation en cours");
            self.rel_pending[idx] = Some((now, 0x4000));
        }
        out
    }

    /// Sweep temporel, à appeler à chaque itération de boucle :
    /// - flush les frappes armées dont la fenêtre de pic est écoulée ;
    /// - relâche les pads dont le stream 0x02 ne parle plus depuis 30ms.
    ///
    /// Idempotent (flush/release une seule fois via `arming`/`prev_pressed`).
    fn sweep(&mut self, now: Instant) -> Vec<PadAction> {
        let mut out = Vec::new();
        for i in 0..16 {
            // Frappe armée mais plus aucun report : le pic est prêt, on émet
            // le press unique (le doigt est encore posé ou le tap était éclair).
            if let Some(since) = self.arming[i] {
                if now.duration_since(since) >= PAD_ARM_WINDOW {
                    out.extend(self.flush(i, 0x4080, now));
                }
            }
            // Stream 0x02 silencieux pour ce pad depuis 30ms.
            // (`last_seen` est rafraîchi dès que le pad est PRÉSENT dans un
            // report, même sous le seuil ON : un appui maintenu dont la pression
            // retombe dans la bande d'hystérésis ne doit PAS relâcher/re-déclencher
            // en boucle.)
            if self.prev_pressed[i]
                && now.duration_since(self.last_seen[i]) > Duration::from_millis(30)
            {
                let held = self.hold_since[i].map(|t| now.duration_since(t).as_millis());
                log::debug!(
                    "pad {i} release (stream 0x02 silencieux depuis 30ms, tenue {held:?}ms)"
                );
                self.prev_pressed[i] = false;
                self.peak_vel[i] = 0;
                self.arming[i] = None;
                self.hold_since[i] = None;
                self.rel_pending[i] = None;
                // Pas de preuve de levée : la prochaine rafale plate et proche
                // sera traitée comme continuation (pas de nouveau press).
                self.gap_since[i] = Some(now);
                out.push(PadAction::Release {
                    pad: i as u8,
                    pressure: 0x4000,
                });
            }
            // Confirmation d'un release en attente (chute/absence persistante =
            // vraie levée ; les micro-coupures ont été pontées entre-temps).
            // Le délai grandit avec la tenue (oscillation à 20Hz pontée).
            let confirm_ms = self.confirm_delay_ms(i, now);
            if let Some((since, pressure)) = self.rel_pending[i] {
                if now.duration_since(since) >= Duration::from_millis(confirm_ms) {
                    self.rel_pending[i] = None;
                    if self.prev_pressed[i] {
                        // Tap éclair : flush du press armé avant le release.
                        if self.arming[i].is_some() {
                            out.extend(self.flush(i, pressure, now));
                        }
                        let held = self.hold_since[i].map(|t| now.duration_since(t).as_millis());
                        log::debug!(
                            "pad {i} release confirmé (chute persistante, tenue {held:?}ms)"
                        );
                        self.prev_pressed[i] = false;
                        self.peak_vel[i] = 0;
                        self.hold_since[i] = None;
                        // Preuve de levée : oublie le gap.
                        self.gap_since[i] = None;
                        out.push(PadAction::Release {
                            pad: i as u8,
                            pressure,
                        });
                    }
                }
            }
        }
        out
    }

    /// Émet le press armé avec son pic de vélocité, puis désarme.
    /// Supprime (aucun événement) si continuation de tenue sur rafale plate
    /// ou voisin bien plus fort dans la même fenêtre (anti-diaphonie).
    fn flush(&mut self, idx: usize, pressure: u16, now: Instant) -> Vec<PadAction> {
        let mut out = Vec::new();
        let (ratio, _, gap_ms, gap_rise) = xtalk_cfg();
        // Continuation de tenue : release sur timeout récent (pas de preuve de
        // levée) + pression restée plate pendant l'armement = même appui.
        // Le gap est consommé au flush (le prochain timeout-release le reposera).
        let rise = self.arm_pmax[idx].saturating_sub(self.arm_pmin[idx]);
        let in_gap = gap_ms > 0
            && self.gap_since[idx]
                .is_some_and(|t| now.duration_since(t) <= Duration::from_millis(gap_ms));
        self.gap_since[idx] = None;
        if in_gap && rise < gap_rise {
            log::debug!(
                "pad {idx} continuation de tenue (rafale plate, montée={rise}) — press supprimé"
            );
            self.arming[idx] = None;
            return out;
        }
        if ratio > 0.0 {
            let armed: [bool; 16] = std::array::from_fn(|j| self.arming[j].is_some());
            let mut rival = xtalk_rival_peak(
                idx,
                &self.peak_vel,
                &armed,
                &self.arm_pmin,
                &self.arm_pmax,
                gap_rise,
            );
            // + mémoire des co-armements récents (le fort a peut-être déjà
            // flushé : l'ordre de flush ne doit rien changer).
            if let (p, Some(t)) = self.rival_mem[idx] {
                if now.duration_since(t) <= Duration::from_millis(RIVAL_MEM_MS) {
                    rival = rival.max(p);
                }
            }
            if xtalk_suppressed(self.peak_vel[idx], rival, ratio) {
                log::debug!(
                    "pad {idx} supprimé par anti-diaphonie (pic {} vs rival {rival})",
                    self.peak_vel[idx]
                );
                self.arming[idx] = None;
                return out;
            }
        }
        let peak = self.peak_vel[idx];
        // Mémorise ce flush comme rival pour les pads co-armés qui flusheront
        // après — mais seulement si la trajectoire monte (un fantôme plat ne
        // devient jamais rival, sinon il ferait taire les vraies notes).
        if rise >= gap_rise {
            for (j, armed_j) in self.arming.iter().enumerate() {
                if j != idx && armed_j.is_some() {
                    self.rival_mem[j] = (peak, Some(now));
                }
            }
        }
        log::debug!("pad {idx} PRESS émis (pic={peak} pression=0x{pressure:04X})");
        self.arming[idx] = None;
        self.last_hit[idx] = (peak, Some(now));
        out.push(PadAction::Press {
            pad: idx as u8,
            velocity: peak.max(1),
            pressure,
        });
        out
    }
}

fn hidraw_loop(
    path: PathBuf,
    cfg: HidrawConfig,
    tx: Sender<HardwareEvent>,
    stop: Arc<AtomicBool>,
    serial: Option<String>,
) -> Result<()> {
    use std::os::unix::io::AsRawFd;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .open(&path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                HardwareError::Permission(format!("{}: {}", path.display(), e))
            } else {
                HardwareError::Io(e)
            }
        })?;

    // non-blocking
    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    unsafe {
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    let _ = tx.send(HardwareEvent::Connected { serial });

    let mut buf = [0u8; 128];
    // Toute la politique pads "un appui = un press" (pics, trajectoire,
    // anti-rebond, anti-diaphonie, rafales) — voir `PadTracker`.
    let mut tracker = PadTracker::new(Instant::now());
    // Set de pads vus au report précédent (debug : détecte le flicker vu/non-vu).
    let mut prev_seen = [false; 16];
    // Set de pads vus au report précédent (debug : détecte le flicker vu/non-vu).
    let mut prev_buttons: std::collections::HashSet<maschine_proto::ButtonId> =
        std::collections::HashSet::new();
    let pad_map = cfg.pad_map;
    // pour debounce boutons : map ButtonId -> Instant last change
    let mut last_poll = Instant::now();
    let mut tick_interval = Instant::now();

    while !stop.load(Ordering::Relaxed) {
        // poll avec timeout cfg.poll_interval
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let timeout_ms = cfg.poll_interval.as_millis() as i32;
        let ret = unsafe { libc::poll(&mut pfd as *mut _, 1, timeout_ms) };
        if ret < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(HardwareError::Io(e));
        }
        if ret == 0 {
            // timeout — heartbeat tick toutes les 100ms
            if tick_interval.elapsed() >= Duration::from_millis(100) {
                let ts = now_ns();
                let _ = tx.send(HardwareEvent::Tick { ts_ns: ts });
                tick_interval = Instant::now();
            }
            // Le firmware n'envoie 0x02 que quand un pad est actif : le sweep
            // temporel (flush des frappes armées + release sur stream silencieux)
            // tourne à chaque itération, y compris sans données (voir fin de boucle).
            let ts = now_ns();
            for a in tracker.sweep(Instant::now()) {
                send_pad_action(&tx, a, ts);
            }
            continue;
        }
        if pfd.revents & libc::POLLIN == 0 {
            if pfd.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
                return Err(HardwareError::Other("hidraw hup/err".to_string()));
            }
            continue;
        }

        let n = match unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) } {
            n if n < 0 => {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    continue;
                }
                return Err(HardwareError::Io(e));
            }
            0 => {
                // EOF — device débranché
                return Err(HardwareError::Other(
                    "hidraw EOF — device débranché ?".to_string(),
                ));
            }
            n => n as usize,
        };

        let data = &buf[..n];
        if data.is_empty() {
            continue;
        }
        let ts = now_ns();

        match data[0] {
            0x01 => {
                match decode_report_01(data) {
                    Ok(inp) => {
                        // Détection de fronts : le report contient l'état complet,
                        // on n'émet que les changements (sinon Play rebascule 1000x/s
                        // tant que le bouton reste tenu, et Shift reste coincé).
                        let current: std::collections::HashSet<maschine_proto::ButtonId> =
                            inp.buttons.iter().map(|b| b.id).collect();
                        for b in &inp.buttons {
                            if !prev_buttons.contains(&b.id) {
                                let _ = tx.send(HardwareEvent::Button {
                                    id: b.id,
                                    pressed: true,
                                    ts_ns: ts,
                                });
                            }
                        }
                        for id in prev_buttons.difference(&current) {
                            let _ = tx.send(HardwareEvent::Button {
                                id: *id,
                                pressed: false,
                                ts_ns: ts,
                            });
                        }
                        prev_buttons = current;
                        for enc in inp.encoders {
                            let _ = tx.send(HardwareEvent::Encoder(enc));
                        }
                        for ts_ev in inp.touchstrip {
                            let _ = tx.send(HardwareEvent::TouchStrip(ts_ev));
                        }
                        for (id, val) in inp.pots {
                            // map KnobId -> ButtonId placeholder
                            let bid = match id {
                                maschine_proto::hid_input::KnobId::MicGain => {
                                    maschine_proto::ButtonId::Unknown(99, 0)
                                }
                                maschine_proto::hid_input::KnobId::Headphone => {
                                    maschine_proto::ButtonId::Unknown(99, 1)
                                }
                                maschine_proto::hid_input::KnobId::Master => {
                                    maschine_proto::ButtonId::Unknown(99, 2)
                                }
                                _ => maschine_proto::ButtonId::Unknown(99, 3),
                            };
                            let _ = tx.send(HardwareEvent::Pot {
                                id: bid,
                                value: val,
                            });
                        }
                    }
                    Err(e) => {
                        log::warn!("decode 0x01 err: {e} data={:02x?}", &data[..n.min(16)]);
                    }
                }
            }
            0x02 => {
                match decode_report_02(data) {
                    Ok(pads) => {
                        // Pour chaque pad brut (proto: déjà hw->logique), applique le
                        // mapping configuré puis hystérésis, et délègue au
                        // `PadTracker` : UN SEUL press par appui (pic de vélocité),
                        // sinon le son mitraille en boucle et le step-sequencer
                        // bascule le pas plusieurs fois par appui.
                        let mut seen = [false; 16];
                        let now = Instant::now();
                        for raw in pads {
                            let logical = pad_map.apply(raw.pad);
                            if logical as usize >= 16 {
                                continue;
                            }
                            let idx = logical as usize;
                            // Reconstruit un événement avec l'index logique effectif.
                            let raw = maschine_proto::hid_input::PadEvent {
                                pad: logical,
                                ..raw
                            };
                            seen[idx] = true;
                            // Pression effective : écart à la baseline du pad.
                            // Un capteur bloqué (ex. pad 3 à 0x413C constant)
                            // devient le zéro de son pad et ne déclenche plus.
                            let adj = tracker.effective_pressure(idx, raw.pressure);
                            let vel_adj = (((adj.saturating_sub(0x4000)) as f32 / 0x0FFD as f32)
                                * 127.0)
                                .clamp(1.0, 127.0) as u8;
                            // Reconstruit un événement avec l'index logique effectif.
                            let raw = maschine_proto::hid_input::PadEvent {
                                pad: logical,
                                pressure: adj,
                                touched: adj >= maschine_proto::consts::pad_on_threshold(),
                                velocity: vel_adj,
                            };
                            let prev = tracker.is_pressed(idx);
                            let st = pad_to_state(raw, ts, prev);
                            for a in tracker.present(idx, st.pressed, st.velocity, st.pressure, now)
                            {
                                send_pad_action(&tx, a, ts);
                            }
                        }
                        // Un pad précédemment pressé mais absent de ce report
                        // (slot devenu 0xFF / invalide) = relâché.
                        for (i, &was_seen) in seen.iter().enumerate() {
                            if !was_seen {
                                for a in tracker.absent(i, now) {
                                    send_pad_action(&tx, a, ts);
                                }
                            }
                        }
                        // pads relâchés mais plus présents dans le report ? Le firmware envoie seulement les actifs.
                        // Géré ci-dessus (release sur absence) + timeout 30ms si plus aucun 0x02.
                        // Debug : log les apparitions/disparitions (flicker vu/non-vu).
                        {
                            let mut changed = Vec::new();
                            for i in 0..16 {
                                if seen[i] != prev_seen[i] {
                                    changed.push(if seen[i] {
                                        format!("+{i}")
                                    } else {
                                        format!("-{i}")
                                    });
                                }
                            }
                            if !changed.is_empty() {
                                log::debug!("pads vus: {}", changed.join(" "));
                            }
                            prev_seen = seen;
                        }
                    }
                    Err(e) => {
                        log::warn!("decode 0x02 err: {e}");
                    }
                }
            }
            other => {
                log::debug!("hid unknown report 0x{other:02x} len {n}");
            }
        }

        // Sweep temporel à chaque itération (pas seulement sur poll-timeout) :
        // le release d'un pad ne doit pas rester coincé derrière du trafic 0x01.
        let ts = now_ns();
        for a in tracker.sweep(Instant::now()) {
            send_pad_action(&tx, a, ts);
        }

        // throttle
        let elapsed = last_poll.elapsed();
        if elapsed < cfg.poll_interval {
            std::thread::sleep(cfg.poll_interval - elapsed);
        }
        last_poll = Instant::now();
    }

    Ok(())
}

/// Envoie une action pad du tracker sur le channel (horodatée).
fn send_pad_action(tx: &Sender<HardwareEvent>, action: PadAction, ts_ns: u64) {
    let st = match action {
        PadAction::Press {
            pad,
            velocity,
            pressure,
        } => crate::events::PadState {
            pad,
            pressed: true,
            velocity,
            pressure,
            ts_ns,
        },
        PadAction::Release { pad, pressure } => crate::events::PadState {
            pad,
            pressed: false,
            velocity: 0,
            pressure,
            ts_ns,
        },
    };
    let _ = tx.send(HardwareEvent::Pad(st));
}

fn now_ns() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos() as u64
}

/// Dump synchrone bloquant pour debug CLI — lit N reports et affiche.
pub fn dump_hidraw_sync(path: PathBuf, duration: Duration) -> Result<()> {
    use std::os::unix::io::AsRawFd;
    let file = std::fs::OpenOptions::new().read(true).open(&path)?;
    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    unsafe {
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    let start = Instant::now();
    let mut buf = [0u8; 128];
    println!("# hid dump {} for {:?}", path.display(), duration);
    while start.elapsed() < duration {
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ret = unsafe { libc::poll(&mut pfd as *mut _, 1, 200) };
        if ret <= 0 {
            continue;
        }
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if n <= 0 {
            continue;
        }
        let n = n as usize;
        let data = &buf[..n];
        match data[0] {
            0x01 => {
                if let Ok(inp) = decode_report_01(data) {
                    if !inp.buttons.is_empty() {
                        println!("0x01 buttons: {:?}", inp.buttons);
                    }
                    if inp.encoders.iter().any(|e| e.value != 0) {
                        println!("0x01 encoders: {:?}", inp.encoders);
                    }
                }
            }
            0x02 => {
                if let Ok(pads) = decode_report_02(data) {
                    if !pads.is_empty() {
                        println!("0x02 pads: {:?}", pads);
                    }
                }
            }
            _ => println!("unknown {:02x?}", &data[..n.min(32)]),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xtalk_ghost_suppressed_but_flam_kept() {
        // Fantôme faible face à une vraie frappe : supprimé.
        assert!(xtalk_suppressed(20, 100, 0.5));
        assert!(xtalk_suppressed(49, 100, 0.5));
        // Flam intentionnel (vélocités proches) : conservé.
        assert!(!xtalk_suppressed(60, 100, 0.5));
        assert!(!xtalk_suppressed(100, 100, 0.5));
        // Ratio 0 = anti-diaphonie désactivée : tout passe.
        assert!(!xtalk_suppressed(1, 127, 0.0));
        // Pas de rival : tout passe.
        assert!(!xtalk_suppressed(1, 0, 0.5));
    }

    #[test]
    fn xtalk_rival_sees_simultaneously_armed_peers_only() {
        let mut peaks = [0u8; 16];
        peaks[3] = 100; // voisin armé fort
        peaks[5] = 18; // candidat fantôme
        let mut armed = [false; 16];
        armed[3] = true;
        armed[5] = true;
        // Trajectoires : le fort monte, le fantôme est plat.
        let mut pmin = [0u16; 16];
        let mut pmax = [0u16; 16];
        pmax[3] = 900;
        pmin[3] = 100;
        pmax[5] = 316;
        pmin[5] = 316;
        // Le fantôme voit le rival armé dans la même rafale.
        assert_eq!(xtalk_rival_peak(5, &peaks, &armed, &pmin, &pmax, 48), 100);
        // Le fort ne se supprime pas lui-même (rival = son propre pic exclu) ;
        // et le voisin plat n'est jamais rival.
        assert_eq!(xtalk_rival_peak(3, &peaks, &armed, &pmin, &pmax, 48), 0);
        assert!(!xtalk_suppressed(100, 0, 0.5));
        // Sans voisin armé : aucun rival, même avec une frappe récente
        // (l'historique ne doit jamais faire taire une note).
        let armed_none = [false; 16];
        let mut peaks2 = [0u8; 16];
        peaks2[5] = 18;
        assert_eq!(
            xtalk_rival_peak(5, &peaks2, &armed_none, &pmin, &pmax, 48),
            0
        );
    }

    // Harnais de rejeu : pilote le tracker avec du temps virtuel.
    struct Replay {
        tracker: PadTracker,
        start: Instant,
        presses: Vec<(u8, u8)>, // (pad, velocity)
        releases: Vec<u8>,
    }

    impl Replay {
        fn new() -> Self {
            let start = Instant::now();
            Self {
                tracker: PadTracker::new(start),
                start,
                presses: Vec::new(),
                releases: Vec::new(),
            }
        }

        fn at(&self, ms: u64) -> Instant {
            self.start + Duration::from_millis(ms)
        }

        fn feed(&mut self, actions: Vec<PadAction>) {
            for a in actions {
                match a {
                    PadAction::Press { pad, velocity, .. } => self.presses.push((pad, velocity)),
                    PadAction::Release { pad, .. } => self.releases.push(pad),
                }
            }
        }

        /// Pad présent dans un report à t (pression BRUTE, comme le firmware).
        /// Reproduit le pipeline de boucle : baseline puis hystérésis, puis
        /// sweep de fin d'itération (comme `hidraw_loop` à chaque tour).
        fn present(&mut self, t_ms: u64, idx: usize, raw: u16) {
            // Seuils par défaut : ON 0x4010 / OFF 0x4005.
            let adj = self.tracker.effective_pressure(idx, raw);
            let pressed = if self.tracker.is_pressed(idx) {
                adj >= 0x4005
            } else {
                adj >= 0x4010
            };
            let velocity = if pressed {
                (((adj - 0x4000) as f32 / 0x0FFD as f32) * 127.0).clamp(1.0, 127.0) as u8
            } else {
                0
            };
            let acts = self
                .tracker
                .present(idx, pressed, velocity, adj, self.at(t_ms));
            self.feed(acts);
            // Sweep de fin d'itération comme en production.
            let acts = self.tracker.sweep(self.at(t_ms));
            self.feed(acts);
        }

        /// Aucun report (silence) : sweep seul.
        fn idle(&mut self, t_ms: u64) {
            let acts = self.tracker.sweep(self.at(t_ms));
            self.feed(acts);
        }

        /// Report 0x02 vide (aucun pad vu) : absence pour tous + sweep.
        fn empty_report(&mut self, t_ms: u64) {
            let now = self.at(t_ms);
            for i in 0..16 {
                let acts = self.tracker.absent(i, now);
                self.feed(acts);
            }
            let acts = self.tracker.sweep(now);
            self.feed(acts);
        }
    }

    /// Scénario du log : appui maintenu en RAFALES plates entrecoupées de
    /// silences (firmware par rafales). Avant le fix : un PRESS par rafale
    /// (boucle). Après : un seul PRESS au total.
    #[test]
    fn replay_bursty_hold_emits_single_press() {
        let mut r = Replay::new();
        // Rafale 1 : montée 0x4481 -> pic, tenue ~plate (jitter ±8 < rise 48).
        for t in 0..12u64 {
            r.present(t, 0, 0x4481 + (t as u16 % 3) * 4);
        }
        r.idle(12); // flush -> PRESS #1
        assert_eq!(r.presses.len(), 1, "la 1re rafale doit jouer une fois");
        // Fin de rafale : silence 40ms -> timeout-release + gap.
        r.idle(52);
        assert!(r.releases.contains(&0));
        // Rafale 2 (même tenue, plate) : aucun nouveau press.
        for t in 55..67u64 {
            r.present(t, 0, 0x44A6 + (t as u16 % 3) * 4);
        }
        r.idle(67);
        // Rafale 3 idem.
        r.idle(100);
        for t in 103..115u64 {
            r.present(t, 0, 0x4490 + (t as u16 % 3) * 4);
        }
        r.idle(115);
        r.idle(150);
        assert_eq!(
            r.presses.len(),
            1,
            "l'appui maintenu en rafales ne doit jouer qu'une fois, presses={:?}",
            r.presses
        );
    }

    /// Capteur bloqué du log (pad 2 à 0x413C constant) : la baseline se cale
    /// dessus et le fantôme ne joue plus (au plus un blip avant convergence :
    /// flush 12ms < 32 lectures).
    #[test]
    fn replay_stuck_sensor_stays_quiet() {
        let mut r = Replay::new();
        for burst in 0..6u64 {
            let t0 = burst * 50;
            for t in t0..t0 + 12 {
                r.present(t, 2, 0x413C);
            }
            r.idle(t0 + 12);
            r.idle(t0 + 45);
        }
        assert!(
            r.presses.len() <= 1,
            "capteur bloqué : silence après convergence baseline, presses={:?}",
            r.presses
        );
    }

    /// Après convergence baseline, une VRAIE frappe sur le pad au capteur
    /// bloqué joue normalement (dynamique préservée, vélocité du delta).
    #[test]
    fn replay_stuck_pad_still_plays_real_hits() {
        let mut r = Replay::new();
        // 40 lectures constantes -> baseline calée à 0x413C.
        for t in 0..40u64 {
            r.present(t, 2, 0x413C);
        }
        r.idle(40);
        r.presses.clear();
        // Laisse le release se confirmer (sweeps continus en production).
        r.idle(60);
        // Vraie frappe : rampe 0x413C -> 0x4700.
        for (i, t) in (100..112u64).enumerate() {
            r.present(t, 2, 0x413C + (i as u16) * 120);
        }
        r.idle(113);
        assert_eq!(
            r.presses.len(),
            1,
            "la vraie frappe doit jouer : {:?}",
            r.presses
        );
        assert_eq!(r.presses[0].0, 2);
        assert!(r.presses[0].1 > 20, "vélocité du delta : {:?}", r.presses);
        // … et le fantôme restant (retour à 0x413C) reste muet.
        for t in 200..212u64 {
            r.present(t, 2, 0x413C);
        }
        r.idle(212);
        assert_eq!(r.presses.len(), 1, "le fantôme ne doit pas rejouer");
    }

    /// Vraies frappes successives (montée franche) : chacune joue,
    /// même rapprochées au-delà de la fenêtre de gap.
    #[test]
    fn replay_fresh_hits_each_play() {
        let mut r = Replay::new();
        // Frappe 1 : rampe 0x40CD -> 0x471B.
        for (i, t) in (0..12u64).enumerate() {
            r.present(t, 0, 0x40CD + (i as u16) * 90);
        }
        r.idle(12);
        assert_eq!(r.presses.len(), 1);
        // Levée explicite (report vide) + sweeps pour confirmer le release.
        r.empty_report(30);
        r.idle(50);
        assert_eq!(r.releases.len(), 1);
        // Frappe 2, 200ms plus tard : rejoue.
        for (i, t) in (200..212u64).enumerate() {
            r.present(t, 0, 0x40D0 + (i as u16) * 85);
        }
        r.idle(212);
        assert_eq!(r.presses.len(), 2, "deux vraies frappes = deux sons");
        // Vélocité = pic (pas la rampe initiale).
        assert!(r.presses[0].1 > 30, "pic capté : {:?}", r.presses);
    }

    /// Rebond : front haut 13ms après la frappe émise -> ignoré.
    #[test]
    fn replay_bounce_ignored() {
        let mut r = Replay::new();
        for (i, t) in (0..12u64).enumerate() {
            r.present(t, 0, 0x40CD + (i as u16) * 90);
        }
        r.idle(12);
        assert_eq!(r.presses.len(), 1);
        r.empty_report(15); // micro-coupure
                            // Rebond haut 13ms après le press : verrou anti-rebond 35ms.
        for t in 25..37u64 {
            r.present(t, 0, 0x4500);
        }
        r.idle(37);
        assert_eq!(r.presses.len(), 1, "le rebond ne doit pas rejouer");
    }

    /// Diaphonie : frappe forte + voisin faible simultanés -> seul le fort joue.
    #[test]
    fn replay_xtalk_ghost_suppressed() {
        let mut r = Replay::new();
        for (i, t) in (0..12u64).enumerate() {
            r.present(t, 0, 0x40CD + (i as u16) * 90); // fort, monte
            r.present(t, 2, 0x413C); // fantôme plat et faible
        }
        r.idle(12);
        assert_eq!(r.presses.len(), 1, "seul le fort joue : {:?}", r.presses);
        assert_eq!(r.presses[0].0, 0);
    }

    /// Diaphonie, ordre inverse : le fantôme (index bas) flushe AVANT le fort.
    /// Le fantôme plat ne devient jamais rival (trajectoire) : les deux
    /// décisions sont bonnes quel que soit l'ordre de flush.
    #[test]
    fn replay_xtalk_ghost_first_still_fine() {
        let mut r = Replay::new();
        for (i, t) in (0..12u64).enumerate() {
            r.present(t, 5, 0x40CD + (i as u16) * 90); // fort, monte
            r.present(t, 3, 0x413C); // fantôme plat et faible
        }
        r.idle(12);
        // Le fort joue dans tous les cas ; le fantôme plat peut blipper avant
        // convergence baseline, mais ne fait jamais taire le fort.
        assert!(
            r.presses.iter().any(|&(p, v)| p == 5 && v > 20),
            "le fort doit jouer : {:?}",
            r.presses
        );
    }

    /// Scénario du log pad 2 (44.009s+) : UNE pression lente dont la lecture
    /// oscille (rampe 0x4077→0x437F avec chutes à 0x4000 toutes les ~25ms).
    /// Avant : ~10 PRESS (chaque chute relâchait puis re-déclenchait).
    /// Après (release confirmé 15ms) : un seul PRESS, release à la vraie fin.
    #[test]
    fn replay_oscillating_hold_emits_single_press() {
        let mut r = Replay::new();
        let mut t = 0u64;
        let mut p = 0x4077u16;
        // 400ms de tenue : +90 toutes les 25ms, avec chute à 0x4000 entre deux.
        for _ in 0..16 {
            r.present(t, 2, p);
            r.present(t + 2, 2, 0x4000); // micro-coupure
            t += 25;
            p += 90;
        }
        // Vraie fin : chute persistante.
        for dt in 0..30u64 {
            r.idle(t + dt);
        }
        assert_eq!(
            r.presses.len(),
            1,
            "une tenue oscillante = un seul son, presses={:?}",
            r.presses
        );
        assert_eq!(r.releases.len(), 1, "un seul release en fin de tenue");
    }

    /// Un tap éclair joue quand même (flush au moment de la confirmation).
    #[test]
    fn replay_quick_tap_still_plays() {
        let mut r = Replay::new();
        for (i, t) in (0..8u64).enumerate() {
            r.present(t, 0, 0x40CD + (i as u16) * 60);
        }
        // Disparition à t=8 : pending, press flushé à t>=20, release confirmé
        // un peu après (délai adaptatif : ~27ms pour une tenue de 25ms).
        r.empty_report(8);
        assert_eq!(r.presses.len(), 0, "pas encore flushé");
        r.idle(25);
        assert_eq!(r.presses.len(), 1, "le tap éclair doit jouer");
        r.idle(45);
        assert_eq!(r.releases.len(), 1);
    }
}
