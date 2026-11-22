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

## Current state (0.19.3)

Cargo workspace with six crates:

| Crate | Responsibility |
|---|---|
| `maschine-proto` | Pure codec (no I/O): HID IN `0x01` (buttons, 4D encoder) / `0x02` (pads), HID OUT `0x80`/`0x81`, raw RGB565 Bulk Display with aligned partial windows, color palette |
| `maschine-hardware` | USB discovery (`hidraw` first, `nusb`/`rusb` fallback), real-time threads, complete hardware mapper, Pad/Step/Keyboard/Chords modes, latched navigation |
| `maschine-engine` | Domain: Project/Group/Sound/Pattern/Scene/Section, transport, multi-bar sequencer, 808 Drum Synth (16 instruments), Acid TB-303, Kick Synth and Nexus wavetable synthesizers, insert effects on Sounds / Groups / Master, master limiter, 480×272 dual-LCD rendering (anti-aliased text and shapes) |
| `maschine-app` | `maschine-linux` binary: native `egui` GUI, headless daemon, low-latency PipeWire audio, guitar amp monitoring on the MK3 input, updater |
| `maschine-dsp` | Real-time guitar DSP: 4x-oversampled multi-model tube preamp (Vox AC30, Fender Blackface/Tweed, High Gain), correct click-free biquad tone stacks, cabinet simulation, gate, delay, tuner (YIN), FFT spectrum, lock-free atomic parameters |
| `maschine-guitar-standalone` | Optional standalone guitar FX rack (JACK/CPAL audio, `egui` GUI); built by the workspace but independent from the Maschine workflow |

Real-time design: the engine snapshot is shared through `Arc` (copy-on-write,
O(1) reads), a revision counter only repaints changed state, VU meters are
computed in the audio callback, and the HID thread wakes the GUI immediately.
Audio uses a 256-frame buffer (64-frame period, about 1.3 ms) at 48 kHz, a
**transparent master limiter** (-1 dBFS ceiling, 1 ms look-ahead, untouched below), **sample-accurate note placement**
inside each block, and a drum kit regenerated at the device sample rate.
Instrumentation: `--debug-latency` (HID→Engine, real buffer size, misses,
errors) and `--stress` (deliberate engine-lock contention).

### 16 Velocities, arpeggiator modes and multi-note editing in 0.17.0

- Shift + Fixed Vel plays the selected Sound at sixteen velocities across the
  pads, like the MK3.
- Arpeggiator modes on knob 7 (up, down, up-down, one or two octaves).
- Events mode edits every note of the Sound at once (knob 6).

### Selected group LED more lit in 0.19.3

- The selected group's LED is the most lit of all groups; the other group
  buttons are dimmed, even when they play.

### Pads played in the pattern light up in keyboard mode in 0.19.2

- The pads of the notes played in the pattern light up according to the
  notes in the loop.

### MK3 LEDs — MUTE button off mode-less, selected group and pad brighter in 0.19.1

- The MUTE button lights only while mute mode is active (or MUTE+group).
- The selected group's LED is brighter than the others.
- The selected pad is brighter than the others.

### Slides recorded during recording in 0.19.0

- Pressing a pad (TB-303, synths) while another is held during recording
  extends the previously recorded event up to the new one — the TB-303
  portamento is recorded, not cut directly.
- Shift + Undo: notes recorded during recording push the project history, so
  Shift + Pad 1 undoes them.

### Note lengths on held pads and knob automations recorded in 0.18.0

- Notes recorded during recording keep the length of the note held: long press
  on a synth pad (synth, TB-303, Nexus…) or a keyboard note (Keyboard /
  Chords) — the pattern event takes the held duration, and the played
  velocity and pitch are recorded.
- Knob automations: during recording, turning a knob of the focused Sound
  (PLUGIN tab) records automation points in the active pattern (new
  `SoundAutomation` model, persisted in the project).

### Arpeggiator and safer recording in 0.16.0

- Note Repeat in Keyboard and Chords modes is an arpeggiator (held notes,
  upwards, at the chosen rate), recorded with REC.
