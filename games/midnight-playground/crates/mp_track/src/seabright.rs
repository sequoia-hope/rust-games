//! A beach town on an open level (docs/vision/peninsula.md, stage 7d): the
//! Beach kit's Seabright as the open world has it (Half Moon Bay on the
//! peninsula), its storefronts, houses and palms laid out here from the
//! road alone, so the simulation stops a car against every one the kit
//! draws ([`crate::props`]).
//!
//! The layout is the kit's own (`mp_worldgen::beach`: `setupRanges`,
//! `reserveStreets`, the frontage, the houses behind main street, the
//! houses on the hill and the palms), on its own random streams: on a
//! closed level the kit draws one stream through layout and looks alike,
//! which only it can follow; here the layout has its streams and the kit
//! keeps its own for the looks.

use pk_math::{Mulberry32, js, kernel, rrange};

use crate::props::{Shape, Solid};
use crate::{Frame, Track};

/// What a storefront's sign says (the kit's sign atlas cells).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Icecream,
    Bikes,
    Cafe,
    Records,
    Swim,
    Pizza,
    Pharmacy,
    Supply,
    Bait,
}

/// A frontage lot along the boulevard (the kit's `Lot`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LotKind {
    Shop {
        sign: Sign,
        w: f64,
    },
    Surf,
    Taco,
    Diner,
    Motel,
    Gas,
    Hotel,
    Parking,
    /// A house on a lot `w` wide; the house itself is `house` (w, d).
    House {
        w: f64,
        house: [f64; 2],
    },
}

impl LotKind {
    pub fn w(&self) -> f64 {
        match *self {
            LotKind::Shop { w, .. } => w,
            LotKind::Surf => 16.0,
            LotKind::Taco => 12.0,
            LotKind::Diner => 26.0,
            LotKind::Motel => 46.0,
            LotKind::Gas => 38.0,
            LotKind::Hotel => 40.0,
            LotKind::Parking => 26.0,
            LotKind::House { w, .. } => w,
        }
    }

    pub fn d(&self) -> f64 {
        match *self {
            LotKind::Shop { .. } => 16.0,
            LotKind::Surf => 15.0,
            LotKind::Taco => 8.0,
            LotKind::Diner => 26.0,
            LotKind::Motel => 34.0,
            LotKind::Gas => 30.0,
            LotKind::Hotel => 18.0,
            LotKind::Parking => 24.0,
            LotKind::House { .. } => 16.0,
        }
    }

    /// What stands on the lot, in its frame (x along the road, z toward
    /// it, the lot's front at 0): (centre x, centre z, w, d, height) of
    /// each block, as the kit's parts build them.
    pub fn blocks(&self) -> Vec<[f64; 5]> {
        match *self {
            LotKind::Shop { w, .. } => vec![[0.0, -7.0, w - 1.0, 14.0, 4.2]],
            LotKind::Surf => vec![[0.0, -6.5, 14.0, 13.0, 4.2]],
            LotKind::Taco => vec![[0.0, -4.5, 7.0, 5.0, 3.2]],
            // The diner's block and its rounded end, pushed back 8 m.
            LotKind::Diner => vec![[0.0, -13.0, 18.0, 10.0, 4.4]],
            // The rooms along the back, the office at the front corner.
            LotKind::Motel => vec![[0.0, -26.0, 42.0, 10.0, 6.2], [-16.0, -6.0, 9.0, 8.0, 4.0]],
            // The shop at the back, the canopy's four columns.
            LotKind::Gas => {
                let mut v = vec![[0.0, -20.0, 12.0, 8.0, 4.0]];
                for x in [-8.5, 8.5] {
                    for z in [-12.5, -3.5] {
                        v.push([x, z, 0.5, 0.5, 5.4]);
                    }
                }
                v
            }
            LotKind::Hotel => vec![[0.0, -8.0, 36.0, 16.0, 19.2]],
            LotKind::Parking => Vec::new(),
            LotKind::House { house, .. } => vec![[0.0, -house[1] / 2.0, house[0], house[1], 6.2]],
        }
    }
}

