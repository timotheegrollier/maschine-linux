//! `GuiApp` — état de l'interface et implémentation `eframe::App`.
//!
//! Disposition "Maschine MK3 Studio" :
//! - haut : barre de transport (Play/Stop, REC, tempo, position) ;
//! - gauche : vues (IDEAS/SEQ/MIXER/SAMPLER/BROWSER) + groupes A–H ;
//! - centre : vue active (grille, mixer, sampler, browser) ;
//! - droite : panneau MK3 (miroir temps réel des 2 écrans 480×272, 8 knobs
//!   contextuels, 8 soft keys, 16 pads) — tout le hardware pilotable à la souris.

use crate::gui::theme::MachColors;
use crate::gui::views::StudioView;
use crate::gui::widgets::{knob, mach_button, pad_button, KnobSpec, PadState};
use eframe::egui;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

use maschine_engine::{Engine, EngineEvent};

/// Paramètre pilotable par les 8 knobs contextuels (comme le hardware).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KParam {
    Tempo,
    Swing,
    Master,
    Headphone,
    GrpVol,
    GrpPan,
    SndVol,
    SndPan,
    Cutoff,
    Reso,
    Attack,
    Decay,
    Sustain,
    Release,
}

fn knob_params(view: StudioView) -> &'static [KParam] {
    match view {
        StudioView::Sequencer => &[
            KParam::Tempo,
            KParam::Swing,
            KParam::Master,
            KParam::Headphone,
            KParam::SndVol,
            KParam::SndPan,
            KParam::Cutoff,
            KParam::Reso,
        ],
        StudioView::Mixer => &[
            KParam::GrpVol,
            KParam::GrpPan,
            KParam::Master,
            KParam::Headphone,
            KParam::SndVol,
            KParam::SndPan,
            KParam::Cutoff,
            KParam::Reso,
        ],
        StudioView::Sampler | StudioView::Browser => &[
            KParam::SndVol,
            KParam::SndPan,
            KParam::Cutoff,
            KParam::Reso,
            KParam::Attack,
            KParam::Decay,
            KParam::Sustain,
            KParam::Release,
        ],
        StudioView::Ideas => &[
            KParam::Tempo,
            KParam::Swing,
            KParam::Master,
            KParam::Headphone,
            KParam::GrpVol,
            KParam::GrpPan,
            KParam::SndVol,
            KParam::SndPan,
        ],
    }
}

/// État de l'application GUI.
pub struct GuiApp {
    /// Moteur partagé.
    pub engine: Arc<Mutex<Engine>>,
    /// Sortie audio partagée (réglages).
    #[cfg(feature = "audio")]
    pub audio: Option<Arc<Mutex<Option<crate::audio::AudioOutput>>>>,
    /// Signal de repaint (le thread HID réveille l'UI).
    pub repaint: crate::gui::RepaintSignal,
    /// Fenêtre réglages audio ouverte.
    pub show_settings: bool,
    /// Périphériques disponibles.
    #[cfg(feature = "audio")]
    pub available_devices: Vec<crate::audio::AudioDeviceInfo>,
    /// Périphérique sélectionné.
    #[cfg(feature = "audio")]
    pub selected_device: String,
    /// Message de statut (texte, est_erreur).
    pub status_message: Option<(String, bool)>,
    /// Pad actuellement enfoncé (note off au relâchement).
    pub pad_down: Option<u8>,
    /// Vue active.
    pub view: StudioView,
    /// Dernier onglet gauche vu (synchro hardware).
    pub last_left_tab: u8,
    /// Recherche browser.
    pub browser_search: String,
    /// Catégorie browser.
    pub browser_category: usize,
    /// Chemin du WAV à importer dans le Sound focalisé.
    pub sample_path_input: String,
    /// Résultats d'import traités sur le thread UI.
    sample_load_tx: crossbeam_channel::Sender<(String, bool)>,
    sample_load_rx: crossbeam_channel::Receiver<(String, bool)>,
    /// Dernière révision moteur rendue pour le miroir écrans.
    mirror_revision: u64,
    /// Instant du dernier rendu miroir.
    mirror_last: Instant,
    /// Textures des écrans MK3.
    left_tex: Option<egui::TextureHandle>,
    right_tex: Option<egui::TextureHandle>,
}

