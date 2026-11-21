//! Guitar amp integration: lock-free control, telemetry and audio processing.
//!
//! The MK3 audio input is captured by a CPAL input stream in `audio.rs` and
//! forwarded to the output callback through a preallocated SPSC ring buffer.
//! The output callback runs the `maschine-dsp` tube amp and mixes the result
//! into the normal Maschine output (input monitoring). Nothing in this module
//! allocates or takes a blocking lock in the audio callback.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use atomic_float::AtomicF32;
use maschine_dsp::{
    AmpModel, AmpSettings, AudioContext, CabModel, ClipDetector, GuitarChain, RtDiagnostics, Tuner,
    TunerReading,
};
use maschine_engine::display_render::GuitarScreenData;
use parking_lot::Mutex;

#[cfg(feature = "analysis-dsp")]
use maschine_dsp::SpectrumAnalyser;

/// Capacity of the input ring buffer, in mono frames (about 170 ms at 48 kHz).
pub const RING_CAPACITY: usize = 8192;

/// Single-producer / single-consumer ring buffer of `f32` samples.
///
/// The input callback is the only producer, the output callback the only
/// consumer. Indexes are atomics; the sample storage is preallocated once.
pub struct SpscRing {
    data: UnsafeCell<Vec<f32>>,
    capacity: usize,
    read: AtomicUsizeFallback,
    write: AtomicUsizeFallback,
}

/// Thin wrapper so the field names read clearly in the implementation.
type AtomicUsizeFallback = AtomicU64;

// SAFETY: exactly one thread writes (`write`) and one thread reads (`read`),
// and samples are only touched through their respective index. The buffer is
// preallocated and never resized after publication.
unsafe impl Send for SpscRing {}
unsafe impl Sync for SpscRing {}

impl SpscRing {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(2);
        Self {
            data: UnsafeCell::new(vec![0.0; capacity]),
            capacity,
            read: AtomicU64::new(0),
            write: AtomicU64::new(0),
        }
    }

    /// Append as many samples as fit. Returns the number actually written.
    pub fn write(&self, samples: &[f32]) -> usize {
        let w = self.write.load(Ordering::Relaxed);
        let r = self.read.load(Ordering::Acquire);
        let used = (w - r) as usize;
        let free = self.capacity.saturating_sub(used);
        let n = free.min(samples.len());
        if n == 0 {
            return 0;
        }
        // SAFETY: only this thread writes, and we stay within the free region.
        let data = unsafe { &mut *self.data.get() };
        for (i, &s) in samples[..n].iter().enumerate() {
            data[((w + i as u64) as usize) % self.capacity] = s;
        }
        self.write.store(w + n as u64, Ordering::Release);
        n
    }

    /// Empty the ring into `dest`. Returns the number of samples read.
    pub fn read_into(&self, dest: &mut [f32]) -> usize {
        let r = self.read.load(Ordering::Relaxed);
        let w = self.write.load(Ordering::Acquire);
        let available = (w - r) as usize;
        let n = available.min(dest.len());
        if n == 0 {
            return 0;
        }
        // SAFETY: only this thread reads, and we stay within the filled region.
        let data = unsafe { &*self.data.get() };
        for (i, slot) in dest[..n].iter_mut().enumerate() {
            *slot = data[((r + i as u64) as usize) % self.capacity];
        }
        self.read.store(r + n as u64, Ordering::Release);
        n
    }

    pub fn available(&self) -> usize {
        let r = self.read.load(Ordering::Acquire);
        let w = self.write.load(Ordering::Acquire);
        (w - r) as usize
    }

    pub fn clear(&self) {
        self.read.store(0, Ordering::Relaxed);
        self.write.store(0, Ordering::Relaxed);
    }
}

/// Audio-thread controls published by the GUI. All values are atomics.
pub struct GuitarControl {
    pub enabled: AtomicBool,
    pub monitor: AtomicBool,
    pub bypass: AtomicBool,
    pub model: AtomicU32,
    pub cab_model: AtomicU32,
    pub gain: AtomicF32,
    pub bass: AtomicF32,
    pub mid: AtomicF32,
    pub treble: AtomicF32,
    pub presence: AtomicF32,
    pub sag: AtomicF32,
    pub master: AtomicF32,
    pub monitor_gain: AtomicF32,
}

