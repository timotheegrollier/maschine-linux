//! Vue PLUGIN — édition universelle des 16 Sons + éditeur TB-303 simplifié.
//!
//! Le bouton PLUGIN du hardware ouvre cette vue sur la page SOUND : tous les
//! réglages du Sound actif (volume, pan, filtre, ADSR, polyphonie, choke) sont
//! accessibles, avec copie vers le groupe et réinitialisation. La page TB-303
//! propose un mode SIMPLE (macros + presets) et un mode AVANCÉ repliable.

use crate::gui::app::GuiApp;
use crate::gui::theme::{card_frame, MachColors};
use crate::gui::widgets::{chip, knob, KnobSpec};
use eframe::egui;

use maschine_engine::kick_synth::{KickDriveType, KickFilterMode, KickWaveform};
use maschine_engine::project::{Adsr, FilterKind};
use maschine_engine::wavetable_synth::{
    LfoTarget, WavetableFilterMode, WavetableParams, WavetableWaveform,
};
use maschine_engine::{AcidParams, AcidWaveform, EngineEvent};

pub fn render(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            render_header(ui, app, snap);
            ui.add_space(8.0);
            if snap.plugin_page == 0 {
                render_sound_page(ui, app, snap);
            } else if snap.focus_sound_kick.is_some() {
                render_kick_page(ui, app, snap);
            } else if snap.focus_sound_wavetable.is_some() {
                render_wavetable_page(ui, app, snap);
            } else if snap.focus_sound_acid.is_some() {
                render_acid_page(ui, app, snap);
            } else {
                render_sound_page(ui, app, snap);
            }
        });
}

fn render_header(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();
    let is_kick = snap.focus_sound_kick.is_some();
    let is_acid = snap.focus_sound_acid.is_some();
    let is_wt = snap.focus_sound_wavetable.is_some();
    let plugin_title = if is_kick {
        "PLUGIN | KICK SYNTH MODELER"
    } else if is_wt {
        "PLUGIN | NEXUS / SERUM SYNTH"
    } else if is_acid {
        "PLUGIN | TB-303 ACID BASS"
    } else {
        "PLUGIN | SAMPLER 808"
    };

    let subtitle = if is_kick {
        "Modélisation de kick techno analogique : punch, decay, sub 50Hz, distorsion multi-types & filtre."
    } else if is_wt {
        "Synthétiseur polyphonique à tables d'ondes & ROMpler : 16 presets Piano, Bass, Lead, Pad, Pluck & FX."
    } else if is_acid {
        "TB-303 Acid Bassline : filtre résonant à diode, slide, overdrive, delay."
    } else {
        "Tous les réglages du Sound actif — volume, pan, filtre, ADSR, voix."
    };

    ui.horizontal_wrapped(|ui| {
        crate::gui::theme::view_header(ui, plugin_title, subtitle);
        ui.add_space(8.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("🔍  Changer d'instrument (Browser)")
                            .small()
                            .strong()
                            .color(c.accent_cyan),
                    )
                    .fill(c.surface),
                )
                .clicked()
            {
                app.send(EngineEvent::SetBrowserKind(2));
                app.send(EngineEvent::SetLeftTab(3));
                app.view = crate::gui::views::StudioView::Browser;
            }
        });
    });

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        let sound_on = snap.plugin_page == 0;
        if chip(ui, "ENVELOPPE & MIX PAD", sound_on, c.accent) {
            let _ = app
                .engine
                .lock()
                .handle_event(EngineEvent::SetPluginPage(0));
        }
        let inst_label = if is_kick {
            "SYNTH : KICK MODELER"
        } else if is_acid {
            "SYNTH : TB-303 ACID"
        } else {
            "SYNTH : INSTRUMENT"
        };
        if chip(ui, inst_label, !sound_on, c.accent_cyan) {
            let _ = app
                .engine
                .lock()
                .handle_event(EngineEvent::SetPluginPage(1));
        }
        ui.separator();
        let name = snap
            .project
            .groups
            .get(snap.group_focus as usize)
            .and_then(|g| g.sounds.get(snap.active_sound as usize))
            .and_then(|s| s.as_ref())
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "(vide)".to_string());
        ui.label(
            egui::RichText::new(format!(
                "Groupe {} • Sound {:02} — {name}",
                crate::gui::views::group_char(snap.group_focus as usize),
                snap.active_sound + 1
            ))
            .small()
            .color(c.text_dim),
        );
    });
}

// ── Page SOUND ──────────────────────────────────────────────────────────

