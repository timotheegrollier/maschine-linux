//! Vue MIXER — groupes A-H, volumes par pad/son du groupe sélectionné,
//! Mute/Solo/pan et master (workflow Maschine).

use super::*;
use crate::gui::app::GuiApp;
use crate::gui::theme::MachColors;
use crate::gui::widgets::{chip, fader_db, pan_control, vu_meter};
use eframe::egui;

use maschine_engine::EngineEvent;

pub fn render(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();
    let focus = snap.group_focus as usize;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                crate::gui::theme::view_header(
                    ui,
                    "MIXER",
                    "Volumes par pad dans chaque groupe (sélectionnez un pad/son), groupes A–H, Mute/Solo/pan.",
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut master = snap.master_volume;
                    if ui
                        .add(
                            egui::Slider::new(&mut master, 0.0..=1.0)
                                .show_value(false)
                                .text("Master"),
                        )
                        .changed()
                    {
                        let _ = app
                            .engine
                            .lock()
                            .handle_event(EngineEvent::MasterVolume(master));
                    }
                    let mut hp = snap.headphone_volume;
                    if ui
                        .add(
                            egui::Slider::new(&mut hp, 0.0..=1.0)
                                .show_value(false)
                                .text("Casque"),
                        )
                        .changed()
                    {
                        let _ = app
                            .engine
                            .lock()
                            .handle_event(EngineEvent::HeadphoneVolume(hp));
                    }
                });
            });

            ui.add_space(8.0);

            // Groupes A-H (niveaux groupe, mute/solo, focus).
            ui.label(egui::RichText::new("GROUPES").strong().color(c.accent_cyan));
            egui::ScrollArea::horizontal().id_salt("mixer_groups").show(ui, |ui| {
                ui.horizontal(|ui| {
                    for g in 0..8usize {
                        group_strip(ui, app, snap, g, g == focus, &c);
                    }
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            // Pads/sons du groupe sélectionné : volume par pad, éditable.
            let group_name = snap
                .project
                .groups
                .get(focus)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| format!("Group {}", focus + 1));
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "PADS / SONS — GROUPE {} {} (volume par pad)",
                        group_char(focus),
                        group_name
                    ))
                    .strong()
                    .color(c.accent),
                );
                ui.label(
                    egui::RichText::new("Cliquez un pad pour le sélectionner, ajustez son volume/pan.")
                        .small()
                        .color(c.text_muted),
                );
            });
            ui.add_space(6.0);

            crate::gui::theme::card_frame().show(ui, |ui| {
                egui::ScrollArea::horizontal().id_salt("mixer_sounds").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for s in 0..16usize {
                            sound_strip(ui, app, snap, focus, s, s == snap.active_sound as usize, &c);
                        }
                    });
                });
            });

            ui.add_space(10.0);
            crate::gui::theme::card_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    vu_meter(
                        ui,
                        [snap.master_level, snap.master_level],
                        [snap.master_peak, snap.master_peak],
                        20.0,
                        90.0,
                    );
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("MASTER").strong().color(c.accent));
                        let master_db = linear_to_db(snap.master_volume);
                        if let Some(db) = fader_db(ui, "master_fader", master_db, 80.0, 34.0) {
                            let master = 10f32.powf(db / 20.0).clamp(0.0, 1.0);
                            let _ = app
                                .engine
                                .lock()
                                .handle_event(EngineEvent::MasterVolume(master));
                        }
                        ui.label(
                            egui::RichText::new(format!("{master_db:>5.1} dB  {:>3.0}%", snap.master_volume * 100.0))
                                .monospace()
                                .small()
                                .color(c.text_dim),
                        );
                    });
                });
            });
        });
}

fn linear_to_db(mul: f32) -> f32 {
    20.0 * mul.max(1.0e-4).log10()
}