/// A place on the ground facing a way: the kit's `Fr` without its height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub x: f64,
    pub z: f64,
    pub yaw: f64,
}

impl Place {
    /// A point in this frame, in the world (the kit's `toWorld`).
    pub fn to_world(&self, lx: f64, lz: f64) -> (f64, f64) {
        let (c, s) = (kernel::cos(self.yaw), kernel::sin(self.yaw));
        (self.x + lx * c + lz * s, self.z - lx * s + lz * c)
    }

    /// A block in this frame as a solid on ground `y`.
    pub fn block(&self, b: [f64; 5], y: f64) -> Solid {
        let (x, z) = self.to_world(b[0], b[1]);
        let (c, s) = (kernel::cos(self.yaw), kernel::sin(self.yaw));
        Solid {
            x,
            z,
            y,
            height: b[4],
            shape: Shape::Box {
                hx: b[2] / 2.0,
                hz: b[3] / 2.0,
                ux: c,
                uz: -s,
            },
        }
    }
}

/// A frontage lot: what it is, where along the road, and its frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lot {
    pub kind: LotKind,
    pub s: f64,
    pub at: Place,
}

/// A house (behind main street or on the hill): its frame (its front at
/// the origin facing +z, its body back from it), size and floors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct House {
    pub at: Place,
    pub w: f64,
    pub d: f64,
    pub floors: f64,
    /// A driveway car out front (the kit draws it).
    pub car: bool,
    pub seed: u32,
}

/// A palm: its foot, height, turn, scale and tint (the kit's `Palm`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palm {
    pub x: f64,
    pub z: f64,
    pub h: f64,
    pub yaw: f64,
    pub s: f64,
    pub c: f64,
}

impl Palm {
    /// Its trunk's radius at the foot (the kit's 0.28, scaled).
    pub fn trunk(&self) -> f64 {
        0.28 * self.s
    }
}

/// A beach town's layout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Town {
    pub zone: usize,
    pub zs0: f64,
    pub zs1: f64,
    pub pier_s: f64,
    pub marina_s: f64,
    pub town_s0: f64,
    pub town_s1: f64,
    pub cross_s: Vec<f64>,
    pub back_lats: Vec<f64>,
    pub front_lat: f64,
    pub prom_out: f64,
    pub lots: Vec<Lot>,
    pub houses: Vec<House>,
    pub hill: Vec<House>,
    pub palms: Vec<Palm>,
    /// The discs the layout took (x, z, r): what the kit builds round.
    pub occ: Vec<[f64; 3]>,
}

fn at(a: &[f32], i: usize) -> f64 {
    a.get(i).map_or(f64::NAN, |&v| f64::from(v))
}

/// The layout's random streams' seeds: the frontage and the houses, the
/// palms.
const LAYOUT_SEED: u32 = 0x5eab_0421;
const PALM_SEED: u32 = 0x5eab_0177;

struct Lay<'a> {
    t: &'a Track,
    rng: Mulberry32,
    palm_rng: Mulberry32,
    town: Town,
}

