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
            if snap.pattern_mode {
                crate::gui::theme::mode_banner(
                    ui,
                    "MODE PATTERN ACTIF — les 16 pads sélectionnent les patterns du groupe. \
                     Appuie à nouveau sur PATTERN pour revenir au comportement par défaut.",
                    c.accent,
                );
                ui.add_space(6.0);
            }
            if snap.controller_mode == 3 {
                crate::gui::theme::mode_banner(
                    ui,
                    "MODE STEP ACTIF — les pads écrivent les pas du Sound actif. \
                     Appuie à nouveau sur STEP pour revenir au mode PAD.",
                    c.warning,
                );
                ui.add_space(6.0);
            }

            // ── Gestionnaire de patterns (création / duplication / édition) ──
            render_pattern_manager(ui, app, snap, &c, gf, pattern_index);
            ui.add_space(6.0);

            // ── Barre d'outils ─────────────────────────────────────────
            ui.horizontal_wrapped(|ui| {
                crate::gui::theme::view_header(ui, "SÉQUENCEUR", "");
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
            ui.horizontal_wrapped(|ui| {
                if mach_button(ui, "4-ON-FLOOR", false).clicked {
                    app.fill_four_on_the_floor(0, selected_bar);
                }
                if mach_button(ui, "8ths Hats", false).clicked {
                    app.fill_eighths(2, selected_bar);
                }
                if mach_button(ui, "Humanize", false).clicked {
                    let _ = app.engine.lock().handle_event(EngineEvent::Variation);
                }
                if mach_button(ui, "QUANTIZE", false).clicked {
                    let _ = app.engine.lock().handle_event(EngineEvent::QuantizeActive);
                }
                if chip(ui, "VIDER LA MESURE", false, c.error) {
                    app.clear_bar(selected_bar);
                }
                if chip(ui, "Reset 808", false, c.text_dim) {
                    app.reset_demo_groove();
                }
            });

            ui.add_space(8.0);

            // ── Grille ────────────────────────────────────────────────
            crate::gui::theme::card_frame().show(ui, |ui| {
                let full_w = ui.available_width();
                let row_label_w = (full_w * 0.24).clamp(112.0, 172.0);
                let avail = (full_w - row_label_w - 8.0).max(16.0 * 14.0);
                let cell_w = (avail / 16.0).clamp(14.0, 42.0);
                let cell_h = if full_w < 560.0 { 19.0 } else { 21.0 };

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

            // ── Éditeur du pas courant (mode STEP) ────────────────────
            if snap.controller_mode == 3 {
                ui.add_space(8.0);
                let active = snap.active_sound;
                let step_in_bar = snap.step_cursor % 16;
                let tick = (selected_bar as u32 * 3840) + (step_in_bar as u32 * 240);
                let current = events
                    .iter()
                    .find(|e| e.pad == active && e.tick >= tick && e.tick < tick + 240);
                crate::gui::theme::card_frame().show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "ÉDITION STEP — pas {:02} • Sound {:02}",
                                snap.step_cursor + 1,
                                active + 1
                            ))
                            .strong()
                            .small()
                            .color(c.warning),
                        );
                        if let Some(ev) = current {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Vél {} • Pitch {} • Longueur {} ticks",
                                    ev.velocity, ev.pitch, ev.length_ticks
                                ))
                                .small()
                                .color(c.text_dim),
                            );
                        } else {
                            ui.label(egui::RichText::new("Pas vide").small().color(c.text_muted));
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        if chip(ui, "◀ PAS", false, c.text_dim) {
                            let _ = app.engine.lock().handle_event(EngineEvent::StepScroll(-1));
                        }
                        if chip(ui, "PAS ▶", false, c.text_dim) {
                            let _ = app.engine.lock().handle_event(EngineEvent::StepScroll(1));
                        }
                        if chip(ui, "◀ BAR", false, c.text_dim) {
                            let _ = app.engine.lock().handle_event(EngineEvent::StepScroll(-16));
                        }
                        if chip(ui, "BAR ▶", false, c.text_dim) {
                            let _ = app.engine.lock().handle_event(EngineEvent::StepScroll(16));
                        }
                        ui.separator();
                        if chip(ui, "VÉL −", false, c.accent_cyan) {
                            let _ = app
                                .engine
                                .lock()
                                .handle_event(EngineEvent::StepVelocityDelta(-5));
                        }
                        if chip(ui, "VÉL +", false, c.accent_cyan) {
                            let _ = app
                                .engine
                                .lock()
                                .handle_event(EngineEvent::StepVelocityDelta(5));
                        }
                        if chip(ui, "PITCH −", false, c.accent) {
                            let _ = app
                                .engine
                                .lock()
                                .handle_event(EngineEvent::StepPitchDelta(-1));
                        }
                        if chip(ui, "PITCH +", false, c.accent) {
                            let _ = app
                                .engine
                                .lock()
                                .handle_event(EngineEvent::StepPitchDelta(1));
                        }
                        if chip(ui, "LONG −", false, c.success) {
                            let _ = app
                                .engine
                                .lock()
                                .handle_event(EngineEvent::StepLengthDelta(-1));
                        }
                        if chip(ui, "LONG +", false, c.success) {
                            let _ = app
                                .engine
                                .lock()
                                .handle_event(EngineEvent::StepLengthDelta(1));
                        }
                        if chip(
                            ui,
                            if current.is_some() {
                                "SUPPRIMER LE PAS"
                            } else {
                                "AJOUTER LE PAS"
                            },
                            current.is_some(),
                            c.error,
                        ) {
                            app.toggle_step(active, tick);
                        }
                    });
                });
            }

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
                    ui.horizontal_wrapped(|ui| {
                        if chip(ui, "EFFACER LES NOTES DE CE SOUND", false, c.error) {
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

/// Gestionnaire de patterns : sélection, création, duplication, suppression,
/// effacement, longueur et renommage. Toujours visible dans le séquenceur,
/// mis en avant par le mode PATTERN hardware.
fn render_pattern_manager(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    c: &MachColors,
    group_idx: usize,
    active_pattern: usize,
) {
    let group = snap.project.groups.get(group_idx);
    let patterns = group.map(|g| g.patterns.as_slice()).unwrap_or(&[]);
    let count = patterns.len();
    let current_name = patterns
        .get(active_pattern)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "—".to_string());
    let current_bars = patterns
        .get(active_pattern)
        .map(|p| p.length.bars.max(1) as usize)
        .unwrap_or(2);
    let current_events = patterns
        .get(active_pattern)
        .map(|p| p.events.len())
        .unwrap_or(0);

    // Synchronise le buffer de renommage quand le pattern actif change.
    let target = Some((snap.group_focus, active_pattern));
    if app.pattern_name_target != target {
        app.pattern_name_target = target;
        app.pattern_name_buf = current_name.clone();
    }

    crate::gui::theme::card_frame().show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(
                c.accent,
                egui::RichText::new("PATTERNS").strong().size(12.5),
            );
            ui.label(
                egui::RichText::new(format!(
                    "Groupe {} • {count} pattern(s) • actif P{:02} « {} » • {current_bars} mes. • {current_events} événements",
                    group_label(group_idx),
                    active_pattern + 1,
                    current_name,
                ))
                .small()
                .color(c.text_dim),
            );
            ui.separator();
            if chip(ui, "◀ PAT", false, c.accent_cyan) {
                app.send(EngineEvent::StepPattern {
                    group: snap.group_focus,
                    delta: -1,
                });
            }
            if chip(ui, "PAT ▶", false, c.accent_cyan) {
                app.send(EngineEvent::StepPattern {
                    group: snap.group_focus,
                    delta: 1,
                });
            }
            if chip(ui, "+ NOUVEAU", false, c.success) {
                app.send(EngineEvent::AddPattern {
                    group: snap.group_focus,
                });
            }
        });

        ui.add_space(4.0);
        egui::ScrollArea::horizontal()
            .id_salt("pattern_slots")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if patterns.is_empty() {
                        ui.label(
                            egui::RichText::new("Aucun pattern — clique sur + NOUVEAU.")
                                .small()
                                .color(c.text_muted),
                        );
                    }
                    for (idx, pat) in patterns.iter().enumerate().take(64) {
                        let label = format!("P{:02} {}", idx + 1, short_name(&pat.name, 14));
                        let active = idx == active_pattern;
                        if chip(ui, &label, active, c.accent) {
                            app.send(EngineEvent::PatternSelect {
                                group: snap.group_focus,
                                pattern: idx,
                            });
                        }
                    }
                });
            });

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if chip(ui, "DUPLIQUER", false, c.accent_cyan) && count > 0 {
                app.send(EngineEvent::DuplicatePattern {
                    group: snap.group_focus,
                    pattern: active_pattern,
                });
            }
            if chip(ui, "VIDER", false, c.warning) && count > 0 {
                app.send(EngineEvent::ClearPattern {
                    group: snap.group_focus,
                    pattern: active_pattern,
                });
            }
            if chip(ui, "SUPPRIMER", false, c.error) && count > 1 {
                app.send(EngineEvent::DeletePattern {
                    group: snap.group_focus,
                    pattern: active_pattern,
                });
            }
            ui.separator();
            ui.label(
                egui::RichText::new("LONGUEUR")
                    .small()
                    .strong()
                    .color(c.text_muted),
            );
            for bars in [1usize, 2, 4, 8, 16] {
                if chip(
                    ui,
                    &format!("{bars} mes."),
                    current_bars == bars,
                    c.accent_cyan,
                ) {
                    app.send(EngineEvent::SetPatternBars(bars));
                }
            }
            ui.separator();
            let edit = ui.add(
                egui::TextEdit::singleline(&mut app.pattern_name_buf)
                    .desired_width(150.0)
                    .hint_text("Nom du pattern"),
            );
            let rename = chip(ui, "RENOMMER", false, c.success)
                || (edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            if rename && count > 0 {
                app.send(EngineEvent::RenamePattern {
                    group: snap.group_focus,
                    pattern: active_pattern,
                    name: app.pattern_name_buf.clone(),
                });
            }
        });
    });
}

fn group_label(group_idx: usize) -> char {
    (b'A' + (group_idx.min(25) as u8)) as char
}

fn short_name(name: &str, max_chars: usize) -> String {
    let mut s: String = name.chars().take(max_chars).collect();
    if name.chars().count() > max_chars {
        s.push('…');
    }
    s
}
