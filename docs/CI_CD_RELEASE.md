# Guide CI/CD, Releases & Auto-Updater

Ce document détaille le fonctionnement des pipelines d'intégration continue (CI) et de publication (Release), les optimisations de vitesse mises en place, la procédure pour tester une release sans la publier, et les erreurs récurrentes avec leurs causes racines.

---

## 1. Vue d'ensemble de l'architecture CI/CD

Le projet utilise GitHub Actions avec deux workflows :

### `CI` (`.github/workflows/ci.yml`)

Déclenché sur chaque `push` et `pull_request` sur `master`/`main`, et manuellement (`workflow_dispatch`). Les vérifications sont réparties en jobs parallèles indépendants, de sorte qu'un échec de formatage n'attend pas la fin des tests :

| Job | Contenu | Rôle |
|---|---|---|
| `metadata` | `desktop-file-validate`, `appstreamcli validate --no-net`, `cargo fmt --all -- --check`, extraction des release notes, `bash -n scripts/*.sh` | Échoue en premier sur les erreurs les moins coûteuses à corriger |
| `test` | `cargo clippy --workspace --all-targets --locked`, `cargo check -p maschine-engine --no-default-features --locked`, `cargo nextest run --workspace`, `cargo test --doc --workspace --locked` | Qualité et non-régression complètes |
| `msrv` | `cargo check --workspace --locked` avec Rust 1.88.0 | Vérifie la version minimale déclarée |
| `release-build` | `cargo build --release --locked -p maschine-app` + smoke test (`--version`, `probe`) | Uniquement sur `push` vers `master` et en manuel |

`RUSTFLAGS="-D warnings"` reste global : tout avertissement est une erreur, comme avant.

### `Release` (`.github/workflows/release.yml`)

