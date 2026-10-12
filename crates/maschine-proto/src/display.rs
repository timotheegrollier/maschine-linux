//! Bulk display — 2× 480×272 RGB565 + RLE encode.

use crate::error::{ProtoError, Result};

/// Identifiant écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DisplayId {
    /// Gauche
    Left = 0x00,
    /// Droite
    Right = 0x01,
}

impl DisplayId {
    fn from_u8(v: u8) -> Result<Self> {
        match v {
            0x00 => Ok(Self::Left),
            0x01 => Ok(Self::Right),
            x => Err(ProtoError::InvalidDisplay(x)),
        }
    }
}

/// Pixel RGB565 BE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayPixel(pub u16);

impl DisplayPixel {
    /// Depuis RGB 8-bit.
    #[must_use]
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        let v = ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | (b as u16 >> 3);
        Self(v)
    }

    /// Depuis u16 BE.
    #[must_use]
    pub fn from_u16_be(v: u16) -> Self {
        Self(v)
    }

    /// Vers bytes BE.
    #[must_use]
    pub fn to_be_bytes(self) -> [u8; 2] {
        self.0.to_be_bytes()
    }

    /// Noir
    pub const BLACK: Self = Self(0x0000);
    /// Blanc
    pub const WHITE: Self = Self(0xFFFF);
    /// Rouge
    pub const RED: Self = Self(0xF800);
    /// Vert
    pub const GREEN: Self = Self(0x07E0);
    /// Bleu
    pub const BLUE: Self = Self(0x001F);
}

/// Frame complète pour un écran.
#[derive(Debug, Clone)]
pub struct DisplayFrame {
    /// Écran
    pub display: DisplayId,
    /// Position X
    pub x: u16,
    /// Position Y
    pub y: u16,
    /// Largeur
    pub w: u16,
    /// Hauteur
    pub h: u16,
    /// Pixels RGB565 row-major, taille w*h
    pub pixels: Vec<DisplayPixel>,
}

impl DisplayFrame {
    /// Frame plein écran noire.
    #[must_use]
    pub fn new_full(display: DisplayId) -> Self {
        Self {
            display,
            x: 0,
            y: 0,
            w: crate::consts::DISPLAY_WIDTH as u16,
            h: crate::consts::DISPLAY_HEIGHT as u16,
            pixels: vec![
                DisplayPixel::BLACK;
                crate::consts::DISPLAY_WIDTH * crate::consts::DISPLAY_HEIGHT
            ],
        }
    }

    /// Frame solid color plein écran.
    #[must_use]
    pub fn solid(display: DisplayId, color: DisplayPixel) -> Self {
        Self {
            display,
            x: 0,
            y: 0,
            w: crate::consts::DISPLAY_WIDTH as u16,
            h: crate::consts::DISPLAY_HEIGHT as u16,
            pixels: vec![color; crate::consts::DISPLAY_WIDTH * crate::consts::DISPLAY_HEIGHT],
        }
    }

    /// Vérifie la cohérence.
    pub fn validate(&self) -> Result<()> {
        if self.w as usize * self.h as usize != self.pixels.len() {
            return Err(ProtoError::InvalidDisplayData("pixels len != w*h"));
        }
        if self.w == 0 || self.h == 0 {
            return Err(ProtoError::InvalidDisplayData("w/h zero"));
        }
        if self.x as usize + self.w as usize > crate::consts::DISPLAY_WIDTH
            || self.y as usize + self.h as usize > crate::consts::DISPLAY_HEIGHT
        {
            return Err(ProtoError::InvalidDisplayData("frame out of bounds"));
        }
        Ok(())
    }
}

/// Taille d'une commande dans le stream (4 octets).
const COMMAND_LENGTH: usize = 4;
/// Taille du header fenêtre (16 octets).
const HEADER_LENGTH: usize = 16;
/// Taille d'un pixel RGB565 (2 octets).
const PIXEL_LENGTH: usize = 2;

