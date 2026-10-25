//! Routes along the real roads: the shortest way between two places over
//! the roads a route may use (its route numbers, or its road classes).
//!
//! A place snaps to the nearest point on an allowed road, not to the
//! nearest junction: on the coast a road can run kilometres between
//! junctions. Roads are two-way here; one-way carriageways of a divided
//! highway are both in the graph and either does, but a route keeps to
//! one: each place may snap to any allowed road near it ([`candidates`]),
//! and [`resolve`] picks the snaps whose way through every place is the
//! shortest, so a freeway route does not cross to the other carriageway
//! (the two are joined only where they merge, often far away) between two
//! places that happened to sit nearer the other one.

use crate::geo::{Geo, Road, V2, nearest_on, polyline_length, sub_line};
use crate::plan::Route;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Which roads a route may use.
pub struct Filter<'a> {
    pub refs: &'a [String],
    pub names: &'a [String],
    pub classes: &'a [String],
}

impl Filter<'_> {
    pub fn of(r: &Route) -> Filter<'_> {
        Filter {
            refs: &r.roads,
            names: &r.names,
            classes: &r.classes,
        }
    }

    pub fn allows(&self, road: &Road) -> bool {
        if !self.refs.is_empty() || !self.names.is_empty() {
            return self.refs.iter().any(|r| road.has_ref(r))
                || road
                    .name
                    .as_ref()
                    .is_some_and(|n| self.names.iter().any(|x| x == n));
        }
        self.classes.is_empty() || self.classes.contains(&road.class)
    }

    /// The roads a route's way may use: those it [`allows`](Self::allows),
    /// and, when it names its roads, the short links between them that the
    /// map leaves without a name or number (a slip lane where a street
    /// splits round an island, a jog at a junction), so a named road is not
    /// cut in two where it is drawn as a pair for a few metres.
    pub fn ways(&self, geo: &Geo) -> Vec<bool> {
        let mut ok: Vec<bool> = geo.roads.iter().map(|r| self.allows(r)).collect();
        if self.refs.is_empty() && self.names.is_empty() {
            return ok;
        }
        let mut touches = vec![false; geo.nodes.len()];
        for (r, &a) in geo.roads.iter().zip(&ok) {
            if a {
                touches[r.a] = true;
                touches[r.b] = true;
            }
        }
        for (r, a) in geo.roads.iter().zip(ok.iter_mut()) {
            *a = *a
                || (r.name.is_none()
                    && r.refs.is_empty()
                    && r.length < LINK_MAX
                    && touches[r.a]
                    && touches[r.b]);
        }
        ok
    }
}

/// The longest unnamed link [`Filter::ways`] lets a named route through (m).
pub const LINK_MAX: f64 = 60.0;

/// Where a place meets the road network.
#[derive(Clone, Copy, Debug)]
pub struct Snap {
    pub road: usize,
    /// Metres along the road from its `a` end.
    pub along: f64,
    /// How far the place is from the road (m).
    pub off: f64,
    pub at: V2,
}

/// The nearest point on an allowed road.
pub fn snap(geo: &Geo, f: &Filter, p: V2) -> Option<Snap> {
    let mut best: Option<Snap> = None;
    for (i, r) in geo.roads.iter().enumerate() {
        if !f.allows(r) {
            continue;
        }
        let (d, along) = nearest_on(p, &r.pts);
        if best.is_none_or(|b| d < b.off) {
            best = Some(Snap {
                road: i,
                along,
                off: d,
                at: crate::geo::point_along(&r.pts, along),
            });
        }
    }
    best
}

/// How much further than the nearest a road may be and still be a place's
/// candidate (m): wider than a divided highway's median.
pub const SNAP_SLACK: f64 = 80.0;
/// A place's candidates, at most this many, nearest first.
pub const MAX_CANDIDATES: usize = 6;
/// What a metre off the road costs against a metre of road when the snaps
/// are chosen: a farther road wins only if it saves ten times the extra
/// metres of road, so a place keeps its nearest road unless the nearest
/// sends the way round by the far side of a divided highway (kilometres).
pub const OFF_WEIGHT: f64 = 10.0;

