//! Where the atlas looks: a centre in the world and a scale (metres per
//! CSS px), north up (world -z), and how zooming, panning and following
//! the car move it. Pure, so the tests can hold it to its rules.

/// The regional zoom: this many metres across the panel's shorter side.
pub const REGION: f64 = 3000.0;
/// The closest zoom: this many metres across the shorter side.
pub const CLOSEST: f64 = 600.0;
/// How far past the whole map the view may zoom out.
const OUT: f64 = 1.25;

/// The two presets Z (or the zoom button) steps between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// The whole level's network, fitted to the panel.
    Whole,
    /// [`REGION`] across, centred on the car and following it.
    Region,
}

/// The view: centre (x, z), metres per CSS px, whether it follows the car.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub cx: f64,
    pub cz: f64,
    pub mpp: f64,
    pub follow: bool,
}

/// The panel's size, CSS px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    pub w: f64,
    pub h: f64,
}

impl Panel {
    fn short(&self) -> f64 {
        self.w.min(self.h).max(1.0)
    }
}

/// The whole box `b` (min x, min z, max x, max z) in the panel, with `pad`
/// CSS px round it.
pub fn whole(b: [f64; 4], p: Panel, pad: f64) -> View {
    let (w, h) = ((p.w - 2.0 * pad).max(1.0), (p.h - 2.0 * pad).max(1.0));
    let mpp = ((b[2] - b[0]) / w).max((b[3] - b[1]) / h).max(1e-3);
    View {
        cx: (b[0] + b[2]) / 2.0,
        cz: (b[1] + b[3]) / 2.0,
        mpp,
        follow: false,
    }
}

/// [`REGION`] across, on the car.
pub fn region(car: (f64, f64), p: Panel) -> View {
    View {
        cx: car.0,
        cz: car.1,
        mpp: REGION / p.short(),
        follow: true,
    }
}

impl View {
    pub fn preset(which: Preset, b: [f64; 4], p: Panel, car: (f64, f64)) -> View {
        match which {
            Preset::Whole => whole(b, p, 24.0),
            Preset::Region => region(car, p),
        }
    }

    /// World (x, z) → panel CSS px.
    pub fn to_screen(&self, p: Panel, x: f64, z: f64) -> (f64, f64) {
        (
            p.w / 2.0 + (x - self.cx) / self.mpp,
            p.h / 2.0 + (z - self.cz) / self.mpp,
        )
    }

    /// Panel CSS px → world (x, z).
    pub fn to_world(&self, p: Panel, sx: f64, sy: f64) -> (f64, f64) {
        (
            self.cx + (sx - p.w / 2.0) * self.mpp,
            self.cz + (sy - p.h / 2.0) * self.mpp,
        )
    }

    /// The scale's limits for the box in the panel.
    pub fn limits(b: [f64; 4], p: Panel) -> (f64, f64) {
        let most = whole(b, p, 24.0).mpp * OUT;
        let least = (CLOSEST / p.short()).min(most);
        (least, most)
    }

    /// Zooms by `factor` (above 1 zooms in) about the panel point (sx,
    /// sy), which stays over the same place; the scale held to its
    /// limits. A view following the car zooms about the car.
    pub fn zoom(&mut self, factor: f64, at: (f64, f64), b: [f64; 4], p: Panel) {
        let (least, most) = View::limits(b, p);
        let mpp = (self.mpp / factor.max(1e-6)).clamp(least, most);
        if !self.follow {
            let (wx, wz) = self.to_world(p, at.0, at.1);
            self.cx = wx - (at.0 - p.w / 2.0) * mpp;
            self.cz = wz - (at.1 - p.h / 2.0) * mpp;
        }
        self.mpp = mpp;
        self.keep_in(b);
    }

    /// Drags the map by (dx, dy) CSS px: it stops following the car.
    pub fn pan(&mut self, dx: f64, dy: f64, b: [f64; 4]) {
        self.cx -= dx * self.mpp;
        self.cz -= dy * self.mpp;
        self.follow = false;
        self.keep_in(b);
    }

    /// Follows the car when it does.
    pub fn track(&mut self, car: (f64, f64)) {
        if self.follow {
            self.cx = car.0;
            self.cz = car.1;
        }
    }

    /// The centre stays over the box.
    fn keep_in(&mut self, b: [f64; 4]) {
        if !self.follow {
            self.cx = self.cx.clamp(b[0], b[2]);
            self.cz = self.cz.clamp(b[1], b[3]);
        }
    }

    /// Which preset it is nearest, by scale.
    pub fn nearest(&self, b: [f64; 4], p: Panel) -> Preset {
        let w = whole(b, p, 24.0).mpp;
        let r = REGION / p.short();
        if (self.mpp / w).ln().abs() <= (self.mpp / r).ln().abs() {
            Preset::Whole
        } else {
            Preset::Region
        }
    }
}

/// A scale bar's length in metres (1, 2 or 5 times a power of ten) and
/// CSS px, about `want` px long.
pub fn scale_bar(mpp: f64, want: f64) -> (f64, f64) {
    let target = want * mpp;
    let p = 10f64.powf(target.log10().floor());
    let m = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|k| k * p)
        .min_by(|a, b| (a - target).abs().total_cmp(&(b - target).abs()))
        .unwrap_or(p);
    (m, m / mpp)
}

/// A scale bar's text: "500 m", "2 km", or miles and feet.
pub fn scale_text(m: f64, mph: bool) -> String {
    if mph {
        let ft = m * 3.28084;
        if ft < 5280.0 * 0.5 {
            format!("{:.0} ft", ft)
        } else {
            let mi = ft / 5280.0;
            if mi < 10.0 {
                format!("{mi:.1} mi")
            } else {
                format!("{mi:.0} mi")
            }
        }
    } else if m < 1000.0 {
        format!("{m:.0} m")
    } else {
        format!("{:.0} km", m / 1000.0)
    }
}

