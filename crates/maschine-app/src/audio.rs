//! Sortie audio via `cpal` vers MK3 (PipeWire).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;

use maschine_engine::Engine;

use crate::guitar::{GuitarProcessor, GuitarRuntime};

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
    /// Taux d'échantillonnage du stream négocié (0 si aucun stream ouvert).
    pub sample_rate: AtomicU64,
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

    /// Taux d'échantillonnage réel du device — 0 si aucun stream n'est ouvert.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed) as u32
    }
}

/// Gère les streams cpal (sortie Maschine + entrée guitare MK3).
pub struct AudioOutput {
    _stream: cpal::Stream,
    /// Flux d'entrée de l'interface MK3 (si disponible).
    _input_stream: Option<cpal::Stream>,
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
        guitar: Arc<GuitarRuntime>,
    ) -> anyhow::Result<Self> {
        auto_configure_djm_routing();
        auto_configure_mk3_input();
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
                            guitar.clone(),
                        ),
                        cpal::SampleFormat::I16 => build_i16_stream(
                            &device,
                            &cc,
                            engine.clone(),
                            channels as usize,
                            hp_dup,
                            stats.clone(),
                            guitar.clone(),
                        ),
                        cpal::SampleFormat::U16 => build_u16_stream(
                            &device,
                            &cc,
                            engine.clone(),
                            channels as usize,
                            hp_dup,
                            stats.clone(),
                            guitar.clone(),
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
        stats
            .sample_rate
            .store(actual_rate as u64, Ordering::Relaxed);

        // Le moteur (et ses presets DSP) se cale sur le taux réel AVANT que le
        // stream ne consomme des échantillons : sinon le kit joue faux.
        {
            let mut eng = engine.lock();
            eng.set_audio_sample_rate(actual_rate);
        }

        // Entrée guitare : on ouvre le port de capture de la MK3 (profil
        // pro-audio) en fallback sur le device par défaut. Un échec ici ne
        // doit pas empêcher le reste de l'application de fonctionner.
        let input_stream = build_guitar_input_stream(&host, actual_rate, guitar.clone());
        if let Some(stream) = input_stream.as_ref() {
            if let Err(e) = stream.play() {
                log::warn!("Impossible de démarrer l'entrée guitare: {e}");
            }
        } else {
            log::warn!("Aucune entrée audio disponible — monitoring guitare désactivé");
            guitar.control.enabled.store(false, Ordering::Relaxed);
        }

        stream.play().context("stream play")?;
        log::info!("✓ Audio stream démarré sur '{dev_name}' {actual_rate}Hz (channels={channels})");

        Ok(Self {
            _stream: stream,
            _input_stream: input_stream,
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
        guitar: Arc<GuitarRuntime>,
    ) -> anyhow::Result<Self> {
        Self::open(engine, explicit_device.as_deref(), hp_dup, guitar)
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

/// Gestionnaire d'événements d'erreur cpal, limité en débit *temporel*.
///
/// L'arrêt du daemon fait remonter des tempêtes d'événements ALSA
/// (POLLERR pendant le drop du stream) : des millions d'événements en une
/// seconde. On journalise la première erreur puis au plus une par seconde.
fn event_logger(stats: Arc<AudioStats>) -> impl FnMut(cpal::StreamError) + Send + 'static {
    let mut logged_at: Option<Instant> = None;
    let mut total: u64 = 0;
    move |err: cpal::StreamError| {
        stats.errors.fetch_add(1, Ordering::Relaxed);
        total += 1;
        let now = Instant::now();
        if logged_at.is_none_or(|t| now.duration_since(t) >= Duration::from_secs(1)) {
            log::warn!("cpal stream event: {err} (erreur audio #{total})");
            logged_at = Some(now);
        }
    }
}

/// Estimation de la latence aller-retour (entrée + sortie) d'après la
/// configuration négociée. Affichée dans les diagnostics de l'ampli.
fn guitar_latency_ms(cfg: &cpal::StreamConfig) -> f32 {
    let frames = match cfg.buffer_size {
        cpal::BufferSize::Fixed(n) => n as f32,
        cpal::BufferSize::Default => 256.0,
    };
    let ms = frames * 1000.0 / cfg.sample_rate.0 as f32;
    // Deux buffers (capture + lecture) plus une petite marge backend.
    ms * 2.0 + 2.0
}

fn get_input_candidates(host: &cpal::Host) -> Vec<cpal::Device> {
    let mut candidates = Vec::new();
    let mut seen_names = Vec::new();

    // 1. Device MK3 dédié (carte matérielle ALSA directe)
    if let Some(dev) = find_mk3_input_device(host) {
        if let Ok(name) = dev.name() {
            seen_names.push(name);
        }
        candidates.push(dev);
    }

    // 2. PipeWire / Pulse / Default (connecté au graphe PipeWire avec node MK3 comme default source)
    if let Ok(devices) = host.input_devices() {
        for dev in devices {
            if let Ok(name) = dev.name() {
                let lower = name.to_lowercase();
                if (lower == "pipewire" || lower == "pulse" || lower == "default")
                    && !seen_names.iter().any(|n| n == &name)
                {
                    seen_names.push(name);
                    candidates.push(dev);
                }
            }
        }
    }

    // 3. Périphérique par défaut de l'hôte
    if let Some(def) = host.default_input_device() {
        if let Ok(name) = def.name() {
            if !seen_names.iter().any(|n| n == &name) {
                candidates.push(def);
            }
        }
    }

    candidates
}

/// Ouvre le flux de capture de la MK3 (ou du device par défaut) et écrit les
/// échantillons mono dans le ring buffer de l'ampli. L'échec est non fatal :
/// l'application continue sans monitoring guitare.
fn build_guitar_input_stream(
    host: &cpal::Host,
    sample_rate: u32,
    guitar: Arc<GuitarRuntime>,
) -> Option<cpal::Stream> {
    for device in get_input_candidates(host) {
        let name = device.name().unwrap_or_else(|_| "unknown".to_string());
        log::info!("Tentative ouverture entrée guitare: {name}");

        let input_format = device
            .default_input_config()
            .map(|c| c.sample_format())
            .unwrap_or(cpal::SampleFormat::F32);

        for channels in [2u16, 1] {
            let cfg = cpal::StreamConfig {
                channels,
                sample_rate: cpal::SampleRate(sample_rate),
                buffer_size: cpal::BufferSize::Default,
            };
            let rt = guitar.clone();
            let attempt = match input_format {
                cpal::SampleFormat::F32 => device.build_input_stream(
                    &cfg,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let ch_mode = rt.control.input_channel.load(Ordering::Relaxed);
                        push_input_frames(
                            &rt.input_ring,
                            &rt.dropped,
                            data,
                            channels as usize,
                            ch_mode,
                        );
                    },
                    |e| log::warn!("Input stream error: {e}"),
                    None,
                ),
                cpal::SampleFormat::I16 => device.build_input_stream(
                    &cfg,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let ch_mode = rt.control.input_channel.load(Ordering::Relaxed);
                        let mut converted = [0.0f32; 1024];
                        for chunk in data.chunks(1024) {
                            for (i, &s) in chunk.iter().enumerate() {
                                converted[i] = s as f32 / i16::MAX as f32;
                            }
                            push_input_frames(
                                &rt.input_ring,
                                &rt.dropped,
                                &converted[..chunk.len()],
                                channels as usize,
                                ch_mode,
                            );
                        }
                    },
                    |e| log::warn!("Input stream error: {e}"),
                    None,
                ),
                cpal::SampleFormat::I32 => device.build_input_stream(
                    &cfg,
                    move |data: &[i32], _: &cpal::InputCallbackInfo| {
                        let ch_mode = rt.control.input_channel.load(Ordering::Relaxed);
                        let mut converted = [0.0f32; 1024];
                        for chunk in data.chunks(1024) {
                            for (i, &s) in chunk.iter().enumerate() {
                                converted[i] = s as f32 / i32::MAX as f32;
                            }
                            push_input_frames(
                                &rt.input_ring,
                                &rt.dropped,
                                &converted[..chunk.len()],
                                channels as usize,
                                ch_mode,
                            );
                        }
                    },
                    |e| log::warn!("Input stream error: {e}"),
                    None,
                ),
                other => {
                    log::warn!("Format d'entrée non supporté sur '{name}': {other:?}");
                    continue;
                }
            };
            match attempt {
                Ok(stream) => {
                    log::info!(
                        "✓ Entrée guitare configurée sur '{name}' ({channels}ch, format={input_format:?})"
                    );
                    *guitar.input_device_name.lock() = Some(name);
                    return Some(stream);
                }
                Err(e) => log::debug!("Config entrée refusée sur '{name}' ({channels}ch): {e}"),
            }
        }
    }
    None
}

