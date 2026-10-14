//! Application configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub audio: AudioConfig,
    pub gui: GuiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    #[cfg(feature = "jack")]
    pub backend: String, // "jack" or "cpal"
    #[cfg(not(feature = "jack"))]
    pub backend: String, // "cpal"
    pub sample_rate: u32,             // 48000
    pub buffer_size: u32,             // 128
    pub input_device: Option<String>, // device name or index
    pub output_device: Option<String>,
    pub input_channels: u16,  // 1 (mono guitar)
    pub output_channels: u16, // 2 (stereo out)
    pub auto_connect: bool,   // auto-connect JACK ports
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiConfig {
    pub window_width: u32,
    pub window_height: u32,
    pub theme: String, // "dark" or "light"
    pub show_vu_meters: bool,
    pub show_tuner: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            audio: AudioConfig {
                #[cfg(feature = "jack")]
                backend: "jack".to_string(),
                #[cfg(not(feature = "jack"))]
                backend: "cpal".to_string(),
                sample_rate: 48000,
                buffer_size: 128,
                input_device: None,
                output_device: None,
                input_channels: 1,
                output_channels: 2,
                auto_connect: true,
            },
            gui: GuiConfig {
                window_width: 1000,
                window_height: 700,
                theme: "dark".to_string(),
                show_vu_meters: true,
                show_tuner: true,
            },
        }
    }
}

impl AppConfig {
    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("maschine-guitar")
            .join("config.json")
    }

    pub fn load_or_default() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => {
                        log::info!("Loaded config from {:?}", path);
                        return config;
                    }
                    Err(e) => log::warn!("Failed to parse config: {}", e),
                },
                Err(e) => log::warn!("Failed to read config: {}", e),
            }
        }
        Self::default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        log::info!("Saved config to {:?}", path);
        Ok(())
    }
}
