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
use crate::gui::widgets::{mach_button, pad_button, PadState};
use eframe::egui;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

use maschine_engine::{Engine, EngineEvent};

/// Taille minimale d'un pad du miroir MK3 avant de faire défiler le panneau.
const PAD_MIN: f32 = 48.0;

/// État de l'application GUI.
pub struct GuiApp {
    /// Moteur partagé.
    pub engine: Arc<Mutex<Engine>>,
    /// Sortie audio partagée (réglages).
    #[cfg(feature = "audio")]
    pub audio: Option<Arc<Mutex<Option<crate::audio::AudioOutput>>>>,
    /// Signal de repaint (le thread HID réveille l'UI).
    pub repaint: crate::gui::RepaintSignal,
    /// État partagé de l'ampli guitare (contrôles + telemetry).
    pub guitar: Arc<crate::guitar::GuitarRuntime>,
    /// Knobs et LEDs partagés avec la boucle matérielle.
    mirror: crate::gui::HardwareMirror,
    /// Fenêtre réglages audio ouverte.
    pub show_settings: bool,
    /// Fenêtre de mise à jour ouverte.
    pub show_updater: bool,
    /// État partagé de l'auto-updater.
    pub updater_state: crate::updater::SharedUpdateState,
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
    /// Dernière demande de navigation GUI traitée (synchro hardware).
    last_nav_serial: u64,
    /// Rail de navigation visible.
    pub show_nav: bool,
    /// Espace de travail central visible.
    pub show_workspace: bool,
    /// Panneau MK3 visible.
    pub show_mk3: bool,
    /// Largeur de fenêtre au frame précédent (redimensionnement détecté).
    last_window_w: f32,
    /// Rail masqué automatiquement par une fenêtre étroite (réaffiché en grandissant).
    nav_auto_hidden: bool,
    /// Largeurs du rail et du panneau MK3 en proportion de la fenêtre.
    nav_frac: f32,
    mk3_frac: f32,
    /// Buffer de renommage du pattern actif.
    pub pattern_name_buf: String,
    /// Cible du buffer de renommage (groupe, pattern).
    pub pattern_name_target: Option<(u8, usize)>,
    /// Sound dont la suppression du slot attend une seconde confirmation.
    pub confirm_clear_sound: Option<(u8, u8)>,
    /// Mode avancé de la TB-303 (les macros restent le mode par défaut).
    pub plugin_advanced: bool,
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
    /// Angle affiché de chaque knob sans fin (repère visuel uniquement).
    knob_turns: [f32; 8],
    /// Sections visibles du panneau MK3.
    mk3_screens: bool,
    mk3_pads: bool,
}

impl GuiApp {
    /// Crée l'app (features audio).
    #[cfg(feature = "audio")]
    pub fn new(
        engine: Arc<Mutex<Engine>>,
        audio: Option<Arc<Mutex<Option<crate::audio::AudioOutput>>>>,
        repaint: crate::gui::RepaintSignal,
        guitar: Arc<crate::guitar::GuitarRuntime>,
        mirror: crate::gui::HardwareMirror,
    ) -> Self {
        let devices = crate::audio::list_output_devices();
        let current_dev = audio.as_ref().and_then(|a| {
            let guard = a.lock();
            (*guard).as_ref().map(|o| o.device_name().to_string())
        });
        let settings = crate::settings::AppSettings::load();
        let selected = current_dev
            .or(settings.audio_device)
            .unwrap_or_else(|| "default".to_string());
        let (sample_load_tx, sample_load_rx) = crossbeam_channel::unbounded();
        let updater_state = Arc::new(Mutex::new(crate::updater::UpdateState::default()));
        {
            let s_check = updater_state.clone();
            let r_check = repaint.clone();
            std::thread::Builder::new()
                .name("maschine-update-check-startup".to_string())
                .spawn(move || {
                    std::thread::sleep(Duration::from_secs(3));
                    crate::updater::spawn_background_update_check(s_check);
                    std::thread::sleep(Duration::from_millis(1500));
                    r_check.notify();
                })
                .ok();
        }
        Self {
            engine,
            audio,
            repaint,
            guitar,
            mirror,
            show_settings: false,
            show_updater: false,
            updater_state,
            available_devices: devices,
            selected_device: selected,
            status_message: None,
            pad_down: None,
            view: StudioView::Sequencer,
            last_left_tab: 0,
            last_nav_serial: 0,
            show_nav: true,
            show_workspace: true,
            show_mk3: true,
            last_window_w: 0.0,
            nav_auto_hidden: false,
            nav_frac: 0.12,
            mk3_frac: 0.36,
            pattern_name_buf: String::new(),
            pattern_name_target: None,
            confirm_clear_sound: None,
            plugin_advanced: false,
            browser_search: String::new(),
            browser_category: 0,
            sample_path_input: String::new(),
            sample_load_tx,
            sample_load_rx,
            mirror_revision: u64::MAX,
            mirror_last: Instant::now() - Duration::from_secs(10),
            left_tex: None,
            right_tex: None,
            knob_turns: [0.0; 8],
            mk3_screens: true,
            mk3_pads: true,
        }
    }

