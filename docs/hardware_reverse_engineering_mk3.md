# Guide Complet de Rétro-Ingénierie Hardware : Native Instruments Maschine MK3

Ce document détaille l'architecture matérielle, le protocole USB/HID, le fonctionnement des écrans et le mapping complet des commandes de la **Native Instruments Maschine MK3** (USB VID `0x17CC`, PID `0x1600`). Il sert de référence technique pour les développeurs souhaitant explorer, maintenir ou étendre le projet `maschine-linux`.

---

## 1. Topologie USB et Interfaces

La Maschine MK3 est un périphérique composite USB 2.0 High-Speed composé de 7 interfaces :

| Interface | Rôle | Endpoints | Spécifications |
|---|---|---|---|
| **IF 0** | Audio Control | - | Gestion des volumes et de la synchronisation d'horloge |
| **IF 1** | Audio Streaming OUT | EP `0x01` OUT | 4 canaux (Main L/R + Casque L/R), 24-bit S32LE, 44.1 / 48 / 88.2 / 96 kHz |
| **IF 2** | Audio Streaming IN | EP `0x82` IN | 2 canaux (Line In / Mic In), 24-bit S32LE |
| **IF 3** | MIDI Streaming | EP `0x02` OUT, EP `0x81` IN | Ports MIDI DIN 5 broches physiques In & Out |
| **IF 4** | HID Contrôleur | EP `0x83` IN, EP `0x03` OUT | Boutons, pads, potentiomètres, bande tactile et LEDs |
| **IF 5** | Écrans couleur | EP `0x04` OUT (Bulk) | Double écran LCD 480×272 pixels en RGB565 |
| **IF 6** | DFU (Firmware Update) | - | Mise à jour du microprogramme interne |

---

## 2. Protocole HID IN (Contrôleur vers Hôte — IF 4, EP `0x83`)

La transmission des actions physiques se fait via deux types de rapports d'interruption :

### Rapport `0x01` — Boutons, Encodeurs, Pots & Touchstrip (42 octets)

- **Octet 0** : ID du rapport (`0x01`).
- **Octets 1 à 10** : Masques binaires des boutons (`1` = enfoncé, `0` = relâché, bit LSB en premier) :
  - **Octet 1** : `[Encoder Push, Pédale Footswitch, 4-Way Haut, 4-Way Bas, 4-Way Gauche, 4-Way Droite, SHIFT, Display Button 8]`
  - **Octet 2** : Boutons de Groupes `A, B, C, D, E, F, G, H`
  - **Octet 3** : `[Notes, Volume, Swing, Tempo, Note Repeat, Lock, inutilisé, inutilisé]`
  - **Octet 4** : `[Pad Mode, Keyboard, Chords, Step, Fixed Velocity, Scene, Pattern, Events]`
  - **Octet 5** : `[Variation, Duplicate, Select, Solo, Mute, Pitch, Mod, inutilisé]`
  - **Octet 6** : `[Perform, Restart, Erase, Tap, Follow, Play, Record, Stop]`
  - **Octet 7** : `[Macro, Settings, Right, Sampling, Mixer, Plugin, inutilisé, inutilisé]`
  - **Octet 8** : `[Channel/MIDI, Arranger, Browser, Left, File/Save, Auto, inutilisé, inutilisé]`
  - **Octet 9** : `Display Buttons 1 à 7` + `Touch Encodeur 4D`
  - **Octet 10** : Contact capacitif (Touch) des 8 encodeurs rotatifs (`Knob Touch 8 à 1`)
- **Octet 11** : Encodeur rotatif 4D principal (poids faible 4 bits, modulo 16 avec gestion de défilement)
- **Octets 12 à 27** : 8 encodeurs rotatifs continus (valeurs absolues 12-bit, Little Endian, $0$ à $1023$)
- **Octets 28 à 35** : Bande tactile Smart Touchstrip (positions et pression sur 2 doigts)
- **Octets 36 à 41** : Potentiomètres analogiques absolus 10-bit (Gain Micro, Volume Casque, Volume Master)

### Rapport `0x02` — Pression des 16 Pads (64 octets)

- **Octet 0** : ID du rapport (`0x02`).
- **16 slots de 3 octets** : `[index_pad, data_hi, data_lo]`
  - `raw_value = (data_hi << 8) | data_lo`
  - Masque de validité : `(raw_value & 0xF000) == 0x4000`
  - Seuil d'activation (`pad_on_threshold`) : `0x4010` (valeur par défaut avec hystérésis)
  - Seuil de relâchement (`pad_off_threshold`) : `0x4005`
  - Ordre de scan hardware : de haut-gauche à bas-droite.