fn render_sound_page(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();
    let gf = snap.group_focus as usize;
    let snd = snap.active_sound as usize;
    let sound = snap
        .project
        .groups
        .get(gf)
        .and_then(|g| g.sounds.get(snd))
        .and_then(|s| s.as_ref());

    let name = sound.map(|s| s.name.clone()).unwrap_or_default();
    let volume_db = sound.map(|s| s.volume_db).unwrap_or(0.0);
    let pan = sound.map(|s| s.pan).unwrap_or(0.0);
    let polyphony = sound.map(|s| s.polyphony).unwrap_or(8);
    let choke_group = sound.and_then(|s| s.choke_group);
    let filter_kind = sound
        .map(|s| s.filter.kind.clone())
        .unwrap_or(FilterKind::Off);
    let cutoff = sound.map(|s| s.filter.cutoff_hz).unwrap_or(20000.0);
    let reso = sound.map(|s| s.filter.resonance).unwrap_or(0.0);
    let adsr = sound.map(|s| s.adsr.clone()).unwrap_or_default();

    // Sélecteur des 16 Sons.
    card_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new("SONS DU GROUPE — clique pour éditer")
                .strong()
                .small()
                .color(c.accent),
        );
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for i in 0..16usize {
                let sname = snap
                    .project
                    .groups
                    .get(gf)
                    .and_then(|g| g.sounds.get(i))
                    .and_then(|s| s.as_ref())
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| "—".to_string());
                let short: String = sname.chars().take(9).collect();
                let selected = i == snd;
                let mut btn = egui::Button::new(
                    egui::RichText::new(format!("{:02} {short}", i + 1))
                        .size(10.5)
                        .color(if selected {
                            egui::Color32::BLACK
                        } else {
                            c.text
                        }),
                )
                .min_size(egui::vec2(92.0, 28.0))
                .fill(if selected {
                    crate::gui::views::pad_color(i)
                } else {
                    c.widget_bg
                })
                .stroke(egui::Stroke::new(1.0_f32, c.border));
                if selected {
                    btn = btn.stroke(egui::Stroke::new(1.5_f32, c.text));
                }
                if ui.add(btn).on_hover_text(&sname).clicked() {
                    let _ = app
                        .engine
                        .lock()
                        .handle_event(EngineEvent::SelectSound(i as u8));
                }
            }
        });
    });

    ui.add_space(8.0);

    let card_w = (ui.available_width() - 16.0).clamp(180.0, 440.0);
    ui.horizontal_wrapped(|ui| {
        // Niveau, pan, voix.
        card_frame().show(ui, |ui| {
            ui.set_width(card_w - 18.0);
            section_title(ui, "NIVEAU & VOIX", c.accent);
            ui.horizontal_wrapped(|ui| {
                if let Some(v) = knob(
                    ui,
                    "plugin_vol",
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
                    "plugin_pan",
                    pan,
                    &KnobSpec::linear("PAN", "", -1.0, 1.0, 0.0),
                ) {
                    let _ = app.engine.lock().handle_event(EngineEvent::SoundPan {
                        group: snap.group_focus,
                        sound: snap.active_sound,
                        pan: v,
                    });
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Polyphonie").small().color(c.text_dim));
                let mut poly = polyphony;
                if ui
                    .add(egui::DragValue::new(&mut poly).range(1..=32).speed(0.2))
                    .changed()
                {
                    let _ = app.engine.lock().handle_event(EngineEvent::SoundPolyphony {
                        group: snap.group_focus,
                        sound: snap.active_sound,
                        polyphony: poly,
                    });
                }
                ui.label(egui::RichText::new("voix").small().color(c.text_muted));
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Choke").small().color(c.text_dim));
                let mut choke = choke_group;
                egui::ComboBox::from_id_salt("plugin_choke")
                    .selected_text(match choke {
                        Some(g) => format!("Groupe {g}"),
                        None => "Aucun".to_string(),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut choke, None, "Aucun");
                        for g in 1..=8u8 {
                            ui.selectable_value(&mut choke, Some(g), format!("Groupe {g}"));
                        }
                    });
                if choke != choke_group {
                    let _ = app.engine.lock().handle_event(EngineEvent::SoundChoke {
                        group: snap.group_focus,
                        sound: snap.active_sound,
                        choke_group: choke,
                    });
                }
            });
            ui.label(
                egui::RichText::new(format!(
                    "Source : {}",
                    if name.is_empty() { "(vide)" } else { &name }
                ))
                .small()
                .color(c.text_muted),
            );
        });

        ui.add_space(8.0);

        // Filtre.
        card_frame().show(ui, |ui| {
            ui.set_width(card_w - 18.0);
            section_title(ui, "FILTRE", c.accent_cyan);
            let mut kind = filter_kind.clone();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Type").small().color(c.text_dim));
                egui::ComboBox::from_id_salt("plugin_filter_kind")
                    .selected_text(match kind {
                        FilterKind::Off => "Off",
                        FilterKind::LowPass => "Low-pass",
                        FilterKind::HighPass => "High-pass",
                        FilterKind::BandPass => "Band-pass",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut kind, FilterKind::Off, "Off");
                        ui.selectable_value(&mut kind, FilterKind::LowPass, "Low-pass");
                        ui.selectable_value(&mut kind, FilterKind::HighPass, "High-pass");
                        ui.selectable_value(&mut kind, FilterKind::BandPass, "Band-pass");
                    });
            });
            ui.horizontal(|ui| {
                if chip(ui, "OFF", filter_kind == FilterKind::Off, c.text_dim) {
                    kind = FilterKind::Off;
                }
                if chip(ui, "LP", filter_kind == FilterKind::LowPass, c.accent) {
                    kind = FilterKind::LowPass;
                }
                if chip(ui, "HP", filter_kind == FilterKind::HighPass, c.accent_cyan) {
                    kind = FilterKind::HighPass;
                }
                if chip(ui, "BP", filter_kind == FilterKind::BandPass, c.warning) {
                    kind = FilterKind::BandPass;
                }
            });
            if kind != filter_kind {
                let _ = app
                    .engine
                    .lock()
                    .handle_event(EngineEvent::SoundFilterKind {
                        group: snap.group_focus,
                        sound: snap.active_sound,
                        kind,
                    });
            }
            let filter_on = filter_kind != FilterKind::Off;
            draw_filter_curve(
                ui,
                (card_w - 26.0).max(160.0),
                88.0,
                cutoff,
                reso,
                filter_on,
            );
            ui.add_space(2.0);
            let mut cutoff_spec = KnobSpec::log("CUTOFF", " Hz", 20.0, 20000.0, 20000.0);
            cutoff_spec.size = 38.0;
            let mut reso_spec = KnobSpec::linear("RESO", " %", 0.0, 1.0, 0.0);
            reso_spec.size = 38.0;
            ui.horizontal_wrapped(|ui| {
                if !filter_on {
                    ui.label(
                        egui::RichText::new(
                            "Filtre inactif — choisis Low-pass pour entendre le cutoff.",
                        )
                        .small()
                        .color(c.text_muted),
                    );
                }
                if let Some(v) = knob(ui, "plugin_cutoff", cutoff, &cutoff_spec) {
                    let _ = app
                        .engine
                        .lock()
                        .handle_event(EngineEvent::SoundFilterCutoff {
                            group: snap.group_focus,
                            sound: snap.active_sound,
                            cutoff_hz: v,
                        });
                }
                if let Some(v) = knob(ui, "plugin_reso", reso, &reso_spec) {
                    let _ = app
                        .engine
                        .lock()
                        .handle_event(EngineEvent::SoundFilterReso {
                            group: snap.group_focus,
                            sound: snap.active_sound,
                            resonance: v,
                        });
                }
            });
        });

        ui.add_space(8.0);

        // ADSR.
        card_frame().show(ui, |ui| {
            ui.set_width(card_w - 18.0);
            section_title(ui, "ENVELOPPE ADSR", c.success);
            draw_adsr(ui, (card_w - 26.0).max(160.0), 80.0, &adsr, c.success);
            ui.add_space(2.0);
            let mut a_spec = KnobSpec::linear("ATTACK", " ms", 0.0, 2000.0, 2.0);
            a_spec.size = 36.0;
            let mut d_spec = KnobSpec::linear("DECAY", " ms", 5.0, 2000.0, 100.0);
            d_spec.size = 36.0;
            let mut s_spec = KnobSpec::linear("SUSTAIN", " %", 0.0, 1.0, 0.8);
            s_spec.size = 36.0;
            let mut r_spec = KnobSpec::linear("RELEASE", " ms", 5.0, 3000.0, 150.0);
            r_spec.size = 36.0;
            ui.horizontal_wrapped(|ui| {
                if let Some(v) = knob(ui, "plugin_att", adsr.attack_ms, &a_spec) {
                    send_adsr(app, snap, v, adsr.decay_ms, adsr.sustain, adsr.release_ms);
                }
                if let Some(v) = knob(ui, "plugin_dec", adsr.decay_ms, &d_spec) {
                    send_adsr(app, snap, adsr.attack_ms, v, adsr.sustain, adsr.release_ms);
                }
                if let Some(v) = knob(ui, "plugin_sus", adsr.sustain, &s_spec) {
                    send_adsr(app, snap, adsr.attack_ms, adsr.decay_ms, v, adsr.release_ms);
                }
                if let Some(v) = knob(ui, "plugin_rel", adsr.release_ms, &r_spec) {
                    send_adsr(app, snap, adsr.attack_ms, adsr.decay_ms, adsr.sustain, v);
                }
            });
        });

        ui.add_space(8.0);

        // Actions.
        card_frame().show(ui, |ui| {
            ui.set_width(card_w - 18.0);
            section_title(ui, "ACTIONS RAPIDES", c.warning);
            ui.horizontal_wrapped(|ui| {
                if chip(ui, "▶ AUDITIONNER", false, c.success) {
                    let _ = app.engine.lock().handle_event(EngineEvent::Pad {
                        pad: snap.active_sound,
                        velocity: 115,
                        pressed: true,
                    });
                }
                if chip(ui, "COPIER VERS TOUS", false, c.accent_cyan) {
                    let _ = app
                        .engine
                        .lock()
                        .handle_event(EngineEvent::SoundApplyToGroup {
                            group: snap.group_focus,
                            source_sound: snap.active_sound,
                        });
                }
                if chip(ui, "RÉINITIALISER", false, c.warning) {
                    let _ = app.engine.lock().handle_event(EngineEvent::SoundReset {
                        group: snap.group_focus,
                        sound: snap.active_sound,
                    });
                }
                if chip(ui, "PRESET BROWSER", false, c.accent) {
                    let _ = app.engine.lock().handle_event(EngineEvent::LoadDrumPreset {
                        group: snap.group_focus,
                        sound: snap.active_sound,
                        preset_idx: snap.browser_preset_idx,
                    });
                }
                if chip(ui, "EFFACER LES NOTES", false, c.error) {
                    let _ = app.engine.lock().handle_event(EngineEvent::EraseSound {
                        group: snap.group_focus,
                        sound: snap.active_sound,
                    });
                }
            });
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "COPIER VERS TOUS recopie volume, pan, filtre, ADSR, polyphonie et choke \
                     sur les autres Sons du groupe. Les samples et noms restent intacts.",
                )
                .small()
                .color(c.text_dim),
            );
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

