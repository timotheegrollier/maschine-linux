//! Barre de transport haute visibilité (inspirée Maschine MK3 / DAW).
//!
//! Sémantique stricte :
//! - ▶/■ = Play/Stop immédiat (`stop:true` quand on joue, `play:true` sinon) ;
//! - ● REC = bascule enregistrement (REC pendant lecture, retour à Play sinon) ;
//! - ⏮ RESTART = retour au début (tick 0).

use crate::gui::app::GuiApp;
use crate::gui::theme::{caption, value_label, MachColors};
use crate::gui::widgets::{chip, icon_button, mach_button};
use eframe::egui;

use maschine_engine::transport::TransportState;
use maschine_engine::EngineEvent;

/// Dessine la barre de transport en haut de l'application : une seule ligne
/// compacte (comme l'en-tête du logiciel Maschine) ; les infos secondaires
/// s'effacent quand la fenêtre est étroite.
pub fn render_transport(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
) {
    let wide = ui.available_width() >= 1400.0;
    // Étroit : les réglages master passent sur une seconde ligne.
    if ui.available_width() < 1100.0 {
        ui.horizontal_wrapped(|ui| {
            transport_controls(ui, app, snap, wide);
        });
        ui.horizontal(|ui| transport_master(ui, app, snap, wide));
    } else {
        ui.horizontal(|ui| {
            transport_controls(ui, app, snap, wide);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                transport_master(ui, app, snap, wide);
            });
        });
    }
}

/// Transport, position, tempo, swing, boucle/métronome/song.
fn transport_controls(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    wide: bool,
) {
    let c = MachColors::dark();
    let is_playing = matches!(
        snap.transport.state,
        TransportState::Playing | TransportState::Recording
    );
    let is_rec = matches!(snap.transport.state, TransportState::Recording);
    let loop_on = snap.transport.loop_enabled;
    let metro_on = snap.transport.metronome;
    ui.spacing_mut().item_spacing.x = 6.0;
    // ── Transport ────────────────────────────────────────────────
    let play_btn = egui::Button::new(
        egui::RichText::new(if is_playing { "■" } else { "▶" })
            .size(18.0)
            .strong()
            .color(egui::Color32::WHITE),
    )
    .min_size(egui::vec2(46.0, 32.0))
    .fill(if is_playing { c.error } else { c.success });
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
    let rec_btn = egui::Button::new(egui::RichText::new("● REC").size(12.0).strong().color(
        if is_rec {
            egui::Color32::WHITE
        } else {
            c.error
        },
    ))
    .min_size(egui::vec2(62.0, 32.0))
    .fill(if is_rec { c.error } else { c.widget_bg });
    let rec_tip = if is_rec {
        "Enregistrement : re-clic pour revenir en lecture"
    } else {
        "Enregistrement au prochain cycle"
    };
    if ui.add(rec_btn).on_hover_text(rec_tip).clicked() {
        // REC pendant lecture → Recording ; re-clic → retour en Play.
        let _ = app.engine.lock().handle_event(EngineEvent::Transport {
            play: is_rec,
            rec: !is_rec,
            stop: false,
        });
    }
    if icon_button(ui, "⏮", false, Some("Retour au début")).clicked {
        let _ = app.engine.lock().handle_event(EngineEvent::Restart);
    }
    ui.separator();

    // ── Position ─────────────────────────────────────────────────
    value_label(ui, format!("{}", snap.bbt), c.text);
    if wide {
        caption(
            ui,
            format!(
                "mesure {}/{}",
                snap.selected_bar + 1,
                snap.pattern_bars.max(1)
            ),
        );
    }
    ui.separator();

    // ── Tempo / swing ────────────────────────────────────────────
    let mut bpm = snap.transport.bpm;
    let resp = ui.add(
        egui::DragValue::new(&mut bpm)
            .speed(0.5)
            .range(20.0..=300.0)
            .custom_formatter(|v, _| format!("{v:.1} BPM")),
    );
    if resp
        .on_hover_text("Tempo : glisser ou double-clic")
        .changed()
    {
        let _ = app.engine.lock().handle_event(EngineEvent::Bpm(bpm));
    }
    let mut swing = snap.transport.swing;
    ui.spacing_mut().slider_width = if wide { 110.0 } else { 70.0 };
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
    ui.separator();

    // ── Boucle / métronome / song ────────────────────────────────
    if chip(ui, "LOOP", loop_on, c.success) {
        let _ = app.engine.lock().handle_event(EngineEvent::ToggleLoop);
    }
    if chip(ui, "MÉTRO", metro_on, c.accent_cyan) {
        let _ = app.engine.lock().handle_event(EngineEvent::ToggleMetronome);
    }
    if chip(ui, "SONG", snap.song_mode, c.accent) {
        let _ = app.engine.lock().handle_event(EngineEvent::ToggleSongMode);
    }
    if wide {
        ui.separator();
        value_label(ui, format!("{} voix", snap.num_voices), c.text_dim);
        caption(ui, app.audio_stats_text());
    }
}

/// Master, réglages audio et mises à jour.
fn transport_master(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    wide: bool,
) {
    let c = MachColors::dark();
    let update_ready = app.updater_state.lock().update_available.is_some();
    let up_txt = if update_ready { "MàJ DISPO" } else { "MàJ" };
    if mach_button(ui, up_txt, update_ready || app.show_updater).clicked {
        app.show_updater = !app.show_updater;
    }
    #[cfg(feature = "audio")]
    if mach_button(ui, "Audio", app.show_settings).clicked {
        app.show_settings = !app.show_settings;
        if app.show_settings {
            app.refresh_devices();
        }
    }
    value_label(ui, format!("{:>3.0}%", snap.master_volume * 100.0), c.text);
    let mut master = snap.master_volume;
    ui.spacing_mut().slider_width = if wide { 120.0 } else { 80.0 };
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
}
