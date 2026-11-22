//! Widgets tactiles réutilisables : boutons, pads 4×4, potentiomètres,
//! faders dB, pan, VU-mètres réels, cellules de pas.
//!
//! Tous les widgets dessinent eux-mêmes (Painter) pour un rendu "hardware" :
//! retour visuel immédiat (survol, pression, actif), pas de latence de layout.

use eframe::egui;

use crate::gui::theme::{db_color, db_to_pos, level_to_db, pos_to_db, MachColors};

/// Résultat d'interaction d'un bouton simple.
pub struct Interaction {
    pub clicked: bool,
}

/// Bouton texte tactile (repos / survol / actif).
pub fn mach_button(ui: &mut egui::Ui, text: &str, active: bool) -> Interaction {
    let c = MachColors::dark();
    let mut btn =
        egui::Button::new(egui::RichText::new(text).strong()).min_size(egui::vec2(64.0, 32.0));
    if active {
        btn = btn
            .fill(c.active_bg)
            .stroke(egui::Stroke::new(1.5_f32, c.accent));
    } else {
        btn = btn
            .fill(c.widget_bg)
            .stroke(egui::Stroke::new(1.0_f32, c.border));
    }
    let resp = ui.add(btn);
    Interaction {
        clicked: resp.clicked(),
    }
}

/// Bouton iconique (petit, carré).
pub fn icon_button(
    ui: &mut egui::Ui,
    icon: &str,
    active: bool,
    tooltip: Option<&str>,
) -> Interaction {
    let c = MachColors::dark();
    let mut btn =
        egui::Button::new(egui::RichText::new(icon).strong()).min_size(egui::vec2(32.0, 32.0));
    if active {
        btn = btn.fill(c.active_bg);
    } else {
        btn = btn.fill(c.widget_bg);
    }
    let resp = if let Some(t) = tooltip {
        ui.add(btn).on_hover_text(t)
    } else {
        ui.add(btn)
    };
    Interaction {
        clicked: resp.clicked(),
    }
}

/// Bouton "chip" compact (toggles, filtres, catégories). Retourne true si cliqué.
pub fn chip(ui: &mut egui::Ui, text: &str, on: bool, color: egui::Color32) -> bool {
    let c = MachColors::dark();
    let mut btn = egui::Button::new(egui::RichText::new(text).strong().size(11.5).color(if on {
        egui::Color32::BLACK
    } else {
        c.text_dim
    }))
    .min_size(egui::vec2(0.0, 24.0))
    .fill(if on { color } else { c.widget_bg })
    .stroke(egui::Stroke::new(
        1.0_f32,
        if on { color } else { c.border },
    ));
    btn = btn.corner_radius(egui::CornerRadius::same(4));
    ui.add(btn).clicked()
}

/// Bouton "chip" avec infobulle au survol.
pub fn chip_tooltip(
    ui: &mut egui::Ui,
    text: &str,
    on: bool,
    color: egui::Color32,
    tooltip: &str,
) -> bool {
    let c = MachColors::dark();
    let mut btn = egui::Button::new(egui::RichText::new(text).strong().size(11.5).color(if on {
        egui::Color32::BLACK
    } else {
        c.text_dim
    }))
    .min_size(egui::vec2(0.0, 24.0))
    .fill(if on { color } else { c.widget_bg })
    .stroke(egui::Stroke::new(
        1.0_f32,
        if on { color } else { c.border },
    ));
    btn = btn.corner_radius(egui::CornerRadius::same(4));
    ui.add(btn).on_hover_text(tooltip).clicked()
}

/// Sélecteur de valeurs discrètes (pill buttons). Retourne l'index sélectionné si changé.
pub fn pill_select(
    ui: &mut egui::Ui,
    id_src: impl std::hash::Hash,
    selected: &mut usize,
    options: &[&str],
    active_color: egui::Color32,
) -> Option<usize> {
    let mut changed = None;
    let id = ui.make_persistent_id(id_src);
    ui.push_id(id, |ui| {
        ui.horizontal(|ui| {
            for (i, opt) in options.iter().enumerate() {
                let is_sel = *selected == i;
                let mut btn = egui::Button::new(egui::RichText::new(*opt).strong().size(11.5))
                    .min_size(egui::vec2(38.0, 26.0));
                if is_sel {
                    btn = btn
                        .fill(active_color)
                        .stroke(egui::Stroke::new(1.0_f32, active_color));
                } else {
                    btn =
                        btn.fill(ui.visuals().widgets.inactive.bg_fill)
                            .stroke(egui::Stroke::new(
                                1.0_f32,
                                ui.visuals().widgets.inactive.bg_stroke.color,
                            ));
                }
                if ui.add(btn).clicked() {
                    *selected = i;
                    changed = Some(i);
                }
            }
        });
    });
    changed
}