impl GuiApp {
    /// Crée l'app (features audio).
    #[cfg(feature = "audio")]
    pub fn new(
        engine: Arc<Mutex<Engine>>,
        audio: Option<Arc<Mutex<Option<crate::audio::AudioOutput>>>>,
        repaint: crate::gui::RepaintSignal,
    ) -> Self {
        let devices = crate::audio::list_output_devices();
        let current_dev = audio
            .as_ref()
            .and_then(|a| {
                let guard = a.lock();
                (*guard).as_ref().map(|o| o.device_name().to_string())
            })
            .unwrap_or_else(|| "default".to_string());
        let settings = crate::settings::AppSettings::load();
        let selected = settings.audio_device.unwrap_or(current_dev);
        let (sample_load_tx, sample_load_rx) = crossbeam_channel::unbounded();
        Self {
            engine,
            audio,
            repaint,
            show_settings: false,
            available_devices: devices,
            selected_device: selected,
            status_message: None,
            pad_down: None,
            view: StudioView::Sequencer,
            last_left_tab: 0,
            browser_search: String::new(),
            browser_category: 0,
            sample_path_input: String::new(),
            sample_load_tx,
            sample_load_rx,
            mirror_revision: u64::MAX,
            mirror_last: Instant::now() - Duration::from_secs(10),
            left_tex: None,
            right_tex: None,
        }
    }

    /// Crée l'app (sans audio).
    #[cfg(not(feature = "audio"))]
    pub fn new(engine: Arc<Mutex<Engine>>, repaint: crate::gui::RepaintSignal) -> Self {
        let (sample_load_tx, sample_load_rx) = crossbeam_channel::unbounded();
        Self {
            engine,
            repaint,
            show_settings: false,
            selected_device: "disabled".to_string(),
            status_message: None,
            pad_down: None,
            view: StudioView::Sequencer,
            last_left_tab: 0,
            browser_search: String::new(),
            browser_category: 0,
            sample_path_input: String::new(),
            sample_load_tx,
            sample_load_rx,
            mirror_revision: u64::MAX,
            mirror_last: Instant::now() - Duration::from_secs(10),
            left_tex: None,
            right_tex: None,
        }
    }

    // ── Actions moteur ───────────────────────────────────────────────

    /// Bascule un pas du séquenceur (tick absolu).
    pub fn toggle_step(&self, pad: u8, tick: u32) {
        self.engine.lock().toggle_step(pad, tick);
    }

    /// Remplit 4-on-the-floor.
    pub fn fill_four_on_the_floor(&self, pad: u8, bar: usize) {
        self.engine.lock().fill_four_on_the_floor(pad, bar);
    }

    /// Remplit les croches.
    pub fn fill_eighths(&self, pad: u8, bar: usize) {
        self.engine.lock().fill_eighths(pad, bar);
    }

    /// Vide une mesure.
    pub fn clear_bar(&self, bar: usize) {
        self.engine.lock().clear_bar(bar);
    }

    /// Reset groove démo.
    pub fn reset_demo_groove(&self) {
        self.engine.lock().reset_demo_groove();
    }

    /// Envoie un événement moteur.
    pub fn send(&self, ev: EngineEvent) {
        let _ = self.engine.lock().handle_event(ev);
    }

    /// Décode le WAV sur un thread de travail puis installe ses données sans
    /// bloquer l'interface ni garder le verrou moteur pendant l'accès disque.
    pub fn load_sample_async(&mut self, group: u8, sound: u8) {
        let path = std::path::PathBuf::from(self.sample_path_input.trim());
        if path.as_os_str().is_empty() {
            self.status_message = Some(("Indique un chemin de fichier WAV.".to_string(), true));
            return;
        }
        let engine = Arc::clone(&self.engine);
        let result_tx = self.sample_load_tx.clone();
        let repaint = self.repaint.clone();
        self.status_message = Some((format!("Chargement {}…", path.display()), false));
        let spawn = std::thread::Builder::new()
            .name("maschine-sample-loader".to_string())
            .spawn(move || {
                let result = maschine_engine::Sampler::prepare_wav(&path, 36 + sound)
                    .map_err(|error| error.to_string())
                    .and_then(|zone| {
                        engine
                            .lock()
                            .handle_event(EngineEvent::LoadPreparedSample { group, sound, zone })
                            .map_err(|error| error.to_string())
                    });
                let message = match result {
                    Ok(()) => (format!("Sample chargé : {}", path.display()), false),
                    Err(error) => (format!("Échec du chargement WAV : {error}"), true),
                };
                let _ = result_tx.send(message);
                repaint.notify();
            });
        if let Err(error) = spawn {
            self.status_message = Some((format!("Impossible de lancer l'import : {error}"), true));
        }
    }

    /// Résumé des stats audio (callbacks, misses, µs max).
    #[must_use]
    pub fn audio_stats_text(&self) -> String {
        #[cfg(feature = "audio")]
        {
            if let Some(audio) = self.audio.as_ref() {
                let guard = audio.lock();
                if let Some(out) = guard.as_ref() {
                    let misses = out
                        .stats
                        .lock_misses
                        .load(std::sync::atomic::Ordering::Relaxed);
                    let max_us = out
                        .stats
                        .max_callback_us
                        .load(std::sync::atomic::Ordering::Relaxed);
                    return if misses == 0 {
                        format!("Audio OK • ≤{max_us} µs")
                    } else {
                        format!("Audio ⚠ {misses} miss • ≤{max_us} µs")
                    };
                }
            }
            "Audio —".to_string()
        }
        #[cfg(not(feature = "audio"))]
        {
            "Sans audio".to_string()
        }
    }

