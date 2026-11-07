# Maschine Linux — MK3 on Fedora

> **Goal**: a Linux music workstation built around the Maschine workflow, with
> native support for the **Maschine MK3** (`17cc:1600`) on **Fedora / PipeWire**.
> The application covers projects, groups, sounds, patterns, scenes, sections,
> a step sequencer, 808 presets, simple WAV import, hardware controls, and both
> MK3 color displays. Advanced sampling, recording, and zone editing are still
> in progress.

Primary language: **Rust** (real-time safety, no GC). Audio: PipeWire-JACK
(`quantum 128`).

Maschine Linux is an independent project. It does not include Native
Instruments code or proprietary assets; it reproduces the documented workflow
and hardware behavior through a clean-room implementation.

## Current state (0.7.0)

Cargo workspace with four main crates:

| Crate | Responsibility |
|---|---|
| `maschine-proto` | Pure codec (no I/O): HID IN `0x01` (buttons, 4D encoder) / `0x02` (pads), HID OUT `0x80`/`0x81`, raw RGB565 Bulk Display with aligned partial windows, color palette |
| `maschine-hardware` | USB discovery (`hidraw` first, `nusb`/`rusb` fallback), real-time threads, complete hardware mapper, Pad/Step/Keyboard/Chords modes, latched navigation |
| `maschine-engine` | Domain: Project/Group/Sound/Pattern/Scene/Section, transport, multi-bar sequencer, 808 Drum Synth (16 instruments), Acid TB-303 synthesizer, 480×272 dual-LCD rendering |
| `maschine-app` | `maschine-linux` binary: native `egui` GUI, headless daemon, low-latency PipeWire audio, updater |

Real-time design: the engine snapshot is shared through `Arc` (copy-on-write,
O(1) reads), a revision counter only repaints changed state, VU meters are
computed in the audio callback, and the HID thread wakes the GUI immediately.
Audio uses a 256-frame buffer (64-frame period, about 1.3 ms) at 48 kHz, a
**soft master limiter** (no hard clipping), **sample-accurate note placement**
inside each block, and a drum kit regenerated at the device sample rate.
Instrumentation: `--debug-latency` (HID→Engine, real buffer size, misses,
errors) and `--stress` (deliberate engine-lock contention).

### Workflow and GUI parity in 0.7.0

- **Latched MK3 menus**: PATTERN, STEP, KEYBOARD, SCENE, EVENTS, MACRO,
  SETTINGS, and PLUGIN stay active after the first press and return to the
  default Pad workflow on a second press. Only one workflow menu is active at
  a time.
- **Hardware-driven navigation**: PATTERN, STEP, KEYBOARD, and SCENE open the
  corresponding GUI view through the engine `NavigateView` event and
  `nav_serial` counter.
- **Unified PLUGIN view**: the physical PLUGIN button opens the SOUND page;
  soft keys switch to the simplified TB-303 page, where advanced parameters
  are collapsed by default.
- **Responsive desktop GUI**: collapsible and resizable navigation/MK3 panels,
  compact layouts for small windows, a dedicated Keyboard/Performance view,
  and context banners for every hardware workflow.
- **Per-Sound sampler control**: polyphony and choke groups are applied per
  Sound, including voice stealing with a short fade.
- **Standalone mode**: `maschine-linux gui --no-hardware` ignores any connected
  MK3 and runs the complete GUI, engine, MIDI, and audio stack without hardware.

## Fedora 44 prerequisites

```bash
sudo dnf install alsa-lib-devel jack-audio-connection-kit-devel \
                 pipewire-jack-audio-connection-kit-devel \
                 libusb1-devel hidapi-devel systemd-devel \
                 clang pkgconf-pkg-config
# real-time + hidraw permissions
sudo usermod -aG jackuser,realtime,audio $USER  # then log out/in
# udev (copied from config/udev/)
sudo cp config/udev/99-maschine-mk3.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger

# PipeWire low latency (quantum 128 = 2.7 ms @48k)
mkdir -p ~/.config/pipewire/pipewire.conf.d ~/.config/wireplumber/wireplumber.conf.d
cp config/pipewire/99-maschine-lowlatency.conf ~/.config/pipewire/pipewire.conf.d/
cp config/wireplumber/51-maschine-mk3.conf ~/.config/wireplumber/wireplumber.conf.d/
systemctl --user restart pipewire pipewire-pulse wireplumber
pw-top  # ERR must stay at 0
```

## Build and run

