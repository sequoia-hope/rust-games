//! The Town kit: an open level's towns and cities drawn
//! (docs/vision/peninsula.md, stage 6e). The buildings are the level's
//! ([`mp_track::Track::buildings`], placed by `mp_levels::city`, collided
//! with by the simulation); this draws each on Downtown Streets' façade
//! atlas, whose shader lights every window by the building's seed, so a
//! city at night glows window by window: walls from below the ground to
//! the roof, a cap over the roof, and plant on the towers.
//!
//! A section draws the buildings that stand in it, merged in chunks small
//! enough that each chunk's centre lies in the section that drew it.

use mp_track::Footprint;
use pk_math::{Mulberry32, kernel};

use crate::geom::{GeoBuilder, P2, P3, PrismOpts, StaticOpts};
use crate::streets::Chunks;
use crate::streets::buildings::wall;
use crate::streets::facades::{F_TILE, f};
use crate::world::{Scenery, SceneryInfo, World};

/// Metres of ground a merged chunk covers.
const CHUNK: f64 = 200.0;

pub struct Town {
    pub label: &'static str,
    pub zone: usize,
}

impl Town {
    pub fn new(info: &SceneryInfo) -> Town {
        Town {
            label: "Raising the town",
            zone: info.zone,
        }
    }
}

/// The façade cells each kind of building draws from.
const CELLS: [&[usize]; 4] = [
    // Houses.
    &[f::BRICK, f::NAPT, f::NTILE, f::BRICK],
    // Low shops and flats.
    &[f::BRICK, f::NAPT, f::HOTEL, f::NTILE, f::STONE],
    // Mid-rises.
    &[f::STONE, f::RIBBON, f::BANDS, f::NAPT, f::HOTEL, f::BRICK],
    // Towers.
    &[
        f::CURTAIN,
        f::DARK,
        f::FINS,
        f::RIBBON,
        f::BANDS,
        f::CURTAIN,
    ],
];

/// `{ cell, color: fd, tileW: 4, tileH: 4, roofCell, roofTile }`, as
/// Streets' panels.
fn panel(cell: f64, fd: P3, roof_cell: f64, roof_tile: Option<f64>) -> PrismOpts {
    PrismOpts {
        cell: Some(cell),
        color: Some(fd),
        tile_w: Some(4.0),
        tile_h: Some(4.0),
        roof_cell: Some(roof_cell),
        roof_tile,
        ..PrismOpts::default()
    }
}

/// One building into a chunk's builder.
pub fn draw(b: &mut GeoBuilder, fp: &Footprint) {
    let mut rng = Mulberry32::new(fp.seed);
    let kind = (fp.style as usize).min(3);
    let cells = CELLS[kind];
    let cell = cells[(rng.next_f64() * cells.len() as f64) as usize % cells.len()];
    // Streets' seeds run 1..900: the shader picks each window's light
    // from the cell's index over 16.
    let seed = 1.0 + (rng.next_f64() * 900.0).floor();
    let street = fp.y + 0.15;
    let top = fp.y + fp.height;
    let hue = 0.02 + rng.next_f64() * 0.96;
    let fd = [street, if kind == 3 { 0.75 } else { 1.0 }, hue];
    let poly: Vec<P2> = fp.corners().to_vec();
    let tile = F_TILE[cell];
    // The walls, u running on round the corners so the windows' indices
    // (and lights) carry on round the building.
    let mut u = (rng.next_f64() * 4.0).floor() / 4.0;
    for k in 0..4 {
        let a = poly[k];
        let c = poly[(k + 1) % 4];
        wall(
            b,
            a,
            c,
            fp.y - 1.5,
            top,
            cell as f64 + 16.0 * seed,
            tile,
            street,
            fd,
            [fp.x, fp.z],
            u,
        );
        u += kernel::hypot(c[0] - a[0], c[1] - a[1]) / tile[0];
    }
    let yaw = kernel::atan2(fp.uz, fp.ux);
    let pan = f::PANEL as f64 + 16.0 * seed;
    // A cap over the roof, a little proud of the walls.
    b.box_(
        fp.x,
        top - 0.5,
        fp.z,
        fp.hx * 2.0 + 0.4,
        1.0,
        fp.hz * 2.0 + 0.4,
        yaw,
        &panel(pan, fd, f::ROOF as f64 + 16.0 * seed, Some(16.0)),
    );
    // Plant on the roofs of the taller ones, a crown on a tower.
    if kind >= 2 {
        let (sx, sz) = (
            fp.hx * (0.5 + 0.3 * rng.next_f64()),
            fp.hz * (0.5 + 0.3 * rng.next_f64()),
        );
        let h = if kind == 3 {
            4.0 + 6.0 * rng.next_f64()
        } else {
            2.5
        };
        let mech = f::MECH as f64 + 16.0 * seed;
        b.box_(
            fp.x,
            top + 0.5,
            fp.z,
            sx,
            h,
            sz,
            yaw,
            &panel(mech, fd, f::ROOF as f64 + 16.0 * seed, Some(16.0)),
        );
    }
}

