//! The atlas's static picture of a level, built once on the CPU: the road
//! network as polylines (thinned), and from them the fields the shader
//! draws with (`atlas.wgsl`).
//!
//! A field is a grid over the level's box that holds, at each texel, the
//! offset in metres from the texel's centre to the nearest point of a road
//! (an "offset field"). Bilinear filtering of an offset is exact along a
//! straight road, where the nearest point moves linearly with the texel,
//! so the shader gets the true distance to the road at any zoom from one
//! sample, and strokes it as wide as it likes in screen pixels; only at a
//! junction or a tight bend, where the nearest road changes, does the
//! blend shorten it, by less than a texel. The offsets are half floats
//! (`Rgba16Float`, filterable on WebGL2 and WebGPU alike), so they are
//! good to a few centimetres. Two channels pairs: every road, and the
//! major roads (anything but a city street), which the atlas draws
//! bolder. An event's route is a field of its own over its own box
//! ([`route_line`]).
//!
//! The fields are built a slice at a time ([`FieldBuilder::step`]), a few
//! milliseconds a frame, the first time the atlas opens: the level's
//! network is about two thousand segments after thinning.

use mp_track::track::Track;

/// A polyline of the network, world (x, z), and whether it is a major road.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub pts: Vec<(f64, f64)>,
    pub major: bool,
}

/// The road type index of a city street (`ROAD_TYPES`' `street`).
const STREET: u8 = 5;

/// How far a thinned polyline may stray from the samples, metres.
pub const THIN: f64 = 1.5;

/// Douglas-Peucker: the points of `pts` to keep, within `tol` metres.
pub fn thin(pts: &[(f64, f64)], tol: f64) -> Vec<(f64, f64)> {
    let n = pts.len();
    if n <= 2 {
        return pts.to_vec();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    let t2 = tol * tol;
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb) = (pts[a], pts[b]);
        let mut worst = (0.0, a);
        for (k, &p) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = seg_d2(p, pa, pb);
            if d > worst.0 {
                worst = (d, k);
            }
        }
        if worst.0 > t2 {
            keep[worst.1] = true;
            stack.push((a, worst.1));
            stack.push((worst.1, b));
        }
    }
    pts.iter()
        .zip(keep)
        .filter_map(|(&p, k)| k.then_some(p))
        .collect()
}

/// The nearest point to `p` on the segment `a`–`b`.
fn nearest_on(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (ex, ez) = (b.0 - a.0, b.1 - a.1);
    let len2 = ex * ex + ez * ez;
    let h = if len2 > 0.0 {
        (((p.0 - a.0) * ex + (p.1 - a.1) * ez) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a.0 + ex * h, a.1 + ez * h)
}

fn seg_d2(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let c = nearest_on(p, a, b);
    (c.0 - p.0) * (c.0 - p.0) + (c.1 - p.1) * (c.1 - p.1)
}

/// Every road of the network as thinned polylines, cut where a road turns
/// from a city street to a major road or back (each run shares its end
/// point with the next). A loop closes on itself.
pub fn network_lines(t: &Track) -> Vec<Line> {
    let mut out = Vec::new();
    for r in t.roads() {
        if r.n < 2 {
            continue;
        }
        let major = |k: usize| r.road_type.get(k).is_none_or(|&ty| ty != STREET);
        let mut run: Vec<(f64, f64)> = Vec::new();
        let mut cls = major(0);
        let n = r.n;
        let count = if r.is_loop { n + 1 } else { n };
        for i in 0..count {
            let k = i % n;
            let p = (f64::from(r.px[k]), f64::from(r.pz[k]));
            let c = major(k);
            if c != cls && run.len() >= 2 {
                run.push(p);
                out.push(Line {
                    pts: thin(&run, THIN),
                    major: cls,
                });
                run = vec![p];
            }
            cls = c;
            run.push(p);
        }
        if run.len() >= 2 {
            out.push(Line {
                pts: thin(&run, THIN),
                major: cls,
            });
        }
    }
    out
}

/// An event's route on the level's road (road 0), from `s0` to `s1` the
/// lap's way (round a loop), thinned.
pub fn route_line(t: &Track, s0: f64, s1: f64) -> Line {
    let len = mp_sim::event::ahead(t, s0, s1);
    let mut pts = Vec::new();
    let mut s = 0.0;
    while s < len {
        let k = t.idx(s0 + s);
        pts.push((f64::from(t.px[k]), f64::from(t.pz[k])));
        s += 2.0;
    }
    let f = t.frame(s0 + len);
    pts.push((f.x, f.z));
    Line {
        pts: thin(&pts, THIN),
        major: true,
    }
}

/// The world box (min x, min z, max x, max z) round some lines.
pub fn bounds(lines: &[Line]) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    for &(x, z) in lines.iter().flat_map(|l| l.pts.iter()) {
        let e = b.get_or_insert([x, z, x, z]);
        e[0] = e[0].min(x);
        e[1] = e[1].min(z);
        e[2] = e[2].max(x);
        e[3] = e[3].max(z);
    }
    b
}