// ── Page TB-303 ─────────────────────────────────────────────────────────

fn render_acid_page(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();

    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("PRESETS")
                .strong()
                .small()
                .color(c.text_dim),
        );
        for (idx, name, desc) in [
            (0u8, "CLASSIC", "Saw râpeuse, réso marquée"),
            (1, "SQUELCH", "Acide, env mod rapide"),
            (2, "DISTORTED", "Drive et filtre agressifs"),
            (3, "DUB ECHO", "Delay large, plus doux"),
        ] {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{name}\n{desc}"))
                            .size(10.5)
                            .strong(),
                    )
                    .min_size(egui::vec2(126.0, 42.0))
                    .fill(c.widget_bg)
                    .stroke(egui::Stroke::new(1.0_f32, c.border)),
                )
                .clicked()
            {
                let _ = app
                    .engine
                    .lock()
                    .handle_event(EngineEvent::AcidSelectPreset(idx));
            }
        }
    });

    ui.add_space(8.0);

    ui.horizontal_wrapped(|ui| {
        if chip(ui, "SIMPLE", !app.plugin_advanced, c.accent_cyan) {
            app.plugin_advanced = false;
        }
        if chip(ui, "AVANCÉ", app.plugin_advanced, c.accent) {
            app.plugin_advanced = true;
        }
        ui.label(
            egui::RichText::new(if app.plugin_advanced {
                "Tous les paramètres du moteur 303."
            } else {
                "Les 8 macros suffisent pour jouer tout de suite."
            })
            .small()
            .color(c.text_dim),
        );
    });

    ui.add_space(8.0);

    if app.plugin_advanced {
        render_acid_advanced(ui, app, snap);
    } else {
        render_acid_simple(ui, app, snap);
    }

    // Clavier commun aux deux modes.
    ui.add_space(8.0);
    full_card(ui, "CLAVIER ACID — C1 → D#2", c.accent, |ui| {
        ui.label(
            egui::RichText::new(
                "Clique une touche pour jouer. Sur le hardware, les pads déclenchent \
                 la même gamme chromatique.",
            )
            .small()
            .color(c.text_dim),
        );
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for pad_idx in 0..16u8 {
                let note = 36 + pad_idx;
                let name = note_name(note);
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
                let btn =
                    egui::Button::new(egui::RichText::new(name).strong().size(10.5).color(text))
                        .min_size(egui::vec2(42.0, 44.0))
                        .fill(fill)
                        .stroke(egui::Stroke::new(1.0_f32, c.border));
                if ui.add(btn).clicked() {
                    let _ = app.engine.lock().handle_event(EngineEvent::Pad {
                        pad: pad_idx,
                        velocity: 108,
                        pressed: true,
                    });
                }
            }
        });
        ui.add_space(4.0);
        if chip(ui, "▶ TESTER LE SON ACTIF", false, c.success) {
            let _ = app.engine.lock().handle_event(EngineEvent::Pad {
                pad: snap.active_sound,
                velocity: 115,
                pressed: true,
            });
        }
    });
}

