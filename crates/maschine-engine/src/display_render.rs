//! Rendu de texte et éléments UI pour les 2 écrans 480×272 de la MK3.

use crate::{screen_text, screen_vector};
use maschine_proto::display::{DisplayFrame, DisplayId, DisplayPixel};

/// Dessine un caractère dans sa cellule 8×16 (fond `bg`, glyphe lissé).
pub fn draw_char(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    c: char,
    fg: DisplayPixel,
    bg: DisplayPixel,
) {
    let mut buf = [0u8; 4];
    draw_text(frame, x, y, c.encode_utf8(&mut buf), fg, bg);
}

/// Dessine du texte sur la grille historique 8×16 : une cellule de 8 px
/// par caractère peinte en `bg`, glyphes Hack lissés par-dessus (les mises
/// en page calculées en `len × 8` restent exactes).
pub fn draw_text(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    text: &str,
    fg: DisplayPixel,
    bg: DisplayPixel,
) {
    draw_text_scaled(frame, x, y, text, 1, fg, bg);
}

/// Remplis un rectangle
pub fn fill_rect(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: DisplayPixel,
) {
    let fw = frame.w as usize;
    let fh = frame.h as usize;
    for py in y..(y + h).min(fh) {
        for px in x..(x + w).min(fw) {
            let idx = py * fw + px;
            if idx < frame.pixels.len() {
                frame.pixels[idx] = color;
            }
        }
    }
}

/// Contour d'un rectangle, coins arrondis (trait lissé de 1 px).
pub fn draw_rect_outline(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: DisplayPixel,
) {
    if w < 2 || h < 2 {
        return;
    }
    let (xf, yf) = (x as f32 + 0.5, y as f32 + 0.5);
    if let Some(path) = screen_vector::round_rect(xf, yf, w as f32 - 1.0, h as f32 - 1.0, 3.0) {
        screen_vector::stroke(frame, &path, 1.0, screen_vector::Paint::Solid(color, 1.0));
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Design system écrans — style logiciel Maschine officiel
// Bandeau titre, contenu dense, rangée d'encodeurs réels, soft keys
// alignées sur les 8 boutons physiques (labels = actions réelles).
// ═══════════════════════════════════════════════════════════════════════

use crate::engine::{EngineSnapshot, DRUM_PRESET_NAMES};
use crate::project::{Adsr, Pattern};
use crate::transport::TransportState;

/// Largeur écran (px).
const SCR_W: usize = 480;
/// Hauteur écran (px).
const SCR_H: usize = 272;
/// Hauteur rangée soft keys.
const FOOTER_H: usize = 24;
/// Hauteur rangée encodeurs.
const ENC_H: usize = 48;
/// Ticks par mesure (4/4, PPQ 960).
const TICKS_PER_BAR: u64 = 3840;
/// Ticks par pas (double-croche).
const TICKS_PER_STEP: u64 = 240;

/// Palette unifiée des écrans.
struct Pal {
    bg: DisplayPixel,
    panel: DisplayPixel,
    panel2: DisplayPixel,
    border: DisplayPixel,
    dim: DisplayPixel,
    muted: DisplayPixel,
    white: DisplayPixel,
    /// Couleur du groupe focalisé (celle de ses pads) : sélection, état actif.
    accent: DisplayPixel,
    /// Texte posé sur `accent` (noir ou blanc selon sa clarté).
    ink: DisplayPixel,
    cyan: DisplayPixel,
    green: DisplayPixel,
    red: DisplayPixel,
    yellow: DisplayPixel,
    track: DisplayPixel,
}

impl Pal {
    /// Palette par défaut (accent orange) — écrans sans projet.
    fn new() -> Self {
        Self::with_accent(DisplayPixel::from_rgb(255, 154, 31))
    }

    /// Palette d'un écran : noir des pixels éteints, graphite pour la
    /// structure, et un accent qui prend la couleur du groupe focalisé — la
    /// même que ses pads : on sait toujours dans quel groupe on travaille.
    fn of(snap: &EngineSnapshot) -> Self {
        focus_group(snap)
            .and_then(|g| maschine_proto::PaletteIndex::from_u8(g.color))
            .map(|c| c.to_rgb())
            .filter(|&(r, g, b)| u16::from(r) + u16::from(g) + u16::from(b) > 60)
            .map_or_else(Self::new, |(r, g, b)| {
                Self::with_accent(screen_color(r, g, b))
            })
    }

    fn with_accent(accent: DisplayPixel) -> Self {
        Self {
            bg: DisplayPixel::from_rgb(0, 0, 0),
            panel: DisplayPixel::from_rgb(20, 22, 26),
            panel2: DisplayPixel::from_rgb(32, 35, 41),
            border: DisplayPixel::from_rgb(46, 50, 58),
            dim: DisplayPixel::from_rgb(166, 172, 183),
            muted: DisplayPixel::from_rgb(104, 110, 122),
            white: DisplayPixel::from_rgb(240, 242, 245),
            accent,
            ink: ink_for(accent),
            cyan: DisplayPixel::from_rgb(72, 200, 240),
            green: DisplayPixel::from_rgb(52, 214, 110),
            red: DisplayPixel::from_rgb(255, 72, 62),
            yellow: DisplayPixel::from_rgb(255, 184, 64),
            track: DisplayPixel::from_rgb(36, 39, 46),
        }
    }

    /// Couleur de niveau VU (vert -> jaune -> rouge).
    fn vu(&self, level: f32) -> DisplayPixel {
        if level >= 0.86 {
            self.red
        } else if level >= 0.62 {
            self.yellow
        } else {
            self.green
        }
    }
}

/// Couleur de LED (saturée) → couleur d'écran lisible sur noir : les teintes
/// sombres (bleu, violet) sont éclaircies, les autres à peine adoucies.
fn screen_color(r: u8, g: u8, b: u8) -> DisplayPixel {
    let luma = 0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b);
    let t = if luma < 110.0 { 0.38 } else { 0.12 };
    let lift = |c: u8| (f32::from(c) + (255.0 - f32::from(c)) * t) as u8;
    DisplayPixel::from_rgb(lift(r), lift(g), lift(b))
}

/// Texte lisible sur un fond `bg` : noir sur les couleurs claires.
fn ink_for(bg: DisplayPixel) -> DisplayPixel {
    let (r, g, b) = rgb565_to_rgb(bg);
    let luma = 0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b);
    if luma > 120.0 {
        DisplayPixel::from_rgb(0, 0, 0)
    } else {
        DisplayPixel::from_rgb(245, 246, 248)
    }
}

// ── Helpers de dessin ──────────────────────────────────────────────────

