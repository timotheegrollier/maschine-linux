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

# La CI fournit le binaire déjà compilé (MASCHINE_SKIP_BUILD=1) : le tarball
# embarque exactement le binaire livré dans les paquets RPM/DEB.
if [[ "${MASCHINE_SKIP_BUILD:-0}" != "1" ]]; then
    cargo build --release --locked -p maschine-app
fi
if [[ ! -x target/release/maschine-linux ]]; then
    echo "error: target/release/maschine-linux introuvable" >&2
    echo "       compilez-le (cargo build --release -p maschine-app) ou retirez MASCHINE_SKIP_BUILD=1" >&2
    exit 1
fi

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

# Les sommes de contrôle portent sur tous les artefacts : la CI les calcule
# une seule fois, après avoir assemblé les quatre formats.
if [[ "${MASCHINE_SKIP_CHECKSUMS:-0}" != "1" ]]; then
    echo "==> Génération des sommes de contrôle SHA-256..."
    cd "${OUT_DIR}"
    rm -f SHA256SUMS
    sha256sum * > SHA256SUMS 2>/dev/null || true
fi

echo "✓ Fichiers prêts dans ${OUT_DIR}/ :"
ls -lh "${OUT_DIR}"
