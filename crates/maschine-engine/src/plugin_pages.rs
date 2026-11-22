//! Pages de paramètres des plugins, comme le mode PLUG-IN de la MK3 : huit
//! paramètres par page, un par knob (nom et valeur affichés juste au-dessus),
//! pages parcourues avec les boutons Page ◄ ►. Chaque page déclare aussi les
//! libellés des quatre boutons d'écran de droite (raccourcis de la page).
//!
//! Ces tables servent à la fois à l'affichage (écrans MK3, miroir GUI) et au
//! routage des knobs : ce qui est affiché est exactement ce qui est piloté.

use crate::acid_synth::{AcidParams, AcidWaveform};
use crate::fx::{FxSlot, FxType};
use crate::kick_synth::{KickDriveType, KickFilterMode, KickParams, KickWaveform};
use crate::project::{FilterKind, Sound};
use crate::wavetable_synth::{LfoTarget, WavetableFilterMode, WavetableParams, WavetableWaveform};

/// Ticks bruts d'un cran de knob (1024 par tour).
pub const DETENT_TICKS: i32 = 32;
/// Ticks pour parcourir toute la plage d'un paramètre continu (~1,5 tour).
const RANGE_TICKS: f32 = 1536.0;

/// Unité d'affichage d'une valeur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Percent,
    Db,
    Hz,
    Ms,
    Semitones,
    Cents,
    Pan,
    Ratio,
    /// Taux de compression (« 4.0:1 »).
    Compression,
    Count,
}

/// Échelle d'un paramètre : réponse aux knobs et affichage.
#[derive(Debug, Clone, Copy)]
pub enum Scale {
    /// Continu, linéaire.
    Linear { min: f32, max: f32, unit: Unit },
    /// Continu, logarithmique (fréquences, temps) ; `min` > 0.
    Log { min: f32, max: f32, unit: Unit },
    /// Entier : un pas par cran.
    Steps { min: f32, max: f32, unit: Unit },
    /// Liste de choix : un choix par cran.
    Choice(&'static [&'static str]),
}

impl Scale {
    /// Bornes de la valeur brute.
    #[must_use]
    pub fn bounds(self) -> (f32, f32) {
        match self {
            Self::Linear { min, max, .. }
            | Self::Log { min, max, .. }
            | Self::Steps { min, max, .. } => (min, max),
            Self::Choice(names) => (0.0, names.len().saturating_sub(1) as f32),
        }
    }

    /// Avance par crans (entiers, choix) plutôt qu'en continu ?
    #[must_use]
    pub fn is_stepped(self) -> bool {
        matches!(self, Self::Steps { .. } | Self::Choice(_))
    }

    /// Position 0..1 (barres de valeur).
    #[must_use]
    pub fn norm(self, v: f32) -> f32 {
        let (min, max) = self.bounds();
        let n = match self {
            Self::Log { .. } => (v.max(min) / min).ln() / (max / min).ln(),
            _ => (v - min) / (max - min).max(f32::EPSILON),
        };
        n.clamp(0.0, 1.0)
    }

    /// Valeur après `ticks` de rotation continue (`fine` = Shift, ÷10).
    #[must_use]
    pub fn turn(self, v: f32, ticks: i32, fine: bool) -> f32 {
        let t = ticks as f32 / RANGE_TICKS / if fine { 10.0 } else { 1.0 };
        let (min, max) = self.bounds();
        let next = match self {
            Self::Log { .. } => v.max(min) * (max / min).powf(t),
            _ => v + t * (max - min),
        };
        next.clamp(min, max)
    }

    /// Valeur après `steps` crans (entiers, choix).
    #[must_use]
    pub fn step(self, v: f32, steps: i32) -> f32 {
        let (min, max) = self.bounds();
        (v.round() + steps as f32).clamp(min, max)
    }