#### Permutation Hardware vs Logique des Pads
La disposition logique Maschine numérote le **Pad 1 en bas à gauche** (index 0) et le **Pad 16 en haut à droite** (index 15).
Le capteur physique scanne la matrice par le haut. Une table de permutation auto-inverse est appliquée :
```
Hardware : [ 0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15]
Logique  : [12, 13, 14, 15,  8,  9, 10, 11,  4,  5,  6,  7,  0,  1,  2,  3]
```

---

## 3. Protocole HID OUT (Hôte vers Contrôleur — IF 4, EP `0x03`)

### Rapport `0x80` — LEDs des Boutons (63 octets)
- **Octet 0** : ID du rapport (`0x80`).
- **Octets 1 à 62** : État de luminosité pour chaque LED individuelle :
  - `0` = Éteinte (Off)
  - `4` = Atténuée (Dim)
  - `5` = Très atténuée (Very Dim)
  - `6` = Allumée normale (On)
  - `7` = Pleine luminosité (Bright)
  - Pour les boutons RGB (Groupes A..H) : index de la palette couleur Native Instruments.

### Rapport `0x81` — LEDs de la Bande Tactile et des 16 Pads (42 octets)
- **Octet 0** : ID du rapport (`0x81`).
- **Octets 1 à 25** : 25 LEDs RVB de la bande tactile (de gauche à droite).
- **Octet 26** : Réservé.
- **Octets 27 à 42** : 16 LEDs RVB des pads (ordre hardware `13-16, 9-12, 5-8, 1-4`), encodées via la palette indexée.

---

## 4. Protocole d'Écran Bulk (Double LCD 480×272 — IF 5, EP `0x04`)

Chaque écran couleur (Gauche et Droit) affiche $480 \times 272$ pixels au format 16-bit **RGB565 Big-Endian** (sans canal alpha).
Les trames sont transmises en blocs USB Bulk sur l'endpoint `0x04` :

```
En-tête de fenêtre (16 octets) :
[0x84, 0x00, ecran_id, 0x60, 0x00, 0x00, 0x00, 0x00, X_hi, X_lo, Y_hi, Y_lo, W_hi, W_lo, H_hi, H_lo]
  ecran_id : 0x00 (Gauche) / 0x01 (Droit)
  X, Y, W, H : Coordonnées et dimensions du rectangle rafraîchi en Big-Endian.

Commande de transmission (4 octets) :
[0x00, nb_paires_hi, nb_paires_mid, nb_paires_lo]
  nb_paires = (W × H) / 2

Données pixels :
(W × H × 2) octets de données brutes RGB565.
```

Un moteur différentiel (`display_differ`) calcule en temps réel les rectangles modifiés entre deux images consécutives afin d'optimiser la bande passante USB et garantir 60 FPS sans latence.

---

## 5. Table Complète du Mapping des Boutons et Raccourcis SHIFT

| Bouton Physique | Action Normale | Action avec SHIFT |
|---|---|---|
| **PLAY** | Lance la lecture audio | Redémarre au début de la mesure / boucle |
| **RECORD** | Active l'enregistrement de notes en temps réel | Active le décompte initial (Count-In) |
| **STOP** | Arrête la lecture et coupe les voix | Coupe immédiatement toutes les notes (Panic) |
| **RESTART** | Revient au tick 0 (début du projet/pattern) | Active ou désactive la lecture en boucle (Loop) |
| **ERASE** | Mode suppression rapide de notes | Supprime le pattern ou le son ciblé |
| **TAP** | Détermine le BPM par frappes consécutives | Active / désactive le Métronome |
| **FOLLOW** | Suit la tête de lecture sur les pages de grille | - |
| **PAD MODE** | Jeu en mode 16 pads drums / one-shot | - |
| **KEYBOARD** | Mode mélodique chromatique / gammes | Sélectionne la fondamentale (Root) ou la gamme |
| **CHORDS** | Déclenchement d'accords pré-programmés | Sélectionne la typologie d'accords et l'inversion |
| **STEP** | Mode Step-Sequencer 16 pas | Raccourcis d'édition de grille (Vider, Quant, Human, Nudge, Octave, Fill) |
| **NOTE REPEAT** | Répétition automatique rythmée à l'enfoncement | Verrouille la répétition (Latch / Lock) |
| **LOCK** | Capture un snapshot d'état temporaire | Restaure l'état mémorisé |
| **MUTE** | Coupe le son du pad ou groupe sélectionné | - |
| **SOLO** | Isole le son du pad ou groupe sélectionné | - |
| **SELECT** | Sélectionne un son sans le déclencher | - |
| **DUPLICATE** | Duplique le pattern ou la mesure active | - |
| **VARIATION** | Génère une variation aléatoire ou humanisée | - |
| **SCENE** | Sélection et déclenchement des scènes | Gestion des sections de la timeline |
| **PATTERN** | Sélection et chaînage de patterns (multi-pad) | Double / divise la longueur du pattern actif |
| **MIXER** | Bascule sur l'onglet de mixage général | Inverse l'action des encodeurs vers les panoramiques |
| **SAMPLING** | Ouvre l'éditeur de samples et l'enveloppe ADSR | Raccourcis de traitement (Reverse, Loop, Normalisation, Filtres) |
| **PLUGIN** | Ouvre le synthétiseur chargé (Kick, 303, Nexus) | Accède à la page secondaire d'effets et modulations |
| **BROWSER** | Explorateur de kits, presets et plugins | Ouvre le gestionnaire de projets (FILE) |

