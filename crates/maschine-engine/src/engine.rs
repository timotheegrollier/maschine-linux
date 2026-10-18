//! Engine — assemblage Project + Transport + Sampler + Sequencer.
//!
//! Thread-safe snapshot pour UI via `parking_lot::RwLock` + `Arc`.

use std::sync::Arc;

use parking_lot::RwLock;

use crate::project::{BarsBeatsTicks, PatternEvent, Project, Scene, Section};
use crate::sampler::Sampler;
use crate::sequencer::{Grid, Sequencer};
use crate::transport::{Transport, TransportState};

/// Événement engine (depuis hardware ou UI).
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// Pad hit
    Pad {
        /// Pad 0..15
        pad: u8,
        /// Velocity
        velocity: u8,
        /// Press
        pressed: bool,
    },
    /// Play/Rec/Stop
    Transport {
        /// Play
        play: bool,
        /// Rec
        rec: bool,
        /// Stop
        stop: bool,
    },
    /// Change BPM
    Bpm(f32),
    /// Change Group focus
    GroupFocus(u8),
    /// Trigger Scene
    Scene(usize),
    /// Crée une scène et la sélectionne
    AddScene,
    /// Quantize pattern
    Quantize {
        /// Group idx
        group: usize,
        /// Pattern idx
        pattern: usize,
    },
    /// Save
    Save(std::path::PathBuf),
    /// Load
    Load(std::path::PathBuf),
    /// Master volume 0.0..1.0
    MasterVolume(f32),
    /// Headphone volume 0.0..1.0
    HeadphoneVolume(f32),
    /// Toggle mute on group
    Mute(u8),
    /// Toggle solo on group
    Solo(u8),
    /// Select pattern in group
    PatternSelect {
        /// Group idx
        group: u8,
        /// Pattern idx
        pattern: usize,
    },
    /// Change Left display tab (0..3)
    SetLeftTab(u8),
    /// Change Right display tab (0..3)
    SetRightTab(u8),
    /// Select active sound (0..15)
    SelectSound(u8),
    /// Volume d'un groupe en dB
    GroupVolume { group: u8, volume_db: f32 },
    /// Pan d'un groupe
    GroupPan { group: u8, pan: f32 },
    /// Volume d'un sound (pad) en dB
    SoundVolume {
        group: u8,
        sound: u8,
        volume_db: f32,
    },
    /// Pan d'un sound
    SoundPan { group: u8, sound: u8, pan: f32 },
    /// Cutoff filtre d'un sound
    SoundFilterCutoff {
        group: u8,
        sound: u8,
        cutoff_hz: f32,
    },
    /// Résonance filtre d'un sound 0..1
    SoundFilterReso {
        group: u8,
        sound: u8,
        resonance: f32,
    },
    /// ADSR d'un sound (attack/decay/sustain/release)
    SoundAdsr {
        group: u8,
        sound: u8,
        attack_ms: f32,
        decay_ms: f32,
        sustain: f32,
        release_ms: f32,
    },
    /// Efface toutes les notes d'un son dans le pattern actif (ERASE + pad)
    EraseSound { group: u8, sound: u8 },
    /// Miroirs workflow pour les écrans (envoyés par le mapper)
    SetSceneMode(bool),
    /// Miroir mode PATTERN
    SetPatternMode(bool),
    /// Miroir mode SELECT
    SetSelectMode(bool),
    /// Miroir mode ERASE
    SetEraseMode(bool),
    /// Miroir mode de jeu 0..3
    SetControllerMode(u8),
    /// Bascule la boucle transport
    ToggleLoop,
    /// Bascule le métronome
    ToggleMetronome,
    /// Apply humanize variation to active pattern
    Variation,
    /// Quantize active pattern
    QuantizeActive,
    /// Active/désactive le métronome
    SetMetronome(bool),
    /// Basculer mode Song (arranger) / Ideas (scènes)
    ToggleSongMode,
    /// Section suivante dans le Song
    NextSection,
    /// Section précédente dans le Song
    PrevSection,
    /// Sélectionne une section du Song dans l'interface
    SelectSection(usize),
    /// Ajoute une section au Song pour la scène active
    AddSection,
    /// Supprime la section active du Song
    DeleteSection,
    /// Duplique la section active (insérée juste après)
    DuplicateSection,
    /// Redimensionne la section active (en mesures, 1..64)
    ResizeSection {
        /// Longueur en mesures
        bars: usize,
    },
    /// Duplique la scène active (patterns compris)
    DuplicateScene,
    /// Supprime la scène active (au moins une scène conservée)
    DeleteScene,
    /// Activer un pattern spécifique (group, pattern idx)
    PatternSelectIndex { group: u8, pattern: usize },
    /// Charge un preset d'instrument drum synth (0..15) sur un sound
    LoadDrumPreset {
        /// Group
        group: u8,
        /// Sound 0..15
        sound: u8,
        /// Preset index 0..15
        preset_idx: u8,
    },
    /// Charge un fichier sample WAV sur un sound
    LoadSample {
        /// Group
        group: u8,
        /// Sound 0..15
        sound: u8,
        /// Path to WAV file
        path: std::path::PathBuf,
    },
    /// Défilement preset dans le Browser (+/- step)
    BrowsePresetStep(i32),
    /// Sélection de la mesure à afficher / éditer
    SelectBar(usize),
    /// Mesure suivante
    NextBar,
    /// Mesure précédente
    PrevBar,
    /// Définir la longueur du pattern en mesures (1, 2, 4, 8)
    SetPatternBars(usize),
    /// Swing 0..1 (knob SWING : synchro séquenceur + écrans)
    Swing(f32),
    /// Sélectionne le pas édité du step sequencer (index absolu)
    StepCursor(usize),
    /// Défile la grille du step sequencer (delta en pas, +/-)
    StepScroll(i32),
    /// Vélocité du pas sélectionné (delta 1..127)
    StepVelocityDelta(i32),
    /// Position (nudge) du pas sélectionné (delta en ticks)
    StepNudgeTicks(i32),
    /// Longueur du pas sélectionné (delta en pas de 1/16)
    StepLengthDelta(i32),
    /// Pitch du pas sélectionné (delta en demi-tons)
    StepPitchDelta(i32),
    /// Retour au début (bouton RESTART hardware)
    Restart,
    /// Soft key d'écran (1..8 : boutons sous les écrans) — action
    /// contextuelle identique au hardware et à la GUI.
    SoftKey(u8),
}

/// Noms des 16 presets de batterie DSP 808
pub const DRUM_PRESET_NAMES: [&str; 16] = [
    "Kick 808",
    "Snare 808",
    "Closed HH",
    "Open HH",
    "Tom Low",
    "Tom Mid",
    "Tom High",
    "Clap 808",
    "Rimshot",
    "Shaker",
    "Cowbell",
    "Crash",
    "Perc 1",
    "Perc 2",
    "Synth Stab",
    "Sub Bass 808",
];