impl Lay<'_> {
    fn take(&mut self, x: f64, z: f64, r: f64) {
        self.town.occ.push([x, z, r]);
    }

    /// Outside the town road's driving corridor, with room to spare (the
    /// kit's `clearOfRoad`), and off every other road of the network.
    fn clear_of_road(&self, x: f64, z: f64, r: f64) -> bool {
        let t = self.t;
        let k = t.nearest(x, z, r + 40.0);
        let clear = k < 0 || {
            let p = t.project_window(x, z, k as f64, 8);
            let wall = if p.lat > 0.0 {
                at(&t.wall_r, p.i)
            } else {
                at(&t.wall_l, p.i)
            };
            p.lat.abs() - r > wall + 0.6
        };
        clear && off_branches(t, x, z, r)
    }

    fn free(&self, x: f64, z: f64, r: f64) -> bool {
        for o in &self.town.occ {
            let (dx, dz) = (x - o[0], z - o[1]);
            if dx * dx + dz * dz < (r + o[2]) * (r + o[2]) {
                return false;
            }
        }
        self.clear_of_road(x, z, r)
    }

    /// The kit's `frameAt(s, lat, road)`: +z facing the road (`road`) or
    /// away from it.
    fn frame_at(&self, s: f64, lat: f64, road: bool) -> (Place, Frame) {
        let f = self.t.frame(s);
        let p = self.t.point_at(s, lat);
        let side = js::or(js::sign(lat), 1.0);
        let k = if road { -side } else { side };
        (
            Place {
                x: p.x,
                z: p.z,
                yaw: kernel::atan2(k * f.rx, k * f.rz),
            },
            f,
        )
    }

    /// `addPalm(x, z, force)`.
    fn palm(&mut self, x: f64, z: f64, force: bool) {
        if !force && !self.free(x, z, 1.2) {
            return;
        }
        if force && !off_branches(self.t, x, z, 1.0) {
            return;
        }
        self.take(x, z, 1.0);
        let rng = &mut self.palm_rng;
        let h = rrange(rng, 9.0, 15.0);
        let yaw = rng.next_f64() * core::f64::consts::PI * 2.0;
        let s = rrange(rng, 0.85, 1.2);
        let c = rng.next_f64();
        self.town.palms.push(Palm { x, z, h, yaw, s, c });
    }
}

/// Whether a disc `r` round (x, z) is off every road but road 0 (the
/// town's), its tarmac and a metre more, as a beach town's lots must be
/// where the network's other roads run through it.
fn off_branches(t: &Track, x: f64, z: f64, r: f64) -> bool {
    t.net.roads_near(x, z, r + 30.0).into_iter().all(|k| {
        if k == 0 {
            return true;
        }
        let rt = t.road(k);
        let d = rt.distance_to_road(x, z, r + 30.0);
        let Some(s) = d.s else { return true };
        if !(rt.is_loop || (0.0..=rt.length).contains(&s)) {
            return true;
        }
        let f = rt.frame(s.clamp(0.0, rt.length));
        d.d - r > f.hw + f.margin + 1.0
    })
}

