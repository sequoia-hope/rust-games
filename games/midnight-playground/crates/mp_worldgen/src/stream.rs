//! Streaming the open world (docs/vision/peninsula.md, stage 4b; DECISIONS
//! D1213): the level planned once, then built in sections of its route
//! round the player, each a [`Build`] of its own.
//!
//! A level build plans before it builds: the track is surveyed, the
//! terrain made, every scenery module's `plan()` run (flattens, carves,
//! fence gaps) and the terrain's fields rasterised. [`Planned`] does that
//! once and keeps the result; [`Planned::build`] then makes the jobs of
//! one [`Section`] over it: the modules plan afresh on the unplanned
//! terrain (their own state, which `build()` needs, comes from `plan()`),
//! the planned terrain stands in, and the section's tiles, its stretch of
//! the road and the kits' builds follow, cut to what belongs to the
//! stretch. A `base` section is the horizon under them all: the coarse
//! tiles, the sky and the sea.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use mp_levels::SeasideData;
use mp_track::{JunctionKind, Level, Track};

use crate::road::{MarkGap, SharedRoadSides};
use crate::section::{Place, Section};
use crate::stages::{LevelSetup, level_stages};
use crate::terrain::Terrain;
use crate::textures::{SharedTextures, TextureCache};
use crate::world::{Build, Job, Scenery, SceneryInfo, SimWorldData, Stages, World, run_stage};

/// A level's scenery modules by name (`loadScenery`).
pub type SceneryFn = Arc<dyn Fn(&SceneryInfo) -> Option<Box<dyn Scenery>> + Send + Sync>;
/// The extra modules built after the level's own (the living world).
pub type ExtraFn = Arc<dyn Fn(&Level) -> Vec<Box<dyn Scenery>> + Send + Sync>;

/// Metres of route a streamed section covers.
pub const SECTION_LEN: f64 = 1000.0;
/// How far round its stretch a section looks for scenery modules and
/// terrain (the kits' chunks reach 720 m; what is kept is by the centre).
pub const SECTION_RADIUS: f64 = 700.0;

/// How far from a road a square of a level streamed by place may be and
/// still be a section: the Land kit plants within 450 m of a road.
const PLACE_REACH: f64 = 600.0;

/// A level streamed by place (`Level::stream_cell`, docs/vision/
/// peninsula.md stage 6d): the squares of a grid over the network's box
/// that have a road within [`PLACE_REACH`], numbered row by row from the
/// north-west.
#[derive(Clone, Debug)]
pub struct PlaceGrid {
    pub x0: f64,
    pub z0: f64,
    pub size: f64,
    /// The squares that are sections, as (column, row), in section order.
    pub cells: Vec<(i64, i64)>,
    index: BTreeMap<(i64, i64), i64>,
    cached: Arc<Vec<OnceLock<Arc<Place>>>>,
}

impl PlaceGrid {
    fn new(track: &Track, size: f64) -> PlaceGrid {
        let mut b = track.bounds;
        for r in track.roads() {
            b.min_x = b.min_x.min(r.bounds.min_x);
            b.max_x = b.max_x.max(r.bounds.max_x);
            b.min_z = b.min_z.min(r.bounds.min_z);
            b.max_z = b.max_z.max(r.bounds.max_z);
        }
        let x0 = ((b.min_x - PLACE_REACH) / size).floor() * size;
        let z0 = ((b.min_z - PLACE_REACH) / size).floor() * size;
        let w = ((b.max_x + PLACE_REACH - x0) / size).ceil() as i64;
        let h = ((b.max_z + PLACE_REACH - z0) / size).ceil() as i64;
        let mut cells = Vec::new();
        let mut index = BTreeMap::new();
        for j in 0..h {
            for i in 0..w {
                let cx = x0 + (i as f64 + 0.5) * size;
                let cz = z0 + (j as f64 + 0.5) * size;
                if track.net.has_roads_near(cx, cz, size / 2.0 + PLACE_REACH) {
                    index.insert((i, j), cells.len() as i64);
                    cells.push((i, j));
                }
            }
        }
        let cached = Arc::new((0..cells.len()).map(|_| OnceLock::new()).collect());
        PlaceGrid {
            x0,
            z0,
            size,
            cells,
            index,
            cached,
        }
    }

