//! CPAL audio backend (fallback when JACK unavailable)

#![cfg(feature = "cpal")]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{Receiver, Sender};
use log::{info, warn, error, debug};

use maschine_dsp::{GuitarChain, AudioContext, ParamId};

use crate::{ParamsBridge, GuiToAudio, AudioToGui, AppConfig};

/// Run audio thread using CPAL
pub fn run_audio_thread(
    config: AppConfig,
    chain_arc: Arc<parking_lot::Mutex<GuitarChain>>,
    params_bridge: Arc<ParamsBridge>,
    audio_to_gui_tx: Sender<AudioToGui>,
    gui_to_audio_rx: Receiver<GuiToAudio>,
    running: Arc<AtomicBool>,
) -> Result<()> {
    let host = cpal::default_host();
    info!("CPAL host: {:?}", host.id());

    // Find input device
    let input_device = if let Some(name) = &config.audio.input_device {
        find_device_by_name(&host, name, true)
            .with_context(|| format!("Input device '{}' not found", name))?
    } else {
        host.default_input_device()
            .context("No default input device")?
    };
    let input_name = input_device.name().unwrap_or_else(|_| "unknown".to_string());
    info!("Input device: {}", input_name);

    // Find output device
    let output_device = if let Some(name) = &config.audio.output_device {
        find_device_by_name(&host, name, false)
            .with_context(|| format!("Output device '{}' not found", name))?
    } else {
        host.default_output_device()
            .context("No default output device")?
    };
    let output_name = output_device.name().unwrap_or_else(|_| "unknown".to_string());
    info!("Output device: {}", output_name);

    // Configure streams
    let sample_rate = config.audio.sample_rate;
    let buffer_size = config.audio.buffer_size;

    // Shared state for duplex processing
    let chain = chain_arc;
    let params_bridge_for_input = params_bridge.clone();
    let params_bridge_for_main = params_bridge;
    let params = params_bridge_for_input.atomic_params();
    let frame_count = Arc::new(AtomicU64::new(0));
    let running_clone = running.clone();
    let audio_to_gui_tx = audio_to_gui_tx.clone();
    let audio_to_gui_tx_for_devices = audio_to_gui_tx.clone();
    
    // Ring buffer for passing processed audio from input to output callback
    // Using a simple mutex-protected buffer for now
    let processed_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    let processed_buffer_out = processed_buffer.clone();

    // Build input stream
    let input_stream = input_device.build_input_stream(
        &cpal::StreamConfig {
            channels: config.audio.input_channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Fixed(buffer_size),
        },
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if !running_clone.load(Ordering::Relaxed) {
                return;
            }
            
            // Apply pending param updates
            params_bridge_for_input.apply_pending();
            
            // Process audio
            let mut chain_lock = chain.lock();
            let frames = data.len() / config.audio.input_channels as usize;
            
            // Mono input - take first channel
            let mut mono_in = vec![0.0f32; frames];
            for (i, chunk) in data.chunks(config.audio.input_channels as usize).enumerate() {
                mono_in[i] = chunk[0];
            }
            
            let mut mono_out = vec![0.0f32; frames];
            let ctx = AudioContext {
                sample_rate: sample_rate as f32,
                bpm: 120.0,
                frame_count: frame_count.load(Ordering::Relaxed),
            };
            chain_lock.process(&mono_in, &mut mono_out, &ctx);
            
            // Store processed audio for output stream
            {
                let mut buf = processed_buffer.lock();
                buf.clear();
                buf.extend_from_slice(&mono_out);
            }
            
            // Meter
            let in_db = chain_lock.input_level_db();
            let out_db = chain_lock.output_level_db();
            let _ = audio_to_gui_tx.try_send(AudioToGui::InputLevel(in_db));
            let _ = audio_to_gui_tx.try_send(AudioToGui::OutputLevel(out_db));
            
            frame_count.fetch_add(frames as u64, Ordering::Relaxed);
        },
        |e| error!("Input stream error: {}", e),
        None,
    ).context("Failed to build input stream")?;

    // Build output stream
    let output_stream = output_device.build_output_stream(
        &cpal::StreamConfig {
            channels: config.audio.output_channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Fixed(buffer_size),
        },
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let frames = data.len() / config.audio.output_channels as usize;
            
            // Read processed audio from ring buffer
            let processed = {
                let buf = processed_buffer_out.lock();
                buf.clone()
            };
            
            // Fill output (stereo)
            for i in 0..frames {
                let sample = if i < processed.len() { processed[i] } else { 0.0 };
                data[i * 2] = sample;     // Left
                if i * 2 + 1 < data.len() {
                    data[i * 2 + 1] = sample; // Right
                }
            }
        },
        |e| error!("Output stream error: {}", e),
        None,
    ).context("Failed to build output stream")?;

    // Start streams
    input_stream.play().context("Failed to start input stream")?;
    output_stream.play().context("Failed to start output stream")?;

    info!("Audio streams started: {}Hz / {} frames", sample_rate, buffer_size);

    // Send device list to GUI (use pre-cloned sender)
    let inputs = list_input_devices(&host);
    let outputs = list_output_devices(&host);
    let _ = audio_to_gui_tx_for_devices.try_send(AudioToGui::DeviceList(inputs, outputs));

    // Handle GUI messages
    while running.load(Ordering::Relaxed) {
        while let Ok(msg) = gui_to_audio_rx.try_recv() {
            match msg {
                GuiToAudio::SetParam(id, value) => {
                    params_bridge_for_main.set_param(id, value);
                }
                GuiToAudio::SetBypass(bypass) => {
                    params_bridge_for_main.set_bypass(bypass);
                }
                GuiToAudio::Quit => {
                    running.store(false, Ordering::Relaxed);
                    break;
                }
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    drop(input_stream);
    drop(output_stream);
    info!("Audio thread stopped");
    Ok(())
}

fn find_device_by_name(host: &cpal::Host, name: &str, input: bool) -> Option<cpal::Device> {
    let devices = if input { host.input_devices().ok()? } else { host.output_devices().ok()? };
    for dev in devices {
        if let Ok(n) = dev.name() {
            if n.to_lowercase().contains(&name.to_lowercase()) {
                return Some(dev);
            }
        }
    }
    None
}

fn list_input_devices(host: &cpal::Host) -> Vec<String> {
    host.input_devices()
        .map(|devs| devs.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

fn list_output_devices(host: &cpal::Host) -> Vec<String> {
    host.output_devices()
        .map(|devs| devs.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

use parking_lot::Mutex;