    /// Crée l'app (sans audio).
    #[cfg(not(feature = "audio"))]
    pub fn new(
        engine: Arc<Mutex<Engine>>,
        repaint: crate::gui::RepaintSignal,
        guitar: Arc<crate::guitar::GuitarRuntime>,
        mirror: crate::gui::HardwareMirror,
    ) -> Self {
        let (sample_load_tx, sample_load_rx) = crossbeam_channel::unbounded();
        let updater_state = Arc::new(Mutex::new(crate::updater::UpdateState::default()));
        {
            let s_check = updater_state.clone();
            let r_check = repaint.clone();
            std::thread::Builder::new()
                .name("maschine-update-check-startup".to_string())
                .spawn(move || {
                    std::thread::sleep(Duration::from_secs(3));
                    crate::updater::spawn_background_update_check(s_check);
                    std::thread::sleep(Duration::from_millis(1500));
                    r_check.notify();
                })
                .ok();
        }
        Self {
            engine,
            repaint,
            guitar,
            mirror,
            show_settings: false,
            show_updater: false,
            updater_state,
            status_message: None,
            pad_down: None,
            view: StudioView::Sequencer,
            last_left_tab: 0,
            last_nav_serial: 0,
            show_nav: true,
            show_workspace: true,
            show_mk3: true,
            last_window_w: 0.0,
            nav_auto_hidden: false,
            nav_frac: 0.12,
            mk3_frac: 0.36,
            pattern_name_buf: String::new(),
            pattern_name_target: None,
            confirm_clear_sound: None,
            plugin_advanced: false,
            browser_search: String::new(),
            browser_category: 0,
            sample_path_input: String::new(),
            sample_load_tx,
            sample_load_rx,
            mirror_revision: u64::MAX,
            mirror_last: Instant::now() - Duration::from_secs(10),
            left_tex: None,
            right_tex: None,
            knob_turns: [0.0; 8],
            mk3_screens: true,
            mk3_pads: true,
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

    /// Change de vue depuis la GUI et synchronise le contexte hardware.
    pub fn switch_view(&mut self, view: StudioView) {
        self.view = view;
        let tab = view.tab_index();
        self.last_left_tab = tab;
        self.send(EngineEvent::SetLeftTab(tab));
        // Page PLUGIN 2 = ampli guitare ; 0 = SOUND, 1 = synth.
        let page = match view {
            StudioView::Guitar => 2,
            StudioView::Plugin => 1,
            _ => 0,
        };
        self.send(EngineEvent::SetPluginPage(page));
    }

    /// Soft keys de la page ampli guitare : mêmes actions sur l'écran MK3,
    /// le miroir GUI et les boutons physiques.
    /// Layout : 1=AMP< 2=AMP> 3=CAB< 4=CAB> 5=ACTIF 6=MON 7=BYP 8=IN+
    pub fn guitar_soft_key(&mut self, key: u8) {
        self.guitar.soft_key(key);
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
            match crate::audio::AudioOutput::open(
                self.engine.clone(),
                Some(target),
                hp_dup,
                self.guitar.clone(),
            ) {
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
        let (space, digits, view_keys, save, quick_save, toggle_nav, toggle_workspace, toggle_mk3) =
            ctx.input(|i| {
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
                    (egui::Key::K, StudioView::Keyboard),
                    (egui::Key::M, StudioView::Mixer),
                    (egui::Key::A, StudioView::Sampler),
                    (egui::Key::T, StudioView::Plugin),
                    (egui::Key::P, StudioView::Plugin),
                    (egui::Key::B, StudioView::Browser),
                    (egui::Key::G, StudioView::Guitar),
                ]
                .map(|(k, v)| (i.key_pressed(k), v));
                let save_key = i.modifiers.command && i.key_pressed(egui::Key::S);
                (
                    i.key_pressed(egui::Key::Space),
                    digits,
                    view_keys,
                    save_key && !i.modifiers.shift,
                    save_key && i.modifiers.shift,
                    i.key_pressed(egui::Key::F1),
                    i.key_pressed(egui::Key::F3),
                    i.key_pressed(egui::Key::F2),
                )
            });

        if toggle_nav {
            self.show_nav = !self.show_nav;
            self.nav_auto_hidden = false;
        }
        if toggle_workspace {
            self.show_workspace = !self.show_workspace;
        }
        if toggle_mk3 {
            self.show_mk3 = !self.show_mk3;
        }
        for (pressed, v) in view_keys {
            if pressed {
                self.switch_view(v);
            }
        }
        for (d, pressed) in digits.iter().enumerate() {
            if *pressed {
                self.send(EngineEvent::GroupFocus(d as u8));
            }
        }
        if quick_save {
            self.quick_save_project();
        } else if save {
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

    /// Sauvegarde le projet courant (format JSON lisible, écriture atomique).
    /// Utilisé par Ctrl+S, autosave GUI et le bouton SAVE.
    pub fn save_project(&mut self) {
        self.save_project_inner(false);
    }

    /// Sauvegarde rapide compacte du projet courant (Shift+FILE SAVE / Ctrl+Shift+S).
    pub fn quick_save_project(&mut self) {
        self.save_project_inner(true);
    }

    fn save_project_inner(&mut self, quick: bool) {
        let mut settings = crate::settings::AppSettings::load();
        let (project, current_path, project_name) = {
            let eng = self.engine.lock();
            let snap = eng.snapshot();
            (
                (*snap.project).clone(),
                snap.current_project_path.clone(),
                snap.project.name.clone(),
            )
        };
        let path = crate::projects::resolve_current_save_path(
            &settings,
            current_path.as_deref(),
            &project_name,
        );
        match crate::projects::save_project(&project, &path, quick) {
            Ok(()) => {
                crate::projects::remember_project(&mut settings, &path);
                self.send(EngineEvent::SetProjectPath(Some(
                    path.to_string_lossy().to_string(),
                )));
                let label = if quick {
                    "Sauvegarde rapide"
                } else {
                    "Projet sauvegardé"
                };
                self.status_message = Some((format!("✓ {label} → {}", path.display()), false));
            }
            Err(e) => {
                self.status_message =
                    Some((format!("Erreur sauvegarde {}: {e}", path.display()), true));
            }
        }
    }

    /// Ouvre le navigateur de projets (FILE) et rafraîchit la liste.
    pub fn open_project_browser(&mut self) {
        let settings = crate::settings::AppSettings::load();
        let entries = crate::projects::scan_projects(&settings);
        self.send(EngineEvent::SetProjectEntries(entries));
        self.send(EngineEvent::SetBrowserKind(1));
        self.send(EngineEvent::SetLeftTab(3));
        self.switch_view(crate::gui::views::StudioView::Browser);
    }

    /// Charge un fichier projet par chemin (GUI/MK3).
    pub fn load_project_file(&mut self, path: &std::path::Path) {
        let result = self
            .engine
            .lock()
            .handle_event(EngineEvent::Load(path.to_path_buf()));
        match result {
            Ok(()) => {
                let mut settings = crate::settings::AppSettings::load();
                crate::projects::remember_project(&mut settings, path);
                self.send(EngineEvent::SetBrowserKind(1));
                self.status_message =
                    Some((format!("✓ Projet chargé → {}", path.display()), false));
            }
            Err(e) => {
                self.status_message =
                    Some((format!("Erreur chargement {}: {e}", path.display()), true));
            }
        }
    }

    // ── Panneau MK3 : miroir du contrôleur ─────────────────────────────

    /// Bouton d'écran `n` (1–8) : même action que le bouton physique.
    fn press_display_button(&mut self, n: u8) {
        if self.view == StudioView::Guitar {
            self.guitar_soft_key(n);
        } else {
            self.send(EngineEvent::SoftKey(n));
        }
    }

    /// Met à jour les textures des 2 écrans (mêmes pixels que la MK3).
    fn refresh_screens(&mut self, ctx: &egui::Context, snap: &maschine_engine::EngineSnapshot) {
        let now = Instant::now();
        // L'ampli vit dans des atomiques : son écran bouge sans `revision`.
        let changed = snap.revision != self.mirror_revision || self.view == StudioView::Guitar;
        let due = changed && now.duration_since(self.mirror_last) >= Duration::from_millis(28);
        if self.left_tex.is_some() && !due {
            return;
        }
        let data = self.guitar.screen_data();
        let (left, right) =
            maschine_engine::display_render::render_screens_with_guitar(snap, Some(&data));
        let opts = egui::TextureOptions::LINEAR;
        for (frame, tex, name) in [
            (&left, &mut self.left_tex, "mk3_left"),
            (&right, &mut self.right_tex, "mk3_right"),
        ] {
            let img = crate::gui::widgets::display_pixels_to_image(
                frame.w as usize,
                frame.h as usize,
                &frame.pixels,
            );
            match tex {
                Some(t) => t.set(img, opts),
                None => *tex = Some(ctx.load_texture(name, img, opts)),
            }
        }
        self.mirror_revision = snap.revision;
        self.mirror_last = now;
    }

    /// Un écran et ses 4 boutons : la bande des libellés, en haut de l'image,
    /// est cliquable comme les boutons physiques posés juste au-dessus.
    fn screen_with_buttons(
        &mut self,
        ui: &mut egui::Ui,
        snap: &maschine_engine::EngineSnapshot,
        right: bool,
        width: f32,
    ) {
        let s = width / 480.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 272.0 * s), egui::Sense::hover());
        let tex = if right {
            &self.right_tex
        } else {
            &self.left_tex
        };
        if let Some(tex) = tex {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(tex.id(), rect, uv, egui::Color32::WHITE);
        }
        let first = if right { 4u8 } else { 0 };
        let strip = maschine_engine::display_render::SOFT_KEYS_H as f32 * s;
        for i in 0..4u8 {
            let n = first + i;
            let label = snap.soft_labels[n as usize];
            let key = egui::Rect::from_min_size(
                rect.min + egui::vec2(f32::from(i) * 120.0 * s, 0.0),
                egui::vec2(120.0 * s, strip),
            );
            let resp = ui
                .interact(key, ui.id().with(("mk3_button", n)), egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(format!("Bouton {} : {label}", n + 1));
            resp.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, !label.is_empty(), label)
            });
            if resp.hovered() || resp.has_focus() {
                ui.painter().rect_stroke(
                    key.shrink(1.0),
                    egui::CornerRadius::same(3),
                    egui::Stroke::new(1.5_f32, MachColors::dark().text),
                    egui::StrokeKind::Inside,
                );
            }
            if resp.clicked() && !label.is_empty() {
                self.press_display_button(n + 1);
            }
        }
    }

