# Usage

## CLI

```bash
maschine-linux --help
maschine-linux probe
maschine-linux -v probe
maschine-linux hid-dump --duration 5
maschine-linux hid-dump --hidraw /dev/hidraw3 --duration 5
maschine-linux led-test --pad 0 --color 6   # red
maschine-linux led-test --pad 15 --color 42 # blue
maschine-linux project new --out demo.json
maschine-linux project show demo.json
maschine-linux project validate demo.json
maschine-linux daemon
maschine-linux daemon --project my.json
maschine-linux gui --project my.json   # GUI, Maschine optional
maschine-linux gui --no-hardware       # force standalone mode, ignore any MK3
```

## Daemon

The daemon:

1. Opens `/dev/hidraw3` (`hid-generic` via `usbhid`, no `libusb` detach)
2. Polls every 1ms (`SCHED_OTHER` for now, `SCHED_FIFO` with `rtkit` as an option)
3. Decodes `0x01` (buttons) and `0x02` (pads, hysteresis 0x4010/0x4005)
4. Maps via `HardwareMapper` → `MidiEvent` + `LedState`
5. Sends to `Engine` (Project/Transport/Sampler)
6. Advances the transport (PPQ 960) and triggers the patterns of the active scene
7. Renders the engine voices; `--no-audio` also advances the transport with silent rendering to test the interface.

Logs: `RUST_LOG=debug maschine-linux -vv daemon`

### Supported Maschine MK3 hardware controls

- **Pads 1..16**:
  - **PAD mode**: full-dynamics triggering (velocity + pressure) of the active sound or full kit. Melodic instruments (Wavetable/Nexus Synth, TB-303) sustain notes for the duration of the pad hold with clean NoteOff release; drum sampler pads trigger authentic one-shots. White hit flash then the color assigned to the sound.
  - **PATTERN mode**: select an existing pattern of the focus Group, or create the corresponding slot when the pad is empty. Pressing the currently active pattern toggles it off (silencing the group), allowing all patterns across any group to be deactivated. Holding two pads simultaneously chains those patterns to play sequentially in a continuous loop.
  - **STEP mode**: silent step sequencer (no stray playback when placing/removing steps), active step LEDs + live white playhead cursor.
  - **KEYBOARD mode**: melodic playing in a minor scale C3..D5 with sustained notes.
  - **CHORDS mode**: enriched diatonic chords with sustained notes.
- **NOTE REPEAT & LOCK**:
  - Pressing `NOTE REPEAT` activates real-time note repetition.
  - Pressing `LOCK` latches Note Repeat mode on with illuminated physical LED feedback.
  - Soft display buttons 1..8 select repeat divisions (1/32, 1/24, 1/16, 1/12, 1/8, 1/6, 1/4, 1/2) with the active rate highlighted on both the screen and the button LED.
  - Live repeat notes are fully recordable into patterns with the `REC` transport button.
- **4-D encoder**:
  - **Up / Down**: selection of the active sound (with one-shot preview); in Browser mode, scrolling through synthesizer presets; in MIXER mode, **Up** drills down into the group's pad mixer strips, and **Down** exits back to group overview / master view.
  - **Left / Right** or `<` / `>` keys: step-by-step navigation between pattern bars (Bar 1, Bar 2, Bar 3, Bar 4...); in PATTERN mode, previous/next pattern; in MIXER pad view, switches between Pads 1-8 and Pads 9-16 banks.
  - **Shift + Right**: dynamic extension of the pattern to 4 or 8 bars.
  - **Pressure (Push)**: one-shot preview of the active sound (or loading the selected preset in Browser mode) — does not accidentally start loop playback.
  - **Shift + Push**: instantly cycles the filter type of the focused sound (`LP` -> `HP` -> `BP` -> `OFF`) across all views without switching pages.
  - **Rotation**: fine adjustment according to the active focus (Tempo, Master Volume, Swing).
- **Shift & Contextual Real-Time Display Labels**:
  - Holding the physical `SHIFT` button dynamically repaints the 8 display button labels across all views in real time, exposing immediate shortcuts directly above each button: `DOUBLE` (double pattern length), `HALF` (halve pattern length), `QUANT` (50% grid quantize), `HUMAN` (humanize timing), `SAV PROJ` (instant quick save), `COUNT-IN` (toggle recording count-in), and direct filter mode selection (`LP`, `HP`, `BP`, `OFF`) for drums, sampler sounds, and Kick/Wavetable synths.
  - Turning filter Cutoff/Resonance encoders while holding `SHIFT` immediately cycles the filter mode (knobs 6/7 in Arranger, knobs 3/4 in Plugin/Sampler).
