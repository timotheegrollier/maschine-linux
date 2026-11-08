# Guide CI/CD, Releases & Auto-Updater

Ce document détaille le fonctionnement des pipelines d'intégration continue (CI) et de publication (Release), répertorie les erreurs récurrentes rencontrées, leurs causes racines, et les mécanismes de fiabilisation mis en place pour garantir la stabilité des déploiements.

---

## 1. Vue d'ensemble de l'architecture CI/CD

Le projet utilise GitHub Actions avec deux workflows distincts :

1. **`CI` (`.github/workflows/ci.yml`)** : Déclenché sur chaque `push` et `pull_request` sur les branches `master` et `main`.
   - **Objectif** : Valider la qualité du code, l'absence de régression et l'exactitude des fichiers de distribution.
   - **Étapes clés** :
     - Validation du fichier `.desktop` (`desktop-file-validate`).
     - Vérification du formatage (`cargo fmt --all -- --check`).
     - Validation des notes de version (`./scripts/extract-release-notes.sh "${version}"`).
     - Linting strict avec warnings en erreurs (`cargo clippy --workspace --all-targets -- -D warnings`).
     - Compilation conditionnelle sans sampler (`cargo check -p maschine-engine --no-default-features`).
     - Exécution des 167 tests automatisés (`cargo test --workspace`).
     - Compilation release (`cargo build --workspace --release`).

2. **`Release` (`.github/workflows/release.yml`)** : Déclenché sur la publication de tags `v*`.
   - **Objectif** : Générer l'ensemble des paquets multi-distributions Linux et publier la release GitHub officielle.
   - **Étapes clés** :
     - Vérification de cohérence entre le tag git (`vX.Y.Z`) et `Cargo.toml`.
     - Compilation release optimisée du binaire `maschine-app`.
     - Génération du paquet RPM Fedora / Nobara (`./scripts/build-rpm.sh`).
     - Génération de l'AppImage universelle (`./scripts/build-appimage.sh`).
     - Génération du paquet DEB Debian / Ubuntu (`./scripts/build-deb.sh`).
     - Génération de l'archive tarball standalone et calcul des sommes SHA-256 (`./scripts/package-release.sh`).
     - Extraction des notes de version anglaises (`./scripts/extract-release-notes.sh`).
     - Publication GitHub Release avec tous les artefacts (`softprops/action-gh-release@v2`).

---

## 2. Analyse des Erreurs Fréquentes & Causes Racines

### Erreur 1 : Échec du contrôle de formatage (`cargo fmt --all -- --check`)
- **Symptôme** : Le job CI échoue à l'étape `Format` avec `Process completed with exit code 1` et un diff sur des fichiers Rust modifiés.
- **Cause racine** : Des modifications de code (refactorings, ajouts de tests, réindentations manuelles) ont été validées sans exécuter `cargo fmt --all` au préalable. Comme `ci.yml` exporte `RUSTFLAGS="-D warnings"`, tout écart de formattage bloque le pipeline.
- **Correction & Fiabilisation** :
  - Exécuter systématiquement `cargo fmt --all` avant tout commit ou git tag.
  - La vérification locale recommandée est :
    ```bash
    cargo fmt --all -- --check
    ```

---

### Erreur 2 : Échec d'extraction des release notes (`extract-release-notes.sh`)
- **Symptôme** : Le workflow Release ou CI échoue avec :
  ```text
  error: no release notes found for X.Y.Z in .../CHANGELOG.md
  Process completed with exit code 1.
  ```
- **Cause racine** : Le script `extract-release-notes.sh` exigeait impérativement une section `## [X.Y.Z]` dans `CHANGELOG.md`. Lorsqu'une version était incrémentée dans `Cargo.toml` sans ajouter immédiatement l'entrée correspondante dans le CHANGELOG, le script retournait un code d'erreur 1, avortant le build après la compilation.
- **Correction & Fiabilisation** :
  - Mise à jour de `extract-release-notes.sh` avec des replis automatiques (*fallbacks*) :
    1. Si `## [X.Y.Z]` est présent dans `CHANGELOG.md`, il est extrait et utilisé.
    2. Sinon, si `dist/RELEASE_NOTES.md` existe et n'est pas vide, son contenu est utilisé.
    3. Sinon, le script extrait automatiquement les commits Git depuis le tag précédent (`git log --oneline <prev_tag>..HEAD`).
    4. Ce n'est qu'en l'absence totale d'informations qu'une erreur est levée.
  - Ajout des sections `## [0.9.10]` et `## [0.9.9]` dans `CHANGELOG.md`.

