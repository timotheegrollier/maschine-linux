# Design system — MK3 screens and GUI

One set of tokens drives both surfaces: the two 480×272 screens of the
Maschine MK3 (`maschine-engine/src/display_render.rs`) and the desktop GUI
(`maschine-app/src/gui`). The GUI is the hardware plus more room — never
different semantics.

## Principles

1. **Edges belong to the controls.** On each screen the top band (26 px) is
   reserved for the labels of the four buttons above it, the bottom band for
   the four parameters of the knobs below it. Nothing else lives there.
2. **Colour means identity or state, never decoration.** The accent is the
   colour of the focused group — the same colour its pads light up in — so the
   screens and the GUI always tell you which group you are editing. Green is
   play, red is record, amber is a warning. Everything structural is graphite.
3. **One live visual per page.** The middle of a screen shows the thing the
   knobs change (sample waveform, envelope, filter response, saturation curve,
   echoes, reverb decay), drawn anti-aliased so turning a knob visibly moves it.
4. **A button always does what its label says.** Labels come from the engine
   (`Engine::soft_key_labels`), the lit state is derived from the label
   (`Engine::soft_key_active`) and drives the screens, the GUI mirror and the
   button LEDs. A button without a function shows nothing and its LED is off.
5. **Reachable at any size.** GUI panels collapse (rail `F1`, MK3 deck `F2`,
   workspace `F3`), rows wrap instead of clipping, and every region scrolls as
   a last resort.
6. **Live wins over the loop while recording.** During `REC` the Sound being
   played is monitored directly: its pattern events do not retrigger its
   (single, monophonic) synth voice, so held notes, slides and note effects stay
   audible — the Maschine MK3 / Maschine flow. Everything else (other Sounds,
   pad LEDs, screens) keeps mirroring the sequencer, and the GUI shows the same
   state as the hardware, never a different one.

## Colour tokens

| Token | Screens (RGB) | GUI (RGB) | Use |
|---|---|---|---|
| background | `0 0 0` (pixels off) | `#131417` | base |
| panel | `20 22 26` | `#1A1C20` | content blocks |
| raised | `32 35 41` | `#22252A` / `#2C3036` | selection rows, widgets |
| rule | `46 50 58` | `#343941` | hairlines, separators |
| text | `240 242 245` | `#EEF0F3` | primary |
| text 2 | `166 172 183` | `#A6ACB7` | secondary, parameter names |
| text 3 | `104 110 122` | `#6B717C` | disabled, hints |
| accent | focused group colour | focused group colour | lit buttons, selection, values |
| ink | black or white | black or white | text on the accent (by luminance) |

The group colour comes from the project palette (`Group::color`, the MK3 LED
palette). LED colours are saturated, so `screen_color` / `theme::ui_color`
lift dark hues (blue, violet) towards white and soften the others before they
are used on graphite.

## Typography

**Barlow Semi Condensed** (SIL Open Font License, `crates/maschine-engine/assets/fonts`)
for both surfaces: a condensed grotesk close to the DIN-like legends printed on
the MK3, so eight labels fit across 2 × 480 px and stay readable at arm's
length.

| Role | Face | Size (screens) | Size (GUI) |
|---|---|---|---|
| button legend | Medium, SemiBold when lit | 15 px | 14.5 px |
| context title | SemiBold | 18 px | 19–21 px |
| parameter name | Medium | 13 px | 12–13 px |
| parameter value | SemiBold | 21 px | 19 px (BPM, position) |
| body / lists | Medium | 14.5 px | 14.5 px |

Hack (monospace) remains available for column-aligned data only.

## Screen anatomy (each 480 × 272)

```
┌──────────────────────────────────────────────┐  y 0
│  BUTTON 1   │  BUTTON 2  │  BUTTON 3 │ BUTTON 4 │  legends, lit = accent pill
├──────────────────────────────────────────────┤  y 26
│ ▌Title                  context   ▮▮▮▮  ▌VU  │  context bar
├──────────────────────────────────────────────┤  y 53
│                                              │
│        content : list + one live visual      │
│                                              │
│ SECTION ─────────────────────────────────────│
│ NAME        NAME        NAME        NAME      │  knob slots
│ Value       Value       Value       Value     │
│ ━━━━━●───   ━━●──────   ━━━━━━●──   ━●─────── │  y 272
└──────────────────────────────────────────────┘
```

Drawing goes through three layers:

- `screen_text` — Barlow / Hack glyphs rasterised by `ab_glyph`, cached;
- `screen_vector` — anti-aliased paths, strokes, rounded rectangles and fades
  rasterised by `tiny-skia` into a coverage mask, then blended into the RGB565
  frame with 4×4 ordered dithering (no banding on 16-bit gradients);
- `display_render` — the screens themselves, built from shared helpers
  (`header_colored`, `finish_soft_keys`, `encoder_row`, `panel`, `pill`,
  `bar_h`, `draw_waveform`, `draw_adsr`, `draw_filter_curve`, …).

`finish_soft_keys` must stay the **last** drawing call of a screen: it draws
the legends at the bottom and rotates the frame so they end up on top.

## GUI layout

```
┌ header: MASCHINE · transport (▶ ● ⏮, position, BPM, swing) · LOOP/MÉTRO/SONG · master · panels ┐
├ rail ────┬ workspace (Ideas, Séquenceur, Clavier, Mixer, Sampler, Plugin, Ampli, Browser) ─────┤
│ views    │                                                                                   │
│ groups   ├ MK3 deck: both screens side by side with their buttons and knobs ── groups + pads ┤
│ A–H      │                                                                                   │
└──────────┴ status ────────────────────────────────────────────────────────────────────────────┘
```

The deck sits below the workspace like the controller sits below a computer
screen. It renders the very frames sent to the hardware; its labels are
clickable like the buttons, its knobs send `HardwareEvent::KnobDelta` through
the hardware mapper, its pads and group buttons use the mapper's LED state.

## Adding a screen or a page

- Add soft-key labels in the engine (`soft_key_labels`) and, if a label
  selects a state, its lit rule in `soft_key_active`.
- Add knob parameters to a table in `plugin_pages.rs` (instrument or effect):
  the same table feeds the knob slots and the knob routing.
- Build the screen from the shared helpers above; check it with
  `cargo run -p maschine-engine --example dump_screens -- <dir>`.
