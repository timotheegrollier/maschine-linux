//! Bulk display via `rusb` (libusb) — optionnel, feature `usb`.
//!
//! Mock si pas de hardware. Gère IF5 Vendor Bulk EP 0x04.

#[cfg(feature = "usb")]
use rusb::{Context, DeviceHandle, UsbContext};

use crate::{HardwareError, Result};
use maschine_proto::display::{encode_bulk_transfer, DisplayFrame};

// ── Constantes du paquet fenêtre MK3 ────────────────────────────────────
// Sortie bulk des écrans (endpoint OUT de l'interface 5).
#[cfg(feature = "usb")]
const EP_DISPLAY: u8 = 0x04;
/// Header fenêtre : 16 octets (magic, display, x/y/w/h).
#[cfg(feature = "usb")]
const PKT_HEADER: usize = 16;
/// Commandes : transmit (4) + blit (4) + end (4) = 12.
#[cfg(feature = "usb")]
const PKT_CMDS: usize = 12;
/// Taille du remplissage de purge après timeout (couvre toute fenêtre < 512 Ko).
#[cfg(feature = "usb")]
const PURGE_BYTES: usize = 131_072;

/// Épilogue d'un paquet fenêtre interrompu : `px_rest` octets de noirs (reste
/// du flux de pixels attendu par le MCU) puis `blit` (0x03) + `end` (0x40).
/// Fonction pure, testée : garantit l'alignement exact du parseur du MCU.
#[cfg(feature = "usb")]
pub(crate) fn packet_tail(px_rest: usize) -> Vec<u8> {
    let mut tail = vec![0u8; px_rest];
    tail.extend_from_slice(&[0x03, 0x00, 0x00, 0x00]);
    tail.extend_from_slice(&[0x40, 0x00, 0x00, 0x00]);
    tail
}

/// Handle bulk display.
#[cfg(feature = "usb")]
pub struct BulkHandle {
    handle: DeviceHandle<Context>,
    iface: u8,
}

#[cfg(feature = "usb")]
impl BulkHandle {
    /// Ouvre le bulk display (IF5) pour la MK3.
    pub fn open() -> Result<Self> {
        let ctx = Context::new().map_err(|e| HardwareError::Usb(format!("rusb context: {e}")))?;
        let devices = ctx
            .devices()
            .map_err(|e| HardwareError::Usb(format!("devices: {e}")))?;
        for device in devices.iter() {
            let desc = device
                .device_descriptor()
                .map_err(|e| HardwareError::Usb(format!("{e}")))?;
            if desc.vendor_id() == maschine_proto::consts::VENDOR_ID
                && desc.product_id() == maschine_proto::consts::PRODUCT_ID_MK3
            {
                let handle = device
                    .open()
                    .map_err(|e| HardwareError::Usb(format!("open: {e}")))?;
                let iface = 5u8;
                let _ = handle.set_auto_detach_kernel_driver(true);
                handle
                    .claim_interface(iface)
                    .map_err(|e| HardwareError::Usb(format!("claim IF5: {e}")))?;
                return Ok(Self { handle, iface });
            }
        }
        Err(HardwareError::NotFound(
            "MK3 bulk IF5 non trouvé".to_string(),
        ))
    }

