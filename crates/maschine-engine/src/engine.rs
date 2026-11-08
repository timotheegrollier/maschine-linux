//! Engine — assemblage Project + Transport + Sampler + Sequencer.
//!
//! Thread-safe snapshot pour UI via `parking_lot::RwLock` + `Arc`.

use std::sync::Arc;

use parking_lot::RwLock;

use crate::project::{BarsBeatsTicks, Pattern, PatternEvent, Project, Scene, Section, Zone};
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
    /// Sauvegarde le projet courant (rapide=JSON compact, sinon pretty).
    SaveCurrent { quick: bool },
    /// Crée un nouveau projet (nom) et le sauvegarde (comme un premier Save).
    NewProject { name: String },
    /// Définit le chemin du projet courant.
    SetProjectPath(Option<String>),
    /// Load
    Load(std::path::PathBuf),
    /// Master volume 0.0..1.0
    MasterVolume(f32),
    /// Headphone volume 0.0..1.0
    HeadphoneVolume(f32),
    /// Toggle mute on group
    Mute(u8),
    /// Bascule le mute d'un pad/son dans un groupe (mode MUTE Maschine).
    TogglePadMute { group: u8, pad: u8 },
    /// Toggle solo on group
    Solo(u8),
    /// Select pattern in group
    PatternSelect {
        /// Group idx
        group: u8,
        /// Pattern idx
        pattern: usize,
    },
    /// Crée un pattern vide à la suite du groupe et l'assigne à la scène active.
    AddPattern {
        /// Group idx
        group: u8,
    },
    /// Crée (si nécessaire) le pattern d'index donné, puis le sélectionne.
    /// Utilisé par les pads vides du mode PATTERN : chaque pad correspond à
    /// un slot de pattern, comme sur Maschine.
    CreatePatternAt {
        /// Group idx
        group: u8,
        /// Slot de pattern (0..)
        pattern: usize,
    },
    /// Duplique un pattern et insère la copie juste après.
    DuplicatePattern {
        /// Group idx
        group: u8,
        /// Pattern source
        pattern: usize,
    },
    /// Supprime un pattern (le dernier pattern du groupe est conservé).
    DeletePattern {
        /// Group idx
        group: u8,
        /// Pattern à supprimer
        pattern: usize,
    },
    /// Efface les événements d'un pattern sans changer sa longueur.
    ClearPattern {
        /// Group idx
        group: u8,
        /// Pattern à vider
        pattern: usize,
    },
    /// Renomme un pattern.
    RenamePattern {
        /// Group idx
        group: u8,
        /// Pattern idx
        pattern: usize,
        /// Nouveau nom
        name: String,
    },
    /// Sélectionne le pattern précédent/suivant dans le groupe focus.
    StepPattern {
        /// Group idx
        group: u8,
        /// -1 = précédent, +1 = suivant
        delta: i32,
    },
    /// Change Left display tab (0..4: Arrange, Mixer, Sampler, Browser, Synth)
    SetLeftTab(u8),
    /// Change Right display tab (0..3)
    SetRightTab(u8),
    /// Demande de navigation GUI (0: Pattern, 1: Step, 2: Keyboard,
    /// 3: Scene, 4: Plugin). Le matériel reste la source de vérité : la GUI
    /// suit ce compteur pour changer de fenêtre au bon moment.
    NavigateView(u8),
    /// Page de l'éditeur PLUGIN (0: Sound, 1: TB-303)
    SetPluginPage(u8),
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
    /// Type de filtre d'un Sound (Off / LowPass / HighPass / BandPass)
    SoundFilterKind {
        group: u8,
        sound: u8,
        kind: crate::project::FilterKind,
    },
    /// Polyphonie d'un Sound (1..32)
    SoundPolyphony { group: u8, sound: u8, polyphony: u8 },
    /// Groupe de choke d'un Sound (None = aucun)
    SoundChoke {
        group: u8,
        sound: u8,
        choke_group: Option<u8>,
    },
    /// Copie les réglages du Sound source vers tous les autres Sounds du Group.
    SoundApplyToGroup { group: u8, source_sound: u8 },
    /// Réinitialise les réglages de mixage/filtre/enveloppe du Sound.
    SoundReset { group: u8, sound: u8 },
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
    /// Déclenche le son actif à une note MIDI (modes Keyboard/Chords).
    KeyboardTrigger {
        /// Note MIDI 0..127
        note: u8,
        /// Vélocité 0..127
        velocity: u8,
        /// Appui / relâchement
        pressed: bool,
    },
    /// Change la fondamentale Keyboard/Chords.
    KeyboardRootDelta(i32),
    /// Change la gamme Keyboard/Chords.
    KeyboardScaleDelta(i32),
    /// Change l'octave de base Keyboard/Chords.
    KeyboardOctaveDelta(i32),
    /// Change le type d'accord (mode Chords).
    KeyboardChordDelta(i32),
    /// Change l'inversion d'accord (mode Chords).
    KeyboardInversionDelta(i32),
    /// Remplace l'état musical Keyboard/Chords.
    SetKeyboardState(crate::keyboard::KeyboardState),
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
    /// Installe un WAV déjà décodé hors du verrou moteur/audio.
    LoadPreparedSample {
        /// Group 0..7
        group: u8,
        /// Sound 0..15
        sound: u8,
        /// Sample et métadonnées pré-calculées
        zone: crate::sampler::ZoneMapping,
    },
    /// Défilement preset dans le Browser (+/- step)
    BrowsePresetStep(i32),
    /// Type de navigateur sur l'onglet BROWSER (0: presets 808, 1: projets FILE).
    SetBrowserKind(u8),
    /// Entrées du navigateur de projets (fournies par l'application).
    SetProjectEntries(Vec<ProjectFileEntry>),
    /// Défilement du navigateur courant.
    BrowseBrowserStep(i32),
    /// Charge l'élément sélectionné du navigateur courant.
    LoadSelectedBrowserItem { group: u8, sound: u8 },
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
    /// Ajustement relatif du cutoff de l'Acid Synth (delta log / octaves)
    AcidCutoffDelta(f32),
    /// Ajustement relatif de la résonance de l'Acid Synth (0..1)
    AcidResoDelta(f32),
    /// Ajustement relatif de la modulation d'enveloppe de l'Acid Synth (0..1)
    AcidEnvModDelta(f32),
    /// Ajustement relatif du déclin d'enveloppe de l'Acid Synth (ms)
    AcidDecayDelta(f32),
    /// Ajustement relatif de l'accent de l'Acid Synth (0..1)
    AcidAccentDelta(f32),
    /// Ajustement relatif du temps de slide de l'Acid Synth (ms)
    AcidSlideDelta(f32),
    /// Ajustement relatif de la distorsion de l'Acid Synth (0..1)
    AcidDriveDelta(f32),
    /// Ajustement relatif de la tonalité de distorsion de l'Acid Synth (Hz)
    AcidDriveToneDelta(f32),
    /// Ajustement relatif du mixage de distorsion de l'Acid Synth (0..1)
    AcidDriveMixDelta(f32),
    /// Ajustement relatif du temps de delay de l'Acid Synth (ms)
    AcidDelayTimeDelta(f32),
    /// Ajustement relatif du feedback de delay de l'Acid Synth (0..0.95)
    AcidDelayFeedbackDelta(f32),
    /// Ajustement relatif du mixage de delay de l'Acid Synth (0..1)
    AcidDelayMixDelta(f32),
    /// Bascule la forme d'onde (Saw <-> Square)
    AcidWaveformToggle,
    /// Décalage d'octave relatif (+1 / -1)
    AcidOctaveDelta(i8),
    /// Sélection d'un preset Acid (0..3)
    AcidSelectPreset(u8),
    /// Remplacement complet des paramètres Acid
    AcidSetParams(crate::acid_synth::AcidParams),
    /// Alterne le type de filtre d'un Sound (Off -> LowPass -> HighPass -> BandPass -> Off)
    CycleSoundFilterKind { group: u8, sound: u8 },
    /// Définit directement le type de filtre d'un Sound
    SetSoundFilterKind {
        group: u8,
        sound: u8,
        kind: crate::project::FilterKind,
    },
    /// Remplacement complet des paramètres Wavetable / Nexus Synth
    WavetableSetParams(crate::wavetable_synth::WavetableParams),
    /// Sélection d'un preset Wavetable par index
    WavetableSetPreset(usize),
    /// Preset Wavetable suivant
    WavetableNextPreset,
    /// Preset Wavetable précédent
    WavetablePrevPreset,
    /// Ajuste le type de distorsion du Kick Synth
    KickDriveTypeDelta(i32),
    /// Ajuste le mode de filtre du Kick Synth
    KickFilterModeDelta(i32),
    /// Ajuste la fréquence de coupure du filtre du Kick Synth
    KickFilterCutoffDelta(f32),
    /// Ajuste la résonance du filtre du Kick Synth
    KickFilterResoDelta(f32),
    /// Supprime définitivement le projet sélectionné dans le navigateur
    DeleteSelectedProject,
    /// Annule la confirmation de suppression de projet
    CancelDeleteProject,
    /// Définit le mode mixer (0: Pads 1-8, 1: Pads 9-16, 2: Groupes A-H, 3: Master)
    SetMixerMode(u8),
    /// Remplacement complet des paramètres Kick Synth
    KickSetParams(crate::kick_synth::KickParams),
    /// Sélectionne un preset de Kick Synth (0..5)
    KickSetPreset(u8),
    /// Charge un plugin sur un Sound
    LoadPluginToSound {
        group: u8,
        sound: u8,
        plugin_idx: u8,
    },
    /// Ajoute un effet à un Sound
    AddFxToSound {
        group: u8,
        sound: u8,
        fx_type: crate::fx::FxType,
    },
    /// Ajoute un effet au bus d'un Group
    AddFxToGroup {
        group: u8,
        fx_type: crate::fx::FxType,
    },
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

/// Entrée du navigateur de projets (FILE), affichable sur la MK3 et la GUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFileEntry {
    /// Nom affiché (souvent le nom du fichier sans extension).
    pub name: String,
    /// Chemin absolu du fichier projet.
    pub path: String,
    /// Métadonnée courte (taille/date) pour les écrans.
    pub meta: String,
}

/// Clé stable de runtime pour un Sound : le même pad d'un autre Group est un
/// instrument distinct, comme dans le modèle Maschine.
const fn sampler_sound_id(group: u8, sound: u8) -> crate::sampler::SoundId {
    crate::sampler::sound_id(group, sound)
}

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
    /// Pads/sons coupés par groupe (bit 0..15 = pad 1..16).
    pub muted_sounds: [u16; 8],
    /// Soloed groups bitmask
    pub soloed_groups: u8,
    /// Tab écran gauche actif (0: Arrange, 1: Mixer, 2: Sampler, 3: Browser)
    pub left_tab: u8,
    /// Tab écran droit actif (0: Pattern, 1: Events, 2: Macro, 3: Settings)
    pub right_tab: u8,
    /// Dernière destination de navigation demandée par le hardware
    /// (0: Pattern, 1: Step, 2: Keyboard, 3: Scene, 4: Plugin).
    pub nav_view: u8,
    /// Numéro de la dernière demande de navigation : la GUI ne réagit qu'aux
    /// changements de ce compteur, pas à chaque snapshot.
    pub nav_serial: u64,
    /// Page de l'éditeur PLUGIN (0: Sound, 1: TB-303).
    pub plugin_page: u8,
    /// Son actif sélectionné (0..15)
    pub active_sound: u8,
    /// Mode Song (arranger) vs Ideas (scènes)
    pub song_mode: bool,
    /// Section active dans le Song
    pub active_section: Option<usize>,
    /// Pads flashés par le séquenceur (lecture pattern) — pour LEDs + GUI
    pub triggered_pads: [bool; 16],
    /// Activité récente des pads (1.0 → 0.0) — animation des écrans
    pub pad_activity: [f32; 16],
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
    /// État musical Keyboard/Chords (gamme, fondamentale, octave, accords).
    pub keyboard: crate::keyboard::KeyboardState,
    /// Index preset sélectionné dans le browser (0..15)
    pub browser_preset_idx: u8,
    /// Type de navigateur (0: presets 808, 1: projets FILE).
    pub browser_kind: u8,
    /// Entrées du navigateur de projets.
    pub project_entries: Vec<ProjectFileEntry>,
    /// Entrée projet sélectionnée.
    pub project_selected: usize,
    /// Chemin du projet courant.
    pub current_project_path: Option<String>,
    /// Message de statut temporaire (sauvegarde, chargement...).
    pub status_message: Option<String>,
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
    /// VU-mètres lissés par pad (0..15) pour le groupe actif
    pub pad_levels: [f32; 16],
    /// Peak hold par pad (0..15) pour le groupe actif
    pub pad_peaks: [f32; 16],
    /// VU master lissé (post volume master)
    pub master_level: f32,
    /// Peak hold master
    pub master_peak: f32,
    /// Labels des 8 soft keys (boutons sous les écrans — contexte courant).
    pub soft_labels: [&'static str; 8],
    /// Enveloppe (min,max) du sample du Sound actif — waveform des écrans.
    pub waveform: Option<Arc<[(f32, f32)]>>,
    /// Durée du sample du Sound actif (secondes).
    pub sample_duration: f32,
    /// Paramètres actuels du synthétiseur TB-303 Acid.
    pub acid_params: crate::acid_synth::AcidParams,
    /// Paramètres actuels de modélisation de Kick.
    pub kick_params: crate::kick_synth::KickParams,
    /// Paramètres actuels du synthétiseur Wavetable / Nexus.
    pub wavetable_params: crate::wavetable_synth::WavetableParams,
    /// Le son actif possède-t-il un Kick Synth ?
    pub focus_sound_kick: Option<crate::kick_synth::KickParams>,
    /// Le son actif possède-t-il un TB-303 ?
    pub focus_sound_acid: Option<crate::acid_synth::AcidParams>,
    /// Le son actif possède-t-il un Wavetable Synth ?
    pub focus_sound_wavetable: Option<crate::wavetable_synth::WavetableParams>,
    /// Chaîne d'effets du Sound actif.
    pub focus_sound_fx: Vec<crate::fx::FxSlot>,
    /// Chaîne d'effets du Group focus.
    pub focus_group_fx: Vec<crate::fx::FxSlot>,
    /// Mode mixer (0: Pads 1-8, 1: Pads 9-16, 2: Groupes A-H, 3: Master).
    pub mixer_mode: u8,
    /// Index plugin sélectionné dans le browser (0..3).
    pub browser_plugin_idx: u8,
    /// Index effet sélectionné dans le browser (0..4).
    pub browser_fx_idx: u8,
    /// Confirmation de suppression de projet active sur la MK3.
    pub delete_project_confirm: bool,
    /// Compteur de révision : change dès qu'un état a bougé (repaint UI/écrans).
    pub revision: u64,
}

// ── Constantes moteur ───────────────────────────────────────────────────

/// Décroissance de l'activité des pads (u/s) : ~170 ms de 1.0 à 0.0.
const PAD_ACTIVITY_DECAY: f32 = 6.0;