impl Default for GuitarControl {
    fn default() -> Self {
        let settings = AmpSettings::default();
        Self {
            enabled: AtomicBool::new(true),
            monitor: AtomicBool::new(true),
            bypass: AtomicBool::new(false),
            model: AtomicU32::new(0),
            cab_model: AtomicU32::new(0),
            gain: AtomicF32::new(settings.gain),
            bass: AtomicF32::new(settings.bass),
            mid: AtomicF32::new(settings.mid),
            treble: AtomicF32::new(settings.treble),
            presence: AtomicF32::new(settings.presence),
            sag: AtomicF32::new(settings.sag),
            master: AtomicF32::new(settings.master),
            monitor_gain: AtomicF32::new(0.8),
        }
    }
}

impl GuitarControl {
    pub fn set_norm(&self, param: GuitarParam, value: f32) {
        let v = value.clamp(0.0, 1.0);
        match param {
            GuitarParam::Gain => self.gain.store(v, Ordering::Relaxed),
            GuitarParam::Bass => self.bass.store(v, Ordering::Relaxed),
            GuitarParam::Mid => self.mid.store(v, Ordering::Relaxed),
            GuitarParam::Treble => self.treble.store(v, Ordering::Relaxed),
            GuitarParam::Presence => self.presence.store(v, Ordering::Relaxed),
            GuitarParam::Sag => self.sag.store(v, Ordering::Relaxed),
            GuitarParam::Master => self.master.store(v, Ordering::Relaxed),
            GuitarParam::MonitorGain => self.monitor_gain.store(v, Ordering::Relaxed),
        }
    }

    pub fn get_norm(&self, param: GuitarParam) -> f32 {
        match param {
            GuitarParam::Gain => self.gain.load(Ordering::Relaxed),
            GuitarParam::Bass => self.bass.load(Ordering::Relaxed),
            GuitarParam::Mid => self.mid.load(Ordering::Relaxed),
            GuitarParam::Treble => self.treble.load(Ordering::Relaxed),
            GuitarParam::Presence => self.presence.load(Ordering::Relaxed),
            GuitarParam::Sag => self.sag.load(Ordering::Relaxed),
            GuitarParam::Master => self.master.load(Ordering::Relaxed),
            GuitarParam::MonitorGain => self.monitor_gain.load(Ordering::Relaxed),
        }
    }

    pub fn amp_settings(&self) -> AmpSettings {
        AmpSettings {
            gain: self.gain.load(Ordering::Relaxed),
            bass: self.bass.load(Ordering::Relaxed),
            mid: self.mid.load(Ordering::Relaxed),
            treble: self.treble.load(Ordering::Relaxed),
            presence: self.presence.load(Ordering::Relaxed),
            sag: self.sag.load(Ordering::Relaxed),
            master: self.master.load(Ordering::Relaxed),
        }
    }

    pub fn amp_model(&self) -> AmpModel {
        AmpModel::from_index(self.model.load(Ordering::Relaxed) as usize)
    }

    pub fn cab_model(&self) -> CabModel {
        CabModel::from_index(self.cab_model.load(Ordering::Relaxed) as usize)
    }
}

/// Continuous guitar parameters exposed in the GUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuitarParam {
    Gain,
    Bass,
    Mid,
    Treble,
    Presence,
    Sag,
    Master,
    MonitorGain,
}

impl GuitarParam {
    pub const ALL: [GuitarParam; 8] = [
        GuitarParam::Gain,
        GuitarParam::Bass,
        GuitarParam::Mid,
        GuitarParam::Treble,
        GuitarParam::Presence,
        GuitarParam::Sag,
        GuitarParam::Master,
        GuitarParam::MonitorGain,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GuitarParam::Gain => "GAIN",
            GuitarParam::Bass => "BASS",
            GuitarParam::Mid => "MID",
            GuitarParam::Treble => "TREBLE",
            GuitarParam::Presence => "PRESENCE",
            GuitarParam::Sag => "SAG",
            GuitarParam::Master => "MASTER",
            GuitarParam::MonitorGain => "MONITOR",
        }
    }
}

/// Immutable-ish snapshot updated by the audio thread, read by the GUI.
#[derive(Debug, Clone)]
pub struct GuitarTelemetry {
    pub input_db: f32,
    pub output_db: f32,
    pub cpu_load: f32,
    pub latency_ms: f32,
    pub peak: f32,
    pub clipped_samples: u32,
    pub total_clipped: u64,
    pub blocks: u64,
    pub xruns: u32,
    pub tuner: Option<TunerReading>,
    pub spectrum: Vec<f32>,
    pub input_device: String,
}

impl Default for GuitarTelemetry {
    fn default() -> Self {
        Self {
            input_db: -120.0,
            output_db: -120.0,
            cpu_load: 0.0,
            latency_ms: 0.0,
            peak: 0.0,
            clipped_samples: 0,
            total_clipped: 0,
            blocks: 0,
            xruns: 0,
            tuner: None,
            spectrum: Vec::new(),
            input_device: String::new(),
        }
    }
}