    /// 4 knobs sous un écran, chacun centré sous le slot qui affiche son nom
    /// et sa valeur. Ils passent par le mapping des knobs physiques.
    fn knob_row(&mut self, ui: &mut egui::Ui, right: bool, width: f32, size: f32) {
        let s = width / 480.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, size + 8.0), egui::Sense::hover());
        let first = if right { 4 } else { 0 };
        for i in 0..4 {
            let k = first + i;
            let cx = rect.left() + maschine_engine::display_render::knob_slot_center(i) as f32 * s;
            let dial = egui::Rect::from_center_size(
                egui::pos2(cx, rect.center().y),
                egui::vec2(size, size),
            );
            let id = ui.id().with(("mk3_knob", k));
            if let Some(delta) =
                crate::gui::widgets::encoder(ui, dial, id, &mut self.knob_turns[k], k as u8 + 1)
            {
                let _ =
                    self.mirror
                        .events
                        .send(maschine_hardware::events::HardwareEvent::KnobDelta {
                            knob: k as u8,
                            delta,
                        });
            }
        }
    }

    /// Miroir MK3 dans l'ordre du contrôleur : boutons d'écran (en haut des
    /// écrans), 2 écrans, 8 knobs, puis groupes + pads. Tout s'ajuste à la
    /// largeur ET à la hauteur disponibles ; défilement en dernier recours.
    fn render_mk3(&mut self, ui: &mut egui::Ui, snap: &maschine_engine::EngineSnapshot) {
        self.refresh_screens(ui.ctx(), snap);
        let c = MachColors::dark();
        // Sections masquables (petits écrans) : écrans + knobs, groupes + pads.
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("MASCHINE MK3")
                    .strong()
                    .size(12.0)
                    .color(c.text_dim),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::gui::widgets::chip(ui, "PADS", self.mk3_pads, c.accent) {
                    self.mk3_pads = !self.mk3_pads;
                }
                if crate::gui::widgets::chip(ui, "ÉCRANS", self.mk3_screens, c.accent) {
                    self.mk3_screens = !self.mk3_screens;
                }
            });
        });
        let gap = 8.0;
        let sp = ui.spacing().item_spacing.y;
        let w = ui.available_width();
        let h = ui.available_height();
        // Côte à côte (comme le matériel) dès que chaque écran garde 75 %.
        let side_by_side = w >= 960.0 * 0.75 + gap;
        let knob = (w / if side_by_side { 8.0 } else { 4.0 } * 0.42).clamp(28.0, 52.0);
        // Tout ce qui n'est pas écran : rangées de knobs, espacements, pads.
        let rows = if side_by_side { 1.0 } else { 2.0 };
        let pads = if self.mk3_pads {
            gap + 4.0 * PAD_MIN + 3.0 * 6.0 + sp
        } else {
            0.0
        };
        let fixed = rows * (knob + 8.0 + 2.0 * sp) + pads;
        let fit_w = if side_by_side {
            (w - gap) / 960.0
        } else {
            w / 480.0
        };
        let fit_h = (h - fixed) / (272.0 * rows);
        // Sous 75 % le texte 8×16 devient illisible : on défile plutôt.
        let scale = fit_w.min(fit_h.max(0.75));
        let screen_w = 480.0 * scale;
        let block_w = if side_by_side {
            2.0 * screen_w + gap
        } else {
            screen_w
        };

        if self.mk3_screens {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                // Écrans plus étroits que le panneau : centrés.
                ui.add_space(((w - block_w) / 2.0).max(0.0));
                let columns: &[&[bool]] = if side_by_side {
                    &[&[false], &[true]]
                } else {
                    &[&[false, true]]
                };
                for column in columns {
                    ui.vertical(|ui| {
                        for &right in *column {
                            self.screen_with_buttons(ui, snap, right, screen_w);
                            self.knob_row(ui, right, screen_w, knob);
                        }
                    });
                }
            });
        }
        if self.mk3_pads {
            ui.add_space(gap);
            self.render_pads(ui, snap);
        }
    }

    fn draw_pad_grid(
        &self,
        ui: &mut egui::Ui,
        snap: &maschine_engine::EngineSnapshot,
        leds: &maschine_proto::hid_output::LedState,
        pad_size: f32,
        press: &mut Option<(u8, u8, bool)>,
        release: &mut Option<u8>,
    ) {
        const KEYBOARD_LABELS: [&str; 16] = [
            "C3", "D3", "D#3", "F3", "G3", "G#3", "A#3", "C4", "D4", "D#4", "F4", "G4", "G#4",
            "A#4", "C5", "D5",
        ];
        const SHIFT_PAD_LABELS: [&str; 16] = [
            "UNDO",
            "REDO",
            "STEP UNDO",
            "STEP REDO",
            "QUANTIZE",
            "QUANT 50%",
            "NUDGE ←",
            "NUDGE →",
            "CLEAR",
            "CLEAR AUTO",
            "COPY",
            "PASTE",
            "− SEMI",
            "+ SEMI",
            "− OCT",
            "+ OCT",
        ];
        let gf = snap.group_focus as usize;
        let shift_layer = snap.shift_held || ui.input(|input| input.modifiers.shift);
        let assigned_pattern = snap
            .active_scene
            .and_then(|scene| snap.project.scenes.get(scene))
            .and_then(|scene| scene.patterns.get(gf))
            .copied()
            .flatten();
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
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
                        .unwrap_or_default();
                    let (label, selected) = if snap.scene_mode {
                        (format!("SCN {:02}", index + 1), false)
                    } else if snap.pattern_mode {
                        (
                            format!("PAT {:02}", index + 1),
                            assigned_pattern == Some(index as usize),
                        )
                    } else if snap.controller_mode == 3 {
                        (
                            format!("PAS {:02}", index + 1),
                            index as usize == snap.step_cursor % 16,
                        )
                    } else if snap.controller_mode == 1 {
                        (KEYBOARD_LABELS[index as usize].to_string(), false)
                    } else if snap.controller_mode == 2 {
                        (format!("ACC {}", (index % 7) + 1), false)
                    } else {
                        (name, index == snap.active_sound)
                    };
                    let label = if shift_layer {
                        SHIFT_PAD_LABELS[index as usize].to_string()
                    } else {
                        label
                    };
                    // Couleur = LED du pad physique (même calcul que la MK3).
                    let st = PadState {
                        index,
                        color: crate::gui::widgets::led_color(leds.pad_leds[index as usize]),
                        label: &label,
                        selected,
                        was_down: self.pad_down == Some(index),
                    };
                    let inter = pad_button(ui, &st, pad_size);
                    if inter.pressed {
                        *press = Some((index, inter.velocity, shift_layer));
                    }
                    if inter.released {
                        *release = Some(index);
                    }
                    if inter.secondary_clicked {
                        // Clic droit sur un pad : efface les notes du son dans le pattern actif.
                        if snap.controller_mode == 0 && !snap.pattern_mode && !snap.scene_mode {
                            self.send(EngineEvent::EraseSound {
                                group: snap.group_focus,
                                sound: index,
                            });
                        } else {
                            self.send(EngineEvent::SelectSound(index));
                        }
                    }
                }
            });
        }
    }

    /// Boutons de groupe A–H (2 × 4 comme sur la MK3), couleurs des LEDs.
    fn draw_group_buttons(
        &mut self,
        ui: &mut egui::Ui,
        snap: &maschine_engine::EngineSnapshot,
        leds: &maschine_proto::hid_output::LedState,
        size: egui::Vec2,
        columns: usize,
    ) {
        let c = MachColors::dark();
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        for row in 0..8usize.div_ceil(columns) {
            ui.horizontal(|ui| {
                for g in (row * columns)..((row + 1) * columns).min(8) {
                    let muted = snap.muted_groups & (1 << g) != 0;
                    let soloed = snap.soloed_groups & (1 << g) != 0;
                    let led =
                        leds.button_leds[maschine_proto::hid_output::offsets::GROUP_A - 1 + g];
                    let fill = crate::gui::widgets::led_color(led);
                    let text = egui::RichText::new(format!(
                        "{}{}",
                        crate::gui::views::group_char(g),
                        if soloed {
                            " S"
                        } else if muted {
                            " M"
                        } else {
                            ""
                        }
                    ))
                    .strong()
                    .color(crate::gui::widgets::ink_on(fill));
                    let stroke = if muted {
                        egui::Stroke::new(1.5_f32, c.warning)
                    } else if soloed {
                        egui::Stroke::new(1.5_f32, c.accent_cyan)
                    } else {
                        egui::Stroke::new(1.0_f32, c.border)
                    };
                    let btn = egui::Button::new(text)
                        .min_size(size)
                        .fill(fill)
                        .stroke(stroke)
                        .corner_radius(egui::CornerRadius::same(4));
                    if ui
                        .add(btn)
                        .on_hover_text(format!("Groupe {}", crate::gui::views::group_char(g)))
                        .clicked()
                    {
                        self.send(EngineEvent::GroupFocus(g as u8));
                    }
                }
            });
        }
    }

    /// Groupes A–H et pads 4 × 4, disposés comme la moitié basse de la MK3
    /// (groupes à gauche) quand la largeur le permet, sinon empilés.
    fn render_pads(&mut self, ui: &mut egui::Ui, snap: &maschine_engine::EngineSnapshot) {
        let leds = self.mirror.leds.lock().clone();
        let width = ui.available_width();
        let height = ui.available_height().max(4.0 * PAD_MIN + 18.0);
        let groups_beside = width >= 4.0 * 64.0 + 18.0 + 2.0 * 56.0 + 16.0;
        let pads_w = if groups_beside { width * 0.64 } else { width };
        // 8 px de marge : le bas du panneau défilant ne coupe pas la rangée 1.
        let pad_size = ((pads_w - 18.0) / 4.0)
            .min((height - 26.0 - if groups_beside { 0.0 } else { 48.0 }) / 4.0)
            .clamp(40.0, 104.0);
        let mut press: Option<(u8, u8, bool)> = None;
        let mut release: Option<u8> = None;

        if groups_beside {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let group = egui::vec2(
                    ((width - pad_size * 4.0 - 18.0 - 12.0 - 6.0) / 2.0).clamp(40.0, 84.0),
                    (pad_size * 0.6).max(30.0),
                );
                // Bloc groupes + pads centré, comme la moitié basse de la MK3.
                let block = 2.0 * group.x + 6.0 + 12.0 + 4.0 * pad_size + 18.0;
                ui.add_space(((width - block) / 2.0).max(0.0));
                ui.vertical(|ui| {
                    self.draw_group_buttons(ui, snap, &leds, group, 2);
                });
                ui.vertical(|ui| {
                    self.draw_pad_grid(ui, snap, &leds, pad_size, &mut press, &mut release);
                });
            });
        } else {
            let group = egui::vec2(((width - 7.0 * 6.0) / 8.0).max(28.0), 30.0);
            self.draw_group_buttons(ui, snap, &leds, group, 8);
            ui.add_space(6.0);
            self.draw_pad_grid(ui, snap, &leds, pad_size, &mut press, &mut release);
        }

        if let Some((pad, velocity, shift_layer)) = press {
            if shift_layer {
                self.send(EngineEvent::ShiftPadAction(pad));
            } else if snap.scene_mode {
                self.send(EngineEvent::Scene(pad as usize));
            } else if snap.pattern_mode {
                // Mode PATTERN officiel : un pad existant sélectionne son
                // pattern, un pad vide crée le pattern de ce slot.
                let exists = snap
                    .project
                    .groups
                    .get(snap.group_focus as usize)
                    .map(|g| (pad as usize) < g.patterns.len())
                    .unwrap_or(false);
                if exists {
                    self.send(EngineEvent::PatternSelect {
                        group: snap.group_focus,
                        pattern: pad as usize,
                    });
                } else {
                    self.send(EngineEvent::CreatePatternAt {
                        group: snap.group_focus,
                        pattern: pad as usize,
                    });
                }
            } else if snap.select_mode {
                self.send(EngineEvent::SelectSound(pad));
            } else if snap.erase_mode {
                self.send(EngineEvent::SelectSound(pad));
                self.send(EngineEvent::EraseSound {
                    group: snap.group_focus,
                    sound: pad,
                });
            } else if snap.controller_mode == 3 {
                let tick = (snap.selected_bar as u32 * 3840) + (pad as u32 * 240);
                self.toggle_step(snap.active_sound, tick);
            } else {
                let trigger = if snap.controller_mode == 1 {
                    snap.active_sound
                } else {
                    pad
                };
                self.pad_down = Some(trigger);
                self.send(EngineEvent::Pad {
                    pad: trigger,
                    velocity,
                    pressed: true,
                });
            }
        }
        if release.is_some() {
            if let Some(down) = self.pad_down.take() {
                self.send(EngineEvent::Pad {
                    pad: down,
                    velocity: 0,
                    pressed: false,
                });
            }
        }
    }
}