fn render_acid_simple(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();
    let p = &snap.acid_params;

    full_card(ui, "DÉMARRAGE EN 3 ÉTAPES", c.success, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("1. Choisis un preset.")
                    .strong()
                    .color(c.text),
            );
            ui.label(
                egui::RichText::new("2. Ajuste les macros.")
                    .strong()
                    .color(c.text),
            );
            ui.label(
                egui::RichText::new("3. Joue sur les pads ou le clavier.")
                    .strong()
                    .color(c.text),
            );
            ui.separator();
            if chip(ui, "RESET TB-303", false, c.warning) {
                let _ = app
                    .engine
                    .lock()
                    .handle_event(EngineEvent::AcidSelectPreset(0));
            }
        });
    });

    ui.add_space(8.0);

    full_card(ui, "MACROS ESSENTIELLES", c.accent, |ui| {
        let mut cutoff = KnobSpec::log("CUTOFF", " Hz", 20.0, 10000.0, 500.0);
        cutoff.size = 48.0;
        let mut reso = KnobSpec::linear("RESONANCE", " %", 0.0, 1.0, 0.5);
        reso.size = 48.0;
        let mut env = KnobSpec::linear("ENV MOD", " %", 0.0, 1.0, 0.7);
        env.size = 48.0;
        let mut decay = KnobSpec::linear("DECAY", " ms", 20.0, 2000.0, 300.0);
        decay.size = 48.0;
        let mut accent = KnobSpec::linear("ACCENT", " %", 0.0, 1.0, 0.5);
        accent.size = 48.0;
        let mut drive = KnobSpec::linear("DRIVE", " %", 0.0, 1.0, 0.0);
        drive.size = 48.0;
        let mut delay = KnobSpec::linear("DELAY MIX", " %", 0.0, 1.0, 0.25);
        delay.size = 48.0;
        let mut volume = KnobSpec::linear("VOLUME", " dB", -60.0, 12.0, 0.0);
        volume.size = 48.0;

        ui.horizontal_wrapped(|ui| {
            acid_knob(
                ui,
                app,
                p,
                "simple_cutoff",
                p.cutoff_hz,
                &cutoff,
                |v, np| {
                    np.cutoff_hz = v;
                },
            );
            acid_knob(ui, app, p, "simple_reso", p.resonance, &reso, |v, np| {
                np.resonance = v;
            });
            acid_knob(ui, app, p, "simple_env", p.env_mod, &env, |v, np| {
                np.env_mod = v;
            });
            acid_knob(ui, app, p, "simple_decay", p.decay_ms, &decay, |v, np| {
                np.decay_ms = v;
            });
            acid_knob(ui, app, p, "simple_accent", p.accent, &accent, |v, np| {
                np.accent = v;
            });
            acid_knob(ui, app, p, "simple_drive", p.drive, &drive, |v, np| {
                np.drive = v;
            });
            acid_knob(ui, app, p, "simple_delay", p.delay_mix, &delay, |v, np| {
                np.delay_mix = v;
            });
            acid_knob(ui, app, p, "simple_vol", p.volume_db, &volume, |v, np| {
                np.volume_db = v;
            });
        });
    });

    ui.add_space(8.0);

    full_card(ui, "CARACTÈRE & JEU", c.accent_cyan, |ui| {
        ui.horizontal_wrapped(|ui| {
            let is_saw = p.waveform == AcidWaveform::Saw;
            if chip(ui, "SAW", is_saw, c.accent) && !is_saw {
                let _ = app
                    .engine
                    .lock()
                    .handle_event(EngineEvent::AcidWaveformToggle);
            }
            if chip(ui, "SQUARE", !is_saw, c.accent) && is_saw {
                let _ = app
                    .engine
                    .lock()
                    .handle_event(EngineEvent::AcidWaveformToggle);
            }
            ui.separator();
            ui.label(egui::RichText::new("OCTAVE").small().color(c.text_dim));
            if ui.button("−").clicked() {
                let _ = app
                    .engine
                    .lock()
                    .handle_event(EngineEvent::AcidOctaveDelta(-1));
            }
            ui.label(
                egui::RichText::new(format!("{:+}", p.octave))
                    .monospace()
                    .strong(),
            );
            if ui.button("+").clicked() {
                let _ = app
                    .engine
                    .lock()
                    .handle_event(EngineEvent::AcidOctaveDelta(1));
            }
        });
        ui.add_space(4.0);
        let mut slide = KnobSpec::linear("SLIDE", " ms", 0.0, 250.0, 60.0);
        slide.size = 38.0;
        let mut tuning = KnobSpec::linear("TUNE", " 1/2 t", -24.0, 24.0, 0.0);
        tuning.size = 38.0;
        ui.horizontal_wrapped(|ui| {
            acid_knob(
                ui,
                app,
                p,
                "simple_slide",
                p.slide_time_ms,
                &slide,
                |v, np| {
                    np.slide_time_ms = v;
                },
            );
            acid_knob(
                ui,
                app,
                p,
                "simple_tuning",
                p.tuning_semitones,
                &tuning,
                |v, np| np.tuning_semitones = v,
            );
            ui.label(
                egui::RichText::new(
                    "SLIDE donne le glissando entre deux notes. TUNE décale la hauteur.",
                )
                .small()
                .color(c.text_dim),
            );
        });
    });
}

fn render_acid_advanced(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
) {
    let c = MachColors::dark();
    let p = &snap.acid_params;
    let card_w = (ui.available_width() - 16.0).clamp(180.0, 440.0);

    ui.horizontal_wrapped(|ui| {
        card_frame().show(ui, |ui| {
            ui.set_width(card_w - 18.0);
            section_title(ui, "OSCILLATEUR", c.accent);
            draw_wave(ui, (card_w - 26.0).max(160.0), 70.0, p.waveform);
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                let is_saw = p.waveform == AcidWaveform::Saw;
                if chip(ui, "SAW", is_saw, c.accent) && !is_saw {
                    let _ = app
                        .engine
                        .lock()
                        .handle_event(EngineEvent::AcidWaveformToggle);
                }
                if chip(ui, "SQUARE", !is_saw, c.accent) && is_saw {
                    let _ = app
                        .engine
                        .lock()
                        .handle_event(EngineEvent::AcidWaveformToggle);
                }
                ui.separator();
                if ui.button("OCT −").clicked() {
                    let _ = app
                        .engine
                        .lock()
                        .handle_event(EngineEvent::AcidOctaveDelta(-1));
                }
                ui.label(
                    egui::RichText::new(format!("{:+}", p.octave))
                        .monospace()
                        .strong(),
                );
                if ui.button("OCT +").clicked() {
                    let _ = app
                        .engine
                        .lock()
                        .handle_event(EngineEvent::AcidOctaveDelta(1));
                }
            });
            let mut slide = KnobSpec::linear("SLIDE", " ms", 0.0, 250.0, 60.0);
            slide.size = 38.0;
            let mut tuning = KnobSpec::linear("TUNE", " 1/2 t", -24.0, 24.0, 0.0);
            tuning.size = 38.0;
            let mut volume = KnobSpec::linear("LEVEL", " dB", -60.0, 12.0, 0.0);
            volume.size = 38.0;
            ui.horizontal_wrapped(|ui| {
                acid_knob(ui, app, p, "adv_slide", p.slide_time_ms, &slide, |v, np| {
                    np.slide_time_ms = v;
                });
                acid_knob(
                    ui,
                    app,
                    p,
                    "adv_tuning",
                    p.tuning_semitones,
                    &tuning,
                    |v, np| np.tuning_semitones = v,
                );
                acid_knob(ui, app, p, "adv_vol", p.volume_db, &volume, |v, np| {
                    np.volume_db = v;
                });
            });
        });

        ui.add_space(8.0);

        card_frame().show(ui, |ui| {
            ui.set_width(card_w - 18.0);
            section_title(ui, "FILTRE 303", c.accent_cyan);
            draw_filter_curve(
                ui,
                (card_w - 26.0).max(160.0),
                70.0,
                p.cutoff_hz,
                p.resonance,
                true,
            );
            ui.add_space(2.0);
            let mut cutoff = KnobSpec::log("CUTOFF", " Hz", 20.0, 10000.0, 500.0);
            cutoff.size = 38.0;
            let mut reso = KnobSpec::linear("RESO", " %", 0.0, 1.0, 0.5);
            reso.size = 38.0;
            let mut env = KnobSpec::linear("ENV MOD", " %", 0.0, 1.0, 0.7);
            env.size = 38.0;
            let mut decay = KnobSpec::linear("DECAY", " ms", 20.0, 2000.0, 300.0);
            decay.size = 38.0;
            let mut accent = KnobSpec::linear("ACCENT", " %", 0.0, 1.0, 0.5);
            accent.size = 38.0;
            ui.horizontal_wrapped(|ui| {
                acid_knob(ui, app, p, "adv_cutoff", p.cutoff_hz, &cutoff, |v, np| {
                    np.cutoff_hz = v;
                });
                acid_knob(ui, app, p, "adv_reso", p.resonance, &reso, |v, np| {
                    np.resonance = v;
                });
                acid_knob(ui, app, p, "adv_env", p.env_mod, &env, |v, np| {
                    np.env_mod = v;
                });
                acid_knob(ui, app, p, "adv_decay", p.decay_ms, &decay, |v, np| {
                    np.decay_ms = v;
                });
                acid_knob(ui, app, p, "adv_accent", p.accent, &accent, |v, np| {
                    np.accent = v;
                });
            });
        });

        ui.add_space(8.0);

        card_frame().show(ui, |ui| {
            ui.set_width(card_w - 18.0);
            section_title(ui, "DISTORSION", c.warning);
            draw_shaper(ui, (card_w - 26.0).max(160.0), 64.0, p.drive);
            ui.add_space(2.0);
            let mut drive = KnobSpec::linear("DRIVE", " %", 0.0, 1.0, 0.0);
            drive.size = 38.0;
            let mut tone = KnobSpec::log("TONE", " Hz", 500.0, 16000.0, 6000.0);
            tone.size = 38.0;
            let mut mix = KnobSpec::linear("DRY/WET", " %", 0.0, 1.0, 1.0);
            mix.size = 38.0;
            ui.horizontal_wrapped(|ui| {
                acid_knob(ui, app, p, "adv_drive", p.drive, &drive, |v, np| {
                    np.drive = v;
                });
                acid_knob(ui, app, p, "adv_tone", p.drive_tone, &tone, |v, np| {
                    np.drive_tone = v;
                });
                acid_knob(ui, app, p, "adv_drive_mix", p.drive_mix, &mix, |v, np| {
                    np.drive_mix = v;
                });
            });
        });

        ui.add_space(8.0);

        card_frame().show(ui, |ui| {
            ui.set_width(card_w - 18.0);
            section_title(ui, "DELAY STÉRÉO", c.success);
            draw_echo(
                ui,
                (card_w - 26.0).max(160.0),
                64.0,
                p.delay_feedback,
                p.delay_mix,
            );
            ui.add_space(2.0);
            let mut time = KnobSpec::linear("TIME", " ms", 10.0, 1000.0, 250.0);
            time.size = 38.0;
            let mut fb = KnobSpec::linear("FEEDBACK", " %", 0.0, 0.95, 0.4);
            fb.size = 38.0;
            let mut mix = KnobSpec::linear("MIX", " %", 0.0, 1.0, 0.25);
            mix.size = 38.0;
            ui.horizontal_wrapped(|ui| {
                acid_knob(
                    ui,
                    app,
                    p,
                    "adv_delay_time",
                    p.delay_time_ms,
                    &time,
                    |v, np| {
                        np.delay_time_ms = v;
                    },
                );
                acid_knob(
                    ui,
                    app,
                    p,
                    "adv_delay_fb",
                    p.delay_feedback,
                    &fb,
                    |v, np| np.delay_feedback = v,
                );
                acid_knob(ui, app, p, "adv_delay_mix", p.delay_mix, &mix, |v, np| {
                    np.delay_mix = v;
                });
            });
        });
    });
}

