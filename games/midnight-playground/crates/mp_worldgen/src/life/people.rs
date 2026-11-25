//! People: fans along the catch fence at Seaside Raceway, who cheer as the
//! player's car goes by, and people in Seabright strolling the promenade
//! and the town's sidewalk or standing at the sea rail.
//!
//! People stay where cars can't go: behind the raceway's fence, on the
//! promenade and the sidewalk. People who might step into the road, and
//! always get out of the way, are a later step (vision WORLD 5).
//!
//! Every person is six instances of six meshes (two legs, a torso, a head,
//! two arms) at the same index. A fan near the player's car waves; a
//! walker near the camera walks. The rest keep their last pose.

use std::f64::consts::PI;

use mp_levels::survey::{Pt, SeasideData};
use mp_track::Track;
use pk_math::{Mulberry32, kernel, lerp, rrange, smoothstep};

use super::{X, Y, Z, dist2d, ground, heading, instance_matrix, rot, v3};
use crate::builder::Builder;
use crate::color::Color;
use crate::material::Material;
use crate::object::NodeId;
use crate::three_geom::{Matrix4, Vector3, icosahedron_geometry};
use crate::world::{Edit, UpdateCtx, World};

/// Walkers farther than this from the camera keep their last pose.
const WALK_NEAR: f64 = 260.0;
/// Fans this close along the track to the player's car react to it.
const FAN_NEAR: f64 = 170.0;

const SHIRTS: [u32; 14] = [
    0xd81e36, 0x1f4fd8, 0xf2b705, 0xf4f4f0, 0x1b1b1f, 0x2e8b57, 0xee5a12, 0x8a2be2, 0x00a7c4,
    0xe86fa6, 0x6b8e23, 0x9b2335, 0x3d5a80, 0xd9c8a9,
];
const PANTS: [u32; 6] = [0x2b3a55, 0x1d2433, 0x3b3b3f, 0x7a6a4f, 0x4a5563, 0x20262e];
const SKIN: [u32; 7] = [
    0x5c3a21, 0x8d5524, 0xa86b3c, 0xc68642, 0xe0ac69, 0xf1c27d, 0xffdbac,
];

#[derive(Clone, Copy, Debug)]
enum Kind {
    /// Watching the race from `s` along the track.
    Fan { s: f64 },
    /// Walking a path back and forth: where on it at time 0, and how fast.
    Walker { path: usize, p0: f64, speed: f64 },
    /// Standing (at the sea rail).
    Still,
}

#[derive(Clone, Copy, Debug)]
struct Person {
    kind: Kind,
    pos: [f64; 3],
    yaw: f64,
    scale: f64,
    phase: f64,
    /// Posed away from rest last frame (so it is put back once).
    moved: bool,
}

/// A path walkers follow: points every [`PATH_STEP`] m.
struct WalkPath {
    pts: Vec<[f64; 3]>,
}

const PATH_STEP: f64 = 2.0;

impl WalkPath {
    fn len(&self) -> f64 {
        (self.pts.len() - 1) as f64 * PATH_STEP
    }

    fn at(&self, d: f64) -> ([f64; 3], f64) {
        let k = (d / PATH_STEP).clamp(0.0, (self.pts.len() - 1) as f64);
        let i = (k as usize).min(self.pts.len() - 2);
        let f = k - i as f64;
        let (a, b) = (self.pts[i], self.pts[i + 1]);
        let p = [
            a[0] + (b[0] - a[0]) * f,
            a[1] + (b[1] - a[1]) * f,
            a[2] + (b[2] - a[2]) * f,
        ];
        (p, heading(b[0] - a[0], b[2] - a[2]))
    }
}

/// The level's people and their six instanced meshes.
pub struct People {
    people: Vec<Person>,
    paths: Vec<WalkPath>,
    /// leg L, leg R, torso, head, arm L, arm R.
    nodes: [NodeId; 6],
    length: f64,
}

/// A body pose: the bounce, the legs' swing and the arms' swing and raise.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Pose {
    bob: f64,
    leg: f64,
    arm_swing: f64,
    arm_raise: f64,
    /// Extra raise on one arm (waving).
    wave: f64,
}

const REST: Pose = Pose {
    bob: 0.0,
    leg: 0.0,
    arm_swing: 0.0,
    arm_raise: 0.1,
    wave: 0.0,
};

