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

## Vues UI (future egui)

- **Ideas View** : header Scenes (horizontal) + grid Groups (colonnes) × Patterns (lignes), highlight = assigné. Jam non-linéaire.
- **Song View** : timeline + Sections (horizontal) + Patterns empilés vertical par Group. Loop brace, playhead.
- **Pattern Editor** : Drum View (rows = Sounds 1-16) / Keyboard View (rows = pitches pour Sound focus), Mod lanes, step grid.
- **Mixer** : strips Group + Master, meters, inserts.
- **Sampler** : Record / Edit (trim, normalize, reverse) / Slice (transient/grid/BPM) / Zone (lokey/hikey/lovel/hivel, vel layers, round-robin seq, off_by, tune/pan, ADSR, filter) / Map View.

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
- `EngineState` snapshot lock-free pour UI (`Arc<RwLock<Project>>` + `Arc<AtomicU64>` version).

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
  - *Boutons 1..4 (gauche)* : commutation des 4 vues principales (ARRANGE, MIXER, SAMPLER, BROWSER).
  - *Boutons 5..8 (droite)* : sélection directe de Bar 1..4 (ou Shift + dimensionnement 1/2/4/8 mesures).
  - *8 Potentiomètres rotatifs* : édition contextuelle des paramètres du son ou du mixeur.

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
