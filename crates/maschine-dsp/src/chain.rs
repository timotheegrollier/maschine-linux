//! Complete guitar effects chain: Gate -> Drive -> ToneStack -> Cab -> Delay

use crate::{
    cab::CabSim,
    delay::Delay,
    drive::Overdrive,
    gate::NoiseGate,
    params::{GuitarParams, ParamId, SmoothParam},
    tonestack::ToneStack,
    utils::PeakMeter,
};

/// Audio context passed to process() for time-varying params
pub struct AudioContext {
    pub sample_rate: f32,
    pub bpm: f32,
    pub frame_count: u64,
}

/// Main guitar effects chain
pub struct GuitarChain {
    gate: NoiseGate,
    drive: Overdrive,
    tonestack: ToneStack,
    cab: CabSim,
    delay: Delay,
    params: GuitarParams,
    sample_rate: f32,
    // Meters
    input_meter: PeakMeter,
    output_meter: PeakMeter,
    // Bypass state
    bypassed: bool,
}

impl GuitarChain {
    /// Create new chain at sample rate
    pub fn new(sample_rate: f32) -> Self {
        let max_delay_ms = 2000.0;
        Self {
            gate: NoiseGate::new(sample_rate),
            drive: Overdrive::new(sample_rate),
            tonestack: ToneStack::new(sample_rate),
            cab: CabSim::new(sample_rate),
            delay: Delay::new(sample_rate, max_delay_ms),
            params: GuitarParams::new(sample_rate),
            sample_rate,
            input_meter: PeakMeter::new(sample_rate),
            output_meter: PeakMeter::new(sample_rate),
            bypassed: false,
        }
    }

    /// Get mutable reference to parameters for GUI updates
    pub fn params_mut(&mut self) -> &mut GuitarParams {
        &mut self.params
    }

    /// Get parameter by ID for GUI binding
    pub fn get_param(&self, id: ParamId) -> &SmoothParam {
        id.get_param(&self.params)
    }

    /// Get mutable parameter by ID
    pub fn get_param_mut(&mut self, id: ParamId) -> &mut SmoothParam {
        id.get_param_mut(&mut self.params)
    }

    /// Update all smoothed params and effect coefficients (call once per block)
    fn update_coefficients(&mut self) {
        let p = &self.params;

        // Gate
        let _gate_thresh = ParamId::GateThreshold.range().0
            + (ParamId::GateThreshold.range().1 - ParamId::GateThreshold.range().0)
                * p.gate_threshold.get_raw();
        let gate_attack = ParamId::GateAttack.range().0
            + (ParamId::GateAttack.range().1 - ParamId::GateAttack.range().0)
                * p.gate_attack.get_raw();
        let gate_release = ParamId::GateRelease.range().0
            + (ParamId::GateRelease.range().1 - ParamId::GateRelease.range().0)
                * p.gate_release.get_raw();
        self.gate.update_params(gate_attack, gate_release);

        // Drive
        let _drive_gain = ParamId::DriveGain.range().0
            + (ParamId::DriveGain.range().1 - ParamId::DriveGain.range().0)
                * p.drive_gain.get_raw();
        let drive_tone = ParamId::DriveTone.range().0
            + (ParamId::DriveTone.range().1 - ParamId::DriveTone.range().0)
                * p.drive_tone.get_raw();
        let _drive_mix = p.drive_mix.get_raw();
        self.drive.set_tone(drive_tone);

        // ToneStack
        let bass_db = ParamId::ToneBass.range().0
            + (ParamId::ToneBass.range().1 - ParamId::ToneBass.range().0) * p.tone_bass.get_raw();
        let mid_db = ParamId::ToneMid.range().0
            + (ParamId::ToneMid.range().1 - ParamId::ToneMid.range().0) * p.tone_mid.get_raw();
        let treble_db = ParamId::ToneTreble.range().0
            + (ParamId::ToneTreble.range().1 - ParamId::ToneTreble.range().0)
                * p.tone_treble.get_raw();
        self.tonestack.set_gains(bass_db, mid_db, treble_db);

        // CabSim
        let cab_enabled = p.cab_enabled.get_raw() > 0.5;
        let cab_highcut = ParamId::CabHighcut.range().0
            + (ParamId::CabHighcut.range().1 - ParamId::CabHighcut.range().0)
                * p.cab_highcut.get_raw();
        let cab_res = ParamId::CabResonance.range().0
            + (ParamId::CabResonance.range().1 - ParamId::CabResonance.range().0)
                * p.cab_resonance.get_raw();
        self.cab.set_enabled(cab_enabled);
        self.cab.set_params(cab_highcut, cab_res);

        // Delay
        let _delay_enabled = p.delay_enabled.get_raw() > 0.5;
        // Delay processed per-sample with current params

        // Master
        let bypass = p.bypass.get_raw() > 0.5;
        self.bypassed = bypass;
    }

