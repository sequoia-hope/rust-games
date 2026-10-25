//! The Land kit: the open world's planting by land cover
//! (docs/vision/peninsula.md, stage 4a; DECISIONS D1212). Not a port: the
//! JS game has no open world.
//!
//! An open level carries the real land cover of its box (`Level::cover`,
//! the atlas's planning grid: forest, shrub, grass, crop, urban, water...).
//! Within reach of the roads (the terrain's far field, which knows every
//! road of the network) the kit walks the land in cells and plants what
//! the cover says: conifers where it is forest (firs and spruce on the
//! verge, cheaper spruce behind, far quads beyond, as the Sierra's forest
//! is built), oaks and scrub where it is shrub, a scattering of oaks and
//! scrub on grass, nothing on crop, urban, barren, sand or water, nothing
//! on the road's clearance, under the sea or on the steepest ground.
//!
//! What grows where, and where the crop land's fences stand, is the
//! simulation's since stage 7d (D1248): [`mp_track::planting`] places
//! every plant and post from the level's ground and cover and the road
//! network, the same rules this kit had, so the car stops against every
//! trunk and fence drawn. The kit draws them: each on the terrain's ground,
//! its looks (tilt, tint, brightness) from the plant's own seed. A cell is
//! the planting's tiles in it and the fences standing in it, so a cell
//! draws the same whatever section asks for it, and each cell's instances
//! of a template are one `InstancedMesh`. On the coast it draws the Coast
//! kit's cypress, pines and outcrops too, which the planting places by
//! that kit's rules.
//!
//! On crop (stage 4d) the land is fields (the terrain paints the parcels,
//! `terrain_mesh`), and the fences are a post-and-rail line along the
//! road's edge wherever the verge is crop, and field boundaries running out
//! from the road.

use mp_scene::three;
use mp_track::planting::{Kind, POST, TILE};
use pk_math::{Mulberry32, kernel, lerp, smoothstep};

use crate::flora::{canopy_geometry, conifer_geometry, foliage_material, shrub_geometry};
use crate::material::{Material, Param};
use crate::mountain::kit::{Item, SurfaceSampler, instanced};
use crate::object::{GeoId, MaterialId, NodeId, SceneGraph};
use crate::terrain::Terrain;
use crate::three_geom::{BufferGeometry, cylinder_geometry, merge_geometries};
use crate::world::{Scenery, SceneryInfo, World};

/// Metres of land a cell is on a side: one draw call per template a cell.
pub const CELL: f64 = 512.0;
/// Nothing is planted farther than this from the nearest road (m): past
/// the ridge a tree is never seen from the car.
pub const REACH: f64 = mp_track::planting::REACH;
const DOUBLE_SIDE: f64 = three::DOUBLE_SIDE as f64;
const FRONT_SIDE: f64 = three::FRONT_SIDE as f64;

/// Instance tints over the vertex colours: the conifers' and the oaks'
/// greens, the scrub's dusty olives.
const CONIFER_COLS: [u32; 6] = [0xffffff, 0xe8f0e4, 0xd4e0d2, 0xf2ecd8, 0xc6d4c0, 0xe0e8f0];
// The canopy's vertex colours are shading only (the valley tints its
// shade trees the same way): the greens are the instance's.
const OAK_COLS: [u32; 5] = [0x40592a, 0x4a6a2e, 0x3a5224, 0x5a6e30, 0x475f2b];
/// The Coast kit's outcrops' sandstone.
const ROCK_COLS: [u32; 6] = [0xc9ad8c, 0xb89c7e, 0xa89484, 0xd2b99a, 0x9e8f82, 0x8e8074];
const SCRUB_COLS: [u32; 7] = [
    0x5f6e4c, 0x6b7458, 0x4d5a36, 0x6e5a3c, 0x55663e, 0x7a8068, 0x4a5638,
];

/// What a cell plants, per template.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Template {
    Fir,
    Spruce,
    SpruceMid,
    SpruceFar,
    Oak,
    OakFar,
    Scrub,
    Post,
    Rail,
    Cypress,
    Pine,
    Rock,
}

const TEMPLATES: [Template; 12] = [
    Template::Fir,
    Template::Spruce,
    Template::SpruceMid,
    Template::SpruceFar,
    Template::Oak,
    Template::OakFar,
    Template::Scrub,
    Template::Post,
    Template::Rail,
    Template::Cypress,
    Template::Pine,
    Template::Rock,
];