### Raccourcis Écran (Soft Keys 1 à 8) selon le Contexte

#### En mode PATTERN avec SHIFT :
1. `DOUBLE` : Double la longueur de la grille.
2. `HALF` : Divise par deux la longueur de la grille.
3. `QUANT` : Quantifie les événements sur la grille 16e.
4. `HUMAN` : Applique une humanisation subtile de groove.
5. `VIDER` : Efface le contenu du pattern actif.
6. `SUPPR` : Supprime le pattern du groupe.
7. `4 MES` : Définit la taille à 4 mesures.
8. `8 MES` : Définit la taille à 8 mesures.

#### En mode STEP avec SHIFT :
1. `VIDER` : Supprime les événements de la mesure courante.
2. `QUANT` : Quantifie le pattern.
3. `HUMAN` : Humanise le pattern.
4. `NUDG -` : Décale les notes d'un pas vers la gauche.
5. `NUDG +` : Décale les notes d'un pas vers la droite.
6. `OCT -` : Transpose les notes d'une octave vers le bas (-12 demi-tons).
7. `OCT +` : Transpose les notes d'une octave vers le haut (+12 demi-tons).
8. `FILL` : Remplit les 16 pas de la mesure pour le son sélectionné.

#### En mode SAMPLER (Tab 2) & PLUGIN (Tab 4 pour sons Sampler) :
- **Sans SHIFT** : `SND <` (Son précédent), `SND >` (Son suivant), `ECOUTE` (Prévisualisation), `CYC FLT` (Cycle mode filtre : LP -> HP -> BP -> OFF), `LP` (Passe-bas direct), `HP` (Passe-haut direct), `BP` (Passe-bande direct), `OFF` (Filtre désactivé).
- **Avec SHIFT** : `REVERSE` (Inversion temporelle du sample), `LOOP` (Bascule One-Shot / Boucle), `NORMLZ` (Normalisation crête à 0 dBFS), `CYC FLT` (Cycle filtre), `LP`, `HP`, `BP`, `OFF` (Sélection directe du type de filtre).

#### Raccourcis Universels de Filtrage & Encodeur 4D :
- **SHIFT + Push Encodeur 4D** : Bascule instantanément le type de filtre (Cycle LP -> HP -> BP -> OFF) du son actif dans TOUTES les vues sans changer d'écran.
- **SHIFT + Tourne Encodeurs Cutoff/Reso** : Tourner l'encodeur de coupure ou résonance avec SHIFT enfoncé alterne immédiatement le mode de filtrage (en vue Arranger knobs 6/7, en vue Sampler/Plugin knobs 3/4).

#### En mode KICK SYNTH (Tab 4) :
- **Sans SHIFT** : `DRV TYP` (Type de saturation analogique), `FLT MOD` (Cycle mode filtre multimode), `TECHNO`, `909`, `HARDCORE`, `808 SUB`, `INDUSTRIAL` (Presets de kick directs), `BROWSER` (Explorateur).
- **Avec SHIFT** : `DRV TYP`, `FLT MOD`, `LP`, `HP`, `BP`, `OFF` (Sélection directe du mode de filtre), `PRESETS`, `BROWSER`.

