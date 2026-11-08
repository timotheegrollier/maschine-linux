# Changelog

All notable changes.

## [0.9.7] - 2026-11-08

### Added

- **Wavetable / Nexus Synth plugin**: 16-voice polyphonic synthesizer modeled with dual oscillators, 7-voice unison supersaw, acoustic and digital wavetables (Grand Piano, M1 Piano, Rhodes, Formants, Digital Waves), multimode SVF filter (LP12, LP24, HP, BP), dual ADSR envelopes, LFO modulation, stereo FX rack (Chorus, Ping-Pong Delay, Reverb, Distortion), and 28 factory presets (Concert Grand Piano, House M1 Piano, Vintage Upright Piano, Honky Tonk, Neo-Soul Rhodes, DX7 E-Piano, 808 Sub Boom, Reese Bass, Acid 303 Sub, Neurofunk Slither Bass, Slap Funk Bass, Cyberpunk Bass, Future House Donk, Titanium Supersaw Lead, Future Rave Lead, Hyperpop Chiptune Lead, Blade Runner Brass, Cyber-Drive Synth, Trance Euphoria, Celestial Strings, Analog Warmth, Cinematic Dark Pad, Spire Pluck, Pizzicato Strings, Kalimba Dream, Crystal Mallet, Deep House Organ, Hammond B3 Organ). Fully integrated in the Desktop GUI, Browser plugin selector, MK3 dual LCD screens, and hardware knob controls.
- **Kick Synth Modeler enhancements**: extended body waveforms (Sine, Triangle, SatSine, SubSquare, NoiseTransient, Parabolic, Ramp), selectable distortion drive types (Analog Tanh, Hard Clip, Wavefolder, Tube Drive, Bitcrush, Fuzz, Rectifier), linear FM frequency modulation (FM AMT / FM RATIO) with hold stage before pitch/amplitude decay, 6 multimode resonant filter modes (Off, LowPass, HighPass, BandPass, Notch, Peak) with cutoff & resonance controls on MK3 and GUI, transient click layer, sub-bass tuning, and 13 factory presets (Techno Berlin, 909 Punch, Hardcore Wavefolder, Sub 808, Minimal Click, Industrial Tube, Acid Gabber, Deep Dub, Frenchcore Fuzz, Dark Psy FM Kick, Glitch Bitcrush, 909 Parabolic Punch, Trap 808 Long Boom).
- **Mixer 4D Navigation & Drill-Down**: 4D Left/Right navigates between Pads 1-8, Pads 9-16, and Group mixer views. EncoderPush toggles between Group overview and pad drill-down with live per-pad audio VU meters (green/yellow/red gradient, clip indicators, peak hold) and volume faders in the GUI sound strips and on the MK3 dual screens. 4D Up/Down selects sound without triggering audio.
- **Instant Filter Selection**: instant filter kind selection chips and cycling (LP, HP, BP, OFF, NOTCH, PEAK) in Sampler and Kick synth via GUI and MK3 soft keys (`CYC FLT`, `LP`, `HP`, `BP`, `OFF`, `NOTCH`, `PEAK`), or on the fly by holding `Shift` and turning the filter knob (cutoff / resonance) in Sampler and Kick synth.

### Fixes

- **Latched Mode Auto-Exit**: pressing any top-row view button (`FILE/SAVE`, `BROWSER`, `PLUGIN`, `CHANNEL/MIDI`, `MIXER`, `SAMPLING`, `ARRANGER`) automatically clears all latched modes (`PATTERN`, `SCENE`, `SELECT`, `ERASE`, `MUTE`) and closes overlays (`EVENTS`, `MACRO`, `SETTINGS`) to return the hardware and pads to standard operational state.
- **Hardware LED synchronization**: synced `EVENTS` and `MACRO` LEDs on the MK3 controller with active screen overlays.

### Validation

- `cargo fmt --all -- --check`
- `cargo test --workspace --no-default-features` (144 passing tests)
- `cargo clippy --workspace --all-targets --no-default-features -- -D warnings` (clean, 0 warnings)

## [0.9.6] - 2026-11-08

### Added

- **Kick Synth Modeler plugin**: complete DSP modeled kick synthesizer for techno, club and analog beats. Features exponential pitch drop envelope, punch snap, transient attack click, resonant 50 Hz sub bass boost, analog tanh overdrive, and 6 built-in presets (Techno Berlin, 909 Punch, Hardcore Drive, Sub 808, Minimal Click, Industrial).
- **MK3 Pad Mixer mode**: dedicated mixer modes allowing encoders 1..8 on the MK3 hardware to directly adjust the volume (and pan with Shift) of individual pads (Pads 1-8 or 9-16) for the active group, with dual-screen VU meters, faders, and dB readout.
- **Effects Chains on Pads and Groups**: real-time zero-allocation DSP effect processors supporting Reverb (space & diffusion), Delay (ping-pong echo), Distortion (tanh saturation), Filter (LP/HP/BP multimode), and Compressor (dynamics punch).
- **Browser Plugins & Effects tabs**: dedicated tabs in the MK3 screens and Desktop GUI to browse, preview, and load instruments (Kick Synth, TB-303, Sampler 808, Guitar Amp) or add effect slots directly to pads or groups.
- **MK3 Project deletion**: direct project deletion from the MK3 FILE/SAVE browser with safe confirmation dialog (soft keys and physical ERASE button) and automatic disk cleanup.