/// A label's box, CSS px: centre x, top y, width, height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Places labels at panel points (`at`, each `w` by `h` CSS px, centred
/// above its point) in order, skipping one that would overlap a placed
/// one or a box to keep clear (`avoid`: left, top, w, h; the player's
/// arrow), or leave the panel (with `margin` inside its edge).
pub fn place(
    at: &[(f64, f64, f64, f64)],
    avoid: &[(f64, f64, f64, f64)],
    p: Panel,
    margin: f64,
) -> Vec<Placed> {
    let mut out: Vec<Placed> = Vec::new();
    for (index, &(x, y, w, h)) in at.iter().enumerate() {
        let (l, t) = (x - w / 2.0, y - h - 4.0);
        if l < margin || t < margin || l + w > p.w - margin || t + h > p.h - margin {
            continue;
        }
        let clash = |ol: f64, ot: f64, ow: f64, oh: f64| {
            l < ol + ow + 6.0 && ol < l + w + 6.0 && t < ot + oh + 2.0 && ot < t + h + 2.0
        };
        let hit = out.iter().any(|o| clash(o.x - o.w / 2.0, o.y, o.w, o.h))
            || avoid.iter().any(|a| clash(a.0, a.1, a.2, a.3));
        if !hit {
            out.push(Placed {
                index,
                x,
                y: t,
                w,
                h,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: [f64; 4] = [-5000.0, -20000.0, 15000.0, 10000.0];
    const P: Panel = Panel {
        w: 1000.0,
        h: 600.0,
    };

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
    }

    #[test]
    fn the_whole_map_fits_the_box_in_the_panel() {
        let v = whole(B, P, 24.0);
        // 30 km tall into 552 px.
        assert!((v.mpp - 30000.0 / 552.0).abs() < 1e-9);
        let (x0, y0) = v.to_screen(P, B[0], B[1]);
        let (x1, y1) = v.to_screen(P, B[2], B[3]);
        assert!(y0 >= 24.0 - 1e-9 && y1 <= 576.0 + 1e-9 && x0 > 24.0 && x1 < 976.0);
        // North (−z) is up.
        assert!(y0 < y1);
        assert!(close(v.to_world(P, 500.0, 300.0), (5000.0, -5000.0)));
    }

    #[test]
    fn the_region_is_three_km_across_on_the_car_and_follows_it() {
        let mut v = region((100.0, 200.0), P);
        assert!(close(v.to_screen(P, 100.0, 200.0), (500.0, 300.0)));
        assert!((v.mpp * 600.0 - REGION).abs() < 1e-9);
        v.track((300.0, 400.0));
        assert!(close(v.to_screen(P, 300.0, 400.0), (500.0, 300.0)));
        // A drag lets go of the car.
        v.pan(50.0, 0.0, B);
        assert!(!v.follow);
        v.track((0.0, 0.0));
        assert!((v.cx - (300.0 - 50.0 * v.mpp)).abs() < 1e-9);
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor_and_holds_its_limits() {
        let mut v = whole(B, P, 24.0);
        let at = (800.0, 100.0);
        let w = v.to_world(P, at.0, at.1);
        v.zoom(2.0, at, B, P);
        assert!(close(v.to_world(P, at.0, at.1), w));
        let (least, most) = View::limits(B, P);
        for _ in 0..40 {
            v.zoom(2.0, at, B, P);
        }
        assert!((v.mpp - least).abs() < 1e-12);
        assert!((least * 600.0 - CLOSEST).abs() < 1e-9);
        for _ in 0..40 {
            v.zoom(0.5, at, B, P);
        }
        assert!((v.mpp - most).abs() < 1e-12);
        // The centre never leaves the box.
        v.pan(1e6, 1e6, B);
        assert!(v.cx >= B[0] && v.cz >= B[1]);
        // Following the car, the car stays in the middle.
        let mut v = region((100.0, 200.0), P);
        v.zoom(2.0, at, B, P);
        assert!(v.follow && close((v.cx, v.cz), (100.0, 200.0)));
        assert_eq!(v.nearest(B, P), Preset::Region);
        assert_eq!(whole(B, P, 24.0).nearest(B, P), Preset::Whole);
    }

    #[test]
    fn scale_bars_are_round_numbers() {
        assert_eq!(scale_bar(10.0, 100.0), (1000.0, 100.0));
        let (m, px) = scale_bar(3.0, 100.0);
        assert_eq!(m, 200.0);
        assert!((px - 200.0 / 3.0).abs() < 1e-9);
        assert_eq!(scale_text(500.0, false), "500 m");
        assert_eq!(scale_text(2000.0, false), "2 km");
        assert_eq!(scale_text(1609.344, true), "1.0 mi");
        assert_eq!(scale_text(100.0, true), "328 ft");
    }

    #[test]
    fn labels_do_not_overlap_or_leave_the_panel() {
        let at = [
            (500.0, 300.0, 100.0, 16.0),
            // Overlaps the first.
            (520.0, 305.0, 100.0, 16.0),
            // Off the left edge.
            (20.0, 300.0, 100.0, 16.0),
            (500.0, 200.0, 100.0, 16.0),
        ];
        let got: Vec<usize> = place(&at, &[], P, 8.0).iter().map(|p| p.index).collect();
        assert_eq!(got, vec![0, 3]);
        // Kept off the player's arrow.
        let got: Vec<usize> = place(&at, &[(480.0, 270.0, 40.0, 40.0)], P, 8.0)
            .iter()
            .map(|p| p.index)
            .collect();
        assert_eq!(got, vec![3]);
    }
}
