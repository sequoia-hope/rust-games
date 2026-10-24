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

use mp_track::{Footprint, Track};
use pk_math::{Mulberry32, kernel};

use crate::geom::{GeoBuilder, P2, P3, PrismOpts, StaticOpts};
use crate::streets::Chunks;
use crate::streets::buildings::wall;
use crate::streets::facades::{F_TILE, f};
use crate::world::{Scenery, SceneryInfo, World};

/// Metres of ground a merged chunk covers.
const CHUNK: f64 = 200.0;
/// The sidewalk either side of a town's streets (`mp_levels::city::WALK`)
/// and its height over the road at the kerb.
const WALK: f64 = 4.0;
const KERB: f64 = 0.15;

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

/// Whether the sidewalk at (x, z) beside road `road` would lie on another
/// road's tarmac (a crossing, a mouth).
fn on_another(net: &Track, road: usize, x: f64, z: f64) -> bool {
    net.net.roads_near(x, z, 30.0).into_iter().any(|r| {
        if r == road {
            return false;
        }
        let t = net.road(r);
        let d = t.distance_to_road(x, z, 30.0);
        d.s.is_some_and(|s| {
            let f = t.frame(s.clamp(0.0, t.length));
            d.d < f.hw + f.margin + 0.5
        })
    })
}

/// The sidewalks of the town's streets: a strip of pavement either side
/// from the kerb out [`WALK`] metres, level across at kerb height, with a
/// skirt down to the land beyond, every 4 m that this build holds.
fn sidewalks(world: &World, chunks: &mut Chunks) {
    let (Some(net), Some(terrain)) = (&world.track, &world.terrain) else {
        return;
    };
    for (r, t) in net.roads().enumerate() {
        let zone = t.zone.first().copied().unwrap_or(0) as usize;
        let town = world
            .level
            .zones
            .get(zone)
            .is_some_and(|z| z.scenery == "Town");
        if r == 0 || !town {
            continue;
        }
        let mut s = 0.0;
        while s + 4.0 <= t.length {
            let held = world
                .section
                .as_ref()
                .is_none_or(|sec| sec.holds_road(r, t, s, sec.scenery_radius));
            let key = mp_track::ROAD_TYPES
                .get(t.road_type[t.idx(s)] as usize)
                .map_or("", |rt| rt.key);
            if held && key != "freeway" {
                let (a, b) = (t.frame(s), t.frame(s + 4.0));
                for side in [-1.0, 1.0] {
                    let lat0 = |f: &mp_track::Frame| f.hw + f.margin;
                    let p = |f: &mp_track::Frame, lat: f64| {
                        [f.x + f.rx * lat * side, f.z + f.rz * lat * side]
                    };
                    let mid = p(&a, lat0(&a) + WALK / 2.0);
                    let ya = t.surface_y(s, lat0(&a) * side) + KERB;
                    let yb = t.surface_y(s + 4.0, lat0(&b) * side) + KERB;
                    let (ai, ao) = (p(&a, lat0(&a)), p(&a, lat0(&a) + WALK));
                    let (bi, bo) = (p(&b, lat0(&b)), p(&b, lat0(&b) + WALK));
                    if [mid, ai, ao, bi, bo]
                        .iter()
                        .any(|c| on_another(net, r, c[0], c[1]))
                    {
                        continue;
                    }
                    let g = chunks.at(mid[0], mid[1]);
                    let grey = Some([0.62, 0.6, 0.57]);
                    let uv = |l: f64, u: f64| [l / 4.0, u / 4.0];
                    // Wound to face up whichever side it is on (the right is south of
                    // east here: forward × right points down).
                    let (q0, q1, q2, q3) = if side < 0.0 {
                        (
                            [ai[0], ya, ai[1]],
                            [bi[0], yb, bi[1]],
                            [bo[0], yb, bo[1]],
                            [ao[0], ya, ao[1]],
                        )
                    } else {
                        (
                            [ao[0], ya, ao[1]],
                            [bo[0], yb, bo[1]],
                            [bi[0], yb, bi[1]],
                            [ai[0], ya, ai[1]],
                        )
                    };
                    g.quad(
                        q0,
                        q1,
                        q2,
                        q3,
                        Some([uv(0.0, s), uv(0.0, s + 4.0), uv(WALK, s + 4.0), uv(WALK, s)]),
                        grey,
                        0.0,
                    );
                    // The skirt down to the land past the outer edge.
                    let (ga, gb) = (
                        terrain.height_at(ao[0], ao[1]),
                        terrain.height_at(bo[0], bo[1]),
                    );
                    if ga < ya - 0.05 || gb < yb - 0.05 {
                        let (lo_a, lo_b) = (ga.min(ya) - 0.3, gb.min(yb) - 0.3);
                        let (w0, w1, w2, w3) = if side < 0.0 {
                            (
                                [ao[0], lo_a, ao[1]],
                                [ao[0], ya, ao[1]],
                                [bo[0], yb, bo[1]],
                                [bo[0], lo_b, bo[1]],
                            )
                        } else {
                            (
                                [bo[0], lo_b, bo[1]],
                                [bo[0], yb, bo[1]],
                                [ao[0], ya, ao[1]],
                                [ao[0], lo_a, ao[1]],
                            )
                        };
                        g.quad(
                            w0,
                            w1,
                            w2,
                            w3,
                            Some([uv(0.0, 0.0), uv(0.0, 1.0), uv(1.0, 1.0), uv(1.0, 0.0)]),
                            grey,
                            0.0,
                        );
                    }
                }
            }
            s += 4.0;
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
