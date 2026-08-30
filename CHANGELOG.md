# Changelog

Tous les changements notables.

## [0.1.0] - 2026-08-30

### Ajouté

- Workspace Rust (`maschine-proto`, `maschine-hardware`, `maschine-engine`, `maschine-app`)
- `maschine-proto` : codec HID `0x01`/`0x02` + `0x80`/`0x81` + display bulk RGB565 RLE + palette indexée, tests complets
- `maschine-hardware` : découverte `hidraw` + thread poll 1ms + hystérésis pads + `HardwareMapper` (pads → MIDI NoteOn/Off, Groups, Transport, knobs CC, touchstrip), `BulkHandle` (feature `usb`)
- `maschine-engine` : modèle `Project` (Groups 16 Sounds, Patterns, Scenes, Sections), `Transport` (PPQ 960, BPM, swing, loop), `Sequencer` (quantize, nudge, humanize, slice), `Sampler` (zones, choke, ADSR, render stéréo), `Engine` (snapshot lock-free)
- `maschine-app` : CLI (`probe`, `hid-dump`, `daemon`, `project new/show/validate`, `led-test`, `gui` stub), daemon headless + heartbeat + autosave
- Configs : `99-maschine-mk3.rules` (udev `TAG+="uaccess"`), PipeWire `99-maschine-lowlatency.conf` (quantum 128), WirePlumber `51-maschine-mk3.conf/.lua` (pro-audio)
- Docs : `README.md`, `docs/architecture.md`, `docs/protocol.md`, `docs/workflow.md`, `docs/INSTALL.md`, `docs/USAGE.md`
- CI GitHub Actions (fmt, clippy, test, build), `rustfmt.toml`, `scripts/install.sh`
- Exemple `examples/demo.machine-linux.json`

### Testé

- MK3 `17cc:1600` sur Fedora 44, kernel 7.1.10, PipeWire 1.6.8
- `probe` détecte `hidraw3` + ALSA `card 2 MK3`
- `hid-dump` + `led-test` OK (write hidraw 0x80/0x81)
- `daemon` connecte, mappe pads/boutons, avance transport, render sampler
- `cargo test --workspace` 31 tests pass
