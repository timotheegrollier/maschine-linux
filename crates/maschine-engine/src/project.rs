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
            if i == 14 {
                s.name = "Acid 303".to_string();
                s.synth = Some(crate::acid_synth::AcidParams::preset_classic_acid());
            }
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
        add_note(7, 0, 3, 0, 95); // Clap doublé sur 1.4

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
        add_note(4, 1, 3, 1, 85); // Tom Low
        add_note(5, 1, 3, 3, 90); // Tom Mid

        // Ligne Acid TB-303 (Pad 14, C minor acid riff avec accents et slides)
        let mut add_acid_note =
            |bar: u32, beat: u32, sixteenth: u32, pitch: u8, vel: u8, len: u32| {
                let tick = (bar * 4 * 960) + (beat * 960) + (sixteenth * 240);
                pat.events.push(PatternEvent {
                    pad: 14,
                    tick,
                    velocity: vel,
                    length_ticks: len,
                    pitch,
                });
            };
        // Mesure 1
        add_acid_note(0, 0, 0, 36, 100, 200); // C2
        add_acid_note(0, 0, 2, 36, 90, 200); // C2
        add_acid_note(0, 1, 0, 39, 105, 200); // D#2
        add_acid_note(0, 1, 2, 48, 120, 240); // C3 (Accent!)
        add_acid_note(0, 2, 0, 46, 110, 240); // A#2 (Slide)
        add_acid_note(0, 2, 2, 43, 95, 200); // G2
        add_acid_note(0, 3, 0, 41, 100, 200); // F2
        add_acid_note(0, 3, 2, 39, 105, 200); // D#2
                                              // Mesure 2
        add_acid_note(1, 0, 0, 36, 105, 200); // C2
        add_acid_note(1, 0, 2, 36, 95, 200); // C2
        add_acid_note(1, 1, 0, 48, 125, 240); // C3 (Accent!)
        add_acid_note(1, 1, 2, 51, 115, 240); // D#3 (Slide)
        add_acid_note(1, 2, 0, 48, 100, 200); // C3
        add_acid_note(1, 2, 2, 46, 95, 200); // A#2
        add_acid_note(1, 3, 0, 43, 110, 200); // G2
        add_acid_note(1, 3, 2, 38, 105, 200); // D2

        g.patterns.push(pat);
        p.groups.push(g);

        // Groupes B à H — chaque pad de chaque groupe dispose d'une piste et d'un son distincts
        let extra_groups: [(&str, u8, [&str; 16]); 7] = [
            (
                "Kit 909",
                10,
                [
                    "909 Kick",
                    "909 Snare",
                    "909 Clsd HH",
                    "909 Open HH",
                    "909 Clap",
                    "909 Low Tom",
                    "909 Mid Tom",
                    "909 Hi Tom",
                    "909 Crash",
                    "909 Ride",
                    "909 Rimshot",
                    "909 Shaker",
                    "909 Perc 1",
                    "909 Perc 2",
                    "Kick Modeler",
                    "909 Sub",
                ],
            ),
            (
                "Nexus Synth",
                50,
                [
                    "Wavetable Synth",
                    "Saw Lead",
                    "Square Lead",
                    "Pluck Synth",
                    "Warm Pad",
                    "Poly Brass",
                    "Strings",
                    "Sub Bass",
                    "Acid Lead",
                    "Bell Synth",
                    "Organ",
                    "Choir",
                    "Sweep FX",
                    "Arp Synth",
                    "Deep Sub",
                    "White Noise",
                ],
            ),
            (
                "Bass & Acid",
                6,
                [
                    "303 Classic",
                    "303 Squelch",
                    "303 Disto",
                    "303 Dub",
                    "Sub 808",
                    "Reese Bass",
                    "FM Bass",
                    "Slap Bass",
                    "Moog Bass",
                    "Wobble",
                    "Acid Slide",
                    "Saw Bass",
                    "Deep Sub 1",
                    "Deep Sub 2",
                    "Punch Bass",
                    "Acid Lead",
                ],
            ),
            (
                "Percussion",
                34,
                [
                    "Bongo Low",
                    "Bongo High",
                    "Conga Low",
                    "Conga Mid",
                    "Conga High",
                    "Timbale",
                    "Claves",
                    "Woodblock",
                    "Tambourine",
                    "Maracas",
                    "Shaker Fast",
                    "Cabasa",
                    "Agogo",
                    "Triangle",
                    "Guiro",
                    "Cowbell Muted",
                ],
            ),
            (
                "Keys & Chords",
                42,
                [
                    "Grand Piano",
                    "Rhodes",
                    "Wurlitzer",
                    "Clavinet",
                    "Hammond B3",
                    "FM Electric",
                    "Vibraphone",
                    "Marimba",
                    "Celesta",
                    "Harpsichord",
                    "Accordion",
                    "Toy Piano",
                    "Bell Keys",
                    "Synth Keys",
                    "Vinyl Piano",
                    "Lo-Fi Keys",
                ],
            ),
            (
                "Vocal & SFX",
                78,
                [
                    "Vox Chant",
                    "Vox Ahh",
                    "Vox Ohh",
                    "Vox Hit",
                    "Riser 1",
                    "Riser 2",
                    "Fall 1",
                    "Fall 2",
                    "Laser",
                    "Scratch",
                    "Noise Sweep",
                    "Impact",
                    "Sub Drop",
                    "Reverse Cymbal",
                    "Glitch 1",
                    "Glitch 2",
                ],
            ),
            (
                "Hits & Stabs",
                58,
                [
                    "Rave Stab 1",
                    "Rave Stab 2",
                    "Horn Hit",
                    "Orchestra Hit",
                    "Brass Stab",
                    "Disco Hit",
                    "Dub Chord",
                    "Minor Stab",
                    "90s Organ",
                    "Tech Stab",
                    "House Chord",
                    "Chord 7th",
                    "Chord 9th",
                    "Chord Min7",
                    "Suspended",
                    "Power Chord",
                ],
            ),
        ];

        for (grp_idx, (grp_name, grp_color, sound_names)) in extra_groups.into_iter().enumerate() {
            let mut grp = Group::new(grp_name, grp_color);
            for (i, &sname) in sound_names.iter().enumerate() {
                let mut snd = Sound::new(sname, grp_color);
                snd.zones.push(Zone {
                    sample_path: None,
                    root_key: 36 + i as u8,
                    lokey: 0,
                    hikey: 127,
                    lovel: 0,
                    hivel: 127,
                    ..Default::default()
                });
                if grp_idx == 0 && i == 14 {
                    snd.kick_synth = Some(crate::kick_synth::KickParams::default());
                } else if grp_idx == 1 && i == 0 {
                    snd.wavetable_synth = Some(crate::wavetable_synth::WavetableParams::default());
                } else if grp_idx == 2 {
                    if i == 0 {
                        snd.synth = Some(crate::acid_synth::AcidParams::preset_classic_acid());
                    } else if i == 1 {
                        snd.synth = Some(crate::acid_synth::AcidParams::preset_squelch_lead());
                    } else if i == 2 {
                        snd.synth = Some(crate::acid_synth::AcidParams::preset_distorted());
                    }
                }
                grp.sounds[i] = Some(snd);
            }
            grp.patterns
                .push(Pattern::new("Pattern 1", BarsBeatsTicks::new(2, 0, 0)));
            p.groups.push(grp);
        }

        p.scenes.push(Scene {
            name: "Scene 1".to_string(),
            color: 22,
            patterns: vec![Some(0); 8],
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

    /// Sauve en JSON. `pretty=true` produit un fichier lisible (GUI/MK3/docs),
    /// `pretty=false` un JSON compact pour les sauvegardes rapides (Shift+Save).
    /// L'écriture est atomique (fichier temporaire puis renommage).
    pub fn save_json_with(&self, path: &std::path::Path, pretty: bool) -> crate::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = if pretty {
            serde_json::to_string_pretty(self)?
        } else {
            serde_json::to_string(self)?
        };
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, json)?;
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        std::fs::rename(&tmp, path).map_err(|e| {
            // Fallback si rename cross-device/Windows-like impossible.
            let _ = std::fs::write(path, std::fs::read(&tmp).unwrap_or_default());
            let _ = std::fs::remove_file(&tmp);
            crate::EngineError::Io(e)
        })?;
        Ok(())
    }

    /// Sauve en JSON pretty (compatible API existante).
    pub fn save_json(&self, path: &std::path::Path) -> crate::Result<()> {
        self.save_json_with(path, true)
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
    /// Pan
    #[serde(default)]
    pub pan: f32,
    /// Chaîne d'effets du Group
    #[serde(default)]
    pub fx_chain: Vec<crate::fx::FxSlot>,
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
            pan: 0.0,
            fx_chain: Vec::new(),
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
    /// Synthétiseur optionnel (ex. TB-303 Acid)
    #[serde(default)]
    pub synth: Option<crate::acid_synth::AcidParams>,
    /// Modélisation de Kick optionnelle (Techno / Analog)
    #[serde(default)]
    pub kick_synth: Option<crate::kick_synth::KickParams>,
    /// Synthétiseur Wavetable / Nexus optionnel (ROMpler & Polyphonique)
    #[serde(default)]
    pub wavetable_synth: Option<crate::wavetable_synth::WavetableParams>,
    /// Chaîne d'effets du Pad
    #[serde(default)]
    pub fx_chain: Vec<crate::fx::FxSlot>,
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
            synth: None,
            kick_synth: None,
            wavetable_synth: None,
            fx_chain: Vec::new(),
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    /// Type
    pub kind: FilterKind,
    /// Cutoff Hz
    pub cutoff_hz: f32,
    /// Resonance 0..1
    pub resonance: f32,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            kind: FilterKind::Off,
            cutoff_hz: 20000.0, // Filtre grand ouvert par défaut (pas de filtrage)
            resonance: 0.0,
        }
    }
}

