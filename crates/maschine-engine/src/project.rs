//! Modèle Project — fidèle à Maschine : Groups, Patterns, Scenes, Sections.

use serde::{Deserialize, Serialize};

/// Project racine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Version schéma
    pub schema_version: u32,
    /// Nom
    pub name: String,
    /// BPM
    #[serde(default = "default_bpm")]
    pub bpm: f32,
    /// Swing 0..1
    #[serde(default)]
    pub swing: f32,
    /// Time signature num/denom
    #[serde(default = "default_time_sig")]
    pub time_signature: (u8, u8),
    /// Groups
    #[serde(default)]
    pub groups: Vec<Group>,
    /// Scenes
    #[serde(default)]
    pub scenes: Vec<Scene>,
    /// Song sections (arranger)
    #[serde(default)]
    pub song: Vec<Section>,
    /// Master
    #[serde(default)]
    pub master: MasterChannel,
}

fn default_bpm() -> f32 {
    120.0
}
fn default_time_sig() -> (u8, u8) {
    (4, 4)
}

impl Default for Project {
    fn default() -> Self {
        Self {
            schema_version: crate::SCHEMA_VERSION,
            name: "Untitled".to_string(),
            bpm: 120.0,
            swing: 0.0,
            time_signature: (4, 4),
            groups: Vec::new(),
            scenes: Vec::new(),
            song: Vec::new(),
            master: MasterChannel::default(),
        }
    }
}

impl Project {
    /// Crée un projet vide avec un Group démo (Kit).
    #[must_use]
    pub fn new_demo() -> Self {
        let mut p = Self {
            name: "Demo".to_string(),
            ..Default::default()
        };
        let mut g = Group::new("Kit 808", 22);
        // 16 sounds demo avec couleurs arc-en-ciel
        let names = [
            "Kick",
            "Snare",
            "HiHat Closed",
            "HiHat Open",
            "Clap",
            "Tom Low",
            "Tom Mid",
            "Tom High",
            "Crash",
            "Ride",
            "Shaker",
            "Cowbell",
            "Perc 1",
            "Perc 2",
            "Bass",
            "FX",
        ];
        let colors = [
            6, 22, 30, 42, 50, 58, 10, 14, 26, 31, 34, 38, 11, 27, 33, 78,
        ];
        for (i, (nm, col)) in names.iter().zip(colors.iter()).enumerate() {
            let mut s = Sound::new(nm, *col);
            // zone placeholder — sera remplacée par vrai sample
            s.zones.push(Zone {
                sample_path: None,
                root_key: 36 + i as u8,
                lokey: 0,
                hikey: 127,
                lovel: 0,
                hivel: 127,
                ..Default::default()
            });
            g.sounds[i] = Some(s);
        }
        // Pattern 1 : Groove 808 complet 2 mesures (Kick, Snare, Hi-Hats, Clap, Toms)
        let mut pat = Pattern::new("808 Groove", BarsBeatsTicks::new(2, 0, 0));

        // Helper pour ajouter une note
        let mut add_note = |pad: u8, bar: u32, beat: u32, sixteenth: u32, vel: u8| {
            let tick = (bar * 4 * 960) + (beat * 960) + (sixteenth * 240);
            pat.events.push(PatternEvent {
                pad,
                tick,
                velocity: vel,
                length_ticks: 120,
                pitch: 36 + pad,
            });
        };

        // Mesure 1
        // Kicks
        add_note(0, 0, 0, 0, 115); // Kick sur 1.1
        add_note(0, 0, 2, 2, 105); // Kick sur 1.3.3 (syncope)

        // Snares (sur temps 2 et 4)
        add_note(1, 0, 1, 0, 110); // Snare sur 1.2
        add_note(1, 0, 3, 0, 115); // Snare sur 1.4
        add_note(7, 0, 3, 0, 95);  // Clap doublé sur 1.4

        // Closed Hi-Hats (en croches 8ths sur toute la mesure)
        for beat in 0..4 {
            add_note(2, 0, beat, 0, 90);
            add_note(2, 0, beat, 2, 75);
        }
        // Open Hi-Hat sur contre-temps
        add_note(3, 0, 1, 2, 85);
        add_note(3, 0, 3, 2, 85);

        // Mesure 2 (variation rythmique)
        // Kicks
        add_note(0, 1, 0, 0, 115); // Kick sur 2.1
        add_note(0, 1, 1, 2, 100); // Kick sur 2.2.3
        add_note(0, 1, 2, 0, 110); // Kick sur 2.3
        add_note(0, 1, 3, 2, 105); // Kick sur 2.4.3

        // Snares
        add_note(1, 1, 1, 0, 110); // Snare sur 2.2
        add_note(1, 1, 3, 0, 120); // Snare sur 2.4
        add_note(7, 1, 3, 0, 100); // Clap sur 2.4

        // Closed Hi-Hats
        for beat in 0..4 {
            add_note(2, 1, beat, 0, 90);
            add_note(2, 1, beat, 2, 75);
        }
        // Open Hi-Hat
        add_note(3, 1, 2, 2, 90);

        // Fill percussif fin de mesure (Toms)
        add_note(4, 1, 3, 1, 85);  // Tom Low
        add_note(5, 1, 3, 3, 90);  // Tom Mid

        g.patterns.push(pat);
        p.groups.push(g);
        p.scenes.push(Scene {
            name: "Scene 1".to_string(),
            color: 22,
            patterns: vec![Some(0)],
        });
        p.song.push(Section {
            scene: 0,
            length: BarsBeatsTicks::new(2, 0, 0),
            name: "Intro".to_string(),
        });
        p
    }

