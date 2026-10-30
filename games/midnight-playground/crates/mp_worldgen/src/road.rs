//! Port of `src/world/Road.js`: the generic `extrude` sweep and the run
//! helpers (`runs`, `chunks`, `groupRuns`) the road and the scenery use
//! (roadmap WP 3.3), and the [`Road`] class (WP 3.5).
//!
//! Road surface, shoulders, markings and barriers, all extruded along the
//! track centreline from cross-section profiles.
//!
//! The asphalt, shoulder and markings shaders (the GLSL `patchAsphalt`,
//! the shoulder patch and `patchMarkings` inject) are the renderer's: the
//! scene carries their material kinds and uniforms (SPEC 6.2).

// Index loops stay index loops (DECISIONS D52).
#![allow(clippy::needless_range_loop)]

use mp_scene::{MaterialKind, three};
use mp_track::road_types::{ROAD_TYPES, RoadType};
use mp_track::{Frame, Track};
use pk_math::{clamp, js, kernel};
use serde_json::Value;

use crate::colorizer::Colorizer;
use crate::material::{Material, num, texture_value};
use crate::object::{Instances, Layer, MaterialId, NodeId, SceneGraph, TextureId};
use crate::terrain::Terrain;
use crate::textures::TextureCache;
use crate::three_geom::{
    BufferAttribute, BufferGeometry, Euler, Matrix4, Quaternion, Vector3, box_geometry,
    cylinder_geometry, plane_geometry,
};
use crate::world::{Change, Edit, Handle};

pub const ROAD_STEP: f64 = 2.0;

/// `(f, s) => number`.
pub type LatFn = Box<dyn Fn(&Frame, f64) -> f64 + Send + Sync>;

/// `(f, s, p) => [r, g, b]`.
pub type ColorFn = Box<dyn Fn(&Frame, f64, usize) -> [f64; 3] + Send + Sync>;

/// A profile value: a number, or a function of the frame and s (`(f, s)`).
pub enum Lat {
    Num(f64),
    Fn(LatFn),
}

impl Lat {
    /// `(f, s) => ...`.
    pub fn f(f: impl Fn(&Frame, f64) -> f64 + Send + Sync + 'static) -> Lat {
        Lat::Fn(Box::new(f))
    }
}

impl From<f64> for Lat {
    fn from(v: f64) -> Lat {
        Lat::Num(v)
    }
}

/// One point of a cross-section: `{lat, dy, u, abs, gapAfter}`.
pub struct ProfilePoint {
    pub lat: Lat,
    /// `None` is the JS's missing `dy` (0).
    pub dy: Option<Lat>,
    /// `None`: u is `lat / 4`.
    pub u: Option<f64>,
    /// y is `dy` alone, not relative to the road.
    pub abs: bool,
    /// No faces between this point and the next.
    pub gap_after: bool,
}

impl ProfilePoint {
    /// `{ lat }`.
    pub fn new(lat: impl Into<Lat>) -> ProfilePoint {
        ProfilePoint {
            lat: lat.into(),
            dy: None,
            u: None,
            abs: false,
            gap_after: false,
        }
    }

    /// `{ ..., dy }`.
    pub fn dy(mut self, dy: impl Into<Lat>) -> ProfilePoint {
        self.dy = Some(dy.into());
        self
    }

    /// `{ ..., u }`.
    pub fn u(mut self, u: f64) -> ProfilePoint {
        self.u = Some(u);
        self
    }

    pub fn abs(mut self) -> ProfilePoint {
        self.abs = true;
        self
    }

    pub fn gap_after(mut self) -> ProfilePoint {
        self.gap_after = true;
        self
    }
}

/// `color`: one colour for every vertex, or `(f, s, p) => [r, g, b]`.
pub enum ExtrudeColor {
    Rgb([f64; 3]),
    Fn(ColorFn),
}

/// `extrude`'s options: `{ step = ROAD_STEP, vScale = 8, flat = false,
/// color = null }` (`flat` is unused by the JS body and not ported).
pub struct ExtrudeOpts {
    pub step: f64,
    pub v_scale: f64,
    pub color: Option<ExtrudeColor>,
}

impl Default for ExtrudeOpts {
    fn default() -> Self {
        ExtrudeOpts {
            step: ROAD_STEP,
            v_scale: 8.0,
            color: None,
        }
    }
}

/// Generic extrusion. profile: [{lat(f,i), dy(f,i), u}] across the section.
/// ranges: [[s0, s1], ...]. Produces one BufferGeometry.
pub fn extrude(
    track: &Track,
    ranges: &[[f64; 2]],
    profile: &[ProfilePoint],
    o: &ExtrudeOpts,
) -> BufferGeometry {
    let mut pos: Vec<f64> = Vec::new();
    let mut uv: Vec<f64> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let mut col: Vec<f64> = Vec::new();
    let p_len = profile.len();
    for &[s0, s1] in ranges {
        if s1 - s0 < 0.5 {
            continue;
        }
        let base = pos.len() / 3;
        let mut rows = 0;
        let mut s = s0;
        loop {
            let ss = js::min(s, s1);
            let f = track.frame(ss);
            for (p, pr) in profile.iter().enumerate() {
                let lat = match &pr.lat {
                    Lat::Fn(l) => l(&f, ss),
                    Lat::Num(v) => *v,
                };
                let dy = match &pr.dy {
                    Some(Lat::Fn(d)) => d(&f, ss),
                    // `pr.dy || 0`
                    Some(Lat::Num(v)) => js::or(*v, 0.0),
                    None => 0.0,
                };
                let x = f.x + f.rx * lat;
                let z = f.z + f.rz * lat;
                let y = (if pr.abs { 0.0 } else { f.y - lat * f.bank }) + dy;
                pos.extend_from_slice(&[x, y, z]);
                uv.extend_from_slice(&[pr.u.unwrap_or(lat / 4.0), ss / o.v_scale]);
                match &o.color {
                    Some(ExtrudeColor::Rgb(c)) => col.extend_from_slice(c),
                    Some(ExtrudeColor::Fn(cf)) => col.extend_from_slice(&cf(&f, ss, p)),
                    None => {}
                }
            }
            rows += 1;
            if ss >= s1 {
                break;
            }
            s += o.step;
        }
        for r in 0..rows.max(1) - 1 {
            for p in 0..p_len.saturating_sub(1) {
                if profile[p].gap_after {
                    continue;
                }
                let a = (base + r * p_len + p) as u32;
                let b = a + 1;
                let c = a + p_len as u32;
                let d = c + 1;
                idx.extend_from_slice(&[a, b, c, b, d, c]);
            }
        }
    }
    let mut g = BufferGeometry::new();
    g.set_attribute("position", BufferAttribute::from_f64(&pos, 3));
    g.set_attribute("uv", BufferAttribute::from_f64(&uv, 2));
    if o.color.is_some() {
        g.set_attribute("color", BufferAttribute::from_f64(&col, 3));
    }
    g.set_index(&idx);
    g.compute_vertex_normals();
    g
}

/// Split [0, len] into chunks for frustum culling (`size` 560 in the JS
/// default).
pub fn chunks(s0: f64, s1: f64, size: f64) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    let mut s = s0;
    while s < s1 {
        out.push([s, js::min(s1, s + size)]);
        s += size;
    }
    out
}

/// Split runs into chunks and bundle neighbouring pieces so each mesh covers
/// about `size` metres of road (fewer draw calls, still frustum-cullable).
pub fn group_runs(ranges: &[[f64; 2]], size: f64) -> Vec<Vec<[f64; 2]>> {
    let pieces: Vec<[f64; 2]> = ranges
        .iter()
        .flat_map(|r| chunks(r[0], r[1], size))
        .collect();
    let mut groups = Vec::new();
    let mut cur: Vec<[f64; 2]> = Vec::new();
    let mut start: Option<f64> = None;
    for p in pieces {
        if start.is_none() {
            start = Some(p[0]);
        }
        if p[1] - start.unwrap() > size && !cur.is_empty() {
            groups.push(std::mem::take(&mut cur));
            start = Some(p[0]);
        }
        cur.push(p);
    }
    if !cur.is_empty() {
        groups.push(cur);
    }
    groups
}

/// Runs where pred(s) holds, sampled every `step` metres from `s0` to `s1`
/// (the JS defaults: 2, 0 and `track.length`).
pub fn runs(pred: impl Fn(f64) -> bool, step: f64, s0: f64, s1: f64) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    let mut start: Option<f64> = None;
    let mut s = s0;
    while s <= s1 {
        let ok = pred(s);
        if ok && start.is_none() {
            start = Some(s);
        }
        if !ok && let Some(st) = start {
            out.push([st, s]);
            start = None;
        }
        s += step;
    }
    if let Some(st) = start {
        out.push([st, s1]);
    }
    out
}

