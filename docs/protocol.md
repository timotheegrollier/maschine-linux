# Protocole Maschine MK3 — référence implémentée dans maschine-proto

Source de vérité : captures `usbmon` + docs `Drachenkaetzchen/cabl` + gist `vascokk` + `autotel/maschine-mk3-linux`.

## USB descriptor (17cc:1600, bcdDevice 1.46)

```
IF0 Audio Control, IF1 Audio Streaming OUT 4ch S32LE 24-bit 44.1/48/88.2/96k, IF2 Audio IN 2ch,
IF3 MIDI Streaming (DIN), IF4 HID (EP 0x83 IN / 0x03 OUT), IF5 Vendor Bulk (EP 0x04 OUT, displays), IF6 DFU
```

`bmChannelConfig=0` → ALSA devine Surround 4.0. Fix WirePlumber `pro-audio` (voir `config/wireplumber/51-maschine-mk3.conf`).

Report descriptor HID : 816 bytes, voir `hexdump -C /sys/class/hidraw/hidraw3/device/report_descriptor`.

## HID IN (Device → Host) — IF4 EP 0x83

### Report 0x01 — boutons / encodeurs / touchstrip / pots (42 bytes)

```
byte 0 : 0x01
bytes 1-10 : bitmasks boutons (1=pressed), LSB first
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

Voir `crates/maschine-proto/src/hid_input.rs` pour le mapping exact `ButtonId`.

### Report 0x02 — pads pression (64 bytes)

```
byte 0 : 0x02
ensuite 16 slots × 3 bytes : [pad_index, dataA, dataB]
  pad_index : 0..15 (ordre hardware top-right → bottom-left, remappé en 0..15 logique pad 1..16)
  valeur raw = (dataA<<8 | dataB)  // 0x4000..0x4FFD = pression, sinon slot inutilisé (high nibble != 0x4)
```

Hystérésis recommandée : `pad_on = 0x4010`, `pad_off = 0x4005`, debounce 4ms, delta ≥3 LSB, velocity = peak sur 3-5 frames.

## HID OUT (Host → Device) — IF4 EP 0x03

Deux reports, ID inclus comme premier byte (Linux `libusb` strip, `hidapi` le garde — gérer les deux) :

### Report 0x80 — LEDs boutons (61+1 bytes)

```
byte 0 : 0x80
bytes 1..61 : une entrée par LED
  unicouleur : 0=Off, 4=Dim, 5=VeryDim, 6=On, 7=Bright (4 niveaux effectifs)
  RGB (Groups, Browser, Nav) : palette indexée (voir `color.rs`)
```

Offsets : voir `crates/maschine-proto/src/hid_output.rs::BUTTON_LED_OFFSETS`.

### Report 0x81 — LEDs touchstrip + pads (42+1 bytes)

```
byte 0 : 0x81
bytes 1..26 : touchstrip 25 LEDs RGB (left→right) + 1 spare
bytes 27..42 : pads 16 RGB (ordre hardware 13-16,9-12,5-8,1-4)
```

## Palette couleurs indexée (pads, Groups, strip)

```
0 Off/Black, 5 DimRed, 6 Red, 7 Rose, 9 DimOrange, 10 Orange, 11 Skin,
13 DimAmber,14 Amber,21 DimYellow,22 Yellow,25 DimSpring,26 Spring,
27 DimTurquoise,29 DimGreen,30 Green,31 Turquoise,33 DimLime,34 Lime,
37 DimSky,38 Sky,42 Blue,45 DimBlue,50 Purple,53 DimPurple,
57 DimMagenta,58 Magenta,61 DimPink,62 Pink,76 DarkGrey,77 Grey,78 White
```

`Color::from_index` / `to_index` dans `color.rs`. Brightness 0..3 pour unicouleur.

## Bulk Display — IF5 EP 0x04

- 2 écrans 480×272, RGB565 big-endian, sans alpha.
- Chaque fenêtre part dans un transfert Bulk OUT sur IF5/EP `0x04`.
- Un transfert contient le header de 16 octets, `transmit` (4 octets), les
  pixels RGB565 bruts, puis `blit` et `end` (4 octets chacun).

```
Header fenêtre (16B):
 0: 0x84, 1: 0x00, 2: display (0x00 left / 0x01 right), 3: 0x60
 4-7: 0x00×4, 8-9: X BE, 10-11: Y BE, 12-13: W BE, 14-15: H BE

Transmit (4B): 0x00 + pixel_pairs encodé sur 24 bits big-endian
  pixel_pairs = (W × H) / 2
Pixels: W × H valeurs RGB565 BE, sans compression
Blit (4B): 0x03 0x00 0x00 0x00
End  (4B): 0x40 0x00 0x00 0x00
```

Une frame complète a `W=480`, `H=272`, `pixel_pairs=65 280` et fait
261 148 octets. La sonde de reprise 2×1 pixels fait 32 octets.

**Contrainte : `W` doit être pair.** Le MCU compte les pixels RGB565 par
paires. Une fenêtre de largeur impaire laisse le dernier pixel hors du compte;
les octets `blit`/`end` sont alors lus au mauvais offset et peuvent bloquer les
rafraîchissements suivants. Le différentiel élargit les rectangles au pixel
voisin pour conserver une largeur paire. L'encodeur refuse les fenêtres
invalides avant l'envoi.

`display_thread.rs` compare les rendus à 30 Hz, transmet les régions modifiées
(jusqu'à 6 rectangles fusionnés si besoin) et renvoie les deux écrans en plein
toutes les 2 secondes pour resynchroniser les framebuffers. Le débit USB moyen
est limité à 1,6 Mo/s. Un succès de `write_bulk` confirme l'acceptation des
octets par USB, pas leur affichage visuel : vérifier les deux LCD directement.

## Notes Linux

- HID IF4 bind `usbhid` → `/dev/hidraw3` (`hid-generic`). Pas besoin de detach si on utilise `hidraw`. Pour `nusb` exclusif : `USBDEVFS_DISCONNECT` + `udev` `TAG+="uaccess"`.
- Bulk IF5 n'a pas de driver → `nusb`/`rusb` claim direct, pas de conflit.
- Audio IF0-3 → `snd-usb-audio` automatique, pas de quirk.

## Tests

Vecteurs dans `crates/maschine-proto/tests/vectors.rs` issus de captures réelles (pads press/release, button Shift+Play).

Voir aussi `crates/maschine-proto/tests/fuzz.rs` (proptest).

Régressions écran : `cargo test -p maschine-proto` contrôle les fenêtres paires,
le diff d'un pixel au bord droit, le compteur de paires et la reconstruction du
framebuffer. `cargo test -p maschine-app` contrôle la sonde 2×1 et le routage des
knobs après une navigation GUI. Ces tests ne remplacent pas le contrôle visuel
sur le contrôleur.