/// A grid over the world: texel (i, j)'s centre is at
/// (x0 + i * texel, z0 + j * texel).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub x0: f64,
    pub z0: f64,
    pub texel: f64,
    pub w: u32,
    pub h: u32,
}

impl Grid {
    /// The grid over `b` grown by `margin` metres, its longer side
    /// `max_dim` texels (or fewer, `min_texel` metres apart at the finest).
    pub fn over(b: [f64; 4], margin: f64, max_dim: u32, min_texel: f64) -> Grid {
        let (x0, z0) = (b[0] - margin, b[1] - margin);
        let (sx, sz) = (b[2] - b[0] + 2.0 * margin, b[3] - b[1] + 2.0 * margin);
        let texel = (sx.max(sz) / f64::from(max_dim - 1)).max(min_texel);
        Grid {
            x0,
            z0,
            texel,
            w: (sx / texel).ceil() as u32 + 1,
            h: (sz / texel).ceil() as u32 + 1,
        }
    }

    /// The world point of texel (i, j)'s centre.
    pub fn at(&self, i: u32, j: u32) -> (f64, f64) {
        (
            self.x0 + f64::from(i) * self.texel,
            self.z0 + f64::from(j) * self.texel,
        )
    }

    pub fn texels(&self) -> usize {
        self.w as usize * self.h as usize
    }
}

/// An `f32` as an IEEE half float's bits (round to nearest; out of range
/// to infinity, tiny to zero).
pub fn f16_bits(v: f32) -> u16 {
    let b = v.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32;
    let man = b & 0x7f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if man != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        // A subnormal half.
        let m = man | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = 1u32 << (shift - 1);
        let r = (m + half - 1 + ((m >> shift) & 1)) >> shift;
        return sign | r as u16;
    }
    let r = man + 0xfff + ((man >> 13) & 1);
    if r & 0x80_0000 != 0 {
        // The mantissa rounded over into the exponent.
        let e = e + 1;
        if e >= 0x1f {
            return sign | 0x7c00;
        }
        return sign | ((e as u16) << 10);
    }
    sign | ((e as u16) << 10) | (r >> 13) as u16
}

/// A half float's bits as an `f32`.
pub fn f16_value(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = i32::from((h >> 10) & 0x1f);
    let m = f32::from(h & 0x3ff);
    if e == 0 {
        return sign * m * (2f32).powi(-24);
    }
    if e == 0x1f {
        return if m == 0.0 {
            sign * f32::INFINITY
        } else {
            f32::NAN
        };
    }
    sign * (1.0 + m / 1024.0) * (2f32).powi(e - 15)
}

/// What a texel holds where no road is within reach: an offset far longer
/// than any line the shader strokes.
pub const FAR: f32 = 4000.0;

/// Builds an offset field a slice at a time: `channels` pairs of half
/// floats a texel (each line goes into pair 0, and pair 1 too when it is
/// major and there are two), row by row from z0.
pub struct FieldBuilder {
    pub grid: Grid,
    channels: usize,
    /// How far from a road an offset is kept, metres.
    reach: f64,
    lines: Vec<Line>,
    /// The next line and segment to splat.
    line: usize,
    seg: usize,
    data: Vec<u8>,
}

