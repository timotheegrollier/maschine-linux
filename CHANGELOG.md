# Changelog

Tous les changements notables.

## [0.2.1] - 2026-10-14

### Corrigé
- **Encodeur 4-D** : correction du bitmask HID report `0x01` byte 1 dans `maschine-proto` pour les 4 directions cardinales (Up = bit 2, Right = bit 3, Down = bit 4, Left = bit 5).
- **One-shot vs Boucle** : remplacement de l'action `PlayPauseToggle` accidentelle sur `EncoderPush` par une pré-écoute one-shot du son actif, évitant le déclenchement intempestif de la boucle transport.
- **Séquenceur pas-à-pas multi-mesures** : prise en charge complète des patterns de 1, 2, 4 et 8 mesures avec navigation fluide au contrôleur (touches `<` / `>`, flèches 4D gauche/droite, boutons d'écran 5..8) et dans la GUI.
- **Curseur playhead step sequencer** : calcul du curseur temporel rapporté à la longueur réelle du pattern (`modulo pattern_bars`) plutôt que restreint aux 2 premières mesures.
- **Pose de pas silencieuse** : en mode Step, l'appui sur un pad bascule la case sans émettre de son parasite ni perturber la pré-écoute.
- **Qualité du code & Clippy** : remplacement des constantes approximatives par `std::f32::consts::FRAC_1_SQRT_2`, élimination des casts redondants, correction des avertissements de documentation et des boucles d'indexation.

### Ajouté
- Tests unitaires automatisés pour les 4 directions de l'encodeur 4D (`decode_01_four_way_directions`).
- Tests unitaires pour la sélection et navigation de mesure (`bar_navigation_and_pattern_length`, `step_sequencer_multi_bar_toggle`, `four_way_encoder_and_bar_selection`).
- Documentation enrichie pour le workflow hardware/software et l'utilisation de l'application.

## [0.2.0] - 2026-10-14

### Ajouté
- **Générateur drum synth 808** : 16 instruments percussifs synthétisés en temps réel (Kick, Snare, Hi-Hats, Clap, Toms, Rimshot, Shaker, Cowbell, Crash, Perc, Synth Stab, Sub Bass).
- **Rendu double écran LCD MK3 (480×272)** : vues en direct pour Arranger, DAW / Step Grid, Sampler & Enveloppes ADSR, Mixer 8 groupes et Browser.
- **Modes de jeu contrôleur** : Pad Mode, Step Sequencer, Keyboard (gammes), Chords (accords diatoniques).
- **Gestion des scènes et patterns** : modes SCENE, PATTERN, SELECT, ERASE directement sur le matériel avec retour LED couleur.
- **GUI native egui** : panneau de transport, séquenceur complet 16 pistes, grille 4×4 pads, éditeur de son et paramètres de filtre.

### Ajouté

- Workspace Rust (`maschine-proto`, `maschine-hardware`, `maschine-engine`, `maschine-app`)
- `maschine-proto` : codec HID `0x01`/`0x02` + `0x80`/`0x81` + display bulk RGB565 RLE + palette indexée, tests complets
- `maschine-hardware` : découverte `hidraw` + thread poll 1ms + hystérésis pads + `HardwareMapper` (pads → MIDI NoteOn/Off, Groups, Transport, knobs CC, touchstrip), `BulkHandle` (feature `usb`)
- `maschine-engine` : modèle `Project` (Groups 16 Sounds, Patterns, Scenes, Sections), `Transport` (PPQ 960, BPM, swing, loop), `Sequencer` (quantize, nudge, humanize, slice), `Sampler` (zones, choke, ADSR, render stéréo), `Engine` (snapshot lock-free)
- `maschine-app` : CLI (`probe`, `hid-dump`, `daemon`, `project new/show/validate`, `led-test`, `gui` stub), daemon headless + heartbeat + autosave
- Configs : `99-maschine-mk3.rules` (udev `TAG+="uaccess"`), PipeWire `99-maschine-lowlatency.conf` (quantum 128), WirePlumber `51-maschine-mk3.conf/.lua` (pro-audio)
- Docs : `README.md`, `docs/architecture.md`, `docs/protocol.md`, `docs/workflow.md`, `docs/INSTALL.md`, `docs/USAGE.md`
- CI GitHub Actions (fmt, clippy, test, build), `rustfmt.toml`, `scripts/install.sh`
- Exemple `examples/demo.machine-linux.json`

### Testé

- MK3 `17cc:1600` sur Fedora 44, kernel 7.1.10, PipeWire 1.6.8
- `probe` détecte `hidraw3` + ALSA `card 2 MK3`
- `hid-dump` + `led-test` OK (write hidraw 0x80/0x81)
- `daemon` connecte, mappe pads/boutons, avance transport, render sampler
- `cargo test --workspace` 31 tests pass