fn acid_knob(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    params: &AcidParams,
    id: &'static str,
    value: f32,
    spec: &KnobSpec<'_>,
    apply: impl FnOnce(f32, &mut AcidParams),
) {
    if let Some(v) = knob(ui, id, value, spec) {
        let mut new_params = params.clone();
        apply(v, &mut new_params);
        let _ = app
            .engine
            .lock()
            .handle_event(EngineEvent::AcidSetParams(new_params));
    }
}

fn full_card(
    ui: &mut egui::Ui,
    title: &str,
    color: egui::Color32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let width = (ui.available_width() - 18.0).max(160.0);
    card_frame().show(ui, |ui| {
        ui.set_width(width);
        section_title(ui, title, color);
        add_contents(ui);
    });
}

fn section_title(ui: &mut egui::Ui, title: &str, color: egui::Color32) {
    ui.label(egui::RichText::new(title).strong().small().color(color));
    ui.add_space(4.0);
}

fn draw_adsr(ui: &mut egui::Ui, width: f32, height: f32, adsr: &Adsr, color: egui::Color32) {
    let c = MachColors::dark();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(4), c.bg);
    let (a, d, s, r) = (
        adsr.attack_ms,
        adsr.decay_ms,
        adsr.sustain.clamp(0.0, 1.0),
        adsr.release_ms,
    );
    let total = (a + d + r).max(1.0);
    let a_w = (a / total).clamp(0.05, 0.45) * rect.width();
    let d_w = (d / total).clamp(0.05, 0.45) * rect.width();
    let r_w = (r / total).clamp(0.08, 0.5) * rect.width();
    let s_w = (rect.width() - a_w - d_w - r_w).max(10.0);
    let top = rect.top() + 10.0;
    let bottom = rect.bottom() - 10.0;
    let s_y = bottom - s * (bottom - top);
    let pts = vec![
        egui::pos2(rect.left(), bottom),
        egui::pos2(rect.left() + a_w, top),
        egui::pos2(rect.left() + a_w + d_w, s_y),
        egui::pos2(rect.left() + a_w + d_w + s_w, s_y),
        egui::pos2(rect.right(), bottom),
    ];
    painter.add(egui::Shape::line(pts, egui::Stroke::new(2.0_f32, color)));
    painter.text(
        egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        format!("A {a:.0} • D {d:.0} • S {:.0}% • R {r:.0}", s * 100.0),
        egui::FontId::monospace(9.5),
        c.text_dim,
    );
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(4),
        egui::Stroke::new(1.0_f32, c.border),
        egui::StrokeKind::Inside,
    );
}

fn draw_filter_curve(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    cutoff_hz: f32,
    resonance: f32,
    on: bool,
) {
    let c = MachColors::dark();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(4), c.bg);
    if on {
        let norm_cutoff = ((cutoff_hz.max(20.0).log10() - 20.0f32.log10())
            / (20000.0f32.log10() - 20.0f32.log10()))
        .clamp(0.03, 0.97);
        let cutoff_x = rect.left() + norm_cutoff * rect.width();
        let mut points = Vec::with_capacity(64);
        for step in 0..=60 {
            let x = rect.left() + (step as f32 / 60.0) * rect.width();
            let dist = (x - cutoff_x) / 32.0;
            let bump = resonance.clamp(0.0, 1.0) * 30.0 * (-dist * dist).exp();
            let rolloff = if x < cutoff_x {
                0.0
            } else {
                ((x - cutoff_x) / 12.0).powi(2)
            };
            let y = (rect.bottom() - 20.0 - bump + rolloff)
                .clamp(rect.top() + 5.0, rect.bottom() - 4.0);
            points.push(egui::pos2(x, y));
        }
        painter.add(egui::Shape::line(
            points,
            egui::Stroke::new(2.0_f32, c.accent_cyan),
        ));
        painter.vline(
            cutoff_x,
            rect.top()..=rect.bottom(),
            egui::Stroke::new(1.0_f32, c.border_active),
        );
        painter.text(
            egui::pos2(rect.left() + 6.0, rect.top() + 5.0),
            egui::Align2::LEFT_TOP,
            format!("{:.0} Hz • réso {:.0}%", cutoff_hz, resonance * 100.0),
            egui::FontId::monospace(9.5),
            c.text_dim,
        );
    } else {
        painter.hline(
            rect.left()..=rect.right(),
            rect.center().y,
            egui::Stroke::new(2.0_f32, c.text_muted),
        );
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "FILTRE OFF",
            egui::FontId::monospace(10.0),
            c.text_muted,
        );
    }
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(4),
        egui::Stroke::new(1.0_f32, c.border),
        egui::StrokeKind::Inside,
    );
}