impl FieldBuilder {
    pub fn new(grid: Grid, lines: Vec<Line>, channels: usize, reach: f64) -> FieldBuilder {
        let far = f16_bits(FAR).to_le_bytes();
        let zero = f16_bits(0.0).to_le_bytes();
        let mut data = Vec::with_capacity(grid.texels() * channels * 4);
        for _ in 0..grid.texels() * channels {
            data.extend_from_slice(&far);
            data.extend_from_slice(&zero);
        }
        FieldBuilder {
            grid,
            channels,
            reach,
            lines,
            line: 0,
            seg: 0,
            data,
        }
    }

    pub fn done(&self) -> bool {
        self.line >= self.lines.len()
    }

    /// Splats segments until about `budget` texels have been visited;
    /// true once every segment is in.
    pub fn step(&mut self, budget: usize) -> bool {
        let mut spent = 0usize;
        while !self.done() && spent < budget {
            let l = &self.lines[self.line];
            if self.seg + 1 >= l.pts.len() {
                self.line += 1;
                self.seg = 0;
                continue;
            }
            let (a, b) = (l.pts[self.seg], l.pts[self.seg + 1]);
            let pairs: &[usize] = if l.major && self.channels > 1 {
                &[0, 1]
            } else {
                &[0]
            };
            spent += splat(
                &mut self.data,
                &self.grid,
                self.channels,
                self.reach,
                a,
                b,
                pairs,
            );
            self.seg += 1;
        }
        self.done()
    }

    /// The texels' bytes (`Rgba16Float` for two pairs, `Rg16Float` for one).
    pub fn finish(self) -> Vec<u8> {
        self.data
    }
}

/// Writes the segment a–b's offsets into the texels within `reach` of it
/// where it is nearer than what is there; the texels visited.
fn splat(
    data: &mut [u8],
    g: &Grid,
    channels: usize,
    reach: f64,
    a: (f64, f64),
    b: (f64, f64),
    pairs: &[usize],
) -> usize {
    let lo = |v: f64, o: f64| ((v - reach - o) / g.texel).ceil().max(0.0);
    let hi = |v: f64, o: f64, n: u32| ((v + reach - o) / g.texel).floor().min(f64::from(n) - 1.0);
    let (i0, i1) = (lo(a.0.min(b.0), g.x0), hi(a.0.max(b.0), g.x0, g.w));
    let (j0, j1) = (lo(a.1.min(b.1), g.z0), hi(a.1.max(b.1), g.z0, g.h));
    if i1 < i0 || j1 < j0 {
        return 0;
    }
    let (i0, i1, j0, j1) = (i0 as u32, i1 as u32, j0 as u32, j1 as u32);
    let r2 = reach * reach;
    let stride = channels * 4;
    let mut visited = 0;
    for j in j0..=j1 {
        for i in i0..=i1 {
            visited += 1;
            let p = g.at(i, j);
            let c = nearest_on(p, a, b);
            let (ox, oz) = (c.0 - p.0, c.1 - p.1);
            let d2 = ox * ox + oz * oz;
            if d2 > r2 {
                continue;
            }
            let base = (j as usize * g.w as usize + i as usize) * stride;
            for &pair in pairs {
                let o = base + pair * 4;
                let old_x = f64::from(f16_value(u16::from_le_bytes([data[o], data[o + 1]])));
                let old_z = f64::from(f16_value(u16::from_le_bytes([data[o + 2], data[o + 3]])));
                if d2 < old_x * old_x + old_z * old_z {
                    data[o..o + 2].copy_from_slice(&f16_bits(ox as f32).to_le_bytes());
                    data[o + 2..o + 4].copy_from_slice(&f16_bits(oz as f32).to_le_bytes());
                }
            }
        }
    }
    visited
}