/// Caractéristiques d'un potentiomètre.
pub struct KnobSpec<'a> {
    /// Libellé sous le knob
    pub label: &'a str,
    /// Unité affichée (ex " dB")
    pub unit: &'a str,
    /// Minimum
    pub min: f32,
    /// Maximum
    pub max: f32,
    /// Valeur par défaut (double-clic)
    pub default: f32,
    /// Échelle logarithmique (fréquences)
    pub log: bool,
    /// Diamètre
    pub size: f32,
}

impl<'a> KnobSpec<'a> {
    /// Knob linéaire standard.
    #[must_use]
    pub fn linear(label: &'a str, unit: &'a str, min: f32, max: f32, default: f32) -> Self {
        Self {
            label,
            unit,
            min,
            max,
            default,
            log: false,
            size: 46.0,
        }
    }

    /// Knob logarithmique (fréquences).
    #[must_use]
    pub fn log(label: &'a str, unit: &'a str, min: f32, max: f32, default: f32) -> Self {
        Self {
            label,
            unit,
            min,
            max,
            default,
            log: true,
            size: 46.0,
        }
    }
}

fn format_value(value: f32, unit: &str) -> String {
    if unit == " Hz" && value >= 1000.0 {
        format!("{:.2} kHz", value / 1000.0)
    } else if unit == " %" {
        format!("{:.0} %", value * 100.0)
    } else if value.abs() >= 100.0 {
        format!("{value:.0}{unit}")
    } else {
        format!("{value:.1}{unit}")
    }
}

/// Potentiomètre rotatif façon hardware : glisser verticalement pour tourner,
/// double-clic pour revenir à la valeur par défaut.
/// Retourne `Some(nouvelle_valeur)` si l'utilisateur l'a modifié.
pub fn knob(
    ui: &mut egui::Ui,
    _id_src: impl std::hash::Hash,
    value: f32,
    spec: &KnobSpec<'_>,
) -> Option<f32> {
    let c = MachColors::dark();
    let size = spec.size;
    let total = egui::vec2(size + 4.0, size + 30.0);
    let (rect, resp) = ui.allocate_exact_size(total, egui::Sense::click_and_drag());

    let mut new_value = value;
    if resp.dragged() {
        let dy = resp.drag_delta().y;
        if spec.log {
            let base = value.max(spec.min.max(1.0));
            new_value = base * 2.0f32.powf(-dy / 30.0);
        } else {
            let span = spec.max - spec.min;
            let frac = ((value - spec.min) / span) - dy / 220.0;
            new_value = spec.min + frac.clamp(0.0, 1.0) * span;
        }
        new_value = new_value.clamp(spec.min, spec.max);
    }
    if resp.double_clicked() {
        new_value = spec.default;
    }

    let frac = if spec.log {
        let lo = spec.min.max(1.0).ln();
        let hi = spec.max.max(1.0001).ln();
        ((new_value.max(1.0).ln() - lo) / (hi - lo)).clamp(0.0, 1.0)
    } else {
        ((new_value - spec.min) / (spec.max - spec.min).max(1.0e-6)).clamp(0.0, 1.0)
    };

    let dial_rect = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.top() + size / 2.0),
        egui::vec2(size, size),
    );
    let center = dial_rect.center();
    let radius = size / 2.0;

    // Fond + arc de valeur (270° de balayage, comme la MK3).
    let start_angle = std::f32::consts::PI * 0.75;
    let sweep = std::f32::consts::PI * 1.5;
    ui.painter().circle_filled(center, radius, c.widget_bg);
    ui.painter().circle_stroke(
        center,
        radius,
        egui::Stroke::new(
            1.0_f32,
            if resp.hovered() {
                c.border_hover
            } else {
                c.border
            },
        ),
    );

    let arc_points: Vec<egui::Pos2> = (0..=32)
        .map(|i| {
            let a = start_angle + sweep * (i as f32 / 32.0) * frac;
            center + egui::vec2(a.cos(), a.sin()) * (radius - 3.0)
        })
        .collect();
    if arc_points.len() >= 2 {
        ui.painter().add(egui::Shape::line(
            arc_points,
            egui::Stroke::new(3.0_f32, c.accent),
        ));
    }

    // Index (aiguille).
    let a = start_angle + sweep * frac;
    let idx_start = center + egui::vec2(a.cos(), a.sin()) * (radius * 0.35);
    let idx_end = center + egui::vec2(a.cos(), a.sin()) * (radius * 0.82);
    ui.painter()
        .line_segment([idx_start, idx_end], egui::Stroke::new(2.5_f32, c.text));
    ui.painter().circle_filled(center, 2.5, c.text_muted);

    // Libellé + valeur.
    let text_color = if resp.dragged() { c.accent } else { c.text_dim };
    ui.painter().text(
        egui::pos2(rect.center().x, rect.top() + size + 6.0),
        egui::Align2::CENTER_TOP,
        format_value(new_value, spec.unit),
        egui::FontId::monospace(10.5),
        text_color,
    );
    ui.painter().text(
        egui::pos2(rect.center().x, rect.top() + size + 17.0),
        egui::Align2::CENTER_TOP,
        spec.label,
        egui::FontId::proportional(9.5),
        c.text_muted,
    );

    if resp.dragged() || resp.double_clicked() {
        Some(new_value)
    } else {
        None
    }
}

