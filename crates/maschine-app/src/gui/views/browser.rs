//! Vue BROWSER — presets du kit 808 (catégories, recherche, pré-écoute,
//! chargement sur le Sound actif), workflow identique au hardware MASCHINE.

use super::*;
use crate::gui::app::GuiApp;
use crate::gui::theme::{card_frame, MachColors};
use crate::gui::widgets::chip;
use eframe::egui;

use maschine_engine::EngineEvent;

/// Catégories du browser → index des presets du kit 808.
const CATEGORIES: &[(&str, &[usize])] = &[
    (
        "TOUT",
        &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    ),
    ("KICK", &[0, 15]),
    ("SNARE", &[1, 7]),
    ("HATS", &[2, 3]),
    ("TOMS", &[4, 5, 6]),
    ("PERC", &[8, 9, 10, 11, 12, 13]),
    ("SYNTH", &[14]),
];

pub fn render(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();
    let gf = snap.group_focus as usize;
    let selected = snap.browser_preset_idx as usize;

    ui.horizontal(|ui| {
        crate::gui::theme::view_header(
            ui,
            "🗂 BROWSER",
            "Presets du kit 808 — flèches/encodeur pour naviguer, Entrée pour charger.",
        );
    });

    ui.add_space(6.0);

    ui.horizontal_top(|ui| {
        // ── Liste des presets ─────────────────────────────────────────
        card_frame().show(ui, |ui| {
            ui.set_width(420.0);

            // Catégories.
            let mut current_cat = app.browser_category;
            ui.horizontal_wrapped(|ui| {
                for (i, (name, _)) in CATEGORIES.iter().enumerate() {
                    if chip(ui, name, current_cat == i, c.accent) && current_cat != i {
                        current_cat = i;
                    }
                }
            });
            app.browser_category = current_cat;

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Recherche").small().color(c.text_dim));
                ui.add(
                    egui::TextEdit::singleline(&mut app.browser_search)
                        .hint_text("kick, hat, perc…")
                        .desired_width(200.0),
                );
                if !app.browser_search.is_empty() && chip(ui, "✕", false, c.text_dim) {
                    app.browser_search.clear();
                }
            });

            ui.add_space(6.0);
            ui.separator();

            let search = app.browser_search.to_lowercase();
            let cat_presets = CATEGORIES
                .get(current_cat)
                .map(|(_, idxs)| *idxs)
                .unwrap_or(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);

            let mut clicked_preset: Option<usize> = None;
            let mut load_preset: Option<usize> = None;
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for &p in cat_presets {
                        let name = maschine_engine::engine::DRUM_PRESET_NAMES[p];
                        if !search.is_empty() && !name.to_lowercase().contains(&search) {
                            continue;
                        }
                        let is_sel = p == selected;
                        let resp = ui.add(
                            egui::Button::new(
                                egui::RichText::new(format!("{:02}  {name}", p + 1))
                                    .monospace()
                                    .size(12.5)
                                    .color(if is_sel { egui::Color32::BLACK } else { c.text }),
                            )
                            .fill(if is_sel { c.accent } else { c.widget_bg })
                            .stroke(egui::Stroke::new(1.0_f32, c.border))
                            .min_size(egui::vec2(ui.available_width(), 26.0)),
                        );
                        if resp.clicked() {
                            clicked_preset = Some(p);
                        }
                        if resp.double_clicked() {
                            load_preset = Some(p);
                        }
                    }
                });

            if let Some(p) = clicked_preset {
                {
                    let mut eng = app.engine.lock();
                    eng.set_browser_preset(p as u8);
                }
                let _ = app
                    .engine
                    .lock()
                    .handle_event(EngineEvent::BrowsePresetStep(p as i32 - selected as i32));
            }
            if let Some(p) = load_preset {
                let _ = app.engine.lock().handle_event(EngineEvent::LoadDrumPreset {
                    group: snap.group_focus,
                    sound: snap.active_sound,
                    preset_idx: p as u8,
                });
            }
        });

        ui.add_space(8.0);

        // ── Détails preset ────────────────────────────────────────────
        card_frame().show(ui, |ui| {
            ui.set_width(280.0);
            let name = maschine_engine::engine::DRUM_PRESET_NAMES[selected.min(15)];
            ui.label(
                egui::RichText::new(name)
                    .strong()
                    .size(17.0)
                    .color(c.accent),
            );
            ui.label(
                egui::RichText::new("Preset de batterie — synthèse DSP 808")
                    .small()
                    .color(c.text_dim),
            );

            ui.add_space(10.0);
            crate::gui::theme::section(ui, "CIBLE (SOUND ACTIF)", |ui| {
                let group_name = snap
                    .project
                    .groups
                    .get(gf)
                    .map(|g| g.name.clone())
                    .unwrap_or_default();
                let sound_name = snap
                    .project
                    .groups
                    .get(gf)
                    .and_then(|g| g.sounds.get(snap.active_sound as usize))
                    .and_then(|s| s.as_ref())
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| "(vide)".to_string());
                ui.label(
                    egui::RichText::new(format!(
                        "Groupe {} — {} • Sound {:02} — {sound_name}",
                        group_char(gf),
                        group_name,
                        snap.active_sound + 1
                    ))
                    .color(c.text),
                );
            });

            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("▶  PRÉ-ÉCOUTE").strong())
                            .min_size(egui::vec2(220.0, 34.0))
                            .fill(c.widget_bg)
                            .stroke(egui::Stroke::new(1.0_f32, c.success)),
                    )
                    .clicked()
                {
                    app.engine.lock().preview_preset(selected as u8);
                }
                ui.add_space(6.0);
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("⏎  CHARGER SUR LE SOUND").strong())
                            .min_size(egui::vec2(220.0, 40.0))
                            .fill(c.accent)
                            .stroke(egui::Stroke::new(1.5_f32, c.accent)),
                    )
                    .clicked()
                {
                    let _ = app.engine.lock().handle_event(EngineEvent::LoadDrumPreset {
                        group: snap.group_focus,
                        sound: snap.active_sound,
                        preset_idx: selected as u8,
                    });
                }
            });

            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(
                    "Raccourcis hardware : encodeur = navigation, \
                     bouton 8 = charger, bouton 8 + SHIFT = pré-écoute.",
                )
                .small()
                .color(c.text_muted),
            );
        });
    });
}