    /// Envoie une frame (complète ou fenêtre partielle).
    ///
    /// Retourne le nombre d'octets effectivement écrits. En cas d'erreur, le
    /// pipe peut être bloqué : voir [`BulkHandle::clear_stall`] et
    /// [`BulkHandle::reacquire`].
    ///
    /// **Critique** : le protocole est un flux de commandes MCU (header fenêtre,
    /// puis `transmit`, pixels, `blit`, `end`). Une écriture interrompue laisse
    /// le parseur en attente du reste exact des pixels du paquet interrompu ;
    /// sans complétion, tout envoi suivant est décalé d'autant et les écrans
    /// se figent ou se brouillent définitivement. On complète donc le paquet avec
    /// ses propres octets restants, et à défaut (position inconnue après timeout)
    /// par une purge de noirs plus `blit` plus `end`.
    pub fn send_frame(&mut self, frame: &DisplayFrame) -> Result<usize> {
        let data = encode_bulk_transfer(frame).map_err(|e| HardwareError::Other(format!("{e}")))?;
        let total = data.len();
        // EP 0x04 OUT — un seul transfert bulk (libusb découpe en paquets USB).
        // Référence : ni-controllers-lib transferOut(endpoint, buffer).
        // Timeout court pour les petites fenêtres (récupération plus rapide).
        let timeout = if total > 100_000 {
            std::time::Duration::from_millis(1000)
        } else {
            std::time::Duration::from_millis(250)
        };
        let mut written = 0usize;
        // Jusqu'à 3 essais : on renvoie TOUJOURS les octets restants du MÊME
        // paquet, jamais le paquet suivant (qui serait consommé comme pixels).
        for attempt in 0..3u32 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            if written >= total {
                break;
            }
            match self
                .handle
                .write_bulk(EP_DISPLAY, &data[written..], timeout)
            {
                Ok(n) => {
                    written += n;
                    if written >= total {
                        return Ok(total);
                    }
                    // Écriture partielle sans erreur (le MCU a cessé d'accepter
                    // en cours) : on continue avec le reste de ce même paquet.
                }
                Err(e) => {
                    // Timeout/pipe : la position du parseur MCU est inconnue →
                    // on purge la fenêtre en cours (timeout court : best-effort),
                    // puis on remonte l'erreur (l'appelant invalide la vue →
                    // frame pleine ressynchronisée).
                    let _ = self.purge_stream(std::time::Duration::from_millis(60));
                    return Err(HardwareError::Usb(format!("bulk write: {e}")));
                }
            }
        }
        if written == 0 {
            // Rien de consommé : le parseur MCU est toujours aligné. On rend
            // l'erreur telle quelle (l'appelant invalide → frame pleine).
            return Err(HardwareError::Usb(format!(
                "bulk write: rien transmis (0/{total})"
            )));
        }
        if written >= PKT_HEADER + 4 {
            // On complète le paquet interrompu : noirs pour le reste des pixels,
            // puis blit + end → le parseur MCU est exactement réalé à la fin du
            // paquet. L'appelant invalide la vue pour une frame pleine ensuite.
            let pixels_total = total - PKT_HEADER - PKT_CMDS;
            let px_written = (written - PKT_HEADER - 4).min(pixels_total);
            let px_rest = pixels_total - px_written;
            let tail = packet_tail(px_rest);
            let _ = self.handle.write_bulk(EP_DISPLAY, &tail, timeout);
        }
        Err(HardwareError::Usb(format!(
            "bulk short write {written}/{total} (paquet complété)"
        )))
    }

    /// Purge de la fenêtre MCU en cours (après un timeout dont la position
    /// d'avancement est inconnue) : on déborde le compteur de pixels du paquet
    /// interrompu avec du noir (tous les octets sont consommés comme pixels ou
    /// comme commandes « transmit 0 », sans effet visuel), puis on termine par
    /// `blit` + `end` pour remettre le parseur en attente d'un nouveau paquet.
    fn purge_stream(&mut self, timeout: std::time::Duration) -> Result<usize> {
        // Purge asymétrique : gros volumes de zéros (compteur de pixels du
        // paquet interrompu dépassé) puis paires blit/end en doublePAGE pour
        // couvrir un décalage de 4 octets et moins.
        let mut buf = vec![0u8; PURGE_BYTES + 8];
        buf[PURGE_BYTES..PURGE_BYTES + 4].copy_from_slice(&[0x03, 0x00, 0x00, 0x00]);
        buf[PURGE_BYTES + 4..PURGE_BYTES + 8].copy_from_slice(&[0x40, 0x00, 0x00, 0x00]);
        self.handle
            .write_bulk(EP_DISPLAY, &buf, timeout)
            .map_err(|e| HardwareError::Usb(format!("purge: {e}")))
    }

    /// Efface un éventuel « halt » sur l'endpoint des écrans.
    pub fn clear_stall(&mut self) {
        let _ = self.handle.clear_halt(0x04);
    }

    /// Ré-acquiert l'interface IF5 sans reset USB complet (récupération après
    /// plusieurs écritures en échec).
    pub fn reacquire(&mut self) -> Result<()> {
        let _ = self.handle.release_interface(self.iface);
        std::thread::sleep(std::time::Duration::from_millis(60));
        self.handle
            .claim_interface(self.iface)
            .map_err(|e| HardwareError::Usb(format!("re-claim IF5: {e}")))?;
        self.clear_stall();
        Ok(())
    }
}

