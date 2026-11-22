# Status and handoff

Living document: what works, what is partial, what is missing, and what to
check on real hardware. Update it with every release (see the release steps in
`AGENTS.md`). Behaviour details live in `workflow.md`; this file is the map.

## Current version: 0.19.3

### Done (with tests)

| Area | What | Tests / proof |
|---|---|---|
| MK3 screens | Official layout (labels top, knob slots bottom), Barlow typography, group-colour accent, anti-aliased shapes (tiny-skia), page transitions | `display_render` tests, `dump_screens` |
| PLUG-IN | Every instrument parameter on pages (Page ◄ ►), plugin chain on the 4D (instrument, Sound / Group / Master inserts, `+ effet`) | `plugin_chain_inserts_edits_and_bypasses_effects_from_soft_keys`, `plugin_pages` tests |
| Effects | Filter, EQ, Distortion, Compressor, Lo-Fi, Chorus, Delay, Reverb; per-slot DSP state; real Sound → Group → Master buses | `fx::tests`, `sound_group_and_master_inserts_process_the_audio` |
| Sound quality | Transparent master limiter, Hermite sampler interpolation, PolyBLEP oscillators, guitar amp (oversampler fixed, compensated gain, 24 dB/oct cabinet) | `limiter` tests, `maschine-dsp` amp tests |
| SAMPLING | Sample editor: RECORD (MK3 input → WAV → pad), EDIT (play range, tune, gain, ADSR), SLICE (grid / transients → pads), ZONE (root, key and velocity ranges, fine tune, pan, gain) | `sample_editor_pages_edit_slice_zone_and_record`, `play_range_*`, `onsets_*` |
| EVENTS | Note editing: position, note, velocity, length; select, delete, duplicate, quantize, nudge; one Sound or all | `events_mode_edits_the_selected_note_with_knobs_and_buttons` |
| Groups together | One synth instance per Sound, meters for all groups, SOLO / MUTE in playback and live, queued scene / pattern launch on the next bar | `every_group_plays_at_once_*`, `launches_wait_for_the_next_bar_*` |
| LEDs | Pads: off / dim / normal (selected) / bright (playing, hit, sequencer); group buttons: focused in assigned colour, playing bright, idle dim, muted/empty off; explicit bright palette mapping (white fallback) | `bright_pad_color_is_valid_and_never_dimmer`, `pad_stays_lit_while_its_sound_is_playing`, `group_button_lights_when_its_group_plays` |
| Arpeggiator | NOTE REPEAT in KEYBOARD / CHORDS plays the held notes one at a time, upwards, at the repeat rate (`arp_held`, `arp_step`) | `note_repeat_in_keyboard_mode_arpeggiates_held_notes_upwards` |
| Audio-thread recording | Notes repeated by the audio thread (Note Repeat, Arp) are queued (`pending_notes`) and written by `commit_launches` (handle_event + HID loop at 60 Hz) | `notes_repeated_by_the_audio_thread_are_written_outside_it` |
| 16 Velocities | `SHIFT + FIXED VEL`: engine remaps pad → velocity of the selected Sound (MK3 and GUI); mapper keeps the selection, shades the pads | `sixteen_velocities_*` (engine and mapper) |
| Arp modes | Knob 7 in KEYBOARD / CHORDS: up, down, up-down, one or two octaves (`ARP_MODES`) | `arp_modes_go_down_bounce_and_add_the_octave` |
| EVENTS multi-edit | Knob 6 `SÉLECTION`: one note ↔ every note in scope, relative edits, `SUPPR` all | `events_knob_six_selects_every_note_for_relative_edits` |
| Count-in | `SHIFT + REC` (or `REC` with `COUNT-IN` on): one 4/4 bar of clicks in `advance`, then `record()`; REC LED blinks | `shift_rec_counts_in_one_bar_of_clicks_then_records` |
| KEYBOARD / CHORDS | One source for pad notes (`KeyboardState`, synced to the mapper as `kbd_pad_notes`), C3 = MIDI 60 chromatic default, OCT / SEMI buttons 5–8, root pads lit, in-scale chords (Harmonizer-like), knobs = on-screen settings | `pads_start_on_c3_*`, `chords_climb_*`, `keyboard_and_chords_knobs_*` |
| Held notes | Firm holds without HID reports, note-off only for the owning synth, menus keep held pads | `replay_firm_hold_without_reports_keeps_the_note` |
| Standby | Standby screen on quit, on panic, and on plug-in (udev + user unit; `standby --install` for AppImage / tarball) | manual (needs hardware) |
| GUI | Single header, colour-coded group rail, MK3 deck at the bottom (screens, buttons, knobs, pads, PAGE / SLOT / EVENTS), plugin chain strip, sample editor strip, nothing clipped from 420×340 up, deck folds below 620 px | Xvfb captures at 1920×1080 → 800×600 |

### Partial or not implemented

- **Sample editor**: no destructive cut / paste / fades on the audio (only
  REVERSE and NORMALIZE, stored in `Sound::edits` and replayed on load), one recording source (the MK3 / default capture
  input), the guitar amp is muted while recording, no slice-to-pattern, the
  synthesized 808 kit cannot be sliced (no file).
- **EVENTS**: one note or every note in scope (knob 6); no arbitrary subset,
  no velocity ramps, `DUPLIQ` copies one note.