/// How far a deck's girders stand out past its barriers (m), and how deep
/// the deck is under its surface: the soffit's depth.
pub const DECK_EDGE: f64 = 0.62;
pub const DECK_DEPTH: f64 = 1.8;

/// A road a deck passes over (one of the network's overpasses, from the
/// deck's side): where along the deck, the road under it and where along
/// that, the sine of the angle between them, and half the metres of the
/// deck the lower road's pavement and verges take.
#[derive(Clone, Copy, Debug)]
pub struct Under {
    pub s: f64,
    pub road: usize,
    pub road_s: f64,
    pub sin: f64,
    pub half: f64,
}

/// The overpasses where road `road` (`t`) is the deck.
pub fn crossings_under(net: &Track, road: usize, t: &Track) -> Vec<Under> {
    let mut out = Vec::new();
    for j in &net.overpasses {
        let (s, o, os) = if j.road == road {
            (j.s, j.other, j.other_s)
        } else if j.other == road {
            (j.other_s, j.road, j.s)
        } else {
            continue;
        };
        let ot = net.road(o);
        let os = os.clamp(0.0, ot.length);
        if t.surface_y(s, 0.0) < ot.surface_y(os, 0.0) + 3.0 {
            continue;
        }
        let (f, of) = (t.frame(s), ot.frame(os));
        let cos = f.fx * of.fx + f.fz * of.fz;
        let sin = (1.0 - cos * cos).max(0.09).sqrt();
        let reach = js::max(of.wall_l, of.wall_r).max(of.hw + of.margin) + ABUT_OUT;
        out.push(Under {
            s,
            road: o,
            road_s: os,
            sin,
            half: reach / sin,
        });
    }
    out
}

/// Whether the deck lands on fill past the lower road on the side `dir`
/// (along the deck) of crossing `u`: the ground under its next 20 m comes
/// within a little of its soffit, an embankment the abutment holds back.
fn has_fill(t: &Track, tr: &Terrain, u: &Under, dir: f64) -> bool {
    (0..=5).any(|k| {
        let sb = (u.s + dir * (u.half + 4.0 * k as f64)).clamp(0.0, t.length);
        let fb = t.frame(sb);
        fb.y - DECK_DEPTH - tr.height_at(fb.x, fb.z) < 2.5
    })
}

/// Abutment walls stand this far outside the lower road's barriers (m),
/// and a pier no nearer another road's than this.
const ABUT_OUT: f64 = 1.6;
const PIER_CLEAR: f64 = 1.5;
const ABUT_FACE: f64 = 1.0;

/// Where a bent at `s` on road `road` (`t`) stands its two columns
/// (lateral metres) clear of every other road under it, neither on
/// another road's pavement or verge: under the deck where it can, else
/// out past the deck's edge on an outrigger, else nowhere on that side.
/// `None` when neither side has room.
pub fn bent_columns(net: &Track, road: usize, t: &Track, s: f64) -> Option<[f64; 2]> {
    let f = t.frame(s);
    let edge = |side: f64| if side < 0.0 { f.wall_l } else { f.wall_r } + DECK_EDGE;
    let mut out = [f64::NAN; 2];
    for (j, side) in [(0usize, -1.0f64), (1, 1.0)] {
        for lat in [f.hw * 0.55, f.hw * 0.85, edge(side) + 2.5] {
            let lat = side * lat;
            if column_clear(net, road, &f, lat) {
                out[j] = lat;
                break;
            }
        }
    }
    match (out[0].is_nan(), out[1].is_nan()) {
        (true, true) => None,
        // One column alone: a hammerhead on the side with room.
        (true, false) => Some([out[1], out[1]]),
        (false, true) => Some([out[0], out[0]]),
        (false, false) => Some(out),
    }
}

fn column_clear(net: &Track, road: usize, f: &mp_track::Frame, lat: f64) -> bool {
    let (x, z) = (f.x + f.rx * lat, f.z + f.rz * lat);
    let mut clear = true;
    net.net.for_each_road_near(x, z, 40.0, |a| {
        if a == road {
            return true;
        }
        let at = net.road(a);
        let d = at.distance_to_road(x, z, 40.0);
        let Some(sa) = d.s else { return true };
        let af = at.frame(sa);
        let wall = if d.lat < 0.0 { af.wall_l } else { af.wall_r };
        if af.y < f.y - 2.0 && d.d < wall.max(af.hw + af.margin) + PIER_CLEAR {
            clear = false;
            return false;
        }
        true
    });
    clear
}

// ── The Road class ──────────────────────────────────────────────────────

/// A stretch where scenery blanks the paint across junctions (an entry of
/// `track.noMarks`, which Streets' `plan()` sets).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkGap {
    pub s0: f64,
    pub s1: f64,
}

/// Lane layout across the section for a marking scheme: lanes start at
/// `origin` (lateral metres) and repeat every `width`. The asphalt shader uses
/// it to lay tyre-worn wheel paths and an oil stripe down each lane.
fn lane_layout(marks: &str, hw: f64) -> [f64; 2] {
    if marks == "freeway" && hw > 9.0 {
        let l = -hw + 1.2;
        let r = hw - 2.0;
        return [l, (r - l) / 4.0];
    }
    if marks == "boulevard" {
        return [0.0, (hw - 0.45) / 2.0];
    }
    if marks == "avenue" || marks == "guide" {
        return [0.0, hw / 2.0];
    }
    [0.0, hw - 0.3]
}

/// What a road reads and writes of the world being built.
pub struct RoadCtx<'a> {
    pub track: &'a Track,
    pub terrain: &'a Terrain,
    pub graph: &'a mut SceneGraph,
    pub textures: &'a mut TextureCache,
    /// `track.noMarks` (empty where no scenery sets it).
    pub no_marks: &'a [MarkGap],
    /// A branch of the level's road (docs/vision/peninsula.md, stage 3):
    /// its surface, paint and barriers begin this far along it (the
    /// parent's paved edge), and it has no start or finish line.
    pub branch_from: Option<f64>,
    /// A streamed section builds exactly its stretch of the road
    /// (`Section::clip`, docs/vision/peninsula.md stage 4b): `None` builds
    /// it whole.
    pub clip: Option<crate::section::Section>,
    /// Which road of the network this is (0 the level's, k its (k - 1)th
    /// branch): a partitioned section builds only its own road's metres.
    pub road: usize,
    /// The level's road, which holds the network's junctions (stage 6c):
    /// a road clears its rails where a lesser road meets it, and stands
    /// nothing on the tarmac of the greater roads it meets (a branch's
    /// mouth and open far end, a street's crossings). `None` for a road
    /// built on its own.
    pub network: Option<&'a Track>,
    /// Optional precomputed `(side_l, side_r)` for this road over the planned
    /// terrain (`Planned` computes them once across all sections).
    pub precomputed_sides: Option<&'a (Vec<u8>, Vec<u8>)>,
}

/// `new Road(track, terrain)` and what `build()` leaves on it.
pub struct Road {
    /// `this.group`: the group `road`.
    pub group: NodeId,
    /// What sits at each metre's left and right edge: 0 none, 1 rock wall,
    /// 2 guardrail, 3 fence, 4 jersey barrier, 5 kerb + railing, 6 kerb
    /// (`track.sideL` and `sideR` in the JS; scenery reads them).
    pub side_l: Vec<u8>,
    pub side_r: Vec<u8>,
    /// `this.materials`, keys in the order made (`asphalt0`..`asphalt2`,
    /// `shoulder`, `markings`, `steel`, `post`, `wood`, `concrete`,
    /// `railPaint`, `chevron`).
    pub materials: Vec<(String, MaterialId)>,
    /// `this.uniforms.tDetail`: the texture module's
    /// `terrainDetailTexture`, the one texture the terrain, sky and sea use
    /// too.
    pub detail: TextureId,
}

/// One painted line of a section (`{ k, lat, w, c, dash }`).
#[derive(Clone, Copy, Debug)]
struct Line {
    k: &'static str,
    lat: f64,
    w: f64,
    c: [f64; 3],
    dash: Option<[f64; 2]>,
}

/// `{ L, f: { ...f } }` on the stack (at most 6 lines per road cross-section).
struct LineSet {
    l: [Line; 6],
    len: usize,
    f: Frame,
}

