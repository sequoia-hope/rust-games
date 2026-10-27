//! The plan measured: every route, event and circuit resolved over the real
//! roads and profiled over the real ground, every region's land summed up,
//! every place checked. [`text`] is the report the owner and Claude read;
//! [`json`] is `resolved.json`, what the planner page draws.

use crate::Atlas;
use crate::geo::{LatLon, V2, inside, nearest_on};
use crate::graph::{Filter, resolve};
use crate::profile::Profile;
use crate::terrain::Cover;
use serde_json::{Map, Value, json};
use std::fmt::Write;

pub struct MeasuredRoute {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub built: bool,
    /// Game metres per real metre.
    pub scale: f64,
    pub line: Vec<V2>,
    pub profile: Profile,
    /// (place, metres off its road).
    pub snaps: Vec<(String, f64)>,
}

pub struct MeasuredRegion {
    pub id: String,
    pub name: String,
    pub area_km2: f64,
    pub land_km2: f64,
    pub h_min: f64,
    pub h_max: f64,
    pub h_mean: f64,
    pub cover: Vec<(Cover, f64)>,
    pub towns: Vec<String>,
}

pub struct MeasuredPlace {
    pub id: String,
    pub name: String,
    pub at: V2,
    pub h: f64,
    pub cover: Cover,
    /// The nearest road (its name or number) and how far (m).
    pub road: Option<(String, f64)>,
}

pub struct Measured {
    pub routes: Vec<MeasuredRoute>,
    pub regions: Vec<MeasuredRegion>,
    pub places: Vec<MeasuredPlace>,
    /// (circuit id, name, length, closes): whether each route ends where the
    /// next begins.
    pub circuits: Vec<(String, String, f64, bool)>,
}

/// The tunnels' and bridges' lines among the roads a route may use.
fn structures<'a>(a: &'a Atlas, f: &Filter) -> Vec<&'a [V2]> {
    a.geo
        .roads
        .iter()
        .filter(|r| (r.tunnel || r.bridge) && f.allows(r))
        .map(|r| r.pts.as_slice())
        .collect()
}

pub fn measure(a: &Atlas) -> Result<Measured, String> {
    let mut routes = Vec::new();
    for r in &a.plan.routes {
        let f = Filter::of(r);
        let mut stops = vec![r.from.as_str()];
        stops.extend(r.via.iter().map(String::as_str));
        stops.push(&r.to);
        let res = resolve(a, &f, &stops).map_err(|e| format!("route `{}`: {e}", r.id))?;
        routes.push(MeasuredRoute {
            id: r.id.clone(),
            name: r.name.clone(),
            kind: "route",
            built: r.built,
            scale: r.scale.unwrap_or(a.plan.scale),
            profile: Profile::of(&res.line, &a.terrain, &structures(a, &f)),
            line: res.line,
            snaps: res.snaps,
        });
    }
    for e in &a.plan.events {
        let r = a
            .plan
            .route(&e.route)
            .expect("checked when the plan loaded");
        let f = Filter::of(r);
        let res = resolve(a, &f, &[e.from.as_str(), e.to.as_str()])
            .map_err(|err| format!("event `{}`: {err}", e.id))?;
        routes.push(MeasuredRoute {
            id: e.id.clone(),
            name: e.name.clone(),
            kind: "event",
            built: r.built,
            scale: r.scale.unwrap_or(a.plan.scale),
            profile: Profile::of(&res.line, &a.terrain, &structures(a, &f)),
            line: res.line,
            snaps: res.snaps,
        });
    }
    let circuits = a
        .plan
        .circuits
        .iter()
        .map(|c| measure_circuit(c, &routes))
        .collect();
    let regions = a.plan.regions.iter().map(|r| region(a, r)).collect();
    let places = a
        .plan
        .places
        .iter()
        .map(|p| {
            let at = a.geo.projection.to_local(p.at);
            let road = a
                .geo
                .roads
                .iter()
                .map(|r| (r, nearest_on(at, &r.pts).0))
                .min_by(|x, y| x.1.total_cmp(&y.1))
                .map(|(r, d)| (road_label(r), d));
            MeasuredPlace {
                id: p.id.clone(),
                name: p.name.clone(),
                at,
                h: a.terrain.height_at(at),
                cover: a.terrain.cover_at(at),
                road,
            }
        })
        .collect();
    Ok(Measured {
        routes,
        regions,
        places,
        circuits,
    })
}