/// The sidewalks of the town's streets (stage 7b, D1246), across as the
/// simulation's ground has them (`mp_track::Track::ground_y_walk`): a
/// gutter on the road's plane out to its shoulder, the kerb's ramp
/// `KERB_RUN` wide up to `KERB`, the pavement level across out to `WALK`,
/// and a slope `WALK_RUN` wide down to the land, every 4 m that this build
/// holds, each piece left out where it would lie on another road's tarmac.
fn sidewalks(world: &World, chunks: &mut Chunks) {
    let (Some(net), Some(terrain)) = (&world.track, &world.terrain) else {
        return;
    };
    for (r, t) in net.roads().enumerate() {
        if r == 0 {
            continue;
        }
        let mut s = 0.0;
        while s + 4.0 <= t.length {
            let held = world
                .section
                .as_ref()
                .is_none_or(|sec| sec.holds_road(r, t, s, sec.scenery_radius));
            if held && t.walks(s) && t.walks(s + 4.0) {
                for side in [-1.0, 1.0] {
                    strip(net, r, t, terrain, s, side, chunks);
                }
            }
            s += 4.0;
        }
    }
}

/// One side's sidewalk from `s` to `s + 4`.
fn strip(
    net: &mp_track::Track,
    r: usize,
    t: &mp_track::Track,
    terrain: &crate::terrain::Terrain,
    s: f64,
    side: f64,
    chunks: &mut Chunks,
) {
    use mp_track::{KERB_RUN, WALK, WALK_RUN};
    let grey = Some([0.62, 0.6, 0.57]);
    // The profile's lateral offsets from the centreline and heights, at
    // one end of the strip.
    let across = |s: f64| -> [(P2, f64); 5] {
        let f = t.frame(s);
        let edge = f.hw + f.margin;
        let p = |lat: f64| [f.x + f.rx * lat * side, f.z + f.rz * lat * side];
        let base = t.surface_y(s, edge * side) + 0.01;
        let top = t.walk_y(s, side);
        let foot = p(edge + WALK + WALK_RUN);
        let land = terrain.height_at(foot[0], foot[1]) - 0.05;
        [
            (p(f.hw), t.surface_y(s, f.hw * side) + 0.01),
            (p(edge), base),
            (p(edge + KERB_RUN), top),
            (p(edge + WALK), top),
            (foot, land.min(top)),
        ]
    };
    let (a, b) = (across(s), across(s + 4.0));
    for k in 0..4 {
        let corners = [a[k].0, a[k + 1].0, b[k].0, b[k + 1].0];
        if corners.iter().any(|c| net.on_other_tarmac(r, c[0], c[1])) {
            continue;
        }
        let q = |p: (P2, f64)| [p.0[0], p.1, p.0[1]];
        let (ia, oa, ib, ob) = (q(a[k]), q(a[k + 1]), q(b[k]), q(b[k + 1]));
        let mid = [
            (a[k].0[0] + b[k + 1].0[0]) / 2.0,
            (a[k].0[1] + b[k + 1].0[1]) / 2.0,
        ];
        let g = chunks.at(mid[0], mid[1]);
        // UVs in metres over 4, across from the road's edge and along.
        let (u0, u1) = (k as f64, k as f64 + 1.0);
        let uv = |u: f64, v: f64| [u / 4.0, v / 4.0];
        // Wound to face up whichever side it is on (the right is south of
        // east here: forward × right points down).
        if side < 0.0 {
            g.quad(
                ia,
                ib,
                ob,
                oa,
                Some([uv(u0, s), uv(u0, s + 4.0), uv(u1, s + 4.0), uv(u1, s)]),
                grey,
                0.0,
            );
        } else {
            g.quad(
                oa,
                ob,
                ib,
                ia,
                Some([uv(u1, s), uv(u1, s + 4.0), uv(u0, s + 4.0), uv(u0, s)]),
                grey,
                0.0,
            );
        }
    }
}

impl Scenery for Town {
    fn name(&self) -> &str {
        "Town"
    }

    fn label(&self) -> Option<&str> {
        Some(self.label)
    }

    fn build(&mut self, world: &mut World) -> Result<(), String> {
        let t = world.track.as_ref().ok_or("the route is surveyed first")?;
        let list = t.buildings.clone();
        let mine: Vec<usize> = (0..list.list.len())
            .filter(|&i| {
                let fp = &list.list[i];
                crate::section::owns_point(world, fp.x, fp.z)
            })
            .collect();
        let mut walks = Chunks::new(CHUNK, true, false);
        sidewalks(world, &mut walks);
        let World {
            graph,
            textures,
            root,
            ..
        } = world;
        let fac = crate::streets::facade_material(graph, textures);
        let mut chunks = Chunks::new(CHUNK, true, true);
        for i in mine {
            let fp = &list.list[i];
            draw(chunks.at(fp.x, fp.z), fp);
        }
        let group = graph.group("town");
        chunks.emit(
            graph,
            group,
            fac,
            &StaticOpts {
                cast: true,
                receive: true,
                name: String::new(),
            },
        );
        // The sidewalks: Downtown Streets' pavement, lit a little by the
        // street as its are.
        let pv = crate::streets::textures::pavement_texture(textures);
        let pv = graph.cached_texture(&pv, crate::object::Layer::Main, "");
        let pave = graph.add_material(crate::streets::ambient_patch(
            crate::material::Material::standard()
                .set("map", pv)
                .set("vertexColors", true)
                .set("roughness", 0.78),
            [0.05, 0.042, 0.036],
            "pave",
        ));
        walks.emit(
            graph,
            group,
            pave,
            &StaticOpts {
                cast: false,
                receive: true,
                name: String::new(),
            },
        );
        graph.add(*root, group);
        Ok(())
    }
}
