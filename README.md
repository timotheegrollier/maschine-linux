# Maschine Linux — MK3 sur Fedora

> **Objectif** : Remplacement libre, fluide et sans latence du logiciel Maschine (Windows) pour la **Maschine MK3** (`17cc:1600`) sous **Fedora / PipeWire**. Workflow identique à Maschine : `Project > Groups (16 Sounds) > Patterns > Scenes > Sections (Song)` + Sampler/Zones/Slicing + Step sequencer + Browser + Écrans & LEDs.

Langage principal : **Rust** (temps-réel, sécurité, pas de GC). Audio : PipeWire-JACK (`quantum 128`).

## État actuel (0.4.0)

Workspace Cargo avec 4 crates principaux :

| Crate | Rôle |
|---|---|
| `maschine-proto` | Codec pur (sans I/O) : HID IN `0x01` (boutons, encodeur 4D 4 directions)/`0x02` (pads), HID OUT `0x80`/`0x81`, Bulk Display RGB565 RLE, palette couleurs |
| `maschine-hardware` | Découverte USB (`hidraw` + `nusb`/`rusb` fallback), threads temps-réel, mapper hardware complet, modes Pad/Step/Keyboard/Chords |
| `maschine-engine` | Domaine : Project/Group/Sound/Pattern/Scene/Section, Transport, Sequencer multi-mesures, Drum Synth 808 (16 instruments), Rendu double LCD 480×272 |
| `maschine-app` | Binaire `maschine-linux` : Application native avec GUI `egui`, daemon headless, PipeWire low-latency |

Temps-réel : l'instantané moteur est partagé en `Arc` (copy-on-write, lectures O(1)), un compteur de révision ne redessine que ce qui a changé, les VU-mètres sont calculés dans le callback audio et le thread HID réveille la GUI instantanément. Audio : buffer 256 frames (période 64 ≈ 1,3 ms) à 48 kHz, **limiteur doux master** (jamais d'écrêtage dur), notes placées **au sample près** dans le bloc, kit DSP régénéré au taux réel du device. Instrumentation : `--debug-latency` (HID→Engine, buffer réel, misses, erreurs) et `--stress` (charge volontaire).

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
# Compilation release optimisée
cargo build --release

# Lancer la suite complète de tests
cargo test --workspace

# Lancer l'application avec GUI et détection hardware automatique
sudo target/release/maschine-linux

# Lancer en mode daemon headless avec logs détaillés
sudo target/release/maschine-linux daemon --verbose

# Instrumentation temps-réel (latence HID→Engine + stats audio, toutes les 2 s)
sudo target/release/maschine-linux daemon --debug-latency
sudo target/release/maschine-linux daemon --debug-latency --stress  # + charge volontaire (diagnostic craquements)
```

## Architecture

Voir `docs/architecture.md` + `docs/protocol.md` + `docs/workflow.md` + `docs/USAGE.md`.

```
MK3 USB IF4 HID (0x01 buttons 42B / 0x02 pads 64B) ─┐
MK3 USB IF5 Bulk (2×480×272 RGB565) ────────────────┤─► hid_thread (SCHED_FIFO, poll 1ms)
                                                     │   lock-free ringbuf (rtrb)
                                                     └─► maschine-hardware::Mapper
                                                          ├─► ALSA Seq (virtual)
                                                          ├─► JACK MIDI
                                                          └─► Engine (Transport + Drum Synth + Sampler)
                                                                    ▲
PipeWire-JACK 48k/128 ───────────────────────────────────────────────┘
```

## Workflow Maschine reproduit

```
Project (*.maschine-linux.json)
 ├─ Groups A-H × N banks (16 Sounds / Group, 1 Sound = 1 pad)
 │    ├─ Sound 1..16 : chain Instrument (808 Drum Synth / Sampler) + FX
 │    │                zones: lokey/hikey/lovel/hivel, choke off_by, ADSR, Filter
 │    └─ Patterns[] : events (note, vel, length) + automation (1..8 mesures)
 ├─ Scenes[] : { GroupA: PatternId, GroupB: PatternId, ... }
 └─ Song : Sections[] -> SceneId + length (arranger Ideas/Song)
```

Voir `docs/workflow.md` pour les détails Sampler/Slice/Zone.

## Licence

GPL-3.0-or-later. Forks doivent publier les sources (AGPL non utilisée volontairement).

## Roadmap

- [x] Proto codec + tests
- [x] Hardware hidraw + nusb bulk, retry/power-cycle guard
- [x] ALSA Seq bridge & JACK MIDI
- [x] Engine: Project JSON, Transport, Step sequencer multi-mesures, swing, mode STEP (curseur de pas éditable : VEL/POS/LEN/PITCH au knob, scroll de la grille complète)
- [x] Drum Synth 808 temps-réel (16 instruments analogiques modélisés)
- [x] GUI egui: Séquenceur 16 pistes, Grille 4×4 pads, Éditeur son/filtres, Mixer 8 groupes
- [x] Écrans MK3: Double écran LCD couleur 480×272 temps-réel (ARRANGE, PATTERN/STEP, SONG, SCENES, MIXER, SAMPLER, BROWSER, EVENTS, MACRO, SETTINGS) avec soft keys contextuels (labels = actions réelles), valeurs live et rafraîchissement immédiat à chaque action (~30 FPS maxi)
- [x] Réactivité: snapshots `Arc<Project>` copy-on-write, repaint piloté par révision, VU-mètres réels par groupe + master (peak hold), `--debug-latency`
- [x] Encodeur 4-D 4 directions & sélection de mesure / sound preview
- [x] Sampler: zones, choke, ADSR, filtre biquad résonant
- [x] Workflow Song/Scènes : dupliquer/supprimer section, longueur de section 1/2/4/8, dupliquer/supprimer scène (ré-indexation des sections)
- [ ] Browser étendu: import kits SFZ/Hydrogen, tag search
- [ ] Tests latence automatisés <6ms round-trip @128

Contrib : `cargo fmt --all && cargo clippy --workspace -- -D warnings`
