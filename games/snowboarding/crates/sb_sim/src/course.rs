//! The mountain: a course is a winding centreline running downhill, and the
//! snow's height everywhere near it as a function of (s, u), the distance
//! along the centreline and the signed offset to its right. The client
//! meshes the same function the rider's physics samples, at the rows and
//! columns [`Course::mesh_rows`] and [`Course::mesh_columns`] choose.
//!
//! A course is authored as a list of [`Section`]s (grade, width, bends,
//! moguls, rollers, kickers, trees, a half-pipe) and built from its seed:
//! every bump and tree comes from the deterministic noise in [`crate::noise`].

use crate::noise::{Rng, fbm, lerp, smoothstep};
use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

/// The spacing of the centreline's samples, in metres.
pub const DS: f32 = 1.0;
/// Snow behind the start gate (the camera sits up there before the drop).
pub const PRE_START: f32 = 45.0;
/// Snow after the finish line to stop on.
pub const RUN_OUT: f32 = 170.0;
/// How far the snow extends beyond the piste's edge on each side.
pub const WALL: f32 = 55.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CourseId {
    Crystal,
    Timber,
    Avalanche,
    Pipe,
}

impl CourseId {
    pub const ALL: [CourseId; 4] = [
        CourseId::Crystal,
        CourseId::Timber,
        CourseId::Avalanche,
        CourseId::Pipe,
    ];
    pub fn name(self) -> &'static str {
        match self {
            CourseId::Crystal => "Crystal Ridge",
            CourseId::Timber => "Timber Run",
            CourseId::Avalanche => "Avalanche Bowl",
            CourseId::Pipe => "Half Pipe",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            CourseId::Crystal => "Wide groomers, rollers and big friendly kickers.",
            CourseId::Timber => "Tight turns through the pines. Watch the trunks.",
            CourseId::Avalanche => "Steep, fast and huge air. Experts only.",
            CourseId::Pipe => "A superpipe. Ride wall to wall and go big.",
        }
    }
    pub fn difficulty(self) -> u8 {
        match self {
            CourseId::Crystal => 1,
            CourseId::Timber => 2,
            CourseId::Avalanche => 3,
            CourseId::Pipe => 2,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            CourseId::Crystal => "crystal",
            CourseId::Timber => "timber",
            CourseId::Avalanche => "avalanche",
            CourseId::Pipe => "pipe",
        }
    }
    pub fn from_key(k: &str) -> Option<CourseId> {
        CourseId::ALL.into_iter().find(|c| c.key() == k)
    }
}

/// A stretch of the course, as authored.
#[derive(Clone, Debug)]
pub struct Section {
    pub len: f32,
    /// Drop per metre run (0.3 is about 17°).
    pub grade: f32,
    /// Half the groomed piste's width.
    pub half_width: f32,
    /// How hard it winds (1 is a 190 m radius at the tightest).
    pub bend: f32,
    pub moguls: f32,
    pub rollers: f32,
    /// Trees on the piste itself (0 none, 1 a forest with lanes).
    pub trees: f32,
    pub kickers: u32,
    pub kicker_size: f32,
    pub pipe: bool,
}

fn sec(len: f32, grade: f32, half_width: f32) -> Section {
    Section {
        len,
        grade,
        half_width,
        bend: 0.0,
        moguls: 0.0,
        rollers: 0.0,
        trees: 0.0,
        kickers: 0,
        kicker_size: 1.0,
        pipe: false,
    }
}

impl Section {
    fn bend(mut self, b: f32) -> Self {
        self.bend = b;
        self
    }
    fn moguls(mut self, m: f32) -> Self {
        self.moguls = m;
        self
    }
    fn rollers(mut self, r: f32) -> Self {
        self.rollers = r;
        self
    }
    fn trees(mut self, t: f32) -> Self {
        self.trees = t;
        self
    }
    fn kickers(mut self, n: u32, size: f32) -> Self {
        self.kickers = n;
        self.kicker_size = size;
        self
    }
    fn pipe(mut self) -> Self {
        self.pipe = true;
        self
    }
}

