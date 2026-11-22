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

The GUI follows Maschine's workflow separation: a common transport bar, groups and scenes on the left, then a dedicated workspace. View buttons stay synchronized with the MK3 hardware tabs. Pressing any top view button (FILE/SAVE, BROWSER, PLUGIN, CHANNEL/MIDI, MIXER, SAMPLING, ARRANGER) automatically exits latched modes (PATTERN, SCENE, SELECT, ERASE, MUTE) and closes overlays (EVENTS, MACRO, SETTINGS).

- **Ideas**: horizontal scenes and a Group × Pattern matrix with 8–16 slots per Group. Clicking an existing pattern assigns it to the scene and focuses its group; clicking an empty slot creates that pattern and assigns it. The scene can be duplicated/deleted (with automatic re-indexing of Song sections). A scene can be added from this view.
- **Song**: horizontal timeline of sections, direct selection, previous/next navigation, duplication, deletion and section length (1/2/4/8 bars).
- **Sequencer**: 16 × 16 steps, navigation and pattern length from 1/2/4/8 bars, preview pads and groove actions. The pattern manager above the grid creates, selects, duplicates, clears, deletes, renames, and resizes patterns. Cells are prepared in a single pass over events to keep editing smooth.
- **Keyboard / Performance**: Maschine-like Keyboard/Chords for the active Sound. Pad 1 plays the root note (C3 = MIDI 60, chromatic by default); buttons 5–8 are `OCT -`, `OCT +`, `SEMI -`, `SEMI +` (SEMI carries into the octave: B3 → C4); buttons 1–4 pick the scale (`CHROMA`, `MINOR`, `SCALE-`, `SCALE+`) or, in CHORDS, scale and chord. Knobs: KEYBOARD `1:ROOT 2:SCALE 3:OCT 4:SND`, `6:GRP 7:ARP 8:MASTER`; CHORDS `1:ROOT 2:SCALE 3:CHORD 4:INV`, `5:OCT 7:ARP 8:MASTER`. `7:ARP` sets the arpeggiator: `HAUT`, `BAS`, `H-B` (up then down), or the same over two octaves (`HAUT 2`, `BAS 2`, `H-B 2`). Pads light in the Sound colour: root note and its octaves lit, other pads dim, hit bright. In a scale, chords stack scale degrees from the pad's note (`1-3-5`, `1-3-5-7`, `1-3`, `1-5`, `1-4-5`, `1-4-7`, like the Harmonizer); in Chromatic they use fixed intervals (Triad, Minor, Sus2, Sus4, 7th, Min7, Maj7, Oct). Hardware KEYBOARD and CHORDS modes open this view and show dedicated screens.
- **Mixer**: volume, pan, mute and solo for groups A–H, plus the master.
- **Sampler / Sound**: selection of the 16 Sounds, asynchronous WAV import, waveform, audition, volume, pan, filter, ADSR envelope, per-Sound polyphony and choke group. WAV paths are saved in the project zones and reloaded on open.
- **PLUGIN**: unified Sound editor. The SOUND page exposes the sampler-style controls; the TB-303 page provides simplified macros (cutoff, resonance, envelope modulation, decay, accent, drive, delay time, delay mix) with advanced oscillator, filter, distortion, and delay controls collapsed by default.
- **Browser**: built-in 808 drum synth presets, loaded onto the selected Sound. The Browser does not yet browse the NI library or the filesystem.

The GUI has a navigation rail, a centre workspace and an MK3 mirror. Each side
panel keeps a share of the window width: resizing the window resizes the
panels, and dragging a panel edge changes that share. Below 1400 px the rail
hides itself and comes back when the window grows again. `F1` toggles the
rail, `F2` the mirror, `F3` the centre workspace (the mirror then fills the
window with both screens side by side), and `TOUT` shows or hides all three.
The application bar shows the active latched workflow; the transport is a
single compact row (Play/Stop, Record, Restart, position, tempo, swing, loop,
metronome, song, master). Cards in every view lay their content out top-down
and wrap to the next row instead of overflowing under the mirror.

