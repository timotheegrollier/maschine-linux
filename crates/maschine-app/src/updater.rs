//! Système de mise à jour automatique in-app depuis les releases GitHub.
//!
//! Permet de :
//! 1. Vérifier la présence d'une nouvelle version sur GitHub Releases (tag semver) ;
//! 2. Détecter l'environnement d'exécution (RPM Fedora/Nobara, AppImage universel, DEB, binaire local) ;
//! 3. Télécharger le bon paquet ou exécutable avec suivi de progression ;
//! 4. Appliquer la mise à jour (via `pkexec dnf/rpm` pour RPM, remplacement atomique pour AppImage/binaire).

use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use log::info;
use parking_lot::Mutex;
use semver::Version;
use serde::Deserialize;

pub const GITHUB_REPO: &str = "timotheegrollier/maschine-linux";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Information sur un asset attaché à une release GitHub.
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    pub content_type: Option<String>,
}

/// Information sur une release GitHub.
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub html_url: String,
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

/// Type de paquet / distribution détecté.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    /// Paquet RPM (Fedora, Nobara, RHEL, openSUSE)
    Rpm,
    /// Paquet universel AppImage
    AppImage,
    /// Paquet Debian/Ubuntu (.deb)
    Deb,
    /// Binaire autonome ou installation locale (~/.local/bin)
    Binary,
}

impl PackageKind {
    /// Détecte le type d'installation courante.
    pub fn detect() -> Self {
        if std::env::var("APPIMAGE").is_ok() {
            return Self::AppImage;
        }

        // Vérification /etc/os-release pour Fedora/Nobara
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            let lower = content.to_lowercase();
            if lower.contains("id=fedora")
                || lower.contains("id=nobara")
                || lower.contains("id_like=\"fedora\"")
                || lower.contains("id_like=fedora")
            {
                return Self::Rpm;
            }
            if lower.contains("id=debian")
                || lower.contains("id=ubuntu")
                || lower.contains("id_like=debian")
            {
                return Self::Deb;
            }
        }

        Self::Binary
    }

    /// Nom lisible
    pub fn label(&self) -> &'static str {
        match self {
            Self::Rpm => "RPM (Fedora / Nobara)",
            Self::AppImage => "AppImage Universel",
            Self::Deb => "Paquet DEB (Debian / Ubuntu)",
            Self::Binary => "Binaire standard / Local",
        }
    }
}

/// Résultat de la vérification de mise à jour.
#[derive(Debug, Clone)]
pub struct UpdateCheck {
    pub current_version: String,
    pub new_version: String,
    pub is_available: bool,
    pub release: ReleaseInfo,
    pub matched_asset: Option<ReleaseAsset>,
    pub package_kind: PackageKind,
}

/// État partagé de l'auto-updater pour l'interface graphique.
#[derive(Debug, Clone, Default)]
pub struct UpdateState {
    pub is_checking: bool,
    pub is_downloading: bool,
    pub update_available: Option<UpdateCheck>,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
    pub progress: Option<(u64, u64)>, // (downloaded, total)
    pub update_completed: bool,
    pub restart_required: bool,
}

/// Handle partagé pour l'interface GUI.
pub type SharedUpdateState = Arc<Mutex<UpdateState>>;

