#!/usr/bin/env bash
set -euo pipefail

# Install maschine-linux — Fedora 44+ / PipeWire
# Usage: ./scripts/install.sh [--no-udev] [--no-pipewire]

NO_UDEV=0
NO_PIPEWIRE=0
for arg in "$@"; do
  case "$arg" in
    --no-udev) NO_UDEV=1 ;;
    --no-pipewire) NO_PIPEWIRE=1 ;;
  esac
done

echo "==> maschine-linux install"

if [[ $NO_UDEV -eq 0 ]]; then
  echo "==> Installing udev rule (needs sudo)"
  sudo cp config/udev/99-maschine-mk3.rules /etc/udev/rules.d/
  sudo udevadm control --reload
  sudo udevadm trigger
  echo "   udev: OK — re-plug MK3 or run: sudo udevadm trigger"
fi

if [[ $NO_PIPEWIRE -eq 0 ]]; then
  echo "==> Installing PipeWire low-latency configs (user)"
  mkdir -p ~/.config/pipewire/pipewire.conf.d
  mkdir -p ~/.config/wireplumber/wireplumber.conf.d
  cp config/pipewire/99-maschine-lowlatency.conf ~/.config/pipewire/pipewire.conf.d/
  # WirePlumber >=0.5
  cp config/wireplumber/51-maschine-mk3.conf ~/.config/wireplumber/wireplumber.conf.d/
  # fallback for <=0.4
  mkdir -p ~/.config/wireplumber/main.lua.d
  cp config/wireplumber/51-maschine-mk3.lua ~/.config/wireplumber/main.lua.d/
  echo "   PipeWire configs copied — restarting services..."
  systemctl --user restart pipewire pipewire-pulse wireplumber || true
  sleep 1
  pw-top --help >/dev/null 2>&1 && echo "   pw-top available — check quantum 128"
fi

echo "==> Building (cargo build --release)"
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release

echo "==> Installing binary to ~/.local/bin (needs PATH)"
mkdir -p ~/.local/bin
cp target/release/maschine-linux ~/.local/bin/
echo "   binary: ~/.local/bin/maschine-linux"

echo ""
echo "==> Done. Next:"
echo "  1) sudo usermod -aG plugdev,audio,realtime,jackuser \$USER  # + relogin if needed"
echo "  2) maschine-linux probe        # verify MK3 detected"
echo "  3) maschine-linux hid-dump --duration 5   # press pads"
echo "  4) maschine-linux daemon      # headless"
echo "  5) maschine-linux --help"
