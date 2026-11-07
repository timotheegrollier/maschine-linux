# Maschine workflow — model reproduced in maschine-engine

Clean-room workflow replica of Maschine 2/3 (NI) adapted to Rust, without the
bloat. It reproduces the documented workflow and hardware behavior; it does not
include Native Instruments code or proprietary assets.

## Hierarchy

```
Project
 ├─ master : Channel (vol, pan, inserts FX)
 ├─ groups: Vec<Group>  // A-H × N banks, extensible, 8 visible at a time on hardware
 ├─ scenes: Vec<Scene>
 ├─ song  : Vec<Section>  // arranger timeline
 └─ tempo: f32, swing: f32, time_signature
```

```
Group (16 fixed pads, like hardware)
 ├─ sounds: [Sound; 16]  // Some(Sound) or None (gap allowed)
 ├─ channel: Channel     // Group inserts
 ├─ patterns: Vec<Pattern>  // pool specific to the Group
 └─ color: u8, name: String, muted/solo
```

```
Sound
 ├─ chain: Vec<PluginSlot>  // Slot0 = Instrument (Sampler/Audio/External), Slot1..N = FX
 ├─ channel: Channel
 ├─ zones: Vec<Zone>        // multi-sample mapping
 ├─ choke_group: Option<u8> // off_by (hi-hat)
 ├─ polyphony: u8           // per-Sound voice limit (1..32)
 └─ midi: MidiInputMode (Off/Focus/Omni)
```

```
Pattern (belongs to ONE Group)
 ├─ events: Vec<Event>  // note, vel (alpha), start ticks, length ticks
 ├─ length: BarsBeatsTicks
 ├─ swing: f32, grid: Grid (1/4, 1/8T, 1/16, etc.)
 └─ color
```

```
Scene : Vec<Option<PatternId>> // one pattern per Group max, None = silence
Section : { scene_id, length, name } // length can be < scene length (trim)
```

Exact replica: Pattern = referenced object (editing propagates everywhere) vs Clip (Song-only, future). Scenes can be triggered via pads in Ideas view or MIDI Note/ProgramChange.

## Application views

The GUI follows Maschine's workflow separation: a common transport bar, groups and scenes on the left, then a dedicated workspace. View buttons stay synchronized with the MK3 hardware tabs.

- **Ideas**: horizontal scenes and a Group × Pattern matrix with 8–16 slots per Group. Clicking an existing pattern assigns it to the scene and focuses its group; clicking an empty slot creates that pattern and assigns it. The scene can be duplicated/deleted (with automatic re-indexing of Song sections). A scene can be added from this view.
- **Song**: horizontal timeline of sections, direct selection, previous/next navigation, duplication, deletion and section length (1/2/4/8 bars).
- **Sequencer**: 16 × 16 steps, navigation and pattern length from 1/2/4/8 bars, preview pads and groove actions. The pattern manager above the grid creates, selects, duplicates, clears, deletes, renames, and resizes patterns. Cells are prepared in a single pass over events to keep editing smooth.
- **Keyboard / Performance**: 16-key performance keyboard, Sound auditioning, previous/next Sound selection, and direct pad access. Hardware KEYBOARD and CHORDS modes open this view.
- **Mixer**: volume, pan, mute and solo for groups A–H, plus the master.
- **Sampler / Sound**: selection of the 16 Sounds, asynchronous WAV import, waveform, audition, volume, pan, filter, ADSR envelope, per-Sound polyphony and choke group. WAV paths are saved in the project zones and reloaded on open.
- **PLUGIN**: unified Sound editor. The SOUND page exposes the sampler-style controls; the TB-303 page provides simplified macros (cutoff, resonance, envelope modulation, decay, accent, drive, delay time, delay mix) with advanced oscillator, filter, distortion, and delay controls collapsed by default.
- **Browser**: built-in 808 drum synth presets, loaded onto the selected Sound. The Browser does not yet browse the NI library or the filesystem.

