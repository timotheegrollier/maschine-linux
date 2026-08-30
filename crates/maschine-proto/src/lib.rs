//! `maschine-proto` — codec pur pour Maschine MK3 (17cc:1600)
//!
//! Zéro dépendance I/O / USB / ALSA. Tout est testable offline.
//!
//! - `hid_input` : parse reports `0x01` (boutons) et `0x02` (pads)
//! - `hid_output` : encode reports `0x80` (LEDs boutons) et `0x81` (pads + touchstrip)
//! - `display` : encode bulk RGB565 RLE pour les 2 écrans 480×272
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
    /// Report 0x80 : LEDs boutons (61 bytes payload + 1 byte ID)
    pub const HID_REPORT_80_SIZE: usize = 62;
    /// Report 0x81 : LEDs pads+strip (42 bytes payload + 1 byte ID)
    pub const HID_REPORT_81_SIZE: usize = 43;
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
}