pub fn build(w: &mut World, level: &str) -> Result<Option<People>, String> {
    let Some(t) = w.track.as_ref() else {
        return Ok(None);
    };
    let mut rng = Mulberry32::new(0x00b0_d1e5 ^ (level.len() as u32 * 7919));
    let mut people = Vec::new();
    let mut paths = Vec::new();
    match level {
        "seaside" => {
            if let Some(d) = w.level_data.clone() {
                fans(w, t, &d, &mut rng, &mut people);
                grandstand_crowd(t, &d, &mut rng, &mut people);
                bridge_crews_and_marshals(w, t, &d, &mut rng, &mut people);
            }
        }
        "coast" => {
            seabright(w, t, &mut rng, &mut people, &mut paths);
            pier_and_beach(w, t, &mut rng, &mut people, &mut paths);
            harbor_workers(w, t, &mut rng, &mut people, &mut paths);
        }
        "sierra" => {
            summit_hikers(w, t, &mut rng, &mut people);
            city_sidewalks(w, t, &mut rng, &mut people, &mut paths);
        }
        "desert" => desert_oasis(w, t, &mut rng, &mut people, &mut paths),
        "streets" => downtown_pedestrians(w, t, &mut rng, &mut people, &mut paths),
        "cruise" => city_sidewalks(w, t, &mut rng, &mut people, &mut paths),
        _ => {
            seabright(w, t, &mut rng, &mut people, &mut paths);
            harbor_workers(w, t, &mut rng, &mut people, &mut paths);
            summit_hikers(w, t, &mut rng, &mut people);
            city_sidewalks(w, t, &mut rng, &mut people, &mut paths);
            desert_oasis(w, t, &mut rng, &mut people, &mut paths);
        }
    }
    if people.is_empty() {
        return Ok(None);
    }
    let length = t.length;

    // The meshes, a person 1.75 m tall facing +z. Legs and arms hang from
    // their hip and shoulder (the instance places the joint).
    let geo = |f: &dyn Fn(&mut Builder)| {
        let mut b = Builder::new();
        f(&mut b);
        b.merge_all()
    };
    let leg = geo(&|b| b.cbox("p", 0.15, 0.88, 0.17, 0.0, -0.44, 0.0, [0.0; 3])).ok_or("a leg")?;
    let torso = geo(&|b| {
        b.cbox("p", 0.36, 0.16, 0.21, 0.0, 0.9, 0.0, [0.0; 3]);
        b.cbox("p", 0.42, 0.52, 0.23, 0.0, 1.22, 0.0, [0.0; 3]);
    })
    .ok_or("a torso")?;
    let mut head = icosahedron_geometry(0.115, 1.0);
    head.translate(0.0, 1.62, 0.0);
    let neck = geo(&|b| b.cbox("p", 0.09, 0.1, 0.09, 0.0, 1.5, 0.0, [0.0; 3])).ok_or("a neck")?;
    let head = {
        let mut b = Builder::new();
        b.add("p", &head, None);
        b.add("p", &neck, None);
        b.merge_all().ok_or("a head")?
    };
    let arm = geo(&|b| b.cbox("p", 0.095, 0.62, 0.1, 0.0, -0.31, 0.0, [0.0; 3])).ok_or("an arm")?;
    let mat = w.graph.add_material(
        Material::standard()
            .set("color", 0xffffff)
            .set("roughness", 0.9),
    );
    let n = people.len() as u32;
    let mut node = |g| {
        let g = w.graph.add_geometry(g);
        let id = w.graph.instanced_mesh(g, mat, n);
        let o = w.graph.get_mut(id);
        o.frustum_culled = false;
        o.cast_shadow = false;
        o.receive_shadow = true;
        id
    };
    let nodes = [
        node(leg.clone()),
        node(leg),
        node(torso),
        node(head),
        node(arm.clone()),
        node(arm),
    ];
    // Colours: trousers, shirt, skin; the arms in the shirt.
    for (i, _) in people.iter().enumerate() {
        let shirt = Color::hex(SHIRTS[(rng.next_f64() * SHIRTS.len() as f64) as usize]);
        let pants = Color::hex(PANTS[(rng.next_f64() * PANTS.len() as f64) as usize]);
        let skin = Color::hex(SKIN[(rng.next_f64() * SKIN.len() as f64) as usize]);
        for (k, c) in [pants, pants, shirt, skin, shirt, shirt]
            .into_iter()
            .enumerate()
        {
            w.graph
                .get_mut(nodes[k])
                .instances
                .as_mut()
                .expect("instanced")
                .set_color_at(i, c);
        }
    }
    let out = People {
        people,
        paths,
        nodes,
        length,
    }
    .with_walkers_placed();
    // Everyone at rest where they start.
    for i in 0..out.people.len() {
        let p = out.people[i];
        let mats = person_matrices(p.pos, p.yaw, p.scale, &REST);
        for (k, m) in mats.iter().enumerate() {
            w.graph
                .get_mut(nodes[k])
                .instances
                .as_mut()
                .expect("instanced")
                .set_matrix_at(i, m);
        }
    }
    let group = w.graph.group("life:people");
    for n in nodes {
        w.graph.add(group, n);
    }
    let root = w.root;
    w.graph.add(root, group);
    Ok(Some(out))
}