fn plan(id: CourseId) -> (u32, Vec<Section>) {
    match id {
        CourseId::Crystal => (
            11,
            vec![
                sec(80.0, 0.22, 26.0),
                sec(200.0, 0.24, 30.0).bend(0.6),
                sec(130.0, 0.26, 28.0).kickers(1, 1.0),
                sec(200.0, 0.22, 32.0).rollers(0.8).bend(0.7),
                sec(190.0, 0.28, 26.0).kickers(2, 0.8).bend(0.4),
                sec(260.0, 0.30, 30.0).bend(1.0),
                sec(130.0, 0.27, 28.0).kickers(1, 1.5),
                sec(200.0, 0.25, 30.0).moguls(0.6).bend(0.5),
                sec(140.0, 0.22, 28.0).kickers(1, 1.1),
                sec(100.0, 0.14, 30.0),
            ],
        ),
        CourseId::Timber => (
            23,
            vec![
                sec(70.0, 0.24, 22.0),
                sec(220.0, 0.27, 20.0).bend(1.1).trees(0.25),
                sec(140.0, 0.28, 22.0).kickers(1, 0.9).bend(0.5),
                sec(260.0, 0.30, 18.0).bend(1.35).trees(0.6),
                sec(150.0, 0.26, 22.0).rollers(1.0),
                sec(160.0, 0.31, 20.0).kickers(2, 1.0).bend(0.6),
                sec(240.0, 0.33, 19.0).bend(1.3).trees(0.75).moguls(0.4),
                sec(150.0, 0.27, 24.0).kickers(1, 1.4),
                sec(110.0, 0.15, 24.0),
            ],
        ),
        CourseId::Avalanche => (
            37,
            vec![
                sec(70.0, 0.28, 30.0),
                sec(160.0, 0.36, 34.0).bend(0.7),
                sec(150.0, 0.38, 32.0).kickers(1, 1.8),
                sec(200.0, 0.42, 30.0).moguls(0.7).bend(0.9),
                sec(170.0, 0.36, 34.0).kickers(2, 1.6),
                sec(220.0, 0.44, 28.0).bend(1.2).trees(0.3),
                sec(160.0, 0.35, 32.0).rollers(1.3),
                sec(170.0, 0.40, 34.0).kickers(1, 2.2),
                sec(130.0, 0.33, 30.0).kickers(1, 1.4),
                sec(110.0, 0.16, 32.0),
            ],
        ),
        CourseId::Pipe => (
            5,
            vec![
                sec(50.0, 0.24, 16.0),
                sec(440.0, 0.30, 5.5).pipe(),
                sec(60.0, 0.14, 16.0),
            ],
        ),
    }
}

/// A kicker: a ramp across part of the piste whose lip is at `s`.
#[derive(Clone, Copy, Debug)]
pub struct Kicker {
    pub s: f32,
    pub u: f32,
    pub half_width: f32,
    pub length: f32,
    pub height: f32,
}

/// How far behind the lip a kicker's back face reaches.
const KICKER_BACK: f32 = 1.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObstacleKind {
    Tree,
    Rock,
}

/// Something to ride around: its base on the snow, its collision radius
/// and height. `scale` is for drawing.
#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    pub kind: ObstacleKind,
    pub pos: Vec3,
    pub s: f32,
    pub u: f32,
    pub radius: f32,
    pub height: f32,
    pub scale: f32,
    pub variant: u32,
}

/// A course marker flag at the piste's edge.
#[derive(Clone, Copy, Debug)]
pub struct Marker {
    pub pos: Vec3,
    pub s: f32,
    pub right_side: bool,
}

/// The centreline's frame at some s.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// On the centreline, at its base elevation.
    pub center: Vec3,
    /// Downhill along the course, horizontal, unit.
    pub dir: Vec3,
    /// To the right of `dir`, horizontal, unit.
    pub right: Vec3,
    pub theta: f32,
    pub kappa: f32,
    pub half_width: f32,
}

/// The snow under a point.
#[derive(Clone, Copy, Debug)]
pub struct Ground {
    pub s: f32,
    pub u: f32,
    pub height: f32,
    pub normal: Vec3,
}

#[derive(Clone, Copy, Debug, Default)]
struct Sample {
    center: Vec2,
    e: f32,
    theta: f32,
    kappa: f32,
    hw: f32,
    moguls: f32,
    rollers: f32,
    trees: f32,
    pipe: f32,
    line: f32,
}

