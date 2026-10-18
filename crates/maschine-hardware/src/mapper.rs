//! Mapper HID → MIDI / Engine.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use maschine_proto::color::{Color, PaletteIndex};
use maschine_proto::hid_output::LedState;

use crate::events::HardwareEvent;

/// Durée du flash blanc de frappe sur un pad (feedback tactile garanti même
/// pour un tap plus court qu'une frame LED 33ms), façon Maschine.
const PAD_FLASH_MS: u64 = 200;

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
    /// Déclenche une scène (mode SCENE, pads 1..16 -> scènes 0..15)
    SceneTrigger(usize),
    /// Sélectionne un pattern du groupe focus pour la scène active (mode PATTERN)
    PatternTrigger { group: u8, pattern: usize },
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
    /// Humanisation / Variation
    Variation,
    /// Basculer mode Song / Arranger
    ToggleSongMode,
    /// Quantification
    Quantize,
    /// Sauvegarder le projet
    SaveProject,
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
    /// Pas sélectionné (curseur d'édition step sequencer, 0..pattern_bars*16-1)
    pub step_cursor: usize,
    pub triggered_pads: [bool; 16],
    pub master_vol: f32,
    pub muted_groups: u8,
    pub soloed_groups: u8,
    pub left_tab: u8,
    pub right_tab: u8,
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
    /// Onglet écran gauche (0..3)
    pub left_tab: u8,
    /// Onglet écran droit (0..3)
    pub right_tab: u8,
    /// Son actif sélectionné pour édition Step/Keyboard (0..15)
    pub active_sound: u8,
    /// Preset sélectionné dans le Browser (0..15)
    pub browser_preset_idx: u8,
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
    /// Bouton Mute tenu (MUTE + Group = mute ce groupe, façon Maschine)
    pub mute_held: bool,
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
            left_tab: 0,
            right_tab: 0,
            active_sound: 0,
            browser_preset_idx: 0,
            selected_bar: 0,
            pattern_bars: 2,
            prev_main_encoder: None,
            prev_knobs: [None; 8],
            knob_carry: [0.0; 8],
            tap_timestamps: Vec::new(),
            shift_held: false,
            mute_held: false,
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

    pub fn handle_hardware_event(&mut self, ev: HardwareEvent) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        match ev {
            HardwareEvent::Pad(p) => {
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
                        actions.push(MapperAction::PatternTrigger {
                            group: self.group_focus,
                            pattern: p.pad as usize,
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
                            // Gamme mineure naturelle C3 (48)..D5 (74)
                            const SCALE_MINOR: [u8; 16] = [
                                48, 50, 51, 53, 55, 56, 58, 60, 62, 63, 65, 67, 68, 70, 72, 74,
                            ];
                            let mel_note = SCALE_MINOR[(p.pad as usize).min(15)];
                            self.pad_note.insert(p.pad, note);
                            actions.push(MapperAction::Midi(MidiEvent::NoteOn {
                                channel: ch,
                                note: mel_note,
                                velocity: vel,
                            }));
                            actions.push(MapperAction::PadTrigger {
                                pad: self.active_sound,
                                velocity: vel,
                                pressed: true,
                            });
                        }
                        ControllerMode::Chords => {
                            // Accords diatoniques en C mineur
                            let root = 48 + (p.pad % 7) * 2;
                            self.pad_note.insert(p.pad, note);
                            for &offset in &[0, 3, 7] {
                                actions.push(MapperAction::Midi(MidiEvent::NoteOn {
                                    channel: ch,
                                    note: root + offset,
                                    velocity: vel,
                                }));
                            }
                            actions.push(MapperAction::PadTrigger {
                                pad: self.active_sound,
                                velocity: vel,
                                pressed: true,
                            });
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
            }
            HardwareEvent::Button { id, pressed, .. } => {
                use maschine_proto::ButtonId;
                match id {
                    ButtonId::Shift => {
                        self.shift_held = pressed;
                        self.led_state.set_button_raw(
                            maschine_proto::hid_output::offsets::SHIFT,
                            if pressed { 7 } else { 0 },
                        );
                    }
                    ButtonId::Tempo => {
                        self.tempo_button_held = pressed;
                    }
                    ButtonId::Mute => {
                        self.mute_held = pressed;
                        if pressed {
                            self.mute_solo_group_used = false;
                        } else if !self.mute_solo_group_used {
                            // Appui simple (sans Group) : mute le groupe focus.
                            actions.push(MapperAction::ToggleMute(self.group_focus));
                        }
                    }
                    ButtonId::Solo => {
                        self.solo_held = pressed;
                        if pressed {
                            self.mute_solo_group_used = false;
                        } else if !self.mute_solo_group_used {
                            actions.push(MapperAction::ToggleSolo(self.group_focus));
                        }
                    }
                    ButtonId::Select => {
                        self.select_held = pressed;
                        if pressed {
                            self.select_used = false;
                            self.select_mode = true;
                            actions.push(MapperAction::SetSelectMode(true));
                        } else {
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
                    }
                    ButtonId::Erase => {
                        self.erase_held = pressed;
                        if pressed {
                            if self.shift_held {
                                actions.push(MapperAction::ClearBar(self.selected_bar));
                            } else {
                                self.erase_used = false;
                                self.erase_mode = true;
                                actions.push(MapperAction::SetEraseMode(true));
                            }
                        } else if !self.shift_held {
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
                    }
                    _ => {}
                }

                if pressed {
                    match id {
                        // Transport
                        ButtonId::Play => actions.push(MapperAction::PlayPauseToggle),
                        ButtonId::Stop => actions.push(MapperAction::Stop),
                        ButtonId::Rec => actions.push(MapperAction::RecordToggle),
                        ButtonId::Restart => {
                            if self.shift_held {
                                actions.push(MapperAction::ToggleLoop);
                            } else {
                                actions.push(MapperAction::Restart);
                            }
                        }
                        ButtonId::Tap => {
                            if self.shift_held {
                                actions.push(MapperAction::ToggleMetronome);
                                return actions;
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
                        }

                        // Modes de contrôle
                        ButtonId::PadMode => {
                            self.mode = ControllerMode::Pad;
                            actions.push(MapperAction::ModeSwitch(ControllerMode::Pad));
                            actions.push(MapperAction::SetControllerMode(0));
                        }
                        ButtonId::Keyboard => {
                            self.mode = ControllerMode::Keyboard;
                            actions.push(MapperAction::ModeSwitch(ControllerMode::Keyboard));
                            actions.push(MapperAction::SetControllerMode(1));
                        }
                        ButtonId::Chords => {
                            self.mode = ControllerMode::Chords;
                            actions.push(MapperAction::ModeSwitch(ControllerMode::Chords));
                            actions.push(MapperAction::SetControllerMode(2));
                        }
                        ButtonId::Step => {
                            self.mode = ControllerMode::Step;
                            actions.push(MapperAction::ModeSwitch(ControllerMode::Step));
                            actions.push(MapperAction::SetControllerMode(3));
                        }

                        // Focus Encodeur 4D
                        ButtonId::Tempo => {
                            self.encoder_focus = EncoderFocus::Tempo;
                        }
                        ButtonId::Volume => {
                            self.encoder_focus = EncoderFocus::Volume;
                        }
                        ButtonId::Swing => {
                            self.encoder_focus = EncoderFocus::Swing;
                        }

                        // Modes Smart Strip
                        ButtonId::Pitch => {
                            self.touchstrip_mode = TouchStripMode::Pitch;
                        }
                        ButtonId::Mod => {
                            self.touchstrip_mode = TouchStripMode::Mod;
                        }
                        ButtonId::Perform => {
                            self.touchstrip_mode = TouchStripMode::Perform;
                        }
                        ButtonId::Notes => {
                            self.touchstrip_mode = TouchStripMode::Notes;
                        }

                        // Features & Vues
                        ButtonId::FixedVel => {
                            self.fixed_velocity = !self.fixed_velocity;
                        }
                        ButtonId::Follow => {
                            self.follow_mode = !self.follow_mode;
                        }
                        ButtonId::NoteRepeat => {
                            self.note_repeat = !self.note_repeat;
                        }
                        ButtonId::Variation => {
                            actions.push(MapperAction::Variation);
                        }
                        ButtonId::Auto => {
                            actions.push(MapperAction::Quantize);
                        }
                        ButtonId::FileSave => {
                            actions.push(MapperAction::SaveProject);
                        }

                        // Onglets d'écrans dédiés
                        ButtonId::Arranger => {
                            self.select_left_tab(0, &mut actions);
                            actions.push(MapperAction::ToggleSongMode);
                        }
                        ButtonId::Mixer => {
                            self.select_left_tab(1, &mut actions);
                        }
                        ButtonId::Sampling => {
                            self.select_left_tab(2, &mut actions);
                        }
                        ButtonId::Browser => {
                            if self.left_tab == 3 {
                                self.select_left_tab(2, &mut actions); // Retour SAMPLER
                            } else {
                                self.select_left_tab(3, &mut actions);
                            }
                        }
                        ButtonId::Settings => {
                            self.right_tab = 3;
                            actions.push(MapperAction::SetRightTab(3));
                        }

                        // Groupes A..H (avec MUTE/SOLO + Group façon Maschine)
                        ButtonId::Group(g) if g < 8 => {
                            if self.mute_held {
                                actions.push(MapperAction::ToggleMute(g));
                                self.mute_solo_group_used = true;
                            } else if self.solo_held {
                                actions.push(MapperAction::ToggleSolo(g));
                                self.mute_solo_group_used = true;
                            } else {
                                self.group_focus = g;
                                actions.push(MapperAction::GroupFocus(g));
                            }
                        }

                        // Navigation 4D & Flèches < >
                        ButtonId::FourWayLeft | ButtonId::Left => {
                            if self.mode == ControllerMode::Step && self.left_tab == 0 {
                                // Mode STEP : défile la grille d'un pas.
                                actions.push(MapperAction::StepScroll(-1));
                            } else {
                                self.selected_bar = self.selected_bar.saturating_sub(1);
                                actions.push(MapperAction::SelectBar(self.selected_bar));
                            }
                        }
                        ButtonId::FourWayRight | ButtonId::Right => {
                            if self.shift_held {
                                let new_len = if self.pattern_bars < 4 { 4 } else { 8 };
                                self.pattern_bars = new_len;
                                actions.push(MapperAction::SetPatternBars(new_len));
                            } else if self.mode == ControllerMode::Step && self.left_tab == 0 {
                                // Mode STEP : défile la grille d'un pas.
                                actions.push(MapperAction::StepScroll(1));
                            } else {
                                self.selected_bar = (self.selected_bar + 1)
                                    .min(self.pattern_bars.saturating_sub(1));
                                actions.push(MapperAction::SelectBar(self.selected_bar));
                            }
                        }
                        ButtonId::FourWayUp => {
                            if self.left_tab == 3 {
                                actions.push(MapperAction::BrowsePresetStep(-1));
                            } else {
                                self.active_sound = (self.active_sound + 1) % 16;
                                actions.push(MapperAction::SelectSound(self.active_sound));
                                actions.push(MapperAction::PadTrigger {
                                    pad: self.active_sound,
                                    velocity: 110,
                                    pressed: true,
                                });
                            }
                        }
                        ButtonId::FourWayDown => {
                            if self.left_tab == 3 {
                                actions.push(MapperAction::BrowsePresetStep(1));
                            } else {
                                self.active_sound = (self.active_sound + 15) % 16;
                                actions.push(MapperAction::SelectSound(self.active_sound));
                                actions.push(MapperAction::PadTrigger {
                                    pad: self.active_sound,
                                    velocity: 110,
                                    pressed: true,
                                });
                            }
                        }
                        ButtonId::EncoderPush => {
                            if self.left_tab == 3 {
                                actions.push(MapperAction::LoadSelectedPreset);
                            } else {
                                // One-shot preview du sound sélectionné (ne démarre PAS la boucle transport !)
                                actions.push(MapperAction::PadTrigger {
                                    pad: self.active_sound,
                                    velocity: 115,
                                    pressed: true,
                                });
                            }
                        }

                        // Actions d'édition
                        ButtonId::Duplicate => {
                            actions.push(MapperAction::DuplicateBar);
                        }
                        // Workflow officiel : SCENE / PATTERN / SELECT / EVENTS / MACRO / PLUGIN
                        ButtonId::Scene => {
                            self.scene_select_mode = !self.scene_select_mode;
                            if self.scene_select_mode {
                                self.pattern_select_mode = false;
                                actions.push(MapperAction::SetPatternMode(false));
                            }
                            actions.push(MapperAction::SetSceneMode(self.scene_select_mode));
                        }
                        ButtonId::Pattern => {
                            self.pattern_select_mode = !self.pattern_select_mode;
                            if self.pattern_select_mode {
                                self.scene_select_mode = false;
                                actions.push(MapperAction::SetSceneMode(false));
                            }
                            actions.push(MapperAction::SetPatternMode(self.pattern_select_mode));
                        }
                        ButtonId::Events => {
                            self.right_tab = 1;
                            actions.push(MapperAction::SetRightTab(1));
                        }
                        ButtonId::Macro => {
                            self.right_tab = 2;
                            actions.push(MapperAction::SetRightTab(2));
                        }
                        ButtonId::Plugin => {
                            self.select_left_tab(2, &mut actions); // SAMPLER / Sound Editor
                        }
                        ButtonId::ChannelMidi => {
                            self.select_left_tab(1, &mut actions); // MIXER
                        }
                        // Verrou : sauvegarde instantanée (comme FileSave).
                        ButtonId::Lock => {
                            actions.push(MapperAction::SaveProject);
                        }
                        // Pédale footswitch : lecture/pause (comme EncoderPush).
                        ButtonId::Pedal => {
                            actions.push(MapperAction::PlayPauseToggle);
                        }

                        // Soft Buttons Écrans 1..8 — actions contextuelles :
                        // c'est l'Engine qui décide (labels = actions réelles
                        // affichées en bas des écrans), le hardware ne fait que
                        // transmettre.
                        ButtonId::DisplayBtn(n @ 1..=8) => {
                            actions.push(MapperAction::SoftKey(n));
                        }
                        _ => {}
                    }
                }
            }
            HardwareEvent::Encoder(enc) => {
                if enc.id == 0 {
                    // Encodeur 4D principal : différence relative 4-bit SANS DÉRIVE
                    let new_val = (enc.value & 0x0F) as u8;
                    let delta = match self.prev_main_encoder {
                        Some(prev) if prev != new_val => wrap_16_delta(new_val, prev),
                        _ => 0,
                    };
                    self.prev_main_encoder = Some(new_val);
                    if delta != 0 {
                        if self.left_tab == 3 {
                            self.browser_preset_idx =
                                (self.browser_preset_idx as i32 + delta as i32).rem_euclid(16)
                                    as u8;
                            actions.push(MapperAction::BrowsePresetStep(delta as i32));
                        } else {
                            match self.encoder_focus {
                                EncoderFocus::Tempo => {
                                    // Valeurs en « ticks bruts » (l'app applique 0.05 BPM/tick) :
                                    // 0.5 BPM par cran, 0.25 avec Shift.
                                    let step = if self.shift_held { 5.0 } else { 10.0 };
                                    actions.push(MapperAction::BpmDelta(delta as f32 * step));
                                }
                                EncoderFocus::Volume => {
                                    // 10 ticks = 0.01 de volume master par cran.
                                    let step = if self.shift_held { 5.0 } else { 10.0 };
                                    actions
                                        .push(MapperAction::MasterVolumeDelta(delta as f32 * step));
                                }
                                EncoderFocus::Swing => {
                                    // 10 ticks = 0.01 de swing par cran.
                                    let step = if self.shift_held { 5.0 } else { 10.0 };
                                    actions.push(MapperAction::SwingDelta(delta as f32 * step));
                                }
                            }
                        }
                    }
                } else if enc.id >= 1 && enc.id <= 8 {
                    let idx = (enc.id - 1) as usize;
                    let new_val = enc.value & 0x3FF;
                    let delta = match self.prev_knobs[idx] {
                        Some(prev) if prev != new_val => wrap_1024_delta(new_val, prev),
                        _ => 0,
                    };
                    self.prev_knobs[idx] = Some(new_val);
                    // Les grands deltas (tours rapides) sont limités, pas jetés :
                    // le knob reste toujours réactif.
                    let delta = delta.clamp(-128, 128);
                    if delta != 0 {
                        if self.mode == ControllerMode::Step && self.left_tab == 0 {
                            // STEP : 1 VEL, 2 POSITION, 3 LONGUEUR, 4 PITCH,
                            // 5 SCROLL (grille complète), 6 VOL SND, 7 CUTOFF, 8 RESO.
                            // Les valeurs entières (pas, demi-tons, vélocité) passent
                            // par un accumulateur : ~1 cran de knob = ~32 ticks bruts.
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
                                6 => {
                                    actions.push(MapperAction::SoundFilterCutoffDelta(delta as f32))
                                }
                                7 => actions.push(MapperAction::SoundFilterResoDelta(delta as f32)),
                                _ => {}
                            }
                        } else if self.left_tab == 1 {
                            // MIXER : knobs 1..8 = volumes des groupes A..H (dB).
                            if idx < 8 {
                                actions.push(MapperAction::GroupVolumeDeltaFor {
                                    group: idx as u8,
                                    delta: delta as f32,
                                });
                            }
                        } else if self.left_tab == 2 {
                            // SAMPLER : son actif. 1-4 = VOL/PAN/CUTOFF/RESO,
                            // 5-8 = ATTACK/DECAY/SUSTAIN/RELEASE. Deltas bruts :
                            // l'app les met à l'échelle (~1 tour = toute la plage).
                            match idx {
                                0 => actions.push(MapperAction::SoundVolumeDelta(delta as f32)),
                                1 => actions.push(MapperAction::SoundPanDelta(delta as f32)),
                                2 => {
                                    actions.push(MapperAction::SoundFilterCutoffDelta(delta as f32))
                                }
                                3 => actions.push(MapperAction::SoundFilterResoDelta(delta as f32)),
                                4 => actions.push(MapperAction::SoundAttackDelta(delta as f32)),
                                5 => actions.push(MapperAction::SoundDecayDelta(delta as f32)),
                                6 => actions.push(MapperAction::SoundSustainDelta(delta as f32)),
                                _ => actions.push(MapperAction::SoundReleaseDelta(delta as f32)),
                            }
                        } else if self.left_tab == 3 {
                            // BROWSER :
                            // Knob 1 (idx 0) : cible Sound (pads), par cran.
                            // Knob 2 (idx 1) : presets 808, par cran + pré-écoute.
                            // Knob 8 (idx 7) : master.
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
                                            (self.browser_preset_idx as i32 + d).rem_euclid(16)
                                                as u8;
                                        actions.push(MapperAction::BrowsePresetStep(d));
                                    }
                                }
                                7 => actions.push(MapperAction::MasterVolumeDelta(delta as f32)),
                                _ => {}
                            }
                        } else {
                            // ARRANGE (tab 0) : 1 TEMPO, 2 SWING, 3 VOL GRP, 4 PAN GRP,
                            // 5 VOL SND, 6 CUTOFF, 7 RESO, 8 MASTER.
                            match idx {
                                0 => {
                                    let fine = if self.shift_held { 0.1 } else { 1.0 };
                                    actions.push(MapperAction::BpmDelta(delta as f32 * fine));
                                }
                                1 => actions.push(MapperAction::SwingDelta(delta as f32)),
                                2 => actions.push(MapperAction::GroupVolumeDelta(delta as f32)),
                                3 => actions.push(MapperAction::GroupPanDelta(delta as f32)),
                                4 => actions.push(MapperAction::SoundVolumeDelta(delta as f32)),
                                5 => {
                                    actions.push(MapperAction::SoundFilterCutoffDelta(delta as f32))
                                }
                                6 => actions.push(MapperAction::SoundFilterResoDelta(delta as f32)),
                                _ => actions.push(MapperAction::MasterVolumeDelta(delta as f32)),
                            }
                        }
                    }
                }
            }
            HardwareEvent::TouchStrip(ts) => {
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
            }
            _ => {}
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
        self.led_state
            .set_button_raw(offsets::FIXED_VEL, if self.fixed_velocity { 7 } else { 0 });
        self.led_state
            .set_button_raw(offsets::FOLLOW, if self.follow_mode { 7 } else { 1 });
        self.led_state
            .set_button_raw(offsets::NOTE_REPEAT, if self.note_repeat { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::VARIATION, 2);
        self.led_state.set_button_raw(offsets::AUTO, 2);
        self.led_state.set_button_raw(offsets::FILE_SAVE, 2);
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
            if self.mute_held || (info.muted_groups & (1 << self.group_focus)) != 0 {
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
            .set_button_raw(offsets::BROWSER, if self.left_tab == 3 { 7 } else { 1 });
        self.led_state
            .set_button_raw(offsets::SETTINGS, if self.right_tab == 3 { 7 } else { 1 });

        // 8. Boutons d'écran 1..8
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
                    for i in 0..16 {
                        if self.pad_flash(i, now) {
                            self.led_state.set_pad(i, white);
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
    fn mute_solo_hold_group() {
        let mut m = HardwareMapper::with_default();
        // MUTE tenu + Group 2 -> mute groupe 2 sans changer le focus
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Mute,
            pressed: true,
            ts_ns: 0,
        });
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Group(2),
            pressed: true,
            ts_ns: 1,
        });
        assert!(a.contains(&MapperAction::ToggleMute(2)));
        assert_eq!(m.group_focus, 0);
        // Relâchement Mute : pas de double toggle (un groupe a été touché)
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Mute,
            pressed: false,
            ts_ns: 2,
        });
        assert!(!a.contains(&MapperAction::ToggleMute(0)));
        // Appui simple Mute : toggle groupe focus au relâchement
        let _ = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Mute,
            pressed: true,
            ts_ns: 3,
        });
        let a = m.handle_hardware_event(HardwareEvent::Button {
            id: maschine_proto::ButtonId::Mute,
            pressed: false,
            ts_ns: 4,
        });
        assert!(a.contains(&MapperAction::ToggleMute(0)));
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
    fn step_mode_knobs_are_inert_off_step_page() {
        let mut m = HardwareMapper::with_default();
        m.mode = ControllerMode::Step;
        m.left_tab = 1; // Page MIXER : les knobs restent les volumes de groupe.
        let _ = m.handle_hardware_event(knob_ev(1, 0, 0));
        let a = m.handle_hardware_event(knob_ev(1, 4, 1));
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
}