impl LineSet {
    #[inline]
    fn push(&mut self, line: Line) {
        self.l[self.len] = line;
        self.len += 1;
    }

    #[inline]
    fn slice(&self) -> &[Line] {
        &self.l[..self.len]
    }
}

/// `ROAD_TYPES[ROAD_KEYS[t.roadType[i]]]`.
fn road_type_at(t: &Track, i: usize) -> &'static RoadType {
    &ROAD_TYPES[t.road_type[i] as usize]
}

/// A profile point `{ lat: (f) => ..., dy, u }`.
fn pt(lat: impl Fn(&Frame) -> f64 + Send + Sync + 'static, dy: f64, u: f64) -> ProfilePoint {
    ProfilePoint::new(Lat::f(move |f, _| lat(f))).dy(dy).u(u)
}

/// `{ step, vScale }`.
fn opts(step: f64, v_scale: f64) -> ExtrudeOpts {
    ExtrudeOpts {
        step,
        v_scale,
        color: None,
    }
}

/// `e.set(0, yaw, 0); q.setFromEuler(e)`.
fn yaw_quaternion(yaw: f64) -> Quaternion {
    Quaternion::from_euler(&Euler::new(0.0, yaw, 0.0))
}

const DOUBLE_SIDE: f64 = three::DOUBLE_SIDE as f64;

const PI: f64 = std::f64::consts::PI;

/// An instanced mesh's instances.
fn instances(graph: &mut SceneGraph, im: NodeId) -> &mut Instances {
    graph
        .get_mut(im)
        .instances
        .as_mut()
        .expect("an instanced mesh")
}

impl RoadCtx<'_> {
    /// The parts of `[a, b]` this build makes.
    fn pieces(&self, a: f64, b: f64) -> Vec<[f64; 2]> {
        match &self.clip {
            Some(sec) => sec.clip_road(self.road, self.track, a, b),
            None => vec![[a, b]],
        }
    }

    /// Bundles of ranges, each clipped to this build's stretch (an empty
    /// bundle is dropped).
    fn clip_runs(&self, groups: Vec<Vec<[f64; 2]>>) -> Vec<Vec<[f64; 2]>> {
        groups
            .into_iter()
            .map(|g| {
                g.into_iter()
                    .flat_map(|r| self.pieces(r[0], r[1]))
                    .collect::<Vec<_>>()
            })
            .filter(|g| !g.is_empty())
            .collect()
    }

    /// Whether the road's edge at `s` on `side` (where a chevron or a post
    /// would stand, past the wall) lies on the tarmac of a lesser road that
    /// meets it near there: the mouth of a road leaving on the outside of a
    /// bend (stage 6).
    fn on_lesser(&self, s: f64, side: f64) -> bool {
        let Some(net) = self.network else {
            return false;
        };
        let t = self.track;
        let f = t.frame(s);
        let lat = side * ((if side < 0.0 { f.wall_l } else { f.wall_r }) + 0.9);
        let (x, z) = (f.x + f.rx * lat, f.z + f.rz * lat);
        net.junctions
            .iter()
            .filter(|j| j.other == self.road && t.ds(j.other_s, s).abs() < 80.0)
            .any(|j| {
                let bt = net.road(j.road);
                let d = bt.distance_to_road(x, z, 40.0);
                d.s.is_some_and(|bs| {
                    let bf = bt.frame(bs.clamp(0.0, bt.length));
                    d.d < bf.hw + bf.margin + 1.0
                })
            })
    }

    /// Whether the road's metre at `s` lies on a greater road it meets,
    /// out to that road's margin and a metre more (D1218).
    fn on_greater(&self, s: f64) -> bool {
        let Some(net) = self.network else {
            return false;
        };
        let mut seen: Vec<usize> = Vec::new();
        for j in net.junctions.iter().filter(|j| j.road == self.road) {
            if seen.contains(&j.other) {
                continue;
            }
            seen.push(j.other);
            if self.on_road(net.road(j.other), s) {
                return true;
            }
        }
        false
    }

    /// Whether the road's metre at `s` lies on road `p`'s tarmac.
    fn on_road(&self, p: &Track, s: f64) -> bool {
        let f = self.track.frame(s);
        let d = p.distance_to_road(f.x, f.z, 60.0);
        let Some(ps) = d.s else { return false };
        if !p.is_loop && !(0.0..=p.length).contains(&ps) {
            return false;
        }
        let pf = p.frame(if p.is_loop { p.wrap(ps) } else { ps });
        d.lat.abs() < pf.hw + pf.margin + f.hw + 1.0
    }

    /// Whether the metre ending at `s` is this build's.
    fn wants(&self, s: f64) -> bool {
        match &self.clip {
            Some(sec) => sec.holds_road(self.road, self.track, s - 0.5, sec.scenery_radius),
            None => true,
        }
    }
}

/// Shared lazy `(side_l, side_r)` cache per road of the network.
pub type SharedRoadSides = std::sync::Arc<Vec<std::sync::OnceLock<(Vec<u8>, Vec<u8>)>>>;

/// Computes `(side_l, side_r)` for `track` over `terrain` (and optional `network`).
pub fn classify_road_sides(
    t: &Track,
    tr: &Terrain,
    road: usize,
    network: Option<&Track>,
) -> (Vec<u8>, Vec<u8>) {
    let n = t.n;
    let mut l = vec![0u8; n];
    let mut r = vec![0u8; n];
    // 0 none, 1 rock wall, 2 guardrail, 3 fence, 4 jersey barrier, 5 kerb + railing
    let edge = |i: usize| road_type_at(t, i).edge;
    // EDGE_CODE: fence 3, jersey 4, rail 5, curb 6, none 0, circuit 0.
    let edge_code = |e: &str| -> u8 {
        match e {
            "fence" => 3,
            "jersey" => 4,
            "rail" => 5,
            "curb" => 6,
            _ => 0,
        }
    };
    // Float32Arrays.
    let mut tmp_l = vec![0f32; n];
    let mut tmp_r = vec![0f32; n];
    for i in (0..n).step_by(2) {
        if edge(i) == "terrain" {
            for side in [-1.0, 1.0] {
                let hw = f64::from(t.hw[i]) + f64::from(t.margin[i]);
                let probe = side * (hw + 5.0);
                let x = f64::from(t.px[i]) + f64::from(t.rx[i]) * probe;
                let zz = f64::from(t.pz[i]) + f64::from(t.rz[i]) * probe;
                let th = tr.height_at(x, zz);
                let rh = t.surface_y(i as f64, side * hw);
                let v = th - rh; // positive = ground rises (wall)
                if side < 0.0 {
                    tmp_l[i] = v as f32;
                } else {
                    tmp_r[i] = v as f32;
                }
            }
        }
    }
    // Smooth the decision so barriers come in sensible runs.
    let decide = |tmp: &[f32], out: &mut [u8]| {
        for i in 0..n {
            let e = edge(i);
            if e != "terrain" {
                out[i] = edge_code(e);
                continue;
            }
            let mut acc = 0.0;
            let mut cnt = 0.0;
            let mut k: i64 = -16;
            while k <= 16 {
                let j = clamp(((i & !1) as i64 + k) as f64, 0.0, (n - 1) as f64) as usize;
                acc += f64::from(tmp[j]);
                cnt += 1.0;
                k += 2;
            }
            out[i] = if acc / cnt > 1.6 { 1 } else { 2 };
        }
    };
    decide(&tmp_l, &mut l);
    decide(&tmp_r, &mut r);
    // Drop stubs: short rock-wall gaps inside guardrail runs become guardrail,
    // and short guardrail pieces between rock walls disappear.
    let despeckle = |arr: &mut [u8]| {
        for (val, other, min_len) in [(1u8, 2u8, 30usize), (2, 1, 40)] {
            let mut i = 0;
            while i < n {
                if edge(i) != "terrain" || arr[i] != val {
                    i += 1;
                    continue;
                }
                let mut j = i;
                while j < n && arr[j] == val && edge(j) == "terrain" {
                    j += 1;
                }
                let before = if i > 0 { arr[i - 1] } else { other };
                let after = if j < n { arr[j] } else { other };
                if j - i < min_len && before == other && after == other {
                    arr[i..j].fill(other);
                }
                i = j;
            }
        }
    };
    despeckle(&mut l);
    despeckle(&mut r);
    // Hairpin outsides always get a guardrail.
    for i in 0..n {
        let k = f64::from(t.k_smooth[i]);
        if edge(i) == "terrain" && k.abs() > 0.022 {
            if k > 0.0 {
                l[i] = 2;
            } else {
                r[i] = 2;
            }
        }
    }
    if let Some(net) = network {
        let own;
        let e = match net.edges.get(road) {
            Some(e) => e,
            None => {
                own = net.edges_of(road);
                &own
            }
        };
        for i in 0..n {
            if e.left[i] == mp_track::barriers::NONE {
                l[i] = 0;
            }
            if e.right[i] == mp_track::barriers::NONE {
                r[i] = 0;
            }
        }
    }
    (l, r)
}