- Notes repeated while recording are written outside the audio thread; sample
  edits (reverse, normalize, loop) survive reopening the project.

### Groups, launches and LEDs in 0.15.0

- Every group plays at once, synthesizers included (one TB-303 / Kick / Nexus
  instance per Sound), with working meters, MUTE and SOLO everywhere.
- Scenes and patterns chosen during playback start on the next bar.
- Pads and group buttons light like on the MK3 (dim, normal, bright, blinking
  when queued); the MK3 shows startup progress.
- Keyboard and Chords modes follow the MK3: C3 chromatic by default, OCT / SEMI
  buttons, root pads lit, chords that stay in the chosen scale.

### Sample editor and EVENTS in 0.14.0

- **SAMPLING** is the MK3 sample editor: RECORD from the MK3 input onto the
  focused pad, EDIT the play range / tuning / gain / envelope, SLICE on a grid
  or at detected hits and spread the slices over the pads, ZONE for root key,
  key and velocity ranges.
- **EVENTS** edits the notes of the pattern one by one (position, note,
  velocity, length, delete, duplicate, quantize, nudge).
- New contributors and coding agents: start with [AGENTS.md](AGENTS.md) and
  [docs/STATUS.md](docs/STATUS.md).

### PLUG-IN chain, effects and redesign in 0.13.0

- **Every plugin parameter on the MK3**: Sampler, TB-303, Kick Synth and Nexus
  pages on both screens, eight knobs per page with name and value above each
  knob, pages switched with `Page ◄ ►` (lit when a page exists) and a short
  transition banner.
- **Plugin chain like the official software**: the 4D encoder walks through
  the instrument, then the insert effects of the Sound, its Group and the
  Master; a free `+ effet` slot inserts **Filter, EQ, Distortion, Compressor,
  Lo-Fi, Chorus, Delay or Reverb** (0.13.1: EQ, Chorus, Lo-Fi); `< TYPE`, `TYPE >`, `BYPASS` and `SUPPR` on the buttons,
  knob 1 always MIX. Effects really process the audio, each slot with its own
  state.
- **Cleaner sound everywhere**: transparent master limiter instead of a
  saturating waveshaper, rewritten effects (TPT filter, anti-aliased
  distortion, soft-knee compressor, smoothed delay, Freeverb reverb), Hermite
  interpolation in the sampler, PolyBLEP oscillators in Nexus, and a guitar amp
  whose oversampler read one input in four (-12 dB, aliasing) — fixed, with
  level-compensated gain and a 24 dB/oct cabinet.
- **Redesign of the screens and the GUI** around one design system
  ([docs/design.md](docs/design.md)): Barlow Semi Condensed on both, accent =
  colour of the focused group, anti-aliased curves (tiny-skia); the GUI gets a
  single header, a colour-coded group rail and an MK3 deck at the bottom with
  both screens side by side, knobs and pads. Nothing is clipped at any window
  size.
- **Held notes and LEDs**: a pad held firmly keeps its note, releasing one pad
  no longer cuts a synth note held on another, pads light in their Sound colour.
- **Standby screen** on quit and on crash: almost black, a dim word mark — and
  since 0.13.1 as soon as the MK3 is plugged in, even before the first launch
  (udev rule + `maschine-linux-standby.service`, installed by the packages).

### Guitar amp on the MK3 input

The `AMPLI GUITARE` view (`G` shortcut) turns the MK3 audio input into a real
tube-amp signal chain, monitored through the normal MK3 outputs. It is **OFF by
default** (`ACTIF` + `MONITOR` must be enabled) and only ever processes the MK3
capture stream — pattern instruments are not routed through it and their samples
are left untouched:

```
jack guitare → MK3 input (CPAL capture) → SPSC ring → gate
             → tube preamp (4x oversampling) → tone stack → cabinet → delay
             → mix into Maschine output (main 1/2 + headphones 3/4)
```

- Four amp voices: **Vox AC30**, **Fender Blackface**, **Fender Tweed** and
  **High Gain**, each with its own gain stages, tone stack and power-amp sag.
