//! Palette indexée NI — mêmes valeurs que le firmware MK3.
//!
//! 17 couleurs + Off/Black. Les pads, Groups RGB et le touchstrip utilisent cette palette.
//! Les LEDs unicouleur utilisent `brightness` (0..3) séparément.

use serde::{Deserialize, Serialize};

/// Index palette NI (valeurs firmware).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PaletteIndex {
    /// Éteint / noir
    Off = 0,
    /// Rouge atténué
    DimRed = 5,
    /// Rouge
    Red = 6,
    /// Rose
    Rose = 7,
    /// Orange atténué
    DimOrange = 9,
    /// Orange
    Orange = 10,
    /// Peau
    Skin = 11,
    /// Ambre atténué
    DimAmber = 13,
    /// Ambre
    Amber = 14,
    /// Jaune atténué
    DimYellow = 21,
    /// Jaune
    Yellow = 22,
    /// Spring atténué
    DimSpring = 25,
    /// Spring
    Spring = 26,
    /// Turquoise atténué
    DimTurquoise = 27,
    /// Vert atténué
    DimGreen = 29,
    /// Vert
    Green = 30,
    /// Turquoise
    Turquoise = 31,
    /// Lime atténué
    DimLime = 33,
    /// Lime
    Lime = 34,
    /// Sky atténué
    DimSky = 37,
    /// Sky
    Sky = 38,
    /// Bleu
    Blue = 42,
    /// Bleu atténué
    DimBlue = 45,
    /// Violet
    Purple = 50,
    /// Violet atténué
    DimPurple = 53,
    /// Magenta atténué
    DimMagenta = 57,
    /// Magenta
    Magenta = 58,
    /// Rose atténué
    DimPink = 61,
    /// Rose vif
    Pink = 62,
    /// Gris foncé
    DarkGrey = 76,
    /// Gris
    Grey = 77,
    /// Blanc
    White = 78,
}

impl PaletteIndex {
    /// Toutes les valeurs valides.
    pub const ALL: &'static [u8] = &[
        0, 5, 6, 7, 9, 10, 11, 13, 14, 21, 22, 25, 26, 27, 29, 30, 31, 33, 34, 37, 38, 42, 45, 50,
        53, 57, 58, 61, 62, 76, 77, 78,
    ];

    /// Convertit un `u8` brut en `PaletteIndex` (ou Off si inconnu selon `strict`).
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Off),
            5 => Some(Self::DimRed),
            6 => Some(Self::Red),
            7 => Some(Self::Rose),
            9 => Some(Self::DimOrange),
            10 => Some(Self::Orange),
            11 => Some(Self::Skin),
            13 => Some(Self::DimAmber),
            14 => Some(Self::Amber),
            21 => Some(Self::DimYellow),
            22 => Some(Self::Yellow),
            25 => Some(Self::DimSpring),
            26 => Some(Self::Spring),
            27 => Some(Self::DimTurquoise),
            29 => Some(Self::DimGreen),
            30 => Some(Self::Green),
            31 => Some(Self::Turquoise),
            33 => Some(Self::DimLime),
            34 => Some(Self::Lime),
            37 => Some(Self::DimSky),
            38 => Some(Self::Sky),
            42 => Some(Self::Blue),
            45 => Some(Self::DimBlue),
            50 => Some(Self::Purple),
            53 => Some(Self::DimPurple),
            57 => Some(Self::DimMagenta),
            58 => Some(Self::Magenta),
            61 => Some(Self::DimPink),
            62 => Some(Self::Pink),
            76 => Some(Self::DarkGrey),
            77 => Some(Self::Grey),
            78 => Some(Self::White),
            _ => None,
        }
    }

    /// Vérifie si l'index est valide.
    #[must_use]
    pub fn is_valid(v: u8) -> bool {
        Self::from_u8(v).is_some()
    }

    /// RGB 8-bit pour affichage GUI (approximation).
    #[must_use]
    pub fn to_rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Off => (0, 0, 0),
            Self::DimRed => (64, 0, 0),
            Self::Red => (255, 0, 0),
            Self::Rose => (255, 64, 64),
            Self::DimOrange => (64, 32, 0),
            Self::Orange => (255, 128, 0),
            Self::Skin => (255, 192, 128),
            Self::DimAmber => (64, 32, 0),
            Self::Amber => (255, 176, 0),
            Self::DimYellow => (64, 64, 0),
            Self::Yellow => (255, 255, 0),
            Self::DimSpring => (32, 64, 0),
            Self::Spring => (128, 255, 0),
            Self::DimTurquoise => (0, 48, 48),
            Self::DimGreen => (0, 64, 0),
            Self::Green => (0, 255, 0),
            Self::Turquoise => (0, 255, 192),
            Self::DimLime => (32, 64, 32),
            Self::Lime => (128, 255, 64),
            Self::DimSky => (32, 48, 64),
            Self::Sky => (64, 192, 255),
            Self::Blue => (0, 64, 255),
            Self::DimBlue => (0, 0, 64),
            Self::Purple => (128, 0, 255),
            Self::DimPurple => (48, 0, 64),
            Self::DimMagenta => (64, 0, 64),
            Self::Magenta => (255, 0, 255),
            Self::DimPink => (64, 0, 32),
            Self::Pink => (255, 64, 128),
            Self::DarkGrey => (32, 32, 32),
            Self::Grey => (128, 128, 128),
            Self::White => (255, 255, 255),
        }
    }
}

