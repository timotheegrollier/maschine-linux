//! Mapper HID → MIDI / Engine.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use maschine_proto::color::{Color, PaletteIndex};
use maschine_proto::hid_output::LedState;

use maschine_proto::hid_input::{EncoderEvent, TouchStripEvent};
use maschine_proto::ButtonId;

use crate::events::{HardwareEvent, PadState};

/// Durée du flash blanc de frappe sur un pad (feedback tactile garanti même
/// pour un tap plus court qu'une frame LED 33ms), façon Maschine.
const PAD_FLASH_MS: u64 = 200;

/// Destinations de navigation GUI émises par les menus matériels.
/// La GUI utilise ces identifiants pour ouvrir la fenêtre correspondante.
pub mod nav {
    /// Vue séquenceur / pattern.
    pub const PATTERN: u8 = 0;
    /// Vue édition pas-à-pas.
    pub const STEP: u8 = 1;
    /// Vue clavier / performance.
    pub const KEYBOARD: u8 = 2;
    /// Vue scènes.
    pub const SCENE: u8 = 3;
    /// Vue plugin / édition des sons.
    pub const PLUGIN: u8 = 4;
}

/// Variante atténuée d'une couleur de pad pour l'état repos, façon Maschine :
/// les pads brillent en demi-teinte au repos, la frappe flashe blanc et les
/// pas programmés restent en couleur pleine.
#[must_use]
pub fn dim_pad_color(color: Color) -> Color {
    use PaletteIndex::*;
    let dim = match color.0 {
        Red => DimRed,
        Orange | Skin => DimOrange,
        Amber => DimAmber,
        Yellow => DimYellow,
        Spring => DimSpring,
        Green => DimGreen,
        Turquoise => DimTurquoise,
        Lime => DimLime,
        Sky => DimSky,
        Blue => DimBlue,
        Purple => DimPurple,
        Magenta => DimMagenta,
        Pink | Rose => DimPink,
        White => Grey,
        Grey => DarkGrey,
        DarkGrey => DarkGrey,
        other => other, // Off + variantes Dim déjà atténuées
    };
    Color(dim)
}

/// Config mapping (future: charge depuis TOML).
#[derive(Debug, Clone)]
pub struct MapperConfig {
    /// Note MIDI base pour pad 0 (défaut C1 = 36, comme Maschine)
    pub pad_base_note: u8,
    /// Channel MIDI pour pads (0..15)
    pub pad_channel: u8,
    /// CC base pour knobs 1..8
    pub knob_base_cc: u8,
    /// Velocity courbe
    pub velocity_curve: VelocityCurve,
}

impl Default for MapperConfig {
    fn default() -> Self {
        Self {
            pad_base_note: 36,
            pad_channel: 9, // 10 = drums en 1-indexed
            knob_base_cc: 14,
            velocity_curve: VelocityCurve::Linear,
        }
    }
}

/// Courbe vélocité.
#[derive(Debug, Clone, Copy)]
pub enum VelocityCurve {
    /// Linéaire
    Linear,
    /// Soft (log)
    Soft,
    /// Hard (exp)
    Hard,
}

/// Événement MIDI virtuel (pour ALSA Seq / JACK).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MidiEvent {
    /// Note on
    NoteOn {
        /// Channel 0..15
        channel: u8,
        /// Note 0..127
        note: u8,
        /// Vel 1..127
        velocity: u8,
    },
    /// Note off
    NoteOff {
        /// Channel
        channel: u8,
        /// Note
        note: u8,
        /// Vel off
        velocity: u8,
    },
    /// CC
    Cc {
        /// Channel
        channel: u8,
        /// CC number
        cc: u8,
        /// Value 0..127
        value: u8,
    },
    /// Transport
    Transport {
        /// Play
        play: bool,
        /// Rec
        rec: bool,
        /// Stop
        stop: bool,
    },
}

/// Mode de fonctionnement du contrôleur Maschine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerMode {
    /// Mode Drum Pads standard (jouer les 16 sons en direct)
    Pad,
    /// Mode Clavier mélodique (les 16 pads jouent des notes de gamme sur le son actif)
    Keyboard,
    /// Mode Accords (les 16 pads jouent des accords sur le son actif)
    Chords,
    /// Mode Séquenceur pas-à-pas (les 16 pads représentent les 16 pas du son sélectionné)
    Step,
}

/// Cible assignée à l'encodeur 4D principal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderFocus {
    /// Réglage du tempo (BPM)
    Tempo,
    /// Réglage du volume général (Master Volume)
    Volume,
    /// Réglage du swing
    Swing,
}

/// Mode assigné au Smart Strip (Touchstrip)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchStripMode {
    /// Pitch Bend bipolaire avec retour au centre
    Pitch,
    /// Modulation (CC 1)
    Mod,
    /// Macro Performance / Filtre (CC 74)
    Perform,
    /// Notes / Résonance (CC 71)
    Notes,
}

/// Différence relative 4-bit (-8..+7) pour l'encodeur 4D sans dérive
#[must_use]
pub fn wrap_16_delta(newer: u8, older: u8) -> i8 {
    let diff = (newer as i8 - older as i8) & 0x0F;
    if diff > 7 {
        diff - 16
    } else {
        diff
    }
}

/// Différence relative 10-bit (-512..+511) pour les 8 potentiomètres d'écran
#[must_use]
pub fn wrap_1024_delta(newer: u16, older: u16) -> i32 {
    let diff = newer as i32 - older as i32;
    if diff > 512 {
        diff - 1024
    } else if diff < -512 {
        diff + 1024
    } else {
        diff
    }
}

/// Actions sémantiques produites par le hardware
#[derive(Debug, Clone, PartialEq)]
pub enum MapperAction {
    /// Événement MIDI virtuel
    Midi(MidiEvent),
    /// Déclenchement d'un son (pad 0..15)
    PadTrigger {
        pad: u8,
        velocity: u8,
        pressed: bool,
    },
    /// Déclenchement mélodique du son actif (Keyboard/Chords).
    KeyboardTrigger {
        note: u8,
        velocity: u8,
        pressed: bool,
    },
    /// Déclenche une scène (mode SCENE, pads 1..16 -> scènes 0..15)
    SceneTrigger(usize),
    /// Sélectionne un pattern du groupe focus pour la scène active (mode PATTERN)
    PatternTrigger { group: u8, pattern: usize },
    /// Duplique le pattern actif du groupe focus (bouton DUPLICATE en mode PATTERN)
    DuplicatePattern { group: u8 },
    /// Efface les notes du pattern actif (SHIFT + ERASE en mode PATTERN)
    ClearPattern { group: u8 },
    /// Pattern précédent/suivant (4D en mode PATTERN)
    PatternStep { group: u8, delta: i32 },
    /// Efface toutes les notes d'un son dans le pattern actif (mode ERASE + pad)
    EraseSound { group: u8, sound: u8 },
    /// Basculement d'un pas du séquenceur (son 0..15, mesure 0..1, pas 0..15)
    ToggleStep { sound: u8, bar: usize, step: usize },
    /// Lecture / Pause
    PlayPauseToggle,
    /// Stop transport
    Stop,
    /// Rec transport
    RecordToggle,
    /// Rembobinage à zéro
    Restart,
    /// Ajustement BPM relatif (+/- delta)
    BpmDelta(f32),
    /// Nouveau BPM absolu (Tap Tempo)
    BpmSet(f32),
    /// Ajustement Swing relatif
    SwingDelta(f32),
    /// Ajustement Volume Master relatif
    MasterVolumeDelta(f32),
    /// Ajustement volume casque relatif
    HeadphoneVolumeDelta(f32),
    /// Ajustement Volume Groupe relatif
    GroupVolumeDelta(f32),
    /// Ajustement Pan Groupe relatif
    GroupPanDelta(f32),
    /// Ajustement Volume Sound relatif (pour le pad actif)
    SoundVolumeDelta(f32),
    /// Ajustement Pan Sound relatif
    SoundPanDelta(f32),
    /// Ajustement Cutoff filtre Sound relatif
    SoundFilterCutoffDelta(f32),
    /// Ajustement Résonance filtre Sound relatif
    SoundFilterResoDelta(f32),
    /// Ajustement Attack ADSR relatif (ms)
    SoundAttackDelta(f32),
    /// Ajustement Decay ADSR relatif (ms)
    SoundDecayDelta(f32),
    /// Ajustement Sustain ADSR relatif (0..1)
    SoundSustainDelta(f32),
    /// Ajustement Release ADSR relatif (ms)
    SoundReleaseDelta(f32),
    /// Ajustement Volume d'un groupe précis (mode MIXER, knobs 1..8)
    GroupVolumeDeltaFor {
        /// Groupe 0..7
        group: u8,
        /// Delta relatif
        delta: f32,
    },
    /// Ajustement Volume d'un son/pad précis (mode MIXER pad, knobs 1..8)
    SoundVolumeDeltaFor { group: u8, sound: u8, delta: f32 },
    /// Ajustement Pan d'un son/pad précis (mode MIXER pad + shift)
    SoundPanDeltaFor { group: u8, sound: u8, delta: f32 },
    /// Ajustement Tune Kick Synth
    KickTuneDelta(f32),
    /// Ajustement Decay Kick Synth
    KickDecayDelta(f32),
    /// Ajustement Snap / Punch Kick Synth
    KickSnapDelta(f32),
    /// Ajustement Sweep time Kick Synth
    KickSweepDelta(f32),
    /// Ajustement Click Kick Synth
    KickClickDelta(f32),
    /// Ajustement Drive Kick Synth
    KickDriveDelta(f32),
    /// Ajustement Sub Kick Synth
    KickSubDelta(f32),
    /// Ajustement Volume Kick Synth
    KickVolumeDelta(f32),
    /// Mode STEP : vélocité du pas sélectionné (delta entier)
    StepVelocityDelta(i32),
    /// Mode STEP : position (nudge) du pas sélectionné (delta en ticks)
    StepNudgeTicks(i32),
    /// Mode STEP : longueur du pas sélectionné (delta en pas de 1/16)
    StepLengthDelta(i32),
    /// Mode STEP : pitch du pas sélectionné (delta en demi-tons)
    StepPitchDelta(i32),
    /// Mode STEP : défile la grille du pattern (delta en pas, +/-)
    StepScroll(i32),
    /// Navigation browser : pas de son (+/-1 par cran)
    BrowseSoundStep(i32),
    /// Navigation browser : pas de preset drum (+/-1 par cran)
    BrowsePresetStep(i32),
    /// Charger le preset sélectionné sur le pad actif
    LoadSelectedPreset,
    /// Navigation browser : pas de groupe
    BrowseGroupStep(i32),
    /// Changement de groupe (A..H, 0..7)
    GroupFocus(u8),
    /// Sélection de mesure (0-indexed : 0, 1, 2, 3...)
    SelectBar(usize),
    /// Définir le nombre de mesures du pattern actif (1, 2, 4, 8)
    SetPatternBars(usize),
    /// Sélection du son actif pour le mode Step (0..15)
    SelectSound(u8),
    /// Bascule Mute sur groupe actif
    ToggleMute(u8),
    /// Bascule mute d'un pad/son (groupe focus).
    TogglePadMute { group: u8, pad: u8 },
    /// Bascule Solo sur groupe actif
    ToggleSolo(u8),
    /// Effacer la mesure courante
    ClearBar(usize),
    /// Dupliquer mesure 1 vers mesure 2
    DuplicateBar,
    /// Changement de mode de contrôle
    ModeSwitch(ControllerMode),
    /// Réinitialiser le groove de démo 808
    ResetDemoGroove,
    /// Définir le tab actif écran gauche (0..3)
    SetLeftTab(u8),
    /// Définir le tab actif écran droit (0..3)
    SetRightTab(u8),
    /// Ouvrir une fenêtre GUI (voir le module `nav`).
    NavigateView(u8),
    /// Choisir la page de l'éditeur PLUGIN (0: Sound, 1: TB-303).
    SetPluginPage(u8),
    /// Humanisation / Variation
    Variation,
    /// Basculer mode Song / Arranger
    ToggleSongMode,
    /// Quantification
    Quantize,
    /// Sauvegarder le projet
    SaveProject,
    /// Sauvegarde rapide du projet courant (Shift+FILE SAVE).
    QuickSaveProject,
    /// Ouvre le navigateur FILE/projets sur la MK3.
    OpenFileBrowser,
    /// Crée un nouveau projet (depuis le navigateur FILE).
    NewProject,
    /// Type de navigateur BROWSER (0: presets, 1: projets FILE).
    SetBrowserKind(u8),
    /// Miroir écran : mode SCENE
    SetSceneMode(bool),
    /// Miroir écran : mode PATTERN
    SetPatternMode(bool),
    /// Miroir écran : mode SELECT
    SetSelectMode(bool),
    /// Miroir écran : mode ERASE
    SetEraseMode(bool),
    /// Miroir écran : mode de jeu 0..3
    SetControllerMode(u8),
    /// Bascule la boucle transport
    ToggleLoop,
    /// Bascule le métronome
    ToggleMetronome,
    /// Soft key d'écran (1..8) — action contextuelle décidée par l'Engine
    /// (les labels affichés sous les écrans correspondent exactement).
    SoftKey(u8),
    // ─── Acid Synth (tab 4) ───────────────────────────────────
    /// Knob 1 : Cutoff filtre acid
    AcidCutoffDelta(f32),
    /// Knob 2 : Résonance filtre acid
    AcidResoDelta(f32),
    /// Knob 3 : Modulation enveloppe acid
    AcidEnvModDelta(f32),
    /// Knob 4 : Decay enveloppe acid
    AcidDecayDelta(f32),
    /// Knob 5 : Accent acid
    AcidAccentDelta(f32),
    /// Knob 6 : Drive distortion acid
    AcidDriveDelta(f32),
    /// Knob 7 : Temps delay acid
    AcidDelayTimeDelta(f32),
    /// Knob 8 : Mix delay acid
    AcidDelayMixDelta(f32),
    /// Mode mixer (0: Pads 1-8, 1: Pads 9-16, 2: Groupes A-H, 3: Master)
    SetMixerMode(u8),
    /// Alterne le type de filtre du son actif
    CycleSoundFilterKind,
    /// Kick Synth : coupure filtre
    KickFilterCutoffDelta(f32),
    /// Kick Synth : résonance filtre
    KickFilterResoDelta(f32),
    /// Kick Synth : cycle type de distorsion
    KickDriveTypeDelta(i32),
    /// Kick Synth : cycle mode de filtre
    KickFilterModeDelta(i32),
    // ─── Wavetable / Nexus Synth (tab 4) ──────────────────────
    /// Knob 1 : Cutoff filtre Wavetable
    WavetableCutoffDelta(f32),
    /// Knob 2 : Résonance filtre Wavetable
    WavetableResoDelta(f32),
    /// Knob 3 : Modulation enveloppe filtre Wavetable
    WavetableEnvAmtDelta(f32),
    /// Knob 4 : Drive filtre Wavetable
    WavetableDriveDelta(f32),
    /// Knob 5 : Attack enveloppe amplitude Wavetable
    WavetableAttackDelta(f32),
    /// Knob 6 : Decay enveloppe amplitude Wavetable
    WavetableDecayDelta(f32),
    /// Knob 7 : Sustain enveloppe amplitude Wavetable
    WavetableSustainDelta(f32),
    /// Knob 8 : Release enveloppe amplitude Wavetable
    WavetableReleaseDelta(f32),
    /// Shift + Knob 1 : Cycle mode de filtre Wavetable
    WavetableFilterModeCycle,
    /// Shift + Knob 2 : Delta voix d'unisson Wavetable
    WavetableUnisonVoicesDelta(i32),
    /// Shift + Knob 3 : Cycle forme d'onde OSC 1 Wavetable
    WavetableOsc1WaveCycle,
    /// Shift + Knob 4 : Cycle forme d'onde OSC 2 Wavetable
    WavetableOsc2WaveCycle,
    /// Shift + Knob 5 : Mix chorus Wavetable
    WavetableChorusDelta(f32),
    /// Shift + Knob 6 : Mix delay Wavetable
    WavetableDelayDelta(f32),
    /// Shift + Knob 7 : Mix reverb Wavetable
    WavetableReverbDelta(f32),
    /// Shift + Knob 8 : Volume Wavetable (dB)
    WavetableVolumeDelta(f32),
    /// Preset Wavetable suivant
    WavetableNextPreset,
    /// Preset Wavetable précédent
    WavetablePrevPreset,
    // ─── Note Repeat ──────────────────────────────────────────
    /// Active / désactive le Note Repeat
    SetNoteRepeat(bool),
    /// Division temporelle du Note Repeat (0..7)
    SetNoteRepeatRate(u8),
    /// Enchaîne plusieurs patterns
    PatternChain { group: u8, patterns: Vec<usize> },
    /// Bouton SHIFT physique enfoncé / relâché
    SetShiftHeld(bool),
    /// Verrou LOCK activé / désactivé
    ToggleLock(bool),
}

/// Données du séquenceur pour mettre à jour les LEDs du contrôleur physique
#[derive(Debug, Clone, Default)]
pub struct SequencerLedInfo {
    pub is_playing: bool,
    pub is_recording: bool,
    pub is_stopped: bool,
    pub current_bar: usize,
    pub current_step: usize,
    pub is_quarter_beat: bool,
    pub active_steps: [bool; 16],
    pub note_repeat_enabled: bool,
    pub lock_active: bool,
    pub shift_held: bool,
    /// Pas sélectionné (curseur d'édition step sequencer, 0..pattern_bars*16-1)
    pub step_cursor: usize,
    pub triggered_pads: [bool; 16],
    pub master_vol: f32,
    pub muted_groups: u8,
    pub muted_sounds: [u16; 8],
    pub soloed_groups: u8,
    pub left_tab: u8,
    pub right_tab: u8,
    /// Type de navigateur (0: presets 808, 1: projets FILE, 2: plugins, 3: effets)
    pub browser_kind: u8,
    /// Mode mixer (0: Pads 1-8, 1: Pads 9-16, 2: Groupes A-H, 3: Master)
    pub mixer_mode: u8,
    /// Son actif possède un Kick Synth
    pub has_kick_synth: bool,
    /// Son actif possède un TB-303
    pub has_acid_synth: bool,
    /// Son actif possède un Wavetable / Nexus Synth
    pub has_wavetable_synth: bool,
    /// Confirmation de suppression de projet
    pub delete_project_confirm: bool,
    /// Mode SCENE actif (pads déclenchent les scènes)
    pub scene_mode: bool,
    /// Mode PATTERN actif (pads sélectionnent le pattern)
    pub pattern_mode: bool,
    /// Mode SELECT actif (pads sélectionnent sans jouer)
    pub select_mode: bool,
    /// Mode ERASE actif (pads gomment)
    pub erase_mode: bool,
    /// Son actif 0..15
    pub active_sound: u8,
    /// Scène active
    pub active_scene: Option<usize>,
    /// Nombre de scènes du projet
    pub num_scenes: usize,
    /// Nombre de patterns du groupe focus
    pub group_num_patterns: usize,
    /// Pattern assigné au groupe focus dans la scène active
    pub scene_pattern: Option<usize>,
    /// Boucle transport active (LED RESTART)
    pub loop_enabled: bool,
    /// Métronome actif (LED TAP quand stoppé)
    pub metronome: bool,
    /// Mesure sélectionnée pour l'affichage / édition pas-à-pas (0-indexed : 0, 1, 2, 3...)
    pub selected_bar: usize,
    /// Nombre de mesures du pattern actif (1, 2, 4, 8)
    pub pattern_bars: usize,
}

