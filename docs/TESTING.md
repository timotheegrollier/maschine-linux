# Automated and hardware checks

This guide pairs each documented promise with a concrete check. Cargo tests cover deterministic behavior; only manual checks confirm what the two screens and physical controls actually display.

## Automated checks

| Area | Check | Representative coverage |
|---|---|---|
| Style and compilation | `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings` | Formatting and warnings treated as errors, including tests. |
| Minimal configuration | `cargo check -p maschine-engine --no-default-features` | The engine compiles when optional WAV import is disabled. |
| LCD protocol | `cargo test -p maschine-proto` | Valid RGB565 windows, even width, single-pixel expansion, pair counter and exact framebuffer reconstruction (`differ_always_reconstructs_exactly`). |
| LCD sending | `cargo test -p maschine-hardware` | Completion and termination of an interrupted window, including the short-write case captured in the logs; HID reports and LED feedback. |
| Hardware navigation | `cargo test -p maschine-hardware selecting_main_view_clears_events_overlay` | Switching from EVENTS to a main view restores its contextual right-hand page. |
| Engine navigation | `cargo test -p maschine-engine selecting_sampler_restores_contextual_right_screen_after_events` | SAMPLER resumes its right-hand screen after EVENTS; changing view outside ARRANGE leaves Song mode. |
| WAV import and playback | `cargo test -p maschine-engine` | Signed 32-bit PCM WAV, state preserved if import fails, cached waveform, Group isolation, writing mappings into the project and reloading a relative path. |
| Mixing during playback | `cargo test -p maschine-engine sound_and_group_mix_edits_reach_active_voices_immediately` | Sound/Group volume and pan are reflected on voices already triggered. |
| Acid TB-303 synthesizer | `cargo test -p maschine-engine acid_synth`; `cargo test -p maschine-app gui_plugin_navigation_routes_knobs_by_page` | PolyBLEP synthesis (Saw/Square), 4-pole tanh resonant filter, distortion, ping-pong delay, preservation of tab 4 and routing of the 8 physical encoders by PLUGIN page. |
| Latched MK3 menus | `cargo test -p maschine-hardware step_button_latches_and_toggles_back_to_pad keyboard_button_latches_and_toggles_back_to_pad pattern_and_scene_buttons_latch_and_exclude_each_other events_macro_and_settings_are_toggle_menus plugin_button_latches_and_toggles_back_to_pattern` | PATTERN, STEP, KEYBOARD, SCENE, EVENTS, MACRO, SETTINGS and PLUGIN latch on first press and return to the default workflow on second press. |
| Per-Sound sampler control | `cargo test -p maschine-engine per_sound_polyphony_steals_only_that_sound` | Polyphony is enforced per Sound and voice stealing does not affect another Sound. |
| Standalone GUI | `cargo test -p maschine-app gui_accepts_the_no_hardware_flag` | `gui --no-hardware` parses correctly and forces the hardware-free path. |
| FILE/project workflow | manual / integration coverage | `FILE` opens the project browser, navigation loads projects, `Shift + FILE SAVE` quick-saves, and saves produce readable `*.maschine-linux.json` files. |
| Mixer per pad | manual / integration coverage | Selecting pads changes the editable Sound; per-pad volume/pan edits apply and persist per Group. |
| Keyboard/Chords | manual / integration coverage | Root/scale/octave/chord controls change musical state; pads play pitched notes/chords; MK3/GUI screens reflect the state. |
| GUI/HID integration | `cargo test -p maschine-app` | Event routing, context transitions and the 2×1 RGB565 recovery probe. |
| General regressions | `cargo test --workspace` | All unit and integration tests in the workspace (167 automated tests). |
| Universal filter shortcuts | `cargo test -p maschine-hardware test_4d_encoder_shift_push_cycles_filter_in_all_views test_knobs_shift_filter_cycle_in_plugin_and_arrange` | Shift+Push on 4D encoder and Shift+Knobs cycle sound filter modes across all views without switching screens. |
| 8 Groups & 128 Tracks | `cargo test -p maschine-hardware test_all_eight_groups_and_pads_routing`; `cargo test -p maschine-engine test_synth_pad_mixer_volume_and_mute` | All 8 groups (A..H) and 16 pads per group route actions independently to distinct voices, synth models, and mix parameters. |
| Mute/Solo with Groups | `cargo test -p maschine-hardware test_mute_and_solo_with_groups` | Mute/Solo modifiers combined with Group buttons target groups without losing track focus; actively held Solo takes precedence over latched Mute pad mode. |
| 4D Navigation & Editing | `cargo test -p maschine-hardware test_4d_encoder_navigation_and_transport_shortcuts test_erase_mode_combinations test_note_repeat_and_touchstrip_modes` | 4D directional controls (browser, mixer, step scroll, pattern step), Note Repeat soft keys, Touchstrip mode select, and transport shortcuts. |
| Sustained Synth Notes | `cargo test -p maschine-engine test_acid_synth_pad_hold_sustains_indefinitely test_wavetable_synth_pad_hold_sustains_indefinitely` | Long pad presses hold notes indefinitely; stuck sensor detection isolates resting noise (< 0x4280) from intentional holds. |
| Clean App Restart | `cargo test -p maschine-app` | Detached restart via nohup/sh restores original working directory and strips AppImage environment variables. |
| Optimized binary | `cargo build --workspace --release` | Compilation of the binary and crates in release mode. |