/// State shared between the GUI thread and the audio callback.
pub struct GuitarRuntime {
    pub control: GuitarControl,
    pub telemetry: Mutex<GuitarTelemetry>,
    pub input_ring: SpscRing,
    pub input_device_name: Mutex<Option<String>>,
    /// Samples dropped because the ring was full (input callback).
    pub dropped: AtomicU64,
}

impl Default for GuitarRuntime {
    fn default() -> Self {
        Self {
            control: GuitarControl::default(),
            telemetry: Mutex::new(GuitarTelemetry::default()),
            input_ring: SpscRing::new(RING_CAPACITY),
            input_device_name: Mutex::new(None),
            dropped: AtomicU64::new(0),
        }
    }
}

impl GuitarRuntime {
    /// Snapshot sans verrou bloquant pour les écrans MK3 et le miroir GUI.
    pub fn screen_data(&self) -> GuitarScreenData {
        let c = &self.control;
        let telemetry = self.telemetry.try_lock();
        let mut data = GuitarScreenData {
            model: c.model.load(Ordering::Relaxed) as u8,
            cab: c.cab_model.load(Ordering::Relaxed) as u8,
            enabled: c.enabled.load(Ordering::Relaxed),
            monitor: c.monitor.load(Ordering::Relaxed),
            bypass: c.bypass.load(Ordering::Relaxed),
            params: [
                c.gain.load(Ordering::Relaxed),
                c.bass.load(Ordering::Relaxed),
                c.mid.load(Ordering::Relaxed),
                c.treble.load(Ordering::Relaxed),
                c.presence.load(Ordering::Relaxed),
                c.sag.load(Ordering::Relaxed),
                c.master.load(Ordering::Relaxed),
                c.monitor_gain.load(Ordering::Relaxed),
            ],
            ..GuitarScreenData::default()
        };
        if let Some(t) = telemetry {
            data.input_db = t.input_db;
            data.output_db = t.output_db;
            data.cpu_load = t.cpu_load;
            data.latency_ms = t.latency_ms;
            data.xruns = t.xruns;
            data.tuner_label = t.tuner.map(|r| r.label());
            data.tuner_freq = t.tuner.map(|r| r.frequency).unwrap_or(0.0);
            let len = t.spectrum.len();
            if len > 0 {
                let bands = data.spectrum.len();
                for (band, slot) in data.spectrum.iter_mut().enumerate() {
                    let from = band * len / bands;
                    let to = ((band + 1) * len / bands).max(from + 1).min(len);
                    let mut peak = -120.0_f32;
                    for &db in &t.spectrum[from..to] {
                        if db > peak {
                            peak = db;
                        }
                    }
                    *slot = peak;
                }
            }
        }
        data
    }

    /// Applique une action de soft key de la page ampli (1..8).
    /// La touche 8 (« PAGE SND ») est gérée par la GUI.
    pub fn soft_key(&self, key: u8) {
        match key {
            1 => {
                let v = self.control.model.load(Ordering::Relaxed);
                self.control
                    .model
                    .store(v.saturating_sub(1), Ordering::Relaxed);
            }
            2 => {
                let v = self.control.model.load(Ordering::Relaxed);
                self.control.model.store(
                    (v + 1).min(AmpModel::ALL.len() as u32 - 1),
                    Ordering::Relaxed,
                );
            }
            3 => {
                let v = self.control.cab_model.load(Ordering::Relaxed);
                self.control
                    .cab_model
                    .store(v.saturating_sub(1), Ordering::Relaxed);
            }
            4 => {
                let v = self.control.cab_model.load(Ordering::Relaxed);
                self.control.cab_model.store(
                    (v + 1).min(CabModel::ALL.len() as u32 - 1),
                    Ordering::Relaxed,
                );
            }
            5 => {
                let v = self.control.bypass.load(Ordering::Relaxed);
                self.control.bypass.store(!v, Ordering::Relaxed);
            }
            6 => {
                let v = self.control.monitor.load(Ordering::Relaxed);
                self.control.monitor.store(!v, Ordering::Relaxed);
            }
            7 => {
                let defaults = GuitarControl::default();
                for param in GuitarParam::ALL {
                    self.control.set_norm(param, defaults.get_norm(param));
                }
            }
            _ => {}
        }
    }
}