/// Convertit un bloc entrelacé en mono selon le canal sélectionné et l'écrit dans le ring buffer.
///
/// `channel_mode` : 0 = Line In 1 (AUX0/gauche), 1 = Line In 2 (AUX1/droite), 2+ = mix stéréo.
fn push_input_frames(
    ring: &crate::guitar::SpscRing,
    dropped: &std::sync::atomic::AtomicU64,
    data: &[f32],
    channels: usize,
    channel_mode: u32,
) {
    let channels = channels.max(1);
    let frames = data.len() / channels;
    if frames == 0 {
        return;
    }
    let mut mono = [0.0f32; 512];
    let mut base = 0usize;
    while base < frames {
        let n = (frames - base).min(mono.len());
        for i in 0..n {
            mono[i] = match channel_mode {
                // Line In 1 : AUX0 = canal 0 (gauche/première prise TRS)
                crate::guitar::INPUT_CHANNEL_LINE1 => data[(base + i) * channels],
                // Line In 2 : AUX1 = canal 1 (droite/deuxième prise TRS)
                crate::guitar::INPUT_CHANNEL_LINE2 => {
                    if channels >= 2 {
                        data[(base + i) * channels + 1]
                    } else {
                        data[(base + i) * channels]
                    }
                }
                // Stéréo : moyenne de tous les canaux disponibles
                crate::guitar::INPUT_CHANNEL_STEREO => {
                    let mut sum = 0.0;
                    for ch in 0..channels {
                        sum += data[(base + i) * channels + ch];
                    }
                    sum / channels as f32
                }
                _ => {
                    let mut sum = 0.0;
                    for ch in 0..channels {
                        sum += data[(base + i) * channels + ch];
                    }
                    sum / channels as f32
                }
            };
        }
        let written = ring.write(&mono[..n]);
        if written < n {
            dropped.fetch_add((n - written) as u64, Ordering::Relaxed);
        }
        base += n;
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
    guitar: Arc<GuitarRuntime>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let sr = cfg.sample_rate.0;
    let mut st_err = event_logger(stats.clone());
    let st_cb = stats;
    let mut guitar_proc = GuitarProcessor::new(sr, guitar);
    guitar_proc.set_latency_ms(guitar_latency_ms(cfg));
    device.build_output_stream(
        cfg,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            audio_callback_f32(
                data,
                &engine,
                sr,
                channels,
                hp_dup,
                &st_cb,
                &mut guitar_proc,
            );
        },
        move |err| {
            st_err(err);
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
    guitar: Arc<GuitarRuntime>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let sr = cfg.sample_rate.0;
    let mut st_err = event_logger(stats.clone());
    let st_cb = stats;
    let mut guitar_proc = GuitarProcessor::new(sr, guitar);
    guitar_proc.set_latency_ms(guitar_latency_ms(cfg));
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
                &mut guitar_proc,
            );
            for (o, &s) in data.iter_mut().zip(tmp.iter()) {
                *o = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            }
        },
        move |err| {
            st_err(err);
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
    guitar: Arc<GuitarRuntime>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let sr = cfg.sample_rate.0;
    let mut st_err = event_logger(stats.clone());
    let st_cb = stats;
    let mut guitar_proc = GuitarProcessor::new(sr, guitar);
    guitar_proc.set_latency_ms(guitar_latency_ms(cfg));
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
                &mut guitar_proc,
            );
            for (o, &s) in data.iter_mut().zip(tmp.iter()) {
                *o = ((s.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16;
            }
        },
        move |err| {
            st_err(err);
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
        if spins.is_multiple_of(128) {
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
    guitar: &mut GuitarProcessor,
) {
    render_audio_block(data, engine, sample_rate, channels, hp_dup, stats, guitar);
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
    guitar: &mut GuitarProcessor,
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

    // Le moteur n'est plus nécessaire : on relâche son verrou avant de
    // traiter la guitare pour ne pas le bloquer pendant le DSP de l'ampli.
    drop(eng);

    // Monitoring : ampli guitare sur l'entrée de la MK3, mixé dans la sortie.
    guitar.process_output(out, channels, sample_rate, hp_vol);

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

/// Liste les périphériques d'entrée (pour le choix du jack guitare).
pub fn list_input_devices() -> Vec<AudioDeviceInfo> {
    let host = cpal::default_host();
    let default_name = host.default_input_device().and_then(|d| d.name().ok());
    let mut list = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for dev in devices {
            if let Ok(name) = dev.name() {
                let lower = name.to_lowercase();
                let is_mk3 = ["maschine", "mk3", "usb audio", "native"]
                    .iter()
                    .any(|kw| lower.contains(*kw));
                let is_default = default_name.as_deref() == Some(name.as_str());
                let (channels, rate) = dev
                    .default_input_config()
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

fn find_mk3_input_device(host: &cpal::Host) -> Option<cpal::Device> {
    let keywords = ["maschine", "mk3", "usb audio", "native"];
    for dev in host.input_devices().ok()? {
        if let Ok(n) = dev.name() {
            let lower = n.to_lowercase();
            if keywords.iter().any(|kw| lower.contains(*kw)) {
                log::info!("MK3 entrance candidate: {n}");
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

/// Détecte et configure automatiquement les tables de mixage Pioneer DJM connectées (ex: DJM-250MK2).
/// Sur Linux/ALSA, la sortie USB 1-2 peut par défaut être routée vers 'Aux' plutôt que 'Ch1',
/// ce qui rend le son inaudible sur la voie 1. Cette fonction s'assure que Output 1 -> Ch1 et Output 3 -> Aux.
pub fn auto_configure_djm_routing() {
    let Ok(cards) = std::fs::read_to_string("/proc/asound/cards") else {
        return;
    };
    for line in cards.lines() {
        if line.contains("DJM") {
            if let Some(card_id) = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.strip_prefix("[").and_then(|s| s.strip_suffix("]")))
            {
                let check = std::process::Command::new("amixer")
                    .args(["-c", card_id, "sget", "Output 1"])
                    .output();
                if let Ok(out) = check {
                    let out_str = String::from_utf8_lossy(&out.stdout);
                    if out_str.contains("Item0: 'Aux'") {
                        log::info!(
                            "Pioneer DJM détectée ({card_id}) avec Output 1 sur Aux — routage automatique vers Ch1"
                        );
                        let _ = std::process::Command::new("amixer")
                            .args(["-c", card_id, "sset", "Output 1", "Ch1"])
                            .output();
                        let _ = std::process::Command::new("amixer")
                            .args(["-c", card_id, "sset", "Output 3", "Aux"])
                            .output();
                    }
                }
            }
        }
    }
}

/// Détecte et configure automatiquement la source d'entrée audio de la Maschine MK3 sous PipeWire.
/// Si PipeWire est actif et que la MK3 est connectée en mode pro-audio, s'assure que
/// la source d'entrée de la MK3 (prises Line In 1/2 TRS) est configurée comme source par défaut
/// ou prête à être capturée par le flux guitare.
pub fn auto_configure_mk3_input() {
    let Ok(output) = std::process::Command::new("wpctl").arg("status").output() else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut in_sources = false;
    for line in text.lines() {
        if line.contains("Sources:") {
            in_sources = true;
            continue;
        }
        if in_sources
            && (line.contains("Filters:")
                || line.contains("Streams:")
                || line.contains("Devices:")
                || line.contains("Sinks:"))
        {
            in_sources = false;
        }
        if in_sources && line.to_lowercase().contains("maschine") {
            let clean = line.replace(['│', '*'], "");
            let trimmed = clean.trim();
            if let Some(node_str) = trimmed.split('.').next() {
                let node_id = node_str.trim();
                if node_id.chars().all(|c| c.is_ascii_digit()) && !node_id.is_empty() {
                    log::info!(
                        "Maschine MK3 source audio détectée (node {node_id}) — définition comme source par défaut wpctl"
                    );
                    let _ = std::process::Command::new("wpctl")
                        .args(["set-default", node_id])
                        .output();
                    break;
                }
            }
        }
    }
}
