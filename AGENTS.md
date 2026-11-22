# Guide for agents and new contributors

This file is the entry point for any coding agent (or human) picking up the
project. Read it first, then `docs/STATUS.md` for what is done and what is
next. Everything here is verified against the code at the version in
`Cargo.toml`.

## What the project is

`maschine-linux` runs a Native Instruments **Maschine MK3** on Linux without the
official software: HID input (pads, buttons, 8 knobs, 4D encoder), both 480×272
colour screens over USB bulk, LEDs, audio (PipeWire/ALSA via CPAL), a groovebox
engine (groups A–H × 16 Sounds, patterns, scenes, song), instruments (sampler,
808 kit, TB-303, Kick Synth, Nexus wavetable), insert effects, a guitar amp on
the MK3 input, and an `egui` desktop GUI that mirrors the hardware.

**The product goal**: behave as close as possible to Maschine 3 + MK3 on
Windows, stay understandable from the two MK3 screens alone, and be fully
usable from the GUI without the controller. User-facing text is French.

## Repository map

| Path | Role |
|---|---|
| `crates/maschine-proto` | Pure codecs (no I/O): HID reports, LED reports, display bulk transfers (RGB565, even-width windows), colour palette |
| `crates/maschine-hardware` | USB/hidraw access, pad tracker (`hidraw.rs`), **`mapper.rs`**: hardware events → `MapperAction` (modes, latches, knob routing, LED state) |
| `crates/maschine-engine` | The domain. **`engine.rs`** (`Engine`, `EngineEvent`, `EngineSnapshot`, soft keys, plugin chain, EVENTS, sample editor), `project.rs` (serde model), `sampler.rs`, synths, `fx.rs` (insert effects), `limiter.rs`, **`plugin_pages.rs`** (parameter tables), **`display_render.rs`** (all MK3 screens), `screen_text.rs` / `screen_vector.rs` (text and anti-aliased shapes) |
| `crates/maschine-dsp` | Guitar amp DSP (oversampled tube preamp, tone stack, cabinet, gate, delay, tuner) |
| `crates/maschine-app` | The `maschine-linux` binary: CLI, HID loop, display thread, LED writer, audio (`audio.rs`), guitar runtime, GUI (`gui/`), updater |
| `crates/maschine-guitar-standalone` | Independent guitar FX rack (not part of the Maschine workflow) |
| `docs/` | `workflow.md` (behaviour reference), `USAGE.md`, `TESTING.md`, `design.md` (design system), `STATUS.md` (handoff), protocol and hardware docs |
| `packaging/` | udev rules, systemd user units, RPM spec, Arch PKGBUILD, metainfo, desktop file, PipeWire/WirePlumber configs |
| `.github/workflows/` | `ci.yml` (fmt, clippy, tests, MSRV 1.88, release build) and `release.yml` (tag `v*` → RPM, DEB, AppImage, tarball, GitHub release) |

## Build, test, check

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
RUSTFLAGS="-D warnings" cargo check -p maschine-engine --no-default-features --locked   # CI runs it with -D warnings
```

- System packages: ALSA, udev and JACK development headers (see CONTRIBUTING.md).
- MSRV **1.88** (CI job). `slice::as_chunks_mut` is used, do not go lower.
- Screens without hardware: `cargo run -p maschine-engine --example dump_screens -- /tmp/screens`
  writes every screen pair (PPM) — use it to look at any screen you change.
- GUI without hardware: `maschine-linux gui --no-hardware --no-audio`. Under
  Xvfb, run it with networking disabled or the updater will query GitHub.
- On the maintainer's machine there is no host toolchain: builds run in a
  docker image; that setup is local and not part of the repository.

## Invariants (do not break)

1. **One source of truth per concern.**
   - Display-button labels: `Engine::soft_key_labels()`; lit state:
     `Engine::soft_key_active()` (derived from the label). Screens, GUI mirror
     and button LEDs all read these. A button always does what its label says.
   - Knob parameters: tables in `plugin_pages.rs` (instruments, effects) and
     `Engine::sample_params()` (sample editor) feed **both** the on-screen knob
     slots and the knob routing. Never add a parallel table in the GUI.
   - GUI knobs send `HardwareEvent::KnobDelta` through the mapper; GUI pads and
     group buttons use the mapper's `LedState`.
2. **Screen layout**: on the MK3, display buttons 1–8 sit above the screens and
   knobs below. Each screen draws the button labels on its top band and the
   four knob slots on its bottom band. `finish_soft_keys` must be the **last**
   drawing call of a screen (it draws the band at the bottom and rotates the
   frame).
3. **Audio thread**: `Engine::render_audio` and everything it calls must not
   allocate, lock or do I/O. Effect buses and DSP states are (re)allocated in
   `Engine::sync_fx_buses` (called after every `handle_event`); recording
   buffers are reserved when recording starts.
4. **Engine API**: user actions go through `Engine::handle_event(EngineEvent)`
   (it bumps `revision` and resyncs effect buses). UIs read
   `Engine::snapshot()` — cheap (`Arc<Project>`), never mutate through it.
5. **Persistence**: `Project` is serde JSON; new fields need `#[serde(default)]`
   so older projects keep loading.
