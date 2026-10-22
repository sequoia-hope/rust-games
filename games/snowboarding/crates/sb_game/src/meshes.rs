//! Building meshes by hand: positions, normals and vertex colours, with
//! flat or smooth shading. The trees, rocks, flags, gates and the rider's
//! board are made here rather than loaded, so the build has no asset files.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use std::f32::consts::TAU;

#[derive(Default)]
pub struct MeshBuilder {
    pub pos: Vec<[f32; 3]>,
    pub nor: Vec<[f32; 3]>,
    pub col: Vec<[f32; 4]>,
    pub idx: Vec<u32>,
}

/// A linear colour from sRGB components.
pub fn rgb(r: f32, g: f32, b: f32) -> [f32; 4] {
    let c = Color::srgb(r, g, b).to_linear();
    [c.red, c.green, c.blue, 1.0]
}

pub fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

impl MeshBuilder {
    pub fn vertex(&mut self, p: Vec3, n: Vec3, c: [f32; 4]) -> u32 {
        self.pos.push(p.into());
        self.nor.push(n.into());
        self.col.push(c);
        (self.pos.len() - 1) as u32
    }

    /// A flat-shaded triangle, counter-clockwise seen from its front.
    pub fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, col: [f32; 4]) {
        let n = (b - a).cross(c - a).normalize_or(Vec3::Y);
        let i = self.vertex(a, n, col);
        self.vertex(b, n, col);
        self.vertex(c, n, col);
        self.idx.extend([i, i + 1, i + 2]);
    }

    /// A flat-shaded triangle coloured by how much it faces up: `up` where
    /// it does (snow on top), `side` where it does not.
    pub fn tri_snowy(&mut self, a: Vec3, b: Vec3, c: Vec3, side: [f32; 4], up: [f32; 4], k: f32) {
        let n = (b - a).cross(c - a).normalize_or(Vec3::Y);
        let col = mix(side, up, ((n.y - 0.35) * k).clamp(0.0, 1.0));
        self.tri(a, b, c, col);
    }

    pub fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: [f32; 4]) {
        self.tri(a, b, c, col);
        self.tri(a, c, d, col);
    }

    /// An axis-aligned box about `center` with half extents `h`.
    pub fn cuboid(&mut self, center: Vec3, h: Vec3, col: [f32; 4]) {
        self.cuboid_rot(center, Quat::IDENTITY, h, col);
    }

    /// A box about `center`, rotated by `rot`, with half extents `h`.
    pub fn cuboid_rot(&mut self, center: Vec3, rot: Quat, h: Vec3, col: [f32; 4]) {
        let p = |x: f32, y: f32, z: f32| center + rot * Vec3::new(x * h.x, y * h.y, z * h.z);
        let faces = [
            [
                p(-1., -1., 1.),
                p(1., -1., 1.),
                p(1., 1., 1.),
                p(-1., 1., 1.),
            ],
            [
                p(1., -1., -1.),
                p(-1., -1., -1.),
                p(-1., 1., -1.),
                p(1., 1., -1.),
            ],
            [
                p(1., -1., 1.),
                p(1., -1., -1.),
                p(1., 1., -1.),
                p(1., 1., 1.),
            ],
            [
                p(-1., -1., -1.),
                p(-1., -1., 1.),
                p(-1., 1., 1.),
                p(-1., 1., -1.),
            ],
            [
                p(-1., 1., 1.),
                p(1., 1., 1.),
                p(1., 1., -1.),
                p(-1., 1., -1.),
            ],
            [
                p(-1., -1., -1.),
                p(1., -1., -1.),
                p(1., -1., 1.),
                p(-1., -1., 1.),
            ],
        ];
        for [a, b, c, d] in faces {
            self.quad(a, b, c, d, col);
        }
    }

    /// A cone (or a frustum, with `top_r` > 0) standing on `base`.
    #[allow(clippy::too_many_arguments)]
    pub fn cone(
        &mut self,
        base: Vec3,
        r: f32,
        top_r: f32,
        h: f32,
        seg: u32,
        side: [f32; 4],
        up: [f32; 4],
        jitter: f32,
        seed: u32,
    ) {
        let ring = |i: u32, radius: f32, y: f32| {
            let a = TAU * i as f32 / seg as f32;
            let j = 1.0 + jitter * (sb_sim::noise::unit(sb_sim::noise::hash(seed ^ i)) - 0.5);
            base + Vec3::new(a.cos() * radius * j, y, a.sin() * radius * j)
        };
        for i in 0..seg {
            let (a0, a1) = (ring(i, r, 0.0), ring(i + 1, r, 0.0));
            if top_r > 0.0 {
                let (b0, b1) = (ring(i, top_r, h), ring(i + 1, top_r, h));
                self.tri_snowy(a0, b1, a1, side, up, 2.0);
                self.tri_snowy(a0, b0, b1, side, up, 2.0);
                self.tri(b0, base + Vec3::Y * h, b1, up);
            } else {
                let apex = base + Vec3::Y * h;
                self.tri_snowy(a0, apex, a1, side, up, 2.2);
            }
            // The underside.
            self.tri(a0, a1, base, mix(side, [0.0, 0.0, 0.0, 1.0], 0.4));
        }
    }

    /// A low-poly lumpy ball (a rock).
    pub fn rock(&mut self, center: Vec3, r: f32, seed: u32, side: [f32; 4], up: [f32; 4]) {
        let t = (1.0 + 5f32.sqrt()) / 2.0;
        let mut v = [
            Vec3::new(-1., t, 0.),
            Vec3::new(1., t, 0.),
            Vec3::new(-1., -t, 0.),
            Vec3::new(1., -t, 0.),
            Vec3::new(0., -1., t),
            Vec3::new(0., 1., t),
            Vec3::new(0., -1., -t),
            Vec3::new(0., 1., -t),
            Vec3::new(t, 0., -1.),
            Vec3::new(t, 0., 1.),
            Vec3::new(-t, 0., -1.),
            Vec3::new(-t, 0., 1.),
        ];
        for (i, p) in v.iter_mut().enumerate() {
            let j = 0.75
                + 0.5 * sb_sim::noise::unit(sb_sim::noise::hash(seed.wrapping_add(i as u32 * 77)));
            *p = p.normalize() * r * j * Vec3::new(1.0, 0.7, 1.0);
        }
        const F: [[usize; 3]; 20] = [
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ];
        for [a, b, c] in F {
            self.tri_snowy(center + v[a], center + v[b], center + v[c], side, up, 2.5);
        }
    }

    pub fn build(self) -> Mesh {
        let mut m = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.nor);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
        m.insert_indices(Indices::U32(self.idx));
        m
    }

    /// A smooth-shaded triangle with explicit per-vertex normals and colours.
    pub fn tri_smooth(
        &mut self,
        a: (Vec3, Vec3, [f32; 4]),
        b: (Vec3, Vec3, [f32; 4]),
        c: (Vec3, Vec3, [f32; 4]),
    ) {
        let i = self.vertex(a.0, a.1, a.2);
        self.vertex(b.0, b.1, b.2);
        self.vertex(c.0, c.1, c.2);
        self.idx.extend([i, i + 1, i + 2]);
    }

    /// Lofts a smooth tube/garment through cross-section [`Ring`]s from
    /// bottom to top, with optional hemispherical end-caps so joints stay
    /// sealed at any bend angle.
    pub fn loft(&mut self, rings: &[Ring], seg: u32, cap_bot: bool, cap_top: bool) {
        if rings.len() < 2 || seg < 3 {
            return;
        }
        let sample = |r: &Ring, i: u32| -> (Vec3, Vec3, [f32; 4]) {
            let a = TAU * (i % seg) as f32 / seg as f32;
            let (ca, sa) = (a.cos(), a.sin());
            let wave = if r.freq > 0.0 {
                (a * r.freq).cos()
            } else {
                0.0
            };
            let fold = 1.0 + r.ripple * wave;
            let rot = Quat::from_rotation_z(r.tilt_z) * Quat::from_rotation_x(r.tilt_x);
            let local = Vec3::new(ca * r.rx * fold, 0.0, sa * r.rz * fold);
            let n_local = Vec3::new(
                ca / r.rx.max(1e-3) - r.ripple * r.freq * (a * r.freq).sin() * sa * 0.25,
                0.0,
                sa / r.rz.max(1e-3) + r.ripple * r.freq * (a * r.freq).sin() * ca * 0.25,
            )
            .normalize_or(Vec3::X);
            let front_t = ((ca + 0.15) * 1.4).clamp(0.0, 1.0);
            let mut col = mix(r.col, r.front_col, front_t);
            if r.ripple.abs() > 1e-3 && wave < 0.0 {
                col = mix(col, [0.0, 0.0, 0.0, 1.0], (-wave * 0.16).min(0.22));
            }
            (
                r.center + rot * local,
                (rot * n_local).normalize_or(Vec3::X),
                col,
            )
        };

        let nr = rings.len();
        for k in 0..nr - 1 {
            let (r0, r1) = (&rings[k], &rings[k + 1]);
            let dy = (r1.center.y - r0.center.y).max(1e-3);
            let dr = ((r0.rx - r1.rx) + (r0.rz - r1.rz)) * 0.5;
            let ny = (dr / dy).clamp(-0.8, 0.8);
            for i in 0..seg {
                let mut a0 = sample(r0, i);
                let mut a1 = sample(r0, i + 1);
                let mut b0 = sample(r1, i);
                let mut b1 = sample(r1, i + 1);
                for v in [&mut a0, &mut a1, &mut b0, &mut b1] {
                    v.1 = (v.1 + Vec3::Y * ny).normalize_or(v.1);
                }
                self.tri_smooth(a0, b1, a1);
                self.tri_smooth(a0, b0, b1);
            }
        }

        if cap_bot {
            let r = &rings[0];
            let h = (r.rx + r.rz) * 0.48;
            let rot = Quat::from_rotation_z(r.tilt_z) * Quat::from_rotation_x(r.tilt_x);
            let pole = (
                r.center - rot * (Vec3::Y * h),
                rot * Vec3::NEG_Y,
                mix(r.col, [0.0, 0.0, 0.0, 1.0], 0.12),
            );
            let mid_ring = Ring {
                center: r.center - rot * (Vec3::Y * (h * 0.55)),
                rx: r.rx * 0.78,
                rz: r.rz * 0.78,
                ripple: r.ripple * 0.4,
                ..*r
            };
            for i in 0..seg {
                let a0 = sample(&mid_ring, i);
                let a1 = sample(&mid_ring, i + 1);
                let b0 = sample(r, i);
                let b1 = sample(r, i + 1);
                self.tri_smooth(a0, b1, a1);
                self.tri_smooth(a0, b0, b1);
                self.tri_smooth(pole, a1, a0);
            }
        }

        if cap_top {
            let r = &rings[nr - 1];
            let h = (r.rx + r.rz) * 0.48;
            let rot = Quat::from_rotation_z(r.tilt_z) * Quat::from_rotation_x(r.tilt_x);
            let pole = (r.center + rot * (Vec3::Y * h), rot * Vec3::Y, r.col);
            let mid_ring = Ring {
                center: r.center + rot * (Vec3::Y * (h * 0.55)),
                rx: r.rx * 0.78,
                rz: r.rz * 0.78,
                ripple: r.ripple * 0.4,
                ..*r
            };
            for i in 0..seg {
                let a0 = sample(r, i);
                let a1 = sample(r, i + 1);
                let b0 = sample(&mid_ring, i);
                let b1 = sample(&mid_ring, i + 1);
                self.tri_smooth(a0, b1, a1);
                self.tri_smooth(a0, b0, b1);
                self.tri_smooth(b0, pole, b1);
            }
        }
    }
}

/// Cross-section slice for [`MeshBuilder::loft`].
#[derive(Clone, Copy)]
pub struct Ring {
    pub center: Vec3,
    pub rx: f32,
    pub rz: f32,
    pub tilt_z: f32,
    pub tilt_x: f32,
    pub col: [f32; 4],
    pub front_col: [f32; 4],
    pub ripple: f32,
    pub freq: f32,
}

impl Ring {
    pub fn new(y: f32, rx: f32, rz: f32, col: [f32; 4]) -> Self {
        Self {
            center: Vec3::Y * y,
            rx,
            rz,
            tilt_z: 0.0,
            tilt_x: 0.0,
            col,
            front_col: col,
            ripple: 0.0,
            freq: 0.0,
        }
    }
}
