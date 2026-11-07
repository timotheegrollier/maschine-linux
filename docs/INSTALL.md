# Installation — Fedora 44 (PipeWire)

## System prerequisites

```bash
sudo dnf install @development-tools \
  alsa-lib-devel jack-audio-connection-kit-devel \
  pipewire-jack-audio-connection-kit-devel pipewire-devel \
  libusb1-devel hidapi-devel systemd-devel \
  clang pkgconf-pkg-config

# Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

## Quick install

```bash
git clone https://github.com/timotheegrollier/maschine-linux
cd maschine-linux
./scripts/install.sh
# or manually:
cargo build --release
sudo cp config/udev/99-maschine-mk3.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger
mkdir -p ~/.config/pipewire/pipewire.conf.d ~/.config/wireplumber/wireplumber.conf.d
cp config/pipewire/99-maschine-lowlatency.conf ~/.config/pipewire/pipewire.conf.d/
cp config/wireplumber/51-maschine-mk3.conf ~/.config/wireplumber/wireplumber.conf.d/
systemctl --user restart pipewire wireplumber
```

## Verification

```bash
maschine-linux probe
# should display: VID:PID 17cc:1600 hidraw open: OK

maschine-linux hid-dump --duration 5
# press pads → should display 0x01 / 0x02

wpctl status | grep -A2 Maschine
# should display "Pro Audio" not "Surround 4.0"
pw-top
# quantum 128, ERR 0
```

## Permissions

The `config/udev/99-maschine-mk3.rules` file uses `TAG+="uaccess"` (systemd-logind): no need to add your user to `plugdev` if you are in a graphical session. Otherwise:

```bash
sudo usermod -aG plugdev,audio $USER
# log out and back in
```

Check: `getfacl /dev/hidraw3` must contain `user:timo:rw-`.

## Build options

```bash
cargo build --release                          # daemon + CLI
cargo build --release --features gui,usb       # + egui GUI + bulk display
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

## Standalone GUI check

```bash
# Run without a connected MK3, or ignore a connected controller
./target/release/maschine-linux gui --no-hardware

# With hardware (optional)
./target/release/maschine-linux gui
```

The GUI starts with the engine, mouse/keyboard controls, virtual MIDI and
system audio. In `--no-hardware` mode, the MK3 is ignored and hot-plug
reconnection is disabled.

## Manual PipeWire tuning

If `pw-top` shows xruns (ERR >0):

```bash
# increase quantum to 256
# edit ~/.config/pipewire/pipewire.conf.d/99-maschine-lowlatency.conf
# default.clock.quantum = 256
systemctl --user restart pipewire
```

Also check:

```bash
cat /proc/asound/cards  # MK3 on card 2?
aplay -l
amidi -l
aconnect -l
pw-dump | grep -i maschine
```

## Uninstallation

```bash
sudo rm /etc/udev/rules.d/99-maschine-mk3.rules
rm ~/.config/pipewire/pipewire.conf.d/99-maschine-lowlatency.conf
rm ~/.config/wireplumber/wireplumber.conf.d/51-maschine-mk3.conf
rm ~/.local/bin/maschine-linux
```
