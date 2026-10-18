# Changelog

Tous les changements notables.

## [0.6.0] - 2026-10-18

### Corrigé
- **Écrans MK3 figés (2e vague)** :
  - **Abandon définitif à l'ouverture** : si une autre instance (gui/daemon resté ouvert) ou une ré-énumération USB tenait IF5, `claim IF5: Resource busy` tuait le thread écran sans réessai → écrans jamais rafraîchis. Le thread a désormais un **cycle de vie** : ouverture avec réessais (3 s), libération, réouverture à chaud quand l'interface redevient libre (+ log clair « nouvel essai dans 3 s »).
  - **Paquets interrompus non complétés** : le protocole est un flux de commandes MCU (header fenêtre + `transmit` + pixels + `blit` + `end`). Une écriture partielle laisse le parseur en attente du *reste exact* des pixels → tout envoi suivant décalé (écran figé pour toujours). `send_frame` complète désormais tout paquet interrompu avec **ses propres octets restants** puis `blit`+`end` (renvoie `Ok` si le paquet aboutit) ; à défaut (timeout, position inconnue) une **purge de noirs** (`blit`+`end`) remet le parseur en attente. 2 tests unitaires (`packet_tail`, cas réel 10588/7680).
  - **Escalade** : si 2 ré-acquisitions successives ne stoppent pas les erreurs → réouverture complète (Drop = release + reset USB). Purge en timeout court (60 ms).
- **Tempête de logs à l'arrêt** : 18 M lignes POLLERR en une seconde (ALSA pendant le drop du stream cpal) → gestionnaire d'événements d'erreur limité en débit (1ʳᵉ erreur puis 1/1000).
- **Écrans MK3 figés (1re vague)** : chaque rafraîchissement envoyait un frame complet non compressé (2 × 261 148 B ≈ 522 Ko) ; à 30 FPS ≈ 15,7 Mo/s → saturation du port USB, `short write`/`Operation timed out`, endpoint bloqué (écran figé). Remplacé par :
  - **Diff incrémental par régions** (`FrameDiffer`, proto) : runs verticaux de rangées modifiées, bbox X par run, fusion au-delà de 6 rectangles, frame pleine si la surface sale dépasse 55 % ; header fenêtre `x/y/w/h` ; saut des frames identiques.
  - **Budget d'octets** 2,5 Mo/s (rafale 700 Ko) avec coalescing (diffs non commités si budget insuffisant) ; récupération USB (`clear_stall` + backoff + `reacquire` après 4 erreurs) ; invalidation → frame pleine après erreur.
  - `send_frame → Result<usize>` (timeout 1 s si >100 Ko, sinon 250 ms), `clear_stall()`, `reacquire()` (stubs mis à jour) ; `display_thread.rs` réécrit (diff, budget, backoff, réacquisition, stats 10 s, commits conditionnels).
  - Bug corrigé dans `col_range` (`x1 = x + 1` écrasait le max → `if x + 1 > x1`) ; 8 tests ajoutés dont la propriété `differ_always_reconstructs_exactly` et la régression `debug_synthetic_rows`.