The GUI uses a collapsible, resizable navigation rail, centre workspace, and
MK3 mirror. `F1` toggles the rail, `F2` toggles the mirror, `F3` toggles the
centre workspace, and the `TOUT` button shows or hides all three panels at
once. The application bar shows the active latched workflow. The transport
offers Play/Stop, Record, loop, tempo and BBT position. GUI pads send note
release; 808 presets are cached when the kit loads.

`maschine-linux gui --no-hardware` forces standalone mode: the GUI, engine,
virtual MIDI, and audio remain available, while any connected MK3 and its
hot-plug reconnection are ignored.

## MK3 screens (480×272) & soft keys

Both screens are rendered by the same function (`display_render::render_screens`): the MK3 panel in the GUI is the mirror of the content computed for the hardware. The USB loop compares renders at 30 Hz, sends only modified rectangles (up to six), limits average throughput to 1.6 MB/s and sends a full-screen pair every 2 seconds to resynchronize the controller.

- **Left screen**: ARRANGE (8 groups, scene pattern, real VU), SCENES/IDEAS (SCENE mode), PATTERN slots (PATTERN mode: 16 slots with name, length, event count, active/assigned markers), SONG (sections), MIXER A–D, SAMPLER (waveform + ADSR + info), BROWSER (808 presets), STEP (full pattern grid + selected step parameters), and PLUGIN (SOUND page or TB-303 page).
- **Right screen**: PATTERN (16 sounds × 16 steps grid + playhead), STEP (active Sound velocities + edit cursor), SONG TIMELINE, MIXER master + group/sound focus, SAMPLER full-screen envelope, BROWSER loading, EVENTS, MACRO, SETTINGS, PLUGIN SOUND envelope.
- **Soft keys (8 buttons under the screens)**: *contextual* action decided by the engine (`EngineEvent::SoftKey`) and displayed at the bottom of the screens. The button always does what the label announces, identically on hardware and in the GUI.
  - ARRANGE: `BAR 1..4` + pattern length `1/2/4/8 BARS`.
  - PATTERN: `PAT <`, `PAT >`, `+ PAT`, `DUPL`, `VIDER`, `SUPPR`, `1 MES`, `2 MES`.
  - STEP: `STEP <</>>` (cursor by one step), `BAR <</>>` (by one bar), `1/2/4/8 BARS`.
  - MIXER: `GRP A..H` (group focus). SAMPLER: `SND 1..8` (active Sound).
  - BROWSER: `PRES <</>>`, `SND <</>>`, `LISTEN`, `LOAD`, `METRO`, `LOOP`.
  - PLUGIN SOUND: `SND <`, `SND >`, `PAGE TB`, `LISTEN`, `APPLY`, `RESET`, `VOL -`, `VOL +`.
  - PLUGIN TB-303: `SAW/SQR`, `OCT -`, `OCT +`, `SLIDE`, `CLASSIC`, `SQUELCH`, `PAGE SND`, `DUB`.
  - SCENE: `SCN 1..8`. SONG: `SEC <</>>`, `+SEC`, `IDEAS`, scenes and `+SCN`.