    /// Process one block of audio (mono in, mono out)
    /// Returns output level for metering
    pub fn process(&mut self, input: &[f32], output: &mut [f32], _ctx: &AudioContext) {
        // Update coefficients from smoothed params
        self.update_coefficients();

        // Meter input
        for &s in input {
            self.input_meter.update(s);
        }

        if self.bypassed {
            output.copy_from_slice(input);
        } else {
            // Two buffers for ping-pong processing to avoid borrow conflicts
            let mut buf1 = vec![0.0f32; input.len()];
            let mut buf2 = vec![0.0f32; input.len()];

            // Gate: input -> buf1
            let gate_thresh = ParamId::GateThreshold.range().0
                + (ParamId::GateThreshold.range().1 - ParamId::GateThreshold.range().0)
                    * self.params.gate_threshold.get_raw();
            self.gate.process_block(input, &mut buf1, gate_thresh);

            // Drive: buf1 -> buf2
            let drive_gain = ParamId::DriveGain.range().0
                + (ParamId::DriveGain.range().1 - ParamId::DriveGain.range().0)
                    * self.params.drive_gain.get_raw();
            let drive_mix = self.params.drive_mix.get_raw();
            self.drive
                .process_block(&buf1, &mut buf2, drive_gain, drive_mix);

            // ToneStack: buf2 -> buf1
            self.tonestack.process_block(&buf2, &mut buf1);

            // CabSim: buf1 -> buf2
            self.cab.process_block(&buf1, &mut buf2);

            // Delay (if enabled): buf2 -> buf1
            let delay_enabled = self.params.delay_enabled.get_raw() > 0.5;
            if delay_enabled {
                let delay_ms = ParamId::DelayTimeMs.range().0
                    + (ParamId::DelayTimeMs.range().1 - ParamId::DelayTimeMs.range().0)
                        * self.params.delay_time_ms.get_raw();
                let delay_fb = ParamId::DelayFeedback.range().0
                    + (ParamId::DelayFeedback.range().1 - ParamId::DelayFeedback.range().0)
                        * self.params.delay_feedback.get_raw();
                let delay_mix = self.params.delay_mix.get_raw();
                self.delay
                    .process_block(&buf2, &mut buf1, delay_ms, delay_fb, delay_mix);
            } else {
                buf1.copy_from_slice(&buf2);
            }

            // Master volume: buf1 -> output
            let master_vol = self.params.master_volume.get_raw();
            for (out_sample, &in_sample) in output.iter_mut().zip(buf1.iter()) {
                *out_sample = in_sample * master_vol;
            }
        }

        // Meter output
        for s in output.iter() {
            self.output_meter.update(*s);
        }
    }

    /// Process stereo (mono in -> stereo out with slight stereo spread on delay)
    pub fn process_stereo(
        &mut self,
        input: &[f32],
        output_l: &mut [f32],
        output_r: &mut [f32],
        ctx: &AudioContext,
    ) {
        // For now, just duplicate mono to stereo
        self.process(input, output_l, ctx);
        output_r.copy_from_slice(output_l);
    }

    /// Get input level in dBFS
    pub fn input_level_db(&self) -> f32 {
        self.input_meter.get_db()
    }

    /// Get output level in dBFS
    pub fn output_level_db(&self) -> f32 {
        self.output_meter.get_db()
    }

    /// Get bypass state
    pub fn is_bypassed(&self) -> bool {
        self.bypassed
    }

    /// Set bypass state
    pub fn set_bypass(&mut self, bypass: bool) {
        self.bypassed = bypass;
        self.params
            .bypass
            .set_target(if bypass { 1.0 } else { 0.0 });
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Reset all internal state
    pub fn reset(&mut self) {
        self.gate = NoiseGate::new(self.sample_rate);
        self.drive = Overdrive::new(self.sample_rate);
        self.tonestack = ToneStack::new(self.sample_rate);
        self.cab = CabSim::new(self.sample_rate);
        self.delay.clear();
        self.input_meter.reset();
        self.output_meter.reset();
    }
}