- **Dispatch MIXER** : `render_mixer_right` était du code mort (l'écran droit MIXER affichait PATTERN) → branché pour `left_tab == 1 && right_tab == 0`.

### Modifié
- **Audio** : déjà livré en 0.5.0 (buffer 64 frames ≈ 1,3 ms, limiteur master + `KIT_HEADROOM`, kit au taux device, anti-clics) — inchangé.
- **Design des 2 écrans** : chrome commun (header 26 px avec ruban d'activité 16 pads + mini VU master, panneaux bordés, encodeurs avec encoche accent, soft keys remplies), labels denses « tracker » via `draw_text_clipped` (glyphes rognés 9 px), palette cohérente.
- `pad_activity: [f32; 16]` (décroissance ~170 ms, `PAD_ACTIVITY_DECAY = 6.0`) pour animer les écrans (jeu + séquenceur), bump de `revision` pendant la décroissance ; alimenté par les pads et `trigger_pattern_events`.
- **ARRANGE gauche** : bandeau transport à pastilles, chip « A S01 », lignes groupes avec pilules M/S et VU 142 px.
- **PATTERN droit** : vélocités par hauteur, lignes de temps, labels tracker, LED activité, playhead.
- **STEP droit** : bandeau M/PAS/VEL/LEN/NOTE, barres vélocité numérotées, curseur/playhead.
- **MIXER** : 4 tranches de console (VU vertical + échelle dB + LED clip, M/S, pan), section master (grand VU + dB + LOOP/METRO + barre), section foyer (groupe + sound + ADSR).
- **SAMPLER** : waveform dégradé par amplitude + repères, ADSR rempli + points de rupture.
- **EVENTS** : tableau à colonnes (SON/PAS/VEL/LEN/NOTE), barres de vélocité, noms de note, mise en évidence de l'événement joué.
- **SETTINGS** : 4 cartes (PROJET / TRANSPORT / AUDIO-JEU / FOCUS) à clés/valeurs alignées.
- **STEP** : titres différenciés « STEP / GRILLE » (gauche) et « STEP / PAS » (droite).

### Ajouté
- Tests : `render_is_deterministic`, `views_are_not_all_identical`, `pad_activity_decays_and_bumps_revision` (display_render) ; tests proto diff déjà présents (27 verts) ; workspace : 94 tests verts.
- `note_pad_activity` rendu public.

## [0.5.0] - 2026-10-18

### Corrigé
- **Craquements + latence audio** : le stream était ouvert en `BufferSize::Default` (≈2048 frames ≈ **46 ms** sur PipeWire) et un `try_lock` raté sur le moteur rendait un **buffer de silence entier**. Négociation désormais **48 kHz / buffer 256 frames (période 64 frames ≈ 1,3 ms)** avec replis progressifs (512 → 1024 → 2048 → défaut) et **attente du verrou moteur jusqu'à 2 ms** avant tout silence. Mesuré sous charge extrême (lecture + verrou moteur toutes les 1 ms + snapshots + écrans 30 FPS + 61 voix) : **0 miss, 0 err, 0 écrêtage**.
- **Groove quantifié au buffer** : les notes du séquenceur démarraient au **début du buffer audio** (jusqu'à 46 ms de décalage, swing/humanize détruits). Chaque voix démarre maintenant à sa **position exacte en échantillons** dans le bloc (`SamplerVoice::start_frame`), métronome inclus.
- **Écrêtage dur en sortie** (`clamp(-1,1)`) : kick + snare + clap montent à ≈1,55 et étaient **écrêtés en plateau** (la vraie source du « craque »). Remplacé par un **limiteur doux** (linéaire sous -3 dB, saturation tanh au-dessus) + **marge de kit ≈ -2,5 dB** : les frappes solo restent linéaires, seules les coïncidences sont lissées (≈0,6 dB).
- **Kit désaccordé de 1,5 demi-ton** : les presets DSP étaient générés à 48 kHz et joués à 44,1 kHz (+8,8 %). `Engine::set_audio_sample_rate()` (appelé à l'ouverture du stream) **régénère le kit au taux réel** et repointe les zones du sampler ; réalignement d'horloge sans allocation dans le callback.
- **Clacs de fin de sample** : toutes les queues (kick, crash, open hat…) étaient coupées net à amplitude non nulle (jusqu'à -22 dB). Fondus raised-cosine de fin + fade-in 0,4 ms dans le drum synth, fondu anti-clic 3 ms en fin de sample dans le sampler, clic métronome fondu.
- **Vol de voix abrupt** : la plus vieille voix était **retirée d'un coup** (clic) ; elle part maintenant en **fondu 5 ms**.
- Buffers > 1024 frames : des échantillons restaient **non écrits** (garbage). Rendu par tranches bornées, quel que soit le format (F32/I16/U16, 2 ou 4 canaux).

### Modifié
- **Drum synth 808 réécrit** : biquads RBJ (LP/HP/BP 12 dB/oct) au lieu des filtres 6 dB/oct, **oscillateurs bande-limités** (somme d'harmoniques : zéro repliement) pour hi-hats/crash, saturation `tanh`, **normalisation par instrument** (kit homogène), enveloppes retravaillées (kick sub + glissando + click filtré, caisse double partiel + bruit 1,4–7,5 kHz, clap 3 rafales + queue, toms avec attaque de baguette, cowbell, crash métallique 1,6 s).
- **Sampler** : plus aucune allocation dans `note_on` (sélection de zone sans `Vec`), rendu par tranches, `replace_zone_sample` pour le changement de taux.
- Négociation audio unifiée (4 canaux sur MK3 puis repli 2) ; `--debug-latency` journalise la taille de buffer **réelle** et les erreurs backend.
- Nouveau mode diagnostic `daemon --stress` (lecture + contention du verrou moteur simulée).

### Ajouté
- Tests : fin de sample silencieuse (anti-clic), absence de DC du kit, placement des notes **dans** le bloc (`notes_start_at_their_position_inside_the_block`), métronome piloté en blocs comme le callback réel.
- Exemples de contrôle qualité : `render_groove` (WAV + crête/RMS/écrêtés) et `render_kit` (16 instruments en WAV).

## [0.4.0] - 2026-10-18

### Ajouté
- **Mode STEP fidèle au logiciel officiel** : curseur de pas (`step_cursor`, index absolu dans le pattern), le pad touché devient le pas sélectionné, édition du pas par les knobs 1–4 (`VEL`, `POS` nudge en ticks, `LEN`, `PITCH` + nom de note), **SCROLL de la grille du pattern complet au knob 5** et à l'encodeur 4-D `<`/`>`, knobs 6–8 = `VOL SND`/`CUTOFF`/`RESO`.
- Nouveaux événements moteur `Swing`, `StepCursor`, `StepScroll`, `StepVelocityDelta`, `StepNudgeTicks`, `StepLengthDelta`, `StepPitchDelta` (+ actions mapper et tests).
- **Écran gauche STEP** : grille du pattern complet (une ligne par mesure), case du curseur encadrée, tête de lecture, détail live du pas sélectionné et slots 1–4. **Écran droit STEP** : contour orange du pas édité, `PAS n/n` dans le bandeau, slot 5 = `SCROLL`.
- Soft keys STEP : `PAS <</>>`, `BAR <</>>`, longueurs `1/2/4/8 MES`.
- Curseur d'édition visible sur les pads à l'arrêt (blanc) dans la couleur du son pour les pas programmés.
- Test `engine` de bout en bout : pose + édition + scroll du curseur ; tests mapper (knobs STEP, accumulateur, clamp des tours rapides, 4-D en STEP).

### Modifié
- **Vitesse des knobs recalibrée** (« les knobs tournent trop vite ») : mise à l'échelle faite **une seule fois** dans l'app (le mapper émet des ticks bruts), grains type logiciel officiel : ≈1.5 BPM/cran (TEMPO), 0.03 dB (volumes), 0.001 (pan/reso/master/sustain), **CUTOFF exponentiel** `2^(ticks/384)` (≈2.7 octaves/tour, ≈1 demi-ton/cran), ADSR 0.5–1 ms/cran, presets/sons du BROWSER par cran.
- Les tours rapides de knob (delta > 128 ticks) sont **limités au lieu d'être jetés** : plus de mouvements perdus.
- Éncodeur 4-D `<`/`>` en mode STEP : défile la grille d'un pas au lieu de changer de mesure.
- Le thread d'écrans réveille sa boucle toutes les 3 ms (drapeau d'activité sans verrou posé par la boucle HID) et envoie dès qu'un changement est prêt : réactivité de quelques millisecondes, verrou moteur pris au plus toutes les 33 ms (plafond USB ~30 FPS, garde-fou de 2 s) — **0 miss audio** conservé.

### Corrigé
- **Écrans qui « ne répondaient pas »** : `Swing` écrivait le projet en direct sans synchroniser le séquenceur ni invalider l'affichage ; `toggle_step`, `clear_bar`, humanize/quantize, reset du groove et DUPLICATE BAR modifiaient le projet sans bump de révision (les écrans restaient figés jusqu'à la garde de 5 s). Tout passe désormais par l'Engine (`EngineEvent::Swing`, `invalidate_display`) et rafraîchit l'affichage.
- `RESTART` passe par `EngineEvent::Restart` (remise à zéro + invalidation immédiate du BBT affiché).
- Le knob SWING agit maintenant réellement sur la lecture (le séquenceur utilisait l'ancienne valeur).

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
