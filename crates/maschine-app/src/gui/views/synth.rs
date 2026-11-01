//! Vue SYNTHÉTISEUR ACID TB-303 — Oscillateur PolyBLEP (Saw/Square),
//! Filtre résonant 4 pôles style 303 avec mod d'enveloppe et accent,
//! Distorsion tanh waveshaper avec réglage de tone, et Delay stéréo ping-pong.

use crate::gui::app::GuiApp;
use crate::gui::theme::{card_frame, MachColors};
use crate::gui::widgets::{knob, KnobSpec};
use eframe::egui;

use maschine_engine::{AcidWaveform, EngineEvent};

pub fn render(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();
    let params = &snap.acid_params;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // ── En-tête de la vue & Presets ──────────────────────────
            ui.horizontal(|ui| {
                let wave_str = match params.waveform {
                    AcidWaveform::Saw => "SAWTOOTH",
                    AcidWaveform::Square => "SQUARE",
                };
                let subtitle = format!(
                    "Osc: {wave_str} (Oct {:+}) • Cutoff: {:.0} Hz • Reso: {:.0}% • Drive: {:.0}% • Delay: {:.0}%",
                    params.octave,
                    params.cutoff_hz,
                    params.resonance * 100.0,
                    params.drive * 100.0,
                    params.delay_mix * 100.0
                );
                crate::gui::theme::view_header(ui, "⚡ ACID TB-303", &subtitle);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🌊 Dub Echo").clicked() {
                        let _ = app.engine.lock().handle_event(EngineEvent::AcidSelectPreset(3));
                    }
                    if ui.button("🔥 Distorted").clicked() {
                        let _ = app.engine.lock().handle_event(EngineEvent::AcidSelectPreset(2));
                    }
                    if ui.button("🐍 Squelch").clicked() {
                        let _ = app.engine.lock().handle_event(EngineEvent::AcidSelectPreset(1));
                    }
                    if ui.button("🎹 Classic Acid").clicked() {
                        let _ = app.engine.lock().handle_event(EngineEvent::AcidSelectPreset(0));
                    }
                    ui.label(egui::RichText::new("Presets :").small().color(c.text_dim));
                });
            });

            ui.add_space(8.0);

            // ── Ligne 1 : Oscillateur & Filtre ─────────────────────────
            ui.horizontal_top(|ui| {
                // ── 1. OSCILLATEUR (Saw/Square, Octave, Slide) ──────────
                card_frame().show(ui, |ui| {
                    ui.set_width(320.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("OSCILLATEUR TB-303")
                                .strong()
                                .small()
                                .color(c.accent),
                        );
                    });
                    ui.add_space(4.0);

                    // Forme d'onde interactive
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(310.0, 90.0), egui::Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(rect, egui::CornerRadius::same(4), c.bg);

                    let mid_y = rect.center().y;
                    let amp = 28.0;
                    let cycles = 4.0;
                    let dx = rect.width() / (cycles * 32.0);

                    // Dessin de la forme d'onde active
                    let mut pts = Vec::with_capacity(128);
                    for i in 0..128 {
                        let x = rect.left() + i as f32 * dx;
                        let phase = (i as f32 % 32.0) / 32.0;
                        let y = match params.waveform {
                            AcidWaveform::Saw => mid_y + (0.5 - phase) * amp * 2.0,
                            AcidWaveform::Square => {
                                if phase < 0.5 {
                                    mid_y - amp
                                } else {
                                    mid_y + amp
                                }
                            }
                        };
                        pts.push(egui::pos2(x, y));
                    }
                    painter.add(egui::Shape::line(pts, egui::Stroke::new(2.0_f32, c.accent)));
                    painter.rect_stroke(
                        rect,
                        egui::CornerRadius::same(4),
                        egui::Stroke::new(1.0_f32, c.border),
                        egui::StrokeKind::Inside,
                    );

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let is_saw = params.waveform == AcidWaveform::Saw;
                        let saw_btn = ui.selectable_label(is_saw, "📐 SAW");
                        if saw_btn.clicked() && !is_saw {
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidWaveformToggle);
                        }

                        let is_sqr = params.waveform == AcidWaveform::Square;
                        let sqr_btn = ui.selectable_label(is_sqr, "⎍ SQUARE");
                        if sqr_btn.clicked() && !is_sqr {
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidWaveformToggle);
                        }

                        ui.separator();

                        ui.label(egui::RichText::new("OCT:").small().color(c.text_dim));
                        if ui.button("−").clicked() {
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidOctaveDelta(-1));
                        }
                        ui.label(egui::RichText::new(format!("{:+}", params.octave)).monospace().strong());
                        if ui.button("+").clicked() {
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidOctaveDelta(1));
                        }
                    });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if let Some(v) = knob(
                            ui,
                            "acid_slide",
                            params.slide_time_ms,
                            &KnobSpec::linear("SLIDE", " ms", 0.0, 500.0, 60.0),
                        ) {
                            let mut new_p = params.clone();
                            new_p.slide_time_ms = v;
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidSetParams(new_p));
                        }
                        if let Some(v) = knob(
                            ui,
                            "acid_vol",
                            params.volume_db,
                            &KnobSpec::linear("LEVEL", " dB", -60.0, 12.0, 0.0),
                        ) {
                            let mut new_p = params.clone();
                            new_p.volume_db = v;
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidSetParams(new_p));
                        }
                    });
                });

                ui.add_space(8.0);

                // ── 2. FILTRE RÉSONANT 303 (Cutoff, Reso, Env, Decay, Accent) ──
                card_frame().show(ui, |ui| {
                    ui.set_width(450.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("FILTRE BAS-PAS 4 PÔLES RÉSONANT (303)")
                                .strong()
                                .small()
                                .color(c.accent_cyan),
                        );
                    });
                    ui.add_space(4.0);

                    // Courbe de réponse en fréquence
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(440.0, 90.0), egui::Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(rect, egui::CornerRadius::same(4), c.bg);

                    // Tracé de la courbe passe-bas avec bosse de résonance
                    let norm_cutoff = ((params.cutoff_hz.max(20.0).log10() - 20.0f32.log10())
                        / (10000.0f32.log10() - 20.0f32.log10()))
                        .clamp(0.05, 0.95);
                    let cutoff_x = rect.left() + norm_cutoff * rect.width();

                    let mut curve_pts = Vec::with_capacity(64);
                    let steps = 60;
                    for step in 0..=steps {
                        let fx = rect.left() + (step as f32 / steps as f32) * rect.width();
                        let dist = (fx - cutoff_x) / 35.0;
                        let reso_bump = params.resonance * 32.0 * (-dist * dist).exp();
                        let rolloff = if fx < cutoff_x {
                            0.0
                        } else {
                            ((fx - cutoff_x) / 12.0).powi(2)
                        };
                        let y = (rect.bottom() - 25.0 - reso_bump + rolloff).clamp(rect.top() + 6.0, rect.bottom() - 4.0);
                        curve_pts.push(egui::pos2(fx, y));
                    }
                    painter.add(egui::Shape::line(curve_pts, egui::Stroke::new(2.5_f32, c.accent_cyan)));

                    // Ligne verticale indicatrice de Cutoff
                    painter.vline(
                        cutoff_x,
                        rect.top()..=rect.bottom(),
                        egui::Stroke::new(1.0_f32, c.border_active),
                    );

                    painter.text(
                        egui::pos2(rect.left() + 6.0, rect.top() + 6.0),
                        egui::Align2::LEFT_TOP,
                        format!("Cutoff: {:.0} Hz • Reso: {:.0}%", params.cutoff_hz, params.resonance * 100.0),
                        egui::FontId::monospace(10.0),
                        c.text_dim,
                    );

                    painter.rect_stroke(
                        rect,
                        egui::CornerRadius::same(4),
                        egui::Stroke::new(1.0_f32, c.border),
                        egui::StrokeKind::Inside,
                    );

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if let Some(v) = knob(
                            ui,
                            "acid_cutoff",
                            params.cutoff_hz,
                            &KnobSpec::log("CUTOFF", " Hz", 20.0, 10000.0, 500.0),
                        ) {
                            let mut new_p = params.clone();
                            new_p.cutoff_hz = v;
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidSetParams(new_p));
                        }
                        if let Some(v) = knob(
                            ui,
                            "acid_reso",
                            params.resonance,
                            &KnobSpec::linear("RESONANCE", " %", 0.0, 1.0, 0.5),
                        ) {
                            let mut new_p = params.clone();
                            new_p.resonance = v;
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidSetParams(new_p));
                        }
                        if let Some(v) = knob(
                            ui,
                            "acid_envmod",
                            params.env_mod,
                            &KnobSpec::linear("ENV MOD", " %", 0.0, 1.0, 0.7),
                        ) {
                            let mut new_p = params.clone();
                            new_p.env_mod = v;
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidSetParams(new_p));
                        }
                        if let Some(v) = knob(
                            ui,
                            "acid_decay",
                            params.decay_ms,
                            &KnobSpec::linear("DECAY", " ms", 20.0, 2000.0, 300.0),
                        ) {
                            let mut new_p = params.clone();
                            new_p.decay_ms = v;
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidSetParams(new_p));
                        }
                        if let Some(v) = knob(
                            ui,
                            "acid_accent",
                            params.accent,
                            &KnobSpec::linear("ACCENT", " %", 0.0, 1.0, 0.5),
                        ) {
                            let mut new_p = params.clone();
                            new_p.accent = v;
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidSetParams(new_p));
                        }
                    });
                });
            });

            ui.add_space(8.0);

            // ── Ligne 2 : Distorsion & Delay ───────────────────────────
            ui.horizontal_top(|ui| {
                // ── 3. DISTORSION TANH (Drive, Tone, Mix) ──────────────
                card_frame().show(ui, |ui| {
                    ui.set_width(380.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("DISTORSION ANALOGIQUE (WAVESHAPER)")
                                .strong()
                                .small()
                                .color(c.warning),
                        );
                    });
                    ui.add_space(4.0);

                    // Courbe de transfert tanh
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(370.0, 80.0), egui::Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(rect, egui::CornerRadius::same(4), c.bg);

                    let mid_x = rect.center().x;
                    let mid_y = rect.center().y;
                    let w_half = rect.width() / 2.0 - 10.0;
                    let h_half = rect.height() / 2.0 - 8.0;

                    let gain = 1.0 + params.drive * 12.0;
                    let mut shaper_pts = Vec::with_capacity(40);
                    for i in -20..=20 {
                        let norm_in = i as f32 / 20.0;
                        let sat_out = (norm_in * gain).tanh();
                        let px = mid_x + norm_in * w_half;
                        let py = mid_y - sat_out * h_half;
                        shaper_pts.push(egui::pos2(px, py));
                    }
                    painter.add(egui::Shape::line(shaper_pts, egui::Stroke::new(2.0_f32, c.warning)));
                    painter.rect_stroke(
                        rect,
                        egui::CornerRadius::same(4),
                        egui::Stroke::new(1.0_f32, c.border),
                        egui::StrokeKind::Inside,
                    );

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if let Some(v) = knob(
                            ui,
                            "acid_drive",
                            params.drive,
                            &KnobSpec::linear("DRIVE", " %", 0.0, 1.0, 0.0),
                        ) {
                            let mut new_p = params.clone();
                            new_p.drive = v;
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidSetParams(new_p));
                        }
                        if let Some(v) = knob(
                            ui,
                            "acid_tone",
                            params.drive_tone,
                            &KnobSpec::linear("TONE", " %", 0.0, 1.0, 0.5),
                        ) {
                            let mut new_p = params.clone();
                            new_p.drive_tone = v;
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidSetParams(new_p));
                        }
                        if let Some(v) = knob(
                            ui,
                            "acid_dist_mix",
                            params.drive_mix,
                            &KnobSpec::linear("DRY/WET", " %", 0.0, 1.0, 1.0),
                        ) {
                            let mut new_p = params.clone();
                            new_p.drive_mix = v;
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidSetParams(new_p));
                        }
                    });
                });

                ui.add_space(8.0);

                // ── 4. DELAY STÉRÉO (Time, Feedback, Mix) ─────────────
                card_frame().show(ui, |ui| {
                    ui.set_width(390.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("DELAY STÉRÉO PING-PONG")
                                .strong()
                                .small()
                                .color(c.success),
                        );
                    });
                    ui.add_space(4.0);

                    // Aperçu visuel des échos
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(380.0, 80.0), egui::Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(rect, egui::CornerRadius::same(4), c.bg);

                    // Échos barres répétées
                    let echoes = 7;
                    let bar_w = 6.0;
                    let spacing = rect.width() / (echoes as f32 + 1.0);
                    let mut echo_amp = 1.0;
                    for e in 0..echoes {
                        let bx = rect.left() + 20.0 + e as f32 * spacing;
                        let bh = echo_amp * (rect.height() - 20.0);
                        let by = rect.bottom() - 10.0 - bh;
                        let bar_rect = egui::Rect::from_min_size(egui::pos2(bx, by), egui::vec2(bar_w, bh));
                        let color = if e % 2 == 0 { c.success } else { c.accent_cyan };
                        painter.rect_filled(bar_rect, egui::CornerRadius::same(2), color);
                        echo_amp *= params.delay_feedback.clamp(0.1, 0.95);
                    }
                    painter.rect_stroke(
                        rect,
                        egui::CornerRadius::same(4),
                        egui::Stroke::new(1.0_f32, c.border),
                        egui::StrokeKind::Inside,
                    );

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if let Some(v) = knob(
                            ui,
                            "acid_dly_time",
                            params.delay_time_ms,
                            &KnobSpec::linear("TIME", " ms", 10.0, 1000.0, 250.0),
                        ) {
                            let mut new_p = params.clone();
                            new_p.delay_time_ms = v;
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidSetParams(new_p));
                        }
                        if let Some(v) = knob(
                            ui,
                            "acid_dly_fb",
                            params.delay_feedback,
                            &KnobSpec::linear("FEEDBACK", " %", 0.0, 0.95, 0.4),
                        ) {
                            let mut new_p = params.clone();
                            new_p.delay_feedback = v;
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidSetParams(new_p));
                        }
                        if let Some(v) = knob(
                            ui,
                            "acid_dly_mix",
                            params.delay_mix,
                            &KnobSpec::linear("MIX", " %", 0.0, 1.0, 0.25),
                        ) {
                            let mut new_p = params.clone();
                            new_p.delay_mix = v;
                            let _ = app.engine.lock().handle_event(EngineEvent::AcidSetParams(new_p));
                        }
                    });
                });
            });

            ui.add_space(8.0);

            // ── 5. CLAVIER TEST ACID (Pads / Touches) ───────────────────
            card_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("🎹 CLAVIER VIRTUEL ACID (C1 — D#2)")
                            .strong()
                            .small()
                            .color(c.accent),
                    );
                    ui.label(egui::RichText::new("— Clique sur une touche pour déclencher l'Acid Synth").small().color(c.text_dim));
                });
                ui.add_space(6.0);

                let notes = [
                    (36, "C1", false),
                    (37, "C#1", true),
                    (38, "D1", false),
                    (39, "D#1", true),
                    (40, "E1", false),
                    (41, "F1", false),
                    (42, "F#1", true),
                    (43, "G1", false),
                    (44, "G#1", true),
                    (45, "A1", false),
                    (46, "A#1", true),
                    (47, "B1", false),
                    (48, "C2", false),
                    (49, "C#2", true),
                    (50, "D2", false),
                    (51, "D#2", true),
                ];

                ui.horizontal(|ui| {
                    for (pad_idx, (_midi_note, name, is_sharp)) in notes.iter().enumerate() {
                        let btn_w = if *is_sharp { 38.0 } else { 44.0 };
                        let btn_h = 50.0;
                        let base_col = if *is_sharp {
                            egui::Color32::from_rgb(30, 32, 40)
                        } else {
                            egui::Color32::from_rgb(220, 222, 230)
                        };
                        let text_col = if *is_sharp {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::BLACK
                        };

                        let btn = egui::Button::new(
                            egui::RichText::new(*name).size(11.0).color(text_col).strong(),
                        )
                        .min_size(egui::vec2(btn_w, btn_h))
                        .fill(base_col)
                        .stroke(egui::Stroke::new(1.0_f32, c.border));

                        let response = ui.add(btn);
                        if response.clicked() {
                            let _ = app.engine.lock().handle_event(EngineEvent::Pad {
                                pad: pad_idx as u8,
                                velocity: 110,
                                pressed: true,
                            });
                        }
                    }
                });
            });
        });
}
