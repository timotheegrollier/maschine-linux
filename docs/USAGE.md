# Usage

## CLI

```bash
maschine-linux --help
maschine-linux probe
maschine-linux -v probe
maschine-linux hid-dump --duration 5
maschine-linux hid-dump --hidraw /dev/hidraw3 --duration 5
maschine-linux led-test --pad 0 --color 6   # rouge
maschine-linux led-test --pad 15 --color 42 # bleu
maschine-linux project new --out demo.json
maschine-linux project show demo.json
maschine-linux project validate demo.json
maschine-linux daemon
maschine-linux daemon --project my.json
maschine-linux gui --project my.json   # --features gui
```

## Daemon

Le daemon :

1. Ouvre `/dev/hidraw3` (`hid-generic` via `usbhid`, pas de `libusb` detach)
2. Poll 1ms (`SCHED_OTHER` pour l'instant, `SCHED_FIFO` avec `rtkit` en option)
3. Décode `0x01` (boutons) et `0x02` (pads, hystérésis 0x4010/0x4005)
4. Mappe via `HardwareMapper` → `MidiEvent` + `LedState`
5. Envoie à `Engine` (Project/Transport/Sampler)
6. Avance le transport (PPQ 960) et déclenche les patterns de la scène active
7. Render le sampler (pour l'instant vers dummy, PipeWire-JACK arrivera)

Logs : `RUST_LOG=debug maschine-linux -vv daemon`

### Contrôles hardware supportés (MVP)

- **Pads 1..16** → Sound 1..16 du Group focus, velocity + LED feedback (couleur selon vélocité)
- **Groups A..H** → change `group_focus`, LED Groups
- **Play / Rec / Stop** → Transport
- **Knobs 1..8** → CC 14..21
- **Touchstrip** → CC 74 + barre LEDs
- **Shift / autres** → loggués, mapping à compléter

### Workflow projet

```bash
# créer
maschine-linux project new --out my.json
# édite my.json (ou future GUI)
# lance
maschine-linux daemon --project my.json
# joue → Scenes / Patterns définis dans le JSON sont séquencés si Transport playing
```

Voir `docs/workflow.md` et `examples/demo.machine-linux.json`.

## GUI (feature `gui`)

```bash
cargo run -p maschine-app --features gui -- gui
```

MVP actuel :

- Top bar : nom projet, BPM, état transport, position BBT, voix
- Left : Groups A.. + Scenes (click pour focus/activation)
- Center : Patterns du Group focus (liste events)

À venir : piano-roll, Ideas/Song grid, sampler, mixer, browser.

## Display bulk (écrans)

Nécessite ` --features usb` + `libusb1-devel` + `sudo dnf install libusb1-devel` :

```rust
let mut bulk = BulkHandle::open()?;
bulk.send_frame(&DisplayFrame::solid(DisplayId::Left, DisplayPixel::RED))?;
```

Le daemon n'envoie pas encore les écrans par défaut (prochaine phase). Test manuel :

```bash
cargo run -p maschine-app --features usb -- led-test --pad 0 --color 6
```

## Latence

Cible : <6ms round-trip @48k/128.

Mesure :

```bash
pw-top          # quantum 128, ERR 0
pw-metadata -n settings 0 clock.force-quantum 128
jack_iodelay    # si jack2 installé
rtcqs           # script linuxmusicians
```

Si xruns : passe à 256 dans `99-maschine-lowlatency.conf`.

## Debug

```bash
# hid brut
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
