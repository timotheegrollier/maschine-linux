# Maschine Linux — MK3 sur Fedora

> **Objectif** : Remplacement libre, fluide et sans latence du logiciel Maschine (Windows) pour la **Maschine MK3** (`17cc:1600`) sous **Fedora / PipeWire**. Workflow identique à Maschine : `Project > Groups (16 Sounds) > Patterns > Scenes > Sections (Song)` + Sampler/Zones/Slicing + Step sequencer + Browser + Écrans & LEDs.

Langage principal : **Rust** (temps-réel, sécurité, pas de GC). Audio : PipeWire-JACK (`quantum 128`).

## État actuel (0.1.0)

Workspace Cargo avec 4 crates :

| Crate | Rôle |
|---|---|
| `maschine-proto` | Codec pur (sans I/O) : HID IN `0x01`/`0x02`, HID OUT `0x80`/`0x81`, Bulk Display RGB565 RLE, palette couleurs |
| `maschine-hardware` | Découverte USB (`hidraw` + `nusb`/`rusb` fallback), threads temps-réel, bridge ALSA Seq / JACK MIDI |
| `maschine-engine` | Domaine : Project/Group/Sound/Pattern/Scene/Section, Transport, Sequencer (swing, quantize), Sampler (zones, choke, ADSR), State JSON |
| `maschine-app` | Binaire `maschine-linux` : CLI, daemon, (future) GUI `egui`, configs PipeWire/WirePlumber |

## Prérequis Fedora 44

```bash
sudo dnf install alsa-lib-devel jack-audio-connection-kit-devel \
                 pipewire-jack-audio-connection-kit-devel \
                 libusb1-devel hidapi-devel systemd-devel \
                 clang pkgconf-pkg-config
# droits temps-réel + hidraw
sudo usermod -aG jackuser,realtime,audio $USER  # + relogin
# udev (copié depuis config/udev/)
sudo cp config/udev/99-maschine-mk3.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger

# PipeWire low-latency (quantum 128 = 2.7ms @48k)
mkdir -p ~/.config/pipewire/pipewire.conf.d ~/.config/wireplumber/wireplumber.conf.d
cp config/pipewire/99-maschine-lowlatency.conf ~/.config/pipewire/pipewire.conf.d/
cp config/wireplumber/51-maschine-mk3.conf ~/.config/wireplumber/wireplumber.conf.d/
systemctl --user restart pipewire pipewire-pulse wireplumber
pw-top  # ERR doit rester à 0
```

## Build & Run

```bash
cargo build --release
# test sans hardware (proto + engine)
cargo test --workspace

# avec MK3 branchée — daemon HID -> MIDI
cargo run -p maschine-app -- --daemon --verbose
# vérif MIDI
aconnect -l  # doit afficher "Maschine MK3: Controller Out/In"
amidi -l

# GUI (quand disponible)
cargo run -p maschine-app -- --gui

# Probe hardware seule
cargo run -p maschine-app -- probe
cargo run -p maschine-app -- hid-dump --duration 5
```

## Architecture

Voir `docs/architecture.md` + `docs/protocol.md` + `docs/workflow.md`.

```
MK3 USB IF4 HID (0x01 buttons 42B / 0x02 pads 64B) ─┐
MK3 USB IF5 Bulk (2×480×272 RGB565) ────────────────┤─► hid_thread (SCHED_FIFO, poll 1ms)
                                                     │   lock-free ringbuf (rtrb)
                                                     └─► maschine-hardware::Mapper
                                                          ├─► ALSA Seq (virtual)
                                                          ├─► JACK MIDI
                                                          └─► Engine (Transport + Sampler)
                                                                    ▲
PipeWire-JACK 48k/128 ───────────────────────────────────────────────┘
```

## Workflow Maschine reproduit

```
Project (*.maschine-linux.json)
 ├─ Groups A-H × N banks (16 Sounds / Group, 1 Sound = 1 pad)
 │    ├─ Sound 1..16 : chain Instrument (Sampler/Audio) + FX
 │    │                zones: lokey/hikey/lovel/hivel, choke off_by, ADSR
 │    └─ Patterns[] : events (note, vel, length) + automation
 ├─ Scenes[] : { GroupA: PatternId, GroupB: PatternId, ... }
 └─ Song : Sections[] -> SceneId + length (arranger Ideas/Song)
```

Voir `docs/workflow.md` pour les détails Sampler/Slice/Zone.

## Licence

GPL-3.0-or-later. Forks doivent publier les sources (AGPL non utilisée volontairement).

## Roadmap 0.1 → 1.0

- [x] Proto codec + tests
- [ ] Hardware hidraw + nusb bulk, retry/power-cycle guard
- [ ] ALSA Seq bridge, JACK opt
- [ ] Engine: Project JSON, Transport, Step sequencer, swing
- [ ] Sampler: SFZ/hound, zones, choke, ADSR, disk streaming stub
- [ ] GUI egui: Ideas/Song grid, Pattern editor, Mixer
- [ ] Écrans MK3: dirty-rect RLE, 30-60 fps
- [ ] Browser: SFZ/Hydrogen kit import, tag search
- [ ] Tests latence: <6ms round-trip @128

Contrib : `cargo fmt --all && cargo clippy --workspace -- -D warnings`