/// The town in zone `zone` of the level, laid out along road 0.
pub fn lay_out(t: &Track, zone: usize) -> Town {
    let mut town = Town {
        zone,
        ..Town::default()
    };
    // `setupRanges()`.
    town.zs0 = t.zone_start[zone] as f64;
    town.zs1 = if zone + 1 < t.zone_start.len() {
        t.zone_start[zone + 1] as f64
    } else {
        t.length
    };
    let (zs0, zs1) = (town.zs0, town.zs1);
    let tag = |n: &str| {
        t.tags
            .iter()
            .find(|g| g.tag == n && g.s0 >= zs0 - 5.0 && g.s0 < zs1)
            .map(|g| (g.s0, g.s1))
    };
    let mid = |g: Option<(f64, f64)>, d: f64| g.map_or(d, |g| js::round((g.0 + g.1) / 2.0));
    town.pier_s = mid(tag("pier"), zs0 + 700.0);
    town.marina_s = mid(tag("marina"), zs1 - 300.0);
    town.town_s0 = tag("promenade").map_or(zs0 + 420.0, |g| g.0) + 50.0;
    town.town_s1 = zs1 - 70.0;
    for k in -1..6 {
        let s = town.pier_s + f64::from(k) * 257.0;
        if s > town.town_s0 + 30.0 && s < town.town_s1 - 25.0 {
            town.cross_s.push(s);
        }
    }
    town.back_lats = vec![95.0, 187.0, 279.0];
    town.prom_out = t.frame(town.pier_s).wall_l + 6.8;
    town.front_lat = t.frame(town.pier_s).wall_r + 6.2;
    // `plan()`'s houses on the hill at the town entry.
    {
        let mut rng = Mulberry32::new(8812);
        let mut s = zs0 + 40.0;
        while s < town.town_s0 - 40.0 {
            let f = t.frame(s);
            let lat = f.wall_r + rrange(&mut rng, 32.0, 60.0);
            let p = t.point_at(s, lat);
            let seed = (rng.next_f64() * 1e9).floor() as u32;
            let mut hr = Mulberry32::new(seed);
            town.hill.push(House {
                at: Place {
                    x: p.x,
                    z: p.z,
                    yaw: kernel::atan2(-f.rx, -f.rz),
                },
                w: rrange(&mut hr, 10.0, 12.0),
                d: 11.0,
                floors: 2.0,
                car: false,
                seed,
            });
            s += rrange(&mut rng, 55.0, 80.0);
        }
    }
    let mut lay = Lay {
        t,
        rng: Mulberry32::new(LAYOUT_SEED),
        palm_rng: Mulberry32::new(PALM_SEED),
        town,
    };
    reserve_streets(&mut lay);
    frontage(&mut lay);
    residential(&mut lay);
    // The hill houses take their ground, and each has a palm.
    for h in lay.town.hill.clone() {
        lay.take(h.at.x, h.at.z, 9.0);
        let (x, z) = h.at.to_world(8.0, -2.0);
        lay.palm(x, z, false);
    }
    // A hill house standing on another road is not built.
    let hill = std::mem::take(&mut lay.town.hill);
    lay.town.hill = hill
        .into_iter()
        .filter(|h| off_branches(t, h.at.x, h.at.z, 8.0))
        .collect();
    palms(&mut lay);
    lay.town
}

/// Streets are laid first so nothing is built across them.
fn reserve_streets(l: &mut Lay) {
    let t = l.t;
    for s in l.town.cross_s.clone() {
        let f = t.frame(s);
        let mut d = f.wall_r + 0.8;
        while d < 300.0 {
            l.take(f.x + f.rx * d, f.z + f.rz * d, 7.5);
            d += 5.0;
        }
    }
    for lat in l.town.back_lats.clone() {
        let mut s = l.town.town_s0 - 20.0;
        while s < l.town.town_s1 + 20.0 {
            let p = t.point_at(s, lat);
            l.take(p.x, p.z, 5.5);
            s += 5.0;
        }
    }
}

