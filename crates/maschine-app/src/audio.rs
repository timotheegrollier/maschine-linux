//! Sortie audio via `cpal` vers MK3 (PipeWire).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::Context;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;

use maschine_engine::Engine;

/// Attente max du verrou moteur dans le callback avant de rendre du silence.
/// L'UI/HID ne tient le verrou que quelques µs : un `try_lock` raté ne doit
/// pas couper 10 ms de son (c'était la source des craquements).
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_millis(2);

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
    /// Frames du dernier callback = taille réelle du buffer du device.
    pub last_frames: AtomicU64,
    /// Plus grande taille de callback observée.
    pub max_frames: AtomicU64,
    /// Erreurs/underruns signalés par le backend ALSA.
    pub errors: AtomicU64,
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

    /// Taille du dernier buffer audio (frames) — 0 si inconnu.
    pub fn buffer_frames(&self) -> u64 {
        self.last_frames.load(Ordering::Relaxed)
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
    /// Ouvre un stream audio vers le périphérique sélectionné (ou défaut système).
    ///
    /// Négociation : 48 kHz si le device l'accepte (le kit DSP est ensuite
    /// généré au taux réel — jamais transposé), canaux 2 (4 sur MK3 : Main +
    /// casque) et **le plus petit buffer accepté** (256 → 512 → 1024 →
    /// défaut) : la latence et les trous de son en dépendaient directement.
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

        let default_cfg = device.default_output_config().ok();

        // Canaux : 4 sur la MK3 (Main 1/2 + casque 3/4), repli 2 si refusé ;
        // 2 (ou moins) ailleurs.
        let def_channels = default_cfg.as_ref().map(|c| c.channels()).unwrap_or(2);
        let channel_candidates: Vec<u16> = if is_mk3 {
            vec![4, def_channels.clamp(2, 4), 2]
        } else {
            vec![def_channels.clamp(1, 2)]
        };
        // Taux : 48 kHz d'abord (native des presets DSP), repli sur le défaut.
        let def_rate = default_cfg
            .as_ref()
            .map(|c| c.sample_rate().0)
            .unwrap_or(48000);
        let rates: Vec<u32> = if def_rate == 48000 {
            vec![48000]
        } else {
            vec![48000, def_rate]
        };
        let format = default_cfg
            .as_ref()
            .map(|c| c.sample_format())
            .unwrap_or(cpal::SampleFormat::F32);

        let stats = Arc::new(AudioStats::default());

        // Buffers courts d'abord : chaque repli double la latence mais reste
        // bien meilleur que le buffer « Default » du device (≈ 46 ms ici).
        let buffers = [
            cpal::BufferSize::Fixed(256),
            cpal::BufferSize::Fixed(512),
            cpal::BufferSize::Fixed(1024),
            cpal::BufferSize::Fixed(2048),
            cpal::BufferSize::Default,
        ];

        let mut built: Option<(cpal::Stream, cpal::StreamConfig)> = None;
        'negotiate: for &channels in &channel_candidates {
            for &rate in &rates {
                for &buffer_size in &buffers {
                    let cc = cpal::StreamConfig {
                        channels,
                        sample_rate: cpal::SampleRate(rate),
                        buffer_size,
                    };
                    let attempt = match format {
                        cpal::SampleFormat::F32 => build_f32_stream(
                            &device,
                            &cc,
                            engine.clone(),
                            channels as usize,
                            hp_dup,
                            stats.clone(),
                        ),
                        cpal::SampleFormat::I16 => build_i16_stream(
                            &device,
                            &cc,
                            engine.clone(),
                            channels as usize,
                            hp_dup,
                            stats.clone(),
                        ),
                        cpal::SampleFormat::U16 => build_u16_stream(
                            &device,
                            &cc,
                            engine.clone(),
                            channels as usize,
                            hp_dup,
                            stats.clone(),
                        ),
                        other => {
                            anyhow::bail!("format d'échantillon non supporté: {other:?}");
                        }
                    };
                    match attempt {
                        Ok(stream) => {
                            log::info!(
                                "Stream config négociée: channels={} rate={} buffer={:?} format={:?}",
                                cc.channels,
                                cc.sample_rate.0,
                                cc.buffer_size,
                                format
                            );
                            built = Some((stream, cc));
                            break 'negotiate;
                        }
                        Err(e) => {
                            log::debug!(
                                "Config refusée ({channels}ch {rate}Hz {buffer_size:?}): {e}"
                            );
                        }
                    }
                }
            }
        }

        let (stream, cfg) = built
            .ok_or_else(|| anyhow::anyhow!("aucune configuration audio acceptée par le device"))?;

        let actual_rate = cfg.sample_rate.0;
        let channels = cfg.channels as usize;

        // Le moteur (et ses presets DSP) se cale sur le taux réel AVANT que le
        // stream ne consomme des échantillons : sinon le kit joue faux.
        {
            let mut eng = engine.lock();
            eng.set_audio_sample_rate(actual_rate);
        }

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

