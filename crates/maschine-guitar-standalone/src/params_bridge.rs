//! Bridge between GUI thread and audio thread for parameter updates

use atomic_float::AtomicF32;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use maschine_dsp::{GuitarChain, ParamId};

/// Thread-safe parameter bridge
pub struct ParamsBridge {
    /// Atomic parameters for lock-free access from audio thread
    pub params: Arc<AtomicParams>,
    /// Reference to chain for direct access (GUI thread only)
    chain: Mutex<Option<Arc<Mutex<GuitarChain>>>>,
    /// Pending parameter updates from GUI
    pending: Mutex<Vec<(ParamId, f32)>>,
    /// Bypass state
    bypass: AtomicBool,
}

/// Raw atomic parameters matching GuitarParams layout
pub struct AtomicParams {
    // Gate
    pub gate_threshold: AtomicF32,
    pub gate_attack: AtomicF32,
    pub gate_release: AtomicF32,
    // Drive
    pub drive_gain: AtomicF32,
    pub drive_tone: AtomicF32,
    pub drive_mix: AtomicF32,
    // ToneStack
    pub tone_bass: AtomicF32,
    pub tone_mid: AtomicF32,
    pub tone_treble: AtomicF32,
    // CabSim
    pub cab_enabled: AtomicF32,
    pub cab_highcut: AtomicF32,
    pub cab_resonance: AtomicF32,
    // Delay
    pub delay_enabled: AtomicF32,
    pub delay_time_ms: AtomicF32,
    pub delay_feedback: AtomicF32,
    pub delay_mix: AtomicF32,
    // Master
    pub master_volume: AtomicF32,
    pub bypass: AtomicF32,
}

impl AtomicParams {
    fn new() -> Self {
        Self {
            gate_threshold: AtomicF32::new(0.3),
            gate_attack: AtomicF32::new(0.1),
            gate_release: AtomicF32::new(0.3),
            drive_gain: AtomicF32::new(0.4),
            drive_tone: AtomicF32::new(0.5),
            drive_mix: AtomicF32::new(1.0),
            tone_bass: AtomicF32::new(0.5),
            tone_mid: AtomicF32::new(0.5),
            tone_treble: AtomicF32::new(0.5),
            cab_enabled: AtomicF32::new(1.0),
            cab_highcut: AtomicF32::new(0.5),
            cab_resonance: AtomicF32::new(0.5),
            delay_enabled: AtomicF32::new(0.0),
            delay_time_ms: AtomicF32::new(0.3),
            delay_feedback: AtomicF32::new(0.3),
            delay_mix: AtomicF32::new(0.3),
            master_volume: AtomicF32::new(0.8),
            bypass: AtomicF32::new(0.0),
        }
    }
}

impl Default for AtomicParams {
    fn default() -> Self {
        Self::new()
    }
}

impl ParamsBridge {
    pub fn new(chain: Arc<Mutex<GuitarChain>>) -> Self {
        Self {
            params: Arc::new(AtomicParams::new()),
            chain: Mutex::new(Some(chain)),
            pending: Mutex::new(Vec::new()),
            bypass: AtomicBool::new(false),
        }
    }

    /// Called from GUI thread to set a parameter
    pub fn set_param(&self, id: ParamId, normalized_value: f32) {
        self.pending
            .lock()
            .push((id, normalized_value.clamp(0.0, 1.0)));
    }

    /// Called from GUI thread to set bypass
    pub fn set_bypass(&self, bypass: bool) {
        self.bypass.store(bypass, Ordering::Relaxed);
        self.params
            .bypass
            .store(if bypass { 1.0 } else { 0.0 }, Ordering::Relaxed);
    }

    /// Called from audio thread to apply pending updates
    pub fn apply_pending(&self) {
        let mut pending = self.pending.lock();
        for (id, value) in pending.drain(..) {
            self.apply_atomic(id, value);
        }
    }

