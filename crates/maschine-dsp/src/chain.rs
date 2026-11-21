//! Complete guitar signal chain: Gate -> tube preamp -> cabinet -> delay.

use crate::{
    amp::{AmpModel, AmpSettings, TubeAmp},
    cab::{CabModel, CabSim},
    delay::Delay,
    gate::NoiseGate,
    params::{GuitarParams, ParamId, SmoothParam},
    utils::PeakMeter,
};

/// Audio context passed to process() for time-varying params.
pub struct AudioContext {
    pub sample_rate: f32,
    pub bpm: f32,
    pub frame_count: u64,
}

/// Main guitar effects chain.
pub struct GuitarChain {
    gate: NoiseGate,
    amp: TubeAmp,
    cab: CabSim,
    delay: Delay,
    params: GuitarParams,
    sample_rate: f32,
    // Preallocated scratch buffers (never allocated in the audio callback).
    buf1: Vec<f32>,
    buf2: Vec<f32>,
    // Meters
    input_meter: PeakMeter,
    output_meter: PeakMeter,
    // Bypass state
    bypassed: bool,
    /// When true, the host drives amp settings directly and the legacy
    /// Drive/Tone parameters must not overwrite them each block.
    external_amp_control: bool,
}

impl GuitarChain {
    /// Create new chain at sample rate.
    pub fn new(sample_rate: f32) -> Self {
        let max_delay_ms = 2000.0;
        Self {
            gate: NoiseGate::new(sample_rate),
            amp: TubeAmp::new(AmpModel::VoxAc30, sample_rate),
            cab: CabSim::new(sample_rate),
            delay: Delay::new(sample_rate, max_delay_ms),
            params: GuitarParams::new(sample_rate),
            sample_rate,
            buf1: Vec::new(),
            buf2: Vec::new(),
            input_meter: PeakMeter::new(sample_rate),
            output_meter: PeakMeter::new(sample_rate),
            bypassed: false,
            external_amp_control: false,
        }
    }

    /// Get mutable reference to parameters for GUI updates.
    pub fn params_mut(&mut self) -> &mut GuitarParams {
        &mut self.params
    }

    /// Get parameter by ID for GUI binding.
    pub fn get_param(&self, id: ParamId) -> &SmoothParam {
        id.get_param(&self.params)
    }

    /// Get mutable parameter by ID.
    pub fn get_param_mut(&mut self, id: ParamId) -> &mut SmoothParam {
        id.get_param_mut(&mut self.params)
    }

    // ---- Amp controls -------------------------------------------------

    pub fn amp(&self) -> &TubeAmp {
        &self.amp
    }

    pub fn amp_mut(&mut self) -> &mut TubeAmp {
        &mut self.amp
    }

    pub fn amp_model(&self) -> AmpModel {
        self.amp.model()
    }

    pub fn set_amp_model(&mut self, model: AmpModel) {
        self.amp.set_model(model);
        self.cab.set_model(match model {
            AmpModel::VoxAc30 => CabModel::Vox,
            AmpModel::FenderBlackface | AmpModel::FenderTweed => CabModel::Fender,
            AmpModel::HighGain => CabModel::Modern,
        });
    }

    pub fn amp_settings(&self) -> &AmpSettings {
        self.amp.settings()
    }

    pub fn set_amp_settings(&mut self, settings: AmpSettings) {
        self.amp.set_settings(settings);
    }

    /// Let an external host own the amp parameters (the legacy Drive/Tone
    /// params are then ignored). Used by the integrated MK3 guitar view.
    pub fn set_external_amp_control(&mut self, enabled: bool) {
        self.external_amp_control = enabled;
    }

    pub fn set_cab_model(&mut self, model: CabModel) {
        self.cab.set_model(model);
    }

    // ---- Processing ---------------------------------------------------

    /// Update all smoothed params and effect coefficients once per block.
    fn update_coefficients(&mut self, block_len: usize) {
        let p = &self.params;
        let n = block_len.max(1);

        // Gate
        let gate_attack = ParamId::GateAttack.range().0
            + (ParamId::GateAttack.range().1 - ParamId::GateAttack.range().0)
                * p.gate_attack.get_block(n);
        let gate_release = ParamId::GateRelease.range().0
            + (ParamId::GateRelease.range().1 - ParamId::GateRelease.range().0)
                * p.gate_release.get_block(n);
        self.gate.update_params(gate_attack, gate_release);

        // Amp: legacy Drive/Tone params feed the amp only when no host is
        // driving the amp settings directly (standalone rack).
        if !self.external_amp_control {
            let gain = p.drive_gain.get_block(n);
            let tone_bass = p.tone_bass.get_block(n);
            let tone_mid = p.tone_mid.get_block(n);
            let tone_treble = p.tone_treble.get_block(n);
            let mut settings = *self.amp.settings();
            settings.gain = gain;
            settings.bass = tone_bass;
            settings.mid = tone_mid;
            settings.treble = tone_treble;
            self.amp.set_settings(settings);
        }

        // Cabinet
        let cab_enabled = p.cab_enabled.get_block(n) > 0.5;
        let cab_highcut = ParamId::CabHighcut.range().0
            + (ParamId::CabHighcut.range().1 - ParamId::CabHighcut.range().0)
                * p.cab_highcut.get_block(n);
        let cab_res = ParamId::CabResonance.range().0
            + (ParamId::CabResonance.range().1 - ParamId::CabResonance.range().0)
                * p.cab_resonance.get_block(n);
        self.cab.set_enabled(cab_enabled);
        self.cab.set_params(cab_highcut, cab_res);

        self.bypassed = p.bypass.get_block(n) > 0.5;
    }