/// Fader vertical de niveau (dB, échelle -60..+12). Clic + glisser.
/// Retourne `Some(nouvelle_valeur_db)` si modifié.
pub fn fader_db(
    ui: &mut egui::Ui,
    _id_src: impl std::hash::Hash,
    db: f32,
    height: f32,
    width: f32,
) -> Option<f32> {
    let c = MachColors::dark();
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());

    let mut new_db = db;
    if (resp.dragged() || resp.clicked()) && resp.interact_pointer_pos().is_some() {
        let pointer = resp.interact_pointer_pos().unwrap_or(rect.center());
        let pos = ((rect.bottom() - pointer.y) / rect.height().max(1.0)).clamp(0.0, 1.0);
        new_db = pos_to_db(pos);
    }

    let track = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2((width * 0.36).clamp(6.0, 14.0), rect.height()),
    );
    ui.painter()
        .rect_filled(track, egui::CornerRadius::same(3), c.bg);
    ui.painter().rect_stroke(
        track,
        egui::CornerRadius::same(3),
        egui::Stroke::new(1.0_f32, c.border),
        egui::StrokeKind::Inside,
    );

    // Graduations 0 dB / -12 / -24 / -48.
    for mark_db in [-48.0f32, -24.0, -12.0, 0.0] {
        let y = rect.bottom() - db_to_pos(mark_db) * rect.height();
        ui.painter().hline(
            (track.left() - 4.0)..=(track.right() + 4.0),
            y,
            egui::Stroke::new(1.0_f32, c.border.gamma_multiply(0.7)),
        );
    }

    let pos = db_to_pos(new_db);
    let y = rect.bottom() - pos * rect.height();
    let fill = egui::Rect::from_min_max(egui::pos2(track.left(), y), track.right_bottom());
    ui.painter()
        .rect_filled(fill, egui::CornerRadius::same(3), db_color(new_db));

    // Poignée.
    let handle = egui::Rect::from_center_size(
        egui::pos2(track.center().x, y),
        egui::vec2(width * 0.95, 7.0),
    );
    ui.painter()
        .rect_filled(handle, egui::CornerRadius::same(3), c.text);

    if resp.dragged() || resp.clicked() {
        Some(new_db)
    } else {
        None
    }
}