GitHub CI runs formatting, Clippy, engine build without Sampler, workspace tests and release build on every push/PR to `master` or `main`.

## Hardware smoke tests

With the MK3 connected, the following commands exercise the output paths without requiring a DAW:

```bash
./target/release/maschine-linux probe
./target/release/maschine-linux led-test --pad 0 --color 6
./target/release/maschine-linux pad-test --step-ms 180
./target/release/maschine-linux display-test --r 0 --g 160 --b 80 --duration 1200
```

Expected results: `probe` reports `17cc:1600`, the `hidraw` device and an openable HID handle; `led-test` writes report `0x80`/`0x81`; `pad-test` lights pads 1..16 in logical order; `display-test` sends a solid RGB565 frame. A successful USB write confirms transport acceptance, not visible pixels, so also inspect both LCDs.

Validation performed for 0.8.0 on a connected MK3 (`serial=1B64AB9B`, `/dev/hidraw2`):

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` (167 tests) pass.
- `probe`: device detected, `hidraw open: OK`, ALSA card `MK3` present.
- GUI hardware startup: LED writer, display thread, first frame OK, serial logged, updater reported up to date.
- GUI hardware close: the HID loop and display/LED writers are joined, the final splash is sent, the display interface is released without a USB reset, and the log reports `Écrans libérés — logo MK3 par défaut`.
- `probe` after GUI exit: the MK3 is still detected and `hidraw open: OK`, confirming the shutdown did not reset or lose the device.
- GUI standalone startup and close with `--no-hardware`: no hardware session, no hot-plug reconnect, virtual MIDI opened, window closes cleanly.
- Automated mapper tests cover PATTERN-mode `DUPLICATE`, `SHIFT + ERASE`, and 4D stepping; engine/display tests cover pattern CRUD, slot creation, Scene re-indexing, soft keys, pattern-mode screens, and the shutdown splash.

## Manual check on the Maschine MK3

USB does not provide acknowledgement of displayed pixels here. After launching `./target/release/maschine-linux gui --no-audio` with the MK3 connected:

1. Check that both screens show the application content.
2. Open **EVENTS**, then **SAMPLER** from the hardware; the right screen must leave EVENTS and show the Sampler page.
3. Turn an encoder once; the displayed value and the active parameter must change on the first detent.
4. Switch between ARRANGE, MIXER and SAMPLER via the GUI and then via the hardware; check that the right page follows the current context.
5. Press **PATTERN**, **STEP**, **KEYBOARD**, and **SCENE** once each; the matching GUI view must open and the button must stay latched. Press the same button again to return to the Pad workflow. Verify that only one workflow menu is active at a time.
6. Press the physical **PLUGIN** button or click **PLUGIN / TB-303** in the GUI (shortcut `T` or `P`): the GUI and left screen must show the SOUND page. Use soft key 3 (`PAGE TB`) to switch to TB-303; the left screen must show the oscillator and resonant filter curve, the right screen the distortion and delay. Soft key 7 (`PAGE SND`) returns to SOUND. Encoders 1..8 must follow the active page live without flicker or unwanted feedback. Press PLUGIN again to close the editor.
7. In Sampler, enter a WAV path, import it, then listen to the pad. Press `Ctrl+S` (the exact path is shown in the interface), close the application and reopen the saved file with `./target/release/maschine-linux gui --project "$HOME/.local/share/maschine-linux/autosave.json" --no-audio`. The name, waveform and sound must be restored.
8. Press **PATTERN**, then tap empty pads: the corresponding slots must be created and shown on the left screen. Press `DUPLICATE`, `SHIFT + ERASE`, the 4D left/right encoder, and soft keys 3/5/7/8; the pattern list, active selection, event count, and length must follow immediately. The last pattern of a Group must not be deletable.
9. In the GUI, use the pattern strip to create, duplicate, clear, delete, and rename a pattern. In Ideas, click an empty matrix slot: it must create that pattern and assign it to the active Scene. Check that existing Scene references remain valid after deletions.
10. Hide the centre workspace with `F3`/`CENTRE` and the MK3 panel with `F2`; `TOUT` must show or hide all panels. Close the window and confirm that both MK3 screens return to the default Maschine logo and the pad LEDs turn off.

The complete official SAMPLING feature (RECORD, EDIT, SLICE and ZONE editing) and event editing are not covered as implemented: they remain outside the current scope. See [workflow.md](workflow.md) for details on the gaps and available controls.

For continuous integration, release building, and auto-updater diagnostics, see [CI_CD_RELEASE.md](CI_CD_RELEASE.md).