fn draw_wave(ui: &mut egui::Ui, width: f32, height: f32, waveform: AcidWaveform) {
    let c = MachColors::dark();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(4), c.bg);
    let mid = rect.center().y;
    let amp = height / 2.0 - 8.0;
    let mut points = Vec::with_capacity(128);
    for i in 0..128 {
        let x = rect.left() + (i as f32 / 127.0) * rect.width();
        let phase = (i as f32 % 32.0) / 32.0;
        let y = match waveform {
            AcidWaveform::Saw => mid + (0.5 - phase) * amp * 2.0,
            AcidWaveform::Square => {
                if phase < 0.5 {
                    mid - amp
                } else {
                    mid + amp
                }
            }
        };
        points.push(egui::pos2(x, y));
    }
    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(2.0_f32, c.accent),
    ));
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(4),
        egui::Stroke::new(1.0_f32, c.border),
        egui::StrokeKind::Inside,
    );
}

fn draw_shaper(ui: &mut egui::Ui, width: f32, height: f32, drive: f32) {
    let c = MachColors::dark();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(4), c.bg);
    let mid_x = rect.center().x;
    let mid_y = rect.center().y;
    let w_half = rect.width() / 2.0 - 8.0;
    let h_half = rect.height() / 2.0 - 6.0;
    let gain = 1.0 + drive * 12.0;
    let mut points = Vec::with_capacity(41);
    for i in -20..=20 {
        let norm_in = i as f32 / 20.0;
        let sat = (norm_in * gain).tanh();
        points.push(egui::pos2(mid_x + norm_in * w_half, mid_y - sat * h_half));
    }
    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(2.0_f32, c.warning),
    ));
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(4),
        egui::Stroke::new(1.0_f32, c.border),
        egui::StrokeKind::Inside,
    );
}

fn draw_echo(ui: &mut egui::Ui, width: f32, height: f32, feedback: f32, mix: f32) {
    let c = MachColors::dark();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(4), c.bg);
    let echoes = 7;
    let spacing = rect.width() / (echoes as f32 + 1.0);
    let mut amp = 1.0;
    for e in 0..echoes {
        let bx = rect.left() + 12.0 + e as f32 * spacing;
        let bh = amp * (rect.height() - 16.0) * mix.max(0.05);
        let by = rect.bottom() - 8.0 - bh;
        let bar = egui::Rect::from_min_size(egui::pos2(bx, by), egui::vec2(6.0, bh));
        let color = if e % 2 == 0 { c.success } else { c.accent_cyan };
        painter.rect_filled(bar, egui::CornerRadius::same(2), color);
        amp *= feedback.clamp(0.1, 0.95);
    }
    painter.rect_stroke(
        rect,
        egui::CornerRadius::same(4),
        egui::Stroke::new(1.0_f32, c.border),
        egui::StrokeKind::Inside,
    );
}

fn note_name(midi: u8) -> &'static str {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = midi / 12;
    let note = NAMES[(midi % 12) as usize];
    // Les notes sont dans C1..D#2 pour ce clavier.
    match (note, octave) {
        ("C", 3) => "C1",
        ("C#", 3) => "C#1",
        ("D", 3) => "D1",
        ("D#", 3) => "D#1",
        ("E", 3) => "E1",
        ("F", 3) => "F1",
        ("F#", 3) => "F#1",
        ("G", 3) => "G1",
        ("G#", 3) => "G#1",
        ("A", 3) => "A1",
        ("A#", 3) => "A#1",
        ("B", 3) => "B1",
        ("C", 4) => "C2",
        ("C#", 4) => "C#2",
        ("D", 4) => "D2",
        ("D#", 4) => "D#2",
        _ => "—",
    }
}

// ── Page KICK SYNTH MODELER ──────────────────────────────────────────────

