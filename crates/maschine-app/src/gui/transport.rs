//! Barre de transport haute visibilité (inspirée Maschine MK3 / DAW).
//!
//! Sémantique stricte :
//! - ▶/■ = Play/Stop immédiat (`stop:true` quand on joue, `play:true` sinon) ;
//! - ● REC = bascule enregistrement (REC pendant lecture, retour à Play sinon) ;
//! - ⏮ RESTART = retour au début (tick 0).

use crate::gui::app::GuiApp;
use crate::gui::theme::{caption, value_label, MachColors};
use crate::gui::widgets::{chip, mach_button};
use eframe::egui;

use maschine_engine::transport::TransportState;
use maschine_engine::EngineEvent;

/// Dessine la barre de transport en haut de l'application.
pub fn render_transport(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
) {
    let c = MachColors::dark();
    let is_playing = matches!(
        snap.transport.state,
        TransportState::Playing | TransportState::Recording
    );
    let is_rec = matches!(snap.transport.state, TransportState::Recording);
    let loop_on = snap.transport.loop_enabled;
    let metro_on = snap.transport.metronome;

    egui::Frame::NONE
        .fill(c.panel)
        .inner_margin(egui::Margin::same(10))
        .stroke(egui::Stroke::new(1.0_f32, c.border))
        .corner_radius(egui::CornerRadius::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // ── Transport ────────────────────────────────────────────
                ui.vertical(|ui| {
                    let play_btn = egui::Button::new(
                        egui::RichText::new(if is_playing { "■" } else { "▶" })
                            .size(26.0)
                            .strong(),
                    )
                    .min_size(egui::vec2(52.0, 42.0))
                    .fill(if is_playing { c.error } else { c.success })
                    .stroke(egui::Stroke::new(1.0_f32, c.border));
                    if ui
                        .add(play_btn)
                        .on_hover_text("Lecture / Arrêt (Espace)")
                        .clicked()
                    {
                        let _ = app.engine.lock().handle_event(EngineEvent::Transport {
                            play: !is_playing,
                            rec: false,
                            stop: is_playing,
                        });
                    }
                    ui.add_space(2.0);
                    if mach_button(ui, "⏮", false).clicked {
                        let _ = app.engine.lock().handle_event(EngineEvent::Restart);
                    }
                });

                ui.add_space(4.0);

                ui.vertical(|ui| {
                    let rec_btn =
                        egui::Button::new(egui::RichText::new("● REC").size(13.0).strong().color(
                            if is_rec {
                                egui::Color32::WHITE
                            } else {
                                c.error
                            },
                        ))
                        .min_size(egui::vec2(74.0, 42.0))
                        .fill(if is_rec { c.error } else { c.widget_bg })
                        .stroke(egui::Stroke::new(
                            1.0_f32,
                            if is_rec { c.error } else { c.border },
                        ));
                    if ui.add(rec_btn).on_hover_text("Enregistrement").clicked() {
                        // REC pendant lecture → Recording ; re-clic → retour en Play.
                        let _ = app.engine.lock().handle_event(EngineEvent::Transport {
                            play: is_rec,
                            rec: !is_rec,
                            stop: false,
                        });
                    }
                    ui.add_space(2.0);
                    caption(
                        ui,
                        if is_rec {
                            "STOP REC: re-clic"
                        } else {
                            "Rec au prochain cycle"
                        },
                    );
                });

                ui.add_space(14.0);

                // ── Position ─────────────────────────────────────────────
                ui.vertical(|ui| {
                    value_label(ui, format!("{}", snap.bbt), c.text);
                    let bar_in_pat = snap.selected_bar + 1;
                    caption(
                        ui,
                        format!(
                            "Bar {}/{} • pattern {} mes.",
                            bar_in_pat,
                            snap.pattern_bars.max(1),
                            snap.pattern_bars
                        ),
                    );
                });

                ui.add_space(14.0);

                // ── Tempo ────────────────────────────────────────────────
                ui.vertical(|ui| {
                    let mut bpm = snap.transport.bpm;
                    let resp = ui.add(
                        egui::DragValue::new(&mut bpm)
                            .speed(0.5)
                            .range(20.0..=300.0)
                            .custom_formatter(|v, _| format!("{v:.1} BPM")),
                    );
                    if resp.changed() {
                        let _ = app.engine.lock().handle_event(EngineEvent::Bpm(bpm));
                    }
                    let mut swing = snap.transport.swing;
                    if ui
                        .add(
                            egui::Slider::new(&mut swing, 0.0..=1.0)
                                .show_value(false)
                                .text("Swing"),
                        )
                        .changed()
                    {
                        app.engine.lock().set_swing(swing);
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                // ── Boucle / métronome / song ───────────────────────────
                ui.vertical(|ui| {
                    if chip(ui, "🔁 LOOP", loop_on, c.success) {
                        let _ = app.engine.lock().handle_event(EngineEvent::ToggleLoop);
                    }
                    if chip(ui, "🎵 MÉTRO", metro_on, c.accent_cyan) {
                        let _ = app.engine.lock().handle_event(EngineEvent::ToggleMetronome);
                    }
                    if chip(ui, "🧭 SONG", snap.song_mode, c.accent) {
                        let _ = app.engine.lock().handle_event(EngineEvent::ToggleSongMode);
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                // ── Voix / niveaux ──────────────────────────────────────
                ui.vertical(|ui| {
                    value_label(ui, format!("Voices {:2}", snap.num_voices), c.text_dim);
                    let cpu = app.audio_stats_text();
                    value_label(ui, cpu, c.text_muted);
                });

                // ── Droite : master + réglages ──────────────────────────
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    #[cfg(feature = "audio")]
                    {
                        let (txt, _col) = if app.show_settings {
                            ("⚙ Fermer", c.warning)
                        } else {
                            ("⚙ Audio", c.text)
                        };
                        if mach_button(ui, txt, app.show_settings).clicked {
                            app.show_settings = !app.show_settings;
                            if app.show_settings {
                                app.refresh_devices();
                            }
                        }
                        ui.add_space(8.0);
                    }

                    let mut master = snap.master_volume;
                    let resp = ui.add(
                        egui::Slider::new(&mut master, 0.0..=1.0)
                            .show_value(false)
                            .text("Master"),
                    );
                    if resp.changed() {
                        let _ = app
                            .engine
                            .lock()
                            .handle_event(EngineEvent::MasterVolume(master));
                    }
                    value_label(ui, format!("{:>3.0}%", snap.master_volume * 100.0), c.text);
                });
            });
        });
}
