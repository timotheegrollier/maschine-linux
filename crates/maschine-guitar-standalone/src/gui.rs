//! egui GUI for guitar FX rack

#![cfg(feature = "gui")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use egui::{Color32, RichText, Stroke, Vec2};

use crate::{AppConfig, AudioToGui, GuiToAudio, ParamsBridge};
use maschine_dsp::ParamId;

/// Run the egui GUI
pub fn run_gui(
    config: AppConfig,
    params_bridge: Arc<ParamsBridge>,
    gui_tx: crossbeam_channel::Sender<GuiToAudio>,
    audio_rx: crossbeam_channel::Receiver<AudioToGui>,
    running: Arc<AtomicBool>,
) -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([
                config.gui.window_width as f32,
                config.gui.window_height as f32,
            ])
            .with_title("Maschine Guitar FX"),
        ..Default::default()
    };

    eframe::run_native(
        "Maschine Guitar FX",
        options,
        Box::new(|cc| {
            // Custom theme
            setup_theme(&cc.egui_ctx, &config.gui.theme);

            Ok(Box::new(GuitarFxApp::new(
                config,
                params_bridge,
                gui_tx,
                audio_rx,
                running,
            )) as Box<dyn eframe::App>)
        }),
    )
}

/// Guitar FX Application
struct GuitarFxApp {
    config: AppConfig,
    params_bridge: Arc<ParamsBridge>,
    gui_tx: crossbeam_channel::Sender<GuiToAudio>,
    audio_rx: crossbeam_channel::Receiver<AudioToGui>,
    running: Arc<AtomicBool>,

    // UI state
    input_level: f32,
    output_level: f32,
    input_devices: Vec<String>,
    output_devices: Vec<String>,
    selected_input: usize,
    selected_output: usize,
    show_settings: bool,
    #[allow(dead_code)] // horodatage du dernier paramètre (télémétrie future)
    last_param_update: std::time::Instant,
}

impl GuitarFxApp {
    fn new(
        config: AppConfig,
        params_bridge: Arc<ParamsBridge>,
        gui_tx: crossbeam_channel::Sender<GuiToAudio>,
        audio_rx: crossbeam_channel::Receiver<AudioToGui>,
        running: Arc<AtomicBool>,
    ) -> Self {
        // Request device list
        let _ = gui_tx.send(GuiToAudio::RequestDeviceList);

        Self {
            config,
            params_bridge,
            gui_tx,
            audio_rx,
            running,
            input_level: -60.0,
            output_level: -60.0,
            input_devices: vec!["Default".to_string()],
            output_devices: vec!["Default".to_string()],
            selected_input: 0,
            selected_output: 0,
            show_settings: false,
            last_param_update: std::time::Instant::now(),
        }
    }

    fn handle_audio_messages(&mut self) {
        while let Ok(msg) = self.audio_rx.try_recv() {
            match msg {
                AudioToGui::InputLevel(db) => self.input_level = db,
                AudioToGui::OutputLevel(db) => self.output_level = db,
                AudioToGui::DeviceList(inputs, outputs) => {
                    self.input_devices = inputs;
                    self.output_devices = outputs;
                }
            }
        }
    }

    fn send_param(&self, id: ParamId, value: f32) {
        let _ = self.gui_tx.send(GuiToAudio::SetParam(id, value));
    }
}

