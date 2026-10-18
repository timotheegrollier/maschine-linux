//! Vue IDEAS — scènes, matrice Groupes × Patterns et timeline SONG.
//!
//! C'est le cœur du workflow Maschine : on assemble des idées (patterns par
//! Groupe) dans des Scènes, puis les Scènes dans le Song.

use super::*;
use crate::gui::app::GuiApp;
use crate::gui::theme::{card_frame, section, MachColors};
use crate::gui::widgets::chip;
use eframe::egui;

use maschine_engine::EngineEvent;

pub fn render(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();
    let active_scene = snap.active_scene;

    ui.horizontal(|ui| {
        crate::gui::theme::view_header(
            ui,
            "🧭 IDEAS",
            "Scènes × Patterns par Groupe — puis SONG pour l'enchaînement.",
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if chip(ui, "▶ MODE SONG", snap.song_mode, c.accent) {
                let _ = app.engine.lock().handle_event(EngineEvent::ToggleSongMode);
            }
        });
    });

    ui.add_space(8.0);

    ui.horizontal_top(|ui| {
        // ── Liste des scènes ─────────────────────────────────────────
        card_frame().show(ui, |ui| {
            ui.set_width(210.0);
            ui.label(
                egui::RichText::new("SCÈNES")
                    .strong()
                    .small()
                    .color(c.accent),
            );
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .max_height(300.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (i, scene) in snap.project.scenes.iter().enumerate() {
                        let is_active = active_scene == Some(i);
                        let used = scene.patterns.iter().filter(|p| p.is_some()).count();
                        let resp = ui.add(
                            egui::Button::new(
                                egui::RichText::new(format!("{:02}   Scène {}", i + 1, i + 1))
                                    .strong()
                                    .color(if is_active {
                                        egui::Color32::BLACK
                                    } else {
                                        c.text
                                    }),
                            )
                            .fill(if is_active { c.accent } else { c.widget_bg })
                            .stroke(egui::Stroke::new(1.0_f32, c.border))
                            .min_size(egui::vec2(190.0, 28.0)),
                        );
                        if resp
                            .on_hover_text(format!("{used} groupe(s) actifs"))
                            .clicked()
                        {
                            let _ = app.engine.lock().handle_event(EngineEvent::Scene(i));
                        }
                    }
                });

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if chip(ui, "＋ Nouvelle scène", false, c.success) {
                    let _ = app.engine.lock().handle_event(EngineEvent::AddScene);
                }
                if chip(ui, "⧉ Dupliquer", false, c.accent_cyan) {
                    let _ = app.engine.lock().handle_event(EngineEvent::DuplicateScene);
                }
                if chip(ui, "🗑 Supprimer", false, c.error) {
                    let _ = app.engine.lock().handle_event(EngineEvent::DeleteScene);
                }
            });
        });

        ui.add_space(8.0);

        // ── Matrice Groupes × Patterns ───────────────────────────────
        card_frame().show(ui, |ui| {
            ui.label(
                egui::RichText::new("MATRICE — cliquer pour assigner le pattern à la scène active")
                    .strong()
                    .small()
                    .color(c.accent),
            );
            ui.add_space(6.0);

            let max_cols = snap
                .project
                .groups
                .iter()
                .map(|g| g.patterns.len().min(8))
                .max()
                .unwrap_or(1)
                .max(1);

            egui::Grid::new("ideas_matrix")
                .spacing(egui::vec2(4.0, 4.0))
                .show(ui, |ui| {
                    // En-tête des colonnes.
                    ui.label("");
                    for col in 0..max_cols {
                        ui.label(
                            egui::RichText::new(format!("P{}", col + 1))
                                .monospace()
                                .small()
                                .color(c.text_dim),
                        );
                    }
                    ui.end_row();

                    for (g, group) in snap.project.groups.iter().enumerate() {
                        ui.label(
                            egui::RichText::new(format!("{} {}", group_char(g), group.name))
                                .small()
                                .color(if g == snap.group_focus as usize {
                                    c.accent
                                } else {
                                    c.text
                                }),
                        );
                        for col in 0..max_cols {
                            let exists = col < group.patterns.len();
                            let assigned = active_scene
                                .and_then(|s| snap.project.scenes.get(s))
                                .and_then(|sc| sc.patterns.get(g))
                                .copied()
                                .flatten()
                                == Some(col);
                            let name = if exists {
                                group.patterns[col].name.clone()
                            } else {
                                "—".to_string()
                            };
                            let short: String = name.chars().take(6).collect();

                            let mut btn = egui::Button::new(
                                egui::RichText::new(short).monospace().size(10.5).color(
                                    if assigned {
                                        egui::Color32::BLACK
                                    } else {
                                        c.text_dim
                                    },
                                ),
                            )
                            .min_size(egui::vec2(52.0, 26.0))
                            .fill(if assigned {
                                c.accent_cyan
                            } else if exists {
                                c.widget_bg
                            } else {
                                c.surface.gamma_multiply(0.6)
                            })
                            .stroke(egui::Stroke::new(1.0_f32, c.border));
                            if !exists {
                                btn = btn.stroke(egui::Stroke::new(
                                    1.0_f32,
                                    c.border.gamma_multiply(0.4),
                                ));
                            }
                            let resp = ui.add_enabled(exists, btn);
                            if resp.clicked() {
                                let _ =
                                    app.engine.lock().handle_event(EngineEvent::PatternSelect {
                                        group: g as u8,
                                        pattern: col,
                                    });
                            }
                            if resp.hovered() && exists {
                                resp.on_hover_text(format!("{name} — assigner à la scène active"));
                            }
                        }
                        ui.end_row();
                    }
                });
        });
    });

    ui.add_space(10.0);

    // ── Timeline SONG ────────────────────────────────────────────────
    section(ui, "SONG — ENCHAÎNEMENT DES SCÈNES", |ui| {
        ui.horizontal(|ui| {
            if chip(ui, "＋ Ajouter section", false, c.success) {
                let _ = app.engine.lock().handle_event(EngineEvent::AddSection);
            }
            if chip(ui, "⏮ Section préc.", false, c.text_dim) {
                let _ = app.engine.lock().handle_event(EngineEvent::PrevSection);
            }
            if chip(ui, "⏭ Section suiv.", false, c.text_dim) {
                let _ = app.engine.lock().handle_event(EngineEvent::NextSection);
            }
            if chip(ui, "⧉ Dupliquer section", false, c.accent_cyan) {
                let _ = app
                    .engine
                    .lock()
                    .handle_event(EngineEvent::DuplicateSection);
            }
            if chip(ui, "🗑 Supprimer section", false, c.error) {
                let _ = app.engine.lock().handle_event(EngineEvent::DeleteSection);
            }
            ui.separator();
            ui.label(egui::RichText::new("Longueur :").small().color(c.text_dim));
            for bars in [1usize, 2, 4, 8] {
                let on = snap
                    .active_section
                    .and_then(|i| snap.project.song.get(i))
                    .map(|s| s.length.bars as usize == bars)
                    .unwrap_or(false);
                if chip(ui, &format!("{bars} mes."), on, c.accent) {
                    let _ = app
                        .engine
                        .lock()
                        .handle_event(EngineEvent::ResizeSection { bars });
                }
            }
            if !snap.song_mode {
                ui.label(
                    egui::RichText::new("(le Song joue en mode SONG — bouton en haut à droite)")
                        .small()
                        .color(c.text_muted),
                );
            }
        });
        ui.add_space(6.0);

        if snap.project.song.is_empty() {
            ui.label(
                egui::RichText::new(
                    "Song vide — ajoutez des sections (scène active + longueur 2 mesures).",
                )
                .color(c.text_dim),
            );
        } else {
            let play_section = snap.active_section;
            ui.horizontal_wrapped(|ui| {
                for (i, sec) in snap.project.song.iter().enumerate() {
                    let is_active = snap.song_mode && play_section == Some(i);
                    let scene_name = format!("Scène {}", sec.scene + 1);
                    let text = format!(
                        "{}  •  {}  •  {} mes.",
                        sec.name, scene_name, sec.length.bars
                    );
                    let mut btn =
                        egui::Button::new(egui::RichText::new(text).strong().color(if is_active {
                            egui::Color32::BLACK
                        } else {
                            c.text
                        }))
                        .min_size(egui::vec2(0.0, 34.0))
                        .fill(if is_active { c.accent } else { c.widget_bg })
                        .stroke(egui::Stroke::new(1.0_f32, c.border));
                    if is_active {
                        btn = btn.stroke(egui::Stroke::new(2.0_f32, c.flash));
                    }
                    if ui.add(btn).clicked() {
                        let _ = app
                            .engine
                            .lock()
                            .handle_event(EngineEvent::SelectSection(i));
                    }
                }
            });
        }
    });
}