/// Construit le stream F32 et branche le callback.
fn build_f32_stream(
    device: &cpal::Device,
    cfg: &cpal::StreamConfig,
    engine: Arc<Mutex<Engine>>,
    channels: usize,
    hp_dup: bool,
    stats: Arc<AudioStats>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let sr = cfg.sample_rate.0;
    let st_err = stats.clone();
    let st_cb = stats;
    device.build_output_stream(
        cfg,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            audio_callback_f32(data, &engine, sr, channels, hp_dup, &st_cb);
        },
        move |err| {
            st_err.errors.fetch_add(1, Ordering::Relaxed);
            log::warn!("cpal stream event: {err}");
        },
        None,
    )
}

/// Construit le stream I16 (device entier 16 bits).
fn build_i16_stream(
    device: &cpal::Device,
    cfg: &cpal::StreamConfig,
    engine: Arc<Mutex<Engine>>,
    channels: usize,
    hp_dup: bool,
    stats: Arc<AudioStats>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let sr = cfg.sample_rate.0;
    let st_err = stats.clone();
    let st_cb = stats;
    // Scratch pré-alloué : aucune allocation dans le callback.
    let mut tmp: Vec<f32> = vec![0.0; 8192 * 2];
    device.build_output_stream(
        cfg,
        move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
            if tmp.len() < data.len() {
                tmp.resize(data.len(), 0.0);
            }
            render_audio_block(
                &mut tmp[..data.len()],
                &engine,
                sr,
                channels,
                hp_dup,
                &st_cb,
            );
            for (o, &s) in data.iter_mut().zip(tmp.iter()) {
                *o = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            }
        },
        move |err| {
            st_err.errors.fetch_add(1, Ordering::Relaxed);
            log::warn!("cpal stream event: {err}");
        },
        None,
    )
}

/// Construit le stream U16 (device entier non signé 16 bits).
fn build_u16_stream(
    device: &cpal::Device,
    cfg: &cpal::StreamConfig,
    engine: Arc<Mutex<Engine>>,
    channels: usize,
    hp_dup: bool,
    stats: Arc<AudioStats>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let sr = cfg.sample_rate.0;
    let st_err = stats.clone();
    let st_cb = stats;
    let mut tmp: Vec<f32> = vec![0.0; 8192 * 2];
    device.build_output_stream(
        cfg,
        move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
            if tmp.len() < data.len() {
                tmp.resize(data.len(), 0.0);
            }
            render_audio_block(
                &mut tmp[..data.len()],
                &engine,
                sr,
                channels,
                hp_dup,
                &st_cb,
            );
            for (o, &s) in data.iter_mut().zip(tmp.iter()) {
                *o = ((s.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16;
            }
        },
        move |err| {
            st_err.errors.fetch_add(1, Ordering::Relaxed);
            log::warn!("cpal stream event: {err}");
        },
        None,
    )
}

/// Acquiert le verrou moteur en réessayant brièvement : l'UI/HID le tient
/// quelques µs, un `try_lock` unique ratait donc régulièrement (silence =
/// craquement). Renvoie `None` si le verrou reste pris au-delà de `LOCK_WAIT`.
fn lock_engine_with_wait(
    engine: &Arc<Mutex<Engine>>,
) -> Option<parking_lot::MutexGuard<'_, Engine>> {
    if let Some(g) = engine.try_lock() {
        return Some(g);
    }
    let deadline = std::time::Instant::now() + LOCK_WAIT;
    let mut spins = 0u32;
    loop {
        if let Some(g) = engine.try_lock() {
            return Some(g);
        }
        std::hint::spin_loop();
        spins += 1;
        if spins % 128 == 0 {
            if std::time::Instant::now() >= deadline {
                return None;
            }
            // Laisse la main au détenteur du verrou (autre cœur ou non).
            std::thread::yield_now();
        }
    }
}

