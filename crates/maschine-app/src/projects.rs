//! Gestion des fichiers projet : scan, chemins, sauvegarde rapide/courante.
//!
//! Format canonique : JSON `.maschine-linux.json` (lisible GUI/MK3).
//! Sauvegarde rapide : JSON compact + écriture atomique (voir `Project`).

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use maschine_engine::ProjectFileEntry;

use crate::settings::AppSettings;

/// Répertoire de données `~/.local/share/maschine-linux`.
pub fn data_dir() -> PathBuf {
    std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".local/share")
        })
        .join("maschine-linux")
}

/// Répertoire des projets utilisateurs.
pub fn projects_dir(settings: &AppSettings) -> PathBuf {
    settings
        .projects_dir
        .as_ref()
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| data_dir().join("projects"))
}

/// Fichier autosave (compatibilité historique).
pub fn autosave_path() -> PathBuf {
    data_dir().join("autosave.json")
}

/// Normalise un nom de fichier projet.
pub fn sanitize_project_filename(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    out = out.trim().trim_matches('.').to_string();
    if out.is_empty() {
        out = "Untitled".to_string();
    }
    if out.len() > 80 {
        out.truncate(80);
    }
    if !out.ends_with(".maschine-linux.json") && !out.ends_with(".json") {
        out.push_str(".maschine-linux.json");
    }
    out
}

/// Résout le chemin de sauvegarde du projet courant.
/// Priorité : chemin courant explicite > autosave (historique).
pub fn resolve_current_save_path(
    settings: &AppSettings,
    snapshot_path: Option<&str>,
    project_name: &str,
) -> PathBuf {
    if let Some(p) = snapshot_path.filter(|s| !s.trim().is_empty()) {
        return PathBuf::from(p);
    }
    if let Some(p) = settings
        .current_project_path
        .as_ref()
        .filter(|s| !s.trim().is_empty())
    {
        return PathBuf::from(p);
    }
    // Nouveau projet : l'enregistrer dans le répertoire projets (pas seulement autosave).
    let dir = projects_dir(settings);
    dir.join(sanitize_project_filename(project_name))
}

/// Sauvegarde le projet (pretty par défaut, compact si `quick`).
pub fn save_project(
    project: &maschine_engine::Project,
    path: &Path,
    quick: bool,
) -> anyhow::Result<()> {
    project.save_json_with(path, !quick)?;
    Ok(())
}

/// Scanne les projets disponibles (répertoire projets, récents, autosave).
pub fn scan_projects(settings: &AppSettings) -> Vec<ProjectFileEntry> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let dir = projects_dir(settings);
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if is_project_file(&p) {
                paths.push(p);
            }
        }
    }
    for recent in &settings.recent_projects {
        let p = PathBuf::from(recent);
        if p.is_file() && is_project_file(&p) {
            paths.push(p);
        }
    }
    let autosave = autosave_path();
    if autosave.is_file() {
        paths.push(autosave);
    }

    // Déduplique en conservant le premier (ordre: dir/récents/autosave), puis trie récents.
    let mut uniq: Vec<PathBuf> = Vec::new();
    for p in paths {
        if let Ok(canon) = p.canonicalize() {
            if !uniq
                .iter()
                .any(|u| u.canonicalize().ok().as_ref() == Some(&canon))
            {
                uniq.push(p);
            }
        } else if !uniq.contains(&p) {
            uniq.push(p);
        }
    }

    uniq.sort_by_key(|a| std::cmp::Reverse(modified_secs(a)));
    uniq.truncate(64);

    uniq.into_iter()
        .map(|p| {
            let name = project_display_name(&p);
            let meta = project_meta(&p);
            ProjectFileEntry {
                name,
                path: p.to_string_lossy().to_string(),
                meta,
            }
        })
        .collect()
}

fn is_project_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    name.ends_with(".maschine-linux.json") || name == "autosave.json" || name.ends_with(".json")
}

fn modified_secs(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn project_meta(path: &Path) -> String {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size >= 1024 * 1024 {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KB", (size / 1024).max(1))
    }
}

fn project_display_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let stem = stem.strip_suffix(".maschine-linux").unwrap_or(stem);
    if stem == "autosave" {
        "Autosave".to_string()
    } else {
        stem.to_string()
    }
}

/// Met à jour les settings après une sauvegarde/chargement réussi.
pub fn remember_project(settings: &mut AppSettings, path: &Path) {
    settings.push_recent_project(&path.to_string_lossy());
    if let Some(parent) = path.parent() {
        settings.projects_dir = Some(parent.to_string_lossy().to_string());
    }
    let _ = settings.save();
}
