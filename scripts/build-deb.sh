#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# build-deb.sh — Génération de paquet DEB pour Debian / Ubuntu / Mint / PopOS
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${ROOT_DIR}"

VERSION="$(grep -m1 '^version =' Cargo.toml | cut -d '"' -f2)"
echo "==> Préparation du paquet Debian/Ubuntu (v${VERSION})..."

cargo build --release --locked -p maschine-app

OUT_DIR="${ROOT_DIR}/dist"
mkdir -p "${OUT_DIR}"

if command -v cargo-deb >/dev/null 2>&1; then
    echo "==> Génération DEB via cargo-deb..."
    (cd crates/maschine-app && cargo deb --target-dir ../../target)
    find target/debian -name "*.deb" -exec cp {} "${OUT_DIR}/" \;
    echo "✓ DEB créé dans ${OUT_DIR}/"
    ls -la "${OUT_DIR}"/*.deb
    exit 0
fi

if command -v dpkg-deb >/dev/null 2>&1; then
    echo "==> Génération DEB manuelle via dpkg-deb..."
    DEB_DIR="${ROOT_DIR}/target/deb_pkg"
    rm -rf "${DEB_DIR}"
    mkdir -p "${DEB_DIR}/DEBIAN"
    mkdir -p "${DEB_DIR}/usr/bin"
    mkdir -p "${DEB_DIR}/usr/share/applications"
    mkdir -p "${DEB_DIR}/usr/share/metainfo"
    mkdir -p "${DEB_DIR}/usr/share/icons/hicolor/scalable/apps"
    mkdir -p "${DEB_DIR}/lib/udev/rules.d"
    mkdir -p "${DEB_DIR}/etc/security/limits.d"
    mkdir -p "${DEB_DIR}/usr/share/pipewire/pipewire.conf.d"
    mkdir -p "${DEB_DIR}/usr/share/wireplumber/wireplumber.conf.d"
    mkdir -p "${DEB_DIR}/usr/lib/systemd/user"

    # Control file
    cat << EOF > "${DEB_DIR}/DEBIAN/control"
Package: maschine-linux
Version: ${VERSION}
Section: sound
Priority: optional
Architecture: amd64
Maintainer: Timothée Grollier <timo@example.com>
Description: Native Instruments Maschine MK3 DAW and Controller integration
 Maschine Linux is an ultra-low-latency Digital Audio Workstation (DAW)
 and controller host tailored specifically for the Native Instruments
 Maschine MK3 and Mikro MK3 hardware controllers on Linux.
EOF

    # Postinst
    cat << 'EOF' > "${DEB_DIR}/DEBIAN/postinst"
#!/bin/sh
set -e
if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules || true
    udevadm trigger --subsystem-match=usb --subsystem-match=hidraw || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
fi
EOF
    chmod 0755 "${DEB_DIR}/DEBIAN/postinst"

    # Assets
    cp target/release/maschine-linux "${DEB_DIR}/usr/bin/maschine-linux"
    cp packaging/maschine-linux.desktop "${DEB_DIR}/usr/share/applications/"
    cp packaging/io.github.timotheegrollier.maschine-linux.metainfo.xml "${DEB_DIR}/usr/share/metainfo/"
    cp packaging/icons/maschine-linux.svg "${DEB_DIR}/usr/share/icons/hicolor/scalable/apps/"
    for size in 16 32 48 64 128 256 512; do
      mkdir -p "${DEB_DIR}/usr/share/icons/hicolor/${size}x${size}/apps"
      cp "packaging/icons/${size}x${size}/maschine-linux.png" "${DEB_DIR}/usr/share/icons/hicolor/${size}x${size}/apps/"
    done
    cp packaging/udev/99-maschine-mk3.rules "${DEB_DIR}/lib/udev/rules.d/"
    cp packaging/limits/99-realtime-audio.conf "${DEB_DIR}/etc/security/limits.d/"
    cp packaging/pipewire/99-maschine-lowlatency.conf "${DEB_DIR}/usr/share/pipewire/pipewire.conf.d/"
    cp packaging/wireplumber/51-maschine-mk3.conf "${DEB_DIR}/usr/share/wireplumber/wireplumber.conf.d/"
    cp packaging/systemd/maschine-linux.service "${DEB_DIR}/usr/lib/systemd/user/"

    dpkg-deb --build "${DEB_DIR}" "${OUT_DIR}/maschine-linux_${VERSION}_amd64.deb"
    echo "✓ DEB créé avec succès dans ${OUT_DIR}/"
    ls -lh "${OUT_DIR}"/*.deb
    exit 0
fi

echo "Ni cargo-deb ni dpkg-deb n'est présent. Installez cargo-deb (cargo install cargo-deb)."
exit 1
