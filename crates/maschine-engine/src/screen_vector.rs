//! Formes lissées des écrans MK3.
//!
//! `tiny-skia` rastérise chemins et traits en couverture 8 bits (masque à la
//! taille de la forme) ; la couverture est ensuite mélangée dans la trame
//! RGB565, avec un tramage ordonné pour que les dégradés ne fassent pas de
//! bandes sur 16 bits. Courbes de filtre, enveloppes, formes d'onde, coins
//! arrondis et barres de valeur passent par ici.

use crate::screen_text::rgb;
use maschine_proto::display::{DisplayFrame, DisplayPixel};
use tiny_skia::{FillRule, LineCap, LineJoin, Mask, Path, PathBuilder, Stroke, Transform};

/// Peinture d'une forme.
#[derive(Debug, Clone, Copy)]
pub enum Paint {
    /// Couleur unie, opacité 0..1.
    Solid(DisplayPixel, f32),
    /// Dégradé vertical d'opacité (couleur unique) entre `y0` et `y1`.
    Fade {
        color: DisplayPixel,
        top: f32,
        bottom: f32,
        y0: f32,
        y1: f32,
    },
}

impl Paint {
    fn at(self, y: f32) -> (DisplayPixel, f32) {
        match self {
            Self::Solid(c, a) => (c, a),
            Self::Fade {
                color,
                top,
                bottom,
                y0,
                y1,
            } => {
                let t = ((y - y0) / (y1 - y0).max(1.0)).clamp(0.0, 1.0);
                (color, top + (bottom - top) * t)
            }
        }
    }
}

/// Matrice de Bayer 4×4 (tramage ordonné), centrée sur 0.
const BAYER: [[f32; 4]; 4] = [
    [0.0, 8.0, 2.0, 10.0],
    [12.0, 4.0, 14.0, 6.0],
    [3.0, 11.0, 1.0, 9.0],
    [15.0, 7.0, 13.0, 5.0],
];

/// Mélange une couleur dans la trame avec tramage (pas de bandes RGB565).
pub fn blend(frame: &mut DisplayFrame, x: i32, y: i32, color: DisplayPixel, alpha: f32) {
    let (w, h) = (frame.w as i32, frame.h as i32);
    if x < 0 || y < 0 || x >= w || y >= h || alpha <= 0.0 {
        return;
    }
    let Some(dst) = frame.pixels.get_mut((y * w + x) as usize) else {
        return;
    };
    // Pixel entièrement couvert : la couleur exacte, sans tramage.
    if alpha >= 0.999 {
        *dst = color;
        return;
    }
    let a = alpha;
    let (fr, fg, fb) = rgb(color);
    let (br, bg, bb) = rgb(*dst);
    let d = (BAYER[(y & 3) as usize][(x & 3) as usize] + 0.5) / 16.0;
    // Pas de quantification : 8 niveaux (5 bits) / 4 niveaux (6 bits).
    let q = |f: f32, b: f32, step: f32| {
        let v = (b + (f - b) * a) * 255.0;
        (v + d * step).clamp(0.0, 255.0) as u8
    };
    *dst = DisplayPixel::from_rgb(q(fr, br, 8.0), q(fg, bg, 4.0), q(fb, bb, 8.0));
}

/// Remplit un chemin (règle non nulle), lissé.
pub fn fill(frame: &mut DisplayFrame, path: &Path, paint: Paint) {
    let b = path.bounds();
    let x0 = b.left().floor().max(0.0) as i32;
    let y0 = b.top().floor().max(0.0) as i32;
    let x1 = (b.right().ceil() as i32).min(frame.w as i32);
    let y1 = (b.bottom().ceil() as i32).min(frame.h as i32);
    let (w, h) = (x1 - x0, y1 - y0);
    if w <= 0 || h <= 0 {
        return;
    }
    let Some(mut mask) = Mask::new(w as u32, h as u32) else {
        return;
    };
    mask.fill_path(
        path,
        FillRule::Winding,
        true,
        Transform::from_translate(-x0 as f32, -y0 as f32),
    );
    let data = mask.data();
    for yy in 0..h {
        let (color, alpha) = paint.at((y0 + yy) as f32);
        for xx in 0..w {
            let cov = data[(yy * w + xx) as usize];
            if cov > 0 {
                blend(
                    frame,
                    x0 + xx,
                    y0 + yy,
                    color,
                    alpha * f32::from(cov) / 255.0,
                );
            }
        }
    }
}