/// Storefronts along the boulevard, block by block between the cross
/// streets, in the kit's order.
fn frontage(l: &mut Lay) {
    let shop = |l: &mut Lay, sign: Sign, w: Option<f64>| LotKind::Shop {
        sign,
        w: w.unwrap_or_else(|| rrange(&mut l.rng, 11.0, 14.0)),
    };
    let house = |l: &mut Lay| {
        let w = rrange(&mut l.rng, 13.0, 16.0);
        let hw = rrange(&mut l.rng, 10.0, 12.0);
        let hd = rrange(&mut l.rng, 10.0, 14.0);
        LotKind::House { w, house: [hw, hd] }
    };
    use Sign::*;
    let mut blocks: Vec<Vec<LotKind>> = Vec::new();
    {
        let a = shop(l, Icecream, None);
        let b = shop(l, Bikes, None);
        let c = shop(l, Cafe, Some(14.0));
        let d = shop(l, Records, None);
        let h = house(l);
        blocks.push(vec![
            LotKind::Surf,
            a,
            LotKind::Taco,
            b,
            c,
            LotKind::Parking,
            d,
            h,
        ]);
    }
    {
        let a = shop(l, Swim, None);
        let b = shop(l, Pizza, Some(15.0));
        let (h1, h2) = (house(l), house(l));
        blocks.push(vec![LotKind::Hotel, a, b, LotKind::Parking, h1, h2]);
    }
    {
        let h = house(l);
        let a = shop(l, Icecream, None);
        blocks.push(vec![LotKind::Motel, LotKind::Diner, h, a]);
    }
    {
        let a = shop(l, Pharmacy, Some(15.0));
        let (h1, h2) = (house(l), house(l));
        let b = shop(l, Cafe, None);
        blocks.push(vec![LotKind::Gas, a, h1, h2, b]);
    }
    {
        let a = shop(l, Supply, Some(16.0));
        let b = shop(l, Bait, None);
        let (h1, h2, h3) = (house(l), house(l), house(l));
        blocks.push(vec![a, b, h1, h2, h3]);
    }
    {
        let (h1, h2, h3, h4) = (house(l), house(l), house(l), house(l));
        blocks.push(vec![h1, h2, h3, h4]);
    }
    let mut edges = vec![l.town.town_s0];
    edges.extend(l.town.cross_s.iter().copied());
    edges.push(l.town.town_s1);
    let front = l.town.front_lat;
    for b in 0..edges.len() - 1 {
        let a = edges[b] + if b == 0 { 0.0 } else { 9.0 };
        let z = edges[b + 1] - if b == edges.len() - 2 { 0.0 } else { 9.0 };
        let list = blocks[b.min(blocks.len() - 1)].clone();
        let mut s = a;
        let mut k = 0;
        loop {
            let lot = match list.get(k) {
                Some(&lot) => lot,
                None => house(l),
            };
            k += 1;
            if s + lot.w() > z {
                break;
            }
            let ls = s + lot.w() / 2.0;
            let (fr, _) = l.frame_at(ls, front, true);
            let (w, d) = (lot.w(), lot.d());
            let (cx, cz) = fr.to_world(0.0, -d / 2.0);
            if l.clear_of_road(cx, cz, js::min(w, d) * 0.45)
                && off_branches(l.t, cx, cz, js::max(w, d) * 0.5)
            {
                l.take(cx, cz, js::max(w, d) * 0.5);
                l.town.lots.push(Lot {
                    kind: lot,
                    s: ls,
                    at: fr,
                });
            }
            s += lot.w() + rrange(&mut l.rng, 0.5, 2.5);
        }
    }
}

/// Houses behind main street, in rows along the back streets, each with a
/// palm or so in the yard.
fn residential(l: &mut Lay) {
    let t = l.t;
    let bl = l.town.back_lats.clone();
    let rows = [
        (bl[0] - 7.0, 1.0, 0.9),
        (bl[0] + 7.0, -1.0, 0.9),
        (bl[1] - 7.0, 1.0, 0.8),
        (bl[1] + 7.0, -1.0, 0.75),
        (bl[2] - 7.0, 1.0, 0.65),
        (bl[2] + 7.0, -1.0, 0.55),
    ];
    for (row_lat, dir, dens) in rows {
        let mut s = l.town.town_s0 + rrange(&mut l.rng, 0.0, 8.0);
        while s < l.town.town_s1 {
            'body: {
                if l.rng.next_f64() > dens {
                    break 'body;
                }
                let f = t.frame(s);
                let p = t.point_at(s, row_lat);
                // +Z (house front) toward the back street.
                let fr = Place {
                    x: p.x,
                    z: p.z,
                    yaw: kernel::atan2(dir * f.rx, dir * f.rz),
                };
                let w = rrange(&mut l.rng, 9.0, 12.0);
                let d = rrange(&mut l.rng, 10.0, 13.0);
                let (cx, cz) = fr.to_world(0.0, -d / 2.0);
                if !l.free(cx, cz, js::max(w, d) * 0.55) {
                    break 'body;
                }
                l.take(cx, cz, js::max(w, d) * 0.55);
                let floors = if l.rng.next_f64() < 0.7 { 2.0 } else { 3.0 };
                let car = l.rng.next_f64() < 0.16;
                let seed = (l.rng.next_f64() * 1e9).floor() as u32;
                l.town.houses.push(House {
                    at: fr,
                    w,
                    d,
                    floors,
                    car,
                    seed,
                });
                if l.rng.next_f64() < 0.55 {
                    let lz = rrange(&mut l.rng, -d, 1.0);
                    let (x, z) = fr.to_world(w / 2.0 + 1.4, lz);
                    l.palm(x, z, true);
                }
            }
            s += rrange(&mut l.rng, 14.0, 19.0);
        }
    }
}