impl eframe::App for GuitarFxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_audio_messages();

        // Check for quit
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.running.store(false, Ordering::Relaxed);
        }

        // Top bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(RichText::new("🎸 Maschine Guitar FX").size(24.0).strong());
                ui.separator();

                // Bypass button
                let bypassed = self.params_bridge.is_bypassed();
                let bypass_text = if bypassed {
                    "🔇 BYPASS"
                } else {
                    "✅ ACTIVE"
                };
                let bypass_color = if bypassed {
                    Color32::RED
                } else {
                    Color32::GREEN
                };
                if ui
                    .add(
                        egui::Button::new(RichText::new(bypass_text).color(Color32::WHITE))
                            .fill(bypass_color)
                            .min_size(Vec2::new(100.0, 32.0)),
                    )
                    .clicked()
                {
                    let new_bypass = !bypassed;
                    self.params_bridge.set_bypass(new_bypass);
                    self.send_param(ParamId::Bypass, if new_bypass { 1.0 } else { 0.0 });
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⚙ Settings").clicked() {
                        self.show_settings = !self.show_settings;
                    }
                    // CPU meter placeholder
                    ui.label(RichText::new("CPU: --%").monospace());
                });
            });
        });

        // Bottom VU meters
        egui::TopBottomPanel::bottom("vu_meters").show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                vu_meter(ui, "IN", self.input_level);
                ui.add_space(20.0);
                vu_meter(ui, "OUT", self.output_level);
            });
        });

        // Settings panel
        if self.show_settings {
            egui::SidePanel::right("settings")
                .resizable(true)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.heading("Settings");
                    ui.separator();

                    // Audio devices
                    ui.label("Audio Input:");
                    egui::ComboBox::from_id_salt("input_device")
                        .selected_text(
                            &self.input_devices
                                [self.selected_input.min(self.input_devices.len() - 1)],
                        )
                        .show_ui(ui, |ui| {
                            for (i, dev) in self.input_devices.iter().enumerate() {
                                ui.selectable_value(&mut self.selected_input, i, dev);
                            }
                        });

                    ui.label("Audio Output:");
                    egui::ComboBox::from_id_salt("output_device")
                        .selected_text(
                            &self.output_devices
                                [self.selected_output.min(self.output_devices.len() - 1)],
                        )
                        .show_ui(ui, |ui| {
                            for (i, dev) in self.output_devices.iter().enumerate() {
                                ui.selectable_value(&mut self.selected_output, i, dev);
                            }
                        });

                    ui.separator();
                    ui.label("Buffer Size:");
                    ui.add(
                        egui::Slider::new(&mut self.config.audio.buffer_size, 32..=1024)
                            .text("frames")
                            .step_by(32.0),
                    );

                    ui.separator();
                    if ui.button("Save Config").clicked() {
                        let _ = self.config.save();
                    }
                });
        }

        // Main parameter grid
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Gate section
                    param_section(
                        ui,
                        "🚪 GATE",
                        &self.params_bridge,
                        &self.gui_tx,
                        &[
                            (ParamId::GateThreshold, -70.0, -20.0),
                            (ParamId::GateAttack, 0.1, 10.0),
                            (ParamId::GateRelease, 10.0, 500.0),
                        ],
                    );

                    // Drive section
                    param_section(
                        ui,
                        "🔥 DRIVE",
                        &self.params_bridge,
                        &self.gui_tx,
                        &[
                            (ParamId::DriveGain, 0.0, 36.0),
                            (ParamId::DriveTone, 200.0, 8000.0),
                            (ParamId::DriveMix, 0.0, 1.0),
                        ],
                    );
                });

                ui.separator();
                ui.horizontal(|ui| {
                    // ToneStack section
                    param_section(
                        ui,
                        "🎛 TONE",
                        &self.params_bridge,
                        &self.gui_tx,
                        &[
                            (ParamId::ToneBass, -12.0, 12.0),
                            (ParamId::ToneMid, -12.0, 12.0),
                            (ParamId::ToneTreble, -12.0, 12.0),
                        ],
                    );

                    // Cab section
                    param_section(
                        ui,
                        "📦 CAB",
                        &self.params_bridge,
                        &self.gui_tx,
                        &[
                            (ParamId::CabEnabled, 0.0, 1.0),
                            (ParamId::CabHighcut, 2000.0, 8000.0),
                            (ParamId::CabResonance, 0.5, 5.0),
                        ],
                    );
                });

                ui.separator();
                ui.horizontal(|ui| {
                    // Delay section
                    param_section(
                        ui,
                        "⏱ DELAY",
                        &self.params_bridge,
                        &self.gui_tx,
                        &[
                            (ParamId::DelayEnabled, 0.0, 1.0),
                            (ParamId::DelayTimeMs, 50.0, 2000.0),
                            (ParamId::DelayFeedback, 0.0, 0.95),
                            (ParamId::DelayMix, 0.0, 1.0),
                        ],
                    );

                    // Master section
                    param_section(
                        ui,
                        "🎚 MASTER",
                        &self.params_bridge,
                        &self.gui_tx,
                        &[(ParamId::MasterVolume, 0.0, 1.0)],
                    );
                });
            });
        });

        // Request repaint for smooth meters
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