/// Measures a circuit across its legs (each `id` forward or `-id` reversed),
/// finding where each leg hands off to the next within the road network's
/// 160 m real-world reach (80 m at game scale 0.5).
fn measure_circuit(
    c: &crate::plan::Circuit,
    routes: &[MeasuredRoute],
) -> (String, String, f64, bool) {
    const REACH_M: f64 = 160.0;
    let legs: Vec<(f64, Vec<(V2, f64)>)> = c
        .routes
        .iter()
        .filter_map(|spec| {
            let (rev, id) = match spec.strip_prefix('-') {
                Some(id) => (true, id),
                None => (false, spec.as_str()),
            };
            let m = routes.iter().find(|m| m.id == id && m.kind == "route")?;
            let len = m.profile.length;
            let pts = if rev {
                m.profile
                    .samples
                    .iter()
                    .rev()
                    .map(|s| (s.at, len - s.d))
                    .collect()
            } else {
                m.profile.samples.iter().map(|s| (s.at, s.d)).collect()
            };
            Some((len, pts))
        })
        .collect();
    let n = legs.len();
    if n == 0 || n != c.routes.len() {
        return (c.id.clone(), c.name.clone(), 0.0, false);
    }
    let mut s_in = vec![0.0; n];
    let mut s_out = vec![0.0; n];
    let mut connected = true;
    for i in 0..n {
        let j = (i + 1) % n;
        let (len_i, pts_i) = &legs[i];
        let (_, pts_j) = &legs[j];
        let mut best = (f64::INFINITY, f64::INFINITY, 0.0, 0.0);
        for &(pi, di) in pts_i {
            for &(pj, dj) in pts_j {
                let d = pi.dist(pj);
                let score = d + 1e-6 * ((len_i - di) + dj);
                if score < best.0 {
                    best = (score, d, di, dj);
                }
            }
        }
        if best.1 > REACH_M {
            connected = false;
        }
        s_out[i] = best.2;
        s_in[j] = best.3;
    }
    let closes = connected && (0..n).all(|i| s_out[i] > s_in[i] + 100.0);
    let length = if closes {
        (0..n).map(|i| s_out[i] - s_in[i]).sum()
    } else {
        legs.iter().map(|(len, _)| *len).sum()
    };
    (c.id.clone(), c.name.clone(), length, closes)
}

fn road_label(r: &crate::geo::Road) -> String {
    match (&r.name, r.refs.first()) {
        (Some(n), Some(x)) => format!("{n} ({x})"),
        (Some(n), None) => n.clone(),
        (None, Some(x)) => format!("route {x}"),
        (None, None) => r.class.clone(),
    }
}