- **Encoders**: each screen displays the 4 parameters actually controlled by its knobs (1–4 on the left, 5–8 on the right) with live values. PLUGIN routes knobs by page: SOUND uses Vol/Pan/Cutoff/Reso + ADSR, TB-303 uses Cutoff/Reso/EnvMod/Decay + Accent/Drive/Time/Mix.
- **Navigation between views**: choosing ARRANGE, MIXER, SAMPLER, BROWSER or PLUGIN returns the right screen to the page linked to that view. EVENTS, MACRO and SETTINGS are right-hand views that can then be explicitly selected. Clicking the active main view again in the GUI returns to its contextual right page.
- **Latched workflow menus**: PATTERN, STEP, KEYBOARD, SCENE, EVENTS, MACRO, SETTINGS and PLUGIN latch on the first press. A second press on the same button returns to the default Pad workflow and the Sequencer view. Only one workflow menu is active at a time.
- **PATTERN mode**: the left screen lists the 16 pattern slots of the focus Group with length and event count; the right screen shows the selected pattern grid. Empty pads create the corresponding slot, `DUPLICATE` copies the active pattern, `SHIFT + ERASE` clears its events, and the 4D encoder moves to the previous/next pattern. The last pattern of a Group cannot be deleted.
- **Clean shutdown**: closing the GUI or stopping the daemon sends a final splash, releases the display interface without a USB reset, and sends an all-off LED report. The MK3 firmware can repaint its default logo and the pads return to their idle state.
- **GUI and hardware controls synchronized**: before mapping each new HID event, the application re-reads the group, sound and tabs from the engine. The first knob detent after a page change therefore already performs the new page's action.
- **Live editing**: volume, pan, filter and ADSR changes also reach voices already playing; Group/Sound gains and pans combine without overwriting the neighboring level.
- **LCD alignment**: each USB rectangle must have an even width, because the Bulk counter represents pairs of RGB565 pixels. Differential rectangles are expanded to the neighboring pixel; odd windows are rejected before sending. See [the hardware protocol](HARDWARE_DISPLAY_PROTOCOL.md).
- **Real validation**: tests check encoding, rendering and control routing. An accepted USB transfer does not prove that the pixels are visible; check both LCDs on the MK3 after launching.

### Official reference and current scope

The MK3 manual describes SAMPLING as a Sample Editor tied to the focused Sound: buttons 1–4 select RECORD, EDIT, SLICE and ZONE; EDIT notably sets the play range, while ZONE handles mapping, tuning and key/velocity ranges. The controller's eight buttons and knobs are contextual and their function is displayed on screen ([official manual, Sampling and sample mapping](https://docs.native-instruments.com/ni-tech-manuals/maschine-mk3-manual/en/sampling-and-sample-mapping), [MK3 quick reference](https://docs.native-instruments.com/ni-tech-manuals/maschine-mk3-manual/en/quick-reference)).