/// Fans along the catch fence where the raceway's banners are (the
/// grandstand stretches and the famous corners), one to three deep,
/// clear of the pit lane, the buildings, the stands and the water, and of
/// any other stretch of the track.
fn fans(w: &World, t: &Track, d: &SeasideData, rng: &mut Mulberry32, out: &mut Vec<Person>) {
    // The banner zones of `raceway::build_banners`.
    const ZONES: [(f64, f64); 4] = [
        (-240.0, 160.0),
        (440.0, 640.0),
        (2440.0, 2720.0),
        (3200.0, 3420.0),
    ];
    let blocked = |x: f64, z: f64| {
        let polys = d
            .grandstands
            .iter()
            .chain(d.water.iter())
            .chain(d.buildings.iter().map(|b| &b.pts));
        polys.into_iter().any(|p| inside(x, z, p)) || near_line(x, z, &d.pit_lane, 7.0)
    };
    for (a, b) in ZONES {
        for side in [-1.0, 1.0] {
            let mut s = a;
            // Clusters: a few metres of fans, then a gap.
            while s < b {
                let run = rrange(rng, 6.0, 22.0);
                let rows = 1 + (rng.next_f64() * 3.0) as u32;
                let end = (s + run).min(b);
                let mut u = s;
                while u < end {
                    let sw = t.wrap(u);
                    let f = t.frame(sw);
                    let wall = if side < 0.0 { f.wall_l } else { f.wall_r };
                    for row in 0..rows {
                        if rng.next_f64() < 0.18 {
                            continue;
                        }
                        let lat = side
                            * (wall + 1.1 + 1.7 + f64::from(row) * 0.85 + rrange(rng, -0.2, 0.2));
                        let p = t.point_at(sw, lat);
                        let y = ground(w, p.x, p.z, f.y);
                        // Allow spectators to stand on rising grassy banks up
                        // to 7.5 m above the wall so they are visible above the
                        // 2.2 m sponsor boards (`living-world.md` 2.0), while
                        // avoiding ditches below the wall.
                        let wall_y = t.surface_y(sw, side * wall);
                        let dy = y - wall_y;
                        if dy < -1.2 || dy > 7.5 || blocked(p.x, p.z) {
                            continue;
                        }
                        // Behind the 2.2 m banner boards (`ZONES`), skip ground
                        // so low that only the top of a fan's head peeks over,
                        // placing fans slightly higher up the bank or along the
                        // unboarded fence stretches below.
                        if dy < 0.75 && row == 0 {
                            continue;
                        }
                        // Not near another stretch of the track.
                        let rd = t.distance_to_road(p.x, p.z, 60.0);
                        if let Some(rs) = rd.s {
                            let ds = (rs - sw).rem_euclid(t.length);
                            if ds.min(t.length - ds) > 30.0 && rd.d < 14.0 {
                                continue;
                            }
                        }
                        // Facing the track.
                        out.push(Person {
                            kind: Kind::Fan { s: sw },
                            pos: [p.x, y, p.z],
                            yaw: heading(f.x - p.x, f.z - p.z) + rrange(rng, -0.3, 0.3),
                            scale: rrange(rng, 0.9, 1.08),
                            phase: rng.next_f64() * PI * 2.0,
                            moved: false,
                        });
                    }
                    u += rrange(rng, 0.9, 1.5);
                }
                s = end + rrange(rng, 3.0, 14.0);
            }
        }
    }
    // Fans along unbannered catch-fence stretches where the whole figure shows
    // cleanly over the 1.05 m concrete wall.
    const OPEN_FENCE: [(f64, f64); 3] = [(180.0, 420.0), (880.0, 1310.0), (2750.0, 3160.0)];
    for (a, b) in OPEN_FENCE {
        for side in [-1.0, 1.0] {
            let mut s = a;
            while s < b {
                let sw = t.wrap(s);
                let f = t.frame(sw);
                let wall = if side < 0.0 { f.wall_l } else { f.wall_r };
                let lat = side * (wall + rrange(rng, 2.4, 4.2));
                let p = t.point_at(sw, lat);
                let y = ground(w, p.x, p.z, f.y);
                let wall_y = t.surface_y(sw, side * wall);
                if y >= wall_y - 0.8 && y <= wall_y + 6.5 && !blocked(p.x, p.z) {
                    let rd = t.distance_to_road(p.x, p.z, 60.0);
                    let clear = rd.s.is_none_or(|rs| {
                        let ds = (rs - sw).rem_euclid(t.length);
                        ds.min(t.length - ds) <= 30.0 || rd.d >= 14.0
                    });
                    if clear {
                        out.push(Person {
                            kind: Kind::Fan { s: sw },
                            pos: [p.x, y, p.z],
                            yaw: heading(f.x - p.x, f.z - p.z) + rrange(rng, -0.25, 0.25),
                            scale: rrange(rng, 0.9, 1.08),
                            phase: rng.next_f64() * PI * 2.0,
                            moved: false,
                        });
                    }
                }
                s += rrange(rng, 2.2, 7.5);
            }
        }
    }
}