/// Couleur haut niveau — soit palette indexée, soit RGB libre (converti en plus proche index).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color(pub PaletteIndex);

impl Color {
    /// Crée depuis un index brut (None si invalide).
    #[must_use]
    pub fn from_index(idx: u8) -> Option<Self> {
        PaletteIndex::from_u8(idx).map(Self)
    }

    /// Index brut firmware.
    #[must_use]
    pub fn to_index(self) -> u8 {
        self.0 as u8
    }

    /// Couleur depuis vélocité (0..127) pour feedback visuel.
    #[must_use]
    pub fn from_velocity(vel: u8) -> Self {
        let idx = match vel {
            0 => 0,
            1..=32 => 5,     // DimRed
            33..=64 => 10,   // Orange
            65..=96 => 22,   // Yellow
            97..=110 => 30,  // Green
            111..=127 => 78, // White
            _ => 0,
        };
        Self(PaletteIndex::from_u8(idx).unwrap())
    }

    /// Couleur éteinte.
    pub const OFF: Self = Self(PaletteIndex::Off);
    /// Rouge
    pub const RED: Self = Self(PaletteIndex::Red);
    /// Vert
    pub const GREEN: Self = Self(PaletteIndex::Green);
    /// Bleu
    pub const BLUE: Self = Self(PaletteIndex::Blue);
    /// Jaune
    pub const YELLOW: Self = Self(PaletteIndex::Yellow);
    /// Blanc
    pub const WHITE: Self = Self(PaletteIndex::White);
}

impl From<PaletteIndex> for Color {
    fn from(p: PaletteIndex) -> Self {
        Self(p)
    }
}

impl From<PaletteIndex> for u8 {
    fn from(p: PaletteIndex) -> Self {
        p as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_valid_roundtrip() {
        for &v in PaletteIndex::ALL {
            let p = PaletteIndex::from_u8(v).expect("valid");
            assert_eq!(p as u8, v);
        }
    }

    #[test]
    fn invalid_is_none() {
        assert!(PaletteIndex::from_u8(1).is_none());
        assert!(PaletteIndex::from_u8(99).is_none());
    }

    #[test]
    fn velocity_mapping() {
        assert_eq!(Color::from_velocity(0).to_index(), 0);
        assert_eq!(Color::from_velocity(127).to_index(), 78);
    }
}