#[cfg(feature = "usb")]
impl Drop for BulkHandle {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(self.iface);
        std::thread::sleep(std::time::Duration::from_millis(100));
        let _ = self.handle.reset();
        std::thread::sleep(std::time::Duration::from_millis(200));
        let _ = self.handle.attach_kernel_driver(self.iface);
    }
}

/// Stub sans feature `usb`.
#[cfg(not(feature = "usb"))]
pub struct BulkHandle;

#[cfg(not(feature = "usb"))]
impl BulkHandle {
    /// Ouvre — erreur si feature non activée.
    pub fn open() -> Result<Self> {
        Err(HardwareError::Other(
            "feature `usb` non activée — compile avec --features usb + installe libusb1-devel"
                .to_string(),
        ))
    }

    /// Envoie — stub.
    pub fn send_frame(&mut self, _frame: &DisplayFrame) -> Result<usize> {
        Err(HardwareError::Other("bulk not available".to_string()))
    }

    /// Efface un halt — stub.
    pub fn clear_stall(&mut self) {}

    /// Ré-acquiert — stub.
    pub fn reacquire(&mut self) -> Result<()> {
        Err(HardwareError::Other("bulk not available".to_string()))
    }
}

#[cfg(all(test, feature = "usb"))]
mod tests {
    use super::*;

    /// L'épilogue complète exactement le reste d'un paquet fenêtre interrompu :
    /// px_rest octets de noirs, puis blit (0x03) et end (0x40).
    #[test]
    fn packet_tail_terminates_interrupted_window() {
        // Fenêtre de 10588 B interrompue après 7680 B (cas réel des logs) :
        // header 16 + cmd 4 = 20 consommés, 7660 octets de pixels consommés
        // sur 10560 → reste 2900 pixels + 8 octets de commandes.
        let total = 10588usize;
        let written = 7680usize;
        let pixels_total = total - PKT_HEADER - PKT_CMDS;
        let px_written = (written - PKT_HEADER - 4).min(pixels_total);
        assert_eq!(px_written, 7660);
        let px_rest = pixels_total - px_written;
        assert_eq!(px_rest, 2900);
        let tail = packet_tail(px_rest);
        assert_eq!(tail.len(), px_rest + 8);
        assert!(tail[..px_rest].iter().all(|&b| b == 0));
        assert_eq!(&tail[px_rest..px_rest + 4], &[0x03, 0x00, 0x00, 0x00]);
        assert_eq!(&tail[px_rest + 4..], &[0x40, 0x00, 0x00, 0x00]);
        // 7680 (déjà envoyés) + 2908 (complétion) = paquet complet vu du MCU.
        assert_eq!(written + tail.len(), total);
    }

    /// Une frame complète interrompue en plein milieu se termine pareil.
    #[test]
    fn packet_tail_full_frame_completion_math() {
        let total = 261_148usize;
        let written = 131_000usize;
        let pixels_total = total - PKT_HEADER - PKT_CMDS;
        assert_eq!(pixels_total, 261_120);
        let px_written = written - PKT_HEADER - 4;
        let px_rest = pixels_total - px_written;
        assert_eq!(px_written, 130_980);
        assert_eq!(px_rest, 130_140);
        let tail = packet_tail(px_rest);
        assert_eq!(written + tail.len(), total);
    }
}
