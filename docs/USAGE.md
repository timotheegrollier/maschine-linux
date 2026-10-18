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
7. Rend les voix du moteur; `--no-audio` avance aussi le transport avec un rendu muet pour tester l'interface.

Logs : `RUST_LOG=debug maschine-linux -vv daemon`

### Contrôles hardware Maschine MK3 supportés

- **Pads 1..16** :
  - **Mode PAD** : déclenchement one-shot pleine dynamique (vélocité + pression) du son actif ou kit complet, flash blanc de frappe puis couleur assignée au son.
  - **Mode STEP** : séquenceur pas-à-pas silencieux (aucune lecture parasite lors de la pose/retrait de pas), LEDs de pas actives + curseur playhead blanc en direct.
  - **Mode KEYBOARD** : jeu mélodique en gamme mineure C3..D5.
  - **Mode CHORDS** : accords diatoniques enrichis.
- **Encodeur 4-D** :
  - **Haut / Bas** : sélection du son actif (avec pré-écoute one-shot) ; en mode Browser, défilement des presets synthétiseur.
  - **Gauche / Droite** ou touches `<` / `>` : navigation pas-à-pas entre les mesures du pattern (Bar 1, Bar 2, Bar 3, Bar 4...).
  - **Shift + Droite** : extension dynamique du pattern à 4 ou 8 mesures.
  - **Pression (Push)** : pré-écoute one-shot du son actif (ou chargement du preset sélectionné en mode Browser) — ne déclenche pas intempestivement la lecture en boucle.
  - **Rotation** : ajustement fin selon le focus actif (Tempo, Volume Master, Swing).
- **Écrans LCD couleur (2 × 480×272)** :
  - **Display Buttons 1..4 (Écran gauche)** : sélection d'onglets (ARRANGE, MIXER, SAMPLER, BROWSER).
  - **Display Buttons 5..8 (Écran droit)** : sélection directe de la mesure affichée/éditée (BAR 1, BAR 2, BAR 3, BAR 4) ; avec **Shift**, dimensionnement du pattern (1, 2, 4, 8 mesures).
  - **8 Potentiomètres crantés sous écrans** :
    - *MIXER* : volumes des Groupes A..H.
    - *SAMPLER* : boutons 1..4 = Vol, Pan, Cutoff filtre, Résonance ; 5..8 = Attack, Decay, Sustain, Release ADSR.
    - *BROWSER* : molette sélection de pad et défilement presets 808.
    - *ARRANGE* : tempo, swing, volume et filtre global.
- **Groupes A..H** : sélection instantanée du groupe actif, combinaison avec touches **MUTE** et **SOLO** pour mise en sourdine / solo immédiats.
- **Transport** : PLAY, REC, STOP, RESTART, TAP (métronome), ERASE (effacement notes par pad).
- **Touchstrip** : modulation / pitch bend avec feedback visuel ruban LED.

### Workflow projet

```bash
# Lancement direct avec GUI et moteur audio PipeWire
maschine-linux

# Lancement en mode daemon console optimisé
maschine-linux daemon --project my_project.json
```

## GUI intégrée (`egui`)

La GUI native affiche en temps réel :

- **Barre supérieure de transport** : BPM, Swing, curseur temporel BBT, état Play/Rec/Stop/Loop, monitoring CPU et voix actives, métronome.
- **Séquenceur pas-à-pas multi-mesures (16 pistes)** :
  - Sélecteur de mesure active (1, 2, 3, 4...) et flèches de navigation `◀` / `▶`.
  - Boutons de dimensionnement de pattern (1M, 2M, 4M, 8M).
  - 16 lignes d'instruments complètes avec boutons de pré-écoute `▶` et 16 pas par mesure.
- **Grille de pads 4×4** : disposition identique au hardware MK3 pour frappe directe à la souris ou écran tactile.
- **Éditeur de son / synthétiseur** : sélecteur de présets 808 (Kick, Snare, Hi-Hats, Clap, Tom, Rimshot, Cowbell, SubBass...), réglage dynamique de gain, panoramique, filtre coupe-bas/passe-haut résonant et enveloppe ADSR.
- **Mixer 8 groupes (A..H)** : faders de volume, indicateurs de crête, mute et solo.
- **Importer un sample** : dans Sampler, renseigner le chemin d'un fichier WAV et lancer l'import. Le décodage se fait en arrière-plan; la GUI reste interactive. Le Sound sélectionné est remplacé et le chemin est enregistré dans le projet.

Le Sample Editor officiel (enregistrement, plages de lecture, slicing et édition des zones depuis la MK3) n'est pas encore implémenté. L'écran EVENTS affiche actuellement des informations sans éditer les événements.

## Écrans matériels LCD (Bulk USB)

Les deux LCD 480×272 sont pilotés en USB Bulk (Interface 5). Le rendu est comparé à 30 Hz maximum; les seules régions modifiées sont envoyées, avec un budget moyen de 1,6 Mo/s et une réécriture complète périodique pour resynchroniser les écrans. Les fenêtres doivent avoir une largeur paire (paires de pixels RGB565).

Pour tester le workflow physique après compilation :

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p maschine-engine --no-default-features
cargo test --workspace
cargo build --workspace --release
./target/release/maschine-linux gui --no-audio
```

Sur la MK3, sélectionne **EVENTS**, puis **SAMPLER** : l'écran droit doit
revenir à l'enveloppe du Sound. Tourne ensuite le knob 1; le volume affiché du
Sound doit réagir dès le premier cran. La fenêtre GUI seule et un log de succès
USB ne valident pas le contenu visible sur les deux LCD.

Pour valider le chargement d'un sample, importe un WAV depuis Sampler, appuie
sur `Ctrl+S`, puis ferme et rouvre le projet sauvegardé via l'option `--project`.
Vérifie que le nom, la waveform et l'audition du Sound sont toujours présents. Les cas automatisés et leur
correspondance avec les commandes CI sont décrits dans [TESTING.md](TESTING.md).

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
