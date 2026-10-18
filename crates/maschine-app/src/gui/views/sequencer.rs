//! Vue SÉQUENCEUR — grille 16 Sons × 16 pas (style Maschine STEP view).
//!
//! Chaque ligne = un Sound du Groupe focus, chaque colonne = un pas de la
//! mesure sélectionnée. Clic = poser/retirer le pas (même mapping que le
//! hardware : tick = mesure × 3840 + pas × 240).

use super::*;
use crate::gui::app::GuiApp;
use crate::gui::theme::{section, MachColors};
use crate::gui::widgets::{chip, icon_button, mach_button, pill_select, step_cell};
use eframe::egui;

use maschine_engine::EngineEvent;

pub fn render(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    is_playing: bool,
) {
    let c = MachColors::dark();
    let gf = snap.group_focus as usize;
    let pattern_index = snap
        .project
        .active_pattern_index(gf, snap.active_scene)
        .unwrap_or(0);
    let total_bars = snap.pattern_bars.max(1);
    let selected_bar = snap.selected_bar.min(total_bars - 1);
    let events = snap
        .project
        .groups
        .get(gf)
        .and_then(|g| g.patterns.get(pattern_index))
        .map(|pat| pat.events.clone())
        .unwrap_or_default();

    // Table d'occupation : [sound][step] → (velocity, velocity_bars)
    let mut occupied = [[false; 16]; 16];
    let mut velocity = [[0.0f32; 16]; 16];
    for e in &events {
        let bar = (e.tick / 3840) as usize;
        if bar != selected_bar {
            continue;
        }
        let step = (((e.tick % 3840) + 120) / 240).min(15) as usize;
        let snd = (e.pad as usize).min(15);
        occupied[snd][step] = true;
        velocity[snd][step] = (e.velocity as f32 / 127.0).clamp(0.2, 1.0);
    }

    let play_step =
        if is_playing && (snap.transport.tick / 3840) as usize % total_bars == selected_bar {
            Some(((snap.transport.tick % 3840) / 240) as usize)
        } else {
            None
        };

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // ── Barre d'outils ─────────────────────────────────────────
            ui.horizontal(|ui| {
                crate::gui::theme::view_header(ui, "🎹 SÉQUENCEUR", "");
                ui.add_space(10.0);

                let mut bar_sel = selected_bar;
                let bar_opts: Vec<String> = (1..=total_bars).map(|b| format!("{b}")).collect();
                let bar_refs: Vec<&str> = bar_opts.iter().map(String::as_str).collect();
                if let Some(i) = pill_select(ui, "seq_bars", &mut bar_sel, &bar_refs, c.accent) {
                    let _ = app.engine.lock().handle_event(EngineEvent::SelectBar(i));
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                let mut len_sel = match total_bars {
                    1 => 0,
                    2 => 1,
                    4 => 2,
                    _ => 3,
                };
                if let Some(i) = pill_select(
                    ui,
                    "seq_len",
                    &mut len_sel,
                    &["1 mes.", "2 mes.", "4 mes.", "8 mes."],
                    c.accent_cyan,
                ) {
                    if let Some(b) = [1usize, 2, 4, 8].get(i) {
                        let _ = app
                            .engine
                            .lock()
                            .handle_event(EngineEvent::SetPatternBars(*b));
                    }
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                if icon_button(ui, "◀", false, Some("Mesure précédente")).clicked {
                    let _ = app.engine.lock().handle_event(EngineEvent::PrevBar);
                }
                if icon_button(ui, "▶", false, Some("Mesure suivante")).clicked {
                    let _ = app.engine.lock().handle_event(EngineEvent::NextBar);
                }
            });

            ui.add_space(6.0);

            // ── Actions musicales ──────────────────────────────────────
            ui.horizontal(|ui| {
                if mach_button(ui, "🥁 4-on-Floor", false).clicked {
                    app.fill_four_on_the_floor(0, selected_bar);
                }
                if mach_button(ui, "🎩 8ths Hats", false).clicked {
                    app.fill_eighths(2, selected_bar);
                }
                if mach_button(ui, "✨ Humanize", false).clicked {
                    let _ = app.engine.lock().handle_event(EngineEvent::Variation);
                }
                if mach_button(ui, "📐 Quantize", false).clicked {
                    let _ = app.engine.lock().handle_event(EngineEvent::QuantizeActive);
                }
                if chip(ui, "🗑 Vider la mesure", false, c.error) {
                    app.clear_bar(selected_bar);
                }
                if chip(ui, "♻ Reset 808", false, c.text_dim) {
                    app.reset_demo_groove();
                }
            });

            ui.add_space(8.0);

            // ── Grille ────────────────────────────────────────────────
            crate::gui::theme::card_frame().show(ui, |ui| {
                let row_label_w = 172.0;
                let avail = ui.available_width() - row_label_w - 8.0;
                let cell_w = (avail / 16.0).clamp(20.0, 42.0);
                let cell_h = 21.0;

                // Numéros de pas + repères de temps.
                ui.horizontal(|ui| {
                    ui.allocate_exact_size(egui::vec2(row_label_w, 16.0), egui::Sense::hover());
                    for step in 0..16usize {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(cell_w, 16.0), egui::Sense::hover());
                        let is_beat = step % 4 == 0;
                        let is_play = play_step == Some(step);
                        let col = if is_play {
                            c.flash
                        } else if is_beat {
                            c.text
                        } else {
                            c.text_muted
                        };
                        let label = if is_beat {
                            format!("{}", step / 4 + 1)
                        } else {
                            "·".to_string()
                        };
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            label,
                            egui::FontId::monospace(11.0),
                            col,
                        );
                    }
                });

                for snd in 0..16usize {
                    ui.horizontal(|ui| {
                        // En-tête de ligne : n° + couleur + nom du Sound.
                        let is_focus_sound = snd == snap.active_sound as usize;
                        let (rect, resp) = ui.allocate_exact_size(
                            egui::vec2(row_label_w, cell_h),
                            egui::Sense::click(),
                        );
                        if resp.clicked() {
                            let _ = app
                                .engine
                                .lock()
                                .handle_event(EngineEvent::SelectSound(snd as u8));
                        }
                        let color = pad_color(snd);
                        let dot = egui::Rect::from_center_size(
                            egui::pos2(rect.left() + 10.0, rect.center().y),
                            egui::vec2(10.0, 10.0),
                        );
                        ui.painter()
                            .rect_filled(dot, egui::CornerRadius::same(2), color);
                        let name = snap
                            .project
                            .groups
                            .get(gf)
                            .and_then(|g| g.sounds.get(snd))
                            .and_then(|s| s.as_ref())
                            .map(|s| s.name.as_str())
                            .unwrap_or("—");
                        ui.painter().text(
                            egui::pos2(rect.left() + 22.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            format!("{:02} {name}", snd + 1),
                            egui::FontId::proportional(11.0),
                            if is_focus_sound { c.accent } else { c.text },
                        );
                        if is_focus_sound {
                            ui.painter().rect_stroke(
                                rect,
                                egui::CornerRadius::same(3),
                                egui::Stroke::new(1.0_f32, c.accent),
                                egui::StrokeKind::Inside,
                            );
                        }

                        for step in 0..16usize {
                            let on_beat = step % 4 == 0;
                            let resp = step_cell(
                                ui,
                                egui::vec2(cell_w, cell_h),
                                occupied[snd][step],
                                velocity[snd][step],
                                play_step == Some(step),
                                on_beat,
                                color,
                            );
                            if resp.clicked() {
                                let tick = (selected_bar as u32 * 4 * 960) + (step as u32 * 240);
                                app.toggle_step(snd as u8, tick);
                            }
                        }
                    });
                }
            });

            ui.add_space(10.0);

            // ── Infos sound actif ──────────────────────────────────────
            let active = snap.active_sound as usize;
            let active_name = snap
                .project
                .groups
                .get(gf)
                .and_then(|g| g.sounds.get(active))
                .and_then(|s| s.as_ref())
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "(vide)".to_string());
            section(
                ui,
                &format!(
                    "SOUND ACTIF : {} {}",
                    active + 1,
                    active_name.to_uppercase()
                ),
                |ui| {
                    ui.horizontal(|ui| {
                        if chip(ui, "🧹 Effacer les notes de ce Sound", false, c.error) {
                            let _ = app.engine.lock().handle_event(EngineEvent::EraseSound {
                                group: snap.group_focus,
                                sound: snap.active_sound,
                            });
                        }
                        ui.label(
                            egui::RichText::new(
                                "Astuce : cliquez une cellule pour poser/retirer le pas \
                                 (identique au hardware, la lecture est en temps réel).",
                            )
                            .small()
                            .color(c.text_dim),
                        );
                    });
                },
            );
        });
}