### Fixes

- **Restart button**: update modal "Redémarrer maintenant" button now reliably spawns detached processes for both native binary and AppImage packaging, with delay to ensure audio and USB HID devices are completely released by the Linux kernel before restart.
- **Unified PLUGIN navigation**: replaced fixed hardcoded "PAGE TB" with dynamic PLUGIN navigation following whichever instrument is loaded on the active pad.

### Validation

- `cargo fmt --all -- --check`
- `cargo test -p maschine-hardware -p maschine-engine` (97 passing tests)
- `cargo test -p maschine-app --no-default-features` (all tests passing)
- `cargo clippy -p maschine-engine -p maschine-hardware -p maschine-dsp -p maschine-proto -- -D warnings` (clean)

## [0.9.5] - 2026-11-08

### Fixes

- **MK3 FILE/SAVE and BROWSER LED mapping**: properly distinguished between project file browsing (`FILE/SAVE`) and sound preset browsing (`BROWSER`). The physical `FILE/SAVE` LED now illuminates brightly (intensity 7) while in FILE mode, while `BROWSER` LED remains dim (and vice versa). `Shift + FILE/SAVE` provides illuminated hint feedback for quick save.
- **New Project creation workflow**: pressing `NOUVEAU` (soft button 3) on the MK3 now generates a unique project file (`Projet 1`, `Projet 2`, etc.), saves it to disk in `~/.local/share/maschine-linux/projects/`, initializes a playable 808 drum kit on the pads with a fresh sequence, updates `project_entries` with instant on-screen status feedback (`COURANT` badge), and provides a status message.
- **MK3 displays & soft buttons alignment**: updated both screens in FILE mode (`PROJ <`, `PROJ >`, `NOUVEAU`, `CHARGER`, `SAUVER`, `RAPIDE`, `PRESETS`, `RETOUR`). Fixed right screen softkey offset from 0 to 4 so buttons 5-8 correspond directly to the displayed actions. Knobs and 4D encoder controls are mapped unambiguously.

### Validation

- `cargo fmt --all -- --check`
- `cargo test -p maschine-hardware -p maschine-engine` (95 passing tests)
- `cargo test -p maschine-app --no-default-features` (all tests passing)

## [0.9.4] - 2026-11-08

### Fixes

- **MK3 FILE browser workflow**: added `NEW` soft key in the project browser (`browser_kind == 1`) to create a new project directly from the MK3, alongside `CHARGER` / `SAUVER` / `RAPIDE` / presets navigation.

### Validation

- `cargo fmt --all -- --check`
- `cargo test -p maschine-engine -p maschine-hardware -p maschine-app --lib` (all passing)

## [0.9.3] - 2026-11-08

### Fixes

- **MK3 FILE/SAVE vs BROWSER mapping**: corrected physical button behavior. `FILE/SAVE` (the button below `BROWSER`) opens the project/file menu on tap and performs a quick save with `Shift + FILE/SAVE`. `BROWSER` opens the preset/sample browser; `Shift + BROWSER` opens the project browser. Updated MK3 on-screen guidance.

### Validation

- `cargo fmt --all -- --check`
- `cargo test -p maschine-hardware --lib`

## [0.9.2] - 2026-11-08

### Fixes

- **MK3 project browser access**: `BROWSER` now opens the project/FILE browser (with project scan) so projects can be loaded/saved from hardware again; `Shift + BROWSER` opens the preset browser; pressing `BROWSER` again exits to Sampler.
- **Mute LED feedback**: while MUTE pad mode is active, muted pads stay visibly dimmed (instead of nearly off) to make the selected muted pads easier to identify, closer to Maschine workflow.
- **Auto-update UX**: added a **Restart now** button in the GUI updater after an update is installed.

### Validation

- `cargo fmt --all -- --check`
- `cargo test -p maschine-engine -p maschine-hardware -p maschine-app --lib` (workspace libs touched by this patch)

## [0.9.1] - 2026-11-08

### Fixes

- **MK3 FILE / save workflow**: The physical `FILE SAVE` button now performs a normal project save and creates a project file when no current path exists. `Shift + FILE SAVE` keeps the compact quick-save behavior. The FILE/project browser on MK3 now shows accurate soft-key labels and guidance (`PROJ </>`, `CHARGER`, `SAUVER`, `RAPIDE`, `PRESETS`) and project soft-key save actions work even for unsaved projects.
- **Save feedback**: MK3 and GUI snapshots now expose a transient status message; the MK3 header shows a visible save confirmation (`Projet sauvegardé` / `Sauvegarde rapide OK`) after save actions.
- **Maschine-style per-pad mute**: `MUTE` now latches a pad-mute mode. While active, tapping pads toggles mute for individual Sounds in the focused Group without triggering playback, and muted pads are rendered dimmed on MK3 LEDs. `MUTE + GROUP` (or Group selection while mute mode is active) still targets Group mute. Engine playback filters muted Sounds in live triggers and sequenced events.
- **Keyboard / Chords recording**: Keyboard and Chords triggers now record the actual MIDI pitch during `REC`. Chord pads record each triggered note as a melodic event, preserving pitched/chord performance in patterns.