    /// Valide la cohérence.
    pub fn validate(&self) -> crate::Result<()> {
        if self.bpm < 20.0 || self.bpm > 999.0 {
            return Err(crate::EngineError::Validation(format!(
                "BPM invalide {}",
                self.bpm
            )));
        }
        for (gi, g) in self.groups.iter().enumerate() {
            if g.sounds.len() != 16 {
                return Err(crate::EngineError::Validation(format!(
                    "Group {gi} sounds len !=16"
                )));
            }
            for pat in &g.patterns {
                for ev in &pat.events {
                    if ev.pad >= 16 {
                        return Err(crate::EngineError::Validation(format!(
                            "pad {} hors borne",
                            ev.pad
                        )));
                    }
                }
            }
        }
        for (si, sc) in self.scenes.iter().enumerate() {
            if sc.patterns.len() != self.groups.len() {
                return Err(crate::EngineError::Validation(format!(
                    "Scene {si} patterns len {} != groups {}",
                    sc.patterns.len(),
                    self.groups.len()
                )));
            }
        }
        for sec in &self.song {
            if sec.scene >= self.scenes.len() {
                return Err(crate::EngineError::Validation(format!(
                    "Section scene {} hors borne {}",
                    sec.scene,
                    self.scenes.len()
                )));
            }
        }
        Ok(())
    }

    /// Sauve en JSON pretty.
    pub fn save_json(&self, path: &std::path::Path) -> crate::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Charge depuis JSON.
    pub fn load_json(path: &std::path::Path) -> crate::Result<Self> {
        let data = std::fs::read_to_string(path)?;
        let mut p: Self = serde_json::from_str(&data)?;
        p.migrate();
        p.validate()?;
        Ok(p)
    }

    /// Migration schéma.
    pub fn migrate(&mut self) {
        if self.schema_version == 0 {
            self.schema_version = 1;
            if self.time_signature == (0, 0) {
                self.time_signature = (4, 4);
            }
        }
    }
}

/// Master channel.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MasterChannel {
    /// Volume dB
    pub volume_db: f32,
    /// Pan -1..1
    pub pan: f32,
}

