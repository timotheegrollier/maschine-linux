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

/// Accords en gamme (hors chromatique, comme l'Harmonizer de Maschine) :
/// degrés de la gamme empilés depuis la note du pad — majeur, mineur ou
/// diminué selon le degré, toujours dans la gamme.
pub const SCALE_CHORDS: &[ChordDef] = &[
    ChordDef {
        name: "1-3-5",
        intervals: &[0, 2, 4],
    },
    ChordDef {
        name: "1-3-5-7",
        intervals: &[0, 2, 4, 6],
    },
    ChordDef {
        name: "1-3",
        intervals: &[0, 2],
    },
    ChordDef {
        name: "1-5",
        intervals: &[0, 4],
    },
    ChordDef {
        name: "1-4-5",
        intervals: &[0, 3, 4],
    },
    ChordDef {
        name: "1-4-7",
        intervals: &[0, 3, 6],
    },
];

/// Nom d'une note MIDI comme dans Maschine (C3 = 60).
#[must_use]
pub fn note_name(note: u8) -> String {
    format!(
        "{}{}",
        NOTE_NAMES[note as usize % 12],
        i32::from(note) / 12 - 2
    )
}

/// État musical Keyboard/Chords partagé moteur/snapshot/GUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KeyboardState {
    /// Fondamentale 0..11 (C=0).
    pub root: u8,
    /// Octave de la fondamentale, notation Maschine (3 = C3 = MIDI 60), -2..=7.
    pub octave: i8,
    /// Index dans `SCALES`.
    pub scale: u8,
    /// Index dans `chord_list()` (mode Chords).
    pub chord: u8,
    /// Quantité d'inversion d'accord 0..3.
    pub inversion: u8,
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self {
            // C3 en chromatique : le réglage par défaut de la MK3.
            root: 0,
            octave: 3,
            scale: 0,
            chord: 0,
            inversion: 0,
        }
    }
}

impl KeyboardState {
    /// Note MIDI de base (fondamentale à l'octave choisie).
    pub fn base_note(self) -> i16 {
        (i16::from(self.octave) + 2) * 12 + i16::from(self.root.min(11))
    }

    pub fn scale_def(self) -> &'static ScaleDef {
        &SCALES[self.scale as usize % SCALES.len()]
    }

    fn chromatic(self) -> bool {
        self.scale_def().intervals.len() == 12
    }

    /// Accords proposés : intervalles fixes en chromatique, degrés de la
    /// gamme sinon (la liste change avec la gamme, comme sur Maschine).
    pub fn chord_list(self) -> &'static [ChordDef] {
        if self.chromatic() {
            CHORDS
        } else {
            SCALE_CHORDS
        }
    }

    pub fn chord_def(self) -> &'static ChordDef {
        let list = self.chord_list();
        &list[self.chord as usize % list.len()]
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

    /// Le pad joue la fondamentale ou une de ses octaves (pad allumé, les
    /// autres atténués, comme sur la MK3).
    pub fn is_root_pad(self, pad: u8) -> bool {
        self.keyboard_note(pad) % 12 == self.root % 12
    }

    /// Notes jouées par un pad : une note en KEYBOARD, un accord en CHORDS.
    pub fn pad_notes(self, chords: bool, pad: u8) -> Vec<u8> {
        if chords {
            self.chord_notes(pad)
        } else {
            vec![self.keyboard_note(pad)]
        }
    }

    /// Notes d'un pad en mode Chords : l'accord part de la note du pad (les
    /// pads montent comme en KEYBOARD) ; hors chromatique, il reste dans la gamme.
    pub fn chord_notes(self, pad: u8) -> Vec<u8> {
        let root = i16::from(self.keyboard_note(pad));
        let chromatic = self.chromatic();
        let mut notes: Vec<i16> = self
            .chord_def()
            .intervals
            .iter()
            .map(|&i| {
                if chromatic {
                    root + i16::from(i)
                } else {
                    i16::from(self.keyboard_note(pad.saturating_add(i)))
                }
            })
            .collect();
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

    /// SEMITONE − / + : la fondamentale bouge d'un demi-ton, octave comprise
    /// (B3 → C4), entre C-2 et B7.
    pub fn cycle_root(&mut self, delta: i32) {
        let n =
            (i32::from(self.octave) * 12 + i32::from(self.root) + delta).clamp(-24, 7 * 12 + 11);
        self.octave = n.div_euclid(12) as i8;
        self.root = n.rem_euclid(12) as u8;
    }

    pub fn cycle_octave(&mut self, delta: i32) {
        self.octave = (i32::from(self.octave) + delta).clamp(-2, 7) as i8;
    }

    pub fn cycle_chord(&mut self, delta: i32) {
        let len = self.chord_list().len() as i32;
        self.chord = (self.chord as i32 + delta).rem_euclid(len) as u8;
    }

    pub fn cycle_inversion(&mut self, delta: i32) {
        self.inversion = (self.inversion as i32 + delta).rem_euclid(4) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_start_on_c3_and_light_the_roots_like_the_mk3() {
        let kb = KeyboardState::default();
        // Chromatique depuis C3 (MIDI 60) sur le pad 1.
        assert_eq!(kb.keyboard_note(0), 60);
        assert_eq!(note_name(kb.keyboard_note(0)), "C3");
        assert_eq!(kb.keyboard_note(15), 75);
        let roots: Vec<u8> = (0..16).filter(|&p| kb.is_root_pad(p)).collect();
        assert_eq!(roots, vec![0, 12]);
        // SEMITONE + depuis B3 passe à C4.
        let mut kb = KeyboardState { root: 11, ..kb };
        kb.cycle_root(1);
        assert_eq!((kb.root, kb.octave, kb.keyboard_note(0)), (0, 4, 72));
        kb.cycle_octave(-9);
        assert_eq!(kb.octave, -2);
        assert_eq!(kb.keyboard_note(0), 0);
    }

    #[test]
    fn chords_climb_with_the_pads_and_stay_in_the_scale() {
        // Do majeur, 1-3-5 : C E G, puis D F A (mineur), B D F (diminué).
        let kb = KeyboardState {
            scale: 1,
            ..KeyboardState::default()
        };
        assert_eq!(kb.chord_def().name, "1-3-5");
        assert_eq!(kb.chord_notes(0), vec![60, 64, 67]);
        assert_eq!(kb.chord_notes(1), vec![62, 65, 69]);
        assert_eq!(kb.chord_notes(6), vec![71, 74, 77]);
        // Le pad 8 rejoue le premier degré une octave plus haut.
        assert_eq!(kb.chord_notes(7), vec![72, 76, 79]);
        // Chromatique : intervalles fixes (Triad = majeur sur chaque pad).
        let kb = KeyboardState::default();
        assert_eq!(kb.chord_notes(2), vec![62, 66, 69]);
        assert_eq!(kb.pad_notes(false, 2), vec![62]);
    }
}
