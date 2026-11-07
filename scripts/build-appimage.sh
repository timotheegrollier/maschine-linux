#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# build-appimage.sh — Paquet universel AppImage pour TOUTES les distributions
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${ROOT_DIR}"

VERSION="$(grep -m1 '^version =' Cargo.toml | cut -d '"' -f2)"
echo "==> Préparation du paquet universel AppImage (v${VERSION})..."

cargo build --release --locked -p maschine-app

APP_DIR="${ROOT_DIR}/target/AppDir"
OUT_DIR="${ROOT_DIR}/dist"
mkdir -p "${APP_DIR}" "${OUT_DIR}"
rm -rf "${APP_DIR:?}"/*

# 1. Structure de l'AppDir
mkdir -p "${APP_DIR}/usr/bin"
mkdir -p "${APP_DIR}/usr/share/applications"
mkdir -p "${APP_DIR}/usr/share/metainfo"
mkdir -p "${APP_DIR}/usr/share/icons/hicolor/scalable/apps"

# 2. Copie du binaire et des métadonnées
cp target/release/maschine-linux "${APP_DIR}/usr/bin/maschine-linux"
cp packaging/maschine-linux.desktop "${APP_DIR}/usr/share/applications/maschine-linux.desktop"
cp packaging/maschine-linux.desktop "${APP_DIR}/maschine-linux.desktop"
cp packaging/io.github.timotheegrollier.maschine-linux.metainfo.xml "${APP_DIR}/usr/share/metainfo/"

# 3. Icônes
cp packaging/icons/maschine-linux.svg "${APP_DIR}/usr/share/icons/hicolor/scalable/apps/maschine-linux.svg"
cp packaging/icons/256x256/maschine-linux.png "${APP_DIR}/maschine-linux.png"
cp packaging/icons/256x256/maschine-linux.png "${APP_DIR}/.DirIcon"

for size in 16 32 48 64 128 256 512; do
  mkdir -p "${APP_DIR}/usr/share/icons/hicolor/${size}x${size}/apps"
  cp "packaging/icons/${size}x${size}/maschine-linux.png" "${APP_DIR}/usr/share/icons/hicolor/${size}x${size}/apps/maschine-linux.png"
done

# 4. AppRun script
cat << 'EOF' > "${APP_DIR}/AppRun"
#!/bin/sh
SELF=$(readlink -f "$0")
HERE=${SELF%/*}
export PATH="${HERE}/usr/bin:${PATH}"
export XDG_DATA_DIRS="${HERE}/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
exec "${HERE}/usr/bin/maschine-linux" "$@"
EOF
chmod +x "${APP_DIR}/AppRun"

# 5. Packaging via appimagetool
APPIMAGETOOL="${ROOT_DIR}/target/appimagetool"
if ! command -v appimagetool >/dev/null 2>&1; then
    if [ ! -f "${APPIMAGETOOL}" ]; then
        echo "==> Téléchargement de appimagetool..."
        curl -sSL "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage" -o "${APPIMAGETOOL}"
        chmod +x "${APPIMAGETOOL}"
    fi
    TOOL="${APPIMAGETOOL}"
else
    TOOL="appimagetool"
fi

echo "==> Génération de l'AppImage..."
ARCH=x86_64 "${TOOL}" --appimage-extract-and-run "${APP_DIR}" "${OUT_DIR}/Maschine_Linux-${VERSION}-x86_64.AppImage" || \
ARCH=x86_64 "${TOOL}" "${APP_DIR}" "${OUT_DIR}/Maschine_Linux-${VERSION}-x86_64.AppImage"

echo "✓ AppImage créée avec succès dans :"
ls -lh "${OUT_DIR}"/*.AppImage
