//! The ground under a corridor: a grid of real heights in game metres,
//! written by `tools/atlas/ground.py` (docs/vision/peninsula.md, stage 2)
//! and included at compile time by the level that drives on it.
//!
//! Little-endian: `b"MPGROUND"`, u32 width, u32 height, f32 step, f32 x0,
//! f32 z0 (game metres; cell (0, 0)'s centre, the north-west corner), u32
//! length of the zlib stream, then zlib of i16 heights in decimetres, row by
//! row from the north.

/// A height grid with bilinear sampling, clamped at its edges.
#[derive(Clone, Debug, PartialEq)]
pub struct HeightGrid {
    pub x0: f64,
    pub z0: f64,
    pub step: f64,
    pub w: usize,
    pub h: usize,
    /// Heights in metres, row by row from the north.
    pub values: Vec<f32>,
}

impl HeightGrid {
    pub fn parse(bytes: &[u8]) -> Result<HeightGrid, String> {
        if bytes.len() < 32 || &bytes[0..8] != b"MPGROUND" {
            return Err("ground: not an MPGROUND file".to_owned());
        }
        let u32_at =
            |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let f32_at =
            |o: usize| f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let w = u32_at(8) as usize;
        let h = u32_at(12) as usize;
        let step = f64::from(f32_at(16));
        let x0 = f64::from(f32_at(20));
        let z0 = f64::from(f32_at(24));
        let len = u32_at(28) as usize;
        if w < 2
            || h < 2
            || step.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)
            || w.checked_mul(h).is_none()
        {
            return Err("ground: bad grid size".to_owned());
        }
        let stream = bytes
            .get(32..32 + len)
            .ok_or_else(|| "ground: short file".to_owned())?;
        let raw = miniz_oxide::inflate::decompress_to_vec_zlib(stream)
            .map_err(|e| format!("ground: inflating: {e:?}"))?;
        if raw.len() != w * h * 2 {
            return Err(format!(
                "ground: {} bytes of heights for {w} x {h} cells",
                raw.len()
            ));
        }
        let values = raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| f32::from(i16::from_le_bytes(*c)) / 10.0)
            .collect();
        Ok(HeightGrid {
            x0,
            z0,
            step,
            w,
            h,
            values,
        })
    }

    /// The height at (x, z), bilinear between the cells' centres, the
    /// edges' values carried on outside the grid.
    pub fn height(&self, x: f64, z: f64) -> f64 {
        let (w, h) = (self.w as f64, self.h as f64);
        let gx = ((x - self.x0) / self.step).clamp(0.0, w - 1.001);
        let gz = ((z - self.z0) / self.step).clamp(0.0, h - 1.001);
        let (i, j) = (gx.floor(), gz.floor());
        let (tx, tz) = (gx - i, gz - j);
        let k = j as usize * self.w + i as usize;
        let v = |k: usize| f64::from(self.values[k]);
        let a = v(k) + (v(k + 1) - v(k)) * tx;
        let b = v(k + self.w) + (v(k + self.w + 1) - v(k + self.w)) * tx;
        a + (b - a) * tz
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> HeightGrid {
        // 3 x 2 cells of 10 m from (0, 0): a slope rising east.
        let heights: [i16; 6] = [0, 100, 200, 0, 100, 200];
        let mut raw = Vec::new();
        for v in heights {
            raw.extend_from_slice(&v.to_le_bytes());
        }
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
        let mut b = b"MPGROUND".to_vec();
        b.extend_from_slice(&3u32.to_le_bytes());
        b.extend_from_slice(&2u32.to_le_bytes());
        b.extend_from_slice(&10f32.to_le_bytes());
        b.extend_from_slice(&0f32.to_le_bytes());
        b.extend_from_slice(&0f32.to_le_bytes());
        b.extend_from_slice(&(z.len() as u32).to_le_bytes());
        b.extend_from_slice(&z);
        HeightGrid::parse(&b).unwrap()
    }

    #[test]
    fn samples_bilinearly_and_clamps() {
        let g = grid();
        assert_eq!(g.height(0.0, 0.0), 0.0);
        assert!((g.height(5.0, 5.0) - 5.0).abs() < 1e-9);
        assert!((g.height(15.0, 3.0) - 15.0).abs() < 1e-9);
        assert!((g.height(-50.0, 100.0) - 0.0).abs() < 1e-9);
        assert!((g.height(500.0, -100.0) - 20.0).abs() < 0.05);
    }

    #[test]
    fn rejects_other_files() {
        assert!(HeightGrid::parse(b"MPATLAS1 not this").is_err());
    }
}

