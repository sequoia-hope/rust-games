//! Animators that are data (DECISIONS D1215). An [`Animator`] is a closure
//! of `world.updaters`, which cannot leave the thread that built it; a
//! streamed section built in the web's worker (stage 4c) hands its scene
//! over as bytes, so its animators are described here as plain data,
//! serialised beside the scene (`section::pack`) and made into animators
//! again on the other side. The open world's kits (the coast, the beach)
//! register theirs this way; a kit that pushes a closure still animates
//! wherever it is built in the client's own thread, and loses that
//! animator in the worker (the packed log says so).
//!
//! Each variant keeps the closure's state (its clock, its angle) with its
//! handles, so an animator read back carries on where the one written
//! stopped; one freshly built starts at zero, as the closure did.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::object::{MaterialId, NodeId};
use crate::three_geom::{Euler, Quaternion};
use crate::world::{Animator, Change, Edit, Handle, SceneEdit, SceneRef, UpdateCtx};
use pk_math::{lerp, smoothstep};

/// One animator as data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AnimatorDesc {
    /// Dew on the tarmac after dark (the road's `setNight`): `uWet` is
    /// `smoothstep(0.55, 1, night) * 0.85`; lamps and headlights glint.
    Dew { mats: Vec<MaterialId> },
    /// The coast's surf and foam: `uTime` runs on one clock and `uBright`
    /// dims to 0.32 at night.
    Surf { mats: Vec<MaterialId>, time: f64 },
    /// The lighthouse: the beam's pivot turns at 0.9 rad/s about `pos`,
    /// the beam's `uStrength` and the glow's opacity and size follow
    /// nightfall.
    Lighthouse {
        pivot: NodeId,
        pos: [f64; 3],
        beam_mat: MaterialId,
        glow_mat: MaterialId,
        glow: NodeId,
        ry: f64,
    },
    /// A material's colour at `rgb * (a + b * night)` (the coffee shack's
    /// string lights, the reflectors).
    NightColor {
        mat: MaterialId,
        rgb: [f64; 3],
        a: f64,
        b: f64,
    },
    /// The fishing boats offshore and their running lights.
    Boats(crate::coast::Boats),
    /// Seabright's Ferris wheel, coaster, surf, light pools and signals.
    Beach(crate::beach::Animate),
}

impl AnimatorDesc {
    /// The animator this describes.
    pub fn into_animator(self) -> Box<dyn Animator> {
        Box::new(self)
    }
}

impl Animator for AnimatorDesc {
    fn update(&mut self, u: &UpdateCtx, out: &mut Vec<Edit>) {
        match self {
            AnimatorDesc::Dew { mats } => {
                let v = smoothstep(0.55, 1.0, u.night) * 0.85;
                for &m in mats.iter() {
                    out.push(Edit {
                        target: Handle::Material(m),
                        change: Change::Number {
                            prop: "uWet",
                            value: v,
                        },
                    });
                }
            }
            AnimatorDesc::Surf { mats, time } => {
                *time += u.dt;
                for &m in mats.iter() {
                    out.push(Edit {
                        target: Handle::Material(m),
                        change: Change::Number {
                            prop: "uTime",
                            value: *time,
                        },
                    });
                    out.push(Edit {
                        target: Handle::Material(m),
                        change: Change::Number {
                            prop: "uBright",
                            value: lerp(1.0, 0.32, u.night),
                        },
                    });
                }
            }
            AnimatorDesc::Lighthouse {
                pivot,
                pos,
                beam_mat,
                glow_mat,
                glow,
                ry,
            } => {
                *ry += u.dt * 0.9;
                let q = Quaternion::from_euler(&Euler::new(0.0, *ry, 0.0));
                out.push(Edit {
                    target: Handle::Node(*pivot),
                    change: Change::Transform {
                        position: *pos,
                        quaternion: [q.x, q.y, q.z, q.w],
                        scale: [1.0, 1.0, 1.0],
                    },
                });
                out.push(Edit {
                    target: Handle::Material(*beam_mat),
                    change: Change::Number {
                        prop: "uStrength",
                        value: 0.03 + 0.32 * smoothstep(0.2, 0.8, u.night),
                    },
                });
                out.push(Edit {
                    target: Handle::Material(*glow_mat),
                    change: Change::Number {
                        prop: "opacity",
                        value: 0.25 + 0.75 * u.night,
                    },
                });
                let sc = 6.0 + 8.0 * u.night;
                out.push(Edit {
                    target: Handle::Node(*glow),
                    change: Change::Transform {
                        position: *pos,
                        quaternion: [0.0, 0.0, 0.0, 1.0],
                        scale: [sc, sc, sc],
                    },
                });
            }
            AnimatorDesc::NightColor { mat, rgb, a, b } => {
                let k = *a + *b * u.night;
                out.push(Edit {
                    target: Handle::Material(*mat),
                    change: Change::Color {
                        prop: "color",
                        rgb: [rgb[0] * k, rgb[1] * k, rgb[2] * k],
                    },
                });
            }
            AnimatorDesc::Boats(b) => b.update(u, out),
            AnimatorDesc::Beach(b) => b.update(u, out),
        }
    }
}

