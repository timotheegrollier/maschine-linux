//! Lock-free atomic parameters for RT-safe parameter updates

use atomic_float::AtomicF32;
use std::sync::atomic::Ordering;

/// Parameter smoothing time constant (seconds)
const SMOOTHING_TIME: f32 = 0.02; // 20ms

/// Smoothed atomic parameter
pub struct SmoothParam {
    current: AtomicF32,
    target: AtomicF32,
    coeff: f32,
}

impl SmoothParam {
    /// Create new smoothed param with default value
    pub fn new(default: f32, sample_rate: f32) -> Self {
        let coeff = 1.0 - (-1.0 / (SMOOTHING_TIME * sample_rate)).exp();
        Self {
            current: AtomicF32::new(default),
            target: AtomicF32::new(default),
            coeff,
        }
    }

    /// Set target value (called from GUI thread)
    pub fn set_target(&self, value: f32) {
        self.target.store(value.clamp(0.0, 1.0), Ordering::Relaxed);
    }

    /// Get current smoothed value (called from audio thread)
    pub fn get(&self) -> f32 {
        let current = self.current.load(Ordering::Relaxed);
        let target = self.target.load(Ordering::Relaxed);
        let next = current + (target - current) * self.coeff;
        self.current.store(next, Ordering::Relaxed);
        next
    }

    /// Get current value without smoothing (for display)
    pub fn get_raw(&self) -> f32 {
        self.current.load(Ordering::Relaxed)
    }

    /// Reset to target immediately
    pub fn reset(&self) {
        let target = self.target.load(Ordering::Relaxed);
        self.current.store(target, Ordering::Relaxed);
    }
}

/// All guitar chain parameters (normalized 0.0..1.0)
pub struct GuitarParams {
    // Gate
    pub gate_threshold: SmoothParam, // -70..-20 dB
    pub gate_attack: SmoothParam,    // 0.1..10 ms
    pub gate_release: SmoothParam,   // 10..500 ms

    // Drive
    pub drive_gain: SmoothParam, // 0..36 dB
    pub drive_tone: SmoothParam, // 0..1 (lowpass cutoff)
    pub drive_mix: SmoothParam,  // 0..1 dry/wet

    // ToneStack (Baxandall-style 3-band)
    pub tone_bass: SmoothParam,   // -12..+12 dB
    pub tone_mid: SmoothParam,    // -12..+12 dB
    pub tone_treble: SmoothParam, // -12..+12 dB

    // CabSim IIR
    pub cab_enabled: SmoothParam,   // 0 or 1
    pub cab_highcut: SmoothParam,   // 2k..8k Hz
    pub cab_resonance: SmoothParam, // 0.5..5.0 Q

    // Delay (synced to BPM)
    pub delay_enabled: SmoothParam,  // 0 or 1
    pub delay_time_ms: SmoothParam,  // 50..2000 ms
    pub delay_feedback: SmoothParam, // 0..0.95
    pub delay_mix: SmoothParam,      // 0..1

    // Master
    pub master_volume: SmoothParam, // 0..1
    pub bypass: SmoothParam,        // 0 or 1
}

impl GuitarParams {
    /// Create all params with defaults at given sample rate
    pub fn new(sample_rate: f32) -> Self {
        Self {
            gate_threshold: SmoothParam::new(0.3, sample_rate), // ~-40dB
            gate_attack: SmoothParam::new(0.1, sample_rate),
            gate_release: SmoothParam::new(0.3, sample_rate),
            drive_gain: SmoothParam::new(0.4, sample_rate),
            drive_tone: SmoothParam::new(0.5, sample_rate),
            drive_mix: SmoothParam::new(1.0, sample_rate),
            tone_bass: SmoothParam::new(0.5, sample_rate),
            tone_mid: SmoothParam::new(0.5, sample_rate),
            tone_treble: SmoothParam::new(0.5, sample_rate),
            cab_enabled: SmoothParam::new(1.0, sample_rate),
            cab_highcut: SmoothParam::new(0.5, sample_rate),
            cab_resonance: SmoothParam::new(0.5, sample_rate),
            delay_enabled: SmoothParam::new(0.0, sample_rate),
            delay_time_ms: SmoothParam::new(0.3, sample_rate),
            delay_feedback: SmoothParam::new(0.3, sample_rate),
            delay_mix: SmoothParam::new(0.3, sample_rate),
            master_volume: SmoothParam::new(0.8, sample_rate),
            bypass: SmoothParam::new(0.0, sample_rate),
        }
    }