/// Contrôle de pan bipolaire horizontal. Retourne `Some(pan)` si modifié.
pub fn pan_control(
    ui: &mut egui::Ui,
    _id_src: impl std::hash::Hash,
    pan: f32,
    width: f32,
) -> Option<f32> {
    let c = MachColors::dark();
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(width, 18.0), egui::Sense::click_and_drag());

    let mut new_pan = pan;
    if (resp.dragged() || resp.clicked()) && resp.interact_pointer_pos().is_some() {
        let pointer = resp.interact_pointer_pos().unwrap_or(rect.center());
        let rel = (pointer.x - rect.center().x) / (rect.width() / 2.0).max(1.0);
        new_pan = rel.clamp(-1.0, 1.0);
    }

    let track = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width(), 8.0));
    ui.painter()
        .rect_filled(track, egui::CornerRadius::same(4), c.bg);
    let center_x = track.center().x;
    let x = center_x + new_pan * (track.width() / 2.0);
    let fill = egui::Rect::from_min_max(
        egui::pos2(center_x.min(x), track.top()),
        egui::pos2(center_x.max(x), track.bottom()),
    );
    ui.painter()
        .rect_filled(fill, egui::CornerRadius::same(4), c.accent_cyan);
    ui.painter().vline(
        center_x,
        track.top()..=track.bottom(),
        egui::Stroke::new(1.0_f32, c.text_muted),
    );

    if resp.dragged() || resp.clicked() {
        Some(new_pan)
    } else {
        None
    }
}

/// VU-mètre vertical réel (niveaux linéaires 0..1+), peak hold, échelle dB.
/// Dessine `bars` barres (1 ou 2) dans la largeur allouée.
pub fn vu_meter(ui: &mut egui::Ui, levels: [f32; 2], peaks: [f32; 2], width: f32, height: f32) {
    let c = MachColors::dark();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let gap = 2.0;
    let bar_w = ((rect.width() - gap) / 2.0).max(3.0);

    for (i, &level) in levels.iter().enumerate() {
        let x = rect.left() + i as f32 * (bar_w + gap);
        let bar =
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(bar_w, rect.height()));
        ui.painter()
            .rect_filled(bar, egui::CornerRadius::same(2), c.bg);

        let db = level_to_db(level);
        let pos = db_to_pos(db);
        if pos > 0.0 {
            let y = rect.bottom() - pos * rect.height();
            let fill =
                egui::Rect::from_min_max(egui::pos2(x, y), egui::pos2(x + bar_w, rect.bottom()));
            ui.painter()
                .rect_filled(fill, egui::CornerRadius::same(2), db_color(db));
        }

        // Peak hold.
        let peak_db = level_to_db(peaks[i]);
        if peak_db > -59.0 {
            let py = rect.bottom() - db_to_pos(peak_db) * rect.height();
            ui.painter()
                .hline(x..=(x + bar_w), py, egui::Stroke::new(1.5_f32, c.text));
        }
    }

    // Graduations dB : 0, -12, -24, -48.
    for mark_db in [0.0f32, -12.0, -24.0, -48.0] {
        let y = rect.bottom() - db_to_pos(mark_db) * rect.height();
        ui.painter().hline(
            (rect.left() - 3.0)..=(rect.right() + 2.0),
            y,
            egui::Stroke::new(1.0_f32, c.border.gamma_multiply(0.6)),
        );
    }
}

/// État d'un pad pour l'affichage.
pub struct PadState<'a> {
    pub index: u8,
    /// Couleur affichée : celle de la LED du pad physique.
    pub color: egui::Color32,
    pub label: &'a str,
    /// Sélectionné (Sound actif, pattern assigné, pas édité…).
    pub selected: bool,
    /// L'utilisateur pressait ce pad au frame précédent (pour détecter le release).
    pub was_down: bool,
}

/// Interaction d'un pad (press/release avec vélocité selon la position du clic).
pub struct PadInteraction {
    pub pressed: bool,
    pub released: bool,
    /// Clic droit (bouton secondaire) : sélectionner le Sound sans jouer.
    pub secondary_clicked: bool,
    pub velocity: u8,
}

/// Couleur écran d'une LED MK3 (index palette NI). Une LED « atténuée »
/// reste bien colorée à l'œil : on relève les tons sombres pour que le
/// miroir ressemble au pad réel plutôt qu'à un pad noir.
#[must_use]
pub fn led_color(index: u8) -> egui::Color32 {
    let (r, g, b) = maschine_proto::color::PaletteIndex::from_u8(index)
        .map_or((0, 0, 0), maschine_proto::color::PaletteIndex::to_rgb);
    let lift = |v: u8| (255.0 * (f32::from(v) / 255.0).powf(0.6)) as u8;
    let off = egui::Color32::from_rgb(0x26, 0x28, 0x2C);
    if (r, g, b) == (0, 0, 0) {
        off
    } else {
        egui::Color32::from_rgb(lift(r), lift(g), lift(b))
    }
}

