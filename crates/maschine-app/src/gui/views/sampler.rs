//! Vue SAMPLER — waveform réelle, enveloppe ADSR dessinée, filtre et
//! paramètres du Sound actif (VOL/PAN/CUT/RESO + ADSR), style Maschine.

use super::*;
use crate::gui::app::GuiApp;
use crate::gui::theme::{card_frame, MachColors};
use crate::gui::widgets::{chip, knob, KnobSpec};
use eframe::egui;

use maschine_engine::EngineEvent;

pub fn render(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();
    let gf = snap.group_focus as usize;
    let sound_idx = snap.active_sound as usize;

    let (sound_name, volume_db, pan, cutoff, reso, adsr) = snap
        .project
        .groups
        .get(gf)
        .and_then(|g| g.sounds.get(sound_idx))
        .and_then(|s| s.as_ref())
        .map(|s| {
            (
                s.name.clone(),
                s.volume_db,
                s.pan,
                s.filter.cutoff_hz,
                s.filter.resonance,
                (
                    s.adsr.attack_ms,
                    s.adsr.decay_ms,
                    s.adsr.sustain,
                    s.adsr.release_ms,
                ),
            )
        })
        .unwrap_or_else(|| {
            (
                "(vide)".to_string(),
                0.0,
                0.0,
                20000.0,
                0.0,
                (2.0, 100.0, 0.8, 150.0),
            )
        });

    let waveform = app
        .engine
        .lock()
        .waveform_envelope(snap.group_focus, snap.active_sound, 512);
    let duration = app
        .engine
        .lock()
        .sample_duration_secs(snap.group_focus, snap.active_sound);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let subtitle = format!(
                    "Groupe {} • Sound {:02} — {sound_name}{}",
                    group_char(gf),
                    sound_idx + 1,
                    if duration > 0.0 {
                        format!(" • {duration:.2} s")
                    } else {
                        String::new()
                    }
                );
                crate::gui::theme::view_header(ui, "🎛 SAMPLER", &subtitle);
            });

            ui.add_space(6.0);

            // ── Sélecteur de Sound (16 pads) ──────────────────────────
            ui.horizontal_wrapped(|ui| {
                for i in 0..16usize {
                    let name = snap
                        .project
                        .groups
                        .get(gf)
                        .and_then(|g| g.sounds.get(i))
                        .and_then(|s| s.as_ref())
                        .map(|s| s.name.as_str())
                        .unwrap_or("—");
                    let selected = i == sound_idx;
                    let mut btn = egui::Button::new(
                        egui::RichText::new(format!("{:02}", i + 1))
                            .monospace()
                            .size(11.0)
                            .color(if selected {
                                egui::Color32::BLACK
                            } else {
                                c.text
                            }),
                    )
                    .min_size(egui::vec2(30.0, 26.0))
                    .fill(if selected { pad_color(i) } else { c.widget_bg })
                    .stroke(egui::Stroke::new(1.0_f32, c.border));
                    if selected {
                        btn = btn.stroke(egui::Stroke::new(1.5_f32, c.text));
                    }
                    if ui.add(btn).on_hover_text(name).clicked() {
                        let _ = app
                            .engine
                            .lock()
                            .handle_event(EngineEvent::SelectSound(i as u8));
                    }
                }
            });

            ui.add_space(8.0);

            ui.horizontal_top(|ui| {
                // ── WAVEFORM ──────────────────────────────────────────
                card_frame().show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("WAVEFORM")
                            .strong()
                            .small()
                            .color(c.accent),
                    );
                    ui.add_space(4.0);
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(430.0, 150.0), egui::Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(rect, egui::CornerRadius::same(4), c.bg);
                    painter.hline(
                        rect.left()..=rect.right(),
                        rect.center().y,
                        egui::Stroke::new(1.0_f32, c.border),
                    );
                    match &waveform {
                        Some(env) if !env.is_empty() => {
                            let mid = rect.center().y;
                            let half = rect.height() / 2.0 - 4.0;
                            let dx = rect.width() / env.len() as f32;
                            for (i, (mn, mx)) in env.iter().enumerate() {
                                let x = rect.left() + i as f32 * dx;
                                let y_top = mid - mx.clamp(-1.0, 1.0) * half;
                                let y_bot = mid - mn.clamp(-1.0, 1.0) * half;
                                painter.line_segment(
                                    [egui::pos2(x, y_top), egui::pos2(x, y_bot)],
                                    egui::Stroke::new(
                                        dx.max(1.0),
                                        c.accent_cyan.gamma_multiply(0.9),
                                    ),
                                );
                            }
                        }
                        _ => {
                            painter.text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "Aucun sample — chargez un preset dans le BROWSER",
                                egui::FontId::proportional(12.0),
                                c.text_muted,
                            );
                        }
                    }
                    painter.rect_stroke(
                        rect,
                        egui::CornerRadius::same(4),
                        egui::Stroke::new(1.0_f32, c.border),
                        egui::StrokeKind::Inside,
                    );

                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Waveform du sample chargé (Kit 808 / WAV importé)")
                            .small()
                            .color(c.text_dim),
                    );
                });

                ui.add_space(8.0);

                // ── ADSR ─────────────────────────────────────────────
                card_frame().show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("ENVELOPPE ADSR")
                            .strong()
                            .small()
                            .color(c.accent),
                    );
                    ui.add_space(4.0);
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(250.0, 150.0), egui::Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(rect, egui::CornerRadius::same(4), c.bg);

                    let (a, d, s, r) = adsr;
                    let total = (a + d + r).max(1.0);
                    let a_w = (a / total).clamp(0.05, 0.45) * rect.width();
                    let d_w = (d / total).clamp(0.05, 0.45) * rect.width();
                    let r_w = (r / total).clamp(0.08, 0.5) * rect.width();
                    let s_w = (rect.width() - a_w - d_w - r_w).max(10.0);
                    let top = rect.top() + 10.0;
                    let bottom = rect.bottom() - 10.0;
                    let s_y = bottom - s.clamp(0.0, 1.0) * (bottom - top);

                    let pts = vec![
                        egui::pos2(rect.left(), bottom),
                        egui::pos2(rect.left() + a_w, top),
                        egui::pos2(rect.left() + a_w + d_w, s_y),
                        egui::pos2(rect.left() + a_w + d_w + s_w, s_y),
                        egui::pos2(rect.right(), bottom),
                    ];
                    painter.add(egui::Shape::line(pts, egui::Stroke::new(2.0_f32, c.accent)));
                    painter.rect_stroke(
                        rect,
                        egui::CornerRadius::same(4),
                        egui::Stroke::new(1.0_f32, c.border),
                        egui::StrokeKind::Inside,
                    );
                    painter.text(
                        egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
                        egui::Align2::LEFT_TOP,
                        format!("A {a:.0}ms • D {d:.0}ms • S {:.0}% • R {r:.0}ms", s * 100.0),
                        egui::FontId::monospace(10.0),
                        c.text_dim,
                    );

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if let Some(v) = knob(
                            ui,
                            "adsr_a",
                            a,
                            &KnobSpec::linear("ATTACK", " ms", 0.0, 2000.0, 2.0),
                        ) {
                            send_adsr(app, snap, v, d, s, r);
                        }
                        if let Some(v) = knob(
                            ui,
                            "adsr_d",
                            d,
                            &KnobSpec::linear("DECAY", " ms", 5.0, 2000.0, 100.0),
                        ) {
                            send_adsr(app, snap, a, v, s, r);
                        }
                        if let Some(v) = knob(
                            ui,
                            "adsr_s",
                            s,
                            &KnobSpec::linear("SUSTAIN", " %", 0.0, 1.0, 0.8),
                        ) {
                            send_adsr(app, snap, a, d, v, r);
                        }
                        if let Some(v) = knob(
                            ui,
                            "adsr_r",
                            r,
                            &KnobSpec::linear("RELEASE", " ms", 5.0, 3000.0, 150.0),
                        ) {
                            send_adsr(app, snap, a, d, s, v);
                        }
                    });
                });
            });

            ui.add_space(8.0);

            // ── Paramètres principaux ────────────────────────────────
            card_frame().show(ui, |ui| {
                ui.label(
                    egui::RichText::new("PARAMÈTRES DU SOUND")
                        .strong()
                        .small()
                        .color(c.accent),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if let Some(v) = knob(
                        ui,
                        "snd_vol",
                        volume_db,
                        &KnobSpec::linear("VOLUME", " dB", -60.0, 12.0, 0.0),
                    ) {
                        let _ = app.engine.lock().handle_event(EngineEvent::SoundVolume {
                            group: snap.group_focus,
                            sound: snap.active_sound,
                            volume_db: v,
                        });
                    }
                    if let Some(v) = knob(
                        ui,
                        "snd_pan",
                        pan,
                        &KnobSpec::linear("PAN", "", -1.0, 1.0, 0.0),
                    ) {
                        let _ = app.engine.lock().handle_event(EngineEvent::SoundPan {
                            group: snap.group_focus,
                            sound: snap.active_sound,
                            pan: v,
                        });
                    }
                    if let Some(v) = knob(
                        ui,
                        "snd_cut",
                        cutoff,
                        &KnobSpec::log("CUTOFF", " Hz", 20.0, 20000.0, 20000.0),
                    ) {
                        let _ = app
                            .engine
                            .lock()
                            .handle_event(EngineEvent::SoundFilterCutoff {
                                group: snap.group_focus,
                                sound: snap.active_sound,
                                cutoff_hz: v,
                            });
                    }
                    if let Some(v) = knob(
                        ui,
                        "snd_reso",
                        reso,
                        &KnobSpec::linear("RÉSONANCE", " %", 0.0, 1.0, 0.0),
                    ) {
                        let _ = app
                            .engine
                            .lock()
                            .handle_event(EngineEvent::SoundFilterReso {
                                group: snap.group_focus,
                                sound: snap.active_sound,
                                resonance: v,
                            });
                    }

                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        ui.add_space(6.0);
                        if chip(ui, "▶ Auditionner le Sound", false, c.success) {
                            let _ = app.engine.lock().handle_event(EngineEvent::Pad {
                                pad: snap.active_sound,
                                velocity: 115,
                                pressed: true,
                            });
                        }
                        ui.add_space(4.0);
                        if chip(ui, "🎯 Charger le preset Browser", false, c.accent_cyan) {
                            let preset = snap.browser_preset_idx;
                            let _ = app.engine.lock().handle_event(EngineEvent::LoadDrumPreset {
                                group: snap.group_focus,
                                sound: snap.active_sound,
                                preset_idx: preset,
                            });
                        }
                        ui.add_space(4.0);
                        if chip(ui, "🧹 Effacer les notes (patterns)", false, c.error) {
                            let _ = app.engine.lock().handle_event(EngineEvent::EraseSound {
                                group: snap.group_focus,
                                sound: snap.active_sound,
                            });
                        }
                    });
                });
            });
        });
}

fn send_adsr(
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    attack_ms: f32,
    decay_ms: f32,
    sustain: f32,
    release_ms: f32,
) {
    let _ = app.engine.lock().handle_event(EngineEvent::SoundAdsr {
        group: snap.group_focus,
        sound: snap.active_sound,
        attack_ms,
        decay_ms,
        sustain,
        release_ms,
    });
}