fn region(a: &Atlas, r: &crate::plan::Region) -> MeasuredRegion {
    let ring: Vec<V2> = r
        .ring
        .iter()
        .map(|&p| a.geo.projection.to_local(p))
        .collect();
    let t = &a.terrain;
    let (mut x0, mut x1, mut z0, mut z1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in &ring {
        x0 = x0.min(p.x);
        x1 = x1.max(p.x);
        z0 = z0.min(p.z);
        z1 = z1.max(p.z);
    }
    let i0 = (((x0 - t.x0) / t.step).floor().max(0.0)) as usize;
    let i1 = (((x1 - t.x0) / t.step).ceil().max(0.0) as usize).min(t.width.saturating_sub(1));
    let j0 = (((z0 - t.z0) / t.step).floor().max(0.0)) as usize;
    let j1 = (((z1 - t.z0) / t.step).ceil().max(0.0) as usize).min(t.height.saturating_sub(1));
    let mut counts = [0usize; 10];
    let (mut n, mut land, mut sum) = (0usize, 0usize, 0.0);
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    if !ring.is_empty() {
        for j in j0..=j1 {
            for i in i0..=i1 {
                let p = t.cell(i, j);
                if !inside(p, &ring) {
                    continue;
                }
                let k = j * t.width + i;
                let c = Cover::from_u8(t.cover[k]);
                counts[c as usize] += 1;
                n += 1;
                if !c.is_water() {
                    let h = f64::from(t.h[k]);
                    land += 1;
                    sum += h;
                    lo = lo.min(h);
                    hi = hi.max(h);
                }
            }
        }
    }
    let cell_km2 = t.step * t.step / 1e6;
    let mut cover: Vec<(Cover, f64)> = Cover::ALL
        .iter()
        .map(|&c| (c, counts[c as usize] as f64 / n.max(1) as f64))
        .filter(|&(_, f)| f > 0.0)
        .collect();
    cover.sort_by(|a, b| b.1.total_cmp(&a.1));
    MeasuredRegion {
        id: r.id.clone(),
        name: r.name.clone(),
        area_km2: n as f64 * cell_km2,
        land_km2: land as f64 * cell_km2,
        h_min: if land > 0 { lo } else { 0.0 },
        h_max: if land > 0 { hi } else { 0.0 },
        h_mean: if land > 0 { sum / land as f64 } else { 0.0 },
        cover,
        towns: a
            .geo
            .towns
            .iter()
            .filter(|tw| tw.kind != "neighborhood" && inside(tw.at, &ring))
            .map(|tw| tw.name.clone())
            .collect(),
    }
}

fn pct(f: f64) -> String {
    format!("{:.0} %", f * 100.0)
}

fn cover_line(c: &[(Cover, f64)], n: usize) -> String {
    c.iter()
        .take(n)
        .map(|(c, f)| format!("{} {}", c.name(), pct(*f)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The report, as plain text.
pub fn text(a: &Atlas, m: &Measured) -> String {
    let mut s = String::new();
    let p = &a.geo.projection;
    let built_routes = a.plan.routes.iter().filter(|r| r.built).count();
    let _ = writeln!(
        s,
        "{}: {} regions, {} places, {} routes ({} built), {} events. Game scale {} of real.\n",
        a.plan.world,
        a.plan.regions.len(),
        a.plan.places.len(),
        a.plan.routes.len(),
        built_routes,
        a.plan.events.len(),
        a.plan.scale
    );
    let _ = writeln!(s, "ROUTES AND EVENTS (real km, game km at the scale)");
    for r in &m.routes {
        let pr = &r.profile;
        let tag = if r.kind == "route" && r.built {
            "built"
        } else {
            r.kind
        };
        let _ = writeln!(
            s,
            "  {:<6} {:<34} {:>5.1} km real, {:>4.1} km game; {:>4.0} to {:>4.0} m, climb {:>4.0} m, \
             steepest {:.0} %, {} crests",
            tag,
            r.name,
            pr.length / 1e3,
            pr.length * r.scale / 1e3,
            pr.h_min,
            pr.h_max,
            pr.climb,
            pr.max_grade * 100.0,
            pr.crests.len()
        );
        let _ = writeln!(s, "         beside the road: {}", cover_line(&pr.cover, 4));
        if let Some(c) = pr.crests.first().map(|&i| &pr.samples[i]) {
            let ll = p.to_latlon(c.at);
            let _ = writeln!(
                s,
                "         first crest {:.0} m along at {:.0} m high ({:.5}, {:.5})",
                c.d, c.h, ll.lat, ll.lon
            );
        }
        for (id, off) in &r.snaps {
            if *off > 150.0 {
                let _ = writeln!(s, "         ! `{id}` is {off:.0} m from the route's roads");
            }
        }
    }
    if !m.circuits.is_empty() {
        let _ = writeln!(s, "\nCIRCUITS");
        for (_, name, len, closes) in &m.circuits {
            let _ = writeln!(
                s,
                "  {:<40} {:>5.1} km real, {}",
                name,
                len / 1e3,
                if *closes {
                    "closes"
                } else {
                    "! does not close"
                }
            );
        }
    }
    let _ = writeln!(s, "\nREGIONS");
    for r in &m.regions {
        let _ = writeln!(
            s,
            "  {:<28} {:>5.0} km2 ({:.0} of land), {:.0} to {:.0} m, mean {:.0} m",
            r.name, r.area_km2, r.land_km2, r.h_min, r.h_max, r.h_mean
        );
        let _ = writeln!(s, "         {}", cover_line(&r.cover, 5));
        if !r.towns.is_empty() {
            let _ = writeln!(s, "         towns: {}", r.towns.join(", "));
        }
    }
    let _ = writeln!(s, "\nPLACES");
    for pl in &m.places {
        let road = pl
            .road
            .as_ref()
            .map(|(n, d)| format!("{d:.0} m from {n}"))
            .unwrap_or_default();
        let _ = writeln!(
            s,
            "  {:<44} {:>4.0} m, {:<7} {}",
            pl.name,
            pl.h,
            pl.cover.name(),
            road
        );
    }
    s
}

fn r1(v: f64) -> Value {
    json!((v * 10.0).round() / 10.0)
}

fn ll(p: LatLon) -> Value {
    json!([(p.lat * 1e6).round() / 1e6, (p.lon * 1e6).round() / 1e6])
}

fn cover_json(c: &[(Cover, f64)]) -> Value {
    let mut m = Map::new();
    for (k, f) in c {
        m.insert(k.name().into(), json!((f * 1000.0).round() / 1000.0));
    }
    Value::Object(m)
}

/// `resolved.json`: what the planner page draws.
pub fn json(a: &Atlas, m: &Measured) -> Value {
    let p = &a.geo.projection;
    let routes: Vec<Value> = m
        .routes
        .iter()
        .map(|r| {
            let pr = &r.profile;
            // Every 60 m is plenty for a line on the map and a profile.
            let pts: Vec<Value> = pr
                .samples
                .iter()
                .step_by(3)
                .chain(pr.samples.last())
                .map(|s| json!([s.at.x.round(), s.at.z.round(), r1(s.h)]))
                .collect();
            json!({
                "id": r.id,
                "name": r.name,
                "kind": r.kind,
                "built": r.built,
                "length_m": pr.length.round(),
                "game_m": (pr.length * r.scale).round(),
                "climb_m": pr.climb.round(),
                "descent_m": pr.descent.round(),
                "h_min": r1(pr.h_min),
                "h_max": r1(pr.h_max),
                "max_grade": (pr.max_grade * 1000.0).round() / 1000.0,
                "crests": pr.crests.iter().map(|&i| {
                    let s = &pr.samples[i];
                    json!({"d": s.d.round(), "h": r1(s.h), "x": s.at.x.round(), "z": s.at.z.round(),
                           "at": ll(p.to_latlon(s.at))})
                }).collect::<Vec<_>>(),
                "cover": cover_json(&pr.cover),
                "snaps": r.snaps.iter().map(|(id, off)| json!([id, off.round()])).collect::<Vec<_>>(),
                "pts": pts,
            })
        })
        .collect();
    json!({
        "world": a.plan.world,
        "made_by": "cargo run -p mp_atlas -- resolve (from plan.json, geo.json and terrain.bin)",
        "routes": routes,
        "circuits": m.circuits.iter().map(|(id, name, len, closes)| json!({
            "id": id, "name": name, "length_m": len.round(), "closes": closes
        })).collect::<Vec<_>>(),
        "regions": m.regions.iter().map(|r| json!({
            "id": r.id, "area_km2": r1(r.area_km2), "land_km2": r1(r.land_km2),
            "h_min": r1(r.h_min), "h_max": r1(r.h_max), "h_mean": r1(r.h_mean),
            "cover": cover_json(&r.cover), "towns": r.towns,
        })).collect::<Vec<_>>(),
        "places": m.places.iter().map(|pl| json!({
            "id": pl.id, "x": pl.at.x.round(), "z": pl.at.z.round(), "h": r1(pl.h),
            "cover": pl.cover.name(),
            "road": pl.road.as_ref().map(|(n, d)| json!([n, d.round()])),
        })).collect::<Vec<_>>(),
    })
}