The MK3 mirror follows the controller from top to bottom: the 8 display
buttons (the label strip along the top of each screen image is clickable), the
two screens (side by side when the panel is wide enough, stacked otherwise,
never below 75 % so the 8×16 text stays readable), the 8 knobs, each centred
under the slot that shows its name and value, then groups A–H and the 4×4
pads. GUI knobs are endless encoders: a drag is sent to the HID loop as
`HardwareEvent::KnobDelta` and goes through the same mapping and speeds as
the physical knob. Pads and group buttons are painted from the LED state the
mapper computes for the MK3 (with or without a controller), so they show the
same colours as the hardware in every mode. GUI pads send note release; 808
presets are cached when the kit loads.

`maschine-linux gui --no-hardware` forces standalone mode: the GUI, engine,
virtual MIDI, and audio remain available, while any connected MK3 and its
hot-plug reconnection are ignored.

## MK3 screens (480×272) & soft keys

Both screens are rendered by the same function (`display_render::render_screens` /
`render_screens_with_guitar`): the MK3 panel in the GUI is the mirror of the content computed for the hardware. The USB loop compares renders at 30 Hz, sends only modified rectangles (up to six), limits average throughput to 1.6 MB/s and sends a full-screen pair every 2 seconds to resynchronize the controller. For the Guitar Amp page, `maschine-app` publishes a plain `GuitarScreenData` snapshot (atomics + telemetry) to both render paths, so the hardware screens and the GUI mirror are always identical.