- **Pattern editing buttons**: `DUPLICATE` copies the active pattern and inserts the copy after it. `SHIFT + ERASE` clears the active pattern events without deleting the pattern or changing its length.
- **Color LCD screens (2 × 480×272)**:
  - **Display Buttons 1..4 (Left screen) & PLUGIN button**: tab selection (ARRANGE, MIXER, SAMPLER, BROWSER, and the unified **PLUGIN** page). The physical PLUGIN button opens the SOUND page and a second press closes it.
  - **Display Buttons 5..8 (Right screen)**: direct selection of the displayed/edited bar (BAR 1, BAR 2, BAR 3, BAR 4); with **Shift**, pattern sizing (1, 2, 4, 8 bars).
  - **8 detented potentiometers under the screens**:
    - *MIXER*: volumes of Groups A..H.
    - *SAMPLER*: buttons 1..4 = Vol, Pan, filter Cutoff, Resonance; 5..8 = Attack, Decay, Sustain, Release ADSR.
    - *PLUGIN / SOUND*: buttons 1..4 = Vol, Pan, Cutoff, Resonance; 5..8 = Attack, Decay, Sustain, Release ADSR.
    - *PLUGIN / TB-303*: buttons 1..4 = Cutoff, Resonance, Envelope Modulation, Decay; 5..8 = Accent, distortion Drive, Delay Time, Delay Mix.
    - *PLUGIN / KICK*: buttons 1..4 = Tune, Decay, Pitch Bend, Drive; 5..8 = Sub Bass, FM Amount, Click, Filter Cutoff.
    - *PLUGIN / WAVETABLE*: buttons 1..4 = Cutoff, Resonance, Env Mod, Drive; 5..8 = Attack, Decay, Sustain, Release.
    - *BROWSER*: knob selects pads and scrolls 808 presets.
    - *ARRANGE*: tempo, swing, volume and global filter.
  - **PLUGIN soft keys**: SOUND page `[1: SND <]`, `[2: SND >]`, `[3: PAGE TB]`, `[4: LISTEN]`, `[5: APPLY]`, `[6: RESET]`, `[7: VOL -]`, `[8: VOL +]`; TB-303 page `[1: SAW/SQR]`, `[2: OCT -]`, `[3: OCT +]`, `[4: SLIDE]`, `[5: CLASSIC]`, `[6: SQUELCH]`, `[7: PAGE SND]`, `[8: DUB]`; Kick page `[1: DRV TYP]`, `[2: FLT MOD]`, presets and direct filter selectors.
  - **PATTERN soft keys**: `[1: PAT <]`, `[2: PAT >]`, `[3: + PAT]`, `[4: DUPL]`, `[5: VIDER]`, `[6: SUPPR]`, `[7: 1 MES]`, `[8: 2 MES]`. `SUPPR` is ignored when only one pattern remains in the Group.
  - **Latched workflow menus**: PATTERN, STEP, KEYBOARD, SCENE, EVENTS, MACRO, SETTINGS, and PLUGIN remain active after the first press. Pressing the same button again returns to the default Pad workflow. Only one workflow menu is active at a time. Pressing any top view button (FILE/SAVE, BROWSER, PLUGIN, CHANNEL/MIDI, MIXER, SAMPLING, ARRANGER) automatically exits any active latch (PATTERN, SCENE, SELECT, ERASE, MUTE) and closes overlays (EVENTS, MACRO, SETTINGS) before entering the selected view.
- **8 Groups A..H & 128 Independent Tracks**:
  - Full matrix of 8 groups × 16 pads = 128 distinct sound tracks with independent polyphony, sampler zones, synth engines, ADSR envelopes, filters, volumes, and pans.
  - **Group A**: Kit 808 with TB-303 Acid synth on pad 15.
  - **Group B**: Kit 909 with Kick Modeler synth on pad 15.
  - **Group C**: Nexus Wavetable synth on pad 1.
  - **Group D**: Bass & Acid bank (pads 1..3).
  - **Groups E..H**: Percussion, Keys & Chords, Vocal & SFX, Hits & Stabs.
  - **MUTE / SOLO + Group**: holding MUTE or SOLO and pressing a group button instantly mutes or solos that group without losing track/pad focus. An actively held SOLO button takes priority over latched MUTE pad mode.
- **Transport**: PLAY, REC, STOP, RESTART (clean restart or loop toggle with Shift), TAP (metronome), ERASE (erase notes by pad or clear pattern with Shift).
- **Touchstrip**: modulation / pitch bend / perform / notes with LED ribbon visual feedback.

### Project workflow

