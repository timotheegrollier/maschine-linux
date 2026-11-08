//! `maschine-engine` — domaine, transport, sequencer, sampler.

#![allow(missing_docs)]

pub mod acid_synth;
pub mod display_render;
pub mod drum_synth;
pub mod engine;
pub mod fx;
pub mod keyboard;
pub mod kick_synth;
pub mod project;
pub mod sampler;
pub mod sequencer;
pub mod transport;
pub mod wavetable_synth;

pub use acid_synth::{AcidParams, AcidSynth, AcidWaveform};
pub use drum_synth::generate_complete_808_kit;
pub use engine::{Engine, EngineEvent, EngineSnapshot, ProjectFileEntry, DRUM_PRESET_NAMES};
pub use fx::{FxChainProcessor, FxSlot, FxType};
pub use keyboard::KeyboardState;
pub use kick_synth::{KickDriveType, KickFilterMode, KickParams, KickSynth, KickWaveform};
pub use project::{Group, Pattern, PatternEvent, Project, Scene, Section, Sound, Zone};
pub use sampler::{sound_id, Sampler, SamplerVoice, SoundId, VoiceParams, ZoneMapping};
pub use sequencer::{Grid, Sequencer};
pub use transport::{Transport, TransportState};
pub use wavetable_synth::{
    LfoTarget, PresetCategory, WavetableFilterMode, WavetableParams, WavetableSynth,
    WavetableWaveform,
};

/// Erreurs engine.
#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    /// I/O
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),

    /// JSON
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// Validation
    #[error("validation: {0}")]
    Validation(String),

    /// Sampler
    #[error("sampler: {0}")]
    Sampler(String),

    /// Autre
    #[error("{0}")]
    Other(String),
}

/// Résultat engine.
pub type Result<T> = std::result::Result<T, EngineError>;

/// Version schéma projet.
pub const SCHEMA_VERSION: u32 = 1;
