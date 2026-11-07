//! État musical des modes Keyboard/Chords (Maschine-like).

/// Noms des notes (classes de hauteur).
pub const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Définition d'une gamme : nom + intervalles en demi-tons depuis la fondamentale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScaleDef {
    pub name: &'static str,
    pub intervals: &'static [u8],
}

/// Gammes disponibles (boutons haut / DisplayBtn).
pub const SCALES: &[ScaleDef] = &[
    ScaleDef {
        name: "Chromatic",
        intervals: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    },
    ScaleDef {
        name: "Major",
        intervals: &[0, 2, 4, 5, 7, 9, 11],
    },
    ScaleDef {
        name: "Minor Nat",
        intervals: &[0, 2, 3, 5, 7, 8, 10],
    },
    ScaleDef {
        name: "Harm Minor",
        intervals: &[0, 2, 3, 5, 7, 8, 11],
    },
    ScaleDef {
        name: "Dorian",
        intervals: &[0, 2, 3, 5, 7, 9, 10],
    },
    ScaleDef {
        name: "Mixolydian",
        intervals: &[0, 2, 4, 5, 7, 9, 10],
    },
    ScaleDef {
        name: "Pent Minor",
        intervals: &[0, 3, 5, 7, 10],
    },
    ScaleDef {
        name: "Pent Major",
        intervals: &[0, 2, 4, 7, 9],
    },
];

/// Types d'accords pour le mode Chords.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChordDef {
    pub name: &'static str,
    pub intervals: &'static [u8],
}

pub const CHORDS: &[ChordDef] = &[
    ChordDef {
        name: "Triad",
        intervals: &[0, 4, 7],
    },
    ChordDef {
        name: "Minor",
        intervals: &[0, 3, 7],
    },
    ChordDef {
        name: "Sus2",
        intervals: &[0, 2, 7],
    },
    ChordDef {
        name: "Sus4",
        intervals: &[0, 5, 7],
    },
    ChordDef {
        name: "7th",
        intervals: &[0, 4, 7, 10],
    },
    ChordDef {
        name: "Min7",
        intervals: &[0, 3, 7, 10],
    },
    ChordDef {
        name: "Maj7",
        intervals: &[0, 4, 7, 11],
    },
    ChordDef {
        name: "Oct",
        intervals: &[0, 12],
    },
];

/// État musical Keyboard/Chords partagé moteur/snapshot/GUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KeyboardState {
    /// Fondamentale 0..11 (C=0).
    pub root: u8,
    /// Octave de base MIDI (typiquement 1..7, base C1=24..C7=96 selon mapping).
    pub octave: i8,
    /// Index dans `SCALES`.
    pub scale: u8,
    /// Index dans `CHORDS` (mode Chords).
    pub chord: u8,
    /// Quantité d'inversion d'accord 0..3.
    pub inversion: u8,
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self {
            root: 0,   // C
            octave: 3, // base around C3
            scale: 2,  // Minor Nat
            chord: 0,  // Triad
            inversion: 0,
        }
    }
}

impl KeyboardState {
    /// Note MIDI de base (fondamentale à l'octave choisie).
    pub fn base_note(self) -> i16 {
        (self.octave.max(0) as i16) * 12 + (self.root.min(11) as i16)
    }

    pub fn scale_def(self) -> &'static ScaleDef {
        &SCALES[self.scale as usize % SCALES.len()]
    }

    pub fn chord_def(self) -> &'static ChordDef {
        &CHORDS[self.chord as usize % CHORDS.len()]
    }

    pub fn root_name(self) -> &'static str {
        NOTE_NAMES[self.root as usize % 12]
    }

    /// Note d'un pad en mode Keyboard (gamme), 16 pads = 16 degrés consécutifs.
    pub fn keyboard_note(self, pad: u8) -> u8 {
        let scale = self.scale_def().intervals;
        let degree = pad as i16;
        let len = scale.len() as i16;
        let octave_shift = degree.div_euclid(len);
        let idx = degree.rem_euclid(len) as usize;
        let note = self.base_note() + octave_shift * 12 + scale[idx] as i16;
        note.clamp(0, 127) as u8
    }

    /// Notes d'un pad en mode Chords (accord diatonique sur le degré de la gamme).
    pub fn chord_notes(self, pad: u8) -> Vec<u8> {
        let scale = self.scale_def().intervals;
        let len = scale.len() as i16;
        let degree = (pad as i16).rem_euclid(len) as usize;
        let root = self.base_note() + scale[degree] as i16;
        let chord = self.chord_def().intervals;
        let mut notes: Vec<i16> = chord.iter().map(|&i| root + i as i16).collect();
        let inv = (self.inversion as usize).min(notes.len().saturating_sub(1));
        for i in 0..inv {
            if let Some(n) = notes.get_mut(i) {
                *n += 12;
            }
        }
        notes.sort_unstable();
        notes.into_iter().map(|n| n.clamp(0, 127) as u8).collect()
    }

    pub fn cycle_scale(&mut self, delta: i32) {
        let len = SCALES.len() as i32;
        self.scale = (self.scale as i32 + delta).rem_euclid(len) as u8;
    }

    pub fn cycle_root(&mut self, delta: i32) {
        self.root = (self.root as i32 + delta).rem_euclid(12) as u8;
    }

    pub fn cycle_octave(&mut self, delta: i32) {
        self.octave = (self.octave as i32 + delta).clamp(0, 8) as i8;
    }

    pub fn cycle_chord(&mut self, delta: i32) {
        let len = CHORDS.len() as i32;
        self.chord = (self.chord as i32 + delta).rem_euclid(len) as u8;
    }

    pub fn cycle_inversion(&mut self, delta: i32) {
        self.inversion = (self.inversion as i32 + delta).rem_euclid(4) as u8;
    }
}