/// Snapshot lecture seule pour UI.
#[derive(Debug, Clone)]
pub struct EngineSnapshot {
    /// Projet partagé (O(1) — plus de clone du projet par frame).
    pub project: Arc<Project>,
    /// Transport clone
    pub transport: Transport,
    /// Group focus
    pub group_focus: u8,
    /// Scene active
    pub active_scene: Option<usize>,
    /// Position BBT
    pub bbt: BarsBeatsTicks,
    /// Voix actives
    pub num_voices: usize,
    /// Master volume 0.0..1.0
    pub master_volume: f32,
    /// Headphone volume 0.0..1.0
    pub headphone_volume: f32,
    /// Muted groups bitmask
    pub muted_groups: u8,
    /// Soloed groups bitmask
    pub soloed_groups: u8,
    /// Tab écran gauche actif (0: Arrange, 1: Mixer, 2: Sampler, 3: Browser)
    pub left_tab: u8,
    /// Tab écran droit actif (0: Pattern, 1: Events, 2: Macro, 3: Settings)
    pub right_tab: u8,
    /// Son actif sélectionné (0..15)
    pub active_sound: u8,
    /// Mode Song (arranger) vs Ideas (scènes)
    pub song_mode: bool,
    /// Section active dans le Song
    pub active_section: Option<usize>,
    /// Pads flashés par le séquenceur (lecture pattern) — pour LEDs + GUI
    pub triggered_pads: [bool; 16],
    /// Mode SCENE (pads déclenchent les scènes) — miroir du mapper pour les écrans
    pub scene_mode: bool,
    /// Mode PATTERN (pads sélectionnent le pattern)
    pub pattern_mode: bool,
    /// Mode SELECT (pads sélectionnent sans jouer)
    pub select_mode: bool,
    /// Mode ERASE (pads gomment)
    pub erase_mode: bool,
    /// Mode de jeu : 0 Pad, 1 Keyboard, 2 Chords, 3 Step
    pub controller_mode: u8,
    /// Index preset sélectionné dans le browser (0..15)
    pub browser_preset_idx: u8,
    /// Mesure sélectionnée pour l'affichage / édition pas-à-pas (0-indexed : 0, 1, 2, 3...)
    pub selected_bar: usize,
    /// Pas sélectionné dans le step sequencer (index absolu 0..pattern_bars*16-1)
    pub step_cursor: usize,
    /// Nombre de mesures du pattern actif (1, 2, 4, 8)
    pub pattern_bars: usize,
    /// VU-mètres lissés par groupe (0..8), 0.0..~1.0
    pub group_levels: [f32; 8],
    /// Peak hold par groupe
    pub group_peaks: [f32; 8],
    /// VU master lissé (post volume master)
    pub master_level: f32,
    /// Peak hold master
    pub master_peak: f32,
    /// Labels des 8 soft keys (boutons sous les écrans — contexte courant).
    pub soft_labels: [&'static str; 8],
    /// Enveloppe (min,max) du sample du Sound actif — waveform des écrans.
    pub waveform: Option<Vec<(f32, f32)>>,
    /// Durée du sample du Sound actif (secondes).
    pub sample_duration: f32,
    /// Compteur de révision : change dès qu'un état a bougé (repaint UI/écrans).
    pub revision: u64,
}

/// Petit instantané utilisé par les LED MK3 à haute fréquence.
/// Il évite de cloner tout le projet à chaque rafraîchissement matériel.
#[derive(Debug, Clone)]
pub struct HardwareSnapshot {
    pub group_focus: u8,
    pub active_sound: u8,
    pub left_tab: u8,
    pub right_tab: u8,
    pub is_playing: bool,
    pub is_recording: bool,
    pub is_stopped: bool,
    pub current_bar: usize,
    pub current_step: usize,
    pub is_quarter_beat: bool,
    pub active_steps: [bool; 16],
    pub triggered_pads: [bool; 16],
    pub master_volume: f32,
    pub muted_groups: u8,
    pub soloed_groups: u8,
    pub active_scene: Option<usize>,
    pub num_scenes: usize,
    pub group_num_patterns: usize,
    pub scene_pattern: Option<usize>,
    pub loop_enabled: bool,
    pub metronome: bool,
    pub selected_bar: usize,
    pub pattern_bars: usize,
    /// Pas sélectionné du step sequencer (index absolu)
    pub step_cursor: usize,
    pub pad_colors: [Option<u8>; 16],
}

/// Engine principal.
pub struct Engine {
    /// `Arc<RwLock<Arc<Project>>>` : lecture O(1) pour les snapshots (UI/écrans),
    /// écriture copy-on-write via `Arc::make_mut` (actions utilisateur seulement).
    project: Arc<RwLock<Arc<Project>>>,
    /// Transport (public pour accès sample_rate depuis audio thread)
    pub transport: Transport,
    sampler: Sampler,
    /// Presets 808 pré-rendus, réutilisés par le Browser sans synthèse au clic.
    preset_samples: Vec<Option<std::sync::Arc<Vec<f32>>>>,
    sequencer: Sequencer,
    group_focus: u8,
    active_scene: Option<usize>,
    // pattern tick cursors pour chaque Group/Pattern actif
    #[allow(dead_code)]
    pattern_cursors: Vec<u64>,
    /// Master volume 0.0..1.0
    master_volume: f32,
    /// Headphone volume 0.0..1.0
    headphone_volume: f32,
    /// Muted groups bitmask
    muted_groups: u8,
    /// Soloed groups bitmask
    soloed_groups: u8,
    /// Tab écran gauche
    pub left_tab: u8,
    /// Tab écran droit
    pub right_tab: u8,
    /// Son actif
    pub active_sound: u8,
    /// Mesure sélectionnée pour l'édition et l'affichage (0..pattern_bars-1)
    pub selected_bar: usize,
    /// Curseur du step sequencer : pas sélectionné (index absolu dans le
    /// pattern, 0..pattern_bars*16-1) — les knobs éditent ce pas.
    pub step_cursor: usize,
    /// Index preset sélectionné dans le Browser
    pub browser_preset_idx: u8,
    /// Mode Song (arranger) vs Ideas (scènes)
    pub song_mode: bool,
    /// Section active dans le Song
    pub active_section: Option<usize>,
    /// Flash pads jusqu'à ce tick (séquenceur) — 16 pads
    pad_flash_until: [u64; 16],
    /// Modes workflow (miroirs du mapper pour les écrans)
    pub scene_mode: bool,
    pub pattern_mode: bool,
    pub select_mode: bool,
    pub erase_mode: bool,
    /// Mode de jeu 0..3 (Pad/Keyboard/Chords/Step)
    pub controller_mode: u8,
    /// Clic métronome accentué (début de mesure)
    click_accent: std::sync::Arc<Vec<f32>>,
    /// Clic métronome normal (temps)
    click_normal: std::sync::Arc<Vec<f32>>,
    /// VU-mètres : niveau lissé par groupe (0..8) + peak hold
    group_meter: [f32; 8],
    /// Peak hold par groupe
    group_peak_hold: [f32; 8],
    /// VU master lissé + peak hold (post volume master)
    master_meter: f32,
    /// Peak hold master
    master_peak_hold: f32,
    /// Compteur de révision (incrémenté à chaque changement d'état) :
    /// l'UI et les écrans évitent de re-rendre/repaint inutilement.
    revision: u64,
}

/// Copie-on-write du projet pour les écritures (les snapshots tiennent un `Arc`).
fn project_mut<'g>(
    guard: &'g mut parking_lot::RwLockWriteGuard<'_, Arc<Project>>,
) -> &'g mut Project {
    Arc::make_mut(&mut **guard)
}

/// Handle partagé du projet pour l'UI : `read()` O(1) (renvoie un `Arc<Project>`),
/// `write()` applique un copy-on-write via `Arc::make_mut`.
#[derive(Clone)]
pub struct ProjectHandle(Arc<RwLock<Arc<Project>>>);

impl ProjectHandle {
    /// Lecture (verrou partagé, O(1) — ne clone pas le projet).
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, Arc<Project>> {
        self.0.read()
    }

    /// Écriture copy-on-write.
    pub fn write(&self) -> ProjectWriteGuard<'_> {
        ProjectWriteGuard {
            guard: self.0.write(),
        }
    }
}

/// Garde d'écriture projet : deref vers `Project`, `Arc::make_mut` à la demande.
pub struct ProjectWriteGuard<'a> {
    guard: parking_lot::RwLockWriteGuard<'a, Arc<Project>>,
}

impl std::ops::Deref for ProjectWriteGuard<'_> {
    type Target = Project;
    fn deref(&self) -> &Project {
        &self.guard
    }
}

impl std::ops::DerefMut for ProjectWriteGuard<'_> {
    fn deref_mut(&mut self) -> &mut Project {
        Arc::make_mut(&mut *self.guard)
    }
}

impl Engine {
    /// Crée avec un projet.
    #[must_use]
    pub fn new(project: Project, sample_rate: u32) -> Self {
        let bpm = project.bpm;
        fn click(sr: u32, freq: f32) -> std::sync::Arc<Vec<f32>> {
            let n = (sr as f32 * 0.03) as usize;
            let v: Vec<f32> = (0..n)
                .map(|i| {
                    let t = i as f32 / sr as f32;
                    let env = (-t * 120.0).exp();
                    (2.0 * std::f32::consts::PI * freq * t).sin() * env * 0.6
                })
                .collect();
            std::sync::Arc::new(v)
        }
        Self {
            project: Arc::new(RwLock::new(Arc::new(project))),
            transport: Transport::new(bpm, sample_rate),
            sampler: Sampler::new(sample_rate),
            preset_samples: vec![None; 16],
            sequencer: Sequencer::new(Grid::Sixteenth, 0.0),
            group_focus: 0,
            active_scene: Some(0),
            pattern_cursors: Vec::new(),
            master_volume: 0.85,
            headphone_volume: 0.85,
            muted_groups: 0,
            soloed_groups: 0,
            left_tab: 0,
            right_tab: 0,
            active_sound: 0,
            selected_bar: 0,
            step_cursor: 0,
            browser_preset_idx: 0,
            song_mode: false,
            active_section: None,
            pad_flash_until: [0; 16],
            scene_mode: false,
            pattern_mode: false,
            select_mode: false,
            erase_mode: false,
            controller_mode: 0,
            click_accent: click(sample_rate, 1760.0),
            click_normal: click(sample_rate, 880.0),
            group_meter: [0.0; 8],
            group_peak_hold: [0.0; 8],
            master_meter: 0.0,
            master_peak_hold: 0.0,
            revision: 0,
        }
    }

    /// Démo.
    #[must_use]
    pub fn new_demo(sample_rate: u32) -> Self {
        let mut eng = Self::new(Project::new_demo(), sample_rate);
        eng.load_default_808_kit();
        eng
    }

    /// Handle partagé du projet (pour UI).
    #[must_use]
    pub fn project_handle(&self) -> ProjectHandle {
        ProjectHandle(Arc::clone(&self.project))
    }

    /// Snapshot pour UI — `project` est un `Arc` (O(1), pas de clone).
    pub fn snapshot(&self) -> EngineSnapshot {
        let project = Arc::clone(&self.project.read());
        let playing = self.transport.is_playing();
        let mut triggered_pads = [false; 16];
        if playing {
            for (i, triggered) in triggered_pads.iter_mut().enumerate() {
                *triggered = self.transport.tick <= self.pad_flash_until[i];
            }
        }
        let pattern_bars = project
            .active_pattern_index(self.group_focus as usize, self.active_scene)
            .and_then(|pi| {
                project
                    .groups
                    .get(self.group_focus as usize)?
                    .patterns
                    .get(pi)
            })
            .map(|p| (p.length.bars as usize).max(1))
            .unwrap_or(2);
        let selected_bar = self.selected_bar.min(pattern_bars.saturating_sub(1));
        let step_cursor = self.step_cursor.min(pattern_bars * 16 - 1);

        EngineSnapshot {
            project,
            transport: self.transport.clone(),
            group_focus: self.group_focus,
            active_scene: self.active_scene,
            bbt: self.transport.position_bbt(),
            num_voices: self.sampler.num_voices(),
            master_volume: self.master_volume,
            headphone_volume: self.headphone_volume,
            muted_groups: self.muted_groups,
            soloed_groups: self.soloed_groups,
            left_tab: self.left_tab,
            right_tab: self.right_tab,
            active_sound: self.active_sound,
            song_mode: self.song_mode,
            active_section: self.active_section,
            triggered_pads,
            scene_mode: self.scene_mode,
            pattern_mode: self.pattern_mode,
            select_mode: self.select_mode,
            erase_mode: self.erase_mode,
            controller_mode: self.controller_mode,
            browser_preset_idx: self.browser_preset_idx,
            selected_bar,
            step_cursor,
            pattern_bars,
            group_levels: self.group_meter,
            group_peaks: self.group_peak_hold,
            master_level: self.master_meter,
            master_peak: self.master_peak_hold,
            soft_labels: self.soft_key_labels(),
            waveform: self.sampler.waveform_envelope(self.active_sound, 120),
            sample_duration: self.sampler.sample_duration_secs(self.active_sound),
            revision: self.revision,
        }
    }

