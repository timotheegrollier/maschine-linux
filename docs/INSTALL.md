# Installation — Fedora 44 (PipeWire)

## Prérequis système

```bash
sudo dnf install @development-tools \
  alsa-lib-devel jack-audio-connection-kit-devel \
  pipewire-jack-audio-connection-kit-devel pipewire-devel \
  libusb1-devel hidapi-devel systemd-devel \
  clang pkgconf-pkg-config

# Rust (si pas déjà)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

## Install rapide

```bash
git clone https://github.com/timo/maschine-linux
cd maschine-linux
./scripts/install.sh
# ou manuellement :
cargo build --release
sudo cp config/udev/99-maschine-mk3.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger
mkdir -p ~/.config/pipewire/pipewire.conf.d ~/.config/wireplumber/wireplumber.conf.d
cp config/pipewire/99-maschine-lowlatency.conf ~/.config/pipewire/pipewire.conf.d/
cp config/wireplumber/51-maschine-mk3.conf ~/.config/wireplumber/wireplumber.conf.d/
systemctl --user restart pipewire wireplumber
```

## Vérification

```bash
maschine-linux probe
# doit afficher: VID:PID 17cc:1600 hidraw open: OK

maschine-linux hid-dump --duration 5
# appuie sur pads → doit afficher 0x01 / 0x02

wpctl status | grep -A2 Maschine
# doit afficher "Pro Audio" pas "Surround 4.0"
pw-top
# quantum 128, ERR 0
```

## Permissions

Le fichier `config/udev/99-maschine-mk3.rules` utilise `TAG+="uaccess"` (systemd-logind) : pas besoin d'ajouter ton user à `plugdev` si tu es en session graphique. Sinon :

```bash
sudo usermod -aG plugdev,audio $USER
# relogin
```

Vérifie : `getfacl /dev/hidraw3` doit contenir `user:timo:rw-`.

## Build options

```bash
cargo build --release                          # daemon + CLI
cargo build --release --features gui,usb       # + GUI egui + bulk display
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all
```

## PipeWire tuning manuel

Si `pw-top` montre des xruns (ERR >0) :

```bash
# augmente quantum à 256
# édite ~/.config/pipewire/pipewire.conf.d/99-maschine-lowlatency.conf
# default.clock.quantum = 256
systemctl --user restart pipewire
```

Vérifie aussi :

```bash
cat /proc/asound/cards  # MK3 en card 2 ?
aplay -l
amidi -l
aconnect -l
pw-dump | grep -i maschine
```

## Désinstallation

```bash
sudo rm /etc/udev/rules.d/99-maschine-mk3.rules
rm ~/.config/pipewire/pipewire.conf.d/99-maschine-lowlatency.conf
rm ~/.config/wireplumber/wireplumber.conf.d/51-maschine-mk3.conf
rm ~/.local/bin/maschine-linux
```
