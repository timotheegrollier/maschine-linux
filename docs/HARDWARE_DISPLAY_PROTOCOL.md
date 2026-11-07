# Native Instruments Maschine MK3 — Display Protocol & Dual-Screen Architecture

Technical reference document for integrating the color screens of the **Maschine MK3 (VID: 0x17cc, PID: 0x1600)** on Linux, 100% native in userspace.

---

## 1. Hardware overview

The Maschine MK3 features two identical color LCD screens:
- **Resolution:** 480 × 272 pixels per screen (16:9 aspect ratio).
- **Color format:** RGB565 (16 bits per pixel, 2 bytes).
  - Red: 5 bits (`0xF800` MSB).
  - Green: 6 bits (`0x07E0`).
  - Blue: 5 bits (`0x001F` LSB).
  - **Endianness on the USB bus:** Big-Endian (most significant byte first).
- **Power:** Runs on USB bus power (5V from the bus). The optional 15V power supply provides a brighter backlight but is not required.

---

## 2. USB architecture

The device exposes 7 USB interfaces:

| Interface | Name / Function | Transfer type | Endpoint |
| :--- | :--- | :--- | :--- |
| **0, 1, 2** | Audio streaming & control | Isochronous | `snd-usb-audio` |
| **3** | MIDI USB Class Compliant | Bulk IN/OUT | EP 0x02 |
| **4** | HID (Pads, buttons, encoders, LEDs) | Interrupt IN/OUT (64B) | EP 0x03 |
| **5** | **Maschine MK3 BD (Bulk Display)** | **Bulk OUT (wMaxPacketSize = 512)** | **EP 0x04** |
| **6** | DFU (Device Firmware Upgrade) | Control | N/A |

### Linux support
- Interface 5 is not claimed by any kernel driver by default, but may require `detach_kernel_driver(5)` or `set_auto_detach_kernel_driver(true)`.
- No root rights are required if a udev rule (`/etc/udev/rules.d/70-maschine.rules`) or systemd ACLs (`TAG+="uaccess"`) grant rights to the user group.

---

## 3. Display packet structure (Wire Protocol)

Each frame transmitted to the display is exactly **261 148 bytes**:

```
┌─────────────────┬─────────────────┬──────────────────────────┬─────────────────┬─────────────────┐
│ Window Header   │ Transmit Cmd    │ Pixel Data (RGB565 BE)   │ Blit Command    │ End Command     │
│ (16 bytes)      │ (4 bytes)       │ (261 120 bytes)          │ (4 bytes)       │ (4 bytes)       │
└─────────────────┴─────────────────┴──────────────────────────┴─────────────────┴─────────────────┘
```

### A. Window Header (Offset 0..15: 16 bytes)
Defines the target drawing area and the screen:

| Offset | Value | Description |
| :--- | :--- | :--- |
| **0x00** | `0x84` | Sync magic byte |
| **0x01** | `0x00` | Reserved |
| **0x02** | `0x00` or `0x01` | **Screen number** (`0x00` = Left Screen, `0x01` = Right Screen) |
| **0x03** | `0x60` | Mode magic byte |
| **0x04..0x07** | `0x00 0x00 0x00 0x00` | Reserved (zeros) |
| **0x08..0x09** | `0x00 0x00` | Start X coordinate (uint16 Big-Endian, default: `0`) |
| **0x0A..0x0B** | `0x00 0x00` | Start Y coordinate (uint16 Big-Endian, default: `0`) |
| **0x0C..0x0D** | `0x01 0xE0` | Width (uint16 Big-Endian = 480) |
| **0x0E..0x0F** | `0x01 0x10` | Height (uint16 Big-Endian = 272) |

### B. Transmit Command (Offset 16..19: 4 bytes)
Defines the number of pixel pairs to transmit:
- Formula: `pixel_pairs = (width * height) / 2 = 65 280 = 0x00FF00` for a full frame.
- Format: opcode `0x00`, then a 24-bit big-endian counter.
- Bytes on the wire for the full frame: `[0x00, 0x00, 0xFF, 0x00]`.