fn render_kick_page(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();
    let mut params = snap.focus_sound_kick.clone().unwrap_or_default();
    let mut changed = false;

    // 1. Presets rapides
    card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("PRESETS KICK TECHNO PRO")
                    .strong()
                    .small()
                    .color(c.accent),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("Preset actif : {}", params.preset_name))
                        .small()
                        .color(c.accent_cyan),
                );
            });
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            let presets = [
                (0u8, "1. Techno Berlin"),
                (1u8, "2. 909 Punch"),
                (2u8, "3. Hardcore"),
                (3u8, "4. Sub 808"),
                (4u8, "5. Minimal Click"),
                (5u8, "6. Industrial"),
                (6u8, "7. Acid Gabber"),
                (7u8, "8. Deep Dub"),
            ];
            for (p_idx, p_name) in presets {
                let is_sel = params.preset_name.contains(&p_name[3..]);
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(p_name).small().strong())
                            .fill(if is_sel { c.accent } else { c.surface }),
                    )
                    .clicked()
                {
                    app.send(EngineEvent::KickSetPreset(p_idx));
                }
            }
        });
    });

    ui.add_space(8.0);

    // 2. Modélisation : Oscillateur, Sweep & Enveloppe
    card_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new("CORPS & PUNCH DU KICK")
                .strong()
                .small()
                .color(c.accent_cyan),
        );
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            // Forme d'onde
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("FORME D'ONDE")
                        .small()
                        .color(c.text_muted),
                );
                ui.horizontal(|ui| {
                    if chip(ui, "SINE", params.waveform == KickWaveform::Sine, c.accent) {
                        params.waveform = KickWaveform::Sine;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "TRI",
                        params.waveform == KickWaveform::Triangle,
                        c.accent,
                    ) {
                        params.waveform = KickWaveform::Triangle;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "SAT SINE",
                        params.waveform == KickWaveform::SatSine,
                        c.accent,
                    ) {
                        params.waveform = KickWaveform::SatSine;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "SUB SQR",
                        params.waveform == KickWaveform::SquareSub,
                        c.accent,
                    ) {
                        params.waveform = KickWaveform::SquareSub;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "NOISE",
                        params.waveform == KickWaveform::NoiseTransient,
                        c.accent,
                    ) {
                        params.waveform = KickWaveform::NoiseTransient;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "PARABOLIC",
                        params.waveform == KickWaveform::Parabolic,
                        c.accent,
                    ) {
                        params.waveform = KickWaveform::Parabolic;
                        changed = true;
                    }
                    if chip(ui, "RAMP", params.waveform == KickWaveform::Ramp, c.accent) {
                        params.waveform = KickWaveform::Ramp;
                        changed = true;
                    }
                });
            });

            ui.separator();

            // Knobs : Tune, Decay, Hold, Snap, Sweep, FM
            if let Some(v) = knob(
                ui,
                "kick_tune",
                params.tuning_semitones,
                &KnobSpec::linear("ACCORD", " st", -12.0, 12.0, 0.0),
            ) {
                params.tuning_semitones = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "kick_decay",
                params.decay_ms,
                &KnobSpec::linear("DECAY", " ms", 40.0, 1500.0, 320.0),
            ) {
                params.decay_ms = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "kick_hold",
                params.hold_ms,
                &KnobSpec::linear("HOLD", " ms", 0.0, 60.0, 0.0),
            ) {
                params.hold_ms = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "kick_snap",
                params.snap,
                &KnobSpec::linear("SNAP / PUNCH", "", 0.0, 1.0, 0.7),
            ) {
                params.snap = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "kick_sweep",
                params.sweep_time_ms,
                &KnobSpec::linear("SWEEP TIME", " ms", 5.0, 120.0, 28.0),
            ) {
                params.sweep_time_ms = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "kick_fm_amt",
                params.fm_amount,
                &KnobSpec::linear("FM AMT", "", 0.0, 1.0, 0.0),
            ) {
                params.fm_amount = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "kick_fm_ratio",
                params.fm_ratio,
                &KnobSpec::linear("FM RATIO", "x", 0.25, 8.0, 1.0),
            ) {
                params.fm_ratio = v;
                changed = true;
            }
        });
    });

    ui.add_space(8.0);

    // 3. Distorsion multi-types & Filtre résonant
    card_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new("DISTORSION MULTI-TYPES & FILTRE RÉSONANT")
                .strong()
                .small()
                .color(c.warning),
        );
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("DISTO TYPE")
                        .small()
                        .color(c.text_muted),
                );
                ui.horizontal(|ui| {
                    if chip(
                        ui,
                        "TANH",
                        params.drive_type == KickDriveType::AnalogTanh,
                        c.warning,
                    ) {
                        params.drive_type = KickDriveType::AnalogTanh;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "HARD CLIP",
                        params.drive_type == KickDriveType::HardClip,
                        c.error,
                    ) {
                        params.drive_type = KickDriveType::HardClip;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "WAVEFOLDER",
                        params.drive_type == KickDriveType::Wavefolder,
                        c.accent_purple,
                    ) {
                        params.drive_type = KickDriveType::Wavefolder;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "TUBE",
                        params.drive_type == KickDriveType::TubeDrive,
                        c.accent,
                    ) {
                        params.drive_type = KickDriveType::TubeDrive;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "CRUSH",
                        params.drive_type == KickDriveType::Bitcrush,
                        c.accent_cyan,
                    ) {
                        params.drive_type = KickDriveType::Bitcrush;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "FUZZ",
                        params.drive_type == KickDriveType::Fuzz,
                        c.error,
                    ) {
                        params.drive_type = KickDriveType::Fuzz;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "RECT",
                        params.drive_type == KickDriveType::Rectifier,
                        c.warning,
                    ) {
                        params.drive_type = KickDriveType::Rectifier;
                        changed = true;
                    }
                });
            });

            ui.separator();

            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("FILTRE MODE")
                        .small()
                        .color(c.text_muted),
                );
                ui.horizontal(|ui| {
                    if chip(
                        ui,
                        "OFF",
                        params.filter_mode == KickFilterMode::Off,
                        c.text_dim,
                    ) {
                        params.filter_mode = KickFilterMode::Off;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "LP",
                        params.filter_mode == KickFilterMode::LowPass,
                        c.accent,
                    ) {
                        params.filter_mode = KickFilterMode::LowPass;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "HP",
                        params.filter_mode == KickFilterMode::HighPass,
                        c.accent_cyan,
                    ) {
                        params.filter_mode = KickFilterMode::HighPass;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "BP",
                        params.filter_mode == KickFilterMode::BandPass,
                        c.warning,
                    ) {
                        params.filter_mode = KickFilterMode::BandPass;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "NOTCH",
                        params.filter_mode == KickFilterMode::Notch,
                        c.accent_purple,
                    ) {
                        params.filter_mode = KickFilterMode::Notch;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "PEAK",
                        params.filter_mode == KickFilterMode::Peak,
                        c.accent,
                    ) {
                        params.filter_mode = KickFilterMode::Peak;
                        changed = true;
                    }
                });
            });
        });

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if let Some(v) = knob(
                ui,
                "kick_drive",
                params.drive,
                &KnobSpec::linear("DRIVE", "", 0.0, 1.0, 0.4),
            ) {
                params.drive = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "kick_drive_tone",
                params.drive_tone,
                &KnobSpec::linear("TONE", "", 0.0, 1.0, 0.6),
            ) {
                params.drive_tone = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "kick_flt_cut",
                params.filter_cutoff_hz,
                &KnobSpec::log("CUTOFF", " Hz", 40.0, 18000.0, 3500.0),
            ) {
                params.filter_cutoff_hz = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "kick_flt_reso",
                params.filter_reso,
                &KnobSpec::linear("RESO", "", 0.0, 0.95, 0.2),
            ) {
                params.filter_reso = v;
                changed = true;
            }
        });
    });

    ui.add_space(8.0);

    // 4. Infrabasse & Transitoire de bruit
    card_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new("INFRABASSE & COUCHE DE BRUIT")
                .strong()
                .small()
                .color(c.accent),
        );
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if let Some(v) = knob(
                ui,
                "kick_click",
                params.click,
                &KnobSpec::linear("CLICK ATTACK", "", 0.0, 1.0, 0.5),
            ) {
                params.click = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "kick_noise_mix",
                params.noise_mix,
                &KnobSpec::linear("NOISE LAYER", "", 0.0, 1.0, 0.15),
            ) {
                params.noise_mix = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "kick_sub",
                params.sub,
                &KnobSpec::linear("SUB BOOST 50Hz", "", 0.0, 1.0, 0.6),
            ) {
                params.sub = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "kick_sub_ratio",
                params.sub_ratio,
                &KnobSpec::linear("SUB RATIO", "x", 0.5, 2.0, 1.0),
            ) {
                params.sub_ratio = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "kick_vol",
                params.volume_db,
                &KnobSpec::linear("OUTPUT VOL", " dB", -24.0, 6.0, 0.0),
            ) {
                params.volume_db = v;
                changed = true;
            }
        });
    });

    if changed {
        app.send(EngineEvent::KickSetParams(params));
    }
}

// ── Page NEXUS / SERUM WAVETABLE SYNTH ────────────────────────────────────

