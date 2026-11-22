//! Texte anti-aliasé des écrans MK3.
//!
//! Mêmes polices que la GUI (embarquées par `epaint_default_fonts`) :
//! Hack, à chasse fixe, remplace l'ancienne police bitmap 8×16 sans changer
//! les mises en page (8 px par caractère) ; Ubuntu sert aux titres, libellés
//! de boutons et noms de paramètres. Les glyphes rastérisés par `ab_glyph`
//! sont mis en cache : un écran redessine souvent les mêmes caractères.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use maschine_proto::display::{DisplayFrame, DisplayPixel};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Police utilisée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    /// Hack : chasse fixe, valeurs et texte courant.
    Mono,
    /// Ubuntu : titres, boutons, noms de paramètres.
    Sans,
}

fn font(face: Face) -> &'static FontRef<'static> {
    static MONO: OnceLock<FontRef<'static>> = OnceLock::new();
    static SANS: OnceLock<FontRef<'static>> = OnceLock::new();
    let (cell, bytes) = match face {
        Face::Mono => (&MONO, epaint_default_fonts::HACK_REGULAR),
        Face::Sans => (&SANS, epaint_default_fonts::UBUNTU_LIGHT),
    };
    cell.get_or_init(|| FontRef::try_from_slice(bytes).expect("police embarquée valide"))
}

/// Glyphe rastérisé : couverture 0..255 et position par rapport à la ligne
/// de base et à la plume.
struct Glyph {
    left: i32,
    top: i32,
    w: usize,
    h: usize,
    cov: Vec<u8>,
}

fn glyph(face: Face, c: char, px: f32) -> Arc<Glyph> {
    type Cache = Mutex<HashMap<(Face, char, u32), Arc<Glyph>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let key = (face, c, (px * 8.0).round() as u32);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(g) = cache.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return g;
    }
    let font = font(face);
    let g = font.as_scaled(PxScale::from(px)).scaled_glyph(c);
    let glyph = match font.outline_glyph(g) {
        Some(outline) => {
            let b = outline.px_bounds();
            let (w, h) = (b.width() as usize, b.height() as usize);
            let mut cov = vec![0u8; w * h];
            outline.draw(|x, y, v| {
                if let Some(slot) = cov.get_mut(y as usize * w + x as usize) {
                    *slot = (v.clamp(0.0, 1.0) * 255.0) as u8;
                }
            });
            Glyph {
                left: b.min.x as i32,
                top: b.min.y as i32,
                w,
                h,
                cov,
            }
        }
        // Espace ou caractère sans contour.
        None => Glyph {
            left: 0,
            top: 0,
            w: 0,
            h: 0,
            cov: Vec::new(),
        },
    };
    let glyph = Arc::new(glyph);
    if let Ok(mut m) = cache.lock() {
        m.insert(key, Arc::clone(&glyph));
    }
    glyph
}

fn rgb(px: DisplayPixel) -> (f32, f32, f32) {
    let v = px.0;
    (
        f32::from(((v >> 11) & 0x1F) as u8) / 31.0,
        f32::from(((v >> 5) & 0x3F) as u8) / 63.0,
        f32::from((v & 0x1F) as u8) / 31.0,
    )
}

fn blend(frame: &mut DisplayFrame, x: i32, y: i32, color: DisplayPixel, alpha: f32) {
    let (w, h) = (frame.w as i32, frame.h as i32);
    if x < 0 || y < 0 || x >= w || y >= h || alpha <= 0.0 {
        return;
    }
    let idx = (y * w + x) as usize;
    let Some(dst) = frame.pixels.get_mut(idx) else {
        return;
    };
    let (fr, fg, fb) = rgb(color);
    let (br, bg, bb) = rgb(*dst);
    let mix = |f: f32, b: f32| ((b + (f - b) * alpha) * 255.0).round() as u8;
    *dst = DisplayPixel::from_rgb(mix(fr, br), mix(fg, bg), mix(fb, bb));
}

