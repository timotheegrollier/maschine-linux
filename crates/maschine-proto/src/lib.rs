//! `maschine-proto` — codec pur pour Maschine MK3 (17cc:1600)
//!
//! Zéro dépendance I/O / USB / ALSA. Tout est testable offline.
//!
//! - `hid_input` : parse reports `0x01` (boutons) et `0x02` (pads)
//! - `hid_output` : encode reports `0x80` (LEDs boutons) et `0x81` (pads + touchstrip)
//! - `display` : encode bulk RGB565 raw par fenêtres pour les 2 écrans 480×272
//! - `color` : palette indexée NI

#![allow(missing_docs)]
#![forbid(unsafe_code)]

pub mod color;
pub mod display;
pub mod error;
pub mod hid_input;
pub mod hid_output;

pub use color::{Color, PaletteIndex};
pub use display::{encode_bulk_transfer, DisplayFrame, DisplayId, DisplayPixel};
pub use error::{ProtoError, Result};
pub use hid_input::{
    ButtonEvent, ButtonId, EncoderEvent, HidInput, KnobId, PadEvent, TouchStripEvent,
};
pub use hid_output::{ButtonLed, LedBrightness, LedState, PadLed};

/// Taille attendue des reports HID.
pub mod consts {
    /// Report 0x01 : boutons + encodeurs + pots (42 bytes payload + 1 byte ID)
    pub const HID_REPORT_01_SIZE: usize = 42;
    /// Report 0x02 : pads pression (64 bytes payload + 1 byte ID)
    pub const HID_REPORT_02_SIZE: usize = 64;
    /// Report 0x80 : LEDs boutons (62 bytes payload + 1 byte ID).
    /// HID descriptor : report count 0x3E = 62 bytes payload.
    pub const HID_REPORT_80_SIZE: usize = 63;
    /// Report 0x81 : LEDs pads+strip (41 bytes payload + 1 byte ID).
    /// HID descriptor : report count 0x29 = 41 = 25 strip + 16 pads.
    pub const HID_REPORT_81_SIZE: usize = 42;
    /// Bulk display : 480×272 RGB565 = 261120 bytes + header 32
    pub const DISPLAY_WIDTH: usize = 480;
    /// Hauteur écran MK3
    pub const DISPLAY_HEIGHT: usize = 272;
    /// Taille image brute RGB565
    pub const DISPLAY_RAW_SIZE: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT * 2;
    /// VID NI
    pub const VENDOR_ID: u16 = 0x17cc;
    /// PID MK3
    pub const PRODUCT_ID_MK3: u16 = 0x1600;
    /// PID Mikro MK3
    pub const PRODUCT_ID_MIKRO_MK3: u16 = 0x1700;
    /// Seuils pression
    pub const PAD_ON_THRESHOLD: u16 = 0x4010;
    /// Seuil relâchement
    pub const PAD_OFF_THRESHOLD: u16 = 0x4005;

    /// Parse un seuil depuis l'env (`0x4040` hex ou `16448` décimal absolu).
    /// Retourne `None` si absent/invalide (le défaut s'applique).
    fn parse_threshold_env(name: &str) -> Option<u16> {
        let s = std::env::var(name).ok()?;
        let s = s.trim();
        if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            u16::from_str_radix(hex, 16).ok()
        } else {
            s.parse::<u16>().ok()
        }
    }

    /// Seuil ON effectif (défaut `PAD_ON_THRESHOLD`, surchargeable via
    /// `MASCHINE_PAD_ON`, ex. `MASCHINE_PAD_ON=0x4040` si les capteurs sont
    /// trop sensibles et déclenchent les voisins par vibration).
    #[must_use]
    pub fn pad_on_threshold() -> u16 {
        static CACHED: std::sync::OnceLock<u16> = std::sync::OnceLock::new();
        *CACHED.get_or_init(|| parse_threshold_env("MASCHINE_PAD_ON").unwrap_or(PAD_ON_THRESHOLD))
    }

    /// Seuil OFF effectif (défaut `PAD_OFF_THRESHOLD`, surchargeable via
    /// `MASCHINE_PAD_OFF`). Doit rester < seuil ON pour l'hystérésis.
    #[must_use]
    pub fn pad_off_threshold() -> u16 {
        static CACHED: std::sync::OnceLock<u16> = std::sync::OnceLock::new();
        *CACHED.get_or_init(|| parse_threshold_env("MASCHINE_PAD_OFF").unwrap_or(PAD_OFF_THRESHOLD))
    }

    /// Mapping pads hardware <-> logique.
    ///
    /// Logique Maschine (celle du soft, des Groups/Patterns, des écrans) :
    /// `0 = Pad 1 (bas-gauche)` ... `15 = Pad 16 (haut-droite)`.
    ///
    /// Le firmware numérote les pads en ordre hardware (scan haut -> bas) :
    /// position 0 = Pad 13 (haut-gauche, logique 12), ..., position 12 = Pad 1.
    /// L'ordre des octets LED 0x81 est `13-16,9-12,5-8,1-4` (haut -> bas).
    ///
    /// La permutation est auto-inverse (échanges de blocs de 4) :
    /// `[12,13,14,15, 8,9,10,11, 4,5,6,7, 0,1,2,3]`.
    pub const HW_PAD_TO_LOGICAL: [u8; 16] = [12, 13, 14, 15, 8, 9, 10, 11, 4, 5, 6, 7, 0, 1, 2, 3];

    /// Inverse (identique ici car auto-inverse, gardé explicite pour lisibilité).
    pub const LOGICAL_TO_HW_PAD: [u8; 16] = [12, 13, 14, 15, 8, 9, 10, 11, 4, 5, 6, 7, 0, 1, 2, 3];

    /// Convertit un index pad hardware (0..15, scan haut->bas) en logique (0=Pad1).
    #[must_use]
    pub fn hw_pad_to_logical(hw: u8) -> u8 {
        if hw < 16 {
            HW_PAD_TO_LOGICAL[hw as usize]
        } else {
            hw
        }
    }

    /// Convertit un index logique (0=Pad1) en position hardware pour les LEDs.
    #[must_use]
    pub fn logical_to_hw_pad(logical: u8) -> u8 {
        if logical < 16 {
            LOGICAL_TO_HW_PAD[logical as usize]
        } else {
            logical
        }
    }
}