/// Type filtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
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

/// Ticks par pas de 16e (PPQ 960).
pub const TICKS_PER_STEP: u32 = 240;
/// Ticks par mesure 4/4.
pub const TICKS_PER_BAR: u32 = 4 * 960;

/// Vrai si deux ticks visent le même pas de 16e du pattern (index de la 16e
/// la plus proche : `(tick + 120) / 240`).
/// Tolère les micro-décalages (humanize ±12, nudge, swing stocké, ±119 ticks)
/// y compris quand ils franchissent une frontière de case : la comparaison
/// exacte `e.tick == tick` obligeait à appuyer plusieurs fois pour
/// (dés)activer un pas.
#[must_use]
pub fn same_step_cell(a_tick: u32, b_tick: u32) -> bool {
    (a_tick + TICKS_PER_STEP / 2) / TICKS_PER_STEP == (b_tick + TICKS_PER_STEP / 2) / TICKS_PER_STEP
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

impl Project {
    /// Index du pattern à éditer / jouer pour un groupe : celui assigné dans
    /// la scène active, sinon le premier pattern (comportement historique).
    /// Retourne `None` si le groupe n'a aucun pattern.
    #[must_use]
    pub fn active_pattern_index(&self, group: usize, active_scene: Option<usize>) -> Option<usize> {
        let group_len = self
            .groups
            .get(group)
            .map(|g| g.patterns.len())
            .unwrap_or(0);
        if group_len == 0 {
            return None;
        }
        if let Some(scene) = active_scene.and_then(|si| self.scenes.get(si)) {
            if let Some(slot) = scene.patterns.get(group) {
                return match *slot {
                    Some(pi) if pi < group_len => Some(pi),
                    Some(_) => Some(0),
                    None => None,
                };
            }
        }
        Some(0)
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

    #[test]
    fn same_step_cell_tolerates_humanize() {
        assert!(same_step_cell(0, 0));
        assert!(same_step_cell(0, 12)); // humanize +12
        assert!(same_step_cell(240, 228)); // humanize -12 (frontière franchie)
        assert!(same_step_cell(240, 239)); // nudge -1
        assert!(!same_step_cell(0, 240));
        assert!(!same_step_cell(239, 0));
        assert!(!same_step_cell(0, 120)); // pile entre deux pas -> rattaché au suivant
    }

    #[test]
    fn active_pattern_index_uses_scene_then_fallback() {
        let mut p = Project::new_demo();
        // Démo : scène 0 -> pattern 0.
        assert_eq!(p.active_pattern_index(0, Some(0)), Some(0));
        // Sans scène : premier pattern.
        assert_eq!(p.active_pattern_index(0, None), Some(0));
        // Assignation hors borne : repli sur 0.
        p.scenes[0].patterns[0] = Some(7);
        assert_eq!(p.active_pattern_index(0, Some(0)), Some(0));
        // Désélection explicite : None
        p.scenes[0].patterns[0] = None;
        assert_eq!(p.active_pattern_index(0, Some(0)), None);
        // Groupe sans pattern : None.
        p.groups[0].patterns.clear();
        assert_eq!(p.active_pattern_index(0, Some(0)), None);
    }
}
