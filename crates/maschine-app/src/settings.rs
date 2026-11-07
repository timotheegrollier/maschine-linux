//! Paramètres de l'application persistant dans ~/.config/maschine-linux/settings.json

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    /// Dernier projet ouvert/sauvegardé explicitement.
    #[serde(default)]
    pub current_project_path: Option<String>,
    /// Projets récents (chemins absolus), du plus récent au plus ancien.
    #[serde(default)]
    pub recent_projects: Vec<String>,
    /// Répertoire de projets scanné par le navigateur FILE de la MK3/GUI.
    #[serde(default = "default_projects_dir")]
    pub projects_dir: Option<String>,
}

fn default_volume() -> f32 {
    0.85
}

fn default_headphone_mode() -> String {
    "duplicate".to_string()
}

fn default_projects_dir() -> Option<String> {
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".local/share")
        });
    Some(
        base.join("maschine-linux")
            .join("projects")
            .to_string_lossy()
            .to_string(),
    )
}

impl AppSettings {
    /// Ajoute un projet en tête des récents (dédupliqué, max 24).
    pub fn push_recent_project(&mut self, path: &str) {
        let path = path.trim().to_string();
        if path.is_empty() {
            return;
        }
        self.recent_projects.retain(|p| p != &path);
        self.recent_projects.insert(0, path.clone());
        self.recent_projects.truncate(24);
        self.current_project_path = Some(path);
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            audio_device: None,
            master_volume: default_volume(),
            headphone_mode: default_headphone_mode(),
            current_project_path: None,
            recent_projects: Vec::new(),
            projects_dir: default_projects_dir(),
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