/// Largeur d'avance (px) d'un texte.
#[must_use]
pub fn width(text: &str, face: Face, px: f32) -> f32 {
    let scaled = font(face).as_scaled(PxScale::from(px));
    let mut prev = None;
    let mut w = 0.0;
    for c in text.chars() {
        let id = scaled.glyph_id(c);
        if let Some(p) = prev {
            w += scaled.kern(p, id);
        }
        w += scaled.h_advance(id);
        prev = Some(id);
    }
    w
}

/// Hauteur au-dessus de la ligne de base (px) pour une taille donnée.
#[must_use]
pub fn ascent(face: Face, px: f32) -> f32 {
    font(face).as_scaled(PxScale::from(px)).ascent()
}

/// Dessine `text` (ligne de base à `baseline`) et renvoie sa largeur.
/// `weight` > 1 épaissit le trait (police Ubuntu fine sur petit écran).
#[allow(clippy::too_many_arguments)] // primitive de dessin : coordonnées explicites
pub fn draw(
    frame: &mut DisplayFrame,
    x: f32,
    baseline: f32,
    text: &str,
    face: Face,
    px: f32,
    color: DisplayPixel,
    weight: f32,
) -> f32 {
    let scaled = font(face).as_scaled(PxScale::from(px));
    let mut pen = x;
    let mut prev = None;
    for c in text.chars() {
        let id = scaled.glyph_id(c);
        if let Some(p) = prev {
            pen += scaled.kern(p, id);
        }
        let g = glyph(face, c, px);
        let (ox, oy) = (pen.round() as i32 + g.left, baseline.round() as i32 + g.top);
        for gy in 0..g.h {
            for gx in 0..g.w {
                let cov = f32::from(g.cov[gy * g.w + gx]) / 255.0;
                blend(
                    frame,
                    ox + gx as i32,
                    oy + gy as i32,
                    color,
                    (cov * weight).min(1.0),
                );
            }
        }
        pen += scaled.h_advance(id);
        prev = Some(id);
    }
    pen - x
}

/// Taille de Hack dont l'avance vaut `advance` px (8 px = ancienne police).
#[must_use]
pub fn mono_px(advance: f32) -> f32 {
    static UNIT: OnceLock<f32> = OnceLock::new();
    let unit = *UNIT.get_or_init(|| {
        let scaled = font(Face::Mono).as_scaled(PxScale::from(1.0));
        scaled.h_advance(scaled.glyph_id('M'))
    });
    advance / unit
}

#[cfg(test)]
mod tests {
    use super::*;
    use maschine_proto::display::DisplayId;

    #[test]
    fn mono_keeps_the_eight_pixel_grid() {
        let px = mono_px(8.0);
        assert!((width("ABCDEFGH", Face::Mono, px) - 64.0).abs() < 0.5);
    }

    #[test]
    fn text_is_antialiased_and_clipped() {
        let bg = DisplayPixel::from_rgb(0, 0, 0);
        let mut frame = DisplayFrame::solid(DisplayId::Left, bg);
        let w = draw(
            &mut frame,
            470.0,
            20.0,
            "Kick",
            Face::Sans,
            16.0,
            DisplayPixel::WHITE,
            1.0,
        );
        assert!(w > 20.0, "largeur mesurée même hors écran");
        let mut frame = DisplayFrame::solid(DisplayId::Left, bg);
        draw(
            &mut frame,
            4.0,
            20.0,
            "Kick",
            Face::Sans,
            16.0,
            DisplayPixel::WHITE,
            1.0,
        );
        let lit: Vec<_> = frame.pixels.iter().filter(|p| **p != bg).collect();
        assert!(!lit.is_empty());
        assert!(
            lit.iter().any(|p| **p != DisplayPixel::WHITE),
            "bords lissés"
        );
    }
}