/// Texte lisible sur un fond donné (noir sur clair, blanc sur sombre).
#[must_use]
pub fn ink_on(fill: egui::Color32) -> egui::Color32 {
    let l = 0.299 * f32::from(fill.r()) + 0.587 * f32::from(fill.g()) + 0.114 * f32::from(fill.b());
    if l > 150.0 {
        egui::Color32::from_rgb(0x11, 0x12, 0x14)
    } else {
        egui::Color32::WHITE
    }
}

/// Pad de la grille 4×4 : couleur de la LED physique, vélocité
/// proportionnelle à la position verticale du clic (comme le hardware).
pub fn pad_button(ui: &mut egui::Ui, st: &PadState<'_>, size: f32) -> PadInteraction {
    let c = MachColors::dark();
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click_and_drag());
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            true,
            format!("Pad {} {}", st.index + 1, st.label),
        )
    });

    let is_down = resp.is_pointer_button_down_on();
    let pressed = is_down && !st.was_down;
    let released = st.was_down && !is_down;

    let mut velocity = 100u8;
    if pressed {
        if let Some(pos) = resp.interact_pointer_pos() {
            let rel = ((pos.y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
            velocity = (60.0 + 67.0 * rel) as u8;
        }
    }

    // Pression souris : le pad s'éclaire comme sous le doigt.
    let fill = if is_down {
        st.color.lerp_to_gamma(egui::Color32::WHITE, 0.35)
    } else {
        st.color
    };
    let radius = egui::CornerRadius::same((size * 0.08).clamp(4.0, 8.0) as u8);
    ui.painter().rect_filled(rect, radius, fill);
    let (border_w, border_col) = if st.selected {
        (2.0_f32, c.text)
    } else if resp.hovered() || resp.has_focus() {
        (1.5, c.border_hover)
    } else {
        (1.0, fill.gamma_multiply(0.55))
    };
    ui.painter().rect_stroke(
        rect,
        radius,
        egui::Stroke::new(border_w, border_col),
        egui::StrokeKind::Inside,
    );

    let ink = ink_on(fill);
    let pad = (size * 0.08).clamp(4.0, 8.0);
    ui.painter().text(
        egui::pos2(rect.left() + pad, rect.top() + pad * 0.7),
        egui::Align2::LEFT_TOP,
        format!("{}", st.index + 1),
        egui::FontId::proportional((size * 0.16).clamp(9.0, 13.0)),
        ink.gamma_multiply(0.7),
    );
    if size >= 36.0 && !st.label.is_empty() {
        let font = (size * 0.15).clamp(8.0, 12.5);
        // Libellé tronqué à la largeur du pad (pas de débordement sur le voisin).
        let max_chars = ((size - 2.0 * pad) / (font * 0.55)).max(3.0) as usize;
        let label: String = st.label.chars().take(max_chars).collect();
        ui.painter().text(
            egui::pos2(rect.left() + pad, rect.bottom() - pad * 0.7),
            egui::Align2::LEFT_BOTTOM,
            label,
            egui::FontId::proportional(font),
            ink,
        );
    }

    PadInteraction {
        pressed,
        released,
        secondary_clicked: resp.secondary_clicked(),
        velocity,
    }
}

/// Encodeur sans fin façon MK3 : glisser verticalement tourne. Comme sur le
/// contrôleur, son nom et sa valeur s'affichent sur l'écran juste au-dessus.
/// `turn` garde l'angle du repère ; retourne des ticks bruts (1024 = 1 tour).
pub fn encoder(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    id: egui::Id,
    turn: &mut f32,
    number: u8,
) -> Option<i32> {
    let c = MachColors::dark();
    let resp = ui
        .interact(rect, id, egui::Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeVertical)
        .on_hover_text(format!("Knob {number} : glisser haut/bas"));
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::DragValue, true, format!("Knob {number}"))
    });
    // ~4 ticks par point : un glisser de 256 px = un tour de knob physique.
    let ticks = if resp.dragged() {
        (-resp.drag_delta().y * 4.0).round() as i32
    } else {
        0
    };
    *turn += ticks as f32 / 1024.0 * std::f32::consts::TAU;

    let center = rect.center();
    let r = rect.width() / 2.0;
    let live = resp.dragged() || resp.hovered();
    ui.painter().circle_filled(center, r, c.widget_bg);
    ui.painter().circle_stroke(
        center,
        r,
        egui::Stroke::new(1.5_f32, if live { c.accent } else { c.border_active }),
    );
    ui.painter().circle_filled(center, r * 0.72, c.surface);
    let dir = egui::vec2(
        (*turn - std::f32::consts::FRAC_PI_2).cos(),
        (*turn - std::f32::consts::FRAC_PI_2).sin(),
    );
    ui.painter().line_segment(
        [center + dir * r * 0.30, center + dir * r * 0.68],
        egui::Stroke::new(2.5_f32, if live { c.accent } else { c.text }),
    );
    (ticks != 0).then_some(ticks)
}