    /// Update sample rate for all smoothing coefficients
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let _coeff = 1.0 - (-1.0 / (SMOOTHING_TIME * sample_rate)).exp();
        // Can't easily update coeff in existing structs, so recreate or use new method
        // For now, params are created once at init with correct SR
    }
}

/// Parameter IDs for GUI/automation mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamId {
    GateThreshold,
    GateAttack,
    GateRelease,
    DriveGain,
    DriveTone,
    DriveMix,
    ToneBass,
    ToneMid,
    ToneTreble,
    CabEnabled,
    CabHighcut,
    CabResonance,
    DelayEnabled,
    DelayTimeMs,
    DelayFeedback,
    DelayMix,
    MasterVolume,
    Bypass,
}

impl ParamId {
    /// All parameter IDs in order
    pub fn all() -> &'static [ParamId] {
        &[
            ParamId::GateThreshold,
            ParamId::GateAttack,
            ParamId::GateRelease,
            ParamId::DriveGain,
            ParamId::DriveTone,
            ParamId::DriveMix,
            ParamId::ToneBass,
            ParamId::ToneMid,
            ParamId::ToneTreble,
            ParamId::CabEnabled,
            ParamId::CabHighcut,
            ParamId::CabResonance,
            ParamId::DelayEnabled,
            ParamId::DelayTimeMs,
            ParamId::DelayFeedback,
            ParamId::DelayMix,
            ParamId::MasterVolume,
            ParamId::Bypass,
        ]
    }

    /// Human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            ParamId::GateThreshold => "Gate Threshold",
            ParamId::GateAttack => "Gate Attack",
            ParamId::GateRelease => "Gate Release",
            ParamId::DriveGain => "Drive Gain",
            ParamId::DriveTone => "Drive Tone",
            ParamId::DriveMix => "Drive Mix",
            ParamId::ToneBass => "Tone Bass",
            ParamId::ToneMid => "Tone Mid",
            ParamId::ToneTreble => "Tone Treble",
            ParamId::CabEnabled => "Cab Enabled",
            ParamId::CabHighcut => "Cab Highcut",
            ParamId::CabResonance => "Cab Resonance",
            ParamId::DelayEnabled => "Delay Enabled",
            ParamId::DelayTimeMs => "Delay Time",
            ParamId::DelayFeedback => "Delay Feedback",
            ParamId::DelayMix => "Delay Mix",
            ParamId::MasterVolume => "Master Volume",
            ParamId::Bypass => "Bypass",
        }
    }

    /// Parameter range (min, max, default)
    pub fn range(&self) -> (f32, f32, f32) {
        match self {
            ParamId::GateThreshold => (-70.0, -20.0, -40.0),
            ParamId::GateAttack => (0.1, 10.0, 1.0),
            ParamId::GateRelease => (10.0, 500.0, 50.0),
            ParamId::DriveGain => (0.0, 36.0, 12.0),
            ParamId::DriveTone => (200.0, 8000.0, 2000.0),
            ParamId::DriveMix => (0.0, 1.0, 1.0),
            ParamId::ToneBass => (-12.0, 12.0, 0.0),
            ParamId::ToneMid => (-12.0, 12.0, 0.0),
            ParamId::ToneTreble => (-12.0, 12.0, 0.0),
            ParamId::CabEnabled => (0.0, 1.0, 1.0),
            ParamId::CabHighcut => (2000.0, 8000.0, 5000.0),
            ParamId::CabResonance => (0.5, 5.0, 1.5),
            ParamId::DelayEnabled => (0.0, 1.0, 0.0),
            ParamId::DelayTimeMs => (50.0, 2000.0, 300.0),
            ParamId::DelayFeedback => (0.0, 0.95, 0.3),
            ParamId::DelayMix => (0.0, 1.0, 0.3),
            ParamId::MasterVolume => (0.0, 1.0, 0.8),
            ParamId::Bypass => (0.0, 1.0, 0.0),
        }
    }

    /// Get reference to parameter in GuitarParams
    pub fn get_param<'a>(&self, params: &'a GuitarParams) -> &'a SmoothParam {
        match self {
            ParamId::GateThreshold => &params.gate_threshold,
            ParamId::GateAttack => &params.gate_attack,
            ParamId::GateRelease => &params.gate_release,
            ParamId::DriveGain => &params.drive_gain,
            ParamId::DriveTone => &params.drive_tone,
            ParamId::DriveMix => &params.drive_mix,
            ParamId::ToneBass => &params.tone_bass,
            ParamId::ToneMid => &params.tone_mid,
            ParamId::ToneTreble => &params.tone_treble,
            ParamId::CabEnabled => &params.cab_enabled,
            ParamId::CabHighcut => &params.cab_highcut,
            ParamId::CabResonance => &params.cab_resonance,
            ParamId::DelayEnabled => &params.delay_enabled,
            ParamId::DelayTimeMs => &params.delay_time_ms,
            ParamId::DelayFeedback => &params.delay_feedback,
            ParamId::DelayMix => &params.delay_mix,
            ParamId::MasterVolume => &params.master_volume,
            ParamId::Bypass => &params.bypass,
        }
    }

    /// Get mutable reference to parameter in GuitarParams
    pub fn get_param_mut<'a>(&self, params: &'a mut GuitarParams) -> &'a mut SmoothParam {
        match self {
            ParamId::GateThreshold => &mut params.gate_threshold,
            ParamId::GateAttack => &mut params.gate_attack,
            ParamId::GateRelease => &mut params.gate_release,
            ParamId::DriveGain => &mut params.drive_gain,
            ParamId::DriveTone => &mut params.drive_tone,
            ParamId::DriveMix => &mut params.drive_mix,
            ParamId::ToneBass => &mut params.tone_bass,
            ParamId::ToneMid => &mut params.tone_mid,
            ParamId::ToneTreble => &mut params.tone_treble,
            ParamId::CabEnabled => &mut params.cab_enabled,
            ParamId::CabHighcut => &mut params.cab_highcut,
            ParamId::CabResonance => &mut params.cab_resonance,
            ParamId::DelayEnabled => &mut params.delay_enabled,
            ParamId::DelayTimeMs => &mut params.delay_time_ms,
            ParamId::DelayFeedback => &mut params.delay_feedback,
            ParamId::DelayMix => &mut params.delay_mix,
            ParamId::MasterVolume => &mut params.master_volume,
            ParamId::Bypass => &mut params.bypass,
        }
    }

    /// Get unit string for display
    pub fn unit(&self) -> &'static str {
        match self {
            ParamId::GateThreshold => "dB",
            ParamId::GateAttack => "ms",
            ParamId::GateRelease => "ms",
            ParamId::DriveGain => "dB",
            ParamId::DriveTone => "Hz",
            ParamId::DriveMix => "%",
            ParamId::ToneBass => "dB",
            ParamId::ToneMid => "dB",
            ParamId::ToneTreble => "dB",
            ParamId::CabEnabled => "",
            ParamId::CabHighcut => "Hz",
            ParamId::CabResonance => "Q",
            ParamId::DelayEnabled => "",
            ParamId::DelayTimeMs => "ms",
            ParamId::DelayFeedback => "%",
            ParamId::DelayMix => "%",
            ParamId::MasterVolume => "%",
            ParamId::Bypass => "",
        }
    }
}