/// Troncature sûre (par caractères).
fn trunc(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Couleur de pad/Sound (16 teintes).
fn hue_color(index: usize) -> DisplayPixel {
    let h = (index % 16) as f32 / 16.0;
    let s = 0.62_f32;
    let v = 0.96_f32;
    let hi = (h * 6.0) as i32 % 6;
    let f = h * 6.0 - (h * 6.0).floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match hi {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    DisplayPixel::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

fn fmt_db(db: f32) -> String {
    if db <= -59.5 {
        "-inf".to_string()
    } else {
        format!("{db:+.1}")
    }
}

fn db_norm(db: f32) -> f32 {
    ((db + 60.0) / 72.0).clamp(0.0, 1.0)
}

fn lin_db(v: f32) -> f32 {
    if v <= 1.0e-4 {
        -60.0
    } else {
        20.0 * v.log10()
    }
}

fn fmt_pan(pan: f32) -> String {
    let p = pan.clamp(-1.0, 1.0);
    if p.abs() < 0.02 {
        "C".to_string()
    } else if p < 0.0 {
        format!("L{:.0}", -p * 100.0)
    } else {
        format!("R{:.0}", p * 100.0)
    }
}

fn fmt_ms(ms: f32) -> String {
    if ms >= 1000.0 {
        format!("{:.2}s", ms / 1000.0)
    } else if ms >= 100.0 {
        format!("{ms:.0}ms")
    } else {
        format!("{ms:.1}ms")
    }
}

fn fmt_hz(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1}k", hz / 1000.0)
    } else {
        format!("{hz:.0}Hz")
    }
}

/// Nom de note MIDI (convention NI : 36 = C1).
fn note_name(pitch: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let p = pitch as i32;
    format!(
        "{}{}",
        NAMES[(p.rem_euclid(12)) as usize],
        p.div_euclid(12) - 2
    )
}

/// Longueur d'un pas en notation musicale (1/16, 1/8...) ou en ticks.
fn fmt_len(ticks: u32) -> String {
    if ticks == 0 {
        // Longueur par défaut : un pas (double-croche).
        return "1/16".to_string();
    }
    if ticks.is_multiple_of(120) {
        return match ticks / 120 {
            1 => "1/32".to_string(),
            2 => "1/16".to_string(),
            4 => "1/8".to_string(),
            8 => "1/4".to_string(),
            16 => "1/2".to_string(),
            32 => "1/1".to_string(),
            n => format!("{}t", n * 120),
        };
    }
    format!("{ticks}t")
}

/// Texte posé sur la grille historique (cellules 8×16 peintes en `bg`, pour
/// que les mises en page en `len × 8` restent valables) ; le texte lui-même
/// est en Barlow, plus étroite que la cellule : il ne déborde pas.
fn draw_text_scaled(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    text: &str,
    scale: usize,
    fg: DisplayPixel,
    bg: DisplayPixel,
) {
    let scale = scale.max(1);
    let cells = text.chars().count();
    fill_rect(frame, x, y, cells * 8 * scale, 16 * scale, bg);
    let k = scale as f32;
    screen_text::draw(
        frame,
        x as f32,
        y as f32 + 12.5 * k,
        text,
        screen_text::Face::Sans,
        14.5 * k,
        fg,
        1.0,
    );
}

/// Segment lissé de 1 px (net à l'horizontale et à la verticale).
fn line(frame: &mut DisplayFrame, x0: i32, y0: i32, x1: i32, y1: i32, color: DisplayPixel) {
    let pts = [
        (x0 as f32 + 0.5, y0 as f32 + 0.5),
        (x1 as f32 + 0.5, y1 as f32 + 0.5),
    ];
    if let Some(path) = screen_vector::polyline(&pts) {
        screen_vector::stroke(frame, &path, 1.2, screen_vector::Paint::Solid(color, 1.0));
    }
}

/// Bandeau titre : titre, contexte, activité des pads et VU master.
///
/// Toutes les vues partagent ce chrome : barre accent, titre, info contexte à
/// droite, ruban d'activité des 16 pads (flashs du jeu + du séquenceur) et
/// mini VU master (niveau + peak).
fn header(frame: &mut DisplayFrame, p: &Pal, snap: &EngineSnapshot, title: &str, right: &str) {
    header_colored(frame, p, snap, title, right, p.accent);
}

/// Bandeau de contexte sous les libellés de boutons : repère de couleur
/// (Sound ou groupe), titre, info de contexte, activité des pads, VU master.
fn header_colored(
    frame: &mut DisplayFrame,
    p: &Pal,
    snap: &EngineSnapshot,
    title: &str,
    right: &str,
    color: DisplayPixel,
) {
    use screen_text::Face;
    const H: usize = 26;
    fill_rect(frame, 0, 0, SCR_W, H, p.bg);
    screen_vector::fill_round_rect(frame, 8.0, 6.0, 4.0, 14.0, 2.0, color);
    let title = fit_text(title, Face::Bold, 18.0, 290.0);
    let mut left_end = 18.0 + bold(frame, 18.0, 19.5, &title, 18.0, p.white);

    // Vélocité fixe : niveau visible en permanence (réglable au 4D).
    if snap.fixed_velocity {
        let text = format!("VEL {} FIXE", snap.fixed_velocity_level);
        left_end += 10.0 + bold(frame, left_end + 10.0, 18.5, &text, 13.0, p.accent);
    }

    // Retour d'action (sauvegarde, effet ajouté…) : prioritaire et visible.
    if let Some(msg) = snap.status_message.as_deref() {
        let msg = fit_text(msg, Face::Bold, 14.0, 250.0);
        let w = screen_text::width(&msg, Face::Bold, 14.0).ceil() + 18.0;
        let mx = SCR_W as f32 - w - 6.0;
        screen_vector::fill_round_rect(frame, mx, 4.0, w, H as f32 - 8.0, 4.0, p.accent);
        bold(frame, mx + 9.0, 18.0, &msg, 14.0, p.ink);
        fill_rect(frame, 0, H, SCR_W, 1, p.border);
        return;
    }

    // Activité des 16 pads (flashs du jeu et du séquenceur).
    let (seg_w, gap) = (2usize, 1usize);
    let strip_w = 16 * seg_w + 15 * gap;
    let strip_x = SCR_W - 10 - strip_w;
    for (i, activity) in snap.pad_activity.iter().enumerate() {
        let x = strip_x + i * (seg_w + gap);
        let act = activity.clamp(0.0, 1.0);
        let col = if act > 0.02 {
            mix(p.track, hue_color(i), 0.35 + 0.65 * act)
        } else {
            p.track
        };
        fill_rect(frame, x, 9, seg_w, 9, col);
    }

    // Mini VU master (niveau + crête).
    let vu_x = strip_x - 10;
    vu_v(
        frame,
        p,
        vu_x,
        6,
        4,
        H - 12,
        snap.master_level,
        snap.master_peak,
    );

    // Le contexte cède la place au titre : jamais de chevauchement.
    let room = (vu_x as f32 - 10.0 - (left_end + 14.0)).max(0.0);
    let txt = fit_text(right, Face::Sans, 14.0, room);
    let w = screen_text::width(&txt, Face::Sans, 14.0);
    label(frame, vu_x as f32 - 10.0 - w, 18.5, &txt, 14.0, p.dim, 1.0);
    fill_rect(frame, 0, H, SCR_W, 1, p.border);
}

/// Texte Barlow lissé ; renvoie la largeur dessinée. Un `weight` ≥ 1,75
/// (anciens appels « gras ») choisit la graisse SemiBold.
fn label(
    frame: &mut DisplayFrame,
    x: f32,
    baseline: f32,
    text: &str,
    px: f32,
    color: DisplayPixel,
    weight: f32,
) -> f32 {
    let face = if weight >= 1.75 {
        screen_text::Face::Bold
    } else {
        screen_text::Face::Sans
    };
    screen_text::draw(frame, x, baseline, text, face, px, color, 1.0)
}

/// Texte SemiBold (titres, valeurs) ; renvoie la largeur dessinée.
fn bold(
    frame: &mut DisplayFrame,
    x: f32,
    baseline: f32,
    text: &str,
    px: f32,
    color: DisplayPixel,
) -> f32 {
    screen_text::draw(
        frame,
        x,
        baseline,
        text,
        screen_text::Face::Bold,
        px,
        color,
        1.0,
    )
}

/// Tronque `text` (avec « … ») pour tenir dans `max_w` pixels. Mesuré en
/// SemiBold (la plus large) : le texte tient quelle que soit sa graisse.
fn fit_text(text: &str, face: screen_text::Face, px: f32, max_w: f32) -> String {
    let face = if face == screen_text::Face::Sans {
        screen_text::Face::Bold
    } else {
        face
    };
    if screen_text::width(text, face, px) <= max_w {
        return text.to_string();
    }
    let mut out: String = text.to_string();
    while !out.is_empty() && screen_text::width(&format!("{out}…"), face, px) > max_w {
        out.pop();
    }
    if out.is_empty() {
        String::new()
    } else {
        format!("{}…", out.trim_end())
    }
}

/// Convertit un pixel RGB565 en (r, g, b) 8 bits.
fn rgb565_to_rgb(px: DisplayPixel) -> (u8, u8, u8) {
    let v = px.0;
    let r5 = ((v >> 11) & 0x1F) as u8;
    let g6 = ((v >> 5) & 0x3F) as u8;
    let b5 = (v & 0x1F) as u8;
    (
        (r5 << 3) | (r5 >> 2),
        (g6 << 2) | (g6 >> 4),
        (b5 << 3) | (b5 >> 2),
    )
}

/// Bloc de contenu : fond graphite aux coins arrondis.
fn panel(frame: &mut DisplayFrame, p: &Pal, x: usize, y: usize, w: usize, h: usize) {
    fill_rect(frame, x, y, w, h, p.bg);
    screen_vector::fill_round_rect(frame, x as f32, y as f32, w as f32, h as f32, 4.0, p.panel);
}

/// Hauteur de la bande des libellés de boutons, en haut de chaque écran.
pub const SOFT_KEYS_H: usize = FOOTER_H + 2;

/// Libellés des 4 boutons d'écran de ce côté — **dernier dessin** de chaque
/// écran. Sur la MK3 les boutons 1–8 sont au-dessus des écrans et les knobs
/// en dessous : la bande, tracée en bas, remonte tout en haut sous les
/// boutons et le reste descend d'autant (encodeurs juste au-dessus des knobs).
///
/// Bouton actif : pastille pleine couleur du groupe ; disponible : texte
/// clair ; sans fonction : rien (et LED éteinte).
fn finish_soft_keys(
    frame: &mut DisplayFrame,
    p: &Pal,
    labels: &[&'static str; 8],
    start: usize,
    active: &[bool; 8],
) {
    use screen_text::Face;
    let y = SCR_H - FOOTER_H;
    fill_rect(frame, 0, y - 2, SCR_W, SOFT_KEYS_H, p.bg);
    for i in 0..4 {
        let idx = start + i;
        let x = (i * 120) as f32;
        if i > 0 {
            fill_rect(frame, i * 120, y + 6, 1, 11, p.border);
        }
        let text = labels[idx];
        if text.is_empty() {
            continue;
        }
        let on = active.get(idx).copied().unwrap_or(false);
        let face = if on { Face::Bold } else { Face::Sans };
        let text = fit_text(text, face, 15.0, 104.0);
        let w = screen_text::width(&text, face, 15.0);
        if on {
            screen_vector::fill_round_rect(
                frame,
                x + 5.0,
                y as f32 + 1.0,
                110.0,
                20.0,
                4.0,
                p.accent,
            );
        }
        let fg = if on { p.ink } else { p.white };
        screen_text::draw(
            frame,
            x + 60.0 - w / 2.0,
            y as f32 + 16.0,
            &text,
            face,
            15.0,
            fg,
            1.0,
        );
    }
    frame.pixels.rotate_right(SOFT_KEYS_H * SCR_W);
}

/// Barre horizontale de valeur (piste arrondie, remplissage, curseur).
#[allow(clippy::too_many_arguments)] // helpers de dessin : coordonnées explicites
fn bar_h(
    frame: &mut DisplayFrame,
    p: &Pal,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    frac: f32,
    color: DisplayPixel,
) {
    let (xf, yf, wf, hf) = (x as f32, y as f32, w as f32, h as f32);
    let r = hf / 2.0;
    screen_vector::fill_round_rect(frame, xf, yf, wf, hf, r, p.track);
    let f = frac.clamp(0.0, 1.0) * wf;
    if f > 0.5 {
        screen_vector::fill_round_rect(frame, xf, yf, f.max(hf), hf, r, color);
    }
}

/// VU-mètre horizontal avec peak hold.
#[allow(clippy::too_many_arguments)] // helpers de dessin : coordonnées explicites
fn vu_h(
    frame: &mut DisplayFrame,
    p: &Pal,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    level: f32,
    peak: f32,
) {
    fill_rect(frame, x, y, w, h, p.track);
    let f = (level.clamp(0.0, 1.0) * w as f32) as usize;
    if f > 0 {
        fill_rect(frame, x, y, f.min(w), h, p.vu(level));
    }
    if peak > 0.02 {
        let px = (x + ((peak.clamp(0.0, 1.0) * w as f32) as usize)).min(x + w - 2);
        fill_rect(frame, px, y, 2, h, p.white);
    }
}

/// VU-mètre vertical avec peak hold.
#[allow(clippy::too_many_arguments)] // helpers de dessin : coordonnées explicites
fn vu_v(
    frame: &mut DisplayFrame,
    p: &Pal,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    level: f32,
    peak: f32,
) {
    fill_rect(frame, x, y, w, h, p.track);
    // Graduations légères (repères -6/-12/-24 dB sur l'échelle linéaire).
    for q in [25usize, 50, 75] {
        fill_rect(frame, x, y + h - h * q / 100, w, 1, p.panel);
    }
    let f = (level.clamp(0.0, 1.0) * h as f32) as usize;
    if f > 0 {
        fill_rect(frame, x, y + h - f.min(h), w, f.min(h), p.vu(level));
    }
    if peak > 0.02 {
        let pf = ((peak.clamp(0.0, 1.0) * h as f32) as usize).clamp(1, h.saturating_sub(1));
        fill_rect(frame, x, y + h - pf, w, 2, p.white);
    }
}

/// Ligne « clé → valeur » : clé à gauche (atténuée), valeur alignée à droite.
fn kv_row(
    frame: &mut DisplayFrame,
    p: &Pal,
    x: usize,
    y: usize,
    key: &str,
    value: &str,
    vcol: DisplayPixel,
) {
    draw_text(frame, x, y, &trunc(key, 15), p.muted, p.panel);
    let v = trunc(value, 18);
    let vx = (x + 196).saturating_sub(v.len() * 8);
    draw_text(frame, vx, y, &v, vcol, p.panel);
}

/// Pastille d'état (M, S, onglet de mode...).
#[allow(clippy::too_many_arguments)] // helpers de dessin : coordonnées explicites
fn pill(
    frame: &mut DisplayFrame,
    p: &Pal,
    x: usize,
    y: usize,
    w: usize,
    label: &str,
    color: DisplayPixel,
    on: bool,
) {
    use screen_text::Face;
    let (bg, fg, face) = if on {
        (color, ink_for(color), Face::Bold)
    } else {
        (p.panel2, p.muted, Face::Sans)
    };
    fill_rect(frame, x, y, w, 16, p.bg);
    screen_vector::fill_round_rect(frame, x as f32, y as f32, w as f32, 16.0, 3.0, bg);
    let text = fit_text(label, face, 13.0, w as f32 - 4.0);
    let tw = screen_text::width(&text, face, 13.0);
    screen_text::draw(
        frame,
        x as f32 + (w as f32 - tw) / 2.0,
        y as f32 + 12.5,
        &text,
        face,
        13.0,
        fg,
        1.0,
    );
}

/// Texte centré horizontalement.
fn center_text(frame: &mut DisplayFrame, y: usize, text: &str, fg: DisplayPixel, bg: DisplayPixel) {
    let x = SCR_W.saturating_sub(text.len() * 8) / 2;
    draw_text(frame, x, y, text, fg, bg);
}

/// Slot d'encodeur : (label, valeur, normalisation 0..1, couleur).
type EncSlot<'a> = Option<(&'a str, String, f32, DisplayPixel)>;

/// Centre horizontal (px) du slot d'encodeur `i` (0..4) d'un écran : le knob
/// physique est juste dessous, la GUI y aligne les siens.
#[must_use]
pub fn knob_slot_center(i: usize) -> usize {
    8 + i * 116 + 55
}

/// Rangée de 4 paramètres (bas de chaque écran, juste au-dessus des knobs) :
/// nom, valeur en grand, barre de valeur couleur du paramètre.
fn encoder_row(frame: &mut DisplayFrame, p: &Pal, slots: &[EncSlot<'_>; 4]) {
    use screen_text::Face;
    let y = SCR_H - FOOTER_H - ENC_H - 6;
    fill_rect(frame, 0, y, SCR_W, ENC_H + 6, p.bg);
    for (i, slot) in slots.iter().enumerate() {
        let x = knob_slot_center(i) - 55;
        if i > 0 {
            fill_rect(frame, x - 3, y + 8, 1, ENC_H - 14, p.border);
        }
        let Some((name, value, norm, color)) = slot else {
            continue;
        };
        // Le numéro de knob (« 1: ») n'est pas affiché : le knob est dessous.
        let name = name.split_once(':').map_or(*name, |(_, n)| n);
        let (xf, yf) = (x as f32 + 3.0, y as f32);
        let name = fit_text(name, Face::Sans, 13.0, 104.0);
        label(frame, xf, yf + 14.0, &name, 13.0, p.dim, 1.0);
        let value = fit_text(value, Face::Bold, 21.0, 104.0);
        bold(frame, xf, yf + 36.0, &value, 21.0, p.white);
        let (bw, by) = (104.0, yf + 42.0);
        screen_vector::fill_round_rect(frame, xf, by, bw, 3.0, 1.5, p.track);
        let f = norm.clamp(0.0, 1.0) * bw;
        if f > 0.5 {
            screen_vector::fill_round_rect(frame, xf, by, f.max(3.0), 3.0, 1.5, *color);
        }
        screen_vector::fill_round_rect(frame, xf + f - 1.5, by - 2.0, 3.0, 7.0, 1.5, p.white);
    }
}

// ── Accès données (snapshot) ───────────────────────────────────────────

fn focus_group(snap: &EngineSnapshot) -> Option<&crate::project::Group> {
    snap.project.groups.get(snap.group_focus as usize)
}

fn focus_sound(snap: &EngineSnapshot) -> Option<&crate::project::Sound> {
    focus_group(snap)?
        .sounds
        .get(snap.active_sound as usize)?
        .as_ref()
}

fn sound_name(snap: &EngineSnapshot, sound: usize) -> String {
    snap.project
        .groups
        .get(snap.group_focus as usize)
        .and_then(|g| g.sounds.get(sound))
        .and_then(|s| s.as_ref())
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "--".to_string())
}

fn group_name(snap: &EngineSnapshot, group: usize) -> String {
    snap.project
        .groups
        .get(group)
        .map(|g| g.name.clone())
        .unwrap_or_else(|| "--".to_string())
}

fn group_letter(group: u8) -> char {
    (b'A' + group.min(25)) as char
}

/// Pattern actif du groupe focus dans la scène active.
fn active_pattern(snap: &EngineSnapshot) -> Option<&Pattern> {
    let gi = snap.group_focus as usize;
    let g = snap.project.groups.get(gi)?;
    let pi = snap.project.active_pattern_index(gi, snap.active_scene)?;
    g.patterns.get(pi)
}

fn current_tick(snap: &EngineSnapshot) -> u64 {
    snap.transport.tick
}

fn tick_bar(snap: &EngineSnapshot) -> usize {
    ((current_tick(snap) / TICKS_PER_BAR) as usize) % snap.pattern_bars.max(1)
}

fn tick_step(snap: &EngineSnapshot) -> usize {
    ((current_tick(snap) / TICKS_PER_STEP) % 16) as usize
}

// ── Sous-blocs réutilisables ───────────────────────────────────────────

/// Forme d'onde d'un sample (enveloppe min/max lissée, couleur du groupe).
fn draw_waveform(
    frame: &mut DisplayFrame,
    p: &Pal,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    buckets: &[(f32, f32)],
) {
    panel(frame, p, x, y, w, h);
    let cy = (y + h / 2) as f32;
    fill_rect(frame, x + 6, y + h / 2, w - 12, 1, p.track);
    if buckets.is_empty() {
        let msg = "Aucun sample : BROWSER pour en charger un";
        let mw = screen_text::width(msg, screen_text::Face::Sans, 14.0);
        label(
            frame,
            (x + w / 2) as f32 - mw / 2.0,
            cy + 5.0,
            msg,
            14.0,
            p.muted,
            1.0,
        );
        return;
    }
    let n = buckets.len();
    let half = (h as f32 / 2.0 - 8.0).max(4.0);
    let px = |i: usize| x as f32 + 4.0 + (i as f32 / (n - 1).max(1) as f32) * (w as f32 - 8.0);
    let mut outline: Vec<(f32, f32)> = buckets
        .iter()
        .enumerate()
        .map(|(i, (_, mx))| (px(i), cy - mx.clamp(-1.0, 1.0) * half - 0.5))
        .collect();
    outline.extend(
        buckets
            .iter()
            .enumerate()
            .rev()
            .map(|(i, (mn, _))| (px(i), cy - mn.clamp(-1.0, 1.0) * half + 0.5)),
    );
    if let Some(path) = screen_vector::polyline(&outline) {
        screen_vector::fill(frame, &path, screen_vector::Paint::Solid(p.accent, 0.9));
    }
}

/// Enveloppe ADSR (vraies durées) : surface en fondu, tracé, points.
fn draw_adsr(
    frame: &mut DisplayFrame,
    p: &Pal,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    adsr: &Adsr,
) {
    use screen_vector::Paint;
    panel(frame, p, x, y, w, h);
    let a = (adsr.attack_ms / 500.0).clamp(0.0, 1.0);
    let d = (adsr.decay_ms / 1000.0).clamp(0.0, 1.0);
    let s = adsr.sustain.clamp(0.0, 1.0);
    let r = (adsr.release_ms / 2000.0).clamp(0.0, 1.0);
    let total = (a + d + 0.4 + r).max(0.001);
    let iw = w as f32 - 20.0;
    let x0 = x as f32 + 10.0;
    let yb = (y + h) as f32 - 24.0;
    let ytop = y as f32 + 10.0;
    let ys = yb - s * (yb - ytop);
    let xa = x0 + a / total * iw;
    let xd = xa + d / total * iw;
    let xs = xd + 0.4 / total * iw;
    let xr = xs + r / total * iw;
    let pts = [(x0, yb), (xa, ytop), (xd, ys), (xs, ys), (xr, yb)];
    if let Some(path) = screen_vector::area(&pts, yb) {
        let fade = Paint::Fade {
            color: p.accent,
            top: 0.42,
            bottom: 0.06,
            y0: ytop,
            y1: yb,
        };
        screen_vector::fill(frame, &path, fade);
    }
    if let Some(path) = screen_vector::polyline(&pts) {
        screen_vector::stroke(frame, &path, 2.0, Paint::Solid(p.accent, 1.0));
    }
    for (px, py) in pts {
        screen_vector::fill_round_rect(frame, px - 2.5, py - 2.5, 5.0, 5.0, 2.5, p.white);
    }
    let text = format!(
        "A {}   D {}   S {:.0} %   R {}",
        fmt_ms(adsr.attack_ms),
        fmt_ms(adsr.decay_ms),
        adsr.sustain * 100.0,
        fmt_ms(adsr.release_ms)
    );
    label(frame, x0, (y + h) as f32 - 7.0, &text, 13.0, p.dim, 1.0);
}

// ── ARRANGE (écran gauche) ─────────────────────────────────────────────

/// Écran gauche — vue ARRANGE : 8 groupes, pattern de la scène, VU réels.
#[must_use]
pub fn render_daw_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let playing = snap.transport.is_playing();
    let recording = snap.transport.state == TransportState::Recording;

    let scene_lbl = match snap.active_scene {
        Some(s) => format!("SCENE {}/{}", s + 1, snap.project.scenes.len().max(1)),
        None => format!("SCENE -/{}", snap.project.scenes.len().max(1)),
    };
    header(
        &mut frame,
        &p,
        snap,
        "ARRANGE",
        &format!("{scene_lbl} | {:.0} BPM", snap.transport.bpm),
    );

    // ── Bandeau transport : état, position, loop/métro (pastilles) ──
    let (state_txt, state_col, state_on) = if recording {
        ("ENREG", p.red, true)
    } else if playing {
        ("LECTURE", p.green, true)
    } else {
        ("STOP", p.track, false)
    };
    pill(&mut frame, &p, 8, 30, 80, state_txt, state_col, state_on);
    let bar = tick_bar(snap) + 1;
    let step = tick_step(snap) + 1;
    draw_text(
        &mut frame,
        98,
        31,
        &format!("BAR {bar:>2}.{step:02}"),
        if playing { p.white } else { p.dim },
        p.bg,
    );
    draw_text(
        &mut frame,
        206,
        31,
        &format!("MES {}/{}", snap.selected_bar + 1, snap.pattern_bars),
        p.dim,
        p.bg,
    );
    pill(
        &mut frame,
        &p,
        264,
        30,
        56,
        "LOOP",
        p.cyan,
        snap.transport.loop_enabled,
    );
    pill(
        &mut frame,
        &p,
        326,
        30,
        64,
        "METRO",
        p.cyan,
        snap.transport.metronome,
    );
    // Foyer courant : groupe + Sound, couleur du groupe.
    let fch = hue_color(snap.group_focus as usize);
    fill_rect(&mut frame, 390, 30, 82, 18, p.panel);
    draw_rect_outline(&mut frame, 390, 30, 82, 18, p.border);
    fill_rect(&mut frame, 390, 30, 3, 18, fch);
    draw_text(
        &mut frame,
        398,
        31,
        &format!(
            "{} S{:02}",
            group_letter(snap.group_focus),
            snap.active_sound + 1
        ),
        p.white,
        p.panel,
    );
    fill_rect(&mut frame, 0, 50, SCR_W, 1, p.border);

    // ── Liste des 8 groupes : chip couleur, nom, pattern, M/S, VU ──
    let mut y = 54;
    for gi in 0..8usize {
        let g = snap.project.groups.get(gi);
        let focused = gi == snap.group_focus as usize;
        let row_bg = if focused { p.panel2 } else { p.bg };
        if focused {
            fill_rect(&mut frame, 0, y - 2, SCR_W, 17, row_bg);
            fill_rect(&mut frame, 0, y - 2, 2, 17, p.accent);
        }
        let letter = (b'A' + gi as u8) as char;
        let gcolor = hue_color(gi);
        let has_sound = g.is_some();
        let (lbg, lfg) = if focused {
            (gcolor, p.bg)
        } else if has_sound {
            (p.panel, gcolor)
        } else {
            (p.panel, p.muted)
        };
        fill_rect(&mut frame, 5, y - 1, 14, 15, lbg);
        draw_char(&mut frame, 8, y - 1, letter, lfg, lbg);
        let name = g
            .map(|g| trunc(&g.name, 13))
            .unwrap_or_else(|| "--".to_string());
        draw_text(
            &mut frame,
            24,
            y - 1,
            &name,
            if focused {
                p.white
            } else if has_sound {
                p.dim
            } else {
                p.muted
            },
            row_bg,
        );

        let pi = snap.project.active_pattern_index(gi, snap.active_scene);
        let pat_txt = pi
            .and_then(|i| g.and_then(|g| g.patterns.get(i)))
            .map(|pat| {
                format!(
                    "{} {}m",
                    trunc(&pat.name, 9),
                    (pat.length.bars as usize).max(1)
                )
            })
            .unwrap_or_else(|| "--".to_string());
        draw_text(&mut frame, 134, y - 1, &pat_txt, p.dim, row_bg);

        // M / S : pastilles réelles (remplies quand actives).
        let muted = snap.muted_groups & (1 << gi) != 0;
        let soloed = snap.soloed_groups & (1 << gi) != 0;
        pill(&mut frame, &p, 244, y - 1, 16, "M", p.red, muted);
        pill(&mut frame, &p, 264, y - 1, 16, "S", p.yellow, soloed);

        vu_h(
            &mut frame,
            &p,
            288,
            y + 2,
            142,
            9,
            snap.group_levels[gi],
            snap.group_peaks[gi],
        );
        let vol = g.map(|g| g.volume_db).unwrap_or(0.0);
        draw_text(
            &mut frame,
            438,
            y - 1,
            &format!("{vol:>4.0}"),
            if focused { p.white } else { p.muted },
            row_bg,
        );
        y += 17;
    }

    // Encodeurs réels 1..4 : TEMPO, SWING, VOL GRP, PAN GRP.
    let gvol = focus_group(snap).map(|g| g.volume_db).unwrap_or(0.0);
    let gpan = focus_group(snap).map(|g| g.pan).unwrap_or(0.0);
    let slots = [
        Some((
            "1:TEMPO",
            format!("{:.1}", snap.transport.bpm),
            ((snap.transport.bpm - 40.0) / 200.0).clamp(0.0, 1.0),
            p.accent,
        )),
        Some((
            "2:SWING",
            format!("{:.0}%", snap.project.swing * 100.0),
            snap.project.swing.clamp(0.0, 1.0),
            p.accent,
        )),
        Some(("3:VOL GRP", fmt_db(gvol), db_norm(gvol), p.cyan)),
        Some(("4:PAN GRP", fmt_pan(gpan), (gpan + 1.0) * 0.5, p.cyan)),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
    frame
}

// ── PATTERN (écran gauche, mode PATTERN) ───────────────────────────────

/// Écran gauche — mode PATTERN : les 16 slots du groupe focus.
#[must_use]
pub fn render_pattern_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let group = focus_group(snap);
    let count = group.map(|g| g.patterns.len()).unwrap_or(0);
    let active = snap
        .project
        .active_pattern_index(snap.group_focus as usize, snap.active_scene);
    let scene_lbl = match snap.active_scene {
        Some(s) => format!("SCENE {}/{}", s + 1, snap.project.scenes.len().max(1)),
        None => format!("SCENE -/{}", snap.project.scenes.len().max(1)),
    };
    header(
        &mut frame,
        &p,
        snap,
        "PATTERN / SLOTS",
        &format!(
            "GRP {} | {} PAT | {}",
            group_letter(snap.group_focus),
            count,
            scene_lbl
        ),
    );

    let y = 30usize;
    for slot in 0..16usize {
        let col = slot / 8;
        let row = slot % 8;
        let x = 8 + col * 236;
        let cy = y + row * 19;
        let exists = slot < count;
        let focused = active == Some(slot);
        let assigned = snap
            .active_scene
            .and_then(|si| snap.project.scenes.get(si))
            .and_then(|sc| sc.patterns.get(snap.group_focus as usize))
            .copied()
            .flatten()
            == Some(slot);
        let bg = if focused { p.panel2 } else { p.panel };
        fill_rect(&mut frame, x, cy, 226, 17, bg);
        draw_rect_outline(
            &mut frame,
            x,
            cy,
            226,
            17,
            if focused { p.accent } else { p.border },
        );
        if focused {
            fill_rect(&mut frame, x, cy, 3, 17, p.accent);
        }
        draw_text(
            &mut frame,
            x + 7,
            cy + 4,
            &format!("P{:02}", slot + 1),
            if focused {
                p.accent
            } else if exists {
                p.dim
            } else {
                p.muted
            },
            bg,
        );

        if let Some(pat) = group.and_then(|g| g.patterns.get(slot)) {
            draw_text(
                &mut frame,
                x + 38,
                cy + 4,
                &trunc(&pat.name, 13),
                if focused { p.white } else { p.dim },
                bg,
            );
            draw_text(
                &mut frame,
                x + 148,
                cy + 4,
                &format!("{}m", pat.length.bars.max(1)),
                p.cyan,
                bg,
            );
            draw_text(
                &mut frame,
                x + 178,
                cy + 4,
                &format!("{:>3}ev", pat.events.len().min(999)),
                p.muted,
                bg,
            );
            if assigned {
                fill_rect(&mut frame, x + 218, cy + 5, 3, 7, p.green);
            }
        } else {
            draw_text(&mut frame, x + 38, cy + 4, "— libre —", p.muted, bg);
            draw_text(&mut frame, x + 178, cy + 4, "PAD=+", p.muted, bg);
        }
    }

    draw_text(
        &mut frame,
        8,
        184,
        "PAD VIDE = CREER | DUPLICATE = COPIER | SHIFT+ERASE = VIDER",
        p.muted,
        p.bg,
    );

    let gvol = focus_group(snap).map(|g| g.volume_db).unwrap_or(0.0);
    let gpan = focus_group(snap).map(|g| g.pan).unwrap_or(0.0);
    let slots = [
        Some((
            "1:TEMPO",
            format!("{:.1}", snap.transport.bpm),
            ((snap.transport.bpm - 40.0) / 200.0).clamp(0.0, 1.0),
            p.accent,
        )),
        Some((
            "2:SWING",
            format!("{:.0}%", snap.project.swing * 100.0),
            snap.project.swing.clamp(0.0, 1.0),
            p.accent,
        )),
        Some(("3:VOL GRP", fmt_db(gvol), db_norm(gvol), p.cyan)),
        Some(("4:PAN GRP", fmt_pan(gpan), (gpan + 1.0) * 0.5, p.cyan)),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
    frame
}

// ── PATTERN (écran droit) ──────────────────────────────────────────────

/// Écran droit — vue PATTERN : grille 16 Sons x 16 pas de la mesure choisie.
#[must_use]
pub fn render_daw_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let playing = snap.transport.is_playing();
    let pat = active_pattern(snap);
    let (pat_name, n_events) = match pat {
        Some(pt) => (trunc(&pt.name, 14), pt.events.len()),
        None => ("--".to_string(), 0),
    };
    header(
        &mut frame,
        &p,
        snap,
        "PATTERN",
        &format!(
            "GRP {} | {} | {} ev",
            group_letter(snap.group_focus),
            pat_name,
            n_events
        ),
    );

    // Masque d'événements de la mesure sélectionnée (16 sons × 16 pas) avec
    // la vélocité max par case (hauteur de cellule).
    let bar_offset = snap.selected_bar as u32 * TICKS_PER_BAR as u32;
    let mut mask = [[0u8; 16]; 16];
    if let Some(pt) = pat {
        for e in &pt.events {
            if e.tick >= bar_offset && e.tick < bar_offset + TICKS_PER_BAR as u32 {
                let st = ((e.tick - bar_offset) / TICKS_PER_STEP as u32) as usize;
                let pad = e.pad as usize;
                if st < 16 && pad < 16 {
                    mask[pad][st] = mask[pad][st].max(e.velocity);
                }
            }
        }
    }

    let gx = 84usize;
    let cw = 24usize;
    let cur_bar = tick_bar(snap);
    let cur_step = tick_step(snap);
    let show_playhead = playing && cur_bar == snap.selected_bar;

    // Règle des 16 pas : numéros, temps forts plus clairs, barres de temps.
    for s in 0..16usize {
        let x = gx + s * cw;
        let quad = s % 4 == 0;
        draw_text(
            &mut frame,
            x + 8,
            30,
            &format!("{}", s + 1),
            if quad { p.dim } else { p.muted },
            p.bg,
        );
        if quad {
            fill_rect(&mut frame, x - 2, 44, 1, 148, p.track);
        }
    }
    // Séparation règle / grille.
    fill_rect(&mut frame, 0, 47, SCR_W, 1, p.border);
    // Barre du header : le pas courant (repère supérieur).
    if show_playhead {
        fill_rect(&mut frame, gx + cur_step * cw - 2, 44, cw, 3, p.accent);
        fill_rect(&mut frame, gx + cur_step * cw - 2, 47, cw, 1, p.border);
    }

    let mut y = 50;
    for (snd, row) in mask.iter().enumerate() {
        let is_active = snd == snap.active_sound as usize;
        let row_bg = if is_active { p.panel2 } else { p.bg };
        if is_active {
            fill_rect(&mut frame, 0, y - 2, SCR_W, 9, row_bg);
        }
        let row_used =
            focus_group(snap).is_some_and(|g| g.sounds.get(snd).is_some_and(Option::is_some));
        // Puce couleur du Sound (slot vide = piste) : 9 px de pas ne laissent
        // pas lire une police 8×16 ; le nom du Sound actif suit la boucle.
        let chip = if row_used { hue_color(snd) } else { p.track };
        cell(&mut frame, 6, y - 2, 12, 7, chip, row_bg);
        // LED d'activité du Sound (jeu + séquenceur).
        let act = snap.pad_activity[snd].clamp(0.0, 1.0);
        if act > 0.02 {
            let (r, g, b) = rgb565_to_rgb(hue_color(snd));
            let f = 0.4 + 0.6 * act;
            let col = DisplayPixel::from_rgb(
                (r as f32 * f) as u8,
                (g as f32 * f) as u8,
                (b as f32 * f) as u8,
            );
            fill_rect(&mut frame, 76, y - 4, 3, 8, col);
        }
        // Grille de pas : case discrète partout (plus claire sur les temps),
        // note pleine dans la couleur du Sound, intensité selon la vélocité.
        for (st, vel) in row.iter().enumerate() {
            let x = gx + st * cw + 1;
            let color = if *vel > 0 {
                mix(
                    p.track,
                    hue_color(snd),
                    0.45 + 0.55 * f32::from(*vel) / 127.0,
                )
            } else if !row_used {
                p.panel
            } else if st % 4 == 0 {
                p.track
            } else {
                p.panel2
            };
            cell(&mut frame, x, y - 2, cw - 4, 7, color, row_bg);
        }
        y += 9;
    }

    // Tête de lecture : colonne voilée de la couleur du groupe.
    if show_playhead {
        let x = (gx + cur_step * cw - 1) as f32;
        if let Some(path) = screen_vector::round_rect(x, 47.0, cw as f32, 146.0, 3.0) {
            screen_vector::fill(
                &mut frame,
                &path,
                screen_vector::Paint::Solid(p.accent, 0.22),
            );
        }
    }

    // Nom du Sound actif, lisible, posé sur sa ligne.
    let active = (snap.active_sound as usize).min(15);
    let ly = (50 + active * 9).saturating_sub(7).min(190 - 16);
    fill_rect(&mut frame, 0, ly, 74, 16, p.panel2);
    fill_rect(&mut frame, 0, ly, 2, 16, hue_color(active));
    let name = format!("{:02} {}", active + 1, trunc(&sound_name(snap, active), 5));
    draw_text(&mut frame, 4, ly, &name, p.white, p.panel2);

    // Encodeurs réels 5..8 : VOL SND, CUTOFF, RESO, MASTER.
    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let slots = [
        Some(("5:VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some((
            "6:CUTOFF",
            fmt_hz(cut),
            ((cut - 20.0) / 19980.0).clamp(0.0, 1.0),
            p.cyan,
        )),
        Some(("7:RESO", format!("{:.0}%", reso * 100.0), reso, p.cyan)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

// ── STEP (écran droit, mode STEP) ──────────────────────────────────────

/// Écran droit — vue STEP : pas du Sound actif (vélocités) de la mesure.
#[must_use]
pub fn render_step_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let playing = snap.transport.is_playing();
    let name = sound_name(snap, snap.active_sound as usize);
    let total_steps = snap.pattern_bars.max(1) * 16;
    let cursor = snap.step_cursor.min(total_steps.saturating_sub(1));
    header(
        &mut frame,
        &p,
        snap,
        "STEP / GRILLE",
        &format!(
            "PAS {:02}/{:02} M{} | GRP {} SND {:02} {}",
            cursor + 1,
            total_steps,
            snap.selected_bar + 1,
            group_letter(snap.group_focus),
            snap.active_sound + 1,
            trunc(&name, 10)
        ),
    );

    let pat = active_pattern(snap);
    let bar_offset = snap.selected_bar as u32 * TICKS_PER_BAR as u32;
    let cur_bar = tick_bar(snap);
    let cur_step = tick_step(snap);
    let show_playhead = playing && cur_bar == snap.selected_bar;
    let cursor_step = cursor % 16;
    let show_cursor = cursor / 16 == snap.selected_bar;
    let pad_hue = hue_color(snap.active_sound as usize);

    // ── Bandeau du pas édité : position, vélocité, longueur, note ──
    let sel_ev = pat.and_then(|pt| {
        let start = cursor as u32 * TICKS_PER_STEP as u32;
        pt.events.iter().find(|e| {
            e.pad == snap.active_sound && e.tick >= start && e.tick < start + TICKS_PER_STEP as u32
        })
    });
    panel(&mut frame, &p, 8, 30, SCR_W - 16, 20);
    fill_rect(&mut frame, 8, 30, 3, 20, pad_hue);
    draw_text(
        &mut frame,
        18,
        32,
        &format!("M{} PAS {:02}", snap.selected_bar + 1, cursor_step + 1),
        p.white,
        p.panel,
    );
    let (vel_txt, len_txt, note_txt) = match sel_ev {
        Some(ev) => (
            format!("{:>3}", ev.velocity),
            fmt_len(ev.length_ticks),
            format!("{} {}", ev.pitch, note_name(ev.pitch)),
        ),
        None => ("---".to_string(), "--".to_string(), "--".to_string()),
    };
    draw_text(
        &mut frame,
        150,
        32,
        &format!("VEL {vel_txt}"),
        p.cyan,
        p.panel,
    );
    draw_text(
        &mut frame,
        246,
        32,
        &format!("LEN {len_txt}"),
        p.dim,
        p.panel,
    );
    draw_text(
        &mut frame,
        356,
        32,
        &format!("NOTE {note_txt}"),
        p.dim,
        p.panel,
    );

    // ── Grille des 16 pas : hauteur ∝ vélocité ──
    let x0 = 16usize;
    let cw = 28usize;
    let top = 56usize;
    let bot = 168usize;
    let pad_area_h = (bot - top) as f32;
    for st in 0..16usize {
        let x = x0 + st * cw;
        let cell_start = bar_offset + st as u32 * TICKS_PER_STEP as u32;
        let vel = pat
            .and_then(|pt| {
                pt.events.iter().find(|e| {
                    e.pad == snap.active_sound
                        && e.tick >= cell_start
                        && e.tick < cell_start + TICKS_PER_STEP as u32
                })
            })
            .map(|e| e.velocity)
            .unwrap_or(0);
        // Fond : temps forts plus contrastés (repère de lecture).
        let bg = if st % 4 == 0 { p.panel } else { p.bg };
        fill_rect(&mut frame, x, top, cw - 2, bot - top, bg);
        if vel > 0 {
            let fh = ((pad_area_h * (vel as f32 / 127.0)) as usize).max(5);
            fill_rect(&mut frame, x + 1, bot - fh, cw - 4, fh, pad_hue);
            fill_rect(&mut frame, x + 1, bot - fh, cw - 4, 1, p.white);
            // Valeur de vélocité au-dessus de la barre (lisible sans cadre).
            let vt = format!("{vel}");
            let tx = x + (cw - 2).saturating_sub(vt.len() * 8) / 2;
            draw_text(
                &mut frame,
                tx,
                (bot - fh).saturating_sub(17),
                &vt,
                p.white,
                if (bot - fh) >= 17 { p.bg } else { pad_hue },
            );
        }
        draw_rect_outline(
            &mut frame,
            x,
            top,
            cw - 2,
            bot - top,
            if st % 4 == 0 { p.border } else { p.track },
        );
        draw_text(
            &mut frame,
            x + 8,
            172,
            &format!("{}", st + 1),
            if st % 4 == 0 { p.dim } else { p.muted },
            p.bg,
        );
        if show_playhead && st == cur_step {
            fill_rect(&mut frame, x, top - 5, cw - 2, 3, p.accent);
            draw_rect_outline(&mut frame, x - 1, top - 1, cw, bot - top + 2, p.white);
        } else if show_cursor && st == cursor_step {
            // Curseur d'édition (pas sélectionné par les pads / le scroll).
            fill_rect(&mut frame, x, top - 5, cw - 2, 3, pad_hue);
            draw_rect_outline(&mut frame, x - 1, top - 1, cw, bot - top + 2, p.accent);
            draw_rect_outline(&mut frame, x - 2, top - 2, cw + 2, bot - top + 4, p.accent);
        }
    }

    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    let cursor_norm = cursor as f32 / total_steps.saturating_sub(1).max(1) as f32;
    let slots = [
        Some((
            "5:SCROLL",
            format!("PAS {:02}/{:02}", cursor + 1, total_steps),
            cursor_norm,
            p.accent,
        )),
        Some(("6:VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some((
            "7:CUTOFF",
            fmt_hz(cut),
            ((cut - 20.0) / 19980.0).clamp(0.0, 1.0),
            p.cyan,
        )),
        Some(("8:RESO", format!("{:.0}%", reso * 100.0), reso, p.cyan)),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

/// Écran gauche — vue STEP : pas sélectionné + grille du pattern complet.
#[must_use]
pub fn render_step_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let total_steps = snap.pattern_bars.max(1) * 16;
    let cursor = snap.step_cursor.min(total_steps.saturating_sub(1));
    let name = sound_name(snap, snap.active_sound as usize);
    header(
        &mut frame,
        &p,
        snap,
        "STEP / PAS",
        &format!(
            "PAS {:02}/{:02} | GRP {} | SND {:02} {}",
            cursor + 1,
            total_steps,
            group_letter(snap.group_focus),
            snap.active_sound + 1,
            trunc(&name, 10)
        ),
    );

    let pat = active_pattern(snap);
    let pad_hue = hue_color(snap.active_sound as usize);
    let playing = snap.transport.is_playing();
    let cur_bar = tick_bar(snap);
    let cur_step = tick_step(snap);

    // Pas édité : événement du son actif dans la fenêtre du curseur.
    let sel = pat.and_then(|pt| {
        let start = cursor as u32 * TICKS_PER_STEP as u32;
        pt.events.iter().find(|e| {
            e.pad == snap.active_sound && e.tick >= start && e.tick < start + TICKS_PER_STEP as u32
        })
    });

    // Grille du pattern complet : une ligne par mesure, 16 cases par ligne.
    let rows = snap.pattern_bars.clamp(1, 8);
    let row_h = if rows <= 4 { 22usize } else { 16 };
    let top = 30usize;
    let x0 = 44usize;
    let cw = 26usize;
    for bar in 0..rows {
        let y = top + bar * row_h;
        let focused_bar = bar == cursor / 16;
        if focused_bar {
            fill_rect(&mut frame, 4, y - 1, SCR_W - 8, row_h, p.panel);
        }
        draw_text(
            &mut frame,
            8,
            y + (row_h.saturating_sub(16)) / 2,
            &format!("M{}", bar + 1),
            if focused_bar { p.accent } else { p.muted },
            if focused_bar { p.panel } else { p.bg },
        );
        for st in 0..16usize {
            let x = x0 + st * cw;
            let cell_start = (bar * 16 + st) as u32 * TICKS_PER_STEP as u32;
            let has = pat
                .map(|pt| {
                    pt.events.iter().any(|e| {
                        e.pad == snap.active_sound
                            && e.tick >= cell_start
                            && e.tick < cell_start + TICKS_PER_STEP as u32
                    })
                })
                .unwrap_or(false);
            let ch = row_h - 4;
            let cy = y + 1;
            let cell_bg = if has {
                pad_hue
            } else if st % 4 == 0 {
                p.panel2
            } else {
                p.track
            };
            fill_rect(&mut frame, x, cy, cw - 2, ch, cell_bg);
            if st % 4 == 0 {
                draw_rect_outline(&mut frame, x, cy, cw - 2, ch, p.border);
            }
            let is_cursor = bar * 16 + st == cursor;
            let is_playhead = playing && cur_bar == bar && cur_step == st;
            if is_playhead {
                draw_rect_outline(&mut frame, x - 1, cy - 1, cw, ch + 2, p.white);
            } else if is_cursor {
                draw_rect_outline(&mut frame, x - 1, cy - 1, cw, ch + 2, p.accent);
                draw_rect_outline(&mut frame, x - 2, cy - 2, cw + 2, ch + 4, p.accent);
            }
        }
    }

    // Détail du pas sélectionné.
    let info_y = top + rows * row_h + 4;
    match sel {
        Some(ev) => {
            let cell_start = (ev.tick / TICKS_PER_STEP as u32) * TICKS_PER_STEP as u32;
            let off = ev.tick as i32 - cell_start as i32;
            draw_text(
                &mut frame,
                8,
                info_y,
                &format!(
                    "VEL {}  POS {:+}t  LEN {}  PITCH {} {}",
                    ev.velocity,
                    off,
                    fmt_len(ev.length_ticks),
                    ev.pitch,
                    note_name(ev.pitch)
                ),
                p.white,
                p.bg,
            );
        }
        None => {
            draw_text(
                &mut frame,
                8,
                info_y,
                "Pas vide : appuie sur un pad pour le poser",
                p.muted,
                p.bg,
            );
        }
    }
    draw_text(
        &mut frame,
        8,
        info_y + 14,
        "Knobs 1–4 : le pas · knob 5 / 4D : défiler · pads : poser",
        p.muted,
        p.bg,
    );

    // Carte du pas sélectionné : remplit l'espace libre des patterns courts.
    let card_top = info_y + 30;
    let card_bottom = SCR_H - FOOTER_H - ENC_H - 8;
    if card_bottom > card_top + 24 {
        let card_h = card_bottom - card_top;
        fill_rect(&mut frame, 6, card_top, SCR_W - 12, card_h, p.panel);
        draw_rect_outline(&mut frame, 6, card_top, SCR_W - 12, card_h, p.border);
        draw_text_scaled(
            &mut frame,
            18,
            card_top + 11,
            &format!("PAS {:02}", cursor + 1),
            2,
            p.white,
            p.panel,
        );
        let vel = sel.map(|e| e.velocity).unwrap_or(0);
        draw_text(&mut frame, 128, card_top + 8, "VELOCITE", p.muted, p.panel);
        bar_h(
            &mut frame,
            &p,
            128,
            card_top + 22,
            220,
            10,
            vel as f32 / 127.0,
            p.cyan,
        );
        draw_text(
            &mut frame,
            360,
            card_top + 8,
            &format!("{vel:>3}"),
            p.white,
            p.panel,
        );
        if card_h >= 56 {
            let detail = match sel {
                Some(ev) => format!(
                    "MES {} PAS {}  NOTE {} {}  LEN {}",
                    cursor / 16 + 1,
                    cursor % 16 + 1,
                    ev.pitch,
                    note_name(ev.pitch),
                    fmt_len(ev.length_ticks)
                ),
                None => format!(
                    "MES {} PAS {}  AUCUN PAS ICI",
                    cursor / 16 + 1,
                    cursor % 16 + 1
                ),
            };
            draw_text(&mut frame, 18, card_top + 40, &detail, p.dim, p.panel);
        }
    }

    let (vel_s, vel_n) = sel
        .map(|e| (format!("{}", e.velocity), e.velocity as f32 / 127.0))
        .unwrap_or_else(|| ("--".to_string(), 0.0));
    let (pos_s, pos_n) = sel
        .map(|e| {
            let cell_start = (e.tick / TICKS_PER_STEP as u32) * TICKS_PER_STEP as u32;
            let off = e.tick as i32 - cell_start as i32;
            (format!("{off:+}t"), 0.5 + off as f32 / 480.0)
        })
        .unwrap_or_else(|| ("--".to_string(), 0.5));
    let (len_s, len_n) = sel
        .map(|e| {
            (
                fmt_len(e.length_ticks),
                (e.length_ticks as f32 / 1920.0).clamp(0.0, 1.0),
            )
        })
        .unwrap_or_else(|| ("--".to_string(), 0.0));
    let (pit_s, pit_n) = sel
        .map(|e| {
            (
                format!("{} {}", e.pitch, note_name(e.pitch)),
                ((e.pitch as f32 - 24.0) / 72.0).clamp(0.0, 1.0),
            )
        })
        .unwrap_or_else(|| ("--".to_string(), 0.0));
    let slots = [
        Some(("1:VELOCITE", vel_s, vel_n, p.cyan)),
        Some(("2:POSITION", pos_s, pos_n, p.cyan)),
        Some(("3:LONGUEUR", len_s, len_n, p.cyan)),
        Some(("4:PITCH", pit_s, pit_n, p.cyan)),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
    frame
}

// ── SONG / ARRANGER ────────────────────────────────────────────────────

/// Écran gauche — vue SONG : sections de l'arrangement.
#[must_use]
pub fn render_arranger_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let active_sec = snap.active_section.unwrap_or(0);
    header(
        &mut frame,
        &p,
        snap,
        "SONG",
        &format!(
            "{}/{} sections | {:.0} BPM",
            (active_sec + 1).min(snap.project.song.len().max(1)),
            snap.project.song.len(),
            snap.transport.bpm
        ),
    );

    if snap.project.song.is_empty() {
        center_text(&mut frame, 70, "AUCUNE SECTION", p.muted, p.bg);
        center_text(&mut frame, 94, "SOFT 3 = +SECTION", p.dim, p.bg);
    }

    let mut y = 30usize;
    for (si, sec) in snap.project.song.iter().enumerate().take(8) {
        let focused = si == active_sec;
        let row_bg = if focused { p.panel2 } else { p.bg };
        if focused {
            fill_rect(&mut frame, 0, y - 2, SCR_W, 19, row_bg);
            fill_rect(&mut frame, 0, y - 2, 3, 19, p.accent);
        }
        let scene_name = snap
            .project
            .scenes
            .get(sec.scene)
            .map(|s| trunc(&s.name, 12))
            .unwrap_or_else(|| "--".to_string());
        draw_text(
            &mut frame,
            10,
            y,
            &format!("{:02} {}", si + 1, trunc(&sec.name, 14)),
            if focused { p.white } else { p.dim },
            row_bg,
        );
        draw_text(
            &mut frame,
            160,
            y,
            &format!("SCENE {} {}", sec.scene + 1, scene_name),
            p.dim,
            row_bg,
        );
        draw_text(
            &mut frame,
            320,
            y,
            &format!("{} mes.", (sec.length.bars as usize).max(1)),
            p.dim,
            row_bg,
        );
        if focused && snap.transport.is_playing() {
            draw_text(&mut frame, 424, y, "LECT", p.green, row_bg);
        }
        y += 19;
    }

    let gvol = focus_group(snap).map(|g| g.volume_db).unwrap_or(0.0);
    let gpan = focus_group(snap).map(|g| g.pan).unwrap_or(0.0);
    let slots = [
        Some((
            "1:TEMPO",
            format!("{:.1}", snap.transport.bpm),
            ((snap.transport.bpm - 40.0) / 200.0).clamp(0.0, 1.0),
            p.accent,
        )),
        Some((
            "2:SWING",
            format!("{:.0}%", snap.project.swing * 100.0),
            snap.project.swing.clamp(0.0, 1.0),
            p.accent,
        )),
        Some(("3:VOL GRP", fmt_db(gvol), db_norm(gvol), p.cyan)),
        Some(("4:PAN GRP", fmt_pan(gpan), (gpan + 1.0) * 0.5, p.cyan)),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
    frame
}

/// Écran droit — vue SONG : timeline des sections + contenu de la section.
#[must_use]
pub fn render_arranger_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let total_bars: u64 = snap
        .project
        .song
        .iter()
        .map(|s| (s.length.bars as u64).max(1))
        .sum();
    header(
        &mut frame,
        &p,
        snap,
        "SONG TIMELINE",
        &format!("{} mesures | BAR {}", total_bars.max(1), tick_bar(snap) + 1),
    );

    // Blocs de sections proportionnels + playhead.
    let x0 = 8usize;
    let tw = 464usize;
    let mut x = x0;
    let mut acc = 0u64;
    let tick = current_tick(snap);
    let mut playhead_x: Option<usize> = None;
    let active_sec = snap.active_section.unwrap_or(0);
    for (si, sec) in snap.project.song.iter().enumerate() {
        let bars = (sec.length.bars as u64).max(1);
        let bw = ((bars * tw as u64) / total_bars.max(1)).max(2) as usize;
        let focused = si == active_sec;
        let bg = if focused { p.panel2 } else { p.panel };
        fill_rect(&mut frame, x, 30, bw.saturating_sub(2), 40, bg);
        draw_rect_outline(
            &mut frame,
            x,
            30,
            bw.saturating_sub(2),
            40,
            if focused { p.accent } else { p.border },
        );
        if bw >= 24 {
            draw_text(
                &mut frame,
                x + 4,
                34,
                &trunc(&sec.name, (bw - 10) / 8),
                if focused { p.white } else { p.dim },
                bg,
            );
            draw_text(
                &mut frame,
                x + 4,
                52,
                &format!("S{}", sec.scene + 1),
                p.muted,
                bg,
            );
        }
        if tick >= acc * TICKS_PER_BAR && tick < (acc + bars) * TICKS_PER_BAR {
            let inner =
                (tick - acc * TICKS_PER_BAR) * bw.saturating_sub(2) as u64 / (bars * TICKS_PER_BAR);
            playhead_x = Some(x + inner as usize);
        }
        x += bw;
        acc += bars;
    }
    if let Some(px) = playhead_x {
        fill_rect(&mut frame, px, 26, 2, 48, p.white);
    }
    fill_rect(&mut frame, 0, 74, SCR_W, 1, p.border);

    // Contenu de la section active : scène + patterns par groupe.
    if let Some(sec) = snap.project.song.get(active_sec) {
        let scene = snap.project.scenes.get(sec.scene);
        draw_text(
            &mut frame,
            8,
            80,
            &format!(
                "SECTION {} {} | SCENE {} {} | {} mes.",
                active_sec + 1,
                trunc(&sec.name, 10),
                sec.scene + 1,
                scene
                    .map(|s| trunc(&s.name, 10))
                    .unwrap_or_else(|| "--".into()),
                (sec.length.bars as usize).max(1)
            ),
            p.white,
            p.bg,
        );
        let mut y = 100usize;
        for gi in 0..8usize {
            let focus = gi == snap.group_focus as usize;
            let row_bg = if focus { p.panel2 } else { p.bg };
            if focus {
                fill_rect(&mut frame, 0, y - 2, SCR_W, 12, row_bg);
            }
            draw_text(
                &mut frame,
                8,
                y - 2,
                &format!(
                    "{}  {}",
                    group_letter(gi as u8),
                    trunc(&group_name(snap, gi), 10)
                ),
                if focus { p.white } else { p.dim },
                row_bg,
            );
            let pat_txt = scene
                .and_then(|s| s.patterns.get(gi).copied().flatten())
                .and_then(|pi| snap.project.groups.get(gi)?.patterns.get(pi))
                .map(|pat| trunc(&pat.name, 12))
                .unwrap_or_else(|| "--".to_string());
            draw_text(&mut frame, 180, y - 2, &pat_txt, p.dim, row_bg);
            y += 12;
        }
    } else {
        center_text(&mut frame, 100, "AUCUNE SECTION", p.muted, p.bg);
    }

    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    let slots = [
        Some(("5:VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some((
            "6:CUTOFF",
            fmt_hz(cut),
            ((cut - 20.0) / 19980.0).clamp(0.0, 1.0),
            p.cyan,
        )),
        Some(("7:RESO", format!("{:.0}%", reso * 100.0), reso, p.cyan)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

// ── SCENES / IDEAS ─────────────────────────────────────────────────────

/// Écran gauche — vue SCENES (IDEAS) : scènes + pads = scènes.
#[must_use]
pub fn render_scenes_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    header(
        &mut frame,
        &p,
        snap,
        "IDEAS | SCENES",
        &format!("{} scenes | PADS = SCENES", snap.project.scenes.len()),
    );

    let mut y = 30usize;
    for (i, scene) in snap.project.scenes.iter().enumerate().take(16) {
        let col = i / 8;
        let row = i % 8;
        let x = 8 + col * 236;
        let cy = y + row * 19;
        let focused = snap.active_scene == Some(i);
        let bg = if focused { p.panel2 } else { p.panel };
        fill_rect(&mut frame, x, cy, 226, 17, bg);
        draw_rect_outline(
            &mut frame,
            x,
            cy,
            226,
            17,
            if focused { p.accent } else { p.border },
        );
        if focused {
            fill_rect(&mut frame, x, cy, 3, 17, p.accent);
        }
        draw_text(
            &mut frame,
            x + 8,
            cy + 1,
            &format!("{:02} {}", i + 1, trunc(&scene.name, 13)),
            if focused { p.white } else { p.dim },
            bg,
        );
        let count = scene.patterns.iter().filter(|p| p.is_some()).count();
        let secs = snap.project.song.iter().filter(|s| s.scene == i).count();
        draw_text(
            &mut frame,
            x + 150,
            cy + 1,
            &format!("{count} pat"),
            p.muted,
            bg,
        );
        if secs > 0 {
            draw_text(&mut frame, x + 196, cy + 1, &format!("{secs}s"), p.cyan, bg);
        }
    }
    y += 8 * 19;
    draw_text(
        &mut frame,
        8,
        y + 2,
        "SOFT 1-8 : SCENES | PADS : SELECTION DIRECTE",
        p.muted,
        p.bg,
    );
    let _ = y;

    let gvol = focus_group(snap).map(|g| g.volume_db).unwrap_or(0.0);
    let gpan = focus_group(snap).map(|g| g.pan).unwrap_or(0.0);
    let slots = [
        Some((
            "1:TEMPO",
            format!("{:.1}", snap.transport.bpm),
            ((snap.transport.bpm - 40.0) / 200.0).clamp(0.0, 1.0),
            p.accent,
        )),
        Some((
            "2:SWING",
            format!("{:.0}%", snap.project.swing * 100.0),
            snap.project.swing.clamp(0.0, 1.0),
            p.accent,
        )),
        Some(("3:VOL GRP", fmt_db(gvol), db_norm(gvol), p.cyan)),
        Some(("4:PAN GRP", fmt_pan(gpan), (gpan + 1.0) * 0.5, p.cyan)),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
    frame
}

/// Écran droit — vue SCENES : matrice Groupes × Patterns pour la scène active.
#[must_use]
pub fn render_scenes_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);

    let active_idx = snap.active_scene.unwrap_or(0);
    let scene_opt = snap.project.scenes.get(active_idx);
    let scene_name = scene_opt.map(|s| s.name.as_str()).unwrap_or("--");
    let n_active_groups = scene_opt
        .map(|s| s.patterns.iter().filter(|p| p.is_some()).count())
        .unwrap_or(0);
    let n_sections = snap
        .project
        .song
        .iter()
        .filter(|s| s.scene == active_idx)
        .count();

    header(
        &mut frame,
        &p,
        snap,
        "MATRICE SCÈNE",
        &format!(
            "SCENE {} \"{}\" | {} GRP | {} SEC",
            active_idx + 1,
            trunc(scene_name, 12),
            n_active_groups,
            n_sections
        ),
    );

    // Titres de colonnes
    draw_text(&mut frame, 8, 30, "GROUPE", p.muted, p.bg);
    draw_text(&mut frame, 120, 30, "PATTERN ASSIGNÉ", p.muted, p.bg);
    draw_text(&mut frame, 310, 30, "MESURES", p.muted, p.bg);
    draw_text(&mut frame, 390, 30, "NOTES", p.muted, p.bg);
    fill_rect(&mut frame, 0, 42, SCR_W, 1, p.border);

    let mut y = 45usize;
    for gi in 0..8usize {
        let focus = gi == snap.group_focus as usize;
        let row_bg = if focus { p.panel2 } else { p.panel };
        let outline_col = if focus { p.accent } else { p.border };
        fill_rect(&mut frame, 6, y, SCR_W - 12, 16, row_bg);
        draw_rect_outline(&mut frame, 6, y, SCR_W - 12, 16, outline_col);
        if focus {
            fill_rect(&mut frame, 6, y, 3, 16, p.accent);
        }

        draw_text(
            &mut frame,
            14,
            y + 2,
            &format!(
                "{} {}",
                group_letter(gi as u8),
                trunc(&group_name(snap, gi), 8)
            ),
            if focus { p.white } else { p.dim },
            row_bg,
        );

        let pat_info = scene_opt
            .and_then(|s| s.patterns.get(gi).copied().flatten())
            .and_then(|pi| {
                snap.project
                    .groups
                    .get(gi)
                    .and_then(|g| g.patterns.get(pi))
                    .map(|pat| (pi, pat))
            });

        match pat_info {
            Some((pi, pat)) => {
                draw_text(
                    &mut frame,
                    120,
                    y + 2,
                    &format!("P{:02} {}", pi + 1, trunc(&pat.name, 13)),
                    if focus { p.cyan } else { p.white },
                    row_bg,
                );
                draw_text(
                    &mut frame,
                    310,
                    y + 2,
                    &format!("{}m", pat.length.bars.max(1)),
                    p.dim,
                    row_bg,
                );
                draw_text(
                    &mut frame,
                    390,
                    y + 2,
                    &format!("{:>3}ev", pat.events.len().min(999)),
                    p.muted,
                    row_bg,
                );
            }
            None => {
                draw_text(
                    &mut frame,
                    120,
                    y + 2,
                    "-- aucun pattern (muet) --",
                    p.muted,
                    row_bg,
                );
            }
        }
        y += 17;
    }

    draw_text(
        &mut frame,
        8,
        y + 2,
        "GROUP A-H = FOCUS | KNOB 5 = PATTERN | SOFT 5 = UNIQUE",
        p.muted,
        p.bg,
    );

    let active_pat_bars = focus_group(snap)
        .and_then(|g| {
            let pi = scene_opt?
                .patterns
                .get(snap.group_focus as usize)?
                .as_ref()?;
            g.patterns.get(*pi)
        })
        .map(|p| p.length.bars as f32)
        .unwrap_or(2.0);
    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let slots = [
        Some((
            "5:PATTERN",
            focus_group(snap)
                .and_then(|_g| {
                    let pi = scene_opt?
                        .patterns
                        .get(snap.group_focus as usize)?
                        .as_ref()?;
                    Some(format!("P{}", pi + 1))
                })
                .unwrap_or_else(|| "--".to_string()),
            0.5,
            p.cyan,
        )),
        Some((
            "6:LONGUEUR",
            format!("{:.0} MES", active_pat_bars),
            (active_pat_bars / 8.0).clamp(0.0, 1.0),
            p.cyan,
        )),
        Some(("7:VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

// ── MIXER ──────────────────────────────────────────────────────────────

/// Knobs du mixer des pads : volume (pan avec Shift) des 4 pads affichés.
fn pad_volume_slots(snap: &EngineSnapshot, start_pad: usize) -> [EncSlot<'static>; 4] {
    let grp = snap.project.groups.get(snap.group_focus as usize);
    std::array::from_fn(|i| {
        let pad = start_pad + i;
        let snd = grp.and_then(|g| g.sounds.get(pad)).and_then(Option::as_ref);
        let (vol, pan) = snd.map_or((0.0, 0.0), |s| (s.volume_db, s.pan));
        let (name, value, norm) = if snap.shift_held {
            (PAN_NAMES[pad % 16], fmt_pan(pan), (pan + 1.0) / 2.0)
        } else {
            (VOL_NAMES[pad % 16], fmt_db(vol), db_norm(vol))
        };
        Some((name, value, norm, hue_color(pad)))
    })
}

const VOL_NAMES: [&str; 16] = [
    "VOL 1", "VOL 2", "VOL 3", "VOL 4", "VOL 5", "VOL 6", "VOL 7", "VOL 8", "VOL 9", "VOL 10",
    "VOL 11", "VOL 12", "VOL 13", "VOL 14", "VOL 15", "VOL 16",
];
const PAN_NAMES: [&str; 16] = [
    "PAN 1", "PAN 2", "PAN 3", "PAN 4", "PAN 5", "PAN 6", "PAN 7", "PAN 8", "PAN 9", "PAN 10",
    "PAN 11", "PAN 12", "PAN 13", "PAN 14", "PAN 15", "PAN 16",
];

fn render_pad_mixer_strips(
    frame: &mut DisplayFrame,
    p: &Pal,
    snap: &EngineSnapshot,
    start_pad: usize,
) {
    let gf = snap.group_focus as usize;
    let grp = snap.project.groups.get(gf);
    for i in 0..4usize {
        let pad_idx = start_pad + i;
        let x = 8 + i * 118;
        let y = 30usize;
        let w = 110usize;
        let h = 156usize;
        let is_focused = pad_idx == snap.active_sound as usize;
        panel(frame, p, x, y, w, h);
        if is_focused {
            draw_rect_outline(frame, x - 1, y - 1, w + 2, h + 2, p.accent);
        }
        let snd = grp
            .and_then(|g| g.sounds.get(pad_idx))
            .and_then(|s| s.as_ref());
        let s_name = snd
            .map(|s| s.name.as_str())
            .unwrap_or_else(|| DRUM_PRESET_NAMES.get(pad_idx).copied().unwrap_or("Sound"));
        let vol_db = snd.map(|s| s.volume_db).unwrap_or(0.0);
        let pan = snd.map(|s| s.pan).unwrap_or(0.0);
        let is_muted = snd.map(|s| s.muted).unwrap_or(false)
            || (snap
                .muted_sounds
                .get(gf)
                .map(|m| (m & (1 << pad_idx)) != 0)
                .unwrap_or(false));

        // Pad header chip
        let chip_color = if is_focused { p.accent } else { p.track };
        fill_rect(frame, x + 5, y + 4, 20, 16, chip_color);
        draw_text(
            frame,
            x + 7,
            y + 4,
            &format!("{:02}", pad_idx + 1),
            if is_focused { p.bg } else { p.white },
            chip_color,
        );
        draw_text(
            frame,
            x + 28,
            y + 4,
            &trunc(s_name, 9),
            if is_muted { p.muted } else { p.white },
            p.panel,
        );

        // Vertical VU meter
        let pad_lvl = snap.pad_levels.get(pad_idx).copied().unwrap_or(0.0);
        let pad_peak = snap.pad_peaks.get(pad_idx).copied().unwrap_or(0.0);
        vu_v(frame, p, x + 10, y + 24, 18, 92, pad_lvl, pad_peak);

        // Fader track and knob
        let fader_x = x + 40;
        let fader_y = y + 26;
        let fader_h = 88;
        fill_rect(frame, fader_x, fader_y, 4, fader_h, p.border);
        let norm = db_norm(vol_db).clamp(0.0, 1.0);
        let fader_knob_y = fader_y + fader_h - (norm * (fader_h - 6) as f32) as usize - 6;
        fill_rect(
            frame,
            fader_x - 6,
            fader_knob_y,
            16,
            6,
            if is_focused { p.accent } else { p.white },
        );

        // Info readouts (dB, Pan, Mute)
        draw_text(frame, x + 58, y + 36, &fmt_db(vol_db), p.white, p.panel);
        draw_text(frame, x + 58, y + 56, &fmt_pan(pan), p.cyan, p.panel);

        if is_muted {
            pill(frame, p, x + 56, y + 84, 46, "MUTE", p.red, true);
        } else if snd.map(|s| s.solo).unwrap_or(false) {
            pill(frame, p, x + 56, y + 84, 46, "SOLO", p.yellow, true);
        }
    }
}

/// Écran gauche — vue MIXER : groupes A-D (VU, dB, pan, M/S).
#[must_use]
pub fn render_mixer_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    if snap.mixer_mode == 0 || snap.mixer_mode == 1 {
        let bank_pad = if snap.mixer_mode == 0 { 0 } else { 8 };
        header(
            &mut frame,
            &p,
            snap,
            if snap.mixer_mode == 0 {
                "PAD MIXER | PADS 1-4"
            } else {
                "PAD MIXER | PADS 9-12"
            },
            &format!("Groupe {}  ·  Shift = pan", group_letter(snap.group_focus)),
        );
        render_pad_mixer_strips(&mut frame, &p, snap, bank_pad);
        encoder_row(&mut frame, &p, &pad_volume_slots(snap, bank_pad));
        finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
        return frame;
    }

    header(
        &mut frame,
        &p,
        snap,
        "MIXER",
        &format!("GRP A-D | MASTER {:.0}%", snap.master_volume * 100.0),
    );

    // 4 tranches de console (une par groupe A-D) : chip, VU vertical avec
    // échelle dB, valeur, pan, M/S.
    for gi in 0..4usize {
        let x = 8 + gi * 118;
        let y = 30usize;
        let w = 110usize;
        let h = 156usize;
        let focused = gi == snap.group_focus as usize;
        panel(&mut frame, &p, x, y, w, h);
        if focused {
            draw_rect_outline(&mut frame, x - 1, y - 1, w + 2, h + 2, p.accent);
            fill_rect(&mut frame, x - 1, y - 1, 3, 3, p.accent);
            fill_rect(&mut frame, x + w - 2, y - 1, 3, 3, p.accent);
        }
        let gcolor = hue_color(gi);
        let has = snap.project.groups.get(gi).is_some();
        let (lbg, lfg) = if focused {
            (gcolor, p.bg)
        } else if has {
            (p.track, gcolor)
        } else {
            (p.track, p.muted)
        };
        fill_rect(&mut frame, x + 5, y + 4, 14, 16, lbg);
        draw_char(&mut frame, x + 8, y + 4, group_letter(gi as u8), lfg, lbg);
        draw_text(
            &mut frame,
            x + 23,
            y + 4,
            &trunc(&group_name(snap, gi), 10),
            if has { p.white } else { p.muted },
            p.panel,
        );

        // VU vertical + échelle dB + LED de clip.
        let vu_y = y + 24;
        let vu_hh = 72usize;
        vu_v(
            &mut frame,
            &p,
            x + 10,
            vu_y,
            26,
            vu_hh,
            snap.group_levels[gi],
            snap.group_peaks[gi],
        );
        let clipped = snap.group_peaks[gi] >= 0.995;
        fill_rect(
            &mut frame,
            x + 41,
            vu_y - 1,
            5,
            5,
            if clipped { p.red } else { p.track },
        );
        for (label, dy) in [("0", 0usize), ("-6", 18), ("-12", 36), ("-24", 54)] {
            draw_text(&mut frame, x + 50, vu_y + dy, label, p.muted, p.panel);
            fill_rect(&mut frame, x + 38, vu_y + dy + 8, 4, 1, p.track);
        }

        let vol = snap
            .project
            .groups
            .get(gi)
            .map(|g| g.volume_db)
            .unwrap_or(0.0);
        let pan = snap.project.groups.get(gi).map(|g| g.pan).unwrap_or(0.0);
        draw_text(
            &mut frame,
            x + 10,
            y + 100,
            &fmt_db(vol),
            if vol > -0.5 { p.white } else { p.dim },
            p.panel,
        );
        draw_text(
            &mut frame,
            x + 10,
            y + 118,
            &format!("PAN {}", fmt_pan(pan)),
            p.muted,
            p.panel,
        );
        let muted = snap.muted_groups & (1 << gi) != 0;
        let soloed = snap.soloed_groups & (1 << gi) != 0;
        pill(&mut frame, &p, x + 10, y + 138, 36, "MUTE", p.red, muted);
        pill(
            &mut frame,
            &p,
            x + 52,
            y + 138,
            36,
            "SOLO",
            p.yellow,
            soloed,
        );
    }

    let gi = snap.group_focus as usize;
    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let s_pan = focus_sound(snap).map(|s| s.pan).unwrap_or(0.0);
    let gvol = snap
        .project
        .groups
        .get(gi)
        .map(|g| g.volume_db)
        .unwrap_or(0.0);
    let gpan = snap.project.groups.get(gi).map(|g| g.pan).unwrap_or(0.0);
    let slots = [
        Some(("1:VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some(("2:PAN SND", fmt_pan(s_pan), (s_pan + 1.0) * 0.5, p.cyan)),
        Some(("3:VOL GRP", fmt_db(gvol), db_norm(gvol), p.green)),
        Some(("4:PAN GRP", fmt_pan(gpan), (gpan + 1.0) * 0.5, p.green)),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
    frame
}

/// Écran droit — vue MIXER : master + groupe focus + sound focus.
#[must_use]
pub fn render_mixer_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    if snap.mixer_mode == 0 || snap.mixer_mode == 1 {
        let bank_pad = if snap.mixer_mode == 0 { 4 } else { 12 };
        header(
            &mut frame,
            &p,
            snap,
            if snap.mixer_mode == 0 {
                "PAD MIXER | PADS 5-8"
            } else {
                "PAD MIXER | PADS 13-16"
            },
            &format!("Groupe {}  ·  Shift = pan", group_letter(snap.group_focus)),
        );
        render_pad_mixer_strips(&mut frame, &p, snap, bank_pad);
        encoder_row(&mut frame, &p, &pad_volume_slots(snap, bank_pad));
        finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
        return frame;
    }

    header(
        &mut frame,
        &p,
        snap,
        "MIXER | MASTER",
        &format!("GRP {} focus", group_letter(snap.group_focus)),
    );

    // ── Section MASTER (moitié gauche) ─────────────────────────────────
    panel(&mut frame, &p, 8, 30, 226, 156);
    draw_text(&mut frame, 16, 36, "MASTER", p.accent, p.panel);
    fill_rect(&mut frame, 16, 54, 60, 1, p.track);

    let mdb = lin_db(snap.master_volume);
    // Grand VU vertical + échelle dB + LED de clip.
    vu_v(
        &mut frame,
        &p,
        20,
        62,
        34,
        102,
        snap.master_level,
        snap.master_peak,
    );
    for (label, dy) in [("0", 0usize), ("-6", 25), ("-12", 50), ("-24", 75)] {
        draw_text(&mut frame, 58, 62 + dy, label, p.muted, p.panel);
        fill_rect(&mut frame, 54, 70 + dy, 3, 1, p.track);
    }
    let clipped = snap.master_peak >= 0.995;
    fill_rect(
        &mut frame,
        20,
        54,
        8,
        6,
        if clipped { p.red } else { p.track },
    );
    draw_text_scaled(
        &mut frame,
        100,
        58,
        &fmt_db(mdb),
        2,
        if mdb > -0.5 { p.red } else { p.white },
        p.panel,
    );
    draw_text(
        &mut frame,
        100,
        94,
        &format!("VOL {:>3.0}%", snap.master_volume * 100.0),
        p.dim,
        p.panel,
    );
    draw_text(
        &mut frame,
        100,
        112,
        &format!("CASQUE {:>3.0}%", snap.headphone_volume * 100.0),
        p.muted,
        p.panel,
    );
    pill(
        &mut frame,
        &p,
        100,
        130,
        58,
        "LOOP",
        p.cyan,
        snap.transport.loop_enabled,
    );
    pill(
        &mut frame,
        &p,
        164,
        130,
        66,
        "METRO",
        p.cyan,
        snap.transport.metronome,
    );
    bar_h(
        &mut frame,
        &p,
        100,
        152,
        132,
        8,
        snap.master_volume,
        p.accent,
    );
    draw_text(
        &mut frame,
        100,
        164,
        &format!("BPM {:.0}  |  {} voix", snap.transport.bpm, snap.num_voices),
        p.dim,
        p.panel,
    );

    // ── Section FOYER : groupe + Sound (moitié droite) ─────────────────
    let gi = snap.group_focus as usize;
    let gcolor = hue_color(gi);
    let gvol = focus_group(snap).map(|g| g.volume_db).unwrap_or(0.0);
    let gpan = focus_group(snap).map(|g| g.pan).unwrap_or(0.0);
    let muted = snap.muted_groups & (1 << gi) != 0;
    let soloed = snap.soloed_groups & (1 << gi) != 0;
    panel(&mut frame, &p, 242, 30, 230, 66);
    fill_rect(&mut frame, 242, 30, 3, 66, gcolor);
    fill_rect(&mut frame, 252, 36, 14, 16, gcolor);
    draw_char(
        &mut frame,
        255,
        36,
        group_letter(snap.group_focus),
        p.bg,
        gcolor,
    );
    draw_text(
        &mut frame,
        272,
        36,
        &format!(
            "GROUPE {} {}",
            group_letter(snap.group_focus),
            trunc(&group_name(snap, gi), 11)
        ),
        p.white,
        p.panel,
    );
    vu_h(
        &mut frame,
        &p,
        252,
        58,
        212,
        10,
        snap.group_levels[gi.min(7)],
        snap.group_peaks[gi.min(7)],
    );
    draw_text(
        &mut frame,
        252,
        74,
        &format!("{}  PAN {}", fmt_db(gvol), fmt_pan(gpan)),
        p.dim,
        p.panel,
    );
    pill(&mut frame, &p, 372, 72, 44, "MUTE", p.red, muted);
    pill(&mut frame, &p, 420, 72, 44, "SOLO", p.yellow, soloed);

    // Sound focus : réglages + enveloppe ADSR.
    let s_name = sound_name(snap, snap.active_sound as usize);
    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let s_pan = focus_sound(snap).map(|s| s.pan).unwrap_or(0.0);
    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    let adsr = focus_sound(snap)
        .map(|s| s.adsr.clone())
        .unwrap_or_default();
    panel(&mut frame, &p, 242, 102, 230, 88);
    fill_rect(&mut frame, 242, 102, 3, 88, p.accent);
    draw_text(
        &mut frame,
        252,
        108,
        &format!("SON {:02} {}", snap.active_sound + 1, trunc(&s_name, 12)),
        p.white,
        p.panel,
    );
    draw_text(
        &mut frame,
        252,
        126,
        &format!(
            "{}  PAN {}  CUT {}",
            fmt_db(s_db),
            fmt_pan(s_pan),
            fmt_hz(cut)
        ),
        p.dim,
        p.panel,
    );
    draw_text(
        &mut frame,
        424,
        126,
        &format!("R{:.0}%", reso * 100.0),
        p.muted,
        p.panel,
    );
    draw_adsr(&mut frame, &p, 252, 142, 218, 44, &adsr);

    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    let slots = [
        Some((
            "5:CUTOFF",
            format!("{:.0}Hz", cut),
            (cut / 20000.0).clamp(0.0, 1.0),
            p.accent,
        )),
        Some((
            "6:RESO",
            format!("{:.0}%", reso * 100.0),
            reso.clamp(0.0, 1.0),
            p.accent,
        )),
        Some((
            "7:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
        Some((
            "8:CASQUE",
            format!("{:.0}%", snap.headphone_volume * 100.0),
            snap.headphone_volume,
            p.cyan,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

// ── SAMPLER ────────────────────────────────────────────────────────────

/// Écran gauche — SAMPLING (éditeur de sample, boutons 1–4 = RECORD, EDIT,
/// SLICE, ZONE) : forme d'onde avec la plage de lecture et les tranches, ou
/// l'état de l'enregistrement ; knobs 1–4 de la page.
#[must_use]
pub fn render_sampler_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let snd = snap.active_sound as usize;
    let page = SAMPLE_PAGE_NAMES[(snap.sample_page as usize).min(3)];
    header_colored(
        &mut frame,
        &p,
        snap,
        page,
        &format!(
            "{} {:02} {}  ·  {:.2} s",
            group_letter(snap.group_focus),
            snd + 1,
            sound_name(snap, snd),
            snap.sample_duration
        ),
        hue_color(snd),
    );
    let (x, y, w, h) = (8usize, 34usize, SCR_W - 16, 142usize);
    if snap.sample_page == 0 {
        draw_record_state(&mut frame, &p, snap, (x, y, w, h));
    } else {
        let buckets = snap.waveform.as_deref().unwrap_or(&[]);
        draw_waveform(&mut frame, &p, x, y, w, h, buckets);
        if !buckets.is_empty() {
            draw_play_range(&mut frame, &p, snap, (x, y, w, h));
        }
    }
    let sections = ["ENTRÉE", "PLAGE & ACCORD", "DÉCOUPE", "TESSITURE"];
    section_title(
        &mut frame,
        &p,
        sections[(snap.sample_page as usize).min(3)],
        p.accent,
    );
    encoder_row(
        &mut frame,
        &p,
        &view_slots(&snap.sample_params, 0, p.accent),
    );
    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
    frame
}

/// Noms des pages de l'éditeur de sample (boutons 1–4).
const SAMPLE_PAGE_NAMES: [&str; 4] = ["RECORD", "EDIT", "SLICE", "ZONE"];

/// Les 4 slots d'une moitié des paramètres (`first` = 0 ou 4).
fn view_slots(
    params: &[Option<crate::plugin_pages::ParamView>; 8],
    first: usize,
    color: DisplayPixel,
) -> [EncSlot<'_>; 4] {
    std::array::from_fn(|i| {
        let pv = params[first + i].as_ref()?;
        Some((pv.name, pv.value.clone(), pv.norm, color))
    })
}

/// Plage de lecture (hors plage voilée, marqueurs début / fin) et tranches.
fn draw_play_range(
    frame: &mut DisplayFrame,
    p: &Pal,
    snap: &EngineSnapshot,
    (x, y, w, h): (usize, usize, usize, usize),
) {
    use screen_vector::Paint;
    let inner = w as f32 - 8.0;
    let px = |f: f32| x as f32 + 4.0 + f.clamp(0.0, 1.0) * inner;
    let (start, end) = snap.sample_range;
    for (a, b) in [(x as f32, px(start)), (px(end), (x + w) as f32)] {
        if b - a > 0.5 {
            if let Some(path) = screen_vector::round_rect(a, y as f32, b - a, h as f32, 0.0) {
                screen_vector::fill(frame, &path, Paint::Solid(p.bg, 0.62));
            }
        }
    }
    if snap.sample_page == 2 {
        for (k, &f) in snap.slice_points.iter().enumerate() {
            let sx = px(f);
            fill_rect(frame, sx as usize, y + 2, 1, h - 4, p.white);
            let n = format!("{}", k + 1);
            label(frame, sx + 3.0, y as f32 + 13.0, &n, 11.0, p.white, 1.8);
        }
    }
    for (f, name) in [(start, "S"), (end, "E")] {
        let mx = px(f);
        screen_vector::fill_round_rect(frame, mx - 1.0, y as f32, 2.0, h as f32, 1.0, p.accent);
        screen_vector::fill_round_rect(
            frame,
            mx - 7.0,
            (y + h) as f32 - 16.0,
            14.0,
            14.0,
            3.0,
            p.accent,
        );
        let tw = screen_text::width(name, screen_text::Face::Bold, 11.0);
        bold(
            frame,
            mx - tw / 2.0,
            (y + h) as f32 - 5.0,
            name,
            11.0,
            p.ink,
        );
    }
}

/// Page RECORD : état, niveau d'entrée avec le seuil, durée.
fn draw_record_state(
    frame: &mut DisplayFrame,
    p: &Pal,
    snap: &EngineSnapshot,
    (x, y, w, h): (usize, usize, usize, usize),
) {
    panel(frame, p, x, y, w, h);
    let (title, color) = match &snap.record {
        Some(r) if r.waiting => ("Armé : joue pour démarrer".to_string(), p.yellow),
        Some(r) => (
            format!("● Enregistrement  {:.1} s / {:.0} s", r.secs, r.max_secs),
            p.red,
        ),
        None => ("Prêt à enregistrer".to_string(), p.white),
    };
    bold(frame, (x + 14) as f32, (y + 34) as f32, &title, 22.0, color);
    let hint = "ENREG lance la prise sur le pad actif, STOP la garde · entrée de la MK3";
    let hint = fit_text(hint, screen_text::Face::Sans, 13.0, (w - 28) as f32);
    label(
        frame,
        (x + 14) as f32,
        (y + 56) as f32,
        &hint,
        13.0,
        p.dim,
        1.0,
    );
    // Niveau d'entrée (dB) avec le seuil.
    let level = snap.record.map_or(0.0, |r| r.level);
    let db = 20.0 * level.max(1e-6).log10();
    let norm = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
    let (bx, by, bw) = (x + 14, y + 84, w - 28);
    bar_h(
        frame,
        p,
        bx,
        by,
        bw,
        10,
        norm,
        if db > -6.0 { p.red } else { p.green },
    );
    let threshold = snap.sample_params[0].as_ref().map_or(-30.0, |pv| pv.raw);
    let tx = bx as f32 + (threshold + 60.0) / 60.0 * bw as f32;
    screen_vector::fill_round_rect(frame, tx - 1.0, (by - 4) as f32, 2.0, 18.0, 1.0, p.white);
    label(
        frame,
        bx as f32,
        (by + 30) as f32,
        "ENTRÉE",
        12.0,
        p.muted,
        1.0,
    );
    let seuil = format!("seuil {threshold:.0} dB");
    let sw = screen_text::width(&seuil, screen_text::Face::Sans, 12.0);
    label(
        frame,
        (bx + bw) as f32 - sw,
        (by + 30) as f32,
        &seuil,
        12.0,
        p.muted,
        1.0,
    );
}

/// Écran droit — SAMPLING : onglets des 4 pages et le visuel de la page
/// (enveloppe en EDIT, tranches en SLICE, clavier en ZONE) ; knobs 5–8.
#[must_use]
pub fn render_sampler_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let page = (snap.sample_page as usize).min(3);
    header_colored(
        &mut frame,
        &p,
        snap,
        "SAMPLE EDITOR",
        &format!("PAGE {}/4", page + 1),
        hue_color(snap.active_sound as usize),
    );
    let tw = (SCR_W - 16) / 4;
    for (i, name) in SAMPLE_PAGE_NAMES.iter().enumerate() {
        let x = 8 + i * tw;
        let on = i == page;
        let bg = if on { p.accent } else { p.panel };
        screen_vector::fill_round_rect(
            &mut frame,
            x as f32 + 1.0,
            32.0,
            tw as f32 - 2.0,
            20.0,
            4.0,
            bg,
        );
        let face = if on {
            screen_text::Face::Bold
        } else {
            screen_text::Face::Sans
        };
        let w = screen_text::width(name, face, 14.0);
        screen_text::draw(
            &mut frame,
            (x + tw / 2) as f32 - w / 2.0,
            46.5,
            name,
            face,
            14.0,
            if on { p.ink } else { p.dim },
            1.0,
        );
    }
    let rect = (8usize, 60usize, SCR_W - 16, 114usize);
    match page {
        0 => {
            panel(&mut frame, &p, rect.0, rect.1, rect.2, rect.3);
            let lines = [
                "SEUIL (bouton 2) : la prise démarre au premier son",
                "Knob 1 : niveau de seuil · knob 2 : durée max",
                "La prise devient le sample du pad, enregistré en WAV",
            ];
            for (k, line) in lines.iter().enumerate() {
                label(
                    &mut frame,
                    22.0,
                    88.0 + k as f32 * 26.0,
                    line,
                    14.0,
                    p.dim,
                    1.0,
                );
            }
        }
        2 => draw_slices(&mut frame, &p, snap, rect),
        3 => draw_zone_map(&mut frame, &p, snap, rect),
        _ => {
            let adsr = focus_sound(snap)
                .map(|s| s.adsr.clone())
                .unwrap_or_default();
            draw_adsr(&mut frame, &p, rect.0, rect.1, rect.2, rect.3, &adsr);
        }
    }
    let section = ["", "ENVELOPPE", "", "VÉLOCITÉ & SORTIE"][page];
    section_title(&mut frame, &p, section, p.accent);
    encoder_row(
        &mut frame,
        &p,
        &view_slots(&snap.sample_params, 4, p.accent),
    );
    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

/// Tranches : une case par pad cible, avec sa durée.
fn draw_slices(
    frame: &mut DisplayFrame,
    p: &Pal,
    snap: &EngineSnapshot,
    (x, y, w, h): (usize, usize, usize, usize),
) {
    panel(frame, p, x, y, w, h);
    let n = snap.slice_points.len();
    if n == 0 {
        label(
            frame,
            (x + 14) as f32,
            (y + 60) as f32,
            "Pas de sample à découper",
            15.0,
            p.muted,
            1.0,
        );
        return;
    }
    let (_, end) = snap.sample_range;
    let cols = 8usize;
    let (cw, ch) = ((w - 16) / cols, 48usize);
    for k in 0..n.min(16) {
        let (cx, cy) = (x + 8 + (k % cols) * cw, y + 8 + (k / cols) * (ch + 6));
        let pad = snap.active_sound as usize + k;
        let color = if pad < 16 { hue_color(pad) } else { p.track };
        screen_vector::fill_round_rect(
            frame,
            cx as f32,
            cy as f32,
            cw as f32 - 6.0,
            ch as f32,
            4.0,
            p.panel2,
        );
        screen_vector::fill_round_rect(frame, cx as f32, cy as f32, 4.0, ch as f32, 2.0, color);
        let pad_txt = if pad < 16 {
            format!("PAD {}", pad + 1)
        } else {
            "—".to_string()
        };
        bold(
            frame,
            (cx + 9) as f32,
            (cy + 18) as f32,
            &pad_txt,
            13.0,
            p.white,
        );
        let a = snap.slice_points[k];
        let b = snap.slice_points.get(k + 1).copied().unwrap_or(end);
        let ms = (b - a).max(0.0) * snap.sample_duration * 1000.0;
        label(
            frame,
            (cx + 9) as f32,
            (cy + 38) as f32,
            &fmt_ms(ms),
            12.0,
            p.dim,
            1.0,
        );
    }
}

/// Zone : tessiture sur un clavier de 128 notes, note racine, vélocités.
fn draw_zone_map(
    frame: &mut DisplayFrame,
    p: &Pal,
    snap: &EngineSnapshot,
    (x, y, w, h): (usize, usize, usize, usize),
) {
    panel(frame, p, x, y, w, h);
    let raw = |i: usize| snap.sample_params[i].as_ref().map_or(0.0, |pv| pv.raw);
    let (root, lo, hi) = (raw(0) as usize, raw(1) as usize, raw(2) as usize);
    let (kx, ky, kw, kh) = (x + 10, y + 14, w - 20, 54usize);
    let key_w = kw as f32 / 128.0;
    for n in 0..128usize {
        let black = matches!(n % 12, 1 | 3 | 6 | 8 | 10);
        let inside = (lo..=hi).contains(&n);
        let base = if black { p.panel } else { p.track };
        let color = if inside {
            mix(base, p.accent, if black { 0.55 } else { 0.8 })
        } else {
            base
        };
        let x0 = kx as f32 + n as f32 * key_w;
        let khh = if black { kh * 3 / 5 } else { kh };
        fill_rect(frame, x0 as usize, ky, key_w.ceil() as usize, khh, color);
    }
    let rx = kx as f32 + (root as f32 + 0.5) * key_w;
    screen_vector::fill_round_rect(
        frame,
        rx - 1.5,
        (ky - 6) as f32,
        3.0,
        (kh + 12) as f32,
        1.5,
        p.white,
    );
    let root_name = snap.sample_params[0]
        .as_ref()
        .map_or(String::new(), |pv| pv.value.clone());
    bold(
        frame,
        rx + 5.0,
        (ky + kh + 18) as f32,
        &format!("racine {root_name}"),
        13.0,
        p.white,
    );
    // Vélocités acceptées.
    let (vlo, vhi) = (raw(4), raw(5));
    let (vx, vy, vw) = (kx, y + h - 22, kw);
    fill_rect(frame, vx, vy, vw, 6, p.track);
    let a = vx as f32 + vlo / 127.0 * vw as f32;
    let b = vx as f32 + vhi / 127.0 * vw as f32;
    screen_vector::fill_round_rect(frame, a, vy as f32, (b - a).max(2.0), 6.0, 3.0, p.accent);
    label(
        frame,
        vx as f32,
        (vy - 4) as f32,
        "VÉLOCITÉ",
        11.0,
        p.muted,
        1.0,
    );
}

// ── KICK SYNTH (TECHNO MODELER) ─────────────────────────────────────────

// ── WAVETABLE / NEXUS SYNTH ──────────────────────────────────────────────

// ── SYNTH (TB-303 ACID) ──────────────────────────────────────────────────

// ── BROWSER ────────────────────────────────────────────────────────────

/// Écran gauche — vue BROWSER : presets du kit 808.
#[must_use]
pub fn render_browser_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);

    if snap.browser_kind == 2 {
        header(
            &mut frame,
            &p,
            snap,
            "BROWSER | PLUGINS",
            "SELECTIONNER UN INSTRUMENT (BTN 4 CHARGER)",
        );
        let plugins = [
            (
                "1. KICK SYNTH",
                "Modélisation de Kick Techno / Analogique",
                "TECHNO",
            ),
            (
                "2. TB-303 ACID",
                "Synthétiseur de basse Acid 303 classique",
                "ACID",
            ),
            (
                "3. NEXUS SYNTH",
                "Synthétiseur polyphonique à tables d'ondes",
                "SYNTH",
            ),
            (
                "4. GUITAR AMP",
                "Ampli & rack d'effets guitare / saturation",
                "AMP",
            ),
            (
                "5. SAMPLER 808",
                "Lecteur d'échantillons de batterie Maschine",
                "SAMPLE",
            ),
        ];
        let p_idx = (snap.browser_plugin_idx as usize).min(4);
        for (i, (name, desc, tag)) in plugins.iter().enumerate() {
            let y = 30 + i * 32;
            let selected = i == p_idx;
            panel(&mut frame, &p, 10, y, 460, 28);
            if selected {
                draw_rect_outline(&mut frame, 9, y - 1, 462, 30, p.accent);
                fill_rect(&mut frame, 12, y + 2, 4, 24, p.accent);
            }
            draw_text(
                &mut frame,
                22,
                y + 2,
                name,
                if selected { p.white } else { p.dim },
                p.panel,
            );
            draw_text(&mut frame, 22, y + 14, desc, p.muted, p.panel);
            pill(
                &mut frame,
                &p,
                390,
                y + 4,
                65,
                tag,
                if selected { p.accent } else { p.track },
                selected,
            );
        }
        finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
        return frame;
    }

    if snap.browser_kind == 3 {
        header(
            &mut frame,
            &p,
            snap,
            "BROWSER | EFFETS",
            "AJOUTER UN EFFET SUR LE PAD/GROUPE (BTN 4)",
        );
        let effects = [
            (
                "1. REVERB STEREO",
                "Réverbération algorithmique espace & diffusion",
                "SPACE",
            ),
            (
                "2. DELAY ECHO",
                "Ligne à retard ping-pong synchronisée",
                "ECHO",
            ),
            (
                "3. OVERDRIVE",
                "Distorsion analogique, saturation & fuzz",
                "DRIVE",
            ),
            (
                "4. FILTER MULTIMODE",
                "Filtre résonant Lowpass / Highpass / Bandpass",
                "FILTER",
            ),
            (
                "5. COMPRESSOR",
                "Compresseur de dynamique punchy & limiteur",
                "DYNAMICS",
            ),
        ];
        let f_idx = (snap.browser_fx_idx as usize).min(4);
        for (i, (name, desc, tag)) in effects.iter().enumerate() {
            let y = 32 + i * 31;
            let selected = i == f_idx;
            panel(&mut frame, &p, 10, y, 460, 28);
            if selected {
                draw_rect_outline(&mut frame, 9, y - 1, 462, 30, p.cyan);
                fill_rect(&mut frame, 12, y + 2, 4, 24, p.cyan);
            }
            draw_text(
                &mut frame,
                22,
                y + 3,
                name,
                if selected { p.white } else { p.dim },
                p.panel,
            );
            draw_text(&mut frame, 22, y + 15, desc, p.muted, p.panel);
            pill(
                &mut frame,
                &p,
                385,
                y + 5,
                70,
                tag,
                if selected { p.cyan } else { p.track },
                selected,
            );
        }
        finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
        return frame;
    }

    let idx = snap.browser_preset_idx as usize % DRUM_PRESET_NAMES.len();
    header(
        &mut frame,
        &p,
        snap,
        "BROWSER | KIT 808",
        &format!(
            "PRESET {}/16 | MASTER {:.0}%",
            idx + 1,
            snap.master_volume * 100.0
        ),
    );

    let y = 28usize;
    for (i, name) in DRUM_PRESET_NAMES.iter().enumerate().take(16) {
        let col = i / 8;
        let row = i % 8;
        let x = 8 + col * 236;
        let cy = y + row * 20;
        let selected = i == idx;
        let bg = if selected { p.accent } else { p.panel };
        fill_rect(&mut frame, x, cy, 226, 18, bg);
        draw_rect_outline(
            &mut frame,
            x,
            cy,
            226,
            18,
            if selected { p.accent } else { p.border },
        );
        draw_text(
            &mut frame,
            x + 8,
            cy + 1,
            &format!("{:02} {}", i + 1, trunc(name, 13)),
            if selected { p.bg } else { p.dim },
            bg,
        );
    }
    // Preset courant : surligné dans la liste et sur le knob 2 (la ligne
    // d'aide débordait sur les encodeurs ; les boutons sont libellés en haut).
    let cur = DRUM_PRESET_NAMES[idx];

    let cible = sound_name(snap, snap.active_sound as usize);
    let slots = [
        Some((
            "1:SND CIBLE",
            format!("{:02} {}", snap.active_sound + 1, trunc(&cible, 8)),
            snap.active_sound as f32 / 15.0,
            p.cyan,
        )),
        Some(("2:PRESET", trunc(cur, 13), idx as f32 / 15.0, p.accent)),
        Some(("3:--", String::new(), 0.0, p.border)),
        Some(("4:--", String::new(), 0.0, p.border)),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
    frame
}

/// Écran droit — vue BROWSER : preset sélectionné + cible de chargement.
#[must_use]
pub fn render_browser_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);

    if snap.browser_kind == 2 {
        let p_idx = (snap.browser_plugin_idx as usize).min(4);
        let plugin_info = match p_idx {
            0 => (
                "KICK SYNTH TECHNO",
                "Synthese analogique dediee aux kicks club/techno.",
                "Pitch env exponentielle, click de transient,",
                "Drive/saturation analogique et boost sub 50Hz.",
                "8 presets inclus : Techno Berlin, 909 Punch...",
            ),
            1 => (
                "TB-303 ACID BASS",
                "Emulation authentique Roland TB-303 Bassline.",
                "Filtre 24dB resonant a diode avec saturation,",
                "Accentuation, slide analogique et overdrive tanh.",
                "Presets rapides : Acid Squelch, Distort, Dub 303.",
            ),
            2 => (
                "NEXUS / SERUM SYNTH",
                "Synthe polyphonique 16 voix a tables d'ondes.",
                "Multi-oscillateurs, filtre multimode resonant,",
                "Rack d'effets complet (Chorus, Delay, Reverb).",
                "16 presets integres : Piano, Bass, Lead, Pad...",
            ),
            3 => (
                "GUITAR AMP STANDALONE",
                "Simulateur de preampli guitare a lampes & baffle.",
                "Drive agressif, egalisation 3-bandes active,",
                "Reverb a ressort et noise gate rapide integre.",
                "Ideal pour leads corrosifs et basses distordues.",
            ),
            _ => (
                "SAMPLER 808 DRUM",
                "Lecteur de samples percussifs pour Maschine.",
                "Pitch tuning, enveloppe AHD, volume, pan,",
                "Routing direct vers la grille de pads 4x4.",
                "Chargement direct d'echantillons WAV.",
            ),
        };
        header(
            &mut frame,
            &p,
            snap,
            "BROWSER | PLUGIN CIBLE",
            &format!("SLOT PAD {:02}", snap.active_sound + 1),
        );
        draw_text_scaled(
            &mut frame,
            12,
            38,
            &trunc(plugin_info.0, 24),
            2,
            p.white,
            p.bg,
        );
        panel(&mut frame, &p, 10, 78, 460, 84);
        draw_text(&mut frame, 18, 86, plugin_info.1, p.accent, p.panel);
        draw_text(&mut frame, 18, 102, plugin_info.2, p.dim, p.panel);
        draw_text(&mut frame, 18, 118, plugin_info.3, p.dim, p.panel);
        draw_text(&mut frame, 18, 134, plugin_info.4, p.cyan, p.panel);

        let target_str = format!(
            "CIBLE: GRP {} | PAD {:02} [{}]",
            group_letter(snap.group_focus),
            snap.active_sound + 1,
            trunc(&sound_name(snap, snap.active_sound as usize), 12)
        );
        draw_text(&mut frame, 12, 172, &target_str, p.green, p.bg);
        draw_text(
            &mut frame,
            12,
            190,
            "-> Appuyez sur [CHARGER] (Bouton 4) pour assigner au pad",
            p.white,
            p.bg,
        );

        let slots = [
            Some(("5:PREV PAD", "PAD -1".to_string(), 0.0, p.panel2)),
            Some(("6:NEXT PAD", "PAD +1".to_string(), 0.0, p.panel2)),
            Some(("7:--", String::new(), 0.0, p.border)),
            Some((
                "8:MASTER",
                format!("{:.0}%", snap.master_volume * 100.0),
                snap.master_volume,
                p.accent,
            )),
        ];
        encoder_row(&mut frame, &p, &slots);

        finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
        return frame;
    }

    if snap.browser_kind == 3 {
        let f_idx = (snap.browser_fx_idx as usize).min(4);
        let fx_info = match f_idx {
            0 => (
                "REVERB STEREO",
                "Reverberation spaciale avec damping, pre-delay et decay.",
            ),
            1 => (
                "DELAY STEREO",
                "Echo stereo synchronise avec feedback et filtre passe-bas.",
            ),
            2 => (
                "OVERDRIVE",
                "Saturation analogique tanh avec tone et compensation de gain.",
            ),
            3 => (
                "FILTER MULTIMODE",
                "Filtre passe-bas/passe-haut/passe-bande 12dB resonant.",
            ),
            _ => (
                "COMPRESSOR",
                "Compresseur de dynamique avec seuil, ratio, attaque et release.",
            ),
        };
        header(
            &mut frame,
            &p,
            snap,
            "BROWSER | EFFET CIBLE",
            "DESTINATION FX",
        );
        draw_text_scaled(&mut frame, 12, 38, &trunc(fx_info.0, 24), 2, p.white, p.bg);
        panel(&mut frame, &p, 10, 78, 460, 68);
        draw_text(&mut frame, 18, 86, fx_info.1, p.accent, p.panel);

        let pad_fx_count = snap.focus_sound_fx.len();
        let grp_fx_count = snap.focus_group_fx.len();

        let s1 = format!(
            "PAD {:02} [{}]: {} effet(s)",
            snap.active_sound + 1,
            trunc(&sound_name(snap, snap.active_sound as usize), 10),
            pad_fx_count
        );
        let s2 = format!(
            "GROUPE {}: {} effet(s)",
            group_letter(snap.group_focus),
            grp_fx_count
        );
        draw_text(&mut frame, 18, 106, &s1, p.cyan, p.panel);
        draw_text(&mut frame, 18, 124, &s2, p.yellow, p.panel);

        draw_text(
            &mut frame,
            12,
            160,
            "BTN 4: AJOUTER SUR LE PAD ACTIF",
            p.green,
            p.bg,
        );
        draw_text(
            &mut frame,
            12,
            178,
            "BTN 5: AJOUTER SUR LE GROUPE ACTIF",
            p.yellow,
            p.bg,
        );

        let slots = [
            Some((
                "5:GRP FX",
                format!("GRP {}", group_letter(snap.group_focus)),
                0.0,
                p.yellow,
            )),
            Some((
                "6:PAD FX",
                format!("PAD {:02}", snap.active_sound + 1),
                0.0,
                p.cyan,
            )),
            Some(("7:--", String::new(), 0.0, p.border)),
            Some((
                "8:MASTER",
                format!("{:.0}%", snap.master_volume * 100.0),
                snap.master_volume,
                p.accent,
            )),
        ];
        encoder_row(&mut frame, &p, &slots);

        finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
        return frame;
    }

    let idx = snap.browser_preset_idx as usize % DRUM_PRESET_NAMES.len();
    let name = DRUM_PRESET_NAMES[idx];
    header(
        &mut frame,
        &p,
        snap,
        "BROWSER | CHARGER",
        &format!("PRESET {}/16", idx + 1),
    );

    // Carte du preset : nom en grand, cible en couleur du pad visé.
    panel(&mut frame, &p, 8, 34, SCR_W - 16, 142);
    let title = fit_text(name, screen_text::Face::Bold, 30.0, (SCR_W - 40) as f32);
    bold(&mut frame, 22.0, 76.0, &title, 30.0, p.white);
    let pad = snap.active_sound as usize;
    cell(&mut frame, 22, 96, 12, 12, hue_color(pad), p.panel);
    let target = format!(
        "Charger sur Groupe {} · pad {} ({})",
        group_letter(snap.group_focus),
        pad + 1,
        sound_name(snap, pad)
    );
    let target = fit_text(&target, screen_text::Face::Sans, 15.0, (SCR_W - 80) as f32);
    label(&mut frame, 42.0, 107.0, &target, 15.0, p.dim, 1.0);
    let hint = "ECOUTE pour pré-écouter, CHARGER pour remplacer le son du pad";
    let hint = fit_text(hint, screen_text::Face::Sans, 13.0, (SCR_W - 44) as f32);
    label(&mut frame, 22.0, 150.0, &hint, 13.0, p.muted, 1.0);

    let slots = [
        None,
        None,
        None,
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

// ── PROJECT BROWSER / FILE ─────────────────────────────────────────────

/// Écran gauche — navigateur de projets (FILE).
#[must_use]
pub fn render_project_browser_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let entries = &snap.project_entries;
    let total = entries.len();
    let selected = if total == 0 {
        0
    } else {
        snap.project_selected.min(total - 1)
    };
    header(
        &mut frame,
        &p,
        snap,
        "FILE | PROJETS",
        &format!(
            "{}/{} | {}",
            if total == 0 { 0 } else { selected + 1 },
            total,
            if snap.current_project_path.is_some() {
                "SAUVE"
            } else {
                "NOUVEAU"
            }
        ),
    );

    if total == 0 {
        draw_text(&mut frame, 10, 44, "Aucun projet trouve.", p.white, p.bg);
        draw_text(
            &mut frame,
            10,
            68,
            "Appuyez sur [NOUVEAU] (Bouton 3) pour creer un projet !",
            p.cyan,
            p.bg,
        );
        draw_text(
            &mut frame,
            10,
            96,
            "Dossier: ~/.local/share/maschine-linux/projects",
            p.muted,
            p.bg,
        );
    } else {
        // Fenêtre de 8 lignes centrée sur la sélection.
        let win = 8usize;
        let start = selected
            .saturating_sub(win / 2)
            .min(total.saturating_sub(win));
        let mut y = 30usize;
        for (i, entry) in entries.iter().enumerate().skip(start).take(win) {
            let sel = i == selected;
            let bg = if sel { p.panel2 } else { p.panel };
            fill_rect(&mut frame, 6, y, 468, 19, bg);
            draw_rect_outline(
                &mut frame,
                6,
                y,
                468,
                19,
                if sel { p.accent } else { p.border },
            );
            if sel {
                fill_rect(&mut frame, 6, y, 3, 19, p.accent);
            }
            draw_text(
                &mut frame,
                14,
                y + 5,
                &format!("{:02}", i + 1),
                if sel { p.accent } else { p.muted },
                bg,
            );
            draw_text(
                &mut frame,
                42,
                y + 5,
                &trunc(&entry.name, 28),
                if sel { p.white } else { p.dim },
                bg,
            );
            draw_text(&mut frame, 280, y + 5, &trunc(&entry.meta, 10), p.muted, bg);
            let is_current = snap
                .current_project_path
                .as_ref()
                .map(|p| p == &entry.path)
                .unwrap_or(false);
            if is_current {
                pill(&mut frame, &p, 360, y + 2, 54, "COURANT", p.green, true);
            }
            y += 21;
        }
    }

    draw_text(
        &mut frame,
        8,
        206,
        "4D/KNOB 1: Choisir | PUSH/BTN 4: Charger | BTN 3: Nouveau",
        p.muted,
        p.bg,
    );

    let slots = [
        Some(("1:NAV", "PROJET".to_string(), 0.0, p.cyan)),
        Some(("2:--", String::new(), 0.0, p.border)),
        Some(("3:--", String::new(), 0.0, p.border)),
        Some((
            "4:COURANT",
            snap.current_project_path
                .as_ref()
                .and_then(|p| std::path::Path::new(p).file_stem())
                .and_then(|s| s.to_str())
                .map(|s| trunc(s, 12))
                .unwrap_or_else(|| "NOUVEAU".to_string()),
            0.0,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);
    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
    frame
}

/// Écran droit — détails et actions du projet sélectionné (FILE).
#[must_use]
pub fn render_project_browser_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let entry = snap.project_entries.get(snap.project_selected);
    header(&mut frame, &p, snap, "FILE | ACTIONS", "PROJET SELECTIONNE");

    if snap.delete_project_confirm {
        panel(&mut frame, &p, 10, 32, 460, 160);
        draw_rect_outline(&mut frame, 10, 32, 460, 160, p.red);
        fill_rect(&mut frame, 12, 34, 456, 30, p.red);
        draw_text_scaled(
            &mut frame,
            24,
            40,
            "SUPPRIMER CE PROJET ?",
            2,
            p.white,
            p.red,
        );
        if let Some(entry) = entry {
            draw_text_scaled(
                &mut frame,
                24,
                74,
                &trunc(&entry.name, 22),
                2,
                p.yellow,
                p.panel,
            );
            draw_text(
                &mut frame,
                24,
                112,
                &trunc(&entry.path, 52),
                p.muted,
                p.panel,
            );
        }
        draw_text(
            &mut frame,
            24,
            138,
            "BTN 6 : CONFIRMER LA SUPPRESSION",
            p.red,
            p.panel,
        );
        draw_text(&mut frame, 24, 158, "BTN 1 : ANNULER", p.cyan, p.panel);
        finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
        return frame;
    }

    if let Some(entry) = entry {
        draw_text_scaled(
            &mut frame,
            10,
            36,
            &trunc(&entry.name, 24),
            2,
            p.white,
            p.bg,
        );
        draw_text(&mut frame, 10, 78, "CHEMIN", p.muted, p.bg);
        draw_text(&mut frame, 10, 94, &trunc(&entry.path, 56), p.dim, p.bg);
        draw_text(
            &mut frame,
            10,
            114,
            &format!("TAILLE: {}", entry.meta),
            p.cyan,
            p.bg,
        );
        let is_current = snap
            .current_project_path
            .as_ref()
            .map(|p| p == &entry.path)
            .unwrap_or(false);
        if is_current {
            pill(
                &mut frame,
                &p,
                10,
                134,
                120,
                "PROJET COURANT",
                p.green,
                true,
            );
        } else {
            pill(
                &mut frame,
                &p,
                10,
                134,
                120,
                "CHARGER (BTN 4)",
                p.accent,
                true,
            );
        }
    } else {
        draw_text(
            &mut frame,
            10,
            44,
            "Aucun projet selectionne.",
            p.white,
            p.bg,
        );
        draw_text(
            &mut frame,
            10,
            66,
            "Appuyez sur [NOUVEAU] (BTN 3) pour creer un projet.",
            p.dim,
            p.bg,
        );
    }

    draw_text(
        &mut frame,
        10,
        156,
        "BTN 3: Nouveau (Kit 808 vierge) | BTN 4/PUSH: Charger",
        p.dim,
        p.bg,
    );
    draw_text(
        &mut frame,
        10,
        172,
        "BTN 5: Sauver | BTN 6: Rapide | BTN 8: Retour",
        p.dim,
        p.bg,
    );
    draw_text(
        &mut frame,
        10,
        188,
        "Raccourci: SHIFT+FILE = Sauvegarde rapide instantanee",
        p.muted,
        p.bg,
    );
    let slots = [
        Some(("5:--", String::new(), 0.0, p.border)),
        Some(("6:--", String::new(), 0.0, p.border)),
        Some(("7:--", String::new(), 0.0, p.border)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);
    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

// ── EVENTS / MACRO / SETTINGS ──────────────────────────────────────────

/// Écran droit — mode EVENTS : liste des notes parcourues (Sound actif ou
/// toutes), la note choisie en surbrillance ; knob 5 = note précédente /
/// suivante.
#[must_use]
pub fn render_events_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let pat = active_pattern(snap);
    let scope_pad = (!snap.events_all).then_some(snap.active_sound);
    let scope = pat.map_or_else(Vec::new, |pt| crate::engine::event_scope(pt, scope_pad));
    let title = if snap.events_all {
        "TOUTES LES NOTES"
    } else {
        "NOTES DU SOUND"
    };
    header(
        &mut frame,
        &p,
        snap,
        title,
        &format!(
            "{} / {}",
            (snap.event_index + 1).min(scope.len()),
            scope.len()
        ),
    );

    // En-tête de colonnes.
    for (x, text) in [
        (10.0, "SON"),
        (176.0, "POSITION"),
        (282.0, "NOTE"),
        (348.0, "VEL"),
        (420.0, "LONG."),
    ] {
        label(&mut frame, x, 44.0, text, 12.0, p.muted, 1.0);
    }
    fill_rect(&mut frame, 8, 49, SCR_W - 16, 1, p.border);

    if let Some(pt) = pat.filter(|_| !scope.is_empty()) {
        // Fenêtre de 7 lignes qui garde la note choisie visible.
        let rows = 7usize;
        let sel = snap.event_index.min(scope.len() - 1);
        let first = sel
            .saturating_sub(rows / 2)
            .min(scope.len().saturating_sub(rows));
        for (row, &i) in scope.iter().enumerate().skip(first).take(rows) {
            let e = &pt.events[i];
            let y = 52 + (row - first) * 19;
            let on = row == sel;
            if on {
                screen_vector::fill_round_rect(
                    &mut frame,
                    6.0,
                    y as f32,
                    (SCR_W - 12) as f32,
                    18.0,
                    4.0,
                    p.panel2,
                );
                screen_vector::fill_round_rect(
                    &mut frame,
                    6.0,
                    y as f32 + 3.0,
                    3.0,
                    12.0,
                    1.5,
                    p.accent,
                );
            }
            let base = (y + 14) as f32;
            cell(
                &mut frame,
                14,
                y + 6,
                8,
                7,
                hue_color(e.pad as usize),
                if on { p.panel2 } else { p.bg },
            );
            let fg = if on { p.white } else { p.dim };
            let name = fit_text(
                &format!("{:02} {}", e.pad + 1, sound_name(snap, e.pad as usize)),
                screen_text::Face::Sans,
                14.0,
                140.0,
            );
            label(
                &mut frame,
                28.0,
                base,
                &name,
                14.0,
                fg,
                if on { 1.8 } else { 1.0 },
            );
            label(
                &mut frame,
                176.0,
                base,
                &fmt_position(e.tick),
                14.0,
                fg,
                1.0,
            );
            label(&mut frame, 282.0, base, &note_name(e.pitch), 14.0, fg, 1.0);
            bar_h(
                &mut frame,
                &p,
                348,
                y + 7,
                54,
                5,
                f32::from(e.velocity) / 127.0,
                p.accent,
            );
            label(
                &mut frame,
                420.0,
                base,
                &fmt_len(e.length_ticks),
                14.0,
                fg,
                1.0,
            );
        }
    } else {
        let msg = "Aucune note : joue en REC ou pose des pas";
        let w = screen_text::width(msg, screen_text::Face::Sans, 15.0);
        label(
            &mut frame,
            (SCR_W as f32 - w) / 2.0,
            120.0,
            msg,
            15.0,
            p.muted,
            1.0,
        );
    }

    let n = scope.len().max(1);
    let slots = [
        Some((
            "NOTE",
            format!(
                "{} / {}",
                (snap.event_index + 1).min(scope.len()),
                scope.len()
            ),
            snap.event_index as f32 / (n - 1).max(1) as f32,
            p.accent,
        )),
        None,
        None,
        None,
    ];
    encoder_row(&mut frame, &p, &slots);
    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

/// Position d'un tick : mesure.temps.double-croche (+ ticks hors grille).
fn fmt_position(tick: u32) -> String {
    let bar = tick / TICKS_PER_BAR as u32 + 1;
    let beat = tick % TICKS_PER_BAR as u32 / 960 + 1;
    let sixteenth = tick % 960 / TICKS_PER_STEP as u32 + 1;
    let rest = tick % TICKS_PER_STEP as u32;
    if rest == 0 {
        format!("{bar}.{beat}.{sixteenth}")
    } else {
        format!("{bar}.{beat}.{sixteenth} +{rest}")
    }
}

/// Écran gauche — mode EVENTS : rouleau de notes du pattern entier, la note
/// choisie en couleur du groupe ; knobs 1–4 POSITION, NOTE, VÉLOCITÉ,
/// LONGUEUR de cette note.
#[must_use]
pub fn render_events_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let snd = snap.active_sound as usize;
    let title = format!("EVENTS — {:02} {}", snd + 1, sound_name(snap, snd));
    let right = if snap.events_all {
        "tous les Sounds"
    } else {
        "Sound actif"
    };
    header_colored(&mut frame, &p, snap, &title, right, hue_color(snd));

    let pat = active_pattern(snap);
    let (x0, y0, w, h) = (8usize, 34usize, SCR_W - 16, 150usize);
    panel(&mut frame, &p, x0, y0, w, h);
    let bars = pat.map_or(1, |pt| u32::from(pt.length.bars.max(1)));
    let len = bars * TICKS_PER_BAR as u32;
    let tx = |tick: u32| x0 as f32 + 6.0 + tick as f32 / len as f32 * (w as f32 - 12.0);
    // Mesures et temps.
    for beat in 0..bars * 4 {
        let x = tx(beat * 960) as usize;
        let c = if beat % 4 == 0 { p.border } else { p.track };
        fill_rect(&mut frame, x, y0 + 4, 1, h - 8, c);
    }
    let scope_pad = (!snap.events_all).then_some(snap.active_sound);
    if let Some(pt) = pat {
        let scope = crate::engine::event_scope(pt, scope_pad);
        let (lo, hi) = scope.iter().fold((127u8, 0u8), |(lo, hi), &i| {
            let n = pt.events[i].pitch;
            (lo.min(n), hi.max(n))
        });
        let (lo, hi) = if scope.is_empty() {
            (36, 48)
        } else {
            let span = (hi - lo).max(11);
            (lo.saturating_sub(1), (lo + span + 1).min(127))
        };
        let rows = f32::from(hi - lo + 1);
        let row_h = ((h as f32 - 12.0) / rows).clamp(3.0, 14.0);
        let ty = |pitch: u8| (y0 + h) as f32 - 6.0 - (f32::from(pitch - lo) + 1.0) * row_h;
        let sel = snap.event_index.min(scope.len().saturating_sub(1));
        for (k, &i) in scope.iter().enumerate() {
            let e = &pt.events[i];
            let (x, y) = (tx(e.tick), ty(e.pitch));
            let bw = (tx(e.tick + e.length_ticks.max(30)) - x).max(3.0);
            let on = k == sel;
            let color = if on {
                p.accent
            } else {
                mix(
                    p.track,
                    hue_color(e.pad as usize),
                    0.35 + 0.65 * f32::from(e.velocity) / 127.0,
                )
            };
            screen_vector::fill_round_rect(&mut frame, x, y, bw, row_h - 1.0, 2.0, color);
            if on {
                draw_rect_outline(
                    &mut frame,
                    x as usize,
                    y as usize,
                    bw as usize + 1,
                    row_h as usize,
                    p.white,
                );
            }
        }
        if snap.transport.is_playing() {
            let tick = (current_tick(snap) % u64::from(len)) as u32;
            fill_rect(&mut frame, tx(tick) as usize, y0 + 2, 1, h - 4, p.white);
        }
    }

    let slots = match &snap.event_sel {
        Some(e) => [
            Some((
                "POSITION",
                fmt_position(e.tick),
                e.tick as f32 / len as f32,
                p.accent,
            )),
            Some((
                "NOTE",
                format!("{} ({})", note_name(e.pitch), e.pitch),
                f32::from(e.pitch) / 127.0,
                p.accent,
            )),
            Some((
                "VÉLOCITÉ",
                format!("{}", e.velocity),
                f32::from(e.velocity) / 127.0,
                p.accent,
            )),
            Some((
                "LONGUEUR",
                fmt_len(e.length_ticks),
                (e.length_ticks as f32 / 3840.0).min(1.0),
                p.accent,
            )),
        ],
        None => [None, None, None, None],
    };
    section_title(&mut frame, &p, "NOTE CHOISIE", p.accent);
    encoder_row(&mut frame, &p, &slots);
    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
    frame
}

/// Écran droit — vue MACRO : macros master / groupe.
#[must_use]
pub fn render_macro_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    header(&mut frame, &p, snap, "MACRO", "MASTER / GROUPE");

    let gvol = focus_group(snap).map(|g| g.volume_db).unwrap_or(0.0);
    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let rows: [(&str, String, f32, DisplayPixel); 5] = [
        (
            "MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        ),
        ("VOL GRP", fmt_db(gvol), db_norm(gvol), p.cyan),
        ("VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan),
        (
            "SWING",
            format!("{:.0}%", snap.project.swing * 100.0),
            snap.project.swing.clamp(0.0, 1.0),
            p.green,
        ),
        (
            "TEMPO",
            format!("{:.1}", snap.transport.bpm),
            ((snap.transport.bpm - 40.0) / 200.0).clamp(0.0, 1.0),
            p.green,
        ),
    ];
    let mut y = 34usize;
    for (label, value, norm, color) in &rows {
        draw_text(&mut frame, 8, y, label, p.dim, p.bg);
        bar_h(&mut frame, &p, 110, y, 300, 14, *norm, *color);
        let vx = SCR_W - 10 - value.len() * 8;
        draw_text(&mut frame, vx, y, value, p.white, p.bg);
        y += 30;
    }

    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let slots = [
        Some(("5:VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some((
            "6:CUTOFF",
            fmt_hz(cut),
            ((cut - 20.0) / 19980.0).clamp(0.0, 1.0),
            p.cyan,
        )),
        Some(("7:RESO", format!("{:.0}%", reso * 100.0), reso, p.cyan)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

/// Écran droit — vue SETTINGS : infos projet / transport / modes.
#[must_use]
pub fn render_settings_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    header(&mut frame, &p, snap, "SETTINGS", "PROJET / TRANSPORT");

    let mut modes = Vec::new();
    if snap.scene_mode {
        modes.push("SCENE");
    }
    if snap.pattern_mode {
        modes.push("PATTERN");
    }
    if snap.select_mode {
        modes.push("SELECT");
    }
    if snap.erase_mode {
        modes.push("ERASE");
    }
    if modes.is_empty() {
        modes.push("JEU");
    }
    let ctrl = match snap.controller_mode {
        1 => "KEYBOARD",
        2 => "CHORDS",
        3 => "STEP",
        _ => "PAD",
    };
    let pad_map = std::env::var("MASCHINE_PAD_MAP").unwrap_or_else(|_| "hw".to_string());

    // ── Carte PROJET ──
    panel(&mut frame, &p, 8, 30, 226, 78);
    fill_rect(&mut frame, 8, 30, 3, 78, p.accent);
    draw_text(
        &mut frame,
        18,
        34,
        &format!("PROJET  v{}", snap.project.schema_version),
        p.accent,
        p.panel,
    );
    fill_rect(&mut frame, 18, 50, 40, 1, p.track);
    kv_row(
        &mut frame,
        &p,
        18,
        54,
        "Nom",
        &trunc(&snap.project.name, 18),
        p.white,
    );
    kv_row(
        &mut frame,
        &p,
        18,
        72,
        "Groupes",
        &format!("{}", snap.project.groups.len()),
        p.cyan,
    );
    kv_row(
        &mut frame,
        &p,
        18,
        90,
        "Scenes / Sect.",
        &format!(
            "{} / {}",
            snap.project.scenes.len(),
            snap.project.song.len()
        ),
        p.cyan,
    );

    // ── Carte TRANSPORT ──
    panel(&mut frame, &p, 8, 112, 226, 78);
    fill_rect(&mut frame, 8, 112, 3, 78, p.green);
    draw_text(&mut frame, 18, 116, "TRANSPORT", p.green, p.panel);
    fill_rect(&mut frame, 18, 132, 40, 1, p.track);
    let (st_txt, st_col) = if snap.transport.is_playing() {
        ("PLAY", p.green)
    } else {
        ("STOP", p.dim)
    };
    kv_row(&mut frame, &p, 18, 136, "Etat", st_txt, st_col);
    kv_row(
        &mut frame,
        &p,
        18,
        154,
        "BPM / Swing",
        &format!(
            "{:.1} / {:.0}%",
            snap.transport.bpm,
            snap.project.swing * 100.0
        ),
        p.white,
    );
    pill(
        &mut frame,
        &p,
        18,
        172,
        52,
        "LOOP",
        p.cyan,
        snap.transport.loop_enabled,
    );
    pill(
        &mut frame,
        &p,
        78,
        172,
        58,
        "METRO",
        p.cyan,
        snap.transport.metronome,
    );
    draw_text(
        &mut frame,
        146,
        176,
        &format!("bar {}.{}", tick_bar(snap) + 1, tick_step(snap) + 1),
        p.muted,
        p.panel,
    );

    // ── Carte AUDIO / JEU ──
    panel(&mut frame, &p, 242, 30, 230, 78);
    fill_rect(&mut frame, 242, 30, 3, 78, p.cyan);
    draw_text(&mut frame, 252, 34, "AUDIO / JEU", p.cyan, p.panel);
    fill_rect(&mut frame, 252, 50, 40, 1, p.track);
    kv_row(
        &mut frame,
        &p,
        252,
        54,
        "Taux",
        &format!("{:.1} kHz", snap.transport.sample_rate as f32 / 1000.0),
        p.white,
    );
    kv_row(
        &mut frame,
        &p,
        252,
        72,
        "Voix",
        &format!("{}", snap.num_voices),
        p.cyan,
    );
    kv_row(
        &mut frame,
        &p,
        252,
        90,
        "Master / Casque",
        &format!(
            "{:.0}% / {:.0}%",
            snap.master_volume * 100.0,
            snap.headphone_volume * 100.0
        ),
        p.white,
    );

    // ── Carte FOCUS ──
    panel(&mut frame, &p, 242, 112, 230, 78);
    fill_rect(
        &mut frame,
        242,
        112,
        3,
        78,
        hue_color(snap.group_focus as usize),
    );
    draw_text(&mut frame, 252, 116, "FOCUS", p.muted, p.panel);
    fill_rect(&mut frame, 252, 132, 40, 1, p.track);
    draw_text(
        &mut frame,
        252,
        136,
        &format!(
            "{} / S{:02} {}",
            group_letter(snap.group_focus),
            snap.active_sound + 1,
            trunc(&sound_name(snap, snap.active_sound as usize), 10)
        ),
        p.white,
        p.panel,
    );
    kv_row(
        &mut frame,
        &p,
        252,
        154,
        "Pads / Jeu",
        &format!("{} / {ctrl}", modes.join("+")),
        p.dim,
    );
    kv_row(&mut frame, &p, 252, 172, "Pad map", &pad_map, p.muted);

    let cut = focus_sound(snap)
        .map(|s| s.filter.cutoff_hz)
        .unwrap_or(20000.0);
    let reso = focus_sound(snap).map(|s| s.filter.resonance).unwrap_or(0.0);
    let s_db = focus_sound(snap).map(|s| s.volume_db).unwrap_or(0.0);
    let slots = [
        Some(("5:VOL SND", fmt_db(s_db), db_norm(s_db), p.cyan)),
        Some((
            "6:CUTOFF",
            fmt_hz(cut),
            ((cut - 20.0) / 19980.0).clamp(0.0, 1.0),
            p.cyan,
        )),
        Some(("7:RESO", format!("{:.0}%", reso * 100.0), reso, p.cyan)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);

    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

// ── Dispatch ───────────────────────────────────────────────────────────

// ── KEYBOARD / CHORDS ──────────────────────────────────────────────────

fn midi_note_name(note: u8) -> String {
    let names = crate::keyboard::NOTE_NAMES;
    let octave = (note as i32 / 12) - 1;
    format!("{}{}", names[note as usize % 12], octave)
}

#[must_use]
pub fn render_keyboard_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let kb = snap.keyboard;
    let scale = kb.scale_def();
    header(
        &mut frame,
        &p,
        snap,
        "KEYBOARD",
        &format!(
            "{} {} | {} | OCT {}",
            kb.root_name(),
            scale.name,
            group_letter(snap.group_focus),
            kb.octave
        ),
    );

    let base_y = 34usize;
    for row in 0..2usize {
        for col in 0..8usize {
            let pad = row * 8 + col;
            let note = kb.keyboard_note(pad as u8);
            let x = 8 + col * 59;
            let y = base_y + row * 76;
            let active = snap.triggered_pads[pad] || snap.pad_activity[pad] > 0.15;
            let bg = if active {
                p.accent
            } else if row == 0 {
                p.panel2
            } else {
                p.panel
            };
            fill_rect(&mut frame, x, y, 55, 70, bg);
            draw_rect_outline(
                &mut frame,
                x,
                y,
                55,
                70,
                if active { p.white } else { p.border },
            );
            let nn = midi_note_name(note);
            draw_text_scaled(
                &mut frame,
                x + 6,
                y + 8,
                &nn,
                2,
                if active { p.bg } else { p.white },
                bg,
            );
            draw_text(
                &mut frame,
                x + 6,
                y + 38,
                &format!("P{:02}", pad + 1),
                p.muted,
                bg,
            );
            draw_text(
                &mut frame,
                x + 6,
                y + 54,
                &format!("N{:03}", note),
                p.dim,
                bg,
            );
        }
    }

    let slots = [
        Some((
            "1:ROOT",
            kb.root_name().to_string(),
            kb.root as f32 / 11.0,
            p.cyan,
        )),
        Some((
            "2:SCALE",
            trunc(scale.name, 10),
            kb.scale as f32 / 7.0,
            p.accent,
        )),
        Some((
            "3:OCT",
            format!("{}", kb.octave),
            kb.octave as f32 / 8.0,
            p.cyan,
        )),
        Some((
            "4:SND",
            format!("{:02}", snap.active_sound + 1),
            snap.active_sound as f32 / 15.0,
            p.green,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);
    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
    frame
}

#[must_use]
pub fn render_keyboard_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let kb = snap.keyboard;
    let scale = kb.scale_def();
    let s_name = sound_name(snap, snap.active_sound as usize);
    header(
        &mut frame,
        &p,
        snap,
        "KEYBOARD | INFO",
        &format!(
            "GRP {} SND {:02}",
            group_letter(snap.group_focus),
            snap.active_sound + 1
        ),
    );
    draw_text_scaled(
        &mut frame,
        10,
        36,
        &format!("{} {}", kb.root_name(), scale.name),
        2,
        p.accent,
        p.bg,
    );
    draw_text(
        &mut frame,
        10,
        78,
        &format!("OCTAVE: {}  |  SON: {}", kb.octave, trunc(&s_name, 20)),
        p.white,
        p.bg,
    );
    draw_text(
        &mut frame,
        10,
        98,
        "DISPLAY BTN / SOFT KEYS: gammes et fondamentales",
        p.dim,
        p.bg,
    );
    draw_text(
        &mut frame,
        10,
        118,
        "Pads: jouent le son actif dans la gamme sélectionnée.",
        p.dim,
        p.bg,
    );
    draw_text(
        &mut frame,
        10,
        138,
        "SHIFT+FILE SAVE: sauvegarde rapide du projet.",
        p.muted,
        p.bg,
    );

    // scale degrees
    let mut x = 10usize;
    let y = 162usize;
    for (i, &st) in scale.intervals.iter().take(12).enumerate() {
        let name = crate::keyboard::NOTE_NAMES[(kb.root as usize + st as usize) % 12];
        let bg = if i == 0 { p.accent } else { p.panel2 };
        fill_rect(&mut frame, x, y, 34, 22, bg);
        draw_rect_outline(&mut frame, x, y, 34, 22, p.border);
        draw_text(
            &mut frame,
            x + 4,
            y + 6,
            name,
            if i == 0 { p.bg } else { p.white },
            bg,
        );
        x += 37;
        if x > 430 {
            break;
        }
    }

    let slots = [
        Some(("5:MODE", "KEYS".to_string(), 1.0, p.green)),
        Some((
            "6:GRP",
            group_letter(snap.group_focus).to_string(),
            snap.group_focus as f32 / 7.0,
            p.cyan,
        )),
        Some(("7:--", String::new(), 0.0, p.border)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);
    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

#[must_use]
pub fn render_chords_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let kb = snap.keyboard;
    let scale = kb.scale_def();
    let chord = kb.chord_def();
    header(
        &mut frame,
        &p,
        snap,
        "CHORDS",
        &format!(
            "{} {} | {} INV{}",
            kb.root_name(),
            scale.name,
            chord.name,
            kb.inversion
        ),
    );
    let base_y = 34usize;
    for row in 0..2usize {
        for col in 0..8usize {
            let pad = row * 8 + col;
            let notes = kb.chord_notes(pad as u8);
            let x = 8 + col * 59;
            let y = base_y + row * 76;
            let active = snap.triggered_pads[pad] || snap.pad_activity[pad] > 0.15;
            let bg = if active { p.accent } else { p.panel };
            fill_rect(&mut frame, x, y, 55, 70, bg);
            draw_rect_outline(
                &mut frame,
                x,
                y,
                55,
                70,
                if active { p.white } else { p.border },
            );
            let label = if let Some(n) = notes.first() {
                midi_note_name(*n)
            } else {
                "-".into()
            };
            draw_text(
                &mut frame,
                x + 4,
                y + 6,
                &trunc(&label, 6),
                if active { p.bg } else { p.white },
                bg,
            );
            draw_text(
                &mut frame,
                x + 4,
                y + 24,
                &format!("{}n", notes.len()),
                p.cyan,
                bg,
            );
            draw_text(
                &mut frame,
                x + 4,
                y + 42,
                &format!("P{:02}", pad + 1),
                p.muted,
                bg,
            );
            let deg = (pad % scale.intervals.len()) + 1;
            draw_text(&mut frame, x + 4, y + 56, &format!("deg{}", deg), p.dim, bg);
        }
    }
    let slots = [
        Some((
            "1:ROOT",
            kb.root_name().to_string(),
            kb.root as f32 / 11.0,
            p.cyan,
        )),
        Some((
            "2:SCALE",
            trunc(scale.name, 8),
            kb.scale as f32 / 7.0,
            p.accent,
        )),
        Some((
            "3:CHORD",
            trunc(chord.name, 8),
            kb.chord as f32 / 7.0,
            p.cyan,
        )),
        Some((
            "4:INV",
            format!("{}", kb.inversion),
            kb.inversion as f32 / 3.0,
            p.green,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);
    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
    frame
}

#[must_use]
pub fn render_chords_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let kb = snap.keyboard;
    let scale = kb.scale_def();
    let chord = kb.chord_def();
    let s_name = sound_name(snap, snap.active_sound as usize);
    header(
        &mut frame,
        &p,
        snap,
        "CHORDS | INFO",
        &format!("{} {} {}", kb.root_name(), scale.name, chord.name),
    );
    draw_text_scaled(
        &mut frame,
        10,
        36,
        &format!("{} {}", kb.root_name(), chord.name),
        2,
        p.accent,
        p.bg,
    );
    draw_text(
        &mut frame,
        10,
        78,
        &format!(
            "Gamme: {}  Inversion: {}  Son: {}",
            scale.name,
            kb.inversion,
            trunc(&s_name, 14)
        ),
        p.white,
        p.bg,
    );
    draw_text(
        &mut frame,
        10,
        98,
        "Pads: accords diatoniques sur la gamme sélectionnée.",
        p.dim,
        p.bg,
    );
    draw_text(
        &mut frame,
        10,
        118,
        "Top buttons: ROOT/SCALE/CHORD/INV.",
        p.dim,
        p.bg,
    );
    let slots = [
        Some((
            "5:OCT",
            format!("{}", kb.octave),
            kb.octave as f32 / 8.0,
            p.cyan,
        )),
        Some(("6:--", String::new(), 0.0, p.border)),
        Some(("7:--", String::new(), 0.0, p.border)),
        Some((
            "8:MASTER",
            format!("{:.0}%", snap.master_volume * 100.0),
            snap.master_volume,
            p.accent,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);
    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

// ── AMPLI GUITARE (page PLUGIN 2) ────────────────────────────────────────

/// Libellés des 8 soft keys de la page ampli guitare (identiques dans
/// l'engine et dans le miroir GUI).
pub const GUITAR_SOFT_LABELS: [&str; 8] = [
    "AMP <", "AMP >", "CAB <", "CAB >", "ACTIF", "MONITOR", "BYPASS", "INPUT+",
];

const GUITAR_AMP_NAMES: [&str; 4] = ["VOX AC30", "FENDER BF", "TWEED", "HIGH GAIN"];
const GUITAR_CAB_NAMES: [&str; 4] = ["VOX 12", "FENDER 12", "MODERN 12", "FLAT"];
const GUITAR_SPECTRUM_BINS: usize = 48;

/// État de l'ampli guitare nécessaire aux écrans. Fourni par `maschine-app`
/// (qui détient les paramètres atomiques et la télémétrie audio) ; l'engine
/// ne connaît pas le crate DSP et reçoit donc une simple structure de données.
#[derive(Debug, Clone)]
pub struct GuitarScreenData {
    pub model: u8,
    pub cab: u8,
    pub enabled: bool,
    pub monitor: bool,
    pub bypass: bool,
    /// Canal d'entrée sélectionné : 0=LINE1, 1=LINE2, 2=STEREO.
    pub input_channel: u8,
    /// Valeurs normalisées 0..1 : gain, bass, mid, treble, presence, sag,
    /// master, monitor.
    pub params: [f32; 8],
    pub input_db: f32,
    pub output_db: f32,
    pub cpu_load: f32,
    pub latency_ms: f32,
    pub xruns: u32,
    /// Ex. "A4 +3c" — construit par l'app depuis la lecture de l'accordeur.
    pub tuner_label: Option<String>,
    pub tuner_freq: f32,
    /// Spectre en dBFS, déjà réduit à `GUITAR_SPECTRUM_BINS` bandes.
    pub spectrum: [f32; GUITAR_SPECTRUM_BINS],
}

impl Default for GuitarScreenData {
    fn default() -> Self {
        Self {
            model: 0,
            cab: 0,
            enabled: true,
            monitor: true,
            bypass: false,
            input_channel: 0,
            params: [0.5; 8],
            input_db: -120.0,
            output_db: -120.0,
            cpu_load: 0.0,
            latency_ms: 0.0,
            xruns: 0,
            tuner_label: None,
            tuner_freq: 0.0,
            spectrum: [-120.0; GUITAR_SPECTRUM_BINS],
        }
    }
}

fn guitar_soft_active(g: &GuitarScreenData) -> [bool; 8] {
    [
        false, false, false, false, g.enabled, g.monitor, g.bypass, false,
    ]
}

fn guitar_param(g: &GuitarScreenData, index: usize) -> f32 {
    g.params.get(index).copied().unwrap_or(0.5).clamp(0.0, 1.0)
}

fn guitar_pct(v: f32) -> String {
    format!("{:.0}%", v * 100.0)
}

/// Écran gauche — page AMPLI GUITARE : préampli, saturation et tone stack.
#[must_use]
pub fn render_guitar_amp_left(snap: &EngineSnapshot, g: &GuitarScreenData) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let model = GUITAR_AMP_NAMES[(g.model as usize) % GUITAR_AMP_NAMES.len()];
    header(
        &mut frame,
        &p,
        snap,
        "GUITAR AMP | PREAMP",
        &format!("{} | {}", model, if g.bypass { "BYPASS" } else { "ON" }),
    );
    // Courbe de saturation du préampli (tanh asymétrique).
    panel(&mut frame, &p, 8, 28, 226, 150);
    draw_text(
        &mut frame,
        14,
        32,
        "TUBE PREAMP / SATURATION",
        p.dim,
        p.panel,
    );
    let cx = 18usize;
    let cy = 44usize;
    let cw = 204usize;
    let ch = 58usize;
    fill_rect(&mut frame, cx, cy, cw, ch, p.track);
    // Ligne de référence à zéro.
    fill_rect(&mut frame, cx, cy + ch / 2, cw, 1, p.border);
    let drive = 1.0 + guitar_param(g, 0) * 7.0;
    let bias: f32 = 0.22;
    let tb = bias.tanh();
    let norm = (1.0 - tb * tb).max(1e-4);
    for x in 0..cw {
        let input = (x as f32 / cw as f32) * 4.0 - 2.0;
        let y = (((input * drive + bias).tanh() - tb) / norm).clamp(-1.0, 1.0);
        let py = if y >= 0.0 {
            cy + ch / 2 - (y * (ch / 2 - 3) as f32) as usize
        } else {
            cy + ch / 2 + (-y * (ch / 2 - 3) as f32) as usize
        };
        fill_rect(&mut frame, cx + x, py.min(cy + ch - 2), 1, 2, p.red);
    }
    draw_text(
        &mut frame,
        cx + 6,
        cy + 4,
        &format!("GAIN {}", guitar_pct(guitar_param(g, 0))),
        p.white,
        p.track,
    );

    // Tone stack 3 bandes.
    draw_text(&mut frame, 14, 110, "TONE STACK", p.dim, p.panel);
    for (i, label) in ["BASS", "MID", "TREB"].iter().enumerate() {
        let y = 126 + i * 16;
        draw_text(&mut frame, 14, y, label, p.muted, p.panel);
        bar_h(
            &mut frame,
            &p,
            64,
            y + 2,
            154,
            7,
            guitar_param(g, i + 1),
            p.cyan,
        );
    }

    // Voix, baffle et état.
    panel(&mut frame, &p, 246, 28, 226, 150);
    draw_text(&mut frame, 252, 32, "AMPLI & BAFFLE", p.dim, p.panel);
    fill_rect(&mut frame, 252, 46, 214, 30, p.panel2);
    let mx = 252 + 214usize.saturating_sub(model.len() * 8) / 2;
    draw_text(&mut frame, mx, 53, model, p.accent, p.panel2);
    let cab = GUITAR_CAB_NAMES[(g.cab as usize) % GUITAR_CAB_NAMES.len()];
    draw_text(
        &mut frame,
        252,
        84,
        &format!("CAB : {}", trunc(cab, 14)),
        p.cyan,
        p.panel,
    );
    pill(&mut frame, &p, 252, 104, 54, "ACTIF", p.green, g.enabled);
    pill(&mut frame, &p, 312, 104, 54, "MON", p.cyan, g.monitor);
    pill(&mut frame, &p, 372, 104, 54, "BYP", p.red, g.bypass);
    // Canal d'entrée sélectionné
    let input_label = match g.input_channel {
        0 => "IN: LINE 1",
        1 => "IN: LINE 2",
        _ => "IN: STEREO",
    };
    // Les boutons 5–8 sont libellés en haut de l'écran : ici, l'entrée seule.
    draw_text(&mut frame, 252, 136, input_label, p.accent, p.panel);

    let slots = [
        Some((
            "1:GAIN",
            guitar_pct(guitar_param(g, 0)),
            guitar_param(g, 0),
            p.red,
        )),
        Some((
            "2:BASS",
            guitar_pct(guitar_param(g, 1)),
            guitar_param(g, 1),
            p.cyan,
        )),
        Some((
            "3:MID",
            guitar_pct(guitar_param(g, 2)),
            guitar_param(g, 2),
            p.cyan,
        )),
        Some((
            "4:TREBLE",
            guitar_pct(guitar_param(g, 3)),
            guitar_param(g, 3),
            p.cyan,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);
    finish_soft_keys(
        &mut frame,
        &p,
        &GUITAR_SOFT_LABELS,
        0,
        &guitar_soft_active(g),
    );
    frame
}

/// Écran droit — page AMPLI GUITARE : baffle, analyse et diagnostics.
#[must_use]
pub fn render_guitar_amp_right(snap: &EngineSnapshot, g: &GuitarScreenData) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let cab = GUITAR_CAB_NAMES[(g.cab as usize) % GUITAR_CAB_NAMES.len()];
    header(
        &mut frame,
        &p,
        snap,
        "GUITAR AMP | ANALYSE",
        &format!("{} | {:.0}ms", cab, g.latency_ms),
    );

    // Niveaux et baffle.
    panel(&mut frame, &p, 8, 28, 226, 150);
    draw_text(&mut frame, 14, 32, "NIVEAUX & BAFFLE", p.dim, p.panel);
    draw_text(&mut frame, 14, 48, "IN", p.muted, p.panel);
    vu_h(
        &mut frame,
        &p,
        38,
        48,
        186,
        10,
        db_norm(g.input_db).max(0.0),
        0.0,
    );
    draw_text(
        &mut frame,
        14,
        64,
        &format!("{:.1} dB", g.input_db.max(-99.0)),
        p.white,
        p.panel,
    );
    draw_text(&mut frame, 14, 82, "OUT", p.muted, p.panel);
    vu_h(
        &mut frame,
        &p,
        38,
        82,
        186,
        10,
        db_norm(g.output_db).max(0.0),
        0.0,
    );
    draw_text(
        &mut frame,
        14,
        98,
        &format!("{:.1} dB", g.output_db.max(-99.0)),
        p.white,
        p.panel,
    );
    draw_text(&mut frame, 14, 118, "TONE / PRESENCE", p.dim, p.panel);
    bar_h(
        &mut frame,
        &p,
        14,
        134,
        210,
        8,
        guitar_param(g, 4),
        p.accent,
    );
    draw_text(&mut frame, 14, 150, "SAG", p.muted, p.panel);
    bar_h(
        &mut frame,
        &p,
        54,
        150,
        170,
        8,
        guitar_param(g, 5),
        p.yellow,
    );

    // Accordeur, spectre et diagnostics.
    panel(&mut frame, &p, 246, 28, 226, 150);
    draw_text(&mut frame, 252, 32, "ACCORDEUR / SPECTRE", p.dim, p.panel);
    match g.tuner_label.as_deref() {
        Some(label) => {
            draw_text(&mut frame, 252, 46, &trunc(label, 18), p.accent, p.panel);
            draw_text(
                &mut frame,
                366,
                46,
                &format!("{:.0}Hz", g.tuner_freq),
                p.muted,
                p.panel,
            );
        }
        None => draw_text(&mut frame, 252, 46, "-- (jouer une note)", p.muted, p.panel),
    }
    // Spectre 48 bandes.
    let sx = 252usize;
    let sy = 66usize;
    let sw = 214usize;
    let sh = 64usize;
    fill_rect(&mut frame, sx, sy, sw, sh, p.track);
    let bw = sw / GUITAR_SPECTRUM_BINS;
    for (i, &db) in g.spectrum.iter().enumerate() {
        let norm = ((db + 90.0) / 90.0).clamp(0.0, 1.0);
        let h = (norm * sh as f32) as usize;
        if h == 0 {
            continue;
        }
        let col = if norm > 0.85 {
            p.red
        } else if norm > 0.65 {
            p.yellow
        } else {
            p.cyan
        };
        fill_rect(
            &mut frame,
            sx + i * bw,
            sy + sh - h,
            bw.saturating_sub(1),
            h,
            col,
        );
    }
    draw_text(
        &mut frame,
        252,
        136,
        &format!("CPU {:.0}%  XRUN {}", g.cpu_load * 100.0, g.xruns),
        if g.cpu_load > 0.8 || g.xruns > 0 {
            p.yellow
        } else {
            p.green
        },
        p.panel,
    );
    draw_text(
        &mut frame,
        252,
        152,
        &format!(
            "LAT {:.1}ms | {}",
            g.latency_ms,
            if g.monitor { "MON" } else { "OFF" }
        ),
        p.muted,
        p.panel,
    );

    let slots = [
        Some((
            "5:PRES",
            guitar_pct(guitar_param(g, 4)),
            guitar_param(g, 4),
            p.accent,
        )),
        Some((
            "6:SAG",
            guitar_pct(guitar_param(g, 5)),
            guitar_param(g, 5),
            p.yellow,
        )),
        Some((
            "7:MASTER",
            guitar_pct(guitar_param(g, 6)),
            guitar_param(g, 6),
            p.green,
        )),
        Some((
            "8:MONITOR",
            guitar_pct(guitar_param(g, 7)),
            guitar_param(g, 7),
            p.cyan,
        )),
    ];
    encoder_row(&mut frame, &p, &slots);
    finish_soft_keys(
        &mut frame,
        &p,
        &GUITAR_SOFT_LABELS,
        4,
        &guitar_soft_active(g),
    );
    frame
}

// ── PLUGIN : pages de paramètres (comme le mode PLUG-IN de la MK3) ──────

use crate::plugin_pages::{self as pp, Instrument, PageView};

/// Instrument affiché, page courante et valeurs de la page (`None` si le
/// pad focalisé est vide).
fn plugin_page_view(snap: &EngineSnapshot) -> Option<(Instrument, usize, Option<PageView>)> {
    let inst = pp::instrument_for(
        snap.left_tab,
        snap.plugin_page,
        snap.focus_sound_kick.is_some(),
        snap.focus_sound_wavetable.is_some(),
        snap.focus_sound_acid.is_some(),
    )?;
    let page = (snap.synth_param_page as usize).min(inst.page_count() - 1);
    let view = match inst {
        Instrument::Kick => snap
            .focus_sound_kick
            .as_ref()
            .map(|k| pp::view(&pp::KICK_PAGES, page, k)),
        Instrument::Wavetable => snap
            .focus_sound_wavetable
            .as_ref()
            .map(|w| pp::view(&pp::WAVETABLE_PAGES, page, w)),
        Instrument::Acid => Some(pp::view(&pp::ACID_PAGES, page, &snap.acid_params)),
        Instrument::Sampler => focus_sound(snap).map(|s| pp::view(&pp::SAMPLER_PAGES, page, s)),
    };
    Some((inst, page, view))
}

/// Les 4 slots d'encodeurs d'une moitié de page (`first` = 0 ou 4).
fn page_slots<'a>(
    view: Option<&'a PageView>,
    first: usize,
    color: DisplayPixel,
) -> [EncSlot<'a>; 4] {
    std::array::from_fn(|i| {
        let pv = view?.params[first + i].as_ref()?;
        Some((pv.name, pv.value.clone(), pv.norm, color))
    })
}

/// Mélange de deux couleurs (t = 0 → a, 1 → b).
fn mix(a: DisplayPixel, b: DisplayPixel, t: f32) -> DisplayPixel {
    let (ar, ag, ab) = rgb565_to_rgb(a);
    let (br, bg, bb) = rgb565_to_rgb(b);
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t.clamp(0.0, 1.0)) as u8;
    DisplayPixel::from_rgb(m(ar, br), m(ag, bg), m(ab, bb))
}

/// Petite case pleine aux coins adoucis (grilles de pas) : rapide, sans
/// rastérisation, les coins sont mêlés au fond `bg`.
fn cell(
    frame: &mut DisplayFrame,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: DisplayPixel,
    bg: DisplayPixel,
) {
    if w < 3 || h < 3 {
        fill_rect(frame, x, y, w, h, color);
        return;
    }
    fill_rect(frame, x, y, w, h, color);
    let soft = mix(bg, color, 0.45);
    for (cx, cy) in [
        (x, y),
        (x + w - 1, y),
        (x, y + h - 1),
        (x + w - 1, y + h - 1),
    ] {
        fill_rect(frame, cx, cy, 1, 1, soft);
    }
}

/// Titre de section au-dessus d'une rangée de 4 knobs, avec un filet.
fn section_title(frame: &mut DisplayFrame, p: &Pal, text: &str, color: DisplayPixel) {
    let y = SCR_H - FOOTER_H - ENC_H - 20;
    fill_rect(frame, 0, y, SCR_W, 14, p.bg);
    let w = bold(frame, 8.0, y as f32 + 11.0, text, 12.0, color);
    let x = if text.is_empty() {
        8
    } else {
        8 + w as usize + 8
    };
    fill_rect(frame, x, y + 6, (SCR_W - 8).saturating_sub(x), 1, p.border);
}

/// Écran gauche PLUGIN : la chaîne de plugins (instrument du Sound, puis
/// effets du Sound, du groupe et du Master — parcourue au 4D) et l'élément
/// sélectionné en grand avec son visuel ; knobs 1–4 de sa page.
#[must_use]
pub fn render_plugin_left(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Left, p.bg);
    let Some((inst, _, view)) = plugin_page_view(snap) else {
        return render_sampler_left(snap);
    };
    let snd = snap.active_sound as usize;
    let tint = hue_color(snd);
    let fx = snap.plugin_fx.as_ref();
    let adding = selected_slot(snap).is_some_and(|s| s.empty);
    let title = match (fx, adding) {
        (Some(fx), _) => fx.fx_type.label(),
        (None, true) => "Insérer un effet",
        (None, false) => inst.title(),
    };
    header_colored(
        &mut frame,
        &p,
        snap,
        title,
        &format!(
            "{} {:02} {}",
            group_letter(snap.group_focus),
            snd + 1,
            sound_name(snap, snd)
        ),
        tint,
    );

    plugin_chain(&mut frame, &p, snap, (6, 32, 142, 152));

    let (vx, vw) = (158usize, SCR_W - 166);
    match (fx, adding) {
        (Some(fx), _) => {
            let owner = selected_slot(snap).map_or(0, |s| s.level);
            bold(
                &mut frame,
                vx as f32,
                50.0,
                fx.fx_type.label(),
                22.0,
                p.white,
            );
            let info = format!(
                "Insert {}  ·  {}",
                level_name(snap, owner),
                if fx.enabled { "actif" } else { "contourné" }
            );
            label(&mut frame, vx as f32, 67.0, &info, 14.0, p.dim, 1.0);
            draw_fx_visual(&mut frame, &p, fx, (vx, 76, vw, 104), p.accent);
        }
        (None, true) => {
            let owner = selected_slot(snap).map_or(0, |s| s.level);
            bold(&mut frame, vx as f32, 50.0, "Nouvel effet", 22.0, p.white);
            let info = format!("en fin de chaîne {}", level_name(snap, owner));
            label(&mut frame, vx as f32, 67.0, &info, 14.0, p.dim, 1.0);
            add_fx_help(&mut frame, &p, (vx, 80, vw, 100));
        }
        (None, false) => {
            let (title, info) = instrument_caption(snap, inst);
            let title = fit_text(&title, screen_text::Face::Bold, 22.0, vw as f32);
            bold(&mut frame, vx as f32, 50.0, &title, 22.0, p.white);
            let info = fit_text(&info, screen_text::Face::Sans, 14.0, vw as f32);
            label(&mut frame, vx as f32, 67.0, &info, 14.0, p.dim, 1.0);
            if view.is_some() {
                draw_instrument_visual(&mut frame, &p, snap, inst, (vx, 76, vw, 104), tint);
            } else {
                panel(&mut frame, &p, vx, 76, vw, 104);
                let msg = "Pad vide";
                let w = screen_text::width(msg, screen_text::Face::Bold, 20.0);
                bold(
                    &mut frame,
                    (vx + vw / 2) as f32 - w / 2.0,
                    134.0,
                    msg,
                    20.0,
                    p.muted,
                );
            }
        }
    }

    let fx_view = fx.map(|fx| pp::view(pp::fx_pages(fx.fx_type), 0, fx));
    let view = fx_view.or(if adding || fx.is_some() { None } else { view });
    let section = view.as_ref().map_or("", |v| v.sections[0]);
    section_title(&mut frame, &p, section, tint);
    encoder_row(&mut frame, &p, &page_slots(view.as_ref(), 0, tint));
    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 0, &snap.soft_active);
    frame
}

/// Écran droit PLUGIN : onglets des pages (Page ◄ ►), visuel de la page,
/// bandeau de transition au changement de page ou d'emplacement ; knobs 5–8.
#[must_use]
pub fn render_plugin_right(snap: &EngineSnapshot) -> DisplayFrame {
    let p = Pal::of(snap);
    let mut frame = DisplayFrame::solid(DisplayId::Right, p.bg);
    let Some((inst, page, view)) = plugin_page_view(snap) else {
        return render_sampler_right(snap);
    };
    let tint = hue_color(snap.active_sound as usize);
    let fx = snap.plugin_fx.as_ref();
    let adding = selected_slot(snap).is_some_and(|s| s.empty);
    let fx_view = fx.map(|fx| pp::view(pp::fx_pages(fx.fx_type), 0, fx));
    let (names, page, view) = match (fx, adding) {
        (Some(fx), _) => (vec![pp::fx_pages(fx.fx_type)[0].name], 0, fx_view),
        (None, true) => (vec!["EFFETS"], 0, None),
        (None, false) => (inst.page_names(), page, view),
    };
    let slot_info = format!(
        "SLOT {}/{}",
        snap.plugin_slot + 1,
        snap.plugin_slots.len().max(1)
    );
    header_colored(
        &mut frame,
        &p,
        snap,
        names[page],
        &format!("PAGE {}/{}  ·  {slot_info}", page + 1, names.len()),
        tint,
    );

    // Onglets : toutes les pages, la page courante en couleur du groupe.
    let tw = (SCR_W - 16) / names.len().max(1);
    for (i, name) in names.iter().enumerate() {
        let x = 8 + i * tw;
        let on = i == page;
        let bg = if on { p.accent } else { p.panel };
        screen_vector::fill_round_rect(
            &mut frame,
            x as f32 + 1.0,
            32.0,
            tw as f32 - 2.0,
            20.0,
            4.0,
            bg,
        );
        let face = if on {
            screen_text::Face::Bold
        } else {
            screen_text::Face::Sans
        };
        let text = fit_text(name, face, 14.0, (tw - 10) as f32);
        let w = screen_text::width(&text, face, 14.0);
        let fg = if on { p.ink } else { p.dim };
        screen_text::draw(
            &mut frame,
            (x + tw / 2) as f32 - w / 2.0,
            46.5,
            &text,
            face,
            14.0,
            fg,
            1.0,
        );
    }

    let rect = (8, 60, SCR_W - 16, 114);
    match (fx, adding, view.as_ref()) {
        (Some(fx), _, _) => draw_fx_visual(&mut frame, &p, fx, rect, p.accent),
        (None, true, _) => add_fx_help(&mut frame, &p, rect),
        (None, false, Some(v)) => draw_page_visual(&mut frame, &p, v, rect, tint),
        (None, false, None) => {
            let msg = "Pad vide : charge un son depuis le BROWSER";
            let w = screen_text::width(msg, screen_text::Face::Sans, 15.0);
            label(
                &mut frame,
                (SCR_W as f32 - w) / 2.0,
                120.0,
                msg,
                15.0,
                p.dim,
                1.0,
            );
        }
    }

    // Transition : le nom de la page (ou de l'effet) s'affiche un instant.
    if snap.page_flash > 0.3 {
        let (x, y, w, h) = (110.0f32, 76.0f32, 260.0f32, 72.0f32);
        screen_vector::fill_round_rect(
            &mut frame,
            x - 1.0,
            y - 1.0,
            w + 2.0,
            h + 2.0,
            8.0,
            p.accent,
        );
        screen_vector::fill_round_rect(
            &mut frame,
            x + 1.0,
            y + 1.0,
            w - 2.0,
            h - 2.0,
            7.0,
            p.panel2,
        );
        let name = fit_text(names[page], screen_text::Face::Bold, 26.0, w - 24.0);
        let nw = screen_text::width(&name, screen_text::Face::Bold, 26.0);
        bold(
            &mut frame,
            x + w / 2.0 - nw / 2.0,
            y + 38.0,
            &name,
            26.0,
            p.white,
        );
        let n = names.len();
        let dots_w = (n * 14 - 6) as f32;
        for i in 0..n {
            let dx = x + (w - dots_w) / 2.0 + (i * 14) as f32;
            let c = if i == page { p.accent } else { p.track };
            screen_vector::fill_round_rect(&mut frame, dx, y + 50.0, 8.0, 8.0, 4.0, c);
        }
    }

    let section = view.as_ref().map_or("", |v| v.sections[1]);
    section_title(&mut frame, &p, section, tint);
    encoder_row(&mut frame, &p, &page_slots(view.as_ref(), 4, tint));
    finish_soft_keys(&mut frame, &p, &snap.soft_labels, 4, &snap.soft_active);
    frame
}

/// Emplacement sélectionné de la chaîne de plugins.
fn selected_slot(snap: &EngineSnapshot) -> Option<&crate::engine::PluginSlotView> {
    snap.plugin_slots.get(snap.plugin_slot)
}

/// Nom d'un niveau de la chaîne : Sound, groupe ou Master.
fn level_name(snap: &EngineSnapshot, level: u8) -> String {
    match level {
        0 => format!("Sound {}", snap.active_sound + 1),
        1 => format!("Groupe {}", group_letter(snap.group_focus)),
        _ => "Master".to_string(),
    }
}

/// Chaîne de plugins (gauche) : titres de niveau, emplacements, sélection.
/// Fait défiler la liste pour garder la sélection visible.
fn plugin_chain(
    frame: &mut DisplayFrame,
    p: &Pal,
    snap: &EngineSnapshot,
    (x, y, w, h): (usize, usize, usize, usize),
) {
    enum Row<'a> {
        Level(u8),
        Slot(usize, &'a crate::engine::PluginSlotView),
    }
    let mut rows = Vec::new();
    let mut level = None;
    for (i, slot) in snap.plugin_slots.iter().enumerate() {
        if level != Some(slot.level) {
            level = Some(slot.level);
            rows.push(Row::Level(slot.level));
        }
        rows.push(Row::Slot(i, slot));
    }
    let row_h = |r: &Row| if matches!(r, Row::Level(_)) { 16 } else { 19 };
    // Défilement : la ligne sélectionnée reste dans la fenêtre.
    let sel = rows
        .iter()
        .position(|r| matches!(r, Row::Slot(i, _) if *i == snap.plugin_slot))
        .unwrap_or(0);
    let mut first = 0;
    while first < sel && rows[first..=sel].iter().map(row_h).sum::<usize>() > h {
        first += 1;
    }
    let mut cy = y;
    for row in &rows[first..] {
        let rh = row_h(row);
        if cy + rh > y + h {
            break;
        }
        match row {
            Row::Level(l) => {
                let text = level_name(snap, *l).to_uppercase();
                bold(
                    frame,
                    (x + 2) as f32,
                    (cy + 12) as f32,
                    &text,
                    11.0,
                    p.muted,
                );
            }
            Row::Slot(i, slot) => {
                let on = *i == snap.plugin_slot;
                if on {
                    screen_vector::fill_round_rect(
                        frame, x as f32, cy as f32, w as f32, 18.0, 4.0, p.panel2,
                    );
                    screen_vector::fill_round_rect(
                        frame,
                        x as f32,
                        cy as f32 + 3.0,
                        3.0,
                        12.0,
                        1.5,
                        p.accent,
                    );
                }
                let fg = if slot.empty || slot.bypassed {
                    p.muted
                } else if on {
                    p.white
                } else {
                    p.dim
                };
                let face = if on {
                    screen_text::Face::Bold
                } else {
                    screen_text::Face::Sans
                };
                let text = fit_text(&slot.name, face, 15.0, (w - 18) as f32);
                let tw = screen_text::draw(
                    frame,
                    (x + 9) as f32,
                    (cy + 14) as f32,
                    &text,
                    face,
                    15.0,
                    fg,
                    1.0,
                );
                if slot.bypassed {
                    fill_rect(frame, x + 9, cy + 9, tw as usize, 1, p.muted);
                }
            }
        }
        cy += rh;
    }
    if first > 0 {
        // Liste défilée : petit triangle « plus haut ».
        let (tx, ty) = ((x + w - 10) as f32, (y + 3) as f32);
        let tri = [(tx, ty + 6.0), (tx + 4.0, ty), (tx + 8.0, ty + 6.0)];
        if let Some(path) = screen_vector::polyline(&tri) {
            screen_vector::fill(frame, &path, screen_vector::Paint::Solid(p.muted, 1.0));
        }
    }
}

/// Emplacement libre : les effets qu'on peut insérer, avec leur bouton.
fn add_fx_help(frame: &mut DisplayFrame, p: &Pal, (x, y, w, h): (usize, usize, usize, usize)) {
    panel(frame, p, x, y, w, h);
    // Même ordre que les boutons : 1–4 à gauche, 5–8 à droite.
    let items = [
        ("FILTER", "filtre résonant"),
        ("EQ", "3 bandes"),
        ("DISTO", "saturation"),
        ("COMP", "dynamique"),
        ("LO-FI", "bits & fréquence"),
        ("CHORUS", "modulation"),
        ("DELAY", "écho stéréo"),
        ("REVERB", "espace"),
    ];
    let col_w = w / 2;
    for (i, (name, what)) in items.iter().enumerate() {
        let (cx, cy) = (x + 12 + (i / 4) * col_w, y + 20 + (i % 4) * 23);
        let nw = bold(frame, cx as f32, cy as f32, name, 14.0, p.white);
        label(
            frame,
            cx as f32 + nw + 8.0,
            cy as f32,
            what,
            12.0,
            p.muted,
            1.0,
        );
    }
}

/// Visuel d'un effet : réponse du filtre, courbe de saturation, courbe de
/// compression, échos du delay, décroissance de la réverbe.
fn draw_fx_visual(
    frame: &mut DisplayFrame,
    p: &Pal,
    fx: &crate::fx::FxSlot,
    (x, y, w, h): (usize, usize, usize, usize),
    color: DisplayPixel,
) {
    use crate::fx::FxType as T;
    use screen_vector::Paint;
    if fx.fx_type == T::Filter {
        let kind = ["LP", "HP", "BP"][(fx.p3.round().clamp(0.0, 2.0)) as usize];
        draw_filter_curve(frame, p, (x, y, w, h), fx.p1, fx.p2, kind, color);
    } else {
        panel(frame, p, x, y, w, h);
        let (xf, yf, wf, hf) = (
            x as f32 + 8.0,
            y as f32 + 8.0,
            w as f32 - 16.0,
            h as f32 - 16.0,
        );
        let curve: Vec<(f32, f32)> = match fx.fx_type {
            T::Distortion => {
                // Entrée -1..1 → sortie (gain compensé, comme le DSP).
                let pre = 10f32.powf(fx.p1.clamp(0.0, 1.0) * 36.0 / 20.0);
                let out = 10f32.powf(fx.p4.clamp(-24.0, 24.0) / 20.0) / pre.sqrt();
                fill_rect(frame, x + 8, y + h / 2, w - 16, 1, p.track);
                (0..=64)
                    .map(|i| {
                        let xin = -1.0 + 2.0 * i as f32 / 64.0;
                        let shaped = if fx.p3 > 0.5 {
                            (xin * pre).clamp(-1.0, 1.0)
                        } else {
                            (xin * pre).tanh()
                        };
                        let yv = (shaped * out * pre.sqrt().min(4.0)).clamp(-1.0, 1.0);
                        (xf + (xin + 1.0) / 2.0 * wf, yf + hf / 2.0 - yv * hf / 2.0)
                    })
                    .collect()
            }
            T::Compressor => {
                // Entrée -48..0 dB → sortie, genou doux et compensation.
                let (t, r) = (fx.p1.clamp(-48.0, 0.0), fx.p2.clamp(1.0, 20.0));
                let slope = 1.0 / r - 1.0;
                let makeup = -0.5 * slope * -t;
                let tx = xf + (t + 48.0) / 48.0 * wf;
                fill_rect(frame, tx as usize, y + 8, 1, h - 16, p.track);
                (0..=48)
                    .map(|i| {
                        let db = -48.0 + i as f32;
                        let over = db - t;
                        let gr = if 2.0 * over < -6.0 {
                            0.0
                        } else if 2.0 * over.abs() <= 6.0 {
                            slope * (over + 3.0).powi(2) / 12.0
                        } else {
                            slope * over
                        };
                        let out = (db + gr + makeup).clamp(-48.0, 0.0);
                        (
                            xf + i as f32 / 48.0 * wf,
                            yf + hf - (out + 48.0) / 48.0 * hf,
                        )
                    })
                    .collect()
            }
            T::Delay => {
                // Échos successifs sur 2 s : hauteur = réinjection^n.
                let step = fx.p1.clamp(10.0, 2000.0) / 2000.0 * wf;
                let mut amp = 1.0f32;
                let mut k = 0;
                while k < 24 && amp > 0.02 {
                    let ex = xf + step * k as f32;
                    if ex > xf + wf {
                        break;
                    }
                    let side = if fx.p4 > 0.5 && k % 2 == 1 {
                        p.cyan
                    } else {
                        color
                    };
                    let bh = amp * hf;
                    screen_vector::fill_round_rect(frame, ex, yf + hf - bh, 4.0, bh, 2.0, side);
                    amp *= fx.p2.clamp(0.0, 0.95);
                    k += 1;
                }
                Vec::new()
            }
            T::Reverb => {
                // Décroissance (taille → durée), pré-délai décalé à gauche.
                let decay_s = 0.4 + 3.6 * fx.p1.clamp(0.0, 1.0);
                let pre = fx.p3.clamp(0.0, 100.0) / 1000.0;
                (0..=96)
                    .map(|i| {
                        let t = i as f32 / 96.0 * 4.0;
                        let a = if t < pre {
                            0.0
                        } else {
                            (-(t - pre) * 6.9 / decay_s).exp()
                        };
                        (xf + i as f32 / 96.0 * wf, yf + hf - a * hf)
                    })
                    .collect()
            }
            T::Eq => {
                // Réponse des trois bandes (±18 dB), échelle log 20 Hz–20 kHz.
                let bands = crate::fx::eq_bands(fx, 48_000.0);
                fill_rect(frame, x + 8, y + h / 2, w - 16, 1, p.track);
                (0..=96)
                    .map(|i| {
                        let f = 20.0 * 1000f32.powf(i as f32 / 96.0);
                        let g: f32 = bands.iter().map(|b| b.magnitude(f, 48_000.0)).product();
                        let db = (20.0 * g.max(1e-4).log10()).clamp(-18.0, 18.0);
                        (
                            xf + i as f32 / 96.0 * wf,
                            yf + hf / 2.0 - db / 18.0 * hf / 2.0,
                        )
                    })
                    .collect()
            }
            T::Chorus => {
                // Retard modulé des deux canaux (quadrature) sur deux cycles.
                let base = fx.p3.clamp(0.5, 30.0);
                let depth = fx.p2.clamp(0.0, 1.0) * 6.0;
                let curve = |offset: f32| -> Vec<(f32, f32)> {
                    (0..=96)
                        .map(|i| {
                            let t = i as f32 / 96.0 * 2.0;
                            let lfo = (2.0 * std::f32::consts::PI * (t + offset)).sin();
                            let ms = base + depth * 0.5 * (1.0 + lfo);
                            (xf + i as f32 / 96.0 * wf, yf + hf - ms / 36.0 * hf)
                        })
                        .collect()
                };
                if let Some(path) = screen_vector::polyline(&curve(0.25)) {
                    screen_vector::stroke(frame, &path, 2.0, Paint::Solid(p.cyan, 0.9));
                }
                curve(0.0)
            }
            T::LoFi => {
                // Une période de sinus échantillonnée-bloquée et quantifiée.
                let levels = 2f32.powf(fx.p1.clamp(2.0, 16.0).round() - 1.0);
                let steps = (fx.p2.clamp(500.0, 48_000.0) / 1000.0).clamp(4.0, 48.0) as usize;
                fill_rect(frame, x + 8, y + h / 2, w - 16, 1, p.track);
                let mut pts = Vec::new();
                for k in 0..steps {
                    let t = k as f32 / steps as f32;
                    let v = ((2.0 * std::f32::consts::PI * t).sin() * levels).round() / levels;
                    let py = yf + hf / 2.0 - v * hf * 0.45;
                    pts.push((xf + t * wf, py));
                    pts.push((xf + (k + 1) as f32 / steps as f32 * wf, py));
                }
                pts
            }
            T::Filter => Vec::new(),
        };
        if fx.fx_type == T::Reverb {
            if let Some(path) = screen_vector::area(&curve, yf + hf) {
                let fade = Paint::Fade {
                    color,
                    top: 0.45,
                    bottom: 0.05,
                    y0: yf,
                    y1: yf + hf,
                };
                screen_vector::fill(frame, &path, fade);
            }
        }
        if fx.fx_type == T::Compressor {
            let diag = [(xf, yf + hf), (xf + wf, yf)];
            if let Some(path) = screen_vector::polyline(&diag) {
                screen_vector::stroke(frame, &path, 1.0, Paint::Solid(p.border, 1.0));
            }
        }
        if let Some(path) = screen_vector::polyline(&curve) {
            screen_vector::stroke(frame, &path, 2.0, Paint::Solid(color, 1.0));
        }
    }
    if !fx.enabled {
        // Contourné : voile et mention explicite.
        if let Some(path) = screen_vector::round_rect(x as f32, y as f32, w as f32, h as f32, 4.0) {
            screen_vector::fill(frame, &path, Paint::Solid(p.bg, 0.6));
        }
        let msg = "BYPASS";
        let mw = screen_text::width(msg, screen_text::Face::Bold, 22.0);
        bold(
            frame,
            (x + w / 2) as f32 - mw / 2.0,
            (y + h / 2 + 8) as f32,
            msg,
            22.0,
            p.white,
        );
    }
}

/// Grand titre et ligne d'info de l'instrument (preset, forme d'onde…).
fn instrument_caption(snap: &EngineSnapshot, inst: Instrument) -> (String, String) {
    match inst {
        Instrument::Kick => {
            let kp = snap.focus_sound_kick.clone().unwrap_or_default();
            let presets = crate::kick_synth::KickParams::all_presets();
            let idx = presets.iter().position(|p| p.preset_name == kp.preset_name);
            let pos = idx.map_or("--".to_string(), |i| format!("{}/{}", i + 1, presets.len()));
            (
                kp.preset_name.clone(),
                format!("PRESET {pos}  ·  {:?}", kp.waveform),
            )
        }
        Instrument::Wavetable => {
            let wp = snap.focus_sound_wavetable.clone().unwrap_or_default();
            (
                wp.name.clone(),
                format!(
                    "{:?}  ·  OSC {:?} + {:?}",
                    wp.category, wp.osc1_wave, wp.osc2_wave
                ),
            )
        }
        Instrument::Acid => {
            let ap = &snap.acid_params;
            (
                "TB-303".to_string(),
                format!(
                    "{:?}  ·  OCT {:+}  ·  TUNE {:+.1}",
                    ap.waveform, ap.octave, ap.tuning_semitones
                ),
            )
        }
        Instrument::Sampler => {
            let snd = snap.active_sound as usize;
            let file = focus_sound(snap)
                .and_then(|s| s.zones.first())
                .and_then(|z| z.sample_path.as_deref())
                .and_then(|f| f.rsplit('/').next())
                .unwrap_or("synthèse 808")
                .to_string();
            (
                sound_name(snap, snd),
                format!("{file}  ·  {:.2} s", snap.sample_duration),
            )
        }
    }
}

/// Visuel propre à l'instrument (gauche) : forme du sample, chute de pitch
/// du kick, réponse du filtre 303, cycle des oscillateurs wavetable.
fn draw_instrument_visual(
    frame: &mut DisplayFrame,
    p: &Pal,
    snap: &EngineSnapshot,
    inst: Instrument,
    (x, y, w, h): (usize, usize, usize, usize),
    tint: DisplayPixel,
) {
    match inst {
        Instrument::Sampler => match snap.waveform.as_deref() {
            Some(buckets) => draw_waveform(frame, p, x, y, w, h, buckets),
            None => {
                fill_rect(frame, x, y, w, h, p.panel);
                label(
                    frame,
                    x as f32 + 10.0,
                    (y + h / 2 + 5) as f32,
                    "Son de synthèse : pas de forme d'onde",
                    13.0,
                    p.muted,
                    1.4,
                );
            }
        },
        Instrument::Kick => {
            let kp = snap.focus_sound_kick.clone().unwrap_or_default();
            fill_rect(frame, x, y, w, h, p.panel);
            let total = (kp.hold_ms + kp.decay_ms * 1.2).max(120.0);
            let mut prev = None;
            for i in 0..w {
                let t = i as f32 / w as f32 * total;
                let amp = if t < kp.hold_ms {
                    1.0
                } else {
                    (-(t - kp.hold_ms) / (kp.decay_ms * 0.35).max(1.0)).exp()
                };
                let ah = (amp * (h - 8) as f32) as usize;
                fill_rect(
                    frame,
                    x + i,
                    y + h - 4 - ah,
                    1,
                    ah,
                    mix(p.panel, tint, 0.45),
                );
                let pitch = kp.snap.clamp(0.0, 1.0) * (-t / kp.sweep_time_ms.max(1.0)).exp();
                let py = (y + h - 6) as i32 - (pitch * 0.8 * (h - 10) as f32) as i32;
                if let Some((px, qy)) = prev {
                    line(frame, px, qy, (x + i) as i32, py, p.white);
                }
                prev = Some(((x + i) as i32, py));
            }
            label(
                frame,
                x as f32 + 8.0,
                y as f32 + 16.0,
                "PITCH",
                12.0,
                p.white,
                1.5,
            );
            label(
                frame,
                x as f32 + 8.0,
                (y + h) as f32 - 8.0,
                "AMP",
                12.0,
                tint,
                1.5,
            );
        }
        Instrument::Acid => {
            let ap = &snap.acid_params;
            draw_filter_curve(
                frame,
                p,
                (x, y, w, h),
                ap.cutoff_hz,
                ap.resonance,
                "LP",
                tint,
            );
            let peak = (ap.cutoff_hz * (1.0 + ap.env_mod * 4.0)).min(18000.0);
            let fx = x + log_x(peak, w);
            fill_rect(frame, fx, y + 4, 1, h - 8, mix(p.panel, p.white, 0.45));
            label(
                frame,
                x as f32 + 8.0,
                y as f32 + 16.0,
                "FILTRE + ENV MOD",
                12.0,
                p.dim,
                1.4,
            );
        }
        Instrument::Wavetable => {
            let wp = snap.focus_sound_wavetable.clone().unwrap_or_default();
            fill_rect(frame, x, y, w, h, p.panel);
            fill_rect(frame, x, y + h / 2, w, 1, p.track);
            let waves = [
                (Some(wp.osc1_wave), tint),
                (wp.osc2_enabled.then_some(wp.osc2_wave), p.cyan),
            ];
            for (wave, color) in waves {
                let Some(wave) = wave else { continue };
                let mut prev = None;
                for i in 0..w {
                    let v = wave_shape(wave, i as f32 / w as f32 * 2.0);
                    let py = (y + h / 2) as i32 - (v * (h / 2 - 6) as f32) as i32;
                    if let Some((px, qy)) = prev {
                        line(frame, px, qy, (x + i) as i32, py, color);
                    }
                    prev = Some(((x + i) as i32, py));
                }
            }
            label(
                frame,
                x as f32 + 8.0,
                y as f32 + 16.0,
                "OSC 1",
                12.0,
                tint,
                1.5,
            );
            if wp.osc2_enabled {
                label(
                    frame,
                    x as f32 + 56.0,
                    y as f32 + 16.0,
                    "OSC 2",
                    12.0,
                    p.cyan,
                    1.5,
                );
            }
        }
    }
}

/// Visuel de la page (droite) : enveloppe, courbe de filtre, ou les 8
/// paramètres en barres quand la page n'a pas de courbe.
fn draw_page_visual(
    frame: &mut DisplayFrame,
    p: &Pal,
    v: &PageView,
    (x, y, w, h): (usize, usize, usize, usize),
    tint: DisplayPixel,
) {
    let raw = |name: &str| {
        v.params
            .iter()
            .flatten()
            .find(|pv| pv.name == name)
            .map(|pv| pv.raw)
    };
    let text = |name: &str| {
        v.params
            .iter()
            .flatten()
            .find(|pv| pv.name == name)
            .map(|pv| pv.value.as_str())
    };
    if let (Some(a), Some(d), Some(s), Some(r)) =
        (raw("ATTACK"), raw("DECAY"), raw("SUSTAIN"), raw("RELEASE"))
    {
        let adsr = Adsr {
            attack_ms: a,
            decay_ms: d,
            sustain: s,
            release_ms: r,
        };
        draw_adsr(frame, p, x, y, w, h, &adsr);
    } else if let (Some(cut), Some(reso)) = (raw("CUTOFF"), raw("RESO")) {
        let kind = text("FILTRE").unwrap_or("LP");
        draw_filter_curve(frame, p, (x, y, w, h), cut, reso, kind, tint);
    } else {
        fill_rect(frame, x, y, w, h, p.panel);
        let cw = w / 8;
        for (i, pv) in v.params.iter().enumerate() {
            let cx = x + i * cw;
            if i > 0 {
                fill_rect(frame, cx, y + 8, 1, h - 16, p.border);
            }
            let Some(pv) = pv else { continue };
            let color = if i < 4 { tint } else { p.cyan };
            let bh = h - 44;
            let fh = (pv.norm.clamp(0.0, 1.0) * bh as f32) as usize;
            fill_rect(frame, cx + cw / 2 - 7, y + 10, 14, bh, p.track);
            fill_rect(frame, cx + cw / 2 - 7, y + 10 + bh - fh, 14, fh, color);
            let name = fit_text(pv.name, screen_text::Face::Sans, 11.0, (cw - 4) as f32);
            let nw = screen_text::width(&name, screen_text::Face::Sans, 11.0);
            label(
                frame,
                (cx + cw / 2) as f32 - nw / 2.0,
                (y + h - 18) as f32,
                &name,
                11.0,
                p.dim,
                1.4,
            );
            let val = fit_text(&pv.value, screen_text::Face::Sans, 11.0, (cw - 4) as f32);
            let vw = screen_text::width(&val, screen_text::Face::Sans, 11.0);
            label(
                frame,
                (cx + cw / 2) as f32 - vw / 2.0,
                (y + h - 5) as f32,
                &val,
                11.0,
                p.white,
                1.4,
            );
        }
    }
}

/// Abscisse (0..w) d'une fréquence sur une échelle log 20 Hz – 20 kHz.
fn log_x(hz: f32, w: usize) -> usize {
    let t = (hz.clamp(20.0, 20000.0) / 20.0).log10() / 3.0;
    ((t * (w - 1) as f32) as usize).min(w - 1)
}

/// Réponse en fréquence d'un filtre résonant (LP/HP/BP/NOTCH/PEAK/OFF).
fn draw_filter_curve(
    frame: &mut DisplayFrame,
    p: &Pal,
    (x, y, w, h): (usize, usize, usize, usize),
    cutoff: f32,
    reso: f32,
    kind: &str,
    color: DisplayPixel,
) {
    use screen_vector::Paint;
    panel(frame, p, x, y, w, h);
    // Repères : 100 Hz, 1 kHz, 10 kHz.
    for f in [100.0f32, 1000.0, 10000.0] {
        fill_rect(frame, x + log_x(f, w), y + 6, 1, h - 12, p.track);
    }
    let q = 0.7 + reso.clamp(0.0, 1.0) * 8.0;
    let fc = cutoff.max(10.0);
    let gain = |f: f32| {
        let r = f / fc;
        let lp = 1.0 / ((1.0 - r * r).powi(2) + (r / q).powi(2)).sqrt();
        match kind {
            "OFF" => 1.0,
            "HP" => lp * r * r,
            "BP" => lp * r / q,
            "NOTCH" => lp * (1.0 - r * r).abs(),
            "PEAK" => 1.0 + lp * r / q,
            _ => lp,
        }
    };
    let zero = (y + h * 2 / 5) as f32;
    fill_rect(frame, x + 4, zero as usize, w - 8, 1, p.border);
    let (top, bottom) = (y as f32 + 4.0, (y + h) as f32 - 4.0);
    let pts: Vec<(f32, f32)> = (0..w)
        .step_by(2)
        .map(|i| {
            let f = 20.0 * 1000f32.powf(i as f32 / (w - 1) as f32);
            let db = (20.0 * gain(f).max(1e-4).log10()).clamp(-36.0, 18.0);
            let py = (zero - db / 36.0 * (h as f32 * 0.6)).clamp(top, bottom);
            ((x + i) as f32 + 0.5, py)
        })
        .collect();
    if let Some(path) = screen_vector::area(&pts, bottom) {
        let fade = Paint::Fade {
            color,
            top: 0.38,
            bottom: 0.04,
            y0: top,
            y1: bottom,
        };
        screen_vector::fill(frame, &path, fade);
    }
    if let Some(path) = screen_vector::polyline(&pts) {
        screen_vector::stroke(frame, &path, 2.0, Paint::Solid(color, 1.0));
    }
    let cx = (x + log_x(fc, w)) as f32;
    screen_vector::fill_round_rect(
        frame,
        cx - 1.0,
        top + 2.0,
        2.0,
        bottom - top - 4.0,
        1.0,
        p.white,
    );
}

/// Forme d'un cycle d'oscillateur wavetable (aperçu, `t` en cycles).
fn wave_shape(wave: crate::wavetable_synth::WavetableWaveform, t: f32) -> f32 {
    use crate::wavetable_synth::WavetableWaveform as W;
    use std::f32::consts::TAU;
    let ph = t.fract();
    let sin = |k: f32| (TAU * k * t).sin();
    match wave {
        W::Sine => sin(1.0),
        W::Triangle => 1.0 - 4.0 * (ph - 0.5).abs(),
        W::Saw => 2.0 * ph - 1.0,
        W::Square => {
            if ph < 0.5 {
                0.8
            } else {
                -0.8
            }
        }
        W::Supersaw => 0.5 * ((2.0 * ph - 1.0) + (2.0 * (t * 1.01).fract() - 1.0)),
        W::GrandPiano | W::M1Piano | W::RhodesTines => {
            0.6 * sin(1.0) + 0.3 * sin(2.0) + 0.1 * sin(3.0)
        }
        W::DigiLead => {
            if ph < 0.25 {
                0.8
            } else {
                -0.8
            }
        }
        W::TonewheelOrgan => 0.45 * sin(1.0) + 0.3 * sin(2.0) + 0.2 * sin(4.0),
        W::VocalFormant => sin(1.0) * (0.6 + 0.4 * sin(3.0)),
        W::Noise => ((t * 9973.0).sin() * 43758.547).fract() * 1.6 - 0.8,
    }
}

/// Rend la paire d'écrans avec l'état ampli guitare éventuel.
#[must_use]
pub fn render_screens_with_guitar(
    snap: &EngineSnapshot,
    guitar: Option<&GuitarScreenData>,
) -> (DisplayFrame, DisplayFrame) {
    let fallback = GuitarScreenData::default();
    let g = guitar.unwrap_or(&fallback);
    let left = if snap.right_tab == 1 {
        render_events_left(snap)
    } else if snap.song_mode {
        render_arranger_left(snap)
    } else if snap.scene_mode && snap.left_tab == 0 {
        render_scenes_left(snap)
    } else if snap.pattern_mode && snap.left_tab == 0 {
        render_pattern_left(snap)
    } else {
        match snap.left_tab {
            1 => render_mixer_left(snap),
            2 => render_sampler_left(snap),
            3 if snap.browser_kind == 1 => render_project_browser_left(snap),
            3 => render_browser_left(snap),
            4 if snap.plugin_page == 2 => render_guitar_amp_left(snap, g),
            4 => render_plugin_left(snap),
            _ => {
                if snap.controller_mode == 3 {
                    render_step_left(snap)
                } else if snap.controller_mode == 1 {
                    render_keyboard_left(snap)
                } else if snap.controller_mode == 2 {
                    render_chords_left(snap)
                } else {
                    render_daw_left(snap)
                }
            }
        }
    };
    let right = if snap.right_tab == 1 {
        render_events_right(snap)
    } else if snap.song_mode {
        render_arranger_right(snap)
    } else if snap.scene_mode && snap.left_tab == 0 && snap.right_tab == 0 {
        render_scenes_right(snap)
    } else if snap.left_tab == 4 && snap.right_tab == 0 && snap.plugin_page == 2 {
        render_guitar_amp_right(snap, g)
    } else if snap.left_tab == 4 && snap.right_tab == 0 {
        render_plugin_right(snap)
    } else if snap.left_tab == 2 && snap.right_tab == 0 {
        render_sampler_right(snap)
    } else if snap.left_tab == 3 && snap.right_tab == 0 {
        if snap.browser_kind == 1 {
            render_project_browser_right(snap)
        } else {
            render_browser_right(snap)
        }
    } else if snap.left_tab == 1 && snap.right_tab == 0 {
        render_mixer_right(snap)
    } else if snap.left_tab == 0 && snap.right_tab == 0 && snap.controller_mode == 3 {
        render_step_right(snap)
    } else if snap.left_tab == 0 && snap.right_tab == 0 && snap.controller_mode == 1 {
        render_keyboard_right(snap)
    } else if snap.left_tab == 0 && snap.right_tab == 0 && snap.controller_mode == 2 {
        render_chords_right(snap)
    } else {
        match snap.right_tab {
            1 => render_events_right(snap),
            2 => render_macro_right(snap),
            3 => render_settings_right(snap),
            _ => render_daw_right(snap),
        }
    };
    (left, right)
}

/// Rend la paire d'écrans (gauche, droite) selon la vue/onglets courants.
/// Partagée par le thread d'affichage USB et le miroir GUI (mêmes écrans).
#[must_use]
pub fn render_screens(snap: &EngineSnapshot) -> (DisplayFrame, DisplayFrame) {
    render_screens_with_guitar(snap, None)
}

/// Écrans de démarrage : même mot-symbole que la veille, l'étape en cours
/// et une barre de progression — la MK3 réagit dès le lancement au lieu de
/// rester figée sur le logo du firmware pendant le chargement.
#[must_use]
pub fn render_boot_screens(step: &str, progress: f32) -> (DisplayFrame, DisplayFrame) {
    let (mut left, mut right) = render_shutdown_screens();
    let accent = DisplayPixel::from_rgb(255, 154, 31);
    let track = DisplayPixel::from_rgb(36, 39, 46);
    let dim = DisplayPixel::from_rgb(140, 146, 158);
    // Barre de progression à cheval sur les deux écrans, sous le mot-symbole.
    let y = SCR_H as f32 / 2.0 + 44.0;
    let total = 2.0 * SCR_W as f32 - 2.0 * 120.0;
    let filled = total * progress.clamp(0.0, 1.0);
    for (frame, x0) in [(&mut left, 120.0f32), (&mut right, -(SCR_W as f32) + 120.0)] {
        screen_vector::fill_round_rect(
            frame,
            x0.max(0.0),
            y,
            (total + x0.min(0.0)).min(SCR_W as f32 - x0.max(0.0)),
            4.0,
            2.0,
            track,
        );
        let w = (filled + x0.min(0.0)).clamp(0.0, SCR_W as f32 - x0.max(0.0));
        if w > 0.5 {
            screen_vector::fill_round_rect(frame, x0.max(0.0), y, w, 4.0, 2.0, accent);
        }
    }
    let step = fit_text(step, screen_text::Face::Sans, 15.0, SCR_W as f32 - 140.0);
    label(&mut left, 120.0, y + 30.0, &step, 15.0, dim, 1.0);
    (left, right)
}

/// Écrans de veille, envoyés à la fermeture (et en cas de plantage) avant
/// de libérer l'interface USB : presque noirs, un seul mot-symbole sobre à
/// cheval sur les deux écrans (« MASCHINE » | « MK3 »), faiblement éclairé —
/// rien ne reste figé ni ne brille inutilement quand l'app est fermée.
#[must_use]
pub fn render_shutdown_screens() -> (DisplayFrame, DisplayFrame) {
    let ink = DisplayPixel::from_rgb(78, 78, 84);
    let glow = DisplayPixel::from_rgb(120, 70, 18);
    let black = DisplayPixel::from_rgb(0, 0, 0);
    let px = 30.0;
    let baseline = SCR_H as f32 / 2.0 + 10.0;
    let mut left = DisplayFrame::solid(DisplayId::Left, black);
    let word = "MASCHINE";
    let w = screen_text::width(word, screen_text::Face::Sans, px);
    // Le mot finit près du bord intérieur : il se lit d'un écran à l'autre.
    let x = SCR_W as f32 - 34.0 - w;
    label(&mut left, x, baseline, word, px, ink, 1.3);
    let mut right = DisplayFrame::solid(DisplayId::Right, black);
    label(&mut right, 34.0, baseline, "MK3", px, ink, 1.3);
    fill_rect(&mut right, 36, baseline as usize + 12, 26, 2, glow);
    label(
        &mut right,
        36.0,
        baseline + 34.0,
        "linux",
        13.0,
        DisplayPixel::from_rgb(52, 52, 58),
        1.2,
    );
    (left, right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Engine, EngineEvent};

    fn check_pair(frame: &DisplayFrame) {
        assert_eq!(frame.w as usize, SCR_W);
        assert_eq!(frame.h as usize, SCR_H);
        assert_eq!(frame.pixels.len(), SCR_W * SCR_H);
        // Au moins le bandeau doit avoir été dessiné (pas un écran vide).
        assert!(frame.pixels.iter().any(|px| *px != Pal::new().bg));
    }

    fn guitar_snapshot() -> EngineSnapshot {
        let mut eng = Engine::new_demo(48000);
        let _ = eng.handle_event(EngineEvent::SetLeftTab(4));
        let _ = eng.handle_event(EngineEvent::SetPluginPage(2));
        eng.snapshot()
    }

    fn guitar_data() -> GuitarScreenData {
        let mut g = GuitarScreenData {
            model: 3,
            cab: 2,
            bypass: true,
            monitor: false,
            input_db: -12.0,
            output_db: -3.0,
            cpu_load: 0.42,
            latency_ms: 7.5,
            xruns: 2,
            tuner_label: Some("A4 +3c".to_string()),
            tuner_freq: 440.0,
            ..GuitarScreenData::default()
        };
        for (i, v) in [0.9, 0.2, 0.6, 0.4, 0.7, 0.3, 0.8, 0.5]
            .into_iter()
            .enumerate()
        {
            g.params[i] = v;
        }
        g.spectrum[12] = -6.0;
        g
    }

    #[test]
    fn guitar_amp_page_renders_on_both_screens() {
        let snap = guitar_snapshot();
        assert_eq!(snap.plugin_page, 2);
        let data = guitar_data();
        let (left, right) = render_screens_with_guitar(&snap, Some(&data));
        check_pair(&left);
        check_pair(&right);
        assert_eq!(left.pixels, render_guitar_amp_left(&snap, &data).pixels);
        assert_eq!(right.pixels, render_guitar_amp_right(&snap, &data).pixels);

        // La page 0 doit afficher le sampler, pas l'ampli.
        let mut other = snap.clone();
        other.plugin_page = 0;
        let (sampler_left, _) = render_screens_with_guitar(&other, Some(&data));
        assert_ne!(left.pixels, sampler_left.pixels);
    }

    #[test]
    fn guitar_screen_reflects_control_values() {
        let snap = guitar_snapshot();
        let mut data = guitar_data();
        let before = render_guitar_amp_left(&snap, &data);
        data.params[0] = 0.05;
        data.params[1] = 0.95;
        let after = render_guitar_amp_left(&snap, &data);
        assert_ne!(
            before.pixels, after.pixels,
            "les valeurs des knobs doivent être dessinées"
        );

        let before_right = render_guitar_amp_right(&snap, &data);
        data.input_db = -3.0;
        data.cpu_load = 0.95;
        let after_right = render_guitar_amp_right(&snap, &data);
        assert_ne!(
            before_right.pixels, after_right.pixels,
            "VU et CPU doivent être dessinés"
        );
    }

    #[test]
    fn guitar_soft_labels_match_the_app_actions() {
        assert_eq!(GUITAR_SOFT_LABELS[0], "AMP <");
        assert_eq!(GUITAR_SOFT_LABELS[1], "AMP >");
        assert_eq!(GUITAR_SOFT_LABELS[4], "ACTIF");
        assert_eq!(GUITAR_SOFT_LABELS[5], "MONITOR");
        assert_eq!(GUITAR_SOFT_LABELS[6], "BYPASS");
        assert_eq!(GUITAR_SOFT_LABELS[7], "INPUT+");
        let eng = Engine::new_demo(48000);
        let mut eng = eng;
        let _ = eng.handle_event(EngineEvent::SetLeftTab(4));
        let _ = eng.handle_event(EngineEvent::SetPluginPage(2));
        assert_eq!(eng.soft_key_labels(), GUITAR_SOFT_LABELS);
    }

    /// Toutes les vues des 2 écrans doivent se rendre sans panic.
    #[test]
    fn all_screens_render_without_panic() {
        let mut eng = Engine::new_demo(48000);
        eng.advance(4800);
        {
            let snap = eng.snapshot();
            let (l, r) = render_screens(&snap);
            check_pair(&l);
            check_pair(&r);
        }
        // Toutes les combinaisons d'onglets (0..4).
        for left_tab in 0..5u8 {
            for right_tab in 0..4u8 {
                let mut snap = eng.snapshot();
                snap.left_tab = left_tab;
                snap.right_tab = right_tab;
                snap.controller_mode = if left_tab == 0 { 3 } else { 0 };
                let (l, r) = render_screens(&snap);
                check_pair(&l);
                check_pair(&r);
            }
        }
        // Song, scènes, waveform, niveaux extrêmes (peak > 1.0).
        let mut snap = eng.snapshot();
        snap.song_mode = true;
        let (l, r) = render_screens(&snap);
        check_pair(&l);
        check_pair(&r);
        snap.song_mode = false;
        snap.scene_mode = true;
        snap.left_tab = 0;
        let (l, r) = render_screens(&snap);
        check_pair(&l);
        check_pair(&r);
        snap.scene_mode = false;
        snap.left_tab = 2;
        snap.right_tab = 0;
        snap.waveform = Some(
            (0..120)
                .map(|i| {
                    let v = (i as f32 * 0.21).sin() * 0.9;
                    (-v.abs(), v.abs())
                })
                .collect(),
        );
        snap.sample_duration = 0.53;
        snap.group_levels = [1.0; 8];
        snap.group_peaks = [1.2; 8];
        snap.master_level = 1.0;
        snap.master_peak = 1.5;
        let (l, r) = render_screens(&snap);
        check_pair(&l);
        check_pair(&r);
    }

    #[test]
    fn pattern_mode_uses_slot_screen_and_shutdown_splash_is_clean() {
        let eng = Engine::new_demo(48000);
        let mut snap = eng.snapshot();
        snap.pattern_mode = true;
        snap.left_tab = 0;
        snap.right_tab = 0;
        snap.controller_mode = 0;
        let (left, right) = render_screens(&snap);
        check_pair(&left);
        check_pair(&right);
        assert_eq!(left.pixels, render_pattern_left(&snap).pixels);

        let (left, right) = render_shutdown_screens();
        check_pair(&left);
        check_pair(&right);
        // Veille : un mot-symbole discret, l'écran reste presque entièrement noir.
        let black = DisplayPixel::from_rgb(0, 0, 0);
        let lit = left.pixels.iter().filter(|px| **px != black).count();
        assert!(lit > 0, "le mot-symbole est dessiné");
        assert!(
            lit * 20 < left.pixels.len(),
            "moins de 5 % de pixels allumés"
        );
        assert_eq!(left.pixels.len(), right.pixels.len());
    }

    #[test]
    fn sampler_page_restores_sampler_on_right_after_events_overlay() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(crate::engine::EngineEvent::SetRightTab(1))
            .unwrap();
        let snap = eng.snapshot();
        let (_, events) = render_screens(&snap);
        assert_eq!(events.pixels, render_events_right(&snap).pixels);

        eng.handle_event(crate::engine::EngineEvent::SetLeftTab(2))
            .unwrap();
        let snap = eng.snapshot();
        let (_, right) = render_screens(&snap);
        assert_eq!(right.pixels, render_sampler_right(&snap).pixels);
    }

    /// Boutons 1–8 au-dessus des écrans, knobs en dessous (comme la MK3) :
    /// libellés de boutons en haut, slots d'encodeurs en bas.
    #[test]
    fn button_labels_on_top_and_knob_slots_at_bottom() {
        let snap = Engine::new_demo(48000).snapshot();
        let p = Pal::of(&snap);
        let (left, _) = render_screens(&snap);
        assert_eq!(snap.selected_bar, 0, "BAR 1 actif sur le bouton 1");
        // Pastille du bouton 1 actif (couleur du groupe), sous le bouton physique.
        assert_eq!(left.pixels[10 * SCR_W + 8], p.accent);
        // Piste de la barre de valeur du knob 1, juste au-dessus du knob.
        assert_eq!(left.pixels[263 * SCR_W + 113], p.track);
        // L'ancien emplacement du bouton 1, en bas, n'est plus une soft key.
        assert_ne!(left.pixels[(SCR_H - 12) * SCR_W + 8], p.accent);
    }

    #[test]
    fn accent_follows_the_focused_group_colour() {
        let mut eng = Engine::new_demo(48000);
        let snap = eng.snapshot();
        let a = Pal::of(&snap).accent;
        // Premier groupe d'une autre couleur que A dans la démo.
        let other = (1..8u8)
            .find(|&g| snap.project.groups[g as usize].color != snap.project.groups[0].color)
            .expect("la démo a des groupes de couleurs différentes");
        eng.handle_event(crate::EngineEvent::GroupFocus(other))
            .unwrap();
        let d = Pal::of(&eng.snapshot()).accent;
        assert_ne!(a, d, "l'accent suit la couleur du groupe");
        // Texte lisible sur l'accent : noir sur clair, blanc sur sombre.
        for c in [a, d] {
            let ink = ink_for(c);
            let black = DisplayPixel::from_rgb(0, 0, 0);
            assert!(ink == black || ink == DisplayPixel::from_rgb(245, 246, 248));
        }
    }

    /// Rendu déterministe : un même snapshot donne des pixels identiques.
    #[test]
    fn render_is_deterministic() {
        let mut eng = Engine::new_demo(48000);
        eng.advance(4800);
        let snap = eng.snapshot();
        let (l1, r1) = render_screens(&snap);
        let (l2, r2) = render_screens(&snap);
        assert_eq!(l1.pixels, l2.pixels);
        assert_eq!(r1.pixels, r2.pixels);
    }

    /// Les 16 vues ne sont pas toutes identiques (sanity : le dispatch fonctionne).
    #[test]
    fn views_are_not_all_identical() {
        let mut eng = Engine::new_demo(48000);
        eng.advance(4800);
        let mut hashes = std::collections::BTreeSet::new();
        for left_tab in 0..5u8 {
            for right_tab in 0..4u8 {
                let mut snap = eng.snapshot();
                snap.left_tab = left_tab;
                snap.right_tab = right_tab;
                snap.controller_mode = if left_tab == 0 { 3 } else { 0 };
                let (l, _r) = render_screens(&snap);
                // FNV-1a simplifié sur les premiers pixels (pour limiter le coût).
                let mut h = 0u64;
                for px in &l.pixels {
                    h ^= px.0 as u64;
                    h = h.wrapping_mul(0x9e3779b97f4a7c15);
                }
                hashes.insert(h);
            }
        }
        // Au moins 4 rendus gauches distincts.
        assert!(hashes.len() >= 4);
    }

    /// L'activité pad décroît et bump la révision.
    #[test]
    fn pad_activity_decays_and_bumps_revision() {
        let mut eng = Engine::new_demo(48000);
        let r0 = eng.snapshot().revision;
        eng.note_pad_activity(0, 100);
        let s1 = eng.snapshot();
        assert!(s1.pad_activity[0] > 0.5);
        // La décroissance (advance) bump la révision.
        eng.advance(1);
        let s2 = eng.snapshot();
        assert!(s2.revision > r0);
        // Avance ~300 ms → décroissance notable.
        eng.advance(14400);
        let s3 = eng.snapshot();
        assert!(s3.pad_activity[0] < s1.pad_activity[0]);
    }
}
