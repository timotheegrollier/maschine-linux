# Native Instruments Maschine MK3 — Protocole d'Affichage & Architecture Dual-Screen

Document de référence technique pour l'intégration des écrans couleur de la **Maschine MK3 (VID: 0x17cc, PID: 0x1600)** sous Linux 100% natif en userspace.

---

## 1. Vue d'ensemble matérielle

La Maschine MK3 embarque deux écrans LCD couleur identiques :
- **Résolution :** 480 × 272 pixels par écran (format 16:9).
- **Format de couleur :** RGB565 (16 bits par pixel, 2 octets).
  - Rouge : 5 bits (`0xF800` MSB).
  - Vert  : 6 bits (`0x07E0`).
  - Bleu  : 5 bits (`0x001F` LSB).
  - **Endianness sur le bus USB :** Big-Endian (octet de poids fort d'abord).
- **Alimentation :** Fonctionne en USB Bus Power (auto-alimenté 5V). L'alimentation optionnelle 15V fournit un rétroéclairage plus puissant mais n'est pas requise.

---

## 2. Architecture USB

L'appareil expose 7 interfaces USB :

| Interface | Nom / Fonction | Type de transfert | Endpoint |
| :--- | :--- | :--- | :--- |
| **0, 1, 2** | Audio streaming & contrôle | Isynchrone | `snd-usb-audio` |
| **3** | MIDI USB Class Compliant | Bulk IN/OUT | EP 0x02 |
| **4** | HID (Pads, boutons, encodeurs, LEDs) | Interrupt IN/OUT (64B) | EP 0x03 |
| **5** | **Maschine MK3 BD (Bulk Display)** | **Bulk OUT (wMaxPacketSize = 512)** | **EP 0x04** |
| **6** | DFU (Device Firmware Upgrade) | Contrôle | N/A |

### Prise en charge sous Linux
- L'Interface 5 n'est réclamée par aucun driver noyau par défaut, mais peut nécessiter `detach_kernel_driver(5)` ou `set_auto_detach_kernel_driver(true)`.
- Aucun droit root n'est requis si une règle udev (`/etc/udev/rules.d/70-maschine.rules`) ou les ACL systemd (`TAG+="uaccess"`) attribuent les droits au groupe utilisateur.

---

## 3. Structure du paquet d'affichage (Wire Protocol)

Chaque trame transmise à l'écran fait exactement **261 148 octets** :

```
┌─────────────────┬─────────────────┬──────────────────────────┬─────────────────┬─────────────────┐
│ Window Header   │ Transmit Cmd    │ Pixel Data (RGB565 BE)   │ Blit Command    │ End Command     │
│ (16 octets)     │ (4 octets)      │ (261 120 octets)         │ (4 octets)      │ (4 octets)      │
└─────────────────┴─────────────────┴──────────────────────────┴─────────────────┴─────────────────┘
```

### A. Window Header (Offset 0..15 : 16 octets)
Définit la zone cible de dessin et l'écran :

| Offset | Valeur | Description |
| :--- | :--- | :--- |
| **0x00** | `0x84` | Magic byte de synchronisation |
| **0x01** | `0x00` | Réservé |
| **0x02** | `0x00` ou `0x01` | **Numéro d'écran** (`0x00` = Écran Gauche, `0x01` = Écran Droit) |
| **0x03** | `0x60` | Magic byte de mode |
| **0x04..0x07** | `0x00 0x00 0x00 0x00` | Réservé (zéros) |
| **0x08..0x09** | `0x00 0x00` | Coordonnée X de départ (uint16 Big-Endian, défaut: `0`) |
| **0x0A..0x0B** | `0x00 0x00` | Coordonnée Y de départ (uint16 Big-Endian, défaut: `0`) |
| **0x0C..0x0D** | `0x01 0xE0` | Largeur (uint16 Big-Endian = 480) |
| **0x0E..0x0F** | `0x01 0x10` | Hauteur (uint16 Big-Endian = 272) |

### B. Commande Transmit (Offset 16..19 : 4 octets)
Définit le nombre de paires de pixels à transmettre :
- Formule : `pixel_pairs = (largeur * hauteur) / 2 = 65 280 = 0x00FF00` pour une frame complète.
- Format : opcode `0x00`, puis compteur 24 bits big-endian.
- Octets sur le câble pour la frame complète : `[0x00, 0x00, 0xFF, 0x00]`.

**Les fenêtres doivent avoir une largeur paire.** Les pixels RGB565 sont
consommés deux par deux par le compteur. Une fenêtre impaire peut décaler la
lecture de `blit` et `end`, puis bloquer les rafraîchissements suivants. Le
différentiel aligne chaque rectangle sur une paire; l'encodeur rejette les
largeurs impaires. La sonde de reprise est une fenêtre valide de 2×1 pixels
(32 octets), jamais une fenêtre 1×1.

### C. Données Pixels (Offset 20..261139 : 261 120 octets)
- `480 × 272 = 130 560` pixels.
- Chaque pixel est encodé en 2 octets RGB565 Big-Endian.

### D. Footer / Commandes de validation (Offset 261140..261147 : 8 octets)
- **Commande Blit (4 octets) :** `[0x03, 0x00, 0x00, 0x00]` — déclenche le rendu du buffer en mémoire vidéo.
- **Commande End (4 octets) :** `[0x40, 0x00, 0x00, 0x00]` — signale la fin de transmission.

---

## 4. Cadence et reprise

1. Le thread compare les rendus à **30 Hz maximum**, mais ne transmet que les régions modifiées et limite le débit Bulk moyen à **1,6 Mo/s**.
2. **Monolithique vs Split :**
   - L'envoi d'un buffer unique de 261 148 octets via `libusb_bulk_transfer` / `write_bulk` est pris en charge nativement par le contrôleur USB High-Speed (qui découpe automatiquement en paquets USB de 512 octets au niveau matériel).
   - Ne pas forcer un découpage manuel arbitraire de paquets dans le code userspace.

Le thread renvoie une paire plein écran toutes les 2 secondes pour resynchroniser
le framebuffer du contrôleur. Cette cadence est une stratégie logicielle de
resynchronisation; elle ne prétend pas mesurer un watchdog firmware.

Un retour `Ok` de `write_bulk` confirme seulement que l'USB a accepté les
octets. Il ne confirme pas le rendu visuel du LCD; vérifier directement les
changements de page, les couleurs et les valeurs des paramètres sur le
contrôleur.

---

## 5. Interface DAW implémentée

L'interface graphique rendue sur les deux écrans respecte fidèlement les conventions visuelles de Maschine :

### Écran Gauche (Screen 0) : Workflows, Pads & Groupes
1. **Top Soft-Buttons (Boutons 1..4) :**
   - `[1:ARRANGE]`, `[2:MIXER]`, `[3:SAMPLER]`, `[4:BROWSER]`.
2. **En-tête de projet & Groupes :**
   - Titre du projet en cours.
   - 8 onglets de groupes A à H avec pastille de couleur, mise en valeur du groupe sélectionné, et indicateurs Mute/Solo.
3. **Matrice 4×4 des 16 Pads :**
   - Reproduction exacte de la grille physique des 16 pads :
     - Rangée 4 (haut) : Pads 13..16
     - Rangée 3 : Pads 9..12
     - Rangée 2 : Pads 5..8
     - Rangée 1 (bas) : Pads 1..4
   - Pastille de couleur assignée au son, numéro de pad, et libellé abrégé.
4. **Bottom Parameter Strip (Encodeurs 1..4) :**
   - 4 slots avec libellé, valeur en direct et mini-jauge :
     - Enc 1 : `TEMPO` (BPM)
     - Enc 2 : `SWING` (%)
     - Enc 3 : `GRP VOL` (dB)
     - Enc 4 : `GRP PAN` (Bipolaire)

### Écran Droit (Screen 1) : Séquenceur, Transport & VU-Mètres
1. **Top Soft-Buttons (Boutons 5..8) :**
   - `[5:PATTERN]`, `[6:EVENTS]`, `[7:MACRO]`, `[8:SETTINGS]`.
2. **Transport & Position :**
   - Badge d'état dynamique : `▶ PLAY` (Vert), `■ STOP` (Gris), `● REC` (Rouge).
   - Compteur BBT grand format : `BAR 001 : 02 : 120`.
   - Compteur de voix actives et signature temporelle `4/4`.
3. **Mini Step-Sequencer 16 pas :**
   - 4 pistes d'instruments (Kick, Snare, HiHat Closed, HiHat Open).
   - Tête de lecture dynamique animée en temps réel selon la position de lecture !
4. **Monitoring Audio & Double VU-Mètre Stéréo :**
   - Canaux Master L et Master R avec barres de niveau dégradées (Vert -> Jaune -> Rouge).
5. **Bottom Parameter Strip (Encodeurs 5..8) :**
   - Enc 5 : `TUNE`
   - Enc 6 : `CUTOFF` (Filtre)
   - Enc 7 : `RESO`
   - Enc 8 : `MASTER` (Volume)
