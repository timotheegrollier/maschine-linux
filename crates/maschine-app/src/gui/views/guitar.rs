//! Vue « Guitar Amp » : préampli multi-modèles, monitoring MK3, accordeur,
//! analyseur de spectre et diagnostics temps réel.

use eframe::egui;
use maschine_dsp::{AmpModel, CabModel};

use crate::gui::theme::{card_frame, view_header, MachColors};
use crate::gui::widgets::{chip, knob, pill_select, vu_meter, KnobSpec};
use crate::guitar::GuitarParam;

/// Rend la vue ampli guitare dans le panneau central.
pub fn render(
    ui: &mut egui::Ui,
    app: &mut crate::gui::app::GuiApp,
    _snap: &maschine_engine::EngineSnapshot,
) {
    let c = MachColors::dark();
    ui.vertical(|ui| {
        view_header(
            ui,
            "AMPLI GUITARE",
            "Guitare → jack IN MK3 → préamp à lampes → cabine → sortie",
        );
        ui.add_space(6.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            render_controls(ui, app, &c);
            ui.add_space(8.0);
            render_analysis(ui, app, &c);
            ui.add_space(8.0);
            render_diagnostics(ui, app, &c);
        });
    });
}

fn render_controls(ui: &mut egui::Ui, app: &mut crate::gui::app::GuiApp, c: &MachColors) {
    card_frame().show(ui, |ui| {
        // État du monitoring.
        ui.horizontal_wrapped(|ui| {
            let enabled = app
                .guitar
                .control
                .enabled
                .load(std::sync::atomic::Ordering::Relaxed);
            let monitor = app
                .guitar
                .control
                .monitor
                .load(std::sync::atomic::Ordering::Relaxed);
            let bypass = app
                .guitar
                .control
                .bypass
                .load(std::sync::atomic::Ordering::Relaxed);

            if chip(ui, "ACTIF", enabled, c.success) {
                app.guitar
                    .control
                    .enabled
                    .store(!enabled, std::sync::atomic::Ordering::Relaxed);
            }
            if chip(ui, "MONITOR", monitor, c.accent_cyan) {
                app.guitar
                    .control
                    .monitor
                    .store(!monitor, std::sync::atomic::Ordering::Relaxed);
            }
            if chip(ui, "BYPASS", bypass, c.error) {
                app.guitar
                    .control
                    .bypass
                    .store(!bypass, std::sync::atomic::Ordering::Relaxed);
            }
        });
        ui.add_space(6.0);

        // Modèles d'ampli et de cabine.
        ui.label(egui::RichText::new("AMPLI").small().color(c.text_dim));
        let model_names: Vec<&str> = AmpModel::ALL.iter().map(|m| m.name()).collect();
        let mut model_idx = app.guitar.control.amp_model() as usize;
        if let Some(idx) = pill_select(ui, "amp_model", &mut model_idx, &model_names, c.accent) {
            app.guitar
                .control
                .model
                .store(idx as u32, std::sync::atomic::Ordering::Relaxed);
        }
        ui.label(egui::RichText::new("BAFFLE").small().color(c.text_dim));
        let cab_names: Vec<&str> = CabModel::ALL.iter().map(|m| m.name()).collect();
        let mut cab_idx = app.guitar.control.cab_model() as usize;
        if let Some(idx) = pill_select(ui, "cab_model", &mut cab_idx, &cab_names, c.accent_cyan) {
            app.guitar
                .control
                .cab_model
                .store(idx as u32, std::sync::atomic::Ordering::Relaxed);
        }
        ui.add_space(6.0);

        // Potentiomètres.
        ui.horizontal_wrapped(|ui| {
            for param in GuitarParam::ALL {
                let value = app.guitar.control.get_norm(param);
                let default = match param {
                    GuitarParam::Gain => 0.45,
                    GuitarParam::Master | GuitarParam::MonitorGain => 0.8,
                    _ => 0.5,
                };
                let spec = KnobSpec::linear(param.label(), " %", 0.0, 1.0, default);
                if let Some(v) = knob(ui, format!("guitar_{param:?}"), value, &spec) {
                    app.guitar.control.set_norm(param, v);
                }
            }
        });
    });
}

fn render_analysis(ui: &mut egui::Ui, app: &mut crate::gui::app::GuiApp, c: &MachColors) {
    let t = app.guitar.telemetry.lock().clone();
    ui.horizontal_wrapped(|ui| {
        card_frame().show(ui, |ui| {
            ui.set_min_width(220.0);
            ui.label(egui::RichText::new("NIVEAUX").small().color(c.text_dim));
            let in_lin = db_to_linear(t.input_db);
            let out_lin = db_to_linear(t.output_db);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("IN").small().color(c.text_muted));
                vu_meter(ui, [in_lin, in_lin], [in_lin, in_lin], 14.0, 92.0);
                ui.label(egui::RichText::new("OUT").small().color(c.text_muted));
                vu_meter(ui, [out_lin, out_lin], [out_lin, out_lin], 14.0, 92.0);
            });
            ui.label(
                egui::RichText::new(format!(
                    "IN {:.1} dB  ·  OUT {:.1} dB",
                    t.input_db, t.output_db
                ))
                .small()
                .monospace()
                .color(c.text_dim),
            );
        });

        // Accordeur.
        card_frame().show(ui, |ui| {
            ui.set_min_width(230.0);
            ui.label(egui::RichText::new("ACCORDEUR").small().color(c.text_dim));
            draw_tuner(ui, &t, c);
        });
    });
    ui.add_space(8.0);

    // Analyseur de spectre.
    card_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new("SPECTRE (entrée guitare)")
                .small()
                .color(c.text_dim),
        );
        draw_spectrum(ui, &t.spectrum, c);
    });
}