/// The water under a grid: 255 where `wet(x, z)`, 0 elsewhere, one byte a
/// texel.
pub fn water(g: &Grid, wet: impl Fn(f64, f64) -> bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(g.texels());
    for j in 0..g.h {
        for i in 0..g.w {
            let (x, z) = g.at(i, j);
            out.push(if wet(x, z) { 255 } else { 0 });
        }
    }
    out
}

/// A place the atlas names: a cruise's start, a zone, an event.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub text: String,
    pub x: f64,
    pub z: f64,
    pub kind: LabelKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelKind {
    /// A town where a cruise can start: always shown.
    Town,
    /// A road of the network by name: shown when there is room.
    Road,
    /// A stretch of road (a zone): likewise, after the roads.
    Zone,
}

/// The level's places: its cruise starts, then its roads by name (at
/// their middle), then each road's zones (at the middle of their stretch,
/// each name once; a zone the road only lists, under 300 m, is left out).
pub fn labels(t: &Track) -> Vec<Label> {
    let mut out: Vec<Label> = Vec::new();
    let road_of = |id: &str| -> usize {
        if id.is_empty() {
            return 0;
        }
        t.branches
            .iter()
            .position(|b| b.id == id)
            .map_or(0, |k| k + 1)
    };
    for s in &t.level.starts {
        let r = t.road(road_of(s.road));
        let f = r.frame(s.s);
        out.push(Label {
            text: s.name.to_uppercase(),
            x: f.x,
            z: f.z,
            kind: LabelKind::Town,
        });
    }
    for b in &t.branches {
        let r = &b.track;
        if b.name.is_empty() || r.n < 2 {
            continue;
        }
        let f = r.frame(r.length / 2.0);
        out.push(Label {
            text: b.name.to_uppercase(),
            x: f.x,
            z: f.z,
            kind: LabelKind::Road,
        });
    }
    for r in t.roads() {
        for z in &r.zones {
            let name = z.zone.name;
            if name.is_empty()
                || z.s1 - z.s0 < 300.0
                || out.iter().any(|l| l.text.eq_ignore_ascii_case(name))
            {
                continue;
            }
            let f = r.frame((z.s0 + z.s1) / 2.0);
            out.push(Label {
                text: name.to_uppercase(),
                x: f.x,
                z: f.z,
                kind: LabelKind::Zone,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats_round_trip() {
        for v in [
            0.0f32, 1.0, -1.0, 0.5, 3.25, -17.75, 1000.0, 4000.0, 0.001, 65504.0,
        ] {
            let back = f16_value(f16_bits(v));
            assert!((back - v).abs() <= v.abs() / 1000.0 + 1e-4, "{v} {back}");
        }
        assert_eq!(f16_bits(1.0), 0x3c00);
        assert_eq!(f16_bits(-2.0), 0xc000);
        assert_eq!(f16_bits(1e6), 0x7c00);
    }

    #[test]
    fn thinning_keeps_corners_and_drops_the_straight() {
        let mut pts: Vec<(f64, f64)> = (0..=100).map(|i| (f64::from(i), 0.0)).collect();
        pts.extend((1..=100).map(|i| (100.0, f64::from(i))));
        let t = thin(&pts, 0.5);
        assert_eq!(t, vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)]);
    }

    /// The field holds the offset to the nearest road: exact on a straight
    /// road, bilinear between texels exact too.
    #[test]
    fn the_field_points_at_the_nearest_road() {
        let lines = vec![
            Line {
                pts: vec![(0.0, 0.0), (1000.0, 0.0)],
                major: false,
            },
            Line {
                pts: vec![(500.0, -500.0), (500.0, 500.0)],
                major: true,
            },
        ];
        let b = bounds(&lines).unwrap();
        let g = Grid::over(b, 100.0, 256, 1.0);
        let mut fb = FieldBuilder::new(g, lines, 2, 80.0);
        let mut steps = 0;
        while !fb.step(100) {
            steps += 1;
        }
        // A segment at a time once over budget: more than one slice.
        assert!(steps >= 1, "{steps} slices");
        let data = fb.finish();
        let at = |i: u32, j: u32, pair: usize| {
            let o = (j as usize * g.w as usize + i as usize) * 8 + pair * 4;
            (
                f16_value(u16::from_le_bytes([data[o], data[o + 1]])),
                f16_value(u16::from_le_bytes([data[o + 2], data[o + 3]])),
            )
        };
        // A texel near (200, 30): 30 m up from the street, north of it.
        let i = ((200.0 - g.x0) / g.texel).round() as u32;
        let j = ((30.0 - g.z0) / g.texel).round() as u32;
        let (x, z) = g.at(i, j);
        let (ox, oz) = at(i, j, 0);
        assert!(
            ox.abs() < 1e-3 && (f64::from(oz) + z).abs() < 0.05,
            "{ox} {oz} {z}"
        );
        // The major channel there: the avenue at x = 500 is out of reach.
        assert_eq!(at(i, j, 1).0, FAR);
        // Near the avenue, both channels point at it.
        let i = ((470.0 - g.x0) / g.texel).round() as u32;
        let j = ((200.0 - g.z0) / g.texel).round() as u32;
        let (x2, _) = g.at(i, j);
        for pair in [0, 1] {
            let (ox, oz) = at(i, j, pair);
            assert!((f64::from(ox) - (500.0 - x2)).abs() < 0.05 && oz.abs() < 1e-3);
        }
        let _ = x;
    }

    #[test]
    fn the_peninsulas_network_its_box_and_its_places() {
        let t = Track::new(&mp_levels::level_by_id("peninsula")).expect("track");
        let lines = network_lines(&t);
        let segs: usize = lines.iter().map(|l| l.pts.len() - 1).sum();
        let b = bounds(&lines).unwrap();
        let km = |v: f64| v / 1000.0;
        println!(
            "{} lines, {segs} segments, box {:.1} x {:.1} km",
            lines.len(),
            km(b[2] - b[0]),
            km(b[3] - b[1])
        );
        assert!(segs > 500 && segs < 20000, "{segs}");
        // The atlas's own field over it, and what building it costs.
        let g = Grid::over(b, 400.0, 1536, 2.0);
        let t0 = std::time::Instant::now();
        let mut fb = FieldBuilder::new(g, lines.clone(), 2, 16.0 * g.texel);
        let mut slices = 0;
        while !fb.step(60_000) {
            slices += 1;
        }
        let t1 = t0.elapsed();
        let cover = t.level.cover.clone().expect("cover");
        let wg = Grid::over(b, 3000.0, 768, 4.0);
        let t2 = std::time::Instant::now();
        let wet = water(&wg, |x, z| cover(x, z) <= 1);
        println!(
            "field {} x {} at {:.1} m: {slices} slices of 60k visits, {:?}; water {} x {}, {:?}, {:.0}% wet",
            g.w,
            g.h,
            g.texel,
            t1,
            wg.w,
            wg.h,
            t2.elapsed(),
            wet.iter().filter(|&&v| v > 0).count() as f64 * 100.0 / wet.len() as f64
        );
        // Every road is in it, the city streets as minor lines.
        assert!(lines.iter().any(|l| l.major) && lines.iter().any(|l| !l.major));
        let ls = labels(&t);
        let find = |n: &str| {
            ls.iter()
                .find(|l| l.text == n)
                .unwrap_or_else(|| panic!("{n}"))
        };
        // North is -z: San Francisco's Lombard Street is north of Redwood
        // City, and the crest above the sea is west of it.
        let (lombard, redwood, crest) = (
            find("LOMBARD STREET"),
            find("REDWOOD CITY"),
            find("SAN GREGORIO"),
        );
        assert!(lombard.z < redwood.z, "{lombard:?} {redwood:?}");
        assert!(crest.x < redwood.x, "{crest:?} {redwood:?}");
        for l in &ls {
            assert!(
                l.x >= b[0] - 1.0 && l.x <= b[2] + 1.0 && l.z >= b[1] - 1.0 && l.z <= b[3] + 1.0
            );
        }
    }
}
