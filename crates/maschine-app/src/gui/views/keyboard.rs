//! Vue CLAVIER / PERFORMANCE — Keyboard/Chords Maschine-like.
//!
//! Utilise l'état musical du moteur (gamme, fondamentale, octave, accords),
//! joue les notes du son actif via `EngineEvent::KeyboardTrigger`, et expose
//! les contrôles de gamme/accords (équivalents aux boutons haut MK3).

use crate::gui::app::GuiApp;
use crate::gui::theme::{card_frame, MachColors};
use crate::gui::widgets::chip;
use eframe::egui;

use maschine_engine::keyboard::{CHORDS, NOTE_NAMES, SCALES};
use maschine_engine::EngineEvent;

fn note_name(note: u8) -> String {
    let octave = (note as i32 / 12) - 1;
    format!("{}{}", NOTE_NAMES[note as usize % 12], octave)
}

pub fn render(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();
    let gf = snap.group_focus as usize;
    let kb = snap.keyboard;
    let sound_name = snap
        .project
        .groups
        .get(gf)
        .and_then(|g| g.sounds.get(snap.active_sound as usize))
        .and_then(|s| s.as_ref())
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
                    "Joue le son actif dans la gamme/accord sélectionné. Boutons haut = gammes/comportement.",
                );
                ui.colored_label(
                    c.accent_cyan,
                    egui::RichText::new(format!("MODE {mode_label}")).strong().size(11.0),
                );
                if snap.controller_mode != 1 && snap.controller_mode != 2 {
                    ui.label(
                        egui::RichText::new("Activez KEYBOARD/CHORDS sur la MK3 ou via le contrôleur.")
                            .small()
                            .color(c.text_muted),
                    );
                }
            });

            ui.add_space(8.0);

            card_frame().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}  •  Oct {}  •  {}  •  SND {:02} {}",
                            kb.root_name(),
                            kb.scale_def().name,
                            kb.octave,
                            if snap.controller_mode == 2 { kb.chord_def().name } else { "Notes" },
                            snap.active_sound + 1,
                            sound_name
                        ))
                        .strong()
                        .color(c.accent),
                    );
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if chip(ui, "◀ ROOT", false, c.accent_cyan) {
                        app.send(EngineEvent::KeyboardRootDelta(-1));
                    }
                    ui.label(egui::RichText::new(kb.root_name()).strong().color(c.text));
                    if chip(ui, "ROOT ▶", false, c.accent_cyan) {
                        app.send(EngineEvent::KeyboardRootDelta(1));
                    }
                    ui.separator();
                    if chip(ui, "◀ SCALE", false, c.accent_cyan) {
                        app.send(EngineEvent::KeyboardScaleDelta(-1));
                    }
                    ui.label(egui::RichText::new(kb.scale_def().name).strong().color(c.text));
                    if chip(ui, "SCALE ▶", false, c.accent_cyan) {
                        app.send(EngineEvent::KeyboardScaleDelta(1));
                    }
                    ui.separator();
                    if chip(ui, "OCT -", false, c.text_dim) {
                        app.send(EngineEvent::KeyboardOctaveDelta(-1));
                    }
                    ui.label(egui::RichText::new(format!("OCT {}", kb.octave)).strong().color(c.text));
                    if chip(ui, "OCT +", false, c.text_dim) {
                        app.send(EngineEvent::KeyboardOctaveDelta(1));
                    }
                });
                if snap.controller_mode == 2 {
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        if chip(ui, "◀ CHORD", false, c.success) {
                            app.send(EngineEvent::KeyboardChordDelta(-1));
                        }
                        ui.label(egui::RichText::new(kb.chord_def().name).strong().color(c.text));
                        if chip(ui, "CHORD ▶", false, c.success) {
                            app.send(EngineEvent::KeyboardChordDelta(1));
                        }
                        ui.separator();
                        if chip(ui, "INV -", false, c.text_dim) {
                            app.send(EngineEvent::KeyboardInversionDelta(-1));
                        }
                        ui.label(egui::RichText::new(format!("INV {}", kb.inversion)).color(c.text));
                        if chip(ui, "INV +", false, c.text_dim) {
                            app.send(EngineEvent::KeyboardInversionDelta(1));
                        }
                    });
                }
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Gammes :").small().color(c.text_muted));
                    for (i, scale) in SCALES.iter().enumerate() {
                        if chip(ui, scale.name, i == kb.scale as usize, c.accent) {
                            app.send(EngineEvent::KeyboardScaleDelta(i as i32 - kb.scale as i32));
                        }
                    }
                });
                if snap.controller_mode == 2 {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new("Accords :").small().color(c.text_muted));
                        for (i, chord) in CHORDS.iter().enumerate() {
                            if chip(ui, chord.name, i == kb.chord as usize, c.success) {
                                app.send(EngineEvent::KeyboardChordDelta(i as i32 - kb.chord as i32));
                            }
                        }
                    });
                }
            });

            ui.add_space(8.0);

            card_frame().show(ui, |ui| {
                let title = if snap.controller_mode == 2 { "PADS — ACCORDS" } else { "PADS — GAMME / CLAVIER" };
                ui.label(egui::RichText::new(title).strong().small().color(c.accent_cyan));
                ui.add_space(6.0);
                egui::Grid::new("kbd_pads").spacing(egui::vec2(6.0, 6.0)).show(ui, |ui| {
                    for row in 0..4 {
                        for col in 0..4 {
                            let pad = (3 - row) * 4 + col; // bas->haut comme Maschine
                            let (label, notes) = if snap.controller_mode == 2 {
                                let notes = kb.chord_notes(pad as u8);
                                let first = notes.first().copied().unwrap_or(60);
                                (note_name(first), notes)
                            } else {
                                let n = kb.keyboard_note(pad as u8);
                                (note_name(n), vec![n])
                            };
                            let resp = ui.add(
                                egui::Button::new(
                                    egui::RichText::new(format!("P{:02}\n{}", pad + 1, label))
                                        .size(11.0)
                                        .strong()
                                        .color(c.text),
                                )
                                .min_size(egui::vec2(74.0, 52.0))
                                .fill(if snap.pad_activity[pad] > 0.1 { c.widget_bg } else { c.panel }),
                            );
                            if resp.clicked() {
                                for &n in &notes {
                                    app.send(EngineEvent::KeyboardTrigger { note: n, velocity: 108, pressed: true });
                                    app.send(EngineEvent::KeyboardTrigger { note: n, velocity: 0, pressed: false });
                                }
                            }
                        }
                        ui.end_row();
                    }
                });
            });
        });
}