```bash
# Direct launch with GUI and PipeWire audio engine
maschine-linux

# Launch in optimized console daemon mode
maschine-linux daemon --project my_project.json
```

## Integrated GUI (`egui`)

The GUI also starts without a Maschine connected: the engine, mouse/keyboard controls, virtual MIDI and system audio output remain available for development. If an MK3 with `hidraw` is detected at launch, hardware inputs, LEDs and screens activate automatically. Use `--no-hardware` to ignore a connected controller and force standalone mode; hot-plug reconnection is disabled in that mode. The headless daemon deliberately remains strict and requires the controller. A remembered but unavailable audio output (for example the MK3 unplugged) falls back to the system output, unless `--audio-device` was explicitly provided.

The application bar can hide the navigation rail (`F1`), the MK3 mirror (`F2`), and the centre workspace (`F3`). `TOUT` toggles all three panels at once. Closing the window performs an ordered shutdown: the HID loop is joined, the screens receive a final splash and release IF5 without a USB reset, and the LED writer sends an all-off report so the MK3 returns to its default logo and idle pad state.

The native GUI displays in real time:

- **Application bar**: view picker, latched workflow indicators, and panel toggles (`F1` navigation, `F2` MK3 mirror).
- **Top transport bar**: BPM, Swing, BBT time cursor, Play/Rec/Stop/Loop status, CPU and active voice monitoring, metronome.
- **Collapsible and resizable panels**: the navigation rail and the MK3 mirror adapt to the window size and can be hidden for a compact workspace.
- **Multi-bar step sequencer (16 tracks)**:
  - Active bar selector (1, 2, 3, 4...) and `◀` / `▶` navigation arrows.
  - Pattern sizing buttons (1M, 2M, 4M, 8M).
  - 16 full instrument rows with `▶` preview buttons and 16 steps per bar.