6. **Tests stay green and meaningful**: every new behaviour gets a test next to
   the code (`#[cfg(test)] mod tests`). Audio changes get level / NaN / bounds
   tests, not only "it compiles".

## Where to change what

| Task | Where |
|---|---|
| New display-button action | label in `soft_key_labels` (or the mode's label function), action in the matching `handle_*_soft_key`, lit rule in `soft_key_active` |
| New knob parameter (instrument / effect) | `plugin_pages.rs` table; screens and routing follow |
| New insert effect | `fx.rs` (`FxType`, `FxSlot::default_for`, DSP struct, `SlotDsp`), `plugin_pages::fx_pages`, visual in `display_render::draw_fx_visual`, shortcuts in `Engine::handle_fx_soft_key` |
| New MK3 screen | `display_render.rs` using the shared helpers (`header_colored`, `encoder_row`, `finish_soft_keys`, `panel`, `cell`, `screen_vector`), dispatch in `render_screens_with_guitar`, add it to `examples/dump_screens.rs` |
| Hardware button / knob routing | `mapper.rs` (`handle_*_button`, `handle_display_knob`, `handle_main_4d_encoder`), map the `MapperAction` in `maschine-app/src/main.rs` |
| GUI layout | `gui/app.rs` (header, rail, MK3 deck), views in `gui/views/`, tokens in `gui/theme.rs` (see `docs/design.md`) |

## Gotchas learned the hard way

- egui: a child created inside a horizontal layout inherits it — use
  `allocate_ui_with_layout(.., Layout::top_down(..), ..)` for columns; in a
  top panel use `horizontal_wrapped` (a `with_main_wrap` layout grows to the
  full height); split a slider from its label so wrapping works.
- RGB565 screens: `screen_vector::blend` dithers partial coverage; fully covered
  pixels get the exact colour (tests compare exact pixels).
- The master limiter adds 1 ms of latency: timing tests account for
  `master_limiter.latency_frames()`.
- The pad tracker keeps a firmly held pad pressed for 60 s without reports
  (`HELD_SILENCE_FALLBACK`); releasing a pad only releases the synth it plays.
- Display bulk windows need an even width (pixel pairs).
- The 808 kit sounds are synthesized (no file): they cannot be sliced.
- Synthesizers live in `Engine::rack` (one instance per Sound, synced from the
  project after every event). `Engine::acid_synth` / `kick_synth` /
  `wavetable_synth` are only the *editing* copies shown on the PLUG-IN pages;
  never render or trigger them.
- `SoundId` packs the group in the high byte: decode with `>> 8` / `& 0xFF`.
- Scene / pattern changes during playback are queued (`pending_*`), applied on
  the next bar in `advance` and written to the project by `commit_launches`
  (never from the audio thread).
- The CI sampler-free check runs with `-D warnings`: items only used under
  `#[cfg(feature = "sampler")]` need the same cfg.

## Conventions

- Commits: conventional prefix + French summary, e.g. `feat(fx): …`,
  `fix(guitar): …`; body explains the why. Keep commits focused.
- Comments: French in existing code, explain *why*. `CHANGELOG.md` is French;
  the AppStream metainfo is English.
- Version bump = separate `chore(release): vX.Y.Z` commit.

## Releasing

1. Feature commits on `master`, all checks green.
2. Bump `version` in the root `Cargo.toml`, run `cargo update --workspace --offline`.
3. Add `## [X.Y.Z] - date` to `CHANGELOG.md` (the release notes are extracted
   from it), a `<release>` entry in
   `packaging/io.github.timotheegrollier.maschine-linux.metainfo.xml`, update
   "Current state" in `README.md` and `docs/STATUS.md`.
4. Commit `chore(release): vX.Y.Z`, `git tag -a vX.Y.Z -m vX.Y.Z`, push
   `master` and **only that tag** (`git push origin vX.Y.Z`; never
   `git push --tags`: an old local tag differs from the remote one).
5. Watch CI and the Release workflow; the release must list the RPM, DEB,
   AppImage, tarball and `SHA256SUMS`.