impl Road {
    /// `new Road(track, terrain)`: the group and the shared uniforms.
    pub fn new(graph: &mut SceneGraph, textures: &mut TextureCache) -> Road {
        let group = graph.group("road");
        let detail = graph.cached_texture(&textures.terrain_detail_texture(), Layer::Main, "");
        Road {
            group,
            side_l: Vec::new(),
            side_r: Vec::new(),
            materials: Vec::new(),
            detail,
        }
    }

    /// `this.materials[key]`.
    pub fn material(&self, key: &str) -> Option<MaterialId> {
        self.materials
            .iter()
            .find(|(k, _)| k == key)
            .map(|&(_, m)| m)
    }

    /// The asphalt materials (they share one `uWet` uniform).
    pub fn asphalt_materials(&self) -> Vec<MaterialId> {
        self.materials
            .iter()
            .filter(|(k, _)| k.starts_with("asphalt"))
            .map(|&(_, m)| m)
            .collect()
    }

    /// `setNight(n)`: night makes the tarmac look damp (see patchAsphalt).
    /// `uWet` is one uniform object shared by the asphalt materials, so the
    /// change is an edit of each.
    pub fn set_night(&self, n: f64, out: &mut Vec<Edit>) {
        for m in self.asphalt_materials() {
            out.push(Edit {
                target: Handle::Material(m),
                change: Change::Number {
                    prop: "uWet",
                    value: n,
                },
            });
        }
    }

    /// The updater `World.build` registers after the road:
    /// `road.setNight(smoothstep(0.55, 1.0, n) * 0.85)` every frame, as
    /// data (`anim`, D1215).
    pub fn dew_animator(&self) -> crate::anim::AnimatorDesc {
        crate::anim::AnimatorDesc::Dew {
            mats: self.asphalt_materials(),
        }
    }

    /// `build()`: fills the group (not yet added anywhere) and returns it.
    pub fn build(&mut self, c: &mut RoadCtx) -> NodeId {
        self.classify_sides(c);
        self.build_surface(c);
        self.build_markings(c);
        self.build_barriers(c);
        self.build_lines(c);
        self.build_chevrons(c);
        self.build_viaduct(c);
        self.group
    }

    /// For each metre and side decide what sits at the edge: a rock face, a
    /// guardrail, a fence or a concrete barrier. Physics and scenery read it.
    pub fn classify_sides(&mut self, c: &RoadCtx) {
        if let Some((l, r)) = c.precomputed_sides {
            self.side_l = l.clone();
            self.side_r = r.clone();
            return;
        }
        let (l, r) = classify_road_sides(c.track, c.terrain, c.road, c.network);
        self.side_l = l;
        self.side_r = r;
    }

    /// `mat(key, make)`: one material per key.
    fn mat(
        &mut self,
        c: &mut RoadCtx,
        key: &str,
        make: impl FnOnce(&mut RoadCtx) -> Material,
    ) -> MaterialId {
        if let Some(m) = self.material(key) {
            return m;
        }
        let m = make(c);
        let id = c.graph.add_material(m);
        self.materials.push((key.to_string(), id));
        id
    }

    /// `new THREE.Mesh(g, m)` with its shadow flags, added to the group.
    fn add_mesh(
        &self,
        c: &mut RoadCtx,
        g: BufferGeometry,
        m: MaterialId,
        cast: bool,
        receive: bool,
    ) -> NodeId {
        let gid = c.graph.add_geometry(g);
        let mesh = c.graph.mesh(gid, m);
        let o = c.graph.get_mut(mesh);
        o.cast_shadow = cast;
        o.receive_shadow = receive;
        c.graph.add(self.group, mesh);
        mesh
    }

    pub fn build_surface(&mut self, c: &mut RoadCtx) {
        let t = c.track;
        let type_at = |s: f64| road_type_at(t, t.idx(s));
        let detail = self.detail;
        let mut asphalt_mats = Vec::new();
        for tone in [0u32, 1, 2] {
            let id = self.mat(c, &format!("asphalt{tone}"), |c| {
                let map =
                    c.graph
                        .cached_texture(&c.textures.asphalt_texture(tone), Layer::Main, "");
                // m.map.repeat.set(1, 1) is three's default; then patchAsphalt.
                Material::standard()
                    .set("map", map)
                    .set("roughness", 0.86)
                    .set("metalness", 0.0)
                    .set("color", 0xffffff)
                    .kind(MaterialKind::Asphalt, None)
                    .uniform("tDetail", texture_value(detail))
                    .uniform("uWet", num(0.0))
                    .uniform("clippingPlanes", Value::Null)
            });
            asphalt_mats.push(id);
        }
        let shoulder_mat = self.mat(c, "shoulder", |c| {
            let map = c
                .graph
                .cached_texture(&c.textures.gravel_texture(), Layer::Main, "");
            // Toward the outer edge the gravel texture flattens to its average so
            // the verge meets the terrain (whose colour the outer vertices carry)
            // without a hard seam.
            Material::standard()
                .set("map", map)
                .set("roughness", 1.0)
                .set("vertexColors", true)
                .kind(MaterialKind::Shoulder, None)
                .uniform("clippingPlanes", Value::Null)
        });
        if c.clip.as_ref().is_some_and(|sec| !sec.has_road(c.road)) {
            return;
        }
        let colorizer = Colorizer::new(c.terrain);

        let from = c.branch_from.unwrap_or(0.0);
        for z in &t.zones {
            let s0 = js::max(from, z.s0 - 1.0);
            let s1 = js::min(t.length, z.s1 + 1.0);
            let pieces: Vec<[f64; 2]> = chunks(s0, s1, 560.0)
                .into_iter()
                .flat_map(|r| c.pieces(r[0], r[1]))
                .collect();
            for r in pieces {
                // Asphalt UVs in metres / 4 both ways, so the aggregate isn't
                // stretched along the road; the shader recovers metres from them.
                let mut surf = extrude(
                    t,
                    &[r],
                    &[
                        ProfilePoint::new(Lat::f(|f, _| -f.hw)),
                        ProfilePoint::new(0.0),
                        ProfilePoint::new(Lat::f(|f, _| f.hw)),
                    ],
                    &opts(ROAD_STEP, 4.0),
                );
                let uv = surf.get_attribute("uv").expect("uv");
                let count = uv.count();
                let mut lane = vec![0f32; count * 3];
                for k in 0..count {
                    let s = uv.get_y(k) * 4.0;
                    let f = t.frame(js::min(s, t.length));
                    let [o, w] = lane_layout(road_type_at(t, t.idx(s)).marks, f.hw);
                    lane[k * 3] = o as f32;
                    lane[k * 3 + 1] = w as f32;
                    lane[k * 3 + 2] = f.hw as f32;
                }
                surf.set_attribute("aLane", BufferAttribute::from_f32(lane, 3));
                let mid = (r[0] + r[1]) / 2.0;
                let mat = asphalt_mats[type_at(mid).tone as usize];
                self.add_mesh(c, surf, mat, false, true);

                // Shoulders + skirt (sloped into the terrain; vertical fascia in the city).
                // Profiles run left→right so faces point up/outward.
                let edge_type = type_at(mid).edge;
                // Kerbed streets: the scenery lays the kerbs and pavements; circuits
                // their kerbs and run-off.
                if edge_type == "curb" || edge_type == "circuit" {
                    continue;
                }
                let city = edge_type == "jersey";
                let open = t.open_ground.is_some();
                let tint = match z.zone.landform {
                    "mountain" => [0.55, 0.52, 0.48],
                    "coast" => [0.6, 0.55, 0.47],
                    "valley" => [0.45, 0.42, 0.33],
                    "beach" => [0.66, 0.64, 0.6],
                    "canyon" => [0.62, 0.45, 0.33],
                    "desert" => [0.66, 0.56, 0.42],
                    "playa" => [0.78, 0.76, 0.7],
                    _ => [0.62, 0.62, 0.6],
                };
                for side in [-1.0f64, 1.0] {
                    let w = move |f: &Frame| if side < 0.0 { f.wall_l } else { f.wall_r };
                    let mut prof = if city {
                        vec![
                            pt(move |f| side * f.hw, 0.0, 0.0),
                            pt(move |f| side * (w(f) + 0.35), 0.0, 0.4),
                            pt(move |f| side * (w(f) + 0.35), -1.6, 1.0),
                        ]
                    } else if open {
                        // An open level's roadside eases down to the land
                        // as the simulation's ground does (stage 7b):
                        // `EDGE_RUN` out, to just under the near field's
                        // drop.
                        vec![
                            pt(move |f| side * f.hw, -0.01, 0.0),
                            pt(move |f| side * w(f), -0.08, 0.5),
                            pt(
                                move |f| side * (w(f) + mp_track::EDGE_RUN),
                                -mp_track::OPEN_DROP - 0.12,
                                1.0,
                            ),
                        ]
                    } else {
                        vec![
                            pt(move |f| side * f.hw, -0.01, 0.0),
                            pt(move |f| side * w(f), -0.08, 0.5),
                            pt(move |f| side * (w(f) + 2.5), -1.6, 1.0),
                        ]
                    };
                    if side < 0.0 {
                        prof.reverse();
                    }
                    let mut g = extrude(
                        t,
                        &[r],
                        &prof,
                        &ExtrudeOpts {
                            step: ROAD_STEP,
                            v_scale: 6.0,
                            color: Some(ExtrudeColor::Rgb(tint)),
                        },
                    );
                    if !city {
                        // Outer edge takes the terrain's own colour (scaled so gravel
                        // map × vertex colour lands on what the terrain shader shows).
                        let gu: Vec<f64> = {
                            let a = g.get_attribute("uv").expect("uv");
                            (0..a.count()).map(|k| a.get_x(k)).collect()
                        };
                        let gp: Vec<(f64, f64)> = {
                            let a = g.get_attribute("position").expect("position");
                            (0..a.count()).map(|k| (a.get_x(k), a.get_z(k))).collect()
                        };
                        let gc = g.get_attribute_mut("color").expect("color");
                        for k in 0..gu.len() {
                            if gu[k] < 0.99 {
                                continue;
                            }
                            let (x, zz) = gp[k];
                            let y = c.terrain.height_at(x, zz);
                            let sl = c.terrain.slope_at(x, zz, 3.0);
                            let (tcol, _) = colorizer.color(x, y, zz, 1.0 / (1.0 + sl * sl).sqrt());
                            let q = 1.25 / 0.29;
                            gc.set_xyz(k, tcol[0] * q, tcol[1] * q, tcol[2] * q);
                        }
                    }
                    self.add_mesh(c, g, shoulder_mat, false, true);
                }
            }
        }
    }

