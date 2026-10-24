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

impl Scenery for Town {
    fn name(&self) -> &str {
        "Town"
    }

    fn label(&self) -> Option<&str> {
        Some(self.label)
    }

    fn build(&mut self, world: &mut World) -> Result<(), String> {
        let t = world.track.as_ref().ok_or("the route is surveyed first")?;
        if t.buildings.is_empty() {
            return Ok(());
        }
        let list = t.buildings.clone();
        let mine: Vec<usize> = (0..list.list.len())
            .filter(|&i| {
                let fp = &list.list[i];
                crate::section::owns_point(world, fp.x, fp.z)
            })
            .collect();
        if mine.is_empty() {
            return Ok(());
        }
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
        graph.add(*root, group);
        Ok(())
    }
}
