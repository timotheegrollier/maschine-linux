//! `maschine-engine` — domaine, transport, sequencer, sampler.

#![allow(missing_docs)]

pub mod engine;
pub mod project;
pub mod sampler;
pub mod sequencer;
pub mod transport;

pub use engine::{Engine, EngineEvent, EngineSnapshot};
pub use project::{Group, Pattern, PatternEvent, Project, Scene, Section, Sound, Zone};
pub use sampler::{Sampler, SamplerVoice, ZoneMapping};
pub use sequencer::{Grid, Sequencer};
pub use transport::{Transport, TransportState};

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