/// The street palms: along the town sidewalk, the sand edge, the
/// boulevard's approach down the hill, and either side of the town sign.
fn palms(l: &mut Lay) {
    let t = l.t;
    let (town_s0, town_s1, pier_s, marina_s, zs0) = (
        l.town.town_s0,
        l.town.town_s1,
        l.town.pier_s,
        l.town.marina_s,
        l.town.zs0,
    );
    let cross = l.town.cross_s.clone();
    let near_cross = |s: f64, d: f64| cross.iter().any(|c| (s - c).abs() < d);
    let mut s = town_s0 - 20.0;
    while s < town_s1 + 20.0 {
        if !near_cross(s, 9.0) {
            let p = t.point_at(s + 11.0, t.frame(s).wall_r + 3.4);
            l.palm(p.x, p.z, true);
        }
        s += 23.0;
    }
    let mut s = town_s0 - 20.0;
    while s < town_s1 + 20.0 {
        if !((s - pier_s).abs() < 14.0 || (s - marina_s).abs() < 10.0) {
            let lat = -(l.town.prom_out + 2.5 + l.palm_rng.next_f64() * 2.0);
            let p = t.point_at(s, lat);
            l.palm(p.x, p.z, true);
        }
        s += 17.0;
    }
    let mut s = zs0 + 60.0;
    while s < town_s0 - 20.0 {
        let f = t.frame(s);
        let lat = f.wall_r + rrange(&mut l.palm_rng, 4.0, 9.0);
        let p = t.point_at(s, lat);
        l.palm(p.x, p.z, false);
        s += 30.0;
    }
    // The town sign's (`buildTownSign`).
    let s = js::max(zs0 + 80.0, town_s0 - 110.0);
    let f = t.frame(s);
    let p = t.point_at(s, f.wall_r + 5.0);
    l.take(p.x, p.z, 5.0);
    for dx in [-7.0, 7.0] {
        let q = t.point_at(s + dx, f.wall_r + 6.0);
        l.palm(q.x, q.z, true);
    }
}

impl Town {
    /// Every block of the town as a solid, on the ground `ground` says.
    pub fn solids(&self, ground: &dyn Fn(f64, f64) -> f64) -> Vec<Solid> {
        let mut out = Vec::new();
        for lot in &self.lots {
            for b in lot.kind.blocks() {
                let (x, z) = lot.at.to_world(b[0], b[1]);
                out.push(lot.at.block(b, ground(x, z)));
            }
        }
        for h in self.houses.iter().chain(&self.hill) {
            let b = [0.0, -h.d / 2.0, h.w, h.d, h.floors * 3.1];
            let (x, z) = h.at.to_world(b[0], b[1]);
            out.push(h.at.block(b, ground(x, z)));
        }
        for p in &self.palms {
            out.push(Solid {
                x: p.x,
                z: p.z,
                y: ground(p.x, p.z),
                height: p.h,
                shape: Shape::Round { r: p.trunk() },
            });
        }
        out
    }
}

/// The town of an open level: the Beach kit's, in the first zone of its
/// scenery (the kit is built once, for that zone), if road 0 runs
/// through it.
pub fn town(t: &Track) -> Option<Town> {
    t.open_ground.as_ref()?;
    let z = t
        .zones
        .iter()
        .filter(|z| z.zone.scenery == "Beach")
        .map(|z| z.id)
        .min()?;
    (z < t.zone_start.len() && t.zone_start[z] < t.n).then(|| lay_out(t, z))
}
