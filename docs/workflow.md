# Workflow Maschine — modèle reproduit dans maschine-engine

Réplique fidèle de Maschine 2/3 (NI) adapté à Rust, sans les lourdeurs.

## Hiérarchie

```
Project
 ├─ master : Channel (vol, pan, inserts FX)
 ├─ groups: Vec<Group>  // A-H × N banks, extensible, 8 visibles à la fois sur hardware
 ├─ scenes: Vec<Scene>
 ├─ song  : Vec<Section>  // timeline arranger
 └─ tempo: f32, swing: f32, time_signature
```

```
Group (16 pads fixes, comme hardware)
 ├─ sounds: [Sound; 16]  // Some(Sound) ou None (gap autorisé)
 ├─ channel: Channel     // inserts Group
 ├─ patterns: Vec<Pattern>  // pool propre au Group
 └─ color: u8, name: String, muted/solo
```

```
Sound
 ├─ chain: Vec<PluginSlot>  // Slot0 = Instrument (Sampler/Audio/External), Slot1..N = FX
 ├─ channel: Channel
 ├─ zones: Vec<Zone>        // multi-sample mapping
 ├─ choke_group: Option<u8> // off_by (hi-hat)
 └─ midi: MidiInputMode (Off/Focus/Omni)
```

```
Pattern (appartient à UN Group)
 ├─ events: Vec<Event>  // note, vel (alpha), start ticks, length ticks
 ├─ length: BarsBeatsTicks
 ├─ swing: f32, grid: Grid (1/4, 1/8T, 1/16, etc.)
 └─ color
```

```
Scene : Vec<Option<PatternId>> // un pattern par Group max, None = silence
Section : { scene_id, length, name } // length peut être < scene length (trim)
```

Réplique exacte : Pattern = objet référencé (éditer propage partout) vs Clip (Song-only, future). Scenes déclenchables via pads en Ideas view ou MIDI Note/ProgramChange.

## Vues de l'application

La GUI reprend la séparation du workflow Maschine : une barre de transport commune, les groupes et scènes à gauche, puis un espace de travail dédié. Les boutons de vue restent synchronisés avec les onglets matériels MK3.

- **Ideas** : scènes horizontales et matrice Group × Pattern. Cliquer un pattern l'assigne à la scène et met son groupe au focus. La scène peut être dupliquée/supprimée (avec ré-indexation automatique des sections du Song). Une scène peut être ajoutée depuis cette vue.
- **Song** : timeline horizontale des sections, sélection directe, navigation précédente/suivante, duplication, suppression et longueur de section (1/2/4/8 mesures).
- **Sequencer** : pas 16 × 16, navigation et longueur de pattern de 1/2/4/8 mesures, pads de pré-écoute et actions de groove. Les cellules sont préparées en une passe sur les événements pour garder l'édition fluide.
- **Mixer** : volume, panoramique, mute et solo des groupes A–H, plus le master.
- **Sampler / Sound** : sélection des 16 Sounds, import WAV asynchrone, waveform, audition, volume, panoramique, filtre et enveloppe ADSR. Les chemins WAV sont enregistrés dans les zones du projet et rechargés à l'ouverture.
- **Browser** : presets de drum synth 808 intégrés, chargés sur le Sound sélectionné. Le Browser ne parcourt pas encore la bibliothèque NI ni le système de fichiers.

Le transport propose Play/Stop, Record, boucle, tempo et position BBT. Les pads GUI envoient le relâchement de note; les presets 808 sont mis en cache au chargement du kit.

## Écrans MK3 (480×272) & soft keys