In this application, the SAMPLING button currently opens **Sampler / Sound**: WAV import from the GUI, waveform, audition, ADSR, filter, volume and pan. Import is decoded on a worker thread. The recording, destructive editing and slicing pages, manipulable play/selection ranges and zone editing are not yet implemented on the controller. The current EVENTS screen is also an information view; it does not yet offer the event editing of position/pitch/velocity/length found in the official mode ([official description of EVENTS mode](https://docs.native-instruments.com/ni-tech-manuals/maschine-mk3-manual/en/quick-reference)).

## STEP mode (following the official software)

STEP mode is activated with the **STEP** button on the hardware (or the GUI). The left tab stays on ARRANGE: the STEP page then replaces ARRANGE on the screens.

- **Pads 1..16**: toggle the corresponding step of the active Sound in the displayed bar (silent, duplicate removed via `same_step_cell`). The touched pad becomes the **selected step**: the screen grids and the LED cursor follow it.
- **Cursor (`Engine::step_cursor`)**: selected step (absolute index 0..`pattern_bars*16-1`). It determines the displayed bar (`selected_bar = cursor / 16`) and the window of the 4 editing knobs.
- **Knobs 1–4 = editing the selected step**: `VEL` (velocity), `POS` (nudge in ticks), `LEN` (length in 1/16 steps), `PITCH` (semitones). A mapper-side accumulator turns continuous rotation into integer increments (~1 detent ≈ 32 raw ticks).
- **Knob 5 = SCROLL**: scrolls the full pattern grid (32 steps for 2 bars, up to 128 steps); knobs 6–8 remain the active Sound's `VOL SND`, `CUTOFF`, `RESO`.
- **4-D encoder `<` / `>` and arrows**: scroll the grid by one step (STEP mode takes over bar navigation).
- **Left screen**: full pattern grid (one row per bar, 16 cells), cursor cell outlined in orange, white playhead, step detail (`VEL`, `POS`, `LEN`, `PITCH` with note name) and slots 1–4.
- **Right screen**: detailed bar grid (height = velocity), orange outline on the edited step, `STEP n/n` in the header, slot 5 = `SCROLL`, 6–8 = `VOL SND`, `CUTOFF`, `RESO`.

## Latency instrumentation

`maschine-linux daemon --debug-latency` (or `gui --debug-latency`) logs every 2 s:

- **HID→Engine** latency: number of events, latest and max in µs per window;
- audio statistics: callbacks, **actual buffer size** in frames (and ms), `lock_misses` (callback that could not acquire the lock → silence), `err` (backend errors/underruns) and max callback time in µs.

`maschine-linux daemon --debug-latency --stress` adds a deliberate load (pattern playback + engine lock and snapshots taken continuously): this is what exposes crackles. Measured reference (PipeWire `default`, 48 kHz): **1500 callbacks/2 s (64-frame buffer ≈ 1.3 ms), 0 miss, 0 err** with 61 voices and the contention.

The HID thread immediately wakes the GUI (`RepaintSignal`), and the engine snapshot shares the project as an `Arc` (copy-on-write): UI reads are O(1) and only user actions clone.

## Audio quality & anti-crackling

Output chain (in order): voices → groups (gains/pans/filters) → **master soft limiter** (linear below -3 dB then `tanh`, never hard clipping) → callback (safety clamp ±1.0).

- **Latency**: the stream is negotiated as short as possible — 48 kHz if accepted, 256-frame buffer (period 64), fallbacks 512 → 1024 → 2048 → default. `set_audio_sample_rate()` regenerates the DSP kit at the device's actual rate on each open (otherwise the kit plays out of tune).
- **Timing**: the callback advances the transport for the whole block then renders; each note is placed **sample-accurately** within the block (`start_frame`), so the groove does not depend on buffer size.
- **Anti-click**: sample end fade (3 ms) + end fade in the drum synth, voice stealing and choke with 5 ms fade, metronome clicks faded.
- **Offline diagnostics**: `cargo run --release -p maschine-engine --example render_groove -- /tmp/g.wav 12 48000 0.85` (peak, RMS, clipped samples, largest jump) and `--example render_kit` (the 16 instruments as WAV). Demo groove reference: peak ≈0.92, RMS ≈-18 dB, 0 clipping, no abnormal jump at loop boundaries.

## Sampler

- Engine: `sampler.rs` — polyphonic voice, `Voice { sample: Arc<[f32]>, pos: f64, gain, pan, adsr: Adsr, filter: Svf }`.
- Zones: `Zone { sample_path, root_key, lokey/hikey, lovel/hivel, seq_len/seq_pos, group, polyphony, tune, pan }`.
- Per-Sound polyphony: `sound_polyphony` limits active voices for each Sound (1..32). When the limit is reached, the oldest voice of the same Sound is stolen with a short fade; other Sounds are unaffected.
- Choke: `HashMap<choke_group, Vec<VoiceId>>`, `off_mode` = kill / fade 5ms. Choke groups can be changed per Sound at runtime from the PLUGIN SOUND page.
- Slice: `slice::detect_transients(&[f32]) -> Vec<usize>` then export `Apply to Group` (each slice → new Sound) + auto 16-step pattern reproducing the original loop.
- Formats: WAV/FLAC/OGG via `hound` + `symphonia` (future), SFZ import/export, Hydrogen drumkit import.

## Sequencer & Transport

- `Transport { bpm, sample_rate, tick, playing, recording, loop_enabled, loop_start, loop_end, swing }`.
- `Sequencer::tick_to_samples(tick)`, `quantize(event, grid)`, `nudge(event, ticks)`, `humanize(vel, timing)`.
- Swing: delay every other 16th by `swing * 0.5 * step_duration`.
- Resolution: 960 ticks / quarter (PPQ 960).
- Multi-bar: patterns configurable from 1 to 8 bars (`1:0:0`, `2:0:0`, `4:0:0`, `8:0:0`).
- Silent step sequencer: in STEP mode, placing a step emits no stray sound and cleanly removes duplicates via 16th-cell comparison (`same_step_cell`).

## State & persistence

- `Project::save(path)` → pretty JSON + bincode binary cache.
- `schema_version: u32` for migrations.
- Lock-free UI snapshot: `Arc<RwLock<Arc<Project>>>` (copy-on-write at write time) + `AtomicU64` revision counter. `Engine::snapshot()` is O(1) (clone of an `Arc`); VU meters and the active Sound envelope travel in the snapshot.

## Complete MK3 hardware mapping

- **Pads 1..16**:
  - *PAD mode*: triggers the corresponding sound of the focused group with dynamic velocity and pressure. One-shot rendering without accidental looping.
  - *STEP mode*: silent toggle of the corresponding step (0..15) in the selected bar (`selected_bar`); the pad becomes the selected step (orange cursor on the screens, white on the pads when stopped). Knobs 1–5 edit the step (`VEL`/`POS`/`LEN`/`PITCH`) and scroll the grid (`SCROLL`) — see "STEP mode".
  - *KEYBOARD / CHORDS modes*: melodic performance and enriched diatonic chords; both open the Keyboard / Performance GUI view.
  - *SCENE / PATTERN / ERASE / SELECT modes*: direct management of the Maschine workflow.
- **Latched menus**: PATTERN, STEP, KEYBOARD, SCENE, EVENTS, MACRO, SETTINGS and PLUGIN stay active after the first press. The same button pressed again clears the menu and returns to the Pad workflow. Selecting another workflow menu clears the previous one.
- **PLUGIN button**: opens the unified PLUGIN view on the SOUND page; a second press closes it and returns to the Sequencer. Soft keys switch between SOUND and TB-303.
- **4-D encoder**:
  - *Up / Down*: quick change of the selected sound (`active_sound`) with instant preview; preset navigation in the Browser.
  - *Left / Right* (or `<` / `>`): navigation between bars (`selected_bar`) — **in STEP mode**, scroll the grid by one step.
  - *Shift + Right*: extension of pattern length to 4 or 8 bars.
  - *Push*: one-shot preview of the sound or preset confirmation.
  - *TEMPO / VOLUME / SWING focus* (dedicated buttons): 0.5 BPM, 0.01 master, 0.01 swing per detent (halved with Shift).
- **Screens & screen buttons 1..8**:
  - *Contextual soft keys*: the 8 buttons under the screens execute the action displayed at the bottom of the screen (see "MK3 screens & soft keys"). Same action engine on the hardware and GUI sides.
  - *8 rotary potentiometers*: contextual editing of sound or mixer parameters; the screen encoder slots display these same parameters with their live values.
- **Knob speed** (scaled *only once* in `main.rs`, not per raw tick or per detent ≈ 32 ticks):
  - ARRANGE: TEMPO 0.05 BPM (≈1.5 BPM/detent, Shift ÷10), SWING 0.001, VOL GRP/SND 0.03 dB, PAN 0.001, MASTER 0.001.
  - CUTOFF: exponential scale `2^(ticks/384)` — ≈2.7 octaves per turn, ≈1 semitone per detent (natural from 20 Hz to 20 kHz).
  - RESO 0.001. SAMPLER: ATTACK 0.5 ms, DECAY/RELEASE 1 ms, SUSTAIN 0.001 per tick.
  - MIXER: 0.03 dB per tick on the 8 groups. BROWSER: sound/preset per detent (accumulator), MASTER 0.001.
  - A fast turn (raw delta > 128 ticks) is **clamped**, never discarded: the knob no longer "loses" movements.

## Minimal JSON example

```json
{
  "schema_version": 1,
  "name": "Demo",
  "bpm": 120.0,
  "groups": [{
    "name": "Kit 808",
    "color": 22,
    "sounds": [{ "name": "Kick", "color": 6, "zones": [{"sample": "kicks/808.wav", "root_key": 36}] }, null],
    "patterns": [{ "name": "Pattern 1", "length": "2:0:0", "events": [{"pad": 0, "tick": 0, "vel": 100, "len": 240}] }]
  }],
  "scenes": [{ "name": "Scene 1", "patterns": [0] }],
  "song": [{ "scene": 0, "length": "2:0:0" }]
}
```

See `crates/maschine-engine/src/project.rs` for the complete schema.
