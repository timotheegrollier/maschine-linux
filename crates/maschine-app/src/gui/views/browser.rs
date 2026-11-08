//! Vue BROWSER / FILE — presets 808 et navigateur de projets (JSON lisible).
//!
//! - PRESETS : navigation/pré-écoute/chargement des kits batterie, comme le hardware.
//! - FILE/PROJETS : liste des projets, sauvegarde rapide/complète, chargement rapide
//!   depuis la MK3 ou la GUI.

use super::*;
use crate::gui::app::GuiApp;
use crate::gui::theme::{card_frame, MachColors};
use crate::gui::widgets::chip;
use eframe::egui;

use maschine_engine::fx::FxType;
use maschine_engine::EngineEvent;

/// Catégories du browser → index des presets du kit 808.
const CATEGORIES: &[(&str, &[usize])] = &[
    (
        "TOUT",
        &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    ),
    ("KICK", &[0, 15]),
    ("SNARE", &[1, 7]),
    ("HATS", &[2, 3]),
    ("TOMS", &[4, 5, 6]),
    ("PERC", &[8, 9, 10, 11, 12, 13]),
    ("SYNTH", &[14]),
];

pub fn render(ui: &mut egui::Ui, app: &mut GuiApp, snap: &maschine_engine::EngineSnapshot) {
    let c = MachColors::dark();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                crate::gui::theme::view_header(
                    ui,
                    "BROWSER / FILE",
                    "Instruments, presets, effets et projets. Chargez rapidement sur les pads ou groupes.",
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if chip(ui, "EFFETS FX", snap.browser_kind == 3, c.accent_purple) {
                        app.send(EngineEvent::SetBrowserKind(3));
                        app.send(EngineEvent::SetLeftTab(3));
                    }
                    if chip(ui, "PLUGINS / INSTRUMENTS", snap.browser_kind == 2, c.accent_cyan) {
                        app.send(EngineEvent::SetBrowserKind(2));
                        app.send(EngineEvent::SetLeftTab(3));
                    }
                    if chip(ui, "PROJETS / FILE", snap.browser_kind == 1, c.accent_yellow) {
                        app.open_project_browser();
                    }
                    if chip(ui, "PRESETS 808", snap.browser_kind == 0, c.accent) {
                        app.send(EngineEvent::SetBrowserKind(0));
                        app.send(EngineEvent::SetLeftTab(3));
                    }
                });
            });
            ui.add_space(8.0);

            match snap.browser_kind {
                1 => render_projects(ui, app, snap, &c),
                2 => render_plugins(ui, app, snap, &c),
                3 => render_effects(ui, app, snap, &c),
                _ => render_presets(ui, app, snap, &c),
            }
        });
}

