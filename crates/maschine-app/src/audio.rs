//! Sortie audio via `cpal` vers MK3 (PipeWire).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::Context;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;

use maschine_engine::Engine;

/// Statistiques temps réel du callback audio (atomiques, sans verrou) —
/// utilisées par `--debug-latency` et affichables dans la GUI.
#[derive(Debug, Default)]
pub struct AudioStats {
    /// Callbacks rendus (verrou obtenu).
    pub callbacks: AtomicU64,
    /// Callbacks où le verrou engine était occupé (silence de sécurité).
    pub lock_misses: AtomicU64,
    /// Durée max de traitement d'un callback (µs).
    pub max_callback_us: AtomicU64,
}

impl AudioStats {
    /// Comptabilise une passe (durée µs).
    pub fn record(&self, us: u64) {
        self.callbacks.fetch_add(1, Ordering::Relaxed);
        self.max_callback_us.fetch_max(us, Ordering::Relaxed);
    }

    /// Comptabilise un échec de verrou (buffer de silence).
    pub fn record_miss(&self) {
        self.lock_misses.fetch_add(1, Ordering::Relaxed);
    }
}

/// Gère le stream cpal.
pub struct AudioOutput {
    _stream: cpal::Stream,
    #[allow(dead_code)]
    device_name: String,
    sample_rate: u32,
    /// Statistiques temps réel partagées.
    pub stats: Arc<AudioStats>,
}

impl AudioOutput {
    /// Ouvre un stream audio vers le périphérique sélectionné (ou défaut système) à 48kHz, buffer 128, stéréo ou 4ch.
    /// `engine` est partagé avec le daemon/GUI (locké dans le callback).
    pub fn open(
        engine: Arc<Mutex<Engine>>,
        explicit_device: Option<&str>,
        hp_dup: bool,
    ) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        log::info!("cpal host: {:?}", host.id());

        let device = resolve_device(&host, explicit_device)?;
        let dev_name = device.name().unwrap_or_else(|_| "unknown".to_string());
        log::info!("Audio device sélectionné: {dev_name}");

        let is_mk3 =
            dev_name.to_lowercase().contains("mk3") || dev_name.to_lowercase().contains("maschine");

        // Récupère la config par défaut du périphérique
        let default_cfg = device.default_output_config().ok();

        let (chosen_cfg, cpal_config) = if is_mk3 {
            // Pour la Maschine MK3 : 4 canaux (Main 1/2 + Casque 3/4), 48kHz, buffer court
            let desired_rate = cpal::SampleRate(48000);
            let mut scfg = None;
            for &ch in &[4u16, 2u16] {
                if let Ok(configs) = device.supported_output_configs() {
                    for c in configs {
                        if c.channels() == ch
                            && c.min_sample_rate() <= desired_rate
                            && desired_rate <= c.max_sample_rate()
                            && c.sample_format() == cpal::SampleFormat::F32
                        {
                            scfg = Some(c.with_sample_rate(desired_rate));
                            break;
                        }
                    }
                }
                if scfg.is_some() {
                    break;
                }
            }
            let sc = scfg.or_else(|| default_cfg.clone()).unwrap_or_else(|| {
                cpal::SupportedStreamConfig::new(
                    4,
                    desired_rate,
                    cpal::SupportedBufferSize::Range { min: 32, max: 8192 },
                    cpal::SampleFormat::F32,
                )
            });
            let mut cc: cpal::StreamConfig = sc.clone().into();
            cc.buffer_size = cpal::BufferSize::Fixed(256);
            (sc, cc)
        } else {
            // Pour les périphériques PC (haut-parleurs, défaut système PipeWire/ALSA) :
            // Toujours privilégier la config par défaut en STEREO (2 canaux) avec buffer Default
            if let Some(ref d) = default_cfg {
                let channels = if d.channels() >= 2 {
                    2u16
                } else {
                    d.channels()
                };
                let sc = cpal::SupportedStreamConfig::new(
                    channels,
                    d.sample_rate(),
                    *d.buffer_size(),
                    d.sample_format(),
                );
                let mut cc: cpal::StreamConfig = sc.clone().into();
                cc.buffer_size = cpal::BufferSize::Default;
                (sc, cc)
            } else {
                // Fallback 48kHz stéréo
                let sc = cpal::SupportedStreamConfig::new(
                    2,
                    cpal::SampleRate(48000),
                    cpal::SupportedBufferSize::Range { min: 64, max: 8192 },
                    cpal::SampleFormat::F32,
                );
                let mut cc: cpal::StreamConfig = sc.clone().into();
                cc.buffer_size = cpal::BufferSize::Default;
                (sc, cc)
            }
        };