/// What the kit planted, for the log and the tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub cells: usize,
    pub conifers: usize,
    pub far_conifers: usize,
    pub oaks: usize,
    pub scrub: usize,
    /// The Coast kit's cypress and pines, and its outcrops (D1248).
    pub coast_trees: usize,
    pub rocks: usize,
    /// Fence posts, and the rails between them.
    pub posts: usize,
    pub rails: usize,
    pub meshes: usize,
}

pub struct Land {
    #[allow(dead_code)]
    zone: usize,
    label: &'static str,
    pub counts: Counts,
}

impl Land {
    pub fn new(info: &SceneryInfo) -> Land {
        Land {
            zone: info.zone,
            label: "Planting the land",
            counts: Counts::default(),
        }
    }
}

/// An oak: a short trunk under a lobed canopy, one template (the valley's
/// shade trees are two instanced meshes; here a tree is one instance).
fn oak_geometry(seed: u32, lod: usize) -> BufferGeometry {
    let mut trunk = cylinder_geometry(
        0.09,
        0.14,
        1.1,
        6.0,
        1.0,
        false,
        0.0,
        core::f64::consts::TAU,
    );
    trunk.translate(0.0, 0.55, 0.0);
    if trunk.has_attribute("uv") {
        trunk.delete_attribute("uv");
    }
    let trunk = crate::coast::kit::colorize(trunk.to_non_indexed(), 0x4a3a2a);
    let mut crown = canopy_geometry("shade", seed, lod);
    crown.scale(1.25, 1.0, 1.25);
    crown.translate(0.0, 1.55, 0.0);
    if crown.has_attribute("uv") {
        crown.delete_attribute("uv");
    }
    let crown = if crown.index.is_some() {
        crown.to_non_indexed()
    } else {
        crown
    };
    merge_geometries(&[&trunk, &crown], false).expect("trunk and crown share their attributes")
}

/// A fence post: a weathered square post, its foot at the origin.
fn post_geometry() -> BufferGeometry {
    let mut g = crate::three_geom::box_geometry(0.13, 1.25, 0.13, 1.0, 1.0, 1.0);
    g.translate(0.0, 0.62, 0.0);
    crate::coast::kit::colorize(g.to_non_indexed(), 0x6e5a44)
}

/// Two rails between posts, along +Z, [`POST`] long.
fn rail_geometry() -> BufferGeometry {
    let mut a = crate::three_geom::box_geometry(0.05, 0.07, POST, 1.0, 1.0, 1.0);
    a.translate(0.0, 1.05, 0.0);
    let mut b = crate::three_geom::box_geometry(0.05, 0.07, POST, 1.0, 1.0, 1.0);
    b.translate(0.0, 0.6, 0.0);
    let a = crate::coast::kit::colorize(a.to_non_indexed(), 0x7a6650);
    let b = crate::coast::kit::colorize(b.to_non_indexed(), 0x7a6650);
    merge_geometries(&[&a, &b], false).expect("two boxes share their attributes")
}

impl Scenery for Land {
    fn name(&self) -> &str {
        "Land"
    }

    fn label(&self) -> Option<&str> {
        Some(self.label)
    }