/// The land cover of the same cells (`<id>.cover.bin`, stage 4a): the
/// nearest planning-grid cell's class ([`mp_track::Cover`] as a byte), row
/// by row from the north. Little-endian: `b"MPCOVER1"`, u32 width, u32
/// height, f32 step, f32 x0, f32 z0, u32 zlib length, zlib of u8 classes.
#[derive(Clone, Debug, PartialEq)]
pub struct CoverGrid {
    pub x0: f64,
    pub z0: f64,
    pub step: f64,
    pub w: usize,
    pub h: usize,
    pub classes: Vec<u8>,
}

impl CoverGrid {
    pub fn parse(bytes: &[u8]) -> Result<CoverGrid, String> {
        if bytes.len() < 32 || &bytes[0..8] != b"MPCOVER1" {
            return Err("cover: not an MPCOVER1 file".to_owned());
        }
        let u32_at =
            |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let f32_at =
            |o: usize| f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let w = u32_at(8) as usize;
        let h = u32_at(12) as usize;
        let step = f64::from(f32_at(16));
        let x0 = f64::from(f32_at(20));
        let z0 = f64::from(f32_at(24));
        let len = u32_at(28) as usize;
        if w < 2
            || h < 2
            || step.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)
            || w.checked_mul(h).is_none()
        {
            return Err("cover: bad grid size".to_owned());
        }
        let stream = bytes
            .get(32..32 + len)
            .ok_or_else(|| "cover: short file".to_owned())?;
        let classes = miniz_oxide::inflate::decompress_to_vec_zlib(stream)
            .map_err(|e| format!("cover: inflating: {e:?}"))?;
        if classes.len() != w * h {
            return Err(format!(
                "cover: {} bytes of classes for {w} x {h} cells",
                classes.len()
            ));
        }
        Ok(CoverGrid {
            x0,
            z0,
            step,
            w,
            h,
            classes,
        })
    }

    /// The class of the cell under (x, z), the edge cells carried on
    /// outside the grid.
    pub fn class(&self, x: f64, z: f64) -> u8 {
        let i = ((x - self.x0) / self.step)
            .round()
            .clamp(0.0, self.w as f64 - 1.0) as usize;
        let j = ((z - self.z0) / self.step)
            .round()
            .clamp(0.0, self.h as f64 - 1.0) as usize;
        self.classes[j * self.w + i]
    }
}

/// The city grid (`<id>.city.bin`, stage 6b, written by
/// `tools/atlas/city.py`): over the ground's box, in cells of its own step
/// (32 game metres), how much of the ground the real buildings cover and
/// how tall they stand. Heights are real metres: the map is compressed,
/// the buildings are not. Little-endian: `b"MPCITY01"`, u32 width, u32
/// height, f32 step, f32 x0, f32 z0 (cell (0, 0)'s centre, the north-west
/// corner), u32 zlib length, zlib of three u8 a cell, row by row from the
/// north: coverage (0 to 255 of the cell), then the median and the
/// ninetieth percentile height of the buildings in it, in units of
/// [`CityGrid::HEIGHT_UNIT`] metres.
#[derive(Clone, Debug, PartialEq)]
pub struct CityGrid {
    pub x0: f64,
    pub z0: f64,
    pub step: f64,
    pub w: usize,
    pub h: usize,
    /// Three bytes a cell: coverage, p50, p90.
    pub cells: Vec<u8>,
}

impl CityGrid {
    /// Real metres in a unit of the file's heights.
    pub const HEIGHT_UNIT: f64 = 2.0;