        log::info!(
            "Stream config retenue: channels={} rate={} buffer={:?} format={:?}",
            cpal_config.channels,
            cpal_config.sample_rate.0,
            cpal_config.buffer_size,
            chosen_cfg.sample_format()
        );

        // Callback — gère stéréo ou 4ch
        let engine_clone = engine.clone();
        let channels = cpal_config.channels as usize;
        let stats = Arc::new(AudioStats::default());
        let stream = match chosen_cfg.sample_format() {
            cpal::SampleFormat::F32 => {
                let eng = engine_clone.clone();
                let st = stats.clone();
                let cb = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    audio_callback(data, &eng, cpal_config.sample_rate.0, channels, hp_dup, &st);
                };
                match device.build_output_stream(
                    &cpal_config,
                    cb,
                    |err| log::warn!("cpal stream event: {err}"),
                    None,
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        log::warn!(
                            "Buffer size initial refusé ({e}) — fallback BufferSize::Default"
                        );
                        let mut cc_def = cpal_config.clone();
                        cc_def.buffer_size = cpal::BufferSize::Default;
                        let eng2 = engine_clone.clone();
                        let st2 = stats.clone();
                        let cb2 = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            audio_callback(
                                data,
                                &eng2,
                                cc_def.sample_rate.0,
                                channels,
                                hp_dup,
                                &st2,
                            );
                        };
                        device.build_output_stream(
                            &cc_def,
                            cb2,
                            |err| log::warn!("cpal stream event: {err}"),
                            None,
                        )?
                    }
                }
            }
            cpal::SampleFormat::I16 => {
                let eng = engine_clone.clone();
                let st = stats.clone();
                let cb = move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    let mut tmp = vec![0.0f32; data.len()];
                    audio_callback_f32(
                        &mut tmp,
                        &eng,
                        cpal_config.sample_rate.0,
                        channels,
                        hp_dup,
                        &st,
                    );
                    for (o, &s) in data.iter_mut().zip(tmp.iter()) {
                        *o = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                    }
                };
                match device.build_output_stream(
                    &cpal_config,
                    cb,
                    |err| log::error!("cpal stream err: {err}"),
                    None,
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        log::warn!("Buffer size Fixed(128) refusé ({e}) — essai buffer Default");
                        let mut cc_def = cpal_config.clone();
                        cc_def.buffer_size = cpal::BufferSize::Default;
                        let eng2 = engine_clone.clone();
                        let st2 = stats.clone();
                        let cb2 = move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                            let mut tmp = vec![0.0f32; data.len()];
                            audio_callback_f32(
                                &mut tmp,
                                &eng2,
                                cc_def.sample_rate.0,
                                channels,
                                hp_dup,
                                &st2,
                            );
                            for (o, &s) in data.iter_mut().zip(tmp.iter()) {
                                *o = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                            }
                        };
                        device.build_output_stream(
                            &cc_def,
                            cb2,
                            |err| log::error!("cpal stream err: {err}"),
                            None,
                        )?
                    }
                }
            }
            cpal::SampleFormat::U16 => {
                let eng = engine_clone.clone();
                let st = stats.clone();
                let cb = move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    let mut tmp = vec![0.0f32; data.len()];
                    audio_callback_f32(
                        &mut tmp,
                        &eng,
                        cpal_config.sample_rate.0,
                        channels,
                        hp_dup,
                        &st,
                    );
                    for (o, &s) in data.iter_mut().zip(tmp.iter()) {
                        *o = ((s.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16;
                    }
                };
                match device.build_output_stream(
                    &cpal_config,
                    cb,
                    |err| log::error!("cpal stream err: {err}"),
                    None,
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        log::warn!("Buffer size Fixed(128) refusé ({e}) — essai buffer Default");
                        let mut cc_def = cpal_config.clone();
                        cc_def.buffer_size = cpal::BufferSize::Default;
                        let eng2 = engine_clone.clone();
                        let st2 = stats.clone();
                        let cb2 = move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                            let mut tmp = vec![0.0f32; data.len()];
                            audio_callback_f32(
                                &mut tmp,
                                &eng2,
                                cc_def.sample_rate.0,
                                channels,
                                hp_dup,
                                &st2,
                            );
                            for (o, &s) in data.iter_mut().zip(tmp.iter()) {
                                *o = ((s.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16;
                            }
                        };
                        device.build_output_stream(
                            &cc_def,
                            cb2,
                            |err| log::error!("cpal stream err: {err}"),
                            None,
                        )?
                    }
                }
            }
            _ => anyhow::bail!("format non supporté"),
        };

        let actual_rate = cpal_config.sample_rate.0;
        stream.play().context("stream play")?;
        log::info!("✓ Audio stream démarré sur '{dev_name}' {actual_rate}Hz (channels={channels})");

        Ok(Self {
            _stream: stream,
            device_name: dev_name,
            sample_rate: actual_rate,
            stats,
        })
    }

    /// Alias pour compatibilité
    #[allow(dead_code)]
    pub fn open_mk3(
        engine: Arc<Mutex<Engine>>,
        explicit_device: Option<String>,
        hp_dup: bool,
    ) -> anyhow::Result<Self> {
        Self::open(engine, explicit_device.as_deref(), hp_dup)
    }

    /// Nom du device.
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Sample rate.
    #[allow(dead_code)]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Liste les devices (pour CLI).
    pub fn list_devices() -> anyhow::Result<()> {
        let host = cpal::default_host();
        println!("Host: {:?}", host.id());
        println!(
            "Default output: {:?}",
            host.default_output_device().and_then(|d| d.name().ok())
        );
        println!("\nDevices de sortie:");
        for dev in host.output_devices()? {
            let name = dev.name().unwrap_or_else(|_| "<err>".to_string());
            let is_default = host
                .default_output_device()
                .and_then(|d| d.name().ok())
                .map(|n| n == name)
                .unwrap_or(false);
            let marker = if is_default { " (défaut)" } else { "" };
            println!("  - {name}{marker}");
            match dev.supported_output_configs() {
                Ok(cfgs) => {
                    for cfg in cfgs {
                        println!(
                            "      {:?} ch={} rate={}..{}",
                            cfg.sample_format(),
                            cfg.channels(),
                            cfg.min_sample_rate().0,
                            cfg.max_sample_rate().0
                        );
                    }
                }
                Err(e) => println!("      err: {e}"),
            }
        }
        Ok(())
    }
}

