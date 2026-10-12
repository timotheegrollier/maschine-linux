//! Paramètres de l'application persistant dans ~/.config/maschine-linux/settings.json

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Périphérique de sortie audio choisi (None = défaut système / PC)
    pub audio_device: Option<String>,
    /// Volume Master (0.0 .. 1.0)
    #[serde(default = "default_volume")]
    pub master_volume: f32,
    /// Mode casque (ex: "duplicate")
    #[serde(default = "default_headphone_mode")]
    pub headphone_mode: String,
}

fn default_volume() -> f32 {
    0.85
}

fn default_headphone_mode() -> String {
    "duplicate".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            audio_device: None,
            master_volume: default_volume(),
            headphone_mode: default_headphone_mode(),
        }
    }
}

impl AppSettings {
    pub fn config_file() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                PathBuf::from(home).join(".config")
            });
        base.join("maschine-linux").join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::config_file();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<Self>(&content) {
                return settings;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        log::info!("Paramètres enregistrés dans {:?}", path);
        Ok(())
    }
}
