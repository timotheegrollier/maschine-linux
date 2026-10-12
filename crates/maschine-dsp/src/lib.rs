//! Real-time guitar DSP effects chain
//!
//! Chain: Gate -> Drive -> ToneStack -> CabSim(IIR) -> Delay
//! All params are lock-free AtomicF32 for RT safety.

#![allow(missing_docs)]

pub mod chain;
pub mod drive;
pub mod gate;
pub mod tonestack;
pub mod cab;
pub mod delay;
pub mod params;
pub mod utils;

pub use chain::{GuitarChain, AudioContext};
pub use params::{GuitarParams, ParamId};