### Validation

- `cargo fmt --all -- --check`
- `cargo test -p maschine-engine -p maschine-hardware` (93 tests passing in this environment)
- `cargo check -p maschine-app` (validated with the Nobara ALSA/JACK `PKG_CONFIG` sysroot)
- Interactive MK3 hardware validation is still recommended for FILE navigation/load/save, mute pad LEDs, and Keyboard/Chords recording/playback.

## [0.9.0] - 2026-11-08

### Highlights

- **FILE / project workflow (MK3 + GUI)**: The MK3 `FILE` button opens a project browser with quick navigation and loading from the hardware or GUI. `Shift + FILE SAVE` performs a compact quick save of the current project; normal saves write readable JSON. Projects are stored as viewable `*.maschine-linux.json` files with atomic writes, and recent/current project paths are remembered for fast reloads.
- **Per-pad volume and pan per Group**: Every Sound/pad in every Group keeps its own mixer level and pan. The GUI Mixer exposes editable per-pad strips for the focused Group, and the selected pad/Sound can be edited from the MK3 mixer workflow and screens.
- **Keyboard and Chords repaired**: Keyboard and Chords modes now play the active Sound melodically with engine-owned musical state (root note, scale, base octave, chord type, inversion). The top display buttons / soft keys change scales and musical behavior, and both MK3 screens plus the GUI Keyboard view show the correct performance information.

### Validation

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (all workspace library tests passing in this environment)
- Hardware validation is recommended on a connected MK3 for HID/display/LED behavior, FILE navigation, mixer control, and Keyboard/Chords performance.

## [0.8.0] - 2026-11-07

### Highlights

- **Complete PATTERN workflow**: create, duplicate, clear, delete, rename, resize, and navigate patterns from the MK3 and the GUI. Empty pattern pads create the corresponding slot, `DUPLICATE` copies the active pattern, `SHIFT + ERASE` clears its events, and the 4D encoder steps through patterns. The eight soft keys expose `PAT <`, `PAT >`, `+ PAT`, `DUPL`, `VIDER`, `SUPPR`, `1 MES`, and `2 MES`.
- **Pattern manager in the GUI**: the Sequencer view now has a dedicated pattern strip with selection, creation, duplication, clearing, deletion, length selection, and inline rename. The Ideas matrix shows 8–16 pattern slots per Group; clicking an empty slot creates that pattern and assigns it to the active Scene.
- **Safe pattern references**: inserting or deleting a pattern re-indexes only the affected Group in every Scene, keeps Scene references valid, and never allows deleting the last pattern of a Group.
- **Hideable workspace**: the centre workspace can be hidden independently with the `CENTRE` button or `F3`. The existing `F1` navigation rail and `F2` MK3 panel toggles remain, and the new `TOUT` button shows or hides all three panels at once for a compact performance layout.
- **Graceful MK3 shutdown**: closing the application now stops the HID loop and joins the display/LED threads, sends a clean-room shutdown splash, releases the display interface without a USB reset, and sends an all-off LED report. The firmware can repaint its default logo and the pads return to their default state instead of staying frozen on the last UI.

### Pattern model and safety

- `EngineEvent` now exposes `AddPattern`, `CreatePatternAt`, `DuplicatePattern`, `DeletePattern`, `ClearPattern`, `RenamePattern`, and `StepPattern`.
- New patterns inherit the active pattern length (fallback: two bars). Names are trimmed and capped at 32 characters; empty names fall back to `Pattern N`.
- Scene references are shifted on insertion and repaired on deletion. A Scene that used a deleted pattern falls back to the nearest valid pattern so every Scene remains playable.
- Pattern selection from the hardware or GUI always targets the active Scene; if no Scene exists, one is created and sized for all Groups.

### Shutdown and hardware lifecycle

- `BulkHandle` tracks whether IF5 was already released for shutdown and skips the historical USB reset in that case. This avoids leaving the screens frozen and avoids disturbing the shared audio stream.
- `DisplayThreadHandle` and `LedWriterHandle` now stop and join from `Drop` as a safety net, so error paths cannot leave background writers running.
- The GUI HID loop has an explicit shutdown flag and is joined before the display and LED writers. The daemon follows the same ordered shutdown path on Ctrl+C.
- The final display splash is generated by `display_render::render_shutdown_screens()` and is covered by a rendering test.

### Validation

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (133 tests)
- Hardware validation on a connected MK3 (`serial=1B64AB9B`, `/dev/hidraw2`): probe, GUI hardware startup, first display frame, window close, shutdown splash/release log, and probe again after exit with the device still available.
- Standalone GUI validation with `--no-hardware`: startup and window close without a hardware session, with hot-plug reconnection disabled.
- Automated mapper tests cover pattern-mode `DUPLICATE`, `SHIFT + ERASE`, and 4D pattern stepping; engine tests cover create-at-slot, duplicate, clear, rename, delete, scene re-indexing, soft keys, and pattern-mode screens.