/// Vérifie la présence d'une nouvelle release sur GitHub.
pub fn check_for_updates() -> Result<Option<UpdateCheck>> {
    let repo = std::env::var("MASCHINE_UPDATE_REPO").unwrap_or_else(|_| GITHUB_REPO.to_string());
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let user_agent = format!("maschine-linux/{CURRENT_VERSION}");

    info!("Vérification des mises à jour sur GitHub: {url}...");

    let res = match ureq::get(&url)
        .header("User-Agent", &user_agent)
        .header("Accept", "application/vnd.github.v3+json")
        .call()
    {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(404)) => {
            info!("Aucune release publiée sur GitHub pour l'instant (dépôt ou release 404).");
            return Ok(None);
        }
        Err(e) => {
            return Err(anyhow!("Impossible de joindre GitHub API ({url}): {e}"));
        }
    };

    let release: ReleaseInfo = res
        .into_body()
        .read_json()
        .with_context(|| "Impossible de décoder la réponse JSON de GitHub Releases")?;

    let remote_tag = release.tag_name.trim().trim_start_matches('v').to_string();
    let remote_ver = Version::parse(&remote_tag)
        .with_context(|| format!("Version distante '{remote_tag}' invalide"))?;
    let local_ver = Version::parse(CURRENT_VERSION)
        .with_context(|| format!("Version locale '{CURRENT_VERSION}' invalide"))?;

    let is_available = remote_ver > local_ver;
    let package_kind = PackageKind::detect();

    // Trouve l'asset correspondant au système
    let matched_asset = find_best_asset(&release.assets, package_kind);

    let check = UpdateCheck {
        current_version: CURRENT_VERSION.to_string(),
        new_version: remote_tag.clone(),
        is_available,
        release,
        matched_asset,
        package_kind,
    };

    if is_available {
        info!(
            "🚀 Nouvelle mise à jour disponible: v{} -> v{} ({})",
            CURRENT_VERSION,
            remote_tag,
            package_kind.label()
        );
        Ok(Some(check))
    } else {
        info!("✓ L'application est à jour (v{}).", CURRENT_VERSION);
        Ok(None)
    }
}

/// Sélectionne l'asset idéal selon le type de paquet et l'architecture.
fn find_best_asset(assets: &[ReleaseAsset], kind: PackageKind) -> Option<ReleaseAsset> {
    match kind {
        PackageKind::Rpm => assets
            .iter()
            .find(|a| a.name.ends_with(".rpm") && a.name.contains("x86_64"))
            .or_else(|| assets.iter().find(|a| a.name.ends_with(".rpm")))
            .cloned(),
        PackageKind::AppImage => assets
            .iter()
            .find(|a| a.name.ends_with(".AppImage") && a.name.contains("x86_64"))
            .or_else(|| assets.iter().find(|a| a.name.ends_with(".AppImage")))
            .cloned(),
        PackageKind::Deb => assets
            .iter()
            .find(|a| {
                a.name.ends_with(".deb") && (a.name.contains("amd64") || a.name.contains("x86_64"))
            })
            .or_else(|| assets.iter().find(|a| a.name.ends_with(".deb")))
            .cloned(),
        PackageKind::Binary => assets
            .iter()
            .find(|a| a.name.contains("linux-x86_64.tar.gz") || a.name.contains("x86_64.tar.gz"))
            .or_else(|| assets.iter().find(|a| a.name.ends_with(".tar.gz")))
            .cloned(),
    }
}

/// Télécharge un asset avec rapport de progression.
pub fn download_asset<F>(asset: &ReleaseAsset, target_path: &Path, on_progress: F) -> Result<()>
where
    F: Fn(u64, u64),
{
    let user_agent = format!("maschine-linux/{CURRENT_VERSION}");
    info!(
        "Téléchargement de '{}' ({:.1} Mo)...",
        asset.name,
        asset.size as f64 / (1024.0 * 1024.0)
    );

    let res = ureq::get(&asset.browser_download_url)
        .header("User-Agent", &user_agent)
        .header("Accept", "application/octet-stream")
        .call()
        .with_context(|| {
            format!(
                "Échec du téléchargement depuis {}",
                asset.browser_download_url
            )
        })?;

    let total_size = asset.size;
    let mut reader = res.into_body().into_reader();
    let mut out_file = File::create(target_path)
        .with_context(|| format!("Impossible de créer le fichier {}", target_path.display()))?;

    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 16384];

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out_file.write_all(&buf[..n])?;
        downloaded += n as u64;
        on_progress(downloaded, total_size);
    }

    out_file.flush()?;
    Ok(())
}

