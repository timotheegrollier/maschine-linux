//! Engine — assemblage Project + Transport + Sampler + Sequencer.
//!
//! Thread-safe snapshot pour UI via `parking_lot::RwLock` + `Arc`.

use std::sync::Arc;

use parking_lot::RwLock;

use crate::project::{BarsBeatsTicks, PatternEvent, Project};
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
    /// Quantize pattern
    Quantize {
        /// Group idx
        group: usize,
        /// Pattern idx
        pattern: usize,
    },
    /// Save
    Save(std::path::PathBuf),
    /// Load
    Load(std::path::PathBuf),
    /// Master volume 0.0..1.0
    MasterVolume(f32),
    /// Headphone volume 0.0..1.0
    HeadphoneVolume(f32),
    /// Toggle mute on group
    Mute(u8),
    /// Toggle solo on group
    Solo(u8),
    /// Select pattern in group
    PatternSelect {
        /// Group idx
        group: u8,
        /// Pattern idx
        pattern: usize,
    },
    /// Change Left display tab (0..3)
    SetLeftTab(u8),
    /// Change Right display tab (0..3)
    SetRightTab(u8),
    /// Select active sound (0..15)
    SelectSound(u8),
    /// Apply humanize variation to active pattern
    Variation,
    /// Quantize active pattern
    QuantizeActive,
}

/// Snapshot lecture seule pour UI.
#[derive(Debug, Clone)]
pub struct EngineSnapshot {
    /// Project clone (coûteux mais ok pour UI 60fps — on pourrait faire diff plus tard)
    pub project: Project,
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
    /// Soloed groups bitmask
    pub soloed_groups: u8,
    /// Tab écran gauche actif (0: Arrange, 1: Mixer, 2: Sampler, 3: Browser)
    pub left_tab: u8,
    /// Tab écran droit actif (0: Pattern, 1: Events, 2: Macro, 3: Settings)
    pub right_tab: u8,
    /// Son actif sélectionné (0..15)
    pub active_sound: u8,
}

/// Engine principal.
pub struct Engine {
    project: Arc<RwLock<Project>>,
    /// Transport (public pour accès sample_rate depuis audio thread)
    pub transport: Transport,
    sampler: Sampler,
    sequencer: Sequencer,
    group_focus: u8,
    active_scene: Option<usize>,
    // pattern tick cursors pour chaque Group/Pattern actif
    #[allow(dead_code)]
    pattern_cursors: Vec<u64>,
    /// Master volume 0.0..1.0
    master_volume: f32,
    /// Headphone volume 0.0..1.0
    headphone_volume: f32,
    /// Muted groups bitmask
    muted_groups: u8,
    /// Soloed groups bitmask
    soloed_groups: u8,
    /// Tab écran gauche
    pub left_tab: u8,
    /// Tab écran droit
    pub right_tab: u8,
    /// Son actif
    pub active_sound: u8,
}

impl Engine {
    /// Crée avec un projet.
    #[must_use]
    pub fn new(project: Project, sample_rate: u32) -> Self {
        let bpm = project.bpm;
        Self {
            project: Arc::new(RwLock::new(project)),
            transport: Transport::new(bpm, sample_rate),
            sampler: Sampler::new(sample_rate),
            sequencer: Sequencer::new(Grid::Sixteenth, 0.0),
            group_focus: 0,
            active_scene: Some(0),
            pattern_cursors: Vec::new(),
            master_volume: 0.85,
            headphone_volume: 0.85,
            muted_groups: 0,
            soloed_groups: 0,
            left_tab: 0,
            right_tab: 0,
            active_sound: 0,
        }
    }

    /// Démo.
    #[must_use]
    pub fn new_demo(sample_rate: u32) -> Self {
        let mut eng = Self::new(Project::new_demo(), sample_rate);
        eng.load_default_808_kit();
        eng
    }

    /// Handle partagé du projet (pour UI).
    #[must_use]
    pub fn project_handle(&self) -> Arc<RwLock<Project>> {
        self.project.clone()
    }