## [0.7.0] - 2026-11-07

### Highlights

- **Latched Maschine MK3 navigation**: PATTERN, STEP, KEYBOARD, SCENE, EVENTS, MACRO, SETTINGS, and PLUGIN stay active after the first press and return to the default Pad workflow on a second press. Only one workflow menu can be active at a time.
- **Hardware-driven GUI navigation**: PATTERN, STEP, KEYBOARD, and SCENE open their corresponding GUI view through the engine `NavigateView` event and `nav_serial` counter, instead of relying only on the left display tab.
- **Unified PLUGIN view**: the physical PLUGIN button opens the SOUND page for the active Sound. Soft keys switch to the TB-303 page, which exposes a simplified macro layout and keeps advanced oscillator, filter, distortion, and delay controls collapsed by default.
- **Keyboard / Performance view**: a dedicated GUI view with a 16-key performance keyboard, Sound auditioning, and direct pad access.
- **Standalone GUI mode**: `maschine-linux gui --no-hardware` ignores a connected MK3 and runs the full GUI, engine, virtual MIDI, and audio stack without hardware. Hot-plug reconnection is disabled in this mode.
- **Per-Sound sampler control**: polyphony limits and choke groups are applied per Sound, including click-free stealing of the oldest voice from the same Sound.
- **Responsive desktop layout**: collapsible and resizable navigation and MK3 panels (`F1`/`F2`), an application bar with view picker and latched-mode indicators, compact layouts for small windows, and context banners for hardware workflows.
- **English documentation and release automation**: repository documentation is now in English, and `scripts/extract-release-notes.sh` supplies the GitHub release body from this changelog.

### Validation

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- Hardware checks on a connected Maschine MK3 (`serial=1B64AB9B`): device probe, LED test, logical pad test, dual-display solid-color test, first display frame, and GUI hardware startup.
- Standalone GUI launch with `--no-hardware`: no hardware session, no hot-plug reconnect, virtual MIDI available, and the updater reports `v0.7.0` up to date.

### Acid TB-303 Synthesizer (Hardware MK3 & Desktop GUI)
- **Complete Acid 303 DSP module** (`crates/maschine-engine/src/acid_synth.rs`):
  - Anti-aliasing PolyBLEP oscillator (Sawtooth and Square waveforms) with octave shift and analog portamento/slide.
  - 4-pole resonant low-pass filter (24 dB/oct) with nonlinear hyperbolic tangent (tanh) saturation, acid self-oscillation above 80% resonance, envelope modulation and velocity-sensitive accent.
  - Analog distortion / overdrive with tanh waveshaping, post-clipping tone control and dry/wet mixing.
  - Stereo ping-pong delay with damped feedback and dry/wet mixing.
  - 4 built-in presets: Classic Acid, Squelch Lead, Distorted Acid, Dub Echo.
- **Maschine MK3 dual-screen rendering (480×272)** (`crates/maschine-engine/src/display_render.rs`):
  - Left Screen (Tab 4): Real-time display of the oscillator waveform, resonant filter frequency response curve with cutoff cursor, status badges and encoder 1..4 mapping (Cutoff, Reso, EnvMod, Decay).
  - Right Screen: Visualization of the tanh distortion curve, ping-pong delay echo bars, preset selector and encoder 5..8 mapping (Accent, Drive, Time, Mix).
  - Function buttons / Soft Keys 1..8: SAW/SQUARE toggle, Octave -/+, Slide, and instant selection of the 4 presets.
- **Maschine MK3 hardware support** (`crates/maschine-hardware/src/mapper.rs`):
  - The physical `PLUGIN` button opens the unified PLUGIN page (tab 4) on the SOUND page, and its LED lights up. Soft keys switch to the TB-303 page.
  - The 8 rotary encoders are routed in real time to the active PLUGIN page.