fn render_projects(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    c: &MachColors,
) {
    let mut refresh = false;
    let mut load_path: Option<String> = None;
    let mut delete_path: Option<String> = None;
    let mut select_idx: Option<usize> = None;

    ui.horizontal(|ui| {
        let cur = snap
            .current_project_path
            .as_deref()
            .unwrap_or("(nouveau projet)");
        ui.label(
            egui::RichText::new(format!("Projet courant : {}", cur))
                .small()
                .color(c.text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Rafraîchir").clicked() {
                refresh = true;
            }
            if ui
                .add(egui::Button::new("Sauvegarde rapide").fill(c.widget_bg))
                .clicked()
            {
                app.quick_save_project();
            }
            if ui
                .add(egui::Button::new("Sauvegarder").fill(c.accent))
                .clicked()
            {
                app.save_project();
            }
        });
    });

    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(
            "Format : JSON .maschine-linux.json (lisible). MK3 : FILE ouvre la liste, 4D/potar navigue, encoder/soft key charge. Shift+FILE SAVE = sauvegarde rapide.",
        )
        .small()
        .color(c.text_muted),
    );
    ui.add_space(8.0);

    card_frame().show(ui, |ui| {
        ui.label(egui::RichText::new("PROJETS").strong().color(c.accent_cyan));
        ui.add_space(4.0);
        if snap.project_entries.is_empty() {
            ui.label(
                egui::RichText::new("Aucun projet trouvé. Sauvegardez pour créer un fichier dans le dossier projets.")
                    .color(c.text_dim),
            );
        } else {
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (i, entry) in snap.project_entries.iter().enumerate() {
                        let selected = i == snap.project_selected;
                        let is_current = snap
                            .current_project_path
                            .as_ref()
                            .map(|p| p == &entry.path)
                            .unwrap_or(false);
                        let fill = if selected { c.widget_bg } else { c.panel };
                        ui.horizontal(|ui| {
                            let label = format!("{}  ·  {}", entry.name, entry.meta);
                            let resp = ui.add(
                                egui::Button::new(
                                    egui::RichText::new(label)
                                        .color(if selected { c.accent } else { c.text }),
                                )
                                .fill(fill)
                                .min_size(egui::vec2(ui.available_width() - 170.0, 28.0)),
                            );
                            if resp.clicked() {
                                select_idx = Some(i);
                            }
                            if resp.double_clicked() {
                                load_path = Some(entry.path.clone());
                            }
                            if is_current {
                                ui.label(egui::RichText::new("COURANT").small().color(c.success));
                            }
                            if ui.small_button("Charger").clicked() {
                                load_path = Some(entry.path.clone());
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("Supprimer").small().color(c.danger),
                                    )
                                    .fill(c.panel2),
                                )
                                .clicked()
                            {
                                delete_path = Some(entry.path.clone());
                            }
                        });
                    }
                });
        }
    });

    if refresh {
        let settings = crate::settings::AppSettings::load();
        let entries = crate::projects::scan_projects(&settings);
        app.send(EngineEvent::SetProjectEntries(entries));
    }
    if let Some(idx) = select_idx {
        let delta = idx as i32 - snap.project_selected as i32;
        if delta != 0 {
            app.send(EngineEvent::BrowseBrowserStep(delta));
        }
    }
    if let Some(path) = load_path {
        app.load_project_file(std::path::Path::new(&path));
    }
    if let Some(path) = delete_path {
        let p = std::path::Path::new(&path);
        if p.exists() {
            let _ = std::fs::remove_file(p);
        }
        let settings = crate::settings::AppSettings::load();
        let entries = crate::projects::scan_projects(&settings);
        app.send(EngineEvent::SetProjectEntries(entries));
    }
}