/// Spectators standing and cheering on the tiers of Seaside Raceway's
/// grandstands (`raceway::build_grandstands`).
fn grandstand_crowd(t: &Track, d: &SeasideData, rng: &mut Mulberry32, out: &mut Vec<Person>) {
    for poly in &d.grandstands {
        if poly.len() < 4 {
            continue;
        }
        let n = poly.len() - 1;
        // Find the longest edge and its perpendicular to orient tiers toward the track.
        let mut best = (0.0, 0usize);
        for i in 0..n {
            let (a, b) = (&poly[i], &poly[(i + 1) % n]);
            let l2 = (b.x - a.x) * (b.x - a.x) + (b.z - a.z) * (b.z - a.z);
            if l2 > best.0 {
                best = (l2, i);
            }
        }
        if best.0 < 36.0 {
            continue;
        }
        let (ea, eb) = (&poly[best.1], &poly[(best.1 + 1) % n]);
        let el = best.0.sqrt();
        let (ux, uz) = ((eb.x - ea.x) / el, (eb.z - ea.z) / el);
        let (vx, vz) = (-uz, ux);
        let (mut u0, mut u1, mut v0, mut v1) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for p in &poly[..n] {
            let pu = p.x * ux + p.z * uz;
            let pv = p.x * vx + p.z * vz;
            u0 = u0.min(pu);
            u1 = u1.max(pu);
            v0 = v0.min(pv);
            v1 = v1.max(pv);
        }
        let hu = (u1 - u0) * 0.5;
        let hv = (v1 - v0) * 0.5;
        if hu < 4.0 || hv < 1.5 {
            continue;
        }
        let cu = (u0 + u1) * 0.5;
        let cv = (v0 + v1) * 0.5;
        let cx = ux * cu + vx * cv;
        let cz = uz * cu + vz * cv;
        let d1 = t.distance_to_road(cx + vx * hv, cz + vz * hv, 200.0).d;
        let d2 = t.distance_to_road(cx - vx * hv, cz - vz * hv, 200.0).d;
        let (bx, bz) = if d1 < d2 { (-vx, -vz) } else { (vx, vz) };
        let fx = cx - bx * hv;
        let fz = cz - bz * hv;
        let near = t.distance_to_road(fx, fz, 200.0);
        let Some(s_stand) = near.s else { continue };
        if (s_stand - 1500.0).abs() <= FAN_NEAR + 20.0 {
            continue;
        }
        let base_y = t.frame(s_stand).y + 0.85;
        let depth = hv * 2.0;
        let rows = ((depth / 0.85) as usize).clamp(3, 10);
        for row in 0..rows {
            let b = 0.55 + row as f64 * 0.82;
            if b >= depth - 0.3 {
                break;
            }
            let y = base_y + row as f64 * 0.42;
            let mut a = -hu + 0.8;
            while a <= hu - 0.8 {
                if rng.next_f64() < 0.42 {
                    let px = fx + ux * a + bx * b;
                    let pz = fz + uz * a + bz * b;
                    let d_road = t.distance_to_road(px, pz, 80.0);
                    let hw = f64::from(t.hw.get(d_road.i.max(0) as usize).copied().unwrap_or(6.0));
                    if d_road.d > hw + 3.0 {
                        out.push(Person {
                            kind: Kind::Fan { s: s_stand },
                            pos: [px, y, pz],
                            yaw: heading(-bx, -bz) + rrange(rng, -0.2, 0.2),
                            scale: rrange(rng, 0.9, 1.06),
                            phase: rng.next_f64() * PI * 2.0,
                            moved: false,
                        });
                    }
                }
                a += rrange(rng, 1.1, 1.8);
            }
        }
    }
}

/// Camera operators on Seaside's pedestrian bridges and corner marshals
/// behind the barriers at key turns (`living-world.md` 2.2).
fn bridge_crews_and_marshals(
    w: &World,
    t: &Track,
    d: &SeasideData,
    rng: &mut Mulberry32,
    out: &mut Vec<Person>,
) {
    for br in &d.bridges {
        for seg in br.pts.windows(2) {
            let mx = (seg[0].x + seg[1].x) * 0.5;
            let mz = (seg[0].z + seg[1].z) * 0.5;
            let near = t.distance_to_road(mx, mz, 25.0);
            let Some(s) = near.s else { continue };
            let f = t.frame(s);
            let wall = f.wall_l.max(f.wall_r);
            if near.d > wall + 2.0 {
                continue;
            }
            let deck_y = f.y + 5.9;
            for side in [-1.0, 1.0] {
                let lat = side * (f.hw + 2.2);
                let p = t.point_at(s, lat);
                let check = t.distance_to_road(p.x, p.z, 30.0);
                if check.d > f.hw + 1.5 {
                    out.push(Person {
                        kind: Kind::Still,
                        pos: [p.x, deck_y, p.z],
                        yaw: heading(-f.fx, -f.fz) + rrange(rng, -0.18, 0.18),
                        scale: rrange(rng, 0.94, 1.05),
                        phase: rng.next_f64() * PI * 2.0,
                        moved: false,
                    });
                }
            }
            break;
        }
    }
    // Corner marshals stationed well behind the barriers around the lap.
    let mut s = 260.0;
    while s < t.length - 200.0 {
        if (s - 1500.0).abs() > FAN_NEAR + 30.0 {
            let f = t.frame(s);
            let side = if rng.next_f64() < 0.5 { -1.0 } else { 1.0 };
            let wall = if side < 0.0 { f.wall_l } else { f.wall_r };
            let p = t.point_at(s, side * (wall + 2.6));
            let check = t.distance_to_road(p.x, p.z, 40.0);
            if check.d > f.hw + 2.0 {
                let y = ground(w, p.x, p.z, f.y);
                out.push(Person {
                    kind: Kind::Fan { s },
                    pos: [p.x, y, p.z],
                    yaw: heading(f.x - p.x, f.z - p.z),
                    scale: 1.02,
                    phase: rng.next_f64() * PI * 2.0,
                    moved: false,
                });
            }
        }
        s += 420.0;
    }
}