Déclenché sur un tag `v*`, ou manuellement (`workflow_dispatch`, avec l'option `publish`).

| Job | Contenu |
|---|---|
| `build` | Résout la version, vérifie la cohérence tag/`Cargo.toml`, compile **une seule fois** `maschine-app` en profil release, smoke test binaire, publie le binaire en artefact |
| `packages` (matrice ×4, parallèle) | Chaque format télécharge le binaire compilé et n'effectue que l'empaquetage : RPM (`cargo-generate-rpm`), DEB (`cargo-deb`), AppImage (`appimagetool`), tarball autonome |
| `verify` | Assemble les quatre formats, vérifie la présence de `/usr/bin/maschine-linux` dans le RPM, le DEB et le tarball, vérifie que chaque nom de fichier porte la version attendue, génère et contrôle `SHA256SUMS` |
| `publish` | Extrait les notes anglaises et publie la GitHub Release avec les artefacts **déjà vérifiés** |

Aucune publication n'a lieu si `verify` échoue : la qualité du contenu livré est donc strictement supérieure à l'ancien pipeline, qui publiait sans contrôle des paquets.

---

## 2. Optimisations de vitesse

| Optimisation | Effet | Pourquoi la qualité ne baisse pas |
|---|---|---|
| **Build once, package many** | La release compile `maschine-app` **1 fois** au lieu de 4 (workflow + AppImage + DEB + tarball ; le RPM, lui, réutilisait déjà le binaire présent), et avec un jeu de features unique au lieu de deux | Tous les formats embarquent désormais le **même** binaire, ce qui supprime une incohérence réelle : l'AppImage et le tarball étaient construits avec `--no-default-features` (donc sans GUI, audio, USB ni MIDI) puis écrasaient le binaire commun. Le binaire livré est maintenant celui validé par le smoke test. |
| **Pas de build release sur les PR** | Supprime la compilation la plus lente du pipeline (LTO `thin`, `codegen-units = 1` sur tout le workspace) | Clippy `--all-targets` et les tests restent exécutés sur chaque PR ; le profil release est compilé et démarré à chaque `push` sur `master` et à chaque tag, avant toute publication. Un tag ne peut pas partir sur une base où ce job n'est pas vert. |
| **`cargo nextest`** | Exécution des tests en parallèle, processus isolés, sortie compacte | nextest exécute exactement les mêmes tests unitaires et d'intégration ; `cargo test --doc --workspace` conserve la couverture des doctests, que nextest ignore. |
| **Outils d'empaquetage précompilés** | `cargo-deb` et `cargo-generate-rpm` installés depuis leurs binaires (quelques secondes) au lieu de `cargo install` (compilation complète) | Versions identiques, checksums vérifiés par `taiki-e/install-action`. |
| **`concurrency` + `cancel-in-progress`** | Un nouveau push annule le run obsolète de la même branche | Les runs de tag ne sont jamais annulés (`cancel-in-progress: false` côté Release) : aucune release à moitié publiée. |
| **Cache partagé, écrit uniquement sur `master`** | Un seul cache cargo pour `metadata`/`test`, restauration sur les PR sans écriture concurrente | Toutes les crates compilées restent les mêmes ; seul le stockage du cache est rationalisé. |
| **`CARGO_PROFILE_{DEV,TEST}_DEBUG=1`** | Réduit le volume de debuginfo produite pour les tests | Les backtraces conservent les numéros de ligne ; seul le détail complet des variables disparaît. |
| **`timeout-minutes` par job** | Un job bloqué échoue au lieu de consommer le maximum du runner | Aucune vérification n'est retirée ; seule la durée maximale est bornée. |
| **`--locked` partout** | `Cargo.lock` désynchronisé détecté immédiatement | Dépendances reproductibles, identiques à celles de la release. |
| **`scripts/*.sh` validés par `bash -n`** | Erreur de syntaxe de script détectée en secondes, avant la release | Vérification ajoutée, rien de retiré. |

Les durées exactes varient selon le runner et le cache ; ces optimisations réduisent le nombre de compilations et de jobs séquentiels plutôt que de supprimer des contrôles.

---

## 3. Tester une release sans la publier (répétition)

C'est le moyen le plus rapide de valider une release candidate avant de poser le tag.

1. Onglet **Actions** → **Release** → **Run workflow**.
2. Choisir la branche à tester (ou `master`), laisser `publish` décoché, puis lancer.
3. Le pipeline s'exécute intégralement : build unique, RPM/DEB/AppImage/tarball en parallèle, vérification des contenus et des sommes SHA-256.
4. En fin de run, télécharger depuis la page du run :
   - `release-assets` (les quatre paquets + `SHA256SUMS` déjà vérifiés, identiques à ceux qui seraient publiés),
   - `pkg-rpm`, `pkg-deb`, `pkg-appimage`, `pkg-tarball` pour inspecter un format isolément,
   - `maschine-linux-x86_64` pour le binaire nu.
5. Vérifier localement si besoin :
   ```bash
   sha256sum --check SHA256SUMS
   dpkg-deb --info maschine-linux_*.deb
   rpm -qpl maschine-linux-*.rpm
   ```

Cocher `publish` (ou pousser un tag `vX.Y.Z`) rejoue exactement le même pipeline et ajoute uniquement la publication. Le test manuel peut donc précéder le tag au lieu de le suivre.

Pour reproduire la même logique en local, sans recompiler pour chaque format :

```bash
cargo build --release --locked -p maschine-app
MASCHINE_SKIP_BUILD=1 ./scripts/build-rpm.sh
MASCHINE_SKIP_BUILD=1 ./scripts/build-deb.sh
MASCHINE_SKIP_BUILD=1 ./scripts/build-appimage.sh
MASCHINE_SKIP_BUILD=1 MASCHINE_SKIP_CHECKSUMS=1 ./scripts/package-release.sh
(cd dist && sha256sum *.rpm *.deb *.AppImage *.tar.gz > SHA256SUMS && sha256sum --check SHA256SUMS)
```

Les scripts refusent de produire un paquet vide : si `MASCHINE_SKIP_BUILD=1` est défini sans binaire présent dans `target/release/`, ils s'arrêtent avec un message explicite.

---

## 4. Analyse des erreurs fréquentes & causes racines

### Erreur 1 : Échec du contrôle de formatage (`cargo fmt --all -- --check`)
- **Symptôme** : le job `metadata` échoue à l'étape `Format` avec `Process completed with exit code 1` et un diff sur des fichiers Rust modifiés.
- **Cause racine** : des modifications de code (refactorings, ajouts de tests, réindentations manuelles) ont été validées sans exécuter `cargo fmt --all` au préalable. Comme `ci.yml` exporte `RUSTFLAGS="-D warnings"`, tout écart de formatage bloque le pipeline.
- **Correction & fiabilisation** :
  - Exécuter systématiquement `cargo fmt --all` avant tout commit ou git tag.
  - La vérification locale recommandée est : `cargo fmt --all -- --check`.

### Erreur 2 : Échec d'extraction des release notes (`extract-release-notes.sh`)
- **Symptôme** : le workflow Release ou le job `metadata` échoue avec :
  ```text
  error: no release notes found for X.Y.Z in .../CHANGELOG.md
  Process completed with exit code 1
  ```
- **Cause racine** : le script exigeait impérativement une section `## [X.Y.Z]` dans `CHANGELOG.md`. Lorsqu'une version était incrémentée dans `Cargo.toml` sans ajouter immédiatement l'entrée correspondante, le script retournait un code d'erreur 1.
- **Correction & fiabilisation** : `extract-release-notes.sh` applique des replis automatiques :
  1. section `## [X.Y.Z]` de `CHANGELOG.md` ;
  2. sinon `dist/RELEASE_NOTES.md` s'il existe et n'est pas vide ;
  3. sinon les commits Git depuis le tag précédent ;
  4. erreur seulement en l'absence totale d'informations (le job `publish` fait un `checkout` avec `fetch-depth: 0` pour que ce repli soit fiable).

### Erreur 3 : Auto-updater in-app ne proposant pas l'installation
- **Symptômes** :
  - Le dialogue d'auto-update affiche « Aucun paquet pré-compilé » ou n'offre pas le bouton d'installation.
  - L'application signale qu'une version est disponible mais ne permet pas l'installation en un clic.
- **Causes racines identifiées** :
  1. **Détection d'environnement trop agressive** : `PackageKind::detect()` vérifiait `/etc/os-release`. Sur une machine Fedora, même avec un binaire compilé localement ou une archive tarball, l'updater détectait `PackageKind::Rpm` et cherchait exclusivement un paquet `.rpm`.
  2. **Artefact RPM manquant lors de releases partielles** : si le workflow Release échouait et qu'une release manuelle ne contenait qu'une AppImage ou un tarball, la recherche de paquet RPM échouait (`None`).
  3. **Absence de repli universel** : si le type natif de la distribution n'était pas disponible, l'updater abandonnait au lieu de proposer l'AppImage ou le binaire autonome.
  4. **Permissions de fichiers système** : remplacer `/usr/bin/maschine-linux` nécessite les privilèges root ; sans `pkexec`, l'opération échouait silencieusement (`Permission denied`).
- **Correction & fiabilisation dans `crates/maschine-app/src/updater.rs`** :
  - `PackageKind::detect()` vérifie si l'exécutable courant se trouve réellement dans `/usr/bin/` ; ailleurs (`~/.local/`, `target/release/`), il classe en `Binary`.
  - `find_best_asset()` applique un repli universel : paquet natif (`.rpm`/`.deb`), sinon `.AppImage`, sinon `.tar.gz`.
  - `apply_update()` dispatche l'installation selon l'extension réellement téléchargée.
  - Repli `pkexec` (Polkit) avec message clair si l'écriture exige root.

### Erreur 4 : Avertissement de syntaxe jour/date dans la spec RPM
- **Symptôme** :
  ```text
  warning: %changelog not in descending chronological order or bad day of week
  ```
- **Cause racine** : le 8 novembre 2026 est un dimanche (`Sun Nov 08 2026`), mais l'entrée indiquait `Sat Nov 08 2026`.
- **Correction** : entrée corrigée en `* Sun Nov 08 2026`.
- **Note** : la publication utilise `cargo-generate-rpm`, qui génère le RPM depuis `[package.metadata.generate-rpm]` sans passer par `rpmbuild`. La spec reste la référence pour les constructions manuelles sur Fedora.

### Erreur 5 : Paquet publié sans binaire de GUI
- **Symptôme** : l'AppImage ou le tarball se lançait en mode CLI (`probe`) au lieu d'ouvrir l'interface.
- **Cause racine** : `build-appimage.sh` et `package-release.sh` recompilaient avec `--no-default-features`, ce qui désactive `gui`, `audio`, `usb` et `midi-alsa`, puis écrasaient le binaire complet produit par les autres étapes.
- **Correction & fiabilisation** : un seul binaire (features par défaut) est compilé par le job `build` et réutilisé par les quatre formats ; le job `verify` contrôle la présence du binaire dans chaque paquet et les scripts refusent de produire un paquet sans binaire.

### Erreur 6 : Artefact périmé dans `dist/`
- **Symptôme** : un `.deb` ou `.rpm` d'une version précédente se retrouvait attaché à la release.
- **Cause racine** : les scripts copiaient avec un `find ... -name "*-VERSION-*"` et certaines étapes laissaient des fichiers d'un run antérieur dans le dossier partagé.
- **Correction & fiabilisation** : chaque format est produit dans un job isolé (donc sans `dist/` hérité), et le job `verify` rejette tout fichier dont le nom ne porte pas la version attendue avant de calculer `SHA256SUMS`.

---

## 5. Procédure de publication robuste (checklist pre-release)

Avant de pousser un tag :

```bash
# 1. Vérification du formatage
cargo fmt --all -- --check

# 2. Validation Clippy stricte (warnings = erreurs)
cargo clippy --workspace --all-targets --locked -- -D warnings

# 3. Tests unitaires et d'intégration (+ doctests)
cargo nextest run --workspace --locked
cargo test --doc --workspace --locked

# 4. Vérification de la spec RPM et du fichier desktop
desktop-file-validate packaging/maschine-linux.desktop
appstreamcli validate --no-net packaging/io.github.timotheegrollier.maschine-linux.metainfo.xml

# 5. Validation de l'extraction des release notes
VERSION="$(grep -m1 '^version =' Cargo.toml | cut -d '"' -f2)"
./scripts/extract-release-notes.sh "${VERSION}"

# 6. Répétition complète du pipeline de release (sans publication)
#    Actions → Release → Run workflow, publish décoché → télécharger release-assets

# 7. Test de détection de l'updater localement
./target/release/maschine-linux update --check
```

Une fois `Release` (mode dry run) vert :
```bash
git add -A
git commit -m "chore(release): vX.Y.Z"
git tag -a "vX.Y.Z" -m "vX.Y.Z - Description"
git push origin master --tags
```

---

## 6. Diagnostiquer un run lent ou bloqué

1. **Vérifier quel job domine** : la vue *Summary* du run donne la durée par job. `metadata` doit rester sous la minute ; sinon, c'est `apt-get` ou l'installation de la toolchain qui coince.
2. **Cache non restauré** : chercher `Cache not found` dans les logs du job. Le cache est écrit uniquement sur `master` ; un fork ou un changement de `Cargo.lock` provoque une reconstruction complète attendue.
3. **`RUSTFLAGS` modifié** : le cache de `Swatinem/rust-cache` est indexé sur les flags et la toolchain ; changer l'un invalide légitimement le cache.
4. **Pas de compilation dans les jobs d'empaquetage** : s'ils compilent, c'est que `MASCHINE_SKIP_BUILD=1` n'est pas transmis ou que `target/release/maschine-linux` n'a pas été restauré (artefact `maschine-linux-x86_64`).
5. **Run annulé** : normal après un nouveau push sur la même branche (`concurrency`). Les runs de tag ne sont jamais annulés.

Pour relancer une release ratée sans reposer le tag : **Actions → Release → Run workflow**, cocher `publish` sur la branche concernée après validation du dry run.
