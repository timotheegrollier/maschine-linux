//! Vue MIXER — 8 tranches de Groupe + Master (style console Maschine).
//!
//! Faders dB réels (-60..+12), pan bipolaire, M/S, VU-mètres temps réel
//! (crêtes mesurées par l'Engine, peak hold) exactement comme les écrans MK3.

use super::*;
use crate::gui::app::GuiApp;
use crate::gui::theme::MachColors;
use crate::gui::widgets::{chip, fader_db, pan_control, vu_meter};
use eframe::egui;

use maschine_engine::EngineEvent;

pub fn render(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();
    let focus = snap.group_focus as usize;

    ui.horizontal(|ui| {
        crate::gui::theme::view_header(
            ui,
            "🎚 MIXER",
            "Faders par Groupe (A–H) — VU-mètres temps réel, Mute/Solo, pan.",
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut master = snap.master_volume;
            if ui
                .add(
                    egui::Slider::new(&mut master, 0.0..=1.0)
                        .show_value(false)
                        .text("Master volume"),
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

    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal(|ui| {
            for g in 0..8usize {
                strip(ui, app, snap, g, g == focus);
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // ── Tranche Master ────────────────────────────────────────
            crate::gui::theme::card_frame().show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("MASTER")
                            .strong()
                            .size(13.0)
                            .color(c.accent),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        vu_meter(
                            ui,
                            [snap.master_level, snap.master_level],
                            [snap.master_peak, snap.master_peak],
                            20.0,
                            170.0,
                        );
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            let master_db = linear_to_db(snap.master_volume);
                            if let Some(db) = fader_db(ui, "master_fader", master_db, 150.0, 34.0) {
                                let master = 10f32.powf(db / 20.0).clamp(0.0, 1.0);
                                let _ = app
                                    .engine
                                    .lock()
                                    .handle_event(EngineEvent::MasterVolume(master));
                            }
                            ui.label(
                                egui::RichText::new(format!("{master_db:>5.1} dB"))
                                    .monospace()
                                    .small()
                                    .color(c.text_dim),
                            );
                        });
                    });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!("{:>3.0} %", snap.master_volume * 100.0))
                            .monospace()
                            .color(c.text),
                    );
                });
            });
        });
    });
}

fn linear_to_db(mul: f32) -> f32 {
    20.0 * mul.max(1.0e-4).log10()
}

fn strip(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    g: usize,
    is_focus: bool,
) {
    let c = MachColors::dark();
    let (name, volume_db, pan, muted) = snap
        .project
        .groups
        .get(g)
        .map(|grp| {
            (
                grp.name.clone(),
                grp.volume_db,
                grp.pan,
                grp.muted || (snap.muted_groups & (1 << g)) != 0,
            )
        })
        .unwrap_or_else(|| (format!("Groupe {}", g + 1), 0.0, 0.0, false));
    let soloed = (snap.soloed_groups & (1 << g)) != 0;

    let mut frame = crate::gui::theme::card_frame();
    if is_focus {
        frame = frame.stroke(egui::Stroke::new(1.5_f32, c.accent));
    }
    frame.show(ui, |ui| {
        ui.set_width(96.0);
        ui.vertical_centered(|ui| {
            // En-tête groupe (clic = focus).
            let letter = group_char(g);
            let head = ui.add(
                egui::Button::new(
                    egui::RichText::new(format!("{letter}"))
                        .strong()
                        .size(16.0)
                        .color(if is_focus { c.accent } else { c.text }),
                )
                .fill(if is_focus {
                    c.widget_bg
                } else {
                    egui::Color32::TRANSPARENT
                })
                .stroke(egui::Stroke::NONE)
                .min_size(egui::vec2(26.0, 24.0)),
            );
            if head.clicked() {
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

            ui.add_space(4.0);

            // Mètre + fader.
            ui.horizontal(|ui| {
                vu_meter(
                    ui,
                    [snap.group_levels[g], snap.group_levels[g]],
                    [snap.group_peaks[g], snap.group_peaks[g]],
                    18.0,
                    150.0,
                );
                ui.add_space(2.0);
                ui.vertical(|ui| {
                    if let Some(v) = fader_db(ui, ("grp_fader", g), volume_db, 150.0, 34.0) {
                        let _ = app.engine.lock().handle_event(EngineEvent::GroupVolume {
                            group: g as u8,
                            volume_db: v,
                        });
                    }
                });
            });

            ui.label(
                egui::RichText::new(format!("{volume_db:>5.1} dB"))
                    .monospace()
                    .small()
                    .color(c.text_dim),
            );

            ui.add_space(4.0);

            // Pan bipolaire.
            if let Some(p) = pan_control(ui, ("grp_pan", g), pan, 88.0) {
                let _ = app.engine.lock().handle_event(EngineEvent::GroupPan {
                    group: g as u8,
                    pan: p,
                });
            }
            let pan_txt = if pan.abs() < 0.02 {
                "C".to_string()
            } else if pan < 0.0 {
                format!("L{:.0}", -pan * 100.0)
            } else {
                format!("R{:.0}", pan * 100.0)
            };
            ui.label(egui::RichText::new(pan_txt).small().color(c.text_muted));

            ui.add_space(4.0);

            // Mute / Solo.
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