    fn cell_of(&self, x: f64, z: f64) -> (i64, i64) {
        (
            ((x - self.x0) / self.size).floor() as i64,
            ((z - self.z0) / self.size).floor() as i64,
        )
    }

    /// The section under (x, z): its square's, or the nearest square's
    /// that is a section, within two squares.
    fn under(&self, x: f64, z: f64) -> Option<i64> {
        let (i, j) = self.cell_of(x, z);
        for r in 0..=2i64 {
            for b in j - r..=j + r {
                for a in i - r..=i + r {
                    if (a - i).abs().max((b - j).abs()) != r {
                        continue;
                    }
                    if let Some(&k) = self.index.get(&(a, b)) {
                        return Some(k);
                    }
                }
            }
        }
        None
    }

    /// The sections within `r` squares of section `k`'s, `k` first, then
    /// ring by ring.
    fn ring(&self, k: i64, r: i64) -> Vec<i64> {
        let Some(&(i, j)) = self.cells.get(k as usize) else {
            return Vec::new();
        };
        let mut out = vec![k];
        for d in 1..=r {
            for b in j - d..=j + d {
                for a in i - d..=i + d {
                    if (a - i).abs().max((b - j).abs()) != d {
                        continue;
                    }
                    if let Some(&q) = self.index.get(&(a, b)) {
                        out.push(q);
                    }
                }
            }
        }
        out
    }
}

/// The level planned: what every section builds on.
pub struct Planned {
    pub level: Level,
    pub track: Track,
    /// The terrain as made (the recorded plan applied), before the
    /// modules' plans and the fields: what a section's modules plan on.
    base: Terrain,
    /// The terrain planned and rasterised: what a section builds on.
    terrain: Terrain,
    no_marks: Vec<MarkGap>,
    sim_data: SimWorldData,
    setup: LevelSetup,
    scenery: SceneryFn,
    extra: ExtraFn,
    level_data: Option<Arc<SeasideData>>,
    /// A level streamed by place: its squares (stage 6d).
    pub places: Option<PlaceGrid>,
    textures: SharedTextures,
    road_sides: SharedRoadSides,
}

impl Planned {
    /// Surveys the route, makes the terrain, plans the modules and builds
    /// the fields, as "Surveying the route" and "Shaping the land" do.
    pub fn new(
        level: Level,
        level_data: Option<Arc<SeasideData>>,
        setup: LevelSetup,
        scenery: SceneryFn,
        extra: ExtraFn,
    ) -> Result<Planned, String> {
        let mut w = World::new(level);
        if let Some(d) = &level_data {
            w = w.with_level_data(d.clone());
        }
        w.track = Some(Track::new(&w.level)?);
        let Stages {
            terrain, fields, ..
        } = level_stages(setup.clone());
        run_stage(terrain, &mut w)?;
        let base = w.terrain.clone().ok_or("no terrain")?;
        let _ = crate::world::plan_modules(&mut w, &*scenery, &*extra);
        run_stage(fields, &mut w)?;
        let track = w.track.take().ok_or("no track")?;
        let places = w.level.stream_cell.map(|size| PlaceGrid::new(&track, size));
        let road_sides = Arc::new(
            (0..=track.branches.len())
                .map(|_| OnceLock::new())
                .collect(),
        );
        Ok(Planned {
            places,
            track,
            terrain: w.terrain.take().ok_or("no terrain")?,
            no_marks: w.no_marks.clone(),
            sim_data: w.sim_data.clone(),
            level: w.level,
            base,
            setup,
            scenery,
            extra,
            level_data,
            textures: SharedTextures::default(),
            road_sides,
        })
    }

    /// Each road's sections as (first index, count): the level's road
    /// first, then its branches in order, `SECTION_LEN` of road each (the
    /// last of a road may be short).
    pub fn road_sections(&self) -> Vec<(i64, i64)> {
        let mut out = Vec::new();
        let mut first = 0;
        for t in self.track.roads() {
            let n = ((t.length / SECTION_LEN).ceil() as i64).max(1);
            out.push((first, n));
            first += n;
        }
        out
    }

