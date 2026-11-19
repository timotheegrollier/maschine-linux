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
        Self::config_file_in(
            std::env::var("XDG_CONFIG_HOME").ok().map(PathBuf::from),
            std::env::var("HOME").ok().map(PathBuf::from),
        )
    }

    /// Résolution pure du chemin (sans lecture d'environnement), testable.
    fn config_file_in(config_home: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
        let base = config_home
            .or_else(|| home.map(|h| h.join(".config")))
            .unwrap_or_else(|| PathBuf::from("/tmp/.config"));
        base.join("maschine-linux").join("settings.json")
    }

    pub fn load() -> Self {
        Self::load_from(&Self::config_file())
    }

    /// Lit `path` ; fichier absent ou JSON invalide → valeurs par défaut.
    /// Un fichier présent mais illisible/corrompu est signalé dans les logs.
    fn load_from(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<Self>(&content) {
                Ok(settings) => settings,
                Err(err) => {
                    log::warn!(
                        "Réglages corrompus dans {}: {err} — réinitialisation aux valeurs par défaut",
                        path.display()
                    );
                    Self::default()
                }
            },
            Err(err) => {
                // Premier lancement : le fichier n'existe pas encore, ce n'est
                // pas une anomalie. Les autres erreurs (droits, I/O) le sont.
                if err.kind() != std::io::ErrorKind::NotFound {
                    log::warn!(
                        "Lecture des réglages impossible ({}): {err} — valeurs par défaut",
                        path.display()
                    );
                }
                Self::default()
            }
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_file();
        self.save_to(&path)?;
        log::info!("Paramètres enregistrés dans {:?}", path);
        Ok(())
    }

    /// Écrit les paramètres dans `path` de façon atomique (fichier temporaire
    /// puis renommage) : un arrêt brutal ne doit pas laisser un fichier
    /// tronqué et perdre le dernier projet ou le périphérique audio mémorisé.
    fn save_to(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, content)?;
        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Mutex, Once};

    /// Capture les `log::warn!` émis par `load_from` (logger global, installé
    /// une seule fois pour tout le binaire de test).
    struct CaptureLogger;

    static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static LOGGER_INIT: Once = Once::new();

    impl log::Log for CaptureLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Warn
        }

        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                if let Ok(mut captured) = CAPTURED.lock() {
                    captured.push(format!("{}: {}", record.level(), record.args()));
                }
            }
        }

        fn flush(&self) {}
    }

    fn capture_logs() {
        LOGGER_INIT.call_once(|| {
            let _ = log::set_boxed_logger(Box::new(CaptureLogger));
            log::set_max_level(log::LevelFilter::Warn);
        });
    }

    #[test]
    fn push_recent_project_keeps_latest_first_and_deduplicates() {
        let mut settings = AppSettings::default();
        settings.push_recent_project("/tmp/a.maschine-linux.json");
        settings.push_recent_project("/tmp/b.maschine-linux.json");
        settings.push_recent_project("/tmp/a.maschine-linux.json");

        assert_eq!(
            settings.recent_projects,
            vec![
                "/tmp/a.maschine-linux.json".to_string(),
                "/tmp/b.maschine-linux.json".to_string()
            ]
        );
        assert_eq!(
            settings.current_project_path.as_deref(),
            Some("/tmp/a.maschine-linux.json")
        );
    }

    #[test]
    fn push_recent_project_ignores_blank_paths() {
        let mut settings = AppSettings::default();
        settings.push_recent_project("");
        settings.push_recent_project("   ");

        assert!(settings.recent_projects.is_empty());
        assert!(settings.current_project_path.is_none());
    }

    #[test]
    fn push_recent_project_trims_paths_before_deduplicating() {
        let mut settings = AppSettings::default();
        settings.push_recent_project("  /tmp/trimmed.maschine-linux.json  ");
        settings.push_recent_project("/tmp/trimmed.maschine-linux.json");

        assert_eq!(
            settings.recent_projects,
            vec!["/tmp/trimmed.maschine-linux.json".to_string()]
        );
        assert_eq!(
            settings.current_project_path.as_deref(),
            Some("/tmp/trimmed.maschine-linux.json")
        );
    }

    #[test]
    fn push_recent_project_truncates_to_24_entries() {
        let mut settings = AppSettings::default();
        for i in 0..30 {
            settings.push_recent_project(&format!("/tmp/project-{i}.json"));
        }

        assert_eq!(settings.recent_projects.len(), 24);
        assert_eq!(
            settings.recent_projects.first().map(String::as_str),
            Some("/tmp/project-29.json")
        );
        assert_eq!(
            settings.recent_projects.last().map(String::as_str),
            Some("/tmp/project-6.json")
        );
    }

    /// Détecte root sans `unsafe` (le projet interdit `unsafe` hors RGB565).
    #[cfg(unix)]
    fn running_as_root() -> bool {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|status| {
                status
                    .lines()
                    .find_map(|line| line.strip_prefix("Uid:"))
                    .and_then(|value| value.split_whitespace().next())
                    .and_then(|uid| uid.parse::<u32>().ok())
            })
            .is_some_and(|uid| uid == 0)
    }

    #[cfg(unix)]
    #[test]
    fn save_to_read_only_directory_fails_without_touching_the_existing_file() {
        use std::os::unix::fs::PermissionsExt;

        // root ignore les permissions POSIX : le scénario n'aurait pas de sens.
        if running_as_root() {
            return;
        }

        let dir = std::env::temp_dir().join(format!(
            "maschine-settings-ro-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("création du dossier temporaire");
        let path = dir.join("settings.json");

        let mut settings = AppSettings {
            master_volume: 0.42,
            ..AppSettings::default()
        };
        settings.save_to(&path).expect("sauvegarde initiale");
        let original = std::fs::read_to_string(&path).expect("lecture initiale");

        let mut perms = std::fs::metadata(&dir).expect("metadata").permissions();
        perms.set_mode(0o555);
        std::fs::set_permissions(&dir, perms).expect("dossier en lecture seule");

        settings.master_volume = 0.10;
        let err = settings.save_to(&path);
        assert!(err.is_err(), "écrire dans un dossier RO doit échouer");

        // Le fichier valide précédent reste intact, sans temporaire résiduel.
        assert_eq!(
            std::fs::read_to_string(&path).expect("lecture après échec"),
            original
        );
        assert!(!dir.join("settings.json.tmp").exists());

        // Restaure les droits pour permettre le nettoyage.
        let mut perms = std::fs::metadata(&dir).expect("metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dir, perms).expect("droits restaurés");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "maschine-settings-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("création du dossier temporaire");
        dir
    }

    #[test]
    fn settings_survive_a_disk_round_trip() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("settings.json");
        let settings = AppSettings {
            audio_device: Some("Maschine MK3".to_string()),
            master_volume: 0.42,
            headphone_mode: "duplicate".to_string(),
            current_project_path: Some("/music/set.maschine-linux.json".to_string()),
            recent_projects: vec![
                "/music/set.maschine-linux.json".to_string(),
                "/music/old.maschine-linux.json".to_string(),
            ],
            projects_dir: Some("/music".to_string()),
        };

        settings.save_to(&path).expect("sauvegarde sur disque");
        assert!(
            !dir.join("settings.json.tmp").exists(),
            "aucun temporaire ne doit rester après un succès"
        );

        let loaded = AppSettings::load_from(&path);
        assert_eq!(loaded.audio_device.as_deref(), Some("Maschine MK3"));
        assert_eq!(loaded.master_volume, 0.42);
        assert_eq!(loaded.headphone_mode, "duplicate");
        assert_eq!(
            loaded.current_project_path.as_deref(),
            Some("/music/set.maschine-linux.json")
        );
        assert_eq!(loaded.recent_projects, settings.recent_projects);
        assert_eq!(loaded.projects_dir.as_deref(), Some("/music"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_falls_back_to_defaults_when_the_file_is_missing_or_corrupt() {
        let dir = temp_dir("fallback");
        let defaults = AppSettings::default();

        let missing = AppSettings::load_from(&dir.join("missing.json"));
        assert_eq!(missing.master_volume, defaults.master_volume);
        assert!(missing.audio_device.is_none());
        assert!(missing.current_project_path.is_none());

        let corrupt_path = dir.join("corrupt.json");
        std::fs::write(&corrupt_path, "{ \"master_volume\": ").expect("écriture corrompue");
        let corrupt = AppSettings::load_from(&corrupt_path);
        assert_eq!(corrupt.master_volume, defaults.master_volume);
        assert!(corrupt.recent_projects.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_settings_emits_a_warning() {
        capture_logs();
        let dir = temp_dir("warn");
        let corrupt_path = dir.join("corrupt.json");
        std::fs::write(&corrupt_path, "{ \"master_volume\": ").expect("écriture corrompue");

        let loaded = AppSettings::load_from(&corrupt_path);
        assert_eq!(loaded.master_volume, AppSettings::default().master_volume);

        let messages = CAPTURED.lock().expect("capture verrouillée").clone();
        assert!(
            messages.iter().any(|message| {
                message.contains("Réglages corrompus") && message.contains("corrupt.json")
            }),
            "avertissement de corruption attendu, messages capturés: {messages:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_file_prefers_xdg_config_home_then_home() {
        assert_eq!(
            AppSettings::config_file_in(
                Some(PathBuf::from("/xdg")),
                Some(PathBuf::from("/home/timo"))
            ),
            PathBuf::from("/xdg/maschine-linux/settings.json")
        );
        assert_eq!(
            AppSettings::config_file_in(None, Some(PathBuf::from("/home/timo"))),
            PathBuf::from("/home/timo/.config/maschine-linux/settings.json")
        );
        assert_eq!(
            AppSettings::config_file_in(None, None),
            PathBuf::from("/tmp/.config/maschine-linux/settings.json")
        );
    }
}