/// Registers a data animator with a build: it runs as an animator, and
/// its description is kept for a packed section.
pub fn push(
    animators: &mut Vec<Box<dyn Animator>>,
    descs: &mut Vec<AnimatorDesc>,
    d: AnimatorDesc,
) {
    animators.push(Box::new(d.clone()));
    descs.push(d);
}

/// Identifier of an animated transform or instance whose displayed position is tracked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrackedTarget {
    /// A scene node (`SceneRef::Node(n)`).
    Node(u32),
    /// One instance of an instanced mesh (`SceneRef::Node(node)`, `index`).
    Instance { node: u32, index: u32 },
    /// A named or numbered external entity (e.g. player car or traffic slot).
    Entity(u32),
}

/// A discontinuity in an animated object's displayed position across consecutive frames.
#[derive(Clone, Debug, PartialEq)]
pub enum PositionAnomaly {
    /// Single-step jump exceeding the maximum plausible displacement for `dt`.
    Teleport {
        target: TrackedTarget,
        frame: usize,
        step: f64,
        max_step: f64,
        from: [f64; 3],
        to: [f64; 3],
    },
    /// Freeze-then-snap staircase: an actively moving object stayed frozen for
    /// `frozen_frames` positive-`dt` steps and then jumped by a multiple of its
    /// normal step.
    StaircaseSnap {
        target: TrackedTarget,
        frame: usize,
        frozen_frames: usize,
        prev_moving_step: f64,
        snap_step: f64,
        from: [f64; 3],
        to: [f64; 3],
    },
}

#[derive(Clone, Copy, Debug)]
struct TargetHistory {
    pos: [f64; 3],
    visible: bool,
    last_moving_step: f64,
    frozen_frames: usize,
}

/// Tracks frame-to-frame displayed positions of animated scene nodes,
/// instanced-mesh slots, and poses to catch teleports and freeze-then-snap
/// staircase jitter.
#[derive(Clone, Debug)]
pub struct PositionTracker {
    /// Maximum allowed speed (m/s) before a displacement is flagged as a teleport.
    pub max_speed: f64,
    /// Fixed distance tolerance (m) added to `max_speed * dt`.
    pub slop: f64,
    /// Minimum normal step (m) for an entity to be considered actively moving
    /// for staircase detection.
    pub min_moving_step: f64,
    /// Distance threshold (m) below which a positive-`dt` frame counts as frozen.
    pub freeze_eps: f64,
    /// Ratio `snap_step / prev_moving_step` above which a post-freeze jump is
    /// flagged as a `StaircaseSnap`.
    pub snap_ratio: f64,
    frame: usize,
    states: BTreeMap<TrackedTarget, TargetHistory>,
    anomalies: Vec<PositionAnomaly>,
}

impl Default for PositionTracker {
    fn default() -> Self {
        Self::new(90.0, 0.5)
    }
}