fn render_presets(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    c: &MachColors,
) {
    let gf = snap.group_focus as usize;
    let selected = snap.browser_preset_idx as usize;

    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("CATÉGORIE")
                .strong()
                .color(c.accent_cyan),
        );
        ui.add_space(6.0);
        for (i, (cat, _)) in CATEGORIES.iter().enumerate() {
            if chip(ui, cat, i == app.browser_category, c.accent_cyan) {
                app.browser_category = i;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.browser_search)
                    .hint_text("🔍  Rechercher un son…")
                    .desired_width(180.0),
            );
        });
    });
    ui.add_space(8.0);

    let search = app.browser_search.to_lowercase();
    let cat_items = CATEGORIES[app.browser_category.min(CATEGORIES.len() - 1)].1;
    let filtered: Vec<usize> = cat_items
        .iter()
        .copied()
        .filter(|&i| {
            let name = maschine_engine::engine::DRUM_PRESET_NAMES[i];
            search.is_empty() || name.to_lowercase().contains(&search)
        })
        .collect();

    let mut clicked_preset: Option<usize> = None;
    let mut load_preset: Option<usize> = None;

    ui.horizontal_top(|ui| {
        let list_w = (ui.available_width() * 0.5).max(220.0);
        card_frame().show(ui, |ui| {
            ui.set_width(list_w);
            ui.label(
                egui::RichText::new("RÉSULTATS")
                    .strong()
                    .color(c.accent_cyan),
            );
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for &i in &filtered {
                        let name = maschine_engine::engine::DRUM_PRESET_NAMES[i];
                        let is_sel = i == selected;
                        let fill = if is_sel { c.widget_bg } else { c.panel };
                        let resp = ui.add(
                            egui::Button::new(
                                egui::RichText::new(format!("{:02}  {name}", i + 1))
                                    .monospace()
                                    .color(if is_sel { c.accent } else { c.text }),
                            )
                            .fill(fill)
                            .min_size(egui::vec2(list_w - 24.0, 26.0)),
                        );
                        if resp.clicked() {
                            clicked_preset = Some(i);
                        }
                        if resp.double_clicked() {
                            load_preset = Some(i);
                        }
                    }
                    if filtered.is_empty() {
                        ui.label(
                            egui::RichText::new("Aucun résultat.")
                                .italics()
                                .color(c.text_muted),
                        );
                    }
                });
        });

        ui.add_space(8.0);
        card_frame().show(ui, |ui| {
            let name = maschine_engine::engine::DRUM_PRESET_NAMES[selected.min(15)];
            ui.label(
                egui::RichText::new(name)
                    .strong()
                    .size(17.0)
                    .color(c.accent),
            );
            ui.label(
                egui::RichText::new("Preset de batterie — synthèse DSP 808")
                    .small()
                    .color(c.text_dim),
            );
            ui.add_space(10.0);
            crate::gui::theme::section(ui, "CIBLE (SOUND ACTIF)", |ui| {
                let group_name = snap
                    .project
                    .groups
                    .get(gf)
                    .map(|g| g.name.clone())
                    .unwrap_or_default();
                let sound_name = snap
                    .project
                    .groups
                    .get(gf)
                    .and_then(|g| g.sounds.get(snap.active_sound as usize))
                    .and_then(|s| s.as_ref())
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| "(vide)".to_string());
                ui.label(
                    egui::RichText::new(format!(
                        "Groupe {} — {} • Sound {:02} — {sound_name}",
                        group_char(gf),
                        group_name,
                        snap.active_sound + 1
                    ))
                    .color(c.text),
                );
            });
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("▶  PRÉ-ÉCOUTE").strong())
                            .min_size(egui::vec2(220.0, 34.0))
                            .fill(c.widget_bg)
                            .stroke(egui::Stroke::new(1.0_f32, c.success)),
                    )
                    .clicked()
                {
                    app.engine.lock().preview_preset(selected as u8);
                }
                ui.add_space(6.0);
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new("⏎  CHARGER SUR LE SOUND").strong())
                            .min_size(egui::vec2(220.0, 40.0))
                            .fill(c.accent)
                            .stroke(egui::Stroke::new(1.5_f32, c.accent)),
                    )
                    .clicked()
                {
                    let _ = app.engine.lock().handle_event(EngineEvent::LoadDrumPreset {
                        group: snap.group_focus,
                        sound: snap.active_sound,
                        preset_idx: selected as u8,
                    });
                }
            });
        });
    });

    if let Some(p) = clicked_preset {
        {
            let mut eng = app.engine.lock();
            eng.set_browser_preset(p as u8);
        }
        let _ = app
            .engine
            .lock()
            .handle_event(EngineEvent::BrowsePresetStep(p as i32 - selected as i32));
    }
    if let Some(p) = load_preset {
        let _ = app.engine.lock().handle_event(EngineEvent::LoadDrumPreset {
            group: snap.group_focus,
            sound: snap.active_sound,
            preset_idx: p as u8,
        });
    }
}