/// Seabright's wooden pier walkers/anglers and beachgoers on the sand.
fn pier_and_beach(
    w: &World,
    t: &Track,
    rng: &mut Mulberry32,
    out: &mut Vec<Person>,
    paths: &mut Vec<WalkPath>,
) {
    let Some(z) = w.level.zones.iter().position(|z| z.scenery == "Beach") else {
        return;
    };
    let zs0 = t.zone_start[z] as f64;
    let zs1 = if z + 1 < t.zone_start.len() {
        t.zone_start[z + 1] as f64
    } else {
        t.length
    };
    let pier_s = t
        .tags
        .iter()
        .find(|g| g.tag == "pier" && g.s0 >= zs0 - 5.0 && g.s0 < zs1)
        .map_or(zs0 + 700.0, |g| (g.s0 + g.s1) * 0.5);
    let pf = t.frame(pier_s);
    let deck_y = t.surface_y(pier_s, 0.0) + 0.25;
    // Two stroll lanes out along the wooden pier deck (`beach::build_pier`).
    for ds in [-2.6, 2.6] {
        let mut pts = Vec::new();
        let mut out_d = pf.wall_l + 8.0;
        while out_d <= pf.wall_l + 190.0 {
            let p = t.point_at(pier_s + ds, -out_d);
            pts.push([p.x, deck_y, p.z]);
            out_d += PATH_STEP;
        }
        if pts.len() >= 4 {
            let idx = paths.len();
            let len = (pts.len() - 1) as f64 * PATH_STEP;
            paths.push(WalkPath { pts });
            for _ in 0..8 {
                let speed = rrange(rng, 0.9, 1.35) * if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
                out.push(Person {
                    kind: Kind::Walker {
                        path: idx,
                        p0: rng.next_f64() * 2.0 * len,
                        speed,
                    },
                    pos: [0.0; 3],
                    yaw: 0.0,
                    scale: rrange(rng, 0.9, 1.06),
                    phase: rng.next_f64() * PI * 2.0,
                    moved: false,
                });
            }
        }
    }
    // Beachgoers on dry sand seaward of the promenade.
    let mut s = zs0 + 260.0;
    while s < zs1 - 160.0 {
        s += rrange(rng, 18.0, 44.0);
        if (s - pier_s).abs() < 34.0 {
            continue;
        }
        let f = t.frame(s);
        let lat = -(f.wall_l + rrange(rng, 15.0, 34.0));
        let p = t.point_at(s, lat);
        let y = ground(w, p.x, p.z, 1.2);
        if y > 0.45 && safe_off_road(t, p.x, p.z) {
            out.push(Person {
                kind: Kind::Still,
                pos: [p.x, y, p.z],
                yaw: heading(-f.rx, -f.rz) + rrange(rng, -0.7, 0.7),
                scale: rrange(rng, 0.88, 1.06),
                phase: rng.next_f64() * PI * 2.0,
                moved: false,
            });
        }
    }
}

/// Dock workers on the container apron in Harbor zones (`living-world.md` 2.3).
fn harbor_workers(
    w: &World,
    t: &Track,
    rng: &mut Mulberry32,
    out: &mut Vec<Person>,
    paths: &mut Vec<WalkPath>,
) {
    let Some(z) = w.level.zones.iter().position(|z| z.scenery == "Harbor") else {
        return;
    };
    let zs0 = t.zone_start[z] as f64;
    let zs1 = if z + 1 < t.zone_start.len() {
        t.zone_start[z + 1] as f64
    } else {
        t.length
    };
    let s0 = t
        .tags
        .iter()
        .find(|g| (g.tag == "port" || g.tag == "containers") && g.s0 >= zs0 - 5.0 && g.s0 < zs1)
        .map_or(zs0 + 500.0, |g| g.s0 + 40.0);
    let s1 = (zs1 - 120.0).max(s0);
    if s1 - s0 < 120.0 {
        return;
    }
    let mut pts = Vec::new();
    let mut s = s0;
    while s <= s1 {
        let f = t.frame(s);
        let p = t.point_at(s, f.wall_r + 11.0);
        if safe_off_road(t, p.x, p.z) {
            pts.push([p.x, ground(w, p.x, p.z, f.y), p.z]);
        }
        s += PATH_STEP;
    }
    if pts.len() >= 10 {
        let idx = paths.len();
        let len = (pts.len() - 1) as f64 * PATH_STEP;
        paths.push(WalkPath { pts });
        for _ in 0..10 {
            let speed = rrange(rng, 1.0, 1.45) * if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
            out.push(Person {
                kind: Kind::Walker {
                    path: idx,
                    p0: rng.next_f64() * 2.0 * len,
                    speed,
                },
                pos: [0.0; 3],
                yaw: 0.0,
                scale: rrange(rng, 0.94, 1.08),
                phase: rng.next_f64() * PI * 2.0,
                moved: false,
            });
        }
    }
}

/// Hikers admiring the view at mountain summit pullouts (`living-world.md` 2.3).
fn summit_hikers(w: &World, t: &Track, rng: &mut Mulberry32, out: &mut Vec<Person>) {
    let mut spots: Vec<f64> = t
        .tags
        .iter()
        .filter(|g| g.tag == "summit" || g.tag == "crest")
        .map(|g| (g.s0 + g.s1) * 0.5)
        .collect();
    if spots.is_empty()
        && let Some(z) = w.level.zones.iter().position(|z| z.scenery == "Mountain")
    {
        spots.push(t.zone_start[z] as f64 + 650.0);
    }
    for s_mid in spots {
        for k in 0..8 {
            let s = s_mid - 16.0 + f64::from(k) * 4.5;
            let f = t.frame(t.wrap(s));
            let p = t.point_at(t.wrap(s), -(f.wall_l + rrange(rng, 4.2, 7.5)));
            if safe_off_road(t, p.x, p.z) {
                let y = ground(w, p.x, p.z, f.y);
                out.push(Person {
                    kind: Kind::Fan { s: t.wrap(s) },
                    pos: [p.x, y, p.z],
                    yaw: heading(-f.rx, -f.rz) + rrange(rng, -0.35, 0.35),
                    scale: rrange(rng, 0.9, 1.06),
                    phase: rng.next_f64() * PI * 2.0,
                    moved: false,
                });
            }
        }
    }
}