/// Draw a parameter section with knobs
fn param_section(
    ui: &mut egui::Ui,
    title: &str,
    bridge: &ParamsBridge,
    gui_tx: &crossbeam_channel::Sender<GuiToAudio>,
    params: &[(ParamId, f32, f32)],
) {
    ui.vertical(|ui| {
        ui.set_min_width(200.0);
        ui.label(RichText::new(title).size(16.0).strong());
        ui.separator();

        for (id, min, max) in params {
            let min = *min;
            let max = *max;
            let current = bridge.get_param(*id);

            ui.horizontal(|ui| {
                ui.label(RichText::new(id.name()).size(13.0));

                // Knob using drag value
                let mut value = current;
                let norm = if max != min {
                    (value - min) / (max - min)
                } else {
                    0.0
                };
                let response = ui.add(
                    egui::DragValue::new(&mut value)
                        .range(min..=max)
                        .speed(0.01)
                        .suffix(match id.unit() {
                            "" => "",
                            u => u,
                        }),
                );

                if response.changed() {
                    let norm = if max != min {
                        (value - min) / (max - min)
                    } else {
                        0.0
                    };
                    gui_tx.send(GuiToAudio::SetParam(*id, norm)).ok();
                }

                // Visual knob indicator
                let knob_size = 24.0;
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(knob_size, knob_size), egui::Sense::hover());
                let painter = ui.painter();
                let center = rect.center();
                let radius = knob_size * 0.4;

                // Background circle
                painter.circle_stroke(center, radius, Stroke::new(2.0_f32, Color32::GRAY));

                // Indicator line
                let angle = -std::f32::consts::FRAC_PI_2 + norm * std::f32::consts::TAU * 0.75;
                let end = center + Vec2::new(angle.cos(), angle.sin()) * radius * 0.8;
                painter.line_segment([center, end], Stroke::new(3.0_f32, Color32::LIGHT_BLUE));
            });
        }
    });
}

/// VU meter widget
fn vu_meter(ui: &mut egui::Ui, label: &str, level_db: f32) {
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(label).size(14.0).strong());

        let meter_width = 40.0;
        let meter_height = 200.0;
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(meter_width, meter_height), egui::Sense::hover());
        let painter = ui.painter();

        // Background
        painter.rect_filled(rect, 4.0, Color32::from_gray(30));
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0_f32, Color32::GRAY),
            egui::StrokeKind::Outside,
        );

        // Level (clamp -60 to 0 dB)
        let level_norm = ((level_db + 60.0) / 60.0).clamp(0.0, 1.0);
        let fill_height = meter_height * level_norm;
        let fill_rect =
            egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.max.y - fill_height), rect.max);

        // Color based on level
        let color = if level_db > -6.0 {
            Color32::RED
        } else if level_db > -18.0 {
            Color32::YELLOW
        } else {
            Color32::GREEN
        };

        painter.rect_filled(fill_rect, 4.0, color);

        // dB labels
        for db in [-60, -48, -36, -24, -12, -6, 0] {
            let y = rect.max.y - meter_height * ((db + 60) as f32 / 60.0);
            painter.text(
                egui::pos2(rect.min.x - 30.0, y),
                egui::Align2::RIGHT_CENTER,
                format!("{}", db),
                egui::FontId::proportional(10.0),
                Color32::GRAY,
            );
        }

        // Current value
        ui.label(
            RichText::new(format!("{:.1} dB", level_db))
                .monospace()
                .size(14.0),
        );
    });
}

fn setup_theme(ctx: &egui::Context, theme: &str) {
    let mut style = (*ctx.style()).clone();

    if theme == "dark" {
        style.visuals.dark_mode = true;
        style.visuals.window_fill = Color32::from_gray(24);
        style.visuals.panel_fill = Color32::from_gray(28);
        style.visuals.extreme_bg_color = Color32::from_gray(20);
        style.visuals.faint_bg_color = Color32::from_gray(36);
        style.visuals.widgets.noninteractive.bg_fill = Color32::from_gray(36);
        style.visuals.widgets.inactive.bg_fill = Color32::from_gray(40);
        style.visuals.widgets.hovered.bg_fill = Color32::from_gray(50);
        style.visuals.widgets.active.bg_fill = Color32::from_gray(60);
        style.visuals.widgets.open.bg_fill = Color32::from_gray(50);
    }

    ctx.set_style(style);
}