    #[cfg(feature = "audio")]
    pub fn refresh_devices(&mut self) {
        self.available_devices = crate::audio::list_output_devices();
    }

    #[cfg(feature = "audio")]
    pub fn switch_audio_device(&mut self, target: &str) {
        if let Some(ref audio_holder) = self.audio {
            {
                let mut guard = audio_holder.lock();
                *guard = None;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
            let hp_dup = true;
            match crate::audio::AudioOutput::open(self.engine.clone(), Some(target), hp_dup) {
                Ok(new_out) => {
                    let dev_name = new_out.device_name().to_string();
                    let rate = new_out.sample_rate();
                    *audio_holder.lock() = Some(new_out);
                    self.selected_device = dev_name.clone();
                    self.status_message =
                        Some((format!("✓ Connecté à '{dev_name}' ({rate} Hz)"), false));
                    let mut s = crate::settings::AppSettings::load();
                    s.audio_device = Some(target.to_string());
                    let _ = s.save();
                }
                Err(e) => {
                    self.status_message = Some((
                        format!("Erreur lors de l'ouverture de '{target}': {e}"),
                        true,
                    ));
                }
            }
        }
    }

    // ── Raccourcis clavier ───────────────────────────────────────────

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let (space, digits, view_keys, save) = ctx.input(|i| {
            let mut digits = [false; 8];
            for (d, pressed) in digits.iter_mut().enumerate() {
                let key = match d {
                    0 => egui::Key::Num1,
                    1 => egui::Key::Num2,
                    2 => egui::Key::Num3,
                    3 => egui::Key::Num4,
                    4 => egui::Key::Num5,
                    5 => egui::Key::Num6,
                    6 => egui::Key::Num7,
                    _ => egui::Key::Num8,
                };
                *pressed = i.key_pressed(key);
            }
            let view_keys = [
                (egui::Key::I, StudioView::Ideas),
                (egui::Key::S, StudioView::Sequencer),
                (egui::Key::M, StudioView::Mixer),
                (egui::Key::A, StudioView::Sampler),
                (egui::Key::B, StudioView::Browser),
            ]
            .map(|(k, v)| (i.key_pressed(k), v));
            (
                i.key_pressed(egui::Key::Space),
                digits,
                view_keys,
                i.modifiers.command && i.key_pressed(egui::Key::S),
            )
        });

        for (pressed, v) in view_keys {
            if pressed {
                self.view = v;
                self.send(EngineEvent::SetLeftTab(v.tab_index()));
            }
        }
        for (d, pressed) in digits.iter().enumerate() {
            if *pressed {
                self.send(EngineEvent::GroupFocus(d as u8));
            }
        }
        if save {
            self.save_project();
        }
        if space {
            let is_playing = self.engine.lock().transport.is_playing();
            self.send(EngineEvent::Transport {
                play: !is_playing,
                rec: false,
                stop: is_playing,
            });
        }
    }