impl eframe::App for GuiApp {
    /// Dernière fermeture : persiste le projet courant comme dernier projet ouvert.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let mut settings = crate::settings::AppSettings::load();
        let path = {
            let eng = self.engine.lock();
            eng.snapshot().current_project_path.clone()
        };
        if let Some(path) = path {
            crate::projects::remember_project(&mut settings, std::path::Path::new(&path));
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.repaint.attach(ctx);

        // La disposition suit la fenêtre à chaque redimensionnement : sous
        // 1400 px le rail des vues s'efface (et revient en grandissant).
        let window_w = ctx.screen_rect().width();
        let window_changed = (window_w - self.last_window_w).abs() > 0.5;
        if window_changed {
            if window_w < 1400.0 && self.show_nav {
                self.show_nav = false;
                self.nav_auto_hidden = true;
            } else if window_w >= 1400.0 && self.nav_auto_hidden {
                self.show_nav = true;
                self.nav_auto_hidden = false;
            }
            self.last_window_w = window_w;
        }

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
                4 => StudioView::Plugin,
                _ => StudioView::Sequencer,
            };
        }

        // Navigation explicite PATTERN / STEP / KEYBOARD / SCENE / PLUGIN.
        // Le compteur ne change que sur une nouvelle demande hardware.
        if snap.nav_serial != self.last_nav_serial {
            self.last_nav_serial = snap.nav_serial;
            self.view = match snap.nav_view {
                2 => StudioView::Keyboard,
                3 => StudioView::Ideas,
                4 => StudioView::Plugin,
                _ => StudioView::Sequencer,
            };
            self.last_left_tab = snap.left_tab;
        }

        // La page PLUGIN 2 est l'ampli guitare : elle doit survivre aux
        // navigations hardware et ramener la GUI sur la bonne vue.
        if snap.left_tab == 4 && snap.plugin_page == 2 {
            self.view = StudioView::Guitar;
        } else if self.view == StudioView::Guitar && snap.plugin_page != 2 {
            self.view = StudioView::Plugin;
        }

        // ── Barre d'application : vues rapides + panneaux + états latchés ──
        egui::TopBottomPanel::top("appbar")
            .exact_height(38.0)
            .show(ctx, |ui| {
                let c = MachColors::dark();
                ui.horizontal_centered(|ui| {
                    // Bascules de panneaux allumées en blanc, comme les boutons MK3.
                    let lit = c.text_dim;
                    if crate::gui::widgets::chip_tooltip(
                        ui,
                        "VUES",
                        self.show_nav,
                        lit,
                        "Rail des vues (F1)",
                    ) {
                        self.show_nav = !self.show_nav;
                        self.nav_auto_hidden = false;
                    }
                    ui.separator();
                    ui.colored_label(c.accent, egui::RichText::new("MASCHINE").strong());
                    ui.label(egui::RichText::new("MK3 LINUX").small().color(c.text_muted));

                    let mut selected = self.view;
                    egui::ComboBox::from_id_salt("view_picker")
                        .selected_text(
                            egui::RichText::new(selected.short_label())
                                .strong()
                                .size(11.5),
                        )
                        .width(132.0)
                        .show_ui(ui, |ui| {
                            for view in StudioView::ALL {
                                ui.selectable_value(&mut selected, view, view.label());
                            }
                        });
                    if selected != self.view {
                        self.switch_view(selected);
                    }

                    for (on, label, color) in [
                        (snap.pattern_mode, "PATTERN", c.accent),
                        (snap.scene_mode, "SCENE", c.success),
                        (snap.controller_mode == 3, "STEP", c.warning),
                        (snap.controller_mode == 1, "CLAVIER", c.accent_cyan),
                        (snap.select_mode, "SELECT", c.accent_cyan),
                        (snap.erase_mode, "ERASE", c.error),
                    ] {
                        if on {
                            ui.colored_label(
                                color,
                                egui::RichText::new(format!("● {label}"))
                                    .strong()
                                    .size(10.5),
                            );
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::gui::widgets::chip_tooltip(
                            ui,
                            "MK3",
                            self.show_mk3,
                            lit,
                            "Miroir MK3 : écrans, knobs, pads (F2)",
                        ) {
                            self.show_mk3 = !self.show_mk3;
                        }
                        if crate::gui::widgets::chip_tooltip(
                            ui,
                            "CENTRE",
                            self.show_workspace,
                            lit,
                            "Vue de travail (F3) — masquée, la MK3 prend toute la fenêtre",
                        ) {
                            self.show_workspace = !self.show_workspace;
                        }
                        let all_visible = self.show_nav && self.show_workspace && self.show_mk3;
                        if crate::gui::widgets::chip_tooltip(
                            ui,
                            "TOUT",
                            all_visible,
                            lit,
                            "Afficher ou masquer les trois panneaux",
                        ) {
                            let show = !all_visible;
                            self.show_nav = show;
                            self.nav_auto_hidden = false;
                            self.show_workspace = show;
                            self.show_mk3 = show;
                        }
                    });
                });
            });

        egui::TopBottomPanel::top("transport").show(ctx, |ui| {
            crate::gui::transport::render_transport(ui, self, &snap);
        });

        // Rail gauche : vues + groupes. Repliable ; largeur proportionnelle.
        let nav_w = fit_side_panel(
            ctx,
            "nav",
            &mut self.nav_frac,
            window_changed,
            150.0..=280.0,
        );
        egui::SidePanel::left("nav")
            .resizable(true)
            .default_width(nav_w)
            .width_range(150.0..=280.0)
            .show_animated(ctx, self.show_nav, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(4.0);
                        let c = MachColors::dark();
                        for v in StudioView::ALL {
                            let on = self.view == v;
                            let btn = egui::Button::new(
                                egui::RichText::new(v.label()).strong().color(if on {
                                    egui::Color32::BLACK
                                } else {
                                    c.text
                                }),
                            )
                            .fill(if on { c.accent } else { c.widget_bg })
                            .min_size(egui::vec2(ui.available_width(), 30.0));
                            if ui.add(btn).clicked() {
                                self.switch_view(v);
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
                        crate::gui::theme::caption(
                            ui,
                            format!("Sound actif : {:02}", snap.active_sound + 1),
                        );

                        ui.add_space(10.0);
                        if mach_button(ui, "SAUVEGARDER", false).clicked {
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

        // Rail droit : miroir MK3, largeur proportionnelle à la fenêtre. Centre
        // masqué (F3) : la MK3 prend toute la place, écrans côte à côte.
        if self.show_workspace {
            let mk3_max = (window_w * 0.62).max(380.0);
            let mk3_w = fit_side_panel(
                ctx,
                "mk3_panel",
                &mut self.mk3_frac,
                window_changed,
                380.0..=mk3_max,
            );
            egui::SidePanel::right("mk3_panel")
                .resizable(true)
                .default_width(mk3_w)
                .width_range(380.0..=mk3_max)
                .show_animated(ctx, self.show_mk3, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.render_mk3(ui, &snap));
                });
            egui::CentralPanel::default().show(ctx, |ui| {
                crate::gui::views::render_view(ui, self, &snap, is_playing);
            });
        } else if self.show_mk3 {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.render_mk3(ui, &snap));
            });
        }

        // Barre de statut basse.
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
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

                // Notification discrète de mise à jour dans la barre de statut
                let update_opt = self.updater_state.lock().update_available.clone();
                if let Some(check) = update_opt {
                    ui.separator();
                    if ui
                        .button(
                            egui::RichText::new(format!("MÀJ v{} disponible", check.new_version))
                                .small()
                                .color(MachColors::dark().accent_cyan),
                        )
                        .clicked()
                    {
                        self.show_updater = true;
                    }
                }
            });
        });

        // Fenêtre réglages audio (HEAD).
        #[cfg(feature = "audio")]
        if self.show_settings {
            let mut settings_open = self.show_settings;
            let mut switch_target: Option<String> = None;
            let c = MachColors::dark();
            egui::Window::new("Paramètres Audio — Choix de la Sortie Son")
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
                            .button(egui::RichText::new("Sortie système (Défaut / DJM)").strong())
                            .clicked()
                        {
                            switch_target = Some("default".to_string());
                        }
                        if ui
                            .button(egui::RichText::new("Maschine MK3").strong())
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
                                egui::RichText::new("Activer le périphérique sélectionné").strong(),
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

        // Fenêtre de mise à jour logicielle in-app.
        if self.show_updater {
            let mut updater_open = self.show_updater;
            let c = MachColors::dark();
            egui::Window::new("Mises à jour de Maschine Linux")
                .open(&mut updater_open)
                .default_width(520.0)
                .min_width(450.0)
                .resizable(true)
                .collapsible(false)
                .show(ctx, |ui| {
                    render_updater_window(ui, &self.updater_state, &c);
                });
            self.show_updater = updater_open;
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

/// Largeur d'un panneau latéral en proportion de la fenêtre : elle suit
/// chaque redimensionnement de la fenêtre et retient la proportion choisie
/// quand l'utilisateur tire la poignée du panneau.
fn fit_side_panel(
    ctx: &egui::Context,
    id: &str,
    frac: &mut f32,
    window_changed: bool,
    range: std::ops::RangeInclusive<f32>,
) -> f32 {
    let window_w = ctx.screen_rect().width().max(1.0);
    let id = egui::Id::new(id);
    let state = egui::containers::panel::PanelState::load(ctx, id);
    // Poignée en cours de glissement (id interne d'egui 0.32 : `<panneau>.__resize`).
    let dragging = ctx
        .read_response(id.with("__resize"))
        .is_some_and(|r| r.dragged());
    if let (true, Some(state)) = (dragging, state) {
        *frac = state.rect.width() / window_w;
    }
    let width = (*frac * window_w).clamp(*range.start(), *range.end());
    if let (true, Some(mut state)) = (window_changed && !dragging, state) {
        state.rect.set_width(width);
        ctx.data_mut(|d| d.insert_persisted(id, state));
    }
    width
}

/// Lance la GUI native (main thread, requise par egui/winit).
#[cfg(feature = "audio")]
pub fn run_native_gui(
    engine: Arc<Mutex<maschine_engine::Engine>>,
    audio: Arc<Mutex<Option<crate::audio::AudioOutput>>>,
    repaint: crate::gui::RepaintSignal,
    guitar: Arc<crate::guitar::GuitarRuntime>,
    mirror: crate::gui::HardwareMirror,
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
            Ok(
                Box::new(GuiApp::new(engine, Some(audio), repaint, guitar, mirror))
                    as Box<dyn eframe::App>,
            )
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {e}"))
}

/// Lance la GUI native sans audio.
#[cfg(not(feature = "audio"))]
pub fn run_native_gui(
    engine: Arc<Mutex<maschine_engine::Engine>>,
    repaint: crate::gui::RepaintSignal,
    guitar: Arc<crate::guitar::GuitarRuntime>,
    mirror: crate::gui::HardwareMirror,
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
            Ok(Box::new(GuiApp::new(engine, repaint, guitar, mirror)) as Box<dyn eframe::App>)
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {e}"))
}

/// Affiche le panneau de mise à jour automatique in-app.
fn render_updater_window(
    ui: &mut egui::Ui,
    state: &crate::updater::SharedUpdateState,
    c: &MachColors,
) {
    let s = state.lock().clone();
    ui.add_space(4.0);

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Version installée :").strong());
            ui.colored_label(c.text, format!("v{}", crate::updater::CURRENT_VERSION));
            ui.separator();
            ui.label(egui::RichText::new("Environnement :").strong());
            ui.colored_label(c.accent_cyan, crate::updater::PackageKind::detect().label());
        });
    });

    ui.add_space(8.0);

    if s.is_checking {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Vérification des mises à jour sur GitHub...");
        });
    } else if let Some(check) = &s.update_available {
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(c.accent_cyan, egui::RichText::new("MISE À JOUR").size(13.0));
                    ui.colored_label(
                        egui::Color32::WHITE,
                        egui::RichText::new(format!(
                            "Nouvelle version disponible : v{}",
                            check.new_version
                        ))
                        .strong()
                        .size(15.0),
                    );
                });

                if let Some(ref title) = check.release.name {
                    ui.label(egui::RichText::new(title).strong().color(c.text_dim));
                }

                if let Some(ref asset) = check.matched_asset {
                    ui.label(format!(
                        "Paquet compatible : {} ({:.1} Mo)",
                        asset.name,
                        asset.size as f64 / (1024.0 * 1024.0)
                    ));
                } else {
                    ui.colored_label(
                        c.warning,
                        "Aucun paquet pré-compilé spécifique pour cette distribution.",
                    );
                }

                if let Some(ref body) = check.release.body {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("Notes de version :")
                            .strong()
                            .size(12.0),
                    );
                    egui::ScrollArea::vertical()
                        .max_height(140.0)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(body).small().color(c.text_dim));
                        });
                }
            });
        });

        ui.add_space(10.0);

        if s.is_downloading {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    if let Some((dl, tot)) = s.progress {
                        let pct = if tot > 0 { dl as f32 / tot as f32 } else { 0.0 };
                        ui.add(egui::ProgressBar::new(pct).show_percentage());
                        ui.label(format!(
                            "{:.1} / {:.1} Mo",
                            dl as f64 / 1_048_576.0,
                            tot as f64 / 1_048_576.0
                        ));
                    } else {
                        ui.label("Téléchargement en cours...");
                    }
                });
            });
        } else if s.update_completed {
            ui.colored_label(
                c.success,
                egui::RichText::new("Mise à jour installée avec succès !")
                    .strong()
                    .size(14.0),
            );
            if s.restart_required {
                ui.colored_label(
                    c.accent_cyan,
                    "Mise à jour prête. Redémarre l'application pour appliquer les changements.",
                );
                if ui
                    .button(
                        egui::RichText::new("Redémarrer maintenant")
                            .strong()
                            .size(13.0),
                    )
                    .clicked()
                {
                    restart_app();
                }
            }
        } else if check.matched_asset.is_some()
            && ui
                .button(
                    egui::RichText::new("Télécharger & installer la mise à jour")
                        .strong()
                        .size(13.0),
                )
                .clicked()
        {
            crate::updater::spawn_background_update_apply(state.clone());
        }
    } else {
        ui.horizontal(|ui| {
            ui.colored_label(c.success, egui::RichText::new("OK").size(13.0));
            ui.label(format!(
                "Maschine Linux est à jour (v{}).",
                crate::updater::CURRENT_VERSION
            ));
        });
    }

    if let Some(ref err) = s.error_message {
        ui.add_space(6.0);
        ui.colored_label(c.error, err);
    }
    if let Some(ref msg) = s.status_message {
        ui.add_space(4.0);
        ui.colored_label(c.text_dim, egui::RichText::new(msg).small());
    }

    ui.add_space(10.0);
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Vérifier maintenant").clicked() && !s.is_checking && !s.is_downloading {
            crate::updater::spawn_background_update_check(state.clone());
        }
        if ui.button("Voir sur GitHub").clicked() {
            let _ = std::process::Command::new("xdg-open")
                .arg(format!(
                    "https://github.com/{}/releases",
                    crate::updater::GITHUB_REPO
                ))
                .spawn();
        }
    });
}