- **Desktop GUI** (`crates/maschine-app/src/gui/views/synth.rs`):
  - Unified "PLUGIN / TB-303" view accessible via the left navigation rail and the `T` or `P` keyboard shortcut.
  - Interactive egui visualizations for the oscillator, filter cutoff/resonance curve, distortion and delay repeats.
  - 16-key virtual keyboard (C1 to D#2) to play and test the acid line instantly with the mouse.
  - Bidirectional synchronization with the MK3 screens and encoders.
- **Demo project**: Pad 15 (group A) is configured by default as an Acid 303 synthesizer with a 2-bar acid bass pattern.

### Standalone GUI for development
- The `maschine-linux gui` command now starts without a Maschine connected:
  engine, mouse/keyboard interface, virtual MIDI and system audio remain
  usable.
- Detection, HID, LEDs and LCD remain optional for the GUI, while the
  headless daemon keeps its controller requirement.
- `--no-hardware` explicitly ignores a connected MK3 and disables hot-plug
  reconnection for reproducible standalone sessions.
- If a remembered audio output is unavailable (offline), it falls back to the
  system output, unless explicitly selected via `--audio-device`.

### Sampler, persistence and smoothness
- Samples are identified by Group/Sound pair: two groups can use the same pad number without sharing or stopping the wrong voice.
- WAV import is decoded in a background task, validates the data before replacing a Sound, fixes signed 32-bit PCM normalization and keeps the sound in place if the file is invalid.
- Sample paths and mappings are kept with the project and relative WAVs are reloaded on open. Group/sound volume and pan are also applied to voices already playing.
- The waveform is precomputed on import and shared read-only: the GUI and LCDs no longer rescan the whole file on every frame.
- Voice endings are now handled by in-place removal in the audio render, without a temporary buffer or sorting on each block.
- Tests cover group isolation, WAV mapping/persistence, reloading, live mixing, 32-bit PCM WAVs and import failure without losing the old sound.
- `docs/TESTING.md`, the README and the usage guide describe CI validations and manual hardware checks. Sampler/Events scope is distinguished from the full features of the official software.

### Fixes from LCD validation and workflow (0.6.0)
- **Display stream misalignment**: some partial rectangles had an odd width while `transmit` counts pairs of RGB565 pixels. The recovery probe was also 1×1. The differential now expands rectangles to an even width, the codec rejects odd windows and the probe sends 2×1 pixels. Tests added for the single pixel at the right edge, the pair counter, probe encoding and random rectangles.
- **Right screen stuck on EVENTS**: selecting a main view now resets the right overlay and leaves SONG when a view outside ARRANGE is chosen. The Sampler page therefore resumes its ADSR screen on the right after EVENTS/MACRO/SETTINGS; clicking Sampler when already active in the GUI also restores that page.
- **First knob after GUI navigation**: the HID mapper now re-reads the engine context before each event. It applies the first detent to the current Sound/Sampler instead of using the previous hardware tab.
- Documentation updated in `docs/protocol.md`, `docs/HARDWARE_DISPLAY_PROTOCOL.md`, `docs/workflow.md`, `docs/USAGE.md`, `docs/architecture.md` and the README. The documents now distinguish USB acceptance from real visual validation of the LCDs.

## [0.6.0] - 2026-10-18

### Fixed
- **Frozen MK3 screens (second wave)**:
  - **Permanent startup failure**: if another instance (gui/daemon left open) or a USB re-enumeration held IF5, `claim IF5: Resource busy` killed the display thread without retry → screens never refreshed. The thread now has a **lifecycle**: opening with retries (3 s), release, hot reopen when the interface becomes free again (+ clear log "new attempt in 3 s").
  - **Interrupted packets not completed**: the protocol is an MCU command stream (window header + `transmit` + pixels + `blit` + `end`). A partial write leaves the parser waiting for the *exact remainder* of the pixels → every subsequent send is shifted (screen frozen forever). `send_frame` now completes any interrupted packet with **its own remaining bytes** then `blit`+`end` (returns `Ok` if the packet succeeds); otherwise (timeout, unknown position) a **black purge** (`blit`+`end`) puts the parser back in a waiting state. 2 unit tests (`packet_tail`, real case 10588/7680).
  - **Gentle escalation**: 2×1 px probe (32 B, valid RGB565 pair) before pushing real frames again after an error; gentle re-acquisition (release + claim, **never a USB reset mid-flight** — it would kill the shared audio stream) after 8 consecutive errors, with increasing cooldown (6→24 s) and invalidations; full reopen only if the device is gone. Extended backoff (250 ms → 4 s).
  - **Calibrated ceiling**: average throughput reduced 2.5 → **1.6 MB/s** (560 KB burst) — above ~2 MB/s sustained, the MCU RX queue overflows and the endpoint stops accepting. Hardware measurement: 70+ s consecutive without error at 1.55 MB/s / 17-20 FPS; the remaining transient wedges (MCU internal states) repair themselves in ~10-20 s instead of freezing the screen permanently.
- **Log storm on stop**: 18 million POLLERR lines in one second (ALSA during the cpal stream drop) → error event handler rate-limited (1st error then 1/1000).
- **Frozen MK3 screens (first wave)**: each refresh sent a full uncompressed frame (2 × 261 148 B ≈ 522 KB); at 30 FPS ≈ 15.7 MB/s → USB port saturation, `short write`/`Operation timed out`, blocked endpoint (frozen screen). Replaced by:
  - **Incremental region diff** (`FrameDiffer`, proto): vertical runs of modified rows, X bbox per run, merge beyond 6 rectangles, full frame if dirty area exceeds 55%; window header `x/y/w/h`; skip identical frames.
  - **Byte budget** 2.5 MB/s (700 KB burst) with coalescing (uncommitted diffs if budget insufficient); USB recovery (`clear_stall` + backoff + `reacquire` after 4 errors); invalidation → full frame after error.
  - `send_frame → Result<usize>` (1 s timeout if >100 KB, otherwise 250 ms), `clear_stall()`, `reacquire()` (stubs updated); `display_thread.rs` rewritten (diff, budget, backoff, reacquisition, 10 s stats, conditional commits).
  - Bug fixed in `col_range` (`x1 = x + 1` overwrote the max → `if x + 1 > x1`); 8 tests added including the `differ_always_reconstructs_exactly` property and the `debug_synthetic_rows` regression.
- **MIXER dispatch**: `render_mixer_right` was dead code (the right MIXER screen displayed PATTERN) → wired for `left_tab == 1 && right_tab == 0`.

### Changed
- **Audio**: already delivered in 0.5.0 (64-frame buffer ≈ 1.3 ms, master limiter + `KIT_HEADROOM`, kit at device rate, anti-click) — unchanged.
- **Design of the 2 screens**: common chrome (26 px header with 16-pad activity ribbon + mini master VU, bordered panels, encoders with accent notch, filled soft keys), dense "tracker" labels via `draw_text_clipped` (9 px clipped glyphs), consistent palette.
- `pad_activity: [f32; 16]` (~170 ms decay, `PAD_ACTIVITY_DECAY = 6.0`) to animate the screens (playing + sequencer), `revision` bump during decay; fed by pads and `trigger_pattern_events`.
- **Left ARRANGE**: transport strip with dots, "A S01" chip, group rows with M/S pills and 142 px VU.
- **Right PATTERN**: velocities by height, time lines, tracker labels, activity LEDs, playhead.
- **Right STEP**: M/STEP/VEL/LEN/NOTE header, numbered velocity bars, cursor/playhead.
- **MIXER**: 4 console strips (vertical VU + dB scale + clip LED, M/S, pan), master section (large VU + dB + LOOP/METRO + bar), focus section (group + sound + ADSR).
- **SAMPLER**: amplitude-gradient waveform + markers, filled ADSR + breakpoints.
- **EVENTS**: column table (SND/STEP/VEL/LEN/NOTE), velocity bars, note names, played event highlight.
- **SETTINGS**: 4 cards (PROJECT / TRANSPORT / AUDIO-PLAY / FOCUS) with aligned keys/values.
- **STEP**: differentiated titles "STEP / GRID" (left) and "STEP / DETAIL" (right).

### Added
- Tests: `render_is_deterministic`, `views_are_not_all_identical`, `pad_activity_decays_and_bumps_revision` (display_render); proto diff tests already present (27 green); workspace: 94 tests green.
- `note_pad_activity` made public.

## [0.5.0] - 2026-10-18

### Fixed
- **Crackles + audio latency**: the stream was opened with `BufferSize::Default` (≈2048 frames ≈ **46 ms** on PipeWire) and a failed `try_lock` on the engine returned an entire **silence buffer**. Negotiation is now **48 kHz / 256-frame buffer (64-frame period ≈ 1.3 ms)** with progressive fallbacks (512 → 1024 → 2048 → default) and **waiting for the engine lock up to 2 ms** before any silence. Measured under extreme load (playback + engine lock every 1 ms + snapshots + 30 FPS screens + 61 voices): **0 miss, 0 err, 0 clipping**.
- **Groove quantized to the buffer**: sequencer notes started at the **beginning of the audio buffer** (up to 46 ms offset, swing/humanize destroyed). Each voice now starts at its **exact sample position** within the block (`SamplerVoice::start_frame`), metronome included.
- **Hard output clipping** (`clamp(-1,1)`): kick + snare + clap reached ≈1.55 and were **clipped flat** (the real source of the "crackle"). Replaced by a **soft limiter** (linear below -3 dB, tanh saturation above) + **kit headroom ≈ -2.5 dB**: solo hits remain linear, only coincidences are smoothed (≈0.6 dB).
- **Kit detuned by 1.5 semitones**: DSP presets were generated at 48 kHz and played at 44.1 kHz (+8.8%). `Engine::set_audio_sample_rate()` (called when the stream opens) **regenerates the kit at the actual rate** and repoints the sampler zones; clock realignment without allocation in the callback.
- **Sample-end clicks**: all tails (kick, crash, open hat...) were cut off abruptly at non-zero amplitude (up to -22 dB). Raised-cosine end fades + 0.4 ms fade-in in the drum synth, 3 ms anti-click fade at sample end in the sampler, faded metronome click.
- **Abrupt voice stealing**: the oldest voice was **removed instantly** (click); it now fades out over **5 ms**.
- Buffers > 1024 frames: some samples remained **unwritten** (garbage). Rendering in bounded chunks, regardless of format (F32/I16/U16, 2 or 4 channels).

### Changed
- **808 drum synth rewritten**: RBJ biquads (LP/HP/BP 12 dB/oct) instead of 6 dB/oct filters, **band-limited oscillators** (harmonic sum: zero aliasing) for hi-hats/crash, `tanh` saturation, **per-instrument normalization** (homogeneous kit), reworked envelopes (kick sub + glissando + filtered click, snare dual partial + 1.4–7.5 kHz noise, clap 3 bursts + tail, toms with stick attack, cowbell, 1.6 s metallic crash).
- **Sampler**: no more allocation in `note_on` (zone selection without `Vec`), chunked rendering, `replace_zone_sample` for rate changes.
- Unified audio negotiation (4 channels on MK3 then 2-channel fallback); `--debug-latency` logs the **actual** buffer size and backend errors.
- New diagnostic mode `daemon --stress` (playback + simulated engine lock contention).

### Added
- Tests: silent sample end (anti-click), kit DC absence, note placement **inside** the block (`notes_start_at_their_position_inside_the_block`), metronome driven in blocks like the real callback.
- Quality-control examples: `render_groove` (WAV + peak/RMS/clipped) and `render_kit` (16 instruments as WAV).

## [0.4.0] - 2026-10-18

### Added
- **STEP mode faithful to the official software**: step cursor (`step_cursor`, absolute index in the pattern), the touched pad becomes the selected step, step editing with knobs 1–4 (`VEL`, `POS` nudge in ticks, `LEN`, `PITCH` + note name), **SCROLL of the full pattern grid on knob 5** and the 4-D encoder `<`/`>`, knobs 6–8 = `VOL SND`/`CUTOFF`/`RESO`.
- New engine events `Swing`, `StepCursor`, `StepScroll`, `StepVelocityDelta`, `StepNudgeTicks`, `StepLengthDelta`, `StepPitchDelta` (+ mapper actions and tests).
- **Left STEP screen**: full pattern grid (one row per bar), cursor cell outlined, playhead, live detail of the selected step and slots 1–4. **Right STEP screen**: orange outline of the edited step, `STEP n/n` in the header, slot 5 = `SCROLL`.
- STEP soft keys: `STEP <</>>`, `BAR <</>>`, lengths `1/2/4/8 BARS`.
- Edit cursor visible on pads when stopped (white) in the sound color for programmed steps.
- End-to-end `engine` test: place + edit + scroll cursor; mapper tests (STEP knobs, accumulator, fast-turn clamp, 4-D in STEP).

### Changed
- **Knob speed recalibrated** ("the knobs turn too fast"): scaling done **only once** in the app (the mapper emits raw ticks), official-software-like increments: ≈1.5 BPM/detent (TEMPO), 0.03 dB (volumes), 0.001 (pan/reso/master/sustain), **exponential CUTOFF** `2^(ticks/384)` (≈2.7 octaves/turn, ≈1 semitone/detent), ADSR 0.5–1 ms/detent, BROWSER presets/sounds per detent.
- Fast knob turns (delta > 128 ticks) are **clamped instead of discarded**: no more lost movements.
- 4-D encoder `<`/`>` in STEP mode: scrolls the grid by one step instead of changing bar.
- The display thread wakes its loop every 3 ms (lock-free activity flag set by the HID loop) and sends as soon as a change is ready: reactivity within a few milliseconds, engine lock taken at most every 33 ms (USB ceiling ~30 FPS, 2 s guard) — **0 audio misses** preserved.

### Fixed
- **Screens that "did not respond"**: `Swing` wrote the project directly without synchronizing the sequencer or invalidating the display; `toggle_step`, `clear_bar`, humanize/quantize, groove reset and DUPLICATE BAR modified the project without bumping the revision (screens stayed frozen until the 5 s guard). Everything now goes through the Engine (`EngineEvent::Swing`, `invalidate_display`) and refreshes the display.
- `RESTART` goes through `EngineEvent::Restart` (reset + immediate invalidation of the displayed BBT).
- The SWING knob now actually affects playback (the sequencer used the old value).

## [0.3.0] - 2026-10-18

### Added
- **Complete redesign of the 2 MK3 screens (480×272)** in the style of the official software: unified design system (title bar, encoder rows, soft key bar), 16 contextual views (ARRANGE, PATTERN, STEP, SONG, SCENES/IDEAS, MIXER ×2, SAMPLER ×2, BROWSER ×2, EVENTS, MACRO, SETTINGS), live values per Sound/Group and real VU meters.
- **Unified contextual soft keys**: `EngineEvent::SoftKey(u8)` + labels exposed in the snapshot — hardware, screens and GUI execute and display exactly the same action (no more hard-wired screen buttons).
- **Waveform + ADSR envelope drawn** on the SAMPLER screen from the active Sound sample (min/max envelope per column).
- **Real-time VU meters** per Group (8) and Master with peak hold and decay, computed in the audio callback and broadcast in the snapshot.
- **`--debug-latency` instrumentation**: HID→Engine latency (latest/max per 2 s window) and audio statistics (`callbacks`, `lock_misses`, max callback time).
- **Immediate GUI wake-up** (`RepaintSignal`) from the HID thread + revision-counter-driven repaint (16 ms during playback, 50 ms at rest); the display thread only resends a frame if the state changed.
- **Extended Song/Scene workflow**: `DuplicateScene`, `DeleteScene` (section re-indexing), `DuplicateSection`, `DeleteSection`, `ResizeSection` (length 1/2/4/8/…), exposed in the GUI IDEAS view.
- Smoke test rendering all screen views (anti-panic guard) + section/scene workflow tests.

### Changed
- **Real-time engine**: the snapshot shares the project as an `Arc` (copy-on-write via `Arc::make_mut`) — no more project clone on every frame, O(1) reads; `ProjectHandle`/`ProjectWriteGuard` for safe writes.
- **egui GUI fully rewritten**: unified theme, dedicated widgets (drag/reset knobs, dB faders, VU meters, velocity-sensitive pads, step cells), IDEAS view (scenes × patterns + Song), MIXER view 8 strips + master, SAMPLER (waveform, ADSR, filter), BROWSER (808 presets, preview, loading), MK3 hardware rail with screen mirror and soft keys.
- `render_screens` shared between the USB display thread and the GUI mirror: the physical screens and the window display strictly the same pair of views.

### Fixed
- Transport: Play/Stop/REC (stop is no longer a no-op), restart from zero (`EngineEvent::Restart`).
- Screen encoders: slots now display the parameters actually controlled by the 8 knobs (TEMPO/SWING/VOL PAN per group, VOL/PAN/CUT/RESO/ADSR per Sound, volumes A–H in MIXER, BROWSER navigation...).
- Sequencer grid (bars/steps), dB fader mapping, audio settings window fields (non-deprecated `ComboBox`).

## [0.2.1] - 2026-10-14

### Fixed
- **4-D encoder**: fixed the HID report `0x01` byte 1 bitmask in `maschine-proto` for the 4 cardinal directions (Up = bit 2, Right = bit 3, Down = bit 4, Left = bit 5).
- **One-shot vs Loop**: replaced the accidental `PlayPauseToggle` action on `EncoderPush` with a one-shot preview of the active sound, avoiding unintended triggering of the transport loop.
- **Multi-bar step sequencer**: full support for 1, 2, 4 and 8-bar patterns with smooth navigation on the controller (`<` / `>` keys, 4D left/right arrows, screen buttons 5..8) and in the GUI.
- **Step sequencer playhead cursor**: time cursor computed relative to the actual pattern length (`modulo pattern_bars`) rather than restricted to the first 2 bars.
- **Silent step placement**: in Step mode, pressing a pad toggles the cell without emitting stray sound or disturbing the preview.
- **Code quality & Clippy**: replaced approximate constants with `std::f32::consts::FRAC_1_SQRT_2`, removed redundant casts, fixed documentation warnings and indexing loops.

### Added
- Automated unit tests for the 4 directions of the 4D encoder (`decode_01_four_way_directions`).
- Unit tests for bar selection and navigation (`bar_navigation_and_pattern_length`, `step_sequencer_multi_bar_toggle`, `four_way_encoder_and_bar_selection`).
- Expanded documentation for the hardware/software workflow and application usage.

## [0.2.0] - 2026-10-14

### Added
- **808 drum synth generator**: 16 percussion instruments synthesized in real time (Kick, Snare, Hi-Hats, Clap, Toms, Rimshot, Shaker, Cowbell, Crash, Perc, Synth Stab, Sub Bass).
- **MK3 dual LCD rendering (480×272)**: live views for Arranger, DAW / Step Grid, Sampler & ADSR Envelopes, 8-group Mixer and Browser.
- **Controller play modes**: Pad Mode, Step Sequencer, Keyboard (scales), Chords (diatonic chords).
- **Scene and pattern management**: SCENE, PATTERN, SELECT, ERASE modes directly on the hardware with color LED feedback.
- **Native egui GUI**: transport panel, full 16-track sequencer, 4×4 pad grid, sound editor and filter parameters.

### Added

- Rust workspace (`maschine-proto`, `maschine-hardware`, `maschine-engine`, `maschine-app`)
- `maschine-proto`: HID codec `0x01`/`0x02` + `0x80`/`0x81` + display bulk RGB565 RLE + indexed palette, full tests
- `maschine-hardware`: `hidraw` discovery + 1ms poll thread + pad hysteresis + `HardwareMapper` (pads → MIDI NoteOn/Off, Groups, Transport, knobs CC, touchstrip), `BulkHandle` (`usb` feature)
- `maschine-engine`: `Project` model (Groups 16 Sounds, Patterns, Scenes, Sections), `Transport` (PPQ 960, BPM, swing, loop), `Sequencer` (quantize, nudge, humanize, slice), `Sampler` (zones, choke, ADSR, stereo render), `Engine` (lock-free snapshot)
- `maschine-app`: CLI (`probe`, `hid-dump`, `daemon`, `project new/show/validate`, `led-test`, `gui` stub), headless daemon + heartbeat + autosave
- Configs: `99-maschine-mk3.rules` (udev `TAG+="uaccess"`), PipeWire `99-maschine-lowlatency.conf` (quantum 128), WirePlumber `51-maschine-mk3.conf/.lua` (pro-audio)
- Docs: `README.md`, `docs/architecture.md`, `docs/protocol.md`, `docs/workflow.md`, `docs/INSTALL.md`, `docs/USAGE.md`
- GitHub Actions CI (fmt, clippy, test, build), `rustfmt.toml`, `scripts/install.sh`
- Example `examples/demo.machine-linux.json`

### Tested

- MK3 `17cc:1600` on Fedora 44, kernel 7.1.10, PipeWire 1.6.8
- `probe` detects `hidraw3` + ALSA `card 2 MK3`
- `hid-dump` + `led-test` OK (hidraw write 0x80/0x81)
- `daemon` connects, maps pads/buttons, advances transport, renders sampler
- `cargo test --workspace` 31 tests pass