**Windows must have an even width.** RGB565 pixels are consumed two at a time by the counter. An odd window can shift the reading of `blit` and `end`, then block subsequent refreshes. The differential aligns each rectangle to a pair; the encoder rejects odd widths. The recovery probe is a valid 2×1 pixel window (32 bytes), never a 1×1 window.

### C. Pixel Data (Offset 20..261139: 261 120 bytes)
- `480 × 272 = 130 560` pixels.
- Each pixel is encoded in 2 bytes RGB565 Big-Endian.

### D. Footer / Validation Commands (Offset 261140..261147: 8 bytes)
- **Blit Command (4 bytes):** `[0x03, 0x00, 0x00, 0x00]` — triggers rendering of the buffer into video memory.
- **End Command (4 bytes):** `[0x40, 0x00, 0x00, 0x00]` — signals the end of transmission.

---

## 4. Cadence and recovery

1. The thread compares renders at **30 Hz maximum**, but only transmits modified regions and limits average Bulk throughput to **1.6 MB/s**.
2. **Monolithic vs Split:**
   - Sending a single 261 148-byte buffer via `libusb_bulk_transfer` / `write_bulk` is natively supported by the High-Speed USB controller (which automatically splits it into 512-byte USB packets at the hardware level).
   - Do not force arbitrary manual packet splitting in userspace code.

The thread sends a full-screen pair every 2 seconds to resynchronize the controller framebuffer. This cadence is a software resynchronization strategy; it does not claim to measure a firmware watchdog.

An `Ok` return from `write_bulk` only confirms that USB accepted the bytes. It does not confirm the visual rendering of the LCD; verify page changes, colors and parameter values directly on the controller.

---

## 5. Implemented DAW interface

The graphical interface rendered on both screens faithfully follows Maschine's visual conventions:

### Left Screen (Screen 0): Workflows, Pads & Groups
1. **Top Soft-Buttons (Buttons 1..4):**
   - `[1:ARRANGE]`, `[2:MIXER]`, `[3:SAMPLER]`, `[4:BROWSER]`.
2. **Project header & Groups:**
   - Current project title.
   - 8 group tabs A to H with color dot, selected group highlight, and Mute/Solo indicators.
3. **4×4 Matrix of the 16 Pads:**
   - Exact reproduction of the physical 16-pad grid:
     - Row 4 (top): Pads 13..16
     - Row 3: Pads 9..12
     - Row 2: Pads 5..8
     - Row 1 (bottom): Pads 1..4
   - Color dot assigned to the sound, pad number, and abbreviated label.
4. **Bottom Parameter Strip (Encoders 1..4):**
   - 4 slots with label, live value and mini-gauge:
     - Enc 1: `TEMPO` (BPM)
     - Enc 2: `SWING` (%)
     - Enc 3: `GRP VOL` (dB)
     - Enc 4: `GRP PAN` (Bipolar)

### Right Screen (Screen 1): Sequencer, Transport & VU-Meters
1. **Top Soft-Buttons (Buttons 5..8):**
   - `[5:PATTERN]`, `[6:EVENTS]`, `[7:MACRO]`, `[8:SETTINGS]`.
2. **Transport & Position:**
   - Dynamic status badge: `▶ PLAY` (Green), `■ STOP` (Gray), `● REC` (Red).
   - Large BBT counter: `BAR 001 : 02 : 120`.
   - Active voice counter and `4/4` time signature.
3. **16-step Mini Step-Sequencer:**
   - 4 instrument tracks (Kick, Snare, HiHat Closed, HiHat Open).
   - Dynamic playhead animated in real time according to playback position!
4. **Audio Monitoring & Dual Stereo VU-Meter:**
   - Master L and Master R channels with gradient level bars (Green -> Yellow -> Red).
5. **Bottom Parameter Strip (Encoders 5..8):**
   - Enc 5: `TUNE`
   - Enc 6: `CUTOFF` (Filter)
   - Enc 7: `RESO`
   - Enc 8: `MASTER` (Volume)
