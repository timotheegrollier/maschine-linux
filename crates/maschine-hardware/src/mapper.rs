//! Mapper HID → MIDI / Engine.

use std::collections::HashMap;

use maschine_proto::color::Color;
use maschine_proto::hid_output::LedState;

use crate::events::HardwareEvent;

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
    PadTrigger { pad: u8, velocity: u8, pressed: bool },
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
    /// Changement de groupe (A..H, 0..7)
    GroupFocus(u8),
    /// Sélection de mesure (0 ou 1)
    SelectBar(usize),
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
    /// Quantification
    Quantize,
    /// Sauvegarder le projet
    SaveProject,
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
    pub triggered_pads: [bool; 16],
    pub master_vol: f32,
    pub muted_groups: u8,
    pub soloed_groups: u8,
    pub left_tab: u8,
    pub right_tab: u8,
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
    /// Mesure sélectionnée (0 ou 1)
    pub selected_bar: usize,
    /// Dernière valeur de l'encodeur 4D
    pub prev_main_encoder: Option<u8>,
    /// Dernières valeurs des 8 knobs d'écran
    pub prev_knobs: [Option<u16>; 8],
    /// Historique des taps tempo
    tap_timestamps: Vec<std::time::Instant>,
    /// Bouton Shift enfoncé
    pub shift_held: bool,
    /// Bouton Tempo enfoncé
    pub tempo_button_held: bool,
    /// État d'appui physique des 16 pads
    pub pad_pressed: [bool; 16],
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
            selected_bar: 0,
            prev_main_encoder: None,
            prev_knobs: [None; 8],
            tap_timestamps: Vec::new(),
            shift_held: false,
            tempo_button_held: false,
            pad_pressed: [false; 16],
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

    /// Traite un événement matériel complet et retourne la liste des actions à appliquer à l'engine.
    pub fn handle_hardware_event(&mut self, ev: HardwareEvent) -> Vec<MapperAction> {
        let mut actions = Vec::new();
        match ev {
            HardwareEvent::Pad(p) => {
                let note = self.cfg.pad_base_note + p.pad;
                let ch = self.cfg.pad_channel;
                if (p.pad as usize) < 16 {
                    self.pad_pressed[p.pad as usize] = p.pressed;
                }
                let vel = if self.fixed_velocity {
                    127
                } else {
                    self.apply_curve(p.velocity).max(1)
                };

                if p.pressed {
                    self.pad_note.insert(p.pad, note);

                    match self.mode {
                        ControllerMode::Step => {
                            // En mode step, taper le pad bascule le pas dans la mesure courante pour le son actif
                            actions.push(MapperAction::ToggleStep {
                                sound: self.active_sound,
                                bar: self.selected_bar,
                                step: p.pad as usize,
                            });
                            // Aperçu sonore
                            actions.push(MapperAction::PadTrigger {
                                pad: self.active_sound,
                                velocity: vel,
                                pressed: true,
                            });
                        }
                        ControllerMode::Keyboard => {
                            // Gamme mineure naturelle C3 (48)..D5 (74)
                            const SCALE_MINOR: [u8; 16] = [
                                48, 50, 51, 53, 55, 56, 58, 60,
                                62, 63, 65, 67, 68, 70, 72, 74,
                            ];
                            let mel_note = SCALE_MINOR[(p.pad as usize).min(15)];
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
                            actions.push(MapperAction::Midi(MidiEvent::NoteOn {
                                channel: ch,
                                note,
                                velocity: vel,
                            }));
                            self.active_sound = p.pad;
                            actions.push(MapperAction::PadTrigger {
                                pad: p.pad,
                                velocity: vel,
                                pressed: true,
                            });
                        }
                    }
                } else {
                    let note = self.pad_note.remove(&p.pad).unwrap_or(note);
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
                        self.led_state.set_button_raw(maschine_proto::hid_output::offsets::SHIFT, if pressed { 7 } else { 0 });
                    }
                    ButtonId::Tempo => {
                        self.tempo_button_held = pressed;
                    }
                    _ => {}
                }

                if pressed {
                    match id {
                        // Transport
                        ButtonId::Play => actions.push(MapperAction::PlayPauseToggle),
                        ButtonId::Stop => actions.push(MapperAction::Stop),
                        ButtonId::Rec => actions.push(MapperAction::RecordToggle),
                        ButtonId::Restart => actions.push(MapperAction::Restart),
                        ButtonId::Tap => {
                            let now = std::time::Instant::now();
                            self.tap_timestamps.retain(|t| now.duration_since(*t).as_secs_f32() < 2.0);
                            self.tap_timestamps.push(now);
                            if self.tap_timestamps.len() >= 2 {
                                let diffs: Vec<f32> = self.tap_timestamps.windows(2)
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
                        }
                        ButtonId::Keyboard => {
                            self.mode = ControllerMode::Keyboard;
                            actions.push(MapperAction::ModeSwitch(ControllerMode::Keyboard));
                        }
                        ButtonId::Chords => {
                            self.mode = ControllerMode::Chords;
                            actions.push(MapperAction::ModeSwitch(ControllerMode::Chords));
                        }
                        ButtonId::Step => {
                            self.mode = ControllerMode::Step;
                            actions.push(MapperAction::ModeSwitch(ControllerMode::Step));
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
                            self.left_tab = 0;
                            actions.push(MapperAction::SetLeftTab(0));
                        }
                        ButtonId::Mixer => {
                            self.left_tab = 1;
                            actions.push(MapperAction::SetLeftTab(1));
                        }
                        ButtonId::Sampling => {
                            self.left_tab = 2;
                            actions.push(MapperAction::SetLeftTab(2));
                        }
                        ButtonId::Browser => {
                            self.left_tab = 3;
                            actions.push(MapperAction::SetLeftTab(3));
                        }
                        ButtonId::Settings => {
                            self.right_tab = 3;
                            actions.push(MapperAction::SetRightTab(3));
                        }

                        // Groupes A..H
                        ButtonId::Group(g) if g < 8 => {
                            self.group_focus = g;
                            actions.push(MapperAction::GroupFocus(g));
                        }

                        // Navigation 4D
                        ButtonId::FourWayLeft | ButtonId::Left => {
                            self.selected_bar = 0;
                            actions.push(MapperAction::SelectBar(0));
                        }
                        ButtonId::FourWayRight | ButtonId::Right => {
                            self.selected_bar = 1;
                            actions.push(MapperAction::SelectBar(1));
                        }
                        ButtonId::FourWayUp => {
                            self.active_sound = (self.active_sound + 1) % 16;
                            actions.push(MapperAction::SelectSound(self.active_sound));
                        }
                        ButtonId::FourWayDown => {
                            self.active_sound = (self.active_sound + 15) % 16;
                            actions.push(MapperAction::SelectSound(self.active_sound));
                        }
                        ButtonId::EncoderPush => {
                            actions.push(MapperAction::PlayPauseToggle);
                        }

                        // Actions d'édition
                        ButtonId::Mute => {
                            actions.push(MapperAction::ToggleMute(self.group_focus));
                        }
                        ButtonId::Solo => {
                            actions.push(MapperAction::ToggleSolo(self.group_focus));
                        }
                        ButtonId::Erase => {
                            actions.push(MapperAction::ClearBar(self.selected_bar));
                        }
                        ButtonId::Duplicate => {
                            actions.push(MapperAction::DuplicateBar);
                        }

                        // Soft Buttons Écrans 1..8
                        ButtonId::DisplayBtn(1) => {
                            self.left_tab = 0;
                            actions.push(MapperAction::SetLeftTab(0));
                        }
                        ButtonId::DisplayBtn(2) => {
                            self.left_tab = 1;
                            actions.push(MapperAction::SetLeftTab(1));
                        }
                        ButtonId::DisplayBtn(3) => {
                            self.left_tab = 2;
                            actions.push(MapperAction::SetLeftTab(2));
                        }
                        ButtonId::DisplayBtn(4) => {
                            self.left_tab = 3;
                            actions.push(MapperAction::SetLeftTab(3));
                        }
                        ButtonId::DisplayBtn(5) => {
                            self.right_tab = 0;
                            self.selected_bar = 0;
                            actions.push(MapperAction::SetRightTab(0));
                            actions.push(MapperAction::SelectBar(0));
                        }
                        ButtonId::DisplayBtn(6) => {
                            self.right_tab = 0;
                            self.selected_bar = 1;
                            actions.push(MapperAction::SetRightTab(0));
                            actions.push(MapperAction::SelectBar(1));
                        }
                        ButtonId::DisplayBtn(7) => {
                            actions.push(MapperAction::ClearBar(self.selected_bar));
                        }
                        ButtonId::DisplayBtn(8) => {
                            actions.push(MapperAction::ResetDemoGroove);
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
                        match self.encoder_focus {
                            EncoderFocus::Tempo => {
                                let step = if self.shift_held { 1.0 } else { 0.5 };
                                actions.push(MapperAction::BpmDelta(delta as f32 * step));
                            }
                            EncoderFocus::Volume => {
                                let step = if self.shift_held { 0.05 } else { 0.01 };
                                actions.push(MapperAction::MasterVolumeDelta(delta as f32 * step));
                            }
                            EncoderFocus::Swing => {
                                let step = if self.shift_held { 0.05 } else { 0.01 };
                                actions.push(MapperAction::SwingDelta(delta as f32 * step));
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
                    if delta != 0 && delta.abs() < 50 {
                        match idx {
                            0 => {
                                let step = if self.shift_held { 0.1 } else { 1.0 };
                                actions.push(MapperAction::BpmDelta(delta as f32 * step));
                            }
                            1 => actions.push(MapperAction::SwingDelta(delta as f32 * 0.01)),
                            2 => actions.push(MapperAction::GroupVolumeDelta(delta as f32 * 0.01)),
                            3 => actions.push(MapperAction::GroupPanDelta(delta as f32 * 0.02)),
                            7 => actions.push(MapperAction::MasterVolumeDelta(delta as f32 * 0.01)),
                            _ => {}
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
        actions.into_iter().filter_map(|a| match a {
            MapperAction::Midi(m) => Some(m),
            _ => None,
        }).collect()
    }

    /// Met à jour l'ensemble des LEDs physiques selon l'état réel de l'Engine.
    pub fn update_leds(&mut self, info: &SequencerLedInfo) {
        use maschine_proto::hid_output::offsets;

        // 1. Boutons de transport
        self.led_state.set_button_raw(offsets::PLAY, if info.is_playing { 7 } else { 2 });
        self.led_state.set_button_raw(offsets::STOP, if info.is_stopped { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::REC, if info.is_recording { 7 } else { 0 });
        self.led_state.set_button_raw(offsets::RESTART, 2);

        // 2. Tap Tempo : pulsation visuelle sur chaque temps de mesure (noire 1..4)
        self.led_state.set_button_raw(offsets::TAP, if info.is_quarter_beat { 7 } else { 2 });

        // 3. Modes Pad vs Keyboard vs Chords vs Step
        self.led_state.set_button_raw(offsets::PAD_MODE, if self.mode == ControllerMode::Pad { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::KEYBOARD, if self.mode == ControllerMode::Keyboard { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::CHORDS, if self.mode == ControllerMode::Chords { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::STEP, if self.mode == ControllerMode::Step { 7 } else { 1 });

        // 4. Focus Encodeur 4D
        self.led_state.set_button_raw(offsets::TEMPO, if self.encoder_focus == EncoderFocus::Tempo { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::VOLUME, if self.encoder_focus == EncoderFocus::Volume { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::SWING, if self.encoder_focus == EncoderFocus::Swing { 7 } else { 1 });

        // 5. Modes Smart Strip
        self.led_state.set_button_raw(offsets::PITCH, if self.touchstrip_mode == TouchStripMode::Pitch { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::MOD, if self.touchstrip_mode == TouchStripMode::Mod { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::PERFORM, if self.touchstrip_mode == TouchStripMode::Perform { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::NOTES, if self.touchstrip_mode == TouchStripMode::Notes { 7 } else { 1 });

        // 6. Features & Workflows
        self.led_state.set_button_raw(offsets::FIXED_VEL, if self.fixed_velocity { 7 } else { 0 });
        self.led_state.set_button_raw(offsets::FOLLOW, if self.follow_mode { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::NOTE_REPEAT, if self.note_repeat { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::VARIATION, 2);
        self.led_state.set_button_raw(offsets::AUTO, 2);
        self.led_state.set_button_raw(offsets::FILE_SAVE, 2);

        // 7. Écrans & Vues
        self.led_state.set_button_raw(offsets::ARRANGER, if self.left_tab == 0 { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::MIXER, if self.left_tab == 1 { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::SAMPLING, if self.left_tab == 2 { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::BROWSER, if self.left_tab == 3 { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::SETTINGS, if self.right_tab == 3 { 7 } else { 1 });

        // 8. Boutons d'écran 1..8
        self.led_state.set_button_raw(offsets::DISPLAY_BTN_1, if self.left_tab == 0 { 7 } else { 2 });
        self.led_state.set_button_raw(offsets::DISPLAY_BTN_2, if self.left_tab == 1 { 7 } else { 2 });
        self.led_state.set_button_raw(offsets::DISPLAY_BTN_3, if self.left_tab == 2 { 7 } else { 2 });
        self.led_state.set_button_raw(offsets::DISPLAY_BTN_4, if self.left_tab == 3 { 7 } else { 2 });
        self.led_state.set_button_raw(offsets::DISPLAY_BTN_5, if self.right_tab == 0 && self.selected_bar == 0 { 7 } else { 2 });
        self.led_state.set_button_raw(offsets::DISPLAY_BTN_6, if self.right_tab == 0 && self.selected_bar == 1 { 7 } else { 2 });
        self.led_state.set_button_raw(offsets::DISPLAY_BTN_7, 2);
        self.led_state.set_button_raw(offsets::DISPLAY_BTN_8, 2);

        // 9. Groupes A..H
        for g in 0usize..8 {
            let col = if g == self.group_focus as usize {
                78 // Blanc éclatant sur groupe sélectionné
            } else {
                self.pad_colors[g].to_index()
            };
            self.led_state.set_button_raw(offsets::GROUP_A + g, col);
        }

        // 10. Mute et Solo
        let is_muted = (info.muted_groups & (1 << self.group_focus)) != 0;
        let is_soloed = (info.soloed_groups & (1 << self.group_focus)) != 0;
        self.led_state.set_button_raw(offsets::MUTE, if is_muted { 7 } else { 1 });
        self.led_state.set_button_raw(offsets::SOLO, if is_soloed { 7 } else { 1 });

        // 11. Flèches de navigation
        self.led_state.set_button_raw(offsets::FOUR_WAY_UP, 3);
        self.led_state.set_button_raw(offsets::FOUR_WAY_DOWN, 3);
        self.led_state.set_button_raw(offsets::FOUR_WAY_LEFT, 3);
        self.led_state.set_button_raw(offsets::FOUR_WAY_RIGHT, 3);

        // 12. 16 Pads RGB physiques
        match self.mode {
            ControllerMode::Pad => {
                for i in 0..16 {
                    if self.pad_pressed[i] || info.triggered_pads[i] {
                        self.led_state.set_pad(i, Color::from_index(78).unwrap_or(Color::OFF));
                    } else {
                        self.led_state.set_pad(i, self.pad_colors[i]);
                    }
                }
            }
            ControllerMode::Keyboard => {
                // Gamme mineure : Pads 0, 7, 14 sont fondamentales (C3, C4, C5)
                for i in 0..16 {
                    if self.pad_pressed[i] {
                        self.led_state.set_pad(i, Color::from_index(78).unwrap_or(Color::OFF)); // Blanc flash
                    } else if i == 0 || i == 7 || i == 14 {
                        self.led_state.set_pad(i, Color::from_index(42).unwrap_or(Color::OFF)); // Bleu fondamental
                    } else {
                        self.led_state.set_pad(i, Color::from_index(31).unwrap_or(Color::OFF)); // Cyan note
                    }
                }
            }
            ControllerMode::Chords => {
                for i in 0..16 {
                    if self.pad_pressed[i] {
                        self.led_state.set_pad(i, Color::from_index(78).unwrap_or(Color::OFF));
                    } else if i % 2 == 0 {
                        self.led_state.set_pad(i, Color::from_index(50).unwrap_or(Color::OFF)); // Violet
                    } else {
                        self.led_state.set_pad(i, Color::from_index(58).unwrap_or(Color::OFF)); // Magenta
                    }
                }
            }
            ControllerMode::Step => {
                for s in 0..16 {
                    let is_cursor = info.is_playing && (info.current_bar == self.selected_bar) && (info.current_step == s);
                    if is_cursor {
                        self.led_state.set_pad(s, Color::from_index(78).unwrap_or(Color::OFF)); // Blanc curseur
                    } else if info.active_steps[s] {
                        let col = self.pad_colors[self.active_sound as usize];
                        self.led_state.set_pad(s, col); // Couleur du son programmé
                    } else if s % 4 == 0 {
                        self.led_state.set_pad(s, Color::from_index(2).unwrap_or(Color::OFF)); // Repère temps
                    } else {
                        self.led_state.set_pad(s, Color::OFF);
                    }
                }
            }
        }

        // 13. Touchstrip 25 LEDs RGB
        if self.strip_touched {
            let lit_idx = ((self.strip_pos as f32 / 1024.0) * 24.0).clamp(0.0, 24.0) as usize;
            match self.touchstrip_mode {
                TouchStripMode::Pitch => {
                    // Bipolaire centré sur LED 12
                    let center = 12usize;
                    let (start, end) = if lit_idx < center { (lit_idx, center) } else { (center, lit_idx) };
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
                let c = if i <= lit { Color::from_index(26).unwrap_or(Color::OFF) } else { Color::OFF };
                self.led_state.set_strip(i, c);
            }
        } else {
            self.led_state.fill_strip(Color::OFF);
            if self.touchstrip_mode == TouchStripMode::Pitch {
                self.led_state.set_strip(12, Color::from_index(31).unwrap_or(Color::OFF)); // Point central
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
        let mut info = SequencerLedInfo::default();
        info.is_playing = true;
        info.active_steps[0] = true;
        info.active_steps[4] = true;
        m.update_leds(&info);

        let led_st = m.led_state();
        // Play LED est allumé
        assert_eq!(led_st.button_leds[maschine_proto::hid_output::offsets::PLAY - 1], 7);
        // Step mode LED est allumé
        assert_eq!(led_st.button_leds[maschine_proto::hid_output::offsets::STEP - 1], 7);
    }
}
