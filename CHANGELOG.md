# Changelog

Tous les changements notables.

## [0.3.0] - 2026-10-18

### Ajouté
- **Refonte complète des 2 écrans MK3 (480×272)** dans le style du logiciel officiel : design system unifié (bandeau titre, rangées d'encodeurs, barre de soft keys), 16 vues contextuelles (ARRANGE, PATTERN, STEP, SONG, SCENES/IDEAS, MIXER ×2, SAMPLER ×2, BROWSER ×2, EVENTS, MACRO, SETTINGS), valeurs live par Sound/Groupe et VU-mètres réels.
- **Soft keys contextuels unifiés** : `EngineEvent::SoftKey(u8)` + labels exposés dans l'instantané — le hardware, les écrans et la GUI exécutent et affichent exactement la même action (fini les boutons d'écran câblés en dur).
- **Waveform + enveloppe ADSR dessinées** sur l'écran SAMPLER à partir du sample du Sound actif (enveloppe min/max par colonne).
- **VU-mètres temps-réel** par Groupe (8) et Master avec peak hold et décroissance, calculés dans le callback audio et diffusés par l'instantané.
- **Instrumentation `--debug-latency`** : latence HID→Engine (dernier/max par fenêtre de 2 s) et statistiques audio (`callbacks`, `lock_misses`, temps max de callback).
- **Réveil immédiat de la GUI** (`RepaintSignal`) depuis le thread HID + repaint piloté par un compteur de révision (16 ms en lecture, 50 ms au repos) ; le thread d'écrans ne renvoie plus une frame que si l'état a bougé.
- **Workflow Song/Scènes étendu** : `DuplicateScene`, `DeleteScene` (ré-indexation des sections), `DuplicateSection`, `DeleteSection`, `ResizeSection` (longueur 1/2/4/8/…), exposés dans la vue IDEAS de la GUI.
- Test de fumée rendant toutes les vues d'écrans (garde-fou anti-panic) + tests des workflows section/scène.

### Modifié
- **Moteur temps-réel** : l'instantané partage le projet en `Arc` (copy-on-write via `Arc::make_mut`) — plus de clone du projet à chaque frame, lectures O(1) ; `ProjectHandle`/`ProjectWriteGuard` pour des écritures sûres.
- **GUI egui entièrement réécrite** : thème unifié, widgets dédiés (knobs glisser/réinitialiser, faders dB, VU-mètres, pads sensibles à la vélocité, cellules de pas), vue IDEAS (scènes × patterns + Song), vue MIXER 8 strips + master, SAMPLER (waveform, ADSR, filtre), BROWSER (presets 808, pré-écoute, chargement), rail matériel MK3 avec miroir des écrans et soft keys.
- `render_screens` partagé entre le thread d'affichage USB et le miroir GUI : les écrans physiques et la fenêtre affichent strictement la même paire de vues.

### Corrigé
- Transport : Play/Stop/REC (le stop n'est plus un no-op), redémarrage à zéro (`EngineEvent::Restart`).
- Encodeurs d'écran : les slots affichent désormais les paramètres réellement pilotés par les 8 knobs (TEMPO/SWING/VOL PAN par groupe, VOL/PAN/CUT/RESO/ADSR par Sound, volumes A–H en MIXER, navigation BROWSER…).
- Grille du séquenceur (mesures/pas), mapping des faders en dB, champs de la fenêtre de réglages audio (`ComboBox` non déprécié).

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