- **4×4 pad grid**: layout identical to the MK3 hardware for direct playing with mouse or touchscreen. The pads respect the active PATTERN, SCENE, SELECT, ERASE, STEP, and KEYBOARD workflows.
- **Keyboard / Performance view (`K`)**: 16-key performance keyboard, Sound auditioning, previous/next Sound selection, and direct pad access.
- **Unified PLUGIN view (`T` or `P`)**: SOUND page with 808 preset selector, gain, pan, resonant filter with quick LP/HP/BP/Off selection, ADSR, per-Sound polyphony and choke.
- **Nexus / Serum Wavetable Synth (`PLUGINS` -> `NEXUS SYNTH`)**: 16-voice polyphonic instrument with multi-oscillators, 7-voice unison supersaw, acoustic/digital tables (Grand Piano, M1 Piano, Rhodes, Formants), multimode SVF filter (LP12, LP24, HP, BP), dual ADSR envelopes, LFO modulation, and stereo FX rack (Chorus, Ping-Pong Delay, Reverb, Distortion) with 28 factory presets (Concert Grand Piano, House M1 Piano, Vintage Upright Piano, Honky Tonk, Neo-Soul Rhodes, DX7 E-Piano, 808 Sub Boom, Reese Bass, Acid 303 Sub, Neurofunk Slither Bass, Slap Funk Bass, Cyberpunk Bass, Future House Donk, Titanium Supersaw Lead, Future Rave Lead, Hyperpop Chiptune Lead, Blade Runner Brass, Cyber-Drive Synth, Trance Euphoria, Celestial Strings, Analog Warmth, Cinematic Dark Pad, Spire Pluck, Pizzicato Strings, Kalimba Dream, Crystal Mallet, Deep House Organ, Hammond B3 Organ).
- **Techno Kick Synth Modeler (`PLUGINS` -> `KICK SYNTH`)**: Analog & techno kick drum modeling with exponential pitch drop, 7 body waveforms (Sine, Triangle, SatSine, SubSquare, NoiseTransient, Parabolic, Ramp), 7 drive & distortion models (Analog Tanh, Hard Clip, Wavefolder, Tube Drive, Bitcrush, Fuzz, Rectifier), 6 multimode resonant filter modes (Off, LowPass, HighPass, BandPass, Notch, Peak) with cutoff & resonance, FM linear frequency modulation, hold stage before pitch/amplitude decay, transient click layer, sub-bass tuning, and 13 factory presets (Techno Berlin, 909 Punch, Hardcore Wavefolder, Sub 808, Minimal Click, Industrial Tube, Acid Gabber, Deep Dub, Frenchcore Fuzz, Dark Psy FM Kick, Glitch Bitcrush, 909 Parabolic Punch, Trap 808 Long Boom).
- **Acid TB-303 page**: PolyBLEP oscillator (Saw/Square, octave, slide), 4-pole resonant low-pass filter with frequency response curve display, tanh distortion waveshaper with dynamic transfer curve, stereo ping-pong delay with graphical echo preview, quick preset selector and 16-key virtual keyboard (C1 to D#2) to play directly with the mouse.
- **8-group mixer (A..H) & Pad VU drill-down**: volume faders, peak indicators, mute and solo. On MK3 hardware, pushing the 4D encoder **Up** drills down into the focused group's pad mixer view (with 4 wide channel strips per screen: Pads 1-4/5-8 or 9-12/13-16) with individual live audio VU meters (green/yellow/red gradient), clip indicators, peak hold markers, and volume faders that strictly isolate each sound. Pushing 4D encoder **Down** returns back to group overview / master view. In the GUI, sound strips show dynamic dual VU meters alongside volume faders.
- **Instant Filter Mode Selection**: In Sampler and Plugin views, filter modes (`LP`, `HP`, `BP`, `OFF`, `NOTCH`, `PEAK`) can be selected instantly on MK3 hardware via Soft Buttons 5..8, cycled via Soft Button 4 (`CYC FLT`), or cycled on-the-fly by holding `Shift` and turning Knob 3 (Filter Cutoff) or pushing `Shift + EncoderPush`.
- **Import a sample**: in Sampler, enter the path of a WAV file and start the import. Decoding happens in the background; the GUI remains interactive. The selected Sound is replaced and the path is saved in the project.

Useful shortcuts: `I` Ideas, `S` Sequencer, `K` Keyboard, `M` Mixer, `A` Sampler, `T`/`P` Plugin/Synth, `B` Browser/File, `Space` Play/Stop, `F1` toggle navigation, `F2` toggle MK3 panel, `Ctrl+S` save, `Ctrl+Shift+S` quick save. On MK3, `FILE` opens the project browser, `Shift + FILE SAVE` performs a quick save, pushing 4D Up in Mixer enters group pad strips, 4D Down exits back to groups/master, and `Shift + EncoderPush` in Sampler cycles filter mode.

The official Sample Editor (recording, playback ranges, slicing and zone editing from the MK3) is not yet implemented. The EVENTS screen currently displays information without editing events.

## Hardware LCD screens (USB Bulk)

The two 480×272 LCDs are driven over USB Bulk (Interface 5). Rendering is compared at 30 Hz maximum; only modified regions are sent, with an average budget of 1.6 MB/s and a periodic full rewrite to resynchronize the screens. Windows must have an even width (RGB565 pixel pairs).

To test the physical workflow after compilation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p maschine-engine --no-default-features
cargo test --workspace
cargo build --workspace --release
./target/release/maschine-linux gui --no-audio
```

On the MK3, select **EVENTS**, then **SAMPLER**: the right screen must return to the Sound envelope. Then turn knob 1; the displayed Sound volume must respond from the first detent. Press **PATTERN**, **STEP**, **KEYBOARD**, and **SCENE** once each: the matching GUI view must open and the button must stay latched. Press the same button again to return to the Pad workflow. Press **PLUGIN** once: the GUI must show the SOUND page; use soft key 3 to switch to TB-303, then soft key 7 to return to SOUND. Press **PLUGIN** again to close it. The GUI window alone and a successful USB log do not validate the content visible on the two LCDs.

For a hardware-free smoke test, run `./target/release/maschine-linux gui --no-audio --no-hardware` and verify that the GUI starts, the MK3 is ignored, and no hot-plug reconnection appears in the logs.

To validate sample loading, import a WAV from Sampler, press `Ctrl+S`, then close and reopen the saved project via the `--project` option. Check that the Sound name, waveform and audition are still present. The automated cases and their correspondence with CI commands are described in [TESTING.md](TESTING.md).

## Latency

Target: <6ms round-trip @48k/128.

Measurement:

```bash
pw-top          # quantum 128, ERR 0
pw-metadata -n settings 0 clock.force-quantum 128
jack_iodelay    # if jack2 installed
rtcqs           # linuxmusicians script
```

If xruns occur: switch to 256 in `99-maschine-lowlatency.conf`.

## Debug

```bash
# raw hid
sudo cat /sys/kernel/debug/hid/0003:17CC:1600.*/rdesc | hidrd-convert --spec --output spec
hexdump -C /sys/class/hidraw/hidraw3/device/report_descriptor | head

# usb
lsusb -v -d 17cc:1600 | grep -A5 Interface
lsusb -t

# alsa
cat /proc/asound/cards
aplay -l; amidi -l; aconnect -l
pw-dump | grep -A5 maschine
wpctl status
journalctl --user -u pipewire -u wireplumber -f
```