/// The nearest point on each allowed road within [`SNAP_SLACK`] of the
/// nearest, nearest first (ties by road), at most [`MAX_CANDIDATES`].
pub fn candidates(geo: &Geo, f: &Filter, p: V2) -> Vec<Snap> {
    let mut all: Vec<Snap> = geo
        .roads
        .iter()
        .enumerate()
        .filter(|(_, r)| f.allows(r))
        .map(|(i, r)| {
            let (off, along) = nearest_on(p, &r.pts);
            Snap {
                road: i,
                along,
                off,
                at: crate::geo::point_along(&r.pts, along),
            }
        })
        .collect();
    all.sort_by(|a, b| a.off.total_cmp(&b.off).then(a.road.cmp(&b.road)));
    let Some(near) = all.first().map(|s| s.off) else {
        return all;
    };
    all.retain(|s| s.off <= near + SNAP_SLACK);
    all.truncate(MAX_CANDIDATES);
    all
}

#[derive(Clone, Copy, PartialEq)]
struct Item {
    cost: f64,
    node: usize,
}

impl Eq for Item {}

impl Ord for Item {
    fn cmp(&self, o: &Self) -> Ordering {
        // A min-heap on cost; ties broken by node, so the result never
        // depends on the heap's internals.
        o.cost
            .total_cmp(&self.cost)
            .then_with(|| o.node.cmp(&self.node))
    }
}

impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// The allowed roads at each node.
fn adjacency(geo: &Geo, f: &Filter) -> Vec<Vec<usize>> {
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); geo.nodes.len()];
    for ((i, r), ok) in geo.roads.iter().enumerate().zip(f.ways(geo)) {
        if ok {
            adj[r.a].push(i);
            adj[r.b].push(i);
        }
    }
    adj
}

/// The shortest ways from one snap to every node over the allowed roads.
pub struct Tree {
    from: Snap,
    dist: Vec<f64>,
    prev: Vec<Option<usize>>,
}

impl Tree {
    fn grow(geo: &Geo, adj: &[Vec<usize>], a: Snap) -> Tree {
        let ra = &geo.roads[a.road];
        let n = geo.nodes.len();
        let mut dist = vec![f64::INFINITY; n];
        let mut prev: Vec<Option<usize>> = vec![None; n];
        let mut heap = BinaryHeap::new();
        for (node, cost) in [(ra.a, a.along), (ra.b, ra.length - a.along)] {
            if cost < dist[node] {
                dist[node] = cost;
                heap.push(Item { cost, node });
            }
        }
        while let Some(Item { cost, node }) = heap.pop() {
            if cost > dist[node] {
                continue;
            }
            for &e in &adj[node] {
                let r = &geo.roads[e];
                let other = if r.a == node { r.b } else { r.a };
                let c = cost + r.length;
                if c < dist[other] {
                    dist[other] = c;
                    prev[other] = Some(e);
                    heap.push(Item {
                        cost: c,
                        node: other,
                    });
                }
            }
        }
        Tree {
            from: a,
            dist,
            prev,
        }
    }

    /// How far it is to `b` (infinite when the roads do not join).
    fn length_to(&self, geo: &Geo, b: Snap) -> f64 {
        let rb = &geo.roads[b.road];
        let via = (self.dist[rb.a] + b.along).min(self.dist[rb.b] + (rb.length - b.along));
        if self.from.road == b.road {
            via.min((self.from.along - b.along).abs())
        } else {
            via
        }
    }

    /// The way to `b` as a polyline from the tree's snap.
    fn line_to(&self, geo: &Geo, b: Snap) -> Option<Vec<V2>> {
        let a = self.from;
        let ra = &geo.roads[a.road];
        let rb = &geo.roads[b.road];
        // Along the same road and nothing shorter round the network: straight
        // along it.
        let direct = (a.road == b.road).then(|| (a.along - b.along).abs());
        let via_a = self.dist[rb.a] + b.along;
        let via_b = self.dist[rb.b] + (rb.length - b.along);
        let best = via_a.min(via_b);
        if let Some(d) = direct
            && d <= best
        {
            return Some(sub_line(&ra.pts, a.along, b.along));
        }
        if !best.is_finite() {
            return None;
        }
        // Back from the node b's road is entered by, to a's road.
        let (end_node, tail) = if via_a <= via_b {
            (rb.a, sub_line(&rb.pts, 0.0, b.along))
        } else {
            (rb.b, sub_line(&rb.pts, rb.length, b.along))
        };
        let mut legs: Vec<Vec<V2>> = vec![tail];
        let mut node = end_node;
        while let Some(e) = self.prev[node] {
            let r = &geo.roads[e];
            let (from, line) = if r.b == node {
                (r.a, r.pts.clone())
            } else {
                let mut p = r.pts.clone();
                p.reverse();
                (r.b, p)
            };
            legs.push(line);
            node = from;
        }
        // node is now ra.a or ra.b: the first leg is along a's road to it.
        let head = if node == ra.a {
            sub_line(&ra.pts, a.along, 0.0)
        } else {
            sub_line(&ra.pts, a.along, ra.length)
        };
        legs.push(head);
        legs.reverse();
        let mut out: Vec<V2> = Vec::new();
        for leg in legs {
            for p in leg {
                if out.last().is_none_or(|q| q.dist(p) > 1e-6) {
                    out.push(p);
                }
            }
        }
        Some(out)
    }
}