pub struct Course {
    pub id: CourseId,
    pub seed: u32,
    samples: Vec<Sample>,
    pub length: f32,
    pub start_s: f32,
    pub finish_s: f32,
    pub kickers: Vec<Kicker>,
    /// Sorted by s.
    pub obstacles: Vec<Obstacle>,
    pub markers: Vec<Marker>,
    pub checkpoints: Vec<f32>,
    /// The pipe's extent in s, for a pipe course.
    pub pipe: Option<(f32, f32)>,
}

/// A box blur of `r` samples each side, twice (a triangle filter).
fn smooth(v: &mut [f32], r: usize) {
    if r == 0 || v.len() < 2 {
        return;
    }
    for _ in 0..2 {
        let src = v.to_vec();
        let n = src.len() as isize;
        let mut sum = 0.0;
        let get = |i: isize| src[i.clamp(0, n - 1) as usize];
        for i in -(r as isize)..=(r as isize) {
            sum += get(i);
        }
        let w = (2 * r + 1) as f32;
        for i in 0..n {
            v[i as usize] = sum / w;
            sum += get(i + r as isize + 1) - get(i - r as isize);
        }
    }
}

/// The half-pipe's cross-section: height above the flat at |u| = `au`.
pub fn pipe_profile(au: f32) -> f32 {
    const FLAT: f32 = 5.5;
    const R: f32 = 6.5;
    const MAX_ANGLE: f32 = 1.40; // 80°
    let (sin_m, cos_m) = MAX_ANGLE.sin_cos();
    if au <= FLAT {
        0.0
    } else if au <= FLAT + R * sin_m {
        let x = au - FLAT;
        R - (R * R - x * x).max(0.0).sqrt()
    } else {
        let deck = au - (FLAT + R * sin_m);
        let far = (deck - 7.0).max(0.0);
        R * (1.0 - cos_m) + deck * 0.02 + far * far * 0.04
    }
}

/// How far the snow rises `x` metres beyond the piste's edge: steepening
/// banks, then the valley's walls at a steady pitch.
fn bank(x: f32) -> f32 {
    const KNEE: f32 = 35.0;
    if x < KNEE {
        x * x * 0.02 + x * 0.1
    } else {
        let d = x - KNEE;
        KNEE * KNEE * 0.02 + KNEE * 0.1 + d * 1.5 * (1.0 - d / 700.0).max(0.3)
    }
}

/// How far the drawn valley reaches beyond the piste (the physics stops at
/// [`WALL`]).
pub const FAR: f32 = 260.0;

/// Where the pipe's walls end (the coping), in |u|.
pub const PIPE_COPING: f32 = 5.5 + 6.5 * 0.985_449_7;
pub const PIPE_FLAT: f32 = 5.5;

