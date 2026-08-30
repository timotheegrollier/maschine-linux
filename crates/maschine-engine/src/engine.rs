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
}

/// Engine principal.
pub struct Engine {
    project: Arc<RwLock<Project>>,
    transport: Transport,
    sampler: Sampler,
    sequencer: Sequencer,
    group_focus: u8,
    active_scene: Option<usize>,
    // pattern tick cursors pour chaque Group/Pattern actif
    #[allow(dead_code)]
    pattern_cursors: Vec<u64>,
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
        }
    }

    /// Démo.
    #[must_use]
    pub fn new_demo(sample_rate: u32) -> Self {
        Self::new(Project::new_demo(), sample_rate)
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
        if pat_len == 0 {
            return;
        }
        // gère wrap pattern (loop)
        let prev_in_pat = prev_tick % pat_len;
        let new_in_pat = new_tick % pat_len;
        let wrapped = new_in_pat < prev_in_pat;
        for ev in &pat.events {
            let tick = ev.tick as u64;
            let triggered = if !wrapped {
                tick > prev_in_pat && tick <= new_in_pat
            } else {
                tick > prev_in_pat || tick <= new_in_pat
            };
            if triggered {
                let sound = ev.pad;
                let note = ev.pitch;
                sampler.note_on(sound, note, ev.velocity, note);
                // note off après length — simplifié : on laisse le sampler cutoff via sample len
                // TODO: scheduler note_off après length_ticks
            }
        }
    }

    fn record_pad_event(&mut self, pad: u8, velocity: u8) {
        let tick_in_pat = (self.transport.tick % 4 * 960) as u32; // 1 bar loop pour démo
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
        // TODO: mixer groups, master
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