fn audio_callback(
    data: &mut [f32],
    engine: &Arc<Mutex<Engine>>,
    sample_rate: u32,
    channels: usize,
    hp_dup: bool,
    stats: &AudioStats,
) {
    audio_callback_f32(data, engine, sample_rate, channels, hp_dup, stats);
}

fn audio_callback_f32(
    data: &mut [f32],
    engine: &Arc<Mutex<Engine>>,
    sample_rate: u32,
    channels: usize,
    hp_dup: bool,
    stats: &AudioStats,
) {
    let started = std::time::Instant::now();
    let frames = data.len() / channels;
    if let Some(mut eng) = engine.try_lock() {
        if eng.transport.sample_rate != sample_rate {
            eng.transport.sample_rate = sample_rate;
            eng.sampler_mut().sample_rate = sample_rate;
        }
        eng.advance(frames as u64);
        let hp_vol = eng.headphone_volume();
        if channels == 2 {
            eng.render_audio(data);
        } else if channels == 4 {
            // Buffer stéréo sur la pile pour éviter allocations sur le thread audio
            let mut stereo = [0.0f32; 2048];
            let stereo_len = (frames * 2).min(stereo.len());
            eng.render_audio(&mut stereo[..stereo_len]);
            for f in 0..frames.min(1024) {
                let l = stereo[f * 2];
                let r = stereo[f * 2 + 1];
                data[f * 4] = l;
                data[f * 4 + 1] = r;
                if hp_dup {
                    data[f * 4 + 2] = l * hp_vol;
                    data[f * 4 + 3] = r * hp_vol;
                } else {
                    data[f * 4 + 2] = 0.0;
                    data[f * 4 + 3] = 0.0;
                }
            }
        } else {
            let mut stereo = [0.0f32; 2048];
            let stereo_len = (frames * 2).min(stereo.len());
            eng.render_audio(&mut stereo[..stereo_len]);
            for f in 0..frames.min(1024) {
                for ch in 0..channels {
                    data[f * channels + ch] = if ch < 2 { stereo[f * 2 + ch] } else { 0.0 };
                }
            }
        }
        for s in data.iter_mut() {
            *s = s.clamp(-1.0, 1.0) * 0.85;
        }
        stats.record(started.elapsed().as_micros() as u64);
    } else {
        stats.record_miss();
        for s in data.iter_mut() {
            *s = 0.0;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
    pub is_mk3: bool,
    pub channels: u16,
    pub default_sample_rate: u32,
}

/// Liste détaillée des périphériques de sortie audio.
pub fn list_output_devices() -> Vec<AudioDeviceInfo> {
    let host = cpal::default_host();
    let default_name = host.default_output_device().and_then(|d| d.name().ok());
    let mut list = Vec::new();

    if let Ok(devices) = host.output_devices() {
        for dev in devices {
            if let Ok(name) = dev.name() {
                let is_default = default_name.as_deref() == Some(&name);
                let lower = name.to_lowercase();
                let is_mk3 = lower.contains("mk3") || lower.contains("maschine");
                let (channels, rate) = dev
                    .default_output_config()
                    .map(|c| (c.channels(), c.sample_rate().0))
                    .unwrap_or((2, 48000));

                list.push(AudioDeviceInfo {
                    name,
                    is_default,
                    is_mk3,
                    channels,
                    default_sample_rate: rate,
                });
            }
        }
    }
    list
}

pub fn resolve_device(host: &cpal::Host, requested: Option<&str>) -> anyhow::Result<cpal::Device> {
    if let Some(req) = requested {
        let req_trim = req.trim();
        let lower = req_trim.to_lowercase();
        if lower == "default" || lower == "pc" || lower == "system" || lower == "sys" {
            return host
                .default_output_device()
                .context("Aucun périphérique audio par défaut trouvé sur le système");
        }
        if lower == "mk3" || lower == "maschine" {
            return find_mk3_device(host).context("Périphérique audio Maschine MK3 non détecté");
        }
        // Recherche exacte
        if let Ok(devices) = host.output_devices() {
            for dev in devices {
                if let Ok(n) = dev.name() {
                    if n == req_trim {
                        return Ok(dev);
                    }
                }
            }
        }
        // Recherche par sous-chaîne
        if let Some(dev) = find_device_by_name(host, req_trim) {
            return Ok(dev);
        }
        anyhow::bail!("Périphérique audio '{req}' non trouvé");
    }

    // Aucun périphérique explicitement demandé :
    // On privilégie le périphérique par défaut du système (PC speakers / PipeWire)
    host.default_output_device()
        .or_else(|| find_mk3_device(host))
        .context("Aucun périphérique audio disponible")
}

fn find_device_by_name(host: &cpal::Host, name: &str) -> Option<cpal::Device> {
    for dev in host.output_devices().ok()? {
        if let Ok(n) = dev.name() {
            if n.to_lowercase().contains(&name.to_lowercase()) {
                return Some(dev);
            }
        }
    }
    None
}

fn find_mk3_device(host: &cpal::Host) -> Option<cpal::Device> {
    let keywords = ["maschine", "mk3", "usb audio", "pro-output", "native"];
    for dev in host.output_devices().ok()? {
        if let Ok(n) = dev.name() {
            let lower = n.to_lowercase();
            for kw in &keywords {
                if lower.contains(kw) {
                    log::info!("MK3 candidate: {n}");
                    return Some(dev);
                }
            }
        }
    }
    None
}

/// Génère un wav de test (sine) et le joue via cpal brièvement (pour test sans engine).
pub fn play_test_tone(
    duration_secs: f32,
    freq: f32,
    device_name: Option<&str>,
) -> anyhow::Result<()> {
    let host = cpal::default_host();
    let device = resolve_device(&host, device_name)?;
    let name = device.name().unwrap_or_else(|_| "unknown".to_string());
    println!("Test tone sur '{name}' {freq}Hz {duration_secs}s");
    let config = device.default_output_config()?;
    println!("Config: {:?}", config);
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let stream_config: cpal::StreamConfig = config.clone().into();
    let mut phase: f32 = 0.0;
    let increment = 2.0 * std::f32::consts::PI * freq / sample_rate as f32;
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _| {
                for frame in data.chunks_mut(channels) {
                    let s = (phase * 0.3).sin();
                    phase += increment;
                    if phase > 2.0 * std::f32::consts::PI {
                        phase -= 2.0 * std::f32::consts::PI;
                    }
                    for out in frame.iter_mut() {
                        *out = s;
                    }
                }
            },
            |e| eprintln!("err {e}"),
            None,
        )?,
        _ => anyhow::bail!("format non f32 pour test tone"),
    };
    stream.play()?;
    std::thread::sleep(std::time::Duration::from_secs_f32(duration_secs));
    Ok(())
}