impl Course {
    pub fn new(id: CourseId) -> Course {
        let (seed, sections) = plan(id);
        let total: f32 = sections.iter().map(|s| s.len).sum();
        let start_s = PRE_START;
        let finish_s = start_s + total;
        let length = finish_s + RUN_OUT;
        let n = (length / DS) as usize + 2;
        let mut rng = Rng::new(seed);

        // The authored section at each sample.
        let section_at = |s: f32| -> &Section {
            let mut acc = start_s;
            for sec in &sections {
                acc += sec.len;
                if s < acc {
                    return sec;
                }
            }
            sections.last().unwrap()
        };
        let mut grade = vec![0.0; n];
        let mut hw = vec![0.0; n];
        let mut bend = vec![0.0; n];
        let mut moguls = vec![0.0; n];
        let mut rollers = vec![0.0; n];
        let mut trees = vec![0.0; n];
        let mut pipe = vec![0.0; n];
        for i in 0..n {
            let s = i as f32 * DS;
            let sc = section_at(s);
            // Gentle summit deck behind the gate, pitching into the opening run.
            grade[i] = if s < start_s { 0.08 } else { sc.grade };
            hw[i] = sc.half_width;
            bend[i] = if s < start_s + 50.0 { 0.0 } else { sc.bend };
            moguls[i] = sc.moguls;
            rollers[i] = sc.rollers;
            trees[i] = sc.trees;
            pipe[i] = if sc.pipe { 1.0 } else { 0.0 };
        }
        smooth(&mut grade, 14);
        // Steep summit start ramp right through the gate: rolls over before
        // `start_s` so everything from `start_s` down is a concave transition
        // onto the piste that holds riders cleanly against the snow.
        for (i, g) in grade.iter_mut().enumerate() {
            let s = i as f32 * DS;
            if s > start_s - 6.0 && s < start_s + 32.0 {
                let ramp = if s <= start_s {
                    smoothstep(start_s - 6.0, start_s, s)
                } else {
                    1.0 - smoothstep(start_s, start_s + 32.0, s)
                };
                *g += 0.25 * ramp;
            }
        }
        smooth(&mut hw, 16);
        smooth(&mut bend, 30);
        smooth(&mut moguls, 12);
        smooth(&mut rollers, 12);
        smooth(&mut trees, 10);
        smooth(&mut pipe, 6);

        // Kickers, spaced within their sections, and a steeper landing
        // after each.
        let mut kickers = Vec::new();
        let mut acc = start_s;
        for sc in &sections {
            for j in 0..sc.kickers {
                let s = acc + sc.len * (j as f32 + 0.62) / sc.kickers as f32;
                let size = sc.kicker_size * rng.range(0.9, 1.1);
                let w = sc.half_width;
                kickers.push(Kicker {
                    s,
                    u: rng.range(-0.3, 0.3) * w,
                    half_width: 3.2 + 1.6 * size,
                    length: 7.0 + 3.0 * size,
                    height: 1.2 + 1.0 * size,
                });
            }
            acc += sc.len;
        }
        for k in &kickers {
            let (a, b) = (k.s + 3.0, k.s + 62.0);
            for (i, g) in grade.iter_mut().enumerate() {
                let s = i as f32 * DS;
                if s > a && s < b {
                    *g += 0.28 * (std::f32::consts::PI * (s - a) / (b - a)).sin();
                }
            }
        }

        // The centreline.
        let (p1, p2) = (rng.range(0.0, TAU), rng.range(0.0, TAU));
        let mut samples = vec![Sample::default(); n];
        let mut theta = 0.0f32;
        let mut center = Vec2::ZERO;
        let mut e = 0.0f32;
        for i in 0..n {
            let s = i as f32 * DS;
            let kappa = bend[i]
                * (0.6 * (TAU * s / 380.0 + p1).sin() + 0.4 * (TAU * s / 215.0 + p2).sin())
                / 190.0;
            samples[i] = Sample {
                center,
                e,
                theta,
                kappa,
                hw: hw[i],
                moguls: moguls[i],
                rollers: rollers[i],
                trees: trees[i],
                pipe: pipe[i],
                line: 0.0,
            };
            let mid = theta + kappa * DS * 0.5;
            center += Vec2::new(mid.sin(), -mid.cos()) * DS;
            theta += kappa * DS;
            e -= grade[i] * DS;
        }
        // The racing line: toward the inside of the bends.
        let mut line: Vec<f32> = samples
            .iter()
            .map(|p| (p.kappa * 2600.0).clamp(-0.42 * p.hw, 0.42 * p.hw))
            .collect();
        smooth(&mut line, 20);
        for (p, l) in samples.iter_mut().zip(line) {
            p.line = l;
        }

        let pipe_range = {
            let mut first = None;
            let mut last = None;
            for (i, p) in samples.iter().enumerate() {
                if p.pipe > 0.99 {
                    first.get_or_insert(i as f32 * DS);
                    last = Some(i as f32 * DS);
                }
            }
            first.zip(last)
        };

        let mut course = Course {
            id,
            seed,
            samples,
            length,
            start_s,
            finish_s,
            kickers,
            obstacles: Vec::new(),
            markers: Vec::new(),
            checkpoints: Vec::new(),
            pipe: pipe_range,
        };
        course.place_scenery(&mut rng);
        let span = finish_s - start_s;
        course.checkpoints = (1..4).map(|k| start_s + span * k as f32 / 4.0).collect();
        course
    }