```bash
# Optimized release build
cargo build --release

# Full test suite
cargo test --workspace

# GUI with a connected MK3 (hardware is optional)
target/release/maschine-linux gui

# Force standalone mode: ignore any connected MK3
target/release/maschine-linux gui --no-hardware

# Headless daemon with detailed logs
target/release/maschine-linux daemon --verbose

# Real-time instrumentation (HID→Engine latency + audio stats every 2 s)
target/release/maschine-linux daemon --debug-latency
target/release/maschine-linux daemon --debug-latency --stress
```

Useful GUI shortcuts: `I` Ideas, `S` Sequencer, `K` Keyboard, `M` Mixer,
`A` Sampler, `T`/`P` Plugin/TB-303, `B` Browser, `Space` Play/Stop,
`F1` toggle navigation rail, `F2` toggle MK3 panel, `Ctrl+S` save.

## Architecture

See `docs/architecture.md`, `docs/protocol.md`, `docs/workflow.md`, and
`docs/USAGE.md`.

```
MK3 USB IF4 HID (0x01 buttons/encoders, 0x02 pads)
  └─► hid-poll ─► HardwareMapper ─► Engine (Transport + Synth + Sampler)
                                       ├─► GUI (state snapshot)
                                       └─► RGB565 LCD rendering ─► display_thread ─► MK3 IF5 Bulk
                                             (30 Hz diff)          (even-width windows)

Engine ─► ALSA Seq (virtual) / JACK MIDI
PipeWire-JACK 48k/128 ─► Engine audio callback
```

## Reproduced Maschine workflow

```
Project (*.maschine-linux.json)
 ├─ Groups A-H × N banks (16 Sounds / Group, 1 Sound = 1 pad)
 │    ├─ Sound 1..16: Drum Synth 808 or WAV sample, ADSR, filter, volume/pan
 │    │                persisted mappings: path, key/velocity range, tuning
 │    └─ Patterns[]: events (note, velocity, length) + automation (1..8 bars)
 ├─ Scenes[]: { GroupA: PatternId, GroupB: PatternId, ... }
 └─ Song: Sections[] -> SceneId + length (Ideas/Song arranger)
```

See [the workflow and current scope](docs/workflow.md),
[the test matrix](docs/TESTING.md), and [usage](docs/USAGE.md).

## License

GPL-3.0-or-later. Forks must publish their source code (AGPL is intentionally
not used).

## Roadmap

- [x] Protocol codec + tests
- [x] Hardware hidraw + nusb bulk, retry/power-cycle guard
- [x] ALSA Seq bridge & JACK MIDI
- [x] Engine: JSON project, transport, multi-bar step sequencer, swing, STEP mode (editable step cursor: VEL/POS/LEN/PITCH with knobs, full-grid scroll)
- [x] Real-time 808 Drum Synth (16 modeled analog instruments)
- [x] Real-time Acid TB-303 synthesizer (PolyBLEP saw/square, nonlinear 4-pole resonant filter with tanh saturation, analog distortion, stereo ping-pong delay, dual LCD tab 4, full GUI view with virtual keyboard)
- [x] egui GUI: 16-track sequencer, 4×4 pad grid, sound/filter editor, 8-group mixer, Keyboard/Performance view, responsive panels
- [x] MK3 displays: dual 480×272 real-time color LCD (ARRANGE, PATTERN/STEP, SONG, SCENES, MIXER, SAMPLER, BROWSER, EVENTS, MACRO, SETTINGS, PLUGIN) with contextual soft keys, live values, and immediate refresh (~30 FPS max)
- [x] Responsiveness: `Arc<Project>` copy-on-write snapshots, revision-driven repaint, real group/master VU meters with peak hold, `--debug-latency`
- [x] 4-D encoder, bar selection, and sound preview
- [x] WAV import (mono/stereo) into a Sound, cached waveform, path persistence and reload, ADSR and resonant biquad filter
- [x] Latched PATTERN/STEP/KEYBOARD/SCENE/EVENTS/MACRO/SETTINGS/PLUGIN navigation on MK3 and in the GUI
- [x] Unified PLUGIN SOUND/TB-303 page, simple macros and collapsible advanced controls
- [x] Per-Sound polyphony and choke groups with click-free voice stealing
- [x] Standalone GUI mode without hardware (`--no-hardware`)
- [ ] MK3 Sample Editor: recording, play-range editing, slicing, and zone editing on the controller
- [x] Song/Scenes: duplicate/delete sections, section lengths 1/2/4/8, duplicate/delete scenes (with section re-indexing)
- [ ] Extended Browser: SFZ/Hydrogen kit import, tag search
- [ ] Automated latency tests <6 ms round-trip @128

Contributing: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