Les deux écrans sont rendus par la même fonction (`display_render::render_screens`) : le panneau MK3 de la GUI est le miroir du contenu calculé pour le hardware. La boucle USB compare les rendus à 30 Hz, envoie seulement les rectangles modifiés (jusqu'à six), limite le débit moyen à 1,6 Mo/s et renvoie une paire plein écran toutes les 2 secondes pour resynchroniser le contrôleur.

- **Écran gauche** : ARRANGE (8 groupes, pattern de la scène, VU réels), SCENES/IDEAS (mode SCENE), SONG (sections), MIXER A–D, SAMPLER (waveform + ADSR + infos), BROWSER (presets 808), STEP (grille du pattern complet + paramètres du pas sélectionné).
- **Écran droit** : PATTERN (grille 16 sons × 16 pas + playhead), STEP (vélocités du Sound actif + curseur d'édition), SONG TIMELINE, MIXER master + groupe/sound focus, SAMPLER enveloppe plein écran, BROWSER chargement, EVENTS, MACRO, SETTINGS.
- **Soft keys (8 boutons sous les écrans)** : action *contextuelle* décidée par le moteur (`EngineEvent::SoftKey`) et affichée en bas des écrans. Le bouton fait toujours ce que le label annonce, identiquement au hardware et dans la GUI.
  - ARRANGE : `BAR 1..4` + longueur de pattern `1/2/4/8 MES`.
  - STEP : `PAS <</>>` (curseur d'un pas), `BAR <</>>` (d'une mesure), `1/2/4/8 MES`.
  - MIXER : `GRP A..H` (focus groupe). SAMPLER : `SND 1..8` (Sound actif).
  - BROWSER : `PRES <</>>`, `SND <</>>`, `ECOUTE`, `CHARGER`, `METRO`, `LOOP`.
  - SCENE : `SCN 1..8`. SONG : `SEC <</>>`, `+SEC`, `IDEE`, scènes et `+SCN`.
- **Encodeurs** : chaque écran affiche les 4 paramètres réellement pilotés par ses knobs (1–4 à gauche, 5–8 à droite) avec les valeurs live.
- **Navigation entre vues** : choisir ARRANGE, MIXER, SAMPLER ou BROWSER remet l'écran droit sur la page liée à cette vue. EVENTS, MACRO et SETTINGS sont des vues de droite que l'on peut ensuite choisir explicitement. Recliquer la vue principale active dans la GUI revient à sa page droite contextuelle.
- **GUI et commandes hardware synchronisées** : avant de mapper chaque nouvel événement HID, l'application reprend le groupe, le son et les onglets du moteur. Le premier cran de knob après un changement de page fait donc déjà l'action de la nouvelle page.
- **Édition en direct** : les changements de volume, pan, filtre et ADSR atteignent aussi les voix déjà en cours; les gains et panoramiques Group/Sound se combinent sans écraser le niveau voisin.
- **Alignement LCD** : chaque rectangle USB doit avoir une largeur paire, car le compteur Bulk représente des paires de pixels RGB565. Les rectangles du différentiel sont élargis au pixel voisin; les fenêtres impaires sont refusées avant l'envoi. Voir [le protocole matériel](HARDWARE_DISPLAY_PROTOCOL.md).
- **Validation réelle** : les tests vérifient l'encodage, le rendu et le routage des contrôles. Un transfert USB accepté ne prouve pas que les pixels sont visibles; contrôler les deux LCD sur la MK3 après le lancement.

### Référence officielle et périmètre actuel

Le manuel MK3 décrit SAMPLING comme un Sample Editor lié au Sound focalisé : les boutons 1–4 choisissent RECORD, EDIT, SLICE et ZONE; EDIT règle notamment le play range, tandis que ZONE gère mapping, accordage et plages de touches/vélocité. Les huit boutons et knobs du contrôleur sont contextuels et leur fonction est affichée à l'écran ([manuel officiel, Sampling et sample mapping](https://docs.native-instruments.com/ni-tech-manuals/maschine-mk3-manual/en/sampling-and-sample-mapping), [référence rapide MK3](https://docs.native-instruments.com/ni-tech-manuals/maschine-mk3-manual/en/quick-reference)).

Dans cette application, le bouton SAMPLING ouvre actuellement **Sampler / Sound** : import WAV depuis la GUI, waveform, audition, ADSR, filtre, volume et pan. L'import est décodé sur un thread de travail. Les pages d'enregistrement, d'édition destructive et de slicing, les play/selection ranges manipulables et l'édition des zones ne sont pas encore implémentés sur le contrôleur. L'écran EVENTS actuel est lui aussi une vue d'information; il ne propose pas encore l'édition événementielle position/hauteur/vélocité/longueur du mode officiel ([description officielle du mode EVENTS](https://docs.native-instruments.com/ni-tech-manuals/maschine-mk3-manual/en/quick-reference)).

## Mode STEP (façon logiciel officiel)

Le mode STEP s'active avec le bouton **STEP** du hardware (ou la GUI). L'onglet gauche reste sur ARRANGE : la page STEP remplace alors ARRANGE sur les écrans.

- **Pads 1..16** : basculent le pas correspondant du Sound actif dans la mesure affichée (silencieux, doublon supprimé via `same_step_cell`). Le pad touché devient le **pas sélectionné** : la grille des écrans et le curseur LED le suivent.
- **Curseur (`Engine::step_cursor`)** : pas sélectionné (index absolu 0..`pattern_bars*16-1`). Il détermine la mesure affichée (`selected_bar = curseur / 16`) et la fenêtre des 4 knobs d'édition.
- **Knobs 1–4 = édition du pas sélectionné** : `VEL` (vélocité), `POS` (nudge en ticks), `LEN` (longueur en pas de 1/16), `PITCH` (demi-tons). Un accumulateur côté mapper transforme la rotation continue en incréments entiers (~1 cran ≈ 32 ticks bruts).
- **Knob 5 = SCROLL** : défile la grille du pattern complet (32 pas pour 2 mesures, jusqu'à 128 pas) ; les knobs 6–8 restent `VOL SND`, `CUTOFF`, `RESO` du Sound actif.
- **Encodeur 4-D `<` / `>` et flèches** : défilent la grille d'un pas (le mode STEP prend la main sur la navigation par mesure).
- **Écran gauche** : grille du pattern complet (une ligne par mesure, 16 cases), case du curseur encadrée en orange, tête de lecture blanche, détail du pas (`VEL`, `POS`, `LEN`, `PITCH` avec nom de note) et slots 1–4.
- **Écran droit** : grille détaillée de la mesure (hauteur = vélocité), contour orange sur le pas édité, `PAS n/n` dans le bandeau, slot 5 = `SCROLL`, 6–8 = `VOL SND`, `CUTOFF`, `RESO`.

## Instrumentation latence

`maschine-linux daemon --debug-latency` (ou `gui --debug-latency`) journalise toutes les 2 s :

- latence **HID→Engine** : nombre d'événements, dernier et max en µs par fenêtre ;
- statistiques audio : callbacks, **taille de buffer réelle** en frames (et ms), `lock_misses` (callback n'ayant pas pu prendre le lock → silence), `err` (erreurs/underruns backend) et temps max de callback en µs.

`maschine-linux daemon --debug-latency --stress` ajoute une charge volontaire (lecture du pattern + verrou moteur et snapshots pris en continu) : c'est le révélateur des craquements. Référence mesurée (PipeWire `default`, 48 kHz) : **1500 callbacks/2 s (buffer 64 frames ≈ 1,3 ms), 0 miss, 0 err** avec 61 voix et la contention.

Le thread HID réveille immédiatement la GUI (`RepaintSignal`), et le snapshot moteur partage le projet en `Arc` (copy-on-write) : les lectures UI sont O(1) et seules les actions utilisateur clonent.

## Qualité audio & anti-craquements

Chaîne de sortie (dans l'ordre) : voix → groupes (gains/pans/filtres) → **limiteur doux master** (linéaire sous -3 dB puis `tanh`, jamais d'écrêtage dur) → callback (clamp de sécurité ±1.0).

- **Latence** : le stream est négocié au plus court — 48 kHz si accepté, buffer 256 frames (période 64), replis 512 → 1024 → 2048 → défaut. `set_audio_sample_rate()` régénère le kit DSP au taux réel du device à chaque ouverture (sinon le kit joue faux).
- **Timing** : le callback avance le transport du bloc complet puis rend ; chaque note est placée au **sample près** dans le bloc (`start_frame`), donc le groove ne dépend pas de la taille de buffer.
- **Anti-clic** : fondu de fin de sample (3 ms) + fondu de fin dans le drum synth, vol de voix et choke en fondu 5 ms, clics métronome fondus.
- **Diagnostic hors-ligne** : `cargo run --release -p maschine-engine --example render_groove -- /tmp/g.wav 12 48000 0.85` (crête, RMS, écrêtés, plus grand saut) et `--example render_kit` (les 16 instruments en WAV). Référence du groove démo : crête ≈0,92, RMS ≈-18 dB, 0 écrêtage, aucun saut anormal aux bords de boucle.

## Sampler

- Moteur : `sampler.rs` — voix polyphonique, `Voice { sample: Arc<[f32]>, pos: f64, gain, pan, adsr: Adsr, filter: Svf }`.
- Zones : `Zone { sample_path, root_key, lokey/hikey, lovel/hivel, seq_len/seq_pos, group, polyphony, tune, pan }`.
- Choke : `HashMap<choke_group, Vec<VoiceId>>`, `off_mode` = kill / fade 5ms.
- Slice : `slice::detect_transients(&[f32]) -> Vec<usize>` puis export `Apply to Group` (chaque slice → nouveau Sound) + auto pattern 16-step reproduisant loop originale.
- Formats : WAV/FLAC/OGG via `hound` + `symphonia` (future), SFZ import/export, Hydrogen drumkit import.

## Sequencer & Transport

- `Transport { bpm, sample_rate, tick, playing, recording, loop_enabled, loop_start, loop_end, swing }`.
- `Sequencer::tick_to_samples(tick)`, `quantize(event, grid)`, `nudge(event, ticks)`, `humanize(vel, timing)`.
- Swing : delay every other 16th par `swing * 0.5 * step_duration`.
- Résolution : 960 ticks / quarter (PPQ 960).
- Multi-mesures : patterns configurables de 1 à 8 mesures (`1:0:0`, `2:0:0`, `4:0:0`, `8:0:0`).
- Séquenceur pas-à-pas silencieux : en mode STEP, la pose d'un pas n'émet aucun son parasite et supprime proprement les doublons via comparaison par cellule de 16e (`same_step_cell`).

## State & persistance

- `Project::save(path)` → JSON pretty + binaire bincode cache.
- `schema_version: u32` pour migrations.
- Instantané UI lock-free : `Arc<RwLock<Arc<Project>>>` (copy-on-write au moment des écritures) + compteur de révision `AtomicU64`. `Engine::snapshot()` est O(1) (clone d'un `Arc`), les VU-mètres et l'enveloppe du Sound actif voyagent dans l'instantané.

## Mapping hardware MK3 complet

- **Pads 1..16** :
  - *Mode PAD* : déclenchement du son correspondant du groupe focus avec vélocité et pression dynamiques. Rendu one-shot sans bouclage accidentel.
  - *Mode STEP* : bascule silencieuse du pas correspondant (0..15) dans la mesure sélectionnée (`selected_bar`) ; le pad devient le pas sélectionné (curseur orange sur les écrans, blanc sur les pads à l'arrêt). Les knobs 1–5 éditent le pas (VEL/POS/LEN/PITCH) et défilent la grille (SCROLL) — voir « Mode STEP ».
  - *Modes SCENE / PATTERN / ERASE / SELECT* : gestion directe du workflow Maschine.
- **Encodeur 4-D** :
  - *Haut / Bas* : changement rapide du son sélectionné (`active_sound`) avec pré-écoute instantanée ; navigation des presets dans le Browser.
  - *Gauche / Droite* (ou `<` / `>`) : navigation entre les mesures (`selected_bar`) — **en mode STEP**, défilent la grille d'un pas.
  - *Shift + Droite* : extension de la longueur du pattern à 4 ou 8 mesures.
  - *Push* : pré-écoute one-shot du son ou validation preset.
  - *Focus TEMPO / VOLUME / SWING* (boutons dédiés) : 0.5 BPM, 0.01 de master, 0.01 de swing par cran (divisés par 2 avec Shift).
- **Écrans & Boutons d'écran 1..8** :
  - *Soft keys contextuels* : les 8 boutons sous les écrans exécutent l'action affichée en bas de l'écran (voir « Écrans MK3 & soft keys »). Même moteur d'action côté hardware et côté GUI.
  - *8 Potentiomètres rotatifs* : édition contextuelle des paramètres du son ou du mixeur ; les slots d'encodeurs des écrans affichent ces mêmes paramètres avec leurs valeurs live.
- **Vitesse des knobs** (mise à l'échelle *une seule fois* dans `main.rs`, pas par tick brut ni par cran ≈ 32 ticks) :
  - ARRANGE : TEMPO 0.05 BPM (≈1.5 BPM/cran, Shift ÷10), SWING 0.001, VOL GRP/SND 0.03 dB, PAN 0.001, MASTER 0.001.
  - CUTOFF : échelle exponentielle `2^(ticks/384)` — ≈2.7 octaves par tour, ≈1 demi-ton par cran (naturel de 20 Hz à 20 kHz).
  - RESO 0.001. SAMPLER : ATTACK 0.5 ms, DECAY/RELEASE 1 ms, SUSTAIN 0.001 par tick.
  - MIXER : 0.03 dB par tick sur les 8 groupes. BROWSER : son/preset par cran (accumulateur), MASTER 0.001.
  - Un tour rapide (delta brut > 128 ticks) est **limité**, jamais jeté : le knob ne « perd » plus de mouvements.

## Exemple JSON minimal

```json
{
  "schema_version": 1,
  "name": "Demo",
  "bpm": 120.0,
  "groups": [{
    "name": "Kit 808",
    "color": 22,
    "sounds": [{ "name": "Kick", "color": 6, "zones": [{"sample": "kicks/808.wav", "root_key": 36}] }, null],
    "patterns": [{ "name": "Pattern 1", "length": "2:0:0", "events": [{"pad": 0, "tick": 0, "vel": 100, "len": 240}] }]
  }],
  "scenes": [{ "name": "Scene 1", "patterns": [0] }],
  "song": [{ "scene": 0, "length": "2:0:0" }]
}
```

Voir `crates/maschine-engine/src/project.rs` pour le schéma complet.