    /// The road section `k` is on, and which of that road's it is.
    pub fn road_of(&self, k: i64) -> (usize, i64) {
        let rs = self.road_sections();
        for (r, &(first, n)) in rs.iter().enumerate() {
            if k < first + n {
                return (r, (k - first).max(0));
            }
        }
        let r = rs.len() - 1;
        (r, rs[r].1 - 1)
    }

    /// Section `k` (`SECTION_LEN` of its road, from s 0): what the client
    /// streams.
    pub fn section(&self, k: i64) -> Section {
        if let Some(g) = &self.places {
            let idx = (k.max(0) as usize).min(g.cells.len() - 1);
            let place = g.cached[idx]
                .get_or_init(|| {
                    let (i, j) = g.cells[idx];
                    let (x0, z0) = (g.x0 + i as f64 * g.size, g.z0 + j as f64 * g.size);
                    Arc::new(Place::new(&self.track, x0, z0, x0 + g.size, z0 + g.size))
                })
                .clone();
            return Section {
                s0: 0.0,
                s1: 0.0,
                terrain_radius: SECTION_RADIUS,
                scenery_radius: SECTION_RADIUS,
                scenery: true,
                partition: true,
                base: false,
                road: 0,
                place: Some(place),
            };
        }
        let (road, i) = self.road_of(k);
        let s0 = i as f64 * SECTION_LEN;
        Section {
            s0,
            s1: s0 + SECTION_LEN,
            terrain_radius: SECTION_RADIUS,
            scenery_radius: SECTION_RADIUS,
            scenery: true,
            partition: true,
            base: false,
            road,
            place: None,
        }
    }

    /// The section a place `s` on road `road` is in.
    pub fn section_of(&self, road: usize, s: f64) -> i64 {
        if let Some(g) = &self.places {
            let t = self.track.road(road.min(self.track.branches.len()));
            let f = t.frame(if t.is_loop {
                t.wrap(s)
            } else {
                s.clamp(0.0, t.length)
            });
            return g.under(f.x, f.z).unwrap_or(0);
        }
        self.section_in(&self.road_sections(), road, s)
    }

    /// [`Planned::section_of`] with the roads' sections given.
    fn section_in(&self, rs: &[(i64, i64)], road: usize, s: f64) -> i64 {
        let (first, n) = rs[road.min(rs.len() - 1)];
        let t = self.track.road(road.min(rs.len() - 1));
        let s = if t.is_loop { t.wrap(s) } else { s.max(0.0) };
        first + ((s / SECTION_LEN).floor() as i64).min(n - 1)
    }

    /// The section under (x, z): the nearest road's, when one is near.
    pub fn section_under(&self, x: f64, z: f64) -> Option<i64> {
        if let Some(g) = &self.places {
            return g.under(x, z);
        }
        Section::s_of(&self.track, x, z).map(|(r, s)| self.section_of(r, s))
    }