- **Left screen**: ARRANGE (8 groups, scene pattern, real VU), SCENES/IDEAS (SCENE mode), PATTERN slots (PATTERN mode: 16 slots with name, length, event count, active/assigned markers), SONG (sections), MIXER A–D, SAMPLER (waveform + ADSR + info), BROWSER (808 presets), STEP (full pattern grid + selected step parameters), and PLUGIN (SOUND page or TB-303 page).
- **Right screen**: PATTERN (16 sounds × 16 steps grid + playhead), STEP (active Sound velocities + edit cursor), SONG TIMELINE, MIXER master + group/sound focus, SAMPLER full-screen envelope, BROWSER loading, EVENTS, MACRO, SETTINGS, PLUGIN SOUND envelope.
- **Display buttons 1–8 (above the screens)**: *contextual* action decided by the engine (`EngineEvent::SoftKey`). As on the official MK3, each label sits along the **top** edge of its screen, right under its physical button, and each knob's name and value sit along the **bottom** edge, right above its knob. A label is lit when the state it selects is the current one (`Engine::soft_key_active`: bar, pattern length, group, mixer bank, filter mode, metronome, song mode, note-repeat rate…), never because its index matches another state. The same lit state drives the screens, the GUI mirror and the button LEDs (dim when the button has a function, bright when lit, off without a label). The button always does what the label announces, identically on hardware and in the GUI.
  - **Dynamic Shift layer**: holding `SHIFT` immediately repaints all 8 soft key labels with contextual shift commands: `DOUBLE` (doubles pattern length), `HALF` (halves pattern length), `QUANT` (50% grid quantize), `HUMAN` (humanize timing), `SAV PROJ` (quick save), `COUNT-IN` (toggle count-in).
  - ARRANGE: `BAR 1..4` + pattern length `1/2/4/8 BARS`.
  - PATTERN: `PAT <`, `PAT >`, `+ PAT`, `DUPL`, `VIDER`, `SUPPR`, `1 MES`, `2 MES`.
  - STEP: `STEP <</>>` (cursor by one step), `BAR <</>>` (by one bar), `1/2/4/8 BARS`.
  - MIXER: `GRP A..H` (in Group mode) or `PAD 1-8`, `PAD 9-16`, `GROUPES`, `MASTER` (in Pad mode). Pushing 4D encoder **Up** enters pad drill-down; pushing 4D encoder **Down** exits back to group overview or master view. Pad mixer renders 4 wider channel strips per screen with isolated volume and mute.
  - NOTE REPEAT (the arpeggiator in KEYBOARD / CHORDS): buttons 1–7 pick the rate (`1/32`, `1/16`, `1/8`, `1/4`, `1/2`, `1/16T`, `1/8T`), lit on the active rate; `OFF` turns it off. In PAD mode held pads repeat; in KEYBOARD / CHORDS the held notes play one at a time, upwards, looping (the first key starts at once). Repeated notes are recorded while REC is on.
  - SAMPLER: `SND <`, `SND >`, `ECOUTE`, `CYC FLT`, `LP`, `HP`, `BP`, `OFF` (instant 1-click filter selection from buttons 5–8 or `Shift + EncoderPush` to cycle).
  - BROWSER: `PRES <</>>`, `SND <</>>`, `LISTEN`, `LOAD`, `METRO`, `LOOP`, plus `PLUGINS` tab with Kick Synth, TB-303, Nexus Wavetable Synth, Guitar Amp, and Sampler.
  - PLUGIN: see [PLUG-IN mode](#plug-in-mode-instruments-and-effects) below — buttons and knobs follow the selected plugin slot and its page.
  - **PLUGIN GUITAR AMP (page 2)** — deux écrans dédiés (`GUITAR AMP | PREAMP` et `GUITAR AMP | ANALYSE`) :
    - Soft keys 1–4 (écran gauche) : `AMP <`, `AMP >` (choix du modèle Vox AC30 / Fender Blackface / Tweed / High Gain), `CAB <`, `CAB >` (Vox 12 / Fender 12 / Modern 12 / Flat).
    - Soft keys 5–8 (écran droit) : `BYPASS`, `MONITOR`, `RESET`, `PAGE SND` (retour à la page PLUGIN SOUND).
    - Écran gauche : courbe de saturation du préampli, tone stack 3 bandes, état ACTIF / MON / BYPASS (ACTIF et MON sont **éteints au démarrage** ; l'utilisateur les active explicitement). Knobs **1:GAIN, 2:BASS, 3:MID, 4:TREBLE**.
    - Écran droit : VU entrée/sortie, accordeur (note + cents + Hz), spectre 48 bandes, CPU / Xruns / latence. Knobs **5:PRESENCE, 6:SAG, 7:MASTER, 8:MONITOR**.
  - SCENE: `SCN 1..8`. SONG: `SEC <</>>`, `+SEC`, `IDEAS`, scenes and `+SCN`.
- **Encoders**: each screen displays, along its bottom edge, the 4 parameters actually controlled by its knobs (1–4 on the left, 5–8 on the right) with live values. The GUI knobs send the same relative turns through the hardware mapper, so they always control exactly what the screen shows. PLUGIN routes knobs by page: SOUND uses Vol/Pan/Cutoff/Reso + ADSR, TB-303 uses Cutoff/Reso/EnvMod/Decay + Accent/Drive/Time/Mix, GUITAR AMP uses Gain/Bass/Mid/Treble + Presence/Sag/Master/Monitor.
- **Navigation between views**: choosing ARRANGE, MIXER, SAMPLER, BROWSER or PLUGIN returns the right screen to the page linked to that view. EVENTS, MACRO and SETTINGS are right-hand views that can then be explicitly selected. Clicking the active main view again in the GUI returns to its contextual right page.
- **Latched workflow menus**: PATTERN, STEP, KEYBOARD, SCENE, EVENTS, MACRO, SETTINGS and PLUGIN latch on the first press. A second press on the same button returns to the default Pad workflow and the Sequencer view. Only one workflow menu is active at a time.
- **PATTERN mode**: the left screen lists the 16 pattern slots of the focus Group with length and event count; the right screen shows the selected pattern grid. Patterns belong to their Group: each Group keeps its own pattern bank and its own selection in the current Scene, so working on one Group never changes another Group's pattern. Empty pads create a new empty pattern in that slot and select it; selecting a pattern only replaces that Group's reference in the current Scene, and pressing the selected pattern again keeps it selected. Pressing two pads simultaneously chains the patterns into a sequential loop;
the chain covers the whole interval between the two pads (pads 1 and 4 chain
patterns 1–4, pads 1 and 16 chain patterns 1–16). Pressing a single pad selects
it for the next bar while playing. **SHIFT + a pattern pad switches immediately**
(the pending launch and current chain are cancelled) — the equivalent of the
MK3 shortcut for auditioning a pattern without waiting for the bar line. The
generic SHIFT-pad layer (UNDO/REDO/…) stays active outside PATTERN mode.
`DUPLICATE` copies the active pattern, `SHIFT + ERASE` clears its events, and
the 4D encoder moves to the previous/next pattern. The last pattern of a Group cannot be deleted.
- **Fonctions Shift + pads MK3** : le séquenceur desktop affiche les mêmes libellés sur sa grille de pads lorsque Shift est tenu. Pad 1–16 (Pad 1 en bas à gauche) : `UNDO`, `REDO`, `STEP UNDO`, `STEP REDO`, `QUANTIZE`, `QUANT 50%`, `NUDGE ←`, `NUDGE →`, `CLEAR`, `CLEAR AUTO`, `COPY`, `PASTE`, `− SEMI`, `+ SEMI`, `− OCT`, `+ OCT`. Les actions ciblent le pattern focalisé; le presse-papiers copie/colle sa liste d'événements. Le projet ne possède pas encore de piste d'automation, donc `CLEAR AUTO` affiche un message d'indisponibilité.
- **Effacer les notes d'un Sound** : dans le séquenceur desktop, clic droit ou `Maj+clic` sur l'en-tête d'une ligne (clic droit sur un pad) retire les notes de ce Sound du pattern actif. Sur MK3, `ERASE + EVENTS + pad` fait la même chose; le Sound et ses réglages restent en place.
- **Supprimer un Sound du slot** : `SHIFT + ERASE + pad` vide le slot et retire ses événements. Sur le desktop, utiliser le menu contextuel de la ligne permet de faire l'action équivalente; cette suppression est destructive pour les réglages du Sound.
- **Clean shutdown and standby**: closing the GUI or stopping the daemon draws a sober standby screen (almost black, a dim `MASCHINE | MK3` word mark across both screens), releases the display interface without a USB reset, and sends an all-off LED report. A panic does the same before the process exits. With the RPM, DEB or Arch package, plugging the MK3 in also shows the standby screen — even if the application was never started — through the udev rule and the `maschine-linux-standby.service` user unit (`maschine-linux standby`, which does nothing when the application already owns the screens). A `SIGKILL` leaves the last frame on screen.
- **GUI and hardware controls synchronized**: before mapping each new HID event, the application re-reads the group, sound and tabs from the engine. The first knob detent after a page change therefore already performs the new page's action.
- **Live editing**: volume, pan, filter and ADSR changes also reach voices already playing; Group/Sound gains and pans combine without overwriting the neighboring level.
- **LCD alignment**: each USB rectangle must have an even width, because the Bulk counter represents pairs of RGB565 pixels. Differential rectangles are expanded to the neighboring pixel; odd windows are rejected before sending. See [the hardware protocol](HARDWARE_DISPLAY_PROTOCOL.md).
- **Real validation**: tests check encoding, rendering and control routing. An accepted USB transfer does not prove that the pixels are visible; check both LCDs on the MK3 after launching.

## PLUG-IN mode: instruments and effects

`PLUG-IN` shows the plugin chain of the focused Sound, like the official
software: its instrument (Sampler, TB-303, Kick Synth or Nexus Wavetable),
then the insert effects of the Sound, of its Group and of the Master, each
chain ending with a free `+ effet` slot. The left screen lists the chain and
shows the selected slot in large with a live visual; the right screen shows
the page tabs and the page visual.

| Control | Action |
|---|---|
| **4D encoder (turn)** | previous / next slot of the chain (GUI: `◀ SLOT` / `SLOT ▶`) |
| **Page ◄ ►** (and 4D left/right) | previous / next parameter page of the slot (LEDs lit when a page exists) |
| **Knobs 1–8** | the eight parameters shown above them; integer and choice parameters move one value per detent, `SHIFT` = fine |
| **4D push** | audition the Sound |

Buttons per slot:

| Slot | Buttons 1–4 | Buttons 5–8 |
|---|---|---|
| Sampler | `SND <` `SND >` `ECOUTE` `BROWSER` | page shortcuts (`LP/HP/BP` `NOTCH` `PEAK` `OFF`, `REVERSE` `LOOP` `NORMLZ` `CYC FLT`) |
| TB-303 / Kick / Nexus | `PRESET <` `PRESET >` `ECOUTE` `BROWSER` | page shortcuts (presets, waveform, filter modes, `OSC 2`, …) |
| effect | `< TYPE` `TYPE >` `BYPASS` `SUPPR` | the effect's modes or quick settings |
| `+ effet` | `FILTER` `EQ` `DISTO` `COMP` | `LO-FI` `CHORUS` `DELAY` `REVERB` |

Every effect uses the same knob layout — **knob 1 is always MIX** (dry/wet),
the effect's own settings follow:

| Effect | Knobs | Buttons 5–8 |
|---|---|---|
| Filter (resonant SVF) | MIX, CUTOFF, RESO, MODE | `LP` `HP` `BP` |
| EQ (low shelf 120 Hz, mid bell, high shelf 8 kHz, ±18 dB) | MIX, LOW, MID, MID FREQ, HIGH | `FLAT` `LOUDNESS` `RADIO` |
| Distortion (anti-aliased, level-compensated) | MIX, DRIVE, TONE, TYPE, OUTPUT | `SOFT` `HARD` |
| Compressor (soft knee, auto make-up) | MIX, THRESH, RATIO, ATTACK, RELEASE | `DOUX` `PUNCH` `SQUASH` |
| Lo-Fi (bit depth, sample rate, tone) | MIX, BITS, RATE, TONE | `8-BIT` `SP-1200` `CRUSH` |
| Chorus (two quadrature-modulated delays) | MIX, RATE, DEPTH, DELAY, FEEDBACK | `CHORUS` `FLANGER` `ENSEMBLE` |
| Delay (stereo / ping-pong, up to 2 s) | MIX, TIME, FEEDBACK, TONE, PING-PONG | `PING-PONG` `1/16` `1/8` `1/4` (tempo-synced times) |
| Reverb (stereo Freeverb) | MIX, SIZE, DAMP, PRE-DLY, WIDTH | `PIÈCE` `SALLE` `HALL` |

Audio routing: each Sound with effects plays into its own bus, processed by
its chain, then into its Group bus (Group effects), then into the Master
(Master effects), then through the transparent Master limiter (ceiling
-1 dBFS, 1 ms look-ahead). Every slot keeps its own DSP state, allocated off
the audio thread when the chain changes.

### Official reference and current scope

The MK3 manual describes SAMPLING as a Sample Editor tied to the focused Sound: buttons 1–4 select RECORD, EDIT, SLICE and ZONE; EDIT notably sets the play range, while ZONE handles mapping, tuning and key/velocity ranges. The controller's eight buttons and knobs are contextual and their function is displayed on screen ([official manual, Sampling and sample mapping](https://docs.native-instruments.com/ni-tech-manuals/maschine-mk3-manual/en/sampling-and-sample-mapping), [MK3 quick reference](https://docs.native-instruments.com/ni-tech-manuals/maschine-mk3-manual/en/quick-reference)).

Since 0.14.0 both modes follow that reference; see the two sections below. Not implemented: destructive editing beyond REVERSE / NORMALIZE (no cut, paste or fades on the audio itself), several zones per Sound from the controller (ZONE edits the zones of the focused Sound together), slicing to a pattern, and multi-note selection in EVENTS (one note at a time).

## Groups, launches and LEDs

- **All groups play at once**: each Sound with a TB-303, Kick Synth or Nexus has
  its own synthesizer instance, so groups never cut or re-tune each other.
- **MUTE / SOLO**: per group (MK3: `MUTE` / `SOLO` + group button; GUI: `M` /
  `S` in the group list or the mixer) and per Sound (`MUTE` + pad). SOLO keeps
  only the soloed groups audible, for patterns, live playing and synths.
- **Launching**: while the transport runs, a scene (`SCENE` + pad) or a pattern
  (`PATTERN` + pad, GUI Ideas grid) starts at the beginning of the next bar;
  its pad blinks and its row on the PATTERN / SCENE screen is framed and marked
  `SUIVANT` until then. Stopped, changes are immediate.
- **Transport**: `PLAY` toggles playback, `STOP` stops (again: back to the
  start), `REC` toggles recording (`SHIFT + REC` from stop: one bar of clicks,
  then recording starts; `REC` blinks meanwhile, `STOP` cancels; with the
  `COUNT-IN` display button on, plain `REC` counts in too),
  `RESTART` (`SHIFT`: loop), `TAP` (`SHIFT`: metronome).
- **Recording monitoring (Maschine flow)**: while `REC` runs, the pattern keeps
  playing normally — you hear your recorded loop immediately, without waiting
  to disarm REC. The Sound played live gets its **own monitoring voice**: the
  pattern and the live note use separate monophonic synth instances, so they
  are both audible and never cut each other (TB-303, Kick, Wavetable). All pad
  LEDs and screens keep mirroring the sequencer. Notes played during REC keep
  their held length, slides and knob automation are recorded.
- **16 Velocities**: `SHIFT + FIXED VEL` — the 16 pads play the selected Sound
  from velocity 7 (pad 1) to 127 (pad 16) without changing the selection; pads
  shaded in its colour, `FIXED VEL` half lit, GUI pads labelled `VEL n`.
  `FIXED VEL` turns it off (the two modes are exclusive).
- **Pad LEDs**: empty pad off, Sound present dim, selected Sound normal, Sound
  currently sounding bright (audio level/activity, so it stays lit for the
  whole note and not only during the 200 ms hit flash), hit or sequencer note
  bright (velocity ≥ 64 = brightest), muted Sound off (dim while `MUTE` is
  held). When the palette has no brighter variant of a hue, the flash uses
  white, as on the MK3. PATTERN mode: patterns in the group colour, playing one
  bright, queued one blinking. SCENE mode: scenes yellow, active bright, queued
  blinking. Group buttons: focused group bright in its assigned colour — the
  most lit of all; other groups dim even while they play; muted/empty off.

## SAMPLING: sample editor (RECORD, EDIT, SLICE, ZONE)

`SAMPLING` (left tab 2) edits the sample of the focused Sound. Display buttons
1–4 choose the page (lit = current), buttons 5–8 and the eight knobs belong to
the page. Select the Sound with the pads; the 4D push auditions it.

| Page | Buttons 5–8 | Knobs 1–4 | Knobs 5–8 | Screens |
|---|---|---|---|---|
| **RECORD** | `ENREG` / `STOP`, `SEUIL` | SEUIL (dB), DURÉE MAX (s) | — | state, input level with the threshold |
| **EDIT** | `REVERSE`, `LOOP`, `NORMLZ`, `PLAGE MAX` | START, END (play range), TUNE (semitones), GAIN | ATTACK, DECAY, SUSTAIN, RELEASE | waveform with the play range (outside dimmed, S / E markers), envelope |
| **SLICE** | `4`, `8`, `16`, `APPLIQUER` | TRANCHES, MODE (GRILLE / TRANSIT.), SENSIB | — | slice markers, one card per target pad with its length |
| **ZONE** | `RÉINIT` | ROOT, LOW KEY, HIGH KEY, FINE (cents) | LOW VEL, HIGH VEL, PAN, GAIN | 128-key map with the key range and root, velocity range |

- **Play range**: non-destructive, stored per zone in the project (`start` /
  `end`, fractions of the sample) and applied by the sampler voice (start
  position and end of playback).
- **REVERSE, NORMLZ, LOOP**: applied to the sample in memory and stored in the
  project (`edits` of the Sound), then replayed when the project is opened;
  the WAV file itself is never modified.
- **RECORD**: records the MK3 audio input (the capture stream also used by the
  guitar amp, which stays silent while recording) into the focused Sound;
  `SEUIL` waits for the input to cross the threshold. `STOP` writes a 32-bit
  float WAV to `~/.local/share/maschine-linux/recordings/` and loads it like an
  imported sample. The buffer is reserved when recording starts: the audio
  callback never allocates.
- **SLICE / APPLIQUER**: splits the play range on a grid or at the detected
  hits (energy jump in 5 ms windows, sensitivity knob) and spreads the slices
  over the pads from the focused Sound on (same sample file, one play range per
  pad, named `<name> #n`). The sample must come from a file (imported or
  recorded); the synthesized 808 kit cannot be sliced.

## EVENTS: note editing

The `EVENTS` button takes over both screens to edit the notes of the active
pattern one at a time: the left screen shows the whole pattern as a piano roll
(selected note in the group colour), the right screen lists the notes.

| Control | Action |
|---|---|
| 4D turn, knob 5, `EVT <` / `EVT >` | previous / next note (time order) |
| Knob 1 POSITION | one step per detent, `SHIFT` = 10 ticks |
| Knob 2 NOTE | one semitone per detent |
| Knob 3 VÉLOCITÉ | continuous |
| Knob 4 LONGUEUR | one step per detent, `SHIFT` = 30 ticks |
| `TOUS` | all Sounds of the pattern instead of the focused Sound (lit when on) |
| Knob 6 SÉLECTION | right: every note in scope (all highlighted; knobs, `QUANT`, `NUDGE`, `SUPPR` apply to all, each moved by the same amount), left: back to one note |
| `SUPPR` / `DUPLIQ` | delete the note / copy it right after itself |
| `QUANT` | snap to the 1/16 grid |
| `NUDGE <` / `NUDGE >` | ±60 ticks |

Edits keep the note selected even when it moves past another one. `EVENTS`
again (or any main view button) leaves the mode. In the GUI the deck has an
`EVENTS` button.

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
  - *Mixer Drill-Down & Live Pad VU Meters*: in MIXER view, pushing the 4D encoder (`EncoderPush`) or nudging Right toggles between Group Master strips (Groups A-H) and the Pad Mixer view (Pads 1-8 / 9-16). In the Pad Mixer view, both MK3 screens display live color-graded audio VU meters (green/yellow/red gradient), clip LED indicators, peak hold markers, and volume faders for each pad slice.
  - *Instant Filter Mode Selection*: in SAMPLER view, Soft Buttons 5..8 allow direct 1-touch switching between `LP`, `HP`, `BP`, and `OFF`, while Soft Button 4 triggers `CYC FLT`. Additionally, rotating Knob 3 (Filter Cutoff) while holding `Shift`, or pressing `Shift + EncoderPush`, instantly cycles filter types without menu diving.
  - *Techno Kick Synth Modeler*: dedicated sound design engine featuring 7 body waveforms (`Sine`, `Triangle`, `SatSine`, `SquareSub`, `NoiseTransient`, `Parabolic`, `Ramp`), 7 drive/distortion models (`AnalogTanh`, `HardClip`, `Wavefolder`, `TubeDrive`, `Bitcrush`, `Fuzz`, `Rectifier`), 6 multimode filter modes (`Off`, `LowPass`, `HighPass`, `BandPass`, `Notch`, `Peak`), linear FM modulation, hold stage, and 13 presets. On the hardware, `Shift + Knob 5` cycles drive type, `Shift + Knob 6` tunes cutoff, and `Shift + Knob 7` cycles filter mode.
  - *Nexus / Serum Polyphonic Wavetable Synth*: dual-oscillator ROMpler/wavetable synthesizer with 7-voice unison supersaw, acoustic and electronic tables, resonant multimode SVF filter, dual ADSRs, LFO, and a 4-effect stereo rack with 28 production-ready factory presets covering pianos, basses, leads, pads, plucks, organs, and strings.
  - *Contextual display buttons*: the 8 buttons above the screens execute the action labelled right under them, along the top edge of the screen (see "MK3 screens & soft keys"). Same action engine on the hardware and GUI sides.
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
