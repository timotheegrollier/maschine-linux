//! Real-time guitar DSP effects chain
//!
//! Chain: Gate -> tube preamp (multi-model) -> CabSim -> Delay
//! All parameters are lock-free atomics with block-wise smoothing for RT safety.

#![allow(missing_docs)]

pub mod amp;
pub mod analysis;
pub mod biquad;
pub mod cab;
pub mod chain;
pub mod delay;
pub mod diagnostics;
pub mod drive;
pub mod gate;
pub mod params;
pub mod tonestack;
pub mod utils;

pub use amp::{AmpModel, AmpSettings, TubeAmp};
pub use analysis::{frequency_to_note, ClipDetector, ClipStatus, Tuner, TunerReading};
pub use cab::{CabModel, CabSim};
pub use chain::{AudioContext, GuitarChain};
pub use diagnostics::{RtDiagnostics, RtStats};
pub use params::{GuitarParams, ParamId};

#[cfg(feature = "analysis")]
pub use analysis::SpectrumAnalyser;