impl PositionTracker {
    /// Creates a tracker allowing up to `max_speed * dt + slop` per frame.
    pub fn new(max_speed: f64, slop: f64) -> Self {
        Self {
            max_speed,
            slop,
            min_moving_step: 0.02,
            freeze_eps: 1e-4,
            snap_ratio: 3.0,
            frame: 0,
            states: BTreeMap::new(),
            anomalies: Vec::new(),
        }
    }

    /// All position anomalies recorded so far.
    pub fn anomalies(&self) -> &[PositionAnomaly] {
        &self.anomalies
    }

    /// Clears recorded anomalies while keeping current positions, or resets
    /// tracking state across a known camera/player cut.
    pub fn reset_positions(&mut self) {
        self.states.clear();
    }

    /// Observes a single frame of [`SceneEdit`]s with step `dt`.
    pub fn observe_edits(&mut self, dt: f64, edits: &[SceneEdit]) {
        let frame = self.frame;
        self.frame += 1;
        for e in edits {
            match (e.target, &e.change) {
                (
                    SceneRef::Node(n),
                    Change::Transform {
                        position, scale, ..
                    },
                ) => {
                    let vis = scale[0].abs() + scale[1].abs() + scale[2].abs() > 1e-6;
                    self.observe_target(TrackedTarget::Node(n), frame, dt, *position, vis);
                }
                (SceneRef::Node(n), Change::InstanceMatrix { index, matrix }) => {
                    let sx = f64::from(matrix[0]).abs()
                        + f64::from(matrix[1]).abs()
                        + f64::from(matrix[2]).abs();
                    let sy = f64::from(matrix[4]).abs()
                        + f64::from(matrix[5]).abs()
                        + f64::from(matrix[6]).abs();
                    let sz = f64::from(matrix[8]).abs()
                        + f64::from(matrix[9]).abs()
                        + f64::from(matrix[10]).abs();
                    let vis = sx + sy + sz > 1e-5;
                    let pos = [
                        f64::from(matrix[12]),
                        f64::from(matrix[13]),
                        f64::from(matrix[14]),
                    ];
                    self.observe_target(
                        TrackedTarget::Instance {
                            node: n,
                            index: *index,
                        },
                        frame,
                        dt,
                        pos,
                        vis,
                    );
                }
                _ => {}
            }
        }
    }

    /// Observes one target's displayed position at `frame` after `dt` seconds.
    pub fn observe_target(
        &mut self,
        target: TrackedTarget,
        frame: usize,
        dt: f64,
        pos: [f64; 3],
        visible: bool,
    ) {
        let Some(prev) = self.states.get_mut(&target) else {
            self.states.insert(
                target,
                TargetHistory {
                    pos,
                    visible,
                    last_moving_step: 0.0,
                    frozen_frames: 0,
                },
            );
            return;
        };
        if !visible || !prev.visible {
            prev.pos = pos;
            prev.visible = visible;
            prev.last_moving_step = 0.0;
            prev.frozen_frames = 0;
            return;
        }
        let dx = pos[0] - prev.pos[0];
        let dy = pos[1] - prev.pos[1];
        let dz = pos[2] - prev.pos[2];
        let step = (dx * dx + dy * dy + dz * dz).sqrt();
        let max_step = self.max_speed * dt.max(1.0 / 240.0) + self.slop;
        if step > max_step {
            self.anomalies.push(PositionAnomaly::Teleport {
                target,
                frame,
                step,
                max_step,
                from: prev.pos,
                to: pos,
            });
        }
        if dt > 0.0 {
            if step <= self.freeze_eps {
                if prev.last_moving_step >= self.min_moving_step {
                    prev.frozen_frames += 1;
                }
            } else {
                if prev.frozen_frames >= 2
                    && prev.last_moving_step >= self.min_moving_step
                    && step > prev.last_moving_step * self.snap_ratio
                {
                    self.anomalies.push(PositionAnomaly::StaircaseSnap {
                        target,
                        frame,
                        frozen_frames: prev.frozen_frames,
                        prev_moving_step: prev.last_moving_step,
                        snap_step: step,
                        from: prev.pos,
                        to: pos,
                    });
                }
                prev.frozen_frames = 0;
                prev.last_moving_step = step;
            }
        }
        prev.pos = pos;
        prev.visible = visible;
    }
}
