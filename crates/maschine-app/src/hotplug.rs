//! Gestionnaire Hotplug & Plug & Play pour contrôleurs Maschine MK3.
//!
//! Permet de :
//! 1. Détecter automatiquement le contrôleur au démarrage ou lors d'un branchement USB à chaud ;
//! 2. Réagir aux déconnexions (câble retiré) sans crasher l'application ni fermer le thread ;
//! 3. Ré-initialiser automatiquement les threads HID, LEDs et écrans dès la reconnexion.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[cfg(feature = "usb")]
use log::warn;
use log::{error, info};
use parking_lot::Mutex;

use maschine_engine::Engine;
use maschine_hardware::device::{find_maschine_devices, DeviceInfo};
use maschine_hardware::events::HardwareEvent;
use maschine_hardware::hidraw::{spawn_hidraw_thread, HidrawConfig, HidrawHandle};
use maschine_proto::hid_output::LedState;

use crate::hw_output::{spawn_led_writer, LedWriterHandle};

/// Session matérielle active pour un contrôleur connecté.
pub struct HardwareSession {
    pub hidraw_path: PathBuf,
    pub hid_handle: Option<HidrawHandle>,
    pub led_handle: Option<LedWriterHandle>,
    #[cfg(feature = "usb")]
    pub display_handle: Option<crate::display_thread::DisplayThreadHandle>,
}

impl HardwareSession {
    /// Initialise les flux matériels (HID input, LED writer, Display bulk).
    pub fn start(
        device: DeviceInfo,
        hardware_tx: crossbeam_channel::Sender<HardwareEvent>,
        led_rx: crossbeam_channel::Receiver<LedState>,
        engine: Arc<Mutex<Engine>>,
        display_activity: Arc<AtomicBool>,
        no_led: bool,
    ) -> anyhow::Result<Self> {
        let hidraw_path = device
            .hidraw_path
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Contrôleur détecté sans chemin hidraw"))?;

        info!(
            "Démarrage session matérielle Maschine MK3 sur {} (serial={:?})",
            hidraw_path.display(),
            device.serial
        );

        let hid_handle = spawn_hidraw_thread(device, HidrawConfig::default(), hardware_tx)?;

        let led_handle = if !no_led {
            match spawn_led_writer(hidraw_path.clone(), led_rx) {
                Ok(h) => Some(h),
                Err(e) => {
                    error!("Échec démarrage LED writer: {e}");
                    None
                }
            }
        } else {
            None
        };

        #[cfg(feature = "usb")]
        let display_handle =
            match crate::display_thread::spawn_display_thread(engine, display_activity) {
                Ok(h) => {
                    info!("✓ Thread affichage écran MK3 démarré en Plug & Play");
                    Some(h)
                }
                Err(e) => {
                    warn!("Thread affichage écran indisponible: {e}");
                    None
                }
            };

        #[cfg(not(feature = "usb"))]
        let _ = (engine, display_activity);

        Ok(Self {
            hidraw_path,
            hid_handle: Some(hid_handle),
            led_handle,
            #[cfg(feature = "usb")]
            display_handle,
        })
    }

    /// Arrête proprement tous les flux du contrôleur.
    pub fn stop(&mut self) {
        if let Some(h) = self.hid_handle.take() {
            h.stop();
        }
        if let Some(h) = self.led_handle.take() {
            h.stop();
        }
        #[cfg(feature = "usb")]
        if let Some(h) = self.display_handle.take() {
            h.join();
        }
    }
}

impl Drop for HardwareSession {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Cherche le premier device Maschine MK3 / Mikro MK3 disponible avec hidraw.
pub fn probe_available_device() -> Option<DeviceInfo> {
    match find_maschine_devices() {
        Ok(devices) => devices.into_iter().find(|d| d.hidraw_path.is_some()),
        Err(_) => None,
    }
}
