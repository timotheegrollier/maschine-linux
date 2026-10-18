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
- **Sampler** : sélection des 16 Sounds, volume, panoramique, filtre et enveloppe ADSR.
- **Browser** : presets de drum synth 808 intégrés, chargés sur le Sound sélectionné. La GUI ne propose pas encore le chargement de samples externes, le slicing ni l'édition des zones.

Le transport propose Play/Stop, Record, boucle, tempo et position BBT. Les pads GUI envoient le relâchement de note; les presets 808 sont mis en cache au chargement du kit.

## Écrans MK3 (480×272) & soft keys

Les deux écrans physiques sont rendus par une fonction partagée (`display_render::render_screens`) : la GUI affiche exactement le même contenu (miroir). Un compteur de révision évite de renvoyer une frame USB tant que rien n'a bougé.

- **Écran gauche** : ARRANGE (8 groupes, pattern de la scène, VU réels), SCENES/IDEAS (mode SCENE), SONG (sections), MIXER A–D, SAMPLER (waveform + ADSR + infos), BROWSER (presets 808).
- **Écran droit** : PATTERN (grille 16 sons × 16 pas + playhead), STEP (vélocités du Sound actif), SONG TIMELINE, MIXER master + groupe/sound focus, SAMPLER enveloppe plein écran, BROWSER chargement, EVENTS, MACRO, SETTINGS.
- **Soft keys (8 boutons sous les écrans)** : action *contextuelle* décidée par le moteur (`EngineEvent::SoftKey`) et affichée en bas des écrans. Le bouton fait toujours ce que le label annonce, identiquement au hardware et dans la GUI.
  - ARRANGE : `BAR 1..4` + longueur de pattern `1/2/4/8 MES`.
  - MIXER : `GRP A..H` (focus groupe). SAMPLER : `SND 1..8` (Sound actif).
  - BROWSER : `PRES <</>>`, `SND <</>>`, `ECOUTE`, `CHARGER`, `METRO`, `LOOP`.
  - SCENE : `SCN 1..8`. SONG : `SEC <</>>`, `+SEC`, `IDEE`, scènes et `+SCN`.
- **Encodeurs** : chaque écran affiche les 4 paramètres réellement pilotés par ses knobs (1–4 à gauche, 5–8 à droite) avec les valeurs live.

## Instrumentation latence

`maschine-linux daemon --debug-latency` (ou `gui --debug-latency`) journalise toutes les 2 s :

- latence **HID→Engine** : nombre d'événements, dernier et max en µs par fenêtre ;
- statistiques audio : callbacks, `lock_misses` (callback n'ayant pas pu prendre le lock → silence) et temps max de callback en µs.

Le thread HID réveille immédiatement la GUI (`RepaintSignal`), et le snapshot moteur partage le projet en `Arc` (copy-on-write) : les lectures UI sont O(1) et seules les actions utilisateur clonent.

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
  - *Mode STEP* : bascule silencieuse du pas correspondant (0..15) dans la mesure sélectionnée (`selected_bar`). Curseur playhead blanc en direct.
  - *Modes SCENE / PATTERN / ERASE / SELECT* : gestion directe du workflow Maschine.
- **Encodeur 4-D** :
  - *Haut / Bas* : changement rapide du son sélectionné (`active_sound`) avec pré-écoute instantanée ; navigation des presets dans le Browser.
  - *Gauche / Droite* (ou `<` / `>`) : navigation entre les mesures (`selected_bar`).
  - *Shift + Droite* : extension de la longueur du pattern à 4 ou 8 mesures.
  - *Push* : pré-écoute one-shot du son ou validation preset.
- **Écrans & Boutons d'écran 1..8** :
  - *Soft keys contextuels* : les 8 boutons sous les écrans exécutent l'action affichée en bas de l'écran (voir « Écrans MK3 & soft keys »). Même moteur d'action côté hardware et côté GUI.
  - *8 Potentiomètres rotatifs* : édition contextuelle des paramètres du son ou du mixeur ; les slots d'encodeurs des écrans affichent ces mêmes paramètres avec leurs valeurs live.

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