    /// Instantané compact pour synchroniser les LED sans cloner le projet.
    pub fn hardware_snapshot(&self) -> HardwareSnapshot {
        let project = self.project.read();
        let playing = self.transport.is_playing();
        let recording = self.transport.state == TransportState::Recording;
        let pattern_bars = project
            .active_pattern_index(self.group_focus as usize, self.active_scene)
            .and_then(|pattern_index| {
                project
                    .groups
                    .get(self.group_focus as usize)?
                    .patterns
                    .get(pattern_index)
            })
            .map(|pattern| (pattern.length.bars as usize).max(1))
            .unwrap_or(2);
        let selected_bar = self.selected_bar.min(pattern_bars.saturating_sub(1));
        let current_bar = ((self.transport.tick / (4 * 960)) as usize) % pattern_bars;
        let current_step = ((self.transport.tick / 240) % 16) as usize;
        let mut triggered_pads = [false; 16];
        if playing {
            for (index, triggered) in triggered_pads.iter_mut().enumerate() {
                *triggered = self.transport.tick <= self.pad_flash_until[index];
            }
        }

        let mut active_steps = [false; 16];
        let group = project.groups.get(self.group_focus as usize);
        if let Some(pattern) = project
            .active_pattern_index(self.group_focus as usize, self.active_scene)
            .and_then(|pattern_index| group.and_then(|g| g.patterns.get(pattern_index)))
        {
            let bar_offset = selected_bar as u32 * 4 * 960;
            for event in &pattern.events {
                if event.pad == self.active_sound
                    && event.tick >= bar_offset
                    && event.tick < bar_offset + 4 * 960
                {
                    let step = ((event.tick - bar_offset) / 240) as usize;
                    if step < active_steps.len() {
                        active_steps[step] = true;
                    }
                }
            }
        }

        let mut pad_colors = [None; 16];
        if let Some(group) = group {
            for (index, color) in pad_colors.iter_mut().enumerate() {
                *color = group.sounds[index].as_ref().map(|sound| sound.color);
            }
        }
        let group_num_patterns = group.map(|g| g.patterns.len().min(16)).unwrap_or(0);
        let scene_pattern = self.active_scene.and_then(|scene_index| {
            project
                .scenes
                .get(scene_index)
                .and_then(|scene| scene.patterns.get(self.group_focus as usize))
                .copied()
                .flatten()
        });

        HardwareSnapshot {
            group_focus: self.group_focus,
            active_sound: self.active_sound,
            left_tab: self.left_tab,
            right_tab: self.right_tab,
            is_playing: playing,
            is_recording: recording,
            is_stopped: !playing && !recording,
            current_bar,
            current_step,
            is_quarter_beat: (self.transport.tick % 960) < 240 && playing,
            active_steps,
            triggered_pads,
            master_volume: self.master_volume,
            muted_groups: self.muted_groups,
            soloed_groups: self.soloed_groups,
            active_scene: self.active_scene,
            num_scenes: project.scenes.len().min(16),
            group_num_patterns,
            scene_pattern,
            loop_enabled: self.transport.loop_enabled,
            metronome: self.transport.metronome,
            selected_bar,
            pattern_bars,
            step_cursor: self.step_cursor.min(pattern_bars * 16 - 1),
            pad_colors,
        }
    }

    /// Charge le kit 808 complet DSP dans le sampler
    pub fn load_default_808_kit(&mut self) {
        let kit = crate::drum_synth::generate_complete_808_kit(self.transport.sample_rate);
        for (pad, _name, samp) in kit {
            let sample = std::sync::Arc::new(samp);
            self.preset_samples[pad as usize] = Some(sample.clone());
            self.sampler.clear_sound(pad);
            self.sampler.add_zone(
                pad,
                crate::sampler::ZoneMapping {
                    sample: Some(sample),
                    sample_path: None,
                    sample_rate: self.transport.sample_rate,
                    root_key: 36 + pad,
                    lokey: 0,
                    hikey: 127,
                    lovel: 0,
                    hivel: 127,
                    volume_db: 0.0,
                    pan: 0.0,
                    tune_cents: 0,
                    choke_group: if pad == 2 || pad == 3 { Some(1) } else { None }, // Choke Closed HH <-> Open HH
                    is_one_shot: true,
                },
            );
        }
        self.transport.loop_enabled = true;
        self.transport.loop_start = 0;
        self.transport.loop_end = 2 * 4 * 960;
    }