    fn build(&mut self, world: &mut World) -> Result<(), String> {
        // A section (the menu's views, 4b's streaming) plants only the
        // cells within its reach.
        let ci0 = (world.terrain.as_ref().map_or(0.0, |t| t.min_x) / CELL).floor() as i64;
        let ci1 = (world.terrain.as_ref().map_or(0.0, |t| t.max_x) / CELL).ceil() as i64;
        let cj0 = (world.terrain.as_ref().map_or(0.0, |t| t.min_z) / CELL).floor() as i64;
        let cj1 = (world.terrain.as_ref().map_or(0.0, |t| t.max_z) / CELL).ceil() as i64;
        let mut wanted: Vec<(i64, i64)> = Vec::new();
        for cj in cj0..cj1 {
            for ci in ci0..ci1 {
                let (x0, z0) = (ci as f64 * CELL, cj as f64 * CELL);
                if crate::section::wants_box(world, x0, z0, x0 + CELL, z0 + CELL) {
                    wanted.push((ci, cj));
                }
            }
        }
        let World {
            track,
            terrain,
            graph,
            root,
            textures,
            ..
        } = world;
        let t = track.as_ref().ok_or("the route is surveyed first")?;
        let terrain: &Terrain = terrain.as_ref().ok_or("no terrain")?;
        let planting = t
            .props
            .planting()
            .ok_or("the level is not planted (an open level with a land cover)")?;
        let group = graph.group("land");
        let mut surf = SurfaceSampler::new(terrain);

        // The templates and their materials, made once.
        let foliage = graph.add_material(foliage_material(&[]));
        let far_mat = graph.add_material(
            Material::lambert()
                .set("vertexColors", true)
                .set("side", DOUBLE_SIDE),
        );
        let scrub_mat = graph.add_material(foliage_material(&[("side", Param::Num(FRONT_SIDE))]));
        let wood_mat = graph.add_material(
            Material::lambert()
                .set("vertexColors", true)
                .set("side", FRONT_SIDE),
        );
        let mut geos: Vec<(Template, GeoId, MaterialId, bool)> = vec![
            (
                Template::Fir,
                graph.add_geometry(conifer_geometry("fir", 0, 31)),
                foliage,
                true,
            ),
            (
                Template::Spruce,
                graph.add_geometry(conifer_geometry("spruce", 0, 32)),
                foliage,
                true,
            ),
            (
                Template::SpruceMid,
                graph.add_geometry(conifer_geometry("spruce", 1, 33)),
                foliage,
                true,
            ),
            (
                Template::SpruceFar,
                graph.add_geometry(conifer_geometry("spruce", 2, 34)),
                far_mat,
                false,
            ),
            (
                Template::Oak,
                graph.add_geometry(oak_geometry(35, 0)),
                foliage,
                true,
            ),
            (
                Template::OakFar,
                graph.add_geometry(oak_geometry(35, 1)),
                foliage,
                false,
            ),
            (
                Template::Scrub,
                graph.add_geometry(shrub_geometry(36, 1.0)),
                scrub_mat,
                false,
            ),
            (
                Template::Post,
                graph.add_geometry(post_geometry()),
                wood_mat,
                true,
            ),
            (
                Template::Rail,
                graph.add_geometry(rail_geometry()),
                wood_mat,
                false,
            ),
        ];

        let coast_mat = graph.add_material(
            Material::standard()
                .set("vertexColors", true)
                .set("roughness", 0.9)
                .set("flatShading", true),
        );
        geos.push((
            Template::Cypress,
            graph.add_geometry(crate::coast::cypress_geometry(1)),
            coast_mat,
            true,
        ));
        geos.push((
            Template::Pine,
            graph.add_geometry(crate::coast::pine_geometry(1)),
            coast_mat,
            true,
        ));
        let rock_mat = {
            let m = crate::coast::kit::rock_material(graph, textures);
            graph.add_material(m)
        };
        geos.push((
            Template::Rock,
            graph.add_geometry(crate::coast::kit::rock_geometry(33, 1.0, 0.72, 0.42)),
            rock_mat,
            true,
        ));

        // The cells over the terrain's box, on a grid fixed in the world
        // (not the box), so a cell is the same cell on every level; each
        // the planting's tiles in it and the fences standing in it.
        let per = (CELL / TILE) as i64;
        let pick =
            |rng: &mut Mulberry32, a: &[u32]| a[(rng.next_f64() * a.len() as f64).floor() as usize];
        let slot = |tpl: Template| {
            TEMPLATES
                .iter()
                .position(|&p| p == tpl)
                .expect("a template")
        };
        let mut counts = Counts::default();
        for &(ci, cj) in &wanted {
            let x0 = ci as f64 * CELL;
            let z0 = cj as f64 * CELL;
            let mut items: Vec<Vec<Item>> = vec![Vec::new(); TEMPLATES.len()];
            for tj in cj * per..(cj + 1) * per {
                for ti in ci * per..(ci + 1) * per {
                    for p in planting.plants(t, ti, tj) {
                        let mut look = p.look;
                        let (x, z) = (p.x, p.z);
                        let sv = surf.sample(x, z);
                        let (tpl, item) = match p.kind {
                            Kind::Fir | Kind::Spruce | Kind::SpruceMid | Kind::SpruceFar => (
                                match p.kind {
                                    Kind::Fir => Template::Fir,
                                    Kind::Spruce => Template::Spruce,
                                    Kind::SpruceMid => Template::SpruceMid,
                                    _ => Template::SpruceFar,
                                },
                                Item {
                                    ry: p.ry,
                                    rx: (look.next_f64() - 0.5) * 0.05,
                                    rz: (look.next_f64() - 0.5) * 0.05,
                                    col: Some(pick(&mut look, &CONIFER_COLS)),
                                    b: Some(lerp(0.7, 1.05, look.next_f64())),
                                    ..Item::at(x, sv.h - 0.4, z, p.sx, p.sy, p.sz)
                                },
                            ),
                            Kind::Oak | Kind::OakFar => (
                                if p.kind == Kind::Oak {
                                    Template::Oak
                                } else {
                                    Template::OakFar
                                },
                                Item {
                                    ry: p.ry,
                                    col: Some(pick(&mut look, &OAK_COLS)),
                                    b: Some(lerp(0.75, 1.05, look.next_f64())),
                                    ..Item::at(x, sv.h - 0.25, z, p.sx, p.sy, p.sz)
                                },
                            ),
                            Kind::Scrub => {
                                let steep = smoothstep(0.35, 0.8, sv.slope);
                                (
                                    Template::Scrub,
                                    Item {
                                        ry: p.ry,
                                        col: Some(pick(&mut look, &SCRUB_COLS)),
                                        b: Some(lerp(0.8, 1.05, look.next_f64())),
                                        ..Item::at(
                                            x,
                                            sv.h - 0.12 - steep * p.sz * 0.45,
                                            z,
                                            p.sx,
                                            p.sy,
                                            p.sz,
                                        )
                                    },
                                )
                            }
                            Kind::Rock => (
                                Template::Rock,
                                Item {
                                    ry: p.ry,
                                    col: Some(pick(&mut look, &ROCK_COLS)),
                                    b: Some(lerp(0.75, 1.05, look.next_f64())),
                                    ..Item::at(x, sv.h - p.sz * 0.35, z, p.sx, p.sy, p.sz)
                                },
                            ),
                            Kind::Cypress | Kind::Pine => (
                                if p.kind == Kind::Cypress {
                                    Template::Cypress
                                } else {
                                    Template::Pine
                                },
                                Item {
                                    ry: p.ry,
                                    col: Some(0xffffff),
                                    b: Some(lerp(0.75, 1.1, look.next_f64())),
                                    ..Item::at(x, sv.h - 0.3, z, p.sx, p.sy, p.sz)
                                },
                            ),
                        };
                        items[slot(tpl)].push(item);
                    }
                }
            }
            // The fences of the crop land: along every road's edge where
            // the verge is crop, and field boundaries out from the road.
            let fences = planting.fences(t, x0, z0, x0 + CELL, z0 + CELL);
            for &[x, z] in &fences.posts {
                let h = surf.sample(x, z).h;
                items[slot(Template::Post)].push(Item::at(x, h - 0.05, z, 1.0, 1.0, 1.0));
            }
            for &[a, b] in &fences.rails {
                let (ha, hb) = (surf.sample(a[0], a[1]).h, surf.sample(b[0], b[1]).h);
                items[slot(Template::Rail)].push(Item {
                    ry: kernel::atan2(b[0] - a[0], b[1] - a[1]),
                    rx: kernel::atan2(ha - hb, POST),
                    ..Item::at(
                        (a[0] + b[0]) / 2.0,
                        (ha + hb) / 2.0 - 0.05,
                        (a[1] + b[1]) / 2.0,
                        1.0,
                        1.0,
                        1.0,
                    )
                });
            }
            counts.posts += fences.posts.len();
            counts.rails += fences.rails.len();
            let mut planted = false;
            for (k, list) in items.iter().enumerate() {
                if list.is_empty() {
                    continue;
                }
                let (tpl, geo, mat, cast) = geos[k];
                let n = instanced(graph, geo, mat, list, cast, true);
                graph.add(group, n);
                planted = true;
                counts.meshes += 1;
                match tpl {
                    Template::SpruceFar => counts.far_conifers += list.len(),
                    Template::Fir | Template::Spruce | Template::SpruceMid => {
                        counts.conifers += list.len()
                    }
                    Template::Oak | Template::OakFar => counts.oaks += list.len(),
                    Template::Scrub => counts.scrub += list.len(),
                    Template::Cypress | Template::Pine => counts.coast_trees += list.len(),
                    Template::Rock => counts.rocks += list.len(),
                    Template::Post | Template::Rail => {}
                }
            }
            if planted {
                counts.cells += 1;
            }
        }
        graph.add(*root, group);
        graph.log.push(format!(
            "Land: {} cells, {} conifers ({} far), {} oaks, {} scrub, {} fence posts, {} meshes",
            counts.cells,
            counts.conifers,
            counts.far_conifers,
            counts.oaks,
            counts.scrub,
            counts.posts,
            counts.meshes
        ));
        self.counts = counts;
        Ok(())
    }
}

/// The group's node, for a test that wants to find it.
pub fn group_of(graph: &SceneGraph, root: NodeId) -> Option<NodeId> {
    graph
        .get(root)
        .children
        .iter()
        .copied()
        .find(|&c| graph.get(c).name == "land")
}