/// Patrons at the Oasis diner, gas station and motel forecourt on Desert Run.
fn desert_oasis(
    w: &World,
    t: &Track,
    rng: &mut Mulberry32,
    out: &mut Vec<Person>,
    paths: &mut Vec<WalkPath>,
) {
    let oasis_s = t
        .tags
        .iter()
        .find(|g| g.tag == "oasis")
        .map(|g| (g.s0 + g.s1) * 0.5)
        .or_else(|| {
            w.level
                .zones
                .iter()
                .position(|z| z.scenery == "Desert")
                .map(|z| t.zone_start[z] as f64 + 900.0)
        });
    let Some(s0) = oasis_s else { return };
    let f0 = t.frame(t.wrap(s0));
    let lot_y = f0.y - 0.17;
    let mut pts = Vec::new();
    let mut ds = -54.0;
    while ds <= 58.0 {
        let s = t.wrap(s0 + ds);
        let f = t.frame(s);
        let p = t.point_at(s, f.wall_r + 12.5);
        if safe_off_road(t, p.x, p.z) {
            pts.push([p.x, ground(w, p.x, p.z, lot_y), p.z]);
        }
        ds += PATH_STEP;
    }
    if pts.len() >= 8 {
        let idx = paths.len();
        let len = (pts.len() - 1) as f64 * PATH_STEP;
        paths.push(WalkPath { pts });
        for _ in 0..8 {
            let speed = rrange(rng, 0.85, 1.25) * if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
            out.push(Person {
                kind: Kind::Walker {
                    path: idx,
                    p0: rng.next_f64() * 2.0 * len,
                    speed,
                },
                pos: [0.0; 3],
                yaw: 0.0,
                scale: rrange(rng, 0.9, 1.06),
                phase: rng.next_f64() * PI * 2.0,
                moved: false,
            });
        }
    }
    for ds in [-42.0, -8.0, 2.0, 8.0, 44.0, 50.0] {
        let s = t.wrap(s0 + ds);
        let f = t.frame(s);
        let p = t.point_at(s, f.wall_r + 15.5);
        if safe_off_road(t, p.x, p.z) {
            out.push(Person {
                kind: Kind::Still,
                pos: [p.x, ground(w, p.x, p.z, lot_y), p.z],
                yaw: heading(-f.rx, -f.rz) + rrange(rng, -0.5, 0.5),
                scale: rrange(rng, 0.9, 1.06),
                phase: rng.next_f64() * PI * 2.0,
                moved: false,
            });
        }
    }
}

/// Nightlife pedestrians along Downtown Streets' sidewalks, kept well clear of
/// grid intersections.
fn downtown_pedestrians(
    w: &World,
    t: &Track,
    rng: &mut Mulberry32,
    out: &mut Vec<Person>,
    paths: &mut Vec<WalkPath>,
) {
    for side in [-1.0, 1.0] {
        let mut s = 180.0;
        while s < t.finish_s - 200.0 {
            let run_end = (s + 96.0).min(t.finish_s - 200.0);
            let mut pts = Vec::new();
            let mut u = s;
            while u <= run_end {
                let f = t.frame(u);
                // Stay on straight avenues away from tight 90° turns.
                if f.kappa.abs() > 0.004 {
                    break;
                }
                let lat = side * (f.wall_r.max(f.wall_l) + 2.4);
                let p = t.point_at(u, lat);
                if !safe_off_road(t, p.x, p.z) {
                    break;
                }
                pts.push([p.x, ground(w, p.x, p.z, f.y) + 0.15, p.z]);
                u += PATH_STEP;
            }
            if pts.len() >= 14 {
                let idx = paths.len();
                let len = (pts.len() - 1) as f64 * PATH_STEP;
                paths.push(WalkPath { pts });
                for _ in 0..3 {
                    let speed =
                        rrange(rng, 0.95, 1.4) * if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
                    out.push(Person {
                        kind: Kind::Walker {
                            path: idx,
                            p0: rng.next_f64() * 2.0 * len,
                            speed,
                        },
                        pos: [0.0; 3],
                        yaw: 0.0,
                        scale: rrange(rng, 0.88, 1.06),
                        phase: rng.next_f64() * PI * 2.0,
                        moved: false,
                    });
                }
            }
            s += 260.0;
        }
    }
}

/// Sidewalk strollers along ground-level City zones (`sierra`, `cruise`).
fn city_sidewalks(
    w: &World,
    t: &Track,
    rng: &mut Mulberry32,
    out: &mut Vec<Person>,
    paths: &mut Vec<WalkPath>,
) {
    let Some(z) = w.level.zones.iter().position(|z| z.scenery == "City") else {
        return;
    };
    let zs0 = t.zone_start[z] as f64 + 120.0;
    let zs1 = if z + 1 < t.zone_start.len() {
        t.zone_start[z + 1] as f64 - 120.0
    } else {
        t.length - 220.0
    };
    let city_y = w.terrain.as_ref().map_or(0.0, |tr| tr.city_y);
    let mut s = zs0;
    while s < zs1 {
        let f = t.frame(t.wrap(s));
        // Only place walkers where the roadway runs near street level (not on elevated viaducts).
        if (f.y - city_y).abs() < 2.2 {
            let mut pts = Vec::new();
            let mut u = s;
            while u <= (s + 110.0).min(zs1) {
                let fu = t.frame(t.wrap(u));
                if (fu.y - city_y).abs() > 2.4 {
                    break;
                }
                let p = t.point_at(t.wrap(u), fu.wall_r + 3.2);
                if !safe_off_road(t, p.x, p.z) {
                    break;
                }
                pts.push([p.x, city_y + 0.16, p.z]);
                u += PATH_STEP;
            }
            if pts.len() >= 14 {
                let idx = paths.len();
                let len = (pts.len() - 1) as f64 * PATH_STEP;
                paths.push(WalkPath { pts });
                for _ in 0..3 {
                    let speed =
                        rrange(rng, 0.95, 1.35) * if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
                    out.push(Person {
                        kind: Kind::Walker {
                            path: idx,
                            p0: rng.next_f64() * 2.0 * len,
                            speed,
                        },
                        pos: [0.0; 3],
                        yaw: 0.0,
                        scale: rrange(rng, 0.88, 1.06),
                        phase: rng.next_f64() * PI * 2.0,
                        moved: false,
                    });
                }
            }
        }
        s += 320.0;
    }
}

