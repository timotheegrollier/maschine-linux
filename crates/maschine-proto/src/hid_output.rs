//! Encode les reports HID OUT `0x80` (boutons) et `0x81` (pads+strip).

use crate::color::Color;
use crate::error::{ProtoError, Result};

/// Luminosité LEDs unicouleur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LedBrightness {
    /// Éteint
    Off = 0,
    /// Dim
    Dim = 4,
    /// VeryDim
    VeryDim = 5,
    /// Allumé
    On = 6,
    /// Bright
    Bright = 7,
}

impl From<u8> for LedBrightness {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::Off,
            4 => Self::Dim,
            5 => Self::VeryDim,
            6 => Self::On,
            7 => Self::Bright,
            _ if v == 0 => Self::Off,
            _ if v < 3 => Self::Dim,
            _ => Self::On,
        }
    }
}

/// État complet LEDs — utilisé par le mapper pour construire les reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedState {
    /// LEDs boutons unicouleur / RGB. Index selon `BUTTON_LED_OFFSETS`.
    pub button_leds: [u8; 61],
    /// Pads RGB 0..15 (index palette)
    pub pad_leds: [u8; 16],
    /// Touchstrip 25 LEDs RGB
    pub strip_leds: [u8; 25],
}

impl Default for LedState {
    fn default() -> Self {
        Self {
            button_leds: [0; 61],
            pad_leds: [0; 16],
            strip_leds: [0; 25],
        }
    }
}

impl LedState {
    /// Crée tout éteint.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Définit un pad.
    pub fn set_pad(&mut self, pad: usize, color: Color) {
        if pad < 16 {
            self.pad_leds[pad] = color.to_index();
        }
    }

    /// Définit tous les pads.
    pub fn set_all_pads(&mut self, color: Color) {
        self.pad_leds.fill(color.to_index());
    }

    /// Définit une LED bouton par offset (1..61).
    pub fn set_button_raw(&mut self, offset: usize, value: u8) {
        if (1..=61).contains(&offset) {
            self.button_leds[offset - 1] = value;
        }
    }

    /// Touchstrip LED.
    pub fn set_strip(&mut self, idx: usize, color: Color) {
        if idx < 25 {
            self.strip_leds[idx] = color.to_index();
        }
    }

    /// Remplit le strip avec une couleur.
    pub fn fill_strip(&mut self, color: Color) {
        self.strip_leds.fill(color.to_index());
    }
}

/// Alias pour compat.
pub type PadLed = Color;
/// Alias.
pub type ButtonLed = u8;

// ---------------------------------------------------------------------------
// Offsets — d'après cabl doc/HIDOutput.md (1-indexed dans doc, 0-indexed ici)
// ---------------------------------------------------------------------------

/// Offsets boutons dans report 0x80 (après byte ID). 0-indexed array position.
pub mod offsets {
    /// Channel/MIDI unicolore
    pub const CHANNEL_MIDI: usize = 1;
    pub const PLUGIN: usize = 2;
    pub const ARRANGER: usize = 3;
    pub const MIXER: usize = 4;
    /// Browser RGB
    pub const BROWSER: usize = 5;
    pub const SAMPLING: usize = 6;
    pub const LEFT: usize = 7;
    pub const RIGHT: usize = 8;
    pub const FILE_SAVE: usize = 9;
    pub const SETTINGS: usize = 10;
    pub const AUTO: usize = 11;
    pub const MACRO: usize = 12;
    /// Display Buttons 1..8 (offsets 12..19)
    pub const DISPLAY_BTN_1: usize = 12;
    pub const DISPLAY_BTN_2: usize = 13;
    pub const DISPLAY_BTN_3: usize = 14;
    pub const DISPLAY_BTN_4: usize = 15;
    pub const DISPLAY_BTN_5: usize = 16;
    pub const DISPLAY_BTN_6: usize = 17;
    pub const DISPLAY_BTN_7: usize = 18;
    pub const DISPLAY_BTN_8: usize = 19;
    pub const VOLUME: usize = 20;
    pub const SWING: usize = 21;
    pub const NOTE_REPEAT: usize = 22;
    pub const TEMPO: usize = 23;
    pub const LOCK: usize = 24;
    pub const PITCH: usize = 25;
    pub const MOD: usize = 26;
    pub const PERFORM: usize = 27;
    pub const NOTES: usize = 28;
    /// Groups A..H RGB (offsets 29..36)
    pub const GROUP_A: usize = 29;
    pub const GROUP_B: usize = 30;
    pub const GROUP_C: usize = 31;
    pub const GROUP_D: usize = 32;
    pub const GROUP_E: usize = 33;
    pub const GROUP_F: usize = 34;
    pub const GROUP_G: usize = 35;
    pub const GROUP_H: usize = 36;
    pub const RESTART: usize = 37;
    pub const ERASE: usize = 38;
    pub const TAP: usize = 39;
    pub const FOLLOW: usize = 40;
    pub const PLAY: usize = 41;
    pub const REC: usize = 42;
    pub const STOP: usize = 43;
    pub const SHIFT: usize = 44;
    pub const FIXED_VEL: usize = 45;
    pub const PAD_MODE: usize = 46;
    pub const KEYBOARD: usize = 47;
    pub const CHORDS: usize = 48;
    pub const STEP: usize = 49;
    pub const SCENE: usize = 50;
    pub const PATTERN: usize = 51;
    pub const EVENTS: usize = 52;
    pub const VARIATION: usize = 53;
    pub const DUPLICATE: usize = 54;
    pub const SELECT: usize = 55;
    pub const SOLO: usize = 56;
    pub const MUTE: usize = 57;
    pub const FOUR_WAY_UP: usize = 58;
    pub const FOUR_WAY_RIGHT: usize = 59;
    pub const FOUR_WAY_DOWN: usize = 60;
    pub const FOUR_WAY_LEFT: usize = 61;
}

