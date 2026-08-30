# Architecture — maschine-linux

## Principes

- **Rust partout**, `edition 2021`, `MSRV 1.75`, `#![deny(warnings)]` en CI.
- **Temps-réel** : aucun `alloc`, `lock` ou `syscall` bloquant dans le callback audio. HID sur thread dédié `SCHED_FIFO` (ou `rtkit` via PipeWire), communication par `rtrb` / `crossbeam-channel` bounded lock-free.
- **Sans I/O dans `maschine-proto`** : codec pur, testable offline, fuzzable.
- **Module `maschine-hardware` isole l'USB** : stratégie `hidraw` d'abord (pas de `usbhid` detach, hotplug friendly, besoin seulement d'une règle udev), fallback `nusb`/`rusb` pour le bulk display qui n'a pas de driver.
- **Engine sans dépendance hardware** : peut tourner headless pour tests, CI, rendering offline.

## Crates & dépendances

```
maschine-proto  ──► maschine-hardware ──► maschine-app (binary)
      ▲                    ▲                     ▲
      └────────────────────┴──── maschine-engine ┘
             (domain, sampler, sequencer, state)
```

- `maschine-proto` : `thiserror`, `serde` (palette), zéro `unsafe` sauf helpers RGB565.
- `maschine-hardware` : `nusb` (ou `rusb` feature), `hidapi`, `midir`/`alsa`, `crossbeam-channel`, `parking_lot`, `log`.
- `maschine-engine` : `serde`, `serde_json`, `hound`, `rubato`, `cpal` (feature `audio`), `midly` pour SMF import.
- `maschine-app` : `clap`, `anyhow`, `env_logger`, `tokio` (daemon), `eframe`/`egui` (feature `gui`).

## Threads & priorités

| Thread | Prio | Affinité | Rôle |
|---|---|---|---|
| `hid-poll` | `SCHED_FIFO 70` (via `chrt` ou `rtkit`) | core isolé si possible | `poll` `/dev/hidraw3` (`bInterval 1ms`), parse `0x01`/`0x02`, push `HardwareEvent` |
| `display` | `SCHED_OTHER nice -5` | - | coalesce dirty rects 16×16, RLE, bulk `0x04` |
| `audio` | `SCHED_FIFO 80` (JACK/PipeWire) | - | `cpal`/`jack` callback : mixer Groups, voices, ADSR |
| `engine` | `SCHED_OTHER` | - | sequencer, swing, quantize, automation |
| `ui` | `SCHED_OTHER` | - | `egui` 60fps, lock-free snapshot du state |

Priorités vérifiables : `ps -eLo pid,cls,rtprio,comm | grep -E "maschine|pipewire|irq"`, `pw-top`, `rtcqs`.

## Flux de données

```
[HID 42B/64B] --parse--> HardwareEvent { Button{ id, pressed }, Pad{ idx 0-15, pressure 0x4000-0x4FFD, velocity }, Encoder{ id, delta|abs }, TouchStrip{ finger, pos }, KnobTouch{ id, touched } }
       │
       └─crossbeam bounded 1024─► Engine::handle_hardware(Event)
                                      ├─ mapping pad_idx -> Sound (Group focus)
                                      ├─ trigger voice (sampler) avec velocity
                                      └─ UI state : pad brightness
                                             │
                                      feedback queue
                                             └─► LedState + DisplayFrame ─► hid_output / bulk
```

Audio : `Engine::audio_callback(&mut [f32], frames, sample_rate)` appelé par `cpal`/`jack`. Ne fait que mixer les `Voice` actives (`Voice { sample: Arc<[f32]>, pos: f64, gain, pan, adsr }`).

## État & persistance

- `Project` sérialisé en JSON `*.maschine-linux.json` (schéma versionné `schema_version: 1`). Binaire : `bincode` optionnel pour cache.
- Migration : `serde` `#[serde(default)]` + `Project::migrate()`.
- Autosave : toutes les 30s + sur `SIGTERM`, fichier `~/.local/share/maschine-linux/autosave/`.

## Gestion d'erreurs

- `thiserror` pour erreurs typées (`ProtoError`, `HardwareError`, `EngineError`), `anyhow` seulement dans `maschine-app` (CLI).
- USB : retry exponentiel, guard `power-cycle needed` si bulk malformé (détecté par `header.0 != 0x84`).

## Tests

- `maschine-proto` : unit + proptest (fuzz reports aléatoires), vecteurs capturés depuis `usbmon` réel.
- `maschine-hardware` : mock `hidraw` via `tempfile` + `pipe`, test `nusb` seulement avec `#[ignore] --features hardware`.
- `maschine-engine` : golden files JSON, property tests sequencer.

## Sécurité

- Pas de `unsafe` sauf conversion RGB565 (`u16` BE).
- `udev` rule limite à `TAG+="uaccess"` (systemd-logind) pas de `MODE="0666"`.
- PipeWire quantum négocié, pas forcé en aveugle.

## Alternatives écartées

- `tokio` dans callback audio : non, latence + `alloc`.
- `JUCE` : GPL/commercial, C++, interop FFI lourde, pas nécessaire pour MVP Rust+cpal.
- `NIHIA IPC` : Win/Mac only, fragile.