fn render_wavetable_page(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
) {
    let c = MachColors::dark();
    let mut params = snap.focus_sound_wavetable.clone().unwrap_or_default();
    let mut changed = false;

    // 1. Presets du Wavetable Synth
    card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("PRESETS NEXUS / SERUM")
                    .strong()
                    .small()
                    .color(c.accent_cyan),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Preset : {} [{:?}]",
                        params.name, params.category
                    ))
                    .small()
                    .color(c.accent),
                );
            });
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            let presets = WavetableParams::all_presets();
            for (idx, p) in presets.iter().enumerate() {
                let is_sel = params.name == p.name;
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(format!("{}. {}", idx + 1, p.name))
                                .small()
                                .strong(),
                        )
                        .fill(if is_sel {
                            c.accent_cyan
                        } else {
                            c.surface
                        }),
                    )
                    .clicked()
                {
                    app.send(EngineEvent::WavetableSetPreset(idx));
                }
            }
        });
    });

    ui.add_space(8.0);

    // 2. Oscillateurs & Unison
    card_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new("OSCILLATEURS & TABLES D'ONDES")
                .strong()
                .small()
                .color(c.accent),
        );
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("OSC 1 ONDE")
                        .small()
                        .color(c.text_muted),
                );
                ui.horizontal(|ui| {
                    if chip(
                        ui,
                        "SAW",
                        params.osc1_wave == WavetableWaveform::Saw,
                        c.accent,
                    ) {
                        params.osc1_wave = WavetableWaveform::Saw;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "SUPERSAW",
                        params.osc1_wave == WavetableWaveform::Supersaw,
                        c.accent,
                    ) {
                        params.osc1_wave = WavetableWaveform::Supersaw;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "PIANO",
                        params.osc1_wave == WavetableWaveform::GrandPiano,
                        c.accent,
                    ) {
                        params.osc1_wave = WavetableWaveform::GrandPiano;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "M1",
                        params.osc1_wave == WavetableWaveform::M1Piano,
                        c.accent,
                    ) {
                        params.osc1_wave = WavetableWaveform::M1Piano;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "RHODES",
                        params.osc1_wave == WavetableWaveform::RhodesTines,
                        c.accent,
                    ) {
                        params.osc1_wave = WavetableWaveform::RhodesTines;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "SQR",
                        params.osc1_wave == WavetableWaveform::Square,
                        c.accent,
                    ) {
                        params.osc1_wave = WavetableWaveform::Square;
                        changed = true;
                    }
                });
            });

            ui.separator();

            if let Some(v) = knob(
                ui,
                "wt_osc1_lvl",
                params.osc1_level,
                &KnobSpec::linear("OSC 1 VOL", "", 0.0, 1.0, 0.8),
            ) {
                params.osc1_level = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_osc2_lvl",
                params.osc2_level,
                &KnobSpec::linear("OSC 2 VOL", "", 0.0, 1.0, 0.6),
            ) {
                params.osc2_level = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_unison_detune",
                params.unison_detune,
                &KnobSpec::linear("DETUNE", "", 0.0, 1.0, 0.2),
            ) {
                params.unison_detune = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_unison_spread",
                params.unison_spread,
                &KnobSpec::linear("SPREAD", "", 0.0, 1.0, 0.7),
            ) {
                params.unison_spread = v;
                changed = true;
            }
        });
    });

    ui.add_space(8.0);

    // 3. Filtre Multimode & ADSR Enveloppe
    card_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new("FILTRE MULTIMODE & ENVELOPPE D'AMPLITUDE")
                .strong()
                .small()
                .color(c.warning),
        );
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("FILTRE MODE")
                        .small()
                        .color(c.text_muted),
                );
                ui.horizontal(|ui| {
                    if chip(
                        ui,
                        "LP 24",
                        params.filter_mode == WavetableFilterMode::LowPass24,
                        c.warning,
                    ) {
                        params.filter_mode = WavetableFilterMode::LowPass24;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "LP 12",
                        params.filter_mode == WavetableFilterMode::LowPass12,
                        c.warning,
                    ) {
                        params.filter_mode = WavetableFilterMode::LowPass12;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "HP",
                        params.filter_mode == WavetableFilterMode::HighPass,
                        c.accent_cyan,
                    ) {
                        params.filter_mode = WavetableFilterMode::HighPass;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "BP",
                        params.filter_mode == WavetableFilterMode::BandPass,
                        c.accent,
                    ) {
                        params.filter_mode = WavetableFilterMode::BandPass;
                        changed = true;
                    }
                    if chip(
                        ui,
                        "OFF",
                        params.filter_mode == WavetableFilterMode::Off,
                        c.text_dim,
                    ) {
                        params.filter_mode = WavetableFilterMode::Off;
                        changed = true;
                    }
                });
            });

            ui.separator();

            if let Some(v) = knob(
                ui,
                "wt_cutoff",
                params.filter_cutoff,
                &KnobSpec::log("CUTOFF", " Hz", 20.0, 20000.0, 4500.0),
            ) {
                params.filter_cutoff = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_reso",
                params.filter_reso,
                &KnobSpec::linear("RESO", "", 0.0, 0.95, 0.2),
            ) {
                params.filter_reso = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_env_amt",
                params.filter_env_amt,
                &KnobSpec::linear("ENV AMT", "", -1.0, 1.0, 0.3),
            ) {
                params.filter_env_amt = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_amp_att",
                params.amp_attack_ms,
                &KnobSpec::linear("ATTACK", " ms", 1.0, 1000.0, 12.0),
            ) {
                params.amp_attack_ms = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_amp_dec",
                params.amp_decay_ms,
                &KnobSpec::linear("DECAY", " ms", 10.0, 3000.0, 450.0),
            ) {
                params.amp_decay_ms = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_amp_sus",
                params.amp_sustain,
                &KnobSpec::linear("SUSTAIN", "", 0.0, 1.0, 0.7),
            ) {
                params.amp_sustain = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_amp_rel",
                params.amp_release_ms,
                &KnobSpec::linear("RELEASE", " ms", 10.0, 3000.0, 350.0),
            ) {
                params.amp_release_ms = v;
                changed = true;
            }
        });
    });

    ui.add_space(8.0);

    // 4. Rack d'effets (Chorus, Ping-Pong Delay, Reverb Spatiale)
    card_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new("RACK D'EFFETS INTÉGRÉ")
                .strong()
                .small()
                .color(c.accent_purple),
        );
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if let Some(v) = knob(
                ui,
                "wt_chorus",
                params.fx_chorus_mix,
                &KnobSpec::linear("CHORUS", "", 0.0, 1.0, 0.25),
            ) {
                params.fx_chorus_mix = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_delay_time",
                params.fx_delay_time_ms,
                &KnobSpec::linear("DELAY TIME", " ms", 50.0, 800.0, 350.0),
            ) {
                params.fx_delay_time_ms = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_delay_mix",
                params.fx_delay_mix,
                &KnobSpec::linear("DELAY MIX", "", 0.0, 1.0, 0.2),
            ) {
                params.fx_delay_mix = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_reverb",
                params.fx_reverb_mix,
                &KnobSpec::linear("REVERB", "", 0.0, 1.0, 0.3),
            ) {
                params.fx_reverb_mix = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_disto",
                params.fx_distortion,
                &KnobSpec::linear("DISTORTION", "", 0.0, 1.0, 0.0),
            ) {
                params.fx_distortion = v;
                changed = true;
            }

            if let Some(v) = knob(
                ui,
                "wt_vol",
                params.master_volume_db,
                &KnobSpec::linear("MASTER VOL", " dB", -24.0, 6.0, 0.0),
            ) {
                params.master_volume_db = v;
                changed = true;
            }
        });
    });

    if changed {
        app.send(EngineEvent::WavetableSetParams(params));
    }
}