/// Mapper — convertit `HardwareEvent` → `MidiEvent` + `LedState` + actions Engine.
pub struct HardwareMapper {
    cfg: MapperConfig,
    /// dernier pad velocity pour note off
    pad_note: HashMap<u8, u8>,
    /// état LEDs à envoyer
    led_state: LedState,
    /// Group focus 0..7
    pub group_focus: u8,
    /// Pad colors par Sound (16)
    pad_colors: [Color; 16],
    /// Mode sélection pattern
    pub pattern_select_mode: bool,
    /// Mode sélection scene (pads déclenchent les scènes, façon Maschine)
    pub scene_select_mode: bool,
    /// Mode sélection (pads sélectionnent le son sans le jouer)
    pub select_mode: bool,
    /// Mode effacement
    pub erase_mode: bool,
    /// Mode actif (Pad, Keyboard, Chords, Step)
    pub mode: ControllerMode,
    /// Focus encodeur 4D (Tempo, Volume, Swing)
    pub encoder_focus: EncoderFocus,
    /// Mode Smart Strip (Pitch, Mod, Perform, Notes)
    pub touchstrip_mode: TouchStripMode,
    /// Vélocité fixe (127) activée
    pub fixed_velocity: bool,
    /// Suivi du playhead activé
    pub follow_mode: bool,
    /// Mode note repeat activé
    pub note_repeat: bool,
    /// Division temporelle du note repeat (0: 1/32, 1: 1/16, etc.)
    pub note_repeat_rate: u8,
    /// Onglet écran gauche (0..3)
    pub left_tab: u8,
    /// Onglet écran droit (0..3)
    pub right_tab: u8,
    /// Page de l'éditeur PLUGIN (0: Sound, 1: TB-303)
    pub plugin_page: u8,
    /// Son actif sélectionné pour édition Step/Keyboard (0..15)
    pub active_sound: u8,
    /// Preset sélectionné dans le Browser (0..15)
    pub browser_preset_idx: u8,
    /// Type de navigateur (0: presets 808, 1: projets FILE, 2: plugins, 3: effets)
    pub browser_kind: u8,
    /// Mode mixer (0: Pads 1-8, 1: Pads 9-16, 2: Groupes A-H, 3: Master)
    pub mixer_mode: u8,
    /// Son actif possède un Kick Synth
    pub has_kick_synth: bool,
    /// Son actif possède un TB-303
    pub has_acid_synth: bool,
    /// Son actif possède un Wavetable / Nexus Synth
    pub has_wavetable_synth: bool,
    /// Confirmation de suppression de projet
    pub delete_project_confirm: bool,
    /// État musical Keyboard/Chords (miroir moteur).
    pub kbd_root: u8,
    pub kbd_octave: i8,
    pub kbd_scale: u8,
    pub kbd_chord: u8,
    pub kbd_inversion: u8,
    /// Notes réellement jouées par pad en Keyboard/Chords (pour note-off).
    pad_played: HashMap<u8, Vec<u8>>,
    /// Mesure sélectionnée (0-indexed)
    pub selected_bar: usize,
    /// Nombre de mesures du pattern
    pub pattern_bars: usize,
    /// Dernière valeur de l'encodeur 4D
    pub prev_main_encoder: Option<u8>,
    /// Dernières valeurs des 8 knobs d'écran
    pub prev_knobs: [Option<u16>; 8],
    /// Accumulateurs fractionnaires des knobs (valeurs entières : pas, pitch...)
    knob_carry: [f32; 8],
    /// Historique des taps tempo
    tap_timestamps: Vec<std::time::Instant>,
    /// Bouton Shift enfoncé
    pub shift_held: bool,
    /// Bouton Mute tenu (MUTE + Group = mute groupe)
    pub mute_held: bool,
    /// Mode MUTE latched : les pads togglent le mute du son (façon Maschine).
    pub mute_pad_mode: bool,
    /// Bouton Solo tenu
    pub solo_held: bool,
    /// Un Group a été touché pendant Mute/Solo tenu (pour éviter le double toggle au relâchement)
    mute_solo_group_used: bool,
    /// Bouton Tempo enfoncé
    pub tempo_button_held: bool,
    /// Bouton Select tenu (SELECT + Pad = sélectionne sans jouer)
    pub select_held: bool,
    /// Un Pad a été touché pendant Select tenu
    select_used: bool,
    /// Mode Select épinglé
    select_pinned: bool,
    /// Bouton Erase tenu
    pub erase_held: bool,
    /// Un Pad a été touché pendant Erase tenu
    erase_used: bool,
    /// Mode Erase épinglé
    erase_pinned: bool,
    /// État d'appui physique des 16 pads
    pub pad_pressed: [bool; 16],
    /// Instant de la dernière frappe par pad (flash tactile temporisé :
    /// garantit un feedback visible même pour un tap éclair)
    pad_hit_at: [Option<Instant>; 16],
    /// Smart strip actuellement touché
    pub strip_touched: bool,
    /// Dernière position sur le smart strip (0..1023)
    pub strip_pos: u16,
    /// Verrou LOCK actif
    pub lock_active: bool,
    /// Bouton Duplicate enfoncé
    pub duplicate_held: bool,
}

impl HardwareMapper {
    /// Crée avec config.
    #[must_use]
    pub fn new(cfg: MapperConfig) -> Self {
        let mut pad_colors = [Color::OFF; 16];
        // palette par défaut arc-en-ciel par pad
        let palette = [
            6, 22, 30, 42, 50, 58, 10, 14, 26, 31, 34, 38, 11, 27, 33, 78,
        ];
        for (i, &idx) in palette.iter().enumerate() {
            pad_colors[i] = Color::from_index(idx).unwrap_or(Color::OFF);
        }
        let mut led_state = LedState::new();
        for (i, &c) in pad_colors.iter().enumerate() {
            led_state.set_pad(i, c);
        }
        Self {
            cfg,
            pad_note: HashMap::new(),
            led_state,
            group_focus: 0,
            pad_colors,
            pattern_select_mode: false,
            scene_select_mode: false,
            select_mode: false,
            erase_mode: false,
            mode: ControllerMode::Pad,
            encoder_focus: EncoderFocus::Tempo,
            touchstrip_mode: TouchStripMode::Perform,
            fixed_velocity: false,
            follow_mode: true,
            note_repeat: false,
            note_repeat_rate: 1,
            left_tab: 0,
            right_tab: 0,
            plugin_page: 0,
            active_sound: 0,
            browser_preset_idx: 0,
            browser_kind: 0,
            mixer_mode: 0,
            has_kick_synth: false,
            has_acid_synth: false,
            has_wavetable_synth: false,
            delete_project_confirm: false,
            kbd_root: 0,
            kbd_octave: 3,
            kbd_scale: 2,
            kbd_chord: 0,
            kbd_inversion: 0,
            pad_played: HashMap::new(),
            selected_bar: 0,
            pattern_bars: 2,
            prev_main_encoder: None,
            prev_knobs: [None; 8],
            knob_carry: [0.0; 8],
            tap_timestamps: Vec::new(),
            shift_held: false,
            mute_held: false,
            mute_pad_mode: false,
            solo_held: false,
            mute_solo_group_used: false,
            tempo_button_held: false,
            select_held: false,
            select_used: false,
            select_pinned: false,
            erase_held: false,
            erase_used: false,
            erase_pinned: false,
            pad_pressed: [false; 16],
            pad_hit_at: [None; 16],
            strip_touched: false,
            strip_pos: 512,
            lock_active: false,
            duplicate_held: false,
        }
    }

    /// Bascule le mode sélection pattern.
    pub fn toggle_pattern_select(&mut self) {
        self.pattern_select_mode = !self.pattern_select_mode;
    }

    /// Bascule le mode effacement.
    pub fn toggle_erase_mode(&mut self) {
        self.erase_mode = !self.erase_mode;
    }

    /// Défaut.
    #[must_use]
    pub fn with_default() -> Self {
        Self::new(MapperConfig::default())
    }

    /// Sélectionne une vue principale et revient à l'écran droit contextuel.
    fn select_left_tab(&mut self, tab: u8, actions: &mut Vec<MapperAction>) {
        self.left_tab = tab;
        self.right_tab = 0;
        actions.push(MapperAction::SetLeftTab(tab));
        // Les anciens overlays (EVENTS/MACRO/SETTINGS) ne doivent pas rester
        // affichés au-dessus de la nouvelle vue.
        if !actions.contains(&MapperAction::SetRightTab(0)) {
            actions.push(MapperAction::SetRightTab(0));
        }
    }

    /// Change le mode de jeu et reflète l'état sur les LEDs/écrans.
    fn set_controller_mode(&mut self, mode: ControllerMode, actions: &mut Vec<MapperAction>) {
        let code = match mode {
            ControllerMode::Pad => 0,
            ControllerMode::Keyboard => 1,
            ControllerMode::Chords => 2,
            ControllerMode::Step => 3,
        };
        if self.mode != mode {
            self.mode = mode;
            actions.push(MapperAction::ModeSwitch(mode));
        }
        actions.push(MapperAction::SetControllerMode(code));
    }

    /// Désactive tous les menus « latchés » (PATTERN, SCENE, SELECT, ERASE, MUTE)
    /// et referme les overlays (EVENTS, MACRO, SETTINGS).
    fn clear_menu_modes(&mut self, actions: &mut Vec<MapperAction>) {
        self.pad_pressed = [false; 16];
        if self.pattern_select_mode {
            self.pattern_select_mode = false;
            self.led_state
                .set_button_raw(maschine_proto::hid_output::offsets::PATTERN, 1);
            actions.push(MapperAction::SetPatternMode(false));
        }
        if self.scene_select_mode {
            self.scene_select_mode = false;
            self.led_state
                .set_button_raw(maschine_proto::hid_output::offsets::SCENE, 1);
            actions.push(MapperAction::SetSceneMode(false));
        }
        if self.select_mode {
            self.select_mode = false;
            self.select_pinned = false;
            self.select_held = false;
            self.led_state
                .set_button_raw(maschine_proto::hid_output::offsets::SELECT, 1);
            actions.push(MapperAction::SetSelectMode(false));
        }
        if self.erase_mode {
            self.erase_mode = false;
            self.erase_pinned = false;
            self.erase_held = false;
            self.led_state
                .set_button_raw(maschine_proto::hid_output::offsets::ERASE, 1);
            actions.push(MapperAction::SetEraseMode(false));
        }
        if self.mute_pad_mode {
            self.mute_pad_mode = false;
            self.led_state
                .set_button_raw(maschine_proto::hid_output::offsets::MUTE, 1);
        }
        if self.right_tab != 0 {
            self.right_tab = 0;
            self.led_state
                .set_button_raw(maschine_proto::hid_output::offsets::EVENTS, 1);
            self.led_state
                .set_button_raw(maschine_proto::hid_output::offsets::MACRO, 1);
            self.led_state
                .set_button_raw(maschine_proto::hid_output::offsets::SETTINGS, 1);
            actions.push(MapperAction::SetRightTab(0));
        }
    }

    /// Revient au comportement par défaut : pads performance + vue séquenceur.
    fn exit_to_default(&mut self, actions: &mut Vec<MapperAction>) {
        self.clear_menu_modes(actions);
        self.select_left_tab(0, actions);
        self.set_controller_mode(ControllerMode::Pad, actions);
        actions.push(MapperAction::NavigateView(nav::PATTERN));
    }

    /// Traite un événement matériel complet et retourne la liste des actions à appliquer à l'engine.
    /// Accumule un delta de knob (ticks bruts) et renvoie la part entière
    /// écoulée. `rate` = unités par tick brut (1024 ticks = 1 tour de knob).
    fn knob_steps(&mut self, idx: usize, delta: i32, rate: f32) -> i32 {
        self.knob_carry[idx] += delta as f32 * rate;
        let whole = self.knob_carry[idx].trunc();
        self.knob_carry[idx] -= whole;
        whole as i32
    }

    fn kbd_base_note(&self) -> i16 {
        (self.kbd_octave.max(0) as i16) * 12 + (self.kbd_root.min(11) as i16)
    }