    fn place_scenery(&mut self, rng: &mut Rng) {
        let mut obstacles = Vec::new();
        let mut s = 4.0;
        let in_kicker_zone = |c: &Course, s: f32| {
            c.kickers
                .iter()
                .any(|k| s > k.s - k.length - 22.0 && s < k.s + 48.0)
        };
        while s < self.length - 4.0 {
            let p = self.sample(s);
            let edge = if p.pipe > 0.5 {
                PIPE_COPING + 4.0
            } else {
                p.hw
            };
            // The forest on the banks.
            for side in [-1.0f32, 1.0] {
                for _ in 0..3 {
                    if !rng.chance(0.62) {
                        continue;
                    }
                    let d = rng.f();
                    let u = side * (edge + 3.0 + d * d * (WALL - 6.0));
                    let ss = s + rng.range(-1.5, 1.5);
                    self.push_tree(&mut obstacles, rng, ss, u);
                }
            }
            // Trees on the piste, leaving lanes.
            if p.trees > 0.05 && !in_kicker_zone(self, s) && rng.chance(p.trees * 0.55) {
                let u = rng.range(-p.hw + 2.5, p.hw - 2.5);
                if (u - p.line).abs() > 5.5 {
                    self.push_tree(&mut obstacles, rng, s, u);
                }
            }
            // Rocks at the edges.
            if rng.chance(0.08) {
                let side = if rng.chance(0.5) { -1.0 } else { 1.0 };
                let u = side * (edge + rng.range(1.5, 9.0));
                let ss = s + rng.range(-1.0, 1.0);
                let scale = rng.range(0.8, 2.2);
                obstacles.push(Obstacle {
                    kind: ObstacleKind::Rock,
                    pos: self.world(ss, u) - Vec3::Y * 0.3 * scale,
                    s: ss,
                    u,
                    radius: 0.9 * scale,
                    height: 0.9 * scale,
                    scale,
                    variant: rng.next_u32() % 4,
                });
            }
            s += 3.0;
        }
        obstacles.sort_by(|a, b| a.s.total_cmp(&b.s));
        self.obstacles = obstacles;

        // Marker flags along both edges.
        let mut s = self.start_s;
        while s < self.finish_s {
            let p = self.sample(s);
            if p.pipe < 0.5 {
                for (u, right_side) in [(-(p.hw + 0.6), false), (p.hw + 0.6, true)] {
                    self.markers.push(Marker {
                        pos: self.world(s, u),
                        s,
                        right_side,
                    });
                }
            }
            s += 24.0;
        }
    }

    fn push_tree(&self, out: &mut Vec<Obstacle>, rng: &mut Rng, s: f32, u: f32) {
        if s < 0.0 || s > self.length {
            return;
        }
        let scale = rng.range(0.75, 1.45);
        out.push(Obstacle {
            kind: ObstacleKind::Tree,
            pos: self.world(s, u),
            s,
            u,
            radius: 0.55 * scale,
            height: 9.0 * scale,
            scale,
            variant: rng.next_u32() % 3,
        });
    }

    fn sample(&self, s: f32) -> Sample {
        let x = (s / DS).clamp(0.0, (self.samples.len() - 1) as f32);
        let i = (x as usize).min(self.samples.len() - 2);
        let t = x - i as f32;
        let (a, b) = (&self.samples[i], &self.samples[i + 1]);
        Sample {
            center: a.center.lerp(b.center, t),
            e: lerp(a.e, b.e, t),
            theta: lerp(a.theta, b.theta, t),
            kappa: lerp(a.kappa, b.kappa, t),
            hw: lerp(a.hw, b.hw, t),
            moguls: lerp(a.moguls, b.moguls, t),
            rollers: lerp(a.rollers, b.rollers, t),
            trees: lerp(a.trees, b.trees, t),
            pipe: lerp(a.pipe, b.pipe, t),
            line: lerp(a.line, b.line, t),
        }
    }

    pub fn frame(&self, s: f32) -> Frame {
        let p = self.sample(s);
        let (sin, cos) = p.theta.sin_cos();
        Frame {
            center: Vec3::new(p.center.x, p.e, p.center.y),
            dir: Vec3::new(sin, 0.0, -cos),
            right: Vec3::new(cos, 0.0, sin),
            theta: p.theta,
            kappa: p.kappa,
            half_width: p.hw,
        }
    }

    /// Half the piste's width at s (the pipe's coping on a pipe).
    pub fn half_width(&self, s: f32) -> f32 {
        let p = self.sample(s);
        lerp(p.hw, PIPE_COPING, p.pipe)
    }