/// Audio-thread-only processor: chain, analysers and diagnostics.
pub struct GuitarProcessor {
    runtime: Arc<GuitarRuntime>,
    chain: GuitarChain,
    tuner: Tuner,
    #[cfg(feature = "analysis-dsp")]
    spectrum: SpectrumAnalyser,
    clip: ClipDetector,
    diag: RtDiagnostics,
    ctx: AudioContext,
    in_buf: Vec<f32>,
    out_buf: Vec<f32>,
    last_model: u32,
    last_cab: u32,
    last_bypass: bool,
    external_control_set: bool,
}

impl GuitarProcessor {
    pub fn new(sample_rate: u32, runtime: Arc<GuitarRuntime>) -> Self {
        let sr = sample_rate as f32;
        #[cfg(feature = "analysis-dsp")]
        let spectrum = SpectrumAnalyser::new(2048);
        Self {
            runtime,
            chain: GuitarChain::new(sr),
            tuner: Tuner::new(sr, 100.0),
            #[cfg(feature = "analysis-dsp")]
            spectrum,
            clip: ClipDetector::new(0.99),
            diag: RtDiagnostics::new(sr, 128),
            ctx: AudioContext {
                sample_rate: sr,
                bpm: 120.0,
                frame_count: 0,
            },
            in_buf: vec![0.0; 4096],
            out_buf: vec![0.0; 4096],
            last_model: u32::MAX,
            last_cab: u32::MAX,
            last_bypass: true,
            external_control_set: false,
        }
    }

    fn sync_controls(&mut self) {
        if !self.external_control_set {
            // The integrated app owns the amp settings; avoid the legacy
            // Drive/Tone params overwriting them on every block.
            self.chain.set_external_amp_control(true);
            self.chain
                .get_param_mut(maschine_dsp::ParamId::MasterVolume)
                .set_target(1.0);
            self.external_control_set = true;
        }
        let c = &self.runtime.control;
        let model = c.model.load(Ordering::Relaxed);
        if model != self.last_model {
            self.chain
                .set_amp_model(AmpModel::from_index(model as usize));
            self.last_model = model;
        }
        let cab = c.cab_model.load(Ordering::Relaxed);
        if cab != self.last_cab {
            self.chain.set_cab_model(CabModel::from_index(cab as usize));
            self.last_cab = cab;
        }
        let bypass = c.bypass.load(Ordering::Relaxed);
        if bypass != self.last_bypass {
            self.chain.set_bypass(bypass);
            self.last_bypass = bypass;
        }
        self.chain.set_amp_settings(c.amp_settings());
    }

    /// Mix the processed guitar signal into the output block.
    pub fn process_output(
        &mut self,
        out: &mut [f32],
        channels: usize,
        sample_rate: u32,
        hp_vol: f32,
    ) {
        let started = self.diag.begin_block();
        let channels = channels.max(1);
        let frames = (out.len() / channels).min(self.in_buf.len());
        if frames == 0 {
            return;
        }

        let enabled = self.runtime.control.enabled.load(Ordering::Relaxed);
        let monitor = self.runtime.control.monitor.load(Ordering::Relaxed);
        if !enabled || !monitor {
            self.runtime.input_ring.clear();
            return;
        }

        let read = self
            .runtime
            .input_ring
            .read_into(&mut self.in_buf[..frames]);
        if read < frames {
            self.in_buf[read..frames].fill(0.0);
        }
        self.sync_controls();
        let c = &self.runtime.control;

        self.ctx.sample_rate = sample_rate as f32;
        self.ctx.frame_count = self.ctx.frame_count.wrapping_add(frames as u64);
        self.chain.process(
            &self.in_buf[..frames],
            &mut self.out_buf[..frames],
            &self.ctx,
        );

        // Analysis runs on the raw guitar signal.
        for &s in &self.in_buf[..frames] {
            self.tuner.process(s);
            #[cfg(feature = "analysis-dsp")]
            self.spectrum.process(s);
        }

        let monitor_gain = c.monitor_gain.load(Ordering::Relaxed);
        for (i, &value) in self.out_buf[..frames].iter().enumerate() {
            let sample = value * monitor_gain;
            let base = i * channels;
            for ch in 0..channels {
                let factor = if ch < 2 { 1.0 } else { hp_vol };
                out[base + ch] += sample * factor;
            }
        }

        let clip = self.clip.process(&self.out_buf[..frames]);
        self.diag.end_block(started, clip);

        // Publish telemetry without ever blocking in the audio callback.
        if let Some(mut t) = self.runtime.telemetry.try_lock() {
            t.input_db = self.chain.input_level_db();
            t.output_db = self.chain.output_level_db();
            let stats = self.diag.snapshot();
            t.cpu_load = stats.cpu_load;
            t.peak = stats.peak;
            t.clipped_samples = stats.clipped_samples;
            t.total_clipped = stats.total_clipped;
            t.blocks = stats.blocks;
            t.xruns = stats.xruns + self.runtime.dropped.load(Ordering::Relaxed) as u32;
            t.tuner = self.tuner.estimate();
            #[cfg(feature = "analysis-dsp")]
            {
                let bins = self.spectrum.analyse();
                if t.spectrum.len() != bins.len() {
                    t.spectrum = vec![-120.0; bins.len()];
                }
                t.spectrum.copy_from_slice(bins);
            }
            let in_name = self.runtime.input_device_name.lock();
            if let Some(name) = in_name.as_ref() {
                if t.input_device != *name {
                    t.input_device = name.clone();
                }
            }
        }
    }