#### En mode SYNTH WAVETABLE / NEXUS (Tab 4) :
- **Knobs 1 à 8** : Cutoff, Résonance, Modulation Enveloppe, Drive, Attaque, Déclin, Maintien (Sustain), Relâchement (Release).
- **Avec SHIFT (Boutons 1 à 8)** : Mode de filtre (LP12/LP24/HP/BP), Nombre de voix d'unisson, Forme d'onde Osc 1, Forme d'onde Osc 2, Mix Chorus, Mix Delay, Mix Reverb, Volume principal du synthétiseur.

#### Structure des 8 Groupes (A à H) et 128 Pistes :
Le projet initialise nativement 8 groupes indépendants (`Groupes A..H`), chacun doté de 16 pads/sons distincts (128 pistes au total) :
- **Groupe A** : `Kit 808` complet (Kick, Snare, HiHats, Clap, Toms, Percs, Acid 303 sur Pad 15).
- **Groupe B** : `Kit 909` (909 Drums analogiques, Kick Modeler sur Pad 15).
- **Groupe C** : `Nexus Synth` (Wavetable Synth sur Pad 1, leads, plucks, pads, strings).
- **Groupe D** : `Bass & Acid` (Presets Acid TB-303 multiples sur Pads 1..3, sub bass, basslines).
- **Groupe E** : `Percussion` (Congas, bongos, timbales, claves, shakers, tambourines).
- **Groupe F** : `Keys & Chords` (Pianos acoustiques/électriques, Rhodes, orgues, stabs harmoniques).
- **Groupe G** : `Vocal & SFX` (Chants, risers, sub drops, reverse cymbals, bleeps).
- **Groupe H** : `Hits & Stabs` (Rave stabs, disco hits, dub chords, cuivres).

Chaque pad dispose d'une piste dédiée avec son propre canal dans le sampler, son enveloppe ADSR, son filtre multimode, son panoramique et son volume indépendants.

---

## 6. Architecture Logicielle et Directives pour les Développeurs

Le dépôt est découpé en crates modulaires avec séparation stricte des responsabilités :

```
maschine-linux/
├── crates/
│   ├── maschine-proto/      # Codec USB pur sans I/O : décodage HID IN 0x01/0x02, encodage LED 0x80/0x81, protocole Display
│   ├── maschine-hardware/   # Gestion bas-niveau des périphériques : hidraw, mapper matériel, gestion LED et filtrage anti-rebond
│   ├── maschine-dsp/        # Algorithmes audio temps réel : filtres SVF biquad, enveloppes ADSR, générateurs d'ondes
│   ├── maschine-engine/     # Cœur métier : séquenceur, échantillonneur, synthétiseurs (Kick, Acid, Wavetable), transport, projet
│   └── maschine-app/        # Binaire principal : interface graphique egui/eframe, threads audio ALSA/JACK, updater, boucle d'événements
```

### Règles d'Or du Développement :
1. **Zéro allocation dans la boucle audio** : Tout rendu dans `render_audio()` ou `render()` doit être lock-free et pré-alloué.
2. **Synchronisation d'affichage par événement** : Toute action utilisateur incrémente un numéro de `revision` qui déclenche le repaint minimal nécessaire.
3. **Tests Hardware Découplés** : Le module `maschine-hardware` possède des tests unitaires complets émulant les trames HID brutes sans nécessiter le contrôleur physique branché.
4. **Tenue de note pad (Sustain)** : Le suivi de pression dans `hidraw.rs` isole les défauts de capteurs au repos (bruit fantôme < 0x4280) des appuis intentionnels (> 0x4280), garantissant que les synthétiseurs jouent en continu tant que le pad reste enfoncé.
5. **Redémarrage Propre de l'Application** : La routine `restart_app()` nettoie automatiquement les variables d'environnement héritées d'AppImage (`APPDIR`, `ARGV0`), restaure le répertoire de travail d'origine `OWD` et détache le nouveau processus via `nohup` pour une réouverture garantie.

---

## 7. Navigation 4D, Hiérarchie de Vues et Interactions Spéciales

L'encodeur 4D réunit un joystick 4 axes (`Haut`, `Bas`, `Gauche`, `Droite`), un encodeur rotatif sans fin et un bouton poussoir (`Encoder Push`), créant une ergonomie contextuelle sans quitter les mains du contrôleur :

### Hiérarchie de Navigation par Vue :

