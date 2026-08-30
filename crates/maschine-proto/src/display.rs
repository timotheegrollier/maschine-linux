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

/// Encode un `DisplayFrame` en transfert bulk complet (header 32 + stream RLE).
pub fn encode_bulk_transfer(frame: &DisplayFrame) -> Result<Vec<u8>> {
    frame.validate()?;
    let mut out = Vec::with_capacity(32 + frame.pixels.len() * 2 + 16);

    // Header 32 bytes
    out.push(0x84);
    out.push(0x00);
    out.push(frame.display as u8);
    out.push(0x60);
    out.extend_from_slice(&[0u8; 4]); // 4..7
    out.extend_from_slice(&frame.x.to_be_bytes()); // 8..9
    out.extend_from_slice(&frame.y.to_be_bytes()); // 10..11
    out.extend_from_slice(&frame.w.to_be_bytes()); // 12..13
    out.extend_from_slice(&frame.h.to_be_bytes()); // 14..15
    out.push(0x02); // 16
    out.extend_from_slice(&[0u8; 5]); // 17..21
    let image_size_half = (frame.pixels.len() as u16).to_be_bytes();
    out.extend_from_slice(&image_size_half); // 22..23
    out.extend_from_slice(&[0u8; 8]); // 24..31

    // Stream : on encode en RAW 0x00 par paires, avec RLE 0x01 pour les runs solides
    // Simplification : tout en RAW si pas de run, sinon RLE quand ≥4 pixels identiques consécutifs
    let pixels = &frame.pixels;
    let mut i = 0;
    while i < pixels.len() {
        // Compte run de même couleur par paire (2 pixels = 4 bytes)
        // RLE encode 2 pixels répétés n fois (n = pair count)
        if i + 1 < pixels.len() && pixels[i] == pixels[i + 1] {
            // au moins une paire identique
            let pair = [pixels[i], pixels[i + 1]];
            let mut run_pairs = 1usize;
            let mut j = i + 2;
            while j + 1 < pixels.len() && pixels[j] == pair[0] && pixels[j + 1] == pair[1] {
                run_pairs += 1;
                j += 2;
                if run_pairs >= 0xFFFFFF {
                    break;
                }
            }
            if run_pairs >= 2 {
                // use RLE
                let n = run_pairs as u32;
                out.push(0x01);
                out.push((n & 0xFF) as u8);
                out.push(((n >> 8) & 0xFF) as u8);
                out.push(((n >> 16) & 0xFF) as u8);
                out.extend_from_slice(&pair[0].to_be_bytes());
                out.extend_from_slice(&pair[1].to_be_bytes());
                i = j;
                continue;
            }
        }
        // RAW : on batch jusqu'à prochain run ou fin, en paires
        let start = i;
        let mut end = i + 2;
        // batch au moins 1 paire, jusqu'à 0xFFFFFF paires
        while end + 1 < pixels.len() {
            // check si un run commence à end
            if pixels[end] == pixels[end + 1] {
                // peek run length
                let mut run = 1usize;
                let mut k = end + 2;
                while k + 1 < pixels.len()
                    && pixels[k] == pixels[end]
                    && pixels[k + 1] == pixels[end + 1]
                {
                    run += 1;
                    k += 2;
                }
                if run >= 2 {
                    break;
                }
            }
            end += 2;
            if end - start >= 0xFFFF * 2 {
                break;
            }
            // si impair trailing pixel, on va le gérer après
        }
        // assure pair count
        if (end - start) % 2 == 1 {
            end -= 1;
        }
        if end > pixels.len() {
            end = pixels.len() & !1;
        }
        if end <= start {
            end = start + 2;
        }
        let pairs = (end - start) / 2;
        let n = pairs as u32;
        out.push(0x00);
        out.push((n & 0xFF) as u8);
        out.push(((n >> 8) & 0xFF) as u8);
        out.push(((n >> 16) & 0xFF) as u8);
        for &px in &pixels[start..end] {
            out.extend_from_slice(&px.to_be_bytes());
        }
        i = end;
        // si reste 1 pixel impair, l'envoyer avec padding noir (firmware attend paires)
        if i == pixels.len() - 1 {
            let n = 1u32;
            out.push(0x00);
            out.push((n & 0xFF) as u8);
            out.push(((n >> 8) & 0xFF) as u8);
            out.push(((n >> 16) & 0xFF) as u8);
            out.extend_from_slice(&pixels[i].to_be_bytes());
            out.extend_from_slice(&DisplayPixel::BLACK.to_be_bytes());
            i += 1;
        }
    }

    // footer
    out.push(0x02);
    out.extend_from_slice(&[0u8; 3]);
    out.push(0x03);
    out.extend_from_slice(&[0u8; 3]);
    out.push(0x03);
    out.extend_from_slice(&[0u8; 3]);
    out.push(0x04);
    out.extend_from_slice(&[0u8; 3]);

    // Certains firmwares attendent exactement 261175 bytes pour full frame — on ne pad pas ici,
    // le caller peut vérifier out.len()
    Ok(out)
}

/// Décode l'header bulk (pour tests / debug).
pub fn decode_bulk_header(data: &[u8]) -> Result<DisplayFrame> {
    if data.len() < 32 {
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
        assert_eq!(enc[0], 0x84);
        assert_eq!(enc[2], 0x00);
        let hdr = decode_bulk_header(&enc).unwrap();
        assert_eq!(hdr.display, DisplayId::Left);
        assert_eq!(hdr.w, 480);
        assert_eq!(hdr.h, 272);
        // solid red should be mostly RLE -> much smaller than raw
        assert!(
            enc.len() < 5000,
            "RLE solid should be tiny, got {}",
            enc.len()
        );
    }

    #[test]
    fn full_black_rle() {
        let f = DisplayFrame::new_full(DisplayId::Right);
        let enc = encode_bulk_transfer(&f).unwrap();
        assert_eq!(enc[2], 0x01);
        assert!(enc.len() < 5000);
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
        // checkerboard : alternating pair WB/WB? Our encoder will detect pair repeat? Actually pairs BW then.
        // Just ensure it encodes and is not insane size (> raw)
        assert!(enc.len() > 32);
        assert!(enc.len() < 400_000);
    }
}