fn draw_tuner(ui: &mut egui::Ui, t: &crate::guitar::GuitarTelemetry, c: &MachColors) {
    if let Some(reading) = t.tuner {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{} {}", reading.note, reading.octave))
                    .size(28.0)
                    .strong()
                    .color(c.accent),
            );
            ui.label(
                egui::RichText::new(format!("{:+.0} cents", reading.cents))
                    .size(16.0)
                    .color(if reading.cents.abs() < 5.0 {
                        c.success
                    } else {
                        c.warning
                    }),
            );
            ui.label(
                egui::RichText::new(format!("{:.1} Hz", reading.frequency))
                    .small()
                    .color(c.text_dim),
            );
        });
        // Aiguille : -50 cents à gauche, +50 à droite.
        let (rect, _) = ui.allocate_exact_size(egui::vec2(210.0, 22.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, egui::CornerRadius::same(3), c.widget_bg);
        let center_x = rect.center().x;
        p.line_segment(
            [
                egui::pos2(center_x, rect.top() + 2.0),
                egui::pos2(center_x, rect.bottom() - 2.0),
            ],
            egui::Stroke::new(1.0_f32, c.success),
        );
        let x = center_x + (reading.cents / 50.0).clamp(-1.0, 1.0) * (rect.width() * 0.5 - 4.0);
        p.circle_filled(
            egui::pos2(x, rect.center().y),
            5.0,
            if reading.cents.abs() < 5.0 {
                c.success
            } else {
                c.accent
            },
        );
    } else {
        ui.label(
            egui::RichText::new("— (jouer une note) —")
                .small()
                .color(c.text_muted),
        );
        ui.allocate_exact_size(egui::vec2(210.0, 22.0), egui::Sense::hover());
    }
}

fn draw_spectrum(ui: &mut egui::Ui, bins: &[f32], c: &MachColors) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(200.0), 110.0),
        egui::Sense::hover(),
    );
    let p = ui.painter();
    p.rect_filled(rect, egui::CornerRadius::same(3), c.bg);
    if bins.is_empty() {
        return;
    }
    // On affiche les 512 premières bines (≈ 0–12 kHz à 48 kHz / 2048 FFT).
    let shown = bins.len().min(512);
    let bar_w = rect.width() / shown as f32;
    for (i, &db) in bins[..shown].iter().enumerate() {
        let norm = ((db + 90.0) / 90.0).clamp(0.0, 1.0);
        let h = norm * rect.height();
        if h <= 0.5 {
            continue;
        }
        let x = rect.left() + i as f32 * bar_w;
        let color = if norm > 0.85 {
            c.error
        } else if norm > 0.65 {
            c.warning
        } else {
            c.accent_cyan
        };
        p.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(x, rect.bottom() - h),
                egui::pos2(x + bar_w.max(1.0), rect.bottom()),
            ),
            egui::CornerRadius::same(0),
            color,
        );
    }
    // Repères fréquentiels (100 Hz, 1 kHz, 10 kHz).
    let sr_hz = 48000.0;
    let bin_hz = sr_hz / (bins.len() as f32 * 2.0 - 2.0);
    for &freq in &[100.0_f32, 1000.0, 10000.0] {
        let x = rect.left() + (freq / bin_hz) / shown as f32 * rect.width();
        if x <= rect.right() {
            p.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(1.0_f32, c.border),
            );
            p.text(
                egui::pos2(x + 2.0, rect.top() + 2.0),
                egui::Align2::LEFT_TOP,
                if freq >= 1000.0 {
                    format!("{}k", (freq / 1000.0) as u32)
                } else {
                    format!("{}", freq as u32)
                },
                egui::FontId::monospace(9.0),
                c.text_muted,
            );
        }
    }
}

fn render_diagnostics(ui: &mut egui::Ui, app: &mut crate::gui::app::GuiApp, c: &MachColors) {
    let t = app.guitar.telemetry.lock().clone();
    card_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new("DIAGNOSTICS TEMPS RÉEL")
                .small()
                .color(c.text_dim),
        );
        egui::Grid::new("guitar_diag_grid")
            .num_columns(2)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                let row = |ui: &mut egui::Ui, label: &str, value: String, color: egui::Color32| {
                    ui.colored_label(c.text_muted, egui::RichText::new(label).small());
                    ui.colored_label(color, egui::RichText::new(value).small().monospace());
                    ui.end_row();
                };
                row(
                    ui,
                    "CPU DSP",
                    format!("{:.1} %", t.cpu_load * 100.0),
                    if t.cpu_load > 0.8 {
                        c.warning
                    } else {
                        c.success
                    },
                );
                row(ui, "Latence", format!("{:.1} ms", t.latency_ms), c.text);
                row(
                    ui,
                    "Clipping",
                    format!("{} éch.", t.total_clipped),
                    if t.total_clipped > 0 {
                        c.error
                    } else {
                        c.success
                    },
                );
                row(ui, "Peak", format!("{:.3}", t.peak), c.text);
                row(ui, "Blocs", format!("{}", t.blocks), c.text_dim);
                row(
                    ui,
                    "Buffer IN",
                    format!("{} frames", app.guitar.input_ring.available()),
                    c.text_dim,
                );
                row(
                    ui,
                    "Xruns",
                    format!("{}", t.xruns),
                    if t.xruns > 0 { c.warning } else { c.success },
                );
                row(
                    ui,
                    "Entrée",
                    if t.input_device.is_empty() {
                        "—".to_string()
                    } else {
                        t.input_device.clone()
                    },
                    c.text_dim,
                );
            });
    });
}

fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}
