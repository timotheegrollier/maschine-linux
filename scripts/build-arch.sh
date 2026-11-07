#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# build-arch.sh — Génération de paquet Arch Linux (.pkg.tar.zst)
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${ROOT_DIR}"

VERSION="$(grep -m1 '^version =' Cargo.toml | cut -d '"' -f2)"
echo "==> Préparation du paquet Arch Linux (v${VERSION})..."

OUT_DIR="${ROOT_DIR}/dist"
mkdir -p "${OUT_DIR}"

if ! command -v makepkg >/dev/null 2>&1; then
    echo "ℹ 'makepkg' n'est pas disponible sur ce système non-Arch."
    echo "  Le fichier PKGBUILD prêt pour l'AUR se trouve dans : packaging/arch/PKGBUILD"
    exit 0
fi

ARCH_WORK_DIR="${ROOT_DIR}/target/arch-build"
rm -rf "${ARCH_WORK_DIR}"
mkdir -p "${ARCH_WORK_DIR}"

cp packaging/arch/PKGBUILD "${ARCH_WORK_DIR}/"
[ -f packaging/arch/maschine-linux.install ] && cp packaging/arch/maschine-linux.install "${ARCH_WORK_DIR}/"

cd "${ARCH_WORK_DIR}"
makepkg -f --nodeps --skipinteg

find . -name "*.pkg.tar.*" -exec cp {} "${OUT_DIR}/" \;
echo "✓ Paquet Arch Linux créé dans ${OUT_DIR}/"
ls -la "${OUT_DIR}"/*.pkg.tar.* 2>/dev/null || true