    /// Valeur lisible, avec son unité.
    #[must_use]
    pub fn format(self, v: f32) -> String {
        let unit = match self {
            Self::Choice(names) => {
                return names
                    .get(v.round().max(0.0) as usize)
                    .copied()
                    .unwrap_or("--")
                    .to_string()
            }
            Self::Linear { unit, .. } | Self::Log { unit, .. } | Self::Steps { unit, .. } => unit,
        };
        match unit {
            Unit::Percent => format!("{:.0} %", v * 100.0),
            Unit::Db if v <= -59.5 => "-inf dB".to_string(),
            Unit::Db => format!("{v:+.1} dB"),
            Unit::Hz if v >= 1000.0 => format!("{:.2} kHz", v / 1000.0),
            Unit::Hz if v >= 100.0 => format!("{v:.0} Hz"),
            Unit::Hz => format!("{v:.1} Hz"),
            Unit::Ms if v >= 1000.0 => format!("{:.2} s", v / 1000.0),
            Unit::Ms if v >= 100.0 => format!("{v:.0} ms"),
            Unit::Ms => format!("{v:.1} ms"),
            Unit::Semitones => format!("{v:+.1} st"),
            Unit::Cents => format!("{:+.0} ct", v.round()),
            Unit::Pan if v.abs() < 0.02 => "C".to_string(),
            Unit::Pan if v < 0.0 => format!("L{:.0}", -v * 100.0),
            Unit::Pan => format!("R{:.0}", v * 100.0),
            Unit::Ratio => format!("x{v:.2}"),
            Unit::Compression => format!("{v:.1}:1"),
            Unit::Count => format!("{:.0}", v.round()),
        }
    }
}

/// Un paramètre de plugin piloté par un knob.
pub struct Param<P> {
    /// Nom court affiché au-dessus du knob.
    pub name: &'static str,
    pub scale: Scale,
    pub get: fn(&P) -> f32,
    pub set: fn(&mut P, f32),
}

/// Une page : 8 paramètres (knobs 1–8), deux titres de section (knobs 1–4
/// et 5–8) et les libellés des boutons d'écran 5–8.
pub struct Page<P: 'static> {
    pub name: &'static str,
    pub sections: [&'static str; 2],
    pub params: [Option<Param<P>>; 8],
    pub buttons: [&'static str; 4],
}

/// Instrument affiché en mode PLUGIN (selon le Sound focalisé).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instrument {
    Sampler,
    Acid,
    Kick,
    Wavetable,
}

impl Instrument {
    /// Nom affiché en tête d'écran.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Sampler => "SAMPLER",
            Self::Acid => "TB-303 ACID",
            Self::Kick => "KICK SYNTH",
            Self::Wavetable => "NEXUS WAVETABLE",
        }
    }

    /// Noms des pages, dans l'ordre des boutons Page ◄ ►.
    #[must_use]
    pub fn page_names(self) -> Vec<&'static str> {
        match self {
            Self::Sampler => SAMPLER_PAGES.iter().map(|p| p.name).collect(),
            Self::Acid => ACID_PAGES.iter().map(|p| p.name).collect(),
            Self::Kick => KICK_PAGES.iter().map(|p| p.name).collect(),
            Self::Wavetable => WAVETABLE_PAGES.iter().map(|p| p.name).collect(),
        }
    }

    /// Nombre de pages.
    #[must_use]
    pub fn page_count(self) -> usize {
        self.page_names().len()
    }

    /// Nom lisible (chaîne de plugins).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Sampler => "Sampler",
            Self::Acid => "TB-303 Acid",
            Self::Kick => "Kick Synth",
            Self::Wavetable => "Nexus Wavetable",
        }
    }
}

/// Instrument affiché en mode PLUGIN : onglet 4 hors page ampli guitare,
/// d'après le plugin du Sound focalisé (sampler à défaut de synthé).
#[must_use]
pub fn instrument_for(
    left_tab: u8,
    plugin_page: u8,
    kick: bool,
    wavetable: bool,
    acid: bool,
) -> Option<Instrument> {
    if left_tab != 4 || plugin_page == 2 {
        return None;
    }
    Some(if kick {
        Instrument::Kick
    } else if wavetable {
        Instrument::Wavetable
    } else if acid {
        Instrument::Acid
    } else {
        Instrument::Sampler
    })
}

/// Paramètre lu pour l'affichage : nom, valeur formatée, position 0..1.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamView {
    pub name: &'static str,
    pub value: String,
    pub norm: f32,
    /// Valeur brute (Hz, ms, 0..1…) pour les courbes des écrans.
    pub raw: f32,
}