    /// Humanise le pattern actif (légères variations de vélocité et micro-timing)
    pub fn humanize_active_pattern(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(self.group_focus as usize, self.active_scene);
        if let Some(group) = proj.groups.get_mut(self.group_focus as usize) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    self.sequencer.humanize(&mut pat.events, 8, 12);
                }
            }
        }
    }

    /// Quantifie le pattern actif sur la grille courante
    pub fn quantize_active_pattern(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(self.group_focus as usize, self.active_scene);
        if let Some(group) = proj.groups.get_mut(self.group_focus as usize) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    self.sequencer.quantize_pattern(pat);
                }
            }
        }
    }

    /// Traite un `EngineEvent`.
    pub fn handle_event(&mut self, ev: EngineEvent) -> crate::Result<()> {
        // Toute action utilisateur invalide l'affichage (repaint UI + écrans).
        self.revision = self.revision.wrapping_add(1);
        match ev {
            EngineEvent::Pad {
                pad,
                velocity,
                pressed,
            } => {
                // Groupe muté ? pas de son ni d'enregistrement
                if self.muted_groups & (1 << self.group_focus) != 0 {
                    return Ok(());
                }
                if pressed {
                    // trigger sampler pour le Sound du Group focus
                    let sound = pad;
                    let note = 36 + pad;
                    let guard = self.project.read();
                    let group_gain = guard
                        .groups
                        .get(self.group_focus as usize)
                        .map(|g| 10.0f32.powf(g.volume_db / 20.0))
                        .unwrap_or(1.0);
                    let (snd_gain, snd_pan, vparams) = guard
                        .groups
                        .get(self.group_focus as usize)
                        .and_then(|g| g.sounds.get(pad as usize).and_then(|s| s.as_ref()))
                        .map(|s| {
                            (
                                10.0f32.powf(s.volume_db / 20.0),
                                s.pan,
                                crate::sampler::VoiceParams {
                                    attack_ms: s.adsr.attack_ms,
                                    decay_ms: s.adsr.decay_ms,
                                    sustain: s.adsr.sustain,
                                    release_ms: s.adsr.release_ms,
                                    cutoff_hz: s.filter.cutoff_hz,
                                    resonance: s.filter.resonance,
                                },
                            )
                        })
                        .unwrap_or((1.0, 0.0, crate::sampler::VoiceParams::default()));
                    drop(guard);
                    if let Some(idx) = self.sampler.note_on(sound, note, velocity, note) {
                        self.sampler.set_voice_gain(idx, group_gain * snd_gain);
                        self.sampler.set_voice_pan(idx, snd_pan);
                        self.sampler.set_voice_group(idx, self.group_focus);
                        self.sampler.set_voice_params(idx, &vparams);
                    }
                    // si Recording, ajoute event au pattern actif
                    if self.transport.state == TransportState::Recording {
                        self.record_pad_event(pad, velocity);
                    }
                } else {
                    let note = 36 + pad;
                    self.sampler.note_off(pad, note);
                }
            }
            EngineEvent::Transport { play, rec, stop } => {
                if play {
                    self.transport.play();
                }
                if rec {
                    self.transport.record();
                }
                if stop {
                    self.transport.stop();
                }
            }
            EngineEvent::Bpm(bpm) => {
                self.transport.bpm = bpm.clamp(20.0, 999.0);
                project_mut(&mut self.project.write()).bpm = self.transport.bpm;
            }
            EngineEvent::GroupFocus(g) => {
                self.group_focus = g.min(7);
            }
            EngineEvent::Scene(s) => {
                let len = self.project.read().scenes.len();
                if s < len {
                    self.active_scene = Some(s);
                }
            }
            EngineEvent::AddScene => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                let index = project.scenes.len();
                let group_count = project.groups.len();
                project
                    .scenes
                    .push(Scene::new(&format!("Scene {}", index + 1), group_count));
                self.active_scene = Some(index);
                self.song_mode = false;
            }
            EngineEvent::Quantize { group, pattern } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group) {
                    if let Some(p) = g.patterns.get_mut(pattern) {
                        self.sequencer.quantize_pattern(p);
                    }
                }
            }
            EngineEvent::Save(path) => {
                self.project.read().save_json(&path)?;
            }
            EngineEvent::Load(path) => {
                let p = Project::load_json(&path)?;
                self.transport.bpm = p.bpm;
                *self.project.write() = Arc::new(p);
            }
            EngineEvent::MasterVolume(v) => {
                self.master_volume = v.clamp(0.0, 1.0);
            }
            EngineEvent::HeadphoneVolume(v) => {
                self.headphone_volume = v.clamp(0.0, 1.0);
            }
            EngineEvent::Mute(g) => {
                if g < 8 {
                    self.muted_groups ^= 1 << g;
                }
            }
            EngineEvent::Solo(g) => {
                if g < 8 {
                    self.soloed_groups ^= 1 << g;
                }
            }
            EngineEvent::PatternSelect { group, pattern } => {
                // Select a pattern in the focused group for the active scene
                if let Some(scene_idx) = self.active_scene {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    if let Some(scene) = proj.scenes.get_mut(scene_idx) {
                        if (group as usize) < scene.patterns.len() {
                            scene.patterns[group as usize] = Some(pattern);
                        }
                    }
                }
            }
            EngineEvent::SetLeftTab(tab) => {
                self.left_tab = tab.min(3);
            }
            EngineEvent::SetRightTab(tab) => {
                self.right_tab = tab.min(3);
            }
            EngineEvent::SelectSound(snd) => {
                self.active_sound = snd.min(15);
            }
            EngineEvent::SoundVolume {
                group,
                sound,
                volume_db,
            } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group as usize) {
                    if let Some(s) = g.sounds.get_mut(sound as usize).and_then(|s| s.as_mut()) {
                        s.volume_db = volume_db.clamp(-60.0, 12.0);
                    }
                }
            }
            EngineEvent::SoundPan { group, sound, pan } => {
                let p = pan.clamp(-1.0, 1.0);
                {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    if let Some(g) = proj.groups.get_mut(group as usize) {
                        if let Some(s) = g.sounds.get_mut(sound as usize).and_then(|s| s.as_mut()) {
                            s.pan = p;
                        }
                    }
                }
                if group == self.group_focus {
                    self.sampler.update_sound_pan(sound, p);
                }
            }
            EngineEvent::SoundFilterCutoff {
                group,
                sound,
                cutoff_hz,
            } => {
                let c = cutoff_hz.clamp(20.0, 20000.0);
                let mut reso = 0.0;
                {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    if let Some(g) = proj.groups.get_mut(group as usize) {
                        if let Some(s) = g.sounds.get_mut(sound as usize).and_then(|s| s.as_mut()) {
                            s.filter.cutoff_hz = c;
                            if s.filter.kind == crate::project::FilterKind::Off {
                                s.filter.kind = crate::project::FilterKind::LowPass;
                            }
                            reso = s.filter.resonance;
                        }
                    }
                }
                if group == self.group_focus {
                    self.sampler.update_sound_filter(sound, c, reso);
                }
            }
            EngineEvent::SoundFilterReso {
                group,
                sound,
                resonance,
            } => {
                let r = resonance.clamp(0.0, 1.0);
                let mut cutoff = 20000.0;
                {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    if let Some(g) = proj.groups.get_mut(group as usize) {
                        if let Some(s) = g.sounds.get_mut(sound as usize).and_then(|s| s.as_mut()) {
                            s.filter.resonance = r;
                            if s.filter.kind == crate::project::FilterKind::Off {
                                s.filter.kind = crate::project::FilterKind::LowPass;
                            }
                            cutoff = s.filter.cutoff_hz;
                        }
                    }
                }
                if group == self.group_focus {
                    self.sampler.update_sound_filter(sound, cutoff, r);
                }
            }
            EngineEvent::SoundAdsr {
                group,
                sound,
                attack_ms,
                decay_ms,
                sustain,
                release_ms,
            } => {
                let a = attack_ms.clamp(0.1, 5000.0);
                let d = decay_ms.clamp(0.1, 5000.0);
                let su = sustain.clamp(0.0, 1.0);
                let rel = release_ms.clamp(0.1, 5000.0);
                {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    if let Some(g) = proj.groups.get_mut(group as usize) {
                        if let Some(s) = g.sounds.get_mut(sound as usize).and_then(|s| s.as_mut()) {
                            s.adsr.attack_ms = a;
                            s.adsr.decay_ms = d;
                            s.adsr.sustain = su;
                            s.adsr.release_ms = rel;
                        }
                    }
                }
                if group == self.group_focus {
                    self.sampler.update_sound_adsr(sound, a, d, su, rel);
                }
            }
            EngineEvent::GroupVolume { group, volume_db } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group as usize) {
                    g.volume_db = volume_db.clamp(-60.0, 12.0);
                }
            }
            EngineEvent::GroupPan { group, pan } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group as usize) {
                    g.pan = pan.clamp(-1.0, 1.0);
                }
            }
            EngineEvent::Variation => {
                self.humanize_active_pattern();
            }
            EngineEvent::QuantizeActive => {
                self.quantize_active_pattern();
            }
            EngineEvent::SetMetronome(on) => {
                self.transport.metronome = on;
            }
            EngineEvent::ToggleSongMode => {
                self.song_mode = !self.song_mode;
                if self.song_mode && self.active_section.is_none() {
                    self.active_section = if self.project.read().song.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                }
            }
            EngineEvent::NextSection => {
                let n = self.project.read().song.len();
                if n > 0 {
                    let cur = self.active_section.unwrap_or(0);
                    self.active_section = Some((cur + 1) % n);
                    if let Some(si) = self.active_section {
                        if let Some(sec) = self.project.read().song.get(si) {
                            self.active_scene = Some(sec.scene);
                        }
                    }
                }
            }
            EngineEvent::PrevSection => {
                let n = self.project.read().song.len();
                if n > 0 {
                    let cur = self.active_section.unwrap_or(0);
                    self.active_section = Some((cur + n - 1) % n);
                    if let Some(si) = self.active_section {
                        if let Some(sec) = self.project.read().song.get(si) {
                            self.active_scene = Some(sec.scene);
                        }
                    }
                }
            }
            EngineEvent::SelectSection(index) => {
                if let Some(section) = self.project.read().song.get(index) {
                    self.active_section = Some(index);
                    self.active_scene = Some(section.scene);
                }
            }
            EngineEvent::AddSection => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                let scene = self
                    .active_scene
                    .or_else(|| (!project.scenes.is_empty()).then_some(0));
                if let Some(scene) = scene {
                    let index = project.song.len();
                    project.song.push(Section {
                        scene,
                        length: BarsBeatsTicks::new(2, 0, 0),
                        name: format!("Section {}", index + 1),
                    });
                    self.active_scene = Some(scene);
                    self.active_section = Some(index);
                    self.song_mode = true;
                }
            }
            EngineEvent::PatternSelectIndex { group, pattern } => {
                let _ = self.handle_event(EngineEvent::PatternSelect { group, pattern });
            }
            EngineEvent::DuplicateSection => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                if let Some(idx) = self.active_section {
                    if let Some(sec) = project.song.get(idx).cloned() {
                        let insert = idx + 1;
                        project.song.insert(
                            insert,
                            Section {
                                name: format!("{} copy", sec.name),
                                ..sec
                            },
                        );
                        self.active_section = Some(insert);
                    }
                }
            }
            EngineEvent::DeleteSection => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                if let Some(idx) = self.active_section {
                    if idx < project.song.len() {
                        project.song.remove(idx);
                    }
                    self.active_section = if project.song.is_empty() {
                        None
                    } else {
                        Some(idx.min(project.song.len() - 1))
                    };
                }
            }
            EngineEvent::ResizeSection { bars } => {
                let bars = bars.clamp(1, 64);
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                if let Some(idx) = self.active_section {
                    if let Some(sec) = project.song.get_mut(idx) {
                        sec.length = BarsBeatsTicks::new(bars as u16, 0, 0);
                    }
                }
            }
            EngineEvent::DuplicateScene => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                if let Some(idx) = self.active_scene {
                    if let Some(scene) = project.scenes.get(idx).cloned() {
                        let insert = idx + 1;
                        project.scenes.insert(
                            insert,
                            Scene {
                                name: format!("{} copy", scene.name),
                                ..scene
                            },
                        );
                        self.active_scene = Some(insert);
                    }
                }
            }
            EngineEvent::DeleteScene => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                if project.scenes.len() > 1 {
                    if let Some(idx) = self.active_scene {
                        if idx < project.scenes.len() {
                            project.scenes.remove(idx);
                            // Sections : retire celles de la scène supprimée,
                            // recale les indices supérieurs.
                            project.song.retain(|s| s.scene != idx);
                            for sec in &mut project.song {
                                if sec.scene > idx {
                                    sec.scene -= 1;
                                }
                            }
                            self.active_scene = Some(idx.min(project.scenes.len() - 1));
                            self.active_section = if project.song.is_empty() {
                                None
                            } else {
                                Some(self.active_section.unwrap_or(0).min(project.song.len() - 1))
                            };
                        }
                    }
                }
            }
            EngineEvent::EraseSound { group, sound } => {
                self.erase_sound(group, sound);
            }
            EngineEvent::SetSceneMode(v) => {
                self.scene_mode = v;
            }
            EngineEvent::SetPatternMode(v) => {
                self.pattern_mode = v;
            }
            EngineEvent::SetSelectMode(v) => {
                self.select_mode = v;
            }
            EngineEvent::SetEraseMode(v) => {
                self.erase_mode = v;
            }
            EngineEvent::SetControllerMode(v) => {
                self.controller_mode = v.min(3);
            }
            EngineEvent::ToggleLoop => {
                self.transport.loop_enabled = !self.transport.loop_enabled;
            }
            EngineEvent::ToggleMetronome => {
                self.transport.metronome = !self.transport.metronome;
            }
            EngineEvent::LoadDrumPreset {
                group,
                sound,
                preset_idx,
            } => {
                let preset_idx = preset_idx.min(15);
                let sr = self.transport.sample_rate;
                let name = DRUM_PRESET_NAMES[preset_idx as usize];
                let sample = self.drum_preset_sample(preset_idx);
                self.sampler.clear_sound(sound);
                self.sampler.add_zone(
                    sound,
                    crate::sampler::ZoneMapping {
                        sample: Some(sample),
                        sample_path: None,
                        sample_rate: sr,
                        root_key: 36 + sound,
                        lokey: 0,
                        hikey: 127,
                        lovel: 0,
                        hivel: 127,
                        volume_db: 0.0,
                        pan: 0.0,
                        tune_cents: 0,
                        choke_group: if preset_idx == 2 || preset_idx == 3 {
                            Some(1)
                        } else {
                            None
                        },
                        is_one_shot: true,
                    },
                );
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group as usize) {
                    if let Some(s) = g.sounds.get_mut(sound as usize).and_then(|s| s.as_mut()) {
                        s.name = name.to_string();
                    }
                }
            }
            EngineEvent::LoadSample { group, sound, path } => {
                self.sampler.clear_sound(sound);
                #[cfg(feature = "sampler")]
                {
                    if let Err(e) = self.sampler.load_wav(sound, &path, 36 + sound) {
                        log::error!("Failed to load sample: {e}");
                    }
                }
                #[cfg(not(feature = "sampler"))]
                {
                    let _ = path;
                    log::warn!("Sampler feature not enabled");
                }
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group as usize) {
                    if let Some(s) = g.sounds.get_mut(sound as usize).and_then(|s| s.as_mut()) {
                        if let Some(file_name) = path.file_stem().and_then(|s| s.to_str()) {
                            s.name = file_name.to_string();
                        }
                    }
                }
            }
            EngineEvent::BrowsePresetStep(step) => {
                let next = (self.browser_preset_idx as i32 + step).rem_euclid(16) as u8;
                self.browser_preset_idx = next;
                self.preview_preset(next);
            }
            EngineEvent::SelectBar(bar) => {
                let max_bars = {
                    let proj = self.project.read();
                    proj.active_pattern_index(self.group_focus as usize, self.active_scene)
                        .and_then(|pi| proj.groups.get(self.group_focus as usize)?.patterns.get(pi))
                        .map(|p| (p.length.bars as usize).max(1))
                        .unwrap_or(2)
                };
                self.selected_bar = bar.min(max_bars.saturating_sub(1));
            }
            EngineEvent::NextBar => {
                let max_bars = {
                    let proj = self.project.read();
                    proj.active_pattern_index(self.group_focus as usize, self.active_scene)
                        .and_then(|pi| proj.groups.get(self.group_focus as usize)?.patterns.get(pi))
                        .map(|p| (p.length.bars as usize).max(1))
                        .unwrap_or(2)
                };
                if self.selected_bar + 1 < max_bars {
                    self.selected_bar += 1;
                }
            }
            EngineEvent::PrevBar => {
                self.selected_bar = self.selected_bar.saturating_sub(1);
            }
            EngineEvent::Restart => {
                self.transport.tick = 0;
                self.transport.tick_fraction = 0.0;
                self.selected_bar = 0;
            }
            EngineEvent::SoftKey(n) => {
                return self.handle_soft_key(n);
            }
            EngineEvent::Swing(v) => {
                self.set_swing(v);
            }
            EngineEvent::StepCursor(step) => {
                self.step_cursor = step;
                self.selected_bar = step / 16;
            }
            EngineEvent::StepScroll(delta) => {
                let total = self.pattern_total_steps();
                if total > 0 {
                    let next = (self.step_cursor as i64 + delta as i64).clamp(0, total as i64 - 1)
                        as usize;
                    self.step_cursor = next;
                    self.selected_bar = next / 16;
                }
            }
            EngineEvent::StepVelocityDelta(delta) => {
                let pad = self.active_sound;
                let (start, end) = self.step_window();
                let _ = self.with_active_pattern(|pat| {
                    if let Some(ev) = pat
                        .events
                        .iter_mut()
                        .find(|e| e.pad == pad && e.tick >= start && e.tick < end)
                    {
                        ev.velocity = (ev.velocity as i32 + delta).clamp(1, 127) as u8;
                    }
                });
            }
            EngineEvent::StepNudgeTicks(delta) => {
                let pad = self.active_sound;
                let (start, end) = self.step_window();
                let _ = self.with_active_pattern(|pat| {
                    let len_ticks = (pat.length.bars as u32).max(1) * 4 * 960;
                    let idx = pat
                        .events
                        .iter()
                        .position(|e| e.pad == pad && e.tick >= start && e.tick < end);
                    if let Some(i) = idx {
                        let t = (pat.events[i].tick as i64 + delta as i64)
                            .clamp(0, len_ticks as i64 - 1) as u32;
                        pat.events[i].tick = t;
                        // Fusionne avec un pas voisin si on glisse dans sa cellule.
                        let keep = i;
                        let mut seen = 0usize;
                        pat.events.retain(|e| {
                            let this = seen;
                            seen += 1;
                            !(this != keep
                                && e.pad == pad
                                && crate::project::same_step_cell(e.tick, t))
                        });
                        pat.events.sort_by_key(|e| e.tick);
                    }
                });
            }
            EngineEvent::StepLengthDelta(delta) => {
                let pad = self.active_sound;
                let (start, end) = self.step_window();
                let _ = self.with_active_pattern(|pat| {
                    if let Some(ev) = pat
                        .events
                        .iter_mut()
                        .find(|e| e.pad == pad && e.tick >= start && e.tick < end)
                    {
                        ev.length_ticks =
                            (ev.length_ticks as i32 + delta * 240).clamp(30, 3840 * 8) as u32;
                    }
                });
            }
            EngineEvent::StepPitchDelta(delta) => {
                let pad = self.active_sound;
                let (start, end) = self.step_window();
                let _ = self.with_active_pattern(|pat| {
                    if let Some(ev) = pat
                        .events
                        .iter_mut()
                        .find(|e| e.pad == pad && e.tick >= start && e.tick < end)
                    {
                        ev.pitch = (ev.pitch as i32 + delta).clamp(0, 127) as u8;
                    }
                });
            }
            EngineEvent::SetPatternBars(bars) => {
                let bars_clamped = bars.clamp(1, 16) as u16;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let pat_idx =
                    proj.active_pattern_index(self.group_focus as usize, self.active_scene);
                if let Some(g) = proj.groups.get_mut(self.group_focus as usize) {
                    if let Some(pi) = pat_idx {
                        if let Some(p) = g.patterns.get_mut(pi) {
                            p.length = crate::project::BarsBeatsTicks::new(bars_clamped, 0, 0);
                        }
                    }
                }
                self.transport.loop_end = (bars_clamped as u64) * 4 * 960;
                self.selected_bar = self.selected_bar.min(bars_clamped as usize - 1);
            }
        }
        Ok(())
    }

    /// Index preset sélectionné dans le Browser
    #[must_use]
    pub fn browser_preset_idx(&self) -> u8 {
        self.browser_preset_idx
    }

    /// Joue un aperçu sonore immédiat d'un preset drum synth (pour pré-écoute dans le browser)
    pub fn preview_preset(&mut self, preset_idx: u8) {
        let sample = self.drum_preset_sample(preset_idx.min(15));
        self.sampler.play_raw(&sample, 0.7);
    }

    fn drum_preset_sample(&self, preset_idx: u8) -> std::sync::Arc<Vec<f32>> {
        if let Some(sample) = self
            .preset_samples
            .get(preset_idx as usize)
            .and_then(Option::as_ref)
        {
            return sample.clone();
        }

        let sample_rate = self.transport.sample_rate;
        let sample = match preset_idx {
            0 => crate::drum_synth::generate_kick(sample_rate),
            1 => crate::drum_synth::generate_snare(sample_rate),
            2 => crate::drum_synth::generate_closed_hh(sample_rate),
            3 => crate::drum_synth::generate_open_hh(sample_rate),
            4 => crate::drum_synth::generate_tom(sample_rate, 85.0),
            5 => crate::drum_synth::generate_tom(sample_rate, 130.0),
            6 => crate::drum_synth::generate_tom(sample_rate, 185.0),
            7 => crate::drum_synth::generate_clap(sample_rate),
            8 => crate::drum_synth::generate_rimshot(sample_rate),
            9 => crate::drum_synth::generate_shaker(sample_rate),
            10 => crate::drum_synth::generate_cowbell(sample_rate),
            11 => crate::drum_synth::generate_crash(sample_rate),
            12 => crate::drum_synth::generate_perc(sample_rate, 880.0),
            13 => crate::drum_synth::generate_perc(sample_rate, 1250.0),
            14 => crate::drum_synth::generate_synth_stab(sample_rate),
            _ => crate::drum_synth::generate_sub_bass(sample_rate),
        };
        std::sync::Arc::new(sample)
    }

    /// Efface toutes les notes d'un son dans le pattern actif du groupe (mode ERASE + pad).
    pub fn erase_sound(&mut self, group: u8, sound: u8) {
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group as usize, self.active_scene);
        if let Some(g) = proj.groups.get_mut(group as usize) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = g.patterns.get_mut(pi) {
                    let before = pat.events.len();
                    pat.events.retain(|e| e.pad != sound);
                    if pat.events.len() != before {
                        log::debug!("erase sound {sound} in group {group}");
                    }
                }
            }
        }
    }

    /// Joue le clic métronome (accent = début de mesure).
    fn trigger_click(&mut self, accent: bool) {
        let sample = if accent {
            self.click_accent.clone()
        } else {
            self.click_normal.clone()
        };
        self.sampler
            .play_raw(&sample, if accent { 0.5 } else { 0.32 });
    }

    /// Avance le transport + séquenceur, déclenche les notes des patterns.
    pub fn advance(&mut self, samples: u64) {
        let prev_tick = self.transport.tick;
        self.transport.advance_samples(samples);
        let new_tick = self.transport.tick;
        if new_tick != prev_tick {
            self.revision = self.revision.wrapping_add(1);
        }
        if !self.transport.is_playing() {
            return;
        }
        // Clic métronome sur chaque temps (accent sur début de mesure).
        if self.transport.metronome && new_tick > prev_tick {
            let first_beat = prev_tick / 960 + 1;
            let last_beat = new_tick / 960;
            for beat in first_beat..=last_beat.min(first_beat + 8) {
                let accent = beat % 4 == 0;
                self.trigger_click(accent);
            }
        }
        // Pour chaque Group, joue les events du pattern actif de la scène courante
        // (verrou lecture O(1) : plus de clone du projet dans le callback audio !)
        let proj = self.project.read();

        // Mode Song : détermine la scène active selon la position dans le Song
        if self.song_mode && !proj.song.is_empty() {
            let total: u64 = proj.song.iter().map(|s| s.length.to_ticks() as u64).sum();
            if total > 0 {
                let pos = new_tick % total;
                let mut acc = 0u64;
                let mut idx = 0usize;
                for (i, sec) in proj.song.iter().enumerate() {
                    let len = sec.length.to_ticks() as u64;
                    if pos < acc + len {
                        idx = i;
                        break;
                    }
                    acc += len;
                    idx = (i + 1).min(proj.song.len() - 1);
                }
                self.active_section = Some(idx);
                if let Some(sec) = proj.song.get(idx) {
                    self.active_scene = Some(sec.scene);
                }
            }
        }

        if let Some(scene_idx) = self.active_scene {
            if let Some(scene) = proj.scenes.get(scene_idx) {
                for (gi, pat_opt) in scene.patterns.iter().enumerate() {
                    // Respecte le mute par groupe
                    if (self.muted_groups & (1 << gi)) != 0 {
                        continue;
                    }
                    if proj.groups.get(gi).map(|g| g.muted).unwrap_or(false) {
                        continue;
                    }
                    if let Some(pat_idx) = pat_opt {
                        if let Some(group) = proj.groups.get(gi) {
                            if let Some(pat) = group.patterns.get(*pat_idx) {
                                let gain = 10.0f32.powf(group.volume_db / 20.0);
                                let mut vp = [crate::sampler::VoiceParams::default(); 16];
                                let mut sg = [1.0f32; 16];
                                let mut sp = [0.0f32; 16];
                                for s in 0..16 {
                                    if let Some(snd) = group.sounds[s].as_ref() {
                                        vp[s] = crate::sampler::VoiceParams {
                                            attack_ms: snd.adsr.attack_ms,
                                            decay_ms: snd.adsr.decay_ms,
                                            sustain: snd.adsr.sustain,
                                            release_ms: snd.adsr.release_ms,
                                            cutoff_hz: snd.filter.cutoff_hz,
                                            resonance: snd.filter.resonance,
                                        };
                                        sg[s] = 10.0f32.powf(snd.volume_db / 20.0);
                                        sp[s] = snd.pan;
                                    }
                                }
                                Self::trigger_pattern_events(
                                    pat,
                                    prev_tick,
                                    new_tick,
                                    &mut self.sampler,
                                    gi as u8,
                                    gain,
                                    &sg,
                                    &sp,
                                    &vp,
                                    self.transport.swing,
                                    &mut self.pad_flash_until,
                                    new_tick,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)] // contexte de trigger (pattern + position + sampler)
    fn trigger_pattern_events(
        pat: &crate::project::Pattern,
        prev_tick: u64,
        new_tick: u64,
        sampler: &mut Sampler,
        group: u8,
        group_gain: f32,
        sound_gains: &[f32],
        sound_pans: &[f32],
        voice_params: &[crate::sampler::VoiceParams],
        swing: f32,
        flash_until: &mut [u64; 16],
        now_tick: u64,
    ) {
        let pat_len = pat.length.to_ticks() as u64;
        if pat_len == 0 || prev_tick == new_tick {
            return;
        }
        // Swing Maschine : retarde les doubles-croches impaires à la lecture.
        let swing_ticks = (swing.clamp(0.0, 1.0) * 120.0) as u64;
        // gère wrap pattern (loop)
        let prev_in_pat = prev_tick % pat_len;
        let new_in_pat = new_tick % pat_len;
        let wrapped = new_in_pat < prev_in_pat;
        for ev in &pat.events {
            let tick = ev.tick as u64;
            // Tick effectif avec swing (16e impaire = retardée)
            let eff = if swing_ticks > 0 && (tick / 240) % 2 == 1 {
                tick + swing_ticks
            } else {
                tick
            };
            let in_window = |t: u64| {
                if !wrapped {
                    if prev_tick == 0 {
                        t >= prev_in_pat && t <= new_in_pat
                    } else {
                        t > prev_in_pat && t <= new_in_pat
                    }
                } else {
                    t > prev_in_pat || t <= new_in_pat
                }
            };
            if in_window(eff % pat_len.max(1)) {
                let sound = ev.pad;
                let note = ev.pitch;
                let sg = sound_gains.get(sound as usize).copied().unwrap_or(1.0);
                let sp = sound_pans.get(sound as usize).copied().unwrap_or(0.0);
                if let Some(idx) = sampler.note_on(sound, note, ev.velocity, note) {
                    sampler.set_voice_gain(idx, group_gain * sg);
                    sampler.set_voice_pan(idx, sp);
                    sampler.set_voice_group(idx, group);
                    if let Some(vp) = voice_params.get(sound as usize) {
                        sampler.set_voice_params(idx, vp);
                    }
                }
                // Flash LED du pad (fenêtre d'une triple-croche : bref et net).
                if (sound as usize) < 16 {
                    flash_until[sound as usize] = now_tick + 120;
                }
            }
            // Note-off / longueur : déclenche release quand la fin de l'événement est franchie
            let end_tick = tick + ev.length_ticks as u64;
            let end_in_pat = end_tick % pat_len;
            let off_triggered = if !wrapped {
                end_tick > tick && end_in_pat > prev_in_pat && end_in_pat <= new_in_pat
            } else {
                end_in_pat > prev_in_pat || end_in_pat <= new_in_pat
            };
            // évite de couper la note à t=0 et seulement si la note a déjà sonné
            if off_triggered && end_tick > tick && tick <= new_tick {
                sampler.note_off(ev.pad, ev.pitch);
            }
        }
    }

    fn record_pad_event(&mut self, pad: u8, velocity: u8) {
        let focus = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        if let Some(group) = proj.groups.get_mut(focus) {
            if group.patterns.is_empty() {
                group.patterns.push(crate::project::Pattern::new(
                    "Recorded",
                    BarsBeatsTicks::new(1, 0, 0),
                ));
            }
        }
        let pat_idx = proj.active_pattern_index(focus, self.active_scene);
        if let Some(group) = proj.groups.get_mut(focus) {
            if let Some(idx) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(idx) {
                    let pat_len = pat.length.to_ticks().max(1);
                    let tick_in_pat = (self.transport.tick % pat_len as u64) as u32;
                    let q_tick = self.sequencer.grid.quantize(tick_in_pat);
                    // évite les doublons exacts
                    pat.events.retain(|e| !(e.pad == pad && e.tick == q_tick));
                    pat.events.push(PatternEvent {
                        pad,
                        tick: q_tick,
                        velocity,
                        length_ticks: 120,
                        pitch: 36 + pad,
                    });
                    pat.events.sort_by_key(|e| e.tick);
                }
            }
        }
    }

    /// Render audio stéréo — à appeler depuis callback cpal/jack.
    pub fn render_audio(&mut self, out: &mut [f32]) {
        self.sampler.render(out);
        // VU-mètres : crêtes par groupe du sampler, attaque immédiate / release
        // exponentiel par bloc (≈ -20 dB en 300 ms), peak hold plus lent (~1,7 s).
        let peaks = self.sampler.group_peaks();
        for (g, p) in peaks.iter().enumerate() {
            let p = p.clamp(0.0, 4.0);
            self.group_meter[g] = if p > self.group_meter[g] {
                p
            } else {
                self.group_meter[g] * 0.98
            };
            self.group_peak_hold[g] = if p > self.group_peak_hold[g] {
                p
            } else {
                self.group_peak_hold[g] * 0.996
            };
        }
        // Volume master + crête master (post-volume = ce qu'on entend).
        let mv = self.master_volume;
        let mut peak = 0.0f32;
        for s in out.iter_mut() {
            *s *= mv;
            let a = s.abs();
            if a > peak {
                peak = a;
            }
        }
        self.master_meter = if peak > self.master_meter {
            peak
        } else {
            self.master_meter * 0.98
        };
        self.master_peak_hold = if peak > self.master_peak_hold {
            peak
        } else {
            self.master_peak_hold * 0.996
        };
    }

    /// Sélectionne un preset du browser (curseur) sans pré-écoute.
    pub fn set_browser_preset(&mut self, idx: u8) {
        self.browser_preset_idx = idx.min(15);
        self.revision = self.revision.wrapping_add(1);
    }

    /// Enveloppe waveform du Sound actif (pour l'UI/écrans) — `buckets` = colonnes.
    #[must_use]
    pub fn waveform_envelope(
        &self,
        group: u8,
        sound: u8,
        buckets: usize,
    ) -> Option<Vec<(f32, f32)>> {
        let _ = group;
        self.sampler.waveform_envelope(sound, buckets)
    }

    /// Durée du sample d'un Sound (secondes).
    #[must_use]
    pub fn sample_duration_secs(&self, group: u8, sound: u8) -> f32 {
        let _ = group;
        self.sampler.sample_duration_secs(sound)
    }

    /// Labels des 8 soft keys (boutons sous les écrans) pour la vue courante.
    /// Les mêmes labels sont dessinés en bas des 2 écrans et affichés dans la
    /// GUI : le bouton fait toujours ce que le label annonce.
    #[must_use]
    pub fn soft_key_labels(&self) -> [&'static str; 8] {
        if self.song_mode {
            return [
                "SEC <", "SEC >", "+SEC", "IDEE", "SCN <", "SCN >", "+SCN", "SCN 1",
            ];
        }
        if self.scene_mode {
            return [
                "SCN 1", "SCN 2", "SCN 3", "SCN 4", "SCN 5", "SCN 6", "SCN 7", "SCN 8",
            ];
        }
        if self.controller_mode == 3 && self.left_tab == 0 {
            // Mode STEP : navigation dans la grille + longueur du pattern.
            return [
                "PAS <", "PAS >", "BAR <", "BAR >", "1 MES", "2 MES", "4 MES", "8 MES",
            ];
        }
        match self.left_tab {
            1 => [
                "GRP A", "GRP B", "GRP C", "GRP D", "GRP E", "GRP F", "GRP G", "GRP H",
            ],
            2 => [
                "SND 1", "SND 2", "SND 3", "SND 4", "SND 5", "SND 6", "SND 7", "SND 8",
            ],
            3 => [
                "PRES <", "PRES >", "SND <", "SND >", "ECOUTE", "CHARGER", "METRO", "LOOP",
            ],
            _ => [
                "BAR 1", "BAR 2", "BAR 3", "BAR 4", "1 MES", "2 MES", "4 MES", "8 MES",
            ],
        }
    }

    /// Exécute l'action contextuelle d'une soft key d'écran (1..8).
    fn handle_soft_key(&mut self, n: u8) -> crate::Result<()> {
        let idx = (n.saturating_sub(1) as usize).min(7);
        if self.song_mode {
            return match idx {
                0 => self.handle_event(EngineEvent::PrevSection),
                1 => self.handle_event(EngineEvent::NextSection),
                2 => self.handle_event(EngineEvent::AddSection),
                3 => self.handle_event(EngineEvent::ToggleSongMode),
                4 => {
                    let scene = self.active_scene.unwrap_or(0).saturating_sub(1);
                    self.handle_event(EngineEvent::Scene(scene))
                }
                5 => {
                    let n_scenes = self.project.read().scenes.len();
                    let scene =
                        (self.active_scene.unwrap_or(0) + 1).min(n_scenes.saturating_sub(1));
                    self.handle_event(EngineEvent::Scene(scene))
                }
                6 => self.handle_event(EngineEvent::AddScene),
                _ => self.handle_event(EngineEvent::Scene(0)),
            };
        }
        if self.scene_mode {
            // Mode SCENES : les 8 soft keys sélectionnent les scènes 1..8.
            let n_scenes = self.project.read().scenes.len();
            let scene = idx.min(n_scenes.saturating_sub(1));
            return self.handle_event(EngineEvent::Scene(scene));
        }
        if self.controller_mode == 3 && self.left_tab == 0 {
            // Mode STEP : PAS <, PAS >, BAR <, BAR > puis longueurs 1/2/4/8.
            return match idx {
                0 => self.handle_event(EngineEvent::StepScroll(-1)),
                1 => self.handle_event(EngineEvent::StepScroll(1)),
                2 => self.handle_event(EngineEvent::StepScroll(-16)),
                3 => self.handle_event(EngineEvent::StepScroll(16)),
                4 => self.handle_event(EngineEvent::SetPatternBars(1)),
                5 => self.handle_event(EngineEvent::SetPatternBars(2)),
                6 => self.handle_event(EngineEvent::SetPatternBars(4)),
                _ => self.handle_event(EngineEvent::SetPatternBars(8)),
            };
        }
        match self.left_tab {
            1 => self.handle_event(EngineEvent::GroupFocus(idx as u8)),
            2 => self.handle_event(EngineEvent::SelectSound(idx as u8)),
            3 => match idx {
                0 => self.handle_event(EngineEvent::BrowsePresetStep(-1)),
                1 => self.handle_event(EngineEvent::BrowsePresetStep(1)),
                2 => {
                    let s = self.active_sound.saturating_sub(1);
                    self.handle_event(EngineEvent::SelectSound(s))
                }
                3 => {
                    let s = (self.active_sound + 1).min(15);
                    self.handle_event(EngineEvent::SelectSound(s))
                }
                4 => {
                    let preset = self.browser_preset_idx;
                    self.preview_preset(preset);
                    Ok(())
                }
                5 => {
                    let (g, s, p) = (self.group_focus, self.active_sound, self.browser_preset_idx);
                    self.handle_event(EngineEvent::LoadDrumPreset {
                        group: g,
                        sound: s,
                        preset_idx: p,
                    })
                }
                6 => self.handle_event(EngineEvent::ToggleMetronome),
                _ => self.handle_event(EngineEvent::ToggleLoop),
            },
            _ => match idx {
                0..=3 => self.handle_event(EngineEvent::SelectBar(idx)),
                4 => self.handle_event(EngineEvent::SetPatternBars(1)),
                5 => self.handle_event(EngineEvent::SetPatternBars(2)),
                6 => self.handle_event(EngineEvent::SetPatternBars(4)),
                _ => self.handle_event(EngineEvent::SetPatternBars(8)),
            },
        }
    }

    /// Master volume 0.0..1.0
    #[must_use]
    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Headphone volume 0.0..1.0
    #[must_use]
    pub fn headphone_volume(&self) -> f32 {
        self.headphone_volume
    }

    /// Compteur de révision : change dès qu'un état a bougé (UI/écrans).
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Transport mutable (pour UI).
    #[must_use]
    pub fn transport_mut(&mut self) -> &mut Transport {
        &mut self.transport
    }

    /// Sampler mutable (pour load samples).
    #[must_use]
    pub fn sampler_mut(&mut self) -> &mut Sampler {
        &mut self.sampler
    }

    /// Change swing.
    pub fn set_swing(&mut self, swing: f32) {
        self.transport.swing = swing.clamp(0.0, 1.0);
        self.revision = self.revision.wrapping_add(1);
        self.sequencer.swing = self.transport.swing;
        project_mut(&mut self.project.write()).swing = self.transport.swing;
    }

    /// Group focus actif.
    #[must_use]
    pub fn group_focus(&self) -> u8 {
        self.group_focus
    }

    /// Invalide l'affichage : les écrans se redessinent au prochain frame.
    /// À appeler après toute mutation du projet faite hors `handle_event`.
    pub fn invalidate_display(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    /// Fenêtre de ticks du pas sélectionné (début, fin).
    fn step_window(&self) -> (u32, u32) {
        let start = (self.step_cursor as u32) * 240;
        (start, start + 240)
    }

    /// Nombre total de pas du pattern actif (mesures x 16).
    fn pattern_total_steps(&self) -> usize {
        let proj = self.project.read();
        proj.active_pattern_index(self.group_focus as usize, self.active_scene)
            .and_then(|pi| proj.groups.get(self.group_focus as usize)?.patterns.get(pi))
            .map(|p| (p.length.bars as usize).max(1) * 16)
            .unwrap_or(32)
    }

    /// Exécute `f` sur le pattern actif du groupe focus (scène active).
    fn with_active_pattern<F: FnOnce(&mut crate::project::Pattern)>(&mut self, f: F) -> bool {
        let group_idx = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group_idx, self.active_scene);
        if let (Some(group), Some(pi)) = (proj.groups.get_mut(group_idx), pat_idx) {
            if let Some(pat) = group.patterns.get_mut(pi) {
                f(pat);
                return true;
            }
        }
        false
    }

    /// Bascule une note à un pas donné (tick) pour le pad donné dans le pattern actif.
    /// Façon Maschine : silencieux — poser un pas ne joue aucun son, l'aperçu
    /// se fait en mode Pad ou via les boutons ▶ de la GUI.
    /// La comparaison se fait par case de 16e (tolère humanize/nudge) et
    /// supprime tous les doublons de la case : un seul appui suffit toujours.
    pub fn toggle_step(&mut self, pad: u8, tick: u32) {
        // Le pas touché devient le pas sélectionné (les knobs l'éditent).
        self.selected_bar = (tick / (4 * 960)) as usize;
        self.step_cursor = (tick / 240) as usize;
        self.revision = self.revision.wrapping_add(1);
        let group_idx = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group_idx, self.active_scene);
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if group.patterns.is_empty() {
                group.patterns.push(crate::project::Pattern::new(
                    "Pattern 1",
                    crate::project::BarsBeatsTicks::new(2, 0, 0),
                ));
            }
            let idx = pat_idx.unwrap_or(0).min(group.patterns.len() - 1);
            if let Some(pat) = group.patterns.get_mut(idx) {
                let before = pat.events.len();
                pat.events
                    .retain(|e| !(e.pad == pad && crate::project::same_step_cell(e.tick, tick)));
                if pat.events.len() == before {
                    pat.events.push(crate::project::PatternEvent {
                        pad,
                        tick,
                        velocity: 115,
                        length_ticks: 120,
                        pitch: 36 + pad,
                    });
                    pat.events.sort_by_key(|e| e.tick);
                }
            }
        }
    }

    /// Remplit les temps 1, 2, 3, 4 pour un pad (ex: Kick 4-on-the-Floor)
    pub fn fill_four_on_the_floor(&mut self, pad: u8, bar: usize) {
        let group_idx = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group_idx, self.active_scene);
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    let bar_start = (bar * 4 * 960) as u32;
                    let bar_end = bar_start + 4 * 960;
                    pat.events
                        .retain(|e| !(e.pad == pad && e.tick >= bar_start && e.tick < bar_end));
                    for beat in 0..4 {
                        pat.events.push(crate::project::PatternEvent {
                            pad,
                            tick: bar_start + beat * 960,
                            velocity: 115,
                            length_ticks: 120,
                            pitch: 36 + pad,
                        });
                    }
                }
            }
        }
    }

    /// Remplit les croches (8ths) pour un pad (ex: Charlestons)
    pub fn fill_eighths(&mut self, pad: u8, bar: usize) {
        let group_idx = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group_idx, self.active_scene);
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    let bar_start = (bar * 4 * 960) as u32;
                    let bar_end = bar_start + 4 * 960;
                    pat.events
                        .retain(|e| !(e.pad == pad && e.tick >= bar_start && e.tick < bar_end));
                    for step in 0..8 {
                        pat.events.push(crate::project::PatternEvent {
                            pad,
                            tick: bar_start + step * 480,
                            velocity: if step % 2 == 0 { 95 } else { 75 },
                            length_ticks: 120,
                            pitch: 36 + pad,
                        });
                    }
                }
            }
        }
    }

    /// Supprime toutes les notes d'une mesure donnée
    pub fn clear_bar(&mut self, bar: usize) {
        self.revision = self.revision.wrapping_add(1);
        let group_idx = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group_idx, self.active_scene);
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    let bar_start = (bar * 4 * 960) as u32;
                    let bar_end = bar_start + 4 * 960;
                    pat.events
                        .retain(|e| !(e.tick >= bar_start && e.tick < bar_end));
                }
            }
        }
    }

    /// Réinitialise le pattern avec le groove 808 complet
    pub fn reset_demo_groove(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        let demo_proj = crate::project::Project::new_demo();
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        if let Some(demo_g) = demo_proj.groups.first() {
            if let Some(demo_p) = demo_g.patterns.first() {
                if let Some(g) = proj.groups.get_mut(0) {
                    if !g.patterns.is_empty() {
                        g.patterns[0] = demo_p.clone();
                    } else {
                        g.patterns.push(demo_p.clone());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_step_is_silent_and_single_press() {
        let mut eng = Engine::new_demo(48000);
        assert_eq!(eng.sampler.num_voices(), 0);
        // Case vide (pad 15 absent du groove démo) : pose le pas.
        eng.toggle_step(15, 0);
        // Silencieux : aucune voix créée (façon Maschine : pas d'aperçu).
        assert_eq!(eng.sampler.num_voices(), 0);
        {
            let guard = eng.project_handle();
            let proj = guard.read();
            let n = proj.groups[0].patterns[0]
                .events
                .iter()
                .filter(|e| e.pad == 15 && e.tick == 0)
                .count();
            assert_eq!(n, 1);
        }
        // Second appui : retire en une fois (pas de doublon, pas de 3e appui).
        eng.toggle_step(15, 0);
        {
            let guard = eng.project_handle();
            let proj = guard.read();
            assert!(proj.groups[0].patterns[0]
                .events
                .iter()
                .all(|e| e.pad != 15));
        }
        assert_eq!(eng.sampler.num_voices(), 0);
    }

    #[test]
    fn step_mode_edits_selected_step_and_scrolls_grid() {
        let mut eng = Engine::new_demo(48000);
        eng.clear_bar(0);
        eng.clear_bar(1);
        // Sélectionne le son 3 puis pose un pas sur son pas 0.
        eng.handle_event(EngineEvent::SelectSound(3)).unwrap();
        eng.toggle_step(3, 0);
        // Le pas touché devient le pas sélectionné (curseur + mesure).
        assert_eq!(eng.step_cursor, 0);
        assert_eq!(eng.selected_bar, 0);

        // Édition par les knobs 1-4 : VEL / POS / LEN / PITCH du pas sélectionné.
        eng.handle_event(EngineEvent::StepVelocityDelta(-15))
            .unwrap();
        eng.handle_event(EngineEvent::StepNudgeTicks(20)).unwrap();
        eng.handle_event(EngineEvent::StepLengthDelta(1)).unwrap();
        eng.handle_event(EngineEvent::StepPitchDelta(2)).unwrap();
        {
            let guard = eng.project_handle();
            let proj = guard.read();
            let ev = proj.groups[0].patterns[0]
                .events
                .iter()
                .find(|e| e.pad == 3 && e.tick < 240)
                .expect("le pas posé existe toujours");
            assert_eq!(ev.velocity, 100); // 115 - 15
            assert_eq!(ev.tick, 20); // nudge +20 ticks
            assert_eq!(ev.length_ticks, 360); // 120 + 240 (1 pas de 1/16)
            assert_eq!(ev.pitch, 41); // 36 + pad(3) + 2
        }

        // Scroll de la grille complète : le curseur avance, la mesure suit.
        eng.handle_event(EngineEvent::StepScroll(20)).unwrap();
        assert_eq!(eng.step_cursor, 20);
        assert_eq!(eng.selected_bar, 1);
        // Bornes : début et fin du pattern (2 mesures = 32 pas).
        eng.handle_event(EngineEvent::StepScroll(-100)).unwrap();
        assert_eq!(eng.step_cursor, 0);
        assert_eq!(eng.selected_bar, 0);
        eng.handle_event(EngineEvent::StepScroll(999)).unwrap();
        assert_eq!(eng.step_cursor, 31);
        assert_eq!(eng.selected_bar, 1);

        // Le curseur est exposé aux écrans / aux LEDs.
        assert_eq!(eng.hardware_snapshot().step_cursor, 31);

        // Pas vide : pas d'édition fantôme, pas de panic.
        eng.handle_event(EngineEvent::StepVelocityDelta(5)).unwrap();
        eng.handle_event(EngineEvent::StepPitchDelta(1)).unwrap();
    }

    #[test]
    fn swing_knob_goes_through_engine_and_syncs_sequencer() {
        let mut eng = Engine::new_demo(48000);
        let before = eng.revision;
        eng.handle_event(EngineEvent::Swing(0.25)).unwrap();
        assert!((eng.transport.swing - 0.25).abs() < 1e-6);
        assert!((eng.sequencer.swing - 0.25).abs() < 1e-6);
        assert!((eng.project.read().swing - 0.25).abs() < 1e-6);
        assert_ne!(eng.revision, before, "l'affichage doit être invalidé");
    }

    #[test]
    fn step_cursor_soft_keys_scroll_and_set_length() {
        let mut eng = Engine::new_demo(48000);
        eng.controller_mode = 3;
        eng.left_tab = 0;
        eng.handle_event(EngineEvent::SoftKey(2)).unwrap(); // PAS >
        assert_eq!(eng.step_cursor, 1);
        eng.handle_event(EngineEvent::SoftKey(4)).unwrap(); // BAR >
        assert_eq!(eng.step_cursor, 17);
        assert_eq!(eng.selected_bar, 1);
        eng.handle_event(EngineEvent::SoftKey(7)).unwrap(); // 4 MES
        assert_eq!(eng.hardware_snapshot().pattern_bars, 4);
    }

    #[test]
    fn toggle_step_removes_humanized_note_in_one_press() {
        let mut eng = Engine::new_demo(48000);
        eng.clear_bar(0);
        eng.clear_bar(1);
        eng.toggle_step(0, 0);
        // Humanize décale de ±12 ticks : l'ancienne comparaison exacte
        // `e.tick == tick` ratait la note et forçait des appuis multiples.
        eng.humanize_active_pattern();
        eng.toggle_step(0, 0);
        let guard = eng.project_handle();
        let proj = guard.read();
        assert!(
            proj.groups[0].patterns[0]
                .events
                .iter()
                .all(|e| !(e.pad == 0 && crate::project::same_step_cell(e.tick, 0))),
            "la note humanisée aurait dû être retirée en un seul appui"
        );
    }

    #[test]
    fn toggle_step_edits_scene_assigned_pattern() {
        let mut eng = Engine::new_demo(48000);
        // 2e pattern vide assigné à la scène courante pour le groupe 0.
        {
            let h = eng.project_handle();
            let mut proj = h.write();
            proj.groups[0].patterns.push(crate::project::Pattern::new(
                "Vide",
                crate::project::BarsBeatsTicks::new(2, 0, 0),
            ));
            proj.scenes[0].patterns[0] = Some(1);
        }
        eng.toggle_step(15, 0);
        let guard = eng.project_handle();
        let proj = guard.read();
        assert!(
            proj.groups[0].patterns[0]
                .events
                .iter()
                .all(|e| e.pad != 15),
            "le pattern 0 (non joué) ne doit pas être modifié"
        );
        assert_eq!(proj.groups[0].patterns[1].events.len(), 1);
    }

    #[test]
    fn engine_pad_triggers_sampler() {
        let mut eng = Engine::new_demo(48000);
        // ajoute un sample dummy au sound 0
        let samp = std::sync::Arc::new(vec![0.5; 4800]);
        eng.sampler_mut().add_zone(
            0,
            crate::sampler::ZoneMapping {
                sample: Some(samp),
                sample_path: None,
                sample_rate: 48000,
                root_key: 36,
                lokey: 0,
                hikey: 127,
                lovel: 0,
                hivel: 127,
                volume_db: 0.0,
                pan: 0.0,
                tune_cents: 0,
                choke_group: None,
                ..Default::default()
            },
        );
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 100,
            pressed: true,
        })
        .unwrap();
        assert_eq!(eng.sampler.num_voices(), 1);
    }

    #[test]
    fn engine_transport() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: false,
            stop: false,
        })
        .unwrap();
        assert!(eng.transport.is_playing());
        eng.advance(4800); // 100ms
        assert!(eng.transport.tick > 0);
    }

    #[test]
    fn metronome_click_audible() {
        // Projet vide : seul le métronome doit produire du son.
        let mut eng = Engine::new(Project::default(), 48000);
        eng.handle_event(EngineEvent::ToggleMetronome).unwrap();
        assert!(eng.transport.metronome);
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: false,
            stop: false,
        })
        .unwrap();
        // Avance d'un temps (0.5s à 120BPM) : au moins un clic.
        eng.advance(24000);
        let mut buf = vec![0.0f32; 1024 * 2];
        eng.render_audio(&mut buf);
        let mx = buf.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(mx > 1e-4, "metronome silencieux, max={mx}");
    }

    #[test]
    fn sound_filter_adsr_audible() {
        let mut eng = Engine::new_demo(48000);
        // Coupe franche : hi-hat fermé à 200Hz doit perdre ses aigus.
        eng.handle_event(EngineEvent::SoundFilterCutoff {
            group: 0,
            sound: 2,
            cutoff_hz: 200.0,
        })
        .unwrap();
        eng.handle_event(EngineEvent::Pad {
            pad: 2,
            velocity: 127,
            pressed: true,
        })
        .unwrap();
        let mut dark = vec![0.0f32; 2048 * 2];
        eng.render_audio(&mut dark);
        // Rouvre le filtre et rejoue.
        eng.handle_event(EngineEvent::SoundFilterCutoff {
            group: 0,
            sound: 2,
            cutoff_hz: 20000.0,
        })
        .unwrap();
        eng.handle_event(EngineEvent::Pad {
            pad: 2,
            velocity: 127,
            pressed: true,
        })
        .unwrap();
        let mut open = vec![0.0f32; 2048 * 2];
        eng.render_audio(&mut open);
        // Le chapeau contient des aigus : sans filtre, plus d'énergie HF.
        // Approximation : compte les passages par zéro (richesse HF).
        fn zc(buf: &[f32]) -> usize {
            buf.iter()
                .skip(2)
                .step_by(2)
                .zip(buf.iter().skip(4).step_by(2))
                .filter(|(a, b)| (**a <= 0.0) != (**b <= 0.0))
                .count()
        }
        let (e_dark, e_open): (f32, f32) = (
            dark.iter().map(|x| x.abs()).sum(),
            open.iter().map(|x| x.abs()).sum(),
        );
        assert!(e_dark > 1e-4 && e_open > 1e-4, "rendu silencieux");
        assert!(
            zc(&open) > zc(&dark),
            "le cutoff ne filtre rien (zc open={} dark={})",
            zc(&open),
            zc(&dark)
        );
        let _ = e_dark;
    }

    #[test]
    fn bar_navigation_and_pattern_length() {
        let mut eng = Engine::new_demo(48000);
        let snap = eng.snapshot();
        assert_eq!(snap.selected_bar, 0);
        assert_eq!(snap.pattern_bars, 2);

        // Sélection directe mesure 1
        eng.handle_event(EngineEvent::SelectBar(1)).unwrap();
        assert_eq!(eng.snapshot().selected_bar, 1);

        // NextBar / PrevBar
        eng.handle_event(EngineEvent::NextBar).unwrap();
        // Limité par pattern_bars (2) -> max bar index est 1
        assert_eq!(eng.snapshot().selected_bar, 1);

        eng.handle_event(EngineEvent::PrevBar).unwrap();
        assert_eq!(eng.snapshot().selected_bar, 0);

        // Étend le pattern à 4 mesures
        eng.handle_event(EngineEvent::SetPatternBars(4)).unwrap();
        assert_eq!(eng.snapshot().pattern_bars, 4);

        // Maintenant NextBar peut aller jusqu'à mesure 3 (0-indexed)
        eng.handle_event(EngineEvent::SelectBar(3)).unwrap();
        assert_eq!(eng.snapshot().selected_bar, 3);
    }

    #[test]
    fn section_workflow_duplicate_resize_delete() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::AddSection).unwrap();
        eng.handle_event(EngineEvent::AddSection).unwrap();
        let n = eng.snapshot().project.song.len();
        assert!(n >= 2);

        // Duplique la section active (insérée juste après, sélectionnée).
        eng.handle_event(EngineEvent::DuplicateSection).unwrap();
        assert_eq!(eng.snapshot().project.song.len(), n + 1);

        // Redimensionne la section active.
        eng.handle_event(EngineEvent::ResizeSection { bars: 4 })
            .unwrap();
        let snap = eng.snapshot();
        let idx = snap.active_section.unwrap();
        assert_eq!(snap.project.song[idx].length.bars, 4);

        // Supprime la section active.
        eng.handle_event(EngineEvent::DeleteSection).unwrap();
        assert_eq!(eng.snapshot().project.song.len(), n);

        // Supprime toutes les sections : plus de section active, pas de panic.
        while !eng.snapshot().project.song.is_empty() {
            eng.handle_event(EngineEvent::DeleteSection).unwrap();
        }
        assert_eq!(eng.snapshot().active_section, None);
    }

    #[test]
    fn scene_workflow_duplicate_delete_reindexes_sections() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::AddScene).unwrap(); // scène 2 active
        eng.handle_event(EngineEvent::AddSection).unwrap(); // section sur scène 2
        let scenes = eng.snapshot().project.scenes.len();
        assert!(scenes >= 2);

        // Duplique la scène active.
        eng.handle_event(EngineEvent::DuplicateScene).unwrap();
        assert_eq!(eng.snapshot().project.scenes.len(), scenes + 1);

        // Supprime la scène active (la copie) : les sections de la scène 2
        // restent, aucun index ne pointe hors limites.
        eng.handle_event(EngineEvent::DeleteScene).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.project.scenes.len(), scenes);
        for sec in &snap.project.song {
            assert!(sec.scene < snap.project.scenes.len());
        }

        // Impossible de supprimer la dernière scène.
        while eng.snapshot().project.scenes.len() > 1 {
            eng.handle_event(EngineEvent::DeleteScene).unwrap();
        }
        eng.handle_event(EngineEvent::DeleteScene).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.project.scenes.len(), 1);
        for sec in &snap.project.song {
            assert!(sec.scene == 0);
        }
        assert_eq!(snap.active_scene, Some(0));
    }
}
