#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# install.sh — Installateur standardisé pour maschine-linux
# Compatible Fedora / Nobara 44, Ubuntu/Debian, Arch Linux & universel
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

PREFIX="/usr/local"
USER_INSTALL=0
UNINSTALL=0

print_usage() {
    cat << EOF
Usage: ./scripts/install.sh [OPTIONS]

Options:
  --user        Installer dans l'espace utilisateur (~/.local) sans sudo
  --system      Installer sur le système (/usr/local) avec sudo [défaut]
  --uninstall   Désinstaller maschine-linux et ses configurations
  --rpm         Construire et installer le paquet RPM natif (Fedora / Nobara)
  --help        Afficher cette aide
EOF
}

INSTALL_RPM=0
for arg in "$@"; do
    case "$arg" in
        --user) USER_INSTALL=1 ;;
        --system) USER_INSTALL=0 ;;
        --rpm) INSTALL_RPM=1 ;;
        --uninstall) UNINSTALL=1 ;;
        --help|-h) print_usage; exit 0 ;;
        *) echo "Option inconnue : $arg"; print_usage; exit 1 ;;
    esac
done

if [ "$UNINSTALL" -eq 1 ]; then
    echo "==> Désinstallation de maschine-linux..."
    if [ "$USER_INSTALL" -eq 1 ]; then
        rm -f ~/.local/bin/maschine-linux
        rm -f ~/.local/share/applications/maschine-linux.desktop
        rm -f ~/.local/share/icons/hicolor/*/apps/maschine-linux.png
        rm -f ~/.local/share/icons/hicolor/scalable/apps/maschine-linux.svg
        rm -f ~/.config/pipewire/pipewire.conf.d/99-maschine-lowlatency.conf
        rm -f ~/.config/wireplumber/wireplumber.conf.d/51-maschine-mk3.conf
        echo "✓ Désinstallation utilisateur terminée."
    else
        sudo rm -f /usr/local/bin/maschine-linux /usr/bin/maschine-linux
        sudo rm -f /usr/share/applications/maschine-linux.desktop
        sudo rm -f /usr/share/metainfo/io.github.timotheegrollier.maschine-linux.metainfo.xml
        sudo rm -f /usr/share/icons/hicolor/*/apps/maschine-linux.png
        sudo rm -f /usr/share/icons/hicolor/scalable/apps/maschine-linux.svg
        sudo rm -f /etc/udev/rules.d/99-maschine-mk3.rules
        sudo rm -f /usr/lib/udev/rules.d/99-maschine-mk3.rules
        sudo rm -f /etc/security/limits.d/99-realtime-audio.conf
        sudo rm -f /usr/share/pipewire/pipewire.conf.d/99-maschine-lowlatency.conf
        sudo rm -f /usr/share/wireplumber/wireplumber.conf.d/51-maschine-mk3.conf
        sudo udevadm control --reload-rules || true
        sudo udevadm trigger || true
        echo "✓ Désinstallation système terminée."
    fi
    exit 0
fi

if [ "$INSTALL_RPM" -eq 1 ]; then
    echo "==> Construction et installation du paquet RPM pour Fedora / Nobara..."
    "${ROOT_DIR}/scripts/build-rpm.sh"
    RPM_FILE=$(find "${ROOT_DIR}/dist" -name "*.rpm" | head -n1)
    if [ -n "$RPM_FILE" ]; then
        echo "==> Installation de $RPM_FILE via dnf..."
        sudo dnf install -y --nogpgcheck "$RPM_FILE"
        echo "✓ Paquet RPM installé avec succès !"
        exit 0
    else
        echo "Erreur : paquet RPM non trouvé."
        exit 1
    fi
fi

# Détection de l'OS
OS="unknown"
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS="$ID"
fi
echo "==> Système détecté : ${PRETTY_NAME:-$OS}"

# Construction si nécessaire
if [ ! -f "${ROOT_DIR}/target/release/maschine-linux" ]; then
    echo "==> Compilation en mode release (cargo build --release)..."
    (cd "${ROOT_DIR}" && cargo build --release -p maschine-app)
fi

echo "==> Installation des fichiers..."