/// Redémarre proprement l'application (support natif & AppImage).
/// Gère le remplacement d'exécutable après mise à jour (strip de ' (deleted)'),
/// la préservation des variables d'environnement graphiques (DISPLAY, WAYLAND_DISPLAY...),
/// et le détachement complet dans un nouveau groupe de processus indépendant.
fn restart_app() -> ! {
    use std::os::unix::process::CommandExt as _;

    let mut target = if let Some(appimage) = std::env::var_os("APPIMAGE") {
        std::path::PathBuf::from(appimage)
    } else if let Ok(mut exe) = std::env::current_exe() {
        let exe_str = exe.to_string_lossy().to_string();
        if let Some(clean) = exe_str.strip_suffix(" (deleted)") {
            exe = std::path::PathBuf::from(clean);
        }
        exe
    } else {
        std::path::PathBuf::from("maschine-linux")
    };

    if !target.exists() {
        if std::path::Path::new("/usr/bin/maschine-linux").exists() {
            target = std::path::PathBuf::from("/usr/bin/maschine-linux");
        } else if std::path::Path::new("/usr/local/bin/maschine-linux").exists() {
            target = std::path::PathBuf::from("/usr/local/bin/maschine-linux");
        }
    }

    let args: Vec<String> = std::env::args().skip(1).collect();

    let quoted_args = args
        .iter()
        .map(|a| format!("'{}'", a.replace('\'', "'\\''")))
        .collect::<Vec<_>>()
        .join(" ");

    // Préserver explicitement l'environnement graphique et audio pour éviter
    // que le nouveau processus échoue à l'initialisation de l'affichage X11/Wayland.
    let mut env_exports = String::new();
    for var in [
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "XDG_SESSION_TYPE",
        "DBUS_SESSION_BUS_ADDRESS",
        "XAUTHORITY",
        "PULSE_SERVER",
        "PIPEWIRE_REMOTE",
        "ALSA_CARD",
    ] {
        if let Ok(val) = std::env::var(var) {
            env_exports.push_str(&format!(
                "export {}='{}'; ",
                var,
                val.replace('\'', "'\\''")
            ));
        }
    }

    // Le court délai de 0.5s permet au processus père de quitter et de libérer
    // complètement les périphériques USB HID et audio ALSA/JACK.
    let appimage_cleanup = if std::env::var_os("APPIMAGE").is_some() {
        "unset APPDIR; unset ARGV0; "
    } else {
        ""
    };
    let cd_owd = "if [ -n \"${OWD:-}\" ] && [ -d \"$OWD\" ]; then cd \"$OWD\"; fi; ";

    let cmd = format!(
        "sleep 0.5; {} {}{}'{}' {}",
        cd_owd,
        appimage_cleanup,
        env_exports,
        target.to_string_lossy().replace('\'', "'\\''"),
        quoted_args
    );

    let mut cmd_builder = std::process::Command::new("nohup");
    cmd_builder
        .arg("sh")
        .arg("-c")
        .arg(&cmd)
        .process_group(0) // détachement de groupe de processus
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    if cmd_builder.spawn().is_err() {
        // Repli direct sur sh si nohup n'est pas présent sur le système
        let mut fallback = std::process::Command::new("sh");
        let _ = fallback
            .arg("-c")
            .arg(&cmd)
            .process_group(0)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    std::process::exit(0);
}
