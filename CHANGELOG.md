# Changelog

All notable changes.

## [0.27.0] - 2026-11-23

### Fixed
- **GUI, position en haut** : affichée en mesure.temps.double-croche (comme Maschine) dans une case de largeur fixe — les ticks ne défilent plus à chaque image et l'en-tête ne saute plus.
- **Chaînage de patterns** : la boucle de lecture suit ce qui joue (chaîne, pattern le plus long de la scène, Song) au lieu de rester fixée à 2 mesures — une chaîne ou un pattern plus long n'est plus coupé. Deux pads éloignés ne chaînent que les patterns existants, et le lancement demandé par le premier pad est annulé par la chaîne.
- **REC sans flam** : une note jouée juste avant la grille n'est plus redéclenchée par le séquenceur quelques millisecondes plus tard (la voix live suffit au premier passage).
- **REC, longueur des notes** : calculée dans le pattern (modulo sa longueur) — une note tenue à travers le bouclage garde sa durée ; la fin quantifiée de la pré-écoute est un compte à rebours qui survit au bouclage.
- **REC, accords** : une fin de note par note (les notes d'un accord partageaient le même emplacement et restaient tenues).
- **REC dans une chaîne** : la note va dans le pattern en cours de lecture.
- **Boucle** : le premier temps (notes au tick 0, clic accentué du métronome, lancements « à la prochaine mesure ») n'est plus perdu ni doublé au tour de boucle ; le métronome clique aussi sur le premier temps au départ.

- **Chaînage de scènes** (deux pads en mode SCENE, bouton `ENCHAIN`) : la chaîne avance réellement avec la boucle active, chaque scène joue sur la longueur de son plus long pattern (au lieu de 2 mesures fixes), les pads vides de l'intervalle sont ignorés et le lancement du premier pad est annulé. Les pads des scènes chaînées sont allumés plus fort que les scènes libres (MK3 et GUI).

- **LEDs de chaîne de patterns** : trois niveaux nets en mode PATTERN — pattern existant atténué, patterns chaînés vifs, pattern en cours de lecture dans la chaîne en blanc (il se déplace avec la lecture).
- **REC, notes tenues** : tant qu'un pad / une touche est tenu, le séquenceur ne relâche plus la note à sa longueur provisoire (un sample tenu était coupé après une triple-croche) ; chaque note d'un accord reçoit sa propre longueur au relâchement (seule la dernière était mise à jour).

### Validation
- Tests : `chained_scenes_play_one_after_the_other_with_the_loop_on`, `chained_scene_pads_are_brighter`, `chained_patterns_play_one_after_the_other_with_the_loop_on`, `rec_note_played_ahead_of_the_grid_is_not_retriggered`, `rec_note_held_across_the_loop_keeps_its_length`, `rec_chord_schedules_one_note_off_per_note` (longueurs par note), `chained_pattern_pads_are_brighter` (pattern joué en blanc).

## [0.26.3] - 2026-11-23

### Fixed
- **Fiabilité USB / anti-freeze** : l'écriture des LEDs utilise un poll avec timeout, et l'arrêt des threads HID/LED/écrans ne bloque plus la boucle HID (plus de gel possible lors d'une déconnexion ou d'un write USB bloqué).
- **PipeWire** : plus de modification automatique de la source par défaut (`wpctl set-default`) à l'ouverture audio ; les commandes `pw-link` du routeur MK3 ont un timeout pour éviter un thread bloqué.
- **REC monitoring** : `clear_rec_press_marks` relâche proprement les fins de notes programmées ; un seul note-off quantifié est conservé par pad.

### Validation
- 309 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveaux tests : `command_output_timeout_returns_output`, `command_output_timeout_kills_slow_process`, `write_with_timeout_times_out_on_full_pipe`, `pending_note_off_replaces_same_pad`, `clear_rec_marks_releases_pending_note_offs`.

## [0.26.2] - 2026-11-22

### Added
- **Chaînage de patterns visible** : les pads appartenant à la boucle chaînée s'allument en couleur vive sur la MK3 (le masque de chaîne du groupe focus est transmis au mapper).

### Validation
- 304 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveaux tests : `chained_pattern_pads_are_brighter`, masque de chaîne dans `hardware_snapshot`.

## [0.26.1] - 2026-11-22

### Fixed
- **GUI** : quand aucun pattern n'est actif, plus d'avertissement — simplement aucune couleur de sélection sur le pattern.

### Validation
- 303 tests, `clippy -D warnings`, vérification sans sampler au vert.

## [0.26.0] - 2026-11-22

### Fixed
- **Pré-écoute REC quantifiée** : la voix live est coupée à la fin quantifiée de la note enregistrée (et non au relâchement physique) — l'écoute et le pattern coïncident exactement.
- **Chaînage de patterns** : deux pads maintenus ensemble OU pressés à moins de 800 ms chaînent tout l'intervalle, même si le premier pad a été relâché.
- **GUI sans pattern actif** : les vues Sequencer / Ideas n'affichent plus le pattern 0 comme actif quand aucun pattern n'est assigné ; les actions d'édition sont désactivées et un message l'indique.

### Validation
- 303 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveaux tests : `rec_live_note_off_is_quantized`, `pattern_chain_on_two_quick_pads_after_release`.

## [0.25.0] - 2026-11-22

### Changed
- **Monitoring live en REC (KEYBOARD)** : quand tu changes de note, la note précédente est coupée nette et la nouvelle attaque immédiatement — plus de glissando dans la voix de monitoring (le pattern, lui, garde ses slides enregistrés). Les longueurs des notes enregistrées sont quantisées à la grille pour coller à ce que tu entends ; le quantize de départ est inchangé.

### Validation
- 301 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveau test : `live_monitor_note_changes_instantly_without_portamento`.

## [0.24.0] - 2026-11-22

### Fixed
- **Pattern actif coupé par ré-appui** : dans le workflow MK3, presser le pad du pattern en cours le désassigne instantanément de la scène (le groupe se tait), annule le lancement en attente et la chaîne — au lieu de le garder sélectionné. Le pattern reste dans la banque du groupe.

### Validation
- 300 tests, `clippy -D warnings`, vérification sans sampler au vert.

## [0.23.0] - 2026-11-22

### Added
- **Bouton AUTO MK3** : allume sa LED, arme l'enregistrement des automations pendant REC et ouvre deux écrans dédiés sur la MK3 (liste des automations du pattern actif et courbe des points du knob focalisé), dans le style du design system partagé. L'enregistrement d'un point exige AUTO allumé.
- **Chaînage de patterns dans la GUI** : deux pads pressés à moins de 900 ms chaînent tout l'intervalle, comme sur la MK3.
- **Kick Synth** : la saturation suit l'enveloppe d'amplitude (plus de grésillement en fin de son) et deux nouveaux presets (House Soft Sub, Breakbeat Snap).

### Fixed
- **Updater in-app** : libellés et boutons aux couleurs du design system, fond sombre — plus de texte noir sur noir.

### Validation
- 300 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveaux tests : `auto_button_arms_automation_recording`, `auto_button_sends_toggle_action`.

## [0.22.0] - 2026-11-22

### Added
- **Workflow PATTERN MK3** : SHIFT + pad pattern bascule immédiatement pendant la lecture (plus d'attente de la fin de mesure ; le lancement en attente et la chaîne courante sont annulés). Deux pads pressés ensemble chaînent tout l'intervalle entre eux, du 1er au 16e pattern. La couche SHIFT générique (UNDO/REDO/…) reste active hors mode PATTERN.
- **Audio** : le routeur PipeWire ne s'active que si l'ampli guitare est réellement utilisé, évitant les blocages après quelques minutes avec la MK3.

### Validation
- 298 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveaux tests : `shift_pattern_switch_is_immediate_while_playing`, `shift_pattern_pad_requests_immediate_switch`, `pattern_chain_wide_interval`.

## [0.21.1] - 2026-11-22

### Fixed
- **Audio sur machine sans MK3 (Nobara, GUI seule)** : l'app ne touche plus aux sources audio toutes seules — `wpctl set-default`, l'entrée guitare et le routeur PipeWire (`pw-link`) ne démarrent que si une Maschine MK3 est réellement détectée (ou si l'ampli guitare est explicitement activé). Plus de coupure ni de changement de source audio après quelques secondes sans contrôleur.

### Validation
- 295 tests, `clippy -D warnings`, vérification sans sampler au vert.

## [0.21.0] - 2026-11-22

### Changed
- **Flow d'enregistrement Maschine** : pendant `REC`, le pattern rejoue immédiatement — la boucle enregistrée est audible sans attendre la désactivation de REC. Le Sound joué en direct dispose de sa propre voix de monitoring : les deux synthés monophoniques (TB-303, Kick, Wavetable) partagent le même gain, pan et bus d'effets sans jamais se couper. Synchro design/workflow avec la MK3 (`docs/design.md`, `docs/workflow.md`). La quantification automatique est inchangée.

### Validation
- 295 tests, `clippy -D warnings`, vérification sans sampler au vert. Test : `rec_pattern_does_not_cut_live_synth_voice` (pattern et live actifs en REC).

## [0.20.0] - 2026-11-22

### Changed
- **Flow d'enregistrement Maschine** : pendant `REC`, le Sound joué en direct reprend la voix sur le pattern — ses événements ne rejouent plus le synthé monophonique (TB-303, Kick, Wavetable). Les notes tenues, les slides et les effets restent audibles en pré-écoute temps réel ; les autres Sounds, les LEDs des pads et les écrans suivent toujours le séquenceur. Design et workflow documentés (`docs/design.md`, `docs/workflow.md`).

### Validation
- 295 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveau test : `rec_pattern_does_not_cut_live_synth_voice`.

## [0.19.4] - 2026-11-22

### Fixed
- **Pré-écoute en temps réel pendant REC en KEYBOARD / synthés** : la lecture du pattern ne vole plus la voix du Sound joué en direct — les notes tenues restent audibles (le TB-303 et les synthés monophoniques ne sont plus coupés par les notes du pattern), le glissando et les effets de note restent audibles pendant l'enregistrement.

### Validation
- 294 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveaux tests : `keyboard_rec_preview_notes_audible`, `keyboard_rec_long_note_is_not_cut_by_pattern_playback`.

## [0.19.3] - 2026-11-22

### Fixed
- **LED du groupe sélectionné** : le groupe sélectionné est plus illuminé que les autres — les autres groupes sont atténués même en lecture, pour distinguer facilement le groupe sélectionné en mode PAD.

### Validation
- 292 tests, `clippy -D warnings`, vérification sans sampler au vert.

## [0.19.2] - 2026-11-22

### Fixed
- **Illumination des pads pendant la lecture du pattern** : les pads des notes jouées dans le pattern s'illuminent en mode keyboard en fonction des notes jouées dans la boucle (via l'activité pendant la lecture) — comme sur la version officielle.

### Validation
- 292 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveau test : `pattern_playback_lights_pads_via_activity`.

## [0.19.1] - 2026-11-22

### Fixed
- **LEDs MK3** : le bouton MUTE ne s'allume que pendant le mode MUTE (ou MUTE+groupe) — plus l'indicateur « groupe muet » quand on navigue en mode PAD ; le groupe sélectionné est plus illuminé que les autres ; le pad sélectionné est plus illuminé que les autres.

### Validation
- 291 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveaux tests : `mute_button_and_muted_pads_leds_in_mute_mode`, `group_navigation_leds`, `selected_pad_is_brighter_than_others`, `rec_pad_sounds_during_hold_like_without_rec`, `rec_slide_sounds_during_rec`.

## [0.19.0] - 2026-11-22

### Added
- **Slides enregistrés pendant REC** — appuyer sur un pad (TB-303, synthés) pendant qu'un autre est tenu : l'événement enregistré précédent est étendu jusqu'au nouvel événement — le portamento n'est plus coupé directement.

### Fixed
- **Shift + Undo pendant REC** : les notes enregistrées pendant l'enregistrement poussent l'historique du projet (écriture du pattern en dehors du thread audio), donc Shift + Pad 1 annule les notes venant d'être enregistrées.

### Validation
- 286 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveaux tests : `rec_pad_slides_are_recorded`, `rec_notes_are_pushed_to_the_undo_history`.

## [0.18.1] - 2026-11-22

### Fixes

- **LEDs des pads et des groupes conformes à la MK3** : `bright_pad_color`
  utilisait un calcul `4k+3` qui produisait des indices de palette invalides
  pour la plupart des teintes (et assombrissait Spring). Une table explicite
  est utilisée désormais, avec passage au blanc quand la palette n'a pas de
  variante vive.
- **On voit quel instrument joue en mode PAD** : l'activité et les niveaux
  audio des pads sont transmis au mapper, donc un pad reste allumé en couleur
  vive pendant toute la note (pas seulement pendant le flash de 200 ms).
- **Boutons de groupe** : le groupe focalisé garde sa couleur assignée en
  pleine intensité ; un groupe en lecture s'allume vivement même s'il n'est pas
  focalisé ; les autres sont atténués, les groupes muets/vides éteints.

### Validation

- `cargo test --workspace --locked` (nouveaux tests de palette, de pad en jeu
  et de LED de groupe)
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

## [0.18.0] - 2026-11-22

### Added
- **Notes enregistrées pendant REC gardent la longueur de la note tenue** — appui long sur un pad (synth, TB-303, Nexus, …) ou touche clavier (Keyboard/Chords) : la note écrite dans le Pattern a la longueur de la note tenue à son relâchement, plutôt que la longueur par défaut ; la vélocité et la hauteur jouées sont enregistrées.
- **Automations de knobs enregistrées pendant REC** — pendant l'enregistrement, tourner un knob du Sound actif (onglet PLUGIN) ajoute un point d'automation dans le Pattern actif : nouveau modèle `SoundAutomation` (Sound, knob, points `(tick, valeur)`) persisté dans le projet.

### Validation
- 281 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveaux tests : `rec_pad_notes_keep_the_held_length`, `rec_keyboard_notes_keep_the_held_length`, `rec_plugin_knob_turns_record_automations` ; fix `needless_range_loop` dans le filtre Lo-Fi.

## [0.17.0] - 2026-11-22

### Added
- **16 vélocités** (`SHIFT + FIXED VEL`, comme sur la MK3) : les 16 pads jouent le Sound sélectionné, du plus doux (pad 1, vélocité 7) au plus fort (pad 16, 127), sans changer la sélection ; pads en dégradé de sa couleur, `FIXED VEL` à mi-intensité ; la GUI affiche la vélocité de chaque pad. Exclusif avec `FIXED VEL`.
- **Modes de l'arpégiateur** sur le knob 7 de KEYBOARD / CHORDS (`7:ARP`) : `HAUT`, `BAS`, `H-B` (aller-retour), et les mêmes sur deux octaves (`HAUT 2`…).
- **EVENTS : toutes les notes d'un coup** — le knob 6 (`SÉLECTION`) passe d'une note à toutes les notes parcourues (le Sound, ou tous avec `TOUS`) : position, hauteur, vélocité, longueur, `QUANT`, `NUDGE` et `SUPPR` s'appliquent à toutes, chacune du même écart ; toutes sont surlignées.

### Validation
- 277 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveaux tests : `sixteen_velocities_play_the_selected_sound_at_sixteen_levels`, `sixteen_velocities_keep_the_selection_and_shade_the_pads`, `arp_modes_go_down_bounce_and_add_the_octave`, `events_knob_six_selects_every_note_for_relative_edits` ; écran EVENTS « toutes les notes » ajouté à `dump_screens`.

## [0.16.0] - 2026-11-22

### Fixed
- **Enregistrement avec NOTE REPEAT** : chaque note répétée était écrite dans le pattern depuis le thread audio, ce qui recopiait tout le projet dans le callback (risque de craquements) ; les notes passent par une file, écrite hors du thread audio environ 60 fois par seconde.
- **Retouches du sample perdues** : `REVERSE`, `NORMLZ` et `LOOP` ne modifiaient que le son en mémoire et disparaissaient à la réouverture du projet ; elles sont gardées dans le projet et rejouées au chargement (le fichier d'origine n'est jamais modifié).
- **Écran PATTERN** : après un lancement à la mesure suivante, le pattern en cours ne changeait qu'à la prochaine action ; il suit maintenant tout seul.
- **FIXED VEL** s'applique aussi en KEYBOARD / CHORDS.
- **PLAY pendant le pré-compte** lançait l'enregistrement à la fin de la mesure ; il annule le pré-compte.
- **NOTE REPEAT** : le bouton 8 s'appelait `LOCK` mais coupait la répétition ; il s'appelle `OFF`.
- Noms par défaut en français (`Scène 2`, `Kick copie`) ; la GUI affiche le nom de la scène, comme la MK3.

### Added
- **Arpégiateur** : en KEYBOARD / CHORDS, `NOTE REPEAT` joue les notes tenues une à une, en montant et en boucle, au débit choisi (boutons 1–7) ; la première touche part tout de suite ; les notes sont enregistrées en REC.

### Changed
- Dépendances jamais utilisées retirées (tokio, rubato, anyhow, crossbeam-channel, parking_lot, thiserror selon les crates) : compilation plus légère.

### Validation
- 273 tests, `clippy -D warnings`, vérification sans sampler au vert. Nouveaux tests : `note_repeat_in_keyboard_mode_arpeggiates_held_notes_upwards`, `notes_repeated_by_the_audio_thread_are_written_outside_it`, `sample_edits_survive_save_and_reload`, `boot_progress_bar_spans_both_screens`, `mk3_keyboard_pads_play_the_notes_the_engine_computes` ; pré-compte annulé par PLAY.

## [0.15.0] - 2026-11-22

### Fixed
- **Tous les groupes jouent ensemble, synthés compris** : le TB-303, le Kick Synth et le Nexus n'existaient qu'en un exemplaire partagé par tout le projet — deux groupes avec un synthé se coupaient, prenaient les réglages l'un de l'autre et sortaient sur un seul groupe. Chaque Sound a désormais sa propre instance (rendu suspendu quand elle se tait), mixée avec le volume, le pan, les mutes et les effets de son groupe.
- **VU-mètres des groupes B à H** : seuls ceux du groupe A bougeaient (identifiant de Sound mal décodé) ; tous les groupes et tous les pads ont maintenant leurs niveaux, dans la GUI comme sur les écrans.
- **Notes perdues à l'enregistrement** : jouer en REC (ou poser des pas, éditer des notes) sur un groupe qui n'a pas de pattern dans la scène ne faisait rien ; un pattern est créé et placé dans la scène, comme sur le logiciel officiel.
- **SOLO** ne coupait rien à la lecture : il s'applique maintenant aux patterns, au jeu en direct (pads, KEYBOARD, CHORDS) et aux synthés, comme MUTE.
- **KEYBOARD / CHORDS** : les knobs réglaient le tempo et le swing au lieu de la fondamentale, de la gamme, de l'octave et de l'accord affichés au-dessus d'eux ; les pads jouaient deux octaves trop bas (un sample sonnait deux octaves sous sa hauteur) ; les LEDs suivaient un motif fixe sans rapport avec la gamme ; la GUI affichait des notes figées ; en CHORDS, les pads 8 à 14 rejouaient les mêmes accords. La MK3 et la GUI lisent maintenant les mêmes notes, calculées par le moteur.
- **SCENE** : les knobs font ce que l'écran affiche (TEMPO, SWING, VOL GRP, PAN GRP, PATTERN, LONGUEUR, VOL SND, MASTER).
- **Écrans PATTERN, SCENE et matrice** : le texte coupait le cadre des lignes et l'aide du bas passait sous les knobs.
- **Pads des Sounds vides** : ils gardaient une couleur par défaut (sur la MK3 comme dans la GUI) ; ils sont éteints, et blancs quand on les frappe ou les sélectionne.

### Added
- **Lancement quantifié** : en lecture, choisir une scène ou un pattern le fait démarrer au début de la mesure suivante (pad clignotant en attente, message à l'écran) ; à l'arrêt, le changement est immédiat.
- **LEDs des pads comme sur la MK3** : pad vide éteint, Sound présent atténué, Sound sélectionné normal, frappe ou note du séquenceur vive (selon la vélocité), Sound muet éteint (atténué en mode MUTE) ; patterns dans la couleur du groupe, scène active vive, éléments en attente clignotants ; boutons de groupe dans leur couleur (focalisé vif, muet éteint).
- **GUI** : boutons **M** (mute) et **S** (solo) sur chaque groupe du rail.
- **Démarrage** : la MK3 affiche l'étape de chargement avec une barre de progression dès le lancement ; la fenêtre apparaît en fondu.
- **Transport** : `STOP` à l'arrêt revient au début ; `SHIFT + REC` lance une mesure de pré-compte (un clic par temps, même métronome coupé) puis l'enregistrement — REC clignote pendant le pré-compte, `STOP` ou un nouveau `SHIFT + REC` l'annule ; le bouton d'écran `COUNT-IN` fait faire de même à `REC`. (Le réglage de pré-compte existait mais rien ne le lisait.)
- **KEYBOARD / CHORDS comme sur la MK3** : C3 en chromatique par défaut sur le pad 1, boutons 5–8 = `OCT -`, `OCT +`, `SEMI -`, `SEMI +` (la fondamentale passe de B3 à C4), fondamentale et octaves allumées et encadrées à l'écran, autres pads atténués dans la couleur du Sound. En gamme (majeure, mineure…), les accords restent dans la gamme (`1-3-5`, `1-3-5-7`, `1-3`, `1-5`, `1-4-5`, `1-4-7`) comme l'Harmonizer ; en chromatique, accords fixes (Triad, Minor, Sus2…). Noms de notes partout en convention Maschine (C3 = 60).
- **Lancements visibles** : le pattern ou la scène en attente est encadré et marqué `SUIVANT` sur les écrans MK3, et encadré dans la vue Ideas de la GUI.

### Validation
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` (268 tests) et `RUSTFLAGS=-D warnings cargo check -p maschine-engine --no-default-features` au vert ; écrans KEYBOARD, CHORDS, lancements en attente et démarrage ajoutés à `dump_screens`.
- Nouveaux tests : groupes simultanés avec leurs synthés et leurs VU (`every_group_plays_at_once_with_its_own_synths_and_meters`), lancement à la mesure suivante, SOLO / MUTE, enregistrement sur un groupe sans pattern (`launches_wait_for_the_next_bar_and_solo_mute_and_record_work_per_group`), intensités des LEDs des pads, notes et accords des pads (`pads_start_on_c3_and_light_the_roots_like_the_mk3`, `chords_climb_with_the_pads_and_stay_in_the_scale`), knobs et boutons de KEYBOARD / CHORDS et de SCENE.

## [0.14.0] - 2026-11-22

### Added
- **Éditeur de sample (SAMPLING)** comme sur la MK3 : boutons 1–4 = **RECORD**, **EDIT**, **SLICE**, **ZONE**.
  - RECORD : enregistre l'entrée audio de la MK3 sur le pad actif (`ENREG` / `STOP`, déclenchement au seuil `SEUIL`, durée max réglable) ; la prise est écrite en WAV dans `~/.local/share/maschine-linux/recordings/` et chargée comme un sample importé.
  - EDIT : plage de lecture non destructive (START / END, enregistrée dans le projet et respectée par le sampler), accord en demi-tons, gain, enveloppe ; `REVERSE`, `LOOP`, `NORMLZ`, `PLAGE MAX`.
  - SLICE : découpe en 4 / 8 / 16 tranches sur une grille ou aux coups détectés (sensibilité réglable), `APPLIQUER` répartit les tranches sur les pads.
  - ZONE : note racine, tessiture, vélocités, accord fin, pan et gain de la zone, avec un clavier de 128 notes à l'écran.
- **Mode EVENTS** : édition des notes du pattern une par une sur les deux écrans (rouleau de notes à gauche, liste à droite) — knobs POSITION, NOTE, VÉLOCITÉ, LONGUEUR, 4D pour passer d'une note à l'autre, `TOUS`, `SUPPR`, `DUPLIQ`, `QUANT`, `NUDGE < >`.
- **GUI** : bouton `EVENTS` dans le pont MK3 ; bandeau « Éditeur de sample » en tête de la vue Sampler (les huit boutons et les valeurs des knobs).
- `maschine-linux standby --install` : installe le service de veille pour l'AppImage ou le tarball et indique la règle udev à ajouter.
- **Documentation pour reprendre le projet** : `AGENTS.md` (carte du dépôt, invariants, où modifier quoi, pièges, publication), `CLAUDE.md`, `docs/STATUS.md` (fait / partiel / manquant, vérifications matérielles, suites).

### Changed
- **Mixer des pads** : rangée de knobs au bas des écrans (volume, pan avec Shift) aux couleurs des pads ; en-têtes allégés.
- **Browser** et **STEP** : carte de preset plus lisible, aides reformulées sans abréviations.
- Les modes de filtre du sampler (LP/HP/BP, NOTCH, PEAK, OFF, CYC FLT) restent sur la page SON du PLUG-IN, les boutons SAMPLING étant pris par l'éditeur.

### Fixed
- **Bas de la fenêtre caché par la barre des tâches** (Fedora KDE, écrans à mise à l'échelle) : la fenêtre demandait 1560 × 960 points, plus haut que l'écran utile d'un portable 1080p à 125 %. Elle s'ouvre désormais maximisée dans la zone utile (taille de retour 1200 × 720).

### Validation
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` (260 tests) et `cargo check -p maschine-engine --no-default-features` au vert.
- Nouveaux tests : éditeur de sample (pages, plage, racine, enregistrement → WAV → pad, découpe en 4 tranches contiguës), plage de lecture du sampler, détection des coups, mode EVENTS (moteur et routage du mapper).
- Rendu de chaque page de l'éditeur et du mode EVENTS via `dump_screens`.

## [0.13.1] - 2026-11-22

### Added
- **Trois nouveaux effets** dans la chaîne de plugins : **EQ** 3 bandes (plateau grave 120 Hz, cloche médium réglable, plateau aigu 8 kHz, ±18 dB ; `FLAT`, `LOUDNESS`, `RADIO`), **Chorus** (deux lignes modulées en quadrature ; `CHORUS`, `FLANGER`, `ENSEMBLE`) et **Lo-Fi** (bits, fréquence d'échantillonnage, tonalité ; `8-BIT`, `SP-1200`, `CRUSH`). L'emplacement `+ effet` propose désormais les huit effets, un par bouton d'écran, dans l'ordre de la chaîne ; chacun a son visuel (réponse de l'EQ, modulation du chorus, marches du Lo-Fi).
- **Écran de veille dès le branchement** : `maschine-linux standby` dessine l'écran de veille ; les paquets RPM, DEB et Arch installent une règle udev et le service utilisateur `maschine-linux-standby.service` qui le lancent au branchement de la MK3 — la veille personnalisée apparaît même si l'application n'a jamais été démarrée (sans effet si l'application tient déjà les écrans).
- **Chaîne d'effets dans la GUI** : bandeau « Chaîne » en tête de la vue PLUGIN, cliquable comme l'écran gauche de la MK3 ; sur `+ effet` les huit effets, sur un effet `< TYPE`, `TYPE >`, `BYPASS`, `SUPPR`, ses raccourcis et ses valeurs — les mêmes actions que les boutons d'écran.

### Changed
- **Grille PATTERN** (écran droit par défaut) redessinée : cases de pas à coins adoucis, temps forts plus clairs, notes dans la couleur du Sound selon la vélocité, tête de lecture en colonne voilée de la couleur du groupe.
- **Contours arrondis et lissés** sur tous les écrans MK3 (cartes, cases, sélection).
- **Fenêtre basse** : sous 620 px de haut, le pont MK3 se replie de lui-même pour laisser la vue de travail lisible (`F2` le rouvre) et revient au-delà de 700 px.
- Libellé « Master » séparé de son curseur dans l'en-tête : il passe à la ligne au lieu d'être coupé.

### Validation
- `cargo fmt`, `cargo clippy -D warnings` et `cargo test --workspace` au vert ; nouveaux tests EQ (à plat transparent, plateau grave), Lo-Fi (marches à 3 bits, transparent à 16 bits) et chorus, bornes et absence de NaN pour les huit effets, tables de paramètres des nouveaux effets.
- Rendu de chaque effet et de la grille PATTERN via `dump_screens` ; captures GUI de la vue PLUGIN (1600×900, 1366×600 avec pont replié).

## [0.13.0] - 2026-11-22

### Added
- **Tous les paramètres des plugins sur la MK3** : pages Sampler (SON, VOIX), TB-303 (MAIN, OSC & SORTIE), Kick Synth (CORPS, TIMBRE, DÉTAILS) et Nexus (FILTRE & AMP, OSCILLATEURS, UNISON & VOIX, ENV FILTRE & LFO, EFFETS) sur les deux écrans : huit paramètres par page, nom et valeur au-dessus de chaque knob, boutons 5–8 propres à la page. Les boutons `Page ◄ ►` (et le 4D gauche/droite) changent de page, leurs LEDs s'allument quand une page existe, un bandeau de transition affiche la nouvelle page. Une seule table par plugin pilote l'affichage et les knobs : ce qui est affiché est ce qui est réglé.
- **Chaîne de plugins façon Maschine** : en PLUG-IN, l'encodeur 4D parcourt l'instrument puis les effets du Sound, du groupe et du Master ; un emplacement `+ effet` en fin de chaque chaîne insère **Filter, Distortion, Compressor, Delay ou Reverb** depuis les boutons. Sur un effet : `< TYPE`, `TYPE >`, `BYPASS`, `SUPPR` et les raccourcis de l'effet (`LP/HP/BP`, `SOFT/HARD`, `DOUX/PUNCH/SQUASH`, `PING-PONG` et temps calés sur le tempo, `PIÈCE/SALLE/HALL`) ; knob 1 toujours MIX. L'écran gauche liste la chaîne (effets contournés barrés), l'écran droit dessine l'effet (réponse du filtre, courbe de saturation et de compression, échos, décroissance). Dans la GUI : `◀ SLOT` / `SLOT ▶`.
- **Effets réellement appliqués** : chaque Sound à effets joue dans son bus, traité puis envoyé au bus de son groupe, puis au Master. Auparavant les effets d'un Sound n'étaient jamais appliqués et ceux du groupe focalisé l'étaient au mix entier. Effets du Master enregistrés dans le projet.
- **Limiteur Master transparent** (plafond -1 dBFS, anticipation 1 ms) à la place du waveshaper qui saturait tout signal au-dessus de -3 dBFS.
- **Écran de veille** sobre (presque noir, mot-symbole discret) à la fermeture et en cas de plantage (hook de panique).
- **Système de design commun** écrans + GUI ([docs/design.md](docs/design.md)) : police Barlow Semi Condensed (OFL) embarquée, couleur d'accent = couleur du groupe focalisé, formes lissées par `tiny-skia` avec tramage RGB565.

### Changed
- **Écrans MK3 redessinés** : noir des pixels éteints, libellés de boutons en pastille couleur du groupe quand ils sont allumés, valeurs en grand, barres de valeur arrondies, courbes (enveloppes, filtres, formes d'onde) lissées avec fondu, texte proportionnel partout.
- **GUI redessinée** façon logiciel Maschine : un seul en-tête (transport, position et tempo en grand, modes, master, panneaux), rail des vues avec les huit groupes en couleur, **pont MK3 en bas** (les deux écrans côte à côte avec leurs boutons et knobs, groupes et pads à droite), redimensionnable et repliable. Rien n'est coupé de 420 × 340 au plein écran : les rangées passent à la ligne, les panneaux se replient, tout défile.
- **Effets réécrits** : filtre SVF topologie TPT à coupure lissée, distorsion anti-repliement (ADAA) à gain compensé avec tonalité et sortie, compresseur à genou doux et compensation automatique, delay stéréo/ping-pong jusqu'à 2 s à temps lissé et réinjection filtrée, réverbe Freeverb stéréo avec pré-délai et largeur ; un état DSP par slot, alloué hors du thread audio.
- **Sampler** : interpolation Hermite 4 points (transpositions sans grain métallique). **Nexus** : scie et carré anti-repliés (PolyBLEP).
- **Pads** : un pad frappé ou tenu s'allume dans la couleur pleine de son Sound (plus de flash blanc).

### Fixed
- **Ampli guitare qui saturait et sonnait mal** : le suréchantillonneur polyphase lisait une entrée sur quatre (filtre d'interpolation faux, -12 dB) ; corrigé. GAIN en dB (-6 à +36 dB) avec compensation de niveau tenant compte de la saturation, étages moins chauds, plus de `tanh` final, baffle en Butterworth 24 dB/oct. Du clair au saturé, le niveau reste dans 12 dB et ne clippe jamais.
- **Notes tenues coupées** : un pad tenu fermement sans nouveau rapport HID n'est plus relâché après le délai de silence (repli à 60 s) ; relâcher un pad ne coupe plus le synthé joué par un autre pad de même hauteur ; entrer dans un menu ne fait plus oublier un pad tenu ; le clavier GUI et les pads GUI en mode KEYBOARD tiennent la note tant que le bouton est enfoncé.
- En-tête GUI qui se chevauchait ou prenait toute la hauteur sur fenêtre étroite ; pont MK3 qui empilait des écrans minuscules.

### Validation
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` (253 tests) et `cargo check -p maschine-engine --no-default-features` au vert.
- Nouveaux tests : chaîne de plugins (`plugin_chain_inserts_edits_and_bypasses_effects_from_soft_keys`), inserts Sound/Groupe/Master audibles (`sound_group_and_master_inserts_process_the_audio`), sept tests d'effets, limiteur, Hermite, suréchantillonneur et niveaux de l'ampli, tenue d'un pad sans rapports, accent suivant le groupe, formes lissées.
- Rendu de tous les écrans (dont chaque effet, l'emplacement `+ effet`, un effet contourné et la veille) via `dump_screens` ; captures de la GUI sous Xvfb en 1920×1080, 1366×768, 1024×640 et 800×600.

## [0.12.0] - 2026-11-22

### Added
- **Miroir MK3 dans l'ordre du contrôleur** : bande des boutons d'écran (cliquable directement sur l'image des écrans), les deux écrans, les 8 knobs centrés sous le slot qui affiche leur nom et leur valeur, puis les groupes A–H et les pads 4×4. Écrans côte à côte dès que la place le permet, empilés sinon, jamais réduits sous 75 % ; boutons `ÉCRANS` et `PADS` pour replier une moitié sur petit écran.
- **Knobs GUI = knobs MK3** : un glisser sur un knob de la GUI envoie `HardwareEvent::KnobDelta` à la boucle matérielle, qui applique exactement le mapping et les vitesses du knob physique, avec ou sans contrôleur branché.
- **Pads et groupes aux couleurs des LEDs** : la GUI peint ses pads et ses boutons de groupe avec l'état LED calculé pour la MK3 (repos, frappe, pas programmés, modes PATTERN/SCENE/STEP/KEYBOARD…).
- **MK3 plein écran** : avec le centre masqué (`F3`), le miroir occupe toute la fenêtre, écrans côte à côte.

### Changed
- **Disposition officielle des écrans MK3** : les libellés des boutons 1–8 sont en haut de chaque écran, juste sous les boutons physiques ; le nom et la valeur de chaque knob restent en bas, juste au-dessus du knob.
- **Boutons allumés selon leur libellé** : un bouton s'allume quand l'état qu'il sélectionne est courant (mesure, longueur, groupe, banque du mixer, mode de filtre, métronome, SONG, cadence Note Repeat…). Le même état pilote les écrans, le miroir GUI et les LEDs des boutons d'écran (éteinte sans fonction, demi-teinte si disponible, pleine si allumé).
- **Panneaux qui suivent la fenêtre** : le rail des vues et le miroir MK3 gardent leur part de la largeur à chaque redimensionnement (la poignée change cette part) ; le rail se masque sous 1400 px et revient en grandissant.
- **Thème graphite façon logiciel Maschine** : gris neutres et plats, un seul orange d'accent partagé avec les écrans MK3, boutons rectangulaires, bascules de panneaux allumées en blanc comme les boutons du contrôleur.
- **Transport compact** sur une seule ligne (statistiques voix/audio visibles quand la fenêtre est large).

### Fixed
- Les boutons 5–8 de l'écran droit TB-303 affichaient les libellés des boutons 2–5.
- `MASTER` s'allumait dans le mixer des pads quand le pad 4 était sélectionné ; `CYC FLT`, `ECOUTE` ou `SND <` s'allumaient selon le son actif dans SAMPLER ; les surbrillances de la couche Shift ne correspondaient pas aux libellés.
- Titres et contextes d'en-tête qui se chevauchaient (mixer des pads), noms de Sounds illisibles dans la grille PATTERN, modèle d'ampli décentré, entrée guitare tronquée, ligne d'aide du browser qui débordait sur les knobs.
- Les knobs de la GUI ne pilotaient pas les mêmes paramètres que la MK3 (ARRANGE, MIXER, IDEAS, STEP, BROWSER, pages synthé).
- Cartes de la GUI aux libellés repliés lettre par lettre (IDEAS), cartes coupées sous le miroir (PLUGIN, SAMPLER) et grille du séquenceur qui débordait en fenêtre étroite.

### Validation
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` et `cargo test --workspace --locked` au vert.
- Nouveaux tests : `button_labels_on_top_and_knob_slots_at_bottom`, `lit_soft_keys_follow_their_labels`, `gui_knob_delta_routes_like_the_physical_knob`, LEDs des boutons d'écran dans `file_save_and_browser_distinction_and_leds`.
- Captures sous Xvfb de la GUI en 1280×720, 1366×768, 1600×900, 1920×1080 et 2560×1440, et rendu des écrans via `dump_screens`.

## [0.11.8] - 2026-11-22

### Fixes

- **Guitar amp latency**: the capture stream now negotiates a small explicit
  buffer (128 frames first, then the output buffer) instead of
  `BufferSize::Default`, and the SPSC ring drops any backlog beyond one output
  block. This removes the extra 20–40 ms introduced by large PipeWire buffers.
- **Guitar amp gain staging**: fixed stage gains were multiplied by the user
  GAIN into a ~200x chain, so every note was square-wave distorted. Stage gains
  are now modest, GAIN maps to 1–5x, power-amp sag no longer halves the level,
  and a fixed output trim with a soft limiter makes the guitar audible without
  hard clipping.
- Defaults retuned: GAIN 30 %, MASTER 90 %, MONITOR 100 %.

### Validation

- `cargo test --workspace --locked` (new clean/audible gain and ring-latency tests)
- `cargo clippy --workspace --all-targets --locked -- -D warnings`


## [0.11.7] - 2026-11-22

### Added
- **Vélocité fixe réglable et visible sur la MK3** : le bouton `FIXED VEL` applique un niveau fixe (100 par défaut) à tous les pads joués, aux pas posés en mode STEP et aux notes enregistrées en REC. Le niveau se règle directement au 4D (1..127, pas de 10 avec `SHIFT`) et reste affiché dans le bandeau des écrans sous la forme `VEL n FIXE`.

### Changed
- **Projet démo réorganisé par familles d'instruments** : Groupe A « Kick » (kick seul sur le pad 1), Groupe B « Snare & Cymbals », Groupe C « Synth » (Acid 303 et Wavetable), Groupe D « Bass », puis Percussion, Keys, Vocal et Hits. Chaque groupe possède son propre pattern (« Kick », « Snare & Cymbals », « Synth ») assigné à la Scène 1.
- **Reset du groove démo** : restaure désormais le pattern initial de tous les groupes, pas seulement du Groupe A.
- **Nouveau projet** : `FILE → NOUVEAU` repart sur un kit 808 complet et vierge dans le Groupe A.

### Fixed
- La cible mémorisée du synthé Acid est validée avant réutilisation, afin d'éviter de piloter un pad obsolète après la réorganisation des groupes.

### Validation
- `cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings` et `cargo test --workspace` au vert.


## [0.11.6] - 2026-11-22

### Fixed
- **Démarrage sans MK3 & ampli guitare** : L'ampli guitare et le monitoring sont désormais inactifs par défaut au lancement (`enabled: false`, `monitor: false`). Cela empêche l'activation involontaire du microphone intégré de la machine ou de bruits parasites dans les enceintes lorsque le contrôleur Maschine MK3 n'est pas connecté. L'utilisateur peut activer l'ampli et l'écoute à la demande via les boutons `ACTIF` et `MONITOR` (ou Soft Keys 5 et 6).

### Added / Improved
- **Isolation de piste pad lors de la création de pattern (`AddPattern` / `CreatePatternAt`)** :
  - Lors de la création d'un nouveau pattern (par exemple sur le Pad 1 Kick du Groupe A, via le bouton `+ NOUVEAU` ou un pad en mode PATTERN), seule la piste du pad sélectionné (`active_sound`) est initialisée vide pour composer une nouvelle variation.
  - Toutes les autres pistes et événements des autres membres du groupe (Snare, Hi-hats, percussions, synthés...) sont automatiquement conservés depuis le pattern précédent actif.
  - Infobulle explicative ajoutée sur le bouton `+ NOUVEAU` dans la vue Séquenceur.

### Validation
- Suite de tests unitaire complète avec test d'isolation de piste pad (`pattern_creation_isolates_active_pad_and_preserves_other_tracks`)
- Validation `cargo fmt`, `cargo clippy -D warnings` et `cargo test --workspace` à 100%

## [0.11.5] - 2026-11-22

### Added
- **Workflow officiel Scènes & Patterns sur Maschine MK3** :
  - **Écran droit dédié en mode SCENE (`render_scenes_right`)** : Matrice complète des 8 Groupes (A..H) affichant pour la scène active le pattern assigné, son nom, sa longueur en mesures, le nombre d'événements/notes et le groupe focus.
  - **Séparation et duplication de patterns indépendants (`DuplicateSceneUnique` & `MakeSceneUnique`)** : Permet de dupliquer une scène ou de rendre ses patterns uniques, créant des copies indépendantes dans chaque groupe pour pouvoir éditer et créer des variations sans écraser la scène d'origine.
  - **Création rapide de scènes par les pads (`CreateSceneAt`)** : En mode SCENE, appuyer sur un pad vide (1..16) crée immédiatement la scène à cet emplacement et la sélectionne.
  - **Enchaînement de scènes multi-pads (`SceneChain`)** : Maintenir deux pads simultanément en mode SCENE enchaîne automatiquement la plage de scènes dans la timeline Song / Arranger et lance la lecture séquencée.
  - **Contrôles hardware complets en mode SCENE** :
    - Bouton `DUPLICATE` du MK3 duplique la scène active (et `SHIFT + DUPLICATE` duplique avec patterns uniques).
    - Encodeur 4D et flèches directionnelles (`Left` / `Right` / `Up` / `Down`) naviguent pas-à-pas entre les scènes (`SceneStep`).
    - Potentiomètre d'écran 5 ajuste et cycle en direct le pattern du groupe focus dans la scène active.
  - **Touches Soft Keys 1..8 contextuelles pour les scènes** :
    - Mode normal : `SCN <`, `SCN >`, `+ SCN` (créer), `DUPL` (dupliquer), `UNIQUE` (séparer les patterns), `SUPPR` (supprimer), `ENCHAIN` (ajouter au Song), `SONG` (basculer Arranger/Ideas).
    - Mode Shift : `SCN 1`, `SCN DER`, `TOUT UNI`, `DUPL UNI`, `VIDER`, `SUPPR`, `ENCH TOUT`, `SONG`.
  - **Synchronisation GUI Ideas** : Nouveaux raccourcis interactifs dans l'interface graphique ("DUPL UNIQUE", "RENDRE UNIQUE", "ENCHAÎNER TOUT") avec rafraîchissement 60 FPS immédiat synchronisé avec le contrôleur MK3.

### Validation
- 222 tests réussis (`cargo test --workspace`)
- `cargo clippy --workspace --all-targets -- -D warnings` propre sans avertissement
- `cargo fmt --all -- --check` propre

## [0.11.4] - 2026-11-22

### Fixed
- **Capture guitare PipeWire** : Un thread routeur `mk3-pw-router` relie explicitement les ports physiques Line In 1/2 du MK3 au flux d'entrée de l'app via `pw-link`, corrigeant le problème où le micro du laptop était capturé au lieu de la guitare
- **Priorité des devices d'entrée** : Les devices virtuels PipeWire/Pulse sont maintenant essayés en premier (avec resampling transparent), repli sur ALSA direct MK3 uniquement en environnement non-PipeWire
- **Routage DJM** : Ne conflit plus avec le service systemd `djm-fixed-daemon.sh` — ne corrige que les conflits sur Output 3 au lieu de forcer Output 1
- **Noise gate** : Seuil du gate abaissé à -70 dB (depuis -40 dB) pour ne pas couper les signaux guitare faibles et le sustain naturel

### Improved
- Thread `mk3-pw-router` en arrière-plan surveille les liens PipeWire chaque seconde et gère le hot-plug/unplug du MK3 automatiquement
- Le nom du device d'entrée dans la GUI affiche "Maschine MK3 (Line In 1)" quand correctement lié

## [0.11.3] - 2026-11-21

### Fixes & Improvements

- **Ampli guitare & capture Line In 1 sur Maschine MK3** :
  - **Auto-configuration et routage PipeWire pro-audio** : Détection automatique de la source audio Maschine MK3 Pro (`node 132`) sous PipeWire/WirePlumber et définition en source par défaut via `wpctl`.
  - **Repli automatique ALSA / PipeWire** : Si l'accès matériel direct ALSA est occupé par PipeWire (`EBUSY`), le flux de capture bascule de manière transparente sur le graphe PipeWire connecté aux prises Line In physiques de la MK3.
  - **Sélection explicite du canal d'entrée** : Prise en charge de `LINE IN 1` (par défaut, AUX0/canal 0), `LINE IN 2` et `STEREO`, évitant l'atténuation de 6 dB liée au downmix mono.
  - **Support du format pro-audio PCM 32-bit** (`cpal::SampleFormat::I32`).
  - **Ampli actif par défaut avec accordage silencieux (Silent Tuning)** : Le signal guitare est immédiatement monitoré à l'ouverture, et la coupure de `MONITOR` laisse l'accordeur et l'analyseur spectral actifs pour s'accorder en silence sur scène.
- **Contrôles matériels MK3 dédiés pour l'ampli guitare** :
  - Les 8 encodeurs rotatifs d'écran pilotent en direct les 8 paramètres de l'ampli (`GAIN`, `BASS`, `MID`, `TREBLE`, `PRESENCE`, `SAG`, `MASTER`, `MONITOR`).
  - Les 8 soft keys MK3 intègrent `ACTIF`, `MONITOR`, `BYPASS`, et le cycle de canal d'entrée `INPUT+` (`LINE 1` / `LINE 2` / `STEREO`).
  - Nouveau sélecteur interactif de canal d'entrée dans la vue GUI.

### Validation

- `cargo test --workspace` (219 tests unitaires, d'intégration et fuzzing réussis)
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all -- --check`

## [0.11.2] - 2026-11-21

### Features

- **Plein accès matériel MK3 et affichage LCD double écran pour Kick Synth & Wavetable** :
  - **Kick Synth Page 2** : Visualisation dynamique des 7 formes d'ondes + modulation FM en temps réel sur l'écran gauche (encodeurs 1: WAVE, 2: FM AMT, 3: RATIO, 4: TONE), courbe de saturation multi-types et réponse fréquentielle résonante sur l'écran droit (encodeurs 5: DISTO, 6: FILTER, 7: CUTOFF, 8: RESO).
  - **Filtres Notch & Peak** : Intégrés et sélectionnables directement depuis les touches MK3 et les encodeurs rotatifs, avec courbes SVF et biquad dédiées.
  - **Wavetable Synth Page 2** : Contrôle complet des filtres SVF multimodes (LP12, LP24, HP, BP, Notch, Peak, Off), voix d'unisson, oscillateurs et rack d'effets (Chorus, Ping-Pong Delay, Reverb spatiale, Distortion, Volume).
  - **Parité totale GUI & MK3** : Les 8 encodeurs virtuels du panneau droit de la GUI basculent et affichent dynamiquement les paramètres exacts du matériel.
- **Interface graphique responsive et fluide** :
  - Remplacement de l'ensemble des conteneurs rigides par des flux adaptatifs `horizontal_wrapped` dans les vues Synth, Sampler et Guitar Amp.
  - Résolution des débordements et masquages des boutons et chips sur les écrans étroits ou avec facteur d'échelle élevé.
  - Courbe de filtre visuelle précise reflétant fidèlement le gain et la bande passante pour tous les types de filtres.

### Validation

- `cargo test --workspace` (216 tests unitaires, d'intégration et fuzzing réussis)
- `cargo clippy --workspace --all-targets -- -D warnings`

## [0.11.1] - 2026-11-21

### Fixes

- **Guitar amp no longer starts automatically**: `ACTIF` and `MONITOR` now
  default to OFF, so nothing is processed or monitored until the user enables
  them in the `AMPLI GUITARE` view.
- **Pattern audio untouched**: the amp chain only processes the MK3 capture
  stream and adds its output for monitoring; engine/instrument samples are
  never routed through it and remain identical while the amp is off.

### Validation

- `cargo test --workspace --locked` (new default-off and engine-output tests)
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

## [0.11.0] - 2026-11-21

### Features

- **Dedicated Guitar Amp screens on both MK3 displays**: PLUGIN page 2 renders
  `GUITAR AMP | PREAMP` (saturation curve, 3-band tone stack, live values above
  knobs 1–4) and `GUITAR AMP | ANALYSE` (IN/OUT VU, YIN tuner, 48-band FFT,
  CPU/latency/Xruns, knobs 5–8). Soft keys 1–8 map to amp/cab selection, bypass,
  monitor, reset and `PAGE SND`, identically on hardware and in the GUI mirror.
- **Guitar amp on the MK3 input**: new `AMPLI GUITARE` view (`G`) monitors the
  guitar jack through a real tube-amp chain inside `maschine-app`. CPAL capture
  (MK3 `pro-audio` input, fallback to default input) feeds a lock-free SPSC ring;
  the output callback runs a 4x-oversampled preamp (Vox AC30, Fender Blackface,
  Fender Tweed, High Gain), click-free biquad tone stack, cabinet simulation and
  delay, then mixes the result into the Maschine outputs. Eight MK3 contextual
  knobs control Gain/Bass/Mid/Treble/Presence/Sag/Master/Monitor.
- **Guitar diagnostics**: YIN tuner (note + cents), live FFT spectrum, real DSP
  CPU load, estimated latency, clipping/peak counters, Xruns and capture-device
  name in the new view. `maschine-linux audio-list` now lists input devices.
- **`maschine-dsp` rewrite**: fixed DF1 biquad state (now DF2T with smooth
  coefficient crossfades), real `set_sample_rate`, preallocated chain buffers,
  per-block parameter smoothing, and a polyphase 4x oversampler for alias-free
  tube saturation. The standalone rack's GUI knobs now actually reach the DSP.

### Validation

- `cargo test --workspace --locked` (all tests pass, including new DSP amp,
  tuner, biquad, diagnostics, ring-buffer and MK3 guitar-screen tests)
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo run -p maschine-engine --example dump_screens` (visual PPM export of
  both guitar screens)

## [0.10.5] - 2026-11-21

### Fixes & Improvements

- **Synth pad sustain on hold**: fixed pad pressure tracking and inter-burst stream silence timeout in hardware mapper so sustained pad presses (TB-303, Wavetable, Sampler) sound continuously for the entire duration of the press ("tout le long de l'appui") matching official Maschine behavior. Added pointer press & hold support in the GUI Keyboard view.
- **Enhanced Kick Synth BP & HP filters**: completely redesigned the Band-Pass and High-Pass filters in the kick modeler with analog state-variable filter non-linear saturation (`tanh`), rich resonant punch, Q-normalized unity gain, transient preservation, intelligent cutoff/reso initialization by filter mode (`set_filter_mode`), and adaptive cutoff knob frequency ranges in GUI and hardware.
- **Improved Nexus / Wavetable Synth filters & parameter persistence**: fixed parameter persistence so changes immediately write to the active sound in the project, updated audio target routing, removed artificial sustain floor clamping (`min_sus`), enhanced SVF HighPass and BandPass DSP with analog warmth, saturation and gain normalization, added `set_filter_mode` for smooth musical transitions, and added a filter Drive knob and adaptive cutoff in the GUI.

## [0.10.4] - 2026-11-21

### Fixes

- **Fix master volume drop to 0% on hardware Play button**: removed erroneous phantom potentiometer handling for bytes 36..41 in HID report `0x01` which reset master and headphone volumes to 0% upon pressing Play or other controller buttons.
- **Auto-configure Pioneer DJM mixer audio routing**: automatically detect connected Pioneer DJM USB soundcards (e.g. DJM-250MK2) and ensure ALSA `Output 1` is routed to `Ch1` (instead of `Aux`) so stereo audio outputs directly to Channel 1 of the mixer.
- **Audio settings clarity**: updated GUI audio output button label to clearly indicate the system default routing (PipeWire / DJM).

## [0.10.3] - 2026-11-21

### Fixes

- **MK3 transport audio stability**: releasing or recovering the display USB interface no longer resets the entire controller, preventing a screen error during playback from interrupting the shared audio interface.
- **Preserve the system audio output**: with no saved device preference, Maschine Linux now uses the system default instead of automatically redirecting audio to the MK3; the DJM remains selected when configured as the system default.

## [0.10.2] - 2026-11-19

### CI/CD

- **Parallel quality gates**: Clippy and workspace tests now run in separate CI jobs, Cargo caches preserve partial failures and write from both `master` and `main`, and release tags execute lint/tests before the expensive release build.
- **Resilient packaging**: AppImageTool is cached and downloaded with retries, package scripts remove stale outputs for the current version, and release verification rejects missing or duplicate RPM/DEB/AppImage/tarball artifacts.

### Validation

- `cargo fmt --all -- --check`
- `bash -n scripts/*.sh`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- GitHub Actions workflow YAML parsed successfully
- `desktop-file-validate packaging/maschine-linux.desktop`
- `appstreamcli validate --no-net packaging/io.github.timotheegrollier.maschine-linux.metainfo.xml`

## [0.10.1] - 2026-11-19

### Fixes

- **GNOME launcher opens the GUI**: the desktop entry explicitly runs `maschine-linux gui`; the `0.10.0` universal tarball binary had no default features and otherwise fell back to CLI `probe`.
- **Consistent desktop binary across packages**: AppImage and tarball reuse the same default-feature binary as RPM and DEB, keeping GUI, audio, USB, and MIDI enabled after an update.

### Validation

- `gtk-launch maschine-linux` opens the GUI through the GNOME desktop entry.
- `cargo test -p maschine-app default_command_launches_the_gui`
- Full release validation is recorded after packaging checks.

### CI/CD

- **Faster pipeline**: CI is split into parallel `metadata`, `test`, `msrv` and `release-build` jobs with a shared cargo cache and `concurrency` cancellation; tests run under `cargo nextest`. Release now compiles the binary once and packages RPM/DEB/AppImage/tarball in parallel from that artifact instead of recompiling it four times (twice with a different feature set).
- **Parallel release gates**: Clippy and tests are independent jobs in CI, caches survive partial failures and are shared across `master`/`main`, and release tags run lint/tests before the expensive LTO build.
- **Resilient artifact assembly**: AppImageTool is cached, downloads are retried, package scripts remove stale outputs for the current version, and release verification rejects missing or duplicate RPM/DEB/AppImage/tarball artifacts.
- **Release verification gate**: a `verify` job checks that each package carries the expected version and ships `/usr/bin/maschine-linux`, then generates `SHA256SUMS` before anything is published. A manual `publish` switch runs the whole pipeline as a dry run.
- **Consistent binary in every format**: AppImage and tarball no longer rebuild with `--no-default-features` (which dropped the GUI, audio, USB and MIDI features) but reuse the tested default-feature binary; the packaging scripts accept `MASCHINE_SKIP_BUILD` / `MASCHINE_SKIP_CHECKSUMS` and fail instead of producing an empty package.

## [0.10.0] - 2026-11-19

### Added

- **MK3 Shift + Pad layer**: implements all 16 secondary pad legends in Maschine order, including Undo/Redo, step Undo/Redo, full and 50% quantize, nudge, clear, copy/paste, and semitone/octave transposition. The desktop pad grid shows the same labels while Shift is held.
- **Pattern edit history and clipboard**: Shift-pad edits support undo/redo; Copy/Paste transfers the focused Pattern's event list. Clear and transpose actions are directly available from the pad layer.
- **Sound slot recovery**: Shift + Erase + Pad removes the Sound and its events across its Group, with Undo restoring the slot; Erase + Events + Pad erases only that Sound's events.
- **Settings and release checks**: atomic settings writes, corrupt-settings warnings, recent-project path handling, negotiated audio-rate reporting, AppStream validation, a CI check for the declared Rust 1.88 MSRV, and version-specific package collection that skips stale cached RPM/DEB files.

### Notes

- The project has no automation model yet; the printed Clear Auto shortcut reports that it is unavailable.
- Pad Copy/Paste operates on the event list of the focused Pattern because event-level selection is not implemented yet.

### Validation

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo +1.88.0 check --workspace --locked` (declared MSRV)
- `cargo check -p maschine-engine --no-default-features`
- `cargo test --workspace` (189 tests passed)
- `cargo build --workspace --release`
- `appstreamcli validate --no-net packaging/io.github.timotheegrollier.maschine-linux.metainfo.xml`
- `desktop-file-validate packaging/maschine-linux.desktop`
- `./scripts/build-rpm.sh`, `./scripts/build-deb.sh`, `./scripts/build-appimage.sh`, and `./scripts/package-release.sh`
- `sha256sum --check SHA256SUMS` from `dist/` (all four release artifacts passed)

## [0.9.12] - 2026-11-08

### Fixes

- **Filter Knob Routing**: `KParam::Cutoff` / `KParam::Reso` now route to the focused sound's synth filter (`KickSetParams`, `WavetableSetParams`, `AcidSetParams`) when a synth is focused, falling back to the multimode sampler filter — the kick filter is now modifiable through knobs (plugin view / MK3 knob panel) exactly like the low-pass, for samples/pads and synths.

## [0.9.11] - 2026-11-08

### Fixes

- **Multimode Sampler Filter**: the sampler voice filter now honors the active filter kind (`Off`, `LowPass`, `HighPass`, `BandPass`) instead of being hard-wired to a single low-pass biquad, making `HP` and `BP` selections audible and controllable through the cutoff/resonance knobs exactly like the low-pass (kick, 303, wavetable and sampler/pads paths unified on the multimode design).
- **Last Project Restore**: at startup without explicit project argument, the last opened project (persisted settings) is loaded first; the current project is persisted in the recent projects on last close.

### Validation

- `cargo fmt --all -- --check` (clean)
- `cargo clippy -p maschine-engine -p maschine-dsp -p maschine-hardware -p maschine-proto --all-targets` (clean, 0 warnings)
- `cargo test -p maschine-engine` (77 passing tests, incl. filter audibility regression test)

## [0.9.10] - 2026-11-08

### Added

- **Modular Hardware Mapping Architecture**: decomposed the monolithic `handle_button_event` in `HardwareMapper` into specialized, isolated handler methods (`handle_modifier_button`, `handle_transport_button`, `handle_view_menu_button`, `handle_4way_button`, `handle_display_button`).
- **Priority Resolution for MUTE & SOLO**: holding physical `SOLO` now takes immediate priority over background latched `MUTE` pad mode when targeting Groups A..H.
- **8 Groups (A..H) & 128 Independent Tracks**: `Project::new_demo()` and `load_default_808_kit()` now initialize a full 8-group matrix (Kit 808, Kit 909, Nexus Synth, Bass & Acid, Percussion, Keys & Chords, Vocal & SFX, Hits & Stabs) with 128 dedicated sound tracks, independent polyphony, sampler zones, synth routing, ADSR envelopes, filters, volume, and pan.
- **Universal Multimode Filter Shortcuts**: `Shift + Push 4D Encoder` instantly cycles the multimode filter type (`LP` -> `HP` -> `BP` -> `OFF`) on the focused sound across all views without changing display pages. `Shift + Knobs 6/7` (Arrange) and `Shift + Knobs 3/4` (Plugin/Sampler) also cycle filter mode.
- **Sustained Pad Notes**: physically held pad presses on synth instruments (TB-303, Wavetable, Kick) sustain indefinitely without cutting out, isolating resting sensor baseline noise (< `0x4280`) from intentional holds (> `0x4280`).
- **Clean AppImage Process Restart**: `restart_app()` uses detached process execution via `nohup`, strips AppImage-specific environment variables (`APPDIR`, `ARGV0`), and restores the working directory (`OWD`) for reliable restarts.
- **Comprehensive Hardware Reverse-Engineering Docs & Tests**: added 5 dedicated hardware mapping test suites in `mapper.rs` (167 passing tests workspace-wide) and extended reverse-engineering documentation covering 4D navigation hierarchy, Smart Strip modes, USB lifecycle, and contribution guidelines.

### Fixes

- **Gain Conversion Optimization**: replaced direct `powf` volume calculations with `db_to_linear()` across engine and synth render loops.

### Validation

- `cargo fmt --all -- --check` (clean)
- `cargo clippy --workspace --all-targets -- -D warnings` (clean, 0 warnings)
- `cargo test --workspace --no-default-features` (167 passing tests)

## [0.9.9] - 2026-11-08

### Added

- **Filter Selector Display Buttons**: direct soft buttons for `LP`, `HP`, `BP`, and `OFF` filter modes on Sampler and Kick synth screens.
- **Reverse Engineering Guide**: initial hardware reverse-engineering documentation for Maschine MK3 USB/HID and bulk displays.

### Fixes

- **Shift Soft Key Synchronization**: aligned soft key labels with actual hardware dispatch when holding Shift across all modes.

### Validation

- `cargo test --workspace --no-default-features` (163 passing tests)

## [0.9.8] - 2026-11-08

### Added

- **Pattern Toggle-off / Silence**: pressing the pad of an active pattern in PATTERN mode deselects it (sets scene slot to None), silencing the group while keeping the pattern intact in the project, allowing all patterns across any group to be deactivated simultaneously matching authentic Maschine MK3 behavior.
- **Multi-Pad Pattern Chaining**: pressing two pads simultaneously in PATTERN mode chains patterns sequentially (e.g., Pattern 1 + Pattern 2), seamlessly cycling through the chained patterns in a continuous loop based on their individual bar/tick lengths.
- **Real-Time Contextual Shift Labels**: holding the physical `SHIFT` button dynamically updates soft key labels on the MK3 displays across all views (e.g. `DOUBLE`, `HALF`, `QUANT`, `HUMAN`, `SAV PROJ`, `COUNT-IN`), providing instant visual discovery of shift shortcuts right above the display buttons.
- **Contextual Shift Soft-Key Actions**: Shift + SoftKey presses execute contextual operations including `DOUBLE` (double pattern length), `HALF` (halve pattern length), `QUANT` (50% grid quantize), `HUMAN` (humanize timing), `SAV PROJ` (quick project save), and `COUNT-IN` (toggle recording count-in).
- **Latchable Note Repeat & Arpeggiator**: pressing `NOTE REPEAT` enables note repeat; pressing `LOCK` latches note repeat active with illuminated LED feedback. Display buttons 1..8 allow immediate rate selection (1/32 to 1/2, triplets) with active rate LED indicators, fully playable live and recordable in real time with the `REC` button.
- **Sustained Melodic Notes**: holding a pad on melodic instruments (Wavetable / Nexus Synth, TB-303) sustains the note until pad release (NoteOff), while one-shot drum sampler pads retain authentic one-shot playback.
- **Wavetable / Nexus Synth Hardware Controls**: knobs 1..8 on MK3 dual screens directly modulate Filter Cutoff, Resonance, Env Amount, Drive, and full ADSR envelope parameters with complete parameter responsiveness.

### Fixes

- **4D Encoder Mixer Navigation**: pushing 4D encoder **Up** enters pad drill-down of the focused group, and pushing 4D encoder **Down** exits back to group overview / master view. Resolves navigation lock-in when returning from group to master.
- **Per-Pad Mixer Volume & Mute Isolation**: adjusting pad volume in the mixer only affects that specific sound track, ensuring Wavetable / Nexus Synth, Kick Synth, and Sampler pad levels do not bleed or affect neighboring synth/sampler volumes.
- **Wider Pad Mixer Strips**: updated MK3 display render to 4 wider channel strips per screen (Pads 1-4 and 5-8, or 9-12 and 13-16) with enlarged faders, higher-resolution VU meters, and clear dB readouts.
- **Hardware LED Synchronization**: fixed LED states for `NOTE REPEAT`, `LOCK`, `SHIFT`, `DUPLICATE`, and soft display rate buttons so physical LEDs accurately mirror system state.
- **Lock Button Functionality**: the physical `LOCK` button toggles lock state and illuminates its LED, providing authentic locking capability for Note Repeat and view locks.

### Validation

- `cargo test --workspace --no-default-features` (150+ passing tests)
- `cargo check --workspace --no-default-features` (clean)

## [0.9.7] - 2026-11-08

### Added

- **Wavetable / Nexus Synth plugin**: 16-voice polyphonic synthesizer modeled with dual oscillators, 7-voice unison supersaw, acoustic and digital wavetables (Grand Piano, M1 Piano, Rhodes, Formants, Digital Waves), multimode SVF filter (LP12, LP24, HP, BP), dual ADSR envelopes, LFO modulation, stereo FX rack (Chorus, Ping-Pong Delay, Reverb, Distortion), and 28 factory presets (Concert Grand Piano, House M1 Piano, Vintage Upright Piano, Honky Tonk, Neo-Soul Rhodes, DX7 E-Piano, 808 Sub Boom, Reese Bass, Acid 303 Sub, Neurofunk Slither Bass, Slap Funk Bass, Cyberpunk Bass, Future House Donk, Titanium Supersaw Lead, Future Rave Lead, Hyperpop Chiptune Lead, Blade Runner Brass, Cyber-Drive Synth, Trance Euphoria, Celestial Strings, Analog Warmth, Cinematic Dark Pad, Spire Pluck, Pizzicato Strings, Kalimba Dream, Crystal Mallet, Deep House Organ, Hammond B3 Organ). Fully integrated in the Desktop GUI, Browser plugin selector, MK3 dual LCD screens, and hardware knob controls.
- **Kick Synth Modeler enhancements**: extended body waveforms (Sine, Triangle, SatSine, SubSquare, NoiseTransient, Parabolic, Ramp), selectable distortion drive types (Analog Tanh, Hard Clip, Wavefolder, Tube Drive, Bitcrush, Fuzz, Rectifier), linear FM frequency modulation (FM AMT / FM RATIO) with hold stage before pitch/amplitude decay, 6 multimode resonant filter modes (Off, LowPass, HighPass, BandPass, Notch, Peak) with cutoff & resonance controls on MK3 and GUI, transient click layer, sub-bass tuning, and 13 factory presets (Techno Berlin, 909 Punch, Hardcore Wavefolder, Sub 808, Minimal Click, Industrial Tube, Acid Gabber, Deep Dub, Frenchcore Fuzz, Dark Psy FM Kick, Glitch Bitcrush, 909 Parabolic Punch, Trap 808 Long Boom).
- **Mixer 4D Navigation & Drill-Down**: 4D Left/Right navigates between Pads 1-8, Pads 9-16, and Group mixer views. EncoderPush toggles between Group overview and pad drill-down with live per-pad audio VU meters (green/yellow/red gradient, clip indicators, peak hold) and volume faders in the GUI sound strips and on the MK3 dual screens. 4D Up/Down selects sound without triggering audio.
- **Instant Filter Selection**: instant filter kind selection chips and cycling (LP, HP, BP, OFF, NOTCH, PEAK) in Sampler and Kick synth via GUI and MK3 soft keys (`CYC FLT`, `LP`, `HP`, `BP`, `OFF`, `NOTCH`, `PEAK`), or on the fly by holding `Shift` and turning the filter knob (cutoff / resonance) in Sampler and Kick synth.

### Fixes

- **Latched Mode Auto-Exit**: pressing any top-row view button (`FILE/SAVE`, `BROWSER`, `PLUGIN`, `CHANNEL/MIDI`, `MIXER`, `SAMPLING`, `ARRANGER`) automatically clears all latched modes (`PATTERN`, `SCENE`, `SELECT`, `ERASE`, `MUTE`) and closes overlays (`EVENTS`, `MACRO`, `SETTINGS`) to return the hardware and pads to standard operational state.
- **Hardware LED synchronization**: synced `EVENTS` and `MACRO` LEDs on the MK3 controller with active screen overlays.

### Validation

- `cargo fmt --all -- --check`
- `cargo test --workspace --no-default-features` (144 passing tests)
- `cargo clippy --workspace --all-targets --no-default-features -- -D warnings` (clean, 0 warnings)

## [0.9.6] - 2026-11-08

### Added

- **Kick Synth Modeler plugin**: complete DSP modeled kick synthesizer for techno, club and analog beats. Features exponential pitch drop envelope, punch snap, transient attack click, resonant 50 Hz sub bass boost, analog tanh overdrive, and 6 built-in presets (Techno Berlin, 909 Punch, Hardcore Drive, Sub 808, Minimal Click, Industrial).
- **MK3 Pad Mixer mode**: dedicated mixer modes allowing encoders 1..8 on the MK3 hardware to directly adjust the volume (and pan with Shift) of individual pads (Pads 1-8 or 9-16) for the active group, with dual-screen VU meters, faders, and dB readout.
- **Effects Chains on Pads and Groups**: real-time zero-allocation DSP effect processors supporting Reverb (space & diffusion), Delay (ping-pong echo), Distortion (tanh saturation), Filter (LP/HP/BP multimode), and Compressor (dynamics punch).
- **Browser Plugins & Effects tabs**: dedicated tabs in the MK3 screens and Desktop GUI to browse, preview, and load instruments (Kick Synth, TB-303, Sampler 808, Guitar Amp) or add effect slots directly to pads or groups.
- **MK3 Project deletion**: direct project deletion from the MK3 FILE/SAVE browser with safe confirmation dialog (soft keys and physical ERASE button) and automatic disk cleanup.

### Fixes

- **Restart button**: update modal "Redémarrer maintenant" button now reliably spawns detached processes for both native binary and AppImage packaging, with delay to ensure audio and USB HID devices are completely released by the Linux kernel before restart.
- **Unified PLUGIN navigation**: replaced fixed hardcoded "PAGE TB" with dynamic PLUGIN navigation following whichever instrument is loaded on the active pad.

### Validation

- `cargo fmt --all -- --check`
- `cargo test -p maschine-hardware -p maschine-engine` (97 passing tests)
- `cargo test -p maschine-app --no-default-features` (all tests passing)
- `cargo clippy -p maschine-engine -p maschine-hardware -p maschine-dsp -p maschine-proto -- -D warnings` (clean)

## [0.9.5] - 2026-11-08

### Fixes

- **MK3 FILE/SAVE and BROWSER LED mapping**: properly distinguished between project file browsing (`FILE/SAVE`) and sound preset browsing (`BROWSER`). The physical `FILE/SAVE` LED now illuminates brightly (intensity 7) while in FILE mode, while `BROWSER` LED remains dim (and vice versa). `Shift + FILE/SAVE` provides illuminated hint feedback for quick save.
- **New Project creation workflow**: pressing `NOUVEAU` (soft button 3) on the MK3 now generates a unique project file (`Projet 1`, `Projet 2`, etc.), saves it to disk in `~/.local/share/maschine-linux/projects/`, initializes a playable 808 drum kit on the pads with a fresh sequence, updates `project_entries` with instant on-screen status feedback (`COURANT` badge), and provides a status message.
- **MK3 displays & soft buttons alignment**: updated both screens in FILE mode (`PROJ <`, `PROJ >`, `NOUVEAU`, `CHARGER`, `SAUVER`, `RAPIDE`, `PRESETS`, `RETOUR`). Fixed right screen softkey offset from 0 to 4 so buttons 5-8 correspond directly to the displayed actions. Knobs and 4D encoder controls are mapped unambiguously.

### Validation

- `cargo fmt --all -- --check`
- `cargo test -p maschine-hardware -p maschine-engine` (95 passing tests)
- `cargo test -p maschine-app --no-default-features` (all tests passing)

## [0.9.4] - 2026-11-08

### Fixes

- **MK3 FILE browser workflow**: added `NEW` soft key in the project browser (`browser_kind == 1`) to create a new project directly from the MK3, alongside `CHARGER` / `SAUVER` / `RAPIDE` / presets navigation.

### Validation

- `cargo fmt --all -- --check`
- `cargo test -p maschine-engine -p maschine-hardware -p maschine-app --lib` (all passing)

## [0.9.3] - 2026-11-08

### Fixes

- **MK3 FILE/SAVE vs BROWSER mapping**: corrected physical button behavior. `FILE/SAVE` (the button below `BROWSER`) opens the project/file menu on tap and performs a quick save with `Shift + FILE/SAVE`. `BROWSER` opens the preset/sample browser; `Shift + BROWSER` opens the project browser. Updated MK3 on-screen guidance.

### Validation

- `cargo fmt --all -- --check`
- `cargo test -p maschine-hardware --lib`

## [0.9.2] - 2026-11-08

### Fixes

- **MK3 project browser access**: `BROWSER` now opens the project/FILE browser (with project scan) so projects can be loaded/saved from hardware again; `Shift + BROWSER` opens the preset browser; pressing `BROWSER` again exits to Sampler.
- **Mute LED feedback**: while MUTE pad mode is active, muted pads stay visibly dimmed (instead of nearly off) to make the selected muted pads easier to identify, closer to Maschine workflow.
- **Auto-update UX**: added a **Restart now** button in the GUI updater after an update is installed.

### Validation

- `cargo fmt --all -- --check`
- `cargo test -p maschine-engine -p maschine-hardware -p maschine-app --lib` (workspace libs touched by this patch)

## [0.9.1] - 2026-11-08

### Fixes

- **MK3 FILE / save workflow**: The physical `FILE SAVE` button now performs a normal project save and creates a project file when no current path exists. `Shift + FILE SAVE` keeps the compact quick-save behavior. The FILE/project browser on MK3 now shows accurate soft-key labels and guidance (`PROJ </>`, `CHARGER`, `SAUVER`, `RAPIDE`, `PRESETS`) and project soft-key save actions work even for unsaved projects.
- **Save feedback**: MK3 and GUI snapshots now expose a transient status message; the MK3 header shows a visible save confirmation (`Projet sauvegardé` / `Sauvegarde rapide OK`) after save actions.
- **Maschine-style per-pad mute**: `MUTE` now latches a pad-mute mode. While active, tapping pads toggles mute for individual Sounds in the focused Group without triggering playback, and muted pads are rendered dimmed on MK3 LEDs. `MUTE + GROUP` (or Group selection while mute mode is active) still targets Group mute. Engine playback filters muted Sounds in live triggers and sequenced events.
- **Keyboard / Chords recording**: Keyboard and Chords triggers now record the actual MIDI pitch during `REC`. Chord pads record each triggered note as a melodic event, preserving pitched/chord performance in patterns.

### Validation

- `cargo fmt --all -- --check`
- `cargo test -p maschine-engine -p maschine-hardware` (93 tests passing in this environment)
- `cargo check -p maschine-app` (validated with the Nobara ALSA/JACK `PKG_CONFIG` sysroot)
- Interactive MK3 hardware validation is still recommended for FILE navigation/load/save, mute pad LEDs, and Keyboard/Chords recording/playback.

## [0.9.0] - 2026-11-08

### Highlights

- **FILE / project workflow (MK3 + GUI)**: The MK3 `FILE` button opens a project browser with quick navigation and loading from the hardware or GUI. `Shift + FILE SAVE` performs a compact quick save of the current project; normal saves write readable JSON. Projects are stored as viewable `*.maschine-linux.json` files with atomic writes, and recent/current project paths are remembered for fast reloads.
- **Per-pad volume and pan per Group**: Every Sound/pad in every Group keeps its own mixer level and pan. The GUI Mixer exposes editable per-pad strips for the focused Group, and the selected pad/Sound can be edited from the MK3 mixer workflow and screens.
- **Keyboard and Chords repaired**: Keyboard and Chords modes now play the active Sound melodically with engine-owned musical state (root note, scale, base octave, chord type, inversion). The top display buttons / soft keys change scales and musical behavior, and both MK3 screens plus the GUI Keyboard view show the correct performance information.

### Validation

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (all workspace library tests passing in this environment)
- Hardware validation is recommended on a connected MK3 for HID/display/LED behavior, FILE navigation, mixer control, and Keyboard/Chords performance.

## [0.8.0] - 2026-11-07

### Highlights

- **Complete PATTERN workflow**: create, duplicate, clear, delete, rename, resize, and navigate patterns from the MK3 and the GUI. Empty pattern pads create the corresponding slot, `DUPLICATE` copies the active pattern, `SHIFT + ERASE` clears its events, and the 4D encoder steps through patterns. The eight soft keys expose `PAT <`, `PAT >`, `+ PAT`, `DUPL`, `VIDER`, `SUPPR`, `1 MES`, and `2 MES`.
- **Pattern manager in the GUI**: the Sequencer view now has a dedicated pattern strip with selection, creation, duplication, clearing, deletion, length selection, and inline rename. The Ideas matrix shows 8–16 pattern slots per Group; clicking an empty slot creates that pattern and assigns it to the active Scene.
- **Safe pattern references**: inserting or deleting a pattern re-indexes only the affected Group in every Scene, keeps Scene references valid, and never allows deleting the last pattern of a Group.
- **Hideable workspace**: the centre workspace can be hidden independently with the `CENTRE` button or `F3`. The existing `F1` navigation rail and `F2` MK3 panel toggles remain, and the new `TOUT` button shows or hides all three panels at once for a compact performance layout.
- **Graceful MK3 shutdown**: closing the application now stops the HID loop and joins the display/LED threads, sends a clean-room shutdown splash, releases the display interface without a USB reset, and sends an all-off LED report. The firmware can repaint its default logo and the pads return to their default state instead of staying frozen on the last UI.

### Pattern model and safety

- `EngineEvent` now exposes `AddPattern`, `CreatePatternAt`, `DuplicatePattern`, `DeletePattern`, `ClearPattern`, `RenamePattern`, and `StepPattern`.
- New patterns inherit the active pattern length (fallback: two bars). Names are trimmed and capped at 32 characters; empty names fall back to `Pattern N`.
- Scene references are shifted on insertion and repaired on deletion. A Scene that used a deleted pattern falls back to the nearest valid pattern so every Scene remains playable.
- Pattern selection from the hardware or GUI always targets the active Scene; if no Scene exists, one is created and sized for all Groups.

### Shutdown and hardware lifecycle

- `BulkHandle` tracks whether IF5 was already released for shutdown and skips the historical USB reset in that case. This avoids leaving the screens frozen and avoids disturbing the shared audio stream.
- `DisplayThreadHandle` and `LedWriterHandle` now stop and join from `Drop` as a safety net, so error paths cannot leave background writers running.
- The GUI HID loop has an explicit shutdown flag and is joined before the display and LED writers. The daemon follows the same ordered shutdown path on Ctrl+C.
- The final display splash is generated by `display_render::render_shutdown_screens()` and is covered by a rendering test.

### Validation

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (133 tests)
- Hardware validation on a connected MK3 (`serial=1B64AB9B`, `/dev/hidraw2`): probe, GUI hardware startup, first display frame, window close, shutdown splash/release log, and probe again after exit with the device still available.
- Standalone GUI validation with `--no-hardware`: startup and window close without a hardware session, with hot-plug reconnection disabled.
- Automated mapper tests cover pattern-mode `DUPLICATE`, `SHIFT + ERASE`, and 4D pattern stepping; engine tests cover create-at-slot, duplicate, clear, rename, delete, scene re-indexing, soft keys, and pattern-mode screens.

## [0.7.0] - 2026-11-07

### Highlights

- **Latched Maschine MK3 navigation**: PATTERN, STEP, KEYBOARD, SCENE, EVENTS, MACRO, SETTINGS, and PLUGIN stay active after the first press and return to the default Pad workflow on a second press. Only one workflow menu can be active at a time.
- **Hardware-driven GUI navigation**: PATTERN, STEP, KEYBOARD, and SCENE open their corresponding GUI view through the engine `NavigateView` event and `nav_serial` counter, instead of relying only on the left display tab.
- **Unified PLUGIN view**: the physical PLUGIN button opens the SOUND page for the active Sound. Soft keys switch to the TB-303 page, which exposes a simplified macro layout and keeps advanced oscillator, filter, distortion, and delay controls collapsed by default.
- **Keyboard / Performance view**: a dedicated GUI view with a 16-key performance keyboard, Sound auditioning, and direct pad access.
- **Standalone GUI mode**: `maschine-linux gui --no-hardware` ignores a connected MK3 and runs the full GUI, engine, virtual MIDI, and audio stack without hardware. Hot-plug reconnection is disabled in this mode.
- **Per-Sound sampler control**: polyphony limits and choke groups are applied per Sound, including click-free stealing of the oldest voice from the same Sound.
- **Responsive desktop layout**: collapsible and resizable navigation and MK3 panels (`F1`/`F2`), an application bar with view picker and latched-mode indicators, compact layouts for small windows, and context banners for hardware workflows.
- **English documentation and release automation**: repository documentation is now in English, and `scripts/extract-release-notes.sh` supplies the GitHub release body from this changelog.

### Validation

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- Hardware checks on a connected Maschine MK3 (`serial=1B64AB9B`): device probe, LED test, logical pad test, dual-display solid-color test, first display frame, and GUI hardware startup.
- Standalone GUI launch with `--no-hardware`: no hardware session, no hot-plug reconnect, virtual MIDI available, and the updater reports `v0.7.0` up to date.

### Acid TB-303 Synthesizer (Hardware MK3 & Desktop GUI)
- **Complete Acid 303 DSP module** (`crates/maschine-engine/src/acid_synth.rs`):
  - Anti-aliasing PolyBLEP oscillator (Sawtooth and Square waveforms) with octave shift and analog portamento/slide.
  - 4-pole resonant low-pass filter (24 dB/oct) with nonlinear hyperbolic tangent (tanh) saturation, acid self-oscillation above 80% resonance, envelope modulation and velocity-sensitive accent.
  - Analog distortion / overdrive with tanh waveshaping, post-clipping tone control and dry/wet mixing.
  - Stereo ping-pong delay with damped feedback and dry/wet mixing.
  - 4 built-in presets: Classic Acid, Squelch Lead, Distorted Acid, Dub Echo.
- **Maschine MK3 dual-screen rendering (480×272)** (`crates/maschine-engine/src/display_render.rs`):
  - Left Screen (Tab 4): Real-time display of the oscillator waveform, resonant filter frequency response curve with cutoff cursor, status badges and encoder 1..4 mapping (Cutoff, Reso, EnvMod, Decay).
  - Right Screen: Visualization of the tanh distortion curve, ping-pong delay echo bars, preset selector and encoder 5..8 mapping (Accent, Drive, Time, Mix).
  - Function buttons / Soft Keys 1..8: SAW/SQUARE toggle, Octave -/+, Slide, and instant selection of the 4 presets.
- **Maschine MK3 hardware support** (`crates/maschine-hardware/src/mapper.rs`):
  - The physical `PLUGIN` button opens the unified PLUGIN page (tab 4) on the SOUND page, and its LED lights up. Soft keys switch to the TB-303 page.
  - The 8 rotary encoders are routed in real time to the active PLUGIN page.
- **Desktop GUI** (`crates/maschine-app/src/gui/views/synth.rs`):
  - Unified "PLUGIN / TB-303" view accessible via the left navigation rail and the `T` or `P` keyboard shortcut.
  - Interactive egui visualizations for the oscillator, filter cutoff/resonance curve, distortion and delay repeats.
  - 16-key virtual keyboard (C1 to D#2) to play and test the acid line instantly with the mouse.
  - Bidirectional synchronization with the MK3 screens and encoders.
- **Demo project**: Pad 15 (group A) is configured by default as an Acid 303 synthesizer with a 2-bar acid bass pattern.

### Standalone GUI for development
- The `maschine-linux gui` command now starts without a Maschine connected:
  engine, mouse/keyboard interface, virtual MIDI and system audio remain
  usable.
- Detection, HID, LEDs and LCD remain optional for the GUI, while the
  headless daemon keeps its controller requirement.
- `--no-hardware` explicitly ignores a connected MK3 and disables hot-plug
  reconnection for reproducible standalone sessions.
- If a remembered audio output is unavailable (offline), it falls back to the
  system output, unless explicitly selected via `--audio-device`.

### Sampler, persistence and smoothness
- Samples are identified by Group/Sound pair: two groups can use the same pad number without sharing or stopping the wrong voice.
- WAV import is decoded in a background task, validates the data before replacing a Sound, fixes signed 32-bit PCM normalization and keeps the sound in place if the file is invalid.
- Sample paths and mappings are kept with the project and relative WAVs are reloaded on open. Group/sound volume and pan are also applied to voices already playing.
- The waveform is precomputed on import and shared read-only: the GUI and LCDs no longer rescan the whole file on every frame.
- Voice endings are now handled by in-place removal in the audio render, without a temporary buffer or sorting on each block.
- Tests cover group isolation, WAV mapping/persistence, reloading, live mixing, 32-bit PCM WAVs and import failure without losing the old sound.
- `docs/TESTING.md`, the README and the usage guide describe CI validations and manual hardware checks. Sampler/Events scope is distinguished from the full features of the official software.

### Fixes from LCD validation and workflow (0.6.0)
- **Display stream misalignment**: some partial rectangles had an odd width while `transmit` counts pairs of RGB565 pixels. The recovery probe was also 1×1. The differential now expands rectangles to an even width, the codec rejects odd windows and the probe sends 2×1 pixels. Tests added for the single pixel at the right edge, the pair counter, probe encoding and random rectangles.
- **Right screen stuck on EVENTS**: selecting a main view now resets the right overlay and leaves SONG when a view outside ARRANGE is chosen. The Sampler page therefore resumes its ADSR screen on the right after EVENTS/MACRO/SETTINGS; clicking Sampler when already active in the GUI also restores that page.
- **First knob after GUI navigation**: the HID mapper now re-reads the engine context before each event. It applies the first detent to the current Sound/Sampler instead of using the previous hardware tab.
- Documentation updated in `docs/protocol.md`, `docs/HARDWARE_DISPLAY_PROTOCOL.md`, `docs/workflow.md`, `docs/USAGE.md`, `docs/architecture.md` and the README. The documents now distinguish USB acceptance from real visual validation of the LCDs.

## [0.6.0] - 2026-10-18

### Fixed
- **Frozen MK3 screens (second wave)**:
  - **Permanent startup failure**: if another instance (gui/daemon left open) or a USB re-enumeration held IF5, `claim IF5: Resource busy` killed the display thread without retry → screens never refreshed. The thread now has a **lifecycle**: opening with retries (3 s), release, hot reopen when the interface becomes free again (+ clear log "new attempt in 3 s").
  - **Interrupted packets not completed**: the protocol is an MCU command stream (window header + `transmit` + pixels + `blit` + `end`). A partial write leaves the parser waiting for the *exact remainder* of the pixels → every subsequent send is shifted (screen frozen forever). `send_frame` now completes any interrupted packet with **its own remaining bytes** then `blit`+`end` (returns `Ok` if the packet succeeds); otherwise (timeout, unknown position) a **black purge** (`blit`+`end`) puts the parser back in a waiting state. 2 unit tests (`packet_tail`, real case 10588/7680).
  - **Gentle escalation**: 2×1 px probe (32 B, valid RGB565 pair) before pushing real frames again after an error; gentle re-acquisition (release + claim, **never a USB reset mid-flight** — it would kill the shared audio stream) after 8 consecutive errors, with increasing cooldown (6→24 s) and invalidations; full reopen only if the device is gone. Extended backoff (250 ms → 4 s).
  - **Calibrated ceiling**: average throughput reduced 2.5 → **1.6 MB/s** (560 KB burst) — above ~2 MB/s sustained, the MCU RX queue overflows and the endpoint stops accepting. Hardware measurement: 70+ s consecutive without error at 1.55 MB/s / 17-20 FPS; the remaining transient wedges (MCU internal states) repair themselves in ~10-20 s instead of freezing the screen permanently.
- **Log storm on stop**: 18 million POLLERR lines in one second (ALSA during the cpal stream drop) → error event handler rate-limited (1st error then 1/1000).
- **Frozen MK3 screens (first wave)**: each refresh sent a full uncompressed frame (2 × 261 148 B ≈ 522 KB); at 30 FPS ≈ 15.7 MB/s → USB port saturation, `short write`/`Operation timed out`, blocked endpoint (frozen screen). Replaced by:
  - **Incremental region diff** (`FrameDiffer`, proto): vertical runs of modified rows, X bbox per run, merge beyond 6 rectangles, full frame if dirty area exceeds 55%; window header `x/y/w/h`; skip identical frames.
  - **Byte budget** 2.5 MB/s (700 KB burst) with coalescing (uncommitted diffs if budget insufficient); USB recovery (`clear_stall` + backoff + `reacquire` after 4 errors); invalidation → full frame after error.
  - `send_frame → Result<usize>` (1 s timeout if >100 KB, otherwise 250 ms), `clear_stall()`, `reacquire()` (stubs updated); `display_thread.rs` rewritten (diff, budget, backoff, reacquisition, 10 s stats, conditional commits).
  - Bug fixed in `col_range` (`x1 = x + 1` overwrote the max → `if x + 1 > x1`); 8 tests added including the `differ_always_reconstructs_exactly` property and the `debug_synthetic_rows` regression.
- **MIXER dispatch**: `render_mixer_right` was dead code (the right MIXER screen displayed PATTERN) → wired for `left_tab == 1 && right_tab == 0`.

### Changed
- **Audio**: already delivered in 0.5.0 (64-frame buffer ≈ 1.3 ms, master limiter + `KIT_HEADROOM`, kit at device rate, anti-click) — unchanged.
- **Design of the 2 screens**: common chrome (26 px header with 16-pad activity ribbon + mini master VU, bordered panels, encoders with accent notch, filled soft keys), dense "tracker" labels via `draw_text_clipped` (9 px clipped glyphs), consistent palette.
- `pad_activity: [f32; 16]` (~170 ms decay, `PAD_ACTIVITY_DECAY = 6.0`) to animate the screens (playing + sequencer), `revision` bump during decay; fed by pads and `trigger_pattern_events`.
- **Left ARRANGE**: transport strip with dots, "A S01" chip, group rows with M/S pills and 142 px VU.
- **Right PATTERN**: velocities by height, time lines, tracker labels, activity LEDs, playhead.
- **Right STEP**: M/STEP/VEL/LEN/NOTE header, numbered velocity bars, cursor/playhead.
- **MIXER**: 4 console strips (vertical VU + dB scale + clip LED, M/S, pan), master section (large VU + dB + LOOP/METRO + bar), focus section (group + sound + ADSR).
- **SAMPLER**: amplitude-gradient waveform + markers, filled ADSR + breakpoints.
- **EVENTS**: column table (SND/STEP/VEL/LEN/NOTE), velocity bars, note names, played event highlight.
- **SETTINGS**: 4 cards (PROJECT / TRANSPORT / AUDIO-PLAY / FOCUS) with aligned keys/values.
- **STEP**: differentiated titles "STEP / GRID" (left) and "STEP / DETAIL" (right).

### Added
- Tests: `render_is_deterministic`, `views_are_not_all_identical`, `pad_activity_decays_and_bumps_revision` (display_render); proto diff tests already present (27 green); workspace: 94 tests green.
- `note_pad_activity` made public.

## [0.5.0] - 2026-10-18

### Fixed
- **Crackles + audio latency**: the stream was opened with `BufferSize::Default` (≈2048 frames ≈ **46 ms** on PipeWire) and a failed `try_lock` on the engine returned an entire **silence buffer**. Negotiation is now **48 kHz / 256-frame buffer (64-frame period ≈ 1.3 ms)** with progressive fallbacks (512 → 1024 → 2048 → default) and **waiting for the engine lock up to 2 ms** before any silence. Measured under extreme load (playback + engine lock every 1 ms + snapshots + 30 FPS screens + 61 voices): **0 miss, 0 err, 0 clipping**.
- **Groove quantized to the buffer**: sequencer notes started at the **beginning of the audio buffer** (up to 46 ms offset, swing/humanize destroyed). Each voice now starts at its **exact sample position** within the block (`SamplerVoice::start_frame`), metronome included.
- **Hard output clipping** (`clamp(-1,1)`): kick + snare + clap reached ≈1.55 and were **clipped flat** (the real source of the "crackle"). Replaced by a **soft limiter** (linear below -3 dB, tanh saturation above) + **kit headroom ≈ -2.5 dB**: solo hits remain linear, only coincidences are smoothed (≈0.6 dB).
- **Kit detuned by 1.5 semitones**: DSP presets were generated at 48 kHz and played at 44.1 kHz (+8.8%). `Engine::set_audio_sample_rate()` (called when the stream opens) **regenerates the kit at the actual rate** and repoints the sampler zones; clock realignment without allocation in the callback.
- **Sample-end clicks**: all tails (kick, crash, open hat...) were cut off abruptly at non-zero amplitude (up to -22 dB). Raised-cosine end fades + 0.4 ms fade-in in the drum synth, 3 ms anti-click fade at sample end in the sampler, faded metronome click.
- **Abrupt voice stealing**: the oldest voice was **removed instantly** (click); it now fades out over **5 ms**.
- Buffers > 1024 frames: some samples remained **unwritten** (garbage). Rendering in bounded chunks, regardless of format (F32/I16/U16, 2 or 4 channels).

### Changed
- **808 drum synth rewritten**: RBJ biquads (LP/HP/BP 12 dB/oct) instead of 6 dB/oct filters, **band-limited oscillators** (harmonic sum: zero aliasing) for hi-hats/crash, `tanh` saturation, **per-instrument normalization** (homogeneous kit), reworked envelopes (kick sub + glissando + filtered click, snare dual partial + 1.4–7.5 kHz noise, clap 3 bursts + tail, toms with stick attack, cowbell, 1.6 s metallic crash).
- **Sampler**: no more allocation in `note_on` (zone selection without `Vec`), chunked rendering, `replace_zone_sample` for rate changes.
- Unified audio negotiation (4 channels on MK3 then 2-channel fallback); `--debug-latency` logs the **actual** buffer size and backend errors.
- New diagnostic mode `daemon --stress` (playback + simulated engine lock contention).

### Added
- Tests: silent sample end (anti-click), kit DC absence, note placement **inside** the block (`notes_start_at_their_position_inside_the_block`), metronome driven in blocks like the real callback.
- Quality-control examples: `render_groove` (WAV + peak/RMS/clipped) and `render_kit` (16 instruments as WAV).

## [0.4.0] - 2026-10-18

### Added
- **STEP mode faithful to the official software**: step cursor (`step_cursor`, absolute index in the pattern), the touched pad becomes the selected step, step editing with knobs 1–4 (`VEL`, `POS` nudge in ticks, `LEN`, `PITCH` + note name), **SCROLL of the full pattern grid on knob 5** and the 4-D encoder `<`/`>`, knobs 6–8 = `VOL SND`/`CUTOFF`/`RESO`.
- New engine events `Swing`, `StepCursor`, `StepScroll`, `StepVelocityDelta`, `StepNudgeTicks`, `StepLengthDelta`, `StepPitchDelta` (+ mapper actions and tests).
- **Left STEP screen**: full pattern grid (one row per bar), cursor cell outlined, playhead, live detail of the selected step and slots 1–4. **Right STEP screen**: orange outline of the edited step, `STEP n/n` in the header, slot 5 = `SCROLL`.
- STEP soft keys: `STEP <</>>`, `BAR <</>>`, lengths `1/2/4/8 BARS`.
- Edit cursor visible on pads when stopped (white) in the sound color for programmed steps.
- End-to-end `engine` test: place + edit + scroll cursor; mapper tests (STEP knobs, accumulator, fast-turn clamp, 4-D in STEP).

### Changed
- **Knob speed recalibrated** ("the knobs turn too fast"): scaling done **only once** in the app (the mapper emits raw ticks), official-software-like increments: ≈1.5 BPM/detent (TEMPO), 0.03 dB (volumes), 0.001 (pan/reso/master/sustain), **exponential CUTOFF** `2^(ticks/384)` (≈2.7 octaves/turn, ≈1 semitone/detent), ADSR 0.5–1 ms/detent, BROWSER presets/sounds per detent.
- Fast knob turns (delta > 128 ticks) are **clamped instead of discarded**: no more lost movements.
- 4-D encoder `<`/`>` in STEP mode: scrolls the grid by one step instead of changing bar.
- The display thread wakes its loop every 3 ms (lock-free activity flag set by the HID loop) and sends as soon as a change is ready: reactivity within a few milliseconds, engine lock taken at most every 33 ms (USB ceiling ~30 FPS, 2 s guard) — **0 audio misses** preserved.

### Fixed
- **Screens that "did not respond"**: `Swing` wrote the project directly without synchronizing the sequencer or invalidating the display; `toggle_step`, `clear_bar`, humanize/quantize, groove reset and DUPLICATE BAR modified the project without bumping the revision (screens stayed frozen until the 5 s guard). Everything now goes through the Engine (`EngineEvent::Swing`, `invalidate_display`) and refreshes the display.
- `RESTART` goes through `EngineEvent::Restart` (reset + immediate invalidation of the displayed BBT).
- The SWING knob now actually affects playback (the sequencer used the old value).

## [0.3.0] - 2026-10-18

### Added
- **Complete redesign of the 2 MK3 screens (480×272)** in the style of the official software: unified design system (title bar, encoder rows, soft key bar), 16 contextual views (ARRANGE, PATTERN, STEP, SONG, SCENES/IDEAS, MIXER ×2, SAMPLER ×2, BROWSER ×2, EVENTS, MACRO, SETTINGS), live values per Sound/Group and real VU meters.
- **Unified contextual soft keys**: `EngineEvent::SoftKey(u8)` + labels exposed in the snapshot — hardware, screens and GUI execute and display exactly the same action (no more hard-wired screen buttons).
- **Waveform + ADSR envelope drawn** on the SAMPLER screen from the active Sound sample (min/max envelope per column).
- **Real-time VU meters** per Group (8) and Master with peak hold and decay, computed in the audio callback and broadcast in the snapshot.
- **`--debug-latency` instrumentation**: HID→Engine latency (latest/max per 2 s window) and audio statistics (`callbacks`, `lock_misses`, max callback time).
- **Immediate GUI wake-up** (`RepaintSignal`) from the HID thread + revision-counter-driven repaint (16 ms during playback, 50 ms at rest); the display thread only resends a frame if the state changed.
- **Extended Song/Scene workflow**: `DuplicateScene`, `DeleteScene` (section re-indexing), `DuplicateSection`, `DeleteSection`, `ResizeSection` (length 1/2/4/8/…), exposed in the GUI IDEAS view.
- Smoke test rendering all screen views (anti-panic guard) + section/scene workflow tests.

### Changed
- **Real-time engine**: the snapshot shares the project as an `Arc` (copy-on-write via `Arc::make_mut`) — no more project clone on every frame, O(1) reads; `ProjectHandle`/`ProjectWriteGuard` for safe writes.
- **egui GUI fully rewritten**: unified theme, dedicated widgets (drag/reset knobs, dB faders, VU meters, velocity-sensitive pads, step cells), IDEAS view (scenes × patterns + Song), MIXER view 8 strips + master, SAMPLER (waveform, ADSR, filter), BROWSER (808 presets, preview, loading), MK3 hardware rail with screen mirror and soft keys.
- `render_screens` shared between the USB display thread and the GUI mirror: the physical screens and the window display strictly the same pair of views.

### Fixed
- Transport: Play/Stop/REC (stop is no longer a no-op), restart from zero (`EngineEvent::Restart`).
- Screen encoders: slots now display the parameters actually controlled by the 8 knobs (TEMPO/SWING/VOL PAN per group, VOL/PAN/CUT/RESO/ADSR per Sound, volumes A–H in MIXER, BROWSER navigation...).
- Sequencer grid (bars/steps), dB fader mapping, audio settings window fields (non-deprecated `ComboBox`).

## [0.2.1] - 2026-10-14

### Fixed
- **4-D encoder**: fixed the HID report `0x01` byte 1 bitmask in `maschine-proto` for the 4 cardinal directions (Up = bit 2, Right = bit 3, Down = bit 4, Left = bit 5).
- **One-shot vs Loop**: replaced the accidental `PlayPauseToggle` action on `EncoderPush` with a one-shot preview of the active sound, avoiding unintended triggering of the transport loop.
- **Multi-bar step sequencer**: full support for 1, 2, 4 and 8-bar patterns with smooth navigation on the controller (`<` / `>` keys, 4D left/right arrows, screen buttons 5..8) and in the GUI.
- **Step sequencer playhead cursor**: time cursor computed relative to the actual pattern length (`modulo pattern_bars`) rather than restricted to the first 2 bars.
- **Silent step placement**: in Step mode, pressing a pad toggles the cell without emitting stray sound or disturbing the preview.
- **Code quality & Clippy**: replaced approximate constants with `std::f32::consts::FRAC_1_SQRT_2`, removed redundant casts, fixed documentation warnings and indexing loops.

### Added
- Automated unit tests for the 4 directions of the 4D encoder (`decode_01_four_way_directions`).
- Unit tests for bar selection and navigation (`bar_navigation_and_pattern_length`, `step_sequencer_multi_bar_toggle`, `four_way_encoder_and_bar_selection`).
- Expanded documentation for the hardware/software workflow and application usage.

## [0.2.0] - 2026-10-14

### Added
- **808 drum synth generator**: 16 percussion instruments synthesized in real time (Kick, Snare, Hi-Hats, Clap, Toms, Rimshot, Shaker, Cowbell, Crash, Perc, Synth Stab, Sub Bass).
- **MK3 dual LCD rendering (480×272)**: live views for Arranger, DAW / Step Grid, Sampler & ADSR Envelopes, 8-group Mixer and Browser.
- **Controller play modes**: Pad Mode, Step Sequencer, Keyboard (scales), Chords (diatonic chords).
- **Scene and pattern management**: SCENE, PATTERN, SELECT, ERASE modes directly on the hardware with color LED feedback.
- **Native egui GUI**: transport panel, full 16-track sequencer, 4×4 pad grid, sound editor and filter parameters.

### Added

- Rust workspace (`maschine-proto`, `maschine-hardware`, `maschine-engine`, `maschine-app`)
- `maschine-proto`: HID codec `0x01`/`0x02` + `0x80`/`0x81` + display bulk RGB565 RLE + indexed palette, full tests
- `maschine-hardware`: `hidraw` discovery + 1ms poll thread + pad hysteresis + `HardwareMapper` (pads → MIDI NoteOn/Off, Groups, Transport, knobs CC, touchstrip), `BulkHandle` (`usb` feature)
- `maschine-engine`: `Project` model (Groups 16 Sounds, Patterns, Scenes, Sections), `Transport` (PPQ 960, BPM, swing, loop), `Sequencer` (quantize, nudge, humanize, slice), `Sampler` (zones, choke, ADSR, stereo render), `Engine` (lock-free snapshot)
- `maschine-app`: CLI (`probe`, `hid-dump`, `daemon`, `project new/show/validate`, `led-test`, `gui` stub), headless daemon + heartbeat + autosave
- Configs: `99-maschine-mk3.rules` (udev `TAG+="uaccess"`), PipeWire `99-maschine-lowlatency.conf` (quantum 128), WirePlumber `51-maschine-mk3.conf/.lua` (pro-audio)
- Docs: `README.md`, `docs/architecture.md`, `docs/protocol.md`, `docs/workflow.md`, `docs/INSTALL.md`, `docs/USAGE.md`
- GitHub Actions CI (fmt, clippy, test, build), `rustfmt.toml`, `scripts/install.sh`
- Example `examples/demo.machine-linux.json`

### Tested

- MK3 `17cc:1600` on Fedora 44, kernel 7.1.10, PipeWire 1.6.8
- `probe` detects `hidraw3` + ALSA `card 2 MK3`
- `hid-dump` + `led-test` OK (hidraw write 0x80/0x81)
- `daemon` connects, maps pads/buttons, advances transport, renders sampler
- `cargo test --workspace` 31 tests pass