/// Applique la mise à jour téléchargée selon l'environnement.
pub fn apply_update(check: &UpdateCheck, downloaded_file: &Path) -> Result<String> {
    match check.package_kind {
        PackageKind::AppImage => {
            let appimage_path = std::env::var("APPIMAGE")
                .map(PathBuf::from)
                .context("Variable APPIMAGE non définie")?;

            // Rendre le fichier téléchargé exécutable
            let mut perms = std::fs::metadata(downloaded_file)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(downloaded_file, perms)?;

            // Remplacement atomique de l'AppImage
            std::fs::rename(downloaded_file, &appimage_path)
                .or_else(|_| {
                    // Si rename direct échoue (ex. cross-device), copie puis suppression
                    std::fs::copy(downloaded_file, &appimage_path)?;
                    let _ = std::fs::remove_file(downloaded_file);
                    Ok::<(), std::io::Error>(())
                })
                .context("Impossible de remplacer l'AppImage courante")?;

            Ok(format!(
                "✓ AppImage mise à jour ({}) ! Veuillez redémarrer l'application.",
                appimage_path.display()
            ))
        }

        PackageKind::Rpm => {
            info!("Installation du paquet RPM via pkexec...");
            // Commande privilégiée polkit
            let dnf_status = std::process::Command::new("pkexec")
                .args(["dnf", "install", "-y", "--nogpgcheck"])
                .arg(downloaded_file)
                .status();

            match dnf_status {
                Ok(s) if s.success() => Ok(
                    "✓ Paquet RPM installé avec succès via DNF ! Redémarrez l'application."
                        .to_string(),
                ),
                _ => {
                    // Repli sur rpm -Uvh
                    let rpm_status = std::process::Command::new("pkexec")
                        .args(["rpm", "-Uvh", "--replacepkgs"])
                        .arg(downloaded_file)
                        .status()
                        .context("Échec d'exécution de pkexec rpm")?;

                    if rpm_status.success() {
                        Ok(
                            "✓ Paquet RPM mis à niveau avec succès ! Redémarrez l'application."
                                .to_string(),
                        )
                    } else {
                        Err(anyhow!(
                            "Échec de l'installation RPM. Vous pouvez installer manuellement : sudo dnf install {}",
                            downloaded_file.display()
                        ))
                    }
                }
            }
        }

        PackageKind::Deb => {
            info!("Installation du paquet DEB via pkexec...");
            let status = std::process::Command::new("pkexec")
                .args(["dpkg", "-i"])
                .arg(downloaded_file)
                .status()
                .context("Échec d'exécution de pkexec dpkg")?;

            if status.success() {
                Ok("✓ Paquet DEB installé avec succès ! Redémarrez l'application.".to_string())
            } else {
                Err(anyhow!(
                    "Échec de l'installation DEB. Vous pouvez installer manuellement : sudo dpkg -i {}",
                    downloaded_file.display()
                ))
            }
        }

        PackageKind::Binary => {
            let current_exe =
                std::env::current_exe().context("Impossible de déterminer le binaire courant")?;

            // Si c'est une archive tar.gz, on extrait le binaire
            if downloaded_file.to_string_lossy().ends_with(".tar.gz") {
                let temp_dir =
                    std::env::temp_dir().join(format!("maschine_update_{}", std::process::id()));
                std::fs::create_dir_all(&temp_dir)?;

                let tar_status = std::process::Command::new("tar")
                    .args([
                        "-xzf",
                        downloaded_file.to_str().unwrap(),
                        "-C",
                        temp_dir.to_str().unwrap(),
                    ])
                    .status()
                    .context("Échec de décompression de l'archive tar.gz")?;

                if !tar_status.success() {
                    return Err(anyhow!("Erreur lors de l'extraction de l'archive"));
                }

                // Trouve le binaire maschine-linux dans l'archive extraite
                let mut found_bin = None;
                for entry in walkdir_simple(&temp_dir)? {
                    if entry.file_name() == Some(std::ffi::OsStr::new("maschine-linux")) {
                        found_bin = Some(entry);
                        break;
                    }
                }

                let bin_src = found_bin.ok_or_else(|| {
                    anyhow!("Binaire 'maschine-linux' introuvable dans l'archive")
                })?;

                let mut perms = std::fs::metadata(&bin_src)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&bin_src, perms)?;

                // Remplacement atomique de l'exécutable
                let temp_dest = current_exe.with_extension("tmp_update");
                std::fs::copy(&bin_src, &temp_dest)?;
                std::fs::rename(&temp_dest, &current_exe)
                    .or_else(|_| {
                        std::fs::copy(&bin_src, &current_exe)?;
                        Ok::<(), std::io::Error>(())
                    })
                    .context("Impossible de remplacer le binaire courant")?;

                let _ = std::fs::remove_dir_all(&temp_dir);
            } else {
                // Remplacement direct
                let mut perms = std::fs::metadata(downloaded_file)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(downloaded_file, perms)?;

                std::fs::rename(downloaded_file, &current_exe)
                    .or_else(|_| {
                        std::fs::copy(downloaded_file, &current_exe)?;
                        Ok::<(), std::io::Error>(())
                    })
                    .context("Impossible de remplacer le binaire courant")?;
            }

            Ok(format!(
                "✓ Binaire mis à jour avec succès dans {} ! Redémarrez l'application.",
                current_exe.display()
            ))
        }
    }
}