    pub fn parse(bytes: &[u8]) -> Result<CityGrid, String> {
        if bytes.len() < 32 || &bytes[0..8] != b"MPCITY01" {
            return Err("city: not an MPCITY01 file".to_owned());
        }
        let u32_at =
            |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let f32_at =
            |o: usize| f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let w = u32_at(8) as usize;
        let h = u32_at(12) as usize;
        let step = f64::from(f32_at(16));
        let x0 = f64::from(f32_at(20));
        let z0 = f64::from(f32_at(24));
        let len = u32_at(28) as usize;
        if w < 1
            || h < 1
            || step.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)
            || w.checked_mul(h).and_then(|n| n.checked_mul(3)).is_none()
        {
            return Err("city: bad grid size".to_owned());
        }
        let stream = bytes
            .get(32..32 + len)
            .ok_or_else(|| "city: short file".to_owned())?;
        let cells = miniz_oxide::inflate::decompress_to_vec_zlib(stream)
            .map_err(|e| format!("city: inflating: {e:?}"))?;
        if cells.len() != w * h * 3 {
            return Err(format!(
                "city: {} bytes of cells for {w} x {h} cells",
                cells.len()
            ));
        }
        Ok(CityGrid {
            x0,
            z0,
            step,
            w,
            h,
            cells,
        })
    }

    /// The cell holding (x, z), as the index of its first byte; None
    /// outside the grid.
    fn cell(&self, x: f64, z: f64) -> Option<usize> {
        let i = ((x - self.x0) / self.step).round();
        let j = ((z - self.z0) / self.step).round();
        if !(0.0..self.w as f64).contains(&i) || !(0.0..self.h as f64).contains(&j) {
            return None;
        }
        Some((j as usize * self.w + i as usize) * 3)
    }

    /// The share of the ground under buildings in the cell at (x, z), 0 to
    /// 1; None outside the grid.
    pub fn coverage(&self, x: f64, z: f64) -> Option<f64> {
        self.cell(x, z).map(|k| f64::from(self.cells[k]) / 255.0)
    }

    /// The median height (real metres) of the buildings in the cell at
    /// (x, z), by footprint area; 0 where there are none, None outside.
    pub fn height_p50(&self, x: f64, z: f64) -> Option<f64> {
        self.cell(x, z)
            .map(|k| f64::from(self.cells[k + 1]) * Self::HEIGHT_UNIT)
    }

    /// The ninetieth percentile height (real metres) of the buildings in
    /// the cell at (x, z), by footprint area; 0 where there are none, None
    /// outside.
    pub fn height_p90(&self, x: f64, z: f64) -> Option<f64> {
        self.cell(x, z)
            .map(|k| f64::from(self.cells[k + 2]) * Self::HEIGHT_UNIT)
    }
}

#[cfg(test)]
mod city_tests {
    use super::*;

    #[test]
    fn reads_the_nearest_cell_and_nothing_outside() {
        // 2 x 1 cells of 32 m from (100, 50): open ground, then a tower.
        let raw = [0u8, 0, 0, 128, 20, 150];
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
        let mut b = b"MPCITY01".to_vec();
        for v in [2u32, 1] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for v in [32f32, 100.0, 50.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&(z.len() as u32).to_le_bytes());
        b.extend_from_slice(&z);
        let g = CityGrid::parse(&b).unwrap();
        assert_eq!(g.coverage(110.0, 60.0), Some(0.0));
        assert_eq!(g.height_p90(110.0, 60.0), Some(0.0));
        let (x, z) = (140.0, 40.0);
        assert!((g.coverage(x, z).unwrap() - 128.0 / 255.0).abs() < 1e-12);
        assert_eq!(g.height_p50(x, z), Some(40.0));
        assert_eq!(g.height_p90(x, z), Some(300.0));
        assert_eq!(g.coverage(160.0, 50.0), None);
        assert_eq!(g.coverage(83.0, 50.0), None);
        assert_eq!(g.height_p50(140.0, 67.0), None);
        assert!(CityGrid::parse(b"MPCOVER1 not this file at all.....").is_err());
    }
}
