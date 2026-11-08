//! Vues de l'espace de travail (Ideas, Sequencer, Mixer, Sampler, Browser).

use eframe::egui;

pub mod browser;
pub mod ideas;
pub mod keyboard;
pub mod mixer;
pub mod sampler;
pub mod sequencer;
pub mod synth;

/// Vues disponibles dans l'espace de travail central.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioView {
    Ideas,
    Sequencer,
    Keyboard,
    Mixer,
    Sampler,
    Browser,
    Plugin,
}

impl StudioView {
    /// Toutes les vues, dans l'ordre du rail de navigation.
    pub const ALL: [Self; 7] = [
        Self::Ideas,
        Self::Sequencer,
        Self::Keyboard,
        Self::Mixer,
        Self::Sampler,
        Self::Plugin,
        Self::Browser,
    ];

    /// Libellé complet (rail de gauche).
    pub fn label(self) -> &'static str {
        match self {
            Self::Ideas => "IDEAS / SONG",
            Self::Sequencer => "SÉQUENCEUR",
            Self::Keyboard => "CLAVIER / PERFORMANCE",
            Self::Mixer => "MIXER",
            Self::Sampler => "SAMPLER",
            Self::Plugin => "PLUGIN",
            Self::Browser => "BROWSER",
        }
    }

    /// Libellé court (barre supérieure, petits écrans).
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Ideas => "IDEAS",
            Self::Sequencer => "SÉQUENCEUR",
            Self::Keyboard => "CLAVIER",
            Self::Mixer => "MIXER",
            Self::Sampler => "SAMPLER",
            Self::Plugin => "PLUGIN",
            Self::Browser => "BROWSER",
        }
    }

    /// Onglet écran gauche associé (0: Arrange/Seq, 1: Mixer, 2: Sampler, 3: Browser, 4: Plugin).
    pub fn tab_index(&self) -> u8 {
        match self {
            Self::Ideas | Self::Sequencer | Self::Keyboard => 0,
            Self::Mixer => 1,
            Self::Sampler => 2,
            Self::Browser => 3,
            Self::Plugin => 4,
        }
    }
}

/// Dessine la vue active dans l'espace de travail central.
pub fn render_view(
    ui: &mut egui::Ui,
    app: &mut crate::gui::app::GuiApp,
    snap: &maschine_engine::EngineSnapshot,
    is_playing: bool,
) {
    match app.view {
        StudioView::Ideas => ideas::render(ui, app, snap),
        StudioView::Sequencer => sequencer::render(ui, app, snap, is_playing),
        StudioView::Keyboard => keyboard::render(ui, app, snap),
        StudioView::Mixer => mixer::render(ui, app, snap),
        StudioView::Sampler => sampler::render(ui, app, snap),
        StudioView::Browser => browser::render(ui, app, snap),
        StudioView::Plugin => synth::render(ui, app, snap),
    }
}

/// Couleur associée à un pad / groupe (palette Maschine).
pub fn pad_color(index: usize) -> egui::Color32 {
    const COLORS: [egui::Color32; 16] = [
        egui::Color32::from_rgb(230, 50, 50),
        egui::Color32::from_rgb(230, 120, 30),
        egui::Color32::from_rgb(230, 200, 0),
        egui::Color32::from_rgb(120, 220, 0),
        egui::Color32::from_rgb(0, 200, 80),
        egui::Color32::from_rgb(0, 200, 180),
        egui::Color32::from_rgb(0, 160, 230),
        egui::Color32::from_rgb(50, 100, 230),
        egui::Color32::from_rgb(120, 60, 230),
        egui::Color32::from_rgb(180, 50, 220),
        egui::Color32::from_rgb(220, 50, 150),
        egui::Color32::from_rgb(200, 80, 80),
        egui::Color32::from_rgb(160, 180, 0),
        egui::Color32::from_rgb(0, 180, 130),
        egui::Color32::from_rgb(80, 80, 200),
        egui::Color32::from_rgb(200, 200, 210),
    ];
    COLORS[index % COLORS.len()]
}

/// Nom court d'un groupe (A..H).
pub fn group_char(index: usize) -> char {
    (b'A' + index as u8) as char
}