    /// Where the CPU riders like to be across the piste at s.
    pub fn racing_line(&self, s: f32) -> f32 {
        self.sample(s).line
    }

    /// How much of the pipe is at s (0 to 1).
    pub fn pipe_weight(&self, s: f32) -> f32 {
        self.sample(s).pipe
    }

    /// The snow's height at (s, u).
    pub fn height(&self, s: f32, u: f32) -> f32 {
        let p = self.sample(s);
        let au = u.abs();
        let hw = p.hw;
        let uc = u.clamp(-hw, hw);
        let mut h = p.e;
        // Banked turns: the outside of a bend raised.
        h += (-p.kappa * 55.0).clamp(-0.2, 0.2) * uc;
        // The piste's dish, then the banks.
        h += 0.0022 * uc * uc;
        let x = (au - hw).max(0.0);
        h += bank(x);
        let seed = self.seed;
        h += fbm(s * 0.035, u * 0.035, seed, 3) * (0.45 + x.min(60.0) * 0.1);
        if x > 40.0 {
            // The valley's far walls: ridges and gullies.
            h += fbm(s * 0.008, u * 0.008, seed.wrapping_add(31), 4) * (x - 40.0) * 0.35;
        }
        h += fbm(s * 0.11, u * 0.11, seed.wrapping_add(9), 2) * 0.16;
        let piste = smoothstep(hw + 6.0, hw - 2.0, au);
        if p.moguls > 0.001 {
            let m = (s * 0.45).sin() * (u * 0.45 + (s * 0.13).sin() * 1.5).sin();
            h += p.moguls * 0.4 * m * piste;
        }
        if p.rollers > 0.001 {
            h += p.rollers * 1.0 * (s * TAU / 34.0).sin() * piste;
        }
        if p.pipe > 0.001 {
            h = lerp(h, p.e + pipe_profile(au), p.pipe);
        }
        h + self.kicker_height(s, u)
    }

    fn kicker_height(&self, s: f32, u: f32) -> f32 {
        let mut add = 0.0f32;
        for k in &self.kickers {
            let ds = s - k.s;
            if ds < -k.length || ds > KICKER_BACK {
                continue;
            }
            let prof = if ds <= 0.0 {
                let t = (ds + k.length) / k.length;
                k.height * t.powf(1.7)
            } else {
                k.height * (1.0 - ds / KICKER_BACK)
            };
            let lat = smoothstep(k.half_width + 1.5, k.half_width, (u - k.u).abs());
            add = add.max(prof * lat);
        }
        add
    }

    /// The point on the snow at (s, u).
    pub fn world(&self, s: f32, u: f32) -> Vec3 {
        let f = self.frame(s);
        let p = f.center + f.right * u;
        Vec3::new(p.x, self.height(s, u), p.z)
    }

    /// (s, u) of a point, searching from `hint` (a nearby s).
    pub fn locate(&self, p: Vec3, hint: f32) -> (f32, f32) {
        let mut s = hint.clamp(0.0, self.length);
        let mut u = 0.0;
        for _ in 0..6 {
            let f = self.frame(s);
            let d = Vec3::new(p.x - f.center.x, 0.0, p.z - f.center.z);
            let a = d.dot(f.dir);
            u = d.dot(f.right);
            let scale = (1.0 - f.kappa * u).max(0.25);
            let next = (s + a / scale).clamp(0.0, self.length);
            let moved = (next - s).abs();
            s = next;
            if moved < 1e-3 {
                break;
            }
        }
        (s, u)
    }

    /// The snow under a point.
    pub fn ground(&self, p: Vec3, hint: f32) -> Ground {
        let (s, u) = self.locate(p, hint);
        self.ground_su(s, u)
    }

    pub fn ground_su(&self, s: f32, u: f32) -> Ground {
        const E: f32 = 0.12;
        let f = self.frame(s);
        let h = self.height(s, u);
        let hs = (self.height(s + E, u) - self.height(s - E, u)) / (2.0 * E);
        let hu = (self.height(s, u + E) - self.height(s, u - E)) / (2.0 * E);
        let scale = (1.0 - f.kappa * u).max(0.25);
        let g = f.dir * (hs / scale) + f.right * hu;
        Ground {
            s,
            u,
            height: h,
            normal: Vec3::new(-g.x, 1.0, -g.z).normalize(),
        }
    }