fn render_plugins(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    c: &MachColors,
) {
    let gf = snap.group_focus;
    let snd = snap.active_sound;
    let pad_name = snap
        .project
        .groups
        .get(gf as usize)
        .and_then(|g| g.sounds.get(snd as usize))
        .and_then(|s| s.as_ref())
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("Sound {}", snd + 1));

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "CIBLE : Groupe {} • Pad {:02} [{}]",
                crate::gui::views::group_char(gf as usize),
                snd + 1,
                pad_name
            ))
            .strong()
            .color(c.accent_cyan),
        );
        ui.label(
            egui::RichText::new("— Sélectionnez un instrument à charger sur ce pad")
                .color(c.text_muted)
                .small(),
        );
    });
    ui.add_space(8.0);

    let plugins = [
        (
            0u8,
            "KICK SYNTH MODELER",
            "MODÉLISATION DE KICK TECHNO / CLUB",
            "Synthèse analogique de kick percussif. Pitch envelope exponentielle ultra-punchy, contrôle du sweep time, transient click réglable, drive / saturation analogique tanh et sub-bass boost dédié 50Hz. 6 presets pros intégrés (Techno Berlin, 909 Punch, Hardcore Drive, Sub 808...).",
            "TECHNO / ANALOG",
            c.accent,
        ),
        (
            1u8,
            "TB-303 ACID BASSLINE",
            "ÉMULATION AUTHENTIQUE ROLAND TB-303",
            "Synthétiseur de basse acid avec filtre passe-bas résonant à diode 24dB, glide/slide analogique legato, accentuation percussive, overdrive tanh et delay stéréo ping-pong synchronisé. Presets rapides intégrés.",
            "ACID / 303",
            c.accent_yellow,
        ),
        (
            2u8,
            "GUITAR AMP STANDALONE",
            "SIMULATEUR DE PRÉAMPLI GUITARE À LAMPES & EFFETS",
            "Saturation à lampes agressive, égalisation 3 bandes active (Bass, Mid, Treble), réverbération à ressort intégrée et noise gate rapide. Conçu pour leads électriques, guitares et basses saturées.",
            "AMP / DRIVE",
            c.danger,
        ),
        (
            3u8,
            "SAMPLER 808 DRUMS",
            "LECTEUR D'ÉCHANTILLONS MASCHINE",
            "Lecteur de samples percussifs stéréo 24/32-bit avec pitch tuning, enveloppe AHD, filtrage multi-pôles, volume et panoramique stéréophonique.",
            "SAMPLE / DRUM",
            c.accent_cyan,
        ),
    ];

    for (idx, title, subtitle, desc, tag, tag_color) in plugins {
        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(title).strong().size(14.0).color(c.text));
                        ui.colored_label(
                            tag_color,
                            egui::RichText::new(format!("[ {tag} ]")).small().strong(),
                        );
                    });
                    ui.label(egui::RichText::new(subtitle).small().color(c.accent_cyan));
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new(desc).small().color(c.text_dim));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_active = match idx {
                        0 => snap.focus_sound_kick.is_some(),
                        1 => snap.focus_sound_acid.is_some(),
                        _ => snap.focus_sound_kick.is_none() && snap.focus_sound_acid.is_none(),
                    };
                    if is_active {
                        ui.label(egui::RichText::new("✓ ACTIF").strong().color(c.success));
                    }
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("Charger sur le Pad").strong())
                                .fill(c.accent)
                                .min_size(egui::vec2(150.0, 32.0)),
                        )
                        .clicked()
                    {
                        app.send(EngineEvent::LoadPluginToSound {
                            group: gf,
                            sound: snd,
                            plugin_idx: idx,
                        });
                        app.send(EngineEvent::SetLeftTab(4));
                        app.view = StudioView::Plugin;
                    }
                });
            });
        });
        ui.add_space(6.0);
    }
}