/// Callback audio F32 : rendu stéréo du moteur vers un buffer interleaved.
fn audio_callback_f32(
    data: &mut [f32],
    engine: &Arc<Mutex<Engine>>,
    sample_rate: u32,
    channels: usize,
    hp_dup: bool,
    stats: &AudioStats,
) {
    render_audio_block(data, engine, sample_rate, channels, hp_dup, stats);
}

/// Rendu d'un bloc complet dans `out`, au format interleaved du device
/// (`channels` canaux). Aucune allocation ; la taille du bloc peut dépasser
/// n'importe quelle constante (le buffer du device n'est jamais tronqué).
fn render_audio_block(
    out: &mut [f32],
    engine: &Arc<Mutex<Engine>>,
    sample_rate: u32,
    channels: usize,
    hp_dup: bool,
    stats: &AudioStats,
) {
    let channels = channels.max(1);
    let frames = out.len() / channels;
    stats.last_frames.store(frames as u64, Ordering::Relaxed);
    stats.max_frames.fetch_max(frames as u64, Ordering::Relaxed);

    let started = std::time::Instant::now();
    let Some(mut eng) = lock_engine_with_wait(engine) else {
        stats.record_miss();
        for s in out.iter_mut() {
            *s = 0.0;
        }
        return;
    };

    // Filet de sécurité : si le device a changé de taux sans réouverture, on
    // réaligne l'horloge (aucune allocation ici, le kit est régénéré à
    // l'ouverture du stream via `set_audio_sample_rate`).
    if eng.transport.sample_rate != sample_rate {
        eng.align_sample_rate_clock(sample_rate);
    }

    eng.advance(frames as u64);
    let hp_vol = eng.headphone_volume();

    const CHUNK_FRAMES: usize = 1024;
    let mut stack_buf = [0.0f32; CHUNK_FRAMES * 2];
    let mut frame = 0usize;
    while frame < frames {
        let n = (frames - frame).min(CHUNK_FRAMES);
        if channels == 2 {
            // Chemin direct : le layout stéréo du moteur est celui du device.
            eng.render_audio(&mut out[frame * 2..(frame + n) * 2]);
        } else {
            let stereo = &mut stack_buf[..n * 2];
            eng.render_audio(stereo);
            for i in 0..n {
                let l = stereo[i * 2];
                let r = stereo[i * 2 + 1];
                let base = (frame + i) * channels;
                out[base] = l;
                out[base + 1] = r;
                #[allow(clippy::needless_range_loop)] // mapping canal → sortie
                for ch in 2..channels {
                    out[base + ch] = if hp_dup {
                        (if ch % 2 == 0 { l } else { r }) * hp_vol
                    } else {
                        0.0
                    };
                }
            }
        }
        frame += n;
    }

    // Sécurité : le limiteur du moteur borne déjà à ±1 (jamais d'écrêtage dur).
    for s in out.iter_mut() {
        *s = s.clamp(-1.0, 1.0);
    }
    stats.record(started.elapsed().as_micros() as u64);
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