    /// Autosave projet (bouton FILESAVE hardware / Ctrl+S).
    pub fn save_project(&mut self) {
        let base = std::env::var("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|h| std::path::PathBuf::from(h).join(".local/share"))
                    .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            });
        let dir = base.join("maschine-linux");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("autosave.json");
        let eng = self.engine.lock();
        let snap = eng.snapshot();
        match serde_json::to_string_pretty(&*snap.project) {
            Ok(json) => {
                let res = std::fs::write(&path, json);
                self.status_message = Some((
                    if res.is_ok() {
                        format!("✓ Projet sauvegardé → {}", path.display())
                    } else {
                        format!("Erreur d'écriture → {}", path.display())
                    },
                    res.is_err(),
                ));
            }
            Err(e) => self.status_message = Some((format!("Erreur sérialisation: {e}"), true)),
        }
    }

    // ── Panneau MK3 ─────────────────────────────────────────────────

    fn apply_knob(&self, param: KParam, v: f32, snap: &maschine_engine::EngineSnapshot) {
        let gf = snap.group_focus;
        let snd = snap.active_sound;
        match param {
            KParam::Tempo => self.send(EngineEvent::Bpm(v)),
            KParam::Swing => self.engine.lock().set_swing(v),
            KParam::Master => self.send(EngineEvent::MasterVolume(v)),
            KParam::Headphone => self.send(EngineEvent::HeadphoneVolume(v)),
            KParam::GrpVol => self.send(EngineEvent::GroupVolume {
                group: gf,
                volume_db: v,
            }),
            KParam::GrpPan => self.send(EngineEvent::GroupPan { group: gf, pan: v }),
            KParam::SndVol => self.send(EngineEvent::SoundVolume {
                group: gf,
                sound: snd,
                volume_db: v,
            }),
            KParam::SndPan => self.send(EngineEvent::SoundPan {
                group: gf,
                sound: snd,
                pan: v,
            }),
            KParam::Cutoff => self.send(EngineEvent::SoundFilterCutoff {
                group: gf,
                sound: snd,
                cutoff_hz: v,
            }),
            KParam::Reso => self.send(EngineEvent::SoundFilterReso {
                group: gf,
                sound: snd,
                resonance: v,
            }),
            KParam::Attack | KParam::Decay | KParam::Sustain | KParam::Release => {
                let (mut a, mut d, mut s, mut r) = (2.0f32, 100.0f32, 0.8f32, 150.0f32);
                if let Some(sound) = snap
                    .project
                    .groups
                    .get(gf as usize)
                    .and_then(|g| g.sounds.get(snd as usize))
                    .and_then(|s| s.as_ref())
                {
                    a = sound.adsr.attack_ms;
                    d = sound.adsr.decay_ms;
                    s = sound.adsr.sustain;
                    r = sound.adsr.release_ms;
                }
                match param {
                    KParam::Attack => a = v,
                    KParam::Decay => d = v,
                    KParam::Sustain => s = v,
                    _ => r = v,
                }
                self.send(EngineEvent::SoundAdsr {
                    group: gf,
                    sound: snd,
                    attack_ms: a,
                    decay_ms: d,
                    sustain: s,
                    release_ms: r,
                });
            }
        }
    }

    fn render_mk3_knobs(&mut self, ui: &mut egui::Ui, snap: &maschine_engine::EngineSnapshot) {
        let params = knob_params(self.view);
        let gf = snap.group_focus as usize;
        let snd = snap.active_sound as usize;
        let group = snap.project.groups.get(gf);
        let sound = group
            .and_then(|g| g.sounds.get(snd))
            .and_then(|s| s.as_ref());
        let (vol_db, pan, cutoff, reso, adsr) = sound
            .map(|s| {
                (
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
            .unwrap_or((0.0, 0.0, 20000.0, 0.0, (2.0, 100.0, 0.8, 150.0)));
        let grp_vol = group.map(|g| g.volume_db).unwrap_or(0.0);
        let grp_pan = group.map(|g| g.pan).unwrap_or(0.0);

        ui.horizontal(|ui| {
            for (i, param) in params.iter().enumerate() {
                let (spec, value) = match param {
                    KParam::Tempo => (
                        KnobSpec::linear("TEMPO", " BPM", 20.0, 300.0, 120.0),
                        snap.transport.bpm,
                    ),
                    KParam::Swing => (
                        KnobSpec::linear("SWING", " %", 0.0, 1.0, 0.0),
                        snap.transport.swing,
                    ),
                    KParam::Master => (
                        KnobSpec::linear("MASTER", " %", 0.0, 1.0, 1.0),
                        snap.master_volume,
                    ),
                    KParam::Headphone => (
                        KnobSpec::linear("CASQUE", " %", 0.0, 1.0, 0.7),
                        snap.headphone_volume,
                    ),
                    KParam::GrpVol => (KnobSpec::linear("VOL ", " dB", -60.0, 12.0, 0.0), grp_vol),
                    KParam::GrpPan => (KnobSpec::linear("PAN ", "", -1.0, 1.0, 0.0), grp_pan),
                    KParam::SndVol => {
                        (KnobSpec::linear("VOL SND", " dB", -60.0, 12.0, 0.0), vol_db)
                    }
                    KParam::SndPan => (KnobSpec::linear("PAN SND", "", -1.0, 1.0, 0.0), pan),
                    KParam::Cutoff => (
                        KnobSpec::log("CUTOFF", " Hz", 20.0, 20000.0, 20000.0),
                        cutoff,
                    ),
                    KParam::Reso => (KnobSpec::linear("RESO", " %", 0.0, 1.0, 0.0), reso),
                    KParam::Attack => (KnobSpec::linear("ATTACK", " ms", 0.0, 2000.0, 2.0), adsr.0),
                    KParam::Decay => (KnobSpec::linear("DECAY", " ms", 5.0, 2000.0, 100.0), adsr.1),
                    KParam::Sustain => (KnobSpec::linear("SUSTAIN", " %", 0.0, 1.0, 0.8), adsr.2),
                    KParam::Release => (
                        KnobSpec::linear("RELEASE", " ms", 5.0, 3000.0, 150.0),
                        adsr.3,
                    ),
                };
                if let Some(v) = knob(ui, ("mk3_knob", i), value, &spec) {
                    self.apply_knob(*param, v, snap);
                }
            }
        });
    }

    fn render_displays(&mut self, ui: &mut egui::Ui, snap: &maschine_engine::EngineSnapshot) {
        let now = Instant::now();
        let due = snap.revision != self.mirror_revision
            && now.duration_since(self.mirror_last) >= Duration::from_millis(28);
        if due || self.left_tex.is_none() {
            let (left, right) = maschine_engine::display_render::render_screens(snap);
            let left_img = crate::gui::widgets::display_pixels_to_image(
                left.w as usize,
                left.h as usize,
                &left.pixels,
            );
            let right_img = crate::gui::widgets::display_pixels_to_image(
                right.w as usize,
                right.h as usize,
                &right.pixels,
            );
            let opts = egui::TextureOptions::LINEAR;
            if let Some(t) = &mut self.left_tex {
                t.set(left_img, opts);
            } else {
                self.left_tex = Some(ui.ctx().load_texture("mk3_left", left_img, opts));
            }
            if let Some(t) = &mut self.right_tex {
                t.set(right_img, opts);
            } else {
                self.right_tex = Some(ui.ctx().load_texture("mk3_right", right_img, opts));
            }
            self.mirror_revision = snap.revision;
            self.mirror_last = now;
        }

        let avail = ui.available_width();
        let scale = (avail / 480.0).min(1.0);
        let size = egui::vec2(480.0 * scale, 272.0 * scale);

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("ÉCRAN GAUCHE — MK3")
                    .small()
                    .color(MachColors::dark().text_muted),
            );
        });
        if let Some(tex) = self.left_tex.clone() {
            ui.add(egui::Image::new(egui::load::SizedTexture::new(
                tex.id(),
                size,
            )));
        }

        // 4 soft keys sous l'écran gauche (1..4), comme le hardware.
        self.render_soft_row(ui, snap, 0, size.x);

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("ÉCRAN DROIT — MK3")
                    .small()
                    .color(MachColors::dark().text_muted),
            );
        });
        if let Some(tex) = self.right_tex.clone() {
            ui.add(egui::Image::new(egui::load::SizedTexture::new(
                tex.id(),
                size,
            )));
        }

        // 4 soft keys sous l'écran droit (5..8).
        self.render_soft_row(ui, snap, 4, size.x);
    }

    /// Rangée de 4 soft keys sous un écran (actions contextuelles de l'Engine).
    fn render_soft_row(
        &mut self,
        ui: &mut egui::Ui,
        snap: &maschine_engine::EngineSnapshot,
        start: usize,
        width: f32,
    ) {
        let c = MachColors::dark();
        ui.horizontal(|ui| {
            for i in start..(start + 4).min(snap.soft_labels.len()) {
                let label = snap.soft_labels[i];
                let active = match self.view {
                    StudioView::Sequencer => {
                        (i < 4 && i == snap.selected_bar)
                            || (i >= 4
                                && [1usize, 2, 4, 8].get(i - 4).copied().unwrap_or(0)
                                    == snap.pattern_bars)
                    }
                    StudioView::Mixer => i == snap.group_focus as usize,
                    StudioView::Sampler => i == snap.active_sound as usize,
                    _ => false,
                };
                let btn = egui::Button::new(egui::RichText::new(label).size(10.0).strong())
                    .min_size(egui::vec2((width - 6.0) / 4.0, 24.0))
                    .fill(if active { c.accent } else { c.widget_bg })
                    .stroke(egui::Stroke::new(1.0_f32, c.border));
                if ui.add(btn).clicked() {
                    self.send(EngineEvent::SoftKey(i as u8 + 1));
                }
            }
        });
    }

    fn render_pads(&mut self, ui: &mut egui::Ui, snap: &maschine_engine::EngineSnapshot) {
        let gf = snap.group_focus as usize;
        let pad_size = ((ui.available_width() - 24.0) / 4.0).clamp(40.0, 62.0);
        let mut press: Option<(u8, u8)> = None;
        let mut release: Option<u8> = None;

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                for row in 0..4usize {
                    ui.horizontal(|ui| {
                        for col in 0..4usize {
                            // Disposition Maschine : bas-gauche = pad 1.
                            let index = ((3 - row) * 4 + col) as u8;
                            let name = snap
                                .project
                                .groups
                                .get(gf)
                                .and_then(|g| g.sounds.get(index as usize))
                                .and_then(|s| s.as_ref())
                                .map(|s| s.name.clone())
                                .unwrap_or_else(|| "—".to_string());
                            let st = PadState {
                                index,
                                color: crate::gui::views::pad_color(index as usize),
                                label: &name,
                                flash: snap.triggered_pads[index as usize],
                                selected: index == snap.active_sound,
                                was_down: self.pad_down == Some(index),
                            };
                            let inter = pad_button(ui, &st, pad_size);
                            if inter.pressed {
                                press = Some((index, inter.velocity));
                            }
                            if inter.released {
                                release = Some(index);
                            }
                            if inter.secondary_clicked {
                                // Clic droit : sélectionner le Sound sans le jouer.
                                self.send(EngineEvent::SelectSound(index));
                            }
                        }
                    });
                }
            });

            ui.add_space(6.0);

            // Colonne : groupes A–H + info.
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("GROUPES")
                        .small()
                        .color(MachColors::dark().text_dim),
                );
                for row in 0..2usize {
                    ui.horizontal(|ui| {
                        for col in 0..4usize {
                            let g = row * 4 + col;
                            let active = g == snap.group_focus as usize;
                            let muted = (snap.muted_groups & (1 << g)) != 0;
                            let soloed = (snap.soloed_groups & (1 << g)) != 0;
                            let mut btn = egui::Button::new(
                                egui::RichText::new(format!(
                                    "{}",
                                    crate::gui::views::group_char(g)
                                ))
                                .strong()
                                .color(if active {
                                    egui::Color32::BLACK
                                } else if muted {
                                    MachColors::dark().warning
                                } else {
                                    MachColors::dark().text
                                }),
                            )
                            .min_size(egui::vec2(pad_size / 1.6, pad_size / 2.4))
                            .fill(if active {
                                MachColors::dark().accent
                            } else if soloed {
                                MachColors::dark().accent_cyan
                            } else {
                                MachColors::dark().widget_bg
                            })
                            .stroke(egui::Stroke::new(1.0_f32, MachColors::dark().border));
                            if muted {
                                btn = btn
                                    .stroke(egui::Stroke::new(1.5_f32, MachColors::dark().warning));
                            }
                            if ui.add(btn).clicked() {
                                self.send(EngineEvent::GroupFocus(g as u8));
                            }
                        }
                    });
                }
                ui.add_space(6.0);
                // Scène active + mode.
                let scene_txt = match snap.active_scene {
                    Some(s) => format!("Scène {} active", s + 1),
                    None => "Scène —".to_string(),
                };
                ui.label(
                    egui::RichText::new(scene_txt)
                        .small()
                        .color(MachColors::dark().text_dim),
                );
                ui.label(
                    egui::RichText::new(format!("Sound actif : {:02}", snap.active_sound + 1))
                        .small()
                        .color(MachColors::dark().text_dim),
                );
            });
        });

        if let Some((pad, velocity)) = press {
            self.pad_down = Some(pad);
            self.send(EngineEvent::Pad {
                pad,
                velocity,
                pressed: true,
            });
        }
        if let Some(pad) = release {
            self.pad_down = None;
            self.send(EngineEvent::Pad {
                pad,
                velocity: 0,
                pressed: false,
            });
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.repaint.attach(ctx);
        self.handle_keyboard(ctx);
        while let Ok(message) = self.sample_load_rx.try_recv() {
            self.status_message = Some(message);
        }

        // Snapshot O(1) (Arc<Project>) : aucune copie du projet par frame.
        let snap = self.engine.lock().snapshot();

        let is_playing = snap.transport.is_playing();

        // Synchro onglet gauche provenant du hardware (boutons écran MK3).
        if snap.left_tab != self.last_left_tab {
            self.last_left_tab = snap.left_tab;
            self.view = match snap.left_tab {
                1 => StudioView::Mixer,
                2 => StudioView::Sampler,
                3 => StudioView::Browser,
                _ => StudioView::Sequencer,
            };
        }

        egui::TopBottomPanel::top("transport").show(ctx, |ui| {
            crate::gui::transport::render_transport(ui, self, &snap);
        });

        // Rail gauche : vues + groupes.
        egui::SidePanel::left("nav")
            .resizable(false)
            .default_width(196.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                for (v, label) in [
                    (StudioView::Ideas, "🧭 IDEAS / SONG"),
                    (StudioView::Sequencer, "🎹 SÉQUENCEUR"),
                    (StudioView::Mixer, "🎚 MIXER"),
                    (StudioView::Sampler, "🎛 SAMPLER"),
                    (StudioView::Browser, "🗂 BROWSER"),
                ] {
                    let active = self.view == v;
                    if mach_button(ui, label, active).clicked {
                        self.view = v;
                        let tab = v.tab_index();
                        // Re-sélectionner la vue active restaure aussi sa page
                        // droite contextuelle après EVENTS/MACRO/SETTINGS.
                        self.last_left_tab = tab;
                        self.send(EngineEvent::SetLeftTab(tab));
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ui.colored_label(
                    MachColors::dark().accent_cyan,
                    egui::RichText::new("GROUPE FOCUS").strong().small(),
                );
                egui::Grid::new("nav_groups")
                    .spacing(egui::vec2(4.0, 4.0))
                    .show(ui, |ui| {
                        for row in 0..2usize {
                            for col in 0..4usize {
                                let g = row * 4 + col;
                                let active = g == snap.group_focus as usize;
                                let muted = (snap.muted_groups & (1 << g)) != 0;
                                let label = format!("{}", crate::gui::views::group_char(g));
                                let mut btn =
                                    egui::Button::new(egui::RichText::new(label).strong())
                                        .min_size(egui::vec2(38.0, 28.0));
                                if active {
                                    btn = btn.fill(MachColors::dark().accent);
                                }
                                if muted {
                                    btn = btn.stroke(egui::Stroke::new(
                                        1.5_f32,
                                        MachColors::dark().warning,
                                    ));
                                }
                                if ui.add(btn).clicked() {
                                    self.send(EngineEvent::GroupFocus(g as u8));
                                }
                            }
                            ui.end_row();
                        }
                    });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);
                crate::gui::theme::caption(ui, format!("Position : {}", snap.bbt));
                crate::gui::theme::caption(
                    ui,
                    format!("Pattern : {} mesure(s)", snap.pattern_bars),
                );

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(4.0);
                    if mach_button(ui, "💾 SAUVEGARDER", false).clicked {
                        self.save_project();
                    }
                    if mach_button(ui, "▶ TEST KICK", false).clicked {
                        self.send(EngineEvent::Pad {
                            pad: 0,
                            velocity: 115,
                            pressed: true,
                        });
                    }
                });
            });

        // Rail droit : panneau MK3 (écrans temps réel + knobs + soft keys + pads).
        egui::SidePanel::right("mk3_panel")
            .resizable(false)
            .default_width(498.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(4.0);
                        ui.colored_label(
                            MachColors::dark().accent,
                            egui::RichText::new("⌨ MASCHINE MK3 — MIROIR TEMPS RÉEL").strong(),
                        );
                        ui.add_space(4.0);

                        // 8 knobs contextuels.
                        self.render_mk3_knobs(ui, &snap);

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Les deux écrans physiques en direct.
                        self.render_displays(ui, &snap);

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(6.0);

                        // 16 pads.
                        self.render_pads(ui, &snap);
                    });
            });

        // Espace de travail central.
        egui::CentralPanel::default().show(ctx, |ui| {
            crate::gui::views::render_view(ui, self, &snap, is_playing);
        });

        // Barre de statut basse.
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Maschine MK3 Linux • {} • {} groupes • {} scènes",
                        snap.project.name,
                        snap.project.groups.len(),
                        snap.project.scenes.len()
                    ))
                    .small()
                    .color(MachColors::dark().text_muted),
                );
                if let Some((msg, is_err)) = &self.status_message {
                    let col = if *is_err {
                        MachColors::dark().error
                    } else {
                        MachColors::dark().success
                    };
                    ui.colored_label(col, egui::RichText::new(msg).small());
                }
            });
        });

        // Fenêtre réglages audio (HEAD).
        #[cfg(feature = "audio")]
        if self.show_settings {
            let mut settings_open = self.show_settings;
            let mut switch_target: Option<String> = None;
            let c = MachColors::dark();
            egui::Window::new("⚙ Paramètres Audio — Choix de la Sortie Son")
                .open(&mut settings_open)
                .default_width(540.0)
                .min_width(460.0)
                .resizable(true)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.add_space(4.0);
                    let active_info = self.audio.as_ref().and_then(|a| {
                        let guard = a.lock();
                        (*guard)
                            .as_ref()
                            .map(|o| (o.device_name().to_string(), o.sample_rate()))
                    });

                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            if let Some((ref dev, rate)) = active_info {
                                ui.colored_label(c.success, egui::RichText::new("●").size(18.0));
                                ui.vertical(|ui| {
                                    ui.colored_label(
                                        egui::Color32::WHITE,
                                        egui::RichText::new(format!("Sortie active : {dev}"))
                                            .strong()
                                            .size(13.0),
                                    );
                                    ui.colored_label(c.text_dim, format!("Fréquence : {rate} Hz"));
                                });
                            } else {
                                ui.colored_label(c.error, egui::RichText::new("●").size(18.0));
                                ui.colored_label(
                                    c.error,
                                    egui::RichText::new("Aucun flux audio actif").strong(),
                                );
                            }
                        });
                    });

                    ui.add_space(10.0);
                    ui.colored_label(
                        c.accent_cyan,
                        egui::RichText::new("SÉLECTION RAPIDE DE LA SORTIE")
                            .strong()
                            .size(12.0),
                    );
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        if ui
                            .button(egui::RichText::new("🔊 Haut-parleurs PC").strong())
                            .clicked()
                        {
                            switch_target = Some("default".to_string());
                        }
                        if ui
                            .button(egui::RichText::new("🎛 Maschine MK3").strong())
                            .clicked()
                        {
                            switch_target = Some("maschine".to_string());
                        }
                    });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let dev_names: Vec<String> = self
                            .available_devices
                            .iter()
                            .cloned()
                            .map(|d| d.name)
                            .collect();
                        let mut sel = self
                            .available_devices
                            .iter()
                            .position(|d| d.name == self.selected_device)
                            .unwrap_or(0);
                        egui::ComboBox::from_label("Périphériques")
                            .selected_text(self.selected_device.as_str())
                            .show_ui(ui, |ui| {
                                for (idx, name) in dev_names.iter().enumerate() {
                                    ui.selectable_value(&mut sel, idx, name.as_str());
                                }
                            });
                        if let Some(chosen) = dev_names.into_iter().nth(sel) {
                            if chosen != self.selected_device {
                                self.selected_device = chosen;
                            }
                        }
                    });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(
                                egui::RichText::new("✔ Activer le périphérique sélectionné")
                                    .strong(),
                            )
                            .clicked()
                        {
                            switch_target = Some(self.selected_device.clone());
                        }
                        ui.colored_label(
                            c.text_dim,
                            format!("Sélectionné : {}", self.selected_device),
                        );
                    });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        for (label, pad_no, vel) in [
                            ("▶ Kick 808", 0u8, 115u8),
                            ("▶ Snare 808", 1u8, 115u8),
                            ("▶ Hi-Hat", 2u8, 110u8),
                            ("▶ Clap", 7u8, 110u8),
                        ] {
                            if ui
                                .button(egui::RichText::new(label).strong().size(13.0))
                                .clicked()
                            {
                                self.send(EngineEvent::Pad {
                                    pad: pad_no,
                                    velocity: vel,
                                    pressed: true,
                                });
                            }
                        }
                    });

                    ui.add_space(6.0);
                    let mut master_vol = snap.master_volume;
                    ui.horizontal(|ui| {
                        ui.label("Volume Master :");
                        if ui
                            .add(egui::Slider::new(&mut master_vol, 0.0..=1.0))
                            .changed()
                        {
                            self.send(EngineEvent::MasterVolume(master_vol));
                        }
                    });

                    let mut hp_vol = snap.headphone_volume;
                    ui.horizontal(|ui| {
                        ui.label("Volume Casque :");
                        if ui.add(egui::Slider::new(&mut hp_vol, 0.0..=1.0)).changed() {
                            self.send(EngineEvent::HeadphoneVolume(hp_vol));
                        }
                    });

                    if let Some((ref msg, is_err)) = self.status_message {
                        ui.add_space(8.0);
                        let col = if is_err { c.error } else { c.success };
                        ui.colored_label(col, egui::RichText::new(msg).strong());
                    }
                });

            self.show_settings = settings_open;
            if let Some(target) = switch_target {
                self.switch_audio_device(&target);
            }
        }

        // Réactivité "tac-au-tac" : repaint continu pendant la lecture
        // (playhead, flash pads, VU-mètres), sinon rafraîchi à 20 Hz + sur
        // événement hardware (RepaintSignal.notify()).
        if is_playing {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}