fn safe_off_road(t: &Track, x: f64, z: f64) -> bool {
    let d = t.distance_to_road(x, z, 60.0);
    let i = d.i.max(0) as usize;
    let hw = f64::from(t.hw.get(i).copied().unwrap_or(6.0));
    d.d > hw + 2.2
}

/// Seabright: walkers on the promenade (the sea side) and the town's
/// sidewalk, and people at the promenade's sea rail. The ranges and
/// lateral places are `beach`'s (`setup_ranges`, `build_promenade`,
/// `build_sidewalk_props`).
fn seabright(
    w: &World,
    t: &Track,
    rng: &mut Mulberry32,
    out: &mut Vec<Person>,
    paths: &mut Vec<WalkPath>,
) {
    let Some(z) = w.level.zones.iter().position(|z| z.scenery == "Beach") else {
        return;
    };
    let zs0 = t.zone_start[z] as f64;
    let zs1 = if z + 1 < t.zone_start.len() {
        t.zone_start[z + 1] as f64
    } else {
        t.length
    };
    let tag = |n: &str| {
        t.tags
            .iter()
            .find(|g| g.tag == n && g.s0 >= zs0 - 5.0 && g.s0 < zs1)
            .map(|g| (g.s0, g.s1))
    };
    let pier_s = tag("pier").map_or(zs0 + 700.0, |g| ((g.0 + g.1) / 2.0).round());
    let town_s0 = tag("promenade").map_or(zs0 + 420.0, |g| g.0) + 50.0;
    let town_s1 = zs1 - 70.0;
    if town_s1 - town_s0 < 100.0 {
        return;
    }
    let pf = t.frame(pier_s);
    // Promenade: from wall_l + 0.45 to + 6.8 on the sea side, 0.22 up.
    let lanes: [(f64, f64); 3] = [
        (-(pf.wall_l + 2.2), 0.22),
        (-(pf.wall_l + 4.9), 0.22),
        // The town's sidewalk, past the hydrants and benches.
        (pf.wall_r + 2.5, 0.14),
    ];
    for (lat, lift) in lanes {
        let mut pts = Vec::new();
        let mut s = town_s0 - 20.0;
        while s < town_s1 + 20.0 {
            let p = t.point_at(s, lat);
            pts.push([p.x, ground(w, p.x, p.z, p.y) + lift, p.z]);
            s += PATH_STEP;
        }
        paths.push(WalkPath { pts });
    }
    for (k, path) in paths.iter().enumerate() {
        let len = path.len();
        let n = (len / if k == 2 { 26.0 } else { 18.0 }) as usize;
        for _ in 0..n {
            let speed = rrange(rng, 1.0, 1.5) * if rng.next_f64() < 0.5 { 1.0 } else { -1.0 };
            out.push(Person {
                kind: Kind::Walker {
                    path: k,
                    p0: rng.next_f64() * 2.0 * len,
                    speed,
                },
                pos: [0.0; 3],
                yaw: 0.0,
                scale: rrange(rng, 0.88, 1.06),
                phase: rng.next_f64() * PI * 2.0,
                moved: false,
            });
        }
    }
    // At the sea rail, looking out, in ones and twos.
    let mut s = town_s0;
    while s < town_s1 {
        s += rrange(rng, 25.0, 70.0);
        if (s - pier_s).abs() < 12.0 {
            continue;
        }
        let pair = rng.next_f64() < 0.45;
        for j in 0..if pair { 2 } else { 1 } {
            let lat = -(pf.wall_l + 6.25);
            let p = t.point_at(s + f64::from(j) * 0.7, lat);
            let f = t.frame(s);
            out.push(Person {
                kind: Kind::Still,
                pos: [p.x, ground(w, p.x, p.z, p.y) + 0.22, p.z],
                // Facing the sea: away from the road, to the left.
                yaw: heading(-f.rx, -f.rz) + rrange(rng, -0.4, 0.4),
                scale: rrange(rng, 0.88, 1.06),
                phase: rng.next_f64() * PI * 2.0,
                moved: false,
            });
        }
    }
}

impl People {
    /// Walkers' start positions (their pose at time 0).
    fn with_walkers_placed(mut self) -> People {
        for p in &mut self.people {
            if let Kind::Walker { path, p0, speed } = p.kind {
                let (pos, yaw) = walker_at(&self.paths[path], p0, speed, 0.0);
                p.pos = pos;
                p.yaw = yaw;
            }
        }
        self
    }

