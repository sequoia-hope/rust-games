//! FNV-1a 64, the hash every lockstep check uses: a tick's trace record, a
//! state's fingerprint on the wire, a recording's checkpoints. Moved from
//! Midnight Playground's `mp_sim::trace` with the same bytes in and out, so
//! every hash the game has recorded still matches.

/// FNV-1a 64 over `bytes` (offset basis `0xcbf29ce484222325`, prime
/// `0x100000001b3`).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// A small little-endian byte writer for building the bytes a hash or a
/// snapshot covers: `f64`s as their bits, integers as-is. The order of the
/// writes is the format.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bytes {
    pub buf: Vec<u8>,
}

impl Bytes {
    pub fn new() -> Bytes {
        Bytes::default()
    }
    pub fn f64(&mut self, x: f64) {
        self.buf.extend_from_slice(&x.to_bits().to_le_bytes());
    }
    pub fn f32(&mut self, x: f32) {
        self.buf.extend_from_slice(&x.to_bits().to_le_bytes());
    }
    pub fn u8(&mut self, x: u8) {
        self.buf.push(x);
    }
    pub fn i32(&mut self, x: i32) {
        self.buf.extend_from_slice(&x.to_le_bytes());
    }
    pub fn u32(&mut self, x: u32) {
        self.buf.extend_from_slice(&x.to_le_bytes());
    }
    pub fn u64(&mut self, x: u64) {
        self.buf.extend_from_slice(&x.to_le_bytes());
    }
    pub fn bool(&mut self, x: bool) {
        self.u8(u8::from(x));
    }
    /// An optional number: a presence byte, then the value when present.
    pub fn opt_f64(&mut self, x: Option<f64>) {
        match x {
            Some(v) => {
                self.u8(1);
                self.f64(v);
            }
            None => self.u8(0),
        }
    }
    /// The FNV-1a 64 of everything written so far.
    pub fn hash(&self) -> u64 {
        fnv1a64(&self.buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The published FNV-1a 64 test vectors, so a moved copy cannot drift.
    #[test]
    fn known_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x85944171f73967e8);
    }

    #[test]
    fn bytes_writes_little_endian_bits() {
        let mut b = Bytes::new();
        b.f64(1.0);
        b.i32(-1);
        b.bool(true);
        b.opt_f64(None);
        assert_eq!(
            b.buf,
            [0, 0, 0, 0, 0, 0, 0xf0, 0x3f, 0xff, 0xff, 0xff, 0xff, 1, 0]
        );
        assert_eq!(b.hash(), fnv1a64(&b.buf));
    }
}