/// Lance la GUI native (main thread, requise par egui/winit).
#[cfg(feature = "audio")]
pub fn run_native_gui(
    engine: Arc<Mutex<maschine_engine::Engine>>,
    audio: Arc<Mutex<Option<crate::audio::AudioOutput>>>,
    repaint: crate::gui::RepaintSignal,
) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1560.0, 960.0])
            .with_min_inner_size([1180.0, 760.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Maschine MK3 Linux",
        options,
        Box::new(move |cc| {
            crate::gui::theme::apply_theme(&cc.egui_ctx);
            Ok(Box::new(GuiApp::new(engine, Some(audio), repaint)) as Box<dyn eframe::App>)
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {e}"))
}

/// Lance la GUI native sans audio.
#[cfg(not(feature = "audio"))]
pub fn run_native_gui(
    engine: Arc<Mutex<maschine_engine::Engine>>,
    repaint: crate::gui::RepaintSignal,
) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1560.0, 960.0])
            .with_min_inner_size([1180.0, 760.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Maschine MK3 Linux",
        options,
        Box::new(move |cc| {
            crate::gui::theme::apply_theme(&cc.egui_ctx);
            Ok(Box::new(GuiApp::new(engine, repaint)) as Box<dyn eframe::App>)
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {e}"))
}