/// The shortest way from one snap to another over the allowed roads, as a
/// polyline from `a` to `b`.
pub fn path(geo: &Geo, f: &Filter, a: Snap, b: Snap) -> Option<Vec<V2>> {
    Tree::grow(geo, &adjacency(geo, f), a).line_to(geo, b)
}

/// A route resolved over the roads: its line, and how far each of its
/// places sat from the road it snapped to.
pub struct Resolved {
    pub line: Vec<V2>,
    pub length: f64,
    /// (place id, metres off the road).
    pub snaps: Vec<(String, f64)>,
}

/// Resolves a route through its places, in order: of each place's
/// [`candidates`], the snaps whose way through them all, plus
/// [`OFF_WEIGHT`] for every metre a place is off its road, is the shortest.
pub fn resolve(atlas: &crate::Atlas, f: &Filter, stops: &[&str]) -> Result<Resolved, String> {
    let mut at = Vec::new();
    for id in stops {
        at.push((*id, atlas.place(id)?));
    }
    resolve_at(&atlas.geo, f, &at)
}

/// [`resolve`] through points given in the local frame, each with a name
/// for the messages.
pub fn resolve_at(geo: &Geo, f: &Filter, at: &[(&str, V2)]) -> Result<Resolved, String> {
    let stops: Vec<&str> = at.iter().map(|s| s.0).collect();
    let mut cands: Vec<Vec<Snap>> = Vec::new();
    for &(id, p) in at {
        let c = candidates(geo, f, p);
        if c.is_empty() {
            return Err(format!("no road the route may use near `{id}`"));
        }
        cands.push(c);
    }
    let adj = adjacency(geo, f);
    // cost[i][k]: the cheapest way to stop i's candidate k; from[i][k] the
    // candidate of stop i - 1 it came from.
    let mut cost: Vec<Vec<f64>> = vec![cands[0].iter().map(|s| OFF_WEIGHT * s.off).collect()];
    let mut from: Vec<Vec<usize>> = vec![vec![0; cands[0].len()]];
    for i in 1..cands.len() {
        let trees: Vec<Tree> = cands[i - 1]
            .iter()
            .map(|&s| Tree::grow(geo, &adj, s))
            .collect();
        let (mut c, mut fr) = (Vec::new(), Vec::new());
        for b in &cands[i] {
            let mut best = (f64::INFINITY, 0);
            for (j, t) in trees.iter().enumerate() {
                let x = cost[i - 1][j] + t.length_to(geo, *b);
                if x < best.0 {
                    best = (x, j);
                }
            }
            c.push(best.0 + OFF_WEIGHT * b.off);
            fr.push(best.1);
        }
        if c.iter().all(|x| !x.is_finite()) {
            return Err(format!(
                "no way from `{}` to `{}` on the route's roads",
                stops[i - 1],
                stops[i]
            ));
        }
        cost.push(c);
        from.push(fr);
    }
    // Back from the cheapest last snap.
    let last = cost.len() - 1;
    let mut k = (0..cost[last].len())
        .min_by(|&a, &b| cost[last][a].total_cmp(&cost[last][b]).then(a.cmp(&b)))
        .expect("every stop has a candidate");
    let mut chosen = vec![0; stops.len()];
    for i in (0..stops.len()).rev() {
        chosen[i] = k;
        k = from[i][k];
    }
    let snaps: Vec<Snap> = (0..stops.len()).map(|i| cands[i][chosen[i]]).collect();
    let mut line: Vec<V2> = Vec::new();
    for (i, w) in snaps.windows(2).enumerate() {
        let leg = Tree::grow(geo, &adj, w[0])
            .line_to(geo, w[1])
            .ok_or_else(|| {
                format!(
                    "no way from `{}` to `{}` on the route's roads",
                    stops[i],
                    stops[i + 1]
                )
            })?;
        for p in leg {
            if line.last().is_none_or(|q| q.dist(p) > 1e-6) {
                line.push(p);
            }
        }
    }
    Ok(Resolved {
        length: polyline_length(&line),
        line,
        snaps: stops
            .iter()
            .zip(&snaps)
            .map(|(id, s)| ((*id).to_owned(), s.off))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::Projection;

    fn road(a: usize, b: usize, pts: &[(f64, f64)]) -> Road {
        let pts: Vec<V2> = pts.iter().map(|&(x, z)| V2::new(x, z)).collect();
        Road {
            a,
            b,
            class: "motorway".into(),
            name: None,
            refs: vec!["101".into()],
            bridge: false,
            tunnel: false,
            length: polyline_length(&pts),
            pts,
        }
    }

    #[test]
    fn a_route_keeps_to_one_carriageway() {
        // Two carriageways 40 m apart, 10 km long, joined only at their
        // west ends. The first place is nearer the north one, the second
        // nearer the south one: snapped to the nearest, the way runs 10 km
        // west, round and back.
        let nodes = vec![
            V2::new(0.0, 0.0),
            V2::new(10_000.0, 0.0),
            V2::new(0.0, 40.0),
            V2::new(10_000.0, 40.0),
        ];
        let roads = vec![
            road(0, 1, &[(0.0, 0.0), (10_000.0, 0.0)]),
            road(2, 3, &[(0.0, 40.0), (10_000.0, 40.0)]),
            road(0, 2, &[(0.0, 0.0), (0.0, 40.0)]),
        ];
        let geo = Geo {
            name: String::new(),
            projection: Projection {
                origin: crate::LatLon { lat: 0.0, lon: 0.0 },
                m_per_deg_lat: 1.0,
                m_per_deg_lon: 1.0,
            },
            nodes,
            roads,
            rail: vec![],
            areas: vec![],
            towns: vec![],
        };
        let refs = vec!["101".to_owned()];
        let f = Filter {
            refs: &refs,
            names: &[],
            classes: &[],
        };
        let r = resolve_at(
            &geo,
            &f,
            &[("a", V2::new(5_000.0, 15.0)), ("b", V2::new(6_000.0, 28.0))],
        )
        .unwrap();
        assert!((r.length - 1_000.0).abs() < 1e-6, "{}", r.length);
        // Both on the south one, the nearer of the two in sum: the first
        // place is 25 m off it, not the 15 m to the north one.
        assert!((r.snaps[0].1 - 25.0).abs() < 1e-9 && (r.snaps[1].1 - 12.0).abs() < 1e-9);
        // With one place only near one carriageway, it is that one.
        let r = resolve_at(
            &geo,
            &f,
            &[
                ("a", V2::new(5_000.0, -100.0)),
                ("b", V2::new(6_000.0, 25.0)),
            ],
        )
        .unwrap();
        assert!((r.length - 1_000.0).abs() < 1e-6, "{}", r.length);
    }

    #[test]
    fn a_named_route_crosses_an_unnamed_link() {
        // Mission Street drawn as a pair round an island, its two halves
        // joined by a short link with no name.
        let mut a = road(0, 1, &[(0.0, 0.0), (500.0, 0.0)]);
        let link = road(1, 2, &[(500.0, 0.0), (530.0, 0.0)]);
        let mut b = road(2, 3, &[(530.0, 0.0), (1_000.0, 0.0)]);
        for r in [&mut a, &mut b] {
            r.refs.clear();
            r.name = Some("Mission Street".into());
        }
        let mut link = link;
        link.refs.clear();
        let geo = Geo {
            name: String::new(),
            projection: Projection {
                origin: crate::LatLon { lat: 0.0, lon: 0.0 },
                m_per_deg_lat: 1.0,
                m_per_deg_lon: 1.0,
            },
            nodes: vec![
                V2::new(0.0, 0.0),
                V2::new(500.0, 0.0),
                V2::new(530.0, 0.0),
                V2::new(1_000.0, 0.0),
            ],
            roads: vec![a, link, b],
            rail: vec![],
            areas: vec![],
            towns: vec![],
        };
        let names = vec!["Mission Street".to_owned()];
        let f = Filter {
            refs: &[],
            names: &names,
            classes: &[],
        };
        assert_eq!(f.ways(&geo), vec![true, true, true]);
        let r = resolve_at(
            &geo,
            &f,
            &[("a", V2::new(100.0, 5.0)), ("b", V2::new(900.0, 5.0))],
        )
        .unwrap();
        assert!((r.length - 800.0).abs() < 1e-6, "{}", r.length);
    }
}