    fn ensure_buffers(&mut self, len: usize) {
        if self.buf1.len() < len {
            self.buf1.resize(len, 0.0);
            self.buf2.resize(len, 0.0);
        }
    }

    /// Process one block of audio (mono in, mono out).
    pub fn process(&mut self, input: &[f32], output: &mut [f32], _ctx: &AudioContext) {
        self.ensure_buffers(input.len().max(output.len()));
        self.update_coefficients(input.len());

        for &s in input {
            self.input_meter.update(s);
        }

        if self.bypassed {
            output.copy_from_slice(input);
        } else {
            let len = input.len().min(output.len());
            let gate_thresh = ParamId::GateThreshold.range().0
                + (ParamId::GateThreshold.range().1 - ParamId::GateThreshold.range().0)
                    * self.params.gate_threshold.get_block(input.len().max(1));
            self.gate
                .process_block(&input[..len], &mut self.buf1[..len], gate_thresh);
            self.amp.process(&self.buf1[..len], &mut self.buf2[..len]);
            self.cab
                .process_block(&self.buf2[..len], &mut self.buf1[..len]);

            let delay_enabled = self.params.delay_enabled.get_raw() > 0.5;
            if delay_enabled {
                let delay_ms = ParamId::DelayTimeMs.range().0
                    + (ParamId::DelayTimeMs.range().1 - ParamId::DelayTimeMs.range().0)
                        * self.params.delay_time_ms.get_block(input.len().max(1));
                let delay_fb = ParamId::DelayFeedback.range().0
                    + (ParamId::DelayFeedback.range().1 - ParamId::DelayFeedback.range().0)
                        * self.params.delay_feedback.get_block(input.len().max(1));
                let delay_mix = self.params.delay_mix.get_block(input.len().max(1));
                self.delay.process_block(
                    &self.buf1[..len],
                    &mut self.buf2[..len],
                    delay_ms,
                    delay_fb,
                    delay_mix,
                );
            } else {
                self.buf2[..len].copy_from_slice(&self.buf1[..len]);
            }

            let master_vol = self.params.master_volume.get_block(input.len().max(1));
            for (out, src) in output[..len].iter_mut().zip(self.buf2[..len].iter()) {
                *out = src * master_vol;
            }
        }

        for &s in output.iter() {
            self.output_meter.update(s);
        }
    }

    /// Process stereo (mono in -> stereo out).
    pub fn process_stereo(
        &mut self,
        input: &[f32],
        output_l: &mut [f32],
        output_r: &mut [f32],
        ctx: &AudioContext,
    ) {
        self.process(input, output_l, ctx);
        let n = output_l.len().min(output_r.len());
        output_r[..n].copy_from_slice(&output_l[..n]);
    }

    pub fn input_level_db(&self) -> f32 {
        self.input_meter.get_db()
    }

    pub fn output_level_db(&self) -> f32 {
        self.output_meter.get_db()
    }

    pub fn is_bypassed(&self) -> bool {
        self.bypassed
    }

    pub fn set_bypass(&mut self, bypass: bool) {
        self.bypassed = bypass;
        self.params
            .bypass
            .set_target(if bypass { 1.0 } else { 0.0 });
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn reset(&mut self) {
        self.gate = NoiseGate::new(self.sample_rate);
        self.amp.reset();
        self.cab.reset();
        self.delay.clear();
        self.input_meter.reset();
        self.output_meter.reset();
    }

    /// Reconfigure for a new sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if (sample_rate - self.sample_rate).abs() < 1.0 {
            return;
        }
        self.sample_rate = sample_rate;
        self.params.set_sample_rate(sample_rate);
        self.gate = NoiseGate::new(sample_rate);
        self.amp = TubeAmp::new(self.amp_model(), sample_rate);
        self.amp.set_settings(*self.amp_settings());
        self.cab = CabSim::new(sample_rate);
        self.delay = Delay::new(sample_rate, 2000.0);
        self.input_meter = PeakMeter::new(sample_rate);
        self.output_meter = PeakMeter::new(sample_rate);
    }
}
