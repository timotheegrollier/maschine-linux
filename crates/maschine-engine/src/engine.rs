//! Engine — assemblage Project + Transport + Sampler + Sequencer.
//!
//! Thread-safe snapshot pour UI via `parking_lot::RwLock` + `Arc`.

use std::sync::Arc;

use parking_lot::RwLock;

use crate::project::{
    BarsBeatsTicks, Pattern, PatternEvent, Project, Scene, Section, SoundAutomation, Zone,
};
use crate::sampler::Sampler;
use crate::sequencer::{Grid, Sequencer};
use crate::transport::{Transport, TransportState};

/// Événement engine (depuis hardware ou UI).
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// Pad hit
    Pad {
        /// Pad 0..15
        pad: u8,
        /// Velocity
        velocity: u8,
        /// Press
        pressed: bool,
    },
    /// Play/Rec/Stop
    Transport {
        /// Play
        play: bool,
        /// Rec
        rec: bool,
        /// Stop
        stop: bool,
    },
    /// Change BPM
    Bpm(f32),
    /// Change Group focus
    GroupFocus(u8),
    /// Trigger Scene
    Scene(usize),
    /// Crée une scène et la sélectionne
    AddScene,
    /// Quantize pattern
    Quantize {
        /// Group idx
        group: usize,
        /// Pattern idx
        pattern: usize,
    },
    /// Save
    Save(std::path::PathBuf),
    /// Sauvegarde le projet courant (rapide=JSON compact, sinon pretty).
    SaveCurrent { quick: bool },
    /// Crée un nouveau projet (nom) et le sauvegarde (comme un premier Save).
    NewProject { name: String },
    /// Définit le chemin du projet courant.
    SetProjectPath(Option<String>),
    /// Load
    Load(std::path::PathBuf),
    /// Master volume 0.0..1.0
    MasterVolume(f32),
    /// Headphone volume 0.0..1.0
    HeadphoneVolume(f32),
    /// Toggle mute on group
    Mute(u8),
    /// Bascule le bouton AUTO (armement des automations + vue dédiée).
    ToggleAutomation,
    /// Bascule le mute d'un pad/son dans un groupe (mode MUTE Maschine).
    TogglePadMute { group: u8, pad: u8 },
    /// Toggle solo on group
    Solo(u8),
    /// Select pattern in group
    PatternSelect {
        /// Group idx
        group: u8,
        /// Pattern idx
        pattern: usize,
    },
    /// Sélection immédiate d'un pattern en lecture (SHIFT + pad MK3) :
    /// pas d'attente de la prochaine mesure.
    PatternSelectInstant {
        /// Group idx
        group: u8,
        /// Pattern idx
        pattern: usize,
    },
    /// Crée un pattern vide à la suite du groupe et l'assigne à la scène active.
    AddPattern {
        /// Group idx
        group: u8,
    },
    /// Crée (si nécessaire) le pattern d'index donné, puis le sélectionne.
    /// Utilisé par les pads vides du mode PATTERN : chaque pad correspond à
    /// un slot de pattern, comme sur Maschine.
    CreatePatternAt {
        /// Group idx
        group: u8,
        /// Slot de pattern (0..)
        pattern: usize,
    },
    /// Duplique un pattern et insère la copie juste après.
    DuplicatePattern {
        /// Group idx
        group: u8,
        /// Pattern source
        pattern: usize,
    },
    /// Supprime un pattern (le dernier pattern du groupe est conservé).
    DeletePattern {
        /// Group idx
        group: u8,
        /// Pattern à supprimer
        pattern: usize,
    },
    /// Efface les événements d'un pattern sans changer sa longueur.
    ClearPattern {
        /// Group idx
        group: u8,
        /// Pattern à vider
        pattern: usize,
    },
    /// Renomme un pattern.
    RenamePattern {
        /// Group idx
        group: u8,
        /// Pattern idx
        pattern: usize,
        /// Nouveau nom
        name: String,
    },
    /// Sélectionne le pattern précédent/suivant dans le groupe focus.
    StepPattern {
        /// Group idx
        group: u8,
        /// -1 = précédent, +1 = suivant
        delta: i32,
    },
    /// Change Left display tab (0..4: Arrange, Mixer, Sampler, Browser, Synth)
    SetLeftTab(u8),
    /// Change Right display tab (0..3)
    SetRightTab(u8),
    /// Demande de navigation GUI (0: Pattern, 1: Step, 2: Keyboard,
    /// 3: Scene, 4: Plugin). Le matériel reste la source de vérité : la GUI
    /// suit ce compteur pour changer de fenêtre au bon moment.
    NavigateView(u8),
    /// Page de l'éditeur PLUGIN (0: Sound, 1: TB-303)
    SetPluginPage(u8),
    /// Select active sound (0..15)
    SelectSound(u8),
    /// Volume d'un groupe en dB
    GroupVolume { group: u8, volume_db: f32 },
    /// Pan d'un groupe
    GroupPan { group: u8, pan: f32 },
    /// Volume d'un sound (pad) en dB
    SoundVolume {
        group: u8,
        sound: u8,
        volume_db: f32,
    },
    /// Pan d'un sound
    SoundPan { group: u8, sound: u8, pan: f32 },
    /// Cutoff filtre d'un sound
    SoundFilterCutoff {
        group: u8,
        sound: u8,
        cutoff_hz: f32,
    },
    /// Résonance filtre d'un sound 0..1
    SoundFilterReso {
        group: u8,
        sound: u8,
        resonance: f32,
    },
    /// ADSR d'un sound (attack/decay/sustain/release)
    SoundAdsr {
        group: u8,
        sound: u8,
        attack_ms: f32,
        decay_ms: f32,
        sustain: f32,
        release_ms: f32,
    },
    /// Type de filtre d'un Sound (Off / LowPass / HighPass / BandPass)
    SoundFilterKind {
        group: u8,
        sound: u8,
        kind: crate::project::FilterKind,
    },
    /// Polyphonie d'un Sound (1..32)
    SoundPolyphony { group: u8, sound: u8, polyphony: u8 },
    /// Groupe de choke d'un Sound (None = aucun)
    SoundChoke {
        group: u8,
        sound: u8,
        choke_group: Option<u8>,
    },
    /// Copie les réglages du Sound source vers tous les autres Sounds du Group.
    SoundApplyToGroup { group: u8, source_sound: u8 },
    /// Réinitialise les réglages de mixage/filtre/enveloppe du Sound.
    SoundReset { group: u8, sound: u8 },
    /// Efface toutes les notes d'un son dans le pattern actif (ERASE + pad)
    EraseSound { group: u8, sound: u8 },
    /// Retire complètement un Sound de son emplacement et efface ses événements.
    ClearSoundSlot { group: u8, sound: u8 },
    /// Raccourci de la couche Shift des pads (index 0..15, ordre Maschine).
    ShiftPadAction(u8),
    /// Annule la dernière modification de projet.
    Undo,
    /// Rétablit la dernière modification annulée.
    Redo,
    /// Miroirs workflow pour les écrans (envoyés par le mapper)
    SetSceneMode(bool),
    /// Miroir mode PATTERN
    SetPatternMode(bool),
    /// Miroir mode SELECT
    SetSelectMode(bool),
    /// Miroir mode ERASE
    SetEraseMode(bool),
    /// Miroir mode de jeu 0..3
    SetControllerMode(u8),
    /// Déclenche le son actif à une note MIDI (modes Keyboard/Chords).
    KeyboardTrigger {
        /// Note MIDI 0..127
        note: u8,
        /// Vélocité 0..127
        velocity: u8,
        /// Appui / relâchement
        pressed: bool,
    },
    /// Change la fondamentale Keyboard/Chords.
    KeyboardRootDelta(i32),
    /// Change la gamme Keyboard/Chords.
    KeyboardScaleDelta(i32),
    /// Change l'octave de base Keyboard/Chords.
    KeyboardOctaveDelta(i32),
    /// Change le type d'accord (mode Chords).
    KeyboardChordDelta(i32),
    /// Change l'inversion d'accord (mode Chords).
    KeyboardInversionDelta(i32),
    /// Remplace l'état musical Keyboard/Chords.
    SetKeyboardState(crate::keyboard::KeyboardState),
    /// Bascule la boucle transport
    ToggleLoop,
    /// Bascule le métronome
    ToggleMetronome,
    /// Apply humanize variation to active pattern
    Variation,
    /// Quantize active pattern
    QuantizeActive,
    /// Active/désactive le métronome
    SetMetronome(bool),
    /// Basculer mode Song (arranger) / Ideas (scènes)
    ToggleSongMode,
    /// Section suivante dans le Song
    NextSection,
    /// Section précédente dans le Song
    PrevSection,
    /// Sélectionne une section du Song dans l'interface
    SelectSection(usize),
    /// Ajoute une section au Song pour la scène active
    AddSection,
    /// Supprime la section active du Song
    DeleteSection,
    /// Duplique la section active (insérée juste après)
    DuplicateSection,
    /// Redimensionne la section active (en mesures, 1..64)
    ResizeSection {
        /// Longueur en mesures
        bars: usize,
    },
    /// Duplique la scène active (patterns compris)
    DuplicateScene,
    /// Duplique la scène active en créant de nouveaux patterns indépendants
    DuplicateSceneUnique,
    /// Rend les patterns de la scène active indépendants (clonage des patterns)
    MakeSceneUnique(usize),
    /// Enchaîne une liste de scènes dans le Song
    SceneChain(Vec<usize>),
    /// Scène précédente / suivante
    SceneStep(i32),
    /// Crée ou sélectionne une scène à l'index donné
    CreateSceneAt(usize),
    /// Supprime la scène active (au moins une scène conservée)
    DeleteScene,
    /// Activer un pattern spécifique (group, pattern idx)
    PatternSelectIndex { group: u8, pattern: usize },
    /// Charge un preset d'instrument drum synth (0..15) sur un sound
    LoadDrumPreset {
        /// Group
        group: u8,
        /// Sound 0..15
        sound: u8,
        /// Preset index 0..15
        preset_idx: u8,
    },
    /// Charge un fichier sample WAV sur un sound
    LoadSample {
        /// Group
        group: u8,
        /// Sound 0..15
        sound: u8,
        /// Path to WAV file
        path: std::path::PathBuf,
    },
    /// Installe un WAV déjà décodé hors du verrou moteur/audio.
    LoadPreparedSample {
        /// Group 0..7
        group: u8,
        /// Sound 0..15
        sound: u8,
        /// Sample et métadonnées pré-calculées
        zone: crate::sampler::ZoneMapping,
    },
    /// Défilement preset dans le Browser (+/- step)
    BrowsePresetStep(i32),
    /// Type de navigateur sur l'onglet BROWSER (0: presets 808, 1: projets FILE).
    SetBrowserKind(u8),
    /// Entrées du navigateur de projets (fournies par l'application).
    SetProjectEntries(Vec<ProjectFileEntry>),
    /// Défilement du navigateur courant.
    BrowseBrowserStep(i32),
    /// Charge l'élément sélectionné du navigateur courant.
    LoadSelectedBrowserItem { group: u8, sound: u8 },
    /// Sélection de la mesure à afficher / éditer
    SelectBar(usize),
    /// Mesure suivante
    NextBar,
    /// Mesure précédente
    PrevBar,
    /// Définir la longueur du pattern en mesures (1, 2, 4, 8)
    SetPatternBars(usize),
    /// Swing 0..1 (knob SWING : synchro séquenceur + écrans)
    Swing(f32),
    /// Sélectionne le pas édité du step sequencer (index absolu)
    StepCursor(usize),
    /// Défile la grille du step sequencer (delta en pas, +/-)
    StepScroll(i32),
    /// Vélocité du pas sélectionné (delta 1..127)
    StepVelocityDelta(i32),
    /// Position (nudge) du pas sélectionné (delta en ticks)
    StepNudgeTicks(i32),
    /// Longueur du pas sélectionné (delta en pas de 1/16)
    StepLengthDelta(i32),
    /// Pitch du pas sélectionné (delta en demi-tons)
    StepPitchDelta(i32),
    /// Retour au début (bouton RESTART hardware)
    Restart,
    /// Soft key d'écran (1..8 : boutons sous les écrans) — action
    /// contextuelle identique au hardware et à la GUI.
    SoftKey(u8),
    /// Ajustement relatif du cutoff de l'Acid Synth (delta log / octaves)
    AcidCutoffDelta(f32),
    /// Ajustement relatif de la résonance de l'Acid Synth (0..1)
    AcidResoDelta(f32),
    /// Ajustement relatif de la modulation d'enveloppe de l'Acid Synth (0..1)
    AcidEnvModDelta(f32),
    /// Ajustement relatif du déclin d'enveloppe de l'Acid Synth (ms)
    AcidDecayDelta(f32),
    /// Ajustement relatif de l'accent de l'Acid Synth (0..1)
    AcidAccentDelta(f32),
    /// Ajustement relatif du temps de slide de l'Acid Synth (ms)
    AcidSlideDelta(f32),
    /// Ajustement relatif de la distorsion de l'Acid Synth (0..1)
    AcidDriveDelta(f32),
    /// Ajustement relatif de la tonalité de distorsion de l'Acid Synth (Hz)
    AcidDriveToneDelta(f32),
    /// Ajustement relatif du mixage de distorsion de l'Acid Synth (0..1)
    AcidDriveMixDelta(f32),
    /// Ajustement relatif du temps de delay de l'Acid Synth (ms)
    AcidDelayTimeDelta(f32),
    /// Ajustement relatif du feedback de delay de l'Acid Synth (0..0.95)
    AcidDelayFeedbackDelta(f32),
    /// Ajustement relatif du mixage de delay de l'Acid Synth (0..1)
    AcidDelayMixDelta(f32),
    /// Bascule la forme d'onde (Saw <-> Square)
    AcidWaveformToggle,
    /// Décalage d'octave relatif (+1 / -1)
    AcidOctaveDelta(i8),
    /// Sélection d'un preset Acid (0..3)
    AcidSelectPreset(u8),
    /// Remplacement complet des paramètres Acid
    AcidSetParams(crate::acid_synth::AcidParams),
    /// Alterne le type de filtre d'un Sound (Off -> LowPass -> HighPass -> BandPass -> Off)
    CycleSoundFilterKind { group: u8, sound: u8 },
    /// Définit directement le type de filtre d'un Sound
    SetSoundFilterKind {
        group: u8,
        sound: u8,
        kind: crate::project::FilterKind,
    },
    /// Remplacement complet des paramètres Wavetable / Nexus Synth
    WavetableSetParams(crate::wavetable_synth::WavetableParams),
    /// Sélection d'un preset Wavetable par index
    WavetableSetPreset(usize),
    /// Preset Wavetable suivant
    WavetableNextPreset,
    /// Preset Wavetable précédent
    WavetablePrevPreset,
    /// Ajuste le type de distorsion du Kick Synth
    KickDriveTypeDelta(i32),
    /// Ajuste le mode de filtre du Kick Synth
    KickFilterModeDelta(i32),
    /// Ajuste la fréquence de coupure du filtre du Kick Synth
    KickFilterCutoffDelta(f32),
    /// Ajuste la résonance du filtre du Kick Synth
    KickFilterResoDelta(f32),
    /// Ajuste la tonalité de saturation du Kick Synth
    KickToneDelta(f32),
    /// Ajuste la forme d'onde du Kick Synth
    KickWaveformDelta(i32),
    /// Ajuste la modulation FM du Kick Synth
    KickFmAmtDelta(f32),
    /// Ajuste le ratio de modulation FM du Kick Synth
    KickFmRatioDelta(f32),
    /// Ajuste le drive du Kick Synth
    KickDriveDelta(f32),
    /// Ajuste l'accordage (tune) du Kick Synth
    KickTuneDelta(f32),
    /// Ajuste le déclin (decay) du Kick Synth
    KickDecayDelta(f32),
    /// Ajuste le snap / punch du Kick Synth
    KickSnapDelta(f32),
    /// Ajuste le sweep time du Kick Synth
    KickSweepDelta(f32),
    /// Ajuste le click d'attaque du Kick Synth
    KickClickDelta(f32),
    /// Ajuste le niveau de sub du Kick Synth
    KickSubDelta(f32),
    /// Ajuste le volume de sortie du Kick Synth
    KickVolumeDelta(f32),
    /// Définit directement le type de distorsion du Kick Synth
    KickSetDriveType(crate::kick_synth::KickDriveType),
    /// Définit directement le mode de filtre du Kick Synth
    KickSetFilterMode(crate::kick_synth::KickFilterMode),
    /// Bascule la sous-page de paramètres synthé (ex: Page 1 Main <-> Page 2 Filtre & Disto)
    ToggleSynthParamPage,
    /// Définit la page de paramètres du plugin affiché.
    SetSynthParamPage(u8),
    /// Rotation d'un knob en mode PLUGIN : paramètre de la page affichée
    /// (`delta` en ticks bruts, 1024 par tour ; Shift = réglage fin).
    PluginKnob { knob: u8, delta: i32 },
    /// Page de paramètres précédente / suivante (boutons Page ◄ ►).
    PluginPageStep(i8),
    /// Emplacement précédent / suivant de la chaîne de plugins (4D).
    PluginSlotStep(i8),
    /// Mode EVENTS : événement précédent / suivant (4D).
    EventStep(i32),
    /// SHIFT + FIXED VEL : 16 vélocités (les 16 pads jouent le Sound
    /// sélectionné, du plus doux au plus fort).
    ToggleSixteenVelocities,
    /// SHIFT + REC : à l'arrêt, une mesure de clics puis l'enregistrement
    /// démarre ; en lecture, REC simple ; pendant le pré-compte, l'annule.
    CountInRecord,
    /// Modes KEYBOARD / CHORDS : knob `knob` tourné de `delta` ticks bruts —
    /// les réglages affichés au-dessus des knobs (fondamentale, gamme, octave,
    /// Sound, accord, renversement, groupe, master).
    KeyboardKnob { knob: u8, delta: i32 },
    /// Éditeur de sample (SAMPLING) : knob `knob` tourné de `delta` ticks
    /// bruts (paramètres de la page RECORD / EDIT / SLICE / ZONE).
    SampleKnob { knob: u8, delta: i32 },
    /// Mode EVENTS : knob `knob` tourné de `delta` ticks bruts (position,
    /// note, vélocité, longueur de l'événement sélectionné ; 5 = sélection).
    EventKnob { knob: u8, delta: i32 },
    /// Sélectionne un emplacement de la chaîne de plugins (GUI).
    SelectPluginSlot(usize),
    /// Supprime définitivement le projet sélectionné dans le navigateur
    DeleteSelectedProject,
    /// Annule la confirmation de suppression de projet
    CancelDeleteProject,
    /// Définit le mode mixer (0: Pads 1-8, 1: Pads 9-16, 2: Groupes A-H, 3: Master)
    SetMixerMode(u8),
    /// Remplacement complet des paramètres Kick Synth
    KickSetParams(crate::kick_synth::KickParams),
    /// Sélectionne un preset de Kick Synth (0..5)
    KickSetPreset(u8),
    /// Charge un plugin sur un Sound
    LoadPluginToSound {
        group: u8,
        sound: u8,
        plugin_idx: u8,
    },
    /// Ajoute un effet à un Sound
    AddFxToSound {
        group: u8,
        sound: u8,
        fx_type: crate::fx::FxType,
    },
    /// Ajoute un effet au bus d'un Group
    AddFxToGroup {
        group: u8,
        fx_type: crate::fx::FxType,
    },
    /// Ajuste la fréquence de coupure du filtre Wavetable Synth (Hz)
    WavetableCutoffDelta(f32),
    /// Ajuste la résonance du filtre Wavetable Synth (0..1)
    WavetableResoDelta(f32),
    /// Ajuste la modulation d'enveloppe du filtre Wavetable Synth (-1..1)
    WavetableEnvAmtDelta(f32),
    /// Ajuste le drive du filtre Wavetable Synth (0..1)
    WavetableDriveDelta(f32),
    /// Ajuste l'attaque de l'enveloppe d'amplitude Wavetable Synth (ms)
    WavetableAttackDelta(f32),
    /// Ajuste le decay de l'enveloppe d'amplitude Wavetable Synth (ms)
    WavetableDecayDelta(f32),
    /// Ajuste le sustain de l'enveloppe d'amplitude Wavetable Synth (0..1)
    WavetableSustainDelta(f32),
    /// Ajuste le release de l'enveloppe d'amplitude Wavetable Synth (ms)
    WavetableReleaseDelta(f32),
    /// Alterne le mode de filtre Wavetable
    WavetableFilterModeCycle,
    /// Ajuste les voix d'unisson Wavetable (1..7)
    WavetableUnisonVoicesDelta(i32),
    /// Alterne la forme d'onde OSC 1 Wavetable
    WavetableOsc1WaveCycle,
    /// Alterne la forme d'onde OSC 2 Wavetable
    WavetableOsc2WaveCycle,
    /// Ajuste le mix de chorus Wavetable (0..1)
    WavetableChorusDelta(f32),
    /// Ajuste le mix de delay Wavetable (0..1)
    WavetableDelayDelta(f32),
    /// Ajuste le mix de reverb Wavetable (0..1)
    WavetableReverbDelta(f32),
    /// Ajuste le volume principal Wavetable (dB)
    WavetableVolumeDelta(f32),
    /// Active / désactive le mode Note Repeat
    SetNoteRepeat(bool),
    /// Définit la division temporelle du Note Repeat (0: 1/32, 1: 1/16, etc.)
    SetNoteRepeatRate(u8),
    /// Indique si le bouton SHIFT physique est enfoncé (bascule des commandes contextuelles à l'écran)
    SetShiftHeld(bool),
    /// Bascule le verrouillage LOCK
    ToggleLock(bool),
    /// Enchaîne plusieurs patterns (chain) pour un groupe
    PatternChain { group: u8, patterns: Vec<usize> },
    /// Active / désactive la vélocité fixe (bouton FIXED VEL).
    SetFixedVelocity(bool),
    /// Niveau de la vélocité fixe (1..127).
    SetFixedVelocityLevel(u8),
}

/// Noms des 16 presets de batterie DSP 808
pub const DRUM_PRESET_NAMES: [&str; 16] = [
    "Kick 808",
    "Snare 808",
    "Closed HH",
    "Open HH",
    "Tom Low",
    "Tom Mid",
    "Tom High",
    "Clap 808",
    "Rimshot",
    "Shaker",
    "Cowbell",
    "Crash",
    "Perc 1",
    "Perc 2",
    "Synth Stab",
    "Sub Bass 808",
];

/// Entrée du navigateur de projets (FILE), affichable sur la MK3 et la GUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFileEntry {
    /// Nom affiché (souvent le nom du fichier sans extension).
    pub name: String,
    /// Chemin absolu du fichier projet.
    pub path: String,
    /// Métadonnée courte (taille/date) pour les écrans.
    pub meta: String,
}

/// Clé stable de runtime pour un Sound : le même pad d'un autre Group est un
/// instrument distinct, comme dans le modèle Maschine.
const fn sampler_sound_id(group: u8, sound: u8) -> crate::sampler::SoundId {
    crate::sampler::sound_id(group, sound)
}

/// Snapshot lecture seule pour UI.
#[derive(Debug, Clone)]
pub struct EngineSnapshot {
    /// Projet partagé (O(1) — plus de clone du projet par frame).
    pub project: Arc<Project>,
    /// Transport clone
    pub transport: Transport,
    /// Group focus
    pub group_focus: u8,
    /// Scene active
    pub active_scene: Option<usize>,
    /// Position BBT
    pub bbt: BarsBeatsTicks,
    /// Voix actives
    pub num_voices: usize,
    /// Master volume 0.0..1.0
    pub master_volume: f32,
    /// Headphone volume 0.0..1.0
    pub headphone_volume: f32,
    /// Muted groups bitmask
    pub muted_groups: u8,
    /// Pads/sons coupés par groupe (bit 0..15 = pad 1..16).
    pub muted_sounds: [u16; 8],
    /// Soloed groups bitmask
    pub soloed_groups: u8,
    /// Tab écran gauche actif (0: Arrange, 1: Mixer, 2: Sampler, 3: Browser)
    pub left_tab: u8,
    /// Tab écran droit actif (0: Pattern, 1: Events, 2: Macro, 3: Settings)
    pub right_tab: u8,
    /// Dernière destination de navigation demandée par le hardware
    /// (0: Pattern, 1: Step, 2: Keyboard, 3: Scene, 4: Plugin).
    pub nav_view: u8,
    /// Numéro de la dernière demande de navigation : la GUI ne réagit qu'aux
    /// changements de ce compteur, pas à chaque snapshot.
    pub nav_serial: u64,
    /// Page de l'éditeur PLUGIN (0: Sound, 1: TB-303).
    pub plugin_page: u8,
    /// Son actif sélectionné (0..15)
    pub active_sound: u8,
    /// Mode Song (arranger) vs Ideas (scènes)
    pub song_mode: bool,
    /// Section active dans le Song
    pub active_section: Option<usize>,
    /// Pads flashés par le séquenceur (lecture pattern) — pour LEDs + GUI
    pub triggered_pads: [bool; 16],
    /// Activité récente des pads (1.0 → 0.0) — animation des écrans
    pub pad_activity: [f32; 16],
    /// Mode SCENE (pads déclenchent les scènes) — miroir du mapper pour les écrans
    pub scene_mode: bool,
    /// Mode PATTERN (pads sélectionnent le pattern)
    pub pattern_mode: bool,
    /// Mode SELECT (pads sélectionnent sans jouer)
    pub select_mode: bool,
    /// Mode ERASE (pads gomment)
    pub erase_mode: bool,
    /// Mode de jeu : 0 Pad, 1 Keyboard, 2 Chords, 3 Step
    pub controller_mode: u8,
    /// État musical Keyboard/Chords (gamme, fondamentale, octave, accords).
    pub keyboard: crate::keyboard::KeyboardState,
    /// Index preset sélectionné dans le browser (0..15)
    pub browser_preset_idx: u8,
    /// Type de navigateur (0: presets 808, 1: projets FILE).
    pub browser_kind: u8,
    /// Entrées du navigateur de projets.
    pub project_entries: Vec<ProjectFileEntry>,
    /// Entrée projet sélectionnée.
    pub project_selected: usize,
    /// Chemin du projet courant.
    pub current_project_path: Option<String>,
    /// Message de statut temporaire (sauvegarde, chargement...).
    pub status_message: Option<String>,
    /// Mesure sélectionnée pour l'affichage / édition pas-à-pas (0-indexed : 0, 1, 2, 3...)
    pub selected_bar: usize,
    /// Pas sélectionné dans le step sequencer (index absolu 0..pattern_bars*16-1)
    pub step_cursor: usize,
    /// Nombre de mesures du pattern actif (1, 2, 4, 8)
    pub pattern_bars: usize,
    /// VU-mètres lissés par groupe (0..8), 0.0..~1.0
    pub group_levels: [f32; 8],
    /// Peak hold par groupe
    pub group_peaks: [f32; 8],
    /// VU-mètres lissés par pad (0..15) pour le groupe actif
    pub pad_levels: [f32; 16],
    /// Peak hold par pad (0..15) pour le groupe actif
    pub pad_peaks: [f32; 16],
    /// VU master lissé (post volume master)
    pub master_level: f32,
    /// Peak hold master
    pub master_peak: f32,
    /// Labels des 8 boutons d'écran (au-dessus des écrans — contexte courant).
    pub soft_labels: [&'static str; 8],
    /// Boutons d'écran allumés (voir `Engine::soft_key_active`).
    pub soft_active: [bool; 8],
    /// Bandeau de changement de page : 1 juste après, 0 au bout de 0,9 s.
    pub page_flash: f32,
    /// Enveloppe (min,max) du sample du Sound actif — waveform des écrans.
    pub waveform: Option<Arc<[(f32, f32)]>>,
    /// Durée du sample du Sound actif (secondes).
    pub sample_duration: f32,
    /// Paramètres actuels du synthétiseur TB-303 Acid.
    pub acid_params: crate::acid_synth::AcidParams,
    /// Paramètres actuels de modélisation de Kick.
    pub kick_params: crate::kick_synth::KickParams,
    /// Paramètres actuels du synthétiseur Wavetable / Nexus.
    pub wavetable_params: crate::wavetable_synth::WavetableParams,
    /// Le son actif possède-t-il un Kick Synth ?
    pub focus_sound_kick: Option<crate::kick_synth::KickParams>,
    /// Le son actif possède-t-il un TB-303 ?
    pub focus_sound_acid: Option<crate::acid_synth::AcidParams>,
    /// Le son actif possède-t-il un Wavetable Synth ?
    pub focus_sound_wavetable: Option<crate::wavetable_synth::WavetableParams>,
    /// Chaîne d'effets du Sound actif.
    pub focus_sound_fx: Vec<crate::fx::FxSlot>,
    /// Chaîne d'effets du Group focus.
    pub focus_group_fx: Vec<crate::fx::FxSlot>,
    /// Chaîne parcourue au 4D en mode PLUGIN (instrument, effets Sound,
    /// Group, Master) et emplacement sélectionné.
    pub plugin_slots: Vec<PluginSlotView>,
    pub plugin_slot: usize,
    /// Effet sélectionné (pages de paramètres de l'effet).
    pub plugin_fx: Option<crate::fx::FxSlot>,
    /// Mode EVENTS : événement sélectionné, son rang et le nombre
    /// d'événements parcourus (ceux du Sound actif, ou tous).
    pub event_sel: Option<crate::project::PatternEvent>,
    pub event_index: usize,
    pub event_count: usize,
    pub events_all: bool,
    /// EVENTS : les éditions portent sur toutes les notes parcourues
    /// (knob 6 « SÉLECTION »), pas seulement la note du curseur.
    pub events_select_all: bool,
    /// Éditeur de sample : page (0 RECORD, 1 EDIT, 2 SLICE, 3 ZONE), les
    /// 8 paramètres affichés au-dessus des knobs, la plage de lecture, les
    /// débuts de tranches (fractions du sample) et l'enregistrement en cours.
    pub sample_page: u8,
    pub sample_params: [Option<crate::plugin_pages::ParamView>; 8],
    pub sample_range: (f32, f32),
    pub slice_points: Vec<f32>,
    pub record: Option<RecordView>,
    /// Scène et patterns (par groupe) qui démarreront à la mesure suivante.
    pub pending_scene: Option<usize>,
    pub pending_patterns: [Option<usize>; 8],
    /// Mode mixer (0: Pads 1-8, 1: Pads 9-16, 2: Groupes A-H, 3: Master).
    pub mixer_mode: u8,
    /// Index plugin sélectionné dans le browser (0..3).
    pub browser_plugin_idx: u8,
    /// Index effet sélectionné dans le browser (0..4).
    pub browser_fx_idx: u8,
    /// Confirmation de suppression de projet active sur la MK3.
    pub delete_project_confirm: bool,
    /// Mode Note Repeat actif ?
    pub note_repeat_enabled: bool,
    /// Division temporelle du Note Repeat (0: 1/32, 1: 1/16, etc.)
    pub note_repeat_rate: u8,
    /// Bouton SHIFT enfoncé sur le contrôleur ?
    pub shift_held: bool,
    /// Verrou LOCK actif ?
    pub lock_active: bool,
    /// Chaîne de patterns active par groupe
    pub pattern_chains: [Option<Vec<usize>>; 8],
    /// Sous-page active de paramètres synthé (0: Main, 1: Filtre & Disto).
    pub synth_param_page: u8,
    /// Vélocité fixe active (FIXED VEL)
    pub fixed_velocity: bool,
    /// Niveau affiché de la vélocité fixe (1..127)
    pub fixed_velocity_level: u8,
    /// 16 vélocités (SHIFT + FIXED VEL).
    pub sixteen_velocities: bool,
    /// Bouton AUTO armé : les automations sont enregistrées pendant REC.
    pub automation_armed: bool,
    /// Vue AUTOMATION affichée sur les deux écrans.
    pub automation_view: bool,
    /// Mode de l'arpégiateur (index dans `ARP_MODES`).
    pub arp_mode: u8,
    /// Compteur de révision : change dès qu'un état a bougé (repaint UI/écrans).
    pub revision: u64,
}

// ── Constantes moteur ───────────────────────────────────────────────────

/// Décroissance de l'activité des pads (u/s) : ~170 ms de 1.0 à 0.0.
const PAD_ACTIVITY_DECAY: f32 = 6.0;

/// Petit instantané utilisé par les LED MK3 à haute fréquence.
/// Il évite de cloner tout le projet à chaque rafraîchissement matériel.
#[derive(Debug, Clone)]
pub struct HardwareSnapshot {
    pub group_focus: u8,
    pub active_sound: u8,
    pub left_tab: u8,
    pub right_tab: u8,
    /// Type de navigateur (0: presets 808, 1: projets FILE, 2: plugins, 3: effets)
    pub browser_kind: u8,
    /// Mode mixer (0: Pads 1-8, 1: Pads 9-16, 2: Groupes A-H, 3: Master)
    pub mixer_mode: u8,
    /// Le son actif a-t-il un Kick Synth ?
    pub has_kick_synth: bool,
    /// Le son actif a-t-il un TB-303 Acid ?
    pub has_acid_synth: bool,
    /// Le son actif a-t-il un Wavetable / Nexus Synth ?
    pub has_wavetable_synth: bool,
    /// Confirmation de suppression de projet active
    pub delete_project_confirm: bool,
    /// Page PLUGIN (0: Sound, 1: TB-303) — routage des knobs hardware.
    pub plugin_page: u8,
    /// Sous-page active de paramètres synthé (0: Main, 1: Filtre & Disto).
    pub synth_param_page: u8,
    pub controller_mode: u8,
    pub keyboard: crate::keyboard::KeyboardState,
    pub is_playing: bool,
    pub is_recording: bool,
    pub is_stopped: bool,
    pub current_bar: usize,
    pub current_step: usize,
    pub is_quarter_beat: bool,
    pub active_steps: [bool; 16],
    pub triggered_pads: [bool; 16],
    /// Activité récente des pads (pour les LEDs MK3 haute fréquence).
    pub pad_activity: [f32; 16],
    /// Niveaux lissés des pads du groupe focalisé (pour les LEDs MK3).
    pub pad_levels: [f32; 16],
    /// Niveaux lissés des 8 groupes (pour les LEDs des boutons de groupe).
    pub group_levels: [f32; 8],
    pub master_volume: f32,
    pub muted_groups: u8,
    pub muted_sounds: [u16; 8],
    pub soloed_groups: u8,
    pub active_scene: Option<usize>,
    pub num_scenes: usize,
    pub group_num_patterns: usize,
    pub scene_pattern: Option<usize>,
    /// Masque des pads appartenant à la chaîne de patterns du groupe focus.
    pub pattern_chain_mask: u16,
    pub loop_enabled: bool,
    pub metronome: bool,
    pub selected_bar: usize,
    pub pattern_bars: usize,
    /// Pas sélectionné du step sequencer (index absolu)
    pub step_cursor: usize,
    pub pad_colors: [Option<u8>; 16],
    /// Mode Note Repeat actif ?
    pub note_repeat_enabled: bool,
    /// Bouton SHIFT enfoncé ?
    pub shift_held: bool,
    /// Verrou LOCK actif ?
    pub lock_active: bool,
    /// Boutons d'écran ayant un libellé (LED en demi-teinte).
    pub soft_present: [bool; 8],
    /// Boutons d'écran allumés (LED pleine), comme sur les écrans.
    pub soft_active: [bool; 8],
    /// Pages PLUGIN : page précédente / suivante disponibles (LED Page ◄ ►),
    /// `None` hors des pages d'un plugin.
    pub page_nav: Option<[bool; 2]>,
    /// Couleurs (palette des LEDs) des 8 groupes, 0 = groupe vide.
    pub group_colors: [u8; 8],
    /// Lancements en attente : scène, pattern du groupe focalisé.
    pub pending_scene: Option<usize>,
    pub pending_pattern: Option<usize>,
    /// Pré-compte en cours (REC clignote).
    pub count_in: bool,
    /// 16 vélocités : les pads ne changent pas le Sound sélectionné.
    pub sixteen_velocities: bool,
    pub fixed_velocity: bool,
    /// Bouton AUTO allumé (armement des automations).
    pub automation_armed: bool,
}

/// Convertit un gain en décibels en multiplicateur linéaire.
#[inline]
pub fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

/// Événements parcourus en mode EVENTS : index dans `pat.events`, triés par
/// temps (puis pad, hauteur) ; ceux d'un seul pad si `pad` est donné.
/// Partagé par le moteur et les écrans : même liste, même ordre.
#[must_use]
pub fn event_scope(pat: &crate::project::Pattern, pad: Option<u8>) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..pat.events.len())
        .filter(|&i| pad.is_none_or(|p| pat.events[i].pad == p))
        .collect();
    idx.sort_by_key(|&i| (pat.events[i].tick, pat.events[i].pad, pat.events[i].pitch));
    idx
}

/// Rejoue sur un sampler fraîchement chargé ce que le projet garde des
/// éditions de sample : plage de lecture et retouches (REVERSE, NORMLZ, LOOP).
fn restore_sound_edits(sampler: &mut Sampler, project: &Project) {
    for (g, group) in project.groups.iter().enumerate().take(8) {
        for (s, snd) in group.sounds.iter().enumerate().take(16) {
            let Some(snd) = snd.as_ref() else { continue };
            let id = sampler_sound_id(g as u8, s as u8);
            if let Some(z) = snd.zones.first() {
                sampler.set_sound_range(id, z.start, z.end);
            }
            if snd.edits.reversed {
                sampler.reverse_sound(id);
            }
            if snd.edits.normalized {
                sampler.normalize_sound(id);
            }
            if snd.edits.looped {
                // Les zones chargées partent en One-Shot.
                sampler.toggle_sound_one_shot(id);
            }
        }
    }
}

/// Pages de l'éditeur de sample (boutons 1–4 en SAMPLING).
const SAMPLE_PAGES: [&str; 4] = ["RECORD", "EDIT", "SLICE", "ZONE"];

/// Enregistrement en cours : entrée audio mono vers un Sound.
#[derive(Debug)]
struct Recorder {
    target: (u8, u8),
    /// Capacité réservée au départ (durée max) : jamais d'allocation audio.
    buf: Vec<f32>,
    /// Attend le seuil (SEUIL) avant de capturer.
    waiting: bool,
    /// Crête récente de l'entrée (affichage).
    level: f32,
}

/// État d'enregistrement affiché (écrans, GUI).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecordView {
    pub waiting: bool,
    pub secs: f32,
    pub max_secs: f32,
    pub level: f32,
}

/// Clé du cache des tranches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SliceKey {
    sound: crate::sampler::SoundId,
    count: u8,
    transients: bool,
    sensitivity: u32,
    range: (u32, u32),
    sample: Option<usize>,
}

/// Réglages d'une zone, communs au projet et au sampler.
#[derive(Debug, Clone, Copy, Default)]
struct ZoneFields {
    root_key: u8,
    lokey: u8,
    hikey: u8,
    lovel: u8,
    hivel: u8,
    tune_cents: i16,
    pan: f32,
    volume_db: f32,
}

impl ZoneFields {
    fn of_mapping(z: &crate::sampler::ZoneMapping) -> Self {
        Self {
            root_key: z.root_key,
            lokey: z.lokey,
            hikey: z.hikey,
            lovel: z.lovel,
            hivel: z.hivel,
            tune_cents: z.tune_cents,
            pan: z.pan,
            volume_db: z.volume_db,
        }
    }

    fn write_mapping(self, z: &mut crate::sampler::ZoneMapping) {
        (z.root_key, z.lokey, z.hikey, z.lovel, z.hivel) = (
            self.root_key,
            self.lokey,
            self.hikey,
            self.lovel,
            self.hivel,
        );
        (z.tune_cents, z.pan, z.volume_db) = (self.tune_cents, self.pan, self.volume_db);
    }

    fn of_zone(z: &Zone) -> Self {
        Self {
            root_key: z.root_key,
            lokey: z.lokey,
            hikey: z.hikey,
            lovel: z.lovel,
            hivel: z.hivel,
            tune_cents: z.tune_cents,
            pan: z.pan,
            volume_db: z.volume_db,
        }
    }

    fn write_zone(self, z: &mut Zone) {
        (z.root_key, z.lokey, z.hikey, z.lovel, z.hivel) = (
            self.root_key,
            self.lokey,
            self.hikey,
            self.lovel,
            self.hivel,
        );
        (z.tune_cents, z.pan, z.volume_db) = (self.tune_cents, self.pan, self.volume_db);
    }
}

/// Lettre d'un groupe (A–H).
#[cfg(feature = "sampler")]
fn group_letter_char(group: u8) -> char {
    (b'A' + group % 8) as char
}

/// Écrit un WAV mono 32 bits flottants.
#[cfg(feature = "sampler")]
fn write_wav_mono(path: &std::path::Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for &s in samples {
        w.write_sample(s).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

/// Boutons du mode EVENTS (écran gauche puis droit).
const EVENT_LABELS: [&str; 8] = [
    "EVT <", "EVT >", "TOUS", "SUPPR", "DUPLIQ", "QUANT", "NUDGE <", "NUDGE >",
];

/// Ticks par pas de 16e (960 par noire).
const TICKS_PER_STEP: u32 = 240;

/// Modes de l'arpégiateur (knob 7 en KEYBOARD / CHORDS) : sens, et « 2 » =
/// les notes tenues plus leur octave au-dessus.
pub const ARP_MODES: [&str; 6] = ["HAUT", "BAS", "H-B", "HAUT 2", "BAS 2", "H-B 2"];

/// Note jouée par le thread audio, à écrire dans le pattern hors du callback.
#[derive(Debug, Clone, Copy)]
struct PendingNote {
    group: u8,
    pad: u8,
    pitch: u8,
    velocity: u8,
    tick: u64,
}

/// Une instance de synthé par Sound (groupe × slot), comme dans le logiciel
/// officiel : chaque groupe joue ses synthés en même temps que les autres,
/// avec son volume, son pan, ses mutes et ses effets. Instances créées et
/// mises à jour hors du thread audio (`Engine::sync_synth_rack`).
struct SynthRack {
    acid: Vec<Option<RackSlot<crate::acid_synth::AcidSynth>>>,
    kick: Vec<Option<RackSlot<crate::kick_synth::KickSynth>>>,
    wavetable: Vec<Option<RackSlot<crate::wavetable_synth::WavetableSynth>>>,
}

/// Un synthé du rack ; s'il reste silencieux quelques blocs, son rendu est
/// suspendu jusqu'à la note suivante.
struct RackSlot<T> {
    synth: Box<T>,
    idle_blocks: u32,
}

impl<T> RackSlot<T> {
    fn new(synth: T) -> Self {
        Self {
            synth: Box::new(synth),
            idle_blocks: 0,
        }
    }
}

/// Blocs silencieux avant suspension du rendu (≈ 0,4 s à 1024 trames).
const RACK_IDLE_BLOCKS: u32 = 20;

/// Index d'un Sound dans le rack.
const fn rack_index(group: u8, sound: u8) -> usize {
    (group as usize % 8) * 16 + sound as usize % 16
}

impl SynthRack {
    fn new() -> Self {
        Self {
            acid: (0..128).map(|_| None).collect(),
            kick: (0..128).map(|_| None).collect(),
            wavetable: (0..128).map(|_| None).collect(),
        }
    }
}

/// Propriétaire d'une chaîne d'effets d'insert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxOwner {
    Sound(u8, u8),
    Group(u8),
    Master,
}

impl FxOwner {
    /// Chaîne d'effets de ce propriétaire dans le projet.
    #[must_use]
    pub fn chain(self, proj: &Project) -> &[crate::fx::FxSlot] {
        match self {
            Self::Sound(g, s) => proj
                .groups
                .get(g as usize)
                .and_then(|grp| grp.sounds.get(s as usize))
                .and_then(Option::as_ref)
                .map_or(&[], |snd| snd.fx_chain.as_slice()),
            Self::Group(g) => proj
                .groups
                .get(g as usize)
                .map_or(&[], |grp| grp.fx_chain.as_slice()),
            Self::Master => proj.master.fx_chain.as_slice(),
        }
    }

    /// Niveau affiché : 0 = Sound, 1 = Group, 2 = Master.
    #[must_use]
    pub fn level(self) -> u8 {
        match self {
            Self::Sound(..) => 0,
            Self::Group(_) => 1,
            Self::Master => 2,
        }
    }

    /// Chaîne modifiable ; `create` crée le Sound s'il n'existe pas encore.
    fn chain_mut(self, proj: &mut Project, create: bool) -> Option<&mut Vec<crate::fx::FxSlot>> {
        match self {
            Self::Sound(g, s) => {
                let slot = proj
                    .groups
                    .get_mut(g as usize)?
                    .sounds
                    .get_mut(s as usize)?;
                if create && slot.is_none() {
                    *slot = Some(crate::project::Sound::new(&format!("Sound {}", s + 1), 78));
                }
                Some(&mut slot.as_mut()?.fx_chain)
            }
            Self::Group(g) => Some(&mut proj.groups.get_mut(g as usize)?.fx_chain),
            Self::Master => Some(&mut proj.master.fx_chain),
        }
    }
}

/// Emplacement de la chaîne de plugins parcourue en mode PLUGIN (4D).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginSlot {
    /// Instrument du Sound focalisé (sampler ou synthé).
    Instrument,
    /// Effet `index` de la chaîne d'un propriétaire.
    Fx(FxOwner, usize),
    /// Emplacement libre en fin de chaîne : insertion d'un effet.
    Add(FxOwner),
}

/// Ligne de la chaîne affichée (écrans MK3, GUI).
#[derive(Debug, Clone, PartialEq)]
pub struct PluginSlotView {
    /// 0 = Sound, 1 = Group, 2 = Master.
    pub level: u8,
    pub name: String,
    /// Effet présent mais contourné (BYPASS).
    pub bypassed: bool,
    /// Emplacement libre « + effet ».
    pub empty: bool,
}

/// Taille des tampons de bus d'effets (échantillons entrelacés) : couvre les
/// blocs de 1024 trames du callback ; un bloc plus long contourne les effets.
const FX_BUS_LEN: usize = 4096;

/// Engine principal.
pub struct Engine {
    /// `Arc<RwLock<Arc<Project>>>` : lecture O(1) pour les snapshots (UI/écrans),
    /// écriture copy-on-write via `Arc::make_mut` (actions utilisateur seulement).
    project: Arc<RwLock<Arc<Project>>>,
    /// Transport (public pour accès sample_rate depuis audio thread)
    pub transport: Transport,
    sampler: Sampler,
    /// Synthétiseur analogique virtuel TB-303 Acid
    pub acid_synth: crate::acid_synth::AcidSynth,
    /// Presets 808 pré-rendus, réutilisés par le Browser sans synthèse au clic.
    preset_samples: Vec<Option<std::sync::Arc<Vec<f32>>>>,
    sequencer: Sequencer,
    group_focus: u8,
    active_scene: Option<usize>,
    /// Master volume 0.0..1.0
    master_volume: f32,
    /// Headphone volume 0.0..1.0
    headphone_volume: f32,
    /// Muted groups bitmask
    muted_groups: u8,
    /// Pads/sons coupés par groupe (bit 0..15 = pad 1..16).
    muted_sounds: [u16; 8],
    /// Soloed groups bitmask
    soloed_groups: u8,
    /// Tab écran gauche
    pub left_tab: u8,
    /// Tab écran droit
    pub right_tab: u8,
    /// Navigation GUI demandée par le hardware (voir `EngineEvent::NavigateView`)
    pub nav_view: u8,
    /// Compteur monotone des demandes de navigation GUI
    nav_serial: u64,
    /// Page de l'éditeur PLUGIN (0: Sound, 1: TB-303)
    pub plugin_page: u8,
    /// Page de paramètres du plugin affiché (voir `plugin_pages`).
    pub synth_param_page: u8,
    /// Crans accumulés par knob pour les paramètres entiers et à choix.
    plugin_knob_accum: [i32; 8],
    /// Dernier preset TB-303 choisi (boutons PRESET ◄ ►).
    acid_preset: u8,
    /// Dernier changement de page (bandeau de transition des écrans).
    page_changed_at: Option<std::time::Instant>,
    /// Son actif
    pub active_sound: u8,
    /// Mesure sélectionnée pour l'édition et l'affichage (0..pattern_bars-1)
    pub selected_bar: usize,
    /// Curseur du step sequencer : pas sélectionné (index absolu dans le
    /// pattern, 0..pattern_bars*16-1) — les knobs éditent ce pas.
    pub step_cursor: usize,
    /// Index preset sélectionné dans le Browser
    pub browser_preset_idx: u8,
    /// Type de navigateur sur l'onglet BROWSER/FILE (0: presets 808, 1: projets, 2: plugins, 3: effets).
    pub browser_kind: u8,
    /// Mode mixer (0: Pads 1-8, 1: Pads 9-16, 2: Groupes A-H, 3: Master).
    pub mixer_mode: u8,
    /// Confirmation suppression de projet active.
    pub delete_project_confirm: bool,
    /// Index plugin sélectionné dans le browser (0..3).
    pub browser_plugin_idx: u8,
    /// Index effet sélectionné dans le browser (0..4).
    pub browser_fx_idx: u8,
    /// Moteur de modélisation de Kick Techno.
    pub kick_synth: crate::kick_synth::KickSynth,
    /// Synthétiseur polyphonique Wavetable / Nexus.
    pub wavetable_synth: crate::wavetable_synth::WavetableSynth,
    /// Processeur d'effets temps réel.
    /// Bus d'effets actifs (Sounds puis groupes) : propriétaire et état DSP.
    fx_buses: Vec<(FxOwner, crate::fx::FxChainProcessor)>,
    /// Inserts du Master, appliqués à la somme de tous les groupes.
    master_fx: crate::fx::FxChainProcessor,
    /// Synthés de lecture : une instance par Sound (voir `SynthRack`).
    rack: SynthRack,
    /// Voix de monitoring pendant REC : le Sound joué en direct utilise ses
    /// propres instances pour ne pas être coupé par la lecture du pattern
    /// (le pattern continue de sonner sur `rack` — flow Maschine).
    monitor_rack: SynthRack,
    /// Lancements en attente pendant la lecture : la scène ou le pattern
    /// choisi démarre au début de la mesure suivante, comme sur la MK3.
    pending_scene: Option<usize>,
    pending_patterns: [Option<usize>; 8],
    /// Pré-compte en cours : samples écoulés (une mesure, puis REC).
    count_in: Option<u64>,
    /// Notes jouées par le thread audio (Note Repeat, Arp) pendant REC,
    /// écrites dans le pattern par `commit_launches` (jamais dans le callback :
    /// écrire le projet le recopie en entier).
    pending_notes: [Option<PendingNote>; 64],
    /// Ticks des notes enregistrées pendant REC : par pad, le tick de
    /// l'événement enregistré (relâchement = fin de la note tenue).
    rec_pad_press: [Option<u32>; 16],
    /// Note tenue sur chaque pad pendant REC (tick de l'événement enregistré),
    /// pour enregistrer les slides (glissando legato) du TB-303.
    rec_sounding: [Option<u32>; 16],
    /// Notes tenues pendant REC : (pad, note, tick de fin quantifié).
    pending_note_offs: [Option<(u8, u8, u64)>; 16],
    /// Notes clavier tenues enregistrées pendant REC : par note, le
    /// (groupe, Sound, tick de l'événement enregistré).
    rec_key_press: [Option<(u8, u8, u32)>; 128],
    /// KEYBOARD / CHORDS : vélocité des notes MIDI tenues (0 = relâchée).
    arp_held: [u8; 128],
    /// Dernière note jouée par l'arpégiateur (relâchée au pas suivant).
    arp_last: Option<u8>,
    /// Mode de l'arpégiateur (`ARP_MODES`) et sens courant en HAUT-BAS.
    arp_mode: u8,
    arp_falling: bool,
    /// 16 vélocités (SHIFT + FIXED VEL).
    sixteen_velocities: bool,
    /// Bouton AUTO de la MK3 : enregistrement des automations armé.
    automation_armed: bool,
    /// Vue AUTOMATION ouverte sur les deux écrans.
    automation_view: bool,
    /// Patterns lancés par le séquenceur, reportés dans la scène du projet
    /// hors du thread audio (`commit_launches`).
    launched_patterns: [Option<usize>; 8],
    /// Limiteur de sortie (après le volume Master).
    master_limiter: crate::limiter::MasterLimiter,
    /// Emplacement sélectionné dans la chaîne de plugins (mode PLUGIN) et
    /// Sound pour lequel il a été choisi (un autre Sound repart de l'instrument).
    plugin_slot: usize,
    plugin_slot_focus: (u8, u8),
    /// Mode EVENTS : rang de l'événement sélectionné, portée (tous les Sounds
    /// ou le Sound actif) et crans de knob en attente.
    event_cursor: usize,
    events_all: bool,
    events_select_all: bool,
    event_knob_accum: [i32; 8],
    /// Éditeur de sample (SAMPLING) : page, découpe, enregistrement.
    sample_page: u8,
    slice_count: u8,
    slice_transients: bool,
    slice_sensitivity: f32,
    record_threshold_db: f32,
    record_max_secs: f32,
    record_wait: bool,
    recorder: Option<Recorder>,
    recordings_dir: std::path::PathBuf,
    sample_knob_accum: [i32; 8],
    /// Débuts de tranches calculés (clé = Sound, réglages, plage).
    slice_cache: parking_lot::Mutex<Option<(SliceKey, Vec<f32>)>>,
    /// Tampons de mixage des bus, préalloués hors du thread audio.
    fx_bus_bufs: Vec<Vec<f32>>,
    /// Entrées du navigateur de projets.
    pub project_entries: Vec<ProjectFileEntry>,
    /// Entrée projet sélectionnée.
    pub project_selected: usize,
    /// Chemin du projet courant (sauvegarde rapide / FILE SAVE).
    pub current_project_path: Option<String>,
    /// Message de statut temporaire (sauvegarde, chargement...) affiché sur MK3/GUI.
    status_message: Option<String>,
    status_expires_at: std::time::Instant,
    /// Mode Song (arranger) vs Ideas (scènes)
    pub song_mode: bool,
    /// Section active dans le Song
    pub active_section: Option<usize>,
    /// Flash pads jusqu'à ce tick (séquenceur) — 16 pads
    pad_flash_until: [u64; 16],
    /// Activité récente des pads (1.0 → 0.0, décroit) — animation écrans
    pad_activity: [f32; 16],
    /// Modes workflow (miroirs du mapper pour les écrans)
    pub scene_mode: bool,
    pub pattern_mode: bool,
    pub select_mode: bool,
    pub erase_mode: bool,
    /// Mode de jeu 0..3 (Pad/Keyboard/Chords/Step)
    pub controller_mode: u8,
    /// État musical Keyboard/Chords.
    pub keyboard: crate::keyboard::KeyboardState,
    /// Clic métronome accentué (début de mesure)
    click_accent: std::sync::Arc<Vec<f32>>,
    /// Clic métronome normal (temps)
    click_normal: std::sync::Arc<Vec<f32>>,
    /// VU-mètres : niveau lissé par groupe (0..8) + peak hold
    group_meter: [f32; 8],
    /// Peak hold par groupe
    group_peak_hold: [f32; 8],
    /// VU-mètres : niveau lissé par pad (0..15) par groupe (0..8)
    pad_meter: [[f32; 16]; 8],
    /// Peak hold par pad (0..15) par groupe (0..8)
    pad_peak_hold: [[f32; 16]; 8],
    /// VU master lissé + peak hold (post volume master)
    master_meter: f32,
    /// Peak hold master
    master_peak_hold: f32,
    /// Mode Note Repeat actif ?
    pub note_repeat_enabled: bool,
    /// Division temporelle du Note Repeat (0: 1/32, 1: 1/16, etc.)
    pub note_repeat_rate: u8,
    /// Vélocités des pads maintenus pour le Note Repeat (None si relâché)
    pub note_repeat_held_pads: [Option<u8>; 16],
    /// Accumulateur d'échantillons pour Note Repeat en mode Stop
    note_repeat_samples: u64,
    /// Vélocité fixe active (FIXED VEL)
    fixed_velocity: bool,
    /// Niveau appliqué aux pads joués et aux pas posés (1..127)
    fixed_velocity_level: u8,
    /// Buffer de mixage temporaire pour le rendu des synthés
    synth_buf: Vec<f32>,
    /// Emplacement cible pour Kick Synth (group, sound)
    pub kick_sound_target: Option<(u8, u8)>,
    /// Emplacement cible pour Acid Synth (group, sound)
    pub acid_sound_target: Option<(u8, u8)>,
    /// Emplacement cible pour Wavetable Synth (group, sound)
    pub wavetable_sound_target: Option<(u8, u8)>,
    /// Bouton SHIFT enfoncé sur le contrôleur ?
    pub shift_held: bool,
    /// Verrou LOCK actif ?
    pub lock_active: bool,
    /// Chaîne de patterns active par groupe
    pub pattern_chains: [Option<Vec<usize>>; 8],
    /// Compteur de révision (incrémenté à chaque changement d'état) :
    /// l'UI et les écrans évitent de re-rendre/repaint inutilement.
    revision: u64,
    /// Historique de projets pour les commandes Shift+Pad Undo/Redo.
    undo_history: Vec<Arc<Project>>,
    redo_history: Vec<Arc<Project>>,
    step_undo_history: Vec<Arc<Project>>,
    step_redo_history: Vec<Arc<Project>>,
    /// Événements copiés avec Shift+Pad 11.
    pattern_clipboard: Option<Vec<PatternEvent>>,
}

/// Copie-on-write du projet pour les écritures (les snapshots tiennent un `Arc`).
fn project_mut<'g>(
    guard: &'g mut parking_lot::RwLockWriteGuard<'_, Arc<Project>>,
) -> &'g mut Project {
    Arc::make_mut(&mut **guard)
}

/// Garantit qu'une scène active existe et que son tableau de patterns couvre
/// tous les groupes. Retourne l'index de la scène active.
fn ensure_active_scene(project: &mut Project, active_scene: &mut Option<usize>) -> usize {
    if project.scenes.is_empty() {
        project
            .scenes
            .push(Scene::new("Scène 1", project.groups.len()));
    }
    let scene_idx = active_scene
        .unwrap_or(0)
        .min(project.scenes.len().saturating_sub(1));
    *active_scene = Some(scene_idx);
    let num_groups = project.groups.len();
    if let Some(scene) = project.scenes.get_mut(scene_idx) {
        if scene.patterns.len() < num_groups {
            scene.patterns.resize(num_groups, None);
        }
    }
    scene_idx
}

/// Pattern du groupe joué dans la scène active ; s'il n'y en a pas, un
/// pattern vide est créé (longueur des autres groupes) et placé dans la scène.
fn ensure_group_pattern(
    project: &mut Project,
    active_scene: &mut Option<usize>,
    group: usize,
) -> Option<usize> {
    if group >= project.groups.len() {
        return None;
    }
    if let Some(i) = project.active_pattern_index(group, *active_scene) {
        return Some(i);
    }
    let length = new_pattern_length(project, group, *active_scene);
    let g = &mut project.groups[group];
    let idx = g.patterns.len();
    g.patterns
        .push(Pattern::new(&format!("Pattern {}", idx + 1), length));
    assign_pattern_to_active_scene(project, active_scene, group, idx);
    Some(idx)
}

/// Assigne un pattern du groupe à la scène active (créée au besoin).
fn assign_pattern_to_active_scene(
    project: &mut Project,
    active_scene: &mut Option<usize>,
    group: usize,
    pattern: usize,
) {
    let scene_idx = ensure_active_scene(project, active_scene);
    if let Some(scene) = project.scenes.get_mut(scene_idx) {
        if let Some(slot) = scene.patterns.get_mut(group) {
            *slot = Some(pattern);
        }
    }
}

/// Longueur par défaut d'un nouveau pattern : celle du pattern actif, sinon
/// 2 mesures (valeur historique de la démo).
fn new_pattern_length(
    project: &Project,
    group: usize,
    active_scene: Option<usize>,
) -> BarsBeatsTicks {
    project
        .active_pattern_index(group, active_scene)
        .and_then(|pi| project.groups.get(group)?.patterns.get(pi))
        .map(|p| p.length)
        .unwrap_or_else(|| BarsBeatsTicks::new(2, 0, 0))
}

/// Réindexe les références de scènes après insertion d'un pattern.
fn shift_scene_refs_after_insert(project: &mut Project, group: usize, inserted: usize) {
    for scene in &mut project.scenes {
        if let Some(slot) = scene.patterns.get_mut(group) {
            if let Some(pi) = *slot {
                if pi >= inserted {
                    *slot = Some(pi + 1);
                }
            }
        }
    }
}

/// Réindexe les références de scènes après suppression d'un pattern. Les
/// scènes qui utilisaient le pattern supprimé basculent sur le pattern voisin
/// (jamais hors limites), afin de rester jouables.
fn shift_scene_refs_after_remove(
    project: &mut Project,
    group: usize,
    removed: usize,
    new_len: usize,
) {
    let fallback = removed.min(new_len.saturating_sub(1));
    for scene in &mut project.scenes {
        if let Some(slot) = scene.patterns.get_mut(group) {
            if let Some(pi) = *slot {
                if pi == removed {
                    *slot = Some(fallback);
                } else if pi > removed {
                    *slot = Some(pi - 1);
                }
            }
        }
    }
}

/// Handle partagé du projet pour l'UI : `read()` O(1) (renvoie un `Arc<Project>`),
/// `write()` applique un copy-on-write via `Arc::make_mut`.
#[derive(Clone)]
pub struct ProjectHandle(Arc<RwLock<Arc<Project>>>);

impl ProjectHandle {
    /// Lecture (verrou partagé, O(1) — ne clone pas le projet).
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, Arc<Project>> {
        self.0.read()
    }

    /// Écriture copy-on-write.
    pub fn write(&self) -> ProjectWriteGuard<'_> {
        ProjectWriteGuard {
            guard: self.0.write(),
        }
    }
}

/// Garde d'écriture projet : deref vers `Project`, `Arc::make_mut` à la demande.
pub struct ProjectWriteGuard<'a> {
    guard: parking_lot::RwLockWriteGuard<'a, Arc<Project>>,
}

impl std::ops::Deref for ProjectWriteGuard<'_> {
    type Target = Project;
    fn deref(&self) -> &Project {
        &self.guard
    }
}

impl std::ops::DerefMut for ProjectWriteGuard<'_> {
    fn deref_mut(&mut self) -> &mut Project {
        Arc::make_mut(&mut *self.guard)
    }
}

/// Contexte d'un bloc audio : permet de placer une note au sample près dans
/// le buffer courant (sinon elle est quantifiée à la taille du buffer).
#[derive(Debug, Clone, Copy)]
struct BlockCtx {
    /// Frames du bloc.
    frames: u64,
    /// Échantillons par tick (fonction du BPM et du taux d'échantillonnage).
    samples_per_tick: f64,
    /// Fraction de tick déjà écoulée au premier sample du bloc.
    tick_fraction: f64,
}

impl BlockCtx {
    /// Position en échantillons (depuis le début du bloc) d'un événement situé
    /// `rel_ticks` ticks après le début du bloc.
    fn offset(&self, rel_ticks: f64) -> usize {
        let s = ((rel_ticks - self.tick_fraction) * self.samples_per_tick).round();
        s.clamp(0.0, self.frames.saturating_sub(1) as f64) as usize
    }
}

impl Engine {
    /// Clic métronome (accent = début de mesure) — généré au taux demandé.
    fn metronome_click(sr: u32, freq: f32) -> std::sync::Arc<Vec<f32>> {
        let n = (sr as f32 * 0.030) as usize;
        let v: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sr as f32;
                let env = (-t * 120.0).exp();
                // Fondu de fin (anti-clic) : la queue du clic n'est pas coupée.
                let tail = if i + 64 >= n {
                    ((n - i) as f32 / 64.0).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                (2.0 * std::f32::consts::PI * freq * t).sin() * env * 0.6 * tail
            })
            .collect();
        std::sync::Arc::new(v)
    }

    /// Crée avec un projet.
    #[must_use]
    pub fn new(project: Project, sample_rate: u32) -> Self {
        let bpm = project.bpm;
        let mut eng = Self {
            project: Arc::new(RwLock::new(Arc::new(project))),
            transport: Transport::new(bpm, sample_rate),
            sampler: Sampler::new(sample_rate),
            acid_synth: crate::acid_synth::AcidSynth::new(sample_rate),
            preset_samples: vec![None; 16],
            sequencer: Sequencer::new(Grid::Sixteenth, 0.0),
            group_focus: 0,
            active_scene: Some(0),
            master_volume: 0.85,
            headphone_volume: 0.85,
            muted_groups: 0,
            muted_sounds: [0; 8],
            soloed_groups: 0,
            left_tab: 0,
            right_tab: 0,
            nav_view: 0,
            nav_serial: 0,
            plugin_page: 0,
            synth_param_page: 0,
            plugin_knob_accum: [0; 8],
            acid_preset: 0,
            page_changed_at: None,
            active_sound: 0,
            selected_bar: 0,
            step_cursor: 0,
            browser_preset_idx: 0,
            browser_kind: 0,
            mixer_mode: 0,
            delete_project_confirm: false,
            browser_plugin_idx: 0,
            browser_fx_idx: 0,
            kick_synth: crate::kick_synth::KickSynth::new(sample_rate),
            wavetable_synth: crate::wavetable_synth::WavetableSynth::new(sample_rate),
            fx_buses: Vec::new(),
            master_fx: crate::fx::FxChainProcessor::new(sample_rate),
            rack: SynthRack::new(),
            monitor_rack: SynthRack::new(),
            pending_scene: None,
            count_in: None,
            pending_notes: [None; 64],
            rec_pad_press: [None; 16],
            rec_sounding: [None; 16],
            pending_note_offs: [None; 16],
            rec_key_press: [None; 128],
            arp_held: [0; 128],
            arp_last: None,
            arp_mode: 0,
            arp_falling: false,
            sixteen_velocities: false,
            automation_armed: false,
            automation_view: false,
            pending_patterns: [None; 8],
            launched_patterns: [None; 8],
            master_limiter: crate::limiter::MasterLimiter::new(sample_rate),
            plugin_slot: 0,
            plugin_slot_focus: (0, 0),
            event_cursor: 0,
            events_all: false,
            events_select_all: false,
            event_knob_accum: [0; 8],
            sample_page: 1,
            slice_count: 8,
            slice_transients: false,
            slice_sensitivity: 0.5,
            record_threshold_db: -30.0,
            record_max_secs: 30.0,
            record_wait: false,
            recorder: None,
            recordings_dir: std::env::temp_dir().join("maschine-linux-recordings"),
            sample_knob_accum: [0; 8],
            slice_cache: parking_lot::Mutex::new(None),
            fx_bus_bufs: Vec::new(),
            project_entries: Vec::new(),
            project_selected: 0,
            current_project_path: None,
            status_message: None,
            status_expires_at: std::time::Instant::now(),
            song_mode: false,
            active_section: None,
            pad_flash_until: [0; 16],
            pad_activity: [0.0; 16],
            scene_mode: false,
            pattern_mode: false,
            select_mode: false,
            erase_mode: false,
            controller_mode: 0,
            keyboard: crate::keyboard::KeyboardState::default(),
            click_accent: Self::metronome_click(sample_rate, 1760.0),
            click_normal: Self::metronome_click(sample_rate, 880.0),
            group_meter: [0.0; 8],
            group_peak_hold: [0.0; 8],
            pad_meter: [[0.0; 16]; 8],
            pad_peak_hold: [[0.0; 16]; 8],
            master_meter: 0.0,
            master_peak_hold: 0.0,
            note_repeat_enabled: false,
            note_repeat_rate: 1, // 1/16 par défaut
            note_repeat_held_pads: [None; 16],
            note_repeat_samples: 0,
            fixed_velocity: false,
            fixed_velocity_level: 100,
            synth_buf: vec![0.0; 4096],
            kick_sound_target: None,
            acid_sound_target: Some((0, 14)), // TB-303 sur Pad 14 par défaut
            wavetable_sound_target: None,
            shift_held: false,
            lock_active: false,
            pattern_chains: [None, None, None, None, None, None, None, None],
            revision: 0,
            undo_history: Vec::new(),
            redo_history: Vec::new(),
            step_undo_history: Vec::new(),
            step_redo_history: Vec::new(),
            pattern_clipboard: None,
        };
        eng.rescan_local_projects();
        // Synthés et bus d'effets du projet chargé, prêts avant la lecture.
        eng.sync_synth_rack();
        eng.sync_fx_buses();
        eng
    }

    /// Rescanne les projets locaux et ajuste la sélection pour l'écran FILE.
    pub fn rescan_local_projects(&mut self) {
        self.project_entries = scan_local_projects();
        if let Some(curr) = self.current_project_path.as_ref() {
            if let Some(pos) = self.project_entries.iter().position(|e| &e.path == curr) {
                self.project_selected = pos;
                return;
            }
        }
        if self.project_selected >= self.project_entries.len() {
            self.project_selected = self.project_entries.len().saturating_sub(1);
        }
    }

    /// Démo.
    #[must_use]
    pub fn new_demo(sample_rate: u32) -> Self {
        let mut eng = Self::new(Project::new_demo(), sample_rate);
        eng.load_default_808_kit();
        eng
    }

    fn push_project_history(&mut self) {
        if self.undo_history.len() >= 64 {
            self.undo_history.remove(0);
        }
        self.undo_history.push(Arc::clone(&self.project.read()));
        self.redo_history.clear();
    }

    fn undo_project(&mut self, redo: bool) {
        let (source, destination) = if redo {
            (&mut self.redo_history, &mut self.undo_history)
        } else {
            (&mut self.undo_history, &mut self.redo_history)
        };
        if let Some(previous) = source.pop() {
            if destination.len() >= 64 {
                destination.remove(0);
            }
            destination.push(Arc::clone(&self.project.read()));
            *self.project.write() = previous;
            self.status_message = Some(if redo { "Redo" } else { "Undo" }.to_string());
            self.status_expires_at =
                std::time::Instant::now() + std::time::Duration::from_millis(1200);
        } else {
            self.status_message = Some(
                if redo {
                    "Rien à refaire"
                } else {
                    "Rien à annuler"
                }
                .to_string(),
            );
            self.status_expires_at =
                std::time::Instant::now() + std::time::Duration::from_millis(1200);
        }
    }

    fn push_step_history(&mut self) {
        if self.step_undo_history.len() >= 64 {
            self.step_undo_history.remove(0);
        }
        self.step_undo_history
            .push(Arc::clone(&self.project.read()));
        self.step_redo_history.clear();
    }

    fn undo_step_project(&mut self, redo: bool) {
        let previous = if redo {
            self.step_redo_history.pop()
        } else {
            self.step_undo_history.pop()
        };
        if let Some(previous) = previous {
            let current = Arc::clone(&self.project.read());
            if redo {
                self.step_undo_history.push(current);
            } else {
                self.step_redo_history.push(current);
            }
            *self.project.write() = previous;
            self.status_message = Some(if redo { "Step Redo" } else { "Step Undo" }.to_string());
        } else {
            self.status_message = Some(
                if redo {
                    "Aucun step à refaire"
                } else {
                    "Aucun step à annuler"
                }
                .to_string(),
            );
        }
        self.status_expires_at = std::time::Instant::now() + std::time::Duration::from_millis(1200);
    }

    fn handle_shift_pad_action(&mut self, pad: u8) {
        match pad {
            0 => self.undo_project(false),
            1 => self.undo_project(true),
            2 => self.undo_step_project(false),
            3 => self.undo_step_project(true),
            4 => self.quantize_active_pattern(),
            5 => self.quantize_active_pattern_strength(0.5),
            6 => self.nudge_active_pattern(-60),
            7 => self.nudge_active_pattern(60),
            8 => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                let index =
                    project.active_pattern_index(self.group_focus as usize, self.active_scene);
                if let Some(pattern) = project
                    .groups
                    .get_mut(self.group_focus as usize)
                    .and_then(|group| index.and_then(|i| group.patterns.get_mut(i)))
                {
                    pattern.events.clear();
                }
            }
            9 => {
                self.status_message = Some(
                    "Clear Auto indisponible : ce projet ne gère pas encore l’automation"
                        .to_string(),
                );
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_secs(2);
            }
            10 => {
                let project = self.project.read();
                let index =
                    project.active_pattern_index(self.group_focus as usize, self.active_scene);
                self.pattern_clipboard = project
                    .groups
                    .get(self.group_focus as usize)
                    .and_then(|group| index.and_then(|i| group.patterns.get(i)))
                    .map(|pattern| pattern.events.clone());
                self.status_message = Some("Pattern copié".to_string());
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(1200);
            }
            11 => {
                if let Some(events) = self.pattern_clipboard.clone() {
                    let mut guard = self.project.write();
                    let project = project_mut(&mut guard);
                    let index =
                        project.active_pattern_index(self.group_focus as usize, self.active_scene);
                    if let Some(pattern) = project
                        .groups
                        .get_mut(self.group_focus as usize)
                        .and_then(|group| index.and_then(|i| group.patterns.get_mut(i)))
                    {
                        pattern.events = events;
                        self.status_message = Some("Pattern collé".to_string());
                        self.status_expires_at =
                            std::time::Instant::now() + std::time::Duration::from_millis(1200);
                    }
                } else {
                    self.status_message =
                        Some("Presse d'abord Shift+Pad 11 pour copier".to_string());
                    self.status_expires_at =
                        std::time::Instant::now() + std::time::Duration::from_secs(2);
                }
            }
            12 => self.transpose_active_pattern(-1),
            13 => self.transpose_active_pattern(1),
            14 => self.transpose_active_pattern(-12),
            15 => self.transpose_active_pattern(12),
            _ => {}
        }
    }

    fn quantize_active_pattern_strength(&mut self, strength: f32) {
        let grid = self.sequencer.grid;
        let mut guard = self.project.write();
        let project = project_mut(&mut guard);
        let index = project.active_pattern_index(self.group_focus as usize, self.active_scene);
        if let Some(pattern) = project
            .groups
            .get_mut(self.group_focus as usize)
            .and_then(|group| index.and_then(|i| group.patterns.get_mut(i)))
        {
            for event in &mut pattern.events {
                let snapped = grid.quantize(event.tick);
                event.tick = (event.tick as f32 + (snapped as f32 - event.tick as f32) * strength)
                    .round()
                    .max(0.0) as u32;
            }
            pattern.events.sort_by_key(|event| event.tick);
        }
    }

    fn nudge_active_pattern(&mut self, delta: i32) {
        let mut guard = self.project.write();
        let project = project_mut(&mut guard);
        let index = project.active_pattern_index(self.group_focus as usize, self.active_scene);
        if let Some(pattern) = project
            .groups
            .get_mut(self.group_focus as usize)
            .and_then(|group| index.and_then(|i| group.patterns.get_mut(i)))
        {
            let max_tick = pattern.length.to_ticks().saturating_sub(1);
            for event in &mut pattern.events {
                event.tick = (event.tick as i64 + delta as i64).clamp(0, max_tick as i64) as u32;
            }
            pattern.events.sort_by_key(|event| event.tick);
        }
    }

    fn transpose_active_pattern(&mut self, semitones: i8) {
        let mut guard = self.project.write();
        let project = project_mut(&mut guard);
        let index = project.active_pattern_index(self.group_focus as usize, self.active_scene);
        if let Some(pattern) = project
            .groups
            .get_mut(self.group_focus as usize)
            .and_then(|group| index.and_then(|i| group.patterns.get_mut(i)))
        {
            for event in &mut pattern.events {
                event.pitch = (event.pitch as i16 + semitones as i16).clamp(0, 127) as u8;
            }
        }
    }

    /// Handle partagé du projet (pour UI).
    #[must_use]
    pub fn project_handle(&self) -> ProjectHandle {
        ProjectHandle(Arc::clone(&self.project))
    }

    /// Snapshot pour UI — `project` est un `Arc` (O(1), pas de clone).
    pub fn snapshot(&self) -> EngineSnapshot {
        let project = Arc::clone(&self.project.read());
        let playing = self.transport.is_playing();
        let mut triggered_pads = [false; 16];
        if playing {
            for (i, triggered) in triggered_pads.iter_mut().enumerate() {
                *triggered = self.transport.tick <= self.pad_flash_until[i];
            }
        }
        let pattern_bars = project
            .active_pattern_index(self.group_focus as usize, self.active_scene)
            .and_then(|pi| {
                project
                    .groups
                    .get(self.group_focus as usize)?
                    .patterns
                    .get(pi)
            })
            .map(|p| (p.length.bars as usize).max(1))
            .unwrap_or(2);
        let selected_bar = self.selected_bar.min(pattern_bars.saturating_sub(1));
        let step_cursor = self.step_cursor.min(pattern_bars * 16 - 1);

        EngineSnapshot {
            project,
            transport: self.transport.clone(),
            group_focus: self.group_focus,
            active_scene: self.active_scene,
            bbt: self.transport.position_bbt(),
            num_voices: self.sampler.num_voices(),
            master_volume: self.master_volume,
            headphone_volume: self.headphone_volume,
            muted_groups: self.muted_groups,
            muted_sounds: self.muted_sounds,
            soloed_groups: self.soloed_groups,
            left_tab: self.left_tab,
            right_tab: self.right_tab,
            nav_view: self.nav_view,
            nav_serial: self.nav_serial,
            plugin_page: self.plugin_page,
            active_sound: self.active_sound,
            song_mode: self.song_mode,
            active_section: self.active_section,
            triggered_pads,
            pad_activity: self.pad_activity,
            scene_mode: self.scene_mode,
            pattern_mode: self.pattern_mode,
            select_mode: self.select_mode,
            erase_mode: self.erase_mode,
            controller_mode: self.controller_mode,
            keyboard: self.keyboard,
            browser_preset_idx: self.browser_preset_idx,
            browser_kind: self.browser_kind,
            project_entries: self.project_entries.clone(),
            project_selected: self.project_selected,
            current_project_path: self.current_project_path.clone(),
            status_message: if std::time::Instant::now() < self.status_expires_at {
                self.status_message.clone()
            } else {
                None
            },
            selected_bar,
            step_cursor,
            pattern_bars,
            group_levels: self.group_meter,
            group_peaks: self.group_peak_hold,
            pad_levels: self.pad_meter[self.group_focus as usize % 8],
            pad_peaks: self.pad_peak_hold[self.group_focus as usize % 8],
            master_level: self.master_meter,
            master_peak: self.master_peak_hold,
            soft_labels: self.soft_key_labels(),
            soft_active: self.soft_key_active(),
            page_flash: self.page_changed_at.map_or(0.0, |t| {
                (1.0 - t.elapsed().as_secs_f32() / 0.9).clamp(0.0, 1.0)
            }),
            waveform: self
                .sampler
                .waveform_envelope(sampler_sound_id(self.group_focus, self.active_sound), 512),
            sample_duration: self
                .sampler
                .sample_duration_secs(sampler_sound_id(self.group_focus, self.active_sound)),
            acid_params: self.acid_synth.params.clone(),
            kick_params: self.kick_synth.params.clone(),
            wavetable_params: self.wavetable_synth.params.clone(),
            focus_sound_kick: self.focus_sound_kick_params(),
            focus_sound_acid: self.focus_sound_synth_params(),
            focus_sound_wavetable: self.focus_sound_wavetable_params(),
            focus_sound_fx: self.focus_sound_fx(),
            focus_group_fx: self.focus_group_fx(),
            plugin_slots: self.plugin_slot_views(),
            plugin_slot: self.plugin_slot_index(),
            plugin_fx: self.selected_fx().map(|(_, _, slot)| slot),
            event_sel: self.selected_event().map(|(_, e)| e),
            event_index: self
                .event_cursor
                .min(self.event_scope().len().saturating_sub(1)),
            event_count: self.event_scope().len(),
            events_all: self.events_all,
            events_select_all: self.events_select_all,
            sample_page: self.sample_page,
            sample_params: self.sample_param_views(),
            sample_range: self.focus_range(),
            slice_points: self.slice_points(),
            pending_scene: self.pending_scene,
            pending_patterns: self.pending_patterns,
            record: self.recorder.as_ref().map(|r| RecordView {
                waiting: r.waiting,
                secs: r.buf.len() as f32 / self.transport.sample_rate.max(1) as f32,
                max_secs: r.buf.capacity() as f32 / self.transport.sample_rate.max(1) as f32,
                level: r.level,
            }),
            mixer_mode: self.mixer_mode,
            browser_plugin_idx: self.browser_plugin_idx,
            browser_fx_idx: self.browser_fx_idx,
            delete_project_confirm: self.delete_project_confirm,
            note_repeat_enabled: self.note_repeat_enabled,
            note_repeat_rate: self.note_repeat_rate,
            fixed_velocity: self.fixed_velocity,
            fixed_velocity_level: self.fixed_velocity_level,
            sixteen_velocities: self.sixteen_velocities,
            automation_armed: self.automation_armed,
            automation_view: self.automation_view,
            arp_mode: self.arp_mode,
            shift_held: self.shift_held,
            lock_active: self.lock_active,
            pattern_chains: self.pattern_chains.clone(),
            synth_param_page: self.synth_param_page,
            revision: self.revision,
        }
    }

    /// Instantané compact pour synchroniser les LED sans cloner le projet.
    pub fn hardware_snapshot(&self) -> HardwareSnapshot {
        // Avant de verrouiller le projet : ces calculs le relisent eux-mêmes.
        let soft_present = self.soft_key_labels().map(|label| !label.is_empty());
        let soft_active = self.soft_key_active();
        let page_nav = self.plugin_instrument().map(|inst| {
            let page = self.plugin_page_index();
            [page > 0, page + 1 < inst.page_count()]
        });
        let project = self.project.read();
        let playing = self.transport.is_playing();
        let recording = self.transport.state == TransportState::Recording;
        let pattern_bars = project
            .active_pattern_index(self.group_focus as usize, self.active_scene)
            .and_then(|pattern_index| {
                project
                    .groups
                    .get(self.group_focus as usize)?
                    .patterns
                    .get(pattern_index)
            })
            .map(|pattern| (pattern.length.bars as usize).max(1))
            .unwrap_or(2);
        let selected_bar = self.selected_bar.min(pattern_bars.saturating_sub(1));
        let current_bar = ((self.transport.tick / (4 * 960)) as usize) % pattern_bars;
        let current_step = ((self.transport.tick / 240) % 16) as usize;
        let mut triggered_pads = [false; 16];
        if playing {
            for (index, triggered) in triggered_pads.iter_mut().enumerate() {
                *triggered = self.transport.tick <= self.pad_flash_until[index];
            }
        }

        let mut active_steps = [false; 16];
        let group = project.groups.get(self.group_focus as usize);
        if let Some(pattern) = project
            .active_pattern_index(self.group_focus as usize, self.active_scene)
            .and_then(|pattern_index| group.and_then(|g| g.patterns.get(pattern_index)))
        {
            let bar_offset = selected_bar as u32 * 4 * 960;
            for event in &pattern.events {
                if event.pad == self.active_sound
                    && event.tick >= bar_offset
                    && event.tick < bar_offset + 4 * 960
                {
                    let step = ((event.tick - bar_offset) / 240) as usize;
                    if step < active_steps.len() {
                        active_steps[step] = true;
                    }
                }
            }
        }

        let mut pad_colors = [None; 16];
        if let Some(group) = group {
            for (index, color) in pad_colors.iter_mut().enumerate() {
                *color = group.sounds[index].as_ref().map(|sound| sound.color);
            }
        }
        let group_num_patterns = group.map(|g| g.patterns.len().min(16)).unwrap_or(0);
        let scene_pattern = self.active_scene.and_then(|scene_index| {
            project
                .scenes
                .get(scene_index)
                .and_then(|scene| scene.patterns.get(self.group_focus as usize))
                .copied()
                .flatten()
        });

        let pattern_chain_mask = self.pattern_chains[self.group_focus as usize % 8]
            .as_ref()
            .map(|chain| {
                chain.iter().fold(
                    0u16,
                    |mask, &i| {
                        if i < 16 {
                            mask | (1u16 << i)
                        } else {
                            mask
                        }
                    },
                )
            })
            .unwrap_or(0);

        HardwareSnapshot {
            group_focus: self.group_focus,
            active_sound: self.active_sound,
            left_tab: self.left_tab,
            right_tab: self.right_tab,
            browser_kind: self.browser_kind,
            mixer_mode: self.mixer_mode,
            has_kick_synth: self.focus_sound_has_kick(),
            has_acid_synth: self.focus_sound_has_acid(),
            has_wavetable_synth: self.focus_sound_has_wavetable(),
            delete_project_confirm: self.delete_project_confirm,
            plugin_page: self.plugin_page,
            synth_param_page: self.synth_param_page,
            controller_mode: self.controller_mode,
            keyboard: self.keyboard,
            is_playing: playing,
            is_recording: recording,
            is_stopped: !playing && !recording,
            current_bar,
            current_step,
            is_quarter_beat: (self.transport.tick % 960) < 240 && playing,
            active_steps,
            triggered_pads,
            pad_activity: self.pad_activity,
            pad_levels: self.pad_meter[self.group_focus as usize % 8],
            group_levels: self.group_meter,
            master_volume: self.master_volume,
            muted_groups: self.muted_groups,
            muted_sounds: self.muted_sounds,
            soloed_groups: self.soloed_groups,
            active_scene: self.active_scene,
            num_scenes: project.scenes.len().min(16),
            group_num_patterns,
            scene_pattern,
            pattern_chain_mask,
            loop_enabled: self.transport.loop_enabled,
            metronome: self.transport.metronome,
            selected_bar,
            pattern_bars,
            step_cursor: self.step_cursor.min(pattern_bars * 16 - 1),
            pad_colors,
            note_repeat_enabled: self.note_repeat_enabled,
            shift_held: self.shift_held,
            lock_active: self.lock_active,
            soft_present,
            soft_active,
            page_nav,
            group_colors: std::array::from_fn(|g| {
                project.groups.get(g).map_or(0, |grp| {
                    if grp.sounds.iter().any(Option::is_some) {
                        grp.color
                    } else {
                        0
                    }
                })
            }),
            pending_scene: self.pending_scene,
            pending_pattern: self.pending_patterns[self.group_focus as usize % 8],
            count_in: self.count_in.is_some(),
            sixteen_velocities: self.sixteen_velocities,
            automation_armed: self.automation_armed,
            fixed_velocity: self.fixed_velocity,
        }
    }

    /// Charge le kit 808 complet DSP dans le sampler et initialise les zones des 8 groupes
    pub fn load_default_808_kit(&mut self) {
        let kit = crate::drum_synth::generate_complete_808_kit(self.transport.sample_rate);
        let kit_arcs: Vec<std::sync::Arc<Vec<f32>>> = kit
            .into_iter()
            .map(|(pad, _name, samp)| {
                let arc = std::sync::Arc::new(samp);
                self.preset_samples[pad as usize] = Some(arc.clone());
                arc
            })
            .collect();

        // Initialise les zones d'échantillons pour les 8 groupes (0..8) et leurs 16 pads
        for group in 0..8u8 {
            for pad in 0..16u8 {
                let sound_id = sampler_sound_id(group, pad);
                self.sampler.clear_sound(sound_id);
                let sample = kit_arcs
                    .get(pad as usize)
                    .cloned()
                    .unwrap_or_else(|| self.drum_preset_sample(pad));
                self.sampler.add_zone(
                    sound_id,
                    crate::sampler::ZoneMapping {
                        sample: Some(sample),
                        waveform_cache: None,
                        sample_path: None,
                        sample_rate: self.transport.sample_rate,
                        root_key: 36 + pad,
                        lokey: 0,
                        hikey: 127,
                        lovel: 0,
                        hivel: 127,
                        volume_db: 0.0,
                        pan: 0.0,
                        tune_cents: 0,
                        choke_group: if pad == 2 || pad == 3 { Some(1) } else { None }, // Choke Closed HH <-> Open HH
                        is_one_shot: true,
                    },
                );
            }
        }
        self.transport.loop_enabled = true;
        self.transport.loop_start = 0;
        self.transport.loop_end = 2 * 4 * 960;
    }

    fn default_runtime_sampler(&self, project: &Project) -> Sampler {
        let mut sampler = Sampler::new(self.transport.sample_rate);
        for (group_index, group) in project.groups.iter().enumerate() {
            let Ok(group_id) = u8::try_from(group_index) else {
                continue;
            };
            for (sound_index, sound) in group.sounds.iter().enumerate() {
                let Some(sound) = sound else { continue };
                let saved_preset = DRUM_PRESET_NAMES
                    .iter()
                    .position(|name| *name == sound.name);
                let preset = saved_preset.or((group_index == 0).then_some(sound_index));
                let Some(preset) = preset.filter(|index| *index < 16) else {
                    continue;
                };
                let sample = self.drum_preset_sample(preset as u8);
                let sound_index = sound_index as u8;
                sampler
                    .set_sound_polyphony(sampler_sound_id(group_id, sound_index), sound.polyphony);
                sampler.add_zone(
                    sampler_sound_id(group_id, sound_index),
                    crate::sampler::ZoneMapping {
                        sample: Some(sample),
                        sample_path: None,
                        sample_rate: self.transport.sample_rate,
                        root_key: 36 + sound_index,
                        lokey: 0,
                        hikey: 127,
                        lovel: 0,
                        hivel: 127,
                        choke_group: if preset == 2 || preset == 3 {
                            Some(1)
                        } else {
                            None
                        },
                        ..Default::default()
                    },
                );
            }
        }
        sampler
    }

    /// Recharge les WAV référencés par le projet, résolus relativement au
    /// fichier projet lorsqu'ils ne sont pas des chemins absolus.
    pub fn restore_project_samples(&mut self, project_path: &std::path::Path) -> crate::Result<()> {
        let project = Arc::clone(&self.project.read());
        let sample_zones = Self::prepare_project_samples(&project, project_path)?;
        self.sampler = self.default_runtime_sampler(&project);
        for (sound_id, zones) in sample_zones {
            self.sampler.clear_sound(sound_id);
            for zone in zones {
                self.sampler.add_zone(sound_id, zone);
            }
        }
        restore_sound_edits(&mut self.sampler, &project);
        // `clear_sound` réinitialise la polyphonie : on la restaure depuis le
        // projet après le rechargement des zones personnalisées.
        for (group_index, group) in project.groups.iter().enumerate() {
            let Ok(group_id) = u8::try_from(group_index) else {
                continue;
            };
            for (sound_index, sound) in group.sounds.iter().enumerate() {
                let Some(sound) = sound else { continue };
                let Ok(sound_index) = u8::try_from(sound_index) else {
                    continue;
                };
                self.sampler
                    .set_sound_polyphony(sampler_sound_id(group_id, sound_index), sound.polyphony);
            }
        }
        Ok(())
    }

    #[cfg(feature = "sampler")]
    fn prepare_project_samples(
        project: &Project,
        project_path: &std::path::Path,
    ) -> crate::Result<Vec<(u16, Vec<crate::sampler::ZoneMapping>)>> {
        let project_dir = project_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let mut result = Vec::new();
        for (group_index, group) in project.groups.iter().enumerate() {
            let group_id = u8::try_from(group_index).map_err(|_| {
                crate::EngineError::Validation("project has more than 256 groups".to_string())
            })?;
            for (sound_index, sound) in group.sounds.iter().enumerate() {
                let Some(sound) = sound else { continue };
                let mut zones = Vec::new();
                for mapping in &sound.zones {
                    let Some(sample_path) = mapping.sample_path.as_deref() else {
                        continue;
                    };
                    let path = std::path::Path::new(sample_path);
                    let resolved = if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        project_dir.join(path)
                    };
                    zones.push(Sampler::prepare_wav_for_zone(&resolved, mapping)?);
                }
                if !zones.is_empty() {
                    result.push((sampler_sound_id(group_id, sound_index as u8), zones));
                }
            }
        }
        Ok(result)
    }

    #[cfg(not(feature = "sampler"))]
    fn prepare_project_samples(
        _project: &Project,
        _project_path: &std::path::Path,
    ) -> crate::Result<Vec<(u16, Vec<crate::sampler::ZoneMapping>)>> {
        Ok(Vec::new())
    }

    fn install_sample_zone(
        &mut self,
        group: u8,
        sound: u8,
        zone: crate::sampler::ZoneMapping,
    ) -> crate::Result<()> {
        if sound >= 16 || group as usize >= self.project.read().groups.len() {
            return Err(crate::EngineError::Validation(format!(
                "sample target out of range: group {group}, sound {sound}"
            )));
        }
        let sample_path = zone.sample_path.clone().ok_or_else(|| {
            crate::EngineError::Validation("loaded sample has no source path".to_string())
        })?;
        let sound_id = sampler_sound_id(group, sound);
        self.sampler.clear_sound(sound_id);
        self.sampler.add_zone(sound_id, zone.clone());

        let mut guard = self.project.write();
        let project = project_mut(&mut guard);
        let group = &mut project.groups[group as usize];
        let name = std::path::Path::new(&sample_path)
            .file_stem()
            .and_then(|part| part.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("Sample")
            .to_string();
        let target = &mut group.sounds[sound as usize];
        let target_sound = target.get_or_insert_with(|| crate::project::Sound::new(&name, 78));
        target_sound.name = name;
        target_sound.zones = vec![Zone {
            sample_path: Some(sample_path),
            root_key: zone.root_key,
            lokey: zone.lokey,
            hikey: zone.hikey,
            lovel: zone.lovel,
            hivel: zone.hivel,
            tune_cents: zone.tune_cents,
            pan: zone.pan,
            volume_db: zone.volume_db,
            ..Zone::default()
        }];
        Ok(())
    }

    /// Réaligne l'horloge transport/sampler sur le taux du périphérique sans
    /// régénérer les presets (appelable depuis le thread audio : aucune
    /// allocation). Filet de sécurité si le device change de taux tout seul.
    pub fn align_sample_rate_clock(&mut self, sample_rate: u32) {
        let rate = sample_rate.max(8000);
        self.transport.sample_rate = rate;
        self.sampler.sample_rate = rate;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Aligne le moteur sur le taux d'échantillonnage réel du périphérique audio.
    ///
    /// Les presets de batterie sont synthétisés : sans cette réalisation, un
    /// device à 44,1 kHz jouait un kit généré à 48 kHz ≈ 1,5 demi-ton trop haut.
    /// Les presets synthétiques sont régénérés et les zones du sampler
    /// repointées (les samples chargés depuis un fichier gardent leur taux).
    pub fn set_audio_sample_rate(&mut self, sample_rate: u32) {
        let rate = sample_rate.max(8000);
        if rate == self.transport.sample_rate && rate == self.sampler.sample_rate {
            return;
        }
        self.align_sample_rate_clock(rate);
        // Inserts recréés au nouveau taux (temps de délai, coefficients).
        self.fx_buses.clear();
        self.master_fx = crate::fx::FxChainProcessor::new(rate);
        self.master_limiter = crate::limiter::MasterLimiter::new(rate);
        self.rack = SynthRack::new();
        self.sync_synth_rack();
        self.sync_fx_buses();
        self.click_accent = Self::metronome_click(rate, 1760.0);
        self.click_normal = Self::metronome_click(rate, 880.0);
        for i in 0..16usize {
            let Some(old) = self.preset_samples.get(i).and_then(Option::as_ref).cloned() else {
                continue;
            };
            self.preset_samples[i] = None;
            let new = self.drum_preset_sample(i as u8);
            self.preset_samples[i] = Some(new.clone());
            self.sampler
                .replace_zone_sample(sampler_sound_id(0, i as u8), &old, new, rate);
        }
    }

    /// Humanise le pattern actif (légères variations de vélocité et micro-timing)
    pub fn humanize_active_pattern(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(self.group_focus as usize, self.active_scene);
        if let Some(group) = proj.groups.get_mut(self.group_focus as usize) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    self.sequencer.humanize(&mut pat.events, 8, 12);
                }
            }
        }
    }

    /// Quantifie le pattern actif sur la grille courante
    pub fn quantize_active_pattern(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(self.group_focus as usize, self.active_scene);
        if let Some(group) = proj.groups.get_mut(self.group_focus as usize) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    self.sequencer.quantize_pattern(pat);
                }
            }
        }
    }

    /// Déclenche la voix sonore associée à un pad pour une note et vélocité données.
    pub fn trigger_sound_voice(&mut self, pad: u8, note: u8, velocity: u8) {
        let sound = sampler_sound_id(self.group_focus, pad);
        self.note_pad_activity(pad, velocity);
        let guard = self.project.read();
        let (group_gain, group_pan) = guard
            .groups
            .get(self.group_focus as usize)
            .map(|g| (db_to_linear(g.volume_db), g.pan))
            .unwrap_or((1.0, 0.0));
        let snd_ref = guard
            .groups
            .get(self.group_focus as usize)
            .and_then(|g| g.sounds.get(pad as usize))
            .and_then(|s| s.as_ref());
        let synth_params = snd_ref.and_then(|s| s.synth.clone());
        let kick_params = snd_ref.and_then(|s| s.kick_synth.clone());
        let wavetable_params = snd_ref.and_then(|s| s.wavetable_synth.clone());
        let is_synth_sound = synth_params.is_some();
        let is_kick_sound = kick_params.is_some();
        let is_wavetable_sound = wavetable_params.is_some();
        if snd_ref.is_none() {
            return;
        }
        let (snd_gain, snd_pan, vparams) = snd_ref
            .map(|s| {
                let filter_off = s.filter.kind == crate::project::FilterKind::Off;
                (
                    db_to_linear(s.volume_db),
                    s.pan,
                    crate::sampler::VoiceParams {
                        attack_ms: s.adsr.attack_ms,
                        decay_ms: s.adsr.decay_ms,
                        sustain: s.adsr.sustain,
                        release_ms: s.adsr.release_ms,
                        cutoff_hz: if filter_off {
                            20000.0
                        } else {
                            s.filter.cutoff_hz
                        },
                        resonance: if filter_off { 0.0 } else { s.filter.resonance },
                        kind: s.filter.kind,
                    },
                )
            })
            .unwrap_or((1.0, 0.0, crate::sampler::VoiceParams::default()));
        drop(guard);
        // Synthés : l'instance de ce Sound dans le rack (créée au besoin, ce
        // n'est pas le thread audio) ; le synthé « édition » suit le Sound joué.
        let i = rack_index(self.group_focus, pad);
        // Pendant REC, la note jouée en direct a sa propre voix : le pattern
        // continue de sonner sur `rack` et le monitoring sur `monitor_rack`.
        let live_monitor =
            self.transport.state == TransportState::Recording && pad == self.active_sound;
        let rack = if live_monitor {
            &mut self.monitor_rack
        } else {
            &mut self.rack
        };
        if is_kick_sound {
            self.kick_sound_target = Some((self.group_focus, pad));
            if let Some(p) = kick_params {
                self.kick_synth.params = p.clone();
                let sr = self.transport.sample_rate;
                let slot = rack.kick[i]
                    .get_or_insert_with(|| RackSlot::new(crate::kick_synth::KickSynth::new(sr)));
                slot.synth.params = p;
                slot.idle_blocks = 0;
                slot.synth.trigger(note, velocity);
            }
        } else if is_synth_sound {
            self.acid_sound_target = Some((self.group_focus, pad));
            if let Some(p) = synth_params {
                self.acid_synth.params = p.clone();
                let sr = self.transport.sample_rate;
                let slot = rack.acid[i]
                    .get_or_insert_with(|| RackSlot::new(crate::acid_synth::AcidSynth::new(sr)));
                slot.synth.params = p;
                slot.idle_blocks = 0;
                if live_monitor {
                    // Monitoring live : coupe nette de la note précédente,
                    // nouvelle note immédiate (pas de portamento).
                    slot.synth.cut_gate();
                    slot.synth.note_on_instant(note, velocity);
                } else {
                    slot.synth.note_on(note, velocity);
                }
            }
        } else if is_wavetable_sound {
            self.wavetable_sound_target = Some((self.group_focus, pad));
            if let Some(p) = wavetable_params {
                self.wavetable_synth.params = p.clone();
                let sr = self.transport.sample_rate;
                let slot = rack.wavetable[i].get_or_insert_with(|| {
                    RackSlot::new(crate::wavetable_synth::WavetableSynth::new(sr))
                });
                slot.synth.params = p;
                slot.idle_blocks = 0;
                if live_monitor {
                    slot.synth.all_notes_off();
                }
                slot.synth.note_on(note, velocity);
            }
        } else if let Some(idx) = self.sampler.note_on(sound, note, velocity, note) {
            self.sampler.set_voice_gain(idx, group_gain * snd_gain);
            self.sampler.set_voice_pan(idx, snd_pan);
            self.sampler.set_voice_group(idx, self.group_focus);
            self.sampler.set_voice_group_pan(idx, group_pan);
            self.sampler.set_voice_params(idx, &vparams);
        }
    }

    /// Relâche la note pour les synthétiseurs et le sampler.
    pub fn release_sound_voice(&mut self, pad: u8, note: u8) {
        let sound = sampler_sound_id(self.group_focus, pad);
        let i = rack_index(self.group_focus, pad);
        // La voix release est celle du monitoring pendant REC (le pattern
        // garde sa propre voix sur `rack`).
        let live_monitor =
            self.transport.state == TransportState::Recording && pad == self.active_sound;
        let rack = if live_monitor {
            &mut self.monitor_rack
        } else {
            &mut self.rack
        };
        if let Some(slot) = rack.acid[i].as_mut() {
            slot.synth.note_off(note);
        }
        if let Some(slot) = rack.wavetable[i].as_mut() {
            slot.synth.note_off(note);
        }
        self.sampler.note_off(sound, note);
    }

    /// Déclenche un son de pad (Kick Synth, TB-303, Wavetable ou Sampler)
    pub fn trigger_pad_voice(&mut self, pad: u8, velocity: u8) {
        if !self.group_audible(self.group_focus) || self.is_sound_muted(self.group_focus, pad) {
            return;
        }
        self.trigger_sound_voice(pad, 36 + pad, velocity);
    }

    /// Traite un `EngineEvent`.
    pub fn handle_event(&mut self, ev: EngineEvent) -> crate::Result<()> {
        self.commit_launches();
        let result = self.apply_event(ev);
        // Chaînes d'effets et synthés modifiés (ajout, chargement, undo…).
        self.sync_fx_buses();
        self.sync_synth_rack();
        result
    }

    /// Ré-appui sur le pad du pattern actif : on le coupe instantanément
    /// (désassigné de la scène), comme sur la Maschine MK3.
    fn cut_active_pattern(&mut self, group_idx: usize, pattern: usize) -> bool {
        if group_idx >= 8 {
            return false;
        }
        {
            let guard = self.project.read();
            let proj: &crate::project::Project = &guard;
            if proj.active_pattern_index(group_idx, self.active_scene) != Some(pattern) {
                return false;
            }
        }
        self.pending_patterns[group_idx] = None;
        self.pattern_chains[group_idx] = None;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        if let Some(scene_idx) = self.active_scene {
            if let Some(scene) = proj.scenes.get_mut(scene_idx) {
                if let Some(slot) = scene.patterns.get_mut(group_idx) {
                    *slot = None;
                }
            }
        }
        true
    }

    fn apply_event(&mut self, ev: EngineEvent) -> crate::Result<()> {
        let records_history = matches!(
            &ev,
            EngineEvent::ClearPattern { .. }
                | EngineEvent::DuplicatePattern { .. }
                | EngineEvent::DeletePattern { .. }
                | EngineEvent::AddPattern { .. }
                | EngineEvent::CreatePatternAt { .. }
                | EngineEvent::RenamePattern { .. }
                | EngineEvent::EraseSound { .. }
                | EngineEvent::ClearSoundSlot { .. }
                | EngineEvent::Quantize { .. }
                | EngineEvent::Variation
                | EngineEvent::QuantizeActive
                | EngineEvent::StepVelocityDelta(_)
                | EngineEvent::StepNudgeTicks(_)
                | EngineEvent::StepLengthDelta(_)
                | EngineEvent::StepPitchDelta(_)
                | EngineEvent::SetPatternBars(_)
                | EngineEvent::ShiftPadAction(4..=8 | 11..=15)
        );
        if records_history {
            self.push_project_history();
        }
        if matches!(
            &ev,
            EngineEvent::StepVelocityDelta(_)
                | EngineEvent::StepNudgeTicks(_)
                | EngineEvent::StepLengthDelta(_)
                | EngineEvent::StepPitchDelta(_)
        ) {
            self.push_step_history();
        }
        // Toute action utilisateur invalide l'affichage (repaint UI + écrans).
        self.revision = self.revision.wrapping_add(1);
        match ev {
            EngineEvent::Pad {
                pad,
                mut velocity,
                pressed,
            } => {
                // La vélocité fixe prime sur la pression réelle : elle
                // s'applique au jeu live comme à l'enregistrement.
                if self.fixed_velocity {
                    velocity = self.fixed_velocity_level;
                }
                // 16 VEL : le pad choisit la vélocité (pad 1 = 7 … pad 16 = 127)
                // du Sound sélectionné, sur la MK3 comme dans la GUI.
                let pad = if self.sixteen_velocities && pad < 16 {
                    velocity = (pad + 1) * 8 - 1;
                    self.active_sound
                } else {
                    pad
                };
                if pressed {
                    self.trigger_pad_voice(pad, velocity);
                    if self.note_repeat_enabled && (pad as usize) < 16 {
                        self.note_repeat_held_pads[pad as usize] = Some(velocity);
                        self.note_repeat_samples = 0;
                    }
                    // si Recording, ajoute event au pattern actif
                    if self.transport.state == TransportState::Recording && (pad as usize) < 16 {
                        // Slide : la note précédente du même pad est encore
                        // sonore (tenue) — on étend son événement enregistré
                        // jusqu'à ce nouvel événement (glissando legato TB-303).
                        let prev = self.rec_sounding[pad as usize].take();
                        let q_new = self.record_pad_event(pad, velocity);
                        if let (Some(q_prev), Some(q_new)) = (prev, q_new) {
                            self.extend_recorded_note(self.group_focus, pad, q_prev, q_new);
                        }
                        self.rec_pad_press[pad as usize] = q_new;
                        self.rec_sounding[pad as usize] = q_new;
                    }
                } else {
                    let mut scheduled = false;
                    if (pad as usize) < 16 {
                        self.note_repeat_held_pads[pad as usize] = None;
                        self.rec_sounding[pad as usize] = None;
                        if let Some(q) = self.rec_pad_press[pad as usize].take() {
                            self.set_recorded_length(self.group_focus, pad, q, self.transport.tick);
                            if self.transport.state == TransportState::Recording {
                                self.schedule_note_off(pad, 36 + pad, q, self.transport.tick);
                                scheduled = true;
                            }
                        }
                    }
                    if !scheduled {
                        self.release_sound_voice(pad, 36 + pad);
                    }
                }
            }
            EngineEvent::Transport { play, rec, stop } => {
                // REC à l'arrêt avec COUNT-IN activé : une mesure de clics d'abord.
                if rec && !stop && !self.transport.is_playing() && self.transport.count_in_bars > 0
                {
                    self.start_count_in();
                } else {
                    self.count_in = None;
                    if play {
                        self.transport.play();
                    }
                    if rec {
                        self.transport.record();
                        self.clear_rec_press_marks();
                    }
                }
                if stop {
                    self.clear_rec_press_marks();
                    self.count_in = None;
                    // STOP déjà à l'arrêt : retour au début (comme la MK3).
                    if !self.transport.is_playing() {
                        self.transport.tick = 0;
                        self.transport.tick_fraction = 0.0;
                        self.selected_bar = 0;
                    }
                    self.transport.stop();
                    // Plus de mesure suivante : les lancements prennent effet.
                    if let Some(sc) = self.pending_scene.take() {
                        self.active_scene = Some(sc);
                    }
                    for g in 0..8 {
                        if let Some(p) = self.pending_patterns[g].take() {
                            self.launched_patterns[g] = Some(p);
                        }
                    }
                }
            }
            EngineEvent::Bpm(bpm) => {
                self.transport.bpm = bpm.clamp(20.0, 999.0);
                project_mut(&mut self.project.write()).bpm = self.transport.bpm;
            }
            EngineEvent::GroupFocus(g) => {
                self.group_focus = g.min(7);
                self.sync_acid_from_active_sound();
                self.sync_kick_from_active_sound();
                self.sync_wavetable_from_active_sound();
                if self.left_tab == 4 && self.plugin_page != 2 {
                    self.plugin_page = if self.focus_sound_has_synth() { 1 } else { 0 };
                }
            }
            EngineEvent::Scene(s) => {
                let len = self.project.read().scenes.len();
                if s < len {
                    if self.transport.is_playing() && self.active_scene != Some(s) {
                        // En lecture : la scène démarre à la mesure suivante.
                        self.pending_scene = Some(s);
                        self.show_status(format!("Scène {} à la prochaine mesure", s + 1));
                    } else {
                        self.pending_scene = None;
                        self.active_scene = Some(s);
                    }
                }
            }
            EngineEvent::AddScene => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                let index = project.scenes.len();
                let group_count = project.groups.len();
                project
                    .scenes
                    .push(Scene::new(&format!("Scène {}", index + 1), group_count));
                self.active_scene = Some(index);
                self.song_mode = false;
            }
            EngineEvent::Quantize { group, pattern } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group) {
                    if let Some(p) = g.patterns.get_mut(pattern) {
                        self.sequencer.quantize_pattern(p);
                    }
                }
            }
            EngineEvent::Save(path) => {
                self.project.read().save_json_with(&path, true)?;
                self.current_project_path = Some(path.to_string_lossy().to_string());
                self.status_message = Some("Projet sauvegardé".to_string());
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_secs(2);
                self.rescan_local_projects();
            }
            EngineEvent::SaveCurrent { quick } => {
                let path = if let Some(p) = self
                    .current_project_path
                    .clone()
                    .filter(|s| !s.trim().is_empty())
                {
                    std::path::PathBuf::from(p)
                } else {
                    // Nouvelle sauvegarde : crée un fichier lisible dans le répertoire projets.
                    let name = self.project.read().name.clone();
                    let safe = sanitize_project_filename(&name);
                    let dir = default_projects_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    dir.join(safe)
                };
                self.project.read().save_json_with(&path, !quick)?;
                self.current_project_path = Some(path.to_string_lossy().to_string());
                self.status_message = Some(if quick {
                    "Sauvegarde rapide OK".to_string()
                } else {
                    "Projet sauvegardé".to_string()
                });
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_secs(2);
                self.rescan_local_projects();
            }
            EngineEvent::NewProject { name } => {
                let (proj_name, path) = generate_unique_project_path(&name);
                let mut proj = Project::new_demo();
                proj.name = proj_name.clone();
                // Un nouveau projet repart sur un kit 808 complet et vierge,
                // indépendant de la disposition « familles » de la démo.
                if let Some(g) = proj.groups.first_mut() {
                    g.name = "Kit 808".to_string();
                    g.color = 22;
                    for (i, sound_name) in DRUM_PRESET_NAMES.iter().enumerate() {
                        let mut sound = crate::project::Sound::new(sound_name, 22);
                        sound.zones.push(crate::project::Zone {
                            sample_path: None,
                            root_key: 36 + i as u8,
                            lokey: 0,
                            hikey: 127,
                            lovel: 0,
                            hivel: 127,
                            ..Default::default()
                        });
                        g.sounds[i] = Some(sound);
                    }
                    g.patterns.clear();
                    g.patterns.push(crate::project::Pattern::new(
                        "Pattern 1",
                        crate::project::BarsBeatsTicks::new(2, 0, 0),
                    ));
                }
                self.load_default_808_kit();
                proj.save_json_with(&path, false)?;
                *self.project.write() = std::sync::Arc::new(proj);
                self.current_project_path = Some(path.to_string_lossy().to_string());
                self.transport.tick = 0;
                self.selected_bar = 0;
                self.step_cursor = 0;
                self.status_message = Some(format!("Nouveau projet: {}", proj_name));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_secs(3);
                self.rescan_local_projects();
            }
            EngineEvent::SetProjectPath(path) => {
                self.current_project_path = path;
                self.rescan_local_projects();
            }
            EngineEvent::Load(path) => {
                let p = Project::load_json(&path)?;
                let sample_zones = Self::prepare_project_samples(&p, &path)?;
                let mut sampler = self.default_runtime_sampler(&p);
                for (sound_id, zones) in sample_zones {
                    sampler.clear_sound(sound_id);
                    for zone in zones {
                        sampler.add_zone(sound_id, zone);
                    }
                }
                restore_sound_edits(&mut sampler, &p);
                self.sampler = sampler;
                self.transport.bpm = p.bpm;
                let proj_name = p.name.clone();
                *self.project.write() = Arc::new(p);
                self.current_project_path = Some(path.to_string_lossy().to_string());
                self.sync_acid_from_active_sound();
                if self.left_tab == 4 {
                    self.sync_plugin_for_active_sound();
                }
                self.status_message = Some(format!("Projet chargé: {}", proj_name));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_secs(2);
                self.rescan_local_projects();
            }
            EngineEvent::MasterVolume(v) => {
                self.master_volume = v.clamp(0.0, 1.0);
            }
            EngineEvent::HeadphoneVolume(v) => {
                self.headphone_volume = v.clamp(0.0, 1.0);
            }
            EngineEvent::Mute(g) => {
                if g < 8 {
                    self.muted_groups ^= 1 << g;
                }
            }
            EngineEvent::TogglePadMute { group, pad } => {
                if (group as usize) < 8 && pad < 16 {
                    self.muted_sounds[group as usize] ^= 1u16 << pad;
                }
            }
            EngineEvent::Solo(g) => {
                if g < 8 {
                    self.soloed_groups ^= 1 << g;
                }
            }
            EngineEvent::PatternSelect { group, pattern } => {
                let group_idx = group as usize;
                self.pattern_chains[group_idx] = None;
                // Ré-appui sur le pad actif : coupe instantanée (MK3).
                if self.cut_active_pattern(group_idx, pattern) {
                    self.show_status(format!("Pattern {} coupé", pattern + 1));
                    return Ok(());
                }
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let valid = proj
                    .groups
                    .get(group_idx)
                    .map(|g| pattern < g.patterns.len())
                    .unwrap_or(false);
                let playing_other = self.transport.is_playing()
                    && proj.active_pattern_index(group_idx, self.active_scene) != Some(pattern);
                if valid && playing_other && group_idx < 8 {
                    // En lecture : le pattern démarre au début de la mesure
                    // suivante (lancement quantifié, comme sur la MK3).
                    self.pending_patterns[group_idx] = Some(pattern);
                    drop(guard);
                    self.show_status(format!("Pattern {} à la prochaine mesure", pattern + 1));
                } else if valid {
                    // Workflow officiel Maschine : le pattern sélectionné remplace
                    // uniquement la référence du Groupe dans la scène active. Les
                    // autres Groupes gardent leur propre pattern, et ré-appuyer sur
                    // le pad du pattern courant ne le désactive pas.
                    if group_idx < 8 {
                        self.pending_patterns[group_idx] = None;
                    }
                    assign_pattern_to_active_scene(
                        proj,
                        &mut self.active_scene,
                        group_idx,
                        pattern,
                    );
                }
            }
            EngineEvent::PatternSelectInstant { group, pattern } => {
                let group_idx = group as usize;
                // Ré-appui sur le pad actif : coupe instantanée (MK3).
                if self.cut_active_pattern(group_idx, pattern) {
                    self.show_status(format!("Pattern {} coupé", pattern + 1));
                    return Ok(());
                }
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let valid = proj
                    .groups
                    .get(group_idx)
                    .map(|g| pattern < g.patterns.len())
                    .unwrap_or(false);
                if valid {
                    if group_idx < 8 {
                        // Écrase tout lancement en attente : le changement est immédiat.
                        self.pending_patterns[group_idx] = None;
                        self.pattern_chains[group_idx] = None;
                    }
                    assign_pattern_to_active_scene(
                        proj,
                        &mut self.active_scene,
                        group_idx,
                        pattern,
                    );
                    drop(guard);
                    self.show_status(format!("Pattern {} (immédiat)", pattern + 1));
                }
            }
            EngineEvent::AddPattern { group } => {
                let group_idx = group as usize;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let length = new_pattern_length(proj, group_idx, self.active_scene);
                if let Some(g) = proj.groups.get_mut(group_idx) {
                    let idx = g.patterns.len();
                    // Workflow officiel Maschine : un nouveau pattern est vide.
                    let pat = Pattern::new(&format!("Pattern {}", idx + 1), length);
                    g.patterns.push(pat);
                    assign_pattern_to_active_scene(proj, &mut self.active_scene, group_idx, idx);
                }
                self.selected_bar = 0;
                self.step_cursor = 0;
            }
            EngineEvent::CreatePatternAt { group, pattern } => {
                let group_idx = group as usize;
                let target = pattern.min(63);
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let length = new_pattern_length(proj, group_idx, self.active_scene);
                if let Some(g) = proj.groups.get_mut(group_idx) {
                    // Chaque slot libre créé pour atteindre le pad visé est un
                    // pattern vide, comme les cellules vides du mode PATTERN MK3.
                    while g.patterns.len() <= target {
                        let idx = g.patterns.len();
                        let pat = Pattern::new(&format!("Pattern {}", idx + 1), length);
                        g.patterns.push(pat);
                    }
                    assign_pattern_to_active_scene(proj, &mut self.active_scene, group_idx, target);
                }
                self.selected_bar = 0;
                self.step_cursor = 0;
            }
            EngineEvent::DuplicatePattern { group, pattern } => {
                let group_idx = group as usize;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let Some(g) = proj.groups.get_mut(group_idx) else {
                    return Ok(());
                };
                if g.patterns.is_empty() {
                    return Ok(());
                }
                let src = pattern.min(g.patterns.len() - 1);
                let insert = src + 1;
                let mut copy = g.patterns[src].clone();
                copy.name = format!("{} copie", copy.name);
                g.patterns.insert(insert, copy);
                shift_scene_refs_after_insert(proj, group_idx, insert);
                assign_pattern_to_active_scene(proj, &mut self.active_scene, group_idx, insert);
                self.selected_bar = 0;
                self.step_cursor = 0;
            }
            EngineEvent::DeletePattern { group, pattern } => {
                let group_idx = group as usize;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let Some(g) = proj.groups.get_mut(group_idx) else {
                    return Ok(());
                };
                if g.patterns.len() <= 1 {
                    return Ok(());
                }
                let idx = pattern.min(g.patterns.len() - 1);
                g.patterns.remove(idx);
                let new_len = g.patterns.len();
                shift_scene_refs_after_remove(proj, group_idx, idx, new_len);
                self.selected_bar = 0;
                self.step_cursor = 0;
            }
            EngineEvent::ClearPattern { group, pattern } => {
                let group_idx = group as usize;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(pat) = proj
                    .groups
                    .get_mut(group_idx)
                    .and_then(|g| g.patterns.get_mut(pattern))
                {
                    pat.events.clear();
                }
                self.selected_bar = 0;
                self.step_cursor = 0;
            }
            EngineEvent::RenamePattern {
                group,
                pattern,
                name,
            } => {
                let group_idx = group as usize;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(pat) = proj
                    .groups
                    .get_mut(group_idx)
                    .and_then(|g| g.patterns.get_mut(pattern))
                {
                    let trimmed = name.trim();
                    pat.name = if trimmed.is_empty() {
                        format!("Pattern {}", pattern + 1)
                    } else {
                        trimmed.chars().take(32).collect()
                    };
                }
            }
            EngineEvent::StepPattern { group, delta } => {
                let group_idx = group as usize;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let len = proj
                    .groups
                    .get(group_idx)
                    .map(|g| g.patterns.len())
                    .unwrap_or(0);
                if len == 0 {
                    return Ok(());
                }
                let current = proj
                    .active_pattern_index(group_idx, self.active_scene)
                    .unwrap_or(0);
                let next = (current as i32 + delta).rem_euclid(len as i32) as usize;
                assign_pattern_to_active_scene(proj, &mut self.active_scene, group_idx, next);
            }
            EngineEvent::SetLeftTab(tab) => {
                self.left_tab = tab.min(4);
                // Une vue principale choisit son écran droit contextuel
                // (SAMPLER, MIXER, BROWSER, PLUGIN, PATTERN/STEP). Un ancien overlay
                // EVENTS/MACRO/SETTINGS ne doit pas masquer cette transition.
                self.right_tab = 0;
                if self.left_tab == 4 {
                    self.sync_plugin_for_active_sound();
                }
                if self.left_tab != 0 {
                    self.song_mode = false;
                }
            }
            EngineEvent::SetRightTab(tab) => {
                self.right_tab = tab.min(3);
                if self.right_tab != 0 {
                    self.song_mode = false;
                }
            }
            EngineEvent::NavigateView(view) => {
                self.nav_view = view.min(4);
                self.nav_serial = self.nav_serial.wrapping_add(1);
            }
            EngineEvent::SetPluginPage(page) => {
                // 0: SOUND, 1: synth, 2: ampli guitare.
                self.plugin_page = page.min(2);
                if self.plugin_page == 1 {
                    self.sync_acid_from_active_sound();
                    self.sync_kick_from_active_sound();
                    self.sync_wavetable_from_active_sound();
                }
            }
            EngineEvent::SelectSound(snd) => {
                self.active_sound = snd.min(15);
                self.sync_acid_from_active_sound();
                self.sync_kick_from_active_sound();
                self.sync_wavetable_from_active_sound();
                if self.left_tab == 4 && self.plugin_page != 2 {
                    self.plugin_page = if self.focus_sound_has_synth() { 1 } else { 0 };
                }
            }
            EngineEvent::SoundVolume {
                group,
                sound,
                volume_db,
            } => {
                let mut gain_ratio = None;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group as usize) {
                    if let Some(s) = g.sounds.get_mut(sound as usize).and_then(|s| s.as_mut()) {
                        let next = volume_db.clamp(-60.0, 12.0);
                        gain_ratio = Some(db_to_linear(next - s.volume_db));
                        s.volume_db = next;
                    }
                }
                drop(guard);
                if let Some(ratio) = gain_ratio {
                    self.sampler
                        .scale_sound_gain(sampler_sound_id(group, sound), ratio);
                }
            }
            EngineEvent::SoundPan { group, sound, pan } => {
                let p = pan.clamp(-1.0, 1.0);
                {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    if let Some(g) = proj.groups.get_mut(group as usize) {
                        if let Some(s) = g.sounds.get_mut(sound as usize).and_then(|s| s.as_mut()) {
                            s.pan = p;
                        }
                    }
                }
                self.sampler
                    .update_sound_pan(sampler_sound_id(group, sound), p);
            }
            EngineEvent::SoundFilterCutoff {
                group,
                sound,
                cutoff_hz,
            } => {
                let c = cutoff_hz.clamp(20.0, 20000.0);
                let mut reso = 0.0;
                let mut filter_kind = crate::project::FilterKind::LowPass;
                {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    if let Some(g) = proj.groups.get_mut(group as usize) {
                        if let Some(s) = g.sounds.get_mut(sound as usize).and_then(|s| s.as_mut()) {
                            s.filter.cutoff_hz = c;
                            if s.filter.kind == crate::project::FilterKind::Off {
                                s.filter.kind = crate::project::FilterKind::LowPass;
                            }
                            reso = s.filter.resonance;
                            filter_kind = s.filter.kind;
                        }
                    }
                }
                if group == self.group_focus {
                    self.sampler.update_sound_filter(
                        sampler_sound_id(group, sound),
                        c,
                        reso,
                        filter_kind,
                    );
                }
            }
            EngineEvent::SoundFilterReso {
                group,
                sound,
                resonance,
            } => {
                let r = resonance.clamp(0.0, 1.0);
                let mut cutoff = 20000.0;
                let mut filter_kind = crate::project::FilterKind::LowPass;
                {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    if let Some(g) = proj.groups.get_mut(group as usize) {
                        if let Some(s) = g.sounds.get_mut(sound as usize).and_then(|s| s.as_mut()) {
                            s.filter.resonance = r;
                            if s.filter.kind == crate::project::FilterKind::Off {
                                s.filter.kind = crate::project::FilterKind::LowPass;
                            }
                            cutoff = s.filter.cutoff_hz;
                            filter_kind = s.filter.kind;
                        }
                    }
                }
                if group == self.group_focus {
                    self.sampler.update_sound_filter(
                        sampler_sound_id(group, sound),
                        cutoff,
                        r,
                        filter_kind,
                    );
                }
            }
            EngineEvent::SoundAdsr {
                group,
                sound,
                attack_ms,
                decay_ms,
                sustain,
                release_ms,
            } => {
                let a = attack_ms.clamp(0.1, 5000.0);
                let d = decay_ms.clamp(0.1, 5000.0);
                let su = sustain.clamp(0.0, 1.0);
                let rel = release_ms.clamp(0.1, 5000.0);
                {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    if let Some(g) = proj.groups.get_mut(group as usize) {
                        if let Some(s) = g.sounds.get_mut(sound as usize).and_then(|s| s.as_mut()) {
                            s.adsr.attack_ms = a;
                            s.adsr.decay_ms = d;
                            s.adsr.sustain = su;
                            s.adsr.release_ms = rel;
                        }
                    }
                }
                if group == self.group_focus {
                    self.sampler
                        .update_sound_adsr(sampler_sound_id(group, sound), a, d, su, rel);
                }
            }
            EngineEvent::SoundFilterKind { group, sound, kind } => {
                let mut filter = None;
                {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    if let Some(s) = proj
                        .groups
                        .get_mut(group as usize)
                        .and_then(|g| g.sounds.get_mut(sound as usize))
                        .and_then(|s| s.as_mut())
                    {
                        s.filter.kind = kind;
                        if s.filter.kind == crate::project::FilterKind::Off {
                            s.filter.cutoff_hz = 20000.0;
                            s.filter.resonance = 0.0;
                        } else if s.filter.cutoff_hz >= 19000.0 {
                            // Passage Off → filtre : ouvre une plage audible
                            // au lieu de laisser le cutoff à 20 kHz.
                            s.filter.cutoff_hz = 6000.0;
                            s.filter.resonance = s.filter.resonance.max(0.15);
                        }
                        filter = Some((s.filter.cutoff_hz, s.filter.resonance));
                    }
                }
                if let Some((cutoff, reso)) = filter {
                    self.sampler.update_sound_filter(
                        sampler_sound_id(group, sound),
                        cutoff,
                        reso,
                        kind,
                    );
                }
            }
            EngineEvent::SoundPolyphony {
                group,
                sound,
                polyphony,
            } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(s) = proj
                    .groups
                    .get_mut(group as usize)
                    .and_then(|g| g.sounds.get_mut(sound as usize))
                    .and_then(|s| s.as_mut())
                {
                    s.polyphony = polyphony.clamp(1, 32);
                }
                drop(guard);
                self.sampler
                    .set_sound_polyphony(sampler_sound_id(group, sound), polyphony);
            }
            EngineEvent::SoundChoke {
                group,
                sound,
                choke_group,
            } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(s) = proj
                    .groups
                    .get_mut(group as usize)
                    .and_then(|g| g.sounds.get_mut(sound as usize))
                    .and_then(|s| s.as_mut())
                {
                    s.choke_group = choke_group;
                }
                drop(guard);
                self.sampler
                    .set_sound_choke_group(sampler_sound_id(group, sound), choke_group);
            }
            EngineEvent::SoundApplyToGroup {
                group,
                source_sound,
            } => {
                let updates = {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    let Some(source) = proj
                        .groups
                        .get(group as usize)
                        .and_then(|g| g.sounds.get(source_sound as usize))
                        .and_then(|s| s.as_ref())
                        .cloned()
                    else {
                        return Ok(());
                    };
                    let Some(target_group) = proj.groups.get_mut(group as usize) else {
                        return Ok(());
                    };
                    let mut updates = Vec::new();
                    for (index, slot) in target_group.sounds.iter_mut().enumerate() {
                        if index == source_sound as usize {
                            continue;
                        }
                        let Some(target) = slot.as_mut() else {
                            continue;
                        };
                        target.volume_db = source.volume_db;
                        target.pan = source.pan;
                        target.choke_group = source.choke_group;
                        target.polyphony = source.polyphony;
                        target.adsr = source.adsr.clone();
                        target.filter = source.filter.clone();
                        updates.push((index as u8, target.clone()));
                    }
                    updates
                };
                for (index, sound) in updates {
                    let id = sampler_sound_id(group, index);
                    self.sampler.update_sound_pan(id, sound.pan);
                    self.sampler.update_sound_filter(
                        id,
                        sound.filter.cutoff_hz,
                        sound.filter.resonance,
                        sound.filter.kind,
                    );
                    self.sampler.update_sound_adsr(
                        id,
                        sound.adsr.attack_ms,
                        sound.adsr.decay_ms,
                        sound.adsr.sustain,
                        sound.adsr.release_ms,
                    );
                    self.sampler.set_sound_choke_group(id, sound.choke_group);
                    self.sampler.set_sound_polyphony(id, sound.polyphony);
                }
            }
            EngineEvent::SoundReset { group, sound } => {
                let mut reset = None;
                {
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    if let Some(s) = proj
                        .groups
                        .get_mut(group as usize)
                        .and_then(|g| g.sounds.get_mut(sound as usize))
                        .and_then(|s| s.as_mut())
                    {
                        s.volume_db = 0.0;
                        s.pan = 0.0;
                        s.choke_group = None;
                        s.polyphony = 8;
                        s.adsr = crate::project::Adsr::default();
                        s.filter = crate::project::Filter::default();
                        reset = Some(s.clone());
                    }
                }
                if let Some(sound_state) = reset {
                    let id = sampler_sound_id(group, sound);
                    self.sampler.update_sound_pan(id, sound_state.pan);
                    self.sampler.update_sound_filter(
                        id,
                        20000.0,
                        0.0,
                        crate::project::FilterKind::Off,
                    );
                    self.sampler.update_sound_adsr(
                        id,
                        sound_state.adsr.attack_ms,
                        sound_state.adsr.decay_ms,
                        sound_state.adsr.sustain,
                        sound_state.adsr.release_ms,
                    );
                    self.sampler.set_sound_choke_group(id, None);
                    self.sampler.set_sound_polyphony(id, 8);
                }
            }
            EngineEvent::GroupVolume { group, volume_db } => {
                let mut gain_ratio = None;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group as usize) {
                    let next = volume_db.clamp(-60.0, 12.0);
                    gain_ratio = Some(db_to_linear(next - g.volume_db));
                    g.volume_db = next;
                }
                drop(guard);
                if let Some(ratio) = gain_ratio {
                    self.sampler.scale_group_gain(group, ratio);
                }
            }
            EngineEvent::GroupPan { group, pan } => {
                let mut next_pan = None;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group as usize) {
                    let next = pan.clamp(-1.0, 1.0);
                    next_pan = Some(next);
                    g.pan = next;
                }
                drop(guard);
                if let Some(pan) = next_pan {
                    self.sampler.update_group_pan(group, pan);
                }
            }
            EngineEvent::Variation => {
                self.humanize_active_pattern();
            }
            EngineEvent::QuantizeActive => {
                self.quantize_active_pattern();
            }
            EngineEvent::SetMetronome(on) => {
                self.transport.metronome = on;
            }
            EngineEvent::ToggleSongMode => {
                self.song_mode = !self.song_mode;
                if self.song_mode && self.active_section.is_none() {
                    self.active_section = if self.project.read().song.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                }
            }
            EngineEvent::NextSection => {
                let n = self.project.read().song.len();
                if n > 0 {
                    let cur = self.active_section.unwrap_or(0);
                    self.active_section = Some((cur + 1) % n);
                    if let Some(si) = self.active_section {
                        if let Some(sec) = self.project.read().song.get(si) {
                            self.active_scene = Some(sec.scene);
                        }
                    }
                }
            }
            EngineEvent::PrevSection => {
                let n = self.project.read().song.len();
                if n > 0 {
                    let cur = self.active_section.unwrap_or(0);
                    self.active_section = Some((cur + n - 1) % n);
                    if let Some(si) = self.active_section {
                        if let Some(sec) = self.project.read().song.get(si) {
                            self.active_scene = Some(sec.scene);
                        }
                    }
                }
            }
            EngineEvent::SelectSection(index) => {
                if let Some(section) = self.project.read().song.get(index) {
                    self.active_section = Some(index);
                    self.active_scene = Some(section.scene);
                }
            }
            EngineEvent::AddSection => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                let scene = self
                    .active_scene
                    .or_else(|| (!project.scenes.is_empty()).then_some(0));
                if let Some(scene) = scene {
                    let index = project.song.len();
                    project.song.push(Section {
                        scene,
                        length: BarsBeatsTicks::new(2, 0, 0),
                        name: format!("Section {}", index + 1),
                    });
                    self.active_scene = Some(scene);
                    self.active_section = Some(index);
                    self.song_mode = true;
                }
            }
            EngineEvent::PatternSelectIndex { group, pattern } => {
                let _ = self.handle_event(EngineEvent::PatternSelect { group, pattern });
            }
            EngineEvent::DuplicateSection => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                if let Some(idx) = self.active_section {
                    if let Some(sec) = project.song.get(idx).cloned() {
                        let insert = idx + 1;
                        project.song.insert(
                            insert,
                            Section {
                                name: format!("{} copie", sec.name),
                                ..sec
                            },
                        );
                        self.active_section = Some(insert);
                    }
                }
            }
            EngineEvent::DeleteSection => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                if let Some(idx) = self.active_section {
                    if idx < project.song.len() {
                        project.song.remove(idx);
                    }
                    self.active_section = if project.song.is_empty() {
                        None
                    } else {
                        Some(idx.min(project.song.len() - 1))
                    };
                }
            }
            EngineEvent::ResizeSection { bars } => {
                let bars = bars.clamp(1, 64);
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                if let Some(idx) = self.active_section {
                    if let Some(sec) = project.song.get_mut(idx) {
                        sec.length = BarsBeatsTicks::new(bars as u16, 0, 0);
                    }
                }
            }
            EngineEvent::DuplicateScene => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                if let Some(idx) = self.active_scene {
                    if let Some(scene) = project.scenes.get(idx).cloned() {
                        let insert = idx + 1;
                        project.scenes.insert(
                            insert,
                            Scene {
                                name: format!("{} copie", scene.name),
                                ..scene
                            },
                        );
                        self.active_scene = Some(insert);
                        self.revision = self.revision.wrapping_add(1);
                    }
                }
            }
            EngineEvent::DuplicateSceneUnique => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                if let Some(idx) = self.active_scene {
                    if let Some(scene) = project.scenes.get(idx).cloned() {
                        let insert = idx + 1;
                        let mut new_scene = Scene {
                            name: format!("{} copie", scene.name),
                            color: scene.color,
                            patterns: vec![None; project.groups.len()],
                        };
                        for (gi, pat_opt) in scene.patterns.iter().enumerate() {
                            if let Some(pi) = *pat_opt {
                                if let Some(group) = project.groups.get_mut(gi) {
                                    if pi < group.patterns.len() {
                                        let mut pat_copy = group.patterns[pi].clone();
                                        pat_copy.name =
                                            format!("{} (Scn {})", pat_copy.name, insert + 1);
                                        group.patterns.push(pat_copy);
                                        new_scene.patterns[gi] = Some(group.patterns.len() - 1);
                                    }
                                }
                            }
                        }
                        project.scenes.insert(insert, new_scene);
                        self.active_scene = Some(insert);
                        self.revision = self.revision.wrapping_add(1);
                    }
                }
            }
            EngineEvent::MakeSceneUnique(scene_idx) => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                if let Some(scene) = project.scenes.get_mut(scene_idx) {
                    for gi in 0..project.groups.len() {
                        if let Some(Some(pi)) = scene.patterns.get(gi).copied() {
                            if let Some(group) = project.groups.get_mut(gi) {
                                if pi < group.patterns.len() {
                                    let mut pat_copy = group.patterns[pi].clone();
                                    pat_copy.name =
                                        format!("{} (Scn {})", pat_copy.name, scene_idx + 1);
                                    group.patterns.push(pat_copy);
                                    let new_idx = group.patterns.len() - 1;
                                    scene.patterns[gi] = Some(new_idx);
                                }
                            }
                        }
                    }
                }
                self.revision = self.revision.wrapping_add(1);
            }
            EngineEvent::SceneChain(indices) => {
                if !indices.is_empty() {
                    let mut guard = self.project.write();
                    let project = project_mut(&mut guard);
                    project.song.clear();
                    for (sec_idx, &scene_idx) in indices.iter().enumerate() {
                        if scene_idx < project.scenes.len() {
                            project.song.push(Section {
                                scene: scene_idx,
                                length: BarsBeatsTicks::new(2, 0, 0),
                                name: format!("Section {}", sec_idx + 1),
                            });
                        }
                    }
                    if !project.song.is_empty() {
                        self.active_section = Some(0);
                        self.active_scene = Some(project.song[0].scene);
                        self.song_mode = true;
                    }
                    self.revision = self.revision.wrapping_add(1);
                }
            }
            EngineEvent::SceneStep(delta) => {
                let len = self.project.read().scenes.len();
                if len > 0 {
                    let current = self.active_scene.unwrap_or(0) as i32;
                    let next = (current + delta).rem_euclid(len as i32) as usize;
                    self.active_scene = Some(next);
                    self.revision = self.revision.wrapping_add(1);
                }
            }
            EngineEvent::CreateSceneAt(target) => {
                let target = target.min(15);
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                let group_count = project.groups.len();
                while project.scenes.len() <= target {
                    let idx = project.scenes.len();
                    project
                        .scenes
                        .push(Scene::new(&format!("Scène {}", idx + 1), group_count));
                }
                self.active_scene = Some(target);
                self.song_mode = false;
                self.revision = self.revision.wrapping_add(1);
            }
            EngineEvent::DeleteScene => {
                let mut guard = self.project.write();
                let project = project_mut(&mut guard);
                if project.scenes.len() > 1 {
                    if let Some(idx) = self.active_scene {
                        if idx < project.scenes.len() {
                            project.scenes.remove(idx);
                            // Sections : retire celles de la scène supprimée,
                            // recale les indices supérieurs.
                            project.song.retain(|s| s.scene != idx);
                            for sec in &mut project.song {
                                if sec.scene > idx {
                                    sec.scene -= 1;
                                }
                            }
                            self.active_scene = Some(idx.min(project.scenes.len() - 1));
                            self.active_section = if project.song.is_empty() {
                                None
                            } else {
                                Some(self.active_section.unwrap_or(0).min(project.song.len() - 1))
                            };
                        }
                    }
                }
            }
            EngineEvent::EraseSound { group, sound } => {
                self.erase_sound(group, sound);
            }
            EngineEvent::ShiftPadAction(pad) => self.handle_shift_pad_action(pad),
            EngineEvent::ClearSoundSlot { group, sound } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(target_group) = proj.groups.get_mut(group as usize) {
                    for pattern in &mut target_group.patterns {
                        pattern.events.retain(|event| event.pad != sound);
                    }
                    if let Some(slot) = target_group.sounds.get_mut(sound as usize) {
                        *slot = None;
                    }
                }
                self.revision = self.revision.wrapping_add(1);
            }
            EngineEvent::Undo => self.undo_project(false),
            EngineEvent::Redo => self.undo_project(true),
            EngineEvent::SetSceneMode(v) => {
                self.scene_mode = v;
            }
            EngineEvent::SetPatternMode(v) => {
                self.pattern_mode = v;
            }
            EngineEvent::SetSelectMode(v) => {
                self.select_mode = v;
            }
            EngineEvent::SetEraseMode(v) => {
                self.erase_mode = v;
            }
            EngineEvent::SetControllerMode(v) => {
                self.controller_mode = v.min(3);
            }
            EngineEvent::KeyboardTrigger {
                note,
                mut velocity,
                pressed,
            } => {
                if self.fixed_velocity && pressed {
                    velocity = self.fixed_velocity_level;
                }
                // MUTE et SOLO comme pour les pads (voir `group_audible`).
                if !self.group_audible(self.group_focus)
                    || self.is_sound_muted(self.group_focus, self.active_sound)
                {
                    return Ok(());
                }
                let note = note.min(127);
                let first_key = self.arp_held.iter().all(|&v| v == 0);
                self.arp_held[note as usize] = if pressed { velocity.max(1) } else { 0 };
                if self.note_repeat_enabled {
                    // NOTE REPEAT en KEYBOARD / CHORDS = arpégiateur (comme la
                    // MK3) : les notes tenues sont jouées une à une, en montant.
                    if pressed && first_key {
                        self.note_repeat_samples = 0;
                        self.arp_step();
                    } else if !pressed && self.arp_held.iter().all(|&v| v == 0) {
                        if let Some(last) = self.arp_last.take() {
                            self.release_sound_voice(self.active_sound, last);
                        }
                    }
                } else if pressed {
                    self.trigger_sound_voice(self.active_sound, note, velocity);
                    if self.transport.state == TransportState::Recording {
                        // Keyboard/Chords : enregistre la hauteur jouée (accords = plusieurs événements).
                        let q = self.record_note_event(self.active_sound, note, velocity);
                        if let Some(q) = q {
                            self.rec_key_press[note as usize] =
                                Some((self.group_focus, self.active_sound, q));
                        }
                    }
                } else {
                    let mut scheduled = false;
                    if let Some((g, s, q)) = self.rec_key_press[note as usize].take() {
                        self.set_recorded_length(g, s, q, self.transport.tick);
                        if self.transport.state == TransportState::Recording {
                            self.schedule_note_off(s, note, q, self.transport.tick);
                            scheduled = true;
                        }
                    }
                    if !scheduled {
                        self.release_sound_voice(self.active_sound, note);
                    }
                }
            }
            EngineEvent::KeyboardRootDelta(d) => self.keyboard.cycle_root(d),
            EngineEvent::KeyboardScaleDelta(d) => self.keyboard.cycle_scale(d),
            EngineEvent::KeyboardOctaveDelta(d) => self.keyboard.cycle_octave(d),
            EngineEvent::KeyboardChordDelta(d) => self.keyboard.cycle_chord(d),
            EngineEvent::KeyboardInversionDelta(d) => self.keyboard.cycle_inversion(d),
            EngineEvent::SetKeyboardState(state) => self.keyboard = state,
            EngineEvent::ToggleLoop => {
                self.transport.loop_enabled = !self.transport.loop_enabled;
            }
            EngineEvent::ToggleMetronome => {
                self.transport.metronome = !self.transport.metronome;
            }
            EngineEvent::LoadDrumPreset {
                group,
                sound,
                preset_idx,
            } => {
                let preset_idx = preset_idx.min(15);
                let sr = self.transport.sample_rate;
                let name = DRUM_PRESET_NAMES[preset_idx as usize];
                let sample = self.drum_preset_sample(preset_idx);
                let sound_id = sampler_sound_id(group, sound);
                self.sampler.clear_sound(sound_id);
                self.sampler.add_zone(
                    sound_id,
                    crate::sampler::ZoneMapping {
                        sample: Some(sample),
                        waveform_cache: None,
                        sample_path: None,
                        sample_rate: sr,
                        root_key: 36 + sound,
                        lokey: 0,
                        hikey: 127,
                        lovel: 0,
                        hivel: 127,
                        volume_db: 0.0,
                        pan: 0.0,
                        tune_cents: 0,
                        choke_group: if preset_idx == 2 || preset_idx == 3 {
                            Some(1)
                        } else {
                            None
                        },
                        is_one_shot: true,
                    },
                );
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(group as usize) {
                    if let Some(target) = g.sounds.get_mut(sound as usize) {
                        let s = target.get_or_insert_with(|| crate::project::Sound::new(name, 78));
                        s.name = name.to_string();
                        s.zones = vec![Zone {
                            root_key: 36 + sound,
                            ..Zone::default()
                        }];
                    }
                }
            }
            EngineEvent::LoadSample { group, sound, path } => {
                #[cfg(feature = "sampler")]
                {
                    let zone = Sampler::prepare_wav(&path, 36 + sound)?;
                    self.install_sample_zone(group, sound, zone)?;
                }
                #[cfg(not(feature = "sampler"))]
                {
                    let _ = (group, sound, path);
                    log::warn!("Sampler feature not enabled");
                }
            }
            EngineEvent::LoadPreparedSample { group, sound, zone } => {
                self.install_sample_zone(group, sound, zone)?;
            }
            EngineEvent::BrowsePresetStep(step) => match self.browser_kind {
                1 => {
                    let len = self.project_entries.len().max(1);
                    self.project_selected =
                        (self.project_selected as i32 + step).rem_euclid(len as i32) as usize;
                }
                2 => {
                    self.browser_plugin_idx =
                        (self.browser_plugin_idx as i32 + step).rem_euclid(5) as u8;
                }
                3 => {
                    self.browser_fx_idx = (self.browser_fx_idx as i32 + step).rem_euclid(5) as u8;
                }
                _ => {
                    let next = (self.browser_preset_idx as i32 + step).rem_euclid(16) as u8;
                    self.browser_preset_idx = next;
                    self.preview_preset(next);
                }
            },
            EngineEvent::SetBrowserKind(kind) => {
                self.browser_kind = kind.min(3);
                self.left_tab = 3;
            }
            EngineEvent::SetProjectEntries(entries) => {
                self.project_entries = entries;
                if self.project_selected >= self.project_entries.len() {
                    self.project_selected = self.project_entries.len().saturating_sub(1);
                }
            }
            EngineEvent::BrowseBrowserStep(step) => match self.browser_kind {
                1 => {
                    let len = self.project_entries.len();
                    if len > 0 {
                        self.project_selected =
                            (self.project_selected as i32 + step).rem_euclid(len as i32) as usize;
                    }
                }
                2 => {
                    self.browser_plugin_idx =
                        (self.browser_plugin_idx as i32 + step).rem_euclid(5) as u8;
                }
                3 => {
                    self.browser_fx_idx = (self.browser_fx_idx as i32 + step).rem_euclid(5) as u8;
                }
                _ => {
                    let next = (self.browser_preset_idx as i32 + step).rem_euclid(16) as u8;
                    self.browser_preset_idx = next;
                    self.preview_preset(next);
                }
            },
            EngineEvent::LoadSelectedBrowserItem { group, sound } => match self.browser_kind {
                1 => {
                    if let Some(entry) = self.project_entries.get(self.project_selected).cloned() {
                        self.handle_event(EngineEvent::Load(std::path::PathBuf::from(entry.path)))?;
                    }
                }
                2 => {
                    self.load_plugin_to_sound(group, sound, self.browser_plugin_idx)?;
                    self.left_tab = 4;
                }
                3 => {
                    let fx_type = match self.browser_fx_idx {
                        0 => crate::fx::FxType::Reverb,
                        1 => crate::fx::FxType::Delay,
                        2 => crate::fx::FxType::Distortion,
                        3 => crate::fx::FxType::Filter,
                        _ => crate::fx::FxType::Compressor,
                    };
                    self.add_fx_to_sound(group, sound, fx_type)?;
                }
                _ => {
                    self.handle_event(EngineEvent::LoadDrumPreset {
                        group,
                        sound,
                        preset_idx: self.browser_preset_idx,
                    })?;
                }
            },
            EngineEvent::DeleteSelectedProject => {
                self.delete_selected_project()?;
            }
            EngineEvent::CancelDeleteProject => {
                self.delete_project_confirm = false;
                self.status_message = Some("Suppression annulée".to_string());
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(2000);
            }
            EngineEvent::SetMixerMode(mode) => {
                self.mixer_mode = mode.min(3);
            }
            EngineEvent::KickSetParams(params) => {
                self.kick_synth.params = params.clone();
                self.kick_sound_target = Some((self.group_focus, self.active_sound));
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(self.group_focus as usize) {
                    let snd = g.sounds[self.active_sound as usize].get_or_insert_with(|| {
                        crate::project::Sound::new(&format!("Kick {}", self.active_sound + 1), 6)
                    });
                    snd.kick_synth = Some(params);
                    snd.synth = None;
                    snd.wavetable_synth = None;
                }
                self.revision = self.revision.wrapping_add(1);
            }
            EngineEvent::KickSetPreset(idx) => {
                let presets = crate::kick_synth::KickParams::all_presets();
                if let Some(p) = presets.get(idx as usize) {
                    self.handle_event(EngineEvent::KickSetParams(p.clone()))?;
                }
            }
            EngineEvent::KickDriveTypeDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                let next = match p.drive_type {
                    crate::kick_synth::KickDriveType::AnalogTanh => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::HardClip
                        } else {
                            crate::kick_synth::KickDriveType::Rectifier
                        }
                    }
                    crate::kick_synth::KickDriveType::HardClip => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::Wavefolder
                        } else {
                            crate::kick_synth::KickDriveType::AnalogTanh
                        }
                    }
                    crate::kick_synth::KickDriveType::Wavefolder => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::TubeDrive
                        } else {
                            crate::kick_synth::KickDriveType::HardClip
                        }
                    }
                    crate::kick_synth::KickDriveType::TubeDrive => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::Bitcrush
                        } else {
                            crate::kick_synth::KickDriveType::Wavefolder
                        }
                    }
                    crate::kick_synth::KickDriveType::Bitcrush => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::Fuzz
                        } else {
                            crate::kick_synth::KickDriveType::TubeDrive
                        }
                    }
                    crate::kick_synth::KickDriveType::Fuzz => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::Rectifier
                        } else {
                            crate::kick_synth::KickDriveType::Bitcrush
                        }
                    }
                    crate::kick_synth::KickDriveType::Rectifier => {
                        if delta > 0 {
                            crate::kick_synth::KickDriveType::AnalogTanh
                        } else {
                            crate::kick_synth::KickDriveType::Fuzz
                        }
                    }
                };
                p.drive_type = next;
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickFilterModeDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                let next = match p.filter_mode {
                    crate::kick_synth::KickFilterMode::Off => {
                        if delta > 0 {
                            crate::kick_synth::KickFilterMode::LowPass
                        } else {
                            crate::kick_synth::KickFilterMode::Peak
                        }
                    }
                    crate::kick_synth::KickFilterMode::LowPass => {
                        if delta > 0 {
                            crate::kick_synth::KickFilterMode::HighPass
                        } else {
                            crate::kick_synth::KickFilterMode::Off
                        }
                    }
                    crate::kick_synth::KickFilterMode::HighPass => {
                        if delta > 0 {
                            crate::kick_synth::KickFilterMode::BandPass
                        } else {
                            crate::kick_synth::KickFilterMode::LowPass
                        }
                    }
                    crate::kick_synth::KickFilterMode::BandPass => {
                        if delta > 0 {
                            crate::kick_synth::KickFilterMode::Notch
                        } else {
                            crate::kick_synth::KickFilterMode::HighPass
                        }
                    }
                    crate::kick_synth::KickFilterMode::Notch => {
                        if delta > 0 {
                            crate::kick_synth::KickFilterMode::Peak
                        } else {
                            crate::kick_synth::KickFilterMode::BandPass
                        }
                    }
                    crate::kick_synth::KickFilterMode::Peak => {
                        if delta > 0 {
                            crate::kick_synth::KickFilterMode::Off
                        } else {
                            crate::kick_synth::KickFilterMode::Notch
                        }
                    }
                };
                p.set_filter_mode(next);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickFilterCutoffDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                let factor = 1.05f32.powf(delta * 0.1);
                let (min_hz, max_hz) = match p.filter_mode {
                    crate::kick_synth::KickFilterMode::HighPass => (20.0, 1500.0),
                    crate::kick_synth::KickFilterMode::BandPass => (25.0, 3500.0),
                    _ => (20.0, 18000.0),
                };
                p.filter_cutoff_hz = (p.filter_cutoff_hz * factor).clamp(min_hz, max_hz);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickFilterResoDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                p.filter_reso = (p.filter_reso + delta * 0.02).clamp(0.0, 0.95);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickToneDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                p.drive_tone = (p.drive_tone + delta).clamp(0.0, 1.0);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickWaveformDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                // Cycle: Sine → Triangle → SatSine → Parabolic → Ramp → SquareSub → NoiseTransient → Sine
                let next = match p.waveform {
                    crate::kick_synth::KickWaveform::Sine => {
                        if delta > 0 {
                            crate::kick_synth::KickWaveform::Triangle
                        } else {
                            crate::kick_synth::KickWaveform::NoiseTransient
                        }
                    }
                    crate::kick_synth::KickWaveform::Triangle => {
                        if delta > 0 {
                            crate::kick_synth::KickWaveform::SatSine
                        } else {
                            crate::kick_synth::KickWaveform::Sine
                        }
                    }
                    crate::kick_synth::KickWaveform::SatSine => {
                        if delta > 0 {
                            crate::kick_synth::KickWaveform::Parabolic
                        } else {
                            crate::kick_synth::KickWaveform::Triangle
                        }
                    }
                    crate::kick_synth::KickWaveform::Parabolic => {
                        if delta > 0 {
                            crate::kick_synth::KickWaveform::Ramp
                        } else {
                            crate::kick_synth::KickWaveform::SatSine
                        }
                    }
                    crate::kick_synth::KickWaveform::Ramp => {
                        if delta > 0 {
                            crate::kick_synth::KickWaveform::SquareSub
                        } else {
                            crate::kick_synth::KickWaveform::Parabolic
                        }
                    }
                    crate::kick_synth::KickWaveform::SquareSub => {
                        if delta > 0 {
                            crate::kick_synth::KickWaveform::NoiseTransient
                        } else {
                            crate::kick_synth::KickWaveform::Ramp
                        }
                    }
                    crate::kick_synth::KickWaveform::NoiseTransient => {
                        if delta > 0 {
                            crate::kick_synth::KickWaveform::Sine
                        } else {
                            crate::kick_synth::KickWaveform::SquareSub
                        }
                    }
                };
                p.waveform = next;
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickFmAmtDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                p.fm_amount = (p.fm_amount + delta).clamp(0.0, 1.0);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickFmRatioDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                p.fm_ratio = (p.fm_ratio + delta * 0.1).clamp(0.25, 8.0);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickDriveDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                p.drive = (p.drive + delta * 0.02).clamp(0.0, 1.0);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickTuneDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                p.tune_hz = (p.tune_hz + delta * 0.5).clamp(30.0, 90.0);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickDecayDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                p.decay_ms = (p.decay_ms + delta * 15.0).clamp(40.0, 2000.0);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickSnapDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                p.snap = (p.snap + delta * 0.02).clamp(0.0, 1.0);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickSweepDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                p.sweep_time_ms = (p.sweep_time_ms + delta * 1.5).clamp(5.0, 120.0);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickClickDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                p.click = (p.click + delta * 0.02).clamp(0.0, 1.0);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickSubDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                p.sub = (p.sub + delta * 0.02).clamp(0.0, 1.0);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickVolumeDelta(delta) => {
                let mut p = self.kick_synth.params.clone();
                p.volume_db = (p.volume_db + delta * 0.5).clamp(-60.0, 12.0);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickSetDriveType(drive_type) => {
                let mut p = self.kick_synth.params.clone();
                p.drive_type = drive_type;
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::KickSetFilterMode(mode) => {
                let mut p = self.kick_synth.params.clone();
                p.set_filter_mode(mode);
                let _ = self.handle_event(EngineEvent::KickSetParams(p));
            }
            EngineEvent::ToggleSynthParamPage => {
                // Page suivante, en boucle (ancien bouton PAGE 1/2).
                let count = self.plugin_instrument().map_or(1, |i| i.page_count());
                let next = (self.plugin_page_index() + 1) % count;
                self.set_plugin_page(next);
            }
            EngineEvent::SetSynthParamPage(page) => {
                self.set_plugin_page(page as usize);
            }
            EngineEvent::PluginPageStep(step) => {
                let count = self.plugin_page_count();
                let page = (self.plugin_page_index() as i32 + i32::from(step))
                    .clamp(0, count as i32 - 1) as usize;
                if page != self.plugin_page_index() {
                    self.set_plugin_page(page);
                }
            }
            EngineEvent::PluginKnob { knob, delta } => {
                self.turn_plugin_knob(knob as usize, delta)?;
            }
            EngineEvent::PluginSlotStep(step) => {
                let next = self.plugin_slot_index() as i32 + i32::from(step);
                self.select_plugin_slot(next.max(0) as usize);
            }
            EngineEvent::SelectPluginSlot(index) => self.select_plugin_slot(index),
            EngineEvent::EventStep(step) => self.step_event_cursor(step),
            EngineEvent::KeyboardKnob { knob, delta } => {
                self.turn_keyboard_knob(knob as usize, delta)?
            }
            EngineEvent::CountInRecord => {
                if self.count_in.take().is_some() {
                    self.show_status("Pré-compte annulé".to_string());
                } else if self.transport.state == TransportState::Recording {
                    self.transport.play();
                } else if self.transport.is_playing() {
                    self.transport.record();
                } else {
                    self.start_count_in();
                }
            }
            EngineEvent::SampleKnob { knob, delta } => self.turn_sample_knob(knob as usize, delta),
            EngineEvent::EventKnob { knob, delta } => self.turn_event_knob(knob as usize, delta),
            EngineEvent::CycleSoundFilterKind { group, sound } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(snd) = proj
                    .groups
                    .get_mut(group as usize)
                    .and_then(|g| g.sounds.get_mut(sound as usize))
                    .and_then(|s| s.as_mut())
                {
                    snd.filter.kind = match snd.filter.kind {
                        crate::project::FilterKind::Off => crate::project::FilterKind::LowPass,
                        crate::project::FilterKind::LowPass => crate::project::FilterKind::HighPass,
                        crate::project::FilterKind::HighPass => {
                            crate::project::FilterKind::BandPass
                        }
                        crate::project::FilterKind::BandPass => crate::project::FilterKind::Notch,
                        crate::project::FilterKind::Notch => crate::project::FilterKind::Peak,
                        crate::project::FilterKind::Peak => crate::project::FilterKind::Off,
                    };
                    let kind = snd.filter.kind;
                    self.status_message =
                        Some(format!("Sound {:02} Filtre : {:?}", sound + 1, kind));
                    self.status_expires_at =
                        std::time::Instant::now() + std::time::Duration::from_millis(2500);
                    self.revision = self.revision.wrapping_add(1);
                }
            }
            EngineEvent::SetSoundFilterKind { group, sound, kind } => {
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(snd) = proj
                    .groups
                    .get_mut(group as usize)
                    .and_then(|g| g.sounds.get_mut(sound as usize))
                    .and_then(|s| s.as_mut())
                {
                    snd.filter.kind = kind;
                    self.status_message =
                        Some(format!("Sound {:02} Filtre : {:?}", sound + 1, kind));
                    self.status_expires_at =
                        std::time::Instant::now() + std::time::Duration::from_millis(2500);
                    self.revision = self.revision.wrapping_add(1);
                }
            }
            EngineEvent::WavetableSetParams(params) => {
                self.wavetable_synth.params = params.clone();
                self.wavetable_sound_target = Some((self.group_focus, self.active_sound));
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                if let Some(g) = proj.groups.get_mut(self.group_focus as usize) {
                    let snd = g.sounds[self.active_sound as usize].get_or_insert_with(|| {
                        crate::project::Sound::new(&format!("Nexus {}", self.active_sound + 1), 78)
                    });
                    snd.wavetable_synth = Some(params);
                    snd.synth = None;
                    snd.kick_synth = None;
                }
                self.revision = self.revision.wrapping_add(1);
            }
            EngineEvent::WavetableSetPreset(idx) => {
                let presets = crate::wavetable_synth::WavetableParams::all_presets();
                if let Some(p) = presets.get(idx).cloned() {
                    let name = p.name.clone();
                    self.handle_event(EngineEvent::WavetableSetParams(p))?;
                    self.status_message = Some(format!("Preset: {name}"));
                    self.status_expires_at =
                        std::time::Instant::now() + std::time::Duration::from_millis(2500);
                }
            }
            EngineEvent::WavetableNextPreset => {
                let presets = crate::wavetable_synth::WavetableParams::all_presets();
                let cur_idx = presets
                    .iter()
                    .position(|p| p.name == self.wavetable_synth.params.name)
                    .unwrap_or(0);
                let next = (cur_idx + 1) % presets.len();
                let p = presets[next].clone();
                let name = p.name.clone();
                self.handle_event(EngineEvent::WavetableSetParams(p))?;
                self.status_message = Some(format!("Preset: {name}"));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(2500);
            }
            EngineEvent::WavetablePrevPreset => {
                let presets = crate::wavetable_synth::WavetableParams::all_presets();
                let cur_idx = presets
                    .iter()
                    .position(|p| p.name == self.wavetable_synth.params.name)
                    .unwrap_or(0);
                let prev = (cur_idx + presets.len() - 1) % presets.len();
                let p = presets[prev].clone();
                let name = p.name.clone();
                self.handle_event(EngineEvent::WavetableSetParams(p))?;
                self.status_message = Some(format!("Preset: {name}"));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(2500);
            }
            EngineEvent::WavetableCutoffDelta(delta) => {
                let (min_hz, max_hz) = match self.wavetable_synth.params.filter_mode {
                    crate::wavetable_synth::WavetableFilterMode::HighPass => (20.0, 3500.0),
                    crate::wavetable_synth::WavetableFilterMode::BandPass => (40.0, 8000.0),
                    _ => (20.0, 20000.0),
                };
                let base = self
                    .wavetable_synth
                    .params
                    .filter_cutoff
                    .clamp(min_hz, max_hz);
                let factor = 2.0f32.powf(delta * 0.08);
                self.wavetable_synth.params.filter_cutoff = (base * factor).clamp(min_hz, max_hz);
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableResoDelta(delta) => {
                let next =
                    (self.wavetable_synth.params.filter_reso + delta * 0.02).clamp(0.0, 0.99);
                self.wavetable_synth.params.filter_reso = next;
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableEnvAmtDelta(delta) => {
                let next =
                    (self.wavetable_synth.params.filter_env_amt + delta * 0.03).clamp(-1.0, 1.0);
                self.wavetable_synth.params.filter_env_amt = next;
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableDriveDelta(delta) => {
                let next =
                    (self.wavetable_synth.params.filter_drive + delta * 0.02).clamp(0.0, 1.0);
                self.wavetable_synth.params.filter_drive = next;
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableAttackDelta(delta) => {
                let next =
                    (self.wavetable_synth.params.amp_attack_ms + delta * 15.0).clamp(0.1, 5000.0);
                self.wavetable_synth.params.amp_attack_ms = next;
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableDecayDelta(delta) => {
                let next =
                    (self.wavetable_synth.params.amp_decay_ms + delta * 30.0).clamp(1.0, 5000.0);
                self.wavetable_synth.params.amp_decay_ms = next;
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableSustainDelta(delta) => {
                let next = (self.wavetable_synth.params.amp_sustain + delta * 0.02).clamp(0.0, 1.0);
                self.wavetable_synth.params.amp_sustain = next;
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableReleaseDelta(delta) => {
                let next =
                    (self.wavetable_synth.params.amp_release_ms + delta * 30.0).clamp(1.0, 5000.0);
                self.wavetable_synth.params.amp_release_ms = next;
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableFilterModeCycle => {
                use crate::wavetable_synth::WavetableFilterMode;
                let next = match self.wavetable_synth.params.filter_mode {
                    WavetableFilterMode::Off => WavetableFilterMode::LowPass12,
                    WavetableFilterMode::LowPass12 => WavetableFilterMode::LowPass24,
                    WavetableFilterMode::LowPass24 => WavetableFilterMode::HighPass,
                    WavetableFilterMode::HighPass => WavetableFilterMode::BandPass,
                    WavetableFilterMode::BandPass => WavetableFilterMode::Notch,
                    WavetableFilterMode::Notch => WavetableFilterMode::Peak,
                    WavetableFilterMode::Peak => WavetableFilterMode::Off,
                };
                self.wavetable_synth.params.set_filter_mode(next);
                self.status_message = Some(format!("Filter Mode: {:?}", next));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(1500);
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableUnisonVoicesDelta(delta) => {
                let next =
                    (self.wavetable_synth.params.unison_voices as i32 + delta).clamp(1, 7) as u8;
                self.wavetable_synth.params.unison_voices = next;
                self.status_message = Some(format!("Unison: {} voices", next));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(1500);
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableOsc1WaveCycle => {
                use crate::wavetable_synth::WavetableWaveform::*;
                let waves = [
                    Sine,
                    Triangle,
                    Saw,
                    Square,
                    Supersaw,
                    GrandPiano,
                    M1Piano,
                    RhodesTines,
                    DigiLead,
                    TonewheelOrgan,
                    VocalFormant,
                    Noise,
                ];
                let cur = self.wavetable_synth.params.osc1_wave;
                let idx = waves.iter().position(|&w| w == cur).unwrap_or(0);
                let next = waves[(idx + 1) % waves.len()];
                self.wavetable_synth.params.osc1_wave = next;
                self.status_message = Some(format!("Osc1 Wave: {:?}", next));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(1500);
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableOsc2WaveCycle => {
                use crate::wavetable_synth::WavetableWaveform::*;
                let waves = [
                    Sine,
                    Triangle,
                    Saw,
                    Square,
                    Supersaw,
                    GrandPiano,
                    M1Piano,
                    RhodesTines,
                    DigiLead,
                    TonewheelOrgan,
                    VocalFormant,
                    Noise,
                ];
                let cur = self.wavetable_synth.params.osc2_wave;
                let idx = waves.iter().position(|&w| w == cur).unwrap_or(0);
                let next = waves[(idx + 1) % waves.len()];
                self.wavetable_synth.params.osc2_wave = next;
                self.status_message = Some(format!("Osc2 Wave: {:?}", next));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(1500);
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableChorusDelta(delta) => {
                let next =
                    (self.wavetable_synth.params.fx_chorus_mix + delta * 0.02).clamp(0.0, 1.0);
                self.wavetable_synth.params.fx_chorus_mix = next;
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableDelayDelta(delta) => {
                let next =
                    (self.wavetable_synth.params.fx_delay_mix + delta * 0.02).clamp(0.0, 1.0);
                self.wavetable_synth.params.fx_delay_mix = next;
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableReverbDelta(delta) => {
                let next =
                    (self.wavetable_synth.params.fx_reverb_mix + delta * 0.02).clamp(0.0, 1.0);
                self.wavetable_synth.params.fx_reverb_mix = next;
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::WavetableVolumeDelta(delta) => {
                let next =
                    (self.wavetable_synth.params.master_volume_db + delta * 0.5).clamp(-30.0, 6.0);
                self.wavetable_synth.params.master_volume_db = next;
                self.save_wavetable_params_to_active_sound();
            }
            EngineEvent::SetNoteRepeat(enabled) => {
                self.note_repeat_enabled = enabled;
                if !enabled {
                    self.note_repeat_held_pads = [None; 16];
                    self.note_repeat_samples = 0;
                    if let Some(last) = self.arp_last.take() {
                        self.release_sound_voice(self.active_sound, last);
                    }
                }
                // Sur la MK3, NOTE REPEAT devient l'arpégiateur en KEYBOARD / CHORDS.
                let name = if matches!(self.controller_mode, 1 | 2) {
                    "Arp"
                } else {
                    "Note Repeat"
                };
                self.status_message = Some(format!(
                    "{name} : {}",
                    if enabled { "ACTIF" } else { "DÉSACTIVÉ" }
                ));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(1500);
                self.revision = self.revision.wrapping_add(1);
            }
            EngineEvent::SetNoteRepeatRate(rate) => {
                self.note_repeat_rate = rate.min(7);
                let label = match rate {
                    0 => "1/32",
                    1 => "1/16",
                    2 => "1/8",
                    3 => "1/4",
                    4 => "1/2",
                    5 => "1/16T",
                    6 => "1/8T",
                    _ => "1/4T",
                };
                self.status_message = Some(format!("Note Repeat: {label}"));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(1500);
                self.revision = self.revision.wrapping_add(1);
            }
            EngineEvent::ToggleSixteenVelocities => {
                self.sixteen_velocities = !self.sixteen_velocities;
                if self.sixteen_velocities {
                    // Deux modes de vélocité exclusifs, comme sur la MK3.
                    self.fixed_velocity = false;
                }
                self.show_status(if self.sixteen_velocities {
                    format!(
                        "16 vélocités : Sound {:02} sur les 16 pads",
                        self.active_sound + 1
                    )
                } else {
                    "16 vélocités coupé".to_string()
                });
            }
            EngineEvent::SetFixedVelocity(enabled) => {
                self.fixed_velocity = enabled;
                if enabled {
                    self.sixteen_velocities = false;
                }
                self.status_message = Some(format!(
                    "Vélocité fixe: {} ({})",
                    self.fixed_velocity_level,
                    if enabled { "ACTIF" } else { "off" }
                ));
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(1500);
                self.revision = self.revision.wrapping_add(1);
            }
            EngineEvent::SetFixedVelocityLevel(level) => {
                self.fixed_velocity_level = level.clamp(1, 127);
                self.revision = self.revision.wrapping_add(1);
            }
            EngineEvent::ToggleAutomation => {
                self.automation_armed = !self.automation_armed;
                self.automation_view = self.automation_armed;
                self.show_status(format!(
                    "Automation {} — {}",
                    if self.automation_armed {
                        "ARMÉE"
                    } else {
                        "OFF"
                    },
                    if self.automation_armed {
                        "tourne un knob en REC pour écrire des points"
                    } else {
                        "vue fermée"
                    }
                ));
            }
            EngineEvent::SetShiftHeld(held) => {
                self.shift_held = held;
                self.revision = self.revision.wrapping_add(1);
            }
            EngineEvent::ToggleLock(locked) => {
                self.lock_active = locked;
                self.status_message = Some(if locked {
                    "Lock: ACTIF".to_string()
                } else {
                    "Lock: DÉSACTIVÉ".to_string()
                });
                self.status_expires_at =
                    std::time::Instant::now() + std::time::Duration::from_millis(1500);
                self.revision = self.revision.wrapping_add(1);
            }
            EngineEvent::PatternChain { group, patterns } => {
                let group_idx = group as usize;
                if !patterns.is_empty() {
                    self.pattern_chains[group_idx] = Some(patterns.clone());
                    let mut guard = self.project.write();
                    let proj = project_mut(&mut guard);
                    assign_pattern_to_active_scene(
                        proj,
                        &mut self.active_scene,
                        group_idx,
                        patterns[0],
                    );
                    self.status_message = Some(format!("Patterns chaînés: {:?}", patterns));
                    self.status_expires_at =
                        std::time::Instant::now() + std::time::Duration::from_millis(1500);
                    self.revision = self.revision.wrapping_add(1);
                }
            }
            EngineEvent::LoadPluginToSound {
                group,
                sound,
                plugin_idx,
            } => {
                self.load_plugin_to_sound(group, sound, plugin_idx)?;
            }
            EngineEvent::AddFxToSound {
                group,
                sound,
                fx_type,
            } => {
                self.add_fx_to_sound(group, sound, fx_type)?;
            }
            EngineEvent::AddFxToGroup { group, fx_type } => {
                self.add_fx_to_group(group, fx_type)?;
            }
            EngineEvent::SelectBar(bar) => {
                let max_bars = {
                    let proj = Arc::clone(&*self.project.read());
                    proj.active_pattern_index(self.group_focus as usize, self.active_scene)
                        .and_then(|pi| proj.groups.get(self.group_focus as usize)?.patterns.get(pi))
                        .map(|p| (p.length.bars as usize).max(1))
                        .unwrap_or(2)
                };
                self.selected_bar = bar.min(max_bars.saturating_sub(1));
            }
            EngineEvent::NextBar => {
                let max_bars = {
                    let proj = Arc::clone(&*self.project.read());
                    proj.active_pattern_index(self.group_focus as usize, self.active_scene)
                        .and_then(|pi| proj.groups.get(self.group_focus as usize)?.patterns.get(pi))
                        .map(|p| (p.length.bars as usize).max(1))
                        .unwrap_or(2)
                };
                if self.selected_bar + 1 < max_bars {
                    self.selected_bar += 1;
                }
            }
            EngineEvent::PrevBar => {
                self.selected_bar = self.selected_bar.saturating_sub(1);
            }
            EngineEvent::Restart => {
                self.transport.tick = 0;
                self.transport.tick_fraction = 0.0;
                self.selected_bar = 0;
            }
            EngineEvent::SoftKey(n) => {
                return self.handle_soft_key(n);
            }
            EngineEvent::Swing(v) => {
                self.set_swing(v);
            }
            EngineEvent::StepCursor(step) => {
                self.step_cursor = step;
                self.selected_bar = step / 16;
            }
            EngineEvent::StepScroll(delta) => {
                let total = self.pattern_total_steps();
                if total > 0 {
                    let next = (self.step_cursor as i64 + delta as i64).clamp(0, total as i64 - 1)
                        as usize;
                    self.step_cursor = next;
                    self.selected_bar = next / 16;
                }
            }
            EngineEvent::StepVelocityDelta(delta) => {
                let pad = self.active_sound;
                let (start, end) = self.step_window();
                let _ = self.with_active_pattern(|pat| {
                    if let Some(ev) = pat
                        .events
                        .iter_mut()
                        .find(|e| e.pad == pad && e.tick >= start && e.tick < end)
                    {
                        ev.velocity = (ev.velocity as i32 + delta).clamp(1, 127) as u8;
                    }
                });
            }
            EngineEvent::StepNudgeTicks(delta) => {
                let pad = self.active_sound;
                let (start, end) = self.step_window();
                let _ = self.with_active_pattern(|pat| {
                    let len_ticks = (pat.length.bars as u32).max(1) * 4 * 960;
                    let idx = pat
                        .events
                        .iter()
                        .position(|e| e.pad == pad && e.tick >= start && e.tick < end);
                    if let Some(i) = idx {
                        let t = (pat.events[i].tick as i64 + delta as i64)
                            .clamp(0, len_ticks as i64 - 1) as u32;
                        pat.events[i].tick = t;
                        // Fusionne avec un pas voisin si on glisse dans sa cellule.
                        let keep = i;
                        let mut seen = 0usize;
                        pat.events.retain(|e| {
                            let this = seen;
                            seen += 1;
                            !(this != keep
                                && e.pad == pad
                                && crate::project::same_step_cell(e.tick, t))
                        });
                        pat.events.sort_by_key(|e| e.tick);
                    }
                });
            }
            EngineEvent::StepLengthDelta(delta) => {
                let pad = self.active_sound;
                let (start, end) = self.step_window();
                let _ = self.with_active_pattern(|pat| {
                    if let Some(ev) = pat
                        .events
                        .iter_mut()
                        .find(|e| e.pad == pad && e.tick >= start && e.tick < end)
                    {
                        ev.length_ticks =
                            (ev.length_ticks as i32 + delta * 240).clamp(30, 3840 * 8) as u32;
                    }
                });
            }
            EngineEvent::StepPitchDelta(delta) => {
                let pad = self.active_sound;
                let (start, end) = self.step_window();
                let _ = self.with_active_pattern(|pat| {
                    if let Some(ev) = pat
                        .events
                        .iter_mut()
                        .find(|e| e.pad == pad && e.tick >= start && e.tick < end)
                    {
                        ev.pitch = (ev.pitch as i32 + delta).clamp(0, 127) as u8;
                    }
                });
            }
            EngineEvent::SetPatternBars(bars) => {
                let bars_clamped = bars.clamp(1, 16) as u16;
                let mut guard = self.project.write();
                let proj = project_mut(&mut guard);
                let pat_idx =
                    proj.active_pattern_index(self.group_focus as usize, self.active_scene);
                if let Some(g) = proj.groups.get_mut(self.group_focus as usize) {
                    if let Some(pi) = pat_idx {
                        if let Some(p) = g.patterns.get_mut(pi) {
                            p.length = crate::project::BarsBeatsTicks::new(bars_clamped, 0, 0);
                        }
                    }
                }
                self.transport.loop_end = (bars_clamped as u64) * 4 * 960;
                self.selected_bar = self.selected_bar.min(bars_clamped as usize - 1);
            }
            EngineEvent::AcidCutoffDelta(delta) => {
                let base = self.acid_synth.params.cutoff_hz.max(20.0);
                let factor = 2.0f32.powf(delta * 0.05);
                self.acid_synth.params.cutoff_hz = (base * factor).clamp(20.0, 18000.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidResoDelta(delta) => {
                let cur = self.acid_synth.params.resonance;
                self.acid_synth.params.resonance = (cur + delta * 0.02).clamp(0.0, 0.985);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidEnvModDelta(delta) => {
                let cur = self.acid_synth.params.env_mod;
                self.acid_synth.params.env_mod = (cur + delta * 0.02).clamp(0.0, 1.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDecayDelta(delta) => {
                let cur = self.acid_synth.params.decay_ms;
                let factor = 2.0f32.powf(delta * 0.05);
                self.acid_synth.params.decay_ms = (cur * factor).clamp(20.0, 3000.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidAccentDelta(delta) => {
                let cur = self.acid_synth.params.accent;
                self.acid_synth.params.accent = (cur + delta * 0.02).clamp(0.0, 1.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidSlideDelta(delta) => {
                let cur = self.acid_synth.params.slide_time_ms;
                self.acid_synth.params.slide_time_ms = (cur + delta * 2.0).clamp(0.0, 250.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDriveDelta(delta) => {
                let cur = self.acid_synth.params.drive;
                self.acid_synth.params.drive = (cur + delta * 0.02).clamp(0.0, 1.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDriveToneDelta(delta) => {
                let cur = self.acid_synth.params.drive_tone.max(200.0);
                let factor = 2.0f32.powf(delta * 0.05);
                self.acid_synth.params.drive_tone = (cur * factor).clamp(500.0, 16000.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDriveMixDelta(delta) => {
                let cur = self.acid_synth.params.drive_mix;
                self.acid_synth.params.drive_mix = (cur + delta * 0.02).clamp(0.0, 1.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDelayTimeDelta(delta) => {
                let cur = self.acid_synth.params.delay_time_ms;
                self.acid_synth.params.delay_time_ms = (cur + delta * 5.0).clamp(20.0, 1000.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDelayFeedbackDelta(delta) => {
                let cur = self.acid_synth.params.delay_feedback;
                self.acid_synth.params.delay_feedback = (cur + delta * 0.02).clamp(0.0, 0.95);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidDelayMixDelta(delta) => {
                let cur = self.acid_synth.params.delay_mix;
                self.acid_synth.params.delay_mix = (cur + delta * 0.02).clamp(0.0, 1.0);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidWaveformToggle => {
                self.acid_synth.params.waveform = match self.acid_synth.params.waveform {
                    crate::acid_synth::AcidWaveform::Saw => crate::acid_synth::AcidWaveform::Square,
                    crate::acid_synth::AcidWaveform::Square => crate::acid_synth::AcidWaveform::Saw,
                };
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidOctaveDelta(d) => {
                self.acid_synth.params.octave = (self.acid_synth.params.octave + d).clamp(-2, 2);
                self.persist_acid_to_active_sound();
            }
            EngineEvent::AcidSelectPreset(idx) => {
                match idx {
                    0 => {
                        self.acid_synth.params =
                            crate::acid_synth::AcidParams::preset_classic_acid()
                    }
                    1 => {
                        self.acid_synth.params =
                            crate::acid_synth::AcidParams::preset_squelch_lead()
                    }
                    2 => self.acid_synth.params = crate::acid_synth::AcidParams::preset_distorted(),
                    3 => self.acid_synth.params = crate::acid_synth::AcidParams::preset_dub_echo(),
                    _ => {}
                }
                if idx <= 3 {
                    self.acid_preset = idx;
                    // En page TB, appliquer le preset au Sound actif (créé synthé si besoin).
                    if self.left_tab == 4 && self.plugin_page == 1 {
                        self.ensure_active_sound_synth();
                    }
                    self.persist_acid_to_active_sound();
                }
            }
            EngineEvent::AcidSetParams(p) => {
                self.acid_synth.params = p;
                if self.left_tab == 4 && self.plugin_page == 1 {
                    self.ensure_active_sound_synth();
                }
                self.persist_acid_to_active_sound();
            }
        }
        Ok(())
    }

    /// Index preset sélectionné dans le Browser
    #[must_use]
    pub fn browser_preset_idx(&self) -> u8 {
        self.browser_preset_idx
    }

    /// Joue un aperçu sonore immédiat d'un preset drum synth (pour pré-écoute dans le browser)
    pub fn preview_preset(&mut self, preset_idx: u8) {
        let sample = self.drum_preset_sample(preset_idx.min(15));
        self.sampler.play_raw(&sample, 0.7);
    }

    fn drum_preset_sample(&self, preset_idx: u8) -> std::sync::Arc<Vec<f32>> {
        if let Some(sample) = self
            .preset_samples
            .get(preset_idx as usize)
            .and_then(Option::as_ref)
        {
            return sample.clone();
        }

        let sample_rate = self.transport.sample_rate;
        let sample = match preset_idx {
            0 => crate::drum_synth::generate_kick(sample_rate),
            1 => crate::drum_synth::generate_snare(sample_rate),
            2 => crate::drum_synth::generate_closed_hh(sample_rate),
            3 => crate::drum_synth::generate_open_hh(sample_rate),
            4 => crate::drum_synth::generate_tom(sample_rate, 85.0),
            5 => crate::drum_synth::generate_tom(sample_rate, 130.0),
            6 => crate::drum_synth::generate_tom(sample_rate, 185.0),
            7 => crate::drum_synth::generate_clap(sample_rate),
            8 => crate::drum_synth::generate_rimshot(sample_rate),
            9 => crate::drum_synth::generate_shaker(sample_rate),
            10 => crate::drum_synth::generate_cowbell(sample_rate),
            11 => crate::drum_synth::generate_crash(sample_rate),
            12 => crate::drum_synth::generate_perc(sample_rate, 880.0),
            13 => crate::drum_synth::generate_perc(sample_rate, 1250.0),
            14 => crate::drum_synth::generate_synth_stab(sample_rate),
            _ => crate::drum_synth::generate_sub_bass(sample_rate),
        };
        std::sync::Arc::new(sample)
    }

    /// Efface toutes les notes d'un son dans le pattern actif du groupe (mode ERASE + pad).
    pub fn erase_sound(&mut self, group: u8, sound: u8) {
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group as usize, self.active_scene);
        if let Some(g) = proj.groups.get_mut(group as usize) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = g.patterns.get_mut(pi) {
                    let before = pat.events.len();
                    pat.events.retain(|e| e.pad != sound);
                    if pat.events.len() != before {
                        log::debug!("erase sound {sound} in group {group}");
                    }
                }
            }
        }
    }

    /// Joue le clic métronome (accent = début de mesure).
    fn trigger_click(&mut self, accent: bool, start_frame: usize) {
        let sample = if accent {
            self.click_accent.clone()
        } else {
            self.click_normal.clone()
        };
        self.sampler
            .play_raw_at(&sample, if accent { 0.5 } else { 0.32 }, start_frame);
    }

    fn start_count_in(&mut self) {
        self.count_in = Some(0);
        self.show_status("Pré-compte : 1 mesure, puis enregistrement".to_string());
    }

    /// Avance le transport + séquenceur, déclenche les notes des patterns.
    pub fn advance(&mut self, samples: u64) {
        // Décroissance de l'activité des pads (animation écrans, ~170 ms).
        let dt = samples as f32 / self.transport.sample_rate.max(1) as f32;
        let mut activity_alive = false;
        for v in &mut self.pad_activity {
            if *v > 0.0 {
                *v = (*v - dt * PAD_ACTIVITY_DECAY).max(0.0);
                activity_alive = true;
            }
        }
        if activity_alive {
            self.revision = self.revision.wrapping_add(1);
        }

        let prev_tick = self.transport.tick;
        let frac_before = self.transport.tick_fraction;
        self.transport.advance_samples(samples);
        let new_tick = self.transport.tick;
        if new_tick != prev_tick {
            self.revision = self.revision.wrapping_add(1);
        }

        // Traitement du Note Repeat
        const NOTE_REPEAT_TICKS: [u32; 8] = [120, 240, 480, 960, 1920, 160, 320, 640];
        let rep_rate_ticks = NOTE_REPEAT_TICKS[self.note_repeat_rate.min(7) as usize];
        if self.note_repeat_enabled {
            let arp_on = self.arp_held.iter().any(|&v| v > 0);
            let due = if self.transport.is_playing() {
                new_tick / rep_rate_ticks as u64 > prev_tick / rep_rate_ticks as u64
            } else if arp_on || self.note_repeat_held_pads.iter().any(|v| v.is_some()) {
                let bpm = self.transport.bpm.max(1.0) as f64;
                let samples_per_repeat = ((rep_rate_ticks as f64 / 960.0)
                    * (60.0 / bpm)
                    * self.transport.sample_rate.max(1) as f64)
                    as u64;
                self.note_repeat_samples += samples;
                let due = self.note_repeat_samples >= samples_per_repeat;
                if due {
                    self.note_repeat_samples %= samples_per_repeat.max(1);
                }
                due
            } else {
                false
            };
            if due {
                let recording = self.transport.state == TransportState::Recording;
                let held = self.note_repeat_held_pads;
                for (pad, opt_v) in held.iter().enumerate() {
                    if let Some(vel) = *opt_v {
                        self.trigger_pad_voice(pad as u8, vel);
                        if recording {
                            self.queue_note(pad as u8, 36 + pad as u8, vel);
                        }
                    }
                }
                if arp_on || self.arp_last.is_some() {
                    self.arp_step();
                }
            }
        }

        // Pré-compte : un clic par temps sur une mesure (accent sur le premier),
        // puis l'enregistrement démarre. Compteur de samples, sans allocation.
        // shortcut: mesure en 4/4 fixe, à revoir si les signatures arrivent.
        if let Some(done) = self.count_in {
            let beat = (f64::from(self.transport.sample_rate) * 60.0
                / f64::from(self.transport.bpm.max(1.0))) as u64;
            let beat = beat.max(1);
            let end = done + samples;
            let mut b = done.div_ceil(beat);
            while b < 4 && b * beat < end {
                self.trigger_click(b == 0, (b * beat - done) as usize);
                b += 1;
            }
            if end >= 4 * beat {
                self.count_in = None;
                self.transport.record();
                self.revision = self.revision.wrapping_add(1);
            } else {
                self.count_in = Some(end);
            }
        }
        // Voix de monitoring : on coupe la note live à la fin quantifiée
        // enregistrée, pour que l'écoute et le pattern restent synchronisés.
        let mut due = [None; 16];
        for (i, slot) in self.pending_note_offs.iter_mut().enumerate() {
            if let Some((pad, note, end)) = *slot {
                if self.transport.tick >= end {
                    due[i] = Some((pad, note));
                    *slot = None;
                }
            }
        }
        for (pad, note) in due.into_iter().flatten() {
            self.release_sound_voice(pad, note);
        }
        if !self.transport.is_playing() {
            return;
        }
        // Contexte du bloc : place chaque note au sample près au lieu de la
        // quantifier à la taille du buffer (sinon le groove est décalé).
        let bpm = self.transport.bpm.max(1.0) as f64;
        let ppq = self.transport.ppq.max(1) as f64;
        let ctx = BlockCtx {
            frames: samples,
            samples_per_tick: self.transport.sample_rate.max(1) as f64 * 60.0 / (bpm * ppq),
            tick_fraction: frac_before,
        };
        // Clic métronome sur chaque temps (accent sur début de mesure).
        if self.transport.metronome && new_tick > prev_tick {
            let first_beat = prev_tick / 960 + 1;
            let last_beat = new_tick / 960;
            for beat in first_beat..=last_beat.min(first_beat + 8) {
                let accent = beat % 4 == 0;
                let rel = (beat * 960).saturating_sub(prev_tick) as f64;
                self.trigger_click(accent, ctx.offset(rel));
            }
        }
        // Pour chaque Group, joue les events du pattern actif de la scène courante
        // (verrou lecture O(1) : plus de clone du projet dans le callback audio !)
        let proj = Arc::clone(&*self.project.read());

        // Mode Song : détermine la scène active selon la position dans le Song
        if self.song_mode && !proj.song.is_empty() {
            let total: u64 = proj.song.iter().map(|s| s.length.to_ticks() as u64).sum();
            if total > 0 {
                let pos = new_tick % total;
                let mut acc = 0u64;
                let mut idx = 0usize;
                for (i, sec) in proj.song.iter().enumerate() {
                    let len = sec.length.to_ticks() as u64;
                    if pos < acc + len {
                        idx = i;
                        break;
                    }
                    acc += len;
                    idx = (i + 1).min(proj.song.len() - 1);
                }
                self.active_section = Some(idx);
                if let Some(sec) = proj.song.get(idx) {
                    self.active_scene = Some(sec.scene);
                }
            }
        }

        // Lancements en attente : ils prennent effet au début de la mesure
        // (sans écrire le projet ici : `commit_launches` s'en charge).
        if new_tick / 3840 > prev_tick / 3840 || prev_tick == 0 {
            if let Some(s) = self.pending_scene.take() {
                self.active_scene = Some(s);
                self.launched_patterns = [None; 8];
            }
            for g in 0..8 {
                if let Some(p) = self.pending_patterns[g].take() {
                    self.launched_patterns[g] = Some(p);
                }
            }
        }
        if let Some(scene_idx) = self.active_scene {
            if let Some(scene) = proj.scenes.get(scene_idx) {
                for (gi, pat_opt) in scene.patterns.iter().enumerate() {
                    let launched = self.launched_patterns.get(gi).copied().flatten();
                    let pat_opt = &launched.or(*pat_opt);
                    // Respecte MUTE et SOLO par groupe.
                    if !self.group_audible(gi as u8) {
                        continue;
                    }
                    if proj.groups.get(gi).map(|g| g.muted).unwrap_or(false) {
                        continue;
                    }
                    if let Some(group) = proj.groups.get(gi) {
                        let mut pat_to_play = None;
                        let mut eff_prev = prev_tick;
                        let mut eff_new = new_tick;

                        if let Some(ref chain) = self.pattern_chains[gi] {
                            if chain.len() > 1 {
                                let total: u64 = chain
                                    .iter()
                                    .filter_map(|&pi| group.patterns.get(pi))
                                    .map(|p| p.length.to_ticks() as u64)
                                    .sum();
                                if total > 0 {
                                    let pos = new_tick % total;
                                    let mut acc = 0u64;
                                    for &pi in chain {
                                        if let Some(p) = group.patterns.get(pi) {
                                            let plen = p.length.to_ticks() as u64;
                                            if pos < acc + plen {
                                                pat_to_play = Some(p);
                                                eff_prev = (prev_tick % total).saturating_sub(acc);
                                                eff_new = pos.saturating_sub(acc);
                                                break;
                                            }
                                            acc += plen;
                                        }
                                    }
                                }
                            }
                        }

                        if pat_to_play.is_none() {
                            if let Some(pat_idx) = pat_opt {
                                pat_to_play = group.patterns.get(*pat_idx);
                            }
                        }

                        if let Some(pat) = pat_to_play {
                            let gain = db_to_linear(group.volume_db);
                            let mut vp = [crate::sampler::VoiceParams::default(); 16];
                            let mut sg = [1.0f32; 16];
                            let mut sp = [0.0f32; 16];
                            let mut is_synth = [false; 16];
                            let mut is_kick = [false; 16];
                            let mut is_wavetable = [false; 16];
                            for s in 0..16 {
                                if let Some(snd) = group.sounds[s].as_ref() {
                                    vp[s] = crate::sampler::VoiceParams {
                                        attack_ms: snd.adsr.attack_ms,
                                        decay_ms: snd.adsr.decay_ms,
                                        sustain: snd.adsr.sustain,
                                        release_ms: snd.adsr.release_ms,
                                        cutoff_hz: snd.filter.cutoff_hz,
                                        resonance: snd.filter.resonance,
                                        kind: snd.filter.kind,
                                    };
                                    sg[s] = db_to_linear(snd.volume_db);
                                    sp[s] = snd.pan;
                                    is_synth[s] = snd.synth.is_some();
                                    is_kick[s] = snd.kick_synth.is_some();
                                    is_wavetable[s] = snd.wavetable_synth.is_some();
                                }
                            }
                            Self::trigger_pattern_events(
                                pat,
                                eff_prev,
                                eff_new,
                                &mut self.sampler,
                                &mut self.rack,
                                gi as u8,
                                gain,
                                group.pan,
                                &sg,
                                &sp,
                                &vp,
                                &is_synth,
                                &is_kick,
                                &is_wavetable,
                                self.transport.swing,
                                &mut self.pad_flash_until,
                                new_tick,
                                &ctx,
                                &mut self.pad_activity,
                                self.muted_sounds[gi],
                            );
                        }
                    }
                }
            }
        }
    }

    /// Knobs des modes KEYBOARD / CHORDS : exactement les réglages affichés
    /// au-dessus d'eux (écrans `render_keyboard_*` / `render_chords_*`).
    fn turn_keyboard_knob(&mut self, knob: usize, delta: i32) -> crate::Result<()> {
        use crate::plugin_pages::DETENT_TICKS;
        let knob = knob.min(7);
        if knob == 7 {
            let v = (self.master_volume + delta as f32 / 1536.0).clamp(0.0, 1.0);
            return self.handle_event(EngineEvent::MasterVolume(v));
        }
        let acc = self.event_knob_accum[knob] + delta;
        let steps = acc / DETENT_TICKS;
        self.event_knob_accum[knob] = acc - steps * DETENT_TICKS;
        if steps == 0 {
            return Ok(());
        }
        let kb = &mut self.keyboard;
        match (self.controller_mode == 2, knob) {
            (_, 0) => kb.cycle_root(steps),
            (_, 1) => kb.cycle_scale(steps),
            (false, 2) | (true, 4) => kb.cycle_octave(steps),
            (true, 2) => kb.cycle_chord(steps),
            (true, 3) => kb.cycle_inversion(steps),
            (_, 6) => {
                self.arp_mode =
                    (i32::from(self.arp_mode) + steps).rem_euclid(ARP_MODES.len() as i32) as u8;
                self.show_status(format!("Arp : {}", ARP_MODES[self.arp_mode as usize]));
            }
            (false, 3) => {
                let s = (i32::from(self.active_sound) + steps).clamp(0, 15) as u8;
                return self.handle_event(EngineEvent::SelectSound(s));
            }
            (false, 5) => {
                let g = (i32::from(self.group_focus) + steps).rem_euclid(8) as u8;
                return self.handle_event(EngineEvent::GroupFocus(g));
            }
            _ => {}
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    /// Reporte dans la scène du projet les patterns lancés par le séquenceur
    /// (appelé hors du thread audio : à chaque événement et par les boucles
    /// de l'interface).
    pub fn commit_launches(&mut self) {
        let notes = self.pending_notes;
        if self.launched_patterns.iter().all(Option::is_none) && notes.iter().all(Option::is_none) {
            return;
        }
        self.pending_notes = [None; 64];
        {
            let mut guard = self.project.write();
            let proj = project_mut(&mut guard);
            for g in 0..8 {
                if let Some(p) = self.launched_patterns[g].take() {
                    assign_pattern_to_active_scene(proj, &mut self.active_scene, g, p);
                }
            }
        }
        for n in notes.into_iter().flatten() {
            let _ = self.record_note_at(n.group, n.pad, n.pitch, n.velocity, n.tick);
        }
        self.revision = self.revision.wrapping_add(1);
    }

    /// Met en file une note jouée par le thread audio (voir `pending_notes`).
    fn queue_note(&mut self, pad: u8, pitch: u8, velocity: u8) {
        let note = PendingNote {
            group: self.group_focus,
            pad,
            pitch,
            velocity,
            tick: self.transport.tick,
        };
        // shortcut: 64 notes entre deux validations (~16 ms), au-delà elles sont perdues.
        if let Some(slot) = self.pending_notes.iter_mut().find(|n| n.is_none()) {
            *slot = Some(note);
        }
    }

    /// Pas de l'arpégiateur : relâche la note précédente et joue la suivante
    /// parmi les notes tenues (et leur octave au-dessus en mode « 2 »), selon
    /// le sens du mode (`ARP_MODES`).
    fn arp_step(&mut self) {
        let two_octaves = self.arp_mode >= 3;
        let held = |n: usize| match self.arp_held[n] {
            0 if two_octaves && n >= 12 => self.arp_held[n - 12],
            v => v,
        };
        let up = |from: usize| (from..128).find(|&n| held(n) > 0);
        let down = |to: usize| (0..to).rev().find(|&n| held(n) > 0);
        let last = self.arp_last.map(usize::from);
        let next = match (self.arp_mode % 3, last) {
            (1, None) => down(128),
            (_, None) => up(0),
            (0, Some(l)) => up(l + 1).or_else(|| up(0)),
            (1, Some(l)) => down(l).or_else(|| down(128)),
            // HAUT-BAS : on repart dans l'autre sens au bout de la plage.
            (_, Some(l)) if self.arp_falling => down(l).or_else(|| up(l + 1)),
            (_, Some(l)) => up(l + 1).or_else(|| down(l)),
        }
        .or(last.filter(|&l| held(l) > 0));
        let velocity = next.map_or(0, held);
        if let (Some(n), Some(l)) = (next, last) {
            if n != l {
                self.arp_falling = n < l;
            }
        }
        if let Some(last) = self.arp_last.take() {
            self.release_sound_voice(self.active_sound, last);
        }
        let Some(note) = next else {
            return;
        };
        if !self.group_audible(self.group_focus) {
            return;
        }
        let note = note as u8;
        self.trigger_sound_voice(self.active_sound, note, velocity);
        self.arp_last = Some(note);
        if self.transport.state == TransportState::Recording {
            self.queue_note(self.active_sound, note, velocity);
        }
    }

    /// Un groupe s'entend-il ? Ni muet (MUTE), ni écarté par un SOLO.
    #[must_use]
    pub fn group_audible(&self, group: u8) -> bool {
        let bit = 1u8 << (group % 8);
        self.muted_groups & bit == 0 && (self.soloed_groups == 0 || self.soloed_groups & bit != 0)
    }

    #[must_use]
    pub fn is_sound_muted(&self, group: u8, pad: u8) -> bool {
        if group >= 8 || pad >= 16 {
            return false;
        }
        (self.muted_sounds[group as usize] & (1u16 << pad)) != 0
    }

    #[must_use]
    pub fn is_group_muted(&self, group: u8) -> bool {
        if group >= 8 {
            return false;
        }
        (self.muted_groups & (1u8 << group)) != 0
    }

    /// Marque l'activité d'un pad (animation écrans) — 0..1 selon vélocité.
    pub fn note_pad_activity(&mut self, pad: u8, velocity: u8) {
        let index = (pad as usize).min(15);
        let level = (velocity as f32 / 127.0).clamp(0.35, 1.0);
        self.pad_activity[index] = self.pad_activity[index].max(level);
    }

    #[allow(clippy::too_many_arguments)] // contexte de trigger (pattern + position + sampler + synth)
    fn trigger_pattern_events(
        pat: &crate::project::Pattern,
        prev_tick: u64,
        new_tick: u64,
        sampler: &mut Sampler,
        rack: &mut SynthRack,
        group: u8,
        group_gain: f32,
        group_pan: f32,
        sound_gains: &[f32],
        sound_pans: &[f32],
        voice_params: &[crate::sampler::VoiceParams],
        is_synth: &[bool],
        is_kick: &[bool],
        is_wavetable: &[bool],
        swing: f32,
        flash_until: &mut [u64; 16],
        now_tick: u64,
        ctx: &BlockCtx,
        activity: &mut [f32; 16],
        muted_sound_mask: u16,
    ) {
        let pat_len = pat.length.to_ticks() as u64;
        if pat_len == 0 || prev_tick == new_tick {
            return;
        }
        // Swing Maschine : retarde les doubles-croches impaires à la lecture.
        let swing_ticks = (swing.clamp(0.0, 1.0) * 120.0) as u64;
        // gère wrap pattern (loop)
        let prev_in_pat = prev_tick % pat_len;
        let new_in_pat = new_tick % pat_len;
        let wrapped = new_in_pat < prev_in_pat;
        for ev in &pat.events {
            let tick = ev.tick as u64;
            // Tick effectif avec swing (16e impaire = retardée)
            let eff = if swing_ticks > 0 && (tick / 240) % 2 == 1 {
                tick + swing_ticks
            } else {
                tick
            };
            let in_window = |t: u64| {
                if !wrapped {
                    if prev_tick == 0 {
                        t >= prev_in_pat && t <= new_in_pat
                    } else {
                        t > prev_in_pat && t <= new_in_pat
                    }
                } else {
                    t > prev_in_pat || t <= new_in_pat
                }
            };
            if in_window(eff % pat_len.max(1)) {
                let sound = ev.pad;
                let sound_muted =
                    (sound as usize) < 16 && (muted_sound_mask & (1u16 << sound)) != 0;
                if !sound_muted {
                    let note = ev.pitch;
                    let sg = sound_gains.get(sound as usize).copied().unwrap_or(1.0);
                    let sp = sound_pans.get(sound as usize).copied().unwrap_or(0.0);
                    // Décalage sample-accurate dans le bloc courant : la note part à
                    // sa vraie position rythmique, pas au début du buffer audio.
                    let local = eff % pat_len.max(1);
                    let rel_ticks = if !wrapped || local > prev_in_pat {
                        local.saturating_sub(prev_in_pat) as f64
                    } else {
                        (pat_len - prev_in_pat + local) as f64
                    };
                    let start_frame = ctx.offset(rel_ticks);
                    let is_acid = is_synth.get(sound as usize).copied().unwrap_or(false);
                    let is_k = is_kick.get(sound as usize).copied().unwrap_or(false);
                    let is_wt = is_wavetable.get(sound as usize).copied().unwrap_or(false);
                    let ri = rack_index(group, sound);
                    if is_k {
                        if let Some(slot) = rack.kick[ri].as_mut() {
                            slot.idle_blocks = 0;
                            slot.synth.trigger(note, ev.velocity);
                        }
                    } else if is_acid {
                        if let Some(slot) = rack.acid[ri].as_mut() {
                            slot.idle_blocks = 0;
                            slot.synth.note_on_at(note, ev.velocity, start_frame);
                        }
                    } else if is_wt {
                        if let Some(slot) = rack.wavetable[ri].as_mut() {
                            slot.idle_blocks = 0;
                            slot.synth.note_on(note, ev.velocity);
                        }
                    } else if let Some(idx) = sampler.note_on_at(
                        sampler_sound_id(group, sound),
                        note,
                        ev.velocity,
                        note,
                        start_frame,
                    ) {
                        sampler.set_voice_gain(idx, group_gain * sg);
                        sampler.set_voice_pan(idx, sp);
                        sampler.set_voice_group(idx, group);
                        sampler.set_voice_group_pan(idx, group_pan);
                        if let Some(vp) = voice_params.get(sound as usize) {
                            sampler.set_voice_params(idx, vp);
                        }
                    }
                    // Flash LED du pad (fenêtre d'une triple-croche : bref et net).
                    if (sound as usize) < 16 {
                        flash_until[sound as usize] = now_tick + 120;
                        let level = (ev.velocity as f32 / 127.0).clamp(0.35, 1.0);
                        if level > activity[sound as usize] {
                            activity[sound as usize] = level;
                        }
                    }
                }
            }
            // Note-off / longueur : déclenche release quand la fin de l'événement est franchie
            let end_tick = tick + ev.length_ticks as u64;
            let end_in_pat = end_tick % pat_len;
            let off_triggered = if !wrapped {
                end_tick > tick && end_in_pat > prev_in_pat && end_in_pat <= new_in_pat
            } else {
                end_in_pat > prev_in_pat || end_in_pat <= new_in_pat
            };
            // évite de couper la note à t=0 et seulement si la note a déjà sonné
            if off_triggered && end_tick > tick && tick <= new_tick {
                let ri = rack_index(group, ev.pad);
                if is_synth.get(ev.pad as usize).copied().unwrap_or(false) {
                    if let Some(slot) = rack.acid[ri].as_mut() {
                        slot.synth.note_off(ev.pitch);
                    }
                } else if is_wavetable.get(ev.pad as usize).copied().unwrap_or(false) {
                    if let Some(slot) = rack.wavetable[ri].as_mut() {
                        slot.synth.note_off(ev.pitch);
                    }
                } else {
                    sampler.note_off(sampler_sound_id(group, ev.pad), ev.pitch);
                }
            }
        }
    }

    fn record_pad_event(&mut self, pad: u8, velocity: u8) -> Option<u32> {
        self.record_note_event(pad, 36 + pad, velocity)
    }

    fn record_note_event(&mut self, pad: u8, pitch: u8, velocity: u8) -> Option<u32> {
        self.record_note_at(self.group_focus, pad, pitch, velocity, self.transport.tick)
    }

    fn record_note_at(
        &mut self,
        group: u8,
        pad: u8,
        pitch: u8,
        velocity: u8,
        tick: u64,
    ) -> Option<u32> {
        let focus = group as usize;
        // Pendant REC : l'historique couvre les notes ajoutées (Shift + Undo).
        if self.transport.state == TransportState::Recording {
            self.push_project_history();
        }
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        // Comme le logiciel officiel : enregistrer sur un groupe sans pattern
        // dans la scène en crée un et l'y place (sinon les notes se perdaient).
        let pat_idx = ensure_group_pattern(proj, &mut self.active_scene, focus);
        let mut recorded = None;
        if let Some(group) = proj.groups.get_mut(focus) {
            if let Some(idx) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(idx) {
                    let pat_len = pat.length.to_ticks().max(1);
                    let tick_in_pat = (tick % pat_len as u64) as u32;
                    let q_tick = self.sequencer.grid.quantize(tick_in_pat);
                    // évite les doublons exacts (même son + tick + hauteur)
                    pat.events
                        .retain(|e| !(e.pad == pad && e.tick == q_tick && e.pitch == pitch));
                    pat.events.push(PatternEvent {
                        pad,
                        tick: q_tick,
                        velocity,
                        length_ticks: 120,
                        pitch,
                    });
                    pat.events.sort_by_key(|e| e.tick);
                    recorded = Some(q_tick);
                }
            }
        }
        recorded
    }

    /// Met à jour la longueur d'une note enregistrée pendant REC : celle de la
    /// note tenue (relâchement du pad / de la touche), comme en mode keyboard.
    /// Longueur quantisée à la grille d'une note tenue pendant REC.
    fn quantize_note_len(&self, from_tick: u32, release_tick: u64) -> u32 {
        let step = self.sequencer.grid.ticks().max(1) as u64;
        let raw = release_tick.saturating_sub(u64::from(from_tick));
        (((raw + step / 2) / step * step).max(step).min(3840 * 8)) as u32
    }

    /// Pendant REC, la voix live est coupée à la fin quantifiée de la note —
    /// pas au relâchement physique — pour que l'écoute colle à l'enregistrement.
    fn schedule_note_off(&mut self, pad: u8, note: u8, from_tick: u32, release_tick: u64) {
        let len = self.quantize_note_len(from_tick, release_tick);
        let end = u64::from(from_tick).saturating_add(len as u64);
        if let Some(slot) = self.pending_note_offs.iter_mut().find(|s| s.is_none()) {
            *slot = Some((pad, note, end));
        }
    }

    fn set_recorded_length(&mut self, group: u8, pad: u8, q_tick: u32, release_tick: u64) {
        if self.transport.state != TransportState::Recording {
            return;
        }
        let len = self.quantize_note_len(q_tick, release_tick);
        let focus = group as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        if let Some(pat) = proj
            .active_pattern_index(focus, self.active_scene)
            .and_then(|i| proj.groups.get_mut(focus)?.patterns.get_mut(i))
        {
            if let Some(e) = pat
                .events
                .iter_mut()
                .rev()
                .find(|e| e.pad == pad && e.tick == q_tick)
            {
                e.length_ticks = len;
            }
        }
    }

    /// Étend l'événement enregistré pendant REC jusqu'à un nouveau tick
    /// (slide legato : le portamento du TB-303 continue jusqu'à la nouvelle
    /// note au lieu d'être coupé).
    fn extend_recorded_note(&mut self, group: u8, pad: u8, q_prev: u32, q_new: u32) {
        if q_new <= q_prev {
            return;
        }
        let focus = group as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        if let Some(pat) = proj
            .active_pattern_index(focus, self.active_scene)
            .and_then(|i| proj.groups.get_mut(focus)?.patterns.get_mut(i))
        {
            if let Some(e) = pat
                .events
                .iter_mut()
                .rev()
                .find(|e| e.pad == pad && e.tick == q_prev)
            {
                let step = self.sequencer.grid.ticks().max(1);
                let raw = q_new - q_prev;
                e.length_ticks = ((raw + step / 2) / step * step).max(step).min(3840 * 8);
            }
        }
    }

    /// Efface les marques de notes tenues en cours d'enregistrement (REC).
    fn clear_rec_press_marks(&mut self) {
        self.rec_pad_press = [None; 16];
        self.rec_key_press = [None; 128];
        self.rec_sounding = [None; 16];
        self.pending_note_offs = [None; 16];
    }

    /// Enregistre un point d'automation pendant REC (knob du Sound actif) dans
    /// le Pattern actif : crée ou complète l'automation du (Sound, knob).
    fn record_sound_automation(&mut self, group: u8, sound: u8, knob: u8, value: f32) {
        // Le bouton AUTO doit être allumé (armé), comme sur la MK3.
        if !self.automation_armed || self.transport.state != TransportState::Recording {
            return;
        }
        let focus = group as usize;
        let tick = self.transport.tick.min(u32::MAX as u64) as u32;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let Some(idx) = proj.active_pattern_index(focus, self.active_scene) else {
            return;
        };
        let Some(pat) = proj
            .groups
            .get_mut(focus)
            .and_then(|g| g.patterns.get_mut(idx))
        else {
            return;
        };
        if let Some(auto) = pat
            .automations
            .iter_mut()
            .find(|a| a.sound == Some(sound) && a.knob == knob)
        {
            if let Some(last) = auto.points.last_mut() {
                if last.0 == tick {
                    last.1 = value;
                    return;
                }
            }
            auto.points.push((tick, value));
        } else {
            pat.automations.push(SoundAutomation {
                sound: Some(sound),
                knob,
                points: vec![(tick, value)],
            });
        }
    }

    fn mix_synth_buffer(
        &mut self,
        out: &mut [f32],
        out_len: usize,
        (group, sound): (u8, u8),
        bus: Option<u8>,
        proj: &Project,
    ) {
        let is_muted = !self.group_audible(group)
            || proj
                .groups
                .get(group as usize)
                .map(|g| g.muted)
                .unwrap_or(false)
            || self.is_sound_muted(group, sound)
            || proj
                .groups
                .get(group as usize)
                .and_then(|g| g.sounds.get(sound as usize))
                .and_then(|s| s.as_ref())
                .map(|s| s.muted)
                .unwrap_or(false);

        if is_muted {
            return;
        }

        let snd_gain = proj
            .groups
            .get(group as usize)
            .and_then(|g| g.sounds.get(sound as usize))
            .and_then(|s| s.as_ref())
            .map(|s| db_to_linear(s.volume_db))
            .unwrap_or(1.0);
        let grp_gain = proj
            .groups
            .get(group as usize)
            .map(|g| db_to_linear(g.volume_db))
            .unwrap_or(1.0);
        let total_gain = snd_gain * grp_gain;
        let snd_pan = proj
            .groups
            .get(group as usize)
            .and_then(|g| g.sounds.get(sound as usize))
            .and_then(|s| s.as_ref())
            .map(|s| s.pan)
            .unwrap_or(0.0);
        let grp_pan = proj
            .groups
            .get(group as usize)
            .map(|g| g.pan)
            .unwrap_or(0.0);
        let total_pan = (snd_pan + grp_pan).clamp(-1.0, 1.0);
        let pan_l = (1.0 - total_pan).min(1.0) * total_gain;
        let pan_r = (1.0 + total_pan).min(1.0) * total_gain;

        let dst: &mut [f32] = match bus.map(usize::from) {
            Some(b) if self.fx_bus_bufs.get(b).is_some_and(|v| v.len() >= out_len) => {
                &mut self.fx_bus_bufs[b]
            }
            _ => &mut *out,
        };
        let mut peak = 0.0f32;
        for f in 0..out_len / 2 {
            let l = self.synth_buf[f * 2] * pan_l;
            let r = self.synth_buf[f * 2 + 1] * pan_r;
            dst[f * 2] += l;
            dst[f * 2 + 1] += r;
            let p = l.abs().max(r.abs());
            if p > peak {
                peak = p;
            }
        }
        let g_idx = group as usize;
        let s_idx = sound as usize;
        if peak > 0.001 && g_idx < 8 && s_idx < 16 {
            if peak > self.pad_meter[g_idx][s_idx] {
                self.pad_meter[g_idx][s_idx] = peak;
            }
            if peak > self.group_meter[g_idx] {
                self.group_meter[g_idx] = peak;
            }
        }
    }

    /// Compteur de silence d'un synthé du rack après son bloc de rendu.
    fn note_idle(&self, idle: u32, out_len: usize) -> u32 {
        let silent = self.synth_buf[..out_len].iter().all(|v| v.abs() < 1e-5);
        if silent {
            idle + 1
        } else {
            0
        }
    }

    /// Render audio stéréo — à appeler depuis callback cpal/jack.
    pub fn render_audio(&mut self, out: &mut [f32]) {
        let out_len = out.len();
        // Routage vers les inserts : bus du Sound, sinon bus de son groupe.
        let mut route = [[None::<u8>; 16]; 8];
        if out_len <= FX_BUS_LEN {
            for buf in &mut self.fx_bus_bufs {
                buf[..out_len].fill(0.0);
            }
            for (i, (owner, _)) in self.fx_buses.iter().enumerate() {
                match *owner {
                    FxOwner::Group(g) => {
                        for slot in &mut route[g as usize % 8] {
                            slot.get_or_insert(i as u8);
                        }
                    }
                    FxOwner::Sound(g, s) => route[g as usize % 8][s as usize % 16] = Some(i as u8),
                    FxOwner::Master => {}
                }
            }
        }
        self.sampler
            .render_routed(out, &route, &mut self.fx_bus_bufs);

        // Rendu des synthétiseurs avec application des volumes, panoramiques et mutes par pad/groupe
        if self.synth_buf.len() < out_len {
            self.synth_buf.resize(out_len, 0.0);
        }

        let proj = Arc::clone(&*self.project.read());

        // Synthés du rack : chaque Sound rend son instance, mixée avec son
        // volume, son pan, ses mutes et ses effets. Les instances muettes
        // depuis un moment ne sont plus rendues (jusqu'à la note suivante).
        for i in 0..128usize {
            let (g, s) = ((i / 16) as u8, (i % 16) as u8);
            let bus = route[g as usize][s as usize];
            if let Some(mut slot) = self.rack.kick[i].take() {
                if slot.idle_blocks < RACK_IDLE_BLOCKS {
                    self.synth_buf[..out_len].fill(0.0);
                    slot.synth.render(&mut self.synth_buf[..out_len]);
                    slot.idle_blocks = self.note_idle(slot.idle_blocks, out_len);
                    self.mix_synth_buffer(out, out_len, (g, s), bus, &proj);
                }
                self.rack.kick[i] = Some(slot);
            }
            if let Some(mut slot) = self.rack.acid[i].take() {
                if slot.idle_blocks < RACK_IDLE_BLOCKS {
                    self.synth_buf[..out_len].fill(0.0);
                    slot.synth.render(&mut self.synth_buf[..out_len]);
                    slot.idle_blocks = self.note_idle(slot.idle_blocks, out_len);
                    self.mix_synth_buffer(out, out_len, (g, s), bus, &proj);
                }
                self.rack.acid[i] = Some(slot);
            }
            if let Some(mut slot) = self.rack.wavetable[i].take() {
                if slot.idle_blocks < RACK_IDLE_BLOCKS {
                    self.synth_buf[..out_len].fill(0.0);
                    slot.synth.render(&mut self.synth_buf[..out_len]);
                    slot.idle_blocks = self.note_idle(slot.idle_blocks, out_len);
                    self.mix_synth_buffer(out, out_len, (g, s), bus, &proj);
                }
                self.rack.wavetable[i] = Some(slot);
            }
        }

        // Monitoring REC : le Sound joué en direct a sa propre voix, mixée
        // exactement comme le rack (même gain, pan et bus) — le pattern et le
        // live sonnent donc ensemble, comme sur la MK3.
        for i in 0..128usize {
            let (g, s) = ((i / 16) as u8, (i % 16) as u8);
            let bus = route[g as usize][s as usize];
            if let Some(mut slot) = self.monitor_rack.kick[i].take() {
                if slot.idle_blocks < RACK_IDLE_BLOCKS {
                    self.synth_buf[..out_len].fill(0.0);
                    slot.synth.render(&mut self.synth_buf[..out_len]);
                    slot.idle_blocks = self.note_idle(slot.idle_blocks, out_len);
                    self.mix_synth_buffer(out, out_len, (g, s), bus, &proj);
                }
                self.monitor_rack.kick[i] = Some(slot);
            }
            if let Some(mut slot) = self.monitor_rack.acid[i].take() {
                if slot.idle_blocks < RACK_IDLE_BLOCKS {
                    self.synth_buf[..out_len].fill(0.0);
                    slot.synth.render(&mut self.synth_buf[..out_len]);
                    slot.idle_blocks = self.note_idle(slot.idle_blocks, out_len);
                    self.mix_synth_buffer(out, out_len, (g, s), bus, &proj);
                }
                self.monitor_rack.acid[i] = Some(slot);
            }
            if let Some(mut slot) = self.monitor_rack.wavetable[i].take() {
                if slot.idle_blocks < RACK_IDLE_BLOCKS {
                    self.synth_buf[..out_len].fill(0.0);
                    slot.synth.render(&mut self.synth_buf[..out_len]);
                    slot.idle_blocks = self.note_idle(slot.idle_blocks, out_len);
                    self.mix_synth_buffer(out, out_len, (g, s), bus, &proj);
                }
                self.monitor_rack.wavetable[i] = Some(slot);
            }
        }

        // Inserts : chaque Sound passe dans sa chaîne puis rejoint le bus de
        // son groupe (traité ensuite : les Sounds précèdent les groupes) ou
        // la sortie ; chaque groupe passe dans la sienne puis sort.
        if out_len <= FX_BUS_LEN {
            for i in 0..self.fx_buses.len() {
                let owner = self.fx_buses[i].0;
                self.fx_buses[i]
                    .1
                    .process(&mut self.fx_bus_bufs[i][..out_len], owner.chain(&proj));
                let group_bus = match owner {
                    FxOwner::Sound(g, _) => self
                        .fx_buses
                        .iter()
                        .position(|(o, _)| *o == FxOwner::Group(g))
                        .filter(|&gb| gb > i),
                    FxOwner::Group(_) | FxOwner::Master => None,
                };
                let (src, rest) = self.fx_bus_bufs.split_at_mut(i + 1);
                let wet = &src[i][..out_len];
                let dst: &mut [f32] = match group_bus {
                    Some(gb) => &mut rest[gb - i - 1][..out_len],
                    None => &mut *out,
                };
                for (d, w) in dst.iter_mut().zip(wet) {
                    *d += *w;
                }
            }
        }
        self.master_fx.process(out, &proj.master.fx_chain);
        drop(proj);
        // VU-mètres : crêtes par groupe du sampler, attaque immédiate / release
        // exponentiel par bloc (≈ -20 dB en 300 ms), peak hold plus lent (~1,7 s).
        let peaks = self.sampler.group_peaks();
        for (g, p) in peaks.iter().enumerate() {
            let p = p.clamp(0.0, 4.0);
            self.group_meter[g] = if p > self.group_meter[g] {
                p
            } else {
                self.group_meter[g] * 0.98
            };
            self.group_peak_hold[g] = if p > self.group_peak_hold[g] {
                p
            } else {
                self.group_peak_hold[g] * 0.996
            };
        }
        // VU-mètres par pad : mesure les crêtes réelles pour chaque tranche de pad
        for g in 0..8 {
            let s_peaks = self.sampler.sound_peaks(g);
            for (s, &peak_val) in s_peaks.iter().enumerate() {
                let mut p = peak_val;
                if g == self.group_focus as usize && s == self.active_sound as usize {
                    let act = self.pad_activity[s];
                    if act > p {
                        p = act;
                    }
                }
                let p = p.clamp(0.0, 4.0);
                self.pad_meter[g][s] = if p > self.pad_meter[g][s] {
                    p
                } else {
                    self.pad_meter[g][s] * 0.96
                };
                self.pad_peak_hold[g][s] = if p > self.pad_peak_hold[g][s] {
                    p
                } else {
                    self.pad_peak_hold[g][s] * 0.995
                };
            }
        }
        // Bus master : volume + limiteur doux (tanh). Avant, la sortie était
        // écrêtée durement (clamp ±1) : kick + snare + clap (~1,7) distordaient.
        // Ici la réponse reste linéaire sous -3 dB puis sature en douceur.
        let mv = self.master_volume;
        for s in out.iter_mut() {
            *s *= mv;
        }
        // Limiteur transparent sous -1 dBFS (plus de saturation du Master).
        self.master_limiter.process(out);
        let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        self.master_meter = if peak > self.master_meter {
            peak
        } else {
            self.master_meter * 0.98
        };
        self.master_peak_hold = if peak > self.master_peak_hold {
            peak
        } else {
            self.master_peak_hold * 0.996
        };
    }

    /// Sélectionne un preset du browser (curseur) sans pré-écoute.
    pub fn set_browser_preset(&mut self, idx: u8) {
        self.browser_preset_idx = idx.min(15);
        self.revision = self.revision.wrapping_add(1);
    }

    /// Enveloppe waveform du Sound actif (pour l'UI/écrans) — `buckets` = colonnes.
    #[must_use]
    pub fn waveform_envelope(
        &self,
        group: u8,
        sound: u8,
        buckets: usize,
    ) -> Option<Arc<[(f32, f32)]>> {
        self.sampler
            .waveform_envelope(sampler_sound_id(group, sound), buckets)
    }

    /// Durée du sample d'un Sound (secondes).
    #[must_use]
    pub fn sample_duration_secs(&self, group: u8, sound: u8) -> f32 {
        self.sampler
            .sample_duration_secs(sampler_sound_id(group, sound))
    }

    /// Labels des 8 soft keys (boutons sous les écrans) pour la vue courante.
    /// Les mêmes labels sont dessinés en bas des 2 écrans et affichés dans la
    /// GUI : le bouton fait toujours ce que le label annonce.
    #[must_use]
    pub fn soft_key_labels(&self) -> [&'static str; 8] {
        if self.note_repeat_enabled {
            return ["1/32", "1/16", "1/8", "1/4", "1/2", "1/16T", "1/8T", "OFF"];
        }
        // Mode EVENTS : les deux écrans éditent les notes du pattern.
        if self.right_tab == 1 {
            return EVENT_LABELS;
        }
        // SAMPLING : boutons 1–4 = pages RECORD / EDIT / SLICE / ZONE.
        if self.left_tab == 2 && !self.shift_held {
            return self.sample_soft_labels();
        }
        // Mode PLUGIN : 4 boutons d'instrument + 4 boutons de la page affichée.
        // Shift n'y change pas les boutons (Shift + knob = réglage fin).
        if let Some(inst) = self.plugin_instrument() {
            return self.plugin_soft_labels(inst);
        }
        if self.shift_held {
            if self.scene_mode {
                return [
                    "SCN 1",
                    "SCN DER",
                    "TOUT UNI",
                    "DUPL UNI",
                    "VIDER",
                    "SUPPR",
                    "ENCH TOUT",
                    "SONG",
                ];
            }
            if self.pattern_mode {
                return [
                    "DOUBLE", "HALF", "QUANT", "HUMAN", "VIDER", "SUPPR", "4 MES", "8 MES",
                ];
            }
            if self.controller_mode == 3 {
                return [
                    "VIDER", "QUANT", "HUMAN", "NUDG -", "NUDG +", "OCT -", "OCT +", "FILL",
                ];
            }
            match self.left_tab {
                1 if self.mixer_mode == 0 => {
                    return [
                        "PAN 1", "PAN 2", "PAN 3", "PAN 4", "PAN 5", "PAN 6", "PAN 7", "PAN 8",
                    ];
                }
                1 if self.mixer_mode == 1 => {
                    return [
                        "PAN 9", "PAN 10", "PAN 11", "PAN 12", "PAN 13", "PAN 14", "PAN 15",
                        "PAN 16",
                    ];
                }
                1 => {
                    return [
                        "PAN A", "PAN B", "PAN C", "PAN D", "PAN E", "PAN F", "PAN G", "PAN H",
                    ];
                }
                2 => {
                    return [
                        "REVERSE", "LOOP", "NORMLZ", "CYC FLT", "LP/HP/BP", "NOTCH", "PEAK", "OFF",
                    ];
                }
                4 => {
                    return crate::display_render::GUITAR_SOFT_LABELS;
                }
                _ => {
                    return [
                        "METRO", "COUNT-IN", "QUANT", "HUMAN", "VIDER", "DUPL", "4 MES", "8 MES",
                    ];
                }
            }
        }
        if self.song_mode {
            return [
                "SEC <", "SEC >", "+SEC", "IDEE", "SCN <", "SCN >", "+SCN", "SCN 1",
            ];
        }
        if self.scene_mode {
            return [
                "SCN <", "SCN >", "+ SCN", "DUPL", "UNIQUE", "SUPPR", "ENCHAIN", "SONG",
            ];
        }
        if self.pattern_mode {
            // Mode PATTERN : gestion complète des patterns du groupe focus.
            return [
                "PAT <", "PAT >", "+ PAT", "DUPL", "VIDER", "SUPPR", "1 MES", "2 MES",
            ];
        }
        if self.controller_mode == 3 && self.left_tab == 0 {
            // Mode STEP : navigation dans la grille + longueur du pattern.
            return [
                "PAS <", "PAS >", "BAR <", "BAR >", "1 MES", "2 MES", "4 MES", "8 MES",
            ];
        }
        // KEYBOARD / CHORDS : boutons 5–8 = OCTAVE − / + et SEMITONE − / +
        // comme sur la MK3 ; 1–4 = gamme (et accord en CHORDS).
        if self.controller_mode == 1 {
            return [
                "CHROMA", "MINOR", "SCALE-", "SCALE+", "OCT -", "OCT +", "SEMI -", "SEMI +",
            ];
        }
        if self.controller_mode == 2 {
            return [
                "SCALE-", "SCALE+", "CHORD-", "CHORD+", "OCT -", "OCT +", "SEMI -", "SEMI +",
            ];
        }
        // Page AMPLI GUITARE : les labels annoncent exactement les actions
        // gérées par l'app (voir GuiApp::guitar_soft_key).
        if self.left_tab == 4 && self.plugin_page == 2 {
            return crate::display_render::GUITAR_SOFT_LABELS;
        }
        match self.left_tab {
            1 if self.mixer_mode == 0 || self.mixer_mode == 1 => [
                "PAD 1-8", "PAD 9-16", "GROUPES", "MASTER", "PAN/VOL", "MUTE", "SOLO", "RETOUR",
            ],
            1 => [
                "GRP A", "GRP B", "GRP C", "GRP D", "GRP E", "GRP F", "GRP G", "GRP H",
            ],
            2 => [
                "SND <", "SND >", "ECOUTE", "CYC FLT", "LP/HP/BP", "NOTCH", "PEAK", "OFF",
            ],
            3 if self.browser_kind == 1 && self.delete_project_confirm => {
                ["ANNULER", "", "", "", "", "CONFIRMER", "", ""]
            }
            3 if self.browser_kind == 1 => [
                "PROJ <", "PROJ >", "NOUVEAU", "CHARGER", "SAUVER", "SUPPR", "PRESETS", "RETOUR",
            ],
            3 if self.browser_kind == 2 => [
                "PLUG <", "PLUG >", "PRESETS", "CHARGER", "KICK", "303", "SAMPLER", "RETOUR",
            ],
            3 if self.browser_kind == 3 => [
                "FX <", "FX >", "GRP/PAD", "CHARGER", "REVERB", "DELAY", "DISTO", "RETOUR",
            ],
            3 => [
                "PRES <", "PRES >", "SND <", "SND >", "ECOUTE", "CHARGER", "PLUGINS", "PROJETS",
            ],
            _ => [
                "BAR 1", "BAR 2", "BAR 3", "BAR 4", "1 MES", "2 MES", "4 MES", "8 MES",
            ],
        }
    }

    /// Boutons d'écran allumés : un bouton s'allume quand l'état qu'il
    /// sélectionne est l'état courant (mesure, longueur, groupe, filtre…).
    /// Déduit du libellé affiché, il reste juste sur toutes les pages
    /// (couche Shift comprise), sur les écrans, la GUI et les LEDs.
    #[must_use]
    pub fn soft_key_active(&self) -> [bool; 8] {
        use crate::kick_synth::KickFilterMode as K;
        use crate::project::FilterKind as F;
        let proj = Arc::clone(&*self.project.read());
        let sound = proj
            .groups
            .get(self.group_focus as usize)
            .and_then(|g| g.sounds.get(self.active_sound as usize))
            .and_then(|s| s.as_ref());
        // Slot vide : aucun mode de filtre n'est « courant ».
        let filter = sound.map(|s| match s.kick_synth.as_ref().map(|k| k.filter_mode) {
            Some(K::Off) => F::Off,
            Some(K::LowPass) => F::LowPass,
            Some(K::HighPass) => F::HighPass,
            Some(K::BandPass) => F::BandPass,
            Some(K::Notch) => F::Notch,
            Some(K::Peak) => F::Peak,
            None => s.filter.kind,
        });
        let bars = self.active_pattern_bars();
        let bar = self.selected_bar.min(bars.saturating_sub(1)) + 1;
        let mixer = self.left_tab == 1;
        let pad_muted = self.muted_sounds[self.group_focus as usize % 8]
            & (1u16 << (self.active_sound % 16))
            != 0;
        let group_soloed = self.soloed_groups & (1u8 << (self.group_focus % 8)) != 0;
        // Même ordre que `NOTE_REPEAT_TICKS` et les libellés NOTE REPEAT.
        const RATES: [&str; 7] = ["1/32", "1/16", "1/8", "1/4", "1/2", "1/16T", "1/8T"];
        let fx = self.selected_fx().map(|(_, _, fx)| fx);
        if self.right_tab == 1 && !self.note_repeat_enabled {
            return EVENT_LABELS.map(|label| label == "TOUS" && self.events_all);
        }
        self.soft_key_labels().map(|label| {
            if let Some(fx) = fx.as_ref().filter(|_| !self.note_repeat_enabled) {
                return self.fx_soft_key_lit(fx, label);
            }
            if self.note_repeat_enabled {
                return RATES.get(self.note_repeat_rate as usize) == Some(&label);
            }
            let number = |s: Option<&str>| s.and_then(|n| n.parse::<usize>().ok());
            if let Some(n) = number(label.strip_prefix("BAR ")) {
                return n == bar;
            }
            if let Some(n) = number(label.strip_suffix(" MES")) {
                return n == bars;
            }
            if let Some(g) = label.strip_prefix("GRP ") {
                return g.bytes().next() == Some(b'A' + self.group_focus);
            }
            match label {
                "PAD 1-8" => mixer && self.mixer_mode == 0,
                "PAD 9-16" => mixer && self.mixer_mode == 1,
                "GROUPES" => mixer && self.mixer_mode == 2,
                "MASTER" => mixer && self.mixer_mode == 3,
                "MUTE" => mixer && pad_muted,
                "SOLO" => mixer && group_soloed,
                "LP/HP/BP" => matches!(filter, Some(F::LowPass | F::HighPass | F::BandPass)),
                "LP" => filter == Some(F::LowPass),
                "HP" => filter == Some(F::HighPass),
                "BP" => filter == Some(F::BandPass),
                "NOTCH" => filter == Some(F::Notch),
                "PEAK" => filter == Some(F::Peak),
                "NTCH/PK" => matches!(filter, Some(F::Notch | F::Peak)),
                "OFF" => filter == Some(F::Off),
                "SONG" => self.song_mode,
                "METRO" => self.transport.metronome,
                "COUNT-IN" => self.transport.count_in_bars > 0,
                "PAGE 1/2" => self.synth_param_page == 1,
                "SLIDE" => self.acid_synth.params.slide_time_ms > 1.0,
                "RECORD" | "EDIT" | "SLICE" | "ZONE" => {
                    self.left_tab == 2
                        && SAMPLE_PAGES.get(self.sample_page as usize) == Some(&label)
                }
                "STOP" => self.left_tab == 2 && self.recorder.is_some(),
                "SEUIL" => self.left_tab == 2 && self.record_wait,
                "4" | "8" | "16" if self.left_tab == 2 => {
                    label.parse::<u8>().ok() == Some(self.slice_count)
                }
                "OSC 2" => self
                    .focus_sound_wavetable_params()
                    .is_some_and(|p| p.osc2_enabled),
                "CHROMA" => self.keyboard.scale == 0,
                "MINOR" => self.keyboard.scale == 2,
                _ => false,
            }
        })
    }

    /// Instrument affiché en mode PLUGIN (onglet 4 hors ampli guitare).
    #[must_use]
    pub fn plugin_instrument(&self) -> Option<crate::plugin_pages::Instrument> {
        crate::plugin_pages::instrument_for(
            self.left_tab,
            self.plugin_page,
            self.focus_sound_has_kick(),
            self.focus_sound_has_wavetable(),
            self.focus_sound_has_acid(),
        )
    }

    /// Page de paramètres affichée, bornée au nombre de pages de l'instrument.
    #[must_use]
    pub fn plugin_page_index(&self) -> usize {
        if self.current_plugin_slot() != PluginSlot::Instrument {
            return 0;
        }
        self.plugin_instrument().map_or(0, |i| {
            (self.synth_param_page as usize).min(i.page_count() - 1)
        })
    }

    /// Nombre de pages de l'emplacement affiché (instrument ou effet).
    #[must_use]
    pub fn plugin_page_count(&self) -> usize {
        match self.current_plugin_slot() {
            PluginSlot::Instrument => self.plugin_instrument().map_or(1, |i| i.page_count()),
            PluginSlot::Fx(..) | PluginSlot::Add(_) => 1,
        }
    }

    /// Chaîne parcourue au 4D en mode PLUGIN : instrument du Sound, puis
    /// effets du Sound, du Group et du Master, chaque chaîne suivie d'un
    /// emplacement libre pour insérer un effet.
    fn plugin_slots(&self) -> Vec<PluginSlot> {
        let proj = Arc::clone(&*self.project.read());
        let mut slots = vec![PluginSlot::Instrument];
        for owner in [
            FxOwner::Sound(self.group_focus, self.active_sound),
            FxOwner::Group(self.group_focus),
            FxOwner::Master,
        ] {
            slots.extend((0..owner.chain(&proj).len()).map(|i| PluginSlot::Fx(owner, i)));
            slots.push(PluginSlot::Add(owner));
        }
        slots
    }

    /// Index de l'emplacement sélectionné (0 = instrument).
    #[must_use]
    pub fn plugin_slot_index(&self) -> usize {
        if self.plugin_slot_focus != (self.group_focus, self.active_sound) {
            return 0;
        }
        self.plugin_slot.min(self.plugin_slots().len() - 1)
    }

    /// Emplacement sélectionné.
    #[must_use]
    pub fn current_plugin_slot(&self) -> PluginSlot {
        self.plugin_slots()[self.plugin_slot_index()]
    }

    fn select_plugin_slot(&mut self, index: usize) {
        if self.plugin_instrument().is_none() {
            return;
        }
        let index = index.min(self.plugin_slots().len() - 1);
        if index == self.plugin_slot_index() {
            return;
        }
        self.plugin_slot = index;
        self.plugin_slot_focus = (self.group_focus, self.active_sound);
        self.plugin_knob_accum = [0; 8];
        self.page_changed_at = Some(std::time::Instant::now());
        self.revision = self.revision.wrapping_add(1);
    }

    /// Chaîne affichée (écrans, GUI).
    fn plugin_slot_views(&self) -> Vec<PluginSlotView> {
        let proj = Arc::clone(&*self.project.read());
        let instrument = crate::plugin_pages::instrument_for(
            4,
            0,
            self.focus_sound_has_kick(),
            self.focus_sound_has_wavetable(),
            self.focus_sound_has_acid(),
        )
        .map_or("Sampler", |i| i.label());
        self.plugin_slots()
            .into_iter()
            .map(|slot| match slot {
                PluginSlot::Instrument => PluginSlotView {
                    level: 0,
                    name: instrument.to_string(),
                    bypassed: false,
                    empty: false,
                },
                PluginSlot::Fx(owner, i) => {
                    let fx = &owner.chain(&proj)[i];
                    PluginSlotView {
                        level: owner.level(),
                        name: fx.fx_type.label().to_string(),
                        bypassed: !fx.enabled,
                        empty: false,
                    }
                }
                PluginSlot::Add(owner) => PluginSlotView {
                    level: owner.level(),
                    name: "+ effet".to_string(),
                    bypassed: false,
                    empty: true,
                },
            })
            .collect()
    }

    /// Effet sélectionné en mode PLUGIN : propriétaire, index et réglages.
    #[must_use]
    pub fn selected_fx(&self) -> Option<(FxOwner, usize, crate::fx::FxSlot)> {
        self.plugin_instrument()?;
        let PluginSlot::Fx(owner, i) = self.current_plugin_slot() else {
            return None;
        };
        let proj = Arc::clone(&*self.project.read());
        owner
            .chain(&proj)
            .get(i)
            .cloned()
            .map(|slot| (owner, i, slot))
    }

    /// Modifie un effet (knobs, boutons d'écran).
    fn edit_fx(&mut self, owner: FxOwner, index: usize, f: impl FnOnce(&mut crate::fx::FxSlot)) {
        let mut guard = self.project.write();
        if let Some(slot) = owner
            .chain_mut(project_mut(&mut guard), false)
            .and_then(|c| c.get_mut(index))
        {
            f(slot);
        }
        drop(guard);
        self.revision = self.revision.wrapping_add(1);
    }

    /// Insère un effet en fin de chaîne et le sélectionne en mode PLUGIN.
    fn insert_fx(&mut self, owner: FxOwner, fx_type: crate::fx::FxType) {
        self.push_project_history();
        let mut guard = self.project.write();
        let len = owner.chain_mut(project_mut(&mut guard), true).map(|chain| {
            chain.push(crate::fx::FxSlot::default_for(fx_type));
            chain.len()
        });
        drop(guard);
        let Some(len) = len else { return };
        let target = match owner {
            FxOwner::Sound(_, s) => format!("Sound {}", s + 1),
            FxOwner::Group(g) => format!("Groupe {}", (b'A' + g % 8) as char),
            FxOwner::Master => "Master".to_string(),
        };
        self.show_status(format!("{} ajouté — {target}", fx_type.label()));
        if let Some(pos) = self
            .plugin_slots()
            .iter()
            .position(|s| *s == PluginSlot::Fx(owner, len - 1))
        {
            if self.plugin_instrument().is_some() {
                self.plugin_slot = pos;
                self.plugin_slot_focus = (self.group_focus, self.active_sound);
                self.page_changed_at = Some(std::time::Instant::now());
            }
        }
        self.revision = self.revision.wrapping_add(1);
    }

    fn remove_fx(&mut self, owner: FxOwner, index: usize) {
        self.push_project_history();
        let mut guard = self.project.write();
        let removed = owner
            .chain_mut(project_mut(&mut guard), false)
            .filter(|c| index < c.len())
            .map(|c| c.remove(index));
        drop(guard);
        if let Some(fx) = removed {
            self.show_status(format!("{} supprimé", fx.fx_type.label()));
        }
        self.revision = self.revision.wrapping_add(1);
    }

    fn show_status(&mut self, message: String) {
        self.status_message = Some(message);
        self.status_expires_at = std::time::Instant::now() + std::time::Duration::from_millis(2500);
    }

    /// Boutons d'écran d'un effet : type, bypass, suppression puis les
    /// raccourcis de sa page ; emplacement libre : les effets à insérer.
    fn fx_soft_labels(&self) -> Option<[&'static str; 8]> {
        match self.current_plugin_slot() {
            PluginSlot::Instrument => None,
            // Les huit effets, un par bouton, dans l'ordre de la chaîne.
            PluginSlot::Add(_) => Some(crate::fx::FxType::ALL.map(crate::fx::FxType::name)),
            PluginSlot::Fx(..) => {
                let (_, _, fx) = self.selected_fx()?;
                let b = crate::plugin_pages::fx_pages(fx.fx_type)[0].buttons;
                Some([
                    "< TYPE", "TYPE >", "BYPASS", "SUPPR", b[0], b[1], b[2], b[3],
                ])
            }
        }
    }

    /// Temps de delay (ms) d'une division de noire au tempo courant.
    fn synced_delay_ms(&self, label: &str) -> Option<f32> {
        let beat = 60_000.0 / self.transport.bpm.max(20.0);
        match label {
            "1/16" => Some(beat / 4.0),
            "1/8" => Some(beat / 2.0),
            "1/4" => Some(beat),
            _ => None,
        }
    }

    /// Bouton d'écran allumé pour l'effet sélectionné.
    fn fx_soft_key_lit(&self, fx: &crate::fx::FxSlot, label: &str) -> bool {
        use crate::fx::FxType as T;
        match (fx.fx_type, label) {
            (_, "BYPASS") => !fx.enabled,
            (T::Filter, "LP") => fx.p3.round() as i32 == 0,
            (T::Filter, "HP") => fx.p3.round() as i32 == 1,
            (T::Filter, "BP") => fx.p3.round() as i32 == 2,
            (T::Distortion, "SOFT") => fx.p3 <= 0.5,
            (T::Distortion, "HARD") => fx.p3 > 0.5,
            (T::Delay, "PING-PONG") => fx.p4 > 0.5,
            (T::Delay, l) => self
                .synced_delay_ms(l)
                .is_some_and(|ms| (fx.p1 - ms).abs() < 0.5),
            _ => false,
        }
    }

    fn handle_fx_soft_key(&mut self, label: &str) -> crate::Result<()> {
        use crate::fx::{FxSlot, FxType as T};
        match self.current_plugin_slot() {
            PluginSlot::Instrument => {}
            PluginSlot::Add(owner) => {
                if let Some(t) = T::ALL.into_iter().find(|t| t.name() == label) {
                    self.insert_fx(owner, t);
                }
            }
            PluginSlot::Fx(owner, i) => {
                let Some((_, _, fx)) = self.selected_fx() else {
                    return Ok(());
                };
                let synced = self.synced_delay_ms(label);
                match label {
                    "SUPPR" => self.remove_fx(owner, i),
                    "BYPASS" => self.edit_fx(owner, i, |s| s.enabled = !s.enabled),
                    "< TYPE" | "TYPE >" => {
                        let all = T::ALL;
                        let cur = all.iter().position(|t| *t == fx.fx_type).unwrap_or(0) as i32;
                        let dir = if label == "< TYPE" { -1 } else { 1 };
                        let next = all[(cur + dir).rem_euclid(all.len() as i32) as usize];
                        self.edit_fx(owner, i, |s| {
                            *s = FxSlot {
                                enabled: s.enabled,
                                ..FxSlot::default_for(next)
                            }
                        });
                    }
                    _ => self.edit_fx(owner, i, |s| match (s.fx_type, label) {
                        (T::Filter, "LP") => s.p3 = 0.0,
                        (T::Filter, "HP") => s.p3 = 1.0,
                        (T::Filter, "BP") => s.p3 = 2.0,
                        (T::Distortion, "SOFT") => s.p3 = 0.0,
                        (T::Distortion, "HARD") => s.p3 = 1.0,
                        (T::Compressor, "DOUX") => {
                            (s.p1, s.p2, s.p3, s.p4) = (-12.0, 2.0, 20.0, 250.0)
                        }
                        (T::Compressor, "PUNCH") => {
                            (s.p1, s.p2, s.p3, s.p4) = (-18.0, 4.0, 12.0, 90.0)
                        }
                        (T::Compressor, "SQUASH") => {
                            (s.p1, s.p2, s.p3, s.p4) = (-32.0, 12.0, 0.5, 60.0)
                        }
                        (T::Delay, "PING-PONG") => s.p4 = if s.p4 > 0.5 { 0.0 } else { 1.0 },
                        (T::Delay, _) if synced.is_some() => s.p1 = synced.unwrap_or(s.p1),
                        (T::Reverb, "PIÈCE") => (s.p1, s.p2, s.p3, s.p4) = (0.35, 0.55, 4.0, 0.7),
                        (T::Reverb, "SALLE") => (s.p1, s.p2, s.p3, s.p4) = (0.65, 0.4, 14.0, 0.9),
                        (T::Reverb, "HALL") => (s.p1, s.p2, s.p3, s.p4) = (0.88, 0.25, 30.0, 1.0),
                        (T::Eq, "FLAT") => (s.p1, s.p2, s.p3, s.p4) = (0.0, 0.0, 1000.0, 0.0),
                        (T::Eq, "LOUDNESS") => (s.p1, s.p2, s.p3, s.p4) = (5.0, -2.5, 800.0, 4.0),
                        (T::Eq, "RADIO") => (s.p1, s.p2, s.p3, s.p4) = (-18.0, 7.0, 1500.0, -18.0),
                        (T::Chorus, "CHORUS") => {
                            (s.mix, s.p1, s.p2, s.p3, s.p4) = (0.5, 0.8, 0.5, 12.0, 0.0)
                        }
                        (T::Chorus, "FLANGER") => {
                            (s.mix, s.p1, s.p2, s.p3, s.p4) = (0.5, 0.25, 0.7, 2.0, 0.6)
                        }
                        (T::Chorus, "ENSEMBLE") => {
                            (s.mix, s.p1, s.p2, s.p3, s.p4) = (0.45, 1.6, 0.35, 18.0, 0.0)
                        }
                        (T::LoFi, "8-BIT") => (s.p1, s.p2, s.p3) = (8.0, 11_025.0, 6000.0),
                        (T::LoFi, "SP-1200") => (s.p1, s.p2, s.p3) = (12.0, 26_040.0, 12_000.0),
                        (T::LoFi, "CRUSH") => (s.p1, s.p2, s.p3) = (4.0, 4000.0, 3000.0),
                        _ => {}
                    }),
                }
            }
        }
        Ok(())
    }

    fn set_plugin_page(&mut self, page: usize) {
        let count = self.plugin_page_count();
        self.synth_param_page = page.min(count - 1) as u8;
        self.plugin_knob_accum = [0; 8];
        self.page_changed_at = Some(std::time::Instant::now());
        self.revision = self.revision.wrapping_add(1);
    }

    fn focus_sound(&self) -> Option<crate::project::Sound> {
        let proj = Arc::clone(&*self.project.read());
        proj.groups
            .get(self.group_focus as usize)?
            .sounds
            .get(self.active_sound as usize)?
            .clone()
    }

    /// Knob `knob` en mode PLUGIN : règle le paramètre affiché au-dessus.
    fn turn_plugin_knob(&mut self, knob: usize, delta: i32) -> crate::Result<()> {
        use crate::plugin_pages::{self as pp, Instrument};
        let Some(inst) = self.plugin_instrument() else {
            return Ok(());
        };
        let page = self.plugin_page_index();
        let knob = knob.min(7);
        if let Some((owner, i, mut fx)) = self.selected_fx() {
            let pages = pp::fx_pages(fx.fx_type);
            let mut steps = 0;
            if pp::is_stepped(pages, 0, knob) {
                let acc = self.plugin_knob_accum[knob] + delta;
                steps = acc / pp::DETENT_TICKS;
                self.plugin_knob_accum[knob] = acc - steps * pp::DETENT_TICKS;
                if steps == 0 {
                    return Ok(());
                }
            }
            if pp::turn(pages, 0, knob, &mut fx, delta, steps, self.shift_held) {
                self.edit_fx(owner, i, |s| *s = fx);
            }
            return Ok(());
        }
        let stepped = match inst {
            Instrument::Sampler => pp::is_stepped(&pp::SAMPLER_PAGES, page, knob),
            Instrument::Acid => pp::is_stepped(&pp::ACID_PAGES, page, knob),
            Instrument::Kick => pp::is_stepped(&pp::KICK_PAGES, page, knob),
            Instrument::Wavetable => pp::is_stepped(&pp::WAVETABLE_PAGES, page, knob),
        };
        // Entiers et choix : un pas par cran, les ticks intermédiaires s'accumulent.
        let mut steps = 0;
        if stepped {
            let acc = self.plugin_knob_accum[knob] + delta;
            steps = acc / pp::DETENT_TICKS;
            self.plugin_knob_accum[knob] = acc - steps * pp::DETENT_TICKS;
            if steps == 0 {
                return Ok(());
            }
        }
        let fine = self.shift_held;
        match inst {
            Instrument::Kick => {
                let mut p = self.focus_sound_kick_params().unwrap_or_default();
                if pp::turn(&pp::KICK_PAGES, page, knob, &mut p, delta, steps, fine) {
                    if let Some(v) = pp::value(&pp::KICK_PAGES, page, knob, &p) {
                        self.record_sound_automation(
                            self.group_focus,
                            self.active_sound,
                            knob as u8,
                            v,
                        );
                    }
                    self.handle_event(EngineEvent::KickSetParams(p))?;
                }
            }
            Instrument::Wavetable => {
                let mut p = self.focus_sound_wavetable_params().unwrap_or_default();
                if pp::turn(&pp::WAVETABLE_PAGES, page, knob, &mut p, delta, steps, fine) {
                    if let Some(v) = pp::value(&pp::WAVETABLE_PAGES, page, knob, &p) {
                        self.record_sound_automation(
                            self.group_focus,
                            self.active_sound,
                            knob as u8,
                            v,
                        );
                    }
                    self.handle_event(EngineEvent::WavetableSetParams(p))?;
                }
            }
            Instrument::Acid => {
                let mut p = self.acid_synth.params.clone();
                if pp::turn(&pp::ACID_PAGES, page, knob, &mut p, delta, steps, fine) {
                    if let Some(v) = pp::value(&pp::ACID_PAGES, page, knob, &p) {
                        self.record_sound_automation(
                            self.group_focus,
                            self.active_sound,
                            knob as u8,
                            v,
                        );
                    }
                    self.handle_event(EngineEvent::AcidSetParams(p))?;
                }
            }
            Instrument::Sampler => {
                let Some(before) = self.focus_sound() else {
                    return Ok(());
                };
                let mut after = before.clone();
                if pp::turn(
                    &pp::SAMPLER_PAGES,
                    page,
                    knob,
                    &mut after,
                    delta,
                    steps,
                    fine,
                ) {
                    if let Some(v) = pp::value(&pp::SAMPLER_PAGES, page, knob, &after) {
                        self.record_sound_automation(
                            self.group_focus,
                            self.active_sound,
                            knob as u8,
                            v,
                        );
                    }
                    self.apply_sound_edit(&before, &after)?;
                }
            }
        }
        Ok(())
    }

    /// Envoie les événements correspondant aux réglages modifiés d'un Sound
    /// (ils atteignent aussi les voix en cours de lecture).
    fn apply_sound_edit(
        &mut self,
        a: &crate::project::Sound,
        b: &crate::project::Sound,
    ) -> crate::Result<()> {
        let (group, sound) = (self.group_focus, self.active_sound);
        let moved = |x: f32, y: f32| x.to_bits() != y.to_bits();
        if moved(a.volume_db, b.volume_db) {
            self.handle_event(EngineEvent::SoundVolume {
                group,
                sound,
                volume_db: b.volume_db,
            })?;
        }
        if moved(a.pan, b.pan) {
            self.handle_event(EngineEvent::SoundPan {
                group,
                sound,
                pan: b.pan,
            })?;
        }
        if moved(a.filter.cutoff_hz, b.filter.cutoff_hz) {
            self.handle_event(EngineEvent::SoundFilterCutoff {
                group,
                sound,
                cutoff_hz: b.filter.cutoff_hz,
            })?;
        }
        if moved(a.filter.resonance, b.filter.resonance) {
            self.handle_event(EngineEvent::SoundFilterReso {
                group,
                sound,
                resonance: b.filter.resonance,
            })?;
        }
        if a.filter.kind != b.filter.kind {
            self.handle_event(EngineEvent::SetSoundFilterKind {
                group,
                sound,
                kind: b.filter.kind,
            })?;
        }
        let (x, y) = (&a.adsr, &b.adsr);
        if moved(x.attack_ms, y.attack_ms)
            || moved(x.decay_ms, y.decay_ms)
            || moved(x.sustain, y.sustain)
            || moved(x.release_ms, y.release_ms)
        {
            self.handle_event(EngineEvent::SoundAdsr {
                group,
                sound,
                attack_ms: y.attack_ms,
                decay_ms: y.decay_ms,
                sustain: y.sustain,
                release_ms: y.release_ms,
            })?;
        }
        if a.polyphony != b.polyphony {
            self.handle_event(EngineEvent::SoundPolyphony {
                group,
                sound,
                polyphony: b.polyphony,
            })?;
        }
        if a.choke_group != b.choke_group {
            self.handle_event(EngineEvent::SoundChoke {
                group,
                sound,
                choke_group: b.choke_group,
            })?;
        }
        Ok(())
    }

    /// Boutons d'écran du mode PLUGIN : 1–4 pour l'instrument (presets,
    /// écoute, browser), 5–8 propres à la page affichée.
    fn plugin_soft_labels(&self, inst: crate::plugin_pages::Instrument) -> [&'static str; 8] {
        use crate::plugin_pages::{self as pp, Instrument};
        if let Some(labels) = self.fx_soft_labels() {
            return labels;
        }
        let page = self.plugin_page_index();
        let right = match inst {
            Instrument::Sampler => pp::SAMPLER_PAGES[page].buttons,
            Instrument::Acid => pp::ACID_PAGES[page].buttons,
            Instrument::Kick => pp::KICK_PAGES[page].buttons,
            Instrument::Wavetable => pp::WAVETABLE_PAGES[page].buttons,
        };
        let left = if inst == Instrument::Sampler {
            ["SND <", "SND >", "ECOUTE", "BROWSER"]
        } else {
            ["PRESET <", "PRESET >", "ECOUTE", "BROWSER"]
        };
        [
            left[0], left[1], left[2], left[3], right[0], right[1], right[2], right[3],
        ]
    }

    /// Action d'un bouton d'écran en mode PLUGIN, choisie d'après le libellé
    /// affiché : le bouton fait toujours ce que son texte annonce.
    fn handle_plugin_soft_key(
        &mut self,
        inst: crate::plugin_pages::Instrument,
        idx: usize,
    ) -> crate::Result<()> {
        use crate::kick_synth::KickFilterMode as K;
        use crate::plugin_pages::Instrument as I;
        use crate::project::FilterKind as F;
        let label = self.plugin_soft_labels(inst)[idx.min(7)];
        if self.current_plugin_slot() != PluginSlot::Instrument {
            return self.handle_fx_soft_key(label);
        }
        let (group, sound) = (self.group_focus, self.active_sound);
        let sound_filter = |kind| EngineEvent::SetSoundFilterKind { group, sound, kind };
        match (inst, label) {
            (_, "SND <") => self.handle_event(EngineEvent::SelectSound(sound.saturating_sub(1))),
            (_, "SND >") => self.handle_event(EngineEvent::SelectSound((sound + 1).min(15))),
            (_, "ECOUTE") => {
                self.handle_event(EngineEvent::Pad {
                    pad: sound,
                    velocity: 110,
                    pressed: true,
                })?;
                // Synthés : relâcher aussitôt, sinon la note resterait tenue.
                if inst == I::Sampler {
                    return Ok(());
                }
                self.handle_event(EngineEvent::Pad {
                    pad: sound,
                    velocity: 0,
                    pressed: false,
                })
            }
            (_, "BROWSER") => {
                self.left_tab = 3;
                self.browser_kind = 2;
                self.revision = self.revision.wrapping_add(1);
                Ok(())
            }
            (_, "PRESET <") => self.step_plugin_preset(inst, -1),
            (_, "PRESET >") => self.step_plugin_preset(inst, 1),
            (I::Sampler, "LP/HP/BP") => self.handle_soft_key_sampler(4),
            (I::Sampler, "CYC FLT") => self.handle_soft_key_sampler(3),
            (I::Sampler, "NOTCH") => self.handle_event(sound_filter(F::Notch)),
            (I::Sampler, "PEAK") => self.handle_event(sound_filter(F::Peak)),
            (I::Sampler, "OFF") => self.handle_event(sound_filter(F::Off)),
            (I::Sampler, "REVERSE") => self.sample_tool(0),
            (I::Sampler, "LOOP") => self.sample_tool(1),
            (I::Sampler, "NORMLZ") => self.sample_tool(2),
            (I::Kick, "TECHNO") => self.handle_event(EngineEvent::KickSetPreset(0)),
            (I::Kick, "909") => self.handle_event(EngineEvent::KickSetPreset(1)),
            (I::Kick, "HARDCORE") => self.handle_event(EngineEvent::KickSetPreset(2)),
            (I::Kick, "808 SUB") => self.handle_event(EngineEvent::KickSetPreset(3)),
            (I::Kick, "INDUSTRIAL") => self.handle_event(EngineEvent::KickSetPreset(5)),
            (I::Kick, "LP") => self.handle_event(EngineEvent::KickSetFilterMode(K::LowPass)),
            (I::Kick, "HP") => self.handle_event(EngineEvent::KickSetFilterMode(K::HighPass)),
            (I::Kick, "BP") => self.handle_event(EngineEvent::KickSetFilterMode(K::BandPass)),
            (I::Kick, "OFF") => self.handle_event(EngineEvent::KickSetFilterMode(K::Off)),
            (I::Kick, "NOTCH") => self.handle_event(EngineEvent::KickSetFilterMode(K::Notch)),
            (I::Kick, "PEAK") => self.handle_event(EngineEvent::KickSetFilterMode(K::Peak)),
            (I::Acid, "CLASSIC") => self.handle_event(EngineEvent::AcidSelectPreset(0)),
            (I::Acid, "SQUELCH") => self.handle_event(EngineEvent::AcidSelectPreset(1)),
            (I::Acid, "DUB") => self.handle_event(EngineEvent::AcidSelectPreset(3)),
            (I::Acid, "SAW/SQR") => self.handle_event(EngineEvent::AcidWaveformToggle),
            (I::Acid, "OCT -") => self.handle_event(EngineEvent::AcidOctaveDelta(-1)),
            (I::Acid, "OCT +") => self.handle_event(EngineEvent::AcidOctaveDelta(1)),
            (I::Acid, "SLIDE") => {
                let slide = if self.acid_synth.params.slide_time_ms > 1.0 {
                    -250.0
                } else {
                    60.0
                };
                self.handle_event(EngineEvent::AcidSlideDelta(slide))
            }
            (I::Wavetable, "PIANO") => self.handle_event(EngineEvent::WavetableSetPreset(0)),
            (I::Wavetable, "M1 PIANO") => self.handle_event(EngineEvent::WavetableSetPreset(1)),
            (I::Wavetable, "808 SUB") => self.handle_event(EngineEvent::WavetableSetPreset(4)),
            (I::Wavetable, "REESE") => self.handle_event(EngineEvent::WavetableSetPreset(5)),
            (I::Wavetable, "OSC 2") => {
                let mut p = self.focus_sound_wavetable_params().unwrap_or_default();
                p.osc2_enabled = !p.osc2_enabled;
                self.handle_event(EngineEvent::WavetableSetParams(p))
            }
            _ => Ok(()),
        }
    }

    /// Preset précédent / suivant de l'instrument affiché.
    fn step_plugin_preset(
        &mut self,
        inst: crate::plugin_pages::Instrument,
        dir: i32,
    ) -> crate::Result<()> {
        use crate::plugin_pages::Instrument as I;
        match inst {
            I::Kick => {
                let presets = crate::kick_synth::KickParams::all_presets();
                let name = self.focus_sound_kick_params().map(|p| p.preset_name);
                let cur = presets
                    .iter()
                    .position(|p| Some(&p.preset_name) == name.as_ref())
                    .unwrap_or(0) as i32;
                let next = (cur + dir).rem_euclid(presets.len() as i32);
                self.handle_event(EngineEvent::KickSetPreset(next as u8))
            }
            I::Wavetable if dir < 0 => self.handle_event(EngineEvent::WavetablePrevPreset),
            I::Wavetable => self.handle_event(EngineEvent::WavetableNextPreset),
            I::Acid => {
                let next = (i32::from(self.acid_preset) + dir).rem_euclid(4) as u8;
                self.handle_event(EngineEvent::AcidSelectPreset(next))
            }
            I::Sampler => Ok(()),
        }
    }

    /// Outils du sample du Sound actif : 0 = REVERSE, 1 = LOOP, 2 = NORMLZ.
    fn sample_tool(&mut self, tool: usize) -> crate::Result<()> {
        let sound_id = sampler_sound_id(self.group_focus, self.active_sound);
        let (ok, done, missing) = match tool {
            0 => (
                self.sampler.reverse_sound(sound_id),
                "Sample inversé (Reverse)",
                "Aucun sample chargé à inverser",
            ),
            1 => (
                self.sampler.toggle_sound_one_shot(sound_id),
                "Mode One-Shot / Boucle basculé",
                "Aucun sample chargé",
            ),
            _ => (
                self.sampler.normalize_sound(sound_id),
                "Sample normalisé à 0 dBFS",
                "Aucun sample chargé à normaliser",
            ),
        };
        self.status_message = Some(if ok { done } else { missing }.to_string());
        if ok {
            // Gardé dans le projet : rejoué au prochain chargement.
            let (g, s) = (self.group_focus as usize, self.active_sound as usize);
            let mut guard = self.project.write();
            if let Some(edits) = project_mut(&mut guard)
                .groups
                .get_mut(g)
                .and_then(|grp| grp.sounds.get_mut(s))
                .and_then(Option::as_mut)
                .map(|snd| &mut snd.edits)
            {
                match tool {
                    0 => edits.reversed = !edits.reversed,
                    1 => edits.looped = !edits.looped,
                    _ => edits.normalized = true,
                }
            }
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    /// Dossier des enregistrements (fixé par l'application au démarrage).
    pub fn set_recordings_dir(&mut self, dir: std::path::PathBuf) {
        self.recordings_dir = dir;
    }

    /// Un enregistrement attend-il l'entrée audio ? (callback audio)
    #[must_use]
    pub fn is_recording(&self) -> bool {
        self.recorder.is_some()
    }

    /// Entrée audio (mono) pendant l'enregistrement — appelée par le callback
    /// audio : aucune allocation, la capacité est réservée au départ.
    pub fn push_record_input(&mut self, input: &[f32]) {
        let threshold = db_to_linear(self.record_threshold_db);
        let Some(rec) = self.recorder.as_mut() else {
            return;
        };
        let peak = input.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        rec.level = peak.max(rec.level * 0.85);
        let mut from = 0;
        if rec.waiting {
            match input.iter().position(|s| s.abs() >= threshold) {
                Some(i) => {
                    rec.waiting = false;
                    from = i;
                }
                None => return,
            }
        }
        let room = rec.buf.capacity() - rec.buf.len();
        let take = (input.len() - from).min(room);
        rec.buf.extend_from_slice(&input[from..from + take]);
    }

    fn focus_sound_id(&self) -> crate::sampler::SoundId {
        sampler_sound_id(self.group_focus, self.active_sound)
    }

    /// Plage de lecture du Sound actif.
    fn focus_range(&self) -> (f32, f32) {
        self.sampler.sound_range(self.focus_sound_id())
    }

    fn sample_soft_labels(&self) -> [&'static str; 8] {
        let page: [&'static str; 4] = match self.sample_page {
            0 => [
                if self.recorder.is_some() {
                    "STOP"
                } else {
                    "ENREG"
                },
                "SEUIL",
                "",
                "",
            ],
            2 => ["4", "8", "16", "APPLIQUER"],
            3 => ["", "", "", "RÉINIT"],
            _ => ["REVERSE", "LOOP", "NORMLZ", "PLAGE MAX"],
        };
        [
            SAMPLE_PAGES[0],
            SAMPLE_PAGES[1],
            SAMPLE_PAGES[2],
            SAMPLE_PAGES[3],
            page[0],
            page[1],
            page[2],
            page[3],
        ]
    }

    fn handle_sample_soft_key(&mut self, idx: usize) -> crate::Result<()> {
        let label = self.sample_soft_labels()[idx.min(7)];
        if let Some(page) = SAMPLE_PAGES.iter().position(|p| *p == label) {
            self.sample_page = page as u8;
            self.sample_knob_accum = [0; 8];
            self.page_changed_at = Some(std::time::Instant::now());
            self.revision = self.revision.wrapping_add(1);
            return Ok(());
        }
        match label {
            "ENREG" => self.start_recording(),
            "STOP" => self.finish_recording(),
            "SEUIL" => self.record_wait = !self.record_wait,
            "4" | "8" | "16" => self.slice_count = label.parse().unwrap_or(8),
            "APPLIQUER" => self.apply_slices(),
            "REVERSE" => return self.sample_tool(0),
            "LOOP" => return self.sample_tool(1),
            "NORMLZ" => return self.sample_tool(2),
            "PLAGE MAX" => self.set_focus_range(0.0, 1.0),
            "RÉINIT" => self.edit_focus_zones(|z| {
                z.root_key = 60;
                z.lokey = 0;
                z.hikey = 127;
                z.lovel = 0;
                z.hivel = 127;
                z.tune_cents = 0;
                z.pan = 0.0;
                z.volume_db = 0.0;
            }),
            _ => {}
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    /// Plage de lecture du Sound actif : sampler et projet.
    fn set_focus_range(&mut self, start: f32, end: f32) {
        let id = self.focus_sound_id();
        self.sampler.set_sound_range(id, start, end);
        let (start, end) = self.sampler.sound_range(id);
        let (g, s) = (self.group_focus as usize, self.active_sound as usize);
        let mut guard = self.project.write();
        if let Some(snd) = project_mut(&mut guard)
            .groups
            .get_mut(g)
            .and_then(|grp| grp.sounds.get_mut(s))
            .and_then(Option::as_mut)
        {
            for z in &mut snd.zones {
                z.start = start;
                z.end = end;
            }
        }
        drop(guard);
        self.revision = self.revision.wrapping_add(1);
    }

    /// Modifie les zones du Sound actif (sampler et projet ensemble).
    fn edit_focus_zones(&mut self, f: impl Fn(&mut ZoneFields)) {
        let id = self.focus_sound_id();
        self.sampler.edit_zones(id, |z| {
            let mut v = ZoneFields::of_mapping(z);
            f(&mut v);
            v.write_mapping(z);
        });
        let (g, s) = (self.group_focus as usize, self.active_sound as usize);
        let mut guard = self.project.write();
        if let Some(snd) = project_mut(&mut guard)
            .groups
            .get_mut(g)
            .and_then(|grp| grp.sounds.get_mut(s))
            .and_then(Option::as_mut)
        {
            for z in &mut snd.zones {
                let mut v = ZoneFields::of_zone(z);
                f(&mut v);
                v.write_zone(z);
            }
        }
        drop(guard);
        self.revision = self.revision.wrapping_add(1);
    }

    /// Les 8 paramètres de la page de l'éditeur (nom, échelle, valeur).
    fn sample_params(&self) -> [Option<(&'static str, crate::plugin_pages::Scale, f32)>; 8] {
        use crate::plugin_pages::{Scale, Unit};
        let lin = |min, max, unit| Scale::Linear { min, max, unit };
        let log = |min, max, unit| Scale::Log { min, max, unit };
        let steps = |min, max, unit| Scale::Steps { min, max, unit };
        let zone = self
            .sampler
            .zone_mapping(self.focus_sound_id())
            .map(|z| ZoneFields::of_mapping(&z))
            .unwrap_or_default();
        let adsr = self.focus_sound().map(|s| s.adsr).unwrap_or_default();
        let (start, end) = self.focus_range();
        match self.sample_page {
            0 => [
                Some(("SEUIL", lin(-60.0, 0.0, Unit::Db), self.record_threshold_db)),
                Some((
                    "DURÉE MAX",
                    log(2.0, 120.0, Unit::Seconds),
                    self.record_max_secs,
                )),
                None,
                None,
                None,
                None,
                None,
                None,
            ],
            2 => [
                Some((
                    "TRANCHES",
                    steps(2.0, 16.0, Unit::Count),
                    f32::from(self.slice_count),
                )),
                Some((
                    "MODE",
                    Scale::Choice(&["GRILLE", "TRANSIT."]),
                    if self.slice_transients { 1.0 } else { 0.0 },
                )),
                Some((
                    "SENSIB",
                    lin(0.0, 1.0, Unit::Percent),
                    self.slice_sensitivity,
                )),
                None,
                None,
                None,
                None,
                None,
            ],
            3 => [
                Some((
                    "ROOT",
                    steps(0.0, 127.0, Unit::Note),
                    f32::from(zone.root_key),
                )),
                Some((
                    "LOW KEY",
                    steps(0.0, 127.0, Unit::Note),
                    f32::from(zone.lokey),
                )),
                Some((
                    "HIGH KEY",
                    steps(0.0, 127.0, Unit::Note),
                    f32::from(zone.hikey),
                )),
                Some((
                    "FINE",
                    steps(-50.0, 50.0, Unit::Cents),
                    f32::from(zone.tune_cents % 100),
                )),
                Some((
                    "LOW VEL",
                    steps(0.0, 127.0, Unit::Count),
                    f32::from(zone.lovel),
                )),
                Some((
                    "HIGH VEL",
                    steps(0.0, 127.0, Unit::Count),
                    f32::from(zone.hivel),
                )),
                Some(("PAN", lin(-1.0, 1.0, Unit::Pan), zone.pan)),
                Some(("GAIN", lin(-24.0, 12.0, Unit::Db), zone.volume_db)),
            ],
            _ => [
                Some(("START", lin(0.0, 1.0, Unit::Percent), start)),
                Some(("END", lin(0.0, 1.0, Unit::Percent), end)),
                Some((
                    "TUNE",
                    steps(-24.0, 24.0, Unit::Semitones),
                    (f32::from(zone.tune_cents) / 100.0).round(),
                )),
                Some(("GAIN", lin(-24.0, 12.0, Unit::Db), zone.volume_db)),
                Some(("ATTACK", log(0.5, 2000.0, Unit::Ms), adsr.attack_ms)),
                Some(("DECAY", log(5.0, 2000.0, Unit::Ms), adsr.decay_ms)),
                Some(("SUSTAIN", lin(0.0, 1.0, Unit::Percent), adsr.sustain)),
                Some(("RELEASE", log(5.0, 3000.0, Unit::Ms), adsr.release_ms)),
            ],
        }
    }

    fn sample_param_views(&self) -> [Option<crate::plugin_pages::ParamView>; 8] {
        self.sample_params().map(|p| {
            p.map(|(name, scale, v)| crate::plugin_pages::ParamView {
                name,
                value: scale.format(v),
                norm: scale.norm(v),
                raw: v,
            })
        })
    }

    /// Knob de l'éditeur de sample : même échelle que l'affichage.
    fn turn_sample_knob(&mut self, knob: usize, delta: i32) {
        use crate::plugin_pages::DETENT_TICKS;
        let knob = knob.min(7);
        let Some((_, scale, value)) = self.sample_params()[knob] else {
            return;
        };
        let next = if scale.is_stepped() {
            let acc = self.sample_knob_accum[knob] + delta;
            let steps = acc / DETENT_TICKS;
            self.sample_knob_accum[knob] = acc - steps * DETENT_TICKS;
            if steps == 0 {
                return;
            }
            scale.step(value, steps)
        } else {
            scale.turn(value, delta, self.shift_held)
        };
        self.set_sample_param(knob, next);
        self.revision = self.revision.wrapping_add(1);
    }

    fn set_sample_param(&mut self, knob: usize, v: f32) {
        let (start, end) = self.focus_range();
        match (self.sample_page, knob) {
            (0, 0) => self.record_threshold_db = v,
            (0, 1) => self.record_max_secs = v,
            (2, 0) => self.slice_count = v.round() as u8,
            (2, 1) => self.slice_transients = v.round() >= 1.0,
            (2, 2) => self.slice_sensitivity = v,
            (3, 0) => self.edit_focus_zones(|z| z.root_key = v.round() as u8),
            (3, 1) => self.edit_focus_zones(|z| z.lokey = (v.round() as u8).min(z.hikey)),
            (3, 2) => self.edit_focus_zones(|z| z.hikey = (v.round() as u8).max(z.lokey)),
            (3, 3) => self.edit_focus_zones(|z| {
                z.tune_cents = z.tune_cents / 100 * 100 + v.round() as i16;
            }),
            (3, 4) => self.edit_focus_zones(|z| z.lovel = (v.round() as u8).min(z.hivel)),
            (3, 5) => self.edit_focus_zones(|z| z.hivel = (v.round() as u8).max(z.lovel)),
            (3, 6) => self.edit_focus_zones(|z| z.pan = v),
            (3, 7) | (1, 3) => self.edit_focus_zones(|z| z.volume_db = v),
            (1, 0) => self.set_focus_range(v.min(end - 0.002), end),
            (1, 1) => self.set_focus_range(start, v.max(start + 0.002)),
            (1, 2) => self.edit_focus_zones(|z| {
                z.tune_cents = v.round() as i16 * 100 + z.tune_cents % 100;
            }),
            (1, k @ 4..=7) => {
                let Some(mut adsr) = self.focus_sound().map(|s| s.adsr) else {
                    return;
                };
                match k {
                    4 => adsr.attack_ms = v,
                    5 => adsr.decay_ms = v,
                    6 => adsr.sustain = v,
                    _ => adsr.release_ms = v,
                }
                let _ = self.handle_event(EngineEvent::SoundAdsr {
                    group: self.group_focus,
                    sound: self.active_sound,
                    attack_ms: adsr.attack_ms,
                    decay_ms: adsr.decay_ms,
                    sustain: adsr.sustain,
                    release_ms: adsr.release_ms,
                });
            }
            _ => {}
        }
    }

    /// Débuts des tranches (fractions du sample) d'après le mode de découpe :
    /// grille régulière dans la plage, ou coups détectés.
    fn slice_points(&self) -> Vec<f32> {
        let id = self.focus_sound_id();
        let (start, end) = self.focus_range();
        let key = SliceKey {
            sound: id,
            count: self.slice_count,
            transients: self.slice_transients,
            sensitivity: (self.slice_sensitivity * 1000.0) as u32,
            range: ((start * 1e6) as u32, (end * 1e6) as u32),
            sample: self
                .sampler
                .zone_mapping(id)
                .and_then(|z| z.sample.map(|s| Arc::as_ptr(&s) as usize)),
        };
        if let Some((k, pts)) = self.slice_cache.lock().as_ref() {
            if *k == key {
                return pts.clone();
            }
        }
        let n = usize::from(self.slice_count.max(1));
        let points: Vec<f32> = match (
            self.slice_transients,
            self.sampler.zone_mapping(id).and_then(|z| z.sample),
        ) {
            (true, Some(sample)) => {
                let len = sample.len().max(1);
                let (a, b) = ((start * len as f32) as usize, (end * len as f32) as usize);
                crate::sampler::detect_onsets(
                    &sample,
                    self.sampler.sample_rate,
                    a,
                    b,
                    n,
                    self.slice_sensitivity,
                )
                .into_iter()
                .map(|i| i as f32 / len as f32)
                .collect()
            }
            _ => (0..n)
                .map(|k| start + (end - start) * k as f32 / n as f32)
                .collect(),
        };
        *self.slice_cache.lock() = Some((key, points.clone()));
        points
    }

    /// Répartit les tranches sur les pads à partir du Sound actif (même
    /// sample, une plage par pad), comme « Apply to pads » de la MK3.
    fn apply_slices(&mut self) {
        let id = self.focus_sound_id();
        let Some(zone) = self.sampler.zone_mapping(id).filter(|z| z.sample.is_some()) else {
            self.show_status("Pas de sample à découper".to_string());
            return;
        };
        if zone.sample_path.is_none() {
            self.show_status("Sample sans fichier : enregistre-le d'abord".to_string());
            return;
        }
        let points = self.slice_points();
        let (_, end) = self.focus_range();
        let base = self
            .focus_sound()
            .map_or_else(|| "Slice".to_string(), |s| s.name);
        self.push_project_history();
        let group = self.group_focus;
        let first = self.active_sound;
        let mut made = 0;
        for (k, &s) in points.iter().enumerate() {
            let pad = first as usize + k;
            if pad >= 16 {
                break;
            }
            let e = points.get(k + 1).copied().unwrap_or(end);
            if self
                .install_sample_zone(group, pad as u8, zone.clone())
                .is_err()
            {
                continue;
            }
            let sid = sampler_sound_id(group, pad as u8);
            self.sampler.set_sound_range(sid, s, e);
            let mut guard = self.project.write();
            if let Some(snd) = project_mut(&mut guard).groups[group as usize].sounds[pad].as_mut() {
                snd.name = format!("{} #{}", base.split(" #").next().unwrap_or("Slice"), k + 1);
                for z in &mut snd.zones {
                    z.start = s;
                    z.end = e;
                }
            }
            made += 1;
        }
        self.show_status(format!(
            "{made} tranches sur les pads {}–{}",
            first + 1,
            first as usize + made
        ));
        self.revision = self.revision.wrapping_add(1);
    }

    fn start_recording(&mut self) {
        let frames =
            (self.record_max_secs.clamp(1.0, 120.0) * self.transport.sample_rate as f32) as usize;
        self.recorder = Some(Recorder {
            target: (self.group_focus, self.active_sound),
            buf: Vec::with_capacity(frames),
            waiting: self.record_wait,
            level: 0.0,
        });
        self.show_status(if self.record_wait {
            "Enregistrement armé : joue pour démarrer".to_string()
        } else {
            "Enregistrement…".to_string()
        });
    }

    /// Arrête l'enregistrement : écrit un WAV et le charge sur le Sound visé.
    fn finish_recording(&mut self) {
        let Some(rec) = self.recorder.take() else {
            return;
        };
        if rec.buf.is_empty() {
            self.show_status("Rien n'a été enregistré".to_string());
            return;
        }
        #[cfg(feature = "sampler")]
        {
            let (group, sound) = rec.target;
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let path = self.recordings_dir.join(format!(
                "rec-{}{:02}-{stamp}.wav",
                group_letter_char(group),
                sound + 1
            ));
            let result = std::fs::create_dir_all(&self.recordings_dir)
                .map_err(|e| e.to_string())
                .and_then(|()| write_wav_mono(&path, &rec.buf, self.transport.sample_rate))
                .and_then(|()| Sampler::prepare_wav(&path, 36 + sound).map_err(|e| e.to_string()))
                .and_then(|zone| {
                    self.install_sample_zone(group, sound, zone)
                        .map_err(|e| e.to_string())
                });
            match result {
                Ok(()) => self.show_status(format!(
                    "Enregistré : {:.1} s sur le pad {}",
                    rec.buf.len() as f32 / self.transport.sample_rate as f32,
                    sound + 1
                )),
                Err(e) => self.show_status(format!("Enregistrement perdu : {e}")),
            }
        }
        #[cfg(not(feature = "sampler"))]
        {
            let _ = rec.target;
            self.show_status("Enregistrement indisponible (sampler désactivé)".to_string());
        }
    }

    /// Index (dans le pattern actif) des événements parcourus en mode EVENTS,
    /// dans l'ordre du temps : ceux du Sound actif, ou tous (`TOUS`).
    fn event_scope(&self) -> Vec<usize> {
        let proj = Arc::clone(&*self.project.read());
        let Some(pat) = proj
            .active_pattern_index(self.group_focus as usize, self.active_scene)
            .and_then(|pi| proj.groups.get(self.group_focus as usize)?.patterns.get(pi))
        else {
            return Vec::new();
        };
        event_scope(pat, (!self.events_all).then_some(self.active_sound))
    }

    /// Événement sélectionné en mode EVENTS (index dans le pattern, copie).
    #[must_use]
    pub fn selected_event(&self) -> Option<(usize, crate::project::PatternEvent)> {
        let scope = self.event_scope();
        let i = *scope.get(self.event_cursor.min(scope.len().checked_sub(1)?))?;
        let proj = Arc::clone(&*self.project.read());
        let pat = proj
            .active_pattern_index(self.group_focus as usize, self.active_scene)
            .and_then(|pi| proj.groups.get(self.group_focus as usize)?.patterns.get(pi))?;
        pat.events.get(i).cloned().map(|e| (i, e))
    }

    fn step_event_cursor(&mut self, step: i32) {
        let n = self.event_scope().len();
        if n == 0 {
            self.event_cursor = 0;
            return;
        }
        let cur = self.event_cursor.min(n - 1) as i32;
        self.event_cursor = (cur + step).rem_euclid(n as i32) as usize;
        self.event_knob_accum = [0; 8];
        self.revision = self.revision.wrapping_add(1);
    }

    /// Modifie l'événement sélectionné puis le garde sélectionné (le tri par
    /// temps peut changer son rang quand sa position bouge).
    fn edit_selected_event(&mut self, f: impl Fn(&mut crate::project::PatternEvent, u32)) {
        if self.events_select_all {
            // Toutes les notes parcourues (le Sound, ou tous avec TOUS),
            // chacune bougée du même écart.
            let scope = self.event_scope();
            self.with_active_pattern(|pat| {
                let len = u32::from(pat.length.bars.max(1)) * 4 * 960;
                for &i in &scope {
                    if let Some(e) = pat.events.get_mut(i) {
                        f(e, len);
                    }
                }
            });
            self.revision = self.revision.wrapping_add(1);
            return;
        }
        let Some((i, _)) = self.selected_event() else {
            return;
        };
        let mut edited = None;
        self.with_active_pattern(|pat| {
            let len = u32::from(pat.length.bars.max(1)) * 4 * 960;
            if let Some(e) = pat.events.get_mut(i) {
                f(e, len);
                edited = Some(e.clone());
            }
        });
        if let Some(e) = edited {
            self.reselect_event(&e);
        }
        self.revision = self.revision.wrapping_add(1);
    }

    fn reselect_event(&mut self, e: &crate::project::PatternEvent) {
        let proj = Arc::clone(&*self.project.read());
        let Some(pat) = proj
            .active_pattern_index(self.group_focus as usize, self.active_scene)
            .and_then(|pi| proj.groups.get(self.group_focus as usize)?.patterns.get(pi))
        else {
            return;
        };
        let scope = self.event_scope();
        if let Some(pos) = scope.iter().position(|&i| {
            pat.events.get(i).is_some_and(|x| {
                x.tick == e.tick && x.pad == e.pad && x.pitch == e.pitch && x.velocity == e.velocity
            })
        }) {
            self.event_cursor = pos;
        }
    }

    /// Knobs du mode EVENTS : 1 POSITION (un pas par cran, Shift = 10 ticks),
    /// 2 NOTE (un demi-ton par cran), 3 VÉLOCITÉ, 4 LONGUEUR (un pas par cran,
    /// Shift = 30 ticks), 5 = événement précédent / suivant.
    fn turn_event_knob(&mut self, knob: usize, delta: i32) {
        use crate::plugin_pages::DETENT_TICKS;
        let knob = knob.min(7);
        if knob == 2 {
            // Vélocité : continue (≈ 1 par 8 ticks bruts).
            let acc = self.event_knob_accum[2] + delta;
            let steps = acc / 8;
            self.event_knob_accum[2] = acc - steps * 8;
            if steps != 0 {
                self.edit_selected_event(|e, _| {
                    e.velocity = (i32::from(e.velocity) + steps).clamp(1, 127) as u8;
                });
            }
            return;
        }
        let acc = self.event_knob_accum[knob] + delta;
        let steps = acc / DETENT_TICKS;
        self.event_knob_accum[knob] = acc - steps * DETENT_TICKS;
        if steps == 0 {
            return;
        }
        let fine = self.shift_held;
        match knob {
            0 => {
                let unit = if fine { 10 } else { TICKS_PER_STEP as i64 };
                self.edit_selected_event(|e, len| {
                    e.tick = (i64::from(e.tick) + i64::from(steps) * unit)
                        .clamp(0, i64::from(len) - 1) as u32;
                });
            }
            1 => self.edit_selected_event(|e, _| {
                e.pitch = (i32::from(e.pitch) + steps).clamp(0, 127) as u8;
            }),
            3 => {
                let unit = if fine { 30 } else { TICKS_PER_STEP as i64 };
                self.edit_selected_event(|e, _| {
                    e.length_ticks = (i64::from(e.length_ticks) + i64::from(steps) * unit)
                        .clamp(30, 3840 * 8) as u32;
                });
            }
            4 => self.step_event_cursor(steps),
            5 => {
                // Knob 6 : une note (curseur) ↔ toutes les notes parcourues.
                self.events_select_all = steps > 0;
                self.show_status(
                    if self.events_select_all {
                        "Sélection : toutes les notes"
                    } else {
                        "Sélection : une note"
                    }
                    .to_string(),
                );
            }
            _ => {}
        }
    }

    /// Boutons du mode EVENTS (voir `EVENT_LABELS`).
    fn handle_event_soft_key(&mut self, idx: usize) {
        match EVENT_LABELS[idx.min(7)] {
            "EVT <" => self.step_event_cursor(-1),
            "EVT >" => self.step_event_cursor(1),
            "TOUS" => {
                self.events_all = !self.events_all;
                self.event_cursor = 0;
                self.revision = self.revision.wrapping_add(1);
            }
            "SUPPR" if self.events_select_all => {
                let mut scope = self.event_scope();
                if !scope.is_empty() {
                    self.push_project_history();
                    scope.sort_unstable_by(|a, b| b.cmp(a));
                    self.with_active_pattern(|pat| {
                        for i in scope {
                            if i < pat.events.len() {
                                pat.events.remove(i);
                            }
                        }
                    });
                    self.show_status("Notes supprimées".to_string());
                    self.event_cursor = 0;
                    self.revision = self.revision.wrapping_add(1);
                }
            }
            "SUPPR" => {
                if let Some((i, e)) = self.selected_event() {
                    self.push_project_history();
                    self.with_active_pattern(|pat| {
                        if i < pat.events.len() {
                            pat.events.remove(i);
                        }
                    });
                    self.show_status(format!(
                        "Note {} supprimée",
                        crate::keyboard::note_name(e.pitch)
                    ));
                    let n = self.event_scope().len();
                    self.event_cursor = self.event_cursor.min(n.saturating_sub(1));
                    self.revision = self.revision.wrapping_add(1);
                }
            }
            "DUPLIQ" => {
                if let Some((_, e)) = self.selected_event() {
                    self.push_project_history();
                    let mut copy = e.clone();
                    let mut added = None;
                    self.with_active_pattern(|pat| {
                        let len = u32::from(pat.length.bars.max(1)) * 4 * 960;
                        copy.tick = (e.tick + e.length_ticks.max(TICKS_PER_STEP)) % len;
                        pat.events.push(copy.clone());
                        pat.events.sort_by_key(|x| x.tick);
                        added = Some(copy);
                    });
                    if let Some(c) = added {
                        self.reselect_event(&c);
                    }
                    self.revision = self.revision.wrapping_add(1);
                }
            }
            "QUANT" => self.edit_selected_event(|e, len| {
                let step = TICKS_PER_STEP;
                e.tick = ((e.tick + step / 2) / step * step).min(len - step);
            }),
            "NUDGE <" => self.edit_selected_event(|e, _| e.tick = e.tick.saturating_sub(60)),
            "NUDGE >" => self.edit_selected_event(|e, len| e.tick = (e.tick + 60).min(len - 1)),
            _ => {}
        }
    }

    /// Crée, met à jour ou retire les synthés du rack d'après le projet : une
    /// instance par Sound qui porte un TB-303, un Kick Synth ou un Nexus.
    /// Appelé hors du thread audio (allocations ici seulement).
    fn sync_synth_rack(&mut self) {
        let proj = Arc::clone(&*self.project.read());
        let sr = self.transport.sample_rate;
        for g in 0..8u8 {
            for s in 0..16u8 {
                let i = rack_index(g, s);
                let snd = proj
                    .groups
                    .get(g as usize)
                    .and_then(|grp| grp.sounds.get(s as usize))
                    .and_then(Option::as_ref);
                match snd.and_then(|snd| snd.synth.as_ref()) {
                    Some(p) => {
                        let slot = self.rack.acid[i].get_or_insert_with(|| {
                            RackSlot::new(crate::acid_synth::AcidSynth::new(sr))
                        });
                        slot.synth.params = p.clone();
                    }
                    None => self.rack.acid[i] = None,
                }
                match snd.and_then(|snd| snd.kick_synth.as_ref()) {
                    Some(p) => {
                        let slot = self.rack.kick[i].get_or_insert_with(|| {
                            RackSlot::new(crate::kick_synth::KickSynth::new(sr))
                        });
                        slot.synth.params = p.clone();
                    }
                    None => self.rack.kick[i] = None,
                }
                match snd.and_then(|snd| snd.wavetable_synth.as_ref()) {
                    Some(p) => {
                        let slot = self.rack.wavetable[i].get_or_insert_with(|| {
                            RackSlot::new(crate::wavetable_synth::WavetableSynth::new(sr))
                        });
                        slot.synth.params = p.clone();
                    }
                    None => self.rack.wavetable[i] = None,
                }
                // Voix de monitoring : mêmes paramètres pour le Sound actif,
                // afin que les knobs tournés pendant REC l'affectent aussi.
                if g == self.group_focus && s == self.active_sound {
                    match snd.and_then(|snd| snd.synth.as_ref()) {
                        Some(p) => {
                            let slot = self.monitor_rack.acid[i].get_or_insert_with(|| {
                                RackSlot::new(crate::acid_synth::AcidSynth::new(sr))
                            });
                            slot.synth.params = p.clone();
                        }
                        None => self.monitor_rack.acid[i] = None,
                    }
                    match snd.and_then(|snd| snd.kick_synth.as_ref()) {
                        Some(p) => {
                            let slot = self.monitor_rack.kick[i].get_or_insert_with(|| {
                                RackSlot::new(crate::kick_synth::KickSynth::new(sr))
                            });
                            slot.synth.params = p.clone();
                        }
                        None => self.monitor_rack.kick[i] = None,
                    }
                    match snd.and_then(|snd| snd.wavetable_synth.as_ref()) {
                        Some(p) => {
                            let slot = self.monitor_rack.wavetable[i].get_or_insert_with(|| {
                                RackSlot::new(crate::wavetable_synth::WavetableSynth::new(sr))
                            });
                            slot.synth.params = p.clone();
                        }
                        None => self.monitor_rack.wavetable[i] = None,
                    }
                }
            }
        }
    }

    /// Crée ou retire les bus d'effets d'après les chaînes du projet. Appelé
    /// hors du thread audio : toutes les allocations des inserts ont lieu ici.
    fn sync_fx_buses(&mut self) {
        let proj = Arc::clone(&*self.project.read());
        let mut owners = Vec::new();
        for (g, group) in proj.groups.iter().enumerate().take(8) {
            for (s, snd) in group.sounds.iter().enumerate().take(16) {
                if snd.as_ref().is_some_and(|snd| !snd.fx_chain.is_empty()) {
                    owners.push(FxOwner::Sound(g as u8, s as u8));
                }
            }
        }
        for (g, group) in proj.groups.iter().enumerate().take(8) {
            if !group.fx_chain.is_empty() {
                owners.push(FxOwner::Group(g as u8));
            }
        }
        if !owners.iter().eq(self.fx_buses.iter().map(|(o, _)| o)) {
            let rate = self.transport.sample_rate;
            let mut old = std::mem::take(&mut self.fx_buses);
            self.fx_buses = owners
                .into_iter()
                .map(|owner| match old.iter().position(|(o, _)| *o == owner) {
                    // Les queues de réverbe / délai survivent à l'ajout d'un autre bus.
                    Some(i) => old.swap_remove(i),
                    None => (owner, crate::fx::FxChainProcessor::new(rate)),
                })
                .collect();
            self.fx_bus_bufs = vec![vec![0.0; FX_BUS_LEN]; self.fx_buses.len()];
        }
        // États DSP slot par slot (ajout, suppression, changement de type).
        for (owner, processor) in &mut self.fx_buses {
            let chain = owner.chain(&proj);
            if !processor.is_prepared_for(chain) {
                processor.prepare(chain);
            }
        }
        if !self.master_fx.is_prepared_for(&proj.master.fx_chain) {
            self.master_fx.prepare(&proj.master.fx_chain);
        }
    }

    /// Index du pattern actif pour le groupe focus dans la scène active.
    pub fn active_pattern_for_focus(&self) -> Option<usize> {
        self.project
            .read()
            .active_pattern_index(self.group_focus as usize, self.active_scene)
    }

    /// Retourne la longueur en mesures du pattern actif pour le groupe focus (défaut 2).
    pub fn active_pattern_bars(&self) -> usize {
        let proj = Arc::clone(&*self.project.read());
        proj.active_pattern_index(self.group_focus as usize, self.active_scene)
            .and_then(|pi| proj.groups.get(self.group_focus as usize)?.patterns.get(pi))
            .map(|p| p.length.bars as usize)
            .unwrap_or(2)
    }

    /// Double la longueur du pattern actif (jusqu'à 16 mesures).
    pub fn double_pattern_length(&mut self) -> crate::Result<()> {
        let cur = self.active_pattern_bars();
        self.handle_event(EngineEvent::SetPatternBars((cur * 2).min(16)))
    }

    /// Divise par 2 la longueur du pattern actif (au minimum 1 mesure).
    pub fn half_pattern_length(&mut self) -> crate::Result<()> {
        let cur = self.active_pattern_bars();
        self.handle_event(EngineEvent::SetPatternBars((cur / 2).max(1)))
    }

    /// Duplique la première mesure du pattern actif sur la deuxième mesure.
    pub fn duplicate_bar(&mut self) {
        let g = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(g, self.active_scene);
        if let Some(group) = proj.groups.get_mut(g) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    let bar1_events: Vec<_> = pat
                        .events
                        .iter()
                        .filter(|e| e.tick < 4 * 960)
                        .cloned()
                        .collect();
                    pat.events.retain(|e| e.tick < 4 * 960);
                    for mut ev in bar1_events {
                        ev.tick += 4 * 960;
                        pat.events.push(ev);
                    }
                }
            }
        }
        self.revision = self.revision.wrapping_add(1);
    }

    fn handle_soft_key_note_repeat(&mut self, idx: usize) -> crate::Result<()> {
        match idx {
            0..=6 => self.handle_event(EngineEvent::SetNoteRepeatRate(idx as u8)),
            _ => self.handle_event(EngineEvent::SetNoteRepeat(false)),
        }
    }

    fn handle_soft_key_shift(&mut self, idx: usize) -> crate::Result<()> {
        if self.scene_mode {
            return match idx {
                0 => self.handle_event(EngineEvent::Scene(0)),
                1 => {
                    let len = self.project.read().scenes.len();
                    self.handle_event(EngineEvent::Scene(len.saturating_sub(1)))
                }
                2 => {
                    let len = self.project.read().scenes.len();
                    for s in 0..len {
                        let _ = self.handle_event(EngineEvent::MakeSceneUnique(s));
                    }
                    Ok(())
                }
                3 => self.handle_event(EngineEvent::DuplicateSceneUnique),
                4 => {
                    if let Some(s) = self.active_scene {
                        let mut guard = self.project.write();
                        let proj = project_mut(&mut guard);
                        if let Some(scene) = proj.scenes.get_mut(s) {
                            scene.patterns.fill(None);
                        }
                        self.revision = self.revision.wrapping_add(1);
                    }
                    Ok(())
                }
                5 => self.handle_event(EngineEvent::DeleteScene),
                6 => {
                    let len = self.project.read().scenes.len();
                    let all: Vec<usize> = (0..len).collect();
                    self.handle_event(EngineEvent::SceneChain(all))
                }
                _ => self.handle_event(EngineEvent::ToggleSongMode),
            };
        }
        if self.pattern_mode {
            return match idx {
                0 => self.double_pattern_length(),
                1 => self.half_pattern_length(),
                2 => {
                    self.quantize_active_pattern();
                    Ok(())
                }
                3 => {
                    self.humanize_active_pattern();
                    Ok(())
                }
                4 => {
                    if let Some(pattern) = self.active_pattern_for_focus() {
                        self.handle_event(EngineEvent::ClearPattern {
                            group: self.group_focus,
                            pattern,
                        })
                    } else {
                        Ok(())
                    }
                }
                5 => {
                    if let Some(pattern) = self.active_pattern_for_focus() {
                        self.handle_event(EngineEvent::DeletePattern {
                            group: self.group_focus,
                            pattern,
                        })
                    } else {
                        Ok(())
                    }
                }
                6 => self.handle_event(EngineEvent::SetPatternBars(4)),
                _ => self.handle_event(EngineEvent::SetPatternBars(8)),
            };
        }
        if self.controller_mode == 3 {
            return match idx {
                0 => {
                    self.clear_bar(self.selected_bar);
                    self.status_message = Some("Mesure vidée".to_string());
                    Ok(())
                }
                1 => {
                    self.quantize_active_pattern();
                    Ok(())
                }
                2 => {
                    self.humanize_active_pattern();
                    Ok(())
                }
                3 => {
                    self.nudge_notes(self.active_sound, self.selected_bar, -240);
                    self.status_message = Some("Nudge -1 pas".to_string());
                    Ok(())
                }
                4 => {
                    self.nudge_notes(self.active_sound, self.selected_bar, 240);
                    self.status_message = Some("Nudge +1 pas".to_string());
                    Ok(())
                }
                5 => {
                    self.transpose_notes(self.active_sound, self.selected_bar, -12);
                    self.status_message = Some("Octave -12".to_string());
                    Ok(())
                }
                6 => {
                    self.transpose_notes(self.active_sound, self.selected_bar, 12);
                    self.status_message = Some("Octave +12".to_string());
                    Ok(())
                }
                _ => {
                    self.fill_sixteenths(self.active_sound, self.selected_bar);
                    self.status_message = Some("Fill 16 pas".to_string());
                    Ok(())
                }
            };
        }
        if self.left_tab == 1 {
            if self.mixer_mode == 0 {
                let pad = idx as u8;
                self.status_message = Some(format!("Pan Pad {} centré (0.0)", pad + 1));
                return self.handle_event(EngineEvent::SoundPan {
                    group: self.group_focus,
                    sound: pad,
                    pan: 0.0,
                });
            } else if self.mixer_mode == 1 {
                let pad = (idx + 8) as u8;
                self.status_message = Some(format!("Pan Pad {} centré (0.0)", pad + 1));
                return self.handle_event(EngineEvent::SoundPan {
                    group: self.group_focus,
                    sound: pad,
                    pan: 0.0,
                });
            } else {
                let grp = idx as u8;
                self.status_message =
                    Some(format!("Pan Groupe {} centré (0.0)", (b'A' + grp) as char));
                return self.handle_event(EngineEvent::GroupPan {
                    group: grp,
                    pan: 0.0,
                });
            }
        }
        if self.left_tab == 2 {
            return match idx {
                0..=2 => self.sample_tool(idx),
                3 => self.handle_event(EngineEvent::CycleSoundFilterKind {
                    group: self.group_focus,
                    sound: self.active_sound,
                }),
                // Touche 4 (idx 4) = «LP/HP/BP» : cycle LP → HP → BP → LP
                4 => {
                    use crate::project::FilterKind;
                    let cur = self
                        .project
                        .read()
                        .groups
                        .get(self.group_focus as usize)
                        .and_then(|g| g.sounds.get(self.active_sound as usize))
                        .and_then(|s| s.as_ref())
                        .map(|s| s.filter.kind)
                        .unwrap_or(FilterKind::Off);
                    let next = match cur {
                        FilterKind::LowPass => FilterKind::HighPass,
                        FilterKind::HighPass => FilterKind::BandPass,
                        _ => FilterKind::LowPass,
                    };
                    self.handle_event(EngineEvent::SetSoundFilterKind {
                        group: self.group_focus,
                        sound: self.active_sound,
                        kind: next,
                    })
                }
                // Touche 5 (idx 5) = NOTCH
                5 => self.handle_event(EngineEvent::SetSoundFilterKind {
                    group: self.group_focus,
                    sound: self.active_sound,
                    kind: crate::project::FilterKind::Notch,
                }),
                // Touche 6 (idx 6) = PEAK
                6 => self.handle_event(EngineEvent::SetSoundFilterKind {
                    group: self.group_focus,
                    sound: self.active_sound,
                    kind: crate::project::FilterKind::Peak,
                }),
                // Touche 7 (idx 7) = OFF
                _ => self.handle_event(EngineEvent::SetSoundFilterKind {
                    group: self.group_focus,
                    sound: self.active_sound,
                    kind: crate::project::FilterKind::Off,
                }),
            };
        }
        match idx {
            0 => {
                self.transport.metronome = !self.transport.metronome;
                self.revision = self.revision.wrapping_add(1);
                Ok(())
            }
            1 => {
                self.transport.count_in_bars = if self.transport.count_in_bars > 0 {
                    0
                } else {
                    1
                };
                self.revision = self.revision.wrapping_add(1);
                Ok(())
            }
            2 => {
                self.quantize_active_pattern();
                Ok(())
            }
            3 => {
                self.humanize_active_pattern();
                Ok(())
            }
            4 => {
                self.clear_bar(self.selected_bar);
                Ok(())
            }
            5 => {
                self.duplicate_bar();
                Ok(())
            }
            6 => self.handle_event(EngineEvent::SetPatternBars(4)),
            _ => self.handle_event(EngineEvent::SetPatternBars(8)),
        }
    }

    fn handle_soft_key_song(&mut self, idx: usize) -> crate::Result<()> {
        match idx {
            0 => self.handle_event(EngineEvent::PrevSection),
            1 => self.handle_event(EngineEvent::NextSection),
            2 => self.handle_event(EngineEvent::AddSection),
            3 => self.handle_event(EngineEvent::ToggleSongMode),
            4 => {
                let scene = self.active_scene.unwrap_or(0).saturating_sub(1);
                self.handle_event(EngineEvent::Scene(scene))
            }
            5 => {
                let n_scenes = self.project.read().scenes.len();
                let scene = (self.active_scene.unwrap_or(0) + 1).min(n_scenes.saturating_sub(1));
                self.handle_event(EngineEvent::Scene(scene))
            }
            6 => self.handle_event(EngineEvent::AddScene),
            _ => self.handle_event(EngineEvent::Scene(0)),
        }
    }

    fn handle_soft_key_pattern(&mut self, idx: usize) -> crate::Result<()> {
        let group = self.group_focus;
        let active = self.active_pattern_for_focus();
        match idx {
            0 => self.handle_event(EngineEvent::StepPattern { group, delta: -1 }),
            1 => self.handle_event(EngineEvent::StepPattern { group, delta: 1 }),
            2 => self.handle_event(EngineEvent::AddPattern { group }),
            3 => match active {
                Some(pattern) => {
                    self.handle_event(EngineEvent::DuplicatePattern { group, pattern })
                }
                None => Ok(()),
            },
            4 => match active {
                Some(pattern) => self.handle_event(EngineEvent::ClearPattern { group, pattern }),
                None => Ok(()),
            },
            5 => match active {
                Some(pattern) => self.handle_event(EngineEvent::DeletePattern { group, pattern }),
                None => Ok(()),
            },
            6 => self.handle_event(EngineEvent::SetPatternBars(1)),
            _ => self.handle_event(EngineEvent::SetPatternBars(2)),
        }
    }

    fn handle_soft_key_step(&mut self, idx: usize) -> crate::Result<()> {
        match idx {
            0 => self.handle_event(EngineEvent::StepScroll(-1)),
            1 => self.handle_event(EngineEvent::StepScroll(1)),
            2 => self.handle_event(EngineEvent::StepScroll(-16)),
            3 => self.handle_event(EngineEvent::StepScroll(16)),
            4 => self.handle_event(EngineEvent::SetPatternBars(1)),
            5 => self.handle_event(EngineEvent::SetPatternBars(2)),
            6 => self.handle_event(EngineEvent::SetPatternBars(4)),
            _ => self.handle_event(EngineEvent::SetPatternBars(8)),
        }
    }

    fn handle_soft_key_keyboard(&mut self, idx: usize) -> crate::Result<()> {
        match idx {
            0 | 1 => self.handle_event(EngineEvent::SetKeyboardState(
                crate::keyboard::KeyboardState {
                    // CHROMA / MINOR (gamme mineure naturelle).
                    scale: if idx == 0 { 0 } else { 2 },
                    ..self.keyboard
                },
            )),
            _ => self.handle_soft_key_chords(idx),
        }
    }

    /// Boutons par paires (− / +) : gamme, accord (CHORDS) ou gamme
    /// (KEYBOARD), octave, demi-ton — voir `soft_key_labels`.
    fn handle_soft_key_chords(&mut self, idx: usize) -> crate::Result<()> {
        let dir = if idx.is_multiple_of(2) { -1 } else { 1 };
        let event = match idx / 2 {
            1 if self.controller_mode == 2 => EngineEvent::KeyboardChordDelta(dir),
            0 | 1 => EngineEvent::KeyboardScaleDelta(dir),
            2 => EngineEvent::KeyboardOctaveDelta(dir),
            _ => EngineEvent::KeyboardRootDelta(dir),
        };
        self.handle_event(event)
    }

    fn handle_soft_key_mixer(&mut self, idx: usize) -> crate::Result<()> {
        if self.mixer_mode == 0 || self.mixer_mode == 1 {
            match idx {
                0 => self.handle_event(EngineEvent::SetMixerMode(0)),
                1 => self.handle_event(EngineEvent::SetMixerMode(1)),
                2 => self.handle_event(EngineEvent::SetMixerMode(2)),
                3 => self.handle_event(EngineEvent::SetMixerMode(3)),
                4 => Ok(()),
                5 => {
                    let p = self.active_sound;
                    self.handle_event(EngineEvent::TogglePadMute {
                        group: self.group_focus,
                        pad: p,
                    })
                }
                6 => self.handle_event(EngineEvent::Solo(self.group_focus)),
                _ => {
                    self.left_tab = 0;
                    Ok(())
                }
            }
        } else {
            self.handle_event(EngineEvent::GroupFocus(idx as u8))
        }
    }

    fn handle_soft_key_sampler(&mut self, idx: usize) -> crate::Result<()> {
        match idx {
            0 => {
                let s = self.active_sound.saturating_sub(1);
                self.handle_event(EngineEvent::SelectSound(s))
            }
            1 => {
                let s = (self.active_sound + 1).min(15);
                self.handle_event(EngineEvent::SelectSound(s))
            }
            2 => self.handle_event(EngineEvent::Pad {
                pad: self.active_sound,
                velocity: 110,
                pressed: true,
            }),
            3 => self.handle_event(EngineEvent::CycleSoundFilterKind {
                group: self.group_focus,
                sound: self.active_sound,
            }),
            // Touche 4 : cycle LP → HP → BP (les 3 filtres «classiques» en un clic)
            4 => {
                use crate::project::FilterKind;
                let cur = self
                    .project
                    .read()
                    .groups
                    .get(self.group_focus as usize)
                    .and_then(|g| g.sounds.get(self.active_sound as usize))
                    .and_then(|s| s.as_ref())
                    .map(|s| s.filter.kind)
                    .unwrap_or(FilterKind::Off);
                let next = match cur {
                    FilterKind::LowPass => FilterKind::HighPass,
                    FilterKind::HighPass => FilterKind::BandPass,
                    _ => FilterKind::LowPass,
                };
                self.handle_event(EngineEvent::SetSoundFilterKind {
                    group: self.group_focus,
                    sound: self.active_sound,
                    kind: next,
                })
            }
            // Touche 5 : filtre Notch
            5 => self.handle_event(EngineEvent::SetSoundFilterKind {
                group: self.group_focus,
                sound: self.active_sound,
                kind: crate::project::FilterKind::Notch,
            }),
            // Touche 6 : filtre Peak (cloche)
            6 => self.handle_event(EngineEvent::SetSoundFilterKind {
                group: self.group_focus,
                sound: self.active_sound,
                kind: crate::project::FilterKind::Peak,
            }),
            // Touche 7 : filtre OFF
            _ => self.handle_event(EngineEvent::SetSoundFilterKind {
                group: self.group_focus,
                sound: self.active_sound,
                kind: crate::project::FilterKind::Off,
            }),
        }
    }

    fn handle_soft_key_browser(&mut self, idx: usize) -> crate::Result<()> {
        if self.browser_kind == 1 && self.delete_project_confirm {
            return match idx {
                0 => self.handle_event(EngineEvent::CancelDeleteProject),
                5 => self.handle_event(EngineEvent::DeleteSelectedProject),
                _ => self.handle_event(EngineEvent::CancelDeleteProject),
            };
        }
        if self.browser_kind == 1 {
            return match idx {
                0 => self.handle_event(EngineEvent::BrowseBrowserStep(-1)),
                1 => self.handle_event(EngineEvent::BrowseBrowserStep(1)),
                2 => self.handle_event(EngineEvent::NewProject {
                    name: String::new(),
                }),
                3 => self.handle_event(EngineEvent::LoadSelectedBrowserItem {
                    group: self.group_focus,
                    sound: self.active_sound,
                }),
                4 => self.handle_event(EngineEvent::SaveCurrent { quick: false }),
                5 => {
                    self.delete_project_confirm = true;
                    self.status_message =
                        Some("Supprimer le projet ? BTN 6: CONFIRMER / BTN 1: ANNULER".into());
                    self.status_expires_at =
                        std::time::Instant::now() + std::time::Duration::from_millis(5000);
                    Ok(())
                }
                6 => self.handle_event(EngineEvent::SetBrowserKind(0)),
                7 => {
                    self.left_tab = 0;
                    Ok(())
                }
                _ => Ok(()),
            };
        }
        if self.browser_kind == 2 {
            return match idx {
                0 => self.handle_event(EngineEvent::BrowseBrowserStep(-1)),
                1 => self.handle_event(EngineEvent::BrowseBrowserStep(1)),
                2 => self.handle_event(EngineEvent::SetBrowserKind(0)),
                3 => self.handle_event(EngineEvent::LoadSelectedBrowserItem {
                    group: self.group_focus,
                    sound: self.active_sound,
                }),
                4 => self.handle_event(EngineEvent::LoadPluginToSound {
                    group: self.group_focus,
                    sound: self.active_sound,
                    plugin_idx: 0,
                }),
                5 => self.handle_event(EngineEvent::LoadPluginToSound {
                    group: self.group_focus,
                    sound: self.active_sound,
                    plugin_idx: 1,
                }),
                6 => self.handle_event(EngineEvent::LoadPluginToSound {
                    group: self.group_focus,
                    sound: self.active_sound,
                    plugin_idx: 3,
                }),
                _ => {
                    self.left_tab = 0;
                    Ok(())
                }
            };
        }
        if self.browser_kind == 3 {
            return match idx {
                0 => self.handle_event(EngineEvent::BrowseBrowserStep(-1)),
                1 => self.handle_event(EngineEvent::BrowseBrowserStep(1)),
                2 => self.handle_event(EngineEvent::SetBrowserKind(0)),
                3 => self.handle_event(EngineEvent::LoadSelectedBrowserItem {
                    group: self.group_focus,
                    sound: self.active_sound,
                }),
                4 => self.handle_event(EngineEvent::AddFxToSound {
                    group: self.group_focus,
                    sound: self.active_sound,
                    fx_type: crate::fx::FxType::Reverb,
                }),
                5 => self.handle_event(EngineEvent::AddFxToSound {
                    group: self.group_focus,
                    sound: self.active_sound,
                    fx_type: crate::fx::FxType::Delay,
                }),
                6 => self.handle_event(EngineEvent::AddFxToSound {
                    group: self.group_focus,
                    sound: self.active_sound,
                    fx_type: crate::fx::FxType::Distortion,
                }),
                _ => {
                    self.left_tab = 0;
                    Ok(())
                }
            };
        }
        match idx {
            0 => self.handle_event(EngineEvent::BrowsePresetStep(-1)),
            1 => self.handle_event(EngineEvent::BrowsePresetStep(1)),
            2 => {
                let s = self.active_sound.saturating_sub(1);
                self.handle_event(EngineEvent::SelectSound(s))
            }
            3 => {
                let s = (self.active_sound + 1).min(15);
                self.handle_event(EngineEvent::SelectSound(s))
            }
            4 => {
                let preset = self.browser_preset_idx;
                self.preview_preset(preset);
                Ok(())
            }
            5 => {
                let (g, s, p) = (self.group_focus, self.active_sound, self.browser_preset_idx);
                self.handle_event(EngineEvent::LoadDrumPreset {
                    group: g,
                    sound: s,
                    preset_idx: p,
                })
            }
            6 => self.handle_event(EngineEvent::SetBrowserKind(2)),
            _ => self.handle_event(EngineEvent::SetBrowserKind(1)),
        }
    }

    fn handle_soft_key(&mut self, n: u8) -> crate::Result<()> {
        let idx = (n.saturating_sub(1) as usize).min(7);
        if self.note_repeat_enabled {
            return self.handle_soft_key_note_repeat(idx);
        }
        if self.right_tab == 1 {
            self.handle_event_soft_key(idx);
            return Ok(());
        }
        if self.left_tab == 2 && !self.shift_held {
            return self.handle_sample_soft_key(idx);
        }
        if let Some(inst) = self.plugin_instrument() {
            return self.handle_plugin_soft_key(inst, idx);
        }
        if self.shift_held {
            return self.handle_soft_key_shift(idx);
        }
        if self.song_mode {
            return self.handle_soft_key_song(idx);
        }
        if self.scene_mode {
            return match idx {
                0 => self.handle_event(EngineEvent::SceneStep(-1)),
                1 => self.handle_event(EngineEvent::SceneStep(1)),
                2 => self.handle_event(EngineEvent::AddScene),
                3 => self.handle_event(EngineEvent::DuplicateScene),
                4 => {
                    if let Some(s) = self.active_scene {
                        self.handle_event(EngineEvent::MakeSceneUnique(s))
                    } else {
                        Ok(())
                    }
                }
                5 => self.handle_event(EngineEvent::DeleteScene),
                6 => self.handle_event(EngineEvent::AddSection),
                _ => self.handle_event(EngineEvent::ToggleSongMode),
            };
        }
        if self.pattern_mode {
            return self.handle_soft_key_pattern(idx);
        }
        if self.controller_mode == 3 && self.left_tab == 0 {
            return self.handle_soft_key_step(idx);
        }
        if self.controller_mode == 1 {
            return self.handle_soft_key_keyboard(idx);
        }
        if self.controller_mode == 2 {
            return self.handle_soft_key_chords(idx);
        }
        match self.left_tab {
            1 => self.handle_soft_key_mixer(idx),
            2 => self.handle_soft_key_sampler(idx),
            3 => self.handle_soft_key_browser(idx),
            // Page ampli guitare : touches gérées par l'application.
            4 => Ok(()),
            _ => match idx {
                0..=3 => self.handle_event(EngineEvent::SelectBar(idx)),
                4 => self.handle_event(EngineEvent::SetPatternBars(1)),
                5 => self.handle_event(EngineEvent::SetPatternBars(2)),
                6 => self.handle_event(EngineEvent::SetPatternBars(4)),
                _ => self.handle_event(EngineEvent::SetPatternBars(8)),
            },
        }
    }

    /// Master volume 0.0..1.0
    #[must_use]
    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Headphone volume 0.0..1.0
    #[must_use]
    pub fn headphone_volume(&self) -> f32 {
        self.headphone_volume
    }

    /// Compteur de révision : change dès qu'un état a bougé (UI/écrans).
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Transport mutable (pour UI).
    #[must_use]
    pub fn transport_mut(&mut self) -> &mut Transport {
        &mut self.transport
    }

    /// Sampler mutable (pour load samples).
    #[must_use]
    pub fn sampler_mut(&mut self) -> &mut Sampler {
        &mut self.sampler
    }

    /// Change swing.
    pub fn set_swing(&mut self, swing: f32) {
        self.transport.swing = swing.clamp(0.0, 1.0);
        self.revision = self.revision.wrapping_add(1);
        self.sequencer.swing = self.transport.swing;
        project_mut(&mut self.project.write()).swing = self.transport.swing;
    }

    /// Group focus actif.
    #[must_use]
    pub fn group_focus(&self) -> u8 {
        self.group_focus
    }

    /// Invalide l'affichage : les écrans se redessinent au prochain frame.
    /// À appeler après toute mutation du projet faite hors `handle_event`.
    pub fn invalidate_display(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    /// Fenêtre de ticks du pas sélectionné (début, fin).
    fn step_window(&self) -> (u32, u32) {
        let start = (self.step_cursor as u32) * 240;
        (start, start + 240)
    }

    /// Nombre total de pas du pattern actif (mesures x 16).
    fn pattern_total_steps(&self) -> usize {
        let proj = Arc::clone(&*self.project.read());
        proj.active_pattern_index(self.group_focus as usize, self.active_scene)
            .and_then(|pi| proj.groups.get(self.group_focus as usize)?.patterns.get(pi))
            .map(|p| (p.length.bars as usize).max(1) * 16)
            .unwrap_or(32)
    }

    /// Paramètres TB-303 du Sound actif, si ce Sound est un synthé.
    pub fn focus_sound_synth_params(&self) -> Option<crate::acid_synth::AcidParams> {
        let proj = Arc::clone(&*self.project.read());
        proj.groups
            .get(self.group_focus as usize)
            .and_then(|g| g.sounds.get(self.active_sound as usize))
            .and_then(|s| s.as_ref())
            .and_then(|s| s.synth.clone())
    }

    /// Le Sound actif est-il un synthétiseur (TB-303, Kick Synth ou Wavetable) ?
    pub fn focus_sound_has_synth(&self) -> bool {
        self.focus_sound_synth_params().is_some()
            || self.focus_sound_kick_params().is_some()
            || self.focus_sound_wavetable_params().is_some()
    }

    /// Le Sound actif est-il un synthétiseur TB-303 Acid ?
    pub fn focus_sound_has_acid(&self) -> bool {
        self.focus_sound_synth_params().is_some()
    }

    /// Paramètres Kick Synth du Sound actif.
    pub fn focus_sound_kick_params(&self) -> Option<crate::kick_synth::KickParams> {
        let proj = Arc::clone(&*self.project.read());
        proj.groups
            .get(self.group_focus as usize)
            .and_then(|g| g.sounds.get(self.active_sound as usize))
            .and_then(|s| s.as_ref())
            .and_then(|s| s.kick_synth.clone())
    }

    /// Le Sound actif est-il un Kick Synth modélisé ?
    pub fn focus_sound_has_kick(&self) -> bool {
        self.focus_sound_kick_params().is_some()
    }

    /// Paramètres Wavetable / Nexus Synth du Sound actif.
    pub fn focus_sound_wavetable_params(&self) -> Option<crate::wavetable_synth::WavetableParams> {
        let proj = Arc::clone(&*self.project.read());
        proj.groups
            .get(self.group_focus as usize)
            .and_then(|g| g.sounds.get(self.active_sound as usize))
            .and_then(|s| s.as_ref())
            .and_then(|s| s.wavetable_synth.clone())
    }

    /// Le Sound actif est-il un Wavetable / Nexus Synth ?
    pub fn focus_sound_has_wavetable(&self) -> bool {
        self.focus_sound_wavetable_params().is_some()
    }

    /// Chaîne d'effets du Sound actif.
    pub fn focus_sound_fx(&self) -> Vec<crate::fx::FxSlot> {
        let proj = Arc::clone(&*self.project.read());
        proj.groups
            .get(self.group_focus as usize)
            .and_then(|g| g.sounds.get(self.active_sound as usize))
            .and_then(|s| s.as_ref())
            .map(|s| s.fx_chain.clone())
            .unwrap_or_default()
    }

    /// Chaîne d'effets du Group focus.
    pub fn focus_group_fx(&self) -> Vec<crate::fx::FxSlot> {
        let proj = Arc::clone(&*self.project.read());
        proj.groups
            .get(self.group_focus as usize)
            .map(|g| g.fx_chain.clone())
            .unwrap_or_default()
    }

    /// Charge les paramètres Kick du Sound actif dans le KickSynth interne.
    pub fn sync_kick_from_active_sound(&mut self) {
        if let Some(params) = self.focus_sound_kick_params() {
            self.kick_synth.params = params;
            self.kick_sound_target = Some((self.group_focus, self.active_sound));
        }
    }

    /// Charge les paramètres Wavetable du Sound actif dans le WavetableSynth interne.
    pub fn sync_wavetable_from_active_sound(&mut self) {
        if let Some(params) = self.focus_sound_wavetable_params() {
            self.wavetable_synth.params = params;
            self.wavetable_sound_target = Some((self.group_focus, self.active_sound));
        }
    }

    /// Enregistre les paramètres Wavetable courants dans le Sound actif du projet.
    pub fn save_wavetable_params_to_active_sound(&mut self) {
        self.wavetable_sound_target = Some((self.group_focus, self.active_sound));
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        if let Some(g) = proj.groups.get_mut(self.group_focus as usize) {
            let snd = g.sounds[self.active_sound as usize].get_or_insert_with(|| {
                crate::project::Sound::new(&format!("Nexus {}", self.active_sound + 1), 78)
            });
            snd.wavetable_synth = Some(self.wavetable_synth.params.clone());
            snd.synth = None;
            snd.kick_synth = None;
        }
        self.revision = self.revision.wrapping_add(1);
    }

    /// Supprime définitivement le projet sélectionné dans le navigateur.
    pub fn delete_selected_project(&mut self) -> crate::Result<()> {
        self.delete_project_confirm = false;
        if let Some(entry) = self.project_entries.get(self.project_selected).cloned() {
            let path = std::path::Path::new(&entry.path);
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
            let deleted_name = entry.name;
            self.rescan_local_projects();
            if self.project_selected >= self.project_entries.len() {
                self.project_selected = self.project_entries.len().saturating_sub(1);
            }
            self.status_message = Some(format!("Projet '{}' supprimé", deleted_name));
            self.status_expires_at =
                std::time::Instant::now() + std::time::Duration::from_millis(3000);
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }

    /// Charge un plugin (0: Kick Synth, 1: TB-303, 2: Nexus Synth, 3: Guitar Amp, 4: Sampler) sur un pad.
    pub fn load_plugin_to_sound(
        &mut self,
        group: u8,
        sound: u8,
        plugin_idx: u8,
    ) -> crate::Result<()> {
        let mut guard = self.project.write();
        let project = project_mut(&mut guard);
        if let Some(grp) = project.groups.get_mut(group as usize) {
            let target_sound = grp.sounds[sound as usize].get_or_insert_with(|| {
                crate::project::Sound::new(&format!("Sound {}", sound + 1), 78)
            });
            match plugin_idx {
                0 => {
                    target_sound.kick_synth = Some(crate::kick_synth::KickParams::default());
                    target_sound.synth = None;
                    target_sound.wavetable_synth = None;
                    target_sound.name = "Kick Synth".to_string();
                    self.kick_synth.params = crate::kick_synth::KickParams::default();
                    self.kick_sound_target = Some((group, sound));
                    self.status_message =
                        Some("Plugin chargé : Kick Synth (Modélisation Techno)".to_string());
                }
                1 => {
                    target_sound.synth = Some(crate::acid_synth::AcidParams::preset_classic_acid());
                    target_sound.kick_synth = None;
                    target_sound.wavetable_synth = None;
                    target_sound.name = "TB-303".to_string();
                    self.acid_synth.params = crate::acid_synth::AcidParams::preset_classic_acid();
                    self.acid_sound_target = Some((group, sound));
                    self.status_message = Some("Plugin chargé : TB-303 Acid Bass".to_string());
                }
                2 => {
                    target_sound.wavetable_synth =
                        Some(crate::wavetable_synth::WavetableParams::default());
                    target_sound.synth = None;
                    target_sound.kick_synth = None;
                    target_sound.name = "Nexus Synth".to_string();
                    self.wavetable_synth.params =
                        crate::wavetable_synth::WavetableParams::default();
                    self.wavetable_sound_target = Some((group, sound));
                    self.status_message =
                        Some("Plugin chargé : Nexus / Serum Wavetable Synth".to_string());
                }
                3 => {
                    target_sound.synth = None;
                    target_sound.kick_synth = None;
                    target_sound.wavetable_synth = None;
                    target_sound.name = "Guitar Amp".to_string();
                    target_sound.fx_chain = vec![
                        crate::fx::FxSlot::default_for(crate::fx::FxType::Distortion),
                        crate::fx::FxSlot::default_for(crate::fx::FxType::Delay),
                    ];
                    self.status_message = Some("Plugin chargé : Guitar Amp & FX".to_string());
                }
                _ => {
                    target_sound.synth = None;
                    target_sound.kick_synth = None;
                    target_sound.wavetable_synth = None;
                    target_sound.name = DRUM_PRESET_NAMES
                        .get(sound as usize)
                        .copied()
                        .unwrap_or("Sampler")
                        .to_string();
                    self.status_message = Some("Plugin chargé : Sampler Kit 808".to_string());
                }
            }
            if group == self.group_focus && sound == self.active_sound {
                self.plugin_page = if plugin_idx <= 2 { 1 } else { 0 };
            }
            self.status_expires_at =
                std::time::Instant::now() + std::time::Duration::from_millis(3000);
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }

    /// Ajoute un effet à un Sound.
    pub fn add_fx_to_sound(
        &mut self,
        group: u8,
        sound: u8,
        fx_type: crate::fx::FxType,
    ) -> crate::Result<()> {
        self.insert_fx(FxOwner::Sound(group, sound), fx_type);
        Ok(())
    }

    /// Ajoute un effet au bus d'un groupe.
    pub fn add_fx_to_group(&mut self, group: u8, fx_type: crate::fx::FxType) -> crate::Result<()> {
        self.insert_fx(FxOwner::Group(group), fx_type);
        Ok(())
    }

    /// Charge les paramètres TB-303 du Sound actif dans le synthé global
    /// (édition, pré-écoute et écrans).
    fn sync_acid_from_active_sound(&mut self) {
        if let Some(params) = self.focus_sound_synth_params() {
            self.acid_synth.params = params;
            self.acid_sound_target = Some((self.group_focus, self.active_sound));
        }
    }

    /// Garantit que le Sound actif possède un slot synthé TB-303 (utile quand
    /// l'utilisateur applique un preset TB sur un pad sampler depuis la page PLUGIN).
    fn ensure_active_sound_synth(&mut self) {
        let params = self.acid_synth.params.clone();
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        if let Some(g) = proj.groups.get_mut(self.group_focus as usize) {
            let slot = g
                .sounds
                .get_mut(self.active_sound as usize)
                .and_then(|s| s.as_mut());
            if slot.is_none() {
                let mut s =
                    crate::project::Sound::new(&format!("TB {}", self.active_sound + 1), 100);
                s.synth = Some(params);
                if let Some(entry) = g.sounds.get_mut(self.active_sound as usize) {
                    *entry = Some(s);
                }
            } else if let Some(sound) = slot {
                if sound.synth.is_none() {
                    sound.synth = Some(params);
                }
            }
        }
    }

    /// Persiste les paramètres TB-303 globaux dans le Sound actif lorsqu'il
    /// s'agit d'un synthé, afin que les éditions suivent le pad sélectionné.
    fn persist_acid_to_active_sound(&mut self) {
        if !self.focus_sound_has_synth() {
            return;
        }
        let params = self.acid_synth.params.clone();
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        if let Some(sound) = proj
            .groups
            .get_mut(self.group_focus as usize)
            .and_then(|g| g.sounds.get_mut(self.active_sound as usize))
            .and_then(|s| s.as_mut())
        {
            if sound.synth.is_some() {
                sound.synth = Some(params);
            }
        }
    }

    /// Synchronise la page PLUGIN et les paramètres TB avec le Sound actif.
    fn sync_plugin_for_active_sound(&mut self) {
        // La page 2 (ampli guitare) est pilotée par la GUI et doit survivre
        // à un changement de Sound.
        if self.plugin_page != 2 {
            self.plugin_page = if self.focus_sound_has_synth() { 1 } else { 0 };
        }
        self.sync_acid_from_active_sound();
        self.sync_kick_from_active_sound();
        self.sync_wavetable_from_active_sound();
    }

    /// Exécute `f` sur le pattern actif du groupe focus (scène active).
    fn with_active_pattern<F: FnOnce(&mut crate::project::Pattern)>(&mut self, f: F) -> bool {
        let group_idx = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = ensure_group_pattern(proj, &mut self.active_scene, group_idx);
        if let (Some(group), Some(pi)) = (proj.groups.get_mut(group_idx), pat_idx) {
            if let Some(pat) = group.patterns.get_mut(pi) {
                f(pat);
                return true;
            }
        }
        false
    }

    /// Bascule une note à un pas donné (tick) pour le pad donné dans le pattern actif.
    /// Façon Maschine : silencieux — poser un pas ne joue aucun son, l'aperçu
    /// se fait en mode Pad ou via les boutons ▶ de la GUI.
    /// La comparaison se fait par case de 16e (tolère humanize/nudge) et
    /// supprime tous les doublons de la case : un seul appui suffit toujours.
    pub fn toggle_step(&mut self, pad: u8, tick: u32) {
        self.push_project_history();
        self.push_step_history();
        // Le pas touché devient le pas sélectionné (les knobs l'éditent).
        self.selected_bar = (tick / (4 * 960)) as usize;
        self.step_cursor = (tick / 240) as usize;
        self.revision = self.revision.wrapping_add(1);
        let group_idx = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group_idx, self.active_scene);
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if group.patterns.is_empty() {
                group.patterns.push(crate::project::Pattern::new(
                    "Pattern 1",
                    crate::project::BarsBeatsTicks::new(2, 0, 0),
                ));
            }
            let idx = pat_idx.unwrap_or(0).min(group.patterns.len() - 1);
            if let Some(pat) = group.patterns.get_mut(idx) {
                let before = pat.events.len();
                pat.events
                    .retain(|e| !(e.pad == pad && crate::project::same_step_cell(e.tick, tick)));
                if pat.events.len() == before {
                    pat.events.push(crate::project::PatternEvent {
                        pad,
                        tick,
                        velocity: if self.fixed_velocity {
                            self.fixed_velocity_level
                        } else {
                            115
                        },
                        length_ticks: 120,
                        pitch: 36 + pad,
                    });
                    pat.events.sort_by_key(|e| e.tick);
                }
            }
        }
    }

    /// Remplit les temps 1, 2, 3, 4 pour un pad (ex: Kick 4-on-the-Floor)
    pub fn fill_four_on_the_floor(&mut self, pad: u8, bar: usize) {
        self.push_project_history();
        let group_idx = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group_idx, self.active_scene);
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    let bar_start = (bar * 4 * 960) as u32;
                    let bar_end = bar_start + 4 * 960;
                    pat.events
                        .retain(|e| !(e.pad == pad && e.tick >= bar_start && e.tick < bar_end));
                    for beat in 0..4 {
                        pat.events.push(crate::project::PatternEvent {
                            pad,
                            tick: bar_start + beat * 960,
                            velocity: 115,
                            length_ticks: 120,
                            pitch: 36 + pad,
                        });
                    }
                }
            }
        }
    }

    /// Remplit les croches (8ths) pour un pad (ex: Charlestons)
    pub fn fill_eighths(&mut self, pad: u8, bar: usize) {
        self.push_project_history();
        let group_idx = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group_idx, self.active_scene);
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    let bar_start = (bar * 4 * 960) as u32;
                    let bar_end = bar_start + 4 * 960;
                    pat.events
                        .retain(|e| !(e.pad == pad && e.tick >= bar_start && e.tick < bar_end));
                    for step in 0..8 {
                        pat.events.push(crate::project::PatternEvent {
                            pad,
                            tick: bar_start + step * 480,
                            velocity: if step % 2 == 0 { 95 } else { 75 },
                            length_ticks: 120,
                            pitch: 36 + pad,
                        });
                    }
                }
            }
        }
    }

    /// Supprime toutes les notes d'une mesure donnée
    pub fn clear_bar(&mut self, bar: usize) {
        self.push_project_history();
        self.revision = self.revision.wrapping_add(1);
        let group_idx = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group_idx, self.active_scene);
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    let bar_start = (bar * 4 * 960) as u32;
                    let bar_end = bar_start + 4 * 960;
                    pat.events
                        .retain(|e| !(e.tick >= bar_start && e.tick < bar_end));
                }
            }
        }
    }

    /// Remplit les 16 doubles-croches de la mesure courante pour le pad actif (FILL)
    pub fn fill_sixteenths(&mut self, pad: u8, bar: usize) {
        self.push_project_history();
        self.revision = self.revision.wrapping_add(1);
        let group_idx = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group_idx, self.active_scene);
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    let bar_start = (bar * 4 * 960) as u32;
                    let bar_end = bar_start + 4 * 960;
                    pat.events
                        .retain(|e| !(e.pad == pad && e.tick >= bar_start && e.tick < bar_end));
                    for step in 0..16 {
                        pat.events.push(crate::project::PatternEvent {
                            pad,
                            tick: bar_start + step * 240,
                            velocity: if step % 4 == 0 { 110 } else { 85 },
                            length_ticks: 120,
                            pitch: 36 + pad,
                        });
                    }
                }
            }
        }
    }

    /// Décale les notes du pad actif dans la mesure (nudge)
    pub fn nudge_notes(&mut self, pad: u8, bar: usize, delta_ticks: i32) {
        self.revision = self.revision.wrapping_add(1);
        let group_idx = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group_idx, self.active_scene);
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    let bar_start = (bar * 4 * 960) as u32;
                    let bar_end = bar_start + 4 * 960;
                    for e in pat.events.iter_mut() {
                        if e.pad == pad && e.tick >= bar_start && e.tick < bar_end {
                            let new_tick = (e.tick as i32 + delta_ticks).max(bar_start as i32);
                            e.tick = (new_tick as u32).min(bar_end.saturating_sub(1));
                        }
                    }
                }
            }
        }
    }

    /// Transpose les notes du pad actif dans la mesure (octave +/- 12 demi-tons)
    pub fn transpose_notes(&mut self, pad: u8, bar: usize, delta_semitones: i8) {
        self.revision = self.revision.wrapping_add(1);
        let group_idx = self.group_focus as usize;
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let pat_idx = proj.active_pattern_index(group_idx, self.active_scene);
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if let Some(pi) = pat_idx {
                if let Some(pat) = group.patterns.get_mut(pi) {
                    let bar_start = (bar * 4 * 960) as u32;
                    let bar_end = bar_start + 4 * 960;
                    for e in pat.events.iter_mut() {
                        if e.pad == pad && e.tick >= bar_start && e.tick < bar_end {
                            let new_pitch = (e.pitch as i16 + delta_semitones as i16).clamp(0, 127);
                            e.pitch = new_pitch as u8;
                        }
                    }
                }
            }
        }
    }

    /// Réinitialise les patterns de démo (une famille d'instruments par Groupe).
    pub fn reset_demo_groove(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        let demo_proj = crate::project::Project::new_demo();
        let mut guard = self.project.write();
        let proj = project_mut(&mut guard);
        let count = proj.groups.len().min(demo_proj.groups.len());
        for (gi, demo_group) in demo_proj.groups.iter().enumerate().take(count) {
            let Some(demo_p) = demo_group.patterns.first() else {
                continue;
            };
            if let Some(g) = proj.groups.get_mut(gi) {
                if g.patterns.is_empty() {
                    g.patterns.push(demo_p.clone());
                } else {
                    g.patterns[0] = demo_p.clone();
                }
            }
        }
    }
}

fn sanitize_project_filename(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    out = out.trim().trim_matches('.').to_string();
    if out.is_empty() {
        out = "Untitled".to_string();
    }
    if out.len() > 80 {
        out.truncate(80);
    }
    if !out.ends_with(".maschine-linux.json") && !out.ends_with(".json") {
        out.push_str(".maschine-linux.json");
    }
    out
}

fn default_projects_dir() -> std::path::PathBuf {
    std::env::var("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            std::path::PathBuf::from(home).join(".local/share")
        })
        .join("maschine-linux")
        .join("projects")
}

/// Scanne le répertoire local des projets et retourne les entrées triées par date.
pub fn scan_local_projects() -> Vec<ProjectFileEntry> {
    let dir = default_projects_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if (name.ends_with(".maschine-linux.json") || name.ends_with(".json"))
                && !name.starts_with('.')
            {
                paths.push(p);
            }
        }
    }
    paths.sort_by_key(|a| {
        std::cmp::Reverse(
            std::fs::metadata(a)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
        )
    });
    paths
        .into_iter()
        .map(|p| {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("Projet");
            let stem = stem.strip_suffix(".maschine-linux").unwrap_or(stem);
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            let meta = if size >= 1024 * 1024 {
                format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
            } else {
                format!("{} KB", (size / 1024).max(1))
            };
            ProjectFileEntry {
                name: stem.to_string(),
                path: p.to_string_lossy().to_string(),
                meta,
            }
        })
        .collect()
}

/// Génère un nom et un chemin de projet unique sans écraser l'existant.
pub fn generate_unique_project_path(base_name: &str) -> (String, std::path::PathBuf) {
    let dir = default_projects_dir();
    let _ = std::fs::create_dir_all(&dir);
    let trimmed = base_name.trim();
    if trimmed.is_empty() || trimmed == "Nouveau" || trimmed == "Untitled" {
        for i in 1..=9999 {
            let candidate_name = format!("Projet {}", i);
            let file_name = format!("{}.maschine-linux.json", candidate_name);
            let path = dir.join(&file_name);
            if !path.exists() {
                return (candidate_name, path);
            }
        }
    }
    let safe_stem = sanitize_project_filename(trimmed);
    let stem = safe_stem
        .strip_suffix(".maschine-linux.json")
        .unwrap_or(&safe_stem)
        .strip_suffix(".json")
        .unwrap_or(&safe_stem)
        .to_string();
    let initial_path = dir.join(format!("{}.maschine-linux.json", stem));
    if !initial_path.exists() {
        return (stem, initial_path);
    }
    for i in 2..=9999 {
        let candidate_name = format!("{} ({})", stem, i);
        let path = dir.join(format!("{}.maschine-linux.json", candidate_name));
        if !path.exists() {
            return (candidate_name, path);
        }
    }
    (stem.clone(), initial_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "sampler")]
    fn write_test_wav(path: &std::path::Path, samples: &[i16], sample_rate: u32) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for &sample in samples {
            writer.write_sample(sample).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn test_temp_dir(label: &str) -> std::path::PathBuf {
        static NEXT_DIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("maschine-{label}-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(feature = "sampler")]
    #[test]
    fn wav_import_is_group_scoped_and_persisted_in_sound() {
        let dir = test_temp_dir("group-sample");
        let wav_path = dir.join("group-b.wav");
        write_test_wav(&wav_path, &[i16::MIN, 0, i16::MAX], 24000);

        let mut project = Project::new_demo();
        project
            .groups
            .push(crate::project::Group::new("Group B", 22));
        for scene in &mut project.scenes {
            scene.patterns.push(None);
        }
        let mut engine = Engine::new(project, 48000);
        engine.sampler_mut().add_zone(
            sampler_sound_id(0, 0),
            crate::sampler::ZoneMapping {
                sample: Some(std::sync::Arc::new(vec![0.25, -0.25, 0.5, -0.5])),
                sample_rate: 48000,
                root_key: 36,
                ..Default::default()
            },
        );
        let group_a_waveform = engine.waveform_envelope(0, 0, 512).unwrap();
        let zone = Sampler::prepare_wav(&wav_path, 36).unwrap();
        engine
            .handle_event(EngineEvent::LoadPreparedSample {
                group: 1,
                sound: 0,
                zone,
            })
            .unwrap();

        assert!(std::sync::Arc::ptr_eq(
            &group_a_waveform,
            &engine.waveform_envelope(0, 0, 512).unwrap()
        ));
        assert_eq!(engine.sample_duration_secs(1, 0), 3.0 / 24000.0);
        let snapshot = engine.snapshot();
        assert_eq!(
            snapshot.project.groups[1].sounds[0].as_ref().unwrap().name,
            "group-b"
        );
        assert_eq!(
            snapshot.project.groups[1].sounds[0].as_ref().unwrap().zones[0]
                .sample_path
                .as_deref(),
            Some(wav_path.canonicalize().unwrap().to_str().unwrap())
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sample_edits_survive_save_and_reload() {
        let dir = test_temp_dir("sample-edits");
        std::fs::create_dir_all(dir.join("samples")).unwrap();
        write_test_wav(
            &dir.join("samples/ramp.wav"),
            &[1000, 2000, 3000, 4000],
            48000,
        );
        let project_path = dir.join("project.json");
        let mut project = Project::new_demo();
        project.groups[0].sounds[0].as_mut().unwrap().zones[0].sample_path =
            Some("samples/ramp.wav".to_string());
        project.save_json(&project_path).unwrap();

        let mut engine = Engine::new_demo(48000);
        engine
            .handle_event(EngineEvent::Load(project_path.clone()))
            .unwrap();
        engine.sample_tool(0).unwrap(); // REVERSE
        engine.sample_tool(1).unwrap(); // LOOP
        engine
            .handle_event(EngineEvent::Save(project_path.clone()))
            .unwrap();

        let mut reopened = Engine::new_demo(48000);
        reopened
            .handle_event(EngineEvent::Load(project_path.clone()))
            .unwrap();
        let zone = reopened
            .sampler_mut()
            .zone_mapping(sampler_sound_id(0, 0))
            .unwrap();
        let sample = zone.sample.unwrap();
        assert!(sample[0] > sample[3], "REVERSE rejoué au chargement");
        assert!(!zone.is_one_shot, "LOOP rejoué au chargement");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn loading_project_rehydrates_relative_sample_paths() {
        let dir = test_temp_dir("project-sample");
        let samples_dir = dir.join("samples");
        std::fs::create_dir_all(&samples_dir).unwrap();
        let wav_path = samples_dir.join("tone.wav");
        write_test_wav(&wav_path, &vec![8192; 24000], 24000);
        let project_path = dir.join("project.json");
        let mut project = Project::new_demo();
        project.groups[0].sounds[0].as_mut().unwrap().zones[0].sample_path =
            Some("samples/tone.wav".to_string());
        project.save_json(&project_path).unwrap();

        let mut engine = Engine::new_demo(48000);
        engine
            .handle_event(EngineEvent::Load(project_path.clone()))
            .unwrap();

        assert!((engine.sample_duration_secs(0, 0) - 1.0).abs() < 0.001);
        let waveform = engine.waveform_envelope(0, 0, 512).unwrap();
        assert_eq!(waveform.len(), 512);
        assert!((waveform[0].0 - 0.25).abs() < 0.001);
        assert!((waveform[0].1 - 0.25).abs() < 0.001);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn selecting_sampler_restores_contextual_right_screen_after_events() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::SetRightTab(1)).unwrap();
        assert_eq!(eng.snapshot().right_tab, 1);

        eng.handle_event(EngineEvent::SetLeftTab(2)).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.left_tab, 2);
        assert_eq!(snap.right_tab, 0);
        assert!(!snap.song_mode);
    }

    #[test]
    fn plugin_follows_selected_sound_and_persists_tb_edits() {
        let mut eng = Engine::new_demo(48000);
        // Un Sound sampler ouvre la page SOUND.
        eng.handle_event(EngineEvent::SelectSound(0)).unwrap();
        eng.handle_event(EngineEvent::SetLeftTab(4)).unwrap();
        assert_eq!(eng.snapshot().plugin_page, 0);

        // Sur la page TB, un preset rend le Sound actif synthé et persiste.
        eng.handle_event(EngineEvent::SetPluginPage(1)).unwrap();
        eng.handle_event(EngineEvent::AcidSelectPreset(1)).unwrap();
        {
            let snap = eng.snapshot();
            assert_eq!(snap.plugin_page, 1);
            let sound = snap.project.groups[snap.group_focus as usize].sounds[0]
                .as_ref()
                .unwrap();
            assert!(sound.synth.is_some());
            let cutoff = sound.synth.as_ref().unwrap().cutoff_hz;
            assert!((cutoff - snap.acid_params.cutoff_hz).abs() < 0.01);
        }

        // L'édition d'un paramètre TB est persistée dans le Sound sélectionné.
        eng.handle_event(EngineEvent::AcidCutoffDelta(10.0))
            .unwrap();
        let cutoff_after = {
            let snap = eng.snapshot();
            snap.project.groups[snap.group_focus as usize].sounds[0]
                .as_ref()
                .unwrap()
                .synth
                .as_ref()
                .unwrap()
                .cutoff_hz
        };

        // Changer de pad en vue PLUGIN suit le type du Sound.
        eng.handle_event(EngineEvent::SelectSound(1)).unwrap();
        assert_eq!(eng.snapshot().plugin_page, 0);

        // Revenir sur le pad TB restaure sa page et ses paramètres.
        eng.handle_event(EngineEvent::SelectSound(0)).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.plugin_page, 1);
        let restored = snap.project.groups[snap.group_focus as usize].sounds[0]
            .as_ref()
            .unwrap()
            .synth
            .as_ref()
            .unwrap()
            .cutoff_hz;
        assert!((restored - cutoff_after).abs() < 0.01);
        assert!((snap.acid_params.cutoff_hz - cutoff_after).abs() < 0.01);
    }

    #[test]
    fn selecting_synth_tab_preserves_left_tab_4() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::SetLeftTab(4)).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.left_tab, 4);
        assert_eq!(snap.right_tab, 0);
        assert!(!snap.song_mode);
    }

    #[test]
    fn selecting_right_overlay_exits_song_mode() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::ToggleSongMode).unwrap();
        assert!(eng.snapshot().song_mode);

        eng.handle_event(EngineEvent::SetRightTab(1)).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.right_tab, 1);
        assert!(!snap.song_mode);
    }

    #[test]
    fn fixed_velocity_applies_to_steps_and_live_pads() {
        let mut eng = Engine::new_demo(48000);
        eng.clear_bar(0);
        eng.clear_bar(1);
        eng.handle_event(EngineEvent::SetFixedVelocity(true))
            .unwrap();
        eng.handle_event(EngineEvent::SetFixedVelocityLevel(64))
            .unwrap();
        eng.toggle_step(0, 0);
        {
            let guard = eng.project_handle();
            let proj = guard.read();
            let ev = proj.groups[0].patterns[0]
                .events
                .iter()
                .find(|e| e.pad == 0 && e.tick == 0)
                .expect("le pas doit être posé");
            assert_eq!(ev.velocity, 64);
        }
        // Jeu live enregistré : la vélocité du pad est remplacée par la valeur fixe.
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        eng.handle_event(EngineEvent::Pad {
            pad: 1,
            velocity: 20,
            pressed: true,
        })
        .unwrap();
        eng.handle_event(EngineEvent::Pad {
            pad: 1,
            velocity: 0,
            pressed: false,
        })
        .unwrap();
        {
            let guard = eng.project_handle();
            let proj = guard.read();
            let ev = proj.groups[0].patterns[0]
                .events
                .iter()
                .find(|e| e.pad == 1 && e.tick == 0)
                .expect("la note enregistrée doit exister");
            assert_eq!(ev.velocity, 64);
        }
        // Désactivée : on retrouve la vélocité par défaut du step sequencer.
        eng.handle_event(EngineEvent::SetFixedVelocity(false))
            .unwrap();
        eng.toggle_step(2, 480);
        let guard = eng.project_handle();
        let proj = guard.read();
        let ev = proj.groups[0].patterns[0]
            .events
            .iter()
            .find(|e| e.pad == 2 && e.tick == 480)
            .expect("le pas doit être posé");
        assert_eq!(ev.velocity, 115);
    }

    #[test]
    fn rec_button_from_transport_records_pad_notes() {
        let mut eng = Engine::new_demo(48000);
        eng.clear_bar(0);
        eng.clear_bar(1);
        // Appui sur REC (bouton MK3) : play + rec.
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        assert_eq!(eng.transport.state, TransportState::Recording);
        eng.handle_event(EngineEvent::Pad {
            pad: 5,
            velocity: 90,
            pressed: true,
        })
        .unwrap();
        let guard = eng.project_handle();
        let proj = guard.read();
        let ev = proj.groups[0].patterns[0]
            .events
            .iter()
            .find(|e| e.pad == 5)
            .expect("la note enregistrée doit exister");
        assert_eq!(ev.velocity, 90);
        assert_eq!(ev.pitch, 41);
    }

    #[test]
    fn rec_pad_slides_are_recorded() {
        let mut eng = Engine::new_demo(48000);
        eng.clear_bar(0);
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        // Note A sur le pad 5, 2 temps de tenue, puis note B (slide TB-303).
        eng.handle_event(EngineEvent::Pad {
            pad: 5,
            velocity: 90,
            pressed: true,
        })
        .unwrap();
        eng.advance(48_000);
        eng.handle_event(EngineEvent::Pad {
            pad: 5,
            velocity: 100,
            pressed: true,
        })
        .unwrap();
        eng.advance(48_000);
        eng.handle_event(EngineEvent::Pad {
            pad: 5,
            velocity: 0,
            pressed: false,
        })
        .unwrap();
        let guard = eng.project_handle();
        let proj = guard.read();
        let mut evs: Vec<&PatternEvent> = proj.groups[0].patterns[0]
            .events
            .iter()
            .filter(|e| e.pad == 5)
            .collect();
        assert_eq!(evs.len(), 2, "les deux notes du slide sont enregistrées");
        evs.sort_by_key(|e| e.tick);
        let (first, second) = (evs[0], evs[1]);
        assert_eq!(first.velocity, 90);
        assert_eq!(second.velocity, 100);
        // Le portamento est enregistré : la première note couvre jusqu'à la
        // seconde au lieu d'être coupée directement.
        assert_eq!(first.length_ticks, second.tick - first.tick);
    }

    #[test]
    fn rec_pad_sounds_during_hold_like_without_rec() {
        let mut eng = Engine::new_demo(48000);
        eng.load_plugin_to_sound(0, 0, 1).unwrap(); // TB-303 sur le pad 0
                                                    // Sans REC : le son sonne pendant la tenue.
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 110,
            pressed: true,
        })
        .unwrap();
        let mut block = vec![0.0f32; 2048];
        let mut peak = 0.0f32;
        for _ in 0..50 {
            eng.advance(1024);
            block.fill(0.0);
            eng.render_audio(&mut block);
            peak = peak.max(block.iter().map(|s| s.abs()).fold(0.0f32, f32::max));
        }
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 0,
            pressed: false,
        })
        .unwrap();
        // Avec REC : idem.
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 110,
            pressed: true,
        })
        .unwrap();
        let mut peak2 = 0.0f32;
        for _ in 0..50 {
            eng.advance(1024);
            block.fill(0.0);
            eng.render_audio(&mut block);
            peak2 = peak2.max(block.iter().map(|s| s.abs()).fold(0.0f32, f32::max));
        }
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 0,
            pressed: false,
        })
        .unwrap();
        assert!(
            peak > 0.02,
            "sans REC, le son sonne pendant la tenue (peak = {peak})"
        );
        assert!(
            peak2 > 0.02,
            "pendant REC, le son sonne pendant la tenue (peak = {peak2})"
        );
    }

    #[test]
    fn rec_slide_sounds_during_rec() {
        let mut eng = Engine::new_demo(48000);
        eng.load_plugin_to_sound(0, 0, 1).unwrap(); // TB-303 sur le pad 0
        let mut block = vec![0.0f32; 2048];
        let mut peak = 0.0f32;
        let mut capture = |eng: &mut Engine, peak: &mut f32| {
            for _ in 0..50 {
                eng.advance(1024);
                block.fill(0.0);
                eng.render_audio(&mut block);
                *peak = peak.max(block.iter().map(|s| s.abs()).fold(0.0f32, f32::max));
            }
        };
        // Sans REC : presser A, tenir, presser B — le son continue (slide).
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 110,
            pressed: true,
        })
        .unwrap();
        eng.advance(48_000);
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 100,
            pressed: true,
        })
        .unwrap();
        capture(&mut eng, &mut peak);
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 0,
            pressed: false,
        })
        .unwrap();
        assert!(
            peak > 0.02,
            "sans REC, le son continue pendant le slide (peak = {peak})"
        );
        // Avec REC.
        peak = 0.0;
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 110,
            pressed: true,
        })
        .unwrap();
        eng.advance(48_000);
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 100,
            pressed: true,
        })
        .unwrap();
        capture(&mut eng, &mut peak);
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 0,
            pressed: false,
        })
        .unwrap();
        assert!(
            peak > 0.02,
            "avec REC, le son continue pendant le slide (peak = {peak})"
        );
    }

    #[test]
    fn pattern_playback_lights_pads_via_activity() {
        let mut eng = Engine::new_demo(48000);
        // Lecture du pattern : les pads des notes jouées doivent s'illuminer
        // via l'activité (utilisée par le mapper pour les LEDs).
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: false,
            stop: false,
        })
        .unwrap();
        for _ in 0..10 {
            eng.advance(480);
        }
        let act = eng.pad_activity.to_vec();
        println!("pad_activity = {act:?}");
        let any_sound = act.iter().any(|&v| v > 0.02);
        assert!(any_sound, "aucun pad allumé pendant la lecture du pattern");
    }

    #[test]
    fn keyboard_rec_long_note_is_not_cut_by_pattern_playback() {
        let mut eng = Engine::new_demo(48000);
        eng.load_plugin_to_sound(0, 0, 1).unwrap(); // TB-303 sur le pad 0
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        // Une note tenue longtemps en KEYBOARD pendant REC.
        eng.handle_event(EngineEvent::KeyboardTrigger {
            note: 60,
            velocity: 110,
            pressed: true,
        })
        .unwrap();
        eng.advance(48_000);
        let mut block = vec![0.0f32; 2048];
        let mut late_peak = 0.0f32;
        for i in 0..50 {
            eng.advance(1024);
            block.fill(0.0);
            eng.render_audio(&mut block);
            if i >= 30 {
                late_peak = late_peak.max(block.iter().map(|s| s.abs()).fold(0.0f32, f32::max));
            }
        }
        println!("keyboard REC late peak = {late_peak}");
        assert!(
            late_peak > 0.02,
            "la note tenue doit rester audible pendant REC (pic tardif = {late_peak})"
        );
    }

    #[test]
    fn keyboard_rec_preview_notes_audible() {
        let mut eng = Engine::new_demo(48000);
        eng.load_plugin_to_sound(0, 0, 1).unwrap(); // TB-303 sur le pad 0
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        let mut block = vec![0.0f32; 2048];
        let mut capture = |eng: &mut Engine| -> f32 {
            let mut peak = 0.0f32;
            for _ in 0..50 {
                eng.advance(1024);
                block.fill(0.0);
                eng.render_audio(&mut block);
                peak = peak.max(block.iter().map(|s| s.abs()).fold(0.0f32, f32::max));
            }
            peak
        };
        // Première note : maintenue longtemps (pattern vide).
        eng.handle_event(EngineEvent::KeyboardTrigger {
            note: 60,
            velocity: 110,
            pressed: true,
        })
        .unwrap();
        let peak1 = capture(&mut eng);
        eng.handle_event(EngineEvent::KeyboardTrigger {
            note: 60,
            velocity: 0,
            pressed: false,
        })
        .unwrap();
        // Avancer jusqu'au bouclage du pattern : la note enregistrée (tick 0)
        // rejoue pendant qu'on tient une nouvelle note en direct.
        eng.advance(144_000);
        eng.handle_event(EngineEvent::KeyboardTrigger {
            note: 62,
            velocity: 110,
            pressed: true,
        })
        .unwrap();
        let peak2 = capture(&mut eng);
        eng.handle_event(EngineEvent::KeyboardTrigger {
            note: 62,
            velocity: 0,
            pressed: false,
        })
        .unwrap();
        println!("keyboard REC: peak1={peak1} peak2={peak2}");
    }

    #[test]
    fn rec_pattern_does_not_cut_live_synth_voice() {
        let mut eng = Engine::new_demo(48000);
        eng.clear_bar(0);
        eng.clear_bar(1);
        eng.load_plugin_to_sound(0, 0, 1).unwrap(); // TB-303 sur le pad 0
        eng.handle_event(EngineEvent::SelectSound(0)).unwrap();
        // Un événement du pattern sur le même Sound, au tick 0, court.
        {
            let mut guard = eng.project.write();
            let proj = project_mut(&mut guard);
            let pi = proj.active_pattern_index(0, eng.active_scene).unwrap();
            proj.groups[0].patterns[pi].events.push(PatternEvent {
                pad: 0,
                tick: 0,
                velocity: 100,
                length_ticks: 120,
                pitch: 36,
            });
        }
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        // Tenir une note live sur ce même Sound pendant REC.
        eng.handle_event(EngineEvent::KeyboardTrigger {
            note: 60,
            velocity: 110,
            pressed: true,
        })
        .unwrap();
        // Rendre au-delà du note_off du pattern (tick 120).
        let mut block = vec![0.0f32; 2048];
        let mut pattern_was_playing = false;
        for n in 0..100 {
            eng.advance(1024);
            block.fill(0.0);
            eng.render_audio(&mut block);
            if n < 4 {
                pattern_was_playing |= eng.rack.acid[0]
                    .as_ref()
                    .is_some_and(|s| s.synth.is_active());
            }
        }
        assert!(pattern_was_playing, "le pattern doit sonner pendant REC");
        assert!(
            eng.monitor_rack.acid[0]
                .as_ref()
                .is_some_and(|s| s.synth.is_active()),
            "la note live doit rester active malgré la lecture du pattern pendant REC"
        );
    }

    #[test]
    fn rec_notes_are_pushed_to_the_undo_history() {
        let mut eng = Engine::new_demo(48000);
        eng.clear_bar(0);
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        let before = eng.undo_history.len();
        eng.handle_event(EngineEvent::Pad {
            pad: 5,
            velocity: 90,
            pressed: true,
        })
        .unwrap();
        assert!(
            eng.undo_history.len() > before,
            "les notes enregistrées poussent l'historique (Shift + Undo)"
        );
        eng.handle_event(EngineEvent::Undo).unwrap();
        let has_note = eng.project_handle().read().groups[0].patterns[0]
            .events
            .iter()
            .any(|e| e.pad == 5);
        assert!(!has_note, "Shift + Undo annule la note enregistrée");
    }

    #[test]
    fn rec_pad_notes_keep_the_held_length() {
        let mut eng = Engine::new_demo(48000);
        eng.clear_bar(0);
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        assert_eq!(eng.transport.state, TransportState::Recording);
        // Appui long sur le pad 0, 2 temps de tenue.
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 90,
            pressed: true,
        })
        .unwrap();
        // 2 temps : 48000 échantillons à 120 bpm (960 ticks/temps).
        let t0 = eng.transport.tick;
        eng.advance(48_000);
        let hold = eng.transport.tick - t0;
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 0,
            pressed: false,
        })
        .unwrap();
        let guard = eng.project_handle();
        let proj = guard.read();
        let ev = proj.groups[0].patterns[0]
            .events
            .iter()
            .find(|e| e.pad == 0)
            .expect("la note enregistrée doit exister");
        assert_eq!(ev.velocity, 90);
        // La longueur suit la note tenue, pas le 120 par défaut.
        assert_eq!(ev.length_ticks, hold as u32);
    }

    #[test]
    fn rec_keyboard_notes_keep_the_held_length() {
        let mut eng = Engine::new_demo(48000);
        eng.clear_bar(0);
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        eng.handle_event(EngineEvent::KeyboardTrigger {
            note: 60,
            velocity: 80,
            pressed: true,
        })
        .unwrap();
        let t0 = eng.transport.tick;
        eng.advance(48_000);
        let hold = eng.transport.tick - t0;
        eng.handle_event(EngineEvent::KeyboardTrigger {
            note: 60,
            velocity: 0,
            pressed: false,
        })
        .unwrap();
        let guard = eng.project_handle();
        let proj = guard.read();
        let ev = proj.groups[0].patterns[0]
            .events
            .iter()
            .find(|e| e.pitch == 60 && e.velocity == 80)
            .expect("la note enregistrée doit exister");
        assert_eq!(ev.length_ticks, hold as u32);
    }

    #[test]
    fn rec_plugin_knob_turns_record_automations() {
        let mut eng = Engine::new_demo(48000);
        eng.load_plugin_to_sound(0, 0, 1).unwrap(); // TB-303 sur le pad 0
        eng.handle_event(EngineEvent::SetLeftTab(4)).unwrap(); // Onglet PLUGIN
        eng.handle_event(EngineEvent::ToggleAutomation).unwrap(); // Bouton AUTO
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        assert_eq!(eng.transport.state, TransportState::Recording);
        eng.handle_event(EngineEvent::PluginKnob { knob: 2, delta: 64 })
            .unwrap();
        let guard = eng.project_handle();
        let proj = guard.read();
        let pat = &proj.groups[0].patterns[0];
        assert!(
            pat.automations
                .iter()
                .any(|a| a.sound == Some(0) && a.knob == 2),
            "l'automation du knob doit être enregistrée pendant REC"
        );
    }

    #[test]
    fn rec_live_note_off_is_quantized() {
        let mut eng = Engine::new_demo(48000);
        eng.load_plugin_to_sound(0, 0, 1).unwrap();
        eng.handle_event(EngineEvent::SelectSound(0)).unwrap();
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        eng.handle_event(EngineEvent::KeyboardTrigger {
            note: 60,
            velocity: 110,
            pressed: true,
        })
        .unwrap();
        eng.advance(1200);
        eng.handle_event(EngineEvent::KeyboardTrigger {
            note: 60,
            velocity: 0,
            pressed: false,
        })
        .unwrap();
        assert!(
            eng.pending_note_offs.iter().any(Option::is_some),
            "la fin de note est programmée à la fin quantifiée"
        );
        for _ in 0..40 {
            eng.advance(240);
        }
        assert!(
            eng.pending_note_offs.iter().all(Option::is_none),
            "la note live est coupée à la fin quantifiée"
        );
    }

    #[test]
    fn auto_button_arms_automation_recording() {
        let mut eng = Engine::new_demo(48000);
        eng.load_plugin_to_sound(0, 0, 1).unwrap();
        eng.handle_event(EngineEvent::SetLeftTab(4)).unwrap();
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        // Sans AUTO, les knobs n'écrivent rien.
        eng.handle_event(EngineEvent::PluginKnob { knob: 2, delta: 64 })
            .unwrap();
        assert!(
            eng.project.read().groups[0].patterns[0]
                .automations
                .is_empty(),
            "AUTO éteint : aucune automation enregistrée"
        );
        // AUTO allumé : LED/vue et enregistrement des points.
        eng.handle_event(EngineEvent::ToggleAutomation).unwrap();
        assert!(eng.snapshot().automation_armed);
        assert!(eng.snapshot().automation_view);
        eng.handle_event(EngineEvent::PluginKnob { knob: 2, delta: 64 })
            .unwrap();
        assert!(
            !eng.project.read().groups[0].patterns[0]
                .automations
                .is_empty(),
            "AUTO allumé : l'automation est enregistrée"
        );
    }

    #[test]
    fn toggle_step_is_silent_and_single_press() {
        let mut eng = Engine::new_demo(48000);
        assert_eq!(eng.sampler.num_voices(), 0);
        // Case vide (pad 15 absent du groove démo) : pose le pas.
        eng.toggle_step(15, 0);
        // Silencieux : aucune voix créée (façon Maschine : pas d'aperçu).
        assert_eq!(eng.sampler.num_voices(), 0);
        {
            let guard = eng.project_handle();
            let proj = guard.read();
            let n = proj.groups[0].patterns[0]
                .events
                .iter()
                .filter(|e| e.pad == 15 && e.tick == 0)
                .count();
            assert_eq!(n, 1);
        }
        // Second appui : retire en une fois (pas de doublon, pas de 3e appui).
        eng.toggle_step(15, 0);
        {
            let guard = eng.project_handle();
            let proj = guard.read();
            assert!(proj.groups[0].patterns[0]
                .events
                .iter()
                .all(|e| e.pad != 15));
        }
        assert_eq!(eng.sampler.num_voices(), 0);
    }

    #[test]
    fn step_mode_edits_selected_step_and_scrolls_grid() {
        let mut eng = Engine::new_demo(48000);
        eng.clear_bar(0);
        eng.clear_bar(1);
        // Sélectionne le son 3 puis pose un pas sur son pas 0.
        eng.handle_event(EngineEvent::SelectSound(3)).unwrap();
        eng.toggle_step(3, 0);
        // Le pas touché devient le pas sélectionné (curseur + mesure).
        assert_eq!(eng.step_cursor, 0);
        assert_eq!(eng.selected_bar, 0);

        // Édition par les knobs 1-4 : VEL / POS / LEN / PITCH du pas sélectionné.
        eng.handle_event(EngineEvent::StepVelocityDelta(-15))
            .unwrap();
        eng.handle_event(EngineEvent::StepNudgeTicks(20)).unwrap();
        eng.handle_event(EngineEvent::StepLengthDelta(1)).unwrap();
        eng.handle_event(EngineEvent::StepPitchDelta(2)).unwrap();
        {
            let guard = eng.project_handle();
            let proj = guard.read();
            let ev = proj.groups[0].patterns[0]
                .events
                .iter()
                .find(|e| e.pad == 3 && e.tick < 240)
                .expect("le pas posé existe toujours");
            assert_eq!(ev.velocity, 100); // 115 - 15
            assert_eq!(ev.tick, 20); // nudge +20 ticks
            assert_eq!(ev.length_ticks, 360); // 120 + 240 (1 pas de 1/16)
            assert_eq!(ev.pitch, 41); // 36 + pad(3) + 2
        }

        // Scroll de la grille complète : le curseur avance, la mesure suit.
        eng.handle_event(EngineEvent::StepScroll(20)).unwrap();
        assert_eq!(eng.step_cursor, 20);
        assert_eq!(eng.selected_bar, 1);
        // Bornes : début et fin du pattern (2 mesures = 32 pas).
        eng.handle_event(EngineEvent::StepScroll(-100)).unwrap();
        assert_eq!(eng.step_cursor, 0);
        assert_eq!(eng.selected_bar, 0);
        eng.handle_event(EngineEvent::StepScroll(999)).unwrap();
        assert_eq!(eng.step_cursor, 31);
        assert_eq!(eng.selected_bar, 1);

        // Le curseur est exposé aux écrans / aux LEDs.
        assert_eq!(eng.hardware_snapshot().step_cursor, 31);

        // Pas vide : pas d'édition fantôme, pas de panic.
        eng.handle_event(EngineEvent::StepVelocityDelta(5)).unwrap();
        eng.handle_event(EngineEvent::StepPitchDelta(1)).unwrap();
    }

    #[test]
    fn swing_knob_goes_through_engine_and_syncs_sequencer() {
        let mut eng = Engine::new_demo(48000);
        let before = eng.revision;
        eng.handle_event(EngineEvent::Swing(0.25)).unwrap();
        assert!((eng.transport.swing - 0.25).abs() < 1e-6);
        assert!((eng.sequencer.swing - 0.25).abs() < 1e-6);
        assert!((eng.project.read().swing - 0.25).abs() < 1e-6);
        assert_ne!(eng.revision, before, "l'affichage doit être invalidé");
    }

    #[test]
    fn step_cursor_soft_keys_scroll_and_set_length() {
        let mut eng = Engine::new_demo(48000);
        eng.controller_mode = 3;
        eng.left_tab = 0;
        eng.handle_event(EngineEvent::SoftKey(2)).unwrap(); // PAS >
        assert_eq!(eng.step_cursor, 1);
        eng.handle_event(EngineEvent::SoftKey(4)).unwrap(); // BAR >
        assert_eq!(eng.step_cursor, 17);
        assert_eq!(eng.selected_bar, 1);
        eng.handle_event(EngineEvent::SoftKey(7)).unwrap(); // 4 MES
        assert_eq!(eng.hardware_snapshot().pattern_bars, 4);
    }

    /// Un bouton d'écran s'allume quand l'état qu'il sélectionne est courant,
    /// jamais parce que son index coïncide avec un autre état.
    #[test]
    fn lit_soft_keys_follow_their_labels() {
        let lit = |eng: &Engine| -> Vec<&'static str> {
            let labels = eng.soft_key_labels();
            let active = eng.soft_key_active();
            (0..8).filter(|&i| active[i]).map(|i| labels[i]).collect()
        };
        let mut eng = Engine::new_demo(48000);
        let bars = eng.active_pattern_bars();
        assert_eq!(
            lit(&eng),
            vec!["BAR 1", if bars == 2 { "2 MES" } else { "1 MES" }]
        );

        eng.handle_event(EngineEvent::SetLeftTab(1)).unwrap();
        eng.handle_event(EngineEvent::SetMixerMode(0)).unwrap();
        eng.handle_event(EngineEvent::SelectSound(3)).unwrap();
        assert_eq!(
            lit(&eng),
            vec!["PAD 1-8"],
            "pas MASTER parce que le pad 4 est actif"
        );

        eng.handle_event(EngineEvent::SetMixerMode(2)).unwrap();
        eng.handle_event(EngineEvent::GroupFocus(2)).unwrap();
        assert_eq!(lit(&eng), vec!["GRP C"]);

        // SAMPLING : seule la page courante de l'éditeur est allumée.
        eng.handle_event(EngineEvent::SetLeftTab(2)).unwrap();
        assert_eq!(lit(&eng), vec!["EDIT"]);
    }

    #[test]
    fn toggle_step_removes_humanized_note_in_one_press() {
        let mut eng = Engine::new_demo(48000);
        eng.clear_bar(0);
        eng.clear_bar(1);
        eng.toggle_step(0, 0);
        // Humanize décale de ±12 ticks : l'ancienne comparaison exacte
        // `e.tick == tick` ratait la note et forçait des appuis multiples.
        eng.humanize_active_pattern();
        eng.toggle_step(0, 0);
        let guard = eng.project_handle();
        let proj = guard.read();
        assert!(
            proj.groups[0].patterns[0]
                .events
                .iter()
                .all(|e| !(e.pad == 0 && crate::project::same_step_cell(e.tick, 0))),
            "la note humanisée aurait dû être retirée en un seul appui"
        );
    }

    #[test]
    fn toggle_step_edits_scene_assigned_pattern() {
        let mut eng = Engine::new_demo(48000);
        // 2e pattern vide assigné à la scène courante pour le groupe 0.
        {
            let h = eng.project_handle();
            let mut proj = h.write();
            proj.groups[0].patterns.push(crate::project::Pattern::new(
                "Vide",
                crate::project::BarsBeatsTicks::new(2, 0, 0),
            ));
            proj.scenes[0].patterns[0] = Some(1);
        }
        eng.toggle_step(15, 0);
        let guard = eng.project_handle();
        let proj = guard.read();
        assert!(
            proj.groups[0].patterns[0]
                .events
                .iter()
                .all(|e| e.pad != 15),
            "le pattern 0 (non joué) ne doit pas être modifié"
        );
        assert_eq!(proj.groups[0].patterns[1].events.len(), 1);
    }

    #[test]
    fn engine_pad_triggers_sampler() {
        let mut eng = Engine::new_demo(48000);
        // ajoute un sample dummy au sound 0
        let samp = std::sync::Arc::new(vec![0.5; 4800]);
        eng.sampler_mut().add_zone(
            0,
            crate::sampler::ZoneMapping {
                sample: Some(samp),
                sample_path: None,
                sample_rate: 48000,
                root_key: 36,
                lokey: 0,
                hikey: 127,
                lovel: 0,
                hivel: 127,
                volume_db: 0.0,
                pan: 0.0,
                tune_cents: 0,
                choke_group: None,
                ..Default::default()
            },
        );
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 100,
            pressed: true,
        })
        .unwrap();
        assert_eq!(eng.sampler.num_voices(), 1);
    }

    #[test]
    fn sound_and_group_mix_edits_reach_active_voices_immediately() {
        let mut engine = Engine::new_demo(48000);
        engine
            .handle_event(EngineEvent::Pad {
                pad: 0,
                velocity: 100,
                pressed: true,
            })
            .unwrap();
        let initial_gain = engine.sampler.voice_mix(0).unwrap().0;

        engine
            .handle_event(EngineEvent::SoundVolume {
                group: 0,
                sound: 0,
                volume_db: -6.0,
            })
            .unwrap();
        let expected_after_sound = initial_gain * 10.0_f32.powf(-6.0 / 20.0);
        assert!((engine.sampler.voice_mix(0).unwrap().0 - expected_after_sound).abs() < 1e-5);

        engine
            .handle_event(EngineEvent::GroupVolume {
                group: 0,
                volume_db: 6.0,
            })
            .unwrap();
        let expected_after_group = expected_after_sound * 10.0_f32.powf(6.0 / 20.0);
        assert!((engine.sampler.voice_mix(0).unwrap().0 - expected_after_group).abs() < 1e-5);

        engine
            .handle_event(EngineEvent::SoundPan {
                group: 0,
                sound: 0,
                pan: 0.3,
            })
            .unwrap();
        engine
            .handle_event(EngineEvent::GroupPan { group: 0, pan: 0.4 })
            .unwrap();
        assert!((engine.sampler.voice_mix(0).unwrap().1 - 0.7).abs() < 1e-5);
    }

    #[test]
    fn engine_transport() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: false,
            stop: false,
        })
        .unwrap();
        assert!(eng.transport.is_playing());
        eng.advance(4800); // 100ms
        assert!(eng.transport.tick > 0);
    }

    /// Les notes démarrent à leur vraie position dans le buffer audio (et non
    /// au début du bloc) : c'est ce qui garde le groove serré même avec un
    /// buffer de plusieurs millisecondes.
    #[test]
    fn notes_start_at_their_position_inside_the_block() {
        let mut eng = Engine::new_demo(48000);
        eng.clear_bar(0);
        eng.clear_bar(1);
        // La démo répartit les notes sur plusieurs Groupes : on part d'un
        // pattern totalement vide pour isoler la position du kick.
        {
            let handle = eng.project_handle();
            let mut proj = handle.write();
            for group in &mut proj.groups {
                for pattern in &mut group.patterns {
                    pattern.events.clear();
                }
            }
        }
        // 150 BPM à 48 kHz : 20 échantillons par tick (blocs de 1000 = 50 ticks).
        eng.handle_event(EngineEvent::Bpm(150.0)).unwrap();
        eng.toggle_step(0, 240); // kick au tick 240
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: false,
            stop: false,
        })
        .unwrap();

        let mut buf = vec![0.0f32; 1000 * 2];
        let mut first: Option<(usize, usize)> = None;
        for block in 0..6 {
            eng.advance(1000);
            eng.render_audio(&mut buf);
            if first.is_none() {
                if let Some(pos) = buf
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .position(|f| f[0].abs() > 2e-3 || f[1].abs() > 2e-3)
                {
                    first = Some((block, pos));
                }
            }
        }
        let (block, pos) = first.expect("kick non entendu");
        assert_eq!(block, 4, "kick déclenché dans le mauvais bloc");
        // Le limiteur Master ajoute une latence fixe (anticipation de 1 ms).
        let expected = 800 + eng.master_limiter.latency_frames() as i64;
        assert!(
            (pos as i64 - expected).abs() < 32,
            "kick à la frame {pos} au lieu de {expected} (position dans le bloc)"
        );
    }

    #[test]
    fn shift_rec_counts_in_one_bar_of_clicks_then_records() {
        // Projet vide, métronome coupé : seuls les clics du pré-compte sonnent.
        let mut eng = Engine::new(Project::default(), 48000);
        eng.handle_event(EngineEvent::CountInRecord).unwrap();
        assert!(eng.hardware_snapshot().count_in);
        let mut buf = vec![0.0f32; 1024 * 2];
        let mut mx = 0.0f32;
        // Une mesure à 120 BPM = 96 000 samples ; 93 blocs n'y arrivent pas.
        for _ in 0..93 {
            eng.advance(1024);
            eng.render_audio(&mut buf);
            mx = buf.iter().fold(mx, |a, b| a.max(b.abs()));
        }
        assert!(mx > 1e-4, "pré-compte silencieux, max={mx}");
        assert!(!eng.transport.is_playing(), "REC démarre après la mesure");
        eng.advance(1024);
        assert_eq!(eng.transport.state, TransportState::Recording);
        assert!(!eng.hardware_snapshot().count_in);
        // STOP annule un pré-compte en cours.
        let stop = EngineEvent::Transport {
            play: false,
            rec: false,
            stop: true,
        };
        eng.handle_event(stop.clone()).unwrap();
        eng.handle_event(EngineEvent::CountInRecord).unwrap();
        eng.handle_event(stop).unwrap();
        eng.advance(100_000);
        assert!(!eng.transport.is_playing());
        // PLAY pendant le pré-compte : lecture simple, sans REC surprise.
        eng.handle_event(EngineEvent::CountInRecord).unwrap();
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: false,
            stop: false,
        })
        .unwrap();
        eng.advance(100_000);
        assert_eq!(eng.transport.state, TransportState::Playing);
        eng.handle_event(EngineEvent::Transport {
            play: false,
            rec: false,
            stop: true,
        })
        .unwrap();
        // COUNT-IN activé : REC simple à l'arrêt passe aussi par le pré-compte.
        eng.transport.count_in_bars = 1;
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        assert!(!eng.transport.is_playing());
        assert!(eng.hardware_snapshot().count_in);
    }

    #[test]
    fn note_repeat_in_keyboard_mode_arpeggiates_held_notes_upwards() {
        let mut eng = Engine::new_demo(48000);
        eng.controller_mode = 1;
        eng.handle_event(EngineEvent::SetNoteRepeat(true)).unwrap();
        eng.handle_event(EngineEvent::SetNoteRepeatRate(1)).unwrap(); // 1/16
        let key = |eng: &mut Engine, note: u8, pressed: bool| {
            eng.handle_event(EngineEvent::KeyboardTrigger {
                note,
                velocity: 100,
                pressed,
            })
            .unwrap();
        };
        // FIXED VEL s'applique aussi au clavier.
        eng.fixed_velocity = true;
        eng.fixed_velocity_level = 127;
        // La première touche part tout de suite, la seconde entre dans le cycle.
        key(&mut eng, 64, true);
        key(&mut eng, 60, true);
        assert_eq!(eng.arp_last, Some(64));
        assert_eq!(eng.arp_held[64], 127);
        // 1/16 à 120 BPM = 6000 samples : E3 → C3 → E3 → C3 (en montant, en boucle).
        let mut seq = Vec::new();
        for _ in 0..3 {
            eng.advance(6000);
            seq.push(eng.arp_last);
        }
        assert_eq!(seq, vec![Some(60), Some(64), Some(60)]);
        // Toutes les touches relâchées : la note en cours s'arrête.
        key(&mut eng, 64, false);
        key(&mut eng, 60, false);
        assert_eq!(eng.arp_last, None);
        eng.advance(6000);
        assert_eq!(eng.arp_last, None);
    }

    #[test]
    fn notes_repeated_by_the_audio_thread_are_written_outside_it() {
        let mut eng = Engine::new_demo(48000);
        let count = |eng: &Engine| {
            let proj = eng.project.read();
            let idx = proj.active_pattern_index(0, eng.active_scene).unwrap();
            proj.groups[0].patterns[idx]
                .events
                .iter()
                .filter(|e| e.pad == 3)
                .count()
        };
        eng.handle_event(EngineEvent::SetNoteRepeat(true)).unwrap();
        eng.handle_event(EngineEvent::SetNoteRepeatRate(1)).unwrap();
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        eng.handle_event(EngineEvent::Pad {
            pad: 3,
            velocity: 100,
            pressed: true,
        })
        .unwrap();
        let before = count(&eng);
        // Un pas de 1/16 : la note répétée est jouée et mise en file…
        eng.advance(6000);
        assert_eq!(
            count(&eng),
            before,
            "le callback audio n'écrit pas le projet"
        );
        // … puis écrite hors du thread audio.
        eng.commit_launches();
        assert_eq!(count(&eng), before + 1);
    }

    #[test]
    fn arp_modes_go_down_bounce_and_add_the_octave() {
        let run = |mode: u8, steps: usize| -> Vec<u8> {
            let mut eng = Engine::new_demo(48000);
            eng.controller_mode = 1;
            eng.arp_mode = mode;
            eng.handle_event(EngineEvent::SetNoteRepeat(true)).unwrap();
            eng.handle_event(EngineEvent::SetNoteRepeatRate(1)).unwrap();
            for note in [60, 64, 67] {
                eng.handle_event(EngineEvent::KeyboardTrigger {
                    note,
                    velocity: 90,
                    pressed: true,
                })
                .unwrap();
            }
            let mut seq = vec![eng.arp_last.unwrap()];
            for _ in 0..steps {
                eng.advance(6000);
                seq.push(eng.arp_last.unwrap());
            }
            seq
        };
        assert_eq!(run(0, 4), [60, 64, 67, 60, 64], "HAUT");
        assert_eq!(run(1, 4), [60, 67, 64, 60, 67], "BAS");
        assert_eq!(run(2, 5), [60, 64, 67, 64, 60, 64], "HAUT-BAS");
        assert_eq!(
            run(3, 6),
            [60, 64, 67, 72, 76, 79, 60],
            "HAUT sur 2 octaves"
        );
    }

    #[test]
    fn sixteen_velocities_play_the_selected_sound_at_sixteen_levels() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::SelectSound(2)).unwrap();
        eng.handle_event(EngineEvent::ToggleSixteenVelocities)
            .unwrap();
        assert!(eng.snapshot().sixteen_velocities);
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: true,
            stop: false,
        })
        .unwrap();
        for pad in [0u8, 15] {
            eng.handle_event(EngineEvent::Pad {
                pad,
                velocity: 50,
                pressed: true,
            })
            .unwrap();
            eng.handle_event(EngineEvent::Pad {
                pad,
                velocity: 0,
                pressed: false,
            })
            .unwrap();
            eng.advance(3000);
        }
        let proj = eng.project.read();
        let idx = proj.active_pattern_index(0, eng.active_scene).unwrap();
        let recorded: Vec<(u8, u8)> = proj.groups[0].patterns[idx]
            .events
            .iter()
            .filter(|e| e.velocity == 7 || e.velocity == 127)
            .map(|e| (e.pad, e.velocity))
            .collect();
        drop(proj);
        // Pad 1 = la plus douce, pad 16 = la plus forte, toujours le Sound 3.
        assert!(
            recorded.contains(&(2, 7)) && recorded.contains(&(2, 127)),
            "{recorded:?}"
        );
        assert_eq!(eng.snapshot().active_sound, 2);
        // FIXED VEL et 16 VEL s'excluent.
        eng.handle_event(EngineEvent::SetFixedVelocity(true))
            .unwrap();
        assert!(!eng.snapshot().sixteen_velocities);
    }

    #[test]
    fn metronome_click_audible() {
        // Projet vide : seul le métronome doit produire du son.
        let mut eng = Engine::new(Project::default(), 48000);
        eng.handle_event(EngineEvent::ToggleMetronome).unwrap();
        assert!(eng.transport.metronome);
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: false,
            stop: false,
        })
        .unwrap();
        // Avance d'un temps (0.5s à 120BPM) par petits blocs, comme le
        // callback audio (advance + render du même bloc) : au moins un clic.
        let mut buf = vec![0.0f32; 1024 * 2];
        let mut mx = 0.0f32;
        for _ in 0..24 {
            eng.advance(1024);
            eng.render_audio(&mut buf);
            mx = buf.iter().fold(mx, |a, b| a.max(b.abs()));
        }
        assert!(mx > 1e-4, "metronome silencieux, max={mx}");
    }

    #[test]
    fn sound_filter_adsr_audible() {
        let mut eng = Engine::new_demo(48000);
        // Les charleys sont dans le Groupe B (famille « Snare & Cymbals »).
        eng.handle_event(EngineEvent::GroupFocus(1)).unwrap();
        // Coupe franche : hi-hat fermé à 200Hz doit perdre ses aigus.
        eng.handle_event(EngineEvent::SoundFilterCutoff {
            group: 1,
            sound: 2,
            cutoff_hz: 200.0,
        })
        .unwrap();
        eng.handle_event(EngineEvent::Pad {
            pad: 2,
            velocity: 127,
            pressed: true,
        })
        .unwrap();
        let mut dark = vec![0.0f32; 2048 * 2];
        eng.render_audio(&mut dark);
        // Rouvre le filtre et rejoue.
        eng.handle_event(EngineEvent::SoundFilterCutoff {
            group: 1,
            sound: 2,
            cutoff_hz: 20000.0,
        })
        .unwrap();
        eng.handle_event(EngineEvent::Pad {
            pad: 2,
            velocity: 127,
            pressed: true,
        })
        .unwrap();
        let mut open = vec![0.0f32; 2048 * 2];
        eng.render_audio(&mut open);
        // Le chapeau contient des aigus : sans filtre, plus d'énergie HF.
        // Approximation : compte les passages par zéro (richesse HF).
        fn zc(buf: &[f32]) -> usize {
            buf.iter()
                .skip(2)
                .step_by(2)
                .zip(buf.iter().skip(4).step_by(2))
                .filter(|(a, b)| (**a <= 0.0) != (**b <= 0.0))
                .count()
        }
        let (e_dark, e_open): (f32, f32) = (
            dark.iter().map(|x| x.abs()).sum(),
            open.iter().map(|x| x.abs()).sum(),
        );
        assert!(e_dark > 1e-4 && e_open > 1e-4, "rendu silencieux");
        assert!(
            zc(&open) > zc(&dark),
            "le cutoff ne filtre rien (zc open={} dark={})",
            zc(&open),
            zc(&dark)
        );
        let _ = e_dark;
    }

    #[test]
    fn bar_navigation_and_pattern_length() {
        let mut eng = Engine::new_demo(48000);
        let snap = eng.snapshot();
        assert_eq!(snap.selected_bar, 0);
        assert_eq!(snap.pattern_bars, 2);

        // Sélection directe mesure 1
        eng.handle_event(EngineEvent::SelectBar(1)).unwrap();
        assert_eq!(eng.snapshot().selected_bar, 1);

        // NextBar / PrevBar
        eng.handle_event(EngineEvent::NextBar).unwrap();
        // Limité par pattern_bars (2) -> max bar index est 1
        assert_eq!(eng.snapshot().selected_bar, 1);

        eng.handle_event(EngineEvent::PrevBar).unwrap();
        assert_eq!(eng.snapshot().selected_bar, 0);

        // Étend le pattern à 4 mesures
        eng.handle_event(EngineEvent::SetPatternBars(4)).unwrap();
        assert_eq!(eng.snapshot().pattern_bars, 4);

        // Maintenant NextBar peut aller jusqu'à mesure 3 (0-indexed)
        eng.handle_event(EngineEvent::SelectBar(3)).unwrap();
        assert_eq!(eng.snapshot().selected_bar, 3);
    }

    #[test]
    fn pattern_crud_create_duplicate_clear_delete() {
        let mut eng = Engine::new_demo(48000);
        let snap = eng.snapshot();
        let group = snap.group_focus as usize;
        let initial_len = snap.project.groups[group].patterns.len();
        assert!(initial_len >= 1);

        // Création : le nouveau pattern est ajouté à la suite et assigné à la
        // scène active.
        eng.handle_event(EngineEvent::AddPattern { group: group as u8 })
            .unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.project.groups[group].patterns.len(), initial_len + 1);
        let active = snap
            .project
            .active_pattern_index(group, snap.active_scene)
            .unwrap();
        assert_eq!(active, initial_len);
        assert_eq!(
            snap.project.scenes[snap.active_scene.unwrap()].patterns[group],
            Some(active)
        );

        // Renommage.
        eng.handle_event(EngineEvent::RenamePattern {
            group: group as u8,
            pattern: active,
            name: "  Mon pattern  ".to_string(),
        })
        .unwrap();
        let snap = eng.snapshot();
        assert_eq!(
            snap.project.groups[group].patterns[active].name,
            "Mon pattern"
        );

        // Duplication : copie insérée juste après, sélectionnée.
        eng.handle_event(EngineEvent::DuplicatePattern {
            group: group as u8,
            pattern: active,
        })
        .unwrap();
        let snap = eng.snapshot();
        let copy = active + 1;
        assert_eq!(snap.project.groups[group].patterns.len(), initial_len + 2);
        assert!(snap.project.groups[group].patterns[copy]
            .name
            .contains("copie"));
        assert_eq!(
            snap.project
                .active_pattern_index(group, snap.active_scene)
                .unwrap(),
            copy
        );

        // Effacement des événements sans changer la longueur.
        eng.handle_event(EngineEvent::ClearPattern {
            group: group as u8,
            pattern: copy,
        })
        .unwrap();
        let snap = eng.snapshot();
        assert!(snap.project.groups[group].patterns[copy].events.is_empty());

        // Suppression : les références de scène restent valides.
        eng.handle_event(EngineEvent::DeletePattern {
            group: group as u8,
            pattern: copy,
        })
        .unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.project.groups[group].patterns.len(), initial_len + 1);
        for scene in &snap.project.scenes {
            if let Some(pi) = scene.patterns[group] {
                assert!(pi < snap.project.groups[group].patterns.len());
            }
        }

        // Le dernier pattern du groupe ne peut pas être supprimé.
        while eng.snapshot().project.groups[group].patterns.len() > 1 {
            let active = eng
                .snapshot()
                .project
                .active_pattern_index(group, eng.snapshot().active_scene)
                .unwrap();
            eng.handle_event(EngineEvent::DeletePattern {
                group: group as u8,
                pattern: active,
            })
            .unwrap();
        }
        eng.handle_event(EngineEvent::DeletePattern {
            group: group as u8,
            pattern: 0,
        })
        .unwrap();
        assert_eq!(eng.snapshot().project.groups[group].patterns.len(), 1);
    }

    #[test]
    fn pattern_insert_delete_only_reindexes_the_target_group() {
        let mut eng = Engine::new_demo(48000);
        {
            let mut guard = eng.project.write();
            let proj = project_mut(&mut guard);
            proj.groups.truncate(1);
            // Deuxième groupe avec 3 patterns.
            let mut group_b = proj.groups[0].clone();
            group_b.name = "Groupe B".to_string();
            group_b
                .patterns
                .push(Pattern::new("B2", BarsBeatsTicks::new(2, 0, 0)));
            group_b
                .patterns
                .push(Pattern::new("B3", BarsBeatsTicks::new(2, 0, 0)));
            proj.groups.push(group_b);

            // Scène 0 : A=P1, B=P3. Scène 1 : A=P2, B=P3.
            proj.scenes[0].patterns = vec![Some(0), Some(2)];
            let mut scene2 = Scene::new("Scene 2", 2);
            scene2.patterns = vec![Some(1), Some(2)];
            proj.scenes.push(scene2);
        }
        eng.active_scene = Some(0);

        // Insère une copie de P1 dans le groupe A : seule la référence A de la
        // scène 1 doit être décalée, la référence du groupe B ne bouge pas.
        eng.handle_event(EngineEvent::DuplicatePattern {
            group: 0,
            pattern: 0,
        })
        .unwrap();
        {
            let snap = eng.snapshot();
            // La scène active sélectionne la copie (index 1), l'autre scène
            // décale sa référence A de 1 vers 2 ; B reste à 2.
            assert_eq!(snap.project.scenes[0].patterns, vec![Some(1), Some(2)]);
            assert_eq!(snap.project.scenes[1].patterns, vec![Some(2), Some(2)]);
            assert_eq!(snap.project.groups[0].patterns.len(), 2);
        }

        // Supprime la copie : la référence A de la scène 1 revient à P2.
        eng.handle_event(EngineEvent::DeletePattern {
            group: 0,
            pattern: 1,
        })
        .unwrap();
        {
            let snap = eng.snapshot();
            assert_eq!(snap.project.scenes[0].patterns, vec![Some(0), Some(2)]);
            assert_eq!(snap.project.scenes[1].patterns, vec![Some(1), Some(2)]);
            assert_eq!(snap.project.groups[0].patterns.len(), 1);
        }
    }

    #[test]
    fn pattern_create_at_slot_fills_intermediate_slots_and_selects() {
        let mut eng = Engine::new_demo(48000);
        let group = eng.snapshot().group_focus as usize;
        eng.handle_event(EngineEvent::CreatePatternAt {
            group: group as u8,
            pattern: 4,
        })
        .unwrap();
        let snap = eng.snapshot();
        assert!(snap.project.groups[group].patterns.len() >= 5);
        assert_eq!(
            snap.project
                .active_pattern_index(group, snap.active_scene)
                .unwrap(),
            4
        );
        assert_eq!(
            snap.project.scenes[snap.active_scene.unwrap()].patterns[group],
            Some(4)
        );
    }

    #[test]
    fn new_patterns_are_empty_and_keep_previous_patterns_intact() {
        let mut eng = Engine::new_demo(48000);
        let group = 0;
        eng.handle_event(EngineEvent::GroupFocus(group)).unwrap();

        // Ajout d'événements sur Pad 0 (Kick), Pad 1 (Snare) et Pad 2 (Hihat) dans Pattern 0
        {
            let mut guard = eng.project.write();
            let proj = project_mut(&mut guard);
            let pat = &mut proj.groups[group as usize].patterns[0];
            pat.events.clear();
            pat.events.push(PatternEvent {
                pad: 0,
                tick: 0,
                velocity: 100,
                length_ticks: 240,
                pitch: 36,
            });
            pat.events.push(PatternEvent {
                pad: 1,
                tick: 960,
                velocity: 110,
                length_ticks: 240,
                pitch: 38,
            });
            pat.events.push(PatternEvent {
                pad: 2,
                tick: 480,
                velocity: 90,
                length_ticks: 120,
                pitch: 42,
            });
        }

        // On sélectionne le Pad 0 (Kick)
        eng.handle_event(EngineEvent::SelectSound(0)).unwrap();
        assert_eq!(eng.active_sound, 0);

        // Création d'un nouveau pattern via AddPattern
        eng.handle_event(EngineEvent::AddPattern { group }).unwrap();

        let snap = eng.snapshot();
        let active_pat_idx = snap
            .project
            .active_pattern_index(group as usize, snap.active_scene)
            .unwrap();
        let pat1 = &snap.project.groups[group as usize].patterns[active_pat_idx];

        // Comme sur Maschine : le nouveau pattern est totalement vide, aucune
        // piste du pattern précédent n'est recopiée.
        assert!(pat1.events.is_empty());
        // Le pattern précédent du Groupe reste intact.
        assert_eq!(
            snap.project.groups[group as usize].patterns[0].events.len(),
            3
        );

        // Maintenant on sélectionne le Pad 1 (Snare) et on crée un pattern au slot 2 via CreatePatternAt
        eng.handle_event(EngineEvent::SelectSound(1)).unwrap();
        eng.handle_event(EngineEvent::CreatePatternAt { group, pattern: 2 })
            .unwrap();

        let snap2 = eng.snapshot();
        let active_pat2_idx = snap2
            .project
            .active_pattern_index(group as usize, snap2.active_scene)
            .unwrap();
        let pat2 = &snap2.project.groups[group as usize].patterns[active_pat2_idx];

        // Idem pour un pattern créé sur un slot précis : il est vide.
        assert!(pat2.events.is_empty());
    }

    #[test]
    fn pattern_step_wraps_and_pattern_soft_keys_are_exposed() {
        let mut eng = Engine::new_demo(48000);
        let group = eng.snapshot().group_focus;
        eng.handle_event(EngineEvent::SetPatternMode(true)).unwrap();
        assert_eq!(
            eng.soft_key_labels(),
            ["PAT <", "PAT >", "+ PAT", "DUPL", "VIDER", "SUPPR", "1 MES", "2 MES"]
        );

        // Ajoute un pattern : le pattern actif est le dernier. +1 boucle sur 0.
        eng.handle_event(EngineEvent::AddPattern { group }).unwrap();
        let len = eng.snapshot().project.groups[group as usize].patterns.len();
        assert!(len > 1);
        eng.handle_event(EngineEvent::StepPattern { group, delta: 1 })
            .unwrap();
        let snap = eng.snapshot();
        assert_eq!(
            snap.project
                .active_pattern_index(group as usize, snap.active_scene)
                .unwrap(),
            0
        );

        // Soft key 3 = + PAT : crée et sélectionne un pattern.
        eng.handle_event(EngineEvent::SoftKey(3)).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.project.groups[group as usize].patterns.len(), len + 1);
        assert_eq!(
            snap.project
                .active_pattern_index(group as usize, snap.active_scene)
                .unwrap(),
            len
        );
    }

    #[test]
    fn section_workflow_duplicate_resize_delete() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::AddSection).unwrap();
        eng.handle_event(EngineEvent::AddSection).unwrap();
        let n = eng.snapshot().project.song.len();
        assert!(n >= 2);

        // Duplique la section active (insérée juste après, sélectionnée).
        eng.handle_event(EngineEvent::DuplicateSection).unwrap();
        assert_eq!(eng.snapshot().project.song.len(), n + 1);

        // Redimensionne la section active.
        eng.handle_event(EngineEvent::ResizeSection { bars: 4 })
            .unwrap();
        let snap = eng.snapshot();
        let idx = snap.active_section.unwrap();
        assert_eq!(snap.project.song[idx].length.bars, 4);

        // Supprime la section active.
        eng.handle_event(EngineEvent::DeleteSection).unwrap();
        assert_eq!(eng.snapshot().project.song.len(), n);

        // Supprime toutes les sections : plus de section active, pas de panic.
        while !eng.snapshot().project.song.is_empty() {
            eng.handle_event(EngineEvent::DeleteSection).unwrap();
        }
        assert_eq!(eng.snapshot().active_section, None);
    }

    #[test]
    fn scene_duplicate_unique_creates_independent_patterns() {
        let mut eng = Engine::new_demo(48000);
        let group_idx = 0usize;

        // Scène 0 active avec pattern 0
        assert_eq!(eng.snapshot().active_scene, Some(0));
        let init_pat_len = eng.snapshot().project.groups[group_idx].patterns.len();

        // Dupliquer la scène en mode UNIQUE
        eng.handle_event(EngineEvent::DuplicateSceneUnique).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.active_scene, Some(1));
        assert_eq!(snap.project.scenes.len(), 2);
        // Le groupe 0 doit maintenant avoir un pattern supplémentaire (clone indépendant)
        let new_pat_len = snap.project.groups[group_idx].patterns.len();
        assert_eq!(new_pat_len, init_pat_len + 1);

        // La nouvelle scène pointe sur le nouveau pattern
        let scene1_pat = snap.project.scenes[1].patterns[group_idx];
        assert_eq!(scene1_pat, Some(new_pat_len - 1));
        // L'ancienne scène pointe toujours sur l'original
        let scene0_pat = snap.project.scenes[0].patterns[group_idx];
        assert_ne!(scene1_pat, scene0_pat);

        // MakeSceneUnique sur scène 0
        eng.handle_event(EngineEvent::Scene(0)).unwrap();
        eng.handle_event(EngineEvent::MakeSceneUnique(0)).unwrap();
        let snap2 = eng.snapshot();
        assert_eq!(
            snap2.project.groups[group_idx].patterns.len(),
            new_pat_len + 1
        );
    }

    #[test]
    fn scene_create_at_pads_and_step_navigation() {
        let mut eng = Engine::new_demo(48000);
        // Créer slot scène 3 (équivalent pad 4 en mode SCENE)
        eng.handle_event(EngineEvent::CreateSceneAt(3)).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.project.scenes.len(), 4);
        assert_eq!(snap.active_scene, Some(3));

        // Navigation pas-à-pas SceneStep (+1 et -1)
        eng.handle_event(EngineEvent::SceneStep(-1)).unwrap();
        assert_eq!(eng.snapshot().active_scene, Some(2));
        eng.handle_event(EngineEvent::SceneStep(1)).unwrap();
        assert_eq!(eng.snapshot().active_scene, Some(3));
        // Wrap
        eng.handle_event(EngineEvent::SceneStep(1)).unwrap();
        assert_eq!(eng.snapshot().active_scene, Some(0));

        // Enchaîner scènes 0, 1, 2
        eng.handle_event(EngineEvent::SceneChain(vec![0, 1, 2]))
            .unwrap();
        let snap_chain = eng.snapshot();
        assert!(snap_chain.song_mode);
        assert_eq!(snap_chain.project.song.len(), 3);
        assert_eq!(snap_chain.project.song[0].scene, 0);
        assert_eq!(snap_chain.project.song[1].scene, 1);
        assert_eq!(snap_chain.project.song[2].scene, 2);
    }

    #[test]
    fn scene_soft_keys_and_labels_workflow() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::SetSceneMode(true)).unwrap();
        assert_eq!(
            eng.soft_key_labels(),
            ["SCN <", "SCN >", "+ SCN", "DUPL", "UNIQUE", "SUPPR", "ENCHAIN", "SONG"]
        );

        // Soft key 3 = + SCN : ajoute une scène
        eng.handle_event(EngineEvent::SoftKey(3)).unwrap();
        assert_eq!(eng.snapshot().project.scenes.len(), 2);

        // Soft key 4 = DUPL : duplique
        eng.handle_event(EngineEvent::SoftKey(4)).unwrap();
        assert_eq!(eng.snapshot().project.scenes.len(), 3);

        // Avec Shift
        eng.handle_event(EngineEvent::SetShiftHeld(true)).unwrap();
        assert_eq!(
            eng.soft_key_labels(),
            [
                "SCN 1",
                "SCN DER",
                "TOUT UNI",
                "DUPL UNI",
                "VIDER",
                "SUPPR",
                "ENCH TOUT",
                "SONG"
            ]
        );
    }

    #[test]
    fn scene_workflow_duplicate_delete_reindexes_sections() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::AddScene).unwrap(); // scène 2 active
        eng.handle_event(EngineEvent::AddSection).unwrap(); // section sur scène 2
        let scenes = eng.snapshot().project.scenes.len();
        assert!(scenes >= 2);

        // Duplique la scène active.
        eng.handle_event(EngineEvent::DuplicateScene).unwrap();
        assert_eq!(eng.snapshot().project.scenes.len(), scenes + 1);

        // Supprime la scène active (la copie) : les sections de la scène 2
        // restent, aucun index ne pointe hors limites.
        eng.handle_event(EngineEvent::DeleteScene).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.project.scenes.len(), scenes);
        for sec in &snap.project.song {
            assert!(sec.scene < snap.project.scenes.len());
        }

        // Impossible de supprimer la dernière scène.
        while eng.snapshot().project.scenes.len() > 1 {
            eng.handle_event(EngineEvent::DeleteScene).unwrap();
        }
        eng.handle_event(EngineEvent::DeleteScene).unwrap();
        let snap = eng.snapshot();
        assert_eq!(snap.project.scenes.len(), 1);
        for sec in &snap.project.song {
            assert!(sec.scene == 0);
        }
        assert_eq!(snap.active_scene, Some(0));
    }

    #[test]
    fn file_mode_soft_keys_and_new_project_creation() {
        let mut eng = Engine::new_demo(48000);
        eng.left_tab = 3;
        eng.browser_kind = 1; // FILE mode

        // 1. Softkey labels en mode FILE (avec suppression directe depuis MK3)
        assert_eq!(
            eng.soft_key_labels(),
            ["PROJ <", "PROJ >", "NOUVEAU", "CHARGER", "SAUVER", "SUPPR", "PRESETS", "RETOUR"]
        );

        // 2. SoftKey 3 = NOUVEAU projet
        eng.handle_event(EngineEvent::SoftKey(3)).unwrap();
        let snap = eng.snapshot();
        assert!(snap.current_project_path.is_some());
        let path = std::path::PathBuf::from(snap.current_project_path.as_ref().unwrap());
        assert!(path.exists(), "Le fichier projet doit être créé sur disque");

        // 3. Test de confirmation et suppression de projet depuis MK3
        eng.handle_event(EngineEvent::SoftKey(6)).unwrap();
        assert!(eng.snapshot().delete_project_confirm);
        assert_eq!(eng.soft_key_labels()[5], "CONFIRMER");
        eng.handle_event(EngineEvent::CancelDeleteProject).unwrap();
        assert!(!eng.snapshot().delete_project_confirm);
        assert_eq!(eng.soft_key_labels()[5], "SUPPR");

        // Le projet doit avoir un kit jouable (16 sons par groupe sur les 8 groupes A..H) et Pattern 1 vide
        assert_eq!(snap.project.groups.len(), 8);
        let g = &snap.project.groups[0];
        assert_eq!(g.name, "Kit 808");
        assert_eq!(g.sounds.iter().filter(|s| s.is_some()).count(), 16);
        assert_eq!(g.patterns[0].name, "Pattern 1");
        assert!(
            g.patterns[0].events.is_empty(),
            "Pattern 1 doit être vierge pour l'utilisateur"
        );

        // Message toast affiché
        assert!(snap.status_message.is_some());
        assert!(snap
            .status_message
            .as_ref()
            .unwrap()
            .starts_with("Nouveau projet:"));

        // Liste des projets mise à jour
        assert!(!eng.project_entries.is_empty());
        assert_eq!(
            eng.project_entries[eng.project_selected].path,
            path.to_string_lossy()
        );

        // 3. SoftKey 7 = PRESETS (bascule vers BROWSER sons)
        eng.handle_event(EngineEvent::SoftKey(7)).unwrap();
        assert_eq!(eng.browser_kind, 0);

        // 4. SoftKey 8 en FILE = RETOUR (referme vers vue principale tab 0)
        eng.browser_kind = 1;
        eng.handle_event(EngineEvent::SoftKey(8)).unwrap();
        assert_eq!(eng.left_tab, 0);

        // Nettoyage du fichier de test
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_filter_selection_and_cycling_via_soft_keys() {
        // Les modes de filtre du sampler sont sur la page SON du PLUG-IN
        // (boutons 5–8), l'éditeur de sample ayant pris les boutons SAMPLING.
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::SelectSound(0)).unwrap();
        eng.handle_event(EngineEvent::SetLeftTab(4)).unwrap();
        assert_eq!(eng.soft_key_labels()[4], "LP/HP/BP");

        // Réinitialise le filtre à Off
        eng.handle_event(EngineEvent::SetSoundFilterKind {
            group: 0,
            sound: 0,
            kind: crate::project::FilterKind::Off,
        })
        .unwrap();

        // Touche 5 = LP/HP/BP : Off → LowPass
        eng.handle_event(EngineEvent::SoftKey(5)).unwrap();
        let f1 = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f1, crate::project::FilterKind::LowPass);

        // Touche 5 : LowPass → HighPass
        eng.handle_event(EngineEvent::SoftKey(5)).unwrap();
        let f2 = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f2, crate::project::FilterKind::HighPass);

        // Touche 5 encore : HighPass → BandPass
        eng.handle_event(EngineEvent::SoftKey(5)).unwrap();
        let f3 = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f3, crate::project::FilterKind::BandPass);

        // Touche 6 (idx 5) = NOTCH
        eng.handle_event(EngineEvent::SoftKey(6)).unwrap();
        let f4 = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f4, crate::project::FilterKind::Notch);

        // Touche 7 (idx 6) = PEAK
        eng.handle_event(EngineEvent::SoftKey(7)).unwrap();
        let f5 = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f5, crate::project::FilterKind::Peak);

        // Touche 8 (idx 7) = OFF
        eng.handle_event(EngineEvent::SoftKey(8)).unwrap();
        let f6 = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f6, crate::project::FilterKind::Off);

        // Touche 5 (idx 4) depuis Off → cycle vers LP
        eng.handle_event(EngineEvent::SoftKey(5)).unwrap();
        let f7 = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f7, crate::project::FilterKind::LowPass);
    }

    #[test]
    fn test_kick_synth_drive_and_filter_events() {
        let mut eng = Engine::new_demo(48000);
        // Charge le Kick Synth sur pad 0
        eng.load_plugin_to_sound(0, 0, 0).unwrap();
        assert!(eng.focus_sound_has_kick());

        // Cycle drive type
        eng.handle_event(EngineEvent::KickDriveTypeDelta(1))
            .unwrap();
        assert_eq!(
            eng.kick_synth.params.drive_type,
            crate::kick_synth::KickDriveType::HardClip
        );

        // Cycle filter mode (de LowPass -> HighPass)
        eng.handle_event(EngineEvent::KickFilterModeDelta(1))
            .unwrap();
        assert_eq!(
            eng.kick_synth.params.filter_mode,
            crate::kick_synth::KickFilterMode::HighPass
        );

        // Ajuste filter cutoff & reso
        eng.handle_event(EngineEvent::KickFilterCutoffDelta(5.0))
            .unwrap();
        assert!(
            eng.kick_synth.params.filter_cutoff_hz > 50.0
                && eng.kick_synth.params.filter_cutoff_hz < 1500.0
        );
        eng.handle_event(EngineEvent::KickFilterResoDelta(3.0))
            .unwrap();
        assert!(eng.kick_synth.params.filter_reso > 0.0);
    }

    #[test]
    fn test_nexus_synth_plugin_and_presets() {
        let mut eng = Engine::new_demo(48000);
        // Charge Nexus Wavetable Synth (plugin_idx 2) sur pad 3
        eng.load_plugin_to_sound(0, 3, 2).unwrap();
        eng.active_sound = 3;
        assert!(eng.focus_sound_has_wavetable());

        // Vérifie les presets
        let p_start = eng.wavetable_synth.params.name.clone();
        eng.handle_event(EngineEvent::WavetableNextPreset).unwrap();
        let p_next = eng.wavetable_synth.params.name.clone();
        assert_ne!(p_start, p_next);

        // Sélection directe preset
        eng.handle_event(EngineEvent::WavetableSetPreset(0))
            .unwrap();
        assert_eq!(eng.wavetable_synth.params.name, "Concert Grand Piano");

        // Rendu audio polyphonique
        let mut out = vec![0.0f32; 1024];
        eng.handle_event(EngineEvent::Pad {
            pad: 3,
            velocity: 110,
            pressed: true,
        })
        .unwrap();
        eng.render_audio(&mut out);
        let max_amp = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            max_amp > 0.01,
            "Le synthé wavetable doit produire du son audible"
        );
    }

    #[test]
    fn test_pattern_press_cuts_active_and_chaining() {
        let mut eng = Engine::new_demo(48000);
        let group = eng.group_focus;

        // 1. Ré-appuyer sur le pad du pattern courant le coupe instantanément
        //    (workflow officiel Maschine MK3).
        let init_pat = eng
            .project
            .read()
            .active_pattern_index(group as usize, eng.active_scene);
        assert_eq!(init_pat, Some(0));

        eng.handle_event(EngineEvent::PatternSelect { group, pattern: 0 })
            .unwrap();
        let still_pat = eng
            .project
            .read()
            .active_pattern_index(group as usize, eng.active_scene);
        assert_eq!(
            still_pat, None,
            "Le pattern actif doit être coupé par un ré-appui sur son pad"
        );
        // Le pattern reste dans le groupe (on ne supprime pas le slot).
        assert!(!eng.project.read().groups[group as usize]
            .patterns
            .is_empty());
        // On réassigne le pattern pour la suite du test.
        eng.handle_event(EngineEvent::PatternSelect { group, pattern: 0 })
            .unwrap();

        // 2. Chaque Groupe possède son propre pattern dans la scène : créer un
        //    pattern dans un autre Groupe ne change pas celui du Groupe focus.
        let other = (group + 1) % 8;
        eng.handle_event(EngineEvent::AddPattern { group: other })
            .unwrap();
        let group_pat = eng
            .project
            .read()
            .active_pattern_index(group as usize, eng.active_scene);
        assert_eq!(group_pat, Some(0));

        // 2. Chaining de patterns (ex: Patterns 0 et 1)
        eng.handle_event(EngineEvent::AddPattern { group }).unwrap(); // Pattern 1 créé
        eng.handle_event(EngineEvent::PatternChain {
            group,
            patterns: vec![0, 1],
        })
        .unwrap();
        assert_eq!(eng.pattern_chains[group as usize], Some(vec![0, 1]));
        let mask = eng.hardware_snapshot().pattern_chain_mask;
        assert_eq!(
            mask & 0b11,
            0b11,
            "les pads 0 et 1 de la chaîne sont allumés"
        );

        // L'avance du transport tourne sans panic avec la chaîne
        eng.transport.state = crate::transport::TransportState::Playing;
        eng.advance(1920);
        eng.advance(1920);
        assert!(eng.transport.tick > 0);
    }

    #[test]
    fn test_shift_held_labels_and_actions() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::SetPatternMode(true)).unwrap();

        // Labels normaux en mode Pattern
        let normal_labels = eng.soft_key_labels();
        assert_eq!(normal_labels[0], "PAT <");

        // Shift enfoncé -> labels Shift (DOUBLE, HALF, QUANT, HUMAN...)
        eng.handle_event(EngineEvent::SetShiftHeld(true)).unwrap();
        let shift_labels = eng.soft_key_labels();
        assert_eq!(shift_labels[0], "DOUBLE");
        assert_eq!(shift_labels[1], "HALF");
        assert_eq!(shift_labels[2], "QUANT");

        // Action DOUBLE via SoftKey(1)
        let pat_len_before = eng.project.read().groups[0].patterns[0].length.bars;
        eng.handle_event(EngineEvent::SoftKey(1)).unwrap();
        let pat_len_after = eng.project.read().groups[0].patterns[0].length.bars;
        assert_eq!(pat_len_after, pat_len_before * 2);

        // Action HALF via SoftKey(2)
        eng.handle_event(EngineEvent::SoftKey(2)).unwrap();
        let pat_len_half = eng.project.read().groups[0].patterns[0].length.bars;
        assert_eq!(pat_len_half, pat_len_before);

        // Relâche Shift
        eng.handle_event(EngineEvent::SetShiftHeld(false)).unwrap();
        let normal_again = eng.soft_key_labels();
        assert_eq!(normal_again[0], "PAT <");
    }

    #[test]
    fn test_synth_pad_mixer_volume_and_mute() {
        let mut eng = Engine::new_demo(48000);
        // Charge Wavetable sur pad 2
        eng.load_plugin_to_sound(0, 2, 2).unwrap();

        let mut out = vec![0.0f32; 1024];
        eng.handle_event(EngineEvent::Pad {
            pad: 2,
            velocity: 120,
            pressed: true,
        })
        .unwrap();
        eng.render_audio(&mut out);
        let max_amp1 = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(max_amp1 > 0.01);

        // Baisse le volume du pad 2 dans le mixeur (-60 dB)
        eng.handle_event(EngineEvent::SoundVolume {
            group: 0,
            sound: 2,
            volume_db: -60.0,
        })
        .unwrap();

        let mut out2 = vec![0.0f32; 1024];
        eng.render_audio(&mut out2);
        // Les premières trames sortent encore de l'anticipation du limiteur.
        let skip = eng.master_limiter.latency_frames() * 2;
        let max_amp2 = out2[skip..].iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            max_amp2 < max_amp1 * 0.1,
            "L'atténuation du mixeur doit impacter le synthé"
        );
    }

    #[test]
    fn test_wavetable_synth_pad_hold_sustains_indefinitely() {
        let mut eng = Engine::new_demo(48000);
        eng.load_plugin_to_sound(0, 0, 2).unwrap(); // Nexus Wavetable on Pad 0

        // Appui maintenu sur le pad
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 110,
            pressed: true,
        })
        .unwrap();

        // On rend l'équivalent de 3 secondes d'audio (3 * 48000 = 144000 samples)
        // en blocs de 1024 frames
        let mut block = vec![0.0f32; 2048];
        let mut sustained_peak = 0.0f32;
        for _ in 0..150 {
            block.fill(0.0);
            eng.render_audio(&mut block);
            sustained_peak = block.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        }

        // Même après 3 secondes avec pad maintenu, le son doit rester audible et fort !
        assert!(
            sustained_peak > 0.02,
            "Le synthé wavetable doit continuer de sonner tant que le pad est tenu (peak = {sustained_peak})"
        );

        // Relâchement du pad
        eng.handle_event(EngineEvent::Pad {
            pad: 0,
            velocity: 0,
            pressed: false,
        })
        .unwrap();

        // Après le release (350ms), le son doit s'éteindre
        for _ in 0..50 {
            block.fill(0.0);
            eng.render_audio(&mut block);
        }
        let release_peak = block.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            release_peak < 0.001,
            "Le synthé wavetable doit s'éteindre proprement après release (peak = {release_peak})"
        );
    }

    #[test]
    fn test_acid_synth_pad_hold_sustains_indefinitely() {
        let mut eng = Engine::new_demo(48000);
        eng.load_plugin_to_sound(0, 1, 1).unwrap(); // TB-303 on Pad 1

        eng.handle_event(EngineEvent::Pad {
            pad: 1,
            velocity: 120,
            pressed: true,
        })
        .unwrap();

        let mut block = vec![0.0f32; 2048];
        let mut sustained_peak = 0.0f32;
        for _ in 0..100 {
            block.fill(0.0);
            eng.render_audio(&mut block);
            sustained_peak = block.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        }

        assert!(
            sustained_peak > 0.02,
            "Le synthé TB-303 doit continuer de sonner tant que le pad est maintenu (peak = {sustained_peak})"
        );

        eng.handle_event(EngineEvent::Pad {
            pad: 1,
            velocity: 0,
            pressed: false,
        })
        .unwrap();

        // Après le release et l'extinction du delay (150 blocs), le son doit être éteint
        for _ in 0..150 {
            block.fill(0.0);
            eng.render_audio(&mut block);
        }
        let release_peak = block.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            release_peak < 0.001,
            "Le synthé TB-303 doit s'éteindre proprement après relâchement (peak = {release_peak})"
        );
    }

    #[test]
    fn test_shift_soft_keys_sampler_view_and_filter_direct_select() {
        let mut eng = Engine::new_demo(48000);
        eng.left_tab = 2; // Sampler view
        eng.active_sound = 0;
        eng.handle_event(EngineEvent::SetShiftHeld(true)).unwrap();

        // Nouvelle disposition : LP/HP/BP=4, NOTCH=5, PEAK=6, OFF=7
        let labels = eng.soft_key_labels();
        assert_eq!(
            labels,
            ["REVERSE", "LOOP", "NORMLZ", "CYC FLT", "LP/HP/BP", "NOTCH", "PEAK", "OFF"]
        );

        // SoftKey 4 (idx 3) = CYC FLT → Off → LowPass
        eng.handle_event(EngineEvent::SoftKey(4)).unwrap();

        // SoftKey 5 (idx 4) = LP/HP/BP cycle : LowPass → HighPass
        eng.handle_event(EngineEvent::SoftKey(5)).unwrap();
        let f_hp = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f_hp, crate::project::FilterKind::HighPass);

        // SoftKey 6 (idx 5) = NOTCH
        eng.handle_event(EngineEvent::SoftKey(6)).unwrap();
        let f_notch = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f_notch, crate::project::FilterKind::Notch);

        // SoftKey 7 (idx 6) = PEAK
        eng.handle_event(EngineEvent::SoftKey(7)).unwrap();
        let f_peak = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f_peak, crate::project::FilterKind::Peak);

        // SoftKey 8 (idx 7) = OFF
        eng.handle_event(EngineEvent::SoftKey(8)).unwrap();
        let f_off = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .filter
            .kind;
        assert_eq!(f_off, crate::project::FilterKind::Off);
    }

    #[test]
    fn test_shift_soft_keys_step_mode_nudge_fill_octave() {
        let mut eng = Engine::new_demo(48000);
        eng.controller_mode = 3; // Step mode
        eng.active_sound = 0;
        eng.selected_bar = 0;
        eng.handle_event(EngineEvent::SetShiftHeld(true)).unwrap();

        let labels = eng.soft_key_labels();
        assert_eq!(
            labels,
            ["VIDER", "QUANT", "HUMAN", "NUDG -", "NUDG +", "OCT -", "OCT +", "FILL"]
        );

        // SoftKey 8 = FILL 16 pas pour la mesure 0
        eng.handle_event(EngineEvent::SoftKey(8)).unwrap();
        let evs_count = eng.project.read().groups[0].patterns[0]
            .events
            .iter()
            .filter(|e| e.pad == 0 && e.tick < 4 * 960)
            .count();
        assert_eq!(evs_count, 16);

        // SoftKey 7 = OCT + (+12 pour le pad 0 dans la mesure 0)
        let pitch_before = eng.project.read().groups[0].patterns[0]
            .events
            .iter()
            .find(|e| e.pad == 0 && e.tick == 0)
            .unwrap()
            .pitch;
        eng.handle_event(EngineEvent::SoftKey(7)).unwrap();
        let pitch_after = eng.project.read().groups[0].patterns[0]
            .events
            .iter()
            .find(|e| e.pad == 0 && e.tick == 0)
            .unwrap()
            .pitch;
        assert_eq!(pitch_after, pitch_before + 12);

        // SoftKey 1 = VIDER (vide la mesure 0)
        eng.handle_event(EngineEvent::SoftKey(1)).unwrap();
        let evs_bar0_after_clear = eng.project.read().groups[0].patterns[0]
            .events
            .iter()
            .filter(|e| e.tick < 4 * 960)
            .count();
        assert_eq!(evs_bar0_after_clear, 0);
    }

    #[test]
    fn test_shift_soft_keys_mixer_mode_pan_center() {
        let mut eng = Engine::new_demo(48000);
        eng.left_tab = 1; // Mixer view
        eng.mixer_mode = 0; // Pads 1-8
        eng.group_focus = 0;

        // Décentre le pan du pad 1 (kick)
        eng.handle_event(EngineEvent::SoundPan {
            group: 0,
            sound: 0,
            pan: 0.85,
        })
        .unwrap();
        assert_eq!(
            eng.project.read().groups[0].sounds[0].as_ref().unwrap().pan,
            0.85
        );

        // Shift + SoftKey 1 (PAN 1) -> centre le pan à 0.0
        eng.handle_event(EngineEvent::SetShiftHeld(true)).unwrap();
        eng.handle_event(EngineEvent::SoftKey(1)).unwrap();
        assert_eq!(
            eng.project.read().groups[0].sounds[0].as_ref().unwrap().pan,
            0.0
        );
    }

    #[test]
    fn shift_pad_quantize_nudge_and_transpose_edit_active_pattern() {
        let mut engine = Engine::new_demo(48_000);
        {
            let mut project = engine.project.write();
            let events = &mut project_mut(&mut project).groups[0].patterns[0].events;
            events.clear();
            events.push(PatternEvent {
                pad: 0,
                tick: 200,
                velocity: 100,
                length_ticks: 120,
                pitch: 60,
            });
        }

        engine.handle_event(EngineEvent::ShiftPadAction(4)).unwrap(); // Quantize
        assert_eq!(
            engine.project.read().groups[0].patterns[0].events[0].tick,
            240
        );

        {
            let mut project = engine.project.write();
            project_mut(&mut project).groups[0].patterns[0].events[0].tick = 200;
        }
        engine.handle_event(EngineEvent::ShiftPadAction(5)).unwrap(); // 50%
        assert_eq!(
            engine.project.read().groups[0].patterns[0].events[0].tick,
            220
        );
        engine.handle_event(EngineEvent::ShiftPadAction(6)).unwrap(); // Nudge left
        assert_eq!(
            engine.project.read().groups[0].patterns[0].events[0].tick,
            160
        );
        engine.handle_event(EngineEvent::ShiftPadAction(7)).unwrap(); // Nudge right
        assert_eq!(
            engine.project.read().groups[0].patterns[0].events[0].tick,
            220
        );

        let pitch = engine.project.read().groups[0].patterns[0].events[0].pitch;
        engine
            .handle_event(EngineEvent::ShiftPadAction(12))
            .unwrap(); // Semitone down
        assert_eq!(
            engine.project.read().groups[0].patterns[0].events[0].pitch,
            pitch - 1
        );
        engine
            .handle_event(EngineEvent::ShiftPadAction(13))
            .unwrap(); // Semitone up
        assert_eq!(
            engine.project.read().groups[0].patterns[0].events[0].pitch,
            pitch
        );
        engine
            .handle_event(EngineEvent::ShiftPadAction(14))
            .unwrap(); // Octave down
        assert_eq!(
            engine.project.read().groups[0].patterns[0].events[0].pitch,
            pitch - 12
        );
        engine
            .handle_event(EngineEvent::ShiftPadAction(15))
            .unwrap(); // Octave up
        assert_eq!(
            engine.project.read().groups[0].patterns[0].events[0].pitch,
            pitch
        );
    }

    #[test]
    fn shift_pad_clear_copy_paste_and_undo_redo_work_on_pattern_events() {
        let mut engine = Engine::new_demo(48_000);
        let original = engine.project.read().groups[0].patterns[0].events.clone();

        engine
            .handle_event(EngineEvent::ShiftPadAction(10))
            .unwrap(); // Copy
        engine.handle_event(EngineEvent::ShiftPadAction(8)).unwrap(); // Clear
        assert!(engine.project.read().groups[0].patterns[0]
            .events
            .is_empty());
        engine
            .handle_event(EngineEvent::ShiftPadAction(11))
            .unwrap(); // Paste
        assert_eq!(engine.project.read().groups[0].patterns[0].events, original);

        engine.handle_event(EngineEvent::ShiftPadAction(8)).unwrap();
        engine.handle_event(EngineEvent::ShiftPadAction(0)).unwrap(); // Undo
        assert_eq!(engine.project.read().groups[0].patterns[0].events, original);
        engine.handle_event(EngineEvent::ShiftPadAction(1)).unwrap(); // Redo
        assert!(engine.project.read().groups[0].patterns[0]
            .events
            .is_empty());
    }

    #[test]
    fn shift_pad_step_undo_redo_and_clear_auto_feedback_are_contextual() {
        let mut engine = Engine::new_demo(48_000);
        let original = engine.project.read().groups[0].patterns[0].events.clone();
        engine.toggle_step(15, 15 * 240);
        assert_ne!(engine.project.read().groups[0].patterns[0].events, original);
        engine.handle_event(EngineEvent::ShiftPadAction(2)).unwrap(); // Step Undo
        assert_eq!(engine.project.read().groups[0].patterns[0].events, original);
        engine.handle_event(EngineEvent::ShiftPadAction(3)).unwrap(); // Step Redo
        assert_ne!(engine.project.read().groups[0].patterns[0].events, original);

        engine.handle_event(EngineEvent::ShiftPadAction(9)).unwrap(); // Clear Auto
        assert!(engine
            .snapshot()
            .status_message
            .as_deref()
            .is_some_and(|message| message.contains("automation")));
    }

    #[test]
    fn shift_erase_pad_removes_a_sound_slot_and_undo_restores_it() {
        let mut engine = Engine::new_demo(48_000);
        let original_sound = engine.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .name
            .clone();
        engine
            .handle_event(EngineEvent::ClearSoundSlot { group: 0, sound: 0 })
            .unwrap();
        assert!(engine.project.read().groups[0].sounds[0].is_none());
        assert!(engine.project.read().groups[0].patterns[0]
            .events
            .iter()
            .all(|event| event.pad != 0));
        engine.handle_event(EngineEvent::ShiftPadAction(0)).unwrap();
        assert_eq!(
            engine.project.read().groups[0].sounds[0]
                .as_ref()
                .unwrap()
                .name,
            original_sound
        );
    }

    /// Rend `blocks` blocs après un coup de pad et renvoie la crête.
    fn pad_peak(eng: &mut Engine, pad: u8) -> f32 {
        eng.handle_event(EngineEvent::Pad {
            pad,
            velocity: 120,
            pressed: true,
        })
        .unwrap();
        let mut buf = vec![0.0f32; 1024 * 2];
        let mut peak = 0.0f32;
        for _ in 0..6 {
            eng.render_audio(&mut buf);
            peak = buf.iter().fold(peak, |m, s| m.max(s.abs()));
        }
        peak
    }

    #[test]
    fn plugin_chain_inserts_edits_and_bypasses_effects_from_soft_keys() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::SelectSound(0)).unwrap();
        eng.handle_event(EngineEvent::SetLeftTab(4)).unwrap();
        assert_eq!(eng.current_plugin_slot(), PluginSlot::Instrument);
        // 4D : de l'instrument à l'emplacement libre du Sound.
        eng.handle_event(EngineEvent::PluginSlotStep(1)).unwrap();
        let owner = FxOwner::Sound(eng.group_focus, 0);
        assert_eq!(eng.current_plugin_slot(), PluginSlot::Add(owner));
        assert_eq!(eng.soft_key_labels()[0], "FILTER");
        eng.handle_event(EngineEvent::SoftKey(1)).unwrap();
        assert_eq!(eng.current_plugin_slot(), PluginSlot::Fx(owner, 0));
        let labels = eng.soft_key_labels();
        assert_eq!(&labels[..5], &["< TYPE", "TYPE >", "BYPASS", "SUPPR", "LP"]);
        assert!(eng.soft_key_active()[4], "LP allumé (mode courant)");
        // Knob 2 = CUTOFF, affiché et piloté depuis la même table.
        let before = eng.selected_fx().unwrap().2.p1;
        eng.handle_event(EngineEvent::PluginKnob {
            knob: 1,
            delta: -300,
        })
        .unwrap();
        assert!(eng.selected_fx().unwrap().2.p1 < before);
        assert_eq!(
            eng.snapshot().plugin_fx.unwrap().p1,
            eng.selected_fx().unwrap().2.p1
        );
        // BYPASS : contourné, bouton allumé ; SUPPR : la chaîne se vide.
        eng.handle_event(EngineEvent::SoftKey(3)).unwrap();
        assert!(!eng.selected_fx().unwrap().2.enabled);
        assert!(eng.soft_key_active()[2]);
        let views = eng.snapshot().plugin_slots;
        assert!(views[1].bypassed && views[1].name == "Filter");
        eng.handle_event(EngineEvent::SoftKey(4)).unwrap();
        assert_eq!(eng.focus_sound_fx().len(), 0);
        // Un autre Sound repart de son instrument.
        eng.handle_event(EngineEvent::PluginSlotStep(2)).unwrap();
        eng.handle_event(EngineEvent::SelectSound(1)).unwrap();
        assert_eq!(eng.current_plugin_slot(), PluginSlot::Instrument);
    }

    #[test]
    fn sound_group_and_master_inserts_process_the_audio() {
        let quiet = |eng: &mut Engine, owner: FxOwner| {
            eng.insert_fx(owner, crate::fx::FxType::Distortion);
            let i = owner.chain(&eng.project.read()).len() - 1;
            eng.edit_fx(owner, i, |s| {
                s.p1 = 0.0;
                s.p4 = -24.0;
            });
            eng.sync_fx_buses();
        };
        for owner in [FxOwner::Sound(0, 0), FxOwner::Group(0), FxOwner::Master] {
            let mut eng = Engine::new_demo(48000);
            eng.handle_event(EngineEvent::GroupFocus(0)).unwrap();
            let dry = pad_peak(&mut eng, 0);
            assert!(dry > 0.05, "le pad 1 sonne : {dry}");
            let mut eng = Engine::new_demo(48000);
            eng.handle_event(EngineEvent::GroupFocus(0)).unwrap();
            quiet(&mut eng, owner);
            let wet = pad_peak(&mut eng, 0);
            assert!(wet < dry * 0.2, "{owner:?} : {wet} vs {dry}");
        }
    }

    #[test]
    fn events_mode_edits_the_selected_note_with_knobs_and_buttons() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::GroupFocus(0)).unwrap();
        eng.handle_event(EngineEvent::SelectSound(0)).unwrap();
        eng.handle_event(EngineEvent::SetRightTab(1)).unwrap();
        assert_eq!(eng.soft_key_labels(), EVENT_LABELS);
        let count = eng.snapshot().event_count;
        assert!(count > 1, "le kick de la démo a plusieurs notes");
        let first = eng.selected_event().unwrap().1;
        // Knob 1 : un cran = un pas plus tard ; la note reste sélectionnée.
        eng.handle_event(EngineEvent::EventKnob { knob: 0, delta: 32 })
            .unwrap();
        let moved = eng.selected_event().unwrap().1;
        assert_eq!(moved.tick, first.tick + TICKS_PER_STEP);
        // Knob 3 : vélocité continue ; knob 2 : un demi-ton par cran.
        eng.handle_event(EngineEvent::EventKnob {
            knob: 2,
            delta: -80,
        })
        .unwrap();
        assert_eq!(
            eng.selected_event().unwrap().1.velocity,
            moved.velocity - 10
        );
        eng.handle_event(EngineEvent::EventKnob { knob: 1, delta: 64 })
            .unwrap();
        assert_eq!(eng.selected_event().unwrap().1.pitch, moved.pitch + 2);
        // NUDGE > puis QUANT : retour sur la grille.
        eng.handle_event(EngineEvent::SoftKey(8)).unwrap();
        assert_eq!(eng.selected_event().unwrap().1.tick, moved.tick + 60);
        eng.handle_event(EngineEvent::SoftKey(6)).unwrap();
        assert_eq!(eng.selected_event().unwrap().1.tick % TICKS_PER_STEP, 0);
        // 4D / EVT > : note suivante ; SUPPR : une note de moins.
        eng.handle_event(EngineEvent::EventStep(1)).unwrap();
        assert_eq!(eng.snapshot().event_index, 1);
        eng.handle_event(EngineEvent::SoftKey(4)).unwrap();
        assert_eq!(eng.snapshot().event_count, count - 1);
        // TOUS : tous les Sounds du pattern, bouton allumé.
        eng.handle_event(EngineEvent::SoftKey(3)).unwrap();
        assert!(eng.snapshot().events_all);
        assert!(eng.soft_key_active()[2]);
        assert!(eng.snapshot().event_count >= count - 1);
    }

    #[test]
    fn events_knob_six_selects_every_note_for_relative_edits() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::GroupFocus(0)).unwrap();
        eng.handle_event(EngineEvent::SelectSound(0)).unwrap();
        eng.handle_event(EngineEvent::SetRightTab(1)).unwrap();
        let velocities = |eng: &Engine| -> Vec<u8> {
            let proj = eng.project.read();
            let idx = proj.active_pattern_index(0, eng.active_scene).unwrap();
            let pat = &proj.groups[0].patterns[idx];
            event_scope(pat, Some(0))
                .into_iter()
                .map(|i| pat.events[i].velocity)
                .collect()
        };
        let before = velocities(&eng);
        assert!(before.len() > 1);
        // Knob 6 vers la droite : toutes les notes du Sound.
        eng.handle_event(EngineEvent::EventKnob { knob: 5, delta: 32 })
            .unwrap();
        assert!(eng.snapshot().events_select_all);
        // Knob 3 : vélocité −10 sur chaque note (bornée à 1).
        eng.handle_event(EngineEvent::EventKnob {
            knob: 2,
            delta: -80,
        })
        .unwrap();
        let expected: Vec<u8> = before.iter().map(|v| v.saturating_sub(10).max(1)).collect();
        assert_eq!(velocities(&eng), expected);
        // SUPPR : toutes les notes du Sound partent d'un coup.
        eng.handle_event(EngineEvent::SoftKey(4)).unwrap();
        assert!(velocities(&eng).is_empty());
        // Knob 6 vers la gauche : retour à une note.
        eng.handle_event(EngineEvent::EventKnob {
            knob: 5,
            delta: -32,
        })
        .unwrap();
        assert!(!eng.snapshot().events_select_all);
    }

    #[test]
    fn sample_editor_pages_edit_slice_zone_and_record() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::GroupFocus(0)).unwrap();
        eng.handle_event(EngineEvent::SelectSound(0)).unwrap();
        eng.handle_event(EngineEvent::SetLeftTab(2)).unwrap();
        assert_eq!(&eng.soft_key_labels()[..4], &SAMPLE_PAGES);
        let id = eng.focus_sound_id();

        // EDIT : knob 1 avance le début de la plage (sampler et projet).
        eng.handle_event(EngineEvent::SampleKnob {
            knob: 0,
            delta: 300,
        })
        .unwrap();
        let (start, end) = eng.sampler.sound_range(id);
        assert!(start > 0.1 && end == 1.0, "{start} {end}");
        let saved = eng.project.read().groups[0].sounds[0]
            .as_ref()
            .unwrap()
            .zones[0]
            .start;
        assert!(
            (saved - start).abs() < 1e-6,
            "plage enregistrée dans le projet"
        );
        eng.handle_event(EngineEvent::SoftKey(8)).unwrap(); // PLAGE MAX
        assert_eq!(eng.sampler.sound_range(id), (0.0, 1.0));

        // ZONE : knob 1 = note racine (un demi-ton par cran).
        eng.handle_event(EngineEvent::SoftKey(4)).unwrap();
        assert_eq!(eng.snapshot().sample_page, 3);
        let root = eng.sampler.zone_mapping(id).unwrap().root_key;
        eng.handle_event(EngineEvent::SampleKnob { knob: 0, delta: 64 })
            .unwrap();
        assert_eq!(eng.sampler.zone_mapping(id).unwrap().root_key, root + 2);

        // RECORD : prise sur le pad 2, WAV écrit puis chargé.
        let dir = std::env::temp_dir().join(format!("mk3-rec-test-{}", std::process::id()));
        eng.set_recordings_dir(dir.clone());
        eng.handle_event(EngineEvent::SelectSound(1)).unwrap();
        eng.handle_event(EngineEvent::SoftKey(1)).unwrap(); // page RECORD
        eng.handle_event(EngineEvent::SoftKey(5)).unwrap(); // ENREG
        assert!(eng.is_recording());
        assert_eq!(eng.soft_key_labels()[4], "STOP");
        let tone: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        eng.push_record_input(&tone);
        eng.handle_event(EngineEvent::SoftKey(5)).unwrap(); // STOP
        assert!(!eng.is_recording());
        #[cfg(feature = "sampler")]
        {
            let zone = eng.sampler.zone_mapping(sampler_sound_id(0, 1)).unwrap();
            assert_eq!(zone.sample.unwrap().len(), 4800);
            assert!(zone.sample_path.unwrap().starts_with(dir.to_str().unwrap()));

            // SLICE de la prise : 4 tranches sur les pads 2–5, plages contiguës.
            eng.handle_event(EngineEvent::SoftKey(3)).unwrap();
            eng.handle_event(EngineEvent::SoftKey(5)).unwrap(); // « 4 »
            assert!(eng.soft_key_active()[4], "4 tranches allumé");
            eng.handle_event(EngineEvent::SoftKey(8)).unwrap(); // APPLIQUER
            let ranges: Vec<(f32, f32)> = (1..5)
                .map(|k| eng.sampler.sound_range(sampler_sound_id(0, k)))
                .collect();
            assert_eq!(ranges[0].0, 0.0);
            assert_eq!(ranges[3].1, 1.0);
            for w in ranges.windows(2) {
                assert!((w[0].1 - w[1].0).abs() < 1e-6, "{ranges:?}");
            }
            let name = eng.project.read().groups[0].sounds[3]
                .as_ref()
                .unwrap()
                .name
                .clone();
            assert!(name.ends_with("#3"), "{name}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn every_group_plays_at_once_with_its_own_synths_and_meters() {
        let mut eng = Engine::new_demo(48000);
        // Lecture du projet démo : A (kick) et B (caisse claire) ensemble,
        // chacun avec son VU.
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: false,
            stop: false,
        })
        .unwrap();
        let mut buf = vec![0.0f32; 2048];
        let mut peaks = [0.0f32; 8];
        for _ in 0..120 {
            eng.advance(1024);
            eng.render_audio(&mut buf);
            for (g, p) in eng.sampler.group_peaks().iter().enumerate() {
                peaks[g] = peaks[g].max(*p);
            }
        }
        assert!(
            peaks[0] > 0.05 && peaks[1] > 0.05,
            "A et B jouent : {peaks:?}"
        );

        // Deux TB-303 de deux groupes : deux instances, aucune ne coupe l'autre.
        let mut eng = Engine::new_demo(48000);
        let acid = |eng: &Engine| {
            (0..8u8)
                .flat_map(|g| (0..16u8).map(move |s| (g, s)))
                .filter(|&(g, s)| eng.rack.acid[rack_index(g, s)].is_some())
                .collect::<Vec<_>>()
        };
        let sounds = acid(&eng);
        let (a, b) = (
            sounds[0],
            *sounds.iter().find(|x| x.0 != sounds[0].0).unwrap(),
        );
        for (g, s, note) in [(a.0, a.1, 48), (b.0, b.1, 36)] {
            eng.handle_event(EngineEvent::GroupFocus(g)).unwrap();
            eng.trigger_sound_voice(s, note, 110);
        }
        let loud = |eng: &mut Engine| {
            let mut buf = vec![0.0f32; 2048];
            eng.render_audio(&mut buf);
            buf.iter().fold(0.0f32, |m, v| m.max(v.abs()))
        };
        assert!(loud(&mut eng) > 0.01);
        // Groupe de A muet : B sonne toujours, et inversement.
        eng.muted_groups = 1 << a.0;
        assert!(loud(&mut eng) > 0.01, "le synthé de B joue seul");
        eng.muted_groups = (1 << a.0) | (1 << b.0);
        for _ in 0..4 {
            loud(&mut eng);
        }
        assert!(loud(&mut eng) < 1e-3, "les deux groupes muets");
    }

    #[test]
    fn shift_pattern_switch_is_immediate_while_playing() {
        let mut eng = Engine::new_demo(48000);
        eng.handle_event(EngineEvent::GroupFocus(0)).unwrap();
        eng.handle_event(EngineEvent::AddPattern { group: 0 })
            .unwrap();
        eng.handle_event(EngineEvent::PatternSelect {
            group: 0,
            pattern: 0,
        })
        .unwrap();
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: false,
            stop: false,
        })
        .unwrap();
        eng.advance(1024);
        // SHIFT + pad : bascule immédiate, pas d'attente de la mesure suivante.
        eng.handle_event(EngineEvent::PatternSelectInstant {
            group: 0,
            pattern: 1,
        })
        .unwrap();
        assert_eq!(eng.active_pattern_for_focus(), Some(1));
        assert_eq!(eng.snapshot().pending_patterns[0], None);
        let guard = eng.project.read();
        assert_eq!(guard.active_pattern_index(0, eng.active_scene), Some(1));
    }

    #[test]
    fn launches_wait_for_the_next_bar_and_solo_mute_and_record_work_per_group() {
        let mut eng = Engine::new_demo(48000);
        // Un second pattern pour A, puis lecture.
        eng.handle_event(EngineEvent::GroupFocus(0)).unwrap();
        eng.handle_event(EngineEvent::AddPattern { group: 0 })
            .unwrap();
        eng.handle_event(EngineEvent::PatternSelect {
            group: 0,
            pattern: 0,
        })
        .unwrap();
        eng.handle_event(EngineEvent::Transport {
            play: true,
            rec: false,
            stop: false,
        })
        .unwrap();
        eng.advance(1024);
        // En lecture : le choix attend la mesure suivante.
        eng.handle_event(EngineEvent::PatternSelect {
            group: 0,
            pattern: 1,
        })
        .unwrap();
        assert_eq!(eng.snapshot().pending_patterns[0], Some(1));
        assert_eq!(eng.active_pattern_for_focus(), Some(0), "pas encore lancé");
        let bar: u64 = 3840 * 48_000 * 60 / (960 * 120);
        let mut done = 0;
        while done < bar + 1024 {
            eng.advance(1024);
            done += 1024;
        }
        eng.commit_launches();
        assert_eq!(eng.active_pattern_for_focus(), Some(1), "lancé à la mesure");
        assert_eq!(eng.snapshot().pending_patterns[0], None);

        // SOLO sur B : A n'est plus audible ; MUTE sur B : plus rien.
        eng.handle_event(EngineEvent::Solo(1)).unwrap();
        assert!(!eng.group_audible(0) && eng.group_audible(1));
        eng.handle_event(EngineEvent::Mute(1)).unwrap();
        assert!(!eng.group_audible(1));

        // Enregistrer sur un groupe sans pattern dans la scène : un pattern
        // est créé et placé dans la scène, la note n'est pas perdue.
        let mut eng = Engine::new_demo(48000);
        let g = 7u8;
        {
            let mut guard = eng.project.write();
            let proj = project_mut(&mut guard);
            proj.scenes[0].patterns[g as usize] = None;
        }
        eng.handle_event(EngineEvent::GroupFocus(g)).unwrap();
        let _ = eng.record_note_event(2, 38, 100);
        let proj = eng.project.read().clone();
        let pi = proj
            .active_pattern_index(g as usize, eng.active_scene)
            .expect("pattern placé");
        assert_eq!(proj.groups[g as usize].patterns[pi].events.len(), 1);
    }

    #[test]
    fn keyboard_and_chords_knobs_set_what_the_screens_show() {
        let mut eng = Engine::new_demo(48000);
        eng.controller_mode = 1; // KEYBOARD
        let before = eng.keyboard;
        let turn = |eng: &mut Engine, knob: u8, detents: i32| {
            eng.handle_event(EngineEvent::KeyboardKnob {
                knob,
                delta: detents * crate::plugin_pages::DETENT_TICKS,
            })
            .unwrap();
        };
        turn(&mut eng, 0, 2); // 1:ROOT
        turn(&mut eng, 1, 1); // 2:SCALE
        turn(&mut eng, 2, -1); // 3:OCT
        assert_eq!(eng.keyboard.root, (before.root + 2) % 12);
        assert_eq!(
            eng.keyboard.scale as usize,
            (before.scale as usize + 1) % crate::keyboard::SCALES.len()
        );
        assert_eq!(eng.keyboard.octave, before.octave - 1);
        turn(&mut eng, 3, 1); // 4:SND
        assert_eq!(eng.active_sound, 1);
        // Boutons 5–8 comme la MK3 : OCT − / +, SEMI − / +.
        let labels = eng.soft_key_labels();
        assert_eq!(&labels[4..], &["OCT -", "OCT +", "SEMI -", "SEMI +"]);
        let root = eng.keyboard.root;
        eng.handle_event(EngineEvent::SoftKey(8)).unwrap(); // SEMI + (boutons 1-based)
        assert_eq!(eng.keyboard.root, (root + 1) % 12);
        eng.handle_event(EngineEvent::SoftKey(7)).unwrap(); // SEMI -
                                                            // CHORDS : 3:CHORD, 4:INV, 5:OCT.
        eng.controller_mode = 2;
        turn(&mut eng, 2, 1);
        turn(&mut eng, 3, 1);
        turn(&mut eng, 4, 1);
        assert_eq!(eng.keyboard.chord, 1);
        assert_eq!(eng.keyboard.inversion, 1);
        assert_eq!(eng.keyboard.octave, before.octave);
    }
}