    pub fn update(
        &mut self,
        time: f64,
        u: &UpdateCtx,
        cam: Option<[f64; 3]>,
        last_s: Option<f64>,
        out: &mut Vec<Edit>,
    ) {
        let len = self.length;
        // How fast the player is going (fans cheer a fast car more).
        let speed = last_s.map_or(0.0, |ls| {
            let d = (u.s - ls).rem_euclid(len);
            d.min(len - d) / u.dt.max(1e-3)
        });
        for i in 0..self.people.len() {
            let p = self.people[i];
            match p.kind {
                Kind::Fan { s } => {
                    let d = (s - u.s).rem_euclid(len);
                    let d = d.min(len - d);
                    if d > FAN_NEAR {
                        if p.moved {
                            self.write(i, p.pos, p.yaw, p.scale, &REST, out);
                            self.people[i].moved = false;
                        }
                        continue;
                    }
                    let e = smoothstep(FAN_NEAR, 25.0, d) * smoothstep(3.0, 25.0, speed).max(0.35);
                    let ph = p.phase;
                    let pose = Pose {
                        bob: if e > 0.6 {
                            0.07 * e * kernel::sin(7.0 * time + ph).abs()
                        } else {
                            0.0
                        },
                        leg: 0.0,
                        arm_swing: 0.0,
                        arm_raise: lerp(0.1, 2.5, e),
                        wave: e * 0.4 * kernel::sin(9.0 * time + ph),
                    };
                    self.write(i, p.pos, p.yaw, p.scale, &pose, out);
                    self.people[i].moved = true;
                }
                Kind::Walker { path, p0, speed } => {
                    let near = cam.is_none_or(|c| dist2d(c, p.pos) < WALK_NEAR);
                    if !near {
                        if p.moved {
                            self.write(i, p.pos, p.yaw, p.scale, &REST, out);
                            self.people[i].moved = false;
                        }
                        continue;
                    }
                    let next_p0 = p0 + speed * u.dt;
                    if let Kind::Walker { p0: stored_p0, .. } = &mut self.people[i].kind {
                        *stored_p0 = next_p0;
                    }
                    let (pos, yaw) = walker_at(&self.paths[path], next_p0, speed, 0.0);
                    let stride = (next_p0 * speed.signum()) / 0.72 * PI;
                    let sw = kernel::sin(stride + p.phase);
                    let pose = Pose {
                        bob: 0.03 * sw.abs(),
                        leg: 0.42 * sw,
                        arm_swing: 0.32 * sw,
                        arm_raise: 0.08,
                        wave: 0.0,
                    };
                    self.people[i].pos = pos;
                    self.people[i].yaw = yaw;
                    self.people[i].moved = true;
                    self.write(i, pos, yaw, p.scale, &pose, out);
                }
                Kind::Still => {}
            }
        }
    }

    fn write(
        &self,
        i: usize,
        pos: [f64; 3],
        yaw: f64,
        scale: f64,
        pose: &Pose,
        out: &mut Vec<Edit>,
    ) {
        for (k, m) in person_matrices(pos, yaw, scale, pose).iter().enumerate() {
            out.push(instance_matrix(self.nodes[k], i, m));
        }
    }
}

/// Where a walker is at time `t`: back and forth along the path.
fn walker_at(path: &WalkPath, p0: f64, speed: f64, t: f64) -> ([f64; 3], f64) {
    let len = path.len();
    let q = (p0 + speed * t).rem_euclid(2.0 * len);
    let (d, back) = if q < len {
        (q, false)
    } else {
        (2.0 * len - q, true)
    };
    let (pos, yaw) = path.at(d);
    (pos, if back { yaw + PI } else { yaw })
}

/// The six matrices: leg L, leg R, torso, head, arm L, arm R.
fn person_matrices(pos: [f64; 3], yaw: f64, scale: f64, pose: &Pose) -> [Matrix4; 6] {
    let base = Matrix4::compose(
        v3(pos[0], pos[1] + pose.bob, pos[2]),
        rot(Y, yaw),
        Vector3::splat(scale),
    );
    let joint =
        |x: f64, y: f64, q| base.multiply(&Matrix4::compose(v3(x, y, 0.0), q, Vector3::splat(1.0)));
    let leg_l = joint(-0.09, 0.88, rot(X, pose.leg));
    let leg_r = joint(0.09, 0.88, rot(X, -pose.leg));
    let arm_l = joint(
        -0.25,
        1.44,
        rot(Z, -pose.arm_raise).multiply(rot(X, -pose.arm_swing)),
    );
    let arm_r = joint(
        0.25,
        1.44,
        rot(Z, pose.arm_raise + pose.wave).multiply(rot(X, pose.arm_swing)),
    );
    [leg_l, leg_r, base, base, arm_l, arm_r]
}

/// Point in polygon (even-odd), for the survey's outlines.
fn inside(x: f64, z: f64, poly: &[Pt]) -> bool {
    let mut c = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (&poly[i], &poly[j]);
        if (a.z > z) != (b.z > z) && x < (b.x - a.x) * (z - a.z) / (b.z - a.z) + a.x {
            c = !c;
        }
        j = i;
    }
    c
}

/// Whether (x, z) is within `r` of a polyline.
fn near_line(x: f64, z: f64, line: &[Pt], r: f64) -> bool {
    line.windows(2).any(|s| {
        let (a, b) = (&s[0], &s[1]);
        let (dx, dz) = (b.x - a.x, b.z - a.z);
        let l2 = dx * dx + dz * dz;
        let k = if l2 > 0.0 {
            (((x - a.x) * dx + (z - a.z) * dz) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (px, pz) = (a.x + dx * k - x, a.z + dz * k - z);
        px * px + pz * pz < r * r
    })
}