/// Vue d'une page prête à dessiner.
#[derive(Debug, Clone, PartialEq)]
pub struct PageView {
    pub name: &'static str,
    pub sections: [&'static str; 2],
    pub params: [Option<ParamView>; 8],
    pub buttons: [&'static str; 4],
}

/// Lit une page d'un jeu de paramètres.
#[must_use]
pub fn view<P>(pages: &[Page<P>], page: usize, p: &P) -> PageView {
    let pg = &pages[page.min(pages.len() - 1)];
    PageView {
        name: pg.name,
        sections: pg.sections,
        params: std::array::from_fn(|i| {
            pg.params[i].as_ref().map(|param| {
                let v = (param.get)(p);
                ParamView {
                    name: param.name,
                    value: param.scale.format(v),
                    norm: param.scale.norm(v),
                    raw: v,
                }
            })
        }),
        buttons: pg.buttons,
    }
}

/// Applique une rotation de knob : `ticks` continus, ou `steps` crans pour
/// les paramètres entiers / à choix. Renvoie `true` si la valeur a changé.
pub fn turn<P>(
    pages: &[Page<P>],
    page: usize,
    knob: usize,
    p: &mut P,
    ticks: i32,
    steps: i32,
    fine: bool,
) -> bool {
    let Some(param) = pages[page.min(pages.len() - 1)]
        .params
        .get(knob)
        .and_then(Option::as_ref)
    else {
        return false;
    };
    let v = (param.get)(p);
    let next = if param.scale.is_stepped() {
        param.scale.step(v, steps)
    } else {
        param.scale.turn(v, ticks, fine)
    };
    if (next - v).abs() <= f32::EPSILON {
        return false;
    }
    (param.set)(p, next);
    true
}

/// Le knob `knob` de la page avance-t-il par crans ?
#[must_use]
pub fn is_stepped<P>(pages: &[Page<P>], page: usize, knob: usize) -> bool {
    pages[page.min(pages.len() - 1)]
        .params
        .get(knob)
        .and_then(Option::as_ref)
        .is_some_and(|param| param.scale.is_stepped())
}

// ── Aides : valeurs énumérées ↔ index ──────────────────────────────────

fn index_of<T: PartialEq>(all: &[T], v: &T) -> f32 {
    all.iter().position(|x| x == v).unwrap_or(0) as f32
}

fn pick<T: Copy>(all: &[T], v: f32) -> T {
    all[(v.round().max(0.0) as usize).min(all.len() - 1)]
}

const fn lin(min: f32, max: f32, unit: Unit) -> Scale {
    Scale::Linear { min, max, unit }
}

const fn log(min: f32, max: f32, unit: Unit) -> Scale {
    Scale::Log { min, max, unit }
}

const fn steps(min: f32, max: f32, unit: Unit) -> Scale {
    Scale::Steps { min, max, unit }
}

const PCT: Scale = lin(0.0, 1.0, Unit::Percent);
const VOLUME: Scale = lin(-60.0, 12.0, Unit::Db);
const PAN: Scale = lin(-1.0, 1.0, Unit::Pan);

/// Paramètre `f32` lu/écrit directement dans un champ.
macro_rules! field {
    ($name:expr, $scale:expr, $($f:ident).+) => {
        Some(Param {
            name: $name,
            scale: $scale,
            get: |p| p.$($f).+,
            set: |p, v| p.$($f).+ = v,
        })
    };
}

/// Paramètre entier (`i8`/`u8`) lu/écrit dans un champ.
macro_rules! int_field {
    ($name:expr, $scale:expr, $f:ident, $t:ty) => {
        Some(Param {
            name: $name,
            scale: $scale,
            get: |p| f32::from(p.$f),
            set: |p, v| p.$f = v.round() as $t,
        })
    };
}

/// Paramètre à choix stocké dans une énumération.
macro_rules! choice {
    ($name:expr, $labels:expr, $all:expr, $f:ident) => {
        Some(Param {
            name: $name,
            scale: Scale::Choice($labels),
            get: |p| index_of(&$all, &p.$f),
            set: |p, v| p.$f = pick(&$all, v),
        })
    };
}

// ── SAMPLER (Sound) ─────────────────────────────────────────────────────

const FILTER_KINDS: [FilterKind; 6] = [
    FilterKind::Off,
    FilterKind::LowPass,
    FilterKind::HighPass,
    FilterKind::BandPass,
    FilterKind::Notch,
    FilterKind::Peak,
];
const FILTER_NAMES: &[&str] = &["OFF", "LP", "HP", "BP", "NOTCH", "PEAK"];
const CHOKE_NAMES: &[&str] = &["OFF", "1", "2", "3", "4", "5", "6", "7", "8"];

pub static SAMPLER_PAGES: [Page<Sound>; 2] = [
    Page {
        name: "SON",
        sections: ["NIVEAU & FILTRE", "ENVELOPPE"],
        params: [
            field!("VOLUME", VOLUME, volume_db),
            field!("PAN", PAN, pan),
            field!("CUTOFF", log(20.0, 20000.0, Unit::Hz), filter.cutoff_hz),
            field!("RESO", PCT, filter.resonance),
            field!("ATTACK", log(0.5, 2000.0, Unit::Ms), adsr.attack_ms),
            field!("DECAY", log(5.0, 2000.0, Unit::Ms), adsr.decay_ms),
            field!("SUSTAIN", PCT, adsr.sustain),
            field!("RELEASE", log(5.0, 3000.0, Unit::Ms), adsr.release_ms),
        ],
        buttons: ["LP/HP/BP", "NOTCH", "PEAK", "OFF"],
    },
    Page {
        name: "VOIX",
        sections: ["VOIX", ""],
        params: [
            Some(Param {
                name: "FILTRE",
                scale: Scale::Choice(FILTER_NAMES),
                get: |s| index_of(&FILTER_KINDS, &s.filter.kind),
                set: |s, v| s.filter.kind = pick(&FILTER_KINDS, v),
            }),
            int_field!("POLY", steps(1.0, 32.0, Unit::Count), polyphony, u8),
            Some(Param {
                name: "CHOKE",
                scale: Scale::Choice(CHOKE_NAMES),
                get: |s| s.choke_group.map_or(0.0, |g| f32::from(g) + 1.0),
                set: |s, v| s.choke_group = (v.round() >= 1.0).then(|| v.round() as u8 - 1),
            }),
            None,
            None,
            None,
            None,
            None,
        ],
        buttons: ["REVERSE", "LOOP", "NORMLZ", "CYC FLT"],
    },
];

// ── TB-303 ACID ─────────────────────────────────────────────────────────

const ACID_WAVES: [AcidWaveform; 2] = [AcidWaveform::Saw, AcidWaveform::Square];

pub static ACID_PAGES: [Page<AcidParams>; 2] = [
    Page {
        name: "MAIN",
        sections: ["FILTRE", "CARACTÈRE & DELAY"],
        params: [
            field!("CUTOFF", log(20.0, 18000.0, Unit::Hz), cutoff_hz),
            field!("RESO", PCT, resonance),
            field!("ENV MOD", PCT, env_mod),
            field!("DECAY", log(30.0, 3000.0, Unit::Ms), decay_ms),
            field!("ACCENT", PCT, accent),
            field!("DRIVE", PCT, drive),
            field!("DLY TIME", log(20.0, 1000.0, Unit::Ms), delay_time_ms),
            field!("DLY MIX", PCT, delay_mix),
        ],
        buttons: ["CLASSIC", "SQUELCH", "DUB", "SAW/SQR"],
    },
    Page {
        name: "OSC & SORTIE",
        sections: ["OSCILLATEUR", "SORTIE"],
        params: [
            choice!("WAVE", &["SAW", "SQUARE"], ACID_WAVES, waveform),
            int_field!("OCTAVE", steps(-2.0, 2.0, Unit::Count), octave, i8),
            field!("TUNE", lin(-24.0, 24.0, Unit::Semitones), tuning_semitones),
            field!("SLIDE", lin(0.0, 250.0, Unit::Ms), slide_time_ms),
            field!("TONE", log(500.0, 16000.0, Unit::Hz), drive_tone),
            field!("DRY/WET", PCT, drive_mix),
            field!("FEEDBACK", lin(0.0, 0.95, Unit::Percent), delay_feedback),
            field!("VOLUME", VOLUME, volume_db),
        ],
        buttons: ["OCT -", "OCT +", "SLIDE", "SAW/SQR"],
    },
];

// ── KICK SYNTH ──────────────────────────────────────────────────────────

const KICK_WAVES: [KickWaveform; 7] = [
    KickWaveform::Sine,
    KickWaveform::Triangle,
    KickWaveform::SatSine,
    KickWaveform::SquareSub,
    KickWaveform::NoiseTransient,
    KickWaveform::Parabolic,
    KickWaveform::Ramp,
];
const KICK_DRIVES: [KickDriveType; 7] = [
    KickDriveType::AnalogTanh,
    KickDriveType::HardClip,
    KickDriveType::Wavefolder,
    KickDriveType::TubeDrive,
    KickDriveType::Bitcrush,
    KickDriveType::Fuzz,
    KickDriveType::Rectifier,
];
const KICK_FILTERS: [KickFilterMode; 6] = [
    KickFilterMode::Off,
    KickFilterMode::LowPass,
    KickFilterMode::HighPass,
    KickFilterMode::BandPass,
    KickFilterMode::Notch,
    KickFilterMode::Peak,
];

pub static KICK_PAGES: [Page<KickParams>; 3] = [
    Page {
        name: "CORPS",
        sections: ["CORPS", "IMPACT & SORTIE"],
        params: [
            field!("TUNE", log(20.0, 200.0, Unit::Hz), tune_hz),
            field!("DECAY", log(20.0, 2000.0, Unit::Ms), decay_ms),
            field!("PUNCH", PCT, snap),
            field!("SWEEP", log(5.0, 120.0, Unit::Ms), sweep_time_ms),
            field!("CLICK", PCT, click),
            field!("DRIVE", PCT, drive),
            field!("SUB", PCT, sub),
            field!("VOLUME", VOLUME, volume_db),
        ],
        buttons: ["TECHNO", "909", "HARDCORE", "808 SUB"],
    },
    Page {
        name: "TIMBRE",
        sections: ["SOURCE", "SATURATION & FILTRE"],
        params: [
            choice!(
                "WAVE",
                &["SINE", "TRIANGLE", "SAT SINE", "SQR SUB", "NOISE", "PARABOL", "RAMP"],
                KICK_WAVES,
                waveform
            ),
            field!("FM AMT", PCT, fm_amount),
            field!("FM RATIO", log(0.25, 8.0, Unit::Ratio), fm_ratio),
            field!("TONE", PCT, drive_tone),
            choice!(
                "DRV TYPE",
                &["TANH", "HARD", "FOLDER", "TUBE", "CRUSH", "FUZZ", "RECTIF"],
                KICK_DRIVES,
                drive_type
            ),
            choice!("FILTRE", FILTER_NAMES, KICK_FILTERS, filter_mode),
            field!("CUTOFF", log(20.0, 18000.0, Unit::Hz), filter_cutoff_hz),
            field!("RESO", lin(0.0, 0.95, Unit::Percent), filter_reso),
        ],
        buttons: ["LP", "HP", "BP", "OFF"],
    },
    Page {
        name: "DÉTAILS",
        sections: ["ACCORDAGE & SUB", "BRUIT"],
        params: [
            field!("FINE", lin(-12.0, 12.0, Unit::Semitones), tuning_semitones),
            field!("HOLD", lin(0.0, 60.0, Unit::Ms), hold_ms),
            field!("SUB RATIO", lin(0.5, 2.0, Unit::Ratio), sub_ratio),
            None,
            field!("NOISE", PCT, noise_mix),
            field!("NOISE DEC", log(2.0, 80.0, Unit::Ms), noise_decay_ms),
            None,
            None,
        ],
        buttons: ["NOTCH", "PEAK", "INDUSTRIAL", ""],
    },
];

// ── NEXUS WAVETABLE ─────────────────────────────────────────────────────

const WT_WAVES: [WavetableWaveform; 12] = [
    WavetableWaveform::Sine,
    WavetableWaveform::Triangle,
    WavetableWaveform::Saw,
    WavetableWaveform::Square,
    WavetableWaveform::Supersaw,
    WavetableWaveform::GrandPiano,
    WavetableWaveform::M1Piano,
    WavetableWaveform::RhodesTines,
    WavetableWaveform::DigiLead,
    WavetableWaveform::TonewheelOrgan,
    WavetableWaveform::VocalFormant,
    WavetableWaveform::Noise,
];
const WT_WAVE_NAMES: &[&str] = &[
    "SINE",
    "TRIANGLE",
    "SAW",
    "SQUARE",
    "SUPERSAW",
    "PIANO",
    "M1 PIANO",
    "RHODES",
    "DIGI LEAD",
    "ORGAN",
    "VOCAL",
    "NOISE",
];
const WT_FILTERS: [WavetableFilterMode; 7] = [
    WavetableFilterMode::Off,
    WavetableFilterMode::LowPass12,
    WavetableFilterMode::LowPass24,
    WavetableFilterMode::HighPass,
    WavetableFilterMode::BandPass,
    WavetableFilterMode::Notch,
    WavetableFilterMode::Peak,
];
const LFO_TARGETS: [LfoTarget; 4] = [
    LfoTarget::FilterCutoff,
    LfoTarget::PitchVibrato,
    LfoTarget::VolumeTremolo,
    LfoTarget::PanSpread,
];

pub static WAVETABLE_PAGES: [Page<WavetableParams>; 5] = [
    Page {
        name: "FILTRE & AMP",
        sections: ["FILTRE", "ENVELOPPE AMP"],
        params: [
            field!("CUTOFF", log(20.0, 20000.0, Unit::Hz), filter_cutoff),
            field!("RESO", lin(0.0, 0.98, Unit::Percent), filter_reso),
            field!("ENV AMT", lin(-1.0, 1.0, Unit::Percent), filter_env_amt),
            field!("DRIVE", PCT, filter_drive),
            field!("ATTACK", log(0.5, 4000.0, Unit::Ms), amp_attack_ms),
            field!("DECAY", log(5.0, 6000.0, Unit::Ms), amp_decay_ms),
            field!("SUSTAIN", PCT, amp_sustain),
            field!("RELEASE", log(5.0, 8000.0, Unit::Ms), amp_release_ms),
        ],
        buttons: ["PIANO", "M1 PIANO", "808 SUB", "REESE"],
    },
    Page {
        name: "OSCILLATEURS",
        sections: ["OSC 1", "OSC 2"],
        params: [
            choice!("WAVE 1", WT_WAVE_NAMES, WT_WAVES, osc1_wave),
            int_field!(
                "SEMI 1",
                steps(-24.0, 24.0, Unit::Count),
                osc1_semitones,
                i8
            ),
            int_field!("FINE 1", steps(-50.0, 50.0, Unit::Cents), osc1_cents, i8),
            field!("LEVEL 1", PCT, osc1_level),
            choice!("WAVE 2", WT_WAVE_NAMES, WT_WAVES, osc2_wave),
            int_field!(
                "SEMI 2",
                steps(-24.0, 24.0, Unit::Count),
                osc2_semitones,
                i8
            ),
            int_field!("FINE 2", steps(-50.0, 50.0, Unit::Cents), osc2_cents, i8),
            field!("LEVEL 2", PCT, osc2_level),
        ],
        buttons: ["OSC 2", "", "", ""],
    },
    Page {
        name: "UNISON & VOIX",
        sections: ["UNISON", "VOIX"],
        params: [
            int_field!("VOICES", steps(1.0, 7.0, Unit::Count), unison_voices, u8),
            field!("DETUNE", PCT, unison_detune),
            field!("SPREAD", PCT, unison_spread),
            field!("GLIDE", lin(0.0, 500.0, Unit::Ms), glide_time_ms),
            choice!(
                "FILTRE",
                &["OFF", "LP12", "LP24", "HP", "BP", "NOTCH", "PEAK"],
                WT_FILTERS,
                filter_mode
            ),
            field!("PAN 1", PAN, osc1_pan),
            field!("PAN 2", PAN, osc2_pan),
            field!("VOLUME", VOLUME, master_volume_db),
        ],
        buttons: ["", "", "", ""],
    },
    Page {
        name: "ENV FILTRE & LFO",
        sections: ["ENVELOPPE FILTRE", "LFO"],
        params: [
            field!("ATTACK", log(0.5, 4000.0, Unit::Ms), filter_attack_ms),
            field!("DECAY", log(5.0, 6000.0, Unit::Ms), filter_decay_ms),
            field!("SUSTAIN", PCT, filter_sustain),
            field!("RELEASE", log(5.0, 8000.0, Unit::Ms), filter_release_ms),
            field!("RATE", log(0.1, 30.0, Unit::Hz), lfo_rate_hz),
            field!("AMOUNT", PCT, lfo_amount),
            choice!(
                "DEST",
                &["CUTOFF", "VIBRATO", "TREMOLO", "PAN"],
                LFO_TARGETS,
                lfo_target
            ),
            None,
        ],
        buttons: ["", "", "", ""],
    },
    Page {
        name: "EFFETS",
        sections: ["COULEUR & ESPACE", "DELAY"],
        params: [
            field!("CHORUS", PCT, fx_chorus_mix),
            field!("DISTO", PCT, fx_distortion),
            field!("REVERB", PCT, fx_reverb_mix),
            field!("SIZE", lin(0.1, 1.0, Unit::Percent), fx_reverb_size),
            field!("DLY MIX", PCT, fx_delay_mix),
            field!("DLY TIME", log(20.0, 800.0, Unit::Ms), fx_delay_time_ms),
            field!("FEEDBACK", lin(0.0, 0.9, Unit::Percent), fx_delay_feedback),
            None,
        ],
        buttons: ["", "", "", ""],
    },
];

// ── EFFETS D'INSERT ─────────────────────────────────────────────────────
//
// Même disposition pour tous les effets : knob 1 = MIX (dry/wet), puis les
// réglages de l'effet ; boutons 5–8 = modes ou réglages rapides.

const FILTER_MODES: &[&str] = &["LP", "HP", "BP"];
const DRIVE_TYPES: &[&str] = &["SOFT", "HARD"];
const ON_OFF: &[&str] = &["OFF", "ON"];

/// Choix stocké dans un `f32` (index arrondi).
macro_rules! f32_choice {
    ($name:expr, $labels:expr, $f:ident) => {
        Some(Param {
            name: $name,
            scale: Scale::Choice($labels),
            get: |p| p.$f.round().clamp(0.0, ($labels.len() - 1) as f32),
            set: |p, v| p.$f = v.round(),
        })
    };
}

pub static FX_FILTER_PAGES: [Page<FxSlot>; 1] = [Page {
    name: "FILTER",
    sections: ["FILTRE", ""],
    params: [
        field!("MIX", PCT, mix),
        field!("CUTOFF", log(20.0, 20000.0, Unit::Hz), p1),
        field!("RESO", PCT, p2),
        f32_choice!("MODE", FILTER_MODES, p3),
        None,
        None,
        None,
        None,
    ],
    buttons: ["LP", "HP", "BP", ""],
}];

pub static FX_DISTORTION_PAGES: [Page<FxSlot>; 1] = [Page {
    name: "DISTORTION",
    sections: ["SATURATION", "SORTIE"],
    params: [
        field!("MIX", PCT, mix),
        field!("DRIVE", PCT, p1),
        field!("TONE", PCT, p2),
        f32_choice!("TYPE", DRIVE_TYPES, p3),
        field!("OUTPUT", lin(-24.0, 24.0, Unit::Db), p4),
        None,
        None,
        None,
    ],
    buttons: ["SOFT", "HARD", "", ""],
}];

pub static FX_COMPRESSOR_PAGES: [Page<FxSlot>; 1] = [Page {
    name: "COMPRESSOR",
    sections: ["DYNAMIQUE", "TEMPS"],
    params: [
        field!("MIX", PCT, mix),
        field!("THRESH", lin(-48.0, 0.0, Unit::Db), p1),
        field!("RATIO", lin(1.0, 20.0, Unit::Compression), p2),
        None,
        field!("ATTACK", log(0.1, 200.0, Unit::Ms), p3),
        field!("RELEASE", log(5.0, 2000.0, Unit::Ms), p4),
        None,
        None,
    ],
    buttons: ["DOUX", "PUNCH", "SQUASH", ""],
}];

pub static FX_DELAY_PAGES: [Page<FxSlot>; 1] = [Page {
    name: "DELAY",
    sections: ["ÉCHO", "STÉRÉO"],
    params: [
        field!("MIX", PCT, mix),
        field!("TIME", log(10.0, 2000.0, Unit::Ms), p1),
        field!("FEEDBACK", lin(0.0, 0.95, Unit::Percent), p2),
        field!("TONE", log(200.0, 18000.0, Unit::Hz), p3),
        f32_choice!("PING-PONG", ON_OFF, p4),
        None,
        None,
        None,
    ],
    buttons: ["PING-PONG", "1/16", "1/8", "1/4"],
}];

pub static FX_REVERB_PAGES: [Page<FxSlot>; 1] = [Page {
    name: "REVERB",
    sections: ["ESPACE", "STÉRÉO"],
    params: [
        field!("MIX", PCT, mix),
        field!("SIZE", PCT, p1),
        field!("DAMP", PCT, p2),
        field!("PRE-DLY", lin(0.0, 100.0, Unit::Ms), p3),
        field!("WIDTH", PCT, p4),
        None,
        None,
        None,
    ],
    buttons: ["PIÈCE", "SALLE", "HALL", ""],
}];

const EQ_DB: Scale = lin(-18.0, 18.0, Unit::Db);

pub static FX_EQ_PAGES: [Page<FxSlot>; 1] = [Page {
    name: "EQ",
    sections: ["ÉGALISEUR", "AIGUS"],
    params: [
        field!("MIX", PCT, mix),
        field!("LOW", EQ_DB, p1),
        field!("MID", EQ_DB, p2),
        field!("MID FREQ", log(80.0, 12_000.0, Unit::Hz), p3),
        field!("HIGH", EQ_DB, p4),
        None,
        None,
        None,
    ],
    buttons: ["FLAT", "LOUDNESS", "RADIO", ""],
}];

pub static FX_CHORUS_PAGES: [Page<FxSlot>; 1] = [Page {
    name: "CHORUS",
    sections: ["MODULATION", "RÉINJECTION"],
    params: [
        field!("MIX", PCT, mix),
        field!("RATE", log(0.05, 10.0, Unit::Hz), p1),
        field!("DEPTH", PCT, p2),
        field!("DELAY", log(0.5, 30.0, Unit::Ms), p3),
        field!("FEEDBACK", lin(0.0, 0.9, Unit::Percent), p4),
        None,
        None,
        None,
    ],
    buttons: ["CHORUS", "FLANGER", "ENSEMBLE", ""],
}];

pub static FX_LOFI_PAGES: [Page<FxSlot>; 1] = [Page {
    name: "LO-FI",
    sections: ["RÉSOLUTION", ""],
    params: [
        field!("MIX", PCT, mix),
        field!("BITS", steps(2.0, 16.0, Unit::Count), p1),
        field!("RATE", log(500.0, 48_000.0, Unit::Hz), p2),
        field!("TONE", log(200.0, 20_000.0, Unit::Hz), p3),
        None,
        None,
        None,
        None,
    ],
    buttons: ["8-BIT", "SP-1200", "CRUSH", ""],
}];

/// Pages de paramètres d'un effet.
#[must_use]
pub fn fx_pages(t: FxType) -> &'static [Page<FxSlot>] {
    match t {
        FxType::Filter => &FX_FILTER_PAGES,
        FxType::Distortion => &FX_DISTORTION_PAGES,
        FxType::Compressor => &FX_COMPRESSOR_PAGES,
        FxType::Delay => &FX_DELAY_PAGES,
        FxType::Reverb => &FX_REVERB_PAGES,
        FxType::Eq => &FX_EQ_PAGES,
        FxType::Chorus => &FX_CHORUS_PAGES,
        FxType::LoFi => &FX_LOFI_PAGES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chaque paramètre relu après écriture redonne une valeur dans ses
    /// bornes, et chaque page affiche ses 8 emplacements.
    fn check_pages<P: Clone>(pages: &[Page<P>], base: &P) {
        for (pi, page) in pages.iter().enumerate() {
            assert!(!page.name.is_empty());
            let v = view(pages, pi, base);
            for (k, param) in page.params.iter().enumerate() {
                let Some(param) = param else { continue };
                let (min, max) = param.scale.bounds();
                let mut p = base.clone();
                (param.set)(&mut p, max);
                let got = (param.get)(&p);
                assert!((got - max).abs() < 1.0, "{} max", param.name);
                (param.set)(&mut p, min);
                assert!(((param.get)(&p) - min).abs() < 1.0, "{} min", param.name);
                assert!(v.params[k]
                    .as_ref()
                    .is_some_and(|pv| (0.0..=1.0).contains(&pv.norm)));
            }
        }
    }

    #[test]
    fn every_plugin_parameter_reads_back_within_bounds() {
        check_pages(&SAMPLER_PAGES, &Sound::new("Test", 0));
        check_pages(&ACID_PAGES, &AcidParams::default());
        check_pages(&KICK_PAGES, &KickParams::default());
        check_pages(&WAVETABLE_PAGES, &WavetableParams::default());
        for t in FxType::ALL {
            check_pages(fx_pages(t), &FxSlot::default_for(t));
        }
    }

    #[test]
    fn knobs_turn_continuous_values_and_step_choices() {
        let mut k = KickParams::default();
        let before = k.tune_hz;
        assert!(turn(&KICK_PAGES, 0, 0, &mut k, 64, 0, false));
        assert!(k.tune_hz > before, "le knob 1 monte TUNE");
        // Choix : un cran = une forme d'onde suivante, jamais au-delà.
        k.waveform = KickWaveform::Sine;
        assert!(turn(&KICK_PAGES, 1, 0, &mut k, 0, 1, false));
        assert_eq!(k.waveform, KickWaveform::Triangle);
        for _ in 0..20 {
            turn(&KICK_PAGES, 1, 0, &mut k, 0, 1, false);
        }
        assert_eq!(k.waveform, KickWaveform::Ramp);
        // Emplacement vide : rien ne bouge.
        assert!(!turn(&KICK_PAGES, 2, 3, &mut k, 500, 5, false));
    }

    #[test]
    fn fine_mode_moves_ten_times_less() {
        let s = PCT;
        let coarse = s.turn(0.5, 100, false) - 0.5;
        let fine = s.turn(0.5, 100, true) - 0.5;
        assert!((coarse - fine * 10.0).abs() < 1e-4);
    }
}