/// Encode un `DisplayFrame` en transfert bulk complet.
///
/// Format vérifié contre `ni-controllers-lib` (fichier `ni_lcd_displays.ts`):
///   - 16 octets : header fenêtre  (magic, display_id, x, y, w, h)
///   -  4 octets : commande "transmit" (0x00 + half-pixel count BE 24-bit)
///   - w*h*2 octets : pixel data RGB565 Big-Endian
///   -  4 octets : commande "blit" (0x03 + 3×0x00)
///   -  4 octets : commande "end"  (0x40 + 3×0x00)
///   Total plein écran = 16 + 4 + 261120 + 4 + 4 = 261148
pub fn encode_bulk_transfer(frame: &DisplayFrame) -> Result<Vec<u8>> {
    frame.validate()?;
    let pixel_count = frame.pixels.len();
    let total_size = HEADER_LENGTH + COMMAND_LENGTH * 3 + pixel_count * PIXEL_LENGTH;
    let mut out = Vec::with_capacity(total_size);

    // ── Header fenêtre : 16 octets ──────────────────────────────────────
    // [0]  0x84  magic
    // [1]  0x00
    // [2]  display_id (0=left, 1=right)
    // [3]  0x60
    // [4..7]  0x00 zeros
    // [8..9]  x  (u16 BE)
    // [10..11] y  (u16 BE)
    // [12..13] w  (u16 BE) = 0x01E0 (480)
    // [14..15] h  (u16 BE) = 0x0110 (272)
    out.push(0x84);
    out.push(0x00);
    out.push(frame.display as u8);
    out.push(0x60);
    out.extend_from_slice(&[0u8; 4]);                   // [4..7]
    out.extend_from_slice(&frame.x.to_be_bytes());      // [8..9]
    out.extend_from_slice(&frame.y.to_be_bytes());      // [10..11]
    out.extend_from_slice(&frame.w.to_be_bytes());      // [12..13]
    out.extend_from_slice(&frame.h.to_be_bytes());      // [14..15]

    // ── Commande "transmit" : 4 octets ──────────────────────────────────
    // Encodes halfPixels as a Big-Endian uint32 with the MSB clobbered to 0x00.
    // Reference: setUint32(0, halfPixels, false); setUint8(0, 0);
    // halfPixels = pixel_count / 2 = 65280 = 0x0000FF00
    // After clobber: [0x00, 0x00, 0xFF, 0x00]
    let half_pixels = (pixel_count / 2) as u32;
    let hp_be = half_pixels.to_be_bytes();  // [0x00, 0x00, 0xFF, 0x00]
    out.push(0x00);         // [16] command byte (clobber MSB to 0x00)
    out.push(hp_be[1]);     // [17]
    out.push(hp_be[2]);     // [18]
    out.push(hp_be[3]);     // [19]

    // ── Pixel data : w*h*2 octets RGB565 Big-Endian ─────────────────────
    for &px in &frame.pixels {
        out.extend_from_slice(&px.to_be_bytes());
    }

    // ── Commande "blit" : 4 octets ──────────────────────────────────────
    out.push(0x03);
    out.extend_from_slice(&[0u8; 3]);

    // ── Commande "end" : 4 octets ───────────────────────────────────────
    out.push(0x40);
    out.extend_from_slice(&[0u8; 3]);

    debug_assert_eq!(out.len(), total_size);
    Ok(out)
}

/// Décode l'header bulk (pour tests / debug).
pub fn decode_bulk_header(data: &[u8]) -> Result<DisplayFrame> {
    if data.len() < HEADER_LENGTH + COMMAND_LENGTH {
        return Err(ProtoError::InvalidBulkHeader("too short"));
    }
    if data[0] != 0x84 || data[1] != 0x00 || data[3] != 0x60 {
        return Err(ProtoError::InvalidBulkHeader("bad magic"));
    }
    let display = DisplayId::from_u8(data[2])?;
    let x = u16::from_be_bytes([data[8], data[9]]);
    let y = u16::from_be_bytes([data[10], data[11]]);
    let w = u16::from_be_bytes([data[12], data[13]]);
    let h = u16::from_be_bytes([data[14], data[15]]);
    Ok(DisplayFrame {
        display,
        x,
        y,
        w,
        h,
        pixels: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_rgb() {
        let p = DisplayPixel::from_rgb(255, 0, 0);
        assert_eq!(p.0, 0xF800);
        let g = DisplayPixel::from_rgb(0, 255, 0);
        assert_eq!(g.0, 0x07E0);
    }

    #[test]
    fn solid_encode_decode_header() {
        let f = DisplayFrame::solid(DisplayId::Left, DisplayPixel::RED);
        let enc = encode_bulk_transfer(&f).unwrap();
        // Header magic
        assert_eq!(enc[0], 0x84);
        assert_eq!(enc[1], 0x00);
        assert_eq!(enc[2], 0x00); // Left display
        assert_eq!(enc[3], 0x60);
        // Transmit command at [16..19]
        assert_eq!(enc[16], 0x00); // command byte
        assert_eq!(enc[17], 0x00);
        assert_eq!(enc[18], 0xFF);
        assert_eq!(enc[19], 0x00);
        // Decode header
        let hdr = decode_bulk_header(&enc).unwrap();
        assert_eq!(hdr.display, DisplayId::Left);
        assert_eq!(hdr.w, 480);
        assert_eq!(hdr.h, 272);
        // Total: 16 (header) + 4 (cmd) + 480*272*2 (pixels) + 4 (blit) + 4 (end) = 261148
        assert_eq!(enc.len(), 261148);
        // Footer: blit=0x03, end=0x40
        assert_eq!(enc[enc.len() - 8], 0x03);
        assert_eq!(enc[enc.len() - 4], 0x40);
    }

    #[test]
    fn full_black_raw() {
        let f = DisplayFrame::new_full(DisplayId::Right);
        let enc = encode_bulk_transfer(&f).unwrap();
        assert_eq!(enc[2], 0x01); // Right display
        assert_eq!(enc.len(), 261148);
    }

    #[test]
    fn checkerboard_not_too_large() {
        let mut pixels = Vec::with_capacity(480 * 272);
        for y in 0..272 {
            for x in 0..480 {
                let c = if (x + y) % 2 == 0 {
                    DisplayPixel::WHITE
                } else {
                    DisplayPixel::BLACK
                };
                pixels.push(c);
            }
        }
        let f = DisplayFrame {
            display: DisplayId::Left,
            x: 0,
            y: 0,
            w: 480,
            h: 272,
            pixels,
        };
        let enc = encode_bulk_transfer(&f).unwrap();
        assert_eq!(enc.len(), 261148);
    }
}