    pub fn build_markings(&mut self, c: &mut RoadCtx) {
        let t = c.track;
        let white = [0.92, 0.92, 0.9];
        let yellow = [0.95, 0.72, 0.12];
        let mut pos: Vec<f64> = Vec::new();
        let mut col: Vec<f64> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        let mut uvs: Vec<f64> = Vec::new();
        let no_marks = c.no_marks;
        let ln = |k, lat, w, c| Line {
            k,
            lat,
            w,
            c,
            dash: None,
        };
        let dashed = |k, lat, w, c, d: [f64; 2]| Line {
            k,
            lat,
            w,
            c,
            dash: Some(d),
        };
        let lines_at = |s: f64| -> LineSet {
            let f = t.frame(s);
            let hw = f.hw;
            let marks = road_type_at(t, t.idx(s)).marks;
            let mut ls = LineSet {
                l: [ln("", 0.0, 0.0, [0.0; 3]); 6],
                len: 0,
                f,
            };
            // Scenery can blank the paint across junctions (crossings, plazas).
            if no_marks.iter().any(|g| s > g.s0 && s < g.s1) {
                return ls;
            }
            if marks == "double" {
                ls.push(ln("c1", -0.14, 0.12, yellow));
                ls.push(ln("c2", 0.14, 0.12, yellow));
                ls.push(ln("el", -(hw - 0.3), 0.15, white));
                ls.push(ln("er", hw - 0.3, 0.15, white));
            } else if marks == "boulevard" {
                // Boulevard: two lanes each way, double yellow in the middle.
                ls.push(ln("c1", -0.14, 0.12, yellow));
                ls.push(ln("c2", 0.14, 0.12, yellow));
                let lane = (hw - 0.45) / 2.0;
                ls.push(dashed("bl", -lane, 0.13, white, [3.0, 9.0]));
                ls.push(dashed("br", lane, 0.13, white, [3.0, 9.0]));
                ls.push(ln("el", -(hw - 0.3), 0.15, white));
                ls.push(ln("er", hw - 0.3, 0.15, white));
            } else if marks == "avenue" {
                // Kerbed city street: double yellow and lane lines; the kerbs are the edges.
                ls.push(ln("c1", -0.14, 0.12, yellow));
                ls.push(ln("c2", 0.14, 0.12, yellow));
                let lane = hw / 2.0;
                ls.push(dashed("bl", -lane, 0.13, white, [3.0, 9.0]));
                ls.push(dashed("br", lane, 0.13, white, [3.0, 9.0]));
            } else if marks == "dashed" {
                ls.push(dashed("c", 0.0, 0.13, yellow, [4.0, 11.0]));
                ls.push(ln("el", -(hw - 0.3), 0.13, white));
                ls.push(ln("er", hw - 0.3, 0.13, white));
            } else if marks == "guide" {
                // Dry lake course: one dark guide line down the middle, no edges.
                ls.push(ln("g", 0.0, 0.3, [0.09, 0.08, 0.08]));
            } else if marks == "freeway" && hw > 9.0 {
                let left = -hw + 1.2;
                let right = hw - 2.0;
                let lw = (right - left) / 4.0;
                ls.push(ln("fl", left, 0.15, yellow));
                ls.push(ln("fr", right, 0.18, white));
                for (q, k) in [(1.0, "f1"), (2.0, "f2"), (3.0, "f3")] {
                    ls.push(dashed(k, left + lw * q, 0.14, white, [3.0, 12.0]));
                }
            } else {
                ls.push(ln("el", -(hw - 0.3), 0.15, white));
                ls.push(ln("er", hw - 0.3, 0.15, white));
            }
            ls
        };
        let mut emit_step = |prev: &LineSet, cur: &LineSet, s: f64| {
            for a in prev.slice() {
                let Some(b) = cur.slice().iter().find(|x| x.k == a.k) else {
                    continue;
                };
                if let Some(d) = a.dash {
                    let ph = (s - 0.5) % d[1];
                    if ph > d[0] {
                        continue;
                    }
                }
                let base = (pos.len() / 3) as u32;
                for (fr, ln, ss) in [(&prev.f, a, s - 1.0), (&cur.f, b, s)] {
                    for side in [-1.0, 1.0] {
                        let lat = ln.lat + side * ln.w * 0.5;
                        pos.extend_from_slice(&[
                            fr.x + fr.rx * lat,
                            fr.y - lat * fr.bank + 0.018,
                            fr.z + fr.rz * lat,
                        ]);
                        col.extend_from_slice(&ln.c);
                        uvs.extend_from_slice(&[lat * 8.0, ss]);
                    }
                }
                idx.extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
            }
        };
        let has_road = c.clip.as_ref().is_none_or(|sec| sec.has_road(c.road));
        if has_road {
            let from = c.branch_from.unwrap_or(0.0);
            if c.branch_from.is_none()
                && let Some(place) = c.clip.as_ref().and_then(|sec| sec.place.as_ref())
            {
                let mut last_k = 0i64;
                if let Some(runs) = place.runs.get(c.road) {
                    for &[r0, r1] in runs {
                        let k0 = (((r0 + 0.5).floor() as i64) - 1).max(last_k + 1);
                        let mut prev = lines_at((k0 - 1) as f64);
                        let mut k = k0;
                        while (k as f64) <= t.length && (k as f64) - 0.5 <= r1 {
                            let s = k as f64;
                            let cur = lines_at(s);
                            if c.wants(s) {
                                emit_step(&prev, &cur, s);
                            }
                            prev = cur;
                            last_k = k;
                            k += 1;
                        }
                    }
                }
            } else {
                let mut prev = lines_at(from);
                let mut s = from + 1.0;
                while s <= t.length {
                    let cur = lines_at(s);
                    if c.wants(s) {
                        emit_step(&prev, &cur, s);
                    }
                    prev = cur;
                    s += 1.0;
                }
            }
        }
        let mut g = BufferGeometry::new();
        g.set_attribute("position", BufferAttribute::from_f64(&pos, 3));
        g.set_attribute("color", BufferAttribute::from_f64(&col, 3));
        g.set_attribute("uv", BufferAttribute::from_f64(&uvs, 2));
        g.set_index(&idx);
        g.compute_vertex_normals();
        // patchMarkings: paint wear. Markings are thin quads carrying (side,
        // s) in uv; the paint fades toward asphalt in blotches and where
        // tyres cross.
        let m = Material::standard()
            .set("vertexColors", true)
            .set("roughness", 0.55)
            .set("metalness", 0.0)
            .set("emissive", 0xffffff)
            .set("emissiveIntensity", 0.0)
            .set("polygonOffset", true)
            .set("polygonOffsetFactor", -2.0)
            .set("polygonOffsetUnits", -2.0)
            .kind(MaterialKind::Markings, None)
            .uniform("tDetail", texture_value(self.detail))
            .uniform("clippingPlanes", Value::Null);
        let id = c.graph.add_material(m);
        self.materials.push(("markings".into(), id));
        self.add_mesh(c, g, id, false, true);
    }