---

### Erreur 3 : Auto-updater in-app ne proposant pas l'installation
- **Symptômes** :
  - Le dialogue d'auto-update affiche "Aucun paquet pré-compilé" ou n'offre pas le bouton d'installation.
  - L'application signale qu'une version est disponible mais ne permet pas l'installation en un clic.
- **Causes racines identifiées** :
  1. **Détection d'environnement trop agressive** : `PackageKind::detect()` vérifiait `/etc/os-release`. Sur une machine Fedora, même si l'utilisateur lançait un binaire compilé localement ou téléchargé sous forme d'archive tarball, l'updater détectait `PackageKind::Rpm` et cherchait exclusivement un paquet `.rpm`.
  2. **Artefact RPM manquant lors de releases partielles** : Si le workflow Release avait échoué sur GitHub et que seule une release manuelle avec AppImage ou tarball avait été déposée, la recherche de paquet RPM échouait (`None`), bloquant l'installation.
  3. **Absence de repli universel (*fallback*)** : Si le type de paquet natif de la distribution n'était pas disponible dans les assets de la release GitHub, l'updater abandonnait au lieu de proposer l'AppImage ou le binaire autonome.
  4. **Permissions de fichiers système** : Le remplacement d'un binaire système (`/usr/bin/maschine-linux`) nécessite les privilèges root. Sans élévation (`pkexec`), l'opération échouait silencieusement avec `Permission denied`.
- **Correction & Fiabilisation dans `crates/maschine-app/src/updater.rs`** :
  - `PackageKind::detect()` vérifie maintenant si l'exécutable courant se trouve réellement dans `/usr/bin/`. Les exécutables dans `~/.local/`, `/target/release/` ou des dossiers utilisateur sont classés comme `Binary`.
  - `find_best_asset()` implémente un système de repli universel : si le paquet natif (`.rpm` ou `.deb`) n'est pas trouvé dans la release distante, il recherche automatiquement une `.AppImage`, puis une archive `.tar.gz`.
  - `apply_update()` dispatche l'installation en fonction de l'extension du fichier réellement téléchargé (`.rpm`, `.deb`, `.AppImage`, `.tar.gz`) plutôt que sur le seul OS détecté.
  - Ajout du repli `pkexec` (Polkit) avec message clair si l'écriture nécessite les droits root.

---

### Erreur 4 : Avertissement de syntaxe jour/date dans la spec RPM
- **Symptôme** : Avertissement ou rejet par `rpmlint` lors de la construction du RPM :
  ```text
  warning: %changelog not in descending chronological order or bad day of week
  ```
- **Cause racine** : Le 8 novembre 2026 est un dimanche (`Sun Nov 08 2026`), mais l'entrée dans `packaging/rpm/maschine-linux.spec` indiquait `Sat Nov 08 2026`.
- **Correction** : Correction de l'entrée en `* Sun Nov 08 2026`.

---

## 3. Procédure de Publication Robuste (Checklist Pre-Release)

Avant de pousser un tag et de déclencher la publication d'une version :

```bash
# 1. Vérification du formatage
cargo fmt --all -- --check

# 2. Validation Clippy stricte (warnings = erreurs)
cargo clippy --workspace --all-targets --no-default-features -- -D warnings

# 3. Exécution de tous les tests unitaires et d'intégration
cargo test --workspace --no-default-features

# 4. Vérification de la spec RPM et du fichier desktop
desktop-file-validate packaging/maschine-linux.desktop

# 5. Validation de l'extraction des release notes
VERSION="$(grep -m1 '^version =' Cargo.toml | cut -d '"' -f2)"
./scripts/extract-release-notes.sh "${VERSION}"

# 6. Test de détection de l'updater localement
./target/release/maschine-linux update --check
```

Une fois validé :
```bash
git add -A
git commit -m "chore(release): vX.Y.Z"
git tag -a "vX.Y.Z" -m "vX.Y.Z - Description"
git push origin master --tags
```