/// Trace un chemin (extrémités et jonctions arrondies).
pub fn stroke(frame: &mut DisplayFrame, path: &Path, width: f32, paint: Paint) {
    let style = Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    if let Some(outline) = path.stroke(&style, 1.0) {
        fill(frame, &outline, paint);
    }
}

/// Ligne brisée passant par `points`.
#[must_use]
pub fn polyline(points: &[(f32, f32)]) -> Option<Path> {
    let (first, rest) = points.split_first()?;
    let mut pb = PathBuilder::new();
    pb.move_to(first.0, first.1);
    for p in rest {
        pb.line_to(p.0, p.1);
    }
    pb.finish()
}

/// Surface sous une courbe, fermée sur la ligne `base_y`.
#[must_use]
pub fn area(points: &[(f32, f32)], base_y: f32) -> Option<Path> {
    let (first, _) = points.split_first()?;
    let last = points.last()?;
    let mut pb = PathBuilder::new();
    pb.move_to(first.0, base_y);
    for p in points {
        pb.line_to(p.0, p.1);
    }
    pb.line_to(last.0, base_y);
    pb.close();
    pb.finish()
}

/// Rectangle aux coins arrondis.
#[must_use]
pub fn round_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    // Approximation cubique d'un quart de cercle.
    let k = r * 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

/// Remplit un rectangle arrondi.
#[allow(clippy::too_many_arguments)] // primitive de dessin : coordonnées explicites
pub fn fill_round_rect(
    frame: &mut DisplayFrame,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    color: DisplayPixel,
) {
    if let Some(path) = round_rect(x, y, w, h, r) {
        fill(frame, &path, Paint::Solid(color, 1.0));
    }
}

/// Arc de cercle (angles en radians, 0 = à droite, sens horaire écran).
#[must_use]
pub fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Option<Path> {
    let steps = (((to - from).abs() * r) / 3.0).ceil().max(2.0) as usize;
    let points: Vec<(f32, f32)> = (0..=steps)
        .map(|i| {
            let a = from + (to - from) * i as f32 / steps as f32;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect();
    polyline(&points)
}

#[cfg(test)]
mod tests {
    use super::*;
    use maschine_proto::display::DisplayId;

    #[test]
    fn shapes_are_antialiased_and_clipped_to_the_screen() {
        let black = DisplayPixel::from_rgb(0, 0, 0);
        let mut f = DisplayFrame::solid(DisplayId::Left, black);
        fill_round_rect(&mut f, 470.0, 260.0, 40.0, 40.0, 6.0, DisplayPixel::WHITE);
        let line = polyline(&[(10.0, 10.0), (200.0, 60.0)]).unwrap();
        stroke(&mut f, &line, 2.0, Paint::Solid(DisplayPixel::WHITE, 1.0));
        let lit: Vec<_> = f.pixels.iter().filter(|p| **p != black).collect();
        assert!(!lit.is_empty());
        assert!(
            lit.iter().any(|p| **p != DisplayPixel::WHITE),
            "bords lissés"
        );
    }

    #[test]
    fn fades_go_from_opaque_to_transparent() {
        let black = DisplayPixel::from_rgb(0, 0, 0);
        let mut f = DisplayFrame::solid(DisplayId::Left, black);
        let path = round_rect(0.0, 0.0, 480.0, 100.0, 0.0).unwrap();
        fill(
            &mut f,
            &path,
            Paint::Fade {
                color: DisplayPixel::WHITE,
                top: 1.0,
                bottom: 0.0,
                y0: 0.0,
                y1: 100.0,
            },
        );
        let luma = |y: usize| rgb(f.pixels[y * 480 + 7]).1;
        assert!(luma(2) > 0.9 && luma(97) < 0.1 && luma(50) > luma(80));
    }
}
