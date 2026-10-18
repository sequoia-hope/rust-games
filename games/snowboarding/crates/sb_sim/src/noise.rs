//! Deterministic hashing, a small seeded generator, and value noise: the
//! mountain's bumps and the placement of trees come from these, so a course
//! is the same on every platform and every run.

/// A 32-bit integer hash (lowbias32).
pub fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn hash2(x: i32, y: i32, seed: u32) -> u32 {
    hash((x as u32).wrapping_mul(0x9e37_79b9) ^ hash((y as u32) ^ seed.wrapping_mul(0x85eb_ca6b)))
}

/// A unit float in [0, 1) from a hash.
pub fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Smooth value noise in [-1, 1].
pub fn value2(x: f32, y: f32, seed: u32) -> f32 {
    let (xf, yf) = (x.floor(), y.floor());
    let (xi, yi) = (xf as i32, yf as i32);
    let (tx, ty) = (x - xf, y - yf);
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let v = |dx: i32, dy: i32| unit(hash2(xi + dx, yi + dy, seed)) * 2.0 - 1.0;
    let a = v(0, 0) + (v(1, 0) - v(0, 0)) * sx;
    let b = v(0, 1) + (v(1, 1) - v(0, 1)) * sx;
    a + (b - a) * sy
}

/// Fractal value noise: `octaves` layers, each twice the frequency and half
/// the amplitude of the last. Roughly in [-1, 1].
pub fn fbm(x: f32, y: f32, seed: u32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for o in 0..octaves {
        sum += value2(x * freq, y * freq, seed.wrapping_add(o * 1013)) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.03;
    }
    sum / norm
}

/// A small seeded generator (splitmix32-style) for placement and the CPU
/// riders' choices.
#[derive(Clone, Debug)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(hash(seed ^ 0xa511_e9b3))
    }
    pub fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x9e37_79b9);
        hash(self.0)
    }
    /// In [0, 1).
    pub fn f(&mut self) -> f32 {
        unit(self.next_u32())
    }
    /// In [a, b).
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// An angle wrapped into (-π, π].
pub fn wrap_angle(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let mut a = (a + PI).rem_euclid(TAU) - PI;
    if a <= -PI {
        a += TAU;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_bounded_and_deterministic() {
        for i in 0..1000 {
            let x = i as f32 * 0.37;
            let v = fbm(x, x * 0.7, 7, 4);
            assert!((-1.0..=1.0).contains(&v));
            assert_eq!(v, fbm(x, x * 0.7, 7, 4));
        }
    }

    #[test]
    fn wraps_angles() {
        assert!((wrap_angle(3.0 * std::f32::consts::PI) - std::f32::consts::PI).abs() < 1e-5);
        assert!(wrap_angle(-0.5).abs() - 0.5 < 1e-6);
    }
}