if [ "$USER_INSTALL" -eq 1 ]; then
    BIN_DIR="$HOME/.local/bin"
    APP_DIR="$HOME/.local/share/applications"
    ICON_DIR="$HOME/.local/share/icons/hicolor"

    mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR/scalable/apps"
    cp "${ROOT_DIR}/target/release/maschine-linux" "$BIN_DIR/"
    cp "${ROOT_DIR}/packaging/maschine-linux.desktop" "$APP_DIR/"
    cp "${ROOT_DIR}/packaging/icons/maschine-linux.svg" "$ICON_DIR/scalable/apps/"

    for size in 16 32 48 64 128 256 512; do
        mkdir -p "$ICON_DIR/${size}x${size}/apps"
        cp "${ROOT_DIR}/packaging/icons/${size}x${size}/maschine-linux.png" "$ICON_DIR/${size}x${size}/apps/"
    done

    echo "==> Configuration PipeWire et WirePlumber (utilisateur)..."
    mkdir -p ~/.config/pipewire/pipewire.conf.d
    mkdir -p ~/.config/wireplumber/wireplumber.conf.d
    mkdir -p ~/.config/systemd/user
    cp "${ROOT_DIR}/packaging/pipewire/99-maschine-lowlatency.conf" ~/.config/pipewire/pipewire.conf.d/
    cp "${ROOT_DIR}/packaging/wireplumber/51-maschine-mk3.conf" ~/.config/wireplumber/wireplumber.conf.d/
    cp "${ROOT_DIR}/packaging/systemd/maschine-linux.service" ~/.config/systemd/user/
    systemctl --user daemon-reload || true
    systemctl --user restart pipewire pipewire-pulse wireplumber || true

    echo "✓ Installation utilisateur terminée dans ~/.local/bin/maschine-linux"
    echo "  Assurez-vous que ~/.local/bin est dans votre PATH."
else
    echo "==> Installation système (requiert sudo)..."
    sudo install -D -p -m 0755 "${ROOT_DIR}/target/release/maschine-linux" /usr/local/bin/maschine-linux
    sudo install -D -p -m 0644 "${ROOT_DIR}/packaging/maschine-linux.desktop" /usr/share/applications/maschine-linux.desktop
    sudo install -D -p -m 0644 "${ROOT_DIR}/packaging/io.github.timotheegrollier.maschine-linux.metainfo.xml" /usr/share/metainfo/io.github.timotheegrollier.maschine-linux.metainfo.xml
    sudo install -D -p -m 0644 "${ROOT_DIR}/packaging/icons/maschine-linux.svg" /usr/share/icons/hicolor/scalable/apps/maschine-linux.svg

    for size in 16 32 48 64 128 256 512; do
        sudo install -D -p -m 0644 "${ROOT_DIR}/packaging/icons/${size}x${size}/maschine-linux.png" "/usr/share/icons/hicolor/${size}x${size}/apps/maschine-linux.png"
    done

    echo "==> Installation des règles udev Plug & Play..."
    sudo install -D -p -m 0644 "${ROOT_DIR}/packaging/udev/99-maschine-mk3.rules" /etc/udev/rules.d/99-maschine-mk3.rules
    sudo udevadm control --reload-rules || true
    sudo udevadm trigger --subsystem-match=usb --subsystem-match=hidraw || true

    echo "==> Configuration des limites temps réel audio (limits.d)..."
    sudo install -D -p -m 0644 "${ROOT_DIR}/packaging/limits/99-realtime-audio.conf" /etc/security/limits.d/99-realtime-audio.conf

    echo "==> Configuration PipeWire et WirePlumber (ultra-low latency)..."
    sudo install -D -p -m 0644 "${ROOT_DIR}/packaging/pipewire/99-maschine-lowlatency.conf" /usr/share/pipewire/pipewire.conf.d/99-maschine-lowlatency.conf 2>/dev/null || \
      cp "${ROOT_DIR}/packaging/pipewire/99-maschine-lowlatency.conf" ~/.config/pipewire/pipewire.conf.d/ 2>/dev/null || true

    sudo install -D -p -m 0644 "${ROOT_DIR}/packaging/wireplumber/51-maschine-mk3.conf" /usr/share/wireplumber/wireplumber.conf.d/51-maschine-mk3.conf 2>/dev/null || \
      cp "${ROOT_DIR}/packaging/wireplumber/51-maschine-mk3.conf" ~/.config/wireplumber/wireplumber.conf.d/ 2>/dev/null || true

    echo "==> Installation du service systemd utilisateur..."
    sudo install -D -p -m 0644 "${ROOT_DIR}/packaging/systemd/maschine-linux.service" /usr/lib/systemd/user/maschine-linux.service 2>/dev/null || true

    # Mise à jour des caches
    sudo update-desktop-database -q /usr/share/applications || true
    sudo gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true

    echo "==> Ajout de l'utilisateur aux groupes audio et realtime..."
    sudo usermod -aG audio "$USER" 2>/dev/null || true
    sudo usermod -aG realtime "$USER" 2>/dev/null || true

    echo ""
    echo "✓ Installation système terminée avec succès !"
    echo "  • Binaire : /usr/local/bin/maschine-linux"
    echo "  • Raccourci application : Maschine Linux (menu applications)"
    echo "  • Règles udev Plug & Play : /etc/udev/rules.d/99-maschine-mk3.rules"
    echo "  • Branchez votre Maschine MK3 et lancez 'maschine-linux' !"
fi
