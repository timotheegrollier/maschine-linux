#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# package-release.sh — Bundle autonome universel (.tar.gz) + Checksums
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${ROOT_DIR}"

VERSION="$(grep -m1 '^version =' Cargo.toml | cut -d '"' -f2)"
echo "==> Création de l'archive autonome universelle v${VERSION}..."

# Construction du binaire autonome (--no-default-features pour compatibilité universelle sans dépendances Alsa de dev)
cargo build --release -p maschine-app --no-default-features

OUT_DIR="${ROOT_DIR}/dist"
STAGE_DIR="${ROOT_DIR}/target/release-stage/maschine-linux-${VERSION}"
mkdir -p "${OUT_DIR}"
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}"/bin "${STAGE_DIR}"/packaging

cp target/release/maschine-linux "${STAGE_DIR}/bin/"
cp -r packaging/* "${STAGE_DIR}/packaging/"
cp scripts/install.sh "${STAGE_DIR}/"
cp README.md LICENSE "${STAGE_DIR}/"

TARBALL="${OUT_DIR}/maschine-linux-${VERSION}-linux-x86_64.tar.gz"
tar -czf "${TARBALL}" -C "${ROOT_DIR}/target/release-stage" "maschine-linux-${VERSION}"

echo "==> Génération des sommes de contrôle SHA-256..."
cd "${OUT_DIR}"
rm -f SHA256SUMS
sha256sum * > SHA256SUMS 2>/dev/null || true

echo "✓ Fichiers prêts dans ${OUT_DIR}/ :"
ls -lh "${OUT_DIR}"