/// Helper simple pour parcourir récursivement un dossier (sans dépendance externe).
fn walkdir_simple(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                files.extend(walkdir_simple(&path)?);
            } else {
                files.push(path);
            }
        }
    }
    Ok(files)
}

/// Lance une vérification asynchrone pour la GUI.
pub fn spawn_background_update_check(state: SharedUpdateState) {
    std::thread::Builder::new()
        .name("maschine-updater-check".to_string())
        .spawn(move || {
            {
                let mut s = state.lock();
                s.is_checking = true;
                s.status_message = Some("Recherche de mises à jour...".to_string());
                s.error_message = None;
            }

            match check_for_updates() {
                Ok(Some(check)) => {
                    let mut s = state.lock();
                    s.is_checking = false;
                    s.status_message =
                        Some(format!("Mise à jour v{} disponible !", check.new_version));
                    s.update_available = Some(check);
                }
                Ok(None) => {
                    let mut s = state.lock();
                    s.is_checking = false;
                    s.status_message =
                        Some(format!("Maschine Linux v{CURRENT_VERSION} est à jour."));
                    s.update_available = None;
                }
                Err(e) => {
                    let mut s = state.lock();
                    s.is_checking = false;
                    s.error_message = Some(format!("Erreur vérification : {e}"));
                }
            }
        })
        .ok();
}

/// Lance le téléchargement et l'installation de la mise à jour en arrière-plan.
pub fn spawn_background_update_apply(state: SharedUpdateState) {
    let check_opt = {
        let s = state.lock();
        s.update_available.clone()
    };

    let Some(check) = check_opt else { return };
    let Some(asset) = check.matched_asset.clone() else {
        let mut s = state.lock();
        s.error_message = Some("Aucun paquet compatible trouvé pour ce système.".to_string());
        return;
    };

    std::thread::Builder::new()
        .name("maschine-updater-download".to_string())
        .spawn(move || {
            {
                let mut s = state.lock();
                s.is_downloading = true;
                s.status_message = Some(format!("Téléchargement de {}...", asset.name));
                s.progress = Some((0, asset.size));
                s.error_message = None;
            }

            let temp_file = std::env::temp_dir().join(&asset.name);

            let state_prog = state.clone();
            let res = download_asset(&asset, &temp_file, move |downloaded, total| {
                let mut s = state_prog.lock();
                s.progress = Some((downloaded, total));
            });

            if let Err(e) = res {
                let mut s = state.lock();
                s.is_downloading = false;
                s.error_message = Some(format!("Erreur téléchargement : {e}"));
                return;
            }

            {
                let mut s = state.lock();
                s.status_message = Some("Installation de la mise à jour...".to_string());
            }

            match apply_update(&check, &temp_file) {
                Ok(msg) => {
                    let mut s = state.lock();
                    s.is_downloading = false;
                    s.update_completed = true;
                    s.restart_required = true;
                    s.status_message = Some(msg);
                }
                Err(e) => {
                    let mut s = state.lock();
                    s.is_downloading = false;
                    s.error_message = Some(format!("Erreur installation : {e}"));
                }
            }
        })
        .ok();
}