    fn apply_atomic(&self, id: ParamId, value: f32) {
        match id {
            ParamId::GateThreshold => self.params.gate_threshold.store(value, Ordering::Relaxed),
            ParamId::GateAttack => self.params.gate_attack.store(value, Ordering::Relaxed),
            ParamId::GateRelease => self.params.gate_release.store(value, Ordering::Relaxed),
            ParamId::DriveGain => self.params.drive_gain.store(value, Ordering::Relaxed),
            ParamId::DriveTone => self.params.drive_tone.store(value, Ordering::Relaxed),
            ParamId::DriveMix => self.params.drive_mix.store(value, Ordering::Relaxed),
            ParamId::ToneBass => self.params.tone_bass.store(value, Ordering::Relaxed),
            ParamId::ToneMid => self.params.tone_mid.store(value, Ordering::Relaxed),
            ParamId::ToneTreble => self.params.tone_treble.store(value, Ordering::Relaxed),
            ParamId::CabEnabled => self.params.cab_enabled.store(value, Ordering::Relaxed),
            ParamId::CabHighcut => self.params.cab_highcut.store(value, Ordering::Relaxed),
            ParamId::CabResonance => self.params.cab_resonance.store(value, Ordering::Relaxed),
            ParamId::DelayEnabled => self.params.delay_enabled.store(value, Ordering::Relaxed),
            ParamId::DelayTimeMs => self.params.delay_time_ms.store(value, Ordering::Relaxed),
            ParamId::DelayFeedback => self.params.delay_feedback.store(value, Ordering::Relaxed),
            ParamId::DelayMix => self.params.delay_mix.store(value, Ordering::Relaxed),
            ParamId::MasterVolume => self.params.master_volume.store(value, Ordering::Relaxed),
            ParamId::Bypass => self.params.bypass.store(value, Ordering::Relaxed),
        }
    }

    /// Get current parameter value (for GUI display)
    pub fn get_param(&self, id: ParamId) -> f32 {
        match id {
            ParamId::GateThreshold => self.params.gate_threshold.load(Ordering::Relaxed),
            ParamId::GateAttack => self.params.gate_attack.load(Ordering::Relaxed),
            ParamId::GateRelease => self.params.gate_release.load(Ordering::Relaxed),
            ParamId::DriveGain => self.params.drive_gain.load(Ordering::Relaxed),
            ParamId::DriveTone => self.params.drive_tone.load(Ordering::Relaxed),
            ParamId::DriveMix => self.params.drive_mix.load(Ordering::Relaxed),
            ParamId::ToneBass => self.params.tone_bass.load(Ordering::Relaxed),
            ParamId::ToneMid => self.params.tone_mid.load(Ordering::Relaxed),
            ParamId::ToneTreble => self.params.tone_treble.load(Ordering::Relaxed),
            ParamId::CabEnabled => self.params.cab_enabled.load(Ordering::Relaxed),
            ParamId::CabHighcut => self.params.cab_highcut.load(Ordering::Relaxed),
            ParamId::CabResonance => self.params.cab_resonance.load(Ordering::Relaxed),
            ParamId::DelayEnabled => self.params.delay_enabled.load(Ordering::Relaxed),
            ParamId::DelayTimeMs => self.params.delay_time_ms.load(Ordering::Relaxed),
            ParamId::DelayFeedback => self.params.delay_feedback.load(Ordering::Relaxed),
            ParamId::DelayMix => self.params.delay_mix.load(Ordering::Relaxed),
            ParamId::MasterVolume => self.params.master_volume.load(Ordering::Relaxed),
            ParamId::Bypass => self.params.bypass.load(Ordering::Relaxed),
        }
    }

    /// Get atomic params reference for audio thread
    pub fn atomic_params(&self) -> Arc<AtomicParams> {
        self.params.clone()
    }

    /// Get bypass state
    pub fn is_bypassed(&self) -> bool {
        self.bypass.load(Ordering::Relaxed)
    }

    /// Take chain reference (once, during audio thread init)
    pub fn take_chain(&self) -> Option<Arc<Mutex<GuitarChain>>> {
        self.chain.lock().take()
    }
}

/// GUI-friendly parameter descriptor
#[derive(Debug, Clone)]
pub struct GuiParam {
    pub id: ParamId,
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
}

impl GuiParam {
    pub fn all() -> Vec<Self> {
        ParamId::all()
            .iter()
            .map(|id| {
                let (min, max, default) = id.range();
                Self {
                    id: *id,
                    name: id.name(),
                    min,
                    max,
                    default,
                    unit: id.unit(),
                }
            })
            .collect()
    }
}