    fn kbd_scale_intervals(&self) -> &'static [u8] {
        const S: [&[u8]; 8] = [
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            &[0, 2, 4, 5, 7, 9, 11],
            &[0, 2, 3, 5, 7, 8, 10],
            &[0, 2, 3, 5, 7, 8, 11],
            &[0, 2, 3, 5, 7, 9, 10],
            &[0, 2, 4, 5, 7, 9, 10],
            &[0, 3, 5, 7, 10],
            &[0, 2, 4, 7, 9],
        ];
        S[self.kbd_scale as usize % S.len()]
    }

    fn kbd_chord_intervals(&self) -> &'static [u8] {
        const C: [&[u8]; 8] = [
            &[0, 4, 7],
            &[0, 3, 7],
            &[0, 2, 7],
            &[0, 5, 7],
            &[0, 4, 7, 10],
            &[0, 3, 7, 10],
            &[0, 4, 7, 11],
            &[0, 12],
        ];
        C[self.kbd_chord as usize % C.len()]
    }

    fn keyboard_note_for_pad(&self, pad: u8) -> u8 {
        let scale = self.kbd_scale_intervals();
        let degree = pad as i16;
        let len = scale.len() as i16;
        let octave_shift = degree.div_euclid(len);
        let idx = degree.rem_euclid(len) as usize;
        let note = self.kbd_base_note() + octave_shift * 12 + scale[idx] as i16;
        note.clamp(0, 127) as u8
    }

    fn chord_notes_for_pad(&self, pad: u8) -> Vec<u8> {
        let scale = self.kbd_scale_intervals();
        let len = scale.len() as i16;
        let degree = (pad as i16).rem_euclid(len) as usize;
        let root = self.kbd_base_note() + scale[degree] as i16;
        let chord = self.kbd_chord_intervals();
        let mut notes: Vec<i16> = chord.iter().map(|&i| root + i as i16).collect();
        let inv = (self.kbd_inversion as usize).min(notes.len().saturating_sub(1));
        for i in 0..inv {
            if let Some(n) = notes.get_mut(i) {
                *n += 12;
            }
        }
        notes.sort_unstable();
        notes.into_iter().map(|n| n.clamp(0, 127) as u8).collect()
    }

    pub fn handle_hardware_event(&mut self, ev: HardwareEvent) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        match ev {
            HardwareEvent::Pad(p) => {
                actions.extend(self.handle_pad_event(p));
            }
            HardwareEvent::Button { id, pressed, .. } => {
                actions.extend(self.handle_button_event(id, pressed));
            }
            HardwareEvent::Encoder(enc) => {
                actions.extend(self.handle_encoder_event(enc));
            }
            HardwareEvent::TouchStrip(ts) => {
                actions.extend(self.handle_touch_strip_event(ts));
            }
            _ => {}
        }
        actions
    }

    fn handle_modifier_button(
        &mut self,
        id: ButtonId,
        pressed: bool,
        actions: &mut Vec<MapperAction>,
    ) -> bool {
        match id {
            ButtonId::Shift => {
                self.shift_held = pressed;
                self.led_state.set_button_raw(
                    maschine_proto::hid_output::offsets::SHIFT,
                    if pressed { 7 } else { 0 },
                );
                actions.push(MapperAction::SetShiftHeld(pressed));
                true
            }
            ButtonId::Duplicate => {
                self.duplicate_held = pressed;
                false
            }
            ButtonId::Tempo => {
                self.tempo_button_held = pressed;
                false
            }
            ButtonId::Mute => {
                self.mute_held = pressed;
                if pressed {
                    self.mute_solo_group_used = false;
                } else if !self.mute_solo_group_used {
                    // Appui simple : bascule le mode MUTE pads (pads sélectionnés = muted, LED dim).
                    self.mute_pad_mode = !self.mute_pad_mode;
                    self.led_state.set_button_raw(
                        maschine_proto::hid_output::offsets::MUTE,
                        if self.mute_pad_mode { 7 } else { 1 },
                    );
                }
                true
            }
            ButtonId::Solo => {
                self.solo_held = pressed;
                if pressed {
                    self.mute_solo_group_used = false;
                } else if !self.mute_solo_group_used {
                    actions.push(MapperAction::ToggleSolo(self.group_focus));
                }
                true
            }
            ButtonId::Select => {
                self.select_held = pressed;
                if pressed {
                    self.select_used = false;
                    self.select_mode = true;
                    actions.push(MapperAction::SetSelectMode(true));
                } else if self.select_mode {
                    // `select_mode` a pu être désactivé par un autre
                    // menu (PATTERN, SCENE…) : ne pas le rallumer au
                    // relâchement du bouton.
                    if self.select_used {
                        self.select_pinned = false;
                        self.select_mode = false;
                        actions.push(MapperAction::SetSelectMode(false));
                    } else {
                        self.select_pinned = !self.select_pinned;
                        self.select_mode = self.select_pinned;
                        actions.push(MapperAction::SetSelectMode(self.select_mode));
                    }
                }
                true
            }
            ButtonId::Erase => {
                self.erase_held = pressed;
                if pressed && self.left_tab == 3 && self.browser_kind == 1 {
                    // En mode FILE, ERASE déclenche / confirme la suppression de projet
                    actions.push(MapperAction::SoftKey(6));
                    return true;
                }
                if pressed {
                    if self.shift_held {
                        if self.pattern_select_mode {
                            actions.push(MapperAction::ClearPattern {
                                group: self.group_focus,
                            });
                        } else {
                            actions.push(MapperAction::ClearBar(self.selected_bar));
                        }
                    } else {
                        self.erase_used = false;
                        self.erase_mode = true;
                        actions.push(MapperAction::SetEraseMode(true));
                    }
                } else if !self.shift_held && self.erase_mode {
                    if self.erase_used {
                        self.erase_pinned = false;
                        self.erase_mode = false;
                        actions.push(MapperAction::SetEraseMode(false));
                    } else {
                        self.erase_pinned = !self.erase_pinned;
                        self.erase_mode = self.erase_pinned;
                        actions.push(MapperAction::SetEraseMode(self.erase_mode));
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn handle_transport_button(&mut self, id: ButtonId, actions: &mut Vec<MapperAction>) -> bool {
        match id {
            ButtonId::Play => {
                actions.push(MapperAction::PlayPauseToggle);
                true
            }
            ButtonId::Stop => {
                actions.push(MapperAction::Stop);
                true
            }
            ButtonId::Rec => {
                actions.push(MapperAction::RecordToggle);
                true
            }
            ButtonId::Restart => {
                if self.shift_held {
                    actions.push(MapperAction::ToggleLoop);
                } else {
                    actions.push(MapperAction::Restart);
                }
                true
            }
            ButtonId::Tap => {
                if self.shift_held {
                    actions.push(MapperAction::ToggleMetronome);
                    return true;
                }
                let now = std::time::Instant::now();
                self.tap_timestamps
                    .retain(|t| now.duration_since(*t).as_secs_f32() < 2.0);
                self.tap_timestamps.push(now);
                if self.tap_timestamps.len() >= 2 {
                    let diffs: Vec<f32> = self
                        .tap_timestamps
                        .windows(2)
                        .map(|w| w[1].duration_since(w[0]).as_secs_f32())
                        .collect();
                    let avg = diffs.iter().sum::<f32>() / diffs.len() as f32;
                    if avg > 0.15 && avg < 2.0 {
                        let bpm = (60.0 / avg).clamp(40.0, 300.0);
                        actions.push(MapperAction::BpmSet(bpm));
                    }
                }
                true
            }
            ButtonId::Pedal => {
                actions.push(MapperAction::PlayPauseToggle);
                true
            }
            _ => false,
        }
    }

    fn handle_view_menu_button(&mut self, id: ButtonId, actions: &mut Vec<MapperAction>) -> bool {
        match id {
            // Modes de contrôle — comportement Maschine : le bouton
            // active le menu, un second appui sur le même bouton
            // revient au mode Pad par défaut.
            ButtonId::PadMode => {
                self.exit_to_default(actions);
                true
            }
            ButtonId::Keyboard => {
                if self.mode == ControllerMode::Keyboard {
                    self.exit_to_default(actions);
                } else {
                    self.clear_menu_modes(actions);
                    self.select_left_tab(0, actions);
                    self.set_controller_mode(ControllerMode::Keyboard, actions);
                    actions.push(MapperAction::NavigateView(nav::KEYBOARD));
                }
                true
            }
            ButtonId::Chords => {
                if self.mode == ControllerMode::Chords {
                    self.exit_to_default(actions);
                } else {
                    self.clear_menu_modes(actions);
                    self.select_left_tab(0, actions);
                    self.set_controller_mode(ControllerMode::Chords, actions);
                    actions.push(MapperAction::NavigateView(nav::KEYBOARD));
                }
                true
            }
            ButtonId::Step => {
                if self.mode == ControllerMode::Step {
                    self.exit_to_default(actions);
                } else {
                    self.clear_menu_modes(actions);
                    self.select_left_tab(0, actions);
                    self.set_controller_mode(ControllerMode::Step, actions);
                    actions.push(MapperAction::NavigateView(nav::STEP));
                }
                true
            }

            // Focus Encodeur 4D
            ButtonId::Tempo => {
                self.encoder_focus = EncoderFocus::Tempo;
                true
            }
            ButtonId::Volume => {
                self.encoder_focus = EncoderFocus::Volume;
                true
            }
            ButtonId::Swing => {
                self.encoder_focus = EncoderFocus::Swing;
                true
            }

            // Modes Smart Strip
            ButtonId::Pitch => {
                self.touchstrip_mode = TouchStripMode::Pitch;
                true
            }
            ButtonId::Mod => {
                self.touchstrip_mode = TouchStripMode::Mod;
                true
            }
            ButtonId::Perform => {
                self.touchstrip_mode = TouchStripMode::Perform;
                true
            }
            ButtonId::Notes => {
                self.touchstrip_mode = TouchStripMode::Notes;
                true
            }

            // Features & Vues
            ButtonId::FixedVel => {
                self.fixed_velocity = !self.fixed_velocity;
                true
            }
            ButtonId::Follow => {
                self.follow_mode = !self.follow_mode;
                true
            }
            ButtonId::NoteRepeat => {
                self.note_repeat = !self.note_repeat;
                actions.push(MapperAction::SetNoteRepeat(self.note_repeat));
                true
            }
            ButtonId::Variation => {
                actions.push(MapperAction::Variation);
                true
            }
            ButtonId::Auto => {
                actions.push(MapperAction::Quantize);
                true
            }
            ButtonId::FileSave => {
                // Bouton FILE/SAVE (physique, sous BROWSER) : menu projets/sauvegarde.
                if self.shift_held {
                    actions.push(MapperAction::QuickSaveProject);
                } else {
                    self.clear_menu_modes(actions);
                    if self.left_tab == 3 && self.browser_kind == 1 {
                        // Toggle : referme le menu FILE
                        self.select_left_tab(0, actions);
                    } else {
                        self.browser_kind = 1;
                        self.select_left_tab(3, actions);
                        actions.push(MapperAction::OpenFileBrowser);
                    }
                }
                true
            }

            // Onglets d'écrans dédiés
            ButtonId::Arranger => {
                self.clear_menu_modes(actions);
                self.select_left_tab(0, actions);
                actions.push(MapperAction::ToggleSongMode);
                true
            }
            ButtonId::Mixer => {
                self.clear_menu_modes(actions);
                self.select_left_tab(1, actions);
                true
            }
            ButtonId::Sampling => {
                self.clear_menu_modes(actions);
                self.select_left_tab(2, actions);
                true
            }
            ButtonId::Browser => {
                self.clear_menu_modes(actions);
                if self.shift_held {
                    // Shift+BROWSER : navigateur de projets/fichiers (comme FILE/SAVE)
                    self.browser_kind = 1;
                    self.select_left_tab(3, actions);
                    actions.push(MapperAction::OpenFileBrowser);
                } else if self.left_tab == 3 && self.browser_kind == 0 {
                    // Déjà dans BROWSER presets : toggle pour fermer
                    self.select_left_tab(0, actions);
                } else {
                    // BROWSER : navigateur de presets/samples
                    self.browser_kind = 0;
                    self.select_left_tab(3, actions);
                    actions.push(MapperAction::SetBrowserKind(0));
                }
                true
            }
            ButtonId::Settings => {
                let tab = if self.right_tab == 3 { 0 } else { 3 };
                self.right_tab = tab;
                self.led_state.set_button_raw(
                    maschine_proto::hid_output::offsets::SETTINGS,
                    if tab == 3 { 7 } else { 1 },
                );
                actions.push(MapperAction::SetRightTab(tab));
                true
            }

            // Groupes A..H (avec MUTE/SOLO + Group façon Maschine)
            ButtonId::Group(g) if g < 8 => {
                // MUTE/SOLO + Group : cible le groupe sans changer le focus.
                // SOLO maintenu physiquement a la priorité sur le mode MUTE latched.
                if self.solo_held {
                    actions.push(MapperAction::ToggleSolo(g));
                    self.mute_solo_group_used = true;
                } else if self.mute_held || self.mute_pad_mode {
                    actions.push(MapperAction::ToggleMute(g));
                    self.mute_solo_group_used = true;
                } else {
                    self.group_focus = g;
                    actions.push(MapperAction::GroupFocus(g));
                }
                true
            }

            // Actions d'édition
            ButtonId::Duplicate => {
                if self.pattern_select_mode {
                    actions.push(MapperAction::DuplicatePattern {
                        group: self.group_focus,
                    });
                } else {
                    actions.push(MapperAction::DuplicateBar);
                }
                true
            }
            // Workflow officiel : SCENE / PATTERN / SELECT / EVENTS / MACRO / PLUGIN
            // Chaque menu reste actif (latch) et un second appui sur
            // le même bouton le désactive, comme sur Maschine Windows.
            ButtonId::Scene => {
                if self.scene_select_mode {
                    self.exit_to_default(actions);
                } else {
                    self.clear_menu_modes(actions);
                    self.scene_select_mode = true;
                    self.select_left_tab(0, actions);
                    self.set_controller_mode(ControllerMode::Pad, actions);
                    actions.push(MapperAction::SetSceneMode(true));
                    actions.push(MapperAction::NavigateView(nav::SCENE));
                }
                true
            }
            ButtonId::Pattern => {
                if self.pattern_select_mode {
                    self.exit_to_default(actions);
                } else {
                    self.clear_menu_modes(actions);
                    self.pattern_select_mode = true;
                    self.select_left_tab(0, actions);
                    self.set_controller_mode(ControllerMode::Pad, actions);
                    actions.push(MapperAction::SetPatternMode(true));
                    actions.push(MapperAction::NavigateView(nav::PATTERN));
                }
                true
            }
            ButtonId::Events => {
                let tab = if self.right_tab == 1 { 0 } else { 1 };
                self.right_tab = tab;
                self.led_state.set_button_raw(
                    maschine_proto::hid_output::offsets::EVENTS,
                    if tab == 1 { 7 } else { 1 },
                );
                actions.push(MapperAction::SetRightTab(tab));
                true
            }
            ButtonId::Macro => {
                let tab = if self.right_tab == 2 { 0 } else { 2 };
                self.right_tab = tab;
                self.led_state.set_button_raw(
                    maschine_proto::hid_output::offsets::MACRO,
                    if tab == 2 { 7 } else { 1 },
                );
                actions.push(MapperAction::SetRightTab(tab));
                true
            }
            ButtonId::Plugin => {
                self.clear_menu_modes(actions);
                if self.left_tab == 4 {
                    // Second appui : quitte l'éditeur PLUGIN.
                    self.select_left_tab(0, actions);
                    actions.push(MapperAction::NavigateView(nav::PATTERN));
                } else {
                    self.select_left_tab(4, actions);
                    // Le moteur choisit la page selon le Sound actif
                    // (TB-303 si le pad sélectionné est un synthé, sinon SOUND).
                    actions.push(MapperAction::NavigateView(nav::PLUGIN));
                }
                true
            }
            ButtonId::ChannelMidi => {
                self.clear_menu_modes(actions);
                self.select_left_tab(1, actions); // MIXER
                true
            }
            ButtonId::Lock => {
                self.lock_active = !self.lock_active;
                self.led_state.set_button_raw(
                    maschine_proto::hid_output::offsets::LOCK,
                    if self.lock_active || self.note_repeat {
                        7
                    } else {
                        1
                    },
                );
                actions.push(MapperAction::ToggleLock(self.lock_active));
                true
            }
            _ => false,
        }
    }

    fn handle_4way_button(&mut self, id: ButtonId, actions: &mut Vec<MapperAction>) -> bool {
        match id {
            // Navigation 4D & Flèches < >
            ButtonId::FourWayLeft | ButtonId::Left => {
                if self.left_tab == 1 {
                    // Mode MIXER : navigation 4D entre sous-vues
                    if self.mixer_mode == 1 {
                        self.mixer_mode = 0;
                        actions.push(MapperAction::SetMixerMode(0));
                    } else if self.mixer_mode == 0 {
                        self.mixer_mode = 2;
                        actions.push(MapperAction::SetMixerMode(2));
                    } else if self.mixer_mode >= 2 {
                        self.group_focus = (self.group_focus as i32 - 1).rem_euclid(8) as u8;
                        actions.push(MapperAction::GroupFocus(self.group_focus));
                    }
                } else if self.pattern_select_mode {
                    actions.push(MapperAction::PatternStep {
                        group: self.group_focus,
                        delta: -1,
                    });
                } else if self.mode == ControllerMode::Step && self.left_tab == 0 {
                    // Mode STEP : défile la grille d'un pas.
                    actions.push(MapperAction::StepScroll(-1));
                } else {
                    self.selected_bar = self.selected_bar.saturating_sub(1);
                    actions.push(MapperAction::SelectBar(self.selected_bar));
                }
                true
            }
            ButtonId::FourWayRight | ButtonId::Right => {
                if self.left_tab == 1 {
                    // Mode MIXER : navigation 4D vers les pads du groupe
                    if self.mixer_mode >= 2 {
                        self.mixer_mode = 0;
                        actions.push(MapperAction::SetMixerMode(0));
                    } else if self.mixer_mode == 0 {
                        self.mixer_mode = 1;
                        actions.push(MapperAction::SetMixerMode(1));
                    }
                } else if self.pattern_select_mode {
                    actions.push(MapperAction::PatternStep {
                        group: self.group_focus,
                        delta: 1,
                    });
                } else if self.shift_held {
                    let new_len = if self.pattern_bars < 4 { 4 } else { 8 };
                    self.pattern_bars = new_len;
                    actions.push(MapperAction::SetPatternBars(new_len));
                } else if self.mode == ControllerMode::Step && self.left_tab == 0 {
                    // Mode STEP : défile la grille d'un pas.
                    actions.push(MapperAction::StepScroll(1));
                } else {
                    self.selected_bar =
                        (self.selected_bar + 1).min(self.pattern_bars.saturating_sub(1));
                    actions.push(MapperAction::SelectBar(self.selected_bar));
                }
                true
            }
            ButtonId::FourWayUp => {
                if self.left_tab == 3 {
                    actions.push(MapperAction::BrowsePresetStep(-1));
                } else if self.left_tab == 1 {
                    if self.mixer_mode >= 2 {
                        // Pousser vers le haut rentre dans le groupe pour afficher les pads
                        self.mixer_mode = 0;
                        actions.push(MapperAction::SetMixerMode(0));
                    } else {
                        self.active_sound = (self.active_sound + 1) % 16;
                        actions.push(MapperAction::SelectSound(self.active_sound));
                    }
                } else {
                    self.active_sound = (self.active_sound + 1) % 16;
                    actions.push(MapperAction::SelectSound(self.active_sound));
                    actions.push(MapperAction::PadTrigger {
                        pad: self.active_sound,
                        velocity: 110,
                        pressed: true,
                    });
                }
                true
            }
            ButtonId::FourWayDown => {
                if self.left_tab == 3 {
                    actions.push(MapperAction::BrowsePresetStep(1));
                } else if self.left_tab == 1 {
                    if self.mixer_mode < 2 {
                        // Pousser vers le bas sort du groupe vers la vue Groupes / Master
                        self.mixer_mode = 2;
                        actions.push(MapperAction::SetMixerMode(2));
                    } else {
                        self.active_sound = (self.active_sound + 15) % 16;
                        actions.push(MapperAction::SelectSound(self.active_sound));
                    }
                } else {
                    self.active_sound = (self.active_sound + 15) % 16;
                    actions.push(MapperAction::SelectSound(self.active_sound));
                    actions.push(MapperAction::PadTrigger {
                        pad: self.active_sound,
                        velocity: 110,
                        pressed: true,
                    });
                }
                true
            }
            ButtonId::EncoderPush => {
                if self.shift_held {
                    // Shift+Push sur encodeur 4D : cycle rapidement le type de filtre (LP/HP/BP/OFF)
                    actions.push(MapperAction::CycleSoundFilterKind);
                } else if self.left_tab == 3 {
                    actions.push(MapperAction::LoadSelectedPreset);
                } else if self.left_tab == 1 {
                    // Pousser l'encodeur dans le mixer rentre dans le groupe pour afficher
                    // les VUs des pads (mode 0: pads 1-8 / 9-16), ou remonte au mixeur groupes (mode 2).
                    let next_mode = if self.mixer_mode >= 2 { 0 } else { 2 };
                    self.mixer_mode = next_mode;
                    actions.push(MapperAction::SetMixerMode(next_mode));
                } else {
                    // One-shot preview du sound sélectionné (ne démarre PAS la boucle transport !)
                    actions.push(MapperAction::PadTrigger {
                        pad: self.active_sound,
                        velocity: 115,
                        pressed: true,
                    });
                }
                true
            }
            _ => false,
        }
    }

    fn handle_display_button(&mut self, id: ButtonId, actions: &mut Vec<MapperAction>) -> bool {
        match id {
            // Soft Buttons Écrans 1..8 — actions contextuelles :
            // c'est l'Engine qui décide (labels = actions réelles
            // affichées en bas des écrans), le hardware ne fait que
            // transmettre.
            ButtonId::DisplayBtn(n @ 1..=8) => {
                if self.note_repeat {
                    if n <= 7 {
                        self.note_repeat_rate = n - 1;
                        actions.push(MapperAction::SetNoteRepeatRate(n - 1));
                    } else {
                        self.note_repeat = !self.note_repeat;
                        actions.push(MapperAction::SetNoteRepeat(self.note_repeat));
                    }
                } else {
                    actions.push(MapperAction::SoftKey(n));
                }
                true
            }
            _ => false,
        }
    }

    fn handle_button_event(&mut self, id: ButtonId, pressed: bool) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        if self.handle_modifier_button(id, pressed, &mut actions) {
            return actions;
        }

        if pressed {
            let _ = self.handle_transport_button(id, &mut actions)
                || self.handle_view_menu_button(id, &mut actions)
                || self.handle_4way_button(id, &mut actions)
                || self.handle_display_button(id, &mut actions);
        }
        actions
    }

    fn handle_main_4d_encoder(&mut self, delta: i8) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        if self.left_tab == 1 {
            // Mode MIXER : l'encodeur 4D scrolle les pistes (banques 1-8 / 9-16 ou groupes)
            if self.mixer_mode == 0 && delta > 0 {
                self.mixer_mode = 1;
                actions.push(MapperAction::SetMixerMode(1));
            } else if self.mixer_mode == 1 && delta < 0 {
                self.mixer_mode = 0;
                actions.push(MapperAction::SetMixerMode(0));
            } else if self.mixer_mode == 2 {
                self.group_focus = (self.group_focus as i32 + delta as i32).rem_euclid(8) as u8;
                actions.push(MapperAction::GroupFocus(self.group_focus));
            }
        } else if self.left_tab == 4 && self.has_wavetable_synth {
            // Vue WAVETABLE / NEXUS SYNTH : encodeur 4D défile les presets
            if delta > 0 {
                actions.push(MapperAction::WavetableNextPreset);
            } else {
                actions.push(MapperAction::WavetablePrevPreset);
            }
        } else if self.left_tab == 3 {
            self.browser_preset_idx =
                (self.browser_preset_idx as i32 + delta as i32).rem_euclid(16) as u8;
            actions.push(MapperAction::BrowsePresetStep(delta as i32));
        } else {
            match self.encoder_focus {
                EncoderFocus::Tempo => {
                    let step = if self.shift_held { 5.0 } else { 10.0 };
                    actions.push(MapperAction::BpmDelta(delta as f32 * step));
                }
                EncoderFocus::Volume => {
                    let step = if self.shift_held { 5.0 } else { 10.0 };
                    actions.push(MapperAction::MasterVolumeDelta(delta as f32 * step));
                }
                EncoderFocus::Swing => {
                    let step = if self.shift_held { 5.0 } else { 10.0 };
                    actions.push(MapperAction::SwingDelta(delta as f32 * step));
                }
            }
        }
        actions
    }

    fn handle_step_knobs(&mut self, idx: usize, delta: i32) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        match idx {
            0 => {
                let d = self.knob_steps(idx, delta, 0.1);
                if d != 0 {
                    actions.push(MapperAction::StepVelocityDelta(d));
                }
            }
            1 => {
                let d = self.knob_steps(idx, delta, 0.25);
                if d != 0 {
                    actions.push(MapperAction::StepNudgeTicks(d));
                }
            }
            2 => {
                let d = self.knob_steps(idx, delta, 0.01);
                if d != 0 {
                    actions.push(MapperAction::StepLengthDelta(d));
                }
            }
            3 => {
                let d = self.knob_steps(idx, delta, 0.025);
                if d != 0 {
                    actions.push(MapperAction::StepPitchDelta(d));
                }
            }
            4 => {
                let d = self.knob_steps(idx, delta, 0.03125);
                if d != 0 {
                    actions.push(MapperAction::StepScroll(d.clamp(-4, 4)));
                }
            }
            5 => actions.push(MapperAction::SoundVolumeDelta(delta as f32)),
            6 => actions.push(MapperAction::SoundFilterCutoffDelta(delta as f32)),
            7 => actions.push(MapperAction::SoundFilterResoDelta(delta as f32)),
            _ => {}
        }
        actions
    }

    fn handle_mixer_knobs(&mut self, idx: usize, delta: i32) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        if self.mixer_mode == 0 && idx < 8 {
            let pad = idx as u8;
            if self.shift_held {
                actions.push(MapperAction::SoundPanDeltaFor {
                    group: self.group_focus,
                    sound: pad,
                    delta: delta as f32,
                });
            } else {
                actions.push(MapperAction::SoundVolumeDeltaFor {
                    group: self.group_focus,
                    sound: pad,
                    delta: delta as f32,
                });
            }
        } else if self.mixer_mode == 1 && idx < 8 {
            let pad = (idx + 8) as u8;
            if self.shift_held {
                actions.push(MapperAction::SoundPanDeltaFor {
                    group: self.group_focus,
                    sound: pad,
                    delta: delta as f32,
                });
            } else {
                actions.push(MapperAction::SoundVolumeDeltaFor {
                    group: self.group_focus,
                    sound: pad,
                    delta: delta as f32,
                });
            }
        } else if self.shift_held || self.mixer_mode == 2 {
            actions.push(MapperAction::GroupVolumeDeltaFor {
                group: idx as u8,
                delta: delta as f32,
            });
        } else {
            match idx {
                0 => actions.push(MapperAction::SoundVolumeDelta(delta as f32)),
                1 => actions.push(MapperAction::SoundPanDelta(delta as f32)),
                2 => actions.push(MapperAction::GroupVolumeDelta(delta as f32)),
                3 => actions.push(MapperAction::GroupPanDelta(delta as f32)),
                4 => actions.push(MapperAction::SoundFilterCutoffDelta(delta as f32)),
                5 => actions.push(MapperAction::SoundFilterResoDelta(delta as f32)),
                6 => actions.push(MapperAction::MasterVolumeDelta(delta as f32)),
                _ => actions.push(MapperAction::HeadphoneVolumeDelta(delta as f32)),
            }
        }
        actions
    }

    fn handle_sampler_knobs(&mut self, idx: usize, delta: i32) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        if self.shift_held && (idx == 2 || idx == 3) {
            actions.push(MapperAction::CycleSoundFilterKind);
        } else {
            if self.shift_held && (idx == 2 || idx == 3) {
                actions.push(MapperAction::CycleSoundFilterKind);
            } else {
                match idx {
                    0 => actions.push(MapperAction::SoundVolumeDelta(delta as f32)),
                    1 => actions.push(MapperAction::SoundPanDelta(delta as f32)),
                    2 => actions.push(MapperAction::SoundFilterCutoffDelta(delta as f32)),
                    3 => actions.push(MapperAction::SoundFilterResoDelta(delta as f32)),
                    4 => actions.push(MapperAction::SoundAttackDelta(delta as f32)),
                    5 => actions.push(MapperAction::SoundDecayDelta(delta as f32)),
                    6 => actions.push(MapperAction::SoundSustainDelta(delta as f32)),
                    _ => actions.push(MapperAction::SoundReleaseDelta(delta as f32)),
                }
            }
        }
        actions
    }

    fn handle_plugin_knobs(&mut self, idx: usize, delta: i32) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        if self.plugin_page == 1
            || self.has_kick_synth
            || self.has_wavetable_synth
            || self.has_acid_synth
        {
            if self.has_kick_synth {
                if self.shift_held && idx == 5 {
                    actions.push(MapperAction::KickDriveTypeDelta(delta.signum()));
                } else if self.shift_held && idx == 6 {
                    actions.push(MapperAction::KickFilterCutoffDelta(delta as f32));
                } else if self.shift_held && idx == 7 {
                    actions.push(MapperAction::KickFilterModeDelta(delta.signum()));
                } else {
                    match idx {
                        0 => actions.push(MapperAction::KickTuneDelta(delta as f32)),
                        1 => actions.push(MapperAction::KickDecayDelta(delta as f32)),
                        2 => actions.push(MapperAction::KickSnapDelta(delta as f32)),
                        3 => actions.push(MapperAction::KickSweepDelta(delta as f32)),
                        4 => actions.push(MapperAction::KickClickDelta(delta as f32)),
                        5 => actions.push(MapperAction::KickDriveDelta(delta as f32)),
                        6 => actions.push(MapperAction::KickSubDelta(delta as f32)),
                        _ => actions.push(MapperAction::KickVolumeDelta(delta as f32)),
                    }
                }
            } else if self.has_wavetable_synth {
                if self.shift_held {
                    match idx {
                        0 => actions.push(MapperAction::WavetableFilterModeCycle),
                        1 => actions.push(MapperAction::WavetableUnisonVoicesDelta(delta.signum())),
                        2 => actions.push(MapperAction::WavetableOsc1WaveCycle),
                        3 => actions.push(MapperAction::WavetableOsc2WaveCycle),
                        4 => actions.push(MapperAction::WavetableChorusDelta(delta as f32)),
                        5 => actions.push(MapperAction::WavetableDelayDelta(delta as f32)),
                        6 => actions.push(MapperAction::WavetableReverbDelta(delta as f32)),
                        _ => actions.push(MapperAction::WavetableVolumeDelta(delta as f32)),
                    }
                } else {
                    match idx {
                        0 => actions.push(MapperAction::WavetableCutoffDelta(delta as f32)),
                        1 => actions.push(MapperAction::WavetableResoDelta(delta as f32)),
                        2 => actions.push(MapperAction::WavetableEnvAmtDelta(delta as f32)),
                        3 => actions.push(MapperAction::WavetableDriveDelta(delta as f32)),
                        4 => actions.push(MapperAction::WavetableAttackDelta(delta as f32)),
                        5 => actions.push(MapperAction::WavetableDecayDelta(delta as f32)),
                        6 => actions.push(MapperAction::WavetableSustainDelta(delta as f32)),
                        _ => actions.push(MapperAction::WavetableReleaseDelta(delta as f32)),
                    }
                }
            } else {
                match idx {
                    0 => actions.push(MapperAction::AcidCutoffDelta(delta as f32)),
                    1 => actions.push(MapperAction::AcidResoDelta(delta as f32)),
                    2 => actions.push(MapperAction::AcidEnvModDelta(delta as f32)),
                    3 => actions.push(MapperAction::AcidDecayDelta(delta as f32)),
                    4 => actions.push(MapperAction::AcidAccentDelta(delta as f32)),
                    5 => actions.push(MapperAction::AcidDriveDelta(delta as f32)),
                    6 => actions.push(MapperAction::AcidDelayTimeDelta(delta as f32)),
                    _ => actions.push(MapperAction::AcidDelayMixDelta(delta as f32)),
                }
            }
        } else {
            if self.shift_held && (idx == 2 || idx == 3) {
                actions.push(MapperAction::CycleSoundFilterKind);
            } else {
                match idx {
                    0 => actions.push(MapperAction::SoundVolumeDelta(delta as f32)),
                    1 => actions.push(MapperAction::SoundPanDelta(delta as f32)),
                    2 => actions.push(MapperAction::SoundFilterCutoffDelta(delta as f32)),
                    3 => actions.push(MapperAction::SoundFilterResoDelta(delta as f32)),
                    4 => actions.push(MapperAction::SoundAttackDelta(delta as f32)),
                    5 => actions.push(MapperAction::SoundDecayDelta(delta as f32)),
                    6 => actions.push(MapperAction::SoundSustainDelta(delta as f32)),
                    _ => actions.push(MapperAction::SoundReleaseDelta(delta as f32)),
                }
            }
        }
        actions
    }

    fn handle_browser_knobs(&mut self, idx: usize, delta: i32) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        if self.browser_kind == 1 {
            match idx {
                0 => {
                    let d = self.knob_steps(idx, delta, 0.03125);
                    if d != 0 {
                        actions.push(MapperAction::BrowsePresetStep(d.clamp(-1, 1)));
                    }
                }
                7 => actions.push(MapperAction::MasterVolumeDelta(delta as f32)),
                _ => {}
            }
        } else {
            match idx {
                0 => {
                    let d = self.knob_steps(idx, delta, 0.03125);
                    if d != 0 {
                        actions.push(MapperAction::BrowseSoundStep(d.clamp(-1, 1)));
                    }
                }
                1 => {
                    let d = self.knob_steps(idx, delta, 0.03125).clamp(-1, 1);
                    if d != 0 {
                        self.browser_preset_idx =
                            (self.browser_preset_idx as i32 + d).rem_euclid(16) as u8;
                        actions.push(MapperAction::BrowsePresetStep(d));
                    }
                }
                7 => actions.push(MapperAction::MasterVolumeDelta(delta as f32)),
                _ => {}
            }
        }
        actions
    }

    fn handle_arrange_knobs(&mut self, idx: usize, delta: i32) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        if self.shift_held && (idx == 5 || idx == 6) {
            actions.push(MapperAction::CycleSoundFilterKind);
        } else {
            match idx {
                0 => {
                    let fine = if self.shift_held { 0.1 } else { 1.0 };
                    actions.push(MapperAction::BpmDelta(delta as f32 * fine));
                }
                1 => actions.push(MapperAction::SwingDelta(delta as f32)),
                2 => actions.push(MapperAction::GroupVolumeDelta(delta as f32)),
                3 => actions.push(MapperAction::GroupPanDelta(delta as f32)),
                4 => actions.push(MapperAction::SoundVolumeDelta(delta as f32)),
                5 => actions.push(MapperAction::SoundFilterCutoffDelta(delta as f32)),
                6 => actions.push(MapperAction::SoundFilterResoDelta(delta as f32)),
                _ => actions.push(MapperAction::MasterVolumeDelta(delta as f32)),
            }
        }
        actions
    }

    fn handle_display_knob(&mut self, idx: usize, delta: i32) -> Vec<MapperAction> {
        if self.mode == ControllerMode::Step && self.left_tab == 0 {
            self.handle_step_knobs(idx, delta)
        } else if self.left_tab == 1 {
            self.handle_mixer_knobs(idx, delta)
        } else if self.left_tab == 2 {
            self.handle_sampler_knobs(idx, delta)
        } else if self.left_tab == 4 {
            self.handle_plugin_knobs(idx, delta)
        } else if self.left_tab == 3 {
            self.handle_browser_knobs(idx, delta)
        } else {
            self.handle_arrange_knobs(idx, delta)
        }
    }

    fn handle_encoder_event(&mut self, enc: EncoderEvent) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        if enc.id == 0 {
            let new_val = (enc.value & 0x0F) as u8;
            let delta = match self.prev_main_encoder {
                Some(prev) if prev != new_val => wrap_16_delta(new_val, prev),
                _ => 0,
            };
            self.prev_main_encoder = Some(new_val);
            if delta != 0 {
                actions.extend(self.handle_main_4d_encoder(delta));
            }
        } else if (1..=8).contains(&enc.id) {
            let idx = (enc.id - 1) as usize;
            let new_val = enc.value & 0x3FF;
            let delta = match self.prev_knobs[idx] {
                Some(prev) if prev != new_val => wrap_1024_delta(new_val, prev),
                _ => 0,
            };
            self.prev_knobs[idx] = Some(new_val);
            let delta = delta.clamp(-128, 128);
            if delta != 0 {
                actions.extend(self.handle_display_knob(idx, delta));
            }
        }
        actions
    }

    fn handle_touch_strip_event(&mut self, ts: TouchStripEvent) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        self.strip_touched = ts.touched;
        self.strip_pos = ts.position;
        if ts.touched {
            match self.touchstrip_mode {
                TouchStripMode::Pitch => {
                    // Position 0..1023 -> Pitch bend -8192..+8191 (centre 512)
                    let pb = ((ts.position as i32 - 512) * 16).clamp(-8192, 8191) as i16;
                    // Envoi CC 128 (placeholder pitch bend dans notre abstraction)
                    actions.push(MapperAction::Midi(MidiEvent::Cc {
                        channel: 0,
                        cc: 128,
                        value: ((pb + 8192) / 128).clamp(0, 127) as u8,
                    }));
                }
                TouchStripMode::Mod => {
                    let val = (ts.position / 8).min(127) as u8;
                    actions.push(MapperAction::Midi(MidiEvent::Cc {
                        channel: 0,
                        cc: 1, // Modulation CC
                        value: val,
                    }));
                }
                TouchStripMode::Perform => {
                    let val = (ts.position / 8).min(127) as u8;
                    actions.push(MapperAction::Midi(MidiEvent::Cc {
                        channel: 0,
                        cc: 74, // Filter Cutoff
                        value: val,
                    }));
                }
                TouchStripMode::Notes => {
                    let val = (ts.position / 8).min(127) as u8;
                    actions.push(MapperAction::Midi(MidiEvent::Cc {
                        channel: 0,
                        cc: 71, // Resonance
                        value: val,
                    }));
                }
            }
        } else if self.touchstrip_mode == TouchStripMode::Pitch {
            // Relâchement : retour pitch bend à zéro au centre
            actions.push(MapperAction::Midi(MidiEvent::Cc {
                channel: 0,
                cc: 128,
                value: 64, // Centre
            }));
        }
        actions
    }

    fn handle_pad_event(&mut self, p: PadState) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        let note = self.cfg.pad_base_note + p.pad;
        let ch = self.cfg.pad_channel;
        // Fronts uniquement : le thread HID réémet `pressed=true` à chaque
        // report tant que le doigt reste posé (pression/aftertouch). Sans
        // ce garde, un appui maintenu re-déclenchait le son en boucle
        // (mitraillette) et basculait le pas du step-sequencer plusieurs fois.
        let prev = if (p.pad as usize) < 16 {
            let was = self.pad_pressed[p.pad as usize];
            self.pad_pressed[p.pad as usize] = p.pressed;
            was
        } else {
            false
        };
        let vel = if self.fixed_velocity {
            127
        } else {
            self.apply_curve(p.velocity).max(1)
        };

        if p.pressed {
            // Répétition d'un appui maintenu : ignorée (pic de vélocité
            // géré côté HID : un seul événement press est émis).
            if prev {
                return actions;
            }
            // Flash tactile : horodate la frappe pour un feedback LED
            // garanti même si le release suit aussitôt (tap éclair).
            if (p.pad as usize) < 16 {
                self.pad_hit_at[p.pad as usize] = Some(Instant::now());
            }

            // Modes workflow prioritaires (façon Maschine) — avant les modes de jeu.
            if self.scene_select_mode {
                actions.push(MapperAction::SceneTrigger(p.pad as usize));
                return actions;
            }
            if self.pattern_select_mode {
                let other_held = self
                    .pad_pressed
                    .iter()
                    .enumerate()
                    .any(|(i, &held)| held && i != p.pad as usize);
                if other_held {
                    let mut held_indices: Vec<usize> = self
                        .pad_pressed
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &held)| if held { Some(i) } else { None })
                        .collect();
                    held_indices.push(p.pad as usize);
                    let min = *held_indices.iter().min().unwrap_or(&(p.pad as usize));
                    let max = *held_indices.iter().max().unwrap_or(&(p.pad as usize));
                    actions.push(MapperAction::PatternChain {
                        group: self.group_focus,
                        patterns: (min..=max).collect(),
                    });
                } else {
                    actions.push(MapperAction::PatternTrigger {
                        group: self.group_focus,
                        pattern: p.pad as usize,
                    });
                }
                return actions;
            }
            if self.mute_pad_mode {
                actions.push(MapperAction::TogglePadMute {
                    group: self.group_focus,
                    pad: p.pad,
                });
                return actions;
            }
            if self.erase_mode {
                self.erase_used = true;
                self.active_sound = p.pad;
                actions.push(MapperAction::SelectSound(p.pad));
                actions.push(MapperAction::EraseSound {
                    group: self.group_focus,
                    sound: p.pad,
                });
                return actions;
            }
            if self.select_mode {
                self.select_used = true;
                self.active_sound = p.pad;
                actions.push(MapperAction::SelectSound(p.pad));
                return actions;
            }

            match self.mode {
                ControllerMode::Step => {
                    // Façon Maschine : en mode Step, le pad bascule le pas
                    // dans la mesure courante pour le son actif, SANS
                    // produire de son (pas d'aperçu, pas de one-shot).
                    actions.push(MapperAction::ToggleStep {
                        sound: self.active_sound,
                        bar: self.selected_bar,
                        step: p.pad as usize,
                    });
                }
                ControllerMode::Keyboard => {
                    let mel_note = self.keyboard_note_for_pad(p.pad);
                    self.pad_played.insert(p.pad, vec![mel_note]);
                    actions.push(MapperAction::KeyboardTrigger {
                        note: mel_note,
                        velocity: vel,
                        pressed: true,
                    });
                }
                ControllerMode::Chords => {
                    let notes = self.chord_notes_for_pad(p.pad);
                    for &n in &notes {
                        actions.push(MapperAction::KeyboardTrigger {
                            note: n,
                            velocity: vel,
                            pressed: true,
                        });
                    }
                    self.pad_played.insert(p.pad, notes);
                }
                ControllerMode::Pad => {
                    self.pad_note.insert(p.pad, note);
                    actions.push(MapperAction::Midi(MidiEvent::NoteOn {
                        channel: ch,
                        note,
                        velocity: vel,
                    }));
                    self.active_sound = p.pad;
                    actions.push(MapperAction::SelectSound(p.pad));
                    actions.push(MapperAction::PadTrigger {
                        pad: p.pad,
                        velocity: vel,
                        pressed: true,
                    });
                }
            }
        } else {
            // Release dupliqué (déjà relâché) : ignoré.
            if !prev {
                return actions;
            }
            // NoteOff uniquement si un NoteOn a réellement été émis pour
            // cet appui (modes de jeu). Les modes Step/workflow restent
            // totalement silencieux, façon Maschine.
            if let Some(notes) = self.pad_played.remove(&p.pad) {
                for note in notes {
                    actions.push(MapperAction::KeyboardTrigger {
                        note,
                        velocity: 0,
                        pressed: false,
                    });
                }
                return actions;
            }
            let Some(note) = self.pad_note.remove(&p.pad) else {
                return actions;
            };
            actions.push(MapperAction::Midi(MidiEvent::NoteOff {
                channel: ch,
                note,
                velocity: 0,
            }));
            if self.mode == ControllerMode::Pad {
                actions.push(MapperAction::PadTrigger {
                    pad: p.pad,
                    velocity: 0,
                    pressed: false,
                });
            }
        }
        actions
    }

    /// Traite un `HardwareEvent`, retourne les `MidiEvent` pour compatibilité.
    pub fn handle(&mut self, ev: HardwareEvent) -> Vec<MidiEvent> {
        let actions = self.handle_hardware_event(ev);
        actions
            .into_iter()
            .filter_map(|a| match a {
                MapperAction::Midi(m) => Some(m),
                _ => None,
            })
            .collect()
    }

    /// Flash tactile d'un pad : appui en cours OU frappe récente (<200ms).
    /// Garantit un feedback visible même pour un tap plus court qu'une frame
    /// LED, façon Maschine (flash blanc).
    #[must_use]
    pub fn pad_flash(&self, i: usize, now: Instant) -> bool {
        if i >= 16 {
            return false;
        }
        if self.pad_pressed[i] {
            return true;
        }
        matches!(self.pad_hit_at[i], Some(t) if now.duration_since(t) <= Duration::from_millis(PAD_FLASH_MS))
    }

    /// Met à jour l'ensemble des LEDs physiques selon l'état réel de l'Engine.
    pub fn update_leds(&mut self, info: &SequencerLedInfo) {
        use maschine_proto::hid_output::offsets;

        self.selected_bar = info.selected_bar;
        if info.pattern_bars > 0 {
            self.pattern_bars = info.pattern_bars;
        }

        // 1. Boutons de transport
        self.led_state
            .set_button_raw(offsets::PLAY, if info.is_playing { 7 } else { 2 });
        self.led_state
            .set_button_raw(offsets::STOP, if info.is_stopped { 7 } else { 1 });
        self.led_state
            .set_button_raw(offsets::REC, if info.is_recording { 7 } else { 0 });
        self.led_state
            .set_button_raw(offsets::RESTART, if info.loop_enabled { 7 } else { 2 });

        // 2. Tap Tempo : pulsation sur chaque temps en lecture, état métronome à l'arrêt.
        self.led_state.set_button_raw(
            offsets::TAP,
            if info.is_playing {
                if info.is_quarter_beat {
                    7
                } else {
                    2
                }
            } else if info.metronome {
                7
            } else {
                1
            },
        );

        // 3. Modes Pad vs Keyboard vs Chords vs Step
        self.led_state.set_button_raw(
            offsets::PAD_MODE,
            if self.mode == ControllerMode::Pad {
                7
            } else {
                1
            },
        );
        self.led_state.set_button_raw(
            offsets::KEYBOARD,
            if self.mode == ControllerMode::Keyboard {
                7
            } else {
                1
            },
        );
        self.led_state.set_button_raw(
            offsets::CHORDS,
            if self.mode == ControllerMode::Chords {
                7
            } else {
                1
            },
        );
        self.led_state.set_button_raw(
            offsets::STEP,
            if self.mode == ControllerMode::Step {
                7
            } else {
                1
            },
        );

        // 4. Focus Encodeur 4D
        self.led_state.set_button_raw(
            offsets::TEMPO,
            if self.encoder_focus == EncoderFocus::Tempo {
                7
            } else {
                1
            },
        );
        self.led_state.set_button_raw(
            offsets::VOLUME,
            if self.encoder_focus == EncoderFocus::Volume {
                7
            } else {
                1
            },
        );
        self.led_state.set_button_raw(
            offsets::SWING,
            if self.encoder_focus == EncoderFocus::Swing {
                7
            } else {
                1
            },
        );

        // 5. Modes Smart Strip
        self.led_state.set_button_raw(
            offsets::PITCH,
            if self.touchstrip_mode == TouchStripMode::Pitch {
                7
            } else {
                1
            },
        );
        self.led_state.set_button_raw(
            offsets::MOD,
            if self.touchstrip_mode == TouchStripMode::Mod {
                7
            } else {
                1
            },
        );
        self.led_state.set_button_raw(
            offsets::PERFORM,
            if self.touchstrip_mode == TouchStripMode::Perform {
                7
            } else {
                1
            },
        );
        self.led_state.set_button_raw(
            offsets::NOTES,
            if self.touchstrip_mode == TouchStripMode::Notes {
                7
            } else {
                1
            },
        );

        // 6. Features & Workflows
        if info.note_repeat_enabled {
            self.note_repeat = true;
        }
        if info.lock_active {
            self.lock_active = true;
        }

        self.led_state
            .set_button_raw(offsets::FIXED_VEL, if self.fixed_velocity { 7 } else { 0 });
        self.led_state
            .set_button_raw(offsets::FOLLOW, if self.follow_mode { 7 } else { 1 });
        self.led_state
            .set_button_raw(offsets::NOTE_REPEAT, if self.note_repeat { 7 } else { 1 });
        self.led_state.set_button_raw(
            offsets::LOCK,
            if self.lock_active || self.note_repeat {
                7
            } else {
                1
            },
        );
        self.led_state
            .set_button_raw(offsets::DUPLICATE, if self.duplicate_held { 7 } else { 1 });
        self.led_state
            .set_button_raw(offsets::SHIFT, if self.shift_held { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::CHANNEL_MIDI, 1);
        self.led_state.set_button_raw(offsets::LEFT, 2);
        self.led_state.set_button_raw(offsets::RIGHT, 2);
        self.led_state.set_button_raw(offsets::VARIATION, 2);
        self.led_state.set_button_raw(offsets::AUTO, 2);
        self.mixer_mode = info.mixer_mode;
        self.has_kick_synth = info.has_kick_synth;
        self.has_acid_synth = info.has_acid_synth;
        self.has_wavetable_synth = info.has_wavetable_synth;
        self.delete_project_confirm = info.delete_project_confirm;
        let is_file_active = self.left_tab == 3 && info.browser_kind == 1;
        let is_browser_active = self.left_tab == 3 && info.browser_kind != 1;
        self.led_state.set_button_raw(
            offsets::FILE_SAVE,
            if is_file_active {
                7
            } else if self.shift_held {
                4
            } else {
                1
            },
        );
        // Modes workflow (latch) : SCENE / PATTERN / SELECT / ERASE
        self.led_state
            .set_button_raw(offsets::SCENE, if self.scene_select_mode { 7 } else { 1 });
        self.led_state.set_button_raw(
            offsets::PATTERN,
            if self.pattern_select_mode { 7 } else { 1 },
        );
        self.led_state
            .set_button_raw(offsets::SELECT, if self.select_mode { 7 } else { 1 });
        self.led_state.set_button_raw(
            offsets::ERASE,
            if self.erase_mode {
                7
            } else if self.shift_held {
                3
            } else {
                1
            },
        );
        self.led_state.set_button_raw(
            offsets::MUTE,
            if self.mute_pad_mode
                || self.mute_held
                || (info.muted_groups & (1 << self.group_focus)) != 0
            {
                7
            } else {
                1
            },
        );
        self.led_state.set_button_raw(
            offsets::SOLO,
            if self.solo_held || (info.soloed_groups & (1 << self.group_focus)) != 0 {
                7
            } else {
                1
            },
        );

        // 7. Écrans & Vues
        self.led_state
            .set_button_raw(offsets::ARRANGER, if self.left_tab == 0 { 7 } else { 1 });
        self.led_state
            .set_button_raw(offsets::MIXER, if self.left_tab == 1 { 7 } else { 1 });
        self.led_state
            .set_button_raw(offsets::SAMPLING, if self.left_tab == 2 { 7 } else { 1 });
        self.led_state
            .set_button_raw(offsets::BROWSER, if is_browser_active { 7 } else { 1 });
        self.led_state
            .set_button_raw(offsets::PLUGIN, if self.left_tab == 4 { 7 } else { 1 });
        self.led_state
            .set_button_raw(offsets::SETTINGS, if self.right_tab == 3 { 7 } else { 1 });
        self.led_state
            .set_button_raw(offsets::EVENTS, if self.right_tab == 1 { 7 } else { 1 });
        self.led_state
            .set_button_raw(offsets::MACRO, if self.right_tab == 2 { 7 } else { 1 });

        // 8. Boutons d'écran 1..8
        if self.note_repeat {
            for i in 0..7 {
                self.led_state.set_button_raw(
                    offsets::DISPLAY_BTN_1 + i,
                    if i as u8 == self.note_repeat_rate {
                        7
                    } else {
                        3
                    },
                );
            }
            self.led_state
                .set_button_raw(offsets::DISPLAY_BTN_8, if self.note_repeat { 7 } else { 3 });
        } else if is_file_active {
            self.led_state.set_button_raw(offsets::DISPLAY_BTN_1, 3);
            self.led_state.set_button_raw(offsets::DISPLAY_BTN_2, 3);
            self.led_state.set_button_raw(offsets::DISPLAY_BTN_3, 6);
            self.led_state.set_button_raw(offsets::DISPLAY_BTN_4, 6);
            self.led_state.set_button_raw(offsets::DISPLAY_BTN_5, 5);
            self.led_state.set_button_raw(offsets::DISPLAY_BTN_6, 4);
            self.led_state.set_button_raw(offsets::DISPLAY_BTN_7, 3);
            self.led_state.set_button_raw(offsets::DISPLAY_BTN_8, 4);
        } else {
            self.led_state.set_button_raw(
                offsets::DISPLAY_BTN_1,
                if self.left_tab == 0 { 7 } else { 2 },
            );
            self.led_state.set_button_raw(
                offsets::DISPLAY_BTN_2,
                if self.left_tab == 1 { 7 } else { 2 },
            );
            self.led_state.set_button_raw(
                offsets::DISPLAY_BTN_3,
                if self.left_tab == 2 { 7 } else { 2 },
            );
            self.led_state.set_button_raw(
                offsets::DISPLAY_BTN_4,
                if self.left_tab == 3 { 7 } else { 2 },
            );
            self.led_state.set_button_raw(
                offsets::DISPLAY_BTN_5,
                if self.right_tab == 0 && self.selected_bar == 0 {
                    7
                } else {
                    2
                },
            );
            self.led_state.set_button_raw(
                offsets::DISPLAY_BTN_6,
                if self.right_tab == 0 && self.selected_bar == 1 {
                    7
                } else {
                    2
                },
            );
            self.led_state.set_button_raw(
                offsets::DISPLAY_BTN_7,
                if self.right_tab == 0 && self.selected_bar == 2 {
                    7
                } else {
                    2
                },
            );
            self.led_state.set_button_raw(
                offsets::DISPLAY_BTN_8,
                if self.right_tab == 0 && self.selected_bar == 3 {
                    7
                } else {
                    2
                },
            );
        }

        // 9. Groupes A..H — focus en blanc, les autres en demi-teinte.
        for g in 0usize..8 {
            let col = if g == self.group_focus as usize {
                Color::from_index(78).unwrap_or(Color::OFF) // Blanc focus
            } else {
                dim_pad_color(self.pad_colors[g])
            };
            self.led_state
                .set_button_raw(offsets::GROUP_A + g, col.to_index());
        }

        // 10. (Mute/Solo déjà gérés plus haut avec les holds)

        // 11. Flèches de navigation
        self.led_state.set_button_raw(offsets::FOUR_WAY_UP, 3);
        self.led_state.set_button_raw(offsets::FOUR_WAY_DOWN, 3);
        self.led_state.set_button_raw(offsets::FOUR_WAY_LEFT, 3);
        self.led_state.set_button_raw(offsets::FOUR_WAY_RIGHT, 3);

        // 12. 16 Pads RGB physiques — les modes workflow priment sur le mode de jeu.
        // Façon Maschine : repos en demi-teinte, frappe = flash blanc garanti
        // 200ms (visible même pour un tap éclair), pas programmés en couleur
        // pleine, curseur blanc, déclenchement séquenceur en couleur du son.
        let now = Instant::now();
        let white = Color::from_index(78).unwrap_or(Color::OFF);
        // Flash et état actif produisent tous deux du blanc mais avec des priorités
        // différentes — les blocs identiques sont intentionnels.
        #[allow(clippy::if_same_then_else)]
        if info.scene_mode {
            // SCENE : un pad par scène existante (jaune atténué), scène active
            // en blanc, frappe en flash blanc.
            let dim_yellow = Color::from_index(21).unwrap_or(Color::OFF); // DimYellow
            for i in 0..16 {
                if self.pad_flash(i, now) {
                    self.led_state.set_pad(i, white);
                } else if Some(i) == info.active_scene {
                    self.led_state.set_pad(i, white);
                } else if i < info.num_scenes {
                    self.led_state.set_pad(i, dim_yellow);
                } else {
                    self.led_state.set_pad(i, Color::OFF);
                }
            }
        } else if info.pattern_mode {
            // PATTERN : patterns existants en vert atténué, assigné en blanc.
            let dim_green = Color::from_index(29).unwrap_or(Color::OFF); // DimGreen
            for i in 0..16 {
                if self.pad_flash(i, now) {
                    self.led_state.set_pad(i, white);
                } else if Some(i) == info.scene_pattern {
                    self.led_state.set_pad(i, white);
                } else if i < info.group_num_patterns {
                    self.led_state.set_pad(i, dim_green);
                } else {
                    self.led_state.set_pad(i, Color::OFF);
                }
            }
        } else if info.erase_mode {
            // ERASE : le son actif en rouge vif (danger), frappe en flash blanc.
            for i in 0..16 {
                if self.pad_flash(i, now) {
                    self.led_state.set_pad(i, white);
                } else if i as u8 == info.active_sound {
                    self.led_state
                        .set_pad(i, Color::from_index(6).unwrap_or(Color::OFF));
                } else {
                    self.led_state.set_pad(i, dim_pad_color(self.pad_colors[i]));
                }
            }
        } else if info.select_mode {
            // SELECT : son actif en blanc, les autres en demi-teinte.
            for i in 0..16 {
                if self.pad_flash(i, now) {
                    self.led_state.set_pad(i, white);
                } else if i as u8 == info.active_sound {
                    self.led_state.set_pad(i, white);
                } else {
                    self.led_state.set_pad(i, dim_pad_color(self.pad_colors[i]));
                }
            }
        } else {
            match self.mode {
                ControllerMode::Pad => {
                    let muted_mask = info.muted_sounds[self.group_focus as usize % 8];
                    for i in 0..16 {
                        let is_muted = (muted_mask & (1u16 << i)) != 0;
                        if self.pad_flash(i, now) && !is_muted {
                            self.led_state.set_pad(i, white);
                        } else if is_muted {
                            // Pads mutés : atténués (visibles, distincts des pads actifs).
                            // En mode MUTE, léger contraste pour identifier la sélection.
                            let base = if self.mute_pad_mode {
                                dim_pad_color(self.pad_colors[i])
                            } else {
                                dim_pad_color(dim_pad_color(self.pad_colors[i]))
                            };
                            self.led_state.set_pad(i, base);
                        } else if info.triggered_pads[i] {
                            // Joué par le séquenceur : couleur pleine du son.
                            self.led_state.set_pad(i, self.pad_colors[i]);
                        } else {
                            self.led_state.set_pad(i, dim_pad_color(self.pad_colors[i]));
                        }
                    }
                }
                ControllerMode::Keyboard => {
                    // Gamme mineure : Pads 0, 7, 14 sont fondamentales (C3, C4, C5)
                    let dim_turquoise = Color::from_index(27).unwrap_or(Color::OFF); // DimTurquoise
                    for i in 0..16 {
                        if self.pad_flash(i, now) {
                            self.led_state.set_pad(i, white); // Blanc flash
                        } else if i == 0 || i == 7 || i == 14 {
                            self.led_state
                                .set_pad(i, Color::from_index(42).unwrap_or(Color::OFF));
                        // Bleu fondamental
                        } else {
                            self.led_state.set_pad(i, dim_turquoise);
                            // Turquoise atténué
                        }
                    }
                }
                ControllerMode::Chords => {
                    let dim_purple = Color::from_index(53).unwrap_or(Color::OFF); // DimPurple
                    let dim_magenta = Color::from_index(57).unwrap_or(Color::OFF); // DimMagenta
                    for i in 0..16 {
                        if self.pad_flash(i, now) {
                            self.led_state.set_pad(i, white);
                        } else if i % 2 == 0 {
                            self.led_state.set_pad(i, dim_purple);
                        // Violet atténué
                        } else {
                            self.led_state.set_pad(i, dim_magenta);
                            // Magenta atténué
                        }
                    }
                }
                ControllerMode::Step => {
                    // Curseur d'édition (pas sélectionné) : visible à l'arrêt.
                    let edit_cursor = info.step_cursor % 16;
                    for s in 0..16 {
                        let is_playhead = info.is_playing
                            && (info.current_bar == self.selected_bar)
                            && (info.current_step == s);
                        let is_edit = !info.is_playing && s == edit_cursor;
                        if self.pad_flash(s, now) {
                            self.led_state.set_pad(s, white);
                        // Blanc frappe (prioritaire sur curseur)
                        } else if is_playhead || is_edit {
                            self.led_state.set_pad(s, white);
                        // Blanc curseur (tête de lecture ou édition)
                        } else if info.active_steps[s] {
                            let col = self.pad_colors[self.active_sound as usize];
                            self.led_state.set_pad(s, col); // Couleur du son programmé
                        } else if s % 4 == 0 {
                            self.led_state
                                .set_pad(s, Color::from_index(76).unwrap_or(Color::OFF));
                        // Repère temps (gris foncé ; l'index 2 n'existe pas)
                        } else {
                            self.led_state.set_pad(s, Color::OFF);
                        }
                    }
                }
            }
        } // fin else (mode de jeu normal)

        // 13. Touchstrip 25 LEDs RGB
        if self.strip_touched {
            let lit_idx = ((self.strip_pos as f32 / 1024.0) * 24.0).clamp(0.0, 24.0) as usize;
            match self.touchstrip_mode {
                TouchStripMode::Pitch => {
                    // Bipolaire centré sur LED 12
                    let center = 12usize;
                    let (start, end) = if lit_idx < center {
                        (lit_idx, center)
                    } else {
                        (center, lit_idx)
                    };
                    for i in 0..25 {
                        let col = if i >= start && i <= end {
                            Color::from_index(38).unwrap_or(Color::OFF) // Cyan clair
                        } else if i == center {
                            Color::from_index(31).unwrap_or(Color::OFF)
                        } else {
                            Color::OFF
                        };
                        self.led_state.set_strip(i, col);
                    }
                }
                TouchStripMode::Mod => {
                    for i in 0..25 {
                        let col = if i <= lit_idx {
                            Color::from_index(22).unwrap_or(Color::OFF) // Jaune
                        } else {
                            Color::OFF
                        };
                        self.led_state.set_strip(i, col);
                    }
                }
                TouchStripMode::Perform => {
                    for i in 0..25 {
                        let col = if i <= lit_idx {
                            Color::from_index(10).unwrap_or(Color::OFF) // Orange
                        } else {
                            Color::OFF
                        };
                        self.led_state.set_strip(i, col);
                    }
                }
                TouchStripMode::Notes => {
                    for i in 0..25 {
                        let col = if i == lit_idx {
                            Color::from_index(78).unwrap_or(Color::OFF)
                        } else {
                            Color::OFF
                        };
                        self.led_state.set_strip(i, col);
                    }
                }
            }
        } else if info.is_playing {
            // Playhead progression sur la mesure courante
            let lit = ((info.current_step as f32 / 16.0) * 25.0) as usize;
            for i in 0..25 {
                let c = if i <= lit {
                    Color::from_index(26).unwrap_or(Color::OFF)
                } else {
                    Color::OFF
                };
                self.led_state.set_strip(i, c);
            }
        } else {
            self.led_state.fill_strip(Color::OFF);
            if self.touchstrip_mode == TouchStripMode::Pitch {
                self.led_state
                    .set_strip(12, Color::from_index(31).unwrap_or(Color::OFF));
                // Point central
            }
        }
    }

    /// État LEDs courant (à encoder en 0x80/0x81).
    #[must_use]
    pub fn led_state(&self) -> &LedState {
        &self.led_state
    }

    /// Définit la couleur d'un pad (ex: depuis engine via Sound color).
    pub fn set_pad_color(&mut self, pad: usize, color: Color) {
        if pad < 16 {
            self.pad_colors[pad] = color;
            self.led_state.set_pad(pad, color);
        }
    }

    fn apply_curve(&self, v: u8) -> u8 {
        match self.cfg.velocity_curve {
            VelocityCurve::Linear => v,
            VelocityCurve::Soft => {
                let f = v as f32 / 127.0;
                (f.sqrt() * 127.0) as u8
            }
            VelocityCurve::Hard => {
                let f = v as f32 / 127.0;
                (f * f * 127.0) as u8
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::PadState;

    #[test]
    fn pad_note_on_off() {
        let mut m = HardwareMapper::with_default();
        let ev = HardwareEvent::Pad(PadState {
            pad: 0,
            pressed: true,
            velocity: 100,
            pressure: 0x4080,
            ts_ns: 0,
        });
        let midi = m.handle(ev);
        assert!(matches!(midi[0], MidiEvent::NoteOn { note: 36, .. }));
        let ev2 = HardwareEvent::Pad(PadState {
            pad: 0,
            pressed: false,
            velocity: 0,
            pressure: 0x4000,
            ts_ns: 1000,
        });
        let midi2 = m.handle(ev2);
        assert!(matches!(midi2[0], MidiEvent::NoteOff { note: 36, .. }));
    }

    #[test]
    fn group_focus() {
        let mut m = HardwareMapper::with_default();
        let ev = HardwareEvent::Button {
            id: maschine_proto::ButtonId::Group(3),
            pressed: true,
            ts_ns: 0,
        };
        m.handle(ev);
        assert_eq!(m.group_focus, 3);
    }

    #[test]
    fn test_wrap_16_delta_no_drift() {
        // Aucun mouvement -> delta = 0
        assert_eq!(wrap_16_delta(0, 0), 0);
        assert_eq!(wrap_16_delta(15, 15), 0);
        assert_eq!(wrap_16_delta(7, 7), 0);

        // Rotation horaire (incrément)
        assert_eq!(wrap_16_delta(1, 0), 1);
        assert_eq!(wrap_16_delta(2, 1), 1);
        assert_eq!(wrap_16_delta(0, 15), 1); // Wrap 15 -> 0

        // Rotation anti-horaire (décrément)
        assert_eq!(wrap_16_delta(0, 1), -1);
        assert_eq!(wrap_16_delta(15, 0), -1); // Wrap 0 -> 15
        assert_eq!(wrap_16_delta(14, 15), -1);
    }

    #[test]
    fn test_wrap_1024_delta() {
        assert_eq!(wrap_1024_delta(500, 500), 0);
        assert_eq!(wrap_1024_delta(505, 500), 5);
        assert_eq!(wrap_1024_delta(495, 500), -5);
        // Wrap 1023 -> 5
        assert_eq!(wrap_1024_delta(5, 1020), 9);
        // Wrap 0 -> 1018
        assert_eq!(wrap_1024_delta(1018, 2), -8);
    }

    #[test]
    fn test_mode_switch_and_leds() {
        let mut m = HardwareMapper::with_default();
        assert_eq!(m.mode, ControllerMode::Pad);

        // Appui sur Step
        let actions = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Step,
            pressed: true,
            ts_ns: 0,
        });
        assert!(actions.contains(&MapperAction::ModeSwitch(ControllerMode::Step)));
        assert_eq!(m.mode, ControllerMode::Step);

        // Test update LEDs
        let mut info = SequencerLedInfo {
            is_playing: true,
            ..SequencerLedInfo::default()
        };
        info.active_steps[0] = true;
        info.active_steps[4] = true;
        m.update_leds(&info);

        let led_st = m.led_state();
        // Play LED est allumé
        assert_eq!(
            led_st.button_leds[maschine_proto::hid_output::offsets::PLAY - 1],
            7
        );
        // Step mode LED est allumé
        assert_eq!(
            led_st.button_leds[maschine_proto::hid_output::offsets::STEP - 1],
            7
        );
    }

    #[test]
    fn scene_pattern_erase_modes() {
        use crate::events::PadState;
        let mut m = HardwareMapper::with_default();

        // SCENE : latch puis pad -> SceneTrigger
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Scene,
            pressed: true,
            ts_ns: 0,
        });
        assert!(m.scene_select_mode);
        assert!(a.contains(&MapperAction::SetSceneMode(true)));
        let a = m.handle_hardware_event(HardwareEvent::Pad(PadState {
            pad: 3,
            pressed: true,
            velocity: 100,
            pressure: 0x4080,
            ts_ns: 1,
        }));
        assert!(a.contains(&MapperAction::SceneTrigger(3)));

        // PATTERN : latch (sort de SCENE), pad -> PatternTrigger
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Pattern,
            pressed: true,
            ts_ns: 2,
        });
        assert!(m.pattern_select_mode);
        assert!(!m.scene_select_mode);
        let a = m.handle_hardware_event(HardwareEvent::Pad(PadState {
            pad: 2,
            pressed: true,
            velocity: 100,
            pressure: 0x4080,
            ts_ns: 3,
        }));
        assert!(a.contains(&MapperAction::PatternTrigger {
            group: 0,
            pattern: 2
        }));

        // ERASE : latch, pad -> EraseSound (pas de son joué)
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Pattern,
            pressed: true,
            ts_ns: 4,
        });
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Erase,
            pressed: true,
            ts_ns: 5,
        });
        assert!(m.erase_mode);
        let a = m.handle_hardware_event(HardwareEvent::Pad(PadState {
            pad: 5,
            pressed: true,
            velocity: 100,
            pressure: 0x4080,
            ts_ns: 6,
        }));
        assert!(a.contains(&MapperAction::EraseSound { group: 0, sound: 5 }));
        assert!(!a
            .iter()
            .any(|x| matches!(x, MapperAction::PadTrigger { .. })));
    }

    fn pad_ev(pad: u8, pressed: bool, ts_ns: u64) -> HardwareEvent {
        HardwareEvent::Pad(PadState {
            pad,
            pressed,
            velocity: if pressed { 100 } else { 0 },
            pressure: if pressed { 0x4080 } else { 0x4000 },
            ts_ns,
        })
    }

    #[test]
    fn dim_pad_color_mapping() {
        use maschine_proto::color::PaletteIndex as PI;
        // Vives -> atténuées.
        assert_eq!(dim_pad_color(Color(PI::Red)).0, PI::DimRed);
        assert_eq!(dim_pad_color(Color(PI::Yellow)).0, PI::DimYellow);
        assert_eq!(dim_pad_color(Color(PI::Green)).0, PI::DimGreen);
        assert_eq!(dim_pad_color(Color(PI::Blue)).0, PI::DimBlue);
        assert_eq!(dim_pad_color(Color(PI::Purple)).0, PI::DimPurple);
        assert_eq!(dim_pad_color(Color(PI::Magenta)).0, PI::DimMagenta);
        assert_eq!(dim_pad_color(Color(PI::Orange)).0, PI::DimOrange);
        assert_eq!(dim_pad_color(Color(PI::Turquoise)).0, PI::DimTurquoise);
        // Déjà atténuées / neutres : stables ou adoucies.
        assert_eq!(dim_pad_color(Color(PI::DimRed)).0, PI::DimRed);
        assert_eq!(dim_pad_color(Color(PI::White)).0, PI::Grey);
        assert_eq!(dim_pad_color(Color(PI::Grey)).0, PI::DarkGrey);
        assert_eq!(dim_pad_color(Color(PI::Off)).0, PI::Off);
    }

    #[test]
    fn pad_hit_flashes_white_then_falls_back_to_dim() {
        let mut m = HardwareMapper::with_default();
        assert_eq!(m.mode, ControllerMode::Pad);
        let info = SequencerLedInfo::default();
        // Repos : demi-teinte (pad 0 = Red 6 -> DimRed 5).
        m.update_leds(&info);
        assert_eq!(m.led_state().pad_leds[0], 5);
        // Tap éclair (press + release aussitôt) : flash blanc garanti.
        let _ = m.handle_hardware_event(pad_ev(0, true, 1));
        let _ = m.handle_hardware_event(pad_ev(0, false, 2));
        m.update_leds(&info);
        assert_eq!(m.led_state().pad_leds[0], 78);
        // Le flash suit la frappe, pas l'état pressé (relâché entre-temps).
        assert!(!m.pad_pressed[0]);
        // Expiré (>200ms) : retour à la demi-teinte.
        m.pad_hit_at[0] = Some(std::time::Instant::now() - std::time::Duration::from_millis(500));
        m.update_leds(&info);
        assert_eq!(m.led_state().pad_leds[0], 5);
    }

    #[test]
    fn step_mode_pad_toggles_once_and_stays_silent() {
        let mut m = HardwareMapper::with_default();
        // Bascule en mode Step.
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Step,
            pressed: true,
            ts_ns: 0,
        });
        assert_eq!(m.mode, ControllerMode::Step);

        // 1er appui : un seul ToggleStep, AUCUN son (ni PadTrigger ni MIDI).
        let a = m.handle_hardware_event(pad_ev(5, true, 1));
        assert_eq!(a.len(), 1, "press inattendu en mode Step : {a:?}");
        assert!(matches!(
            a[0],
            MapperAction::ToggleStep {
                sound: 0,
                bar: 0,
                step: 5
            }
        ));

        // Le HID réémet pressed=true tant que le doigt reste posé :
        // ces répétitions doivent être ignorées (sinon le pas bascule
        // plusieurs fois et oblige à appuyer plusieurs fois).
        let a = m.handle_hardware_event(pad_ev(5, true, 2));
        assert!(a.is_empty(), "re-trigger sur appui maintenu : {a:?}");
        let a = m.handle_hardware_event(pad_ev(5, true, 3));
        assert!(a.is_empty());

        // Release : silence total.
        let a = m.handle_hardware_event(pad_ev(5, false, 4));
        assert!(a.is_empty(), "release bruyant en mode Step : {a:?}");

        // Nouvel appui après release : re-bascule exactement une fois.
        let a = m.handle_hardware_event(pad_ev(5, true, 5));
        assert_eq!(a.len(), 1);
        assert!(matches!(a[0], MapperAction::ToggleStep { step: 5, .. }));
    }

    #[test]
    fn pad_mode_no_retrigger_while_held() {
        let mut m = HardwareMapper::with_default();
        assert_eq!(m.mode, ControllerMode::Pad);

        // 1er appui : NoteOn + sélection + trigger, une seule fois.
        let a = m.handle_hardware_event(pad_ev(0, true, 1));
        assert_eq!(a.len(), 3, "actions inattendues : {a:?}");
        assert!(a
            .iter()
            .any(|x| matches!(x, MapperAction::Midi(MidiEvent::NoteOn { note: 36, .. }))));

        // Appui maintenu (reports HID répétés) : pas de mitraillette.
        let a = m.handle_hardware_event(pad_ev(0, true, 2));
        assert!(
            a.is_empty(),
            "re-trigger en boucle sur appui maintenu : {a:?}"
        );

        // Release : NoteOff une seule fois.
        let a = m.handle_hardware_event(pad_ev(0, false, 3));
        assert!(a
            .iter()
            .any(|x| matches!(x, MapperAction::Midi(MidiEvent::NoteOff { note: 36, .. }))));
        // Release dupliqué : ignoré.
        let a = m.handle_hardware_event(pad_ev(0, false, 4));
        assert!(a.is_empty());
    }

    #[test]
    fn mute_pad_mode_latches_and_toggles_selected_pads() {
        let mut m = HardwareMapper::with_default();
        m.group_focus = 1;

        // Appui simple MUTE : active le mode mute des pads (latched).
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Mute,
            pressed: true,
            ts_ns: 0,
        });
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Mute,
            pressed: false,
            ts_ns: 1,
        });
        assert!(m.mute_pad_mode);

        // Pad tap en mode MUTE : toggle le mute du pad (pas de lecture).
        let a = m.handle_hardware_event(pad_ev(4, true, 2));
        assert_eq!(a, vec![MapperAction::TogglePadMute { group: 1, pad: 4 }]);
        let a = m.handle_hardware_event(pad_ev(4, false, 3));
        assert!(a.is_empty());

        // MUTE + Group (mode actif) : mute le groupe sans changer le focus.
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Group(2),
            pressed: true,
            ts_ns: 4,
        });
        assert!(a.contains(&MapperAction::ToggleMute(2)));
        assert_eq!(m.group_focus, 1);

        // Second appui MUTE : sort du mode mute pads.
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Mute,
            pressed: true,
            ts_ns: 5,
        });
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Mute,
            pressed: false,
            ts_ns: 6,
        });
        assert!(!m.mute_pad_mode);
    }

    #[test]
    fn four_way_encoder_and_bar_selection() {
        let mut m = HardwareMapper::with_default();
        m.pattern_bars = 4;
        m.selected_bar = 0;

        // 4D Droite -> Mesure 1
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayRight,
            pressed: true,
            ts_ns: 0,
        });
        assert!(a.contains(&MapperAction::SelectBar(1)));
        assert_eq!(m.selected_bar, 1);

        // 4D Gauche -> Mesure 0
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayLeft,
            pressed: true,
            ts_ns: 1,
        });
        assert!(a.contains(&MapperAction::SelectBar(0)));
        assert_eq!(m.selected_bar, 0);

        // 4D Haut -> Son suivant avec one-shot preview
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayUp,
            pressed: true,
            ts_ns: 2,
        });
        assert!(a.contains(&MapperAction::SelectSound(1)));
        assert!(a.iter().any(|act| matches!(
            act,
            MapperAction::PadTrigger {
                pad: 1,
                velocity: 110,
                pressed: true
            }
        )));

        // Pression 4D (EncoderPush) : One-shot preview, JAMAIS PlayPauseToggle
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::EncoderPush,
            pressed: true,
            ts_ns: 3,
        });
        assert!(!a.contains(&MapperAction::PlayPauseToggle));
        assert!(a.iter().any(|act| matches!(
            act,
            MapperAction::PadTrigger {
                pad: 1,
                velocity: 115,
                pressed: true
            }
        )));

        // DisplayBtn(7) -> SoftKey(7) : action contextuelle décidée par l'Engine
        // (le label affiché sous l'écran fait exactement ce que le bouton fait).
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::DisplayBtn(7),
            pressed: true,
            ts_ns: 4,
        });
        assert!(a.contains(&MapperAction::SoftKey(7)));

        // Shift + DisplayBtn(8) -> SoftKey(8) (l'Engine applique le contexte).
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Shift,
            pressed: true,
            ts_ns: 5,
        });
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::DisplayBtn(8),
            pressed: true,
            ts_ns: 6,
        });
        assert!(a.contains(&MapperAction::SoftKey(8)));
    }

    #[test]
    fn step_sequencer_multi_bar_toggle() {
        let mut m = HardwareMapper::with_default();
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Step,
            pressed: true,
            ts_ns: 0,
        });
        assert_eq!(m.mode, ControllerMode::Step);

        // Sélectionne Bar 2 (0-indexed 2)
        m.selected_bar = 2;
        m.active_sound = 3;

        // Press pad 7 (HW) -> ToggleStep sur Bar 2, step 7, sound 3
        let a = m.handle_hardware_event(pad_ev(7, true, 1));
        assert_eq!(a.len(), 1);
        assert_eq!(
            a[0],
            MapperAction::ToggleStep {
                sound: 3,
                bar: 2,
                step: 7,
            }
        );

        // Release silencieux
        let a = m.handle_hardware_event(pad_ev(7, false, 2));
        assert!(a.is_empty());
    }

    fn knob_ev(id: u8, value: u16, ts_ns: u64) -> HardwareEvent {
        use maschine_proto::hid_input::EncoderEvent;
        let _ = ts_ns;
        HardwareEvent::Encoder(EncoderEvent {
            id,
            value: value & 0x3FF,
            touched: true,
        })
    }

    #[test]
    fn step_mode_knobs_edit_selected_step() {
        let mut m = HardwareMapper::with_default();
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Step,
            pressed: true,
            ts_ns: 0,
        });
        assert_eq!(m.mode, ControllerMode::Step);
        m.left_tab = 0;

        // Knob 2 (POSITION) : 4 ticks bruts = +1 tick de nudge (accumulateur).
        let _ = m.handle_hardware_event(knob_ev(2, 100, 1));
        let mut got = Vec::new();
        for i in 0..4u16 {
            got.extend(m.handle_hardware_event(knob_ev(2, 101 + i, 2 + u64::from(i))));
        }
        assert!(
            got.contains(&MapperAction::StepNudgeTicks(1)),
            "got={got:?}"
        );

        // Knob 5 (SCROLL) : 32 ticks bruts = +1 pas dans la grille.
        let _ = m.handle_hardware_event(knob_ev(5, 200, 10));
        let mut got = Vec::new();
        for i in 0..32u16 {
            got.extend(m.handle_hardware_event(knob_ev(5, 201 + i, 11 + u64::from(i))));
        }
        assert_eq!(
            got.iter()
                .filter(|a| matches!(a, MapperAction::StepScroll(1)))
                .count(),
            1,
            "got={got:?}"
        );

        // Knob 6 = VOL SND : delta brut transmis tel quel (mis à l'échelle par l'app).
        let _ = m.handle_hardware_event(knob_ev(6, 300, 100));
        let a = m.handle_hardware_event(knob_ev(6, 303, 101));
        assert!(a.contains(&MapperAction::SoundVolumeDelta(3.0)), "a={a:?}");
    }

    #[test]
    fn step_mode_four_way_scrolls_grid() {
        let mut m = HardwareMapper::with_default();
        m.mode = ControllerMode::Step;
        m.left_tab = 0;

        // 4D droite -> défile la grille d'un pas (ne change pas la mesure locale).
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayRight,
            pressed: true,
            ts_ns: 0,
        });
        assert_eq!(a, vec![MapperAction::StepScroll(1)]);
        assert_eq!(m.selected_bar, 0);

        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayLeft,
            pressed: true,
            ts_ns: 1,
        });
        assert_eq!(a, vec![MapperAction::StepScroll(-1)]);
    }

    #[test]
    fn fast_knob_turns_are_clamped_not_dropped() {
        let mut m = HardwareMapper::with_default();
        m.left_tab = 0;
        let _ = m.handle_hardware_event(knob_ev(1, 0, 0));
        // Tour rapide : 200 ticks d'un coup -> limité à 128, jamais jeté.
        let a = m.handle_hardware_event(knob_ev(1, 200, 1));
        assert_eq!(a.len(), 1, "a={a:?}");
        match a[0] {
            MapperAction::BpmDelta(d) => assert!((d - 128.0).abs() < f32::EPSILON, "d={d}"),
            ref other => panic!("attendu BpmDelta, obtenu {other:?}"),
        }
    }

    #[test]
    fn mixer_knobs_edit_selected_sound_and_shift_edits_groups() {
        let mut m = HardwareMapper::with_default();
        m.left_tab = 1; // MIXER
        let _ = m.handle_hardware_event(knob_ev(1, 0, 0));
        let a = m.handle_hardware_event(knob_ev(1, 4, 1));
        // En mode Mixer Pads 1-8 (défaut), le bouton 1 règle le volume du pad 1 directement
        assert_eq!(
            a,
            vec![MapperAction::SoundVolumeDeltaFor {
                group: 0,
                sound: 0,
                delta: 4.0
            }]
        );

        m.shift_held = true;
        let _ = m.handle_hardware_event(knob_ev(1, 4, 2));
        let a = m.handle_hardware_event(knob_ev(1, 8, 3));
        // Shift en mode Mixer Pads règle le panoramique du pad 1
        assert_eq!(
            a,
            vec![MapperAction::SoundPanDeltaFor {
                group: 0,
                sound: 0,
                delta: 4.0
            }]
        );

        // En mode Mixer Groupes (mode 2)
        m.mixer_mode = 2;
        m.shift_held = false;
        let _ = m.handle_hardware_event(knob_ev(1, 8, 4));
        let a = m.handle_hardware_event(knob_ev(1, 12, 5));
        assert_eq!(
            a,
            vec![MapperAction::GroupVolumeDeltaFor {
                group: 0,
                delta: 4.0
            }]
        );
    }

    #[test]
    fn selecting_main_view_clears_events_overlay() {
        let mut m = HardwareMapper::with_default();
        let events = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Events,
            pressed: true,
            ts_ns: 1,
        });
        assert!(events.contains(&MapperAction::SetRightTab(1)));
        assert_eq!(m.right_tab, 1);

        let sampler = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Sampling,
            pressed: true,
            ts_ns: 2,
        });
        assert!(sampler.contains(&MapperAction::SetLeftTab(2)));
        assert_eq!(m.left_tab, 2);
        assert_eq!(m.right_tab, 0);
    }

    fn press_button(m: &mut HardwareMapper, id: maschine_proto::ButtonId) -> Vec<MapperAction> {
        m.handle_hardware_event(HardwareEvent::Button {
            id,
            pressed: true,
            ts_ns: 1,
        })
    }

    #[test]
    fn step_button_latches_and_toggles_back_to_pad() {
        let mut m = HardwareMapper::with_default();
        let on = press_button(&mut m, maschine_proto::ButtonId::Step);
        assert_eq!(m.mode, ControllerMode::Step);
        assert!(on.contains(&MapperAction::ModeSwitch(ControllerMode::Step)));
        assert!(on.contains(&MapperAction::SetControllerMode(3)));
        assert!(on.contains(&MapperAction::NavigateView(nav::STEP)));
        assert!(on.contains(&MapperAction::SetLeftTab(0)));

        let off = press_button(&mut m, maschine_proto::ButtonId::Step);
        assert_eq!(m.mode, ControllerMode::Pad);
        assert!(off.contains(&MapperAction::ModeSwitch(ControllerMode::Pad)));
        assert!(off.contains(&MapperAction::SetControllerMode(0)));
        assert!(off.contains(&MapperAction::NavigateView(nav::PATTERN)));
    }

    #[test]
    fn keyboard_button_latches_and_toggles_back_to_pad() {
        let mut m = HardwareMapper::with_default();
        let on = press_button(&mut m, maschine_proto::ButtonId::Keyboard);
        assert_eq!(m.mode, ControllerMode::Keyboard);
        assert!(on.contains(&MapperAction::SetControllerMode(1)));
        assert!(on.contains(&MapperAction::NavigateView(nav::KEYBOARD)));

        let off = press_button(&mut m, maschine_proto::ButtonId::Keyboard);
        assert_eq!(m.mode, ControllerMode::Pad);
        assert!(off.contains(&MapperAction::SetControllerMode(0)));
    }

    #[test]
    fn pattern_mode_duplicate_clear_and_step_actions() {
        let mut m = HardwareMapper::with_default();
        let _ = press_button(&mut m, maschine_proto::ButtonId::Pattern);

        let dup = press_button(&mut m, maschine_proto::ButtonId::Duplicate);
        assert!(dup.contains(&MapperAction::DuplicatePattern { group: 0 }));
        assert!(!dup.contains(&MapperAction::DuplicateBar));

        let next = press_button(&mut m, maschine_proto::ButtonId::FourWayRight);
        assert!(next.contains(&MapperAction::PatternStep { group: 0, delta: 1 }));
        let prev = press_button(&mut m, maschine_proto::ButtonId::FourWayLeft);
        assert!(prev.contains(&MapperAction::PatternStep {
            group: 0,
            delta: -1
        }));

        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Shift,
            pressed: true,
            ts_ns: 2,
        });
        let clear = press_button(&mut m, maschine_proto::ButtonId::Erase);
        assert!(clear.contains(&MapperAction::ClearPattern { group: 0 }));
        assert!(!clear.contains(&MapperAction::ClearBar(0)));
    }

    #[test]
    fn pattern_and_scene_buttons_latch_and_exclude_each_other() {
        let mut m = HardwareMapper::with_default();
        let pattern = press_button(&mut m, maschine_proto::ButtonId::Pattern);
        assert!(m.pattern_select_mode);
        assert!(!m.scene_select_mode);
        assert!(pattern.contains(&MapperAction::SetPatternMode(true)));
        assert!(pattern.contains(&MapperAction::NavigateView(nav::PATTERN)));

        let scene = press_button(&mut m, maschine_proto::ButtonId::Scene);
        assert!(!m.pattern_select_mode);
        assert!(m.scene_select_mode);
        assert!(scene.contains(&MapperAction::SetPatternMode(false)));
        assert!(scene.contains(&MapperAction::SetSceneMode(true)));
        assert!(scene.contains(&MapperAction::NavigateView(nav::SCENE)));

        let scene_off = press_button(&mut m, maschine_proto::ButtonId::Scene);
        assert!(!m.scene_select_mode);
        assert!(scene_off.contains(&MapperAction::SetSceneMode(false)));
        assert!(scene_off.contains(&MapperAction::NavigateView(nav::PATTERN)));
    }

    #[test]
    fn events_macro_and_settings_are_toggle_menus() {
        let mut m = HardwareMapper::with_default();
        let events = press_button(&mut m, maschine_proto::ButtonId::Events);
        assert_eq!(m.right_tab, 1);
        assert!(events.contains(&MapperAction::SetRightTab(1)));
        let events_off = press_button(&mut m, maschine_proto::ButtonId::Events);
        assert_eq!(m.right_tab, 0);
        assert!(events_off.contains(&MapperAction::SetRightTab(0)));

        let macro_on = press_button(&mut m, maschine_proto::ButtonId::Macro);
        assert_eq!(m.right_tab, 2);
        assert!(macro_on.contains(&MapperAction::SetRightTab(2)));
        let _ = press_button(&mut m, maschine_proto::ButtonId::Macro);
        assert_eq!(m.right_tab, 0);

        let settings = press_button(&mut m, maschine_proto::ButtonId::Settings);
        assert_eq!(m.right_tab, 3);
        assert!(settings.contains(&MapperAction::SetRightTab(3)));
        let _ = press_button(&mut m, maschine_proto::ButtonId::Settings);
        assert_eq!(m.right_tab, 0);
    }

    #[test]
    fn plugin_button_latches_and_toggles_back_to_pattern() {
        let mut m = HardwareMapper::with_default();
        let on = press_button(&mut m, maschine_proto::ButtonId::Plugin);
        assert_eq!(m.left_tab, 4);
        assert!(on.contains(&MapperAction::SetLeftTab(4)));
        // La page (SOUND/TB-303) est choisie par le moteur selon le Sound actif.
        assert!(on.contains(&MapperAction::NavigateView(nav::PLUGIN)));

        let off = press_button(&mut m, maschine_proto::ButtonId::Plugin);
        assert_eq!(m.left_tab, 0);
        assert!(off.contains(&MapperAction::SetLeftTab(0)));
        assert!(off.contains(&MapperAction::NavigateView(nav::PATTERN)));
    }

    #[test]
    fn file_save_and_browser_distinction_and_leds() {
        use maschine_proto::hid_output::offsets;

        let mut m = HardwareMapper::with_default();

        // 1. Appui FILE_SAVE -> tab 3, browser_kind 1, action OpenFileBrowser
        let act_file = press_button(&mut m, maschine_proto::ButtonId::FileSave);
        assert_eq!(m.left_tab, 3);
        assert_eq!(m.browser_kind, 1);
        assert!(act_file.contains(&MapperAction::OpenFileBrowser));

        let info_file = SequencerLedInfo {
            left_tab: 3,
            browser_kind: 1,
            ..Default::default()
        };
        m.update_leds(&info_file);

        // La LED FILE_SAVE DOIT être allumée (7), et BROWSER DOIT être éteinte/dim (1)
        assert_eq!(m.led_state().button_leds[offsets::FILE_SAVE - 1], 7);
        assert_eq!(m.led_state().button_leds[offsets::BROWSER - 1], 1);

        // Bouton 3 (NOUVEAU) et 4 (CHARGER) mis en valeur
        assert_eq!(m.led_state().button_leds[offsets::DISPLAY_BTN_3 - 1], 6);
        assert_eq!(m.led_state().button_leds[offsets::DISPLAY_BTN_4 - 1], 6);

        // 2. Second appui FILE_SAVE -> toggle vers tab 0 (referme le menu)
        let act_close = press_button(&mut m, maschine_proto::ButtonId::FileSave);
        assert_eq!(m.left_tab, 0);
        assert!(act_close.contains(&MapperAction::SetLeftTab(0)));

        // 3. Appui BROWSER -> tab 3, browser_kind 0, action SetBrowserKind(0)
        let act_browse = press_button(&mut m, maschine_proto::ButtonId::Browser);
        assert_eq!(m.left_tab, 3);
        assert_eq!(m.browser_kind, 0);
        assert!(act_browse.contains(&MapperAction::SetBrowserKind(0)));

        let info_browser = SequencerLedInfo {
            left_tab: 3,
            browser_kind: 0,
            mixer_mode: 0,
            has_kick_synth: false,
            has_acid_synth: false,
            has_wavetable_synth: false,
            delete_project_confirm: false,
            ..Default::default()
        };
        m.update_leds(&info_browser);

        // La LED BROWSER DOIT être allumée (7), et FILE_SAVE DOIT être éteinte/dim (1)
        assert_eq!(m.led_state().button_leds[offsets::BROWSER - 1], 7);
        assert_eq!(m.led_state().button_leds[offsets::FILE_SAVE - 1], 1);

        // 4. Shift enfoncé : FILE_SAVE brille en hint (4) et déclenche QuickSaveProject
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Shift,
            pressed: true,
            ts_ns: 10,
        });
        m.update_leds(&info_browser);
        assert_eq!(m.led_state().button_leds[offsets::FILE_SAVE - 1], 4);

        let act_quick = press_button(&mut m, maschine_proto::ButtonId::FileSave);
        assert!(act_quick.contains(&MapperAction::QuickSaveProject));
    }

    #[test]
    fn mixer_encoder_drills_down_to_pads_and_returns() {
        let mut m = HardwareMapper::with_default();
        m.left_tab = 1; // MIXER
        m.mixer_mode = 2; // Vue groupes

        // 1. EncoderPush rentre dans le groupe -> vue pads 1-8 (mode 0)
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::EncoderPush,
            pressed: true,
            ts_ns: 0,
        });
        assert_eq!(a, vec![MapperAction::SetMixerMode(0)]);
        assert_eq!(m.mixer_mode, 0);

        // 2. FourWayRight bascule vers pads 9-16 (mode 1)
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayRight,
            pressed: true,
            ts_ns: 1,
        });
        assert_eq!(a, vec![MapperAction::SetMixerMode(1)]);
        assert_eq!(m.mixer_mode, 1);

        // 3. FourWayLeft revient aux pads 1-8 (mode 0)
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayLeft,
            pressed: true,
            ts_ns: 2,
        });
        assert_eq!(a, vec![MapperAction::SetMixerMode(0)]);
        assert_eq!(m.mixer_mode, 0);

        // 4. Second FourWayLeft remonte au mixeur général des groupes (mode 2)
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayLeft,
            pressed: true,
            ts_ns: 3,
        });
        assert_eq!(a, vec![MapperAction::SetMixerMode(2)]);
        assert_eq!(m.mixer_mode, 2);

        // 5. EncoderPush depuis la vue groupes redescend immédiatement
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::EncoderPush,
            pressed: true,
            ts_ns: 4,
        });
        assert_eq!(a, vec![MapperAction::SetMixerMode(0)]);
        assert_eq!(m.mixer_mode, 0);

        // 6. FourWayDown depuis la vue pads remonte au mixeur groupes (mode 2)
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayDown,
            pressed: true,
            ts_ns: 5,
        });
        assert_eq!(a, vec![MapperAction::SetMixerMode(2)]);
        assert_eq!(m.mixer_mode, 2);

        // 7. FourWayUp depuis la vue groupes rentre dans la vue pads (mode 0)
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayUp,
            pressed: true,
            ts_ns: 6,
        });
        assert_eq!(a, vec![MapperAction::SetMixerMode(0)]);
        assert_eq!(m.mixer_mode, 0);
    }

    #[test]
    fn top_row_buttons_exit_latched_pattern_events_mute_modes() {
        let mut m = HardwareMapper::with_default();

        let tap_mute = |m: &mut HardwareMapper| {
            m.handle_hardware_event(HardwareEvent::Button {
                id: maschine_proto::ButtonId::Mute,
                pressed: true,
                ts_ns: 0,
            });
            m.handle_hardware_event(HardwareEvent::Button {
                id: maschine_proto::ButtonId::Mute,
                pressed: false,
                ts_ns: 1,
            });
        };

        // 1. En mode PATTERN, appuyer sur FILE_SAVE désactive PATTERN et ouvre FILE
        let _ = press_button(&mut m, maschine_proto::ButtonId::Pattern);
        assert!(m.pattern_select_mode);
        let a = press_button(&mut m, maschine_proto::ButtonId::FileSave);
        assert!(!m.pattern_select_mode);
        assert!(a.contains(&MapperAction::SetPatternMode(false)));
        assert_eq!(m.left_tab, 3);
        assert_eq!(m.browser_kind, 1);

        // 2. En mode PATTERN, appuyer sur BROWSER désactive PATTERN et ouvre BROWSER
        let _ = press_button(&mut m, maschine_proto::ButtonId::Pattern);
        assert!(m.pattern_select_mode);
        let a = press_button(&mut m, maschine_proto::ButtonId::Browser);
        assert!(!m.pattern_select_mode);
        assert!(a.contains(&MapperAction::SetPatternMode(false)));
        assert_eq!(m.left_tab, 3);
        assert_eq!(m.browser_kind, 0);

        // 3. En mode PATTERN, appuyer sur PLUGIN désactive PATTERN et ouvre PLUGIN
        let _ = press_button(&mut m, maschine_proto::ButtonId::Pattern);
        assert!(m.pattern_select_mode);
        let a = press_button(&mut m, maschine_proto::ButtonId::Plugin);
        assert!(!m.pattern_select_mode);
        assert!(a.contains(&MapperAction::SetPatternMode(false)));
        assert_eq!(m.left_tab, 4);

        // 4. En mode EVENTS (overlay écran droit), appuyer sur BROWSER referme EVENTS
        let _ = press_button(&mut m, maschine_proto::ButtonId::Events);
        assert_eq!(m.right_tab, 1);
        let a = press_button(&mut m, maschine_proto::ButtonId::Browser);
        assert_eq!(m.right_tab, 0);
        assert!(a.contains(&MapperAction::SetRightTab(0)));

        // 5. En mode MUTE pad, appuyer sur MIXER désactive le mode MUTE
        tap_mute(&mut m);
        assert!(m.mute_pad_mode);
        let _ = press_button(&mut m, maschine_proto::ButtonId::Mixer);
        assert!(!m.mute_pad_mode);
        assert_eq!(m.left_tab, 1);

        // 6. En mode combiné PATTERN + EVENTS + MUTE, FILE_SAVE désactive tout d'un coup
        let _ = press_button(&mut m, maschine_proto::ButtonId::Pattern);
        let _ = press_button(&mut m, maschine_proto::ButtonId::Events);
        tap_mute(&mut m);
        assert!(m.pattern_select_mode);
        assert_eq!(m.right_tab, 1);
        assert!(m.mute_pad_mode);

        let a = press_button(&mut m, maschine_proto::ButtonId::FileSave);
        assert!(!m.pattern_select_mode);
        assert_eq!(m.right_tab, 0);
        assert!(!m.mute_pad_mode);
        assert!(a.contains(&MapperAction::SetPatternMode(false)));
        assert!(a.contains(&MapperAction::SetRightTab(0)));
        assert_eq!(m.left_tab, 3);
        assert_eq!(m.browser_kind, 1);
    }

    #[test]
    fn hardware_filter_and_kick_knobs_cycle_modes() {
        let mut m = HardwareMapper::with_default();

        // 1. En vue SAMPLER (left_tab = 2), Shift + Knob 3 (index 2) alterne le type de filtre
        m.left_tab = 2;
        m.shift_held = true;
        let _ = m.handle_hardware_event(knob_ev(3, 100, 0));
        let a = m.handle_hardware_event(knob_ev(3, 101, 1));
        assert_eq!(a, vec![MapperAction::CycleSoundFilterKind]);

        // 2. En vue KICK SYNTH (left_tab = 4, plugin_page = 1, has_kick_synth = true)
        m.left_tab = 4;
        m.plugin_page = 1;
        m.has_kick_synth = true;
        m.shift_held = true;

        // Shift + Knob 6 (index 5) = Disto Type
        let _ = m.handle_hardware_event(knob_ev(6, 200, 2));
        let a = m.handle_hardware_event(knob_ev(6, 201, 3));
        assert_eq!(a, vec![MapperAction::KickDriveTypeDelta(1)]);

        // Shift + Knob 8 (index 7) = Filter Mode
        let _ = m.handle_hardware_event(knob_ev(8, 300, 4));
        let a = m.handle_hardware_event(knob_ev(8, 301, 5));
        assert_eq!(a, vec![MapperAction::KickFilterModeDelta(1)]);

        // 3. En vue WAVETABLE SYNTH (left_tab = 4, plugin_page = 1, has_wavetable_synth = true)
        m.has_kick_synth = false;
        m.has_wavetable_synth = true;
        m.shift_held = true;

        // Shift + Knob 1 (index 0) = Cycle filter mode
        let _ = m.handle_hardware_event(knob_ev(1, 400, 6));
        let a = m.handle_hardware_event(knob_ev(1, 401, 7));
        assert_eq!(a, vec![MapperAction::WavetableFilterModeCycle]);

        // Regular Knob 1 (Cutoff)
        m.shift_held = false;
        let _ = m.handle_hardware_event(knob_ev(1, 401, 8));
        let a = m.handle_hardware_event(knob_ev(1, 402, 9));
        assert_eq!(a, vec![MapperAction::WavetableCutoffDelta(1.0)]);
    }

    #[test]
    fn test_pattern_chaining_via_two_pads() {
        use crate::events::PadState;
        let mut m = HardwareMapper::with_default();
        let _ = press_button(&mut m, maschine_proto::ButtonId::Pattern);
        assert!(m.pattern_select_mode);

        // Premier pad enfoncé (Pad 1)
        let a1 = m.handle_hardware_event(HardwareEvent::Pad(PadState {
            pad: 1,
            pressed: true,
            velocity: 100,
            pressure: 0x4000,
            ts_ns: 0,
        }));
        assert_eq!(
            a1,
            vec![MapperAction::PatternTrigger {
                group: 0,
                pattern: 1
            }]
        );

        // Deuxième pad enfoncé simultanément (Pad 3)
        let a2 = m.handle_hardware_event(HardwareEvent::Pad(PadState {
            pad: 3,
            pressed: true,
            velocity: 100,
            pressure: 0x4000,
            ts_ns: 1,
        }));
        assert_eq!(
            a2,
            vec![MapperAction::PatternChain {
                group: 0,
                patterns: vec![1, 2, 3]
            }]
        );
    }

    #[test]
    fn test_lock_button_and_leds() {
        use maschine_proto::hid_output::offsets;
        let mut m = HardwareMapper::with_default();

        // 1. Bouton LOCK bascule le verrouillage
        let a = press_button(&mut m, maschine_proto::ButtonId::Lock);
        assert_eq!(a, vec![MapperAction::ToggleLock(true)]);
        assert!(m.lock_active);
        assert_eq!(m.led_state().button_leds[offsets::LOCK - 1], 7);

        // 2. Second appui désactive le verrou
        let a2 = press_button(&mut m, maschine_proto::ButtonId::Lock);
        assert_eq!(a2, vec![MapperAction::ToggleLock(false)]);
        assert!(!m.lock_active);
        assert_eq!(m.led_state().button_leds[offsets::LOCK - 1], 1);

        // 3. Note Repeat allume aussi la LED LOCK
        m.note_repeat = true;
        m.update_leds(&SequencerLedInfo::default());
        assert_eq!(m.led_state().button_leds[offsets::LOCK - 1], 7);
        assert_eq!(m.led_state().button_leds[offsets::NOTE_REPEAT - 1], 7);
    }

    #[test]
    fn test_wavetable_synth_all_knobs_and_shift() {
        let mut m = HardwareMapper::with_default();
        m.left_tab = 4; // Plugin tab
        m.has_wavetable_synth = true;

        // Knobs 1..8 réguliers en vue Wavetable Synth
        let tests = [
            (1, MapperAction::WavetableCutoffDelta(1.0)),
            (2, MapperAction::WavetableResoDelta(1.0)),
            (3, MapperAction::WavetableEnvAmtDelta(1.0)),
            (4, MapperAction::WavetableDriveDelta(1.0)),
            (5, MapperAction::WavetableAttackDelta(1.0)),
            (6, MapperAction::WavetableDecayDelta(1.0)),
            (7, MapperAction::WavetableSustainDelta(1.0)),
            (8, MapperAction::WavetableReleaseDelta(1.0)),
        ];

        for (knob, expected) in tests {
            let _ = m.handle_hardware_event(knob_ev(knob, 100, 0));
            let a = m.handle_hardware_event(knob_ev(knob, 101, 1));
            assert_eq!(a, vec![expected], "Knob {knob} en vue Wavetable");
        }

        // Avec Shift : Knobs 1..8
        m.shift_held = true;
        let shift_tests = [
            (1, MapperAction::WavetableFilterModeCycle),
            (2, MapperAction::WavetableUnisonVoicesDelta(1)),
            (3, MapperAction::WavetableOsc1WaveCycle),
            (4, MapperAction::WavetableOsc2WaveCycle),
            (5, MapperAction::WavetableChorusDelta(1.0)),
            (6, MapperAction::WavetableDelayDelta(1.0)),
            (7, MapperAction::WavetableReverbDelta(1.0)),
            (8, MapperAction::WavetableVolumeDelta(1.0)),
        ];

        for (knob, expected) in shift_tests {
            let _ = m.handle_hardware_event(knob_ev(knob, 200, 2));
            let a = m.handle_hardware_event(knob_ev(knob, 201, 3));
            assert_eq!(a, vec![expected], "Shift + Knob {knob} en vue Wavetable");
        }
    }

    #[test]
    fn test_display_buttons_shift_context_routing() {
        let mut m = HardwareMapper::with_default();

        // 1. Boutons d'écran 1..8 émettent SoftKey(1..8)
        for n in 1..=8 {
            let a = m.handle_hardware_event(HardwareEvent::Button {
                id: maschine_proto::ButtonId::DisplayBtn(n),
                pressed: true,
                ts_ns: n as u64,
            });
            assert_eq!(a, vec![MapperAction::SoftKey(n)]);
        }

        // 2. Avec Shift, les boutons émettent toujours SoftKey(n) pour
        // que l'Engine applique le contexte exactement aligné sur les labels
        m.shift_held = true;
        for n in 1..=8 {
            let a = m.handle_hardware_event(HardwareEvent::Button {
                id: maschine_proto::ButtonId::DisplayBtn(n),
                pressed: true,
                ts_ns: 10 + n as u64,
            });
            assert_eq!(a, vec![MapperAction::SoftKey(n)]);
        }
    }

    #[test]
    fn test_mixer_knobs_pad_track_isolation() {
        let mut m = HardwareMapper::with_default();
        m.left_tab = 1; // Mixer
        m.mixer_mode = 0; // Pads 1..8 du groupe focus
        m.group_focus = 2; // Groupe C

        // Knob 1 -> SoundVolumeDeltaFor (Group 2, Sound 0)
        let _ = m.handle_hardware_event(knob_ev(1, 100, 0));
        let a = m.handle_hardware_event(knob_ev(1, 102, 1));
        assert_eq!(
            a,
            vec![MapperAction::SoundVolumeDeltaFor {
                group: 2,
                sound: 0,
                delta: 2.0
            }]
        );

        // Knob 5 -> SoundVolumeDeltaFor (Group 2, Sound 4)
        let _ = m.handle_hardware_event(knob_ev(5, 100, 2));
        let a = m.handle_hardware_event(knob_ev(5, 103, 3));
        assert_eq!(
            a,
            vec![MapperAction::SoundVolumeDeltaFor {
                group: 2,
                sound: 4,
                delta: 3.0
            }]
        );

        // Passage en Pads 9..16
        m.mixer_mode = 1;
        let _ = m.handle_hardware_event(knob_ev(1, 100, 4));
        let a = m.handle_hardware_event(knob_ev(1, 101, 5));
        assert_eq!(
            a,
            vec![MapperAction::SoundVolumeDeltaFor {
                group: 2,
                sound: 8,
                delta: 1.0
            }]
        );

        // Shift + Knob en mixer mode = Pan de la piste
        m.shift_held = true;
        let _ = m.handle_hardware_event(knob_ev(1, 100, 6));
        let a = m.handle_hardware_event(knob_ev(1, 101, 7));
        assert_eq!(
            a,
            vec![MapperAction::SoundPanDeltaFor {
                group: 2,
                sound: 8,
                delta: 1.0
            }]
        );
    }

    #[test]
    fn test_4d_encoder_shift_push_cycles_filter_in_all_views() {
        let mut m = HardwareMapper::with_default();
        m.shift_held = true;

        // Vue par défaut (tab 0)
        m.left_tab = 0;
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::EncoderPush,
            pressed: true,
            ts_ns: 1,
        });
        assert_eq!(a, vec![MapperAction::CycleSoundFilterKind]);

        // Vue Plugin (tab 4)
        m.left_tab = 4;
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::EncoderPush,
            pressed: true,
            ts_ns: 2,
        });
        assert_eq!(a, vec![MapperAction::CycleSoundFilterKind]);

        // Vue Mixer (tab 1)
        m.left_tab = 1;
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::EncoderPush,
            pressed: true,
            ts_ns: 3,
        });
        assert_eq!(a, vec![MapperAction::CycleSoundFilterKind]);
    }

    #[test]
    fn test_knobs_shift_filter_cycle_in_plugin_and_arrange() {
        let mut m = HardwareMapper::with_default();
        m.shift_held = true;

        // 1. En vue arrangeur (tab 0) : knob 6 (cutoff) et knob 7 (reso) avec shift cyclent le filtre
        m.left_tab = 0;
        let _ = m.handle_hardware_event(knob_ev(6, 100, 0));
        let a = m.handle_hardware_event(knob_ev(6, 101, 1));
        assert_eq!(a, vec![MapperAction::CycleSoundFilterKind]);

        let _ = m.handle_hardware_event(knob_ev(7, 100, 2));
        let a = m.handle_hardware_event(knob_ev(7, 101, 3));
        assert_eq!(a, vec![MapperAction::CycleSoundFilterKind]);

        // 2. En vue plugin standard (tab 4) : knob 3 (cutoff) et knob 4 (reso) avec shift cyclent le filtre
        m.left_tab = 4;
        m.plugin_page = 0;
        m.has_kick_synth = false;
        m.has_wavetable_synth = false;
        m.has_acid_synth = false;

        let _ = m.handle_hardware_event(knob_ev(3, 100, 4));
        let a = m.handle_hardware_event(knob_ev(3, 101, 5));
        assert_eq!(a, vec![MapperAction::CycleSoundFilterKind]);

        let _ = m.handle_hardware_event(knob_ev(4, 100, 6));
        let a = m.handle_hardware_event(knob_ev(4, 101, 7));
        assert_eq!(a, vec![MapperAction::CycleSoundFilterKind]);
    }

    #[test]
    fn test_all_eight_groups_and_pads_routing() {
        let mut m = HardwareMapper::with_default();

        // Vérification de la sélection des 8 groupes A..H via boutons hardware
        for expected_group in 0..8u8 {
            let a = m.handle_hardware_event(HardwareEvent::Button {
                id: maschine_proto::ButtonId::Group(expected_group),
                pressed: true,
                ts_ns: expected_group as u64,
            });
            assert_eq!(a, vec![MapperAction::GroupFocus(expected_group)]);
            assert_eq!(m.group_focus, expected_group);

            // Vérifier que chaque pad (0..15) dans ce groupe émet bien son PadTrigger distinct
            for pad in 0..16u8 {
                let pad_down =
                    m.handle_hardware_event(HardwareEvent::Pad(crate::events::PadState {
                        pad,
                        pressed: true,
                        velocity: 100,
                        pressure: 0x4500,
                        ts_ns: pad as u64,
                    }));
                assert!(pad_down.contains(&MapperAction::SelectSound(pad)));
                assert!(pad_down.contains(&MapperAction::PadTrigger {
                    pad,
                    velocity: 100,
                    pressed: true,
                }));

                let pad_up = m.handle_hardware_event(HardwareEvent::Pad(crate::events::PadState {
                    pad,
                    pressed: false,
                    velocity: 0,
                    pressure: 0x4000,
                    ts_ns: pad as u64 + 10,
                }));
                assert!(pad_up.contains(&MapperAction::PadTrigger {
                    pad,
                    velocity: 0,
                    pressed: false,
                }));
            }
        }
    }

    #[test]
    fn test_mute_and_solo_with_groups() {
        let mut m = HardwareMapper::with_default();
        m.group_focus = 0;

        // 1. Appui Mute + Group B(1) : ToggleMute(1) sans changer le focus (focus reste 0)
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Mute,
            pressed: true,
            ts_ns: 1,
        });
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Group(1),
            pressed: true,
            ts_ns: 2,
        });
        assert_eq!(a, vec![MapperAction::ToggleMute(1)]);
        assert_eq!(m.group_focus, 0);

        // Relâchement Mute après avoir utilisé un groupe : ne bascule pas le mute_pad_mode
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Mute,
            pressed: false,
            ts_ns: 3,
        });
        assert!(a.is_empty());
        assert!(!m.mute_pad_mode);

        // 2. Appui simple Mute (sans groupe) : active le mute_pad_mode latched
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Mute,
            pressed: true,
            ts_ns: 4,
        });
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Mute,
            pressed: false,
            ts_ns: 5,
        });
        assert!(m.mute_pad_mode);

        // En mute_pad_mode, Group C(2) mute le groupe directement
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Group(2),
            pressed: true,
            ts_ns: 6,
        });
        assert_eq!(a, vec![MapperAction::ToggleMute(2)]);
        assert_eq!(m.group_focus, 0);

        // 3. Solo + Group D(3) : ToggleSolo(3) sans changer focus
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Solo,
            pressed: true,
            ts_ns: 7,
        });
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Group(3),
            pressed: true,
            ts_ns: 8,
        });
        assert_eq!(a, vec![MapperAction::ToggleSolo(3)]);
        assert_eq!(m.group_focus, 0);
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Solo,
            pressed: false,
            ts_ns: 9,
        });

        // 4. Solo simple (sans groupe) : bascule le solo sur le group_focus (0)
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Solo,
            pressed: true,
            ts_ns: 10,
        });
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Solo,
            pressed: false,
            ts_ns: 11,
        });
        assert_eq!(a, vec![MapperAction::ToggleSolo(0)]);
    }

    #[test]
    fn test_erase_mode_combinations() {
        let mut m = HardwareMapper::with_default();
        m.group_focus = 1;
        m.selected_bar = 2;

        // 1. Shift + Erase en mode pattern : ClearPattern sur group_focus
        m.pattern_select_mode = true;
        m.shift_held = true;
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Erase,
            pressed: true,
            ts_ns: 1,
        });
        assert_eq!(a, vec![MapperAction::ClearPattern { group: 1 }]);
        m.pattern_select_mode = false;
        m.shift_held = false;

        // 2. Shift + Erase en mode normal : ClearBar sur selected_bar
        m.shift_held = true;
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Erase,
            pressed: true,
            ts_ns: 2,
        });
        assert_eq!(a, vec![MapperAction::ClearBar(2)]);
        m.shift_held = false;

        // 3. Erase en mode FILE (left_tab = 3, browser_kind = 1) déclenche SoftKey(6) (Suppression projet)
        m.left_tab = 3;
        m.browser_kind = 1;
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Erase,
            pressed: true,
            ts_ns: 3,
        });
        assert_eq!(a, vec![MapperAction::SoftKey(6)]);
    }

    #[test]
    fn test_4d_encoder_navigation_and_transport_shortcuts() {
        let mut m = HardwareMapper::with_default();

        // 1. FourWayUp / FourWayDown en vue Browser (tab 3)
        m.left_tab = 3;
        let a_up = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayUp,
            pressed: true,
            ts_ns: 1,
        });
        assert_eq!(a_up, vec![MapperAction::BrowsePresetStep(-1)]);

        let a_down = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayDown,
            pressed: true,
            ts_ns: 2,
        });
        assert_eq!(a_down, vec![MapperAction::BrowsePresetStep(1)]);

        let a_push = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::EncoderPush,
            pressed: true,
            ts_ns: 3,
        });
        assert_eq!(a_push, vec![MapperAction::LoadSelectedPreset]);

        // 2. FourWayLeft / FourWayRight en mode Pattern
        m.left_tab = 0;
        m.pattern_select_mode = true;
        m.group_focus = 2;
        let a_left = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayLeft,
            pressed: true,
            ts_ns: 4,
        });
        assert_eq!(
            a_left,
            vec![MapperAction::PatternStep {
                group: 2,
                delta: -1
            }]
        );

        let a_right = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::FourWayRight,
            pressed: true,
            ts_ns: 5,
        });
        assert_eq!(
            a_right,
            vec![MapperAction::PatternStep { group: 2, delta: 1 }]
        );
        m.pattern_select_mode = false;

        // 3. FourWayLeft / FourWayRight en mode Step
        m.mode = ControllerMode::Step;
        let a_sleft = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Left,
            pressed: true,
            ts_ns: 6,
        });
        assert_eq!(a_sleft, vec![MapperAction::StepScroll(-1)]);

        let a_sright = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Right,
            pressed: true,
            ts_ns: 7,
        });
        assert_eq!(a_sright, vec![MapperAction::StepScroll(1)]);

        // 4. Footswitch Pedal déclenche PlayPauseToggle
        let a_pedal = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Pedal,
            pressed: true,
            ts_ns: 8,
        });
        assert_eq!(a_pedal, vec![MapperAction::PlayPauseToggle]);

        // 5. Restart normal vs Shift+Restart (ToggleLoop)
        let a_restart = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Restart,
            pressed: true,
            ts_ns: 9,
        });
        assert_eq!(a_restart, vec![MapperAction::Restart]);

        m.shift_held = true;
        let a_loop = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Restart,
            pressed: true,
            ts_ns: 10,
        });
        assert_eq!(a_loop, vec![MapperAction::ToggleLoop]);
    }

    #[test]
    fn test_note_repeat_and_touchstrip_modes() {
        let mut m = HardwareMapper::with_default();

        // 1. Bouton NoteRepeat active le mode
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::NoteRepeat,
            pressed: true,
            ts_ns: 1,
        });
        assert_eq!(a, vec![MapperAction::SetNoteRepeat(true)]);
        assert!(m.note_repeat);

        // Quand note_repeat est actif, DisplayBtn 1..7 changent le débit (rates 0..6)
        for btn in 1..=7u8 {
            let a = m.handle_hardware_event(HardwareEvent::Button {
                id: maschine_proto::ButtonId::DisplayBtn(btn),
                pressed: true,
                ts_ns: btn as u64 + 10,
            });
            assert_eq!(a, vec![MapperAction::SetNoteRepeatRate(btn - 1)]);
            assert_eq!(m.note_repeat_rate, btn - 1);
        }

        // DisplayBtn 8 désactive note_repeat
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::DisplayBtn(8),
            pressed: true,
            ts_ns: 20,
        });
        assert_eq!(a, vec![MapperAction::SetNoteRepeat(false)]);
        assert!(!m.note_repeat);

        // 2. Modes Touchstrip
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Pitch,
            pressed: true,
            ts_ns: 21,
        });
        assert_eq!(m.touchstrip_mode, TouchStripMode::Pitch);

        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Mod,
            pressed: true,
            ts_ns: 22,
        });
        assert_eq!(m.touchstrip_mode, TouchStripMode::Mod);

        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Perform,
            pressed: true,
            ts_ns: 23,
        });
        assert_eq!(m.touchstrip_mode, TouchStripMode::Perform);

        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Notes,
            pressed: true,
            ts_ns: 24,
        });
        assert_eq!(m.touchstrip_mode, TouchStripMode::Notes);
    }
}