    pub fn build_lines(&mut self, c: &mut RoadCtx) {
        // Start and finish lines — checkered bands across the road.
        let t = c.track;
        let tex = c
            .graph
            .cached_texture(&c.textures.checker_texture(10), Layer::Main, "");
        let mat = c.graph.add_material(
            Material::standard()
                .set("map", tex)
                .set("roughness", 0.6)
                .set("polygonOffset", true)
                .set("polygonOffsetFactor", -3.0)
                .set("polygonOffsetUnits", -3.0),
        );
        // A circuit starts and finishes on the same line; a branch has
        // neither.
        if c.branch_from.is_some() {
            return;
        }
        let list: Vec<f64> = if t.is_loop {
            if t.laps != 0 {
                vec![t.start_s]
            } else {
                Vec::new()
            }
        } else {
            vec![t.start_s, t.finish_s]
        };
        for s in list {
            if c.pieces(s - 1.2, s + 1.2).is_empty() {
                continue;
            }
            let mut g = extrude(
                t,
                &[[s - 1.2, s + 1.2]],
                &[pt(|f| -f.hw, 0.022, 0.0), pt(|f| f.hw, 0.022, 1.0)],
                &opts(2.4, 2.4),
            );
            // u runs across the road (checker columns), v along it (4 rows).
            let uv = g.get_attribute_mut("uv").expect("uv");
            let v0 = uv.get_y(0);
            for k in 0..uv.count() {
                let v = uv.get_y(k) - v0;
                uv.set_y(k, v);
            }
            self.add_mesh(c, g, mat, false, false);
        }
    }

