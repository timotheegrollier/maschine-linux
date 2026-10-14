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

#[cfg(feature = "midi-alsa")]
pub mod midi_io;

pub use device::{find_maschine_devices, DeviceInfo, MaschineDevice};
pub use events::{HardwareEvent, PadState};
pub use mapper::{HardwareMapper, MapperConfig};

/// Mapping pads hardware <-> logique (voir `maschine_proto::consts`).
///
/// - `Hw` (défaut) : le firmware numérote en scan haut->bas, on convertit vers
///   logique `0 = Pad 1 (bas-gauche)`. Avec la sortie LEDs `hw`, taper Pad N
///   allume Pad N.
/// - `Identity` : pas de conversion (firmwares exotiques). Réglez via
///   `MASCHINE_PAD_MAP=identity` si `pad-calibrate` montre raw == logique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PadMap {
    /// Scan hardware haut->bas (défaut).
    #[default]
    Hw,
    /// Identité (pas de remap).
    Identity,
}

impl PadMap {
    /// Lit depuis l'environnement (`MASCHINE_PAD_MAP=identity|hw`).
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var("MASCHINE_PAD_MAP")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "identity" | "ident" | "raw" | "0" => Self::Identity,
            _ => Self::Hw,
        }
    }

    /// Convertit un index décodé (déjà mappé `hw->logique` par le proto, table
    /// auto-inverse) vers le logique effectif selon ce mode.
    #[must_use]
    pub fn apply(self, proto_logical: u8) -> u8 {
        match self {
            Self::Hw => proto_logical,
            // La table est auto-inverse : ré-appliquer annule le remap proto.
            Self::Identity => maschine_proto::consts::hw_pad_to_logical(proto_logical),
        }
    }
}

/// Ordre des octets LEDs pads (`MASCHINE_PAD_OUTPUT=identity|hw`, défaut `hw`).
#[must_use]
pub fn pad_output_hw_order() -> bool {
    !matches!(
        std::env::var("MASCHINE_PAD_OUTPUT")
            .unwrap_or_default()
            .to_lowercase()
            .as_str(),
        "identity" | "ident" | "logical" | "0"
    )
}

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
