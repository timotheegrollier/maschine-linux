//! Thème visuel "Maschine MK3 Studio" — proche du logiciel Maschine et des
//! écrans MK3 : gris graphite neutres et plats, la couleur vient des groupes
//! et des sons ; un seul orange d'accent, partagé avec les écrans du
//! contrôleur (`display_render::Pal`), et un cyan pour les infos.

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
            bg: egui::Color32::from_rgb(0x17, 0x17, 0x19),
            panel: egui::Color32::from_rgb(0x1E, 0x1E, 0x21),
            surface: egui::Color32::from_rgb(0x26, 0x26, 0x2A),
            widget_bg: egui::Color32::from_rgb(0x30, 0x30, 0x35),
            border: egui::Color32::from_rgb(0x38, 0x38, 0x3E),
            border_active: egui::Color32::from_rgb(0x4C, 0x4C, 0x54),
            border_hover: egui::Color32::from_rgb(0xFF, 0x9A, 0x1F),
            text: egui::Color32::from_rgb(0xE9, 0xE9, 0xEC),
            text_dim: egui::Color32::from_rgb(0xA2, 0xA2, 0xAB),
            text_muted: egui::Color32::from_rgb(0x6C, 0x6C, 0x75),
            accent: egui::Color32::from_rgb(0xFF, 0x9A, 0x1F),
            accent_cyan: egui::Color32::from_rgb(0x26, 0xC6, 0xF5),
            success: egui::Color32::from_rgb(0x3C, 0xCF, 0x7E),
            error: egui::Color32::from_rgb(0xF0, 0x52, 0x4D),
            warning: egui::Color32::from_rgb(0xF2, 0xB2, 0x33),
            active_bg: egui::Color32::from_rgb(0xFF, 0x9A, 0x1F),
            flash: egui::Color32::from_rgb(0xFF, 0xE0, 0x66),
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
    visuals.menu_corner_radius = egui::CornerRadius::same(4);
    // Boutons plats et presque rectangulaires, comme le logiciel Maschine.
    for w in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        w.corner_radius = egui::CornerRadius::same(4);
        w.expansion = 0.0;
    }
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, c.border);
    visuals.widgets.open.bg_fill = c.border_active;
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
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(10))
}

/// Carte standard au contenu empilé verticalement, même posée dans une
/// rangée (`horizontal_wrapped`) : sinon ses widgets héritent du flux
/// horizontal et les libellés se replient lettre par lettre.
pub fn card<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    card_frame().show(ui, |ui| ui.vertical(add_contents).inner)
}

/// Rangée de cartes qui passe à la ligne, cartes alignées par le haut
/// (`horizontal_wrapped` les centre verticalement : décalages en escalier).
#[must_use]
pub fn card_row() -> egui::Layout {
    egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true)
}

/// Marges et bordure d'une carte : largeur intérieure = extérieure − ceci.
pub const CARD_CHROME: f32 = 22.0;

/// Largeur extérieure de cartes alignées qui remplissent `avail` : autant de
/// colonnes d'au moins `min_w` que possible (une au minimum).
#[must_use]
pub fn card_width(avail: f32, min_w: f32) -> f32 {
    let gap = 8.0;
    let cols = ((avail + gap) / (min_w + gap)).floor().max(1.0);
    ((avail - (cols - 1.0) * gap) / cols).floor()
}

/// Carte de largeur fixe (extérieure) qui passe à la ligne dans une rangée
/// `horizontal_wrapped` au lieu de déborder sous le panneau voisin.
pub fn card_sized<R>(
    ui: &mut egui::Ui,
    width: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.allocate_ui(egui::vec2(width, 0.0), |ui| {
        card(ui, |ui| {
            ui.set_width(width - CARD_CHROME);
            add_contents(ui)
        })
        .inner
    })
    .inner
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
            // L'aide ne passe jamais sous les commandes alignées à droite :
            // tronquée à la moitié de la place restante, entière au survol.
            let max = ui.available_width() * 0.5;
            ui.scope(|ui| {
                ui.set_max_width(max);
                ui.add(
                    egui::Label::new(egui::RichText::new(subtitle).size(11.5).color(c.text_dim))
                        .truncate(),
                )
                .on_hover_text(subtitle);
            });
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

/// Bandeau d'état d'un menu matériel latché (PATTERN, STEP, SCENE…).
pub fn mode_banner(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::NONE
        .fill(color.gamma_multiply(0.14))
        .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.75)))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(9, 6))
        .show(ui, |ui| {
            ui.colored_label(color, egui::RichText::new(text).strong().size(11.0));
        });
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