/// Cellule de pas du séquenceur. Retourne cliqué / clic droit (effacer).
pub fn step_cell(
    ui: &mut egui::Ui,
    size: egui::Vec2,
    active: bool,
    velocity: f32,
    playhead: bool,
    on_beat: bool,
    color: egui::Color32,
) -> egui::Response {
    let c = MachColors::dark();
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let bg = if on_beat { c.widget_bg } else { c.surface };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(4), bg);
    if active {
        let v = velocity.clamp(0.15, 1.0);
        let fill = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 2.0, rect.bottom() - rect.height() * v + 2.0),
            egui::pos2(rect.right() - 2.0, rect.bottom() - 2.0),
        );
        ui.painter()
            .rect_filled(fill, egui::CornerRadius::same(3), color);
    }
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(4),
        egui::Stroke::new(
            if playhead { 2.0_f32 } else { 1.0_f32 },
            if playhead { c.flash } else { c.border },
        ),
        egui::StrokeKind::Inside,
    );
    if resp.hovered() {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(4),
            egui::Stroke::new(1.0_f32, c.border_hover),
            egui::StrokeKind::Outside,
        );
    }
    resp
}

/// Rendu d'une image RGB565 (DisplayFrame MK3) en texture egui.
#[must_use]
pub fn display_pixels_to_image(
    w: usize,
    h: usize,
    pixels: &[maschine_proto::display::DisplayPixel],
) -> egui::ColorImage {
    let mut colors = Vec::with_capacity(w * h);
    for p in pixels.iter().take(w * h) {
        let v = p.0;
        let r5 = ((v >> 11) & 0x1F) as u8;
        let g6 = ((v >> 5) & 0x3F) as u8;
        let b5 = (v & 0x1F) as u8;
        let r = (r5 << 3) | (r5 >> 2);
        let g = (g6 << 2) | (g6 >> 4);
        let b = (b5 << 3) | (b5 >> 2);
        colors.push(egui::Color32::from_rgb(r, g, b));
    }
    colors.resize(w * h, egui::Color32::BLACK);
    egui::ColorImage::new([w, h], colors)
}

/// Les 8 boutons d'écran de la MK3 en boutons GUI (allumés comme leurs LEDs)
/// puis les paramètres des knobs : la GUI fait exactement ce que fait le
/// contrôleur, avec les mêmes libellés. `params` : valeurs à afficher.
pub fn soft_key_strip(
    ui: &mut egui::Ui,
    app: &mut crate::gui::app::GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    params: &[Option<maschine_engine::plugin_pages::ParamView>; 8],
) {
    let c = crate::gui::theme::MachColors::dark();
    ui.horizontal_wrapped(|ui| {
        for (n, label) in snap.soft_labels.iter().enumerate() {
            if label.is_empty() {
                continue;
            }
            let lit = snap.soft_active[n];
            let text = egui::RichText::new(*label).color(if lit {
                crate::gui::theme::ink_on(c.accent)
            } else {
                c.text
            });
            let button = egui::Button::new(text).fill(if lit { c.accent } else { c.surface });
            if ui
                .add(button)
                .on_hover_text(format!("Bouton {} de la MK3", n + 1))
                .clicked()
            {
                app.send(maschine_engine::EngineEvent::SoftKey(n as u8 + 1));
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        for (k, pv) in params.iter().enumerate() {
            let Some(pv) = pv else { continue };
            ui.label(
                egui::RichText::new(format!("{} {}", pv.name, pv.value))
                    .small()
                    .color(c.text_dim),
            )
            .on_hover_text(format!("Knob {} de la MK3", k + 1));
        }
    });
}