// ---------------------------------------------------------------------------
// Encoders
// ---------------------------------------------------------------------------

/// Encode le report `0x80` (boutons) — retourne `Vec<u8>` de taille 62 (ID + 61).
pub fn encode_report_80(state: &LedState) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(crate::consts::HID_REPORT_80_SIZE);
    out.push(0x80);
    out.extend_from_slice(&state.button_leds);
    // 61 bytes already, need 61 → out len 62 ok. Doc dit 61 payload → 62 total.
    // Certains implémentent 62 payload ; on pad à 61 si besoin
    if out.len() < crate::consts::HID_REPORT_80_SIZE {
        out.resize(crate::consts::HID_REPORT_80_SIZE, 0);
    }
    Ok(out)
}

/// Encode le report `0x81` (strip + pads) — retourne `Vec<u8>` de taille 43 (ID + 42).
pub fn encode_report_81(state: &LedState) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(crate::consts::HID_REPORT_81_SIZE);
    out.push(0x81);
    // 25 strip + 1 spare (0) + 16 pads = 42. On map strip 0..24 -> 1..25, pad 0..15 -> 27..42
    // Doc: bytes 1-26 strip, 27-42 pads (hardware order 13-16,9-12,5-8,1-4)
    // Pour MVP on map linéaire pad 0..15 -> 27..42
    for &c in &state.strip_leds {
        out.push(c);
    }
    // pad 26 spare qu'on met à 0 pour aligner à 26
    // mais on a déjà 25, on pad 1 byte à 0
    out.push(0);
    // hardware order remap : logique pad 0 = bottom-left hardware 0, mais doc hardware order = 13,9,5,1...
    // On convertit : logique -> hardware
    let remap = [12, 13, 14, 15, 8, 9, 10, 11, 4, 5, 6, 7, 0, 1, 2, 3];
    // en fait hardware order list [13,14,15,16,9,10,11,12,5,6,7,8,1,2,3,4] 1-indexed → 0-indexed [12,13,14,15,8,9,10,11,4,5,6,7,0,1,2,3]
    // On veut envoyer dans hardware order, donc pour chaque position hardware on met la couleur du pad logique
    // Simplification : inverse — on émet dans hardware order
    for &hw in &remap {
        out.push(state.pad_leds[hw]);
    }
    if out.len() != crate::consts::HID_REPORT_81_SIZE {
        return Err(ProtoError::InvalidSize {
            report_id: 0x81,
            expected: crate::consts::HID_REPORT_81_SIZE,
            got: out.len(),
        });
    }
    Ok(out)
}

/// Helper : encode les deux reports d'un coup.
pub fn encode_all_leds(state: &LedState) -> Result<(Vec<u8>, Vec<u8>)> {
    Ok((encode_report_80(state)?, encode_report_81(state)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::PaletteIndex;

    #[test]
    fn report_80_size() {
        let s = LedState::new();
        let r = encode_report_80(&s).unwrap();
        assert_eq!(r[0], 0x80);
        assert_eq!(r.len(), crate::consts::HID_REPORT_80_SIZE);
    }

    #[test]
    fn report_81_size_and_remap() {
        let mut s = LedState::new();
        s.set_pad(0, Color(PaletteIndex::Red)); // logique pad 0 = bottom-left
        s.set_pad(15, Color(PaletteIndex::Blue));
        let r = encode_report_81(&s).unwrap();
        assert_eq!(r[0], 0x81);
        assert_eq!(r.len(), crate::consts::HID_REPORT_81_SIZE);
        // pads start at r[27] after 25 strip + 1 spare, remap[12]=0 → r[27+12]=r[39] = pad 0
        assert_eq!(r[39], PaletteIndex::Red as u8);
        // pad 15 hw pos 3 → r[30]
        assert_eq!(r[30], PaletteIndex::Blue as u8);
    }

    #[test]
    fn button_led_set() {
        let mut s = LedState::new();
        s.set_button_raw(offsets::SHIFT, LedBrightness::Bright as u8);
        let r = encode_report_80(&s).unwrap();
        assert_eq!(r[offsets::SHIFT], LedBrightness::Bright as u8);
    }
}
