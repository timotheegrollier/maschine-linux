# Maschine Linux — MK3 sur Fedora

> **Objectif** : workstation musicale Linux inspirée du workflow Maschine, avec prise en charge de la **Maschine MK3** (`17cc:1600`) sous **Fedora / PipeWire**. L'application couvre les projets, groupes, sons, patterns, scènes, sections, séquenceur pas-à-pas, presets 808, import WAV simple, contrôles et écrans matériels. Le slicing, l'enregistrement et l'édition avancée des zones restent à construire.

Langage principal : **Rust** (temps-réel, sécurité, pas de GC). Audio : PipeWire-JACK (`quantum 128`).

## État actuel (0.6.0)

Workspace Cargo avec 4 crates principaux :

| Crate | Rôle |
|---|---|
| `maschine-proto` | Codec pur (sans I/O) : HID IN `0x01` (boutons, encodeur 4D 4 directions)/`0x02` (pads), HID OUT `0x80`/`0x81`, Bulk Display RGB565 brut avec fenêtres partielles alignées, palette couleurs |
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
target/release/maschine-linux

# Lancer en mode daemon headless avec logs détaillés
target/release/maschine-linux daemon --verbose

# Instrumentation temps-réel (latence HID→Engine + stats audio, toutes les 2 s)
target/release/maschine-linux daemon --debug-latency
target/release/maschine-linux daemon --debug-latency --stress  # + charge volontaire (diagnostic craquements)
```

## Architecture

Voir `docs/architecture.md` + `docs/protocol.md` + `docs/workflow.md` + `docs/USAGE.md`.

```
MK3 USB IF4 HID (0x01 boutons/encodeurs, 0x02 pads)
  └─► hid-poll ─► HardwareMapper ─► Engine (Transport + Synth + Sampler)
                                       ├─► GUI (snapshot d'état)
                                       └─► rendu LCD RGB565 ─► display_thread ─► MK3 IF5 Bulk
                                             (diff 30 Hz)       (fenêtres paires)

Engine ─► ALSA Seq (virtual) / JACK MIDI
PipeWire-JACK 48k/128 ─► callback audio Engine
```

## Workflow Maschine reproduit

```
Project (*.maschine-linux.json)
 ├─ Groups A-H × N banks (16 Sounds / Group, 1 Sound = 1 pad)
 │    ├─ Sound 1..16 : Drum Synth 808 ou sample WAV, ADSR, Filter, volume/pan
 │    │                mappings persistés: chemin, key/velocity range, accordage
 │    └─ Patterns[] : events (note, vel, length) + automation (1..8 mesures)
 ├─ Scenes[] : { GroupA: PatternId, GroupB: PatternId, ... }
 └─ Song : Sections[] -> SceneId + length (arranger Ideas/Song)
```

Voir [le workflow et son périmètre actuel](docs/workflow.md), [les tests](docs/TESTING.md) et [l'usage](docs/USAGE.md).

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
- [x] Import WAV mono/stéréo vers un Sound, waveform mise en cache, persistance et rechargement des chemins, ADSR et filtre biquad résonant
- [ ] Sample Editor MK3 : enregistrement, édition de plage, slicing et édition des zones sur le contrôleur
- [x] Workflow Song/Scènes : dupliquer/supprimer section, longueur de section 1/2/4/8, dupliquer/supprimer scène (ré-indexation des sections)
- [ ] Browser étendu: import kits SFZ/Hydrogen, tag search
- [ ] Tests latence automatisés <6ms round-trip @128

Contrib : `cargo fmt --all && cargo clippy --workspace -- -D warnings`