/// Group — 16 Sounds fixes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    /// Nom
    pub name: String,
    /// Couleur palette index
    pub color: u8,
    /// 16 sounds (Some/None pour gaps)
    pub sounds: [Option<Sound>; 16],
    /// Patterns du Group
    #[serde(default)]
    pub patterns: Vec<Pattern>,
    /// Couleur étendue
    #[serde(default)]
    pub muted: bool,
    /// Solo
    #[serde(default)]
    pub solo: bool,
    /// Volume
    #[serde(default)]
    pub volume_db: f32,
}

impl Group {
    /// Nouveau Group vide.
    #[must_use]
    pub fn new(name: &str, color: u8) -> Self {
        Self {
            name: name.to_string(),
            color,
            sounds: Default::default(),
            patterns: Vec::new(),
            muted: false,
            solo: false,
            volume_db: 0.0,
        }
    }
}

/// Sound — un pad.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sound {
    /// Nom
    pub name: String,
    /// Couleur
    pub color: u8,
    /// Volume
    #[serde(default)]
    pub volume_db: f32,
    /// Pan
    #[serde(default)]
    pub pan: f32,
    /// Mute
    #[serde(default)]
    pub muted: bool,
    /// Solo
    #[serde(default)]
    pub solo: bool,
    /// Choke group (hi-hat)
    pub choke_group: Option<u8>,
    /// Polyphony
    #[serde(default = "default_poly")]
    pub polyphony: u8,
    /// Zones multi-sample
    #[serde(default)]
    pub zones: Vec<Zone>,
    /// ADSR
    #[serde(default)]
    pub adsr: Adsr,
    /// Filtre
    #[serde(default)]
    pub filter: Filter,
}

fn default_poly() -> u8 {
    8
}

impl Sound {
    /// Nouveau Sound.
    #[must_use]
    pub fn new(name: &str, color: u8) -> Self {
        Self {
            name: name.to_string(),
            color,
            volume_db: 0.0,
            pan: 0.0,
            muted: false,
            solo: false,
            choke_group: None,
            polyphony: 8,
            zones: Vec::new(),
            adsr: Adsr::default(),
            filter: Filter::default(),
        }
    }
}

/// ADSR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adsr {
    /// Attack ms
    pub attack_ms: f32,
    /// Decay ms
    pub decay_ms: f32,
    /// Sustain 0..1
    pub sustain: f32,
    /// Release ms
    pub release_ms: f32,
}

impl Default for Adsr {
    fn default() -> Self {
        Self {
            attack_ms: 2.0,
            decay_ms: 100.0,
            sustain: 0.8,
            release_ms: 150.0,
        }
    }
}

/// Filtre.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Filter {
    /// Type
    pub kind: FilterKind,
    /// Cutoff Hz
    pub cutoff_hz: f32,
    /// Resonance 0..1
    pub resonance: f32,
}

/// Type filtre.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum FilterKind {
    /// Off
    #[default]
    Off,
    /// Lowpass
    LowPass,
    /// HighPass
    HighPass,
    /// BandPass
    BandPass,
}

/// Zone multi-sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    /// Chemin sample (relatif au projet)
    pub sample_path: Option<String>,
    /// Root key
    pub root_key: u8,
    /// Lokey
    pub lokey: u8,
    /// Hikey
    pub hikey: u8,
    /// Lovel
    pub lovel: u8,
    /// Hivel
    pub hivel: u8,
    /// Seq len pour round-robin
    #[serde(default = "one")]
    pub seq_length: u8,
    /// Seq pos
    #[serde(default)]
    pub seq_position: u8,
    /// Tune cents
    #[serde(default)]
    pub tune_cents: i16,
    /// Pan -1..1
    #[serde(default)]
    pub pan: f32,
    /// Volume dB
    #[serde(default)]
    pub volume_db: f32,
}

fn one() -> u8 {
    1
}

impl Default for Zone {
    fn default() -> Self {
        Self {
            sample_path: None,
            root_key: 60,
            lokey: 0,
            hikey: 127,
            lovel: 0,
            hivel: 127,
            seq_length: 1,
            seq_position: 0,
            tune_cents: 0,
            pan: 0.0,
            volume_db: 0.0,
        }
    }
}