fn group_strip(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    g: usize,
    is_focus: bool,
    c: &MachColors,
) {
    let (name, muted) = snap
        .project
        .groups
        .get(g)
        .map(|grp| {
            (
                grp.name.clone(),
                grp.muted || (snap.muted_groups & (1 << g)) != 0,
            )
        })
        .unwrap_or_else(|| (format!("Groupe {}", g + 1), false));
    let soloed = (snap.soloed_groups & (1 << g)) != 0;

    let mut frame = crate::gui::theme::card_frame();
    if is_focus {
        frame = frame.stroke(egui::Stroke::new(1.5_f32, c.accent));
    }
    frame.show(ui, |ui| {
        ui.set_width(74.0);
        ui.vertical_centered(|ui| {
            let letter = group_char(g);
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{letter}"))
                            .strong()
                            .size(15.0)
                            .color(if is_focus { c.accent } else { c.text }),
                    )
                    .fill(if is_focus {
                        c.widget_bg
                    } else {
                        egui::Color32::TRANSPARENT
                    })
                    .stroke(egui::Stroke::NONE)
                    .min_size(egui::vec2(24.0, 22.0)),
                )
                .clicked()
            {
                let _ = app
                    .engine
                    .lock()
                    .handle_event(EngineEvent::GroupFocus(g as u8));
            }
            ui.label(egui::RichText::new(&name).small().color(if is_focus {
                c.text
            } else {
                c.text_dim
            }));
            vu_meter(
                ui,
                [snap.group_levels[g], snap.group_levels[g]],
                [snap.group_peaks[g], snap.group_peaks[g]],
                16.0,
                78.0,
            );
            ui.horizontal(|ui| {
                if chip(ui, "M", muted, c.warning) {
                    let _ = app.engine.lock().handle_event(EngineEvent::Mute(g as u8));
                }
                if chip(ui, "S", soloed, c.accent_cyan) {
                    let _ = app.engine.lock().handle_event(EngineEvent::Solo(g as u8));
                }
            });
        });
    });
}

fn sound_strip(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    group: usize,
    sound: usize,
    is_selected: bool,
    c: &MachColors,
) {
    let (name, volume_db, pan) = snap
        .project
        .groups
        .get(group)
        .and_then(|g| g.sounds.get(sound))
        .and_then(|s| s.as_ref())
        .map(|s| (s.name.clone(), s.volume_db, s.pan))
        .unwrap_or_else(|| (format!("Pad {}", sound + 1), 0.0, 0.0));

    let mut frame = crate::gui::theme::card_frame();
    if is_selected {
        frame = frame.stroke(egui::Stroke::new(2.0_f32, c.accent));
    }
    frame.show(ui, |ui| {
        ui.set_width(80.0);
        ui.vertical_centered(|ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{:02}", sound + 1))
                            .strong()
                            .size(13.0)
                            .color(if is_selected { c.accent } else { c.text }),
                    )
                    .fill(if is_selected {
                        c.widget_bg
                    } else {
                        egui::Color32::TRANSPARENT
                    })
                    .stroke(egui::Stroke::NONE),
                )
                .clicked()
            {
                let _ = app
                    .engine
                    .lock()
                    .handle_event(EngineEvent::SelectSound(sound as u8));
            }
            ui.label(
                egui::RichText::new(truncate(&name, 8))
                    .small()
                    .color(if is_selected { c.text } else { c.text_dim }),
            );
            ui.horizontal(|ui| {
                let lvl = snap.pad_levels.get(sound).copied().unwrap_or(0.0);
                let peak = snap.pad_peaks.get(sound).copied().unwrap_or(0.0);
                vu_meter(ui, [lvl, lvl], [peak, peak], 12.0, 120.0);
                if let Some(v) = fader_db(ui, ("snd_fader", group, sound), volume_db, 120.0, 26.0) {
                    let _ = app.engine.lock().handle_event(EngineEvent::SoundVolume {
                        group: group as u8,
                        sound: sound as u8,
                        volume_db: v,
                    });
                }
            });
            ui.label(
                egui::RichText::new(format!("{volume_db:>5.1}"))
                    .monospace()
                    .small()
                    .color(c.text_dim),
            );
            if let Some(p) = pan_control(ui, ("snd_pan", group, sound), pan, 58.0) {
                let _ = app.engine.lock().handle_event(EngineEvent::SoundPan {
                    group: group as u8,
                    sound: sound as u8,
                    pan: p,
                });
            }
        });
    });
}

fn truncate(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        out.push('…');
    }
    out
}