- Controls: Gain, Bass, Mid, Treble, Presence, Sag, Master and Monitor level,
  editable from the GUI or the eight contextual MK3 knobs.
- **Both MK3 screens switch to the amp** (PLUGIN page 2): `PREAMP` on the left
  (saturation curve, 3-band tone stack, knobs 1–4 Gain/Bass/Mid/Treble, soft keys
  `AMP <`/`AMP >`/`CAB <`/`CAB >`) and `ANALYSE` on the right (IN/OUT VU, YIN
  tuner, 48-band FFT, CPU/latency/Xruns, knobs 5–8 Presence/Sag/Master/Monitor,
  soft keys `BYPASS`/`MONITOR`/`RESET`/`PAGE SND`). The GUI panel is a mirror of
  the very same frames.
- Diagnostics: input/output VU meters, real DSP CPU load, estimated latency,
  clipping counter, Xruns, YIN tuner (note + cents) and a live FFT spectrum.
- The capture stream is opened from the MK3 when available (WirePlumber
  `pro-audio` profile, see below), with fallback to the system default input.
  Failure to open an input never prevents the rest of the app from running.

### Workflow and GUI parity in 0.10.1

- **Complete PATTERN mode**: empty pads create pattern slots, `DUPLICATE`
  copies the active pattern, `SHIFT + ERASE` clears its events, and the 4D
  encoder steps through patterns. The soft keys manage creation, duplication,
  clearing, deletion, and length.
- **Pattern manager and Ideas matrix**: the Sequencer view exposes a full
  pattern strip with inline rename, and the Ideas matrix shows 8–16 slots per
  Group. Empty slots can be clicked to create a pattern and assign it to the
  active Scene.
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
- **Responsive, hideable GUI**: the navigation rail (`F1`), centre workspace
  (`F3`), and MK3 mirror (`F2`) keep their share of the window when it is
  resized and can be hidden independently; `TOUT` toggles all three panels.
  The mirror follows the controller (display buttons, screens, knobs, groups,
  pads): its knobs use the hardware mapping and its pads the hardware LED
  colours. With the centre hidden, the MK3 fills the window.
- **MK3 screen layout**: as on the official controller, each display button's
  label sits along the top edge of the screens (under the button) and each
  knob's name and value along the bottom edge (above the knob); lit labels and
  button LEDs follow the same state.
- **Graceful MK3 shutdown**: closing the app joins the HID/display/LED threads,
  sends a shutdown splash, releases the display interface without resetting the
  device, and turns the pad LEDs off so the MK3 returns to its default logo and
  idle state.
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

Requires **Rust 1.88** or newer: this is the `rust-version` declared in
`Cargo.toml` (inherited by every workspace crate), and the `msrv` job in CI
compiles the workspace with exactly that toolchain.

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

# List output AND input devices (MK3 capture port for the guitar amp)
target/release/maschine-linux audio-list

# Real-time instrumentation (HID→Engine latency + audio stats every 2 s)
target/release/maschine-linux daemon --debug-latency
target/release/maschine-linux daemon --debug-latency --stress
```

Useful GUI shortcuts: `I` Ideas, `S` Sequencer, `K` Keyboard, `M` Mixer,
`A` Sampler, `T`/`P` Plugin/TB-303, `G` Guitar Amp, `B` Browser, `Space` Play/Stop,
`F1` toggle navigation rail, `F2` toggle MK3 panel, `F3` toggle centre
workspace, `Ctrl+S` save, `Ctrl+Shift+S` quick save.

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

MK3 input (jack guitare) ─► CPAL capture ─► SPSC ring ─► maschine-dsp
                            (tube amp + cab) ─► mix ─► CPAL output (MK3)
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
[the test matrix](docs/TESTING.md), [usage](docs/USAGE.md), and [CI/CD & release guide](docs/CI_CD_RELEASE.md).

## License

GPL-3.0-or-later. Forks must publish their source code (AGPL is intentionally
not used).

The interface and MK3 screens use **Barlow Semi Condensed** by Jeremy Tribby,
distributed under the SIL Open Font License 1.1
(`crates/maschine-engine/assets/fonts/OFL.txt`).

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