/// Événement pattern.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatternEvent {
    /// Pad 0..15
    pub pad: u8,
    /// Tick absolu dans pattern (0 = début)
    pub tick: u32,
    /// Vélocité 1..127
    pub velocity: u8,
    /// Longueur ticks
    pub length_ticks: u32,
    /// Pitch MIDI (36..)
    pub pitch: u8,
}

/// Longueur pattern / section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BarsBeatsTicks {
    /// Bars
    pub bars: u16,
    /// Beats
    pub beats: u8,
    /// Ticks (0..959)
    pub ticks: u16,
}

impl BarsBeatsTicks {
    /// Crée.
    #[must_use]
    pub fn new(bars: u16, beats: u8, ticks: u16) -> Self {
        Self { bars, beats, ticks }
    }

    /// En ticks totaux (PPQ 960, 4/4).
    #[must_use]
    pub fn to_ticks(self) -> u32 {
        (self.bars as u32 * 4 * 960) + (self.beats as u32 * 960) + self.ticks as u32
    }

    /// Depuis ticks.
    #[must_use]
    pub fn from_ticks(ticks: u32) -> Self {
        let bars = (ticks / (4 * 960)) as u16;
        let rem = ticks % (4 * 960);
        let beats = (rem / 960) as u8;
        let ticks = (rem % 960) as u16;
        Self { bars, beats, ticks }
    }
}

impl std::fmt::Display for BarsBeatsTicks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{:03}", self.bars, self.beats, self.ticks)
    }
}

/// Pattern — appartient à un Group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    /// Nom
    pub name: String,
    /// Couleur
    #[serde(default)]
    pub color: u8,
    /// Longueur
    pub length: BarsBeatsTicks,
    /// Événements
    #[serde(default)]
    pub events: Vec<PatternEvent>,
    /// Swing 0..1 override Group
    #[serde(default)]
    pub swing: Option<f32>,
}

impl Pattern {
    /// Nouveau.
    #[must_use]
    pub fn new(name: &str, length: BarsBeatsTicks) -> Self {
        Self {
            name: name.to_string(),
            color: 22,
            length,
            events: Vec::new(),
            swing: None,
        }
    }
}

/// Scene — un pattern par Group max.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    /// Nom
    pub name: String,
    /// Couleur
    pub color: u8,
    /// Option<PatternId> par Group (index dans group.patterns)
    pub patterns: Vec<Option<usize>>,
}

impl Scene {
    /// Nouvelle vide.
    #[must_use]
    pub fn new(name: &str, num_groups: usize) -> Self {
        Self {
            name: name.to_string(),
            color: 22,
            patterns: vec![None; num_groups],
        }
    }
}

/// Section — référence à une Scene sur la timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    /// Index scene
    pub scene: usize,
    /// Longueur (peut être < scene length = trim)
    pub length: BarsBeatsTicks,
    /// Nom
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_validate() {
        let p = Project::new_demo();
        p.validate().unwrap();
    }

    #[test]
    fn bbt_roundtrip() {
        let b = BarsBeatsTicks::new(2, 1, 480);
        let ticks = b.to_ticks();
        let b2 = BarsBeatsTicks::from_ticks(ticks);
        assert_eq!(b, b2);
        assert_eq!(ticks, 2 * 4 * 960 + 960 + 480);
    }

    #[test]
    fn save_load_roundtrip() {
        let p = Project::new_demo();
        let dir = std::env::temp_dir().join("maschine-test");
        let path = dir.join("proj.json");
        p.save_json(&path).unwrap();
        let p2 = Project::load_json(&path).unwrap();
        assert_eq!(p.name, p2.name);
        assert_eq!(p.groups.len(), p2.groups.len());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn scene_mismatch_fails() {
        let mut p = Project::new_demo();
        p.scenes[0].patterns.push(None); // trop long
        assert!(p.validate().is_err());
    }
}