    /// The sections kept up round `k`: `behind` and `ahead` along its road
    /// (wrapped on a loop, clamped at a branch's ends), and at a junction
    /// the other road's sections beside it, so the car turns off onto a
    /// road that is already there: on the greater road, where the ring
    /// covers the junction, the lesser road's two sections there (a
    /// branch's first two at its mouth, its last two at its far end, the
    /// one either side of a crossing); on the lesser road, near the
    /// junction, the greater road's sections round it.
    pub fn ring(&self, k: i64, behind: i64, ahead: i64) -> Vec<i64> {
        // By place: the squares round it, as far as the ring reaches
        // behind (a square a side is a kilometre's view each way).
        if let Some(g) = &self.places {
            return g.ring(k, behind.min(ahead).max(1));
        }
        let rs = self.road_sections();
        let (road, i) = self.road_of(k);
        let mut out: Vec<i64> = Vec::new();
        let push = |out: &mut Vec<i64>, k: i64| {
            if !out.contains(&k) {
                out.push(k);
            }
        };
        let along = |r: usize, i: i64, behind: i64, ahead: i64, out: &mut Vec<i64>| {
            let (first, n) = rs[r];
            let t = self.track.road(r);
            for j in i - behind..=i + ahead {
                let idx = if t.is_loop {
                    j.rem_euclid(n)
                } else {
                    j.clamp(0, n - 1)
                };
                push(out, first + idx);
            }
        };
        along(road, i, behind, ahead, &mut out);
        for j in &self.track.junctions {
            let (lf, ln) = rs[j.road];
            let at = self.section_in(&rs, j.other, j.other_s);
            // The lesser road's sections at the junction, and how near its
            // own the ring's must be.
            let (mine, near): (Vec<i64>, Box<dyn Fn(i64) -> bool>) = match j.kind {
                JunctionKind::Start => (vec![lf, lf + 1.min(ln - 1)], Box::new(|i| i <= 1)),
                JunctionKind::End => (
                    vec![lf + ln - 1, lf + (ln - 2).max(0)],
                    Box::new(move |i| i >= ln - 2),
                ),
                JunctionKind::Cross => {
                    let c = self.section_in(&rs, j.road, j.s) - lf;
                    (
                        vec![lf + c, lf + (c + 1).min(ln - 1), lf + (c - 1).max(0)],
                        Box::new(move |i| (i - c).abs() <= 1),
                    )
                }
            };
            if road == j.other {
                if out.contains(&at) {
                    for m in mine {
                        push(&mut out, m);
                    }
                }
            } else if road == j.road && near(i) {
                let (_, m) = self.road_of(at);
                along(j.other, m, 1, 1, &mut out);
            }
        }
        out
    }

    /// The horizon layer.
    pub fn base_section(&self) -> Section {
        Section {
            s0: 0.0,
            s1: self.track.length,
            scenery: false,
            partition: true,
            base: true,
            ..Section::default()
        }
    }

    /// How many sections the network has: every road's.
    pub fn sections(&self) -> i64 {
        if let Some(g) = &self.places {
            return g.cells.len() as i64;
        }
        self.road_sections().iter().map(|r| r.1).sum()
    }

    /// The section a place on the level's road is in.
    pub fn section_at(&self, s: f64) -> i64 {
        self.section_of(0, s)
    }

    /// The build of one section (or the base) over the plan: its jobs run
    /// as a level's do, then [`crate::section::finish`] cuts and finishes.
    pub fn build(&self, sec: Section) -> Build {
        let mut w = World::new(self.level.clone());
        if let Some(d) = &self.level_data {
            w = w.with_level_data(d.clone());
        }
        w.textures = TextureCache::with_shared(self.textures.clone());
        w.track = Some(self.track.clone());
        w.terrain = Some(self.base.clone());
        w.section = Some(sec.clone());
        w.road_sides = Some(self.road_sides.clone());
        let planned = self.terrain.clone();
        let no_marks = self.no_marks.clone();
        let sim_data = self.sim_data.clone();
        let (scenery, extra) = (self.scenery.clone(), self.extra.clone());
        let Stages {
            terrain_meshes,
            road,
            sky,
            sea,
            ..
        } = level_stages(self.setup.clone());
        let base = sec.base;
        let shape = Job::serial("Shaping the land", 0.06, move |w| {
            let kept = crate::world::plan_modules(w, &*scenery, &*extra);
            w.terrain = Some(planned);
            w.no_marks = no_marks;
            w.sim_data = sim_data;
            let mut out = vec![Job::serial("Sculpting terrain", 0.1, move |w| {
                run_stage(terrain_meshes, w)
            })];
            if base {
                out.push(Job::serial("Paving roads", 0.67, move |w| {
                    run_stage(sky, w)
                }));
                if w.level.sea_y.is_some() {
                    out.push(Job::serial("Filling the sea", 0.7, move |w| {
                        run_stage(sea, w)
                    }));
                }
            } else {
                out.push(Job::serial("Paving roads", 0.67, move |w| {
                    run_stage(road, w)
                }));
                out.extend(crate::world::build_jobs(kept));
            }
            out.push(Job::serial("Ready", 1.0, |_| Ok(Vec::new())));
            Ok(out)
        });
        Build::new(w, vec![shape])
    }
}
