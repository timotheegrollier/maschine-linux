//! Thème visuel "Maschine MK3 Studio" — inspiré des meilleurs DAW Linux
//! (Ableton Live, Bitwig, Renoise) et du hardware officiel Maschine MK3.
//!
//! Palette : fond charbon profond, surfaces légèrement relevées, bordures
//! discrètes, accent orange Maschine (#FF8C28) + cyan pour les sélections.

use eframe::egui;

/// Couleurs du thème.
pub struct MachColors {
    pub bg: egui::Color32,
    pub panel: egui::Color32,
    pub surface: egui::Color32,
    pub widget_bg: egui::Color32,
    pub border: egui::Color32,
    pub border_active: egui::Color32,
    pub border_hover: egui::Color32,
    pub text: egui::Color32,
    pub text_dim: egui::Color32,
    pub text_muted: egui::Color32,
    pub accent: egui::Color32,
    pub accent_cyan: egui::Color32,
    pub success: egui::Color32,
    pub error: egui::Color32,
    pub warning: egui::Color32,
    pub active_bg: egui::Color32,
    pub flash: egui::Color32,
}

impl MachColors {
    pub fn dark() -> Self {
        Self {
            bg: egui::Color32::from_rgb(0x0B, 0x0D, 0x14),
            panel: egui::Color32::from_rgb(0x13, 0x16, 0x20),
            surface: egui::Color32::from_rgb(0x1A, 0x1E, 0x2A),
            widget_bg: egui::Color32::from_rgb(0x21, 0x25, 0x32),
            border: egui::Color32::from_rgb(0x27, 0x2C, 0x3A),
            border_active: egui::Color32::from_rgb(0x3C, 0x42, 0x52),
            border_hover: egui::Color32::from_rgb(0xFF, 0x8C, 0x28),
            text: egui::Color32::from_rgb(0xE8, 0xEA, 0xEE),
            text_dim: egui::Color32::from_rgb(0x8B, 0x91, 0xA0),
            text_muted: egui::Color32::from_rgb(0x5A, 0x60, 0x70),
            accent: egui::Color32::from_rgb(0xFF, 0x8C, 0x28),
            accent_cyan: egui::Color32::from_rgb(0x00, 0xD4, 0xFF),
            success: egui::Color32::from_rgb(0x22, 0xC5, 0x5E),
            error: egui::Color32::from_rgb(0xEF, 0x44, 0x44),
            warning: egui::Color32::from_rgb(0xF5, 0x9E, 0x0B),
            active_bg: egui::Color32::from_rgb(0xFF, 0x8C, 0x28),
            flash: egui::Color32::from_rgb(0xFF, 0xD5, 0x4F),
        }
    }
}

/// Applique le thème visuel à un contexte egui.
pub fn apply_theme(ctx: &egui::Context) {
    let c = MachColors::dark();

    let mut visuals = egui::Visuals::dark();
    visuals.dark_mode = true;
    visuals.panel_fill = c.panel;
    visuals.window_fill = c.surface;
    visuals.extreme_bg_color = c.bg;
    visuals.faint_bg_color = c.surface;
    visuals.widgets.inactive.bg_fill = c.widget_bg;
    visuals.widgets.hovered.bg_fill = c.border_active;
    visuals.widgets.active.bg_fill = c.accent;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, c.border);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, c.border_hover);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, c.accent);
    visuals.selection.bg_fill = c.accent;
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, c.accent);
    visuals.override_text_color = Some(c.text);
    visuals.hyperlink_color = c.accent_cyan;
    visuals.code_bg_color = c.widget_bg;
    visuals.window_corner_radius = egui::CornerRadius::same(6);
    visuals.menu_corner_radius = egui::CornerRadius::same(6);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.menu_margin = egui::vec2(10.0, 10.0).into();
    style.text_styles = {
        use egui::TextStyle;
        [
            (
                TextStyle::Heading,
                egui::FontId::new(17.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Body,
                egui::FontId::new(13.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                egui::FontId::new(11.0, egui::FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                egui::FontId::new(12.0, egui::FontFamily::Monospace),
            ),
            (
                TextStyle::Button,
                egui::FontId::new(13.0, egui::FontFamily::Proportional),
            ),
        ]
        .into_iter()
        .collect::<std::collections::BTreeMap<egui::TextStyle, egui::FontId>>()
    };
    ctx.set_style(style);
}

/// Cadre "carte" standard (fond surface, bordure, coins arrondis).
pub fn card_frame() -> egui::Frame {
    let c = MachColors::dark();
    egui::Frame::NONE
        .fill(c.surface)
        .stroke(egui::Stroke::new(1.0_f32, c.border))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(8))
}

/// En-tête de vue : accent orange + titre.
pub fn view_header(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    let c = MachColors::dark();
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(4.0, 20.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(2), c.accent);
        ui.colored_label(c.text, egui::RichText::new(title).strong().size(15.0));
        if !subtitle.is_empty() {
            ui.colored_label(c.text_dim, egui::RichText::new(subtitle).size(11.5));
        }
    });
}

/// Dessin d'une section avec en-tête coloré.
pub fn section(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    let c = MachColors::dark();
    ui.colored_label(c.accent_cyan, egui::RichText::new(title).strong().small());
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(6.0);
    add_contents(ui);
}

/// Étiquette de valeur monospace (BPM, BBT, etc.).
pub fn value_label(ui: &mut egui::Ui, text: impl AsRef<str>, color: egui::Color32) {
    ui.label(
        egui::RichText::new(text.as_ref())
            .monospace()
            .color(color)
            .strong(),
    );
}

/// Étiquette petite atténuée.
pub fn caption(ui: &mut egui::Ui, text: impl AsRef<str>) {
    ui.colored_label(
        MachColors::dark().text_dim,
        egui::RichText::new(text.as_ref()).small(),
    );
}

/// dB → position 0..1 sur le fader (échelle -60..+12 dB, 0 dB ≈ 83 %).
#[must_use]
pub fn db_to_pos(db: f32) -> f32 {
    ((db.clamp(-60.0, 12.0) + 60.0) / 72.0).clamp(0.0, 1.0)
}

/// Position 0..1 → dB (inverse de `db_to_pos`).
#[must_use]
pub fn pos_to_db(pos: f32) -> f32 {
    (pos.clamp(0.0, 1.0) * 72.0) - 60.0
}

/// Niveau linéaire (0..1+) → dB.
#[must_use]
pub fn level_to_db(level: f32) -> f32 {
    20.0 * level.max(1.0e-6).log10()
}

/// Couleur d'un niveau en dB (vert → jaune → rouge).
#[must_use]
pub fn db_color(db: f32) -> egui::Color32 {
    let c = MachColors::dark();
    if db >= -0.5 {
        c.error
    } else if db >= -6.0 {
        c.warning
    } else {
        c.success
    }
}