fn render_effects(
    ui: &mut egui::Ui,
    app: &mut GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    c: &MachColors,
) {
    let gf = snap.group_focus;
    let snd = snap.active_sound;

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "DESTINATION : Pad {:02} ou Groupe {}",
                snd + 1,
                crate::gui::views::group_char(gf as usize)
            ))
            .strong()
            .color(c.accent_purple),
        );
        ui.label(
            egui::RichText::new(
                "— Ajoutez des processeurs DSP sur le pad sélectionné ou sur le groupe entier",
            )
            .color(c.text_muted)
            .small(),
        );
    });
    ui.add_space(8.0);

    // Liste des effets déjà actifs sur le pad et le groupe
    card_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new("CHAÎNE D'EFFETS ACTUELLE")
                .strong()
                .small()
                .color(c.accent_purple),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("Pad {:02} :", snd + 1))
                    .strong()
                    .color(c.text),
            );
            if snap.focus_sound_fx.is_empty() {
                ui.label(
                    egui::RichText::new("(aucun effet)")
                        .small()
                        .color(c.text_muted),
                );
            } else {
                for (slot_idx, fx) in snap.focus_sound_fx.iter().enumerate() {
                    let fx_name = match fx.effect_type {
                        FxType::Reverb => "Reverb",
                        FxType::Delay => "Delay",
                        FxType::Distortion => "Drive",
                        FxType::Filter => "Filter",
                        FxType::Compressor => "Comp",
                    };
                    ui.label(
                        egui::RichText::new(format!("{}. {}", slot_idx + 1, fx_name))
                            .small()
                            .color(c.accent_cyan),
                    );
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "Groupe {} :",
                    crate::gui::views::group_char(gf as usize)
                ))
                .strong()
                .color(c.text),
            );
            if snap.focus_group_fx.is_empty() {
                ui.label(
                    egui::RichText::new("(aucun effet)")
                        .small()
                        .color(c.text_muted),
                );
            } else {
                for (slot_idx, fx) in snap.focus_group_fx.iter().enumerate() {
                    let fx_name = match fx.effect_type {
                        FxType::Reverb => "Reverb",
                        FxType::Delay => "Delay",
                        FxType::Distortion => "Drive",
                        FxType::Filter => "Filter",
                        FxType::Compressor => "Comp",
                    };
                    ui.label(
                        egui::RichText::new(format!("{}. {}", slot_idx + 1, fx_name))
                            .small()
                            .color(c.accent_yellow),
                    );
                }
            }
        });
    });

    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("BIBLIOTHÈQUE D'EFFETS DISPONIBLES")
            .strong()
            .small()
            .color(c.text_dim),
    );
    ui.add_space(4.0);

    let fx_library = [
        (
            FxType::Reverb,
            "RÉVERBÉRATION STÉRÉO",
            "ESPACE & DIFFUSION SPATIALE",
            "Simulateur de réverbération algorithmique pour donner de l'ampleur et de l'espace à vos sons. Paramètres : taille de pièce, damping et mix dry/wet.",
            "ESPACE",
            c.accent_cyan,
        ),
        (
            FxType::Delay,
            "DELAY ÉCHO STÉRÉO",
            "LIGNE À RETARD SYNCHRONISÉE",
            "Délai numérique avec feedback, temps réglable en ms et filtrage passe-bas intégré pour des répétitions douces et atmosphériques.",
            "ÉCHO",
            c.accent_purple,
        ),
        (
            FxType::Distortion,
            "OVERDRIVE / DISTORSION",
            "SATURATION NON-LINÉAIRE ANALOGIQUE",
            "Saturation harmonique basée sur une fonction de transfert tanh, idéale pour rajouter du mordant aux kicks, caisses claires ou synthés.",
            "DRIVE",
            c.danger,
        ),
        (
            FxType::Filter,
            "FILTRE MULTIMODE 12dB",
            "LOWPASS / HIGHPASS / BANDPASS",
            "Filtre résonant modélisé avec coupure de 20 Hz à 20 kHz et résonance prononcée pour sculptage spectral dynamique.",
            "FILTRE",
            c.accent_yellow,
        ),
        (
            FxType::Compressor,
            "COMPRESSEUR DE DYNAMIQUE",
            "CONTRÔLE DU PUNCH & DE LA DYNAMIQUE",
            "Compresseur avec seuil (threshold), ratio, temps d'attaque, release et gain de sortie (makeup) pour compacter vos pistes de batterie.",
            "DYNAMIQUES",
            c.success,
        ),
    ];

    for (fx_type, title, subtitle, desc, tag, tag_color) in fx_library {
        card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(title).strong().size(14.0).color(c.text));
                        ui.colored_label(
                            tag_color,
                            egui::RichText::new(format!("[ {tag} ]")).small().strong(),
                        );
                    });
                    ui.label(egui::RichText::new(subtitle).small().color(c.accent_purple));
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new(desc).small().color(c.text_dim));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("+ Ajouter au Groupe").strong())
                                .fill(c.widget_bg)
                                .min_size(egui::vec2(140.0, 30.0)),
                        )
                        .clicked()
                    {
                        app.send(EngineEvent::AddFxToGroup {
                            group: gf,
                            fx_type: fx_type.clone(),
                        });
                    }
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("+ Ajouter au Pad").strong())
                                .fill(c.accent_purple)
                                .min_size(egui::vec2(130.0, 30.0)),
                        )
                        .clicked()
                    {
                        app.send(EngineEvent::AddFxToSound {
                            group: gf,
                            sound: snd,
                            fx_type,
                        });
                    }
                });
            });
        });
        ui.add_space(6.0);
    }
}
