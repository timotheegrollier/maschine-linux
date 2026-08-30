//! `maschine-hardware` — couche USB/MIDI pour MK3.
//!
//! Stratégie :
//! - `hidraw` d'abord (`/dev/hidraw*` via `hid-generic`, pas de detach) — hotplug, pas de root.
//! - Fallback `rusb`/`nusb` pour le bulk display (IF5) et pour HID si hidraw non accessible.
//! - Bridge ALSA Seq virtuel (et optionnellement JACK) via `midir`/`alsa`.
//! - Threads temps-réel + `crossbeam-channel` bounded.

#![allow(missing_docs)]

pub mod device;
pub mod events;
pub mod hidraw;
pub mod mapper;

#[cfg(feature = "usb")]
pub mod usb;

pub use device::{find_maschine_devices, DeviceInfo, MaschineDevice};
pub use events::{HardwareEvent, PadState};
pub use mapper::{HardwareMapper, MapperConfig};

/// Erreurs hardware.
#[derive(thiserror::Error, Debug)]
pub enum HardwareError {
    /// I/O
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),

    /// Proto
    #[error("proto: {0}")]
    Proto(#[from] maschine_proto::error::ProtoError),

    /// Device non trouvé
    #[error("device non trouvé: {0}")]
    NotFound(String),

    /// Permission
    #[error("permission hidraw/bulk: {0} — vérifie udev 99-maschine-mk3.rules + TAG+=\"uaccess\" + relogin")]
    Permission(String),

    /// USB
    #[error("USB: {0}")]
    Usb(String),

    /// MIDI
    #[error("MIDI: {0}")]
    Midi(String),

    /// Autre
    #[error("{0}")]
    Other(String),
}

/// Résultat hardware.
pub type Result<T> = std::result::Result<T, HardwareError>;
