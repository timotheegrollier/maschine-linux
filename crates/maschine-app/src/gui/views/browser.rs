//! Vue BROWSER / FILE — presets 808 et navigateur de projets (JSON lisible).
//!
//! - PRESETS : navigation/pré-écoute/chargement des kits batterie, comme le hardware.
//! - FILE/PROJETS : liste des projets, sauvegarde rapide/complète, chargement rapide
//!   depuis la MK3 ou la GUI.

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
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                crate::gui::theme::view_header(
                    ui,
                    "BROWSER / FILE",
                    "Presets batterie et projets. Chargez rapidement depuis la MK3 ou la GUI.",
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if chip(ui, "PROJETS / FILE", snap.browser_kind == 1, c.accent_cyan) {
                        app.open_project_browser();
                    }
                    if chip(ui, "PRESETS 808", snap.browser_kind == 0, c.accent) {
                        app.send(EngineEvent::SetBrowserKind(0));
                        app.send(EngineEvent::SetLeftTab(3));
                    }
                });
            });
            ui.add_space(8.0);

            if snap.browser_kind == 1 {
                render_projects(ui, app, snap, &c);
            } else {
                render_presets(ui, app, snap, &c);
            }
        });
}

fn render_projects(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    c: &MachColors,
) {
    let mut refresh = false;
    let mut load_path: Option<String> = None;
    let mut select_idx: Option<usize> = None;

    ui.horizontal(|ui| {
        let cur = snap
            .current_project_path
            .as_deref()
            .unwrap_or("(nouveau projet)");
        ui.label(
            egui::RichText::new(format!("Projet courant : {}", cur))
                .small()
                .color(c.text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Rafraîchir").clicked() {
                refresh = true;
            }
            if ui
                .add(egui::Button::new("Sauvegarde rapide").fill(c.widget_bg))
                .clicked()
            {
                app.quick_save_project();
            }
            if ui
                .add(egui::Button::new("Sauvegarder").fill(c.accent))
                .clicked()
            {
                app.save_project();
            }
        });
    });

    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(
            "Format : JSON .maschine-linux.json (lisible). MK3 : FILE ouvre la liste, 4D/potar navigue, encoder/soft key charge. Shift+FILE SAVE = sauvegarde rapide.",
        )
        .small()
        .color(c.text_muted),
    );
    ui.add_space(8.0);

    card_frame().show(ui, |ui| {
        ui.label(egui::RichText::new("PROJETS").strong().color(c.accent_cyan));
        ui.add_space(4.0);
        if snap.project_entries.is_empty() {
            ui.label(
                egui::RichText::new("Aucun projet trouvé. Sauvegardez pour créer un fichier dans le dossier projets.")
                    .color(c.text_dim),
            );
        } else {
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (i, entry) in snap.project_entries.iter().enumerate() {
                        let selected = i == snap.project_selected;
                        let is_current = snap
                            .current_project_path
                            .as_ref()
                            .map(|p| p == &entry.path)
                            .unwrap_or(false);
                        let fill = if selected { c.widget_bg } else { c.panel };
                        ui.horizontal(|ui| {
                            let label = format!("{}  ·  {}", entry.name, entry.meta);
                            let resp = ui.add(
                                egui::Button::new(
                                    egui::RichText::new(label)
                                        .color(if selected { c.accent } else { c.text }),
                                )
                                .fill(fill)
                                .min_size(egui::vec2(ui.available_width() - 120.0, 28.0)),
                            );
                            if resp.clicked() {
                                select_idx = Some(i);
                            }
                            if resp.double_clicked() {
                                load_path = Some(entry.path.clone());
                            }
                            if is_current {
                                ui.label(egui::RichText::new("COURANT").small().color(c.success));
                            }
                            if ui.small_button("Charger").clicked() {
                                load_path = Some(entry.path.clone());
                            }
                        });
                    }
                });
        }
    });

    if refresh {
        let settings = crate::settings::AppSettings::load();
        let entries = crate::projects::scan_projects(&settings);
        app.send(EngineEvent::SetProjectEntries(entries));
    }
    if let Some(idx) = select_idx {
        let delta = idx as i32 - snap.project_selected as i32;
        if delta != 0 {
            app.send(EngineEvent::BrowseBrowserStep(delta));
        }
    }
    if let Some(path) = load_path {
        app.load_project_file(std::path::Path::new(&path));
    }
}

fn render_presets(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    c: &MachColors,
) {
    let gf = snap.group_focus as usize;
    let selected = snap.browser_preset_idx as usize;

    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("CATÉGORIE")
                .strong()
                .color(c.accent_cyan),
        );
        ui.add_space(6.0);
        for (i, (cat, _)) in CATEGORIES.iter().enumerate() {
            if chip(ui, cat, i == app.browser_category, c.accent_cyan) {
                app.browser_category = i;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.browser_search)
                    .hint_text("🔍  Rechercher un son…")
                    .desired_width(180.0),
            );
        });
    });
    ui.add_space(8.0);

    let search = app.browser_search.to_lowercase();
    let cat_items = CATEGORIES[app.browser_category.min(CATEGORIES.len() - 1)].1;
    let filtered: Vec<usize> = cat_items
        .iter()
        .copied()
        .filter(|&i| {
            let name = maschine_engine::engine::DRUM_PRESET_NAMES[i];
            search.is_empty() || name.to_lowercase().contains(&search)
        })
        .collect();

    let mut clicked_preset: Option<usize> = None;
    let mut load_preset: Option<usize> = None;

    ui.horizontal_top(|ui| {
        let list_w = (ui.available_width() * 0.5).max(220.0);
        card_frame().show(ui, |ui| {
            ui.set_width(list_w);
            ui.label(
                egui::RichText::new("RÉSULTATS")
                    .strong()
                    .color(c.accent_cyan),
            );
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for &i in &filtered {
                        let name = maschine_engine::engine::DRUM_PRESET_NAMES[i];
                        let is_sel = i == selected;
                        let fill = if is_sel { c.widget_bg } else { c.panel };
                        let resp = ui.add(
                            egui::Button::new(
                                egui::RichText::new(format!("{:02}  {name}", i + 1))
                                    .monospace()
                                    .color(if is_sel { c.accent } else { c.text }),
                            )
                            .fill(fill)
                            .min_size(egui::vec2(list_w - 24.0, 26.0)),
                        );
                        if resp.clicked() {
                            clicked_preset = Some(i);
                        }
                        if resp.double_clicked() {
                            load_preset = Some(i);
                        }
                    }
                    if filtered.is_empty() {
                        ui.label(
                            egui::RichText::new("Aucun résultat.")
                                .italics()
                                .color(c.text_muted),
                        );
                    }
                });
        });

        ui.add_space(8.0);
        card_frame().show(ui, |ui| {
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
        });
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
}
