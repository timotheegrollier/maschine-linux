# Architecture — maschine-linux

## Principles

- **Rust everywhere**, `edition 2021`, `MSRV 1.75`, `#![deny(warnings)]` in CI.
- **Real-time**: no `alloc`, `lock` or blocking `syscall` in the audio callback. HID on a dedicated `SCHED_FIFO` thread (or `rtkit` via PipeWire), communication through bounded lock-free `rtrb` / `crossbeam-channel`.
- **No I/O in `maschine-proto`**: pure codec, offline-testable, fuzzable.
- **`maschine-hardware` module isolates USB**: `hidraw` strategy first (no `usbhid` detach, hotplug friendly, only needs a udev rule), `nusb`/`rusb` fallback for the bulk display, which has no driver.
- **Engine with no hardware dependency**: can run headless for tests, CI, offline rendering.

## Crates & dependencies

```
maschine-proto  ──► maschine-hardware ──► maschine-app (binary)
      ▲                    ▲                     ▲
      └────────────────────┴──── maschine-engine ┘
             (domain, sampler, sequencer, state)
```

- `maschine-proto`: `thiserror`, `serde` (palette), zero `unsafe` except RGB565 helpers.
- `maschine-hardware`: `nusb` (or `rusb` feature), `hidapi`, `midir`/`alsa`, `crossbeam-channel`, `parking_lot`, `log`.
- `maschine-engine`: `serde`, `serde_json`, `hound`, `rubato`, `cpal` (`audio` feature), `midly` for SMF import.
- `maschine-app`: `clap`, `anyhow`, `env_logger`, `tokio` (daemon), `eframe`/`egui` (`gui` feature).

## Threads & priorities

| Thread | Priority | Affinity | Role |
|---|---|---|---|
| `hid-poll` | `SCHED_FIFO 70` (via `chrt` or `rtkit`) | isolated core if possible | `poll` `/dev/hidraw3` (`bInterval 1ms`), parse `0x01`/`0x02`, push `HardwareEvent` |
| `display` | `SCHED_OTHER nice -5` | - | compare renders at 30 Hz, merge RGB565 rectangles into even-width regions, 1.6 MB/s budget, bulk `0x04` |
| `audio` | `SCHED_FIFO 80` (JACK/PipeWire) | - | `cpal`/`jack` callback: mix Groups, voices, ADSR |
| `engine` | `SCHED_OTHER` | - | sequencer, swing, quantize, automation |
| `ui` | `SCHED_OTHER` | - | `egui` 60fps, lock-free state snapshot |

Verifiable priorities: `ps -eLo pid,cls,rtprio,comm | grep -E "maschine|pipewire|irq"`, `pw-top`, `rtcqs`.

## Data flow

```
[HID 42B/64B] --parse--> HardwareEvent { Button{ id, pressed }, Pad{ idx 0-15, pressure 0x4000-0x4FFD, velocity }, Encoder{ id, delta|abs }, TouchStrip{ finger, pos }, KnobTouch{ id, touched } }
       │
       └─crossbeam bounded 1024─► Engine::handle_hardware(Event)
                                      ├─ mapping pad_idx -> Sound (Group focus)
                                      ├─ trigger voice (sampler) with velocity
                                      └─ UI state: pad brightness
                                             │
                                      feedback queue
                                             └─► LedState + DisplayFrame ─► hid_output / bulk
```

Audio: `Engine::audio_callback(&mut [f32], frames, sample_rate)` called by `cpal`/`jack`. It only mixes active voices (`Voice { sample: Arc<[f32]>, pos: f64, gain, pan, adsr }`).

## State & persistence

- `Project` serialized as JSON `*.maschine-linux.json` (versioned schema `schema_version: 1`). Binary: optional `bincode` for cache.
- Migration: `serde` `#[serde(default)]` + `Project::migrate()`.
- Save: `Ctrl+S` or the File/Save button writes `~/.local/share/maschine-linux/autosave.json`; a save is also made on close.

## Hardware optionality

The engine has no hardware dependency. The GUI can run with a connected MK3,
without any controller, or with `--no-hardware` to ignore a connected device
explicitly. In standalone mode the hot-plug reconnection thread is disabled,
while the virtual MIDI port, audio backend and GUI remain active. The headless
daemon keeps requiring a controller by design.

## Error handling

- `thiserror` for typed errors (`ProtoError`, `HardwareError`, `EngineError`), `anyhow` only in `maschine-app` (CLI).
- Display USB: exponential retry, 2×1 pixel recovery probe, even-width rectangles and gentle interface re-acquisition after repeated errors.

## Tests

- `maschine-proto`: HID/LCD codec tests, differential and RGB565 reconstruction.
- `maschine-hardware`: HID input replay, control mapping, and pure tests of USB short-write recovery.
- `maschine-engine`: sequencer, transport, WAV mapping/persistence, DSP and screen transitions.
- `maschine-app`: HID/GUI workflow integration and display probe.
- The commands required by CI and the physical checklist are detailed in [TESTING.md](TESTING.md).

## Security

- No `unsafe` except RGB565 conversion (`u16` BE).
- `udev` rule limited to `TAG+="uaccess"` (systemd-logind), no `MODE="0666"`.
- PipeWire quantum negotiated, not blindly forced.

## Rejected alternatives

- `tokio` in the audio callback: no, latency + `alloc`.
- `JUCE`: GPL/commercial, C++, heavy FFI interop, not necessary for a Rust+cpal MVP.
- `NIHIA IPC`: Win/Mac only, fragile.