- **Zones**: ZONE edits all zones of the Sound together; no multi-zone editing
  from the controller.
- **LED levels**: `bright_pad_color` now uses an explicit per-hue table and a
  white fallback when the palette has no brighter variant; the previous
  `4k+1..4k+3` rule produced invalid indices (and dimmed Spring). Exact factory
  brightness remains user-adjustable in NI's Hardware preferences, so a visual
  check on hardware is still worthwhile.
- **Keyboard / Chords**: no scale banks (8 scales), no Chord Set / User Chords
  modes; the arpeggiator has up / down / up-down over one or two octaves, no
  gate, sequence or "order played"; display buttons 1–4 are our own (scale
  and chord shortcuts), the official ones are not documented.
- **Destructive sample editing** (cut, paste, fades on the audio) is not
  planned for now: the play range, the ADSR (2 ms attack by default) and the
  3 ms anti-click fade at the end of the range cover clicks without rewriting
  files.
- **Long press on pads**: the engine sustains synth notes while a pad is held
  (tests); if notes still stop early on hardware, capture `hid-dump` with a held
  pad — the tracker releases a pad when it disappears from the 0x02 report.
- **Secondary screens** (mixer, browser, step) use the new typography, palette,
  rounded shapes and knob slots, but keep their older layouts.
- **Standby before any launch** needs the packages (or `standby --install`) and
  a logged-in systemd user session; a `SIGKILL` leaves the last frame.
- **Automation** (`CLEAR AUTO`) and touch-sensitive knob behaviour are not
  implemented.

### Check on real hardware (not covered by CI)

1. Plug the MK3 with the application closed: the standby screen appears after
   about 3 s (`journalctl --user -u maschine-linux-standby` otherwise).
2. PLUG-IN: turn the 4D to `+ effet`, insert each effect, listen on a pad,
   `BYPASS`, `SUPPR`.
3. SAMPLING: `RECORD` → `ENREG` → play → `STOP`; the take plays on the pad;
   `SLICE` → `8` → `APPLIQUER` spreads it over 8 pads.
4. EVENTS: select notes with the 4D, move one with knob 1, listen.
5. Guitar amp: levels from clean to full gain with a real guitar.
6. Pads: colour on hit, held notes on synths (TB-303, Nexus) for > 10 s.
7. KEYBOARD: pad 1 plays C3 at the sample's pitch, root pads lit, `SEMI +`
   from B3 gives C4; CHORDS in a major scale: pad 2 plays a minor chord.
8. Play, then pick another pattern / scene: its pad blinks and the screen marks
   it `SUIVANT` until the next bar.

### Suggested next steps

- Sample editor: destructive EDIT functions (truncate to range, fades, cut /
  copy / paste), slice-to-pattern, recording through the guitar amp.
- EVENTS: multi-selection with pads (select all notes of a Sound) and relative
  edits on the selection.
- Redesign the mixer, browser and step layouts with the same care as PLUG-IN
  (see `docs/design.md`).
- Automation lanes (knob recording) — would make `CLEAR AUTO` meaningful.
- Confirm the pad LED behaviour on hardware and adjust `HardwareMapper::update_leds`.

## History (recent)

- **0.19.3** — LED du groupe sélectionné : plus illuminé que les autres
  (atténués même en lecture).
- **0.19.2** — illumination des pads pendant la lecture du pattern (notes
  jouées dans le pattern en mode keyboard).
- **0.19.1** — LEDs MK3 : bouton MUTE hors mode mute, groupe et pad
  sélectionnés plus illuminés ; notes pendant REC — son pendant la tenue
  (slide à l'écran).
- **0.19.0** — slides enregistrés pendant REC (glissando du TB-303 entre
  notes tenues), notes enregistrées pendant REC annulées par Shift+Undo.
- **0.18.0** — notes enregistrées pendant REC gardent la longueur de la note
  tenue (pads et clavier), automations de knobs enregistrées dans le Pattern
  pendant REC.
- **0.17.0** — 16 Velocities, arpeggiator modes, EVENTS multi-note editing.
- **0.16.0** — arpeggiator, queued recording of repeated notes,
  sample edits persisted, PATTERN screen follows launches, FIXED VEL in
  KEYBOARD, PLAY cancels the count-in, `OFF` label on the note repeat page,
  French default names, unused dependencies removed.
- **0.15.0** — all groups play together (synth rack), meters B–H, recording
  into groups without a pattern, SOLO, queued launches, MK3-like pad and group
  LEDs, GUI mute / solo per group, startup progress on the MK3, STOP twice,
  SHIFT + REC count-in.
- **0.14.0** — sample editor (RECORD / EDIT / SLICE / ZONE), EVENTS note
  editing, mixer knob slots, cleaner browser and step screens, GUI EVENTS
  button and sample editor strip, `standby --install`, agent docs (`AGENTS.md`,
  this file).
- **0.13.1** — EQ, Chorus, Lo-Fi; standby on plug-in; GUI plugin chain strip;
  pattern grid redesign; deck auto-fold.
- **0.13.0** — plugin parameter pages and chain, insert effects with real
  buses, master limiter, guitar amp fix, held-note fixes, full redesign of the
  MK3 screens and GUI (`docs/design.md`).
- **0.12.0** — official MK3 screen layout, single source for lit buttons, GUI
  mirror with real knob mapping and LED-coloured pads.