/// Petit instantané utilisé par les LED MK3 à haute fréquence.
/// Il évite de cloner tout le projet à chaque rafraîchissement matériel.
#[derive(Debug, Clone)]
pub struct HardwareSnapshot {
    pub group_focus: u8,
    pub active_sound: u8,
    pub left_tab: u8,
    pub right_tab: u8,
    /// Type de navigateur (0: presets 808, 1: projets FILE, 2: plugins, 3: effets)
    pub browser_kind: u8,
    /// Mode mixer (0: Pads 1-8, 1: Pads 9-16, 2: Groupes A-H, 3: Master)
    pub mixer_mode: u8,
    /// Le son actif a-t-il un Kick Synth ?
    pub has_kick_synth: bool,
    /// Le son actif a-t-il un TB-303 Acid ?
    pub has_acid_synth: bool,
    /// Le son actif a-t-il un Wavetable / Nexus Synth ?
    pub has_wavetable_synth: bool,
    /// Confirmation de suppression de projet active
    pub delete_project_confirm: bool,
    /// Page PLUGIN (0: Sound, 1: TB-303) — routage des knobs hardware.
    pub plugin_page: u8,
    pub controller_mode: u8,
    pub keyboard: crate::keyboard::KeyboardState,
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
    pub muted_sounds: [u16; 8],
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
    /// Synthétiseur analogique virtuel TB-303 Acid
    pub acid_synth: crate::acid_synth::AcidSynth,
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
    /// Pads/sons coupés par groupe (bit 0..15 = pad 1..16).
    muted_sounds: [u16; 8],
    /// Soloed groups bitmask
    soloed_groups: u8,
    /// Tab écran gauche
    pub left_tab: u8,
    /// Tab écran droit
    pub right_tab: u8,
    /// Navigation GUI demandée par le hardware (voir `EngineEvent::NavigateView`)
    pub nav_view: u8,
    /// Compteur monotone des demandes de navigation GUI
    nav_serial: u64,
    /// Page de l'éditeur PLUGIN (0: Sound, 1: TB-303)
    pub plugin_page: u8,
    /// Son actif
    pub active_sound: u8,
    /// Mesure sélectionnée pour l'édition et l'affichage (0..pattern_bars-1)
    pub selected_bar: usize,
    /// Curseur du step sequencer : pas sélectionné (index absolu dans le
    /// pattern, 0..pattern_bars*16-1) — les knobs éditent ce pas.
    pub step_cursor: usize,
    /// Index preset sélectionné dans le Browser
    pub browser_preset_idx: u8,
    /// Type de navigateur sur l'onglet BROWSER/FILE (0: presets 808, 1: projets, 2: plugins, 3: effets).
    pub browser_kind: u8,
    /// Mode mixer (0: Pads 1-8, 1: Pads 9-16, 2: Groupes A-H, 3: Master).
    pub mixer_mode: u8,
    /// Confirmation suppression de projet active.
    pub delete_project_confirm: bool,
    /// Index plugin sélectionné dans le browser (0..3).
    pub browser_plugin_idx: u8,
    /// Index effet sélectionné dans le browser (0..4).
    pub browser_fx_idx: u8,
    /// Moteur de modélisation de Kick Techno.
    pub kick_synth: crate::kick_synth::KickSynth,
    /// Synthétiseur polyphonique Wavetable / Nexus.
    pub wavetable_synth: crate::wavetable_synth::WavetableSynth,
    /// Processeur d'effets temps réel.
    pub fx_processor: crate::fx::FxChainProcessor,
    /// Entrées du navigateur de projets.
    pub project_entries: Vec<ProjectFileEntry>,
    /// Entrée projet sélectionnée.
    pub project_selected: usize,
    /// Chemin du projet courant (sauvegarde rapide / FILE SAVE).
    pub current_project_path: Option<String>,
    /// Message de statut temporaire (sauvegarde, chargement...) affiché sur MK3/GUI.
    status_message: Option<String>,
    status_expires_at: std::time::Instant,
    /// Mode Song (arranger) vs Ideas (scènes)
    pub song_mode: bool,
    /// Section active dans le Song
    pub active_section: Option<usize>,
    /// Flash pads jusqu'à ce tick (séquenceur) — 16 pads
    pad_flash_until: [u64; 16],
    /// Activité récente des pads (1.0 → 0.0, décroit) — animation écrans
    pad_activity: [f32; 16],
    /// Modes workflow (miroirs du mapper pour les écrans)
    pub scene_mode: bool,
    pub pattern_mode: bool,
    pub select_mode: bool,
    pub erase_mode: bool,
    /// Mode de jeu 0..3 (Pad/Keyboard/Chords/Step)
    pub controller_mode: u8,
    /// État musical Keyboard/Chords.
    pub keyboard: crate::keyboard::KeyboardState,
    /// Clic métronome accentué (début de mesure)
    click_accent: std::sync::Arc<Vec<f32>>,
    /// Clic métronome normal (temps)
    click_normal: std::sync::Arc<Vec<f32>>,
    /// VU-mètres : niveau lissé par groupe (0..8) + peak hold
    group_meter: [f32; 8],
    /// Peak hold par groupe
    group_peak_hold: [f32; 8],
    /// VU-mètres : niveau lissé par pad (0..15) par groupe (0..8)
    pad_meter: [[f32; 16]; 8],
    /// Peak hold par pad (0..15) par groupe (0..8)
    pad_peak_hold: [[f32; 16]; 8],
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

/// Garantit qu'une scène active existe et que son tableau de patterns couvre
/// tous les groupes. Retourne l'index de la scène active.
fn ensure_active_scene(project: &mut Project, active_scene: &mut Option<usize>) -> usize {
    if project.scenes.is_empty() {
        project
            .scenes
            .push(Scene::new("Scene 1", project.groups.len()));
    }
    let scene_idx = active_scene
        .unwrap_or(0)
        .min(project.scenes.len().saturating_sub(1));
    *active_scene = Some(scene_idx);
    let num_groups = project.groups.len();
    if let Some(scene) = project.scenes.get_mut(scene_idx) {
        if scene.patterns.len() < num_groups {
            scene.patterns.resize(num_groups, None);
        }
    }
    scene_idx
}

/// Assigne un pattern du groupe à la scène active (créée au besoin).
fn assign_pattern_to_active_scene(
    project: &mut Project,
    active_scene: &mut Option<usize>,
    group: usize,
    pattern: usize,
) {
    let scene_idx = ensure_active_scene(project, active_scene);
    if let Some(scene) = project.scenes.get_mut(scene_idx) {
        if let Some(slot) = scene.patterns.get_mut(group) {
            *slot = Some(pattern);
        }
    }
}

/// Longueur par défaut d'un nouveau pattern : celle du pattern actif, sinon
/// 2 mesures (valeur historique de la démo).
fn new_pattern_length(
    project: &Project,
    group: usize,
    active_scene: Option<usize>,
) -> BarsBeatsTicks {
    project
        .active_pattern_index(group, active_scene)
        .and_then(|pi| project.groups.get(group)?.patterns.get(pi))
        .map(|p| p.length)
        .unwrap_or_else(|| BarsBeatsTicks::new(2, 0, 0))
}

/// Réindexe les références de scènes après insertion d'un pattern.
fn shift_scene_refs_after_insert(project: &mut Project, group: usize, inserted: usize) {
    for scene in &mut project.scenes {
        if let Some(slot) = scene.patterns.get_mut(group) {
            if let Some(pi) = *slot {
                if pi >= inserted {
                    *slot = Some(pi + 1);
                }
            }
        }
    }
}

/// Réindexe les références de scènes après suppression d'un pattern. Les
/// scènes qui utilisaient le pattern supprimé basculent sur le pattern voisin
/// (jamais hors limites), afin de rester jouables.
fn shift_scene_refs_after_remove(
    project: &mut Project,
    group: usize,
    removed: usize,
    new_len: usize,
) {
    let fallback = removed.min(new_len.saturating_sub(1));
    for scene in &mut project.scenes {
        if let Some(slot) = scene.patterns.get_mut(group) {
            if let Some(pi) = *slot {
                if pi == removed {
                    *slot = Some(fallback);
                } else if pi > removed {
                    *slot = Some(pi - 1);
                }
            }
        }
    }
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

/// Contexte d'un bloc audio : permet de placer une note au sample près dans
/// le buffer courant (sinon elle est quantifiée à la taille du buffer).
#[derive(Debug, Clone, Copy)]
struct BlockCtx {
    /// Frames du bloc.
    frames: u64,
    /// Échantillons par tick (fonction du BPM et du taux d'échantillonnage).
    samples_per_tick: f64,
    /// Fraction de tick déjà écoulée au premier sample du bloc.
    tick_fraction: f64,
}

impl BlockCtx {
    /// Position en échantillons (depuis le début du bloc) d'un événement situé
    /// `rel_ticks` ticks après le début du bloc.
    fn offset(&self, rel_ticks: f64) -> usize {
        let s = ((rel_ticks - self.tick_fraction) * self.samples_per_tick).round();
        s.clamp(0.0, self.frames.saturating_sub(1) as f64) as usize
    }
}

impl Engine {
    /// Clic métronome (accent = début de mesure) — généré au taux demandé.
    fn metronome_click(sr: u32, freq: f32) -> std::sync::Arc<Vec<f32>> {
        let n = (sr as f32 * 0.030) as usize;
        let v: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sr as f32;
                let env = (-t * 120.0).exp();
                // Fondu de fin (anti-clic) : la queue du clic n'est pas coupée.
                let tail = if i + 64 >= n {
                    ((n - i) as f32 / 64.0).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                (2.0 * std::f32::consts::PI * freq * t).sin() * env * 0.6 * tail
            })
            .collect();
        std::sync::Arc::new(v)
    }

    /// Crée avec un projet.
    #[must_use]
    pub fn new(project: Project, sample_rate: u32) -> Self {
        let bpm = project.bpm;
        let mut eng = Self {
            project: Arc::new(RwLock::new(Arc::new(project))),
            transport: Transport::new(bpm, sample_rate),
            sampler: Sampler::new(sample_rate),
            acid_synth: crate::acid_synth::AcidSynth::new(sample_rate),
            preset_samples: vec![None; 16],
            sequencer: Sequencer::new(Grid::Sixteenth, 0.0),
            group_focus: 0,
            active_scene: Some(0),
            pattern_cursors: Vec::new(),
            master_volume: 0.85,
            headphone_volume: 0.85,
            muted_groups: 0,
            muted_sounds: [0; 8],
            soloed_groups: 0,
            left_tab: 0,
            right_tab: 0,
            nav_view: 0,
            nav_serial: 0,
            plugin_page: 0,
            active_sound: 0,
            selected_bar: 0,
            step_cursor: 0,
            browser_preset_idx: 0,
            browser_kind: 0,
            mixer_mode: 0,
            delete_project_confirm: false,
            browser_plugin_idx: 0,
            browser_fx_idx: 0,
            kick_synth: crate::kick_synth::KickSynth::new(sample_rate),
            wavetable_synth: crate::wavetable_synth::WavetableSynth::new(sample_rate),
            fx_processor: crate::fx::FxChainProcessor::new(sample_rate),
            project_entries: Vec::new(),
            project_selected: 0,
            current_project_path: None,
            status_message: None,
            status_expires_at: std::time::Instant::now(),
            song_mode: false,
            active_section: None,
            pad_flash_until: [0; 16],
            pad_activity: [0.0; 16],
            scene_mode: false,
            pattern_mode: false,
            select_mode: false,
            erase_mode: false,
            controller_mode: 0,
            keyboard: crate::keyboard::KeyboardState::default(),
            click_accent: Self::metronome_click(sample_rate, 1760.0),
            click_normal: Self::metronome_click(sample_rate, 880.0),
            group_meter: [0.0; 8],
            group_peak_hold: [0.0; 8],
            pad_meter: [[0.0; 16]; 8],
            pad_peak_hold: [[0.0; 16]; 8],
            master_meter: 0.0,
            master_peak_hold: 0.0,
            revision: 0,
        };
        eng.rescan_local_projects();
        eng
    }

    /// Rescanne les projets locaux et ajuste la sélection pour l'écran FILE.
    pub fn rescan_local_projects(&mut self) {
        self.project_entries = scan_local_projects();
        if let Some(curr) = self.current_project_path.as_ref() {
            if let Some(pos) = self.project_entries.iter().position(|e| &e.path == curr) {
                self.project_selected = pos;
                return;
            }
        }
        if self.project_selected >= self.project_entries.len() {
            self.project_selected = self.project_entries.len().saturating_sub(1);
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
            muted_sounds: self.muted_sounds,
            soloed_groups: self.soloed_groups,
            left_tab: self.left_tab,
            right_tab: self.right_tab,
            nav_view: self.nav_view,
            nav_serial: self.nav_serial,
            plugin_page: self.plugin_page,
            active_sound: self.active_sound,
            song_mode: self.song_mode,
            active_section: self.active_section,
            triggered_pads,
            pad_activity: self.pad_activity,
            scene_mode: self.scene_mode,
            pattern_mode: self.pattern_mode,
            select_mode: self.select_mode,
            erase_mode: self.erase_mode,
            controller_mode: self.controller_mode,
            keyboard: self.keyboard,
            browser_preset_idx: self.browser_preset_idx,
            browser_kind: self.browser_kind,
            project_entries: self.project_entries.clone(),
            project_selected: self.project_selected,
            current_project_path: self.current_project_path.clone(),
            status_message: if std::time::Instant::now() < self.status_expires_at {
                self.status_message.clone()
            } else {
                None
            },
            selected_bar,
            step_cursor,
            pattern_bars,
            group_levels: self.group_meter,
            group_peaks: self.group_peak_hold,
            pad_levels: self.pad_meter[self.group_focus as usize % 8],
            pad_peaks: self.pad_peak_hold[self.group_focus as usize % 8],
            master_level: self.master_meter,
            master_peak: self.master_peak_hold,
            soft_labels: self.soft_key_labels(),
            waveform: self
                .sampler
                .waveform_envelope(sampler_sound_id(self.group_focus, self.active_sound), 512),
            sample_duration: self
                .sampler
                .sample_duration_secs(sampler_sound_id(self.group_focus, self.active_sound)),
            acid_params: self.acid_synth.params.clone(),
            kick_params: self.kick_synth.params.clone(),
            wavetable_params: self.wavetable_synth.params.clone(),
            focus_sound_kick: self.focus_sound_kick_params(),
            focus_sound_acid: self.focus_sound_synth_params(),
            focus_sound_wavetable: self.focus_sound_wavetable_params(),
            focus_sound_fx: self.focus_sound_fx(),
            focus_group_fx: self.focus_group_fx(),
            mixer_mode: self.mixer_mode,
            browser_plugin_idx: self.browser_plugin_idx,
            browser_fx_idx: self.browser_fx_idx,
            delete_project_confirm: self.delete_project_confirm,
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
            browser_kind: self.browser_kind,
            mixer_mode: self.mixer_mode,
            has_kick_synth: self.focus_sound_has_kick(),
            has_acid_synth: self.focus_sound_has_acid(),
            has_wavetable_synth: self.focus_sound_has_wavetable(),
            delete_project_confirm: self.delete_project_confirm,
            plugin_page: self.plugin_page,
            controller_mode: self.controller_mode,
            keyboard: self.keyboard,
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
            muted_sounds: self.muted_sounds,
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
            let sound_id = sampler_sound_id(0, pad);
            self.sampler.clear_sound(sound_id);
            self.sampler.add_zone(
                sound_id,
                crate::sampler::ZoneMapping {
                    sample: Some(sample),
                    waveform_cache: None,
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

    fn default_runtime_sampler(&self, project: &Project) -> Sampler {
        let mut sampler = Sampler::new(self.transport.sample_rate);
        for (group_index, group) in project.groups.iter().enumerate() {
            let Ok(group_id) = u8::try_from(group_index) else {
                continue;
            };
            for (sound_index, sound) in group.sounds.iter().enumerate() {
                let Some(sound) = sound else { continue };
                let saved_preset = DRUM_PRESET_NAMES
                    .iter()
                    .position(|name| *name == sound.name);
                let preset = saved_preset.or((group_index == 0).then_some(sound_index));
                let Some(preset) = preset.filter(|index| *index < 16) else {
                    continue;
                };
                let sample = self.drum_preset_sample(preset as u8);
                let sound_index = sound_index as u8;
                sampler
                    .set_sound_polyphony(sampler_sound_id(group_id, sound_index), sound.polyphony);
                sampler.add_zone(
                    sampler_sound_id(group_id, sound_index),
                    crate::sampler::ZoneMapping {
                        sample: Some(sample),
                        sample_path: None,
                        sample_rate: self.transport.sample_rate,
                        root_key: 36 + sound_index,
                        lokey: 0,
                        hikey: 127,
                        lovel: 0,
                        hivel: 127,
                        choke_group: if preset == 2 || preset == 3 {
                            Some(1)
                        } else {
                            None
                        },
                        ..Default::default()
                    },
                );
            }
        }
        sampler
    }

    /// Recharge les WAV référencés par le projet, résolus relativement au
    /// fichier projet lorsqu'ils ne sont pas des chemins absolus.
    pub fn restore_project_samples(&mut self, project_path: &std::path::Path) -> crate::Result<()> {
        let project = Arc::clone(&self.project.read());
        let sample_zones = Self::prepare_project_samples(&project, project_path)?;
        self.sampler = self.default_runtime_sampler(&project);
        for (sound_id, zones) in sample_zones {
            self.sampler.clear_sound(sound_id);
            for zone in zones {
                self.sampler.add_zone(sound_id, zone);
            }
        }
        // `clear_sound` réinitialise la polyphonie : on la restaure depuis le
        // projet après le rechargement des zones personnalisées.
        for (group_index, group) in project.groups.iter().enumerate() {
            let Ok(group_id) = u8::try_from(group_index) else {
                continue;
            };
            for (sound_index, sound) in group.sounds.iter().enumerate() {
                let Some(sound) = sound else { continue };
                let Ok(sound_index) = u8::try_from(sound_index) else {
                    continue;
                };
                self.sampler
                    .set_sound_polyphony(sampler_sound_id(group_id, sound_index), sound.polyphony);
            }
        }
        Ok(())
    }

    #[cfg(feature = "sampler")]
    fn prepare_project_samples(
        project: &Project,
        project_path: &std::path::Path,
    ) -> crate::Result<Vec<(u16, Vec<crate::sampler::ZoneMapping>)>> {
        let project_dir = project_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let mut result = Vec::new();
        for (group_index, group) in project.groups.iter().enumerate() {
            let group_id = u8::try_from(group_index).map_err(|_| {
                crate::EngineError::Validation("project has more than 256 groups".to_string())
            })?;
            for (sound_index, sound) in group.sounds.iter().enumerate() {
                let Some(sound) = sound else { continue };
                let mut zones = Vec::new();
                for mapping in &sound.zones {
                    let Some(sample_path) = mapping.sample_path.as_deref() else {
                        continue;
                    };
                    let path = std::path::Path::new(sample_path);
                    let resolved = if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        project_dir.join(path)
                    };
                    zones.push(Sampler::prepare_wav_for_zone(&resolved, mapping)?);
                }
                if !zones.is_empty() {
                    result.push((sampler_sound_id(group_id, sound_index as u8), zones));
                }
            }
        }
        Ok(result)
    }

    #[cfg(not(feature = "sampler"))]
    fn prepare_project_samples(
        _project: &Project,
        _project_path: &std::path::Path,
    ) -> crate::Result<Vec<(u16, Vec<crate::sampler::ZoneMapping>)>> {
        Ok(Vec::new())
    }

    fn install_sample_zone(
        &mut self,
        group: u8,
        sound: u8,
        zone: crate::sampler::ZoneMapping,
    ) -> crate::Result<()> {
        if sound >= 16 || group as usize >= self.project.read().groups.len() {
            return Err(crate::EngineError::Validation(format!(
                "sample target out of range: group {group}, sound {sound}"
            )));
        }
        let sample_path = zone.sample_path.clone().ok_or_else(|| {
            crate::EngineError::Validation("loaded sample has no source path".to_string())
        })?;
        let sound_id = sampler_sound_id(group, sound);
        self.sampler.clear_sound(sound_id);
        self.sampler.add_zone(sound_id, zone.clone());

        let mut guard = self.project.write();
        let project = project_mut(&mut guard);
        let group = &mut project.groups[group as usize];
        let name = std::path::Path::new(&sample_path)
            .file_stem()
            .and_then(|part| part.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("Sample")
            .to_string();
        let target = &mut group.sounds[sound as usize];
        let target_sound = target.get_or_insert_with(|| crate::project::Sound::new(&name, 78));
        target_sound.name = name;
        target_sound.zones = vec![Zone {
            sample_path: Some(sample_path),
            root_key: zone.root_key,
            lokey: zone.lokey,
            hikey: zone.hikey,
            lovel: zone.lovel,
            hivel: zone.hivel,
            tune_cents: zone.tune_cents,
            pan: zone.pan,
            volume_db: zone.volume_db,
            ..Zone::default()
        }];
        Ok(())
    }

    /// Réaligne l'horloge transport/sampler sur le taux du périphérique sans
    /// régénérer les presets (appelable depuis le thread audio : aucune
    /// allocation). Filet de sécurité si le device change de taux tout seul.
    pub fn align_sample_rate_clock(&mut self, sample_rate: u32) {
        let rate = sample_rate.max(8000);
        self.transport.sample_rate = rate;
        self.sampler.sample_rate = rate;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Aligne le moteur sur le taux d'échantillonnage réel du périphérique audio.
    ///
    /// Les presets de batterie sont synthétisés : sans cette réalisation, un
    /// device à 44,1 kHz jouait un kit généré à 48 kHz ≈ 1,5 demi-ton trop haut.
    /// Les presets synthétiques sont régénérés et les zones du sampler
    /// repointées (les samples chargés depuis un fichier gardent leur taux).
    pub fn set_audio_sample_rate(&mut self, sample_rate: u32) {
        let rate = sample_rate.max(8000);
        if rate == self.transport.sample_rate && rate == self.sampler.sample_rate {
            return;
        }
        self.align_sample_rate_clock(rate);
        self.click_accent = Self::metronome_click(rate, 1760.0);
        self.click_normal = Self::metronome_click(rate, 880.0);
        for i in 0..16usize {
            let Some(old) = self.preset_samples.get(i).and_then(Option::as_ref).cloned() else {
                continue;
            };
            self.preset_samples[i] = None;
            let new = self.drum_preset_sample(i as u8);
            self.preset_samples[i] = Some(new.clone());
            self.sampler
                .replace_zone_sample(sampler_sound_id(0, i as u8), &old, new, rate);
        }
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
                // Groupe ou pad muté ? pas de son ni d'enregistrement
                if self.muted_groups & (1 << self.group_focus) != 0
                    || self.is_sound_muted(self.group_focus, pad)
                {
                    // En mode MUTE, l'appui sert au toggle via mapper ; sinon on ignore.
                    return Ok(());
                }
                if pressed {
                    // trigger sampler ou TB-303 pour le Sound du Group focus
                    let sound = sampler_sound_id(self.group_focus, pad);
                    let note = 36 + pad;
                    self.note_pad_activity(pad, velocity);
                    let guard = self.project.read();
                    let (group_gain, group_pan) = guard
                        .groups
                        .get(self.group_focus as usize)
                        .map(|g| (10.0f32.powf(g.volume_db / 20.0), g.pan))
                        .unwrap_or((1.0, 0.0));
                    let snd_ref = guard
                        .groups
                        .get(self.group_focus as usize)
                        .and_then(|g| g.sounds.get(pad as usize))
                        .and_then(|s| s.as_ref());
                    let synth_params = snd_ref.and_then(|s| s.synth.clone());
                    let kick_params = snd_ref.and_then(|s| s.kick_synth.clone());
                    let wavetable_params = snd_ref.and_then(|s| s.wavetable_synth.clone());
                    let is_synth_sound = synth_params.is_some();
                    let is_kick_sound = kick_params.is_some();
                    let is_wavetable_sound = wavetable_params.is_some();
                    let (snd_gain, snd_pan, vparams) = snd_ref
                        .map(|s| {
                            let filter_off = s.filter.kind == crate::project::FilterKind::Off;
                            (
                                10.0f32.powf(s.volume_db / 20.0),
                                s.pan,
                                crate::sampler::VoiceParams {
                                    attack_ms: s.adsr.attack_ms,
                                    decay_ms: s.adsr.decay_ms,
                                    sustain: s.adsr.sustain,
                                    release_ms: s.adsr.release_ms,
                                    cutoff_hz: if filter_off {
                                        20000.0
                                    } else {
                                        s.filter.cutoff_hz
                                    },
                                    resonance: if filter_off { 0.0 } else { s.filter.resonance },
                                },
                            )
                        })
                        .unwrap_or((1.0, 0.0, crate::sampler::VoiceParams::default()));
                    drop(guard);
                    if is_kick_sound {
                        if let Some(p) = kick_params {
                            self.kick_synth.params = p;
                        }
                        self.kick_synth.trigger(note, velocity);
                    } else if is_synth_sound {
                        if let Some(p) = synth_params {
                            self.acid_synth.params = p;
                        }
                        self.acid_synth.note_on(note, velocity);
                    } else if is_wavetable_sound {
                        if let Some(p) = wavetable_params {
                            self.wavetable_synth.params = p;
                        }
                        self.wavetable_synth.note_on(note, velocity);
                    } else if let Some(idx) = self.sampler.note_on(sound, note, velocity, note) {
                        self.sampler.set_voice_gain(idx, group_gain * snd_gain);
                        self.sampler.set_voice_pan(idx, snd_pan);
                        self.sampler.set_voice_group(idx, self.group_focus);
                        self.sampler.set_voice_group_pan(idx, group_pan);
                        self.sampler.set_voice_params(idx, &vparams);
                    }
                    // si Recording, ajoute event au pattern actif
                    if self.transport.state == TransportState::Recording {
                        self.record_pad_event(pad, velocity);
                    }
                } else {
                    let note = 36 + pad;
                    self.acid_synth.note_off(note);
                    self.wavetable_synth.note_off(note);
                    self.sampler
                        .note_off(sampler_sound_id(self.group_focus, pad), note);
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
                self.sync_acid_from_active_sound();
                if self.left_tab == 4 {
                    self.plugin_page = if self.focus_sound_has_synth() { 1 } else { 0 };
                }
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
                self.project.read().save_json_with(&path, true)?;
                self.current_project_path = Some(path.to_string_lossy().to_string());
                self.status_message = Some("Projet sauvegardé".to_string());
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_secs(2);
                self.rescan_local_projects();
            }
            EngineEvent::SaveCurrent { quick } => {
                let path = if let Some(p) = self
                    .current_project_path
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                {
                    std::path::PathBuf::from(p)
                } else {
                    // Nouvelle sauvegarde : crée un fichier lisible dans le répertoire projets.
                    let name = self.project.read().name.clone();
                    let safe = sanitize_project_filename(&name);
                    let dir = default_projects_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    dir.join(safe)
                };
                self.project.read().save_json_with(&path, !quick)?;
                self.current_project_path = Some(path.to_string_lossy().to_string());
                self.status_message = Some(if quick {
                    "Sauvegarde rapide OK".to_string()
                } else {
                    "Projet sauvegardé".to_string()
                });
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_secs(2);
                self.rescan_local_projects();
            }
            EngineEvent::NewProject { name } => {
                let (proj_name, path) = generate_unique_project_path(&name);
                let mut proj = Project::new_demo();
                proj.name = proj_name.clone();
                if let Some(g) = proj.groups.first_mut() {
                    if let Some(pat) = g.patterns.first_mut() {
                        pat.name = "Pattern 1".to_string();
                        pat.events.clear();
                    }
                }
                self.load_default_808_kit();
                proj.save_json_with(&path, false)?;
                *self.project.write() = std::sync::Arc::new(proj);
                self.current_project_path = Some(path.to_string_lossy().to_string());
                self.transport.tick = 0;
                self.selected_bar = 0;
                self.step_cursor = 0;
                self.status_message = Some(format!("Nouveau projet: {}", proj_name));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_secs(3);
                self.rescan_local_projects();
            }
            EngineEvent::SetProjectPath(path) => {
                self.current_project_path = path;
                self.rescan_local_projects();
            }
            EngineEvent::Load(path) => {
                let p = Project::load_json(&path)?;
                let sample_zones = Self::prepare_project_samples(&p, &path)?;
                let mut sampler = self.default_runtime_sampler(&p);
                for (sound_id, zones) in sample_zones {
                    sampler.clear_sound(sound_id);
                    for zone in zones {
                        sampler.add_zone(sound_id, zone);
                    }
                }
                self.sampler = sampler;
                self.transport.bpm = p.bpm;
                let proj_name = p.name.clone();
                *self.project.write() = Arc::new(p);
                self.current_project_path = Some(path.to_string_lossy().to_string());
                self.sync_acid_from_active_sound();
                if self.left_tab == 4 {
                    self.sync_plugin_for_active_sound();
                }
                self.status_message = Some(format!("Projet chargé: {}", proj_name));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_secs(2);
                self.rescan_local_projects();
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
            EngineEvent::TogglePadMute { group, pad } => {
                if (group as usize) < 8 && pad < 16 {
                    self.muted_sounds[group as usize] ^= 1u16 << pad;
                }
            }
            EngineEvent::Solo(g) => {
                if g < 8 {
                    self.soloed_groups ^= 1 << g;
                }
            }
            EngineEvent::PatternSelect { group, pattern } => {
                let group_idx = group as usize;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let valid = proj
                    .groups
                    .get(group_idx)
                    .map(|g| pattern < g.patterns.len())
                    .unwrap_or(false);
                if valid {
                    assign_pattern_to_active_scene(
                        proj,
                        &mut self.active_scene,
                        group_idx,
                        pattern,
                    );
                }
            }
            EngineEvent::AddPattern { group } => {
                let group_idx = group as usize;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let length = new_pattern_length(proj, group_idx, self.active_scene);
                if let Some(g) = proj.groups.get_mut(group_idx) {
                    let idx = g.patterns.len();
                    g.patterns
                        .push(Pattern::new(&format!("Pattern {}", idx + 1), length));
                    assign_pattern_to_active_scene(proj, &mut self.active_scene, group_idx, idx);
                }
            }
            EngineEvent::CreatePatternAt { group, pattern } => {
                let group_idx = group as usize;
                let target = pattern.min(63);
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let length = new_pattern_length(proj, group_idx, self.active_scene);
                if let Some(g) = proj.groups.get_mut(group_idx) {
                    while g.patterns.len() <= target {
                        let idx = g.patterns.len();
                        g.patterns
                            .push(Pattern::new(&format!("Pattern {}", idx + 1), length));
                    }
                    assign_pattern_to_active_scene(proj, &mut self.active_scene, group_idx, target);
                }
            }
            EngineEvent::DuplicatePattern { group, pattern } => {
                let group_idx = group as usize;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let Some(g) = proj.groups.get_mut(group_idx) else {
                    return Ok(());
                };
                if g.patterns.is_empty() {
                    return Ok(());
                }
                let src = pattern.min(g.patterns.len() - 1);
                let insert = src + 1;
                let mut copy = g.patterns[src].clone();
                copy.name = format!("{} copy", copy.name);
                g.patterns.insert(insert, copy);
                shift_scene_refs_after_insert(proj, group_idx, insert);
                assign_pattern_to_active_scene(proj, &mut self.active_scene, group_idx, insert);
                self.selected_bar = 0;
                self.step_cursor = 0;
            }
            EngineEvent::DeletePattern { group, pattern } => {
                let group_idx = group as usize;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let Some(g) = proj.groups.get_mut(group_idx) else {
                    return Ok(());
                };
                if g.patterns.len() <= 1 {
                    return Ok(());
                }
                let idx = pattern.min(g.patterns.len() - 1);
                g.patterns.remove(idx);
                let new_len = g.patterns.len();
                shift_scene_refs_after_remove(proj, group_idx, idx, new_len);
                self.selected_bar = 0;
                self.step_cursor = 0;
            }
            EngineEvent::ClearPattern { group, pattern } => {
                let group_idx = group as usize;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(pat) = proj
                    .groups
                    .get_mut(group_idx)
                    .and_then(|g| g.patterns.get_mut(pattern))
                {
                    pat.events.clear();
                }
                self.selected_bar = 0;
                self.step_cursor = 0;
            }
            EngineEvent::RenamePattern {
                group,
                pattern,
                name,
            } => {
                let group_idx = group as usize;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(pat) = proj
                    .groups
                    .get_mut(group_idx)
                    .and_then(|g| g.patterns.get_mut(pattern))
                {
                    let trimmed = name.trim();
                    pat.name = if trimmed.is_empty() {
                        format!("Pattern {}", pattern + 1)
                    } else {
                        trimmed.chars().take(32).collect()
                    };
                }
            }
            EngineEvent::StepPattern { group, delta } => {
                let group_idx = group as usize;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let len = proj
                    .groups
                    .get(group_idx)
                    .map(|g| g.patterns.len())
                    .unwrap_or(0);
                if len == 0 {
                    return Ok(());
                }
                let current = proj
                    .active_pattern_index(group_idx, self.active_scene)
                    .unwrap_or(0);
                let next = (current as i32 + delta).rem_euclid(len as i32) as usize;
                assign_pattern_to_active_scene(proj, &mut self.active_scene, group_idx, next);
            }
            EngineEvent::SetLeftTab(tab) => {
                self.left_tab = tab.min(4);
                // Une vue principale choisit son écran droit contextuel
                // (SAMPLER, MIXER, BROWSER, PLUGIN, PATTERN/STEP). Un ancien overlay
                // EVENTS/MACRO/SETTINGS ne doit pas masquer cette transition.
                self.right_tab = 0;
                if self.left_tab == 4 {
                    self.sync_plugin_for_active_sound();
                }
                if self.left_tab != 0 {
                    self.song_mode = false;
                }
            }
            EngineEvent::SetRightTab(tab) => {
                self.right_tab = tab.min(3);
                if self.right_tab != 0 {
                    self.song_mode = false;
                }
            }
            EngineEvent::NavigateView(view) => {
                self.nav_view = view.min(4);
                self.nav_serial = self.nav_serial.wrapping_add(1);
            }
            EngineEvent::SetPluginPage(page) => {
                self.plugin_page = page.min(1);
                if self.plugin_page == 1 {
                    self.sync_acid_from_active_sound();
                }
            }
            EngineEvent::SelectSound(snd) => {
                self.active_sound = snd.min(15);
                self.sync_acid_from_active_sound();
                if self.left_tab == 4 {
                    self.plugin_page = if self.focus_sound_has_synth() { 1 } else { 0 };
                }
            }
            EngineEvent::SoundVolume {
                group,
                sound,
                volume_db,
            } => {
                let mut gain_ratio = None;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group as usize) {
                    if let Some(s) = g.sounds.get_mut(sound as usize).and_then(|s| s.as_mut()) {
                        let next = volume_db.clamp(-60.0, 12.0);
                        gain_ratio = Some(10.0_f32.powf((next - s.volume_db) / 20.0));
                        s.volume_db = next;
                    }
                }
                drop(guard);
                if let Some(ratio) = gain_ratio {
                    self.sampler
                        .scale_sound_gain(sampler_sound_id(group, sound), ratio);
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
                self.sampler
                    .update_sound_pan(sampler_sound_id(group, sound), p);
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
                    self.sampler
                        .update_sound_filter(sampler_sound_id(group, sound), c, reso);
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
                    self.sampler
                        .update_sound_filter(sampler_sound_id(group, sound), cutoff, r);
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
                    self.sampler
                        .update_sound_adsr(sampler_sound_id(group, sound), a, d, su, rel);
                }
            }
            EngineEvent::SoundFilterKind { group, sound, kind } => {
                let mut filter = None;
                {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    if let Some(s) = proj
                        .groups
                        .get_mut(group as usize)
                        .and_then(|g| g.sounds.get_mut(sound as usize))
                        .and_then(|s| s.as_mut())
                    {
                        s.filter.kind = kind;
                        if s.filter.kind == crate::project::FilterKind::Off {
                            s.filter.cutoff_hz = 20000.0;
                            s.filter.resonance = 0.0;
                        } else if s.filter.cutoff_hz >= 19000.0 {
                            // Passage Off → filtre : ouvre une plage audible
                            // au lieu de laisser le cutoff à 20 kHz.
                            s.filter.cutoff_hz = 6000.0;
                            s.filter.resonance = s.filter.resonance.max(0.15);
                        }
                        filter = Some((s.filter.cutoff_hz, s.filter.resonance));
                    }
                }
                if let Some((cutoff, reso)) = filter {
                    self.sampler
                        .update_sound_filter(sampler_sound_id(group, sound), cutoff, reso);
                }
            }
            EngineEvent::SoundPolyphony {
                group,
                sound,
                polyphony,
            } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(s) = proj
                    .groups
                    .get_mut(group as usize)
                    .and_then(|g| g.sounds.get_mut(sound as usize))
                    .and_then(|s| s.as_mut())
                {
                    s.polyphony = polyphony.clamp(1, 32);
                }
                drop(guard);
                self.sampler
                    .set_sound_polyphony(sampler_sound_id(group, sound), polyphony);
            }
            EngineEvent::SoundChoke {
                group,
                sound,
                choke_group,
            } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(s) = proj
                    .groups
                    .get_mut(group as usize)
                    .and_then(|g| g.sounds.get_mut(sound as usize))
                    .and_then(|s| s.as_mut())
                {
                    s.choke_group = choke_group;
                }
                drop(guard);
                self.sampler
                    .set_sound_choke_group(sampler_sound_id(group, sound), choke_group);
            }
            EngineEvent::SoundApplyToGroup {
                group,
                source_sound,
            } => {
                let updates = {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    let Some(source) = proj
                        .groups
                        .get(group as usize)
                        .and_then(|g| g.sounds.get(source_sound as usize))
                        .and_then(|s| s.as_ref())
                        .cloned()
                    else {
                        return Ok(());
                    };
                    let Some(target_group) = proj.groups.get_mut(group as usize) else {
                        return Ok(());
                    };
                    let mut updates = Vec::new();
                    for (index, slot) in target_group.sounds.iter_mut().enumerate() {
                        if index == source_sound as usize {
                            continue;
                        }
                        let Some(target) = slot.as_mut() else {
                            continue;
                        };
                        target.volume_db = source.volume_db;
                        target.pan = source.pan;
                        target.choke_group = source.choke_group;
                        target.polyphony = source.polyphony;
                        target.adsr = source.adsr.clone();
                        target.filter = source.filter.clone();
                        updates.push((index as u8, target.clone()));
                    }
                    updates
                };
                for (index, sound) in updates {
                    let id = sampler_sound_id(group, index);
                    self.sampler.update_sound_pan(id, sound.pan);
                    self.sampler.update_sound_filter(
                        id,
                        sound.filter.cutoff_hz,
                        sound.filter.resonance,
                    );
                    self.sampler.update_sound_adsr(
                        id,
                        sound.adsr.attack_ms,
                        sound.adsr.decay_ms,
                        sound.adsr.sustain,
                        sound.adsr.release_ms,
                    );
                    self.sampler.set_sound_choke_group(id, sound.choke_group);
                    self.sampler.set_sound_polyphony(id, sound.polyphony);
                }
            }
            EngineEvent::SoundReset { group, sound } => {
                let mut reset = None;
                {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    if let Some(s) = proj
                        .groups
                        .get_mut(group as usize)
                        .and_then(|g| g.sounds.get_mut(sound as usize))
                        .and_then(|s| s.as_mut())
                    {
                        s.volume_db = 0.0;
                        s.pan = 0.0;
                        s.choke_group = None;
                        s.polyphony = 8;
                        s.adsr = crate::project::Adsr::default();
                        s.filter = crate::project::Filter::default();
                        reset = Some(s.clone());
                    }
                }
                if let Some(sound_state) = reset {
                    let id = sampler_sound_id(group, sound);
                    self.sampler.update_sound_pan(id, sound_state.pan);
                    self.sampler.update_sound_filter(id, 20000.0, 0.0);
                    self.sampler.update_sound_adsr(
                        id,
                        sound_state.adsr.attack_ms,
                        sound_state.adsr.decay_ms,
                        sound_state.adsr.sustain,
                        sound_state.adsr.release_ms,
                    );
                    self.sampler.set_sound_choke_group(id, None);
                    self.sampler.set_sound_polyphony(id, 8);
                }
            }
            EngineEvent::GroupVolume { group, volume_db } => {
                let mut gain_ratio = None;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group as usize) {
                    let next = volume_db.clamp(-60.0, 12.0);
                    gain_ratio = Some(10.0_f32.powf((next - g.volume_db) / 20.0));
                    g.volume_db = next;
                }
                drop(guard);
                if let Some(ratio) = gain_ratio {
                    self.sampler.scale_group_gain(group, ratio);
                }
            }
            EngineEvent::GroupPan { group, pan } => {
                let mut next_pan = None;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group as usize) {
                    let next = pan.clamp(-1.0, 1.0);
                    next_pan = Some(next);
                    g.pan = next;
                }
                drop(guard);
                if let Some(pan) = next_pan {
                    self.sampler.update_group_pan(group, pan);
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
            EngineEvent::KeyboardTrigger {
                note,
                velocity,
                pressed,
            } => {
                if self.muted_groups & (1 << self.group_focus) != 0
                    || self.is_sound_muted(self.group_focus, self.active_sound)
                {
                    return Ok(());
                }
                let note = note.min(127);
                let sound = sampler_sound_id(self.group_focus, self.active_sound);
                if pressed {
                    self.note_pad_activity(self.active_sound, velocity);
                    let guard = self.project.read();
                    let (group_gain, group_pan) = guard
                        .groups
                        .get(self.group_focus as usize)
                        .map(|g| (10.0f32.powf(g.volume_db / 20.0), g.pan))
                        .unwrap_or((1.0, 0.0));
                    let snd_ref = guard
                        .groups
                        .get(self.group_focus as usize)
                        .and_then(|g| g.sounds.get(self.active_sound as usize))
                        .and_then(|s| s.as_ref());
                    let synth_params = snd_ref.and_then(|s| s.synth.clone());
                    let kick_params = snd_ref.and_then(|s| s.kick_synth.clone());
                    let wavetable_params = snd_ref.and_then(|s| s.wavetable_synth.clone());
                    let (snd_gain, snd_pan, vparams) = snd_ref
                        .map(|s| {
                            let filter_off = s.filter.kind == crate::project::FilterKind::Off;
                            (
                                10.0f32.powf(s.volume_db / 20.0),
                                s.pan,
                                crate::sampler::VoiceParams {
                                    attack_ms: s.adsr.attack_ms,
                                    decay_ms: s.adsr.decay_ms,
                                    sustain: s.adsr.sustain,
                                    release_ms: s.adsr.release_ms,
                                    cutoff_hz: if filter_off {
                                        20000.0
                                    } else {
                                        s.filter.cutoff_hz
                                    },
                                    resonance: if filter_off { 0.0 } else { s.filter.resonance },
                                },
                            )
                        })
                        .unwrap_or((1.0, 0.0, crate::sampler::VoiceParams::default()));
                    drop(guard);
                    if let Some(p) = kick_params {
                        self.kick_synth.params = p;
                        self.kick_synth.trigger(note, velocity);
                    } else if let Some(p) = synth_params {
                        self.acid_synth.params = p;
                        self.acid_synth.note_on(note, velocity);
                    } else if let Some(p) = wavetable_params {
                        self.wavetable_synth.params = p;
                        self.wavetable_synth.note_on(note, velocity);
                    } else if let Some(idx) = self.sampler.note_on(sound, note, velocity, note) {
                        self.sampler.set_voice_gain(idx, group_gain * snd_gain);
                        self.sampler.set_voice_pan(idx, snd_pan);
                        self.sampler.set_voice_group(idx, self.group_focus);
                        self.sampler.set_voice_group_pan(idx, group_pan);
                        self.sampler.set_voice_params(idx, &vparams);
                    }
                    if self.transport.state == TransportState::Recording {
                        // Keyboard/Chords : enregistre la hauteur jouée (accords = plusieurs événements).
                        self.record_note_event(self.active_sound, note, velocity);
                    }
                } else {
                    self.acid_synth.note_off(note);
                    self.wavetable_synth.note_off(note);
                    self.sampler.note_off(sound, note);
                }
            }
            EngineEvent::KeyboardRootDelta(d) => self.keyboard.cycle_root(d),
            EngineEvent::KeyboardScaleDelta(d) => self.keyboard.cycle_scale(d),
            EngineEvent::KeyboardOctaveDelta(d) => self.keyboard.cycle_octave(d),
            EngineEvent::KeyboardChordDelta(d) => self.keyboard.cycle_chord(d),
            EngineEvent::KeyboardInversionDelta(d) => self.keyboard.cycle_inversion(d),
            EngineEvent::SetKeyboardState(state) => self.keyboard = state,
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
                let sound_id = sampler_sound_id(group, sound);
                self.sampler.clear_sound(sound_id);
                self.sampler.add_zone(
                    sound_id,
                    crate::sampler::ZoneMapping {
                        sample: Some(sample),
                        waveform_cache: None,
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
                    if let Some(target) = g.sounds.get_mut(sound as usize) {
                        let s = target.get_or_insert_with(|| crate::project::Sound::new(name, 78));
                        s.name = name.to_string();
                        s.zones = vec![Zone {
                            root_key: 36 + sound,
                            ..Zone::default()
                        }];
                    }
                }
            }
            EngineEvent::LoadSample { group, sound, path } => {
                #[cfg(feature = "sampler")]
                {
                    let zone = Sampler::prepare_wav(&path, 36 + sound)?;
                    self.install_sample_zone(group, sound, zone)?;
                }
                #[cfg(not(feature = "sampler"))]
                {
                    let _ = (group, sound, path);
                    log::warn!("Sampler feature not enabled");
                }
            }
            EngineEvent::LoadPreparedSample { group, sound, zone } => {
                self.install_sample_zone(group, sound, zone)?;
            }
            EngineEvent::BrowsePresetStep(step) => match self.browser_kind {
                1 => {
                    let len = self.project_entries.len().max(1);
                    self.project_selected =
                        (self.project_selected as i32 + step).rem_euclid(len as i32) as usize;
                }
                2 => {
                    self.browser_plugin_idx =
                        (self.browser_plugin_idx as i32 + step).rem_euclid(5) as u8;
                }
                3 => {
                    self.browser_fx_idx = (self.browser_fx_idx as i32 + step).rem_euclid(5) as u8;
                }
                _ => {
                    let next = (self.browser_preset_idx as i32 + step).rem_euclid(16) as u8;
                    self.browser_preset_idx = next;
                    self.preview_preset(next);
                }
            },
            EngineEvent::SetBrowserKind(kind) => {
                self.browser_kind = kind.min(3);
                self.left_tab = 3;
            }
            EngineEvent::SetProjectEntries(entries) => {
                self.project_entries = entries;
                if self.project_selected >= self.project_entries.len() {
                    self.project_selected = self.project_entries.len().saturating_sub(1);
                }
            }
            EngineEvent::BrowseBrowserStep(step) => match self.browser_kind {
                1 => {
                    let len = self.project_entries.len();
                    if len > 0 {
                        self.project_selected =
                            (self.project_selected as i32 + step).rem_euclid(len as i32) as usize;
                    }
                }
                2 => {
                    self.browser_plugin_idx =
                        (self.browser_plugin_idx as i32 + step).rem_euclid(5) as u8;
                }
                3 => {
                    self.browser_fx_idx = (self.browser_fx_idx as i32 + step).rem_euclid(5) as u8;
                }
                _ => {
                    let next = (self.browser_preset_idx as i32 + step).rem_euclid(16) as u8;
                    self.browser_preset_idx = next;
                    self.preview_preset(next);
                }
            },
            EngineEvent::LoadSelectedBrowserItem { group, sound } => match self.browser_kind {
                1 => {
                    if let Some(entry) = self.project_entries.get(self.project_selected).cloned() {
                        self.handle_event(EngineEvent::Load(std::path::PathBuf::from(entry.path)))?;
                    }
                }
                2 => {
                    self.load_plugin_to_sound(group, sound, self.browser_plugin_idx)?;
                    self.left_tab = 4;
                }
                3 => {
                    let fx_type = match self.browser_fx_idx {
                        0 => crate::fx::FxType::Reverb,
                        1 => crate::fx::FxType::Delay,
                        2 => crate::fx::FxType::Distortion,
                        3 => crate::fx::FxType::Filter,
                        _ => crate::fx::FxType::Compressor,
                    };
                    self.add_fx_to_sound(group, sound, fx_type)?;
                }
                _ => {
                    self.handle_event(EngineEvent::LoadDrumPreset {
                        group,
                        sound,
                        preset_idx: self.browser_preset_idx,
                    })?;
                }
            },
            EngineEvent::DeleteSelectedProject => {
                self.delete_selected_project()?;
            }
            EngineEvent::CancelDeleteProject => {
                self.delete_project_confirm = false;
                self.status_message = Some("Suppression annulée".to_string());
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(2000);
            }
            EngineEvent::SetMixerMode(mode) => {
                self.mixer_mode = mode.min(3);
            }
            EngineEvent::KickSetParams(params) => {
                self.kick_synth.params = params.clone();
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(s) = proj
                    .groups
                    .get_mut(self.group_focus as usize)
                    .and_then(|g| g.sounds.get_mut(self.active_sound as usize))
                    .and_then(|s| s.as_mut())
                {
                    s.kick_synth = Some(params);
                }
            }
            EngineEvent::KickSetPreset(idx) => {
                let presets = crate::kick_synth::KickParams::all_presets();
                if let Some(p) = presets.get(idx as usize) {
                    self.handle_event(EngineEvent::KickSetParams(p.clone()))?;
                }
            }
            EngineEvent::KickDriveTypeDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                let next = match p.drive_type {
                    crate::kick_synth::KickDriveType::AnalogTanh => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::HardClip
                        } else {
                            crate::kick_synth::KickDriveType::Rectifier
                        }
                    }
                    crate::kick_synth::KickDriveType::HardClip => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::Wavefolder
                        } else {
                            crate::kick_synth::KickDriveType::AnalogTanh
                        }
                    }
                    crate::kick_synth::KickDriveType::Wavefolder => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::TubeDrive
                        } else {
                            crate::kick_synth::KickDriveType::HardClip
                        }
                    }
                    crate::kick_synth::KickDriveType::TubeDrive => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::Bitcrush
                        } else {
                            crate::kick_synth::KickDriveType::Wavefolder
                        }
                    }
                    crate::kick_synth::KickDriveType::Bitcrush => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::Fuzz
                        } else {
                            crate::kick_synth::KickDriveType::TubeDrive
                        }
                    }
                    crate::kick_synth::KickDriveType::Fuzz => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::Rectifier
                        } else {
                            crate::kick_synth::KickDriveType::Bitcrush
                        }
                    }
                    crate::kick_synth::KickDriveType::Rectifier => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::AnalogTanh
                        } else {
                            crate::kick_synth::KickDriveType::Fuzz
                        }
                    }
                };
                p.drive_type = next;
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickFilterModeDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                let next = match p.filter_mode {
                    crate::kick_synth::KickFilterMode::Off => {
                        if delta > 0 {
                            crate::kick_synth::KickFilterMode::LowPass
                        } else {
                            crate::kick_synth::KickFilterMode::Peak
                        }
                    }
                    crate::kick_synth::KickFilterMode::LowPass => {
                        if delta > 0 {
                            crate::kick_synth::KickFilterMode::HighPass
                        } else {
                            crate::kick_synth::KickFilterMode::Off
                        }
                    }
                    crate::kick_synth::KickFilterMode::HighPass => {
                        if delta > 0 {
                            crate::kick_synth::KickFilterMode::BandPass
                        } else {
                            crate::kick_synth::KickFilterMode::LowPass
                        }
                    }
                    crate::kick_synth::KickFilterMode::BandPass => {
                        if delta > 0 {
                            crate::kick_synth::KickFilterMode::Notch
                        } else {
                            crate::kick_synth::KickFilterMode::HighPass
                        }
                    }
                    crate::kick_synth::KickFilterMode::Notch => {
                        if delta > 0 {
                            crate::kick_synth::KickFilterMode::Peak
                        } else {
                            crate::kick_synth::KickFilterMode::BandPass
                        }
                    }
                    crate::kick_synth::KickFilterMode::Peak => {
                        if delta > 0 {
                            crate::kick_synth::KickFilterMode::Off
                        } else {
                            crate::kick_synth::KickFilterMode::Notch
                        }
                    }
                };
                p.filter_mode = next;
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickFilterCutoffDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                let factor = 1.05f32.powf(delta * 0.1);
                p.filter_cutoff_hz = (p.filter_cutoff_hz * factor).clamp(20.0, 18000.0);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickFilterResoDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                p.filter_reso = (p.filter_reso + delta * 0.02).clamp(0.0, 0.95);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::CycleSoundFilterKind { group, sound } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(snd) = proj
                    .groups
                    .get_mut(group as usize)
                    .and_then(|g| g.sounds.get_mut(sound as usize))
                    .and_then(|s| s.as_mut())
                {
                    snd.filter.kind = match snd.filter.kind {
                        crate::project::FilterKind::Off => crate::project::FilterKind::LowPass,
                        crate::project::FilterKind::LowPass => crate::project::FilterKind::HighPass,
                        crate::project::FilterKind::HighPass => {
                            crate::project::FilterKind::BandPass
                        }
                        crate::project::FilterKind::BandPass => crate::project::FilterKind::Off,
                    };
                    let kind = snd.filter.kind;
                    self.status_message =
                        Some(format!("Sound {:02} Filtre : {:?}", sound + 1, kind));
                    self.status_expires_at =
                        std::time::Instant::now() + std::time::Duration::from_millis(2500);
                    self.revision = self.revision.wrapping_add(1);
                }
            }
            EngineEvent::SetSoundFilterKind { group, sound, kind } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(snd) = proj
                    .groups
                    .get_mut(group as usize)
                    .and_then(|g| g.sounds.get_mut(sound as usize))
                    .and_then(|s| s.as_mut())
                {
                    snd.filter.kind = kind;
                    self.status_message =
                        Some(format!("Sound {:02} Filtre : {:?}", sound + 1, kind));
                    self.status_expires_at =
                        std::time::Instant::now() + std::time::Duration::from_millis(2500);
                    self.revision = self.revision.wrapping_add(1);
                }
            }
            EngineEvent::WavetableSetParams(params) => {
                self.wavetable_synth.params = params.clone();
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(s) = proj
                    .groups
                    .get_mut(self.group_focus as usize)
                    .and_then(|g| g.sounds.get_mut(self.active_sound as usize))
                    .and_then(|s| s.as_mut())
                {
                    s.wavetable_synth = Some(params);
                }
                self.revision = self.revision.wrapping_add(1);
            }
            EngineEvent::WavetableSetPreset(idx) => {
                let presets = crate::wavetable_synth::WavetableParams::all_presets();
                if let Some(p) = presets.get(idx).cloned() {
                    let name = p.name.clone();
                    self.handle_event(EngineEvent::WavetableSetParams(p))?;
                    self.status_message = Some(format!("Preset: {name}"));
                    self.status_expires_at =
                        std::time::Instant::now() + std::time::Duration::from_millis(2500);
                }
            }
            EngineEvent::WavetableNextPreset => {
                let presets = crate::wavetable_synth::WavetableParams::all_presets();
                let cur_idx = presets
                    .iter()
                    .position(|p| p.name == self.wavetable_synth.params.name)
                    .unwrap_or(0);
                let next = (cur_idx + 1) % presets.len();
                let p = presets[next].clone();
                let name = p.name.clone();
                self.handle_event(EngineEvent::WavetableSetParams(p))?;
                self.status_message = Some(format!("Preset: {name}"));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(2500);
            }
            EngineEvent::WavetablePrevPreset => {
                let presets = crate::wavetable_synth::WavetableParams::all_presets();
                let cur_idx = presets
                    .iter()
                    .position(|p| p.name == self.wavetable_synth.params.name)
                    .unwrap_or(0);
                let prev = (cur_idx + presets.len() - 1) % presets.len();
                let p = presets[prev].clone();
                let name = p.name.clone();
                self.handle_event(EngineEvent::WavetableSetParams(p))?;
                self.status_message = Some(format!("Preset: {name}"));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(2500);
            }
            EngineEvent::LoadPluginToSound {
                group,
                sound,
                plugin_idx,
            } => {
                self.load_plugin_to_sound(group, sound, plugin_idx)?;
            }
            EngineEvent::AddFxToSound {
                group,
                sound,
                fx_type,
            } => {
                self.add_fx_to_sound(group, sound, fx_type)?;
            }
            EngineEvent::AddFxToGroup { group, fx_type } => {
                self.add_fx_to_group(group, fx_type)?;
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
            EngineEvent::AcidCutoffDelta(delta) => {
                let base = self.acid_synth.params.cutoff_hz.max(20.0);
                let factor = 2.0f32.powf(delta * 0.05);
                self.acid_synth.params.cutoff_hz = (base * factor).clamp(20.0, 18000.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidResoDelta(delta) => {
                let cur = self.acid_synth.params.resonance;
                self.acid_synth.params.resonance = (cur + delta * 0.02).clamp(0.0, 0.985);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidEnvModDelta(delta) => {
                let cur = self.acid_synth.params.env_mod;
                self.acid_synth.params.env_mod = (cur + delta * 0.02).clamp(0.0, 1.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDecayDelta(delta) => {
                let cur = self.acid_synth.params.decay_ms;
                let factor = 2.0f32.powf(delta * 0.05);
                self.acid_synth.params.decay_ms = (cur * factor).clamp(20.0, 3000.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidAccentDelta(delta) => {
                let cur = self.acid_synth.params.accent;
                self.acid_synth.params.accent = (cur + delta * 0.02).clamp(0.0, 1.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidSlideDelta(delta) => {
                let cur = self.acid_synth.params.slide_time_ms;
                self.acid_synth.params.slide_time_ms = (cur + delta * 2.0).clamp(0.0, 250.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDriveDelta(delta) => {
                let cur = self.acid_synth.params.drive;
                self.acid_synth.params.drive = (cur + delta * 0.02).clamp(0.0, 1.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDriveToneDelta(delta) => {
                let cur = self.acid_synth.params.drive_tone.max(200.0);
                let factor = 2.0f32.powf(delta * 0.05);
                self.acid_synth.params.drive_tone = (cur * factor).clamp(500.0, 16000.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDriveMixDelta(delta) => {
                let cur = self.acid_synth.params.drive_mix;
                self.acid_synth.params.drive_mix = (cur + delta * 0.02).clamp(0.0, 1.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDelayTimeDelta(delta) => {
                let cur = self.acid_synth.params.delay_time_ms;
                self.acid_synth.params.delay_time_ms = (cur + delta * 5.0).clamp(20.0, 1000.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDelayFeedbackDelta(delta) => {
                let cur = self.acid_synth.params.delay_feedback;
                self.acid_synth.params.delay_feedback = (cur + delta * 0.02).clamp(0.0, 0.95);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDelayMixDelta(delta) => {
                let cur = self.acid_synth.params.delay_mix;
                self.acid_synth.params.delay_mix = (cur + delta * 0.02).clamp(0.0, 1.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidWaveformToggle => {
                self.acid_synth.params.waveform = match self.acid_synth.params.waveform {
                    crate::acid_synth::AcidWaveform::Saw => crate::acid_synth::AcidWaveform::Square,
                    crate::acid_synth::AcidWaveform::Square => crate::acid_synth::AcidWaveform::Saw,
                };
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidOctaveDelta(d) => {
                self.acid_synth.params.octave = (self.acid_synth.params.octave + d).clamp(-2, 2);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidSelectPreset(idx) => {
                match idx {
                    0 => {
                        self.acid_synth.params =
                            crate::acid_synth::AcidParams::preset_classic_acid()
                    }
                    1 => {
                        self.acid_synth.params =
                            crate::acid_synth::AcidParams::preset_squelch_lead()
                    }
                    2 => self.acid_synth.params = crate::acid_synth::AcidParams::preset_distorted(),
                    3 => self.acid_synth.params = crate::acid_synth::AcidParams::preset_dub_echo(),
                    _ => {}
                }
                if idx <= 3 {
                    // En page TB, appliquer le preset au Sound actif (créé synthé si besoin).
                    if self.left_tab == 4 && self.plugin_page == 1 {
                        self.ensure_active_sound_synth();
                    }
                    self.persist_acid_to_active_sound();
                }
            }
            EngineEvent::AcidSetParams(p) => {
                self.acid_synth.params = p;
                if self.left_tab == 4 && self.plugin_page == 1 {
                    self.ensure_active_sound_synth();
                }
                self.persist_acid_to_active_sound();
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
    fn trigger_click(&mut self, accent: bool, start_frame: usize) {
        let sample = if accent {
            self.click_accent.clone()
        } else {
            self.click_normal.clone()
        };
        self.sampler
            .play_raw_at(&sample, if accent { 0.5 } else { 0.32 }, start_frame);
    }

    /// Avance le transport + séquenceur, déclenche les notes des patterns.
    pub fn advance(&mut self, samples: u64) {
        // Décroissance de l'activité des pads (animation écrans, ~170 ms).
        let dt = samples as f32 / self.transport.sample_rate.max(1) as f32;
        let mut activity_alive = false;
        for v in &mut self.pad_activity {
            if *v > 0.0 {
                *v = (*v - dt * PAD_ACTIVITY_DECAY).max(0.0);
                activity_alive = true;
            }
        }
        if activity_alive {
            self.revision = self.revision.wrapping_add(1);
        }

        let prev_tick = self.transport.tick;
        let frac_before = self.transport.tick_fraction;
        self.transport.advance_samples(samples);
        let new_tick = self.transport.tick;
        if new_tick != prev_tick {
            self.revision = self.revision.wrapping_add(1);
        }
        if !self.transport.is_playing() {
            return;
        }
        // Contexte du bloc : place chaque note au sample près au lieu de la
        // quantifier à la taille du buffer (sinon le groove est décalé).
        let bpm = self.transport.bpm.max(1.0) as f64;
        let ppq = self.transport.ppq.max(1) as f64;
        let ctx = BlockCtx {
            frames: samples,
            samples_per_tick: self.transport.sample_rate.max(1) as f64 * 60.0 / (bpm * ppq),
            tick_fraction: frac_before,
        };
        // Clic métronome sur chaque temps (accent sur début de mesure).
        if self.transport.metronome && new_tick > prev_tick {
            let first_beat = prev_tick / 960 + 1;
            let last_beat = new_tick / 960;
            for beat in first_beat..=last_beat.min(first_beat + 8) {
                let accent = beat % 4 == 0;
                let rel = (beat * 960).saturating_sub(prev_tick) as f64;
                self.trigger_click(accent, ctx.offset(rel));
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
                                let mut is_synth = [false; 16];
                                let mut is_kick = [false; 16];
                                let mut is_wavetable = [false; 16];
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
                                        is_synth[s] = snd.synth.is_some();
                                        is_kick[s] = snd.kick_synth.is_some();
                                        is_wavetable[s] = snd.wavetable_synth.is_some();
                                    }
                                }
                                Self::trigger_pattern_events(
                                    pat,
                                    prev_tick,
                                    new_tick,
                                    &mut self.sampler,
                                    &mut self.acid_synth,
                                    &mut self.kick_synth,
                                    &mut self.wavetable_synth,
                                    gi as u8,
                                    gain,
                                    group.pan,
                                    &sg,
                                    &sp,
                                    &vp,
                                    &is_synth,
                                    &is_kick,
                                    &is_wavetable,
                                    self.transport.swing,
                                    &mut self.pad_flash_until,
                                    new_tick,
                                    &ctx,
                                    &mut self.pad_activity,
                                    self.muted_sounds[gi],
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[must_use]
    pub fn is_sound_muted(&self, group: u8, pad: u8) -> bool {
        if group >= 8 || pad >= 16 {
            return false;
        }
        (self.muted_sounds[group as usize] & (1u16 << pad)) != 0
    }

    #[must_use]
    pub fn is_group_muted(&self, group: u8) -> bool {
        if group >= 8 {
            return false;
        }
        (self.muted_groups & (1u8 << group)) != 0
    }

    /// Marque l'activité d'un pad (animation écrans) — 0..1 selon vélocité.
    pub fn note_pad_activity(&mut self, pad: u8, velocity: u8) {
        let index = (pad as usize).min(15);
        let level = (velocity as f32 / 127.0).clamp(0.35, 1.0);
        self.pad_activity[index] = self.pad_activity[index].max(level);
    }

    #[allow(clippy::too_many_arguments)] // contexte de trigger (pattern + position + sampler + synth)
    fn trigger_pattern_events(
        pat: &crate::project::Pattern,
        prev_tick: u64,
        new_tick: u64,
        sampler: &mut Sampler,
        acid_synth: &mut crate::acid_synth::AcidSynth,
        kick_synth: &mut crate::kick_synth::KickSynth,
        wavetable_synth: &mut crate::wavetable_synth::WavetableSynth,
        group: u8,
        group_gain: f32,
        group_pan: f32,
        sound_gains: &[f32],
        sound_pans: &[f32],
        voice_params: &[crate::sampler::VoiceParams],
        is_synth: &[bool],
        is_kick: &[bool],
        is_wavetable: &[bool],
        swing: f32,
        flash_until: &mut [u64; 16],
        now_tick: u64,
        ctx: &BlockCtx,
        activity: &mut [f32; 16],
        muted_sound_mask: u16,
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
                let sound_muted =
                    (sound as usize) < 16 && (muted_sound_mask & (1u16 << sound)) != 0;
                if !sound_muted {
                    let note = ev.pitch;
                    let sg = sound_gains.get(sound as usize).copied().unwrap_or(1.0);
                    let sp = sound_pans.get(sound as usize).copied().unwrap_or(0.0);
                    // Décalage sample-accurate dans le bloc courant : la note part à
                    // sa vraie position rythmique, pas au début du buffer audio.
                    let local = eff % pat_len.max(1);
                    let rel_ticks = if !wrapped || local > prev_in_pat {
                        local.saturating_sub(prev_in_pat) as f64
                    } else {
                        (pat_len - prev_in_pat + local) as f64
                    };
                    let start_frame = ctx.offset(rel_ticks);
                    let is_acid = is_synth.get(sound as usize).copied().unwrap_or(false);
                    let is_k = is_kick.get(sound as usize).copied().unwrap_or(false);
                    let is_wt = is_wavetable.get(sound as usize).copied().unwrap_or(false);
                    if is_k {
                        kick_synth.trigger(note, ev.velocity);
                    } else if is_acid {
                        acid_synth.note_on_at(note, ev.velocity, start_frame);
                    } else if is_wt {
                        wavetable_synth.note_on(note, ev.velocity);
                    } else if let Some(idx) = sampler.note_on_at(
                        sampler_sound_id(group, sound),
                        note,
                        ev.velocity,
                        note,
                        start_frame,
                    ) {
                        sampler.set_voice_gain(idx, group_gain * sg);
                        sampler.set_voice_pan(idx, sp);
                        sampler.set_voice_group(idx, group);
                        sampler.set_voice_group_pan(idx, group_pan);
                        if let Some(vp) = voice_params.get(sound as usize) {
                            sampler.set_voice_params(idx, vp);
                        }
                    }
                    // Flash LED du pad (fenêtre d'une triple-croche : bref et net).
                    if (sound as usize) < 16 {
                        flash_until[sound as usize] = now_tick + 120;
                        let level = (ev.velocity as f32 / 127.0).clamp(0.35, 1.0);
                        if level > activity[sound as usize] {
                            activity[sound as usize] = level;
                        }
                    }
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
                if is_synth.get(ev.pad as usize).copied().unwrap_or(false) {
                    acid_synth.note_off(ev.pitch);
                } else if is_wavetable.get(ev.pad as usize).copied().unwrap_or(false) {
                    wavetable_synth.note_off(ev.pitch);
                } else {
                    sampler.note_off(sampler_sound_id(group, ev.pad), ev.pitch);
                }
            }
        }
    }

    fn record_pad_event(&mut self, pad: u8, velocity: u8) {
        self.record_note_event(pad, 36 + pad, velocity);
    }

    fn record_note_event(&mut self, pad: u8, pitch: u8, velocity: u8) {
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
                    // évite les doublons exacts (même son + tick + hauteur)
                    pat.events
                        .retain(|e| !(e.pad == pad && e.tick == q_tick && e.pitch == pitch));
                    pat.events.push(PatternEvent {
                        pad,
                        tick: q_tick,
                        velocity,
                        length_ticks: 120,
                        pitch,
                    });
                    pat.events.sort_by_key(|e| e.tick);
                }
            }
        }
    }

    /// Render audio stéréo — à appeler depuis callback cpal/jack.
    pub fn render_audio(&mut self, out: &mut [f32]) {
        self.sampler.render(out);
        self.acid_synth.render(out);
        self.kick_synth.render(out);
        self.wavetable_synth.render(out);
        let group_fx = self.focus_group_fx();
        if !group_fx.is_empty() {
            self.fx_processor.process(out, &group_fx);
        }
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
        // VU-mètres par pad : mesure les crêtes réelles pour chaque tranche de pad
        for g in 0..8 {
            let s_peaks = self.sampler.sound_peaks(g);
            for (s, &peak_val) in s_peaks.iter().enumerate() {
                let mut p = peak_val;
                if g == self.group_focus as usize && s == self.active_sound as usize {
                    let act = self.pad_activity[s];
                    if act > p {
                        p = act;
                    }
                }
                let p = p.clamp(0.0, 4.0);
                self.pad_meter[g][s] = if p > self.pad_meter[g][s] {
                    p
                } else {
                    self.pad_meter[g][s] * 0.96
                };
                self.pad_peak_hold[g][s] = if p > self.pad_peak_hold[g][s] {
                    p
                } else {
                    self.pad_peak_hold[g][s] * 0.995
                };
            }
        }
        // Bus master : volume + limiteur doux (tanh). Avant, la sortie était
        // écrêtée durement (clamp ±1) : kick + snare + clap (~1,7) distordaient.
        // Ici la réponse reste linéaire sous -3 dB puis sature en douceur.
        let mv = self.master_volume;
        let mut peak = 0.0f32;
        for s in out.iter_mut() {
            let x = *s * mv;
            let a = x.abs();
            let y = if a <= 0.7 {
                x
            } else {
                x.signum() * (0.7 + 0.3 * ((a - 0.7) / 0.3).tanh())
            };
            *s = y;
            let a = y.abs();
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
    ) -> Option<Arc<[(f32, f32)]>> {
        self.sampler
            .waveform_envelope(sampler_sound_id(group, sound), buckets)
    }

    /// Durée du sample d'un Sound (secondes).
    #[must_use]
    pub fn sample_duration_secs(&self, group: u8, sound: u8) -> f32 {
        self.sampler
            .sample_duration_secs(sampler_sound_id(group, sound))
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
        if self.pattern_mode {
            // Mode PATTERN : gestion complète des patterns du groupe focus.
            return [
                "PAT <", "PAT >", "+ PAT", "DUPL", "VIDER", "SUPPR", "1 MES", "2 MES",
            ];
        }
        if self.controller_mode == 3 && self.left_tab == 0 {
            // Mode STEP : navigation dans la grille + longueur du pattern.
            return [
                "PAS <", "PAS >", "BAR <", "BAR >", "1 MES", "2 MES", "4 MES", "8 MES",
            ];
        }
        if self.controller_mode == 1 {
            // KEYBOARD : les boutons haut changent gamme/fondamentale/octave.
            return [
                "ROOT -", "ROOT +", "SCALE-", "SCALE+", "OCT -", "OCT +", "CHROMA", "MINOR",
            ];
        }
        if self.controller_mode == 2 {
            // CHORDS : gammes/accords/inversions.
            return [
                "ROOT -", "ROOT +", "SCALE-", "SCALE+", "CHORD-", "CHORD+", "INV -", "INV +",
            ];
        }
        match self.left_tab {
            1 if self.mixer_mode == 0 || self.mixer_mode == 1 => [
                "PAD 1-8", "PAD 9-16", "GROUPES", "MASTER", "PAN/VOL", "MUTE", "SOLO", "RETOUR",
            ],
            1 => [
                "GRP A", "GRP B", "GRP C", "GRP D", "GRP E", "GRP F", "GRP G", "GRP H",
            ],
            2 => [
                "SND <", "SND >", "ECOUTE", "CYC FLT", "LP", "HP", "BP", "OFF",
            ],
            3 if self.browser_kind == 1 && self.delete_project_confirm => {
                ["ANNULER", "", "", "", "", "CONFIRMER", "", ""]
            }
            3 if self.browser_kind == 1 => [
                "PROJ <", "PROJ >", "NOUVEAU", "CHARGER", "SAUVER", "SUPPR", "PRESETS", "RETOUR",
            ],
            3 if self.browser_kind == 2 => [
                "PLUG <", "PLUG >", "PRESETS", "CHARGER", "KICK", "303", "SAMPLER", "RETOUR",
            ],
            3 if self.browser_kind == 3 => [
                "FX <", "FX >", "GRP/PAD", "CHARGER", "REVERB", "DELAY", "DISTO", "RETOUR",
            ],
            3 => [
                "PRES <", "PRES >", "SND <", "SND >", "ECOUTE", "CHARGER", "PLUGINS", "PROJETS",
            ],
            4 => {
                if self.focus_sound_has_kick() {
                    [
                        "DRV TYPE",
                        "FLT MODE",
                        "TECHNO",
                        "909",
                        "HARDCORE",
                        "808 SUB",
                        "INDUSTRIAL",
                        "BROWSER",
                    ]
                } else if self.focus_sound_has_acid() {
                    [
                        "SAW/SQR", "OCT -", "OCT +", "SLIDE", "CLASSIC", "SQUELCH", "DUB",
                        "BROWSER",
                    ]
                } else if self.focus_sound_has_wavetable() {
                    [
                        "PRES <", "PRES >", "PIANO", "M1 PIANO", "808 SUB", "REESE", "TITANIUM",
                        "STRINGS",
                    ]
                } else {
                    [
                        "SND <", "SND >", "ONE-SHOT", "ECOUTE", "PLUGINS", "RESET", "VOL -",
                        "VOL +",
                    ]
                }
            }
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
        if self.pattern_mode {
            let group = self.group_focus;
            let active = {
                let proj = self.project.read();
                proj.active_pattern_index(group as usize, self.active_scene)
            };
            return match idx {
                0 => self.handle_event(EngineEvent::StepPattern { group, delta: -1 }),
                1 => self.handle_event(EngineEvent::StepPattern { group, delta: 1 }),
                2 => self.handle_event(EngineEvent::AddPattern { group }),
                3 => match active {
                    Some(pattern) => {
                        self.handle_event(EngineEvent::DuplicatePattern { group, pattern })
                    }
                    None => Ok(()),
                },
                4 => match active {
                    Some(pattern) => {
                        self.handle_event(EngineEvent::ClearPattern { group, pattern })
                    }
                    None => Ok(()),
                },
                5 => match active {
                    Some(pattern) => {
                        self.handle_event(EngineEvent::DeletePattern { group, pattern })
                    }
                    None => Ok(()),
                },
                6 => self.handle_event(EngineEvent::SetPatternBars(1)),
                _ => self.handle_event(EngineEvent::SetPatternBars(2)),
            };
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
        if self.controller_mode == 1 {
            return match idx {
                0 => self.handle_event(EngineEvent::KeyboardRootDelta(-1)),
                1 => self.handle_event(EngineEvent::KeyboardRootDelta(1)),
                2 => self.handle_event(EngineEvent::KeyboardScaleDelta(-1)),
                3 => self.handle_event(EngineEvent::KeyboardScaleDelta(1)),
                4 => self.handle_event(EngineEvent::KeyboardOctaveDelta(-1)),
                5 => self.handle_event(EngineEvent::KeyboardOctaveDelta(1)),
                6 => self.handle_event(EngineEvent::SetKeyboardState(
                    crate::keyboard::KeyboardState {
                        scale: 0,
                        ..self.keyboard
                    },
                )),
                _ => self.handle_event(EngineEvent::SetKeyboardState(
                    crate::keyboard::KeyboardState {
                        scale: 2,
                        ..self.keyboard
                    },
                )),
            };
        }
        if self.controller_mode == 2 {
            return match idx {
                0 => self.handle_event(EngineEvent::KeyboardRootDelta(-1)),
                1 => self.handle_event(EngineEvent::KeyboardRootDelta(1)),
                2 => self.handle_event(EngineEvent::KeyboardScaleDelta(-1)),
                3 => self.handle_event(EngineEvent::KeyboardScaleDelta(1)),
                4 => self.handle_event(EngineEvent::KeyboardChordDelta(-1)),
                5 => self.handle_event(EngineEvent::KeyboardChordDelta(1)),
                6 => self.handle_event(EngineEvent::KeyboardInversionDelta(-1)),
                _ => self.handle_event(EngineEvent::KeyboardInversionDelta(1)),
            };
        }
        match self.left_tab {
            1 if self.mixer_mode == 0 || self.mixer_mode == 1 => match idx {
                0 => self.handle_event(EngineEvent::SetMixerMode(0)),
                1 => self.handle_event(EngineEvent::SetMixerMode(1)),
                2 => self.handle_event(EngineEvent::SetMixerMode(2)),
                3 => self.handle_event(EngineEvent::SetMixerMode(3)),
                4 => Ok(()),
                5 => {
                    let p = self.active_sound;
                    self.handle_event(EngineEvent::TogglePadMute {
                        group: self.group_focus,
                        pad: p,
                    })
                }
                6 => self.handle_event(EngineEvent::Solo(self.group_focus)),
                _ => {
                    self.left_tab = 0;
                    Ok(())
                }
            },
            1 => self.handle_event(EngineEvent::GroupFocus(idx as u8)),
            2 => match idx {
                0 => {
                    let s = self.active_sound.saturating_sub(1);
                    self.handle_event(EngineEvent::SelectSound(s))
                }
                1 => {
                    let s = (self.active_sound + 1).min(15);
                    self.handle_event(EngineEvent::SelectSound(s))
                }
                2 => self.handle_event(EngineEvent::Pad {
                    pad: self.active_sound,
                    velocity: 110,
                    pressed: true,
                }),
                3 => self.handle_event(EngineEvent::CycleSoundFilterKind {
                    group: self.group_focus,
                    sound: self.active_sound,
                }),
                4 => self.handle_event(EngineEvent::SetSoundFilterKind {
                    group: self.group_focus,
                    sound: self.active_sound,
                    kind: crate::project::FilterKind::LowPass,
                }),
                5 => self.handle_event(EngineEvent::SetSoundFilterKind {
                    group: self.group_focus,
                    sound: self.active_sound,
                    kind: crate::project::FilterKind::HighPass,
                }),
                6 => self.handle_event(EngineEvent::SetSoundFilterKind {
                    group: self.group_focus,
                    sound: self.active_sound,
                    kind: crate::project::FilterKind::BandPass,
                }),
                _ => self.handle_event(EngineEvent::SetSoundFilterKind {
                    group: self.group_focus,
                    sound: self.active_sound,
                    kind: crate::project::FilterKind::Off,
                }),
            },
            3 if self.browser_kind == 1 && self.delete_project_confirm => match idx {
                0 => self.handle_event(EngineEvent::CancelDeleteProject),
                5 => self.handle_event(EngineEvent::DeleteSelectedProject),
                _ => self.handle_event(EngineEvent::CancelDeleteProject),
            },
            3 if self.browser_kind == 1 => match idx {
                0 => self.handle_event(EngineEvent::BrowseBrowserStep(-1)),
                1 => self.handle_event(EngineEvent::BrowseBrowserStep(1)),
                2 => self.handle_event(EngineEvent::NewProject {
                    name: String::new(),
                }),
                3 => self.handle_event(EngineEvent::LoadSelectedBrowserItem {
                    group: self.group_focus,
                    sound: self.active_sound,
                }),
                4 => self.handle_event(EngineEvent::SaveCurrent { quick: false }),
                5 => {
                    self.delete_project_confirm = true;
                    self.status_message =
                        Some("Supprimer le projet ? BTN 6: CONFIRMER / BTN 1: ANNULER".into());
                    self.status_expires_at =
                        std::time::Instant::now() + std::time::Duration::from_millis(5000);
                    Ok(())
                }
                6 => self.handle_event(EngineEvent::SetBrowserKind(0)),
                7 => {
                    self.left_tab = 0;
                    Ok(())
                }
                _ => Ok(()),
            },
            3 if self.browser_kind == 2 => match idx {
                0 => self.handle_event(EngineEvent::BrowseBrowserStep(-1)),
                1 => self.handle_event(EngineEvent::BrowseBrowserStep(1)),
                2 => self.handle_event(EngineEvent::SetBrowserKind(0)),
                3 => self.handle_event(EngineEvent::LoadSelectedBrowserItem {
                    group: self.group_focus,
                    sound: self.active_sound,
                }),
                4 => self.handle_event(EngineEvent::LoadPluginToSound {
                    group: self.group_focus,
                    sound: self.active_sound,
                    plugin_idx: 0,
                }),
                5 => self.handle_event(EngineEvent::LoadPluginToSound {
                    group: self.group_focus,
                    sound: self.active_sound,
                    plugin_idx: 1,
                }),
                6 => self.handle_event(EngineEvent::LoadPluginToSound {
                    group: self.group_focus,
                    sound: self.active_sound,
                    plugin_idx: 3,
                }),
                _ => {
                    self.left_tab = 0;
                    Ok(())
                }
            },
            3 if self.browser_kind == 3 => match idx {
                0 => self.handle_event(EngineEvent::BrowseBrowserStep(-1)),
                1 => self.handle_event(EngineEvent::BrowseBrowserStep(1)),
                2 => self.handle_event(EngineEvent::SetBrowserKind(0)),
                3 => self.handle_event(EngineEvent::LoadSelectedBrowserItem {
                    group: self.group_focus,
                    sound: self.active_sound,
                }),
                4 => self.handle_event(EngineEvent::AddFxToSound {
                    group: self.group_focus,
                    sound: self.active_sound,
                    fx_type: crate::fx::FxType::Reverb,
                }),
                5 => self.handle_event(EngineEvent::AddFxToSound {
                    group: self.group_focus,
                    sound: self.active_sound,
                    fx_type: crate::fx::FxType::Delay,
                }),
                6 => self.handle_event(EngineEvent::AddFxToSound {
                    group: self.group_focus,
                    sound: self.active_sound,
                    fx_type: crate::fx::FxType::Distortion,
                }),
                _ => {
                    self.left_tab = 0;
                    Ok(())
                }
            },
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
                6 => self.handle_event(EngineEvent::SetBrowserKind(2)),
                _ => self.handle_event(EngineEvent::SetBrowserKind(1)),
            },
            4 => {
                if self.focus_sound_has_kick() {
                    match idx {
                        0 => self.handle_event(EngineEvent::KickDriveTypeDelta(1)),
                        1 => self.handle_event(EngineEvent::KickFilterModeDelta(1)),
                        2 => self.handle_event(EngineEvent::KickSetPreset(0)),
                        3 => self.handle_event(EngineEvent::KickSetPreset(1)),
                        4 => self.handle_event(EngineEvent::KickSetPreset(2)),
                        5 => self.handle_event(EngineEvent::KickSetPreset(3)),
                        6 => self.handle_event(EngineEvent::KickSetPreset(5)),
                        _ => {
                            self.left_tab = 3;
                            self.browser_kind = 2;
                            Ok(())
                        }
                    }
                } else if self.focus_sound_has_acid() {
                    match idx {
                        0 => self.handle_event(EngineEvent::AcidWaveformToggle),
                        1 => self.handle_event(EngineEvent::AcidOctaveDelta(-1)),
                        2 => self.handle_event(EngineEvent::AcidOctaveDelta(1)),
                        3 => {
                            let slide = if self.acid_synth.params.slide_time_ms > 1.0 {
                                -100.0
                            } else {
                                60.0
                            };
                            self.handle_event(EngineEvent::AcidSlideDelta(slide))
                        }
                        4 => self.handle_event(EngineEvent::AcidSelectPreset(0)),
                        5 => self.handle_event(EngineEvent::AcidSelectPreset(1)),
                        6 => self.handle_event(EngineEvent::AcidSelectPreset(3)),
                        _ => {
                            self.left_tab = 3;
                            self.browser_kind = 2;
                            Ok(())
                        }
                    }
                } else if self.focus_sound_has_wavetable() {
                    match idx {
                        0 => self.handle_event(EngineEvent::WavetablePrevPreset),
                        1 => self.handle_event(EngineEvent::WavetableNextPreset),
                        2 => self.handle_event(EngineEvent::WavetableSetPreset(0)),
                        3 => self.handle_event(EngineEvent::WavetableSetPreset(1)),
                        4 => self.handle_event(EngineEvent::WavetableSetPreset(4)),
                        5 => self.handle_event(EngineEvent::WavetableSetPreset(5)),
                        6 => self.handle_event(EngineEvent::WavetableSetPreset(8)),
                        _ => self.handle_event(EngineEvent::WavetableSetPreset(11)),
                    }
                } else {
                    match idx {
                        0 => {
                            let s = self.active_sound.saturating_sub(1);
                            self.handle_event(EngineEvent::SelectSound(s))
                        }
                        1 => {
                            let s = (self.active_sound + 1).min(15);
                            self.handle_event(EngineEvent::SelectSound(s))
                        }
                        2 => self.handle_event(EngineEvent::Pad {
                            pad: self.active_sound,
                            velocity: 112,
                            pressed: true,
                        }),
                        3 => {
                            let preset = self.browser_preset_idx;
                            self.preview_preset(preset);
                            Ok(())
                        }
                        4 => {
                            self.left_tab = 3;
                            self.browser_kind = 2;
                            Ok(())
                        }
                        5 => self.handle_event(EngineEvent::SoundReset {
                            group: self.group_focus,
                            sound: self.active_sound,
                        }),
                        6 | 7 => {
                            let delta = if idx == 6 { -3.0 } else { 3.0 };
                            let current = self
                                .project
                                .read()
                                .groups
                                .get(self.group_focus as usize)
                                .and_then(|g| g.sounds.get(self.active_sound as usize))
                                .and_then(|s| s.as_ref())
                                .map(|s| s.volume_db)
                                .unwrap_or(0.0);
                            self.handle_event(EngineEvent::SoundVolume {
                                group: self.group_focus,
                                sound: self.active_sound,
                                volume_db: (current + delta).clamp(-60.0, 12.0),
                            })
                        }
                        _ => Ok(()),
                    }
                }
            }
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

    /// Paramètres TB-303 du Sound actif, si ce Sound est un synthé.
    pub fn focus_sound_synth_params(&self) -> Option<crate::acid_synth::AcidParams> {
        let proj = self.project.read();
        proj.groups
            .get(self.group_focus as usize)
            .and_then(|g| g.sounds.get(self.active_sound as usize))
            .and_then(|s| s.as_ref())
            .and_then(|s| s.synth.clone())
    }

    /// Le Sound actif est-il un synthétiseur (TB-303, Kick Synth ou Wavetable) ?
    pub fn focus_sound_has_synth(&self) -> bool {
        self.focus_sound_synth_params().is_some()
            || self.focus_sound_kick_params().is_some()
            || self.focus_sound_wavetable_params().is_some()
    }

    /// Le Sound actif est-il un synthétiseur TB-303 Acid ?
    pub fn focus_sound_has_acid(&self) -> bool {
        self.focus_sound_synth_params().is_some()
    }

    /// Paramètres Kick Synth du Sound actif.
    pub fn focus_sound_kick_params(&self) -> Option<crate::kick_synth::KickParams> {
        let proj = self.project.read();
        proj.groups
            .get(self.group_focus as usize)
            .and_then(|g| g.sounds.get(self.active_sound as usize))
            .and_then(|s| s.as_ref())
            .and_then(|s| s.kick_synth.clone())
    }

    /// Le Sound actif est-il un Kick Synth modélisé ?
    pub fn focus_sound_has_kick(&self) -> bool {
        self.focus_sound_kick_params().is_some()
    }

    /// Paramètres Wavetable / Nexus Synth du Sound actif.
    pub fn focus_sound_wavetable_params(&self) -> Option<crate::wavetable_synth::WavetableParams> {
        let proj = self.project.read();
        proj.groups
            .get(self.group_focus as usize)
            .and_then(|g| g.sounds.get(self.active_sound as usize))
            .and_then(|s| s.as_ref())
            .and_then(|s| s.wavetable_synth.clone())
    }

    /// Le Sound actif est-il un Wavetable / Nexus Synth ?
    pub fn focus_sound_has_wavetable(&self) -> bool {
        self.focus_sound_wavetable_params().is_some()
    }

    /// Chaîne d'effets du Sound actif.
    pub fn focus_sound_fx(&self) -> Vec<crate::fx::FxSlot> {
        let proj = self.project.read();
        proj.groups
            .get(self.group_focus as usize)
            .and_then(|g| g.sounds.get(self.active_sound as usize))
            .and_then(|s| s.as_ref())
            .map(|s| s.fx_chain.clone())
            .unwrap_or_default()
    }

    /// Chaîne d'effets du Group focus.
    pub fn focus_group_fx(&self) -> Vec<crate::fx::FxSlot> {
        let proj = self.project.read();
        proj.groups
            .get(self.group_focus as usize)
            .map(|g| g.fx_chain.clone())
            .unwrap_or_default()
    }

    /// Charge les paramètres Kick du Sound actif dans le KickSynth interne.
    pub fn sync_kick_from_active_sound(&mut self) {
        if let Some(params) = self.focus_sound_kick_params() {
            self.kick_synth.params = params;
        }
    }

    /// Charge les paramètres Wavetable du Sound actif dans le WavetableSynth interne.
    pub fn sync_wavetable_from_active_sound(&mut self) {
        if let Some(params) = self.focus_sound_wavetable_params() {
            self.wavetable_synth.params = params;
        }
    }

    /// Supprime définitivement le projet sélectionné dans le navigateur.
    pub fn delete_selected_project(&mut self) -> crate::Result<()> {
        self.delete_project_confirm = false;
        if let Some(entry) = self.project_entries.get(self.project_selected).cloned() {
            let path = std::path::Path::new(&entry.path);
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
            let deleted_name = entry.name;
            self.rescan_local_projects();
            if self.project_selected >= self.project_entries.len() {
                self.project_selected = self.project_entries.len().saturating_sub(1);
            }
            self.status_message = Some(format!("Projet '{}' supprimé", deleted_name));
            self.status_expires_at =
                std::time::Instant::now() + std::time::Duration::from_millis(3000);
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }

    /// Charge un plugin (0: Kick Synth, 1: TB-303, 2: Nexus Synth, 3: Guitar Amp, 4: Sampler) sur un pad.
    pub fn load_plugin_to_sound(
        &mut self,
        group: u8,
        sound: u8,
        plugin_idx: u8,
    ) -> crate::Result<()> {
        let mut guard = self.project.write();
        let project = project_mut(&mut guard);
        if let Some(grp) = project.groups.get_mut(group as usize) {
            let target_sound = grp.sounds[sound as usize].get_or_insert_with(|| {
                crate::project::Sound::new(&format!("Sound {}", sound + 1), 78)
            });
            match plugin_idx {
                0 => {
                    target_sound.kick_synth = Some(crate::kick_synth::KickParams::default());
                    target_sound.synth = None;
                    target_sound.wavetable_synth = None;
                    target_sound.name = "Kick Synth".to_string();
                    self.kick_synth.params = crate::kick_synth::KickParams::default();
                    self.status_message =
                        Some("Plugin chargé : Kick Synth (Modélisation Techno)".to_string());
                }
                1 => {
                    target_sound.synth = Some(crate::acid_synth::AcidParams::preset_classic_acid());
                    target_sound.kick_synth = None;
                    target_sound.wavetable_synth = None;
                    target_sound.name = "TB-303".to_string();
                    self.acid_synth.params = crate::acid_synth::AcidParams::preset_classic_acid();
                    self.status_message = Some("Plugin chargé : TB-303 Acid Bass".to_string());
                }
                2 => {
                    target_sound.wavetable_synth =
                        Some(crate::wavetable_synth::WavetableParams::default());
                    target_sound.synth = None;
                    target_sound.kick_synth = None;
                    target_sound.name = "Nexus Synth".to_string();
                    self.wavetable_synth.params =
                        crate::wavetable_synth::WavetableParams::default();
                    self.status_message =
                        Some("Plugin chargé : Nexus / Serum Wavetable Synth".to_string());
                }
                3 => {
                    target_sound.synth = None;
                    target_sound.kick_synth = None;
                    target_sound.wavetable_synth = None;
                    target_sound.name = "Guitar Amp".to_string();
                    target_sound.fx_chain = vec![
                        crate::fx::FxSlot::default_for(crate::fx::FxType::Distortion),
                        crate::fx::FxSlot::default_for(crate::fx::FxType::Delay),
                    ];
                    self.status_message = Some("Plugin chargé : Guitar Amp & FX".to_string());
                }
                _ => {
                    target_sound.synth = None;
                    target_sound.kick_synth = None;
                    target_sound.wavetable_synth = None;
                    target_sound.name = DRUM_PRESET_NAMES
                        .get(sound as usize)
                        .copied()
                        .unwrap_or("Sampler")
                        .to_string();
                    self.status_message = Some("Plugin chargé : Sampler Kit 808".to_string());
                }
            }
            self.status_expires_at =
                std::time::Instant::now() + std::time::Duration::from_millis(3000);
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }

    /// Ajoute un effet au pad actif.
    pub fn add_fx_to_sound(
        &mut self,
        group: u8,
        sound: u8,
        fx_type: crate::fx::FxType,
    ) -> crate::Result<()> {
        let mut guard = self.project.write();
        let project = project_mut(&mut guard);
        if let Some(grp) = project.groups.get_mut(group as usize) {
            let target_sound = grp.sounds[sound as usize].get_or_insert_with(|| {
                crate::project::Sound::new(&format!("Sound {}", sound + 1), 78)
            });
            let slot = crate::fx::FxSlot::default_for(fx_type);
            target_sound.fx_chain.push(slot);
            self.status_message = Some(format!(
                "Effet {} ajouté au Pad {}",
                fx_type.name(),
                sound + 1
            ));
            self.status_expires_at =
                std::time::Instant::now() + std::time::Duration::from_millis(3000);
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }

    /// Ajoute un effet au groupe focus.
    pub fn add_fx_to_group(&mut self, group: u8, fx_type: crate::fx::FxType) -> crate::Result<()> {
        let mut guard = self.project.write();
        let project = project_mut(&mut guard);
        if let Some(grp) = project.groups.get_mut(group as usize) {
            let slot = crate::fx::FxSlot::default_for(fx_type);
            grp.fx_chain.push(slot);
            self.status_message = Some(format!(
                "Effet {} ajouté au Groupe {}",
                fx_type.name(),
                (b'A' + group) as char
            ));
            self.status_expires_at =
                std::time::Instant::now() + std::time::Duration::from_millis(3000);
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }

    /// Charge les paramètres TB-303 du Sound actif dans le synthé global
    /// (édition, pré-écoute et écrans).
    fn sync_acid_from_active_sound(&mut self) {
        if let Some(params) = self.focus_sound_synth_params() {
            self.acid_synth.params = params;
        }
    }

    /// Garantit que le Sound actif possède un slot synthé TB-303 (utile quand
    /// l'utilisateur applique un preset TB sur un pad sampler depuis la page PLUGIN).
    fn ensure_active_sound_synth(&mut self) {
        let params = self.acid_synth.params.clone();
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        if let Some(g) = proj.groups.get_mut(self.group_focus as usize) {
            let slot = g
                .sounds
                .get_mut(self.active_sound as usize)
                .and_then(|s| s.as_mut());
            if slot.is_none() {
                let mut s =
                    crate::project::Sound::new(&format!("TB {}", self.active_sound + 1), 100);
                s.synth = Some(params);
                if let Some(entry) = g.sounds.get_mut(self.active_sound as usize) {
                    *entry = Some(s);
                }
            } else if let Some(sound) = slot {
                if sound.synth.is_none() {
                    sound.synth = Some(params);
                }
            }
        }
    }

    /// Persiste les paramètres TB-303 globaux dans le Sound actif lorsqu'il
    /// s'agit d'un synthé, afin que les éditions suivent le pad sélectionné.
    fn persist_acid_to_active_sound(&mut self) {
        if !self.focus_sound_has_synth() {
            return;
        }
        let params = self.acid_synth.params.clone();
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        if let Some(sound) = proj
            .groups
            .get_mut(self.group_focus as usize)
            .and_then(|g| g.sounds.get_mut(self.active_sound as usize))
            .and_then(|s| s.as_mut())
        {
            if sound.synth.is_some() {
                sound.synth = Some(params);
            }
        }
    }

    /// Synchronise la page PLUGIN et les paramètres TB avec le Sound actif.
    fn sync_plugin_for_active_sound(&mut self) {
        self.plugin_page = if self.focus_sound_has_synth() { 1 } else { 0 };
        self.sync_acid_from_active_sound();
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

fn sanitize_project_filename(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    out = out.trim().trim_matches('.').to_string();
    if out.is_empty() {
        out = "Untitled".to_string();
    }
    if out.len() > 80 {
        out.truncate(80);
    }
    if !out.ends_with(".maschine-linux.json") && !out.ends_with(".json") {
        out.push_str(".maschine-linux.json");
    }
    out
}

fn default_projects_dir() -> std::path::PathBuf {
    std::env::var("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            std::path::PathBuf::from(home).join(".local/share")
        })
        .join("maschine-linux")
        .join("projects")
}

/// Scanne le répertoire local des projets et retourne les entrées triées par date.
pub fn scan_local_projects() -> Vec<ProjectFileEntry> {
    let dir = default_projects_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if (name.ends_with(".maschine-linux.json") || name.ends_with(".json"))
                && !name.starts_with('.')
            {
                paths.push(p);
            }
        }
    }
    paths.sort_by_key(|a| {
        std::cmp::Reverse(
            std::fs::metadata(a)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
        )
    });
    paths
        .into_iter()
        .map(|p| {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("Projet");
            let stem = stem.strip_suffix(".maschine-linux").unwrap_or(stem);
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            let meta = if size >= 1024 * 1024 {
                format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
            } else {
                format!("{} KB", (size / 1024).max(1))
            };
            ProjectFileEntry {
                name: stem.to_string(),
                path: p.to_string_lossy().to_string(),
                meta,
            }
        })
        .collect()
}

/// Génère un nom et un chemin de projet unique sans écraser l'existant.
pub fn generate_unique_project_path(base_name: &str) -> (String, std::path::PathBuf) {
    let dir = default_projects_dir();
    let _ = std::fs::create_dir_all(&dir);
    let trimmed = base_name.trim();
    if trimmed.is_empty() || trimmed == "Nouveau" || trimmed == "Untitled" {
        for i in 1..=9999 {
            let candidate_name = format!("Projet {}", i);
            let file_name = format!("{}.maschine-linux.json", candidate_name);
            let path = dir.join(&file_name);
            if !path.exists() {
                return (candidate_name, path);
            }
        }
    }
    let safe_stem = sanitize_project_filename(trimmed);
    let stem = safe_stem
        .strip_suffix(".maschine-linux.json")
        .unwrap_or(&safe_stem)
        .strip_suffix(".json")
        .unwrap_or(&safe_stem)
        .to_string();
    let initial_path = dir.join(format!("{}.maschine-linux.json", stem));
    if !initial_path.exists() {
        return (stem, initial_path);
    }
    for i in 2..=9999 {
        let candidate_name = format!("{} ({})", stem, i);
        let path = dir.join(format!("{}.maschine-linux.json", candidate_name));
        if !path.exists() {
            return (candidate_name, path);
        }
    }
    (stem.clone(), initial_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_test_wav(path: &std::path::Path, samples: &[i16], sample_rate: u32) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for &sample in samples {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn test_temp_dir(label: &str) -> std::path::PathBuf {
        static NEXT_DIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("maschine-{label}-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn wav_import_is_group_scoped_and_persisted_in_sound() {
        let dir = test_temp_dir("group-sample");
        let wav_path = dir.join("group-b.wav");
        write_test_wav(&wav_path, &[i16::MIN, 0, i16::MAX], 24000);

        let mut project = Project::new_demo();
        project
            .groups
            .push(crate::project::Group::new("Group B", 22));
        for scene in &mut project.scenes {
            scene.patterns.push(None);
        }
        let mut engine = Engine::new(project, 48000);
        engine.sampler_mut().add_zone(
            sampler_sound_id(0, 0),
            crate::sampler::ZoneMapping {
                sample: Some(std::sync::Arc::new(vec![0.25, -0.25, 0.5, -0.5])),
                sample_rate: 48000,
                root_key: 36,
                ..Default::default()
            },
        );
        let group_a_waveform = engine.waveform_envelope(0, 0, 512).unwrap();
        let zone = Sampler::prepare_wav(&wav_path, 36).unwrap();
        engine
            .handle_event(EngineEvent::LoadPreparedSample {
                group: 1,
                sound: 0,
                zone,
            })
            .unwrap();

        assert!(std::sync::Arc::ptr_eq(
            &group_a_waveform,
            &engine.waveform_envelope(0, 0, 512).unwrap()
        ));
        assert_eq!(engine.sample_duration_secs(1, 0), 3.0 / 24000.0);
        let snapshot = engine.snapshot();
        assert_eq!(
            snapshot.project.groups[1].sounds[0].as_ref().unwrap().name,
            "group-b"
        );
        assert_eq!(
            snapshot.project.groups[1].sounds[0].as_ref().unwrap().zones[0]
                .sample_path
                .as_deref(),
            Some(wav_path.canonicalize().unwrap().to_str().unwrap())
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn loading_project_rehydrates_relative_sample_paths() {
        let dir = test_temp_dir("project-sample");
        let samples_dir = dir.join("samples");
        std::fs::create_dir_all(&samples_dir).unwrap();
        let wav_path = samples_dir.join("tone.wav");
        write_test_wav(&wav_path, &vec![8192; 24000], 24000);
        let project_path = dir.join("project.json");
        let mut project = Project::new_demo();
        project.groups[0].sounds[0].as_mut().unwrap().zones[0].sample_path =
            Some("samples/tone.wav".to_string());
        project.save_json(&project_path).unwrap();

        let mut engine = Engine::new_demo(48000);
        engine
            .handle_event(EngineEvent::Load(project_path.clone()))
            .unwrap();

        assert!((engine.sample_duration_secs(0, 0) - 1.0).abs() < 0.001);
        let waveform = engine.waveform_envelope(0, 0, 512).unwrap();
        assert_eq!(waveform.len(), 512);
        assert!((waveform[0].0 - 0.25).abs() < 0.001);
        assert!((waveform[0].1 - 0.25).abs() < 0.001);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn selecting_sampler_restores_contextual_right_screen_after_events() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::SetRightTab(1)).unwrap();
        assert_eq!(eng.snapshot().right_tab, 1);

        eng.handle_event(EngineEvent::SetLeftTab(2)).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.left_tab, 2);
        assert_eq!(snap.right_tab, 0);
        assert!(!snap.song_mode);
    }

    #[test]
    fn plugin_follows_selected_sound_and_persists_tb_edits() {
        let mut eng = Engine::new_demo(48000);
        // Un Sound sampler ouvre la page SOUND.
        eng.handle_event(EngineEvent::SelectSound(0)).unwrap();
        eng.handle_event(EngineEvent::SetLeftTab(4)).unwrap();
        assert_eq!(eng.snapshot().plugin_page, 0);

        // Sur la page TB, un preset rend le Sound actif synthé et persiste.
        eng.handle_event(EngineEvent::SetPluginPage(1)).unwrap();
        eng.handle_event(EngineEvent::AcidSelectPreset(1)).unwrap();
        {
            let snap = eng.snapshot();
            assert_eq!(snap.plugin_page, 1);
            let sound = snap.project.groups[snap.group_focus as usize].sounds[0]
                .as_ref()
                .unwrap();
            assert!(sound.synth.is_some());
            let cutoff = sound.synth.as_ref().unwrap().cutoff_hz;
            assert!((cutoff - snap.acid_params.cutoff_hz).abs() < 0.01);
        }

        // L'édition d'un paramètre TB est persistée dans le Sound sélectionné.
        eng.handle_event(EngineEvent::AcidCutoffDelta(10.0))
            .unwrap();
        let cutoff_after = {
            let snap = eng.snapshot();
            snap.project.groups[snap.group_focus as usize].sounds[0]
                .as_ref()
                .unwrap()
                .synth
                .as_ref()
                .unwrap()
                .cutoff_hz
        };

        // Changer de pad en vue PLUGIN suit le type du Sound.
        eng.handle_event(EngineEvent::SelectSound(1)).unwrap();
        assert_eq!(eng.snapshot().plugin_page, 0);

        // Revenir sur le pad TB restaure sa page et ses paramètres.
        eng.handle_event(EngineEvent::SelectSound(0)).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.plugin_page, 1);
        let restored = snap.project.groups[snap.group_focus as usize].sounds[0]
            .as_ref()
            .unwrap()
            .synth
            .as_ref()
            .unwrap()
            .cutoff_hz;
        assert!((restored - cutoff_after).abs() < 0.01);
        assert!((snap.acid_params.cutoff_hz - cutoff_after).abs() < 0.01);
    }

    #[test]
    fn selecting_synth_tab_preserves_left_tab_4() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::SetLeftTab(4)).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.left_tab, 4);
        assert_eq!(snap.right_tab, 0);
        assert!(!snap.song_mode);
    }

    #[test]
    fn selecting_right_overlay_exits_song_mode() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::ToggleSongMode).unwrap();
        assert!(eng.snapshot().song_mode);

        eng.handle_event(EngineEvent::SetRightTab(1)).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.right_tab, 1);
        assert!(!snap.song_mode);
    }

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
    fn sound_and_group_mix_edits_reach_active_voices_immediately() {
        let mut engine = Engine::new_demo(48000);
        engine
            .handle_event(EngineEvent::Pad {
                pad: 0,
                velocity: 100,
                pressed: true,
            })
            .unwrap();
        let initial_gain = engine.sampler.voice_mix(0).unwrap().0;

        engine
            .handle_event(EngineEvent::SoundVolume {
                group: 0,
                sound: 0,
                volume_db: -6.0,
            })
            .unwrap();
        let expected_after_sound = initial_gain * 10.0_f32.powf(-6.0 / 20.0);
        assert!((engine.sampler.voice_mix(0).unwrap().0 - expected_after_sound).abs() < 1e-5);

        engine
            .handle_event(EngineEvent::GroupVolume {
                group: 0,
                volume_db: 6.0,
            })
            .unwrap();
        let expected_after_group = expected_after_sound * 10.0_f32.powf(6.0 / 20.0);
        assert!((engine.sampler.voice_mix(0).unwrap().0 - expected_after_group).abs() < 1e-5);

        engine
            .handle_event(EngineEvent::SoundPan {
                group: 0,
                sound: 0,
                pan: 0.3,
            })
            .unwrap();
        engine
            .handle_event(EngineEvent::GroupPan { group: 0, pan: 0.4 })
            .unwrap();
        assert!((engine.sampler.voice_mix(0).unwrap().1 - 0.7).abs() < 1e-5);
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

    /// Les notes démarrent à leur vraie position dans le buffer audio (et non
    /// au début du bloc) : c'est ce qui garde le groove serré même avec un
    /// buffer de plusieurs millisecondes.
    #[test]
    fn notes_start_at_their_position_inside_the_block() {
        let mut eng = Engine::new_demo(48000);
        eng.clear_bar(0);
        eng.clear_bar(1);
        // 150 BPM à 48 kHz : 20 échantillons par tick (blocs de 1000 = 50 ticks).
        eng.handle_event(EngineEvent::Bpm(150.0)).unwrap();
        eng.toggle_step(0, 240); // kick au tick 240
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: false,
            stop: false,
        })
        .unwrap();

        let mut buf = vec![0.0f32; 1000 * 2];
        let mut first: Option<(usize, usize)> = None;
        for block in 0..6 {
            eng.advance(1000);
            eng.render_audio(&mut buf);
            if first.is_none() {
                if let Some(pos) = buf
                    .chunks_exact(2)
                    .position(|f| f[0].abs() > 2e-3 || f[1].abs() > 2e-3)
                {
                    first = Some((block, pos));
                }
            }
        }
        let (block, pos) = first.expect("kick non entendu");
        assert_eq!(block, 4, "kick déclenché dans le mauvais bloc");
        assert!(
            (pos as i64 - 800).abs() < 32,
            "kick à la frame {pos} au lieu de 800 (position dans le bloc)"
        );
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
        // Avance d'un temps (0.5s à 120BPM) par petits blocs, comme le
        // callback audio (advance + render du même bloc) : au moins un clic.
        let mut buf = vec![0.0f32; 1024 * 2];
        let mut mx = 0.0f32;
        for _ in 0..24 {
            eng.advance(1024);
            eng.render_audio(&mut buf);
            mx = buf.iter().fold(mx, |a, b| a.max(b.abs()));
        }
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
    fn pattern_crud_create_duplicate_clear_delete() {
        let mut eng = Engine::new_demo(48000);
        let snap = eng.snapshot();
        let group = snap.group_focus as usize;
        let initial_len = snap.project.groups[group].patterns.len();
        assert!(initial_len >= 1);

        // Création : le nouveau pattern est ajouté à la suite et assigné à la
        // scène active.
        eng.handle_event(EngineEvent::AddPattern { group: group as u8 })
            .unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.project.groups[group].patterns.len(), initial_len + 1);
        let active = snap
            .project
            .active_pattern_index(group, snap.active_scene)
            .unwrap();
        assert_eq!(active, initial_len);
        assert_eq!(
            snap.project.scenes[snap.active_scene.unwrap()].patterns[group],
            Some(active)
        );

        // Renommage.
        eng.handle_event(EngineEvent::RenamePattern {
            group: group as u8,
            pattern: active,
            name: "  Mon pattern  ".to_string(),
        })
        .unwrap();
        let snap = eng.snapshot();
        assert_eq!(
            snap.project.groups[group].patterns[active].name,
            "Mon pattern"
        );

        // Duplication : copie insérée juste après, sélectionnée.
        eng.handle_event(EngineEvent::DuplicatePattern {
            group: group as u8,
            pattern: active,
        })
        .unwrap();
        let snap = eng.snapshot();
        let copy = active + 1;
        assert_eq!(snap.project.groups[group].patterns.len(), initial_len + 2);
        assert!(snap.project.groups[group].patterns[copy]
            .name
            .contains("copy"));
        assert_eq!(
            snap.project
                .active_pattern_index(group, snap.active_scene)
                .unwrap(),
            copy
        );

        // Effacement des événements sans changer la longueur.
        eng.handle_event(EngineEvent::ClearPattern {
            group: group as u8,
            pattern: copy,
        })
        .unwrap();
        let snap = eng.snapshot();
        assert!(snap.project.groups[group].patterns[copy].events.is_empty());

        // Suppression : les références de scène restent valides.
        eng.handle_event(EngineEvent::DeletePattern {
            group: group as u8,
            pattern: copy,
        })
        .unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.project.groups[group].patterns.len(), initial_len + 1);
        for scene in &snap.project.scenes {
            if let Some(pi) = scene.patterns[group] {
                assert!(pi < snap.project.groups[group].patterns.len());
            }
        }

        // Le dernier pattern du groupe ne peut pas être supprimé.
        while eng.snapshot().project.groups[group].patterns.len() > 1 {
            let active = eng
                .snapshot()
                .project
                .active_pattern_index(group, eng.snapshot().active_scene)
                .unwrap();
            eng.handle_event(EngineEvent::DeletePattern {
                group: group as u8,
                pattern: active,
            })
            .unwrap();
        }
        eng.handle_event(EngineEvent::DeletePattern {
            group: group as u8,
            pattern: 0,
        })
        .unwrap();
        assert_eq!(eng.snapshot().project.groups[group].patterns.len(), 1);
    }

    #[test]
    fn pattern_insert_delete_only_reindexes_the_target_group() {
        let mut eng = Engine::new_demo(48000);
        {
            let mut guard = eng.project.write();
            let proj = project_mut(&mut guard);
            // Deuxième groupe avec 3 patterns.
            let mut group_b = proj.groups[0].clone();
            group_b.name = "Groupe B".to_string();
            group_b
                .patterns
                .push(Pattern::new("B2", BarsBeatsTicks::new(2, 0, 0)));
            group_b
                .patterns
                .push(Pattern::new("B3", BarsBeatsTicks::new(2, 0, 0)));
            proj.groups.push(group_b);

            // Scène 0 : A=P1, B=P3. Scène 1 : A=P2, B=P3.
            proj.scenes[0].patterns = vec![Some(0), Some(2)];
            let mut scene2 = Scene::new("Scene 2", 2);
            scene2.patterns = vec![Some(1), Some(2)];
            proj.scenes.push(scene2);
        }
        eng.active_scene = Some(0);

        // Insère une copie de P1 dans le groupe A : seule la référence A de la
        // scène 1 doit être décalée, la référence du groupe B ne bouge pas.
        eng.handle_event(EngineEvent::DuplicatePattern {
            group: 0,
            pattern: 0,
        })
        .unwrap();
        {
            let snap = eng.snapshot();
            // La scène active sélectionne la copie (index 1), l'autre scène
            // décale sa référence A de 1 vers 2 ; B reste à 2.
            assert_eq!(snap.project.scenes[0].patterns, vec![Some(1), Some(2)]);
            assert_eq!(snap.project.scenes[1].patterns, vec![Some(2), Some(2)]);
            assert_eq!(snap.project.groups[0].patterns.len(), 2);
        }

        // Supprime la copie : la référence A de la scène 1 revient à P2.
        eng.handle_event(EngineEvent::DeletePattern {
            group: 0,
            pattern: 1,
        })
        .unwrap();
        {
            let snap = eng.snapshot();
            assert_eq!(snap.project.scenes[0].patterns, vec![Some(0), Some(2)]);
            assert_eq!(snap.project.scenes[1].patterns, vec![Some(1), Some(2)]);
            assert_eq!(snap.project.groups[0].patterns.len(), 1);
        }
    }

    #[test]
    fn pattern_create_at_slot_fills_intermediate_slots_and_selects() {
        let mut eng = Engine::new_demo(48000);
        let group = eng.snapshot().group_focus as usize;
        eng.handle_event(EngineEvent::CreatePatternAt {
            group: group as u8,
            pattern: 4,
        })
        .unwrap();
        let snap = eng.snapshot();
        assert!(snap.project.groups[group].patterns.len() >= 5);
        assert_eq!(
            snap.project
                .active_pattern_index(group, snap.active_scene)
                .unwrap(),
            4
        );
        assert_eq!(
            snap.project.scenes[snap.active_scene.unwrap()].patterns[group],
            Some(4)
        );
    }

    #[test]
    fn pattern_step_wraps_and_pattern_soft_keys_are_exposed() {
        let mut eng = Engine::new_demo(48000);
        let group = eng.snapshot().group_focus;
        eng.handle_event(EngineEvent::SetPatternMode(true)).unwrap();
        assert_eq!(
            eng.soft_key_labels(),
            ["PAT <", "PAT >", "+ PAT", "DUPL", "VIDER", "SUPPR", "1 MES", "2 MES"]
        );

        // Ajoute un pattern : le pattern actif est le dernier. +1 boucle sur 0.
        eng.handle_event(EngineEvent::AddPattern { group }).unwrap();
        let len = eng.snapshot().project.groups[group as usize].patterns.len();
        assert!(len > 1);
        eng.handle_event(EngineEvent::StepPattern { group, delta: 1 })
            .unwrap();
        let snap = eng.snapshot();
        assert_eq!(
            snap.project
                .active_pattern_index(group as usize, snap.active_scene)
                .unwrap(),
            0
        );

        // Soft key 3 = + PAT : crée et sélectionne un pattern.
        eng.handle_event(EngineEvent::SoftKey(3)).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.project.groups[group as usize].patterns.len(), len + 1);
        assert_eq!(
            snap.project
                .active_pattern_index(group as usize, snap.active_scene)
                .unwrap(),
            len
        );
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

    #[test]
    fn file_mode_soft_keys_and_new_project_creation() {
        let mut eng = Engine::new_demo(48000);
        eng.left_tab = 3;
        eng.browser_kind = 1; // FILE mode

        // 1. Softkey labels en mode FILE (avec suppression directe depuis MK3)
        assert_eq!(
            eng.soft_key_labels(),
            ["PROJ <", "PROJ >", "NOUVEAU", "CHARGER", "SAUVER", "SUPPR", "PRESETS", "RETOUR"]
        );

        // 2. SoftKey 3 = NOUVEAU projet
        eng.handle_event(EngineEvent::SoftKey(3)).unwrap();
        let snap = eng.snapshot();
        assert!(snap.current_project_path.is_some());
        let path = std::path::PathBuf::from(snap.current_project_path.as_ref().unwrap());
        assert!(path.exists(), "Le fichier projet doit être créé sur disque");

        // 3. Test de confirmation et suppression de projet depuis MK3
        eng.handle_event(EngineEvent::SoftKey(6)).unwrap();
        assert!(eng.snapshot().delete_project_confirm);
        assert_eq!(eng.soft_key_labels()[5], "CONFIRMER");
        eng.handle_event(EngineEvent::CancelDeleteProject).unwrap();
        assert!(!eng.snapshot().delete_project_confirm);
        assert_eq!(eng.soft_key_labels()[5], "SUPPR");

        // Le projet doit avoir un kit jouable (16 sons) et Pattern 1 vide
        assert_eq!(snap.project.groups.len(), 1);
        let g = &snap.project.groups[0];
        assert_eq!(g.name, "Kit 808");
        assert_eq!(g.sounds.iter().filter(|s| s.is_some()).count(), 16);
        assert_eq!(g.patterns[0].name, "Pattern 1");
        assert!(
            g.patterns[0].events.is_empty(),
            "Pattern 1 doit être vierge pour l'utilisateur"
        );

        // Message toast affiché
        assert!(snap.status_message.is_some());
        assert!(snap
            .status_message
            .as_ref()
            .unwrap()
            .starts_with("Nouveau projet:"));

        // Liste des projets mise à jour
        assert!(!eng.project_entries.is_empty());
        assert_eq!(
            eng.project_entries[eng.project_selected].path,
            path.to_string_lossy()
        );

        // 3. SoftKey 7 = PRESETS (bascule vers BROWSER sons)
        eng.handle_event(EngineEvent::SoftKey(7)).unwrap();
        assert_eq!(eng.browser_kind, 0);

        // 4. SoftKey 8 en FILE = RETOUR (referme vers vue principale tab 0)
        eng.browser_kind = 1;
        eng.handle_event(EngineEvent::SoftKey(8)).unwrap();
        assert_eq!(eng.left_tab, 0);

        // Nettoyage du fichier de test
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_filter_selection_and_cycling_via_soft_keys() {
        let mut eng = Engine::new_demo(48000);
        eng.left_tab = 2; // Sampler view
        eng.active_sound = 0;

        // Par défaut le filtre peut être LowPass ou Off
        eng.handle_event(EngineEvent::SetSoundFilterKind {
            group: 0,
            sound: 0,
            kind: crate::project::FilterKind::Off,
        })
        .unwrap();

        // 1. Cycle filtre : Off -> LowPass -> HighPass -> BandPass -> Off
        eng.handle_event(EngineEvent::SoftKey(4)).unwrap(); // SoftKey 4 = CYC FLT (idx 3)
        let f1 = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f1, crate::project::FilterKind::LowPass);

        eng.handle_event(EngineEvent::SoftKey(4)).unwrap();
        let f2 = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f2, crate::project::FilterKind::HighPass);

        // 2. Sélection directe via soft keys 5..8 (LP, HP, BP, OFF)
        eng.handle_event(EngineEvent::SoftKey(7)).unwrap(); // SoftKey 7 = BP (idx 6)
        let f3 = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f3, crate::project::FilterKind::BandPass);

        eng.handle_event(EngineEvent::SoftKey(8)).unwrap(); // SoftKey 8 = OFF (idx 7)
        let f4 = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f4, crate::project::FilterKind::Off);

        eng.handle_event(EngineEvent::SoftKey(5)).unwrap(); // SoftKey 5 = LP (idx 4)
        let f5 = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f5, crate::project::FilterKind::LowPass);
    }

    #[test]
    fn test_kick_synth_drive_and_filter_events() {
        let mut eng = Engine::new_demo(48000);
        // Charge le Kick Synth sur pad 0
        eng.load_plugin_to_sound(0, 0, 0).unwrap();
        assert!(eng.focus_sound_has_kick());

        // Cycle drive type
        eng.handle_event(EngineEvent::KickDriveTypeDelta(1))
            .unwrap();
        assert_eq!(
            eng.kick_synth.params.drive_type,
            crate::kick_synth::KickDriveType::HardClip
        );

        // Cycle filter mode (de LowPass -> HighPass)
        eng.handle_event(EngineEvent::KickFilterModeDelta(1))
            .unwrap();
        assert_eq!(
            eng.kick_synth.params.filter_mode,
            crate::kick_synth::KickFilterMode::HighPass
        );

        // Ajuste filter cutoff & reso
        eng.handle_event(EngineEvent::KickFilterCutoffDelta(5.0))
            .unwrap();
        assert!(eng.kick_synth.params.filter_cutoff_hz > 500.0);
        eng.handle_event(EngineEvent::KickFilterResoDelta(3.0))
            .unwrap();
        assert!(eng.kick_synth.params.filter_reso > 0.0);
    }

    #[test]
    fn test_nexus_synth_plugin_and_presets() {
        let mut eng = Engine::new_demo(48000);
        // Charge Nexus Wavetable Synth (plugin_idx 2) sur pad 3
        eng.load_plugin_to_sound(0, 3, 2).unwrap();
        eng.active_sound = 3;
        assert!(eng.focus_sound_has_wavetable());

        // Vérifie les presets
        let p_start = eng.wavetable_synth.params.name.clone();
        eng.handle_event(EngineEvent::WavetableNextPreset).unwrap();
        let p_next = eng.wavetable_synth.params.name.clone();
        assert_ne!(p_start, p_next);

        // Sélection directe preset
        eng.handle_event(EngineEvent::WavetableSetPreset(0))
            .unwrap();
        assert_eq!(eng.wavetable_synth.params.name, "Concert Grand Piano");

        // Rendu audio polyphonique
        let mut out = vec![0.0f32; 1024];
        eng.handle_event(EngineEvent::Pad {
            pad: 3,
            velocity: 110,
            pressed: true,
        })
        .unwrap();
        eng.render_audio(&mut out);
        let max_amp = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            max_amp > 0.01,
            "Le synthé wavetable doit produire du son audible"
        );
    }
}
