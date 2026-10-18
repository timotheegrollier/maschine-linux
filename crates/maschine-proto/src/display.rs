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

    /// Copie pleine écran : colle cette frame (éventuellement une fenêtre
    /// partielle) sur un fond noir, à sa position `x/y`.
    #[must_use]
    pub fn to_full(&self) -> Self {
        let mut full = Self::new_full(self.display);
        for row in 0..self.h as usize {
            let dst = (self.y as usize + row) * full.w as usize + self.x as usize;
            let src = row * self.w as usize;
            let w = self.w as usize;
            full.pixels[dst..dst + w].copy_from_slice(&self.pixels[src..src + w]);
        }
        full
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
///
/// Total plein écran = 16 + 4 + 261120 + 4 + 4 = 261148
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
    out.extend_from_slice(&[0u8; 4]); // [4..7]
    out.extend_from_slice(&frame.x.to_be_bytes()); // [8..9]
    out.extend_from_slice(&frame.y.to_be_bytes()); // [10..11]
    out.extend_from_slice(&frame.w.to_be_bytes()); // [12..13]
    out.extend_from_slice(&frame.h.to_be_bytes()); // [14..15]

    // ── Commande "transmit" : 4 octets ──────────────────────────────────
    // Encodes halfPixels as a Big-Endian uint32 with the MSB clobbered to 0x00.
    // Reference: setUint32(0, halfPixels, false); setUint8(0, 0);
    // halfPixels = pixel_count / 2 = 65280 = 0x0000FF00
    // After clobber: [0x00, 0x00, 0xFF, 0x00]
    let half_pixels = (pixel_count / 2) as u32;
    let hp_be = half_pixels.to_be_bytes(); // [0x00, 0x00, 0xFF, 0x00]
    out.push(0x00); // [16] command byte (clobber MSB to 0x00)
    out.push(hp_be[1]); // [17]
    out.push(hp_be[2]); // [18]
    out.push(hp_be[3]); // [19]

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

/// Nombre maximum de régions envoyées pour un rafraîchissement partiel.
pub const MAX_DIRTY_RECTS: usize = 6;
/// Au-delà de cette fraction de la surface, on renvoie la frame complète.
pub const FULL_FRAME_RATIO: f32 = 0.55;

/// Taille exacte d'un transfert bulk pour `pixel_count` pixels.
#[must_use]
pub const fn transfer_len(pixel_count: usize) -> usize {
    HEADER_LENGTH + COMMAND_LENGTH * 3 + pixel_count * PIXEL_LENGTH
}

/// Rect (x, y, w, h) d'une région modifiée.
type Rect = (u16, u16, u16, u16);

/// Suivi des différences entre deux rendus d'un même écran.
///
/// Le protocole bulk accepte un header de fenêtre (`x/y/w/h`) : on n'envoie que
/// les régions qui ont changé. C'est indispensable pour la MK3 : une frame
/// complète pèse 261 148 octets, et 2 écrans à 30 FPS saturent le bus USB
/// (écritures en timeout, écrans figés).
#[derive(Debug)]
pub struct FrameDiffer {
    display: DisplayId,
    w: usize,
    h: usize,
    /// Dernier état connu du framebuffer de l'écran (sur le device).
    prev: Vec<DisplayPixel>,
    /// `false` tant qu'aucun état n'a été confirmé → frame complète.
    valid: bool,
}

impl FrameDiffer {
    /// Crée un differ pour un écran (état initial inconnu).
    #[must_use]
    pub fn new(display: DisplayId) -> Self {
        let w = crate::consts::DISPLAY_WIDTH;
        let h = crate::consts::DISPLAY_HEIGHT;
        Self {
            display,
            w,
            h,
            prev: vec![DisplayPixel::BLACK; w * h],
            valid: false,
        }
    }

    /// Invalide l'état (le prochain diff renverra la frame complète).
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Régions à envoyer pour amener l'écran à l'état de `next`.
    ///
    /// Pas de modification d'état : appeler [`FrameDiffer::commit`] après un
    /// envoi réussi. Si l'envoi échoue, l'état reste l'ancien et le prochain
    /// diff renvoie à nouveau toutes les différences.
    #[must_use]
    pub fn diff(&self, next: &DisplayFrame) -> Vec<DisplayFrame> {
        let full = |frame: &DisplayFrame| {
            vec![DisplayFrame {
                display: frame.display,
                x: 0,
                y: 0,
                w: frame.w,
                h: frame.h,
                pixels: frame.pixels.clone(),
            }]
        };

        if !self.valid || next.w as usize != self.w || next.h as usize != self.h {
            return full(next);
        }
        debug_assert_eq!(next.pixels.len(), self.w * self.h);

        // 1. Rangées modifiées → runs verticaux contigus.
        let mut runs: Vec<(usize, usize)> = Vec::with_capacity(8);
        let mut y = 0;
        while y < self.h {
            if self.row_changed(next, y) {
                let start = y;
                while y < self.h && self.row_changed(next, y) {
                    y += 1;
                }
                runs.push((start, y));
            } else {
                y += 1;
            }
        }
        if runs.is_empty() {
            return Vec::new();
        }

        // 2. Boîte englobante horizontale de chaque run.
        let mut rects: Vec<Rect> = runs
            .iter()
            .map(|&(y0, y1)| {
                let (x0, x1) = self.col_range(next, y0, y1);
                (x0 as u16, y0 as u16, (x1 - x0) as u16, (y1 - y0) as u16)
            })
            .collect();

        // 3. Trop de régions → fusion des paires les moins coûteuses.
        while rects.len() > MAX_DIRTY_RECTS {
            let mut best = 0usize;
            let mut best_area = u64::MAX;
            for i in 0..rects.len() - 1 {
                let a = rects[i];
                let b = rects[i + 1];
                let area = merge_area(a, b);
                if area < best_area {
                    best_area = area;
                    best = i;
                }
            }
            let merged = merge_rects(rects[best], rects[best + 1]);
            rects[best] = merged;
            rects.remove(best + 1);
        }

        // 4. Surface modifiée trop grande → frame complète (moins de risque de
        //    tearing et un seul transfert cohérent).
        let screen_area = (self.w * self.h) as f32;
        let dirty_area: u64 = rects
            .iter()
            .map(|&(_, _, w, h)| u64::from(w) * u64::from(h))
            .sum();
        if dirty_area as f32 > screen_area * FULL_FRAME_RATIO {
            return full(next);
        }

        rects
            .iter()
            .map(|&(x, y, w, h)| self.extract(next, x, y, w, h))
            .collect()
    }

    /// Enregistre `next` comme l'état affiché sur le device.
    pub fn commit(&mut self, next: &DisplayFrame) {
        if next.w as usize == self.w && next.h as usize == self.h {
            self.prev.copy_from_slice(&next.pixels);
            self.valid = true;
        }
    }

    fn row_changed(&self, next: &DisplayFrame, y: usize) -> bool {
        let base = y * self.w;
        self.prev[base..base + self.w] != next.pixels[base..base + self.w]
    }

    /// Plage horizontale (x0 inclus, x1 exclus) des pixels modifiés dans des
    /// rangées déjà détectées comme modifiées.
    fn col_range(&self, next: &DisplayFrame, y0: usize, y1: usize) -> (usize, usize) {
        let mut x0 = self.w;
        let mut x1 = 0usize;
        for y in y0..y1 {
            let base = y * self.w;
            let prev_row = &self.prev[base..base + self.w];
            let next_row = &next.pixels[base..base + self.w];
            for (x, (p, n)) in prev_row.iter().zip(next_row).enumerate() {
                if p != n {
                    if x < x0 {
                        x0 = x;
                    }
                    if x + 1 > x1 {
                        x1 = x + 1;
                    }
                }
            }
        }
        (x0.min(x1), x1.max(x0))
    }

    fn extract(&self, next: &DisplayFrame, x: u16, y: u16, w: u16, h: u16) -> DisplayFrame {
        let (x, y, w, h) = (x as usize, y as usize, w as usize, h as usize);
        let mut pixels = Vec::with_capacity(w * h);
        for row in y..y + h {
            let base = row * self.w + x;
            pixels.extend_from_slice(&next.pixels[base..base + w]);
        }
        DisplayFrame {
            display: self.display,
            x: x as u16,
            y: y as u16,
            w: w as u16,
            h: h as u16,
            pixels,
        }
    }
}

/// Aire de la boîte englobante de deux rects fusionnés.
fn merge_area(a: Rect, b: Rect) -> u64 {
    let (_, _, w, h) = merge_rects(a, b);
    u64::from(w) * u64::from(h)
}

/// Fusionne deux rects en leur boîte englobante.
fn merge_rects(a: Rect, b: Rect) -> Rect {
    let x0 = a.0.min(b.0);
    let y0 = a.1.min(b.1);
    let x1 = (a.0 + a.2).max(b.0 + b.2);
    let y1 = (a.1 + a.3).max(b.1 + b.3);
    (x0, y0, x1 - x0, y1 - y0)
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

    #[test]
    fn transfer_len_matches_full_encoder() {
        let enc = encode_bulk_transfer(&DisplayFrame::new_full(DisplayId::Left)).unwrap();
        assert_eq!(enc.len(), transfer_len(480 * 272));
    }

    /// Construit une frame neutre pour les tests de diff.
    fn blank_pair(display: DisplayId) -> DisplayFrame {
        DisplayFrame::new_full(display)
    }

    /// Applique des régions sur un buffer plein écran (comme le ferait le device).
    fn apply_rects(base: &mut [DisplayPixel], w: usize, rects: &[DisplayFrame]) {
        for r in rects {
            for row in 0..r.h as usize {
                let dst = (r.y as usize + row) * w + r.x as usize;
                let src = row * r.w as usize;
                base[dst..dst + r.w as usize].copy_from_slice(&r.pixels[src..src + r.w as usize]);
            }
        }
    }

    #[test]
    fn differ_first_diff_is_full_frame() {
        let mut d = FrameDiffer::new(DisplayId::Left);
        let frame = blank_pair(DisplayId::Left);
        let rects = d.diff(&frame);
        assert_eq!(rects.len(), 1);
        assert_eq!(
            (rects[0].x, rects[0].y, rects[0].w, rects[0].h),
            (0, 0, 480, 272)
        );
        assert_eq!(transfer_len(rects[0].pixels.len()), 261148);
    }

    #[test]
    fn differ_identical_is_empty() {
        let mut d = FrameDiffer::new(DisplayId::Right);
        let frame = blank_pair(DisplayId::Right);
        d.commit(&frame);
        assert!(d.diff(&frame).is_empty());
    }

    #[test]
    fn differ_small_change_sends_small_rect() {
        let mut d = FrameDiffer::new(DisplayId::Left);
        let mut frame = blank_pair(DisplayId::Left);
        d.commit(&frame);
        // Un « caractère » 8×16 en (120, 40).
        for y in 40..56 {
            for x in 120..128 {
                frame.pixels[y * 480 + x] = DisplayPixel::WHITE;
            }
        }
        let rects = d.diff(&frame);
        assert_eq!(rects.len(), 1);
        let r = &rects[0];
        assert_eq!((r.x, r.y, r.w, r.h), (120, 40, 8, 16));
        // Le transfert est minuscule comparé à une frame pleine.
        assert!(transfer_len(r.pixels.len()) < 400);
        // L'application des régions reproduit exactement la nouvelle frame.
        let mut applied = vec![DisplayPixel::BLACK; 480 * 272];
        apply_rects(&mut applied, 480, &rects);
        assert_eq!(applied, frame.pixels);
    }

    #[test]
    fn differ_two_zones_give_two_rects() {
        let mut d = FrameDiffer::new(DisplayId::Left);
        let mut frame = blank_pair(DisplayId::Left);
        d.commit(&frame);
        // Zone haute (bandeau) et zone basse (encodeurs), très disjointes.
        for y in 0..20 {
            for x in 10..200 {
                frame.pixels[y * 480 + x] = DisplayPixel::RED;
            }
        }
        for y in 250..270 {
            for x in 300..460 {
                frame.pixels[y * 480 + x] = DisplayPixel::GREEN;
            }
        }
        let rects = d.diff(&frame);
        assert_eq!(rects.len(), 2);
        let mut applied = vec![DisplayPixel::BLACK; 480 * 272];
        apply_rects(&mut applied, 480, &rects);
        assert_eq!(applied, frame.pixels);
    }

    #[test]
    fn differ_commit_then_identical_is_empty() {
        let mut d = FrameDiffer::new(DisplayId::Left);
        let mut frame = blank_pair(DisplayId::Left);
        d.commit(&frame);
        frame.pixels[42] = DisplayPixel::from_rgb(0, 196, 255);
        let rects = d.diff(&frame);
        assert!(!rects.is_empty());
        d.commit(&frame);
        assert!(d.diff(&frame).is_empty());
    }

    #[test]
    fn differ_invalidate_forces_full_frame() {
        let mut d = FrameDiffer::new(DisplayId::Left);
        let frame = blank_pair(DisplayId::Left);
        d.commit(&frame);
        assert!(d.diff(&frame).is_empty());
        d.invalidate();
        let rects = d.diff(&frame);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].pixels.len(), 480 * 272);
    }

    #[test]
    fn debug_synthetic_rows() {
        let mut d = FrameDiffer::new(DisplayId::Left);
        let mut frame = blank_pair(DisplayId::Left);
        d.commit(&frame);
        // Le cas qui a révélé un bug : la plage x ne doit pas être écrasée
        // par la dernière rangée scannée.
        for x in 296..355 {
            frame.pixels[23 * 480 + x] = DisplayPixel(1303);
        }
        for y in 24..65 {
            for x in 18..209 {
                frame.pixels[y * 480 + x] = DisplayPixel(999);
            }
        }
        let rects = d.diff(&frame);
        let mut applied = vec![DisplayPixel::BLACK; 480 * 272];
        apply_rects(&mut applied, 480, &rects);
        assert_eq!(applied, frame.pixels);
    }

    /// Propriété : pour n'importe quel motif de modifications, appliquer les
    /// régions retournées sur l'ancien buffer reproduit exactement le nouveau.
    #[test]
    fn differ_always_reconstructs_exactly() {
        let mut d = FrameDiffer::new(DisplayId::Right);
        let mut frame = blank_pair(DisplayId::Right);
        d.commit(&frame);
        // Générateur pseudo-aléatoire simple (xorshift) — pas de dépendance.
        let mut rng = 0x1234_5678_9ABC_DEF0u64;
        let mut next_u32 = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng >> 32) as u32
        };
        for round in 0..12 {
            let mut prev = frame.pixels.clone();
            assert_eq!(d.prev, prev, "round {round}: differ désynchronisé");
            let changes = 5 + (next_u32() % 400) as usize;
            for _ in 0..changes {
                let x = (next_u32() as usize) % 480;
                let y = (next_u32() as usize) % 272;
                let w = 1 + (next_u32() as usize) % 60;
                let h = 1 + (next_u32() as usize) % 24;
                let color = DisplayPixel((next_u32() & 0xFFFF) as u16);
                for yy in y..(y + h).min(272) {
                    for xx in x..(x + w).min(480) {
                        frame.pixels[yy * 480 + xx] = color;
                    }
                }
            }
            let rects = d.diff(&frame);
            let mut applied = prev.clone();
            apply_rects(&mut applied, 480, &rects);
            if applied != frame.pixels {
                let idx = applied
                    .iter()
                    .zip(&frame.pixels)
                    .position(|(a, b)| a != b)
                    .unwrap();
                let extra = rects
                    .iter()
                    .map(|r| format!("({},{},{}x{})", r.x, r.y, r.w, r.h))
                    .collect::<Vec<_>>()
                    .join(" ");
                panic!(
                    "round {round}: mismatch at y={} x={} got {:?} want {:?} — {} rects: {extra}",
                    idx / 480,
                    idx % 480,
                    applied[idx],
                    frame.pixels[idx],
                    rects.len()
                );
            }
            d.commit(&frame);
        }
    }

    #[test]
    fn differ_caps_number_of_rects() {
        let mut d = FrameDiffer::new(DisplayId::Left);
        let mut frame = blank_pair(DisplayId::Left);
        d.commit(&frame);
        // 16 bandes horizontales modifiées, disjointes.
        for band in 0..16 {
            let y = band * 16 + 2;
            for x in 0..10 {
                frame.pixels[y * 480 + x] = DisplayPixel::from_rgb(235, 205, 60);
            }
        }
        let rects = d.diff(&frame);
        assert!(
            rects.len() <= MAX_DIRTY_RECTS,
            "{} régions > max",
            rects.len()
        );
        let mut applied = vec![DisplayPixel::BLACK; 480 * 272];
        apply_rects(&mut applied, 480, &rects);
        assert_eq!(applied, frame.pixels);
    }

    #[test]
    fn differ_full_change_returns_full_frame() {
        let mut d = FrameDiffer::new(DisplayId::Left);
        let mut frame = blank_pair(DisplayId::Left);
        d.commit(&frame);
        for (i, px) in frame.pixels.iter_mut().enumerate() {
            *px = DisplayPixel((i & 0xFFFF) as u16);
        }
        let rects = d.diff(&frame);
        assert_eq!(rects.len(), 1);
        assert_eq!(
            (rects[0].x, rects[0].y, rects[0].w, rects[0].h),
            (0, 0, 480, 272)
        );
    }
}
