//! Port MIDI virtuel ALSA via `midir`.

use crossbeam_channel::{Receiver, Sender};
use midir::os::unix::{VirtualInput, VirtualOutput};
use midir::{MidiInput, MidiInputConnection, MidiOutput};

use crate::mapper::MidiEvent;
use crate::{HardwareError, Result};

/// Handle pour les ports MIDI virtuels (In & Out).
pub struct MidiPorts {
    output_tx: Sender<Vec<u8>>,
    _input_conn: Option<MidiInputConnection<()>>,
}

impl MidiPorts {
    /// Ouvre les ports MIDI virtuels sous le nom `port_name`.
    /// Retourne l'instance `MidiPorts` et un canal de réception pour les événements MIDI entrants du DAW.
    pub fn open(port_name: &str) -> Result<(Self, Receiver<MidiEvent>)> {
        let (in_tx, in_rx) = crossbeam_channel::unbounded();
        let (out_tx, out_rx) = crossbeam_channel::unbounded::<Vec<u8>>();

        // Port d'entrée MIDI
        let input_conn = match MidiInput::new("maschine-linux-in") {
            Ok(midi_in) => {
                match midi_in.create_virtual(
                    port_name,
                    move |_ts, data, _| {
                        if let Some(ev) = parse_midi_bytes(data) {
                            let _ = in_tx.send(ev);
                        }
                    },
                    (),
                ) {
                    Ok(conn) => Some(conn),
                    Err(e) => {
                        log::warn!("Impossible de créer le port virtuel MIDI in: {e}");
                        None
                    }
                }
            }
            Err(e) => {
                log::warn!("Erreur init MidiInput: {e}");
                None
            }
        };

        // Port de sortie MIDI
        match MidiOutput::new("maschine-linux-out") {
            Ok(midi_out) => match midi_out.create_virtual(port_name) {
                Ok(mut out_conn) => {
                    std::thread::Builder::new()
                        .name("maschine-midi-out".to_string())
                        .spawn(move || {
                            while let Ok(bytes) = out_rx.recv() {
                                let _ = out_conn.send(&bytes);
                            }
                        })
                        .map_err(|e| HardwareError::Other(format!("spawn midi out thread: {e}")))?;
                }
                Err(e) => {
                    log::warn!("Impossible de créer le port virtuel MIDI out: {e}");
                }
            },
            Err(e) => {
                log::warn!("Erreur init MidiOutput: {e}");
            }
        }

        Ok((
            Self {
                output_tx: out_tx,
                _input_conn: input_conn,
            },
            in_rx,
        ))
    }

    /// Envoie un événement MIDI au DAW.
    pub fn send(&self, ev: &MidiEvent) {
        let bytes = midi_event_to_bytes(ev);
        if !bytes.is_empty() {
            let _ = self.output_tx.send(bytes);
        }
    }
}

/// Convertit un `MidiEvent` en octets MIDI bruts.
pub fn midi_event_to_bytes(ev: &MidiEvent) -> Vec<u8> {
    match ev {
        MidiEvent::NoteOn {
            channel,
            note,
            velocity,
        } => {
            vec![0x90 | (channel & 0x0F), *note & 0x7F, *velocity & 0x7F]
        }
        MidiEvent::NoteOff {
            channel,
            note,
            velocity,
        } => {
            vec![0x80 | (channel & 0x0F), *note & 0x7F, *velocity & 0x7F]
        }
        MidiEvent::Cc { channel, cc, value } => {
            vec![0xB0 | (channel & 0x0F), *cc & 0x7F, *value & 0x7F]
        }
        MidiEvent::Transport { play, rec, stop } => {
            if *play {
                vec![0xFA] // MIDI Start
            } else if *stop {
                vec![0xFC] // MIDI Stop
            } else if *rec {
                vec![0xFA]
            } else {
                Vec::new()
            }
        }
    }
}

/// Parse des octets MIDI bruts vers un `MidiEvent`.
pub fn parse_midi_bytes(data: &[u8]) -> Option<MidiEvent> {
    if data.is_empty() {
        return None;
    }
    let status = data[0];
    let cmd = status & 0xF0;
    let ch = status & 0x0F;

    match cmd {
        0x90 if data.len() >= 3 => {
            let vel = data[2];
            if vel == 0 {
                Some(MidiEvent::NoteOff {
                    channel: ch,
                    note: data[1],
                    velocity: 0,
                })
            } else {
                Some(MidiEvent::NoteOn {
                    channel: ch,
                    note: data[1],
                    velocity: vel,
                })
            }
        }
        0x80 if data.len() >= 3 => Some(MidiEvent::NoteOff {
            channel: ch,
            note: data[1],
            velocity: data[2],
        }),
        0xB0 if data.len() >= 3 => Some(MidiEvent::Cc {
            channel: ch,
            cc: data[1],
            value: data[2],
        }),
        _ => match status {
            0xFA => Some(MidiEvent::Transport {
                play: true,
                rec: false,
                stop: false,
            }),
            0xFC => Some(MidiEvent::Transport {
                play: false,
                rec: false,
                stop: true,
            }),
            _ => None,
        },
    }
}