    pub fn build_barriers(&mut self, c: &mut RoadCtx) {
        let t = c.track;
        let steel = self.mat(c, "steel", |_| {
            Material::standard()
                .set("color", 0xa9adb3)
                .set("metalness", 0.45)
                .set("roughness", 0.55)
                .set("side", DOUBLE_SIDE)
        });
        let post = self.mat(c, "post", |_| {
            Material::standard()
                .set("color", 0x6d6f73)
                .set("metalness", 0.5)
                .set("roughness", 0.5)
        });
        let wood = self.mat(c, "wood", |_| {
            Material::standard()
                .set("color", 0x7a6248)
                .set("roughness", 0.95)
        });
        let concrete = self.mat(c, "concrete", |c| {
            let map = c
                .graph
                .cached_texture(&c.textures.concrete_texture(), Layer::Main, "");
            Material::standard()
                .set("map", map)
                .set("roughness", 0.9)
                .set("color", 0xd8d5ce)
        });

        let from = c.branch_from.unwrap_or(0.0);
        for side in [-1.0f64, 1.0] {
            let arr = if side < 0.0 {
                self.side_l.clone()
            } else {
                self.side_r.clone()
            };
            let w = move |f: &Frame| if side < 0.0 { f.wall_l } else { f.wall_r };
            // Guardrail W-beam: a vertical band.
            let gr = runs(|s| arr[t.idx(s)] == 2, 2.0, from, t.length);
            for r in c.clip_runs(group_runs(&gr, 560.0)) {
                // f.hw + (wall - f.hw), as the JS writes it.
                let out = move |f: &Frame| f.hw + (w(f) - f.hw);
                let prof = [
                    pt(move |f| side * (out(f) + 0.15), 0.48, 0.0),
                    pt(move |f| side * (out(f) + 0.1), 0.64, 0.5),
                    pt(move |f| side * (out(f) + 0.15), 0.8, 1.0),
                ];
                let g = extrude(t, &r, &prof, &opts(2.0, 4.0));
                self.add_mesh(c, g, steel, true, false);
            }
            self.post_instances(c, &gr, side, 4.0, post, [0.12, 0.85, 0.12], 0.25);

            // Wooden fence: two rails + posts.
            let gaps: Vec<_> = t
                .fence_gaps
                .iter()
                .filter(|g| g.side == side)
                .copied()
                .collect();
            let fr = runs(
                |s| arr[t.idx(s)] == 3 && !gaps.iter().any(|g| s > g.s0 && s < g.s1),
                1.0,
                from,
                t.length,
            );
            for r in c.clip_runs(group_runs(&fr, 560.0)) {
                for h in [0.55, 1.0] {
                    let mut prof = vec![
                        pt(move |f| side * (w(f) + 0.25), h - 0.07, 0.0),
                        pt(move |f| side * (w(f) + 0.25), h + 0.07, 1.0),
                    ];
                    if side > 0.0 {
                        prof.reverse();
                    }
                    let g = extrude(t, &r, &prof, &opts(3.0, 3.0));
                    // m.material.side = THREE.DoubleSide: the shared wood
                    // material, its posts included.
                    c.graph
                        .material_mut(wood)
                        .set_value("side", DOUBLE_SIDE.into());
                    self.add_mesh(c, g, wood, true, false);
                }
            }
            self.post_instances(c, &fr, side, 3.0, wood, [0.14, 1.2, 0.14], 0.3);

            // Jersey barrier: extruded profile.
            let jr = runs(|s| arr[t.idx(s)] == 4, 2.0, from, t.length);
            for r in c.clip_runs(group_runs(&jr, 560.0)) {
                let mut prof = vec![
                    pt(move |f| side * (w(f) - 0.02), 0.0, 0.0),
                    pt(move |f| side * (w(f) + 0.02), 0.25, 0.2),
                    pt(move |f| side * (w(f) + 0.2), 0.36, 0.35),
                    pt(move |f| side * (w(f) + 0.28), 1.0, 0.7),
                    pt(move |f| side * (w(f) + 0.42), 1.0, 0.8),
                    pt(move |f| side * (w(f) + 0.62), 0.25, 0.9),
                    pt(move |f| side * (w(f) + 0.62), -1.8, 1.0),
                ];
                if side < 0.0 {
                    prof.reverse();
                }
                let g = extrude(t, &r, &prof, &opts(4.0, 3.0));
                self.add_mesh(c, g, concrete, true, true);
            }

            // Kerb + pipe railing (seafront promenade).
            let rr = runs(
                |s| arr[t.idx(s)] == 5 && !gaps.iter().any(|g| s > g.s0 && s < g.s1),
                1.0,
                0.0,
                t.length,
            );
            let paint = self.mat(c, "railPaint", |_| {
                Material::standard()
                    .set("color", 0xdfe6e8)
                    .set("metalness", 0.4)
                    .set("roughness", 0.45)
                    .set("side", DOUBLE_SIDE)
            });
            for r in c.clip_runs(group_runs(&rr, 560.0)) {
                let mut kerb = vec![
                    pt(move |f| side * (w(f) - 0.05), 0.0, 0.0),
                    pt(move |f| side * (w(f) - 0.05), 0.22, 0.3),
                    pt(move |f| side * (w(f) + 0.35), 0.22, 0.7),
                    pt(move |f| side * (w(f) + 0.35), -1.2, 1.0),
                ];
                if side < 0.0 {
                    kerb.reverse();
                }
                let gk = extrude(t, &r, &kerb, &opts(4.0, 3.0));
                self.add_mesh(c, gk, concrete, false, true);
                for h in [0.62, 1.08] {
                    let bar = [
                        pt(move |f| side * (w(f) + 0.15), h - 0.03, 0.0),
                        pt(move |f| side * (w(f) + 0.15), h + 0.03, 1.0),
                    ];
                    let gb = extrude(t, &r, &bar, &opts(3.0, 3.0));
                    self.add_mesh(c, gb, paint, true, false);
                }
            }
            self.post_instances(c, &rr, side, 2.5, paint, [0.06, 1.1, 0.06], 0.15);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn post_instances(
        &self,
        c: &mut RoadCtx,
        ranges: &[[f64; 2]],
        side: f64,
        spacing: f64,
        material: MaterialId,
        size: [f64; 3],
        extra: f64,
    ) {
        let t = c.track;
        let filter_non_partition = c.clip.as_ref().is_some_and(|sec| !sec.partition);
        let mut list = Vec::new();
        for &[s0, s1] in ranges {
            let mut s = s0;
            while s <= s1 {
                if !filter_non_partition || c.wants(s) {
                    list.push(s);
                }
                s += spacing;
            }
        }
        if list.is_empty() {
            return;
        }
        let mut geo = box_geometry(size[0], size[1], size[2], 1.0, 1.0, 1.0);
        geo.translate(0.0, size[1] / 2.0, 0.0);
        let gid = c.graph.add_geometry(geo);
        let im = c.graph.instanced_mesh(gid, material, list.len() as u32);
        for (k, &s) in list.iter().enumerate() {
            let f = t.frame(s);
            let lat = side * ((if side < 0.0 { f.wall_l } else { f.wall_r }) + extra);
            let x = f.x + f.rx * lat;
            let z = f.z + f.rz * lat;
            let y = f.y - lat * f.bank - 0.1;
            let q = yaw_quaternion(-kernel::atan2(f.fz, f.fx));
            let m4 = Matrix4::compose(Vector3::new(x, y, z), q, Vector3::new(1.0, 1.0, 1.0));
            instances(c.graph, im).set_matrix_at(k, &m4);
        }
        let o = c.graph.get_mut(im);
        o.cast_shadow = true;
        o.receive_shadow = true;
        c.graph.compute_instance_bounding_sphere(im);
        c.graph.add(self.group, im);
    }

    pub fn build_chevrons(&mut self, c: &mut RoadCtx) {
        let t = c.track;
        let tex = c
            .graph
            .cached_texture(&c.textures.chevron_texture(), Layer::Main, "");
        let mat = c.graph.add_material(
            Material::standard()
                .set("map", tex)
                .set("roughness", 0.5)
                .set("emissive", 0xffffff)
                .set("emissiveMap", tex)
                .set("emissiveIntensity", 0.12)
                .set("side", DOUBLE_SIDE),
        );
        self.materials.push(("chevron".into(), mat));
        if c.clip.as_ref().is_some_and(|sec| !sec.has_road(c.road)) {
            return;
        }
        struct Item {
            s: f64,
            side: f64,
            dir: f64,
        }
        let mut items = Vec::new();
        let mut last = -100.0;
        let mut consider = |s: f64| {
            if !c.wants(s) {
                return;
            }
            let k = f64::from(t.k_smooth[t.idx(s)]);
            let key = road_type_at(t, t.idx(s)).key;
            let skip = [
                "freeway",
                "boulevard",
                "street",
                "playa",
                "circuit",
                "circuitWide",
            ]
            .contains(&key);
            if !skip
                && k.abs() > 0.03
                && s - last > 11.0
                && !c.on_greater(s)
                && !c.on_lesser(s, if k > 0.0 { -1.0 } else { 1.0 })
            {
                items.push(Item {
                    s,
                    side: if k > 0.0 { -1.0 } else { 1.0 },
                    dir: if k > 0.0 { 1.0 } else { -1.0 },
                });
                last = s;
            }
        };
        if c.branch_from.is_none()
            && let Some(place) = c.clip.as_ref().and_then(|sec| sec.place.as_ref())
        {
            let mut last_k = -1i64;
            if let Some(runs) = place.runs.get(c.road) {
                for &[r0, r1] in runs {
                    let mut k = (((r0 + 0.5).floor() as i64) - 1).max(last_k + 1);
                    while (k as f64) < t.length && (k as f64) - 0.5 <= r1 {
                        consider(k as f64);
                        last_k = k;
                        k += 1;
                    }
                }
            }
        } else {
            let mut s = c.branch_from.unwrap_or(0.0);
            while s < t.length {
                consider(s);
                s += 1.0;
            }
        }
        if items.is_empty() {
            return;
        }
        let mut plate = plane_geometry(0.75, 0.75, 1.0, 1.0);
        plate.translate(0.0, 1.35, 0.0);
        let mut pole = cylinder_geometry(0.04, 0.04, 1.1, 5.0, 1.0, false, 0.0, 2.0 * PI);
        pole.translate(0.0, 0.55, 0.0);
        let post = self.mat(c, "post", |_| Material::standard().set("color", 0x777777));
        let gp = c.graph.add_geometry(plate);
        let gq = c.graph.add_geometry(pole);
        let n = items.len() as u32;
        let im_plate = c.graph.instanced_mesh(gp, mat, n);
        let im_pole = c.graph.instanced_mesh(gq, post, n);
        for (k, it) in items.iter().enumerate() {
            let f = t.frame(it.s);
            let lat = it.side * ((if it.side < 0.0 { f.wall_l } else { f.wall_r }) + 0.9);
            let v = Vector3::new(f.x + f.rx * lat, f.y - lat * f.bank, f.z + f.rz * lat);
            // Face back down the road toward approaching drivers.
            let q = yaw_quaternion(kernel::atan2(-f.fx, -f.fz));
            let m4 = Matrix4::compose(v, q, Vector3::new(it.dir, 1.0, 1.0));
            instances(c.graph, im_plate).set_matrix_at(k, &m4);
            let m4 = Matrix4::compose(v, q, Vector3::new(1.0, 1.0, 1.0));
            instances(c.graph, im_pole).set_matrix_at(k, &m4);
        }
        c.graph.compute_instance_bounding_sphere(im_plate);
        c.graph.compute_instance_bounding_sphere(im_pole);
        c.graph.add(self.group, im_plate);
        c.graph.add(self.group, im_pole);
    }

    pub fn build_viaduct(&mut self, c: &mut RoadCtx) {
        // Elevated freeway: piers and a deck underside where the road is well
        // above the city ground.
        let (t, tr) = (c.track, c.terrain);
        let elevated = |s: f64| tr.is_elevated(t, t.idx(s));
        // Cable-stayed main spans have no piers; the harbour scenery builds those.
        let spans: Vec<[f64; 2]> = t
            .tags
            .iter()
            .filter(|g| g.tag == "bridge")
            .map(|g| [g.s0, g.s1])
            .collect();
        let concrete = self.mat(c, "concrete", |c| {
            let map = c
                .graph
                .cached_texture(&c.textures.concrete_texture(), Layer::Main, "");
            Material::standard().set("map", map).set("roughness", 0.9)
        });
        let ranges = runs(elevated, 2.0, 0.0, t.length);
        // In the open world a deck is a structure (stage 9a.4 of
        // docs/vision/bay-cities.md): a girder either side, its fascia
        // from the parapet's foot down to the soffit, closed under the
        // deck. A level's own viaduct keeps its soffit alone.
        let net = c.network.filter(|_| t.open_ground.is_some());
        let structure = net.is_some();
        for r in c.clip_runs(group_runs(&ranges, 560.0)) {
            let prof = if structure {
                let wr = |d: f64| Lat::f(move |f, _| f.wall_r + d);
                let wl = |d: f64| Lat::f(move |f, _| -(f.wall_l + d));
                // Hard edges: each corner twice, no faces between.
                vec![
                    ProfilePoint::new(wr(0.3)),
                    ProfilePoint::new(wr(DECK_EDGE)).gap_after(),
                    ProfilePoint::new(wr(DECK_EDGE)),
                    ProfilePoint::new(wr(DECK_EDGE)).dy(-DECK_DEPTH).gap_after(),
                    ProfilePoint::new(wr(DECK_EDGE)).dy(-DECK_DEPTH),
                    ProfilePoint::new(wl(DECK_EDGE)).dy(-DECK_DEPTH).gap_after(),
                    ProfilePoint::new(wl(DECK_EDGE)).dy(-DECK_DEPTH),
                    ProfilePoint::new(wl(DECK_EDGE)).gap_after(),
                    ProfilePoint::new(wl(DECK_EDGE)),
                    ProfilePoint::new(wl(0.3)),
                ]
            } else {
                vec![
                    ProfilePoint::new(Lat::f(|f, _| f.wall_r + 0.62)).dy(-1.8),
                    ProfilePoint::new(Lat::f(|f, _| -(f.wall_l + 0.62))).dy(-1.8),
                ]
            };
            let g = extrude(t, &r, &prof, &opts(4.0, 8.0));
            self.add_mesh(c, g, concrete, false, false);
        }
        let filter_non_partition = c.clip.as_ref().is_some_and(|sec| !sec.partition);
        // The crossings this road passes over (the network's overpasses
        // where it is the deck): a bent either side of the road under it,
        // abutment walls along that road's edges.
        let under: Vec<Under> = match net {
            Some(net) => crossings_under(net, c.road, t),
            None => Vec::new(),
        };
        let mut cand: Vec<f64> = Vec::new();
        for &[s0, s1] in &ranges {
            let mut s = s0 + 15.0;
            while s < s1 - 5.0 {
                cand.push(s);
                s += 32.0;
            }
        }
        for u in &under {
            for dir in [-1.0, 1.0] {
                // Where the deck lands on fill, the abutment carries it.
                if has_fill(t, c.terrain, u, dir) {
                    continue;
                }
                let s = u.s + dir * (u.half + 3.0);
                if ranges.iter().any(|r| s > r[0] + 2.0 && s < r[1] - 2.0) {
                    cand.push(s);
                }
            }
        }
        cand.sort_by(f64::total_cmp);
        // Each bent: where along, and its two columns' lateral metres.
        let mut piers: Vec<(f64, [f64; 2])> = Vec::new();
        for s in cand {
            if structure && piers.last().is_some_and(|&(p, _)| s - p < 12.0) {
                continue;
            }
            // A streamed section of the open world builds the bents on its
            // own metres, so each lands in the section that holds it (a
            // partitioned section keeps a mesh by its centre).
            let mine = if structure {
                c.clip.is_none() || c.wants(s)
            } else {
                !filter_non_partition || c.wants(s)
            };
            if !mine || spans.iter().any(|g| s > g[0] - 10.0 && s < g[1] + 10.0) {
                continue;
            }
            let cols = match net {
                Some(net) => bent_columns(net, c.road, t, s),
                None => {
                    let hw = t.frame(s).hw;
                    Some([-hw * 0.55, hw * 0.55])
                }
            };
            if let Some(cols) = cols {
                piers.push((s, cols));
            }
        }
        let under: Vec<Under> = under
            .into_iter()
            .filter(|u| c.clip.is_none() || c.wants(u.s))
            .collect();
        if structure && !under.is_empty() {
            self.build_abutments(c, &under, concrete);
        }
        if piers.is_empty() {
            return;
        }
        let mut col_geo = cylinder_geometry(1.1, 1.3, 1.0, 10.0, 1.0, false, 0.0, 2.0 * PI);
        col_geo.translate(0.0, 0.5, 0.0);
        let cap_geo = box_geometry(2.2, 1.2, 1.0, 1.0, 1.0, 1.0);
        let gc = c.graph.add_geometry(col_geo);
        let gk = c.graph.add_geometry(cap_geo);
        let im_col = c.graph.instanced_mesh(gc, concrete, piers.len() as u32 * 2);
        let im_cap = c.graph.instanced_mesh(gk, concrete, piers.len() as u32);
        for (k, &(s, cols)) in piers.iter().enumerate() {
            let f = t.frame(s);
            let q = yaw_quaternion(-kernel::atan2(f.fz, f.fx));
            let top = f.y - 2.8;
            for (j, lat) in [(0usize, cols[0]), (1, cols[1])] {
                let x = f.x + f.rx * lat;
                let z = f.z + f.rz * lat;
                let g = tr.height_at(x, z) - 0.5;
                let m4 = Matrix4::compose(
                    Vector3::new(x, g, z),
                    q,
                    Vector3::new(1.0, js::max(0.5, top - g), 1.0),
                );
                instances(c.graph, im_col).set_matrix_at(k * 2 + j, &m4);
            }
            // Crossbeam: local z maps onto the road's right vector, so stretch z.
            // Across the deck, and out to a column past its edge.
            let (lo, hi) = (cols[0].min(-f.hw), cols[1].max(f.hw));
            let mid = (lo + hi) / 2.0;
            let m4 = Matrix4::compose(
                Vector3::new(f.x + f.rx * mid, top + 0.4, f.z + f.rz * mid),
                q,
                Vector3::new(1.0, 1.0, hi - lo + 2.0),
            );
            instances(c.graph, im_cap).set_matrix_at(k, &m4);
        }
        c.graph.get_mut(im_col).cast_shadow = true;
        c.graph.get_mut(im_cap).cast_shadow = true;
        c.graph.compute_instance_bounding_sphere(im_col);
        c.graph.compute_instance_bounding_sphere(im_cap);
        c.graph.add(self.group, im_col);
        c.graph.add(self.group, im_cap);
    }

    /// The walls that hold a deck's fill back from the road it passes
    /// over (stage 9a.4): along each edge of that road, outside its
    /// barriers, from below its surface up to the soffit, where the ground
    /// under the deck past that edge is near the soffit (an embankment).
    /// Where the deck stands high over open ground its bents carry it.
    fn build_abutments(&mut self, c: &mut RoadCtx, under: &[Under], concrete: MaterialId) {
        let (t, tr) = (c.track, c.terrain);
        let Some(net) = c.network else { return };
        let mut walls: Vec<Matrix4> = Vec::new();
        for u in under {
            // A shallow crossing (a freeway along a street) has no
            // abutments to speak of.
            if u.sin < 0.45 {
                continue;
            }
            let ot = net.road(u.road);
            let of = ot.frame(u.road_s);
            for dir in [-1.0, 1.0] {
                // Which edge of the lower road this end of the deck is over,
                // and whether there is fill behind it.
                let sd = (u.s + dir * u.half).clamp(0.0, t.length);
                let fd = t.frame(sd);
                let q = ot.project(fd.x, fd.z, u.road_s);
                let side = if q.lat < 0.0 { -1.0 } else { 1.0 };
                if !has_fill(t, tr, u, dir) {
                    continue;
                }
                // Its face just past the road's verge, where the cut through
                // the fill begins (a street's: behind its pavement).
                let kerbed = road_type_at(ot, ot.idx(u.road_s)).edge == "curb";
                let wall = if side < 0.0 { of.wall_l } else { of.wall_r };
                let edge = if kerbed { wall } else { of.hw + of.margin };
                let lat = side * (edge + ABUT_FACE + 0.4);
                // Along the lower road, as far as the deck's girders reach.
                let deck = js::max(fd.wall_l, fd.wall_r) + DECK_EDGE + 0.5;
                let along = deck / u.sin;
                let end = |k: f64| {
                    let so = (u.road_s + k * along).clamp(0.0, ot.length);
                    let f = ot.frame(so);
                    let (x, z) = (f.x + f.rx * lat, f.z + f.rz * lat);
                    let p = t.project(x, z, u.s);
                    let top = t.surface_y(p.s, 0.0) - DECK_DEPTH;
                    (x, z, top, f.y - 1.0)
                };
                let (ax, az, at, ab) = end(-1.0);
                let (bx, bz, bt, bb) = end(1.0);
                let h = js::max(at - ab, bt - bb);
                if h < 1.0 {
                    continue;
                }
                let (dx, dz) = (bx - ax, bz - az);
                let l = kernel::hypot(dx, dz).max(1e-6);
                // Sheared so its top follows the soffit from end to end.
                let mut m = Matrix4::make_basis(
                    Vector3::new(dx, bt - at, dz),
                    Vector3::new(0.0, h, 0.0),
                    Vector3::new(-dz / l * 0.8, 0.0, dx / l * 0.8),
                );
                m.elements[12] = (ax + bx) / 2.0;
                m.elements[13] = (at + bt) / 2.0 - h / 2.0;
                m.elements[14] = (az + bz) / 2.0;
                walls.push(m);
            }
        }
        if walls.is_empty() {
            return;
        }
        let gw = c
            .graph
            .add_geometry(box_geometry(1.0, 1.0, 1.0, 1.0, 1.0, 1.0));
        let im = c.graph.instanced_mesh(gw, concrete, walls.len() as u32);
        for (k, m) in walls.iter().enumerate() {
            instances(c.graph, im).set_matrix_at(k, m);
        }
        c.graph.get_mut(im).cast_shadow = true;
        c.graph.compute_instance_bounding_sphere(im);
        c.graph.add(self.group, im);
    }
}