    /// Snapshot pour UI.
    pub fn snapshot(&self) -> EngineSnapshot {
        let project = self.project.read().clone();
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
            soloed_groups: self.soloed_groups,
            left_tab: self.left_tab,
            right_tab: self.right_tab,
            active_sound: self.active_sound,
        }
    }

    /// Charge le kit 808 complet DSP dans le sampler
    pub fn load_default_808_kit(&mut self) {
        let kit = crate::drum_synth::generate_complete_808_kit(self.transport.sample_rate);
        for (pad, _name, samp) in kit {
            self.sampler.add_zone(
                pad,
                crate::sampler::ZoneMapping {
                    sample: Some(std::sync::Arc::new(samp)),
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
        self.transport.loop_enabled = true;
        self.transport.loop_start = 0;
        self.transport.loop_end = 2 * 4 * 960;
    }

    /// Humanise le pattern actif (légères variations de vélocité et micro-timing)
    pub fn humanize_active_pattern(&mut self) {
        let mut proj = self.project.write();
        if let Some(group) = proj.groups.get_mut(self.group_focus as usize) {
            if let Some(pat) = group.patterns.first_mut() {
                self.sequencer.humanize(&mut pat.events, 8, 12);
            }
        }
    }

    /// Quantifie le pattern actif sur la grille courante
    pub fn quantize_active_pattern(&mut self) {
        let mut proj = self.project.write();
        if let Some(group) = proj.groups.get_mut(self.group_focus as usize) {
            if let Some(pat) = group.patterns.first_mut() {
                self.sequencer.quantize_pattern(pat);
            }
        }
    }

    /// Traite un `EngineEvent`.
    pub fn handle_event(&mut self, ev: EngineEvent) -> crate::Result<()> {
        match ev {
            EngineEvent::Pad {
                pad,
                velocity,
                pressed,
            } => {
                if pressed {
                    // trigger sampler pour le Sound du Group focus
                    let sound = pad;
                    let note = 36 + pad;
                    self.sampler.note_on(sound, note, velocity, note);
                    // si Recording, ajoute event au pattern actif
                    if self.transport.state == TransportState::Recording {
                        self.record_pad_event(pad, velocity);
                    }
                } else {
                    let note = 36 + pad;
                    self.sampler.note_off(pad, note);
                }
            }
            EngineEvent::Transport { play, rec, stop } => {
                if play {
                    self.transport.play();
                }
                if rec {
                    self.transport.record();
                }
                if stop {
                    self.transport.stop();
                }
            }
            EngineEvent::Bpm(bpm) => {
                self.transport.bpm = bpm.clamp(20.0, 999.0);
                self.project.write().bpm = self.transport.bpm;
            }
            EngineEvent::GroupFocus(g) => {
                self.group_focus = g.min(7);
            }
            EngineEvent::Scene(s) => {
                let len = self.project.read().scenes.len();
                if s < len {
                    self.active_scene = Some(s);
                }
            }
            EngineEvent::Quantize { group, pattern } => {
                let mut proj = self.project.write();
                if let Some(g) = proj.groups.get_mut(group) {
                    if let Some(p) = g.patterns.get_mut(pattern) {
                        self.sequencer.quantize_pattern(p);
                    }
                }
            }
            EngineEvent::Save(path) => {
                self.project.read().save_json(&path)?;
            }
            EngineEvent::Load(path) => {
                let p = Project::load_json(&path)?;
                self.transport.bpm = p.bpm;
                *self.project.write() = p;
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
            EngineEvent::Solo(g) => {
                if g < 8 {
                    self.soloed_groups ^= 1 << g;
                }
            }
            EngineEvent::PatternSelect { group, pattern } => {
                // Select a pattern in the focused group for the active scene
                if let Some(scene_idx) = self.active_scene {
                    let mut proj = self.project.write();
                    if let Some(scene) = proj.scenes.get_mut(scene_idx) {
                        if (group as usize) < scene.patterns.len() {
                            scene.patterns[group as usize] = Some(pattern);
                        }
                    }
                }
            }
            EngineEvent::SetLeftTab(tab) => {
                self.left_tab = tab.min(3);
            }
            EngineEvent::SetRightTab(tab) => {
                self.right_tab = tab.min(3);
            }
            EngineEvent::SelectSound(snd) => {
                self.active_sound = snd.min(15);
            }
            EngineEvent::Variation => {
                self.humanize_active_pattern();
            }
            EngineEvent::QuantizeActive => {
                self.quantize_active_pattern();
            }
        }
        Ok(())
    }

    /// Avance le transport + séquenceur, déclenche les notes des patterns.
    pub fn advance(&mut self, samples: u64) {
        let prev_tick = self.transport.tick;
        self.transport.advance_samples(samples);
        let new_tick = self.transport.tick;
        if !self.transport.is_playing() {
            return;
        }
        // Pour chaque Group, joue les events du pattern actif de la scène courante
        let proj = self.project.read().clone(); // clone pour éviter borrow sampler
        if let Some(scene_idx) = self.active_scene {
            if let Some(scene) = proj.scenes.get(scene_idx) {
                for (gi, pat_opt) in scene.patterns.iter().enumerate() {
                    if let Some(pat_idx) = pat_opt {
                        if let Some(group) = proj.groups.get(gi) {
                            if let Some(pat) = group.patterns.get(*pat_idx) {
                                Self::trigger_pattern_events(
                                    pat,
                                    prev_tick,
                                    new_tick,
                                    &mut self.sampler,
                                    gi as u8,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn trigger_pattern_events(
        pat: &crate::project::Pattern,
        prev_tick: u64,
        new_tick: u64,
        sampler: &mut Sampler,
        _group: u8,
    ) {
        let pat_len = pat.length.to_ticks() as u64;
        if pat_len == 0 || prev_tick == new_tick {
            return;
        }
        // gère wrap pattern (loop)
        let prev_in_pat = prev_tick % pat_len;
        let new_in_pat = new_tick % pat_len;
        let wrapped = new_in_pat < prev_in_pat;
        for ev in &pat.events {
            let tick = ev.tick as u64;
            let triggered = if !wrapped {
                if prev_tick == 0 {
                    tick >= prev_in_pat && tick <= new_in_pat
                } else {
                    tick > prev_in_pat && tick <= new_in_pat
                }
            } else {
                tick > prev_in_pat || tick <= new_in_pat
            };
            if triggered {
                let sound = ev.pad;
                let note = ev.pitch;
                sampler.note_on(sound, note, ev.velocity, note);
            }
        }
    }

    fn record_pad_event(&mut self, pad: u8, velocity: u8) {
        let tick_in_pat = (self.transport.tick % (4 * 960)) as u32; // 1 bar loop pour démo
        // quantize en live
        let q_tick = self.sequencer.grid.quantize(tick_in_pat);
        let mut proj = self.project.write();
        if let Some(group) = proj.groups.get_mut(self.group_focus as usize) {
            if group.patterns.is_empty() {
                group.patterns.push(crate::project::Pattern::new(
                    "Recorded",
                    BarsBeatsTicks::new(1, 0, 0),
                ));
            }
            if let Some(pat) = group.patterns.get_mut(0) {
                pat.events.push(PatternEvent {
                    pad,
                    tick: q_tick,
                    velocity,
                    length_ticks: 120,
                    pitch: 36 + pad,
                });
                pat.events.sort_by_key(|e| e.tick);
            }
        }
    }

    /// Render audio stéréo — à appeler depuis callback cpal/jack.
    pub fn render_audio(&mut self, out: &mut [f32]) {
        self.sampler.render(out);
        // Apply master volume
        let mv = self.master_volume;
        for s in out.iter_mut() {
            *s *= mv;
        }
        // TODO: mixer groups, master
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
        self.sequencer.swing = self.transport.swing;
        self.project.write().swing = self.transport.swing;
    }

    /// Group focus actif.
    #[must_use]
    pub fn group_focus(&self) -> u8 {
        self.group_focus
    }

    /// Bascule une note à un pas donné (tick) pour le pad donné dans le pattern actif
    pub fn toggle_step(&mut self, pad: u8, tick: u32) {
        let group_idx = self.group_focus as usize;
        let mut proj = self.project.write();
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if group.patterns.is_empty() {
                group.patterns.push(crate::project::Pattern::new(
                    "Pattern 1",
                    crate::project::BarsBeatsTicks::new(2, 0, 0),
                ));
            }
            if let Some(pat) = group.patterns.get_mut(0) {
                if let Some(pos) = pat.events.iter().position(|e| e.pad == pad && e.tick == tick) {
                    pat.events.remove(pos);
                } else {
                    pat.events.push(crate::project::PatternEvent {
                        pad,
                        tick,
                        velocity: 115,
                        length_ticks: 120,
                        pitch: 36 + pad,
                    });
                    self.sampler.note_on(pad, 36 + pad, 115, 36 + pad);
                }
            }
        }
    }

    /// Remplit les temps 1, 2, 3, 4 pour un pad (ex: Kick 4-on-the-Floor)
    pub fn fill_four_on_the_floor(&mut self, pad: u8, bar: usize) {
        let group_idx = self.group_focus as usize;
        let mut proj = self.project.write();
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if let Some(pat) = group.patterns.get_mut(0) {
                let bar_start = (bar * 4 * 960) as u32;
                let bar_end = bar_start + 4 * 960;
                pat.events.retain(|e| !(e.pad == pad && e.tick >= bar_start && e.tick < bar_end));
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

    /// Remplit les croches (8ths) pour un pad (ex: Charlestons)
    pub fn fill_eighths(&mut self, pad: u8, bar: usize) {
        let group_idx = self.group_focus as usize;
        let mut proj = self.project.write();
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if let Some(pat) = group.patterns.get_mut(0) {
                let bar_start = (bar * 4 * 960) as u32;
                let bar_end = bar_start + 4 * 960;
                pat.events.retain(|e| !(e.pad == pad && e.tick >= bar_start && e.tick < bar_end));
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

    /// Supprime toutes les notes d'une mesure donnée
    pub fn clear_bar(&mut self, bar: usize) {
        let group_idx = self.group_focus as usize;
        let mut proj = self.project.write();
        if let Some(group) = proj.groups.get_mut(group_idx) {
            if let Some(pat) = group.patterns.get_mut(0) {
                let bar_start = (bar * 4 * 960) as u32;
                let bar_end = bar_start + 4 * 960;
                pat.events.retain(|e| !(e.tick >= bar_start && e.tick < bar_end));
            }
        }
    }

    /// Réinitialise le pattern avec le groove 808 complet
    pub fn reset_demo_groove(&mut self) {
        let demo_proj = crate::project::Project::new_demo();
        let mut proj = self.project.write();
        if let Some(demo_g) = demo_proj.groups.get(0) {
            if let Some(demo_p) = demo_g.patterns.get(0) {
                if let Some(g) = proj.groups.get_mut(0) {
                    if !g.patterns.is_empty() {
                        g.patterns[0] = demo_p.clone();
                    } else {
                        g.patterns.push(demo_p.clone());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