    pub fn set_latency_ms(&mut self, ms: f32) {
        self.diag.set_latency_ms(ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_data_maps_controls_and_telemetry() {
        let runtime = GuitarRuntime::default();
        runtime.control.set_norm(GuitarParam::Gain, 0.8);
        runtime.control.model.store(2, Ordering::Relaxed);
        runtime.control.cab_model.store(1, Ordering::Relaxed);
        {
            let mut t = runtime.telemetry.lock();
            t.input_db = -9.5;
            t.cpu_load = 0.33;
            t.spectrum = vec![-120.0; 1025];
            t.spectrum[100] = -6.0;
        }
        let data = runtime.screen_data();
        assert_eq!(data.model, 2);
        assert_eq!(data.cab, 1);
        assert!((data.params[0] - 0.8).abs() < 1e-6);
        assert!((data.input_db + 9.5).abs() < 1e-6);
        assert!((data.cpu_load - 0.33).abs() < 1e-6);
        assert!(data.spectrum.iter().any(|v| *v > -20.0));
    }

    #[test]
    fn soft_keys_change_amp_and_bypass() {
        let runtime = GuitarRuntime::default();
        runtime.soft_key(2);
        assert_eq!(runtime.control.model.load(Ordering::Relaxed), 1);
        runtime.soft_key(1);
        assert_eq!(runtime.control.model.load(Ordering::Relaxed), 0);
        runtime.soft_key(4);
        assert_eq!(runtime.control.cab_model.load(Ordering::Relaxed), 1);
        runtime.soft_key(5);
        assert!(runtime.control.bypass.load(Ordering::Relaxed));
        runtime.soft_key(6);
        assert!(!runtime.control.monitor.load(Ordering::Relaxed));
    }

    #[test]
    fn ring_buffer_write_read() {
        let ring = SpscRing::new(8);
        assert_eq!(ring.write(&[1.0, 2.0, 3.0]), 3);
        assert_eq!(ring.available(), 3);
        let mut out = [0.0; 4];
        assert_eq!(ring.read_into(&mut out), 3);
        assert_eq!(&out[..3], &[1.0, 2.0, 3.0]);
        assert_eq!(ring.available(), 0);
    }

    #[test]
    fn ring_buffer_wraparound() {
        let ring = SpscRing::new(4);
        ring.write(&[1.0, 2.0, 3.0]);
        let mut out = [0.0; 3];
        ring.read_into(&mut out);
        assert_eq!(&out, &[1.0, 2.0, 3.0]);
        ring.write(&[4.0, 5.0]);
        let mut out2 = [0.0; 2];
        ring.read_into(&mut out2);
        assert_eq!(&out2, &[4.0, 5.0]);
    }

    #[test]
    fn controls_roundtrip() {
        let c = GuitarControl::default();
        c.set_norm(GuitarParam::Gain, 0.75);
        assert!((c.get_norm(GuitarParam::Gain) - 0.75).abs() < 1e-6);
        c.model.store(2, Ordering::Relaxed);
        assert_eq!(c.amp_model(), AmpModel::from_index(2));
    }

    #[test]
    fn processor_mixes_signal_without_nan() {
        let runtime = Arc::new(GuitarRuntime::default());
        let mut proc = GuitarProcessor::new(48000, runtime.clone());
        let input: Vec<f32> = (0..256)
            .map(|i| 0.2 * (2.0 * std::f32::consts::PI * 110.0 * i as f32 / 48000.0).sin())
            .collect();
        assert_eq!(runtime.input_ring.write(&input), 256);
        let mut out = vec![0.0; 256 * 2];
        proc.process_output(&mut out, 2, 48000, 1.0);
        assert!(out.iter().all(|s| s.is_finite()));
        assert!(out.iter().any(|s| s.abs() > 1e-5));
    }
}
