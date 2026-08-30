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
        }
    }

    /// Défaut.
    #[must_use]
    pub fn with_default() -> Self {
        Self::new(MapperConfig::default())
    }

    /// Traite un `HardwareEvent`, retourne les `MidiEvent` à envoyer au séquenceur.
    pub fn handle(&mut self, ev: HardwareEvent) -> Vec<MidiEvent> {
        let mut out = Vec::new();
        match ev {
            HardwareEvent::Pad(p) => {
                let note = self.cfg.pad_base_note + p.pad;
                let ch = self.cfg.pad_channel;
                if p.pressed {
                    let vel = self.apply_curve(p.velocity);
                    out.push(MidiEvent::NoteOn {
                        channel: ch,
                        note,
                        velocity: vel.max(1),
                    });
                    self.pad_note.insert(p.pad, note);
                    // feedback : brillance selon vélocité
                    let col = Color::from_velocity(vel);
                    self.led_state.set_pad(p.pad as usize, col);
                } else {
                    let note = self.pad_note.remove(&p.pad).unwrap_or(note);
                    out.push(MidiEvent::NoteOff {
                        channel: ch,
                        note,
                        velocity: 0,
                    });
                    // restore couleur pad
                    self.led_state
                        .set_pad(p.pad as usize, self.pad_colors[p.pad as usize]);
                }
            }
            HardwareEvent::Button { id, pressed, .. } => {
                use maschine_proto::ButtonId;
                if pressed {
                    match id {
                        ButtonId::Group(g) if g < 8 => {
                            self.group_focus = g;
                            // feedback Groups : allume le Group focus
                            // on éteint tous les Groups puis allume le focus en blanc
                            for g in 0..8 {
                                let v = if g == self.group_focus {
                                    78
                                } else {
                                    self.pad_colors[g as usize].to_index()
                                };
                                self.led_state.set_button_raw(
                                    maschine_proto::hid_output::offsets::GROUP_A + g as usize + 1,
                                    v,
                                );
                            }
                        }
                        ButtonId::Play => out.push(MidiEvent::Transport {
                            play: true,
                            rec: false,
                            stop: false,
                        }),
                        ButtonId::Rec => out.push(MidiEvent::Transport {
                            play: false,
                            rec: true,
                            stop: false,
                        }),
                        ButtonId::Stop => out.push(MidiEvent::Transport {
                            play: false,
                            rec: false,
                            stop: true,
                        }),
                        _ => {}
                    }
                    // LED feedback simple : allume le bouton pressé
                    // (les offsets boutons sont documentés dans hid_output::offsets)
                }
            }
            HardwareEvent::Encoder(enc) => {
                if enc.id >= 1 && enc.id <= 8 {
                    let cc = self.cfg.knob_base_cc + (enc.id - 1);
                    let val = (enc.value as u8).min(127); // 10-bit -> 7-bit
                    out.push(MidiEvent::Cc {
                        channel: 0,
                        cc,
                        value: val & 0x7F,
                    });
                }
            }
            HardwareEvent::TouchStrip(ts) => {
                if ts.touched {
                    let cc = 74; // cutoff par défaut
                    let val = (ts.position / 8).min(127) as u8;
                    out.push(MidiEvent::Cc {
                        channel: 0,
                        cc,
                        value: val,
                    });
                    // feedback strip : barre de progression
                    let lit = (ts.position as usize * 25 / 1024).min(25);
                    self.led_state.fill_strip(Color::OFF);
                    for i in 0..lit {
                        self.led_state.set_strip(i, Color::from_index(22).unwrap());
                        // yellow
                    }
                } else {
                    self.led_state.fill_strip(Color::OFF);
                }
            }
            _ => {}
        }
        out
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
}