| Contexte / Vue | Pousser Haut | Pousser Bas | Pousser Gauche | Pousser Droite | Appui (Push) | SHIFT + Push |
|---|---|---|---|---|---|---|
| **ARRANGER / PATTERN** | Son suivant (preview) | Son précédent (preview) | Mesure précédente (`SelectBar`) | Mesure suivante (`SelectBar`) | Preview du pad actif | Cycle filtre multimode |
| **PATTERN SELECT** | - | - | Pattern précédent | Pattern suivant | Valide la sélection | Cycle filtre multimode |
| **STEP SEQUENCER** | Son suivant | Son précédent | Défilement pas à gauche | Défilement pas à droite | Preview du pad actif | Cycle filtre multimode |
| **MIXER (Vue Groupes)** | Entre dans le groupe (pads 1..8) | - | Groupe précédent | Groupe suivant | Alterne vue pads / vue groupes | Cycle filtre multimode |
| **MIXER (Vue Pads)** | - | Sort vers vue Groupes | Banque Pads 1..8 | Banque Pads 9..16 | Alterne vue pads / vue groupes | Cycle filtre multimode |
| **BROWSER (Presets)** | Preset précédent (`-1`) | Preset suivant (`+1`) | Catégorie précédente | Catégorie suivante | Charge le preset sélectionné | Cycle filtre multimode |
| **FILE BROWSER** | Projet précédent | Projet suivant | - | - | Charge le projet sélectionné | Cycle filtre multimode |

### Priorité MUTE / SOLO :
- **SOLO actif / maintenu** : La combinaison `SOLO + Group(A..H)` est prioritaire sur tout état `MUTE` latched en arrière-plan. Elle isole le groupe ciblé sans basculer l'écran ni perdre le pad en cours d'édition.
- **MUTE latched (`mute_pad_mode`)** : L'appui court sur le bouton physique `MUTE` active le mode pad mute. Enfoncer `Group(A..H)` dans cet état bascule le mute complet du groupe.

### Bande Tactile Smart Strip :
Quatre modes physiques commutables par les boutons dédiés au-dessus du ruban :
1. **PITCH** : Pitch-bend avec retour automatique au centre (centre = pas de transposition).
2. **MOD** : Molette de modulation MIDI CC 1 absolue.
3. **PERFORM** : Macro-contrôle d'effets temps réel (sweep de filtre, écho, distorsion).
4. **NOTES** : Strumming / arpégiateur de pads ou accords par glissement de doigt.

---

## 8. Gestion USB, Cycle de Vie Hardware et Déconnexion

Pour garantir la stabilité sur les noyaux Linux et éviter les resets USB intempestifs :
1. **Découverte automatique** : L'application balaie `/sys/class/hidraw` pour repérer le couple `VID=0x17CC` et `PID=0x1600` sans exiger de permissions root via une règle `udev` standard (`60-maschine.rules`).
2. **Initialisation progressive** :
   - Émission des trames LED `0x80` et `0x81` éteintes.
   - Test de transmission d'un rectangle $2 \times 1$ RGB565 pour purger le pipeline de l'interface Bulk IF 5.
   - Démarrage du thread de lecture d'interruption HID avec lecture non-bloquante via `poll` / timeout 15 ms.
3. **Arrêt Propre (Graceful Release)** :
   - À la fermeture, un écran splash avec le logo Maschine natif est envoyé.
   - L'interface Bulk USB est libérée via `libusb_release_interface` sans émettre d'appel `USBDEVFS_RESET` qui déconnecterait la carte son USB audio intégrée.

---

## 9. Guide de Contribution : Ajouter et Tester un Nouveau Contrôle

Pour ajouter une nouvelle interaction matérielle :
1. **Définition dans le codec** : Ajouter ou valider le `ButtonId` dans `crates/maschine-proto/src/hid_input.rs`.
2. **Action Mapper** : Déclarer la variante d'action dans `MapperAction` (`crates/maschine-hardware/src/mapper.rs`).
3. **Routage modulaire** : Dans `mapper.rs`, brancher l'événement dans la fonction dédiée (`handle_transport_button`, `handle_view_menu_button`, `handle_4way_button`, ou `handle_display_button`).
4. **Consommation Engine** : Traiter l'événement correspondant dans `Engine::handle_event()` (`crates/maschine-engine/src/engine.rs`).
5. **Test Unitaire Hardware** : Écrire un test dans `mapper::tests` simulant l'envoi de `HardwareEvent::Button` ou `HardwareEvent::Pad` et vérifiant que le vecteur de `MapperAction` généré correspond exactement à l'attente.
