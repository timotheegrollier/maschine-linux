//! Vue CLAVIER / PERFORMANCE — miroir du mode Keyboard/Chords du hardware.
//!
//! Elle sert de guide de jeu et de contrôle du Sound actif : les 16 pads
//! matériels jouent la gamme mineure, tandis que cette vue affiche les notes,
//! l'état des menus latchés et permet de déclencher/auditionner le Sound.

use super::*;
use crate::gui::app::GuiApp;
use crate::gui::theme::{card_frame, MachColors};
use crate::gui::widgets::chip;
use eframe::egui;

use maschine_engine::EngineEvent;

/// Gamme mineure naturelle utilisée par le mapper Keyboard (C3..D5).
const SCALE_MINOR: [(&str, u8); 16] = [
    ("C3", 48),
    ("D3", 50),
    ("D#3", 51),
    ("F3", 53),
    ("G3", 55),
    ("G#3", 56),
    ("A#3", 58),
    ("C4", 60),
    ("D4", 62),
    ("D#4", 63),
    ("F4", 65),
    ("G4", 67),
    ("G#4", 68),
    ("A#4", 70),
    ("C5", 72),
    ("D5", 74),
];

pub fn render(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();
    let gf = snap.group_focus as usize;
    let sound = snap
        .project
        .groups
        .get(gf)
        .and_then(|g| g.sounds.get(snap.active_sound as usize))
        .and_then(|s| s.as_ref());
    let sound_name = sound
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "(vide)".to_string());
    let mode_label = match snap.controller_mode {
        1 => "KEYBOARD",
        2 => "CHORDS",
        3 => "STEP",
        _ => "PAD",
    };

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                crate::gui::theme::view_header(
                    ui,
                    "CLAVIER / PERFORMANCE",
                    "Joue le Sound actif, visualise la gamme et les menus actifs.",
                );
                ui.add_space(8.0);
                ui.colored_label(
                    c.accent_cyan,
                    egui::RichText::new(format!("MODE {mode_label}"))
                        .strong()
                        .size(11.0),
                );
                if snap.pattern_mode {
                    ui.colored_label(c.accent, egui::RichText::new("● PATTERN").small());
                }
                if snap.scene_mode {
                    ui.colored_label(c.success, egui::RichText::new("● SCENE").small());
                }
            });

            ui.add_space(8.0);

            card_frame().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "GROUPE {} • SOUND {:02} — {}",
                            group_char(gf),
                            snap.active_sound + 1,
                            sound_name.to_uppercase()
                        ))
                        .strong()
                        .color(c.accent),
                    );
                    ui.add_space(8.0);
                    if chip(ui, "▶ AUDITIONNER", false, c.success) {
                        let _ = app.engine.lock().handle_event(EngineEvent::Pad {
                            pad: snap.active_sound,
                            velocity: 112,
                            pressed: true,
                        });
                    }
                    if chip(ui, "◀ SOUND PRÉCÉDENT", false, c.text_dim) {
                        let idx = (snap.active_sound + 15) % 16;
                        let _ = app
                            .engine
                            .lock()
                            .handle_event(EngineEvent::SelectSound(idx));
                    }
                    if chip(ui, "SOUND SUIVANT ▶", false, c.text_dim) {
                        let idx = (snap.active_sound + 1) % 16;
                        let _ = app
                            .engine
                            .lock()
                            .handle_event(EngineEvent::SelectSound(idx));
                    }
                });
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Astuce : maintiens KEYBOARD sur le hardware pour jouer la gamme, \
                         puis appuie à nouveau dessus pour revenir au mode PAD.",
                    )
                    .small()
                    .color(c.text_dim),
                );
            });

            ui.add_space(8.0);

            // ── Clavier visuel 16 touches ──────────────────────────
            card_frame().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("GAMME MINEURE — 16 PADS")
                            .strong()
                            .small()
                            .color(c.accent_cyan),
                    );
                    ui.label(
                        egui::RichText::new("(les touches blanches sont les fondamentales)")
                            .small()
                            .color(c.text_muted),
                    );
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for (index, (name, midi)) in SCALE_MINOR.iter().enumerate() {
                        let sharp = name.contains('#');
                        let fill = if sharp {
                            egui::Color32::from_rgb(30, 32, 40)
                        } else {
                            egui::Color32::from_rgb(218, 220, 228)
                        };
                        let text = if sharp {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::BLACK
                        };
                        let btn = egui::Button::new(
                            egui::RichText::new(*name).strong().size(11.0).color(text),
                        )
                        .min_size(egui::vec2(48.0, 48.0))
                        .fill(fill)
                        .stroke(egui::Stroke::new(
                            1.0_f32,
                            if index == snap.active_sound as usize {
                                c.accent
                            } else {
                                c.border
                            },
                        ));
                        if ui
                            .add(btn)
                            .on_hover_text(format!(
                                "Pad {} • note MIDI {} — déclenche le Sound actif",
                                index + 1,
                                midi
                            ))
                            .clicked()
                        {
                            let _ = app.engine.lock().handle_event(EngineEvent::Pad {
                                pad: snap.active_sound,
                                velocity: 108,
                                pressed: true,
                            });
                        }
                    }
                });
            });

            ui.add_space(8.0);

            // ── Pads de performance du groupe ───────────────────────
            card_frame().show(ui, |ui| {
                ui.label(
                    egui::RichText::new("PADS DU GROUPE — clic pour jouer / sélectionner")
                        .strong()
                        .small()
                        .color(c.accent),
                );
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    for i in 0..16usize {
                        let name = snap
                            .project
                            .groups
                            .get(gf)
                            .and_then(|g| g.sounds.get(i))
                            .and_then(|s| s.as_ref())
                            .map(|s| s.name.clone())
                            .unwrap_or_else(|| "—".to_string());
                        let selected = i == snap.active_sound as usize;
                        let btn = egui::Button::new(
                            egui::RichText::new(format!("{:02}\n{}", i + 1, name))
                                .size(10.0)
                                .color(if selected {
                                    egui::Color32::BLACK
                                } else {
                                    c.text
                                }),
                        )
                        .min_size(egui::vec2(68.0, 46.0))
                        .fill(if selected {
                            crate::gui::views::pad_color(i)
                        } else {
                            c.widget_bg
                        })
                        .stroke(egui::Stroke::new(1.0_f32, c.border));
                        if ui.add(btn).clicked() {
                            let _ = app
                                .engine
                                .lock()
                                .handle_event(EngineEvent::SelectSound(i as u8));
                            let _ = app.engine.lock().handle_event(EngineEvent::Pad {
                                pad: i as u8,
                                velocity: 110,
                                pressed: true,
                            });
                        }
                    }
                });
            });
        });
}
