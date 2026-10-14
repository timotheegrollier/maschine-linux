//! Real-time guitar DSP effects chain
//!
//! Chain: Gate -> Drive -> ToneStack -> CabSim(IIR) -> Delay
//! All params are lock-free AtomicF32 for RT safety.

#![allow(missing_docs)]

pub mod cab;
pub mod chain;
pub mod delay;
pub mod drive;
pub mod gate;
pub mod params;
pub mod tonestack;
pub mod utils;

pub use chain::{AudioContext, GuitarChain};
pub use params::{GuitarParams, ParamId};