    /// The s of the rows the client meshes: 1.5 m apart, closer on kickers.
    pub fn mesh_rows(&self) -> Vec<f32> {
        let mut rows = Vec::new();
        let mut s = 0.0;
        while s < self.length {
            rows.push(s);
            let fine = self
                .kickers
                .iter()
                .any(|k| s > k.s - k.length - 1.5 && s < k.s + KICKER_BACK + 1.0);
            let ramp = s > self.start_s - 7.0 && s < self.start_s + 14.0;
            s += if fine {
                0.35
            } else if ramp {
                0.6
            } else {
                1.5
            };
        }
        rows.push(self.length);
        rows
    }

    /// The u of the columns the client meshes: fine on the piste, coarse up
    /// the banks.
    pub fn mesh_columns(&self) -> Vec<f32> {
        let max_hw = self.samples.iter().map(|p| p.hw).fold(0.0, f32::max);
        let (fine_step, fine_to) = if self.pipe.is_some() {
            (0.5, PIPE_COPING + 2.0)
        } else {
            (1.0, max_hw + 6.0)
        };
        let reach = if self.pipe.is_some() {
            PIPE_COPING + 4.0 + WALL
        } else {
            max_hw + WALL
        };
        let far = reach - WALL + FAR;
        let mut half = vec![0.0];
        let mut u = 0.0;
        while u < far {
            u += if u < fine_to {
                fine_step
            } else if u < fine_to + 16.0 {
                1.5
            } else if u < reach {
                3.0
            } else {
                (6.0 + (u - reach) * 0.12).min(24.0)
            };
            half.push(u.min(far));
        }
        let mut cols: Vec<f32> = half.iter().rev().map(|u| -u).collect();
        cols.extend(half.iter().skip(1));
        cols
    }

    /// The obstacles whose s is within [a, b].
    pub fn obstacles_between(&self, a: f32, b: f32) -> &[Obstacle] {
        let lo = self.obstacles.partition_point(|o| o.s < a);
        let hi = self.obstacles.partition_point(|o| o.s <= b);
        &self.obstacles[lo..hi]
    }

    /// The next kicker whose lip is ahead of s, within `reach`.
    pub fn kicker_ahead(&self, s: f32, reach: f32) -> Option<&Kicker> {
        self.kickers.iter().find(|k| k.s > s && k.s < s + reach)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_course_builds_and_descends() {
        for id in CourseId::ALL {
            let c = Course::new(id);
            assert!(c.length > 400.0);
            let top = c.world(c.start_s, 0.0).y;
            let bottom = c.world(c.finish_s, 0.0).y;
            assert!(top - bottom > 80.0, "{id:?} drops {}", top - bottom);
            let mut s = 0.0;
            while s < c.length {
                for u in [-40.0, -10.0, 0.0, 7.0, 30.0] {
                    let h = c.height(s, u);
                    assert!(h.is_finite());
                }
                s += 3.3;
            }
        }
    }

    #[test]
    fn locate_inverts_world() {
        let c = Course::new(CourseId::Timber);
        for i in 0..200 {
            let s = 20.0 + i as f32 * 7.3;
            for u in [-25.0, -3.0, 0.0, 12.0, 30.0] {
                let p = c.world(s, u);
                let (s2, u2) = c.locate(p, s + 9.0);
                assert!((s2 - s).abs() < 0.05, "s {s} → {s2}");
                assert!((u2 - u).abs() < 0.05, "u {u} → {u2}");
            }
        }
    }

    #[test]
    fn normals_point_up_and_downhill() {
        let c = Course::new(CourseId::Crystal);
        let g = c.ground_su(300.0, 0.0);
        assert!(g.normal.y > 0.85);
        let f = c.frame(300.0);
        assert!(
            g.normal.dot(f.dir) > 0.05,
            "the slope falls along the course"
        );
    }

    #[test]
    fn mesh_grid_is_ordered() {
        for id in CourseId::ALL {
            let c = Course::new(id);
            let cols = c.mesh_columns();
            assert!(cols.windows(2).all(|w| w[1] > w[0]));
            let rows = c.mesh_rows();
            assert!(rows.windows(2).all(|w| w[1] > w[0]));
        }
    }
}
