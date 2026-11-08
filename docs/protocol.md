# Maschine MK3 protocol — reference implemented in maschine-proto

Source of truth: `usbmon` captures + `Drachenkaetzchen/cabl` docs + `vascokk` gist + `autotel/maschine-mk3-linux`.

## USB descriptor (17cc:1600, bcdDevice 1.46)

```
IF0 Audio Control, IF1 Audio Streaming OUT 4ch S32LE 24-bit 44.1/48/88.2/96k, IF2 Audio IN 2ch,
IF3 MIDI Streaming (DIN), IF4 HID (EP 0x83 IN / 0x03 OUT), IF5 Vendor Bulk (EP 0x04 OUT, displays), IF6 DFU
```

`bmChannelConfig=0` → ALSA guesses Surround 4.0. Fix WirePlumber `pro-audio` (see `config/wireplumber/51-maschine-mk3.conf`).

HID report descriptor: 816 bytes, see `hexdump -C /sys/class/hidraw/hidraw3/device/report_descriptor`.

## HID IN (Device → Host) — IF4 EP 0x83

### Report 0x01 — buttons / encoders / touchstrip / pots (42 bytes)

```
byte 0 : 0x01
bytes 1-10 : button bitmasks (1=pressed), LSB first
  1: [enc push, pedal, 4-way dirs, Shift, Disp Btn8]
  2: Groups A-H
  3: Notes, Volume, Swing, Tempo, Note Repeat, Lock
  4: PadMode, Keyboard, Chords, Step, FixedVel, Scene, Pattern, Events
  5: Variation, Duplicate, Select, Solo, Mute, Pitch, Mod
  6: Perform, Restart, Erase, Tap, Follow, Play, Rec, Stop
  7: Macro, Settings, Right, Sampling, Mixer, Plugin
  8: Channel/MIDI, Arranger, Browser+Plugin, Left, File/Save, Auto
  9: Display Btn 1-7 + Main Knob Touch
 10: Knob Touch 8..1
byte 11 : main encoder low nibble 0-15
bytes 12-27 : knob 1..8 absolute 12-bit (0-1023, little endian split)
bytes 28-35 : touchstrip 2 fingers × 4 bytes (pos + pressure)
bytes 36-41 : Mic Gain / Headphone / Master absolute 10-bit
```

See `crates/maschine-proto/src/hid_input.rs` for the exact `ButtonId` mapping.

### Report 0x02 — pad pressure (64 bytes)

```
byte 0 : 0x02
then 16 slots × 3 bytes : [pad_index, dataA, dataB]
  pad_index : 0..15 (hardware order top-right → bottom-left, remapped to logical 0..15 pad 1..16)
  raw value = (dataA<<8 | dataB)  // 0x4000..0x4FFD = pressure, otherwise unused slot (high nibble != 0x4)
```

Recommended hysteresis: `pad_on = 0x4010`, `pad_off = 0x4005`, 4ms debounce, delta ≥3 LSB, velocity = peak over 3-5 frames.

## HID OUT (Host → Device) — IF4 EP 0x03

Two reports, with the ID included as the first byte (Linux `libusb` strips it, `hidapi` keeps it — handle both):

### Report 0x80 — button LEDs (61+1 bytes)

```
byte 0 : 0x80
bytes 1..61 : one entry per LED
  single color: 0=Off, 4=Dim, 5=VeryDim, 6=On, 7=Bright (4 effective levels)
  RGB (Groups, Browser, Nav): indexed palette (see `color.rs`)
```

Offsets: see `crates/maschine-proto/src/hid_output.rs::BUTTON_LED_OFFSETS`.

### Report 0x81 — touchstrip + pad LEDs (42+1 bytes)

```
byte 0 : 0x81
bytes 1..26 : touchstrip 25 RGB LEDs (left→right) + 1 spare
bytes 27..42 : pads 16 RGB (hardware order 13-16,9-12,5-8,1-4)
```

## Indexed color palette (pads, Groups, strip)

```
0 Off/Black, 5 DimRed, 6 Red, 7 Rose, 9 DimOrange, 10 Orange, 11 Skin,
13 DimAmber,14 Amber,21 DimYellow,22 Yellow,25 DimSpring,26 Spring,
27 DimTurquoise,29 DimGreen,30 Green,31 Turquoise,33 DimLime,34 Lime,
37 DimSky,38 Sky,42 Blue,45 DimBlue,50 Purple,53 DimPurple,
57 DimMagenta,58 Magenta,61 DimPink,62 Pink,76 DarkGrey,77 Grey,78 White
```

`Color::from_index` / `to_index` in `color.rs`. Brightness 0..3 for single color.

## Bulk Display — IF5 EP 0x04

- 2 screens 480×272, RGB565 big-endian, no alpha.
- Each window is sent in a Bulk OUT transfer on IF5/EP `0x04`.
- A transfer contains the 16-byte header, `transmit` (4 bytes), the raw RGB565 pixels, then `blit` and `end` (4 bytes each).

```
Window header (16B):
 0: 0x84, 1: 0x00, 2: display (0x00 left / 0x01 right), 3: 0x60
 4-7: 0x00×4, 8-9: X BE, 10-11: Y BE, 12-13: W BE, 14-15: H BE

Transmit (4B): 0x00 + pixel_pairs encoded as 24-bit big-endian
  pixel_pairs = (W × H) / 2
Pixels: W × H RGB565 BE values, no compression
Blit (4B): 0x03 0x00 0x00 0x00
End  (4B): 0x40 0x00 0x00 0x00
```

A complete frame has `W=480`, `H=272`, `pixel_pairs=65 280` and is 261 148 bytes. The 2×1 pixel recovery probe is 32 bytes.

**Constraint: `W` must be even.** The MCU counts RGB565 pixels in pairs. An odd-width window leaves the last pixel out of the count; the `blit`/`end` bytes are then read at the wrong offset and can block subsequent refreshes. The differential expands rectangles to the neighboring pixel to keep an even width. The encoder rejects invalid windows before sending.

`display_thread.rs` compares renders at 30 Hz, transmits modified regions (up to 6 merged rectangles if needed) and sends both screens full-screen every 2 seconds to resynchronize the framebuffers. Average USB throughput is limited to 1.6 MB/s. A successful `write_bulk` confirms that USB accepted the bytes, not that they are visually displayed: check both LCDs directly.

## Linux notes

- HID IF4 binds `usbhid` → `/dev/hidraw3` (`hid-generic`). No detach needed when using `hidraw`. For exclusive `nusb`: `USBDEVFS_DISCONNECT` + `udev` `TAG+="uaccess"`.
- Bulk IF5 has no driver → direct `nusb`/`rusb` claim, no conflict.
- Audio IF0-3 → automatic `snd-usb-audio`, no quirk.

## Tests

Vectors in `crates/maschine-proto/tests/vectors.rs` from real captures (pad press/release, Shift+Play button).

See also `crates/maschine-proto/tests/fuzz.rs` (proptest).

Display regressions: `cargo test -p maschine-proto` checks even windows, the single-pixel diff at the right edge, the pair counter and framebuffer reconstruction. `cargo test -p maschine-app` checks the 2×1 probe and knob routing after GUI navigation. These tests do not replace visual verification on the controller.

For a detailed reverse-engineering guide covering HID reports, pad remapping, LED states and complete button/shift functions, refer to [hardware_reverse_engineering_mk3.md](hardware_reverse_engineering_mk3.md).
