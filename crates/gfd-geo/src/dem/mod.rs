//! Digital Elevation Model (DEM) — a regular raster of terrain heights, plus a
//! bilinear world-coordinate sampler and crop/downsample helpers.
//!
//! Storage convention matches the ESRI ASCII grid on disk: `z` is **row-major
//! from the top (north) row down**, so `z[0]` is the NW cell and row `j`'s world
//! y decreases as `j` increases. World coordinates use the cell-corner origin
//! `(xll, yll)` of the lower-left corner.

pub mod asc;

pub use asc::{parse_asc, read_asc};

/// A regular grid of terrain elevations in a projected CRS (units = `z`'s units,
/// usually metres). `origin` is the lower-left **corner** (xllcorner, yllcorner).
#[derive(Debug, Clone)]
pub struct Dem {
    pub ncols: usize,
    pub nrows: usize,
    pub cellsize: f64,
    /// Lower-left corner (xllcorner, yllcorner).
    pub origin: [f64; 2],
    /// Sentinel for missing data (cells equal to this are "no data").
    pub nodata: f64,
    /// Elevations, row-major, top (north) row first. `len == ncols * nrows`.
    pub z: Vec<f64>,
}

impl Dem {
    /// Construct from raw parts, validating the grid size.
    pub fn new(ncols: usize, nrows: usize, cellsize: f64, origin: [f64; 2], nodata: f64, z: Vec<f64>) -> Option<Self> {
        if ncols == 0 || nrows == 0 || cellsize <= 0.0 || z.len() != ncols * nrows {
            return None;
        }
        Some(Self { ncols, nrows, cellsize, origin, nodata, z })
    }

    #[inline]
    fn idx(&self, col: usize, row: usize) -> usize {
        row * self.ncols + col
    }

    /// True if a value is the NODATA sentinel (within a small tolerance).
    #[inline]
    pub fn is_nodata(&self, v: f64) -> bool {
        !v.is_finite() || (v - self.nodata).abs() <= 1e-6 * self.nodata.abs().max(1.0)
    }

    /// Elevation at grid index (col, row), or None if out of range / NODATA.
    pub fn at(&self, col: usize, row: usize) -> Option<f64> {
        if col >= self.ncols || row >= self.nrows {
            return None;
        }
        let v = self.z[self.idx(col, row)];
        if self.is_nodata(v) { None } else { Some(v) }
    }

    /// World extent `[xmin, ymin, xmax, ymax]` (corner-based).
    pub fn bounds(&self) -> [f64; 4] {
        [
            self.origin[0],
            self.origin[1],
            self.origin[0] + self.ncols as f64 * self.cellsize,
            self.origin[1] + self.nrows as f64 * self.cellsize,
        ]
    }

    /// Min / max elevation over valid (non-NODATA) cells.
    pub fn elevation_range(&self) -> Option<(f64, f64)> {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for &v in &self.z {
            if !self.is_nodata(v) {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        if lo <= hi { Some((lo, hi)) } else { None }
    }

    /// World coordinates of cell (col, row)'s **center**.
    pub fn cell_center(&self, col: usize, row: usize) -> [f64; 2] {
        // row 0 is the top (max y); world y decreases with row.
        let x = self.origin[0] + (col as f64 + 0.5) * self.cellsize;
        let y = self.origin[1] + (self.nrows as f64 - 1.0 - row as f64 + 0.5) * self.cellsize;
        [x, y]
    }

    /// Bilinear-interpolated elevation at world (x, y). Returns None outside the
    /// grid; falls back to the nearest valid cell-center value when a bilinear
    /// stencil straddles NODATA, and None only if no neighbour is valid.
    pub fn sample(&self, x: f64, y: f64) -> Option<f64> {
        // Continuous grid coordinates in (col, row) space, cell-center aligned.
        let fx = (x - self.origin[0]) / self.cellsize - 0.5;
        // Invert the y flip: row increases downward (south).
        let fy = (self.nrows as f64 - 1.0) - ((y - self.origin[1]) / self.cellsize - 0.5);
        if fx < -0.5 || fy < -0.5 || fx > self.ncols as f64 - 0.5 || fy > self.nrows as f64 - 0.5 {
            return None;
        }
        let c0 = fx.floor().clamp(0.0, self.ncols as f64 - 1.0) as usize;
        let r0 = fy.floor().clamp(0.0, self.nrows as f64 - 1.0) as usize;
        let c1 = (c0 + 1).min(self.ncols - 1);
        let r1 = (r0 + 1).min(self.nrows - 1);
        let tx = (fx - c0 as f64).clamp(0.0, 1.0);
        let ty = (fy - r0 as f64).clamp(0.0, 1.0);
        let w = [(1.0 - tx) * (1.0 - ty), tx * (1.0 - ty), (1.0 - tx) * ty, tx * ty];
        let vals = [self.at(c0, r0), self.at(c1, r0), self.at(c0, r1), self.at(c1, r1)];
        if vals.iter().all(|v| v.is_some()) {
            let mut acc = 0.0;
            for k in 0..4 {
                acc += vals[k].unwrap() * w[k];
            }
            return Some(acc);
        }
        // NODATA in the stencil → nearest valid corner by weight.
        let mut best: Option<(f64, f64)> = None; // (weight, value)
        for k in 0..4 {
            if let Some(v) = vals[k] {
                if best.map_or(true, |(bw, _)| w[k] > bw) {
                    best = Some((w[k], v));
                }
            }
        }
        best.map(|(_, v)| v)
    }

    /// Phase 4 bridge — flatten this DEM into SWE-solver inputs: the grid dims,
    /// cell size, and a row-major bed-elevation field `z_b` (same layout as `z`).
    /// NODATA cells become an impermeable wall by setting their bed far above the
    /// terrain (`nodata_height`, default = max elevation + 1000 m), so the
    /// well-balanced reconstruction yields zero flux there. Returns
    /// `(ncols, nrows, cellsize, z_b)`; the caller builds `SwGrid`/`SwState`
    /// (keeps `gfd-fluid` decoupled from `gfd-geo`).
    pub fn to_bed_field(&self, nodata_height: Option<f64>) -> (usize, usize, f64, Vec<f64>) {
        let wall = nodata_height.unwrap_or_else(|| self.elevation_range().map_or(1.0e6, |(_, hi)| hi + 1000.0));
        let z_b = self.z.iter().map(|&v| if self.is_nodata(v) { wall } else { v }).collect();
        (self.ncols, self.nrows, self.cellsize, z_b)
    }

    /// Crop to a sub-window `[col0, row0]` of size `nc × nr` (clamped to range).
    pub fn crop(&self, col0: usize, row0: usize, nc: usize, nr: usize) -> Option<Dem> {
        if col0 >= self.ncols || row0 >= self.nrows || nc == 0 || nr == 0 {
            return None;
        }
        let nc = nc.min(self.ncols - col0);
        let nr = nr.min(self.nrows - row0);
        let mut z = Vec::with_capacity(nc * nr);
        for r in row0..row0 + nr {
            for c in col0..col0 + nc {
                z.push(self.z[self.idx(c, r)]);
            }
        }
        // New lower-left corner: x shifts right by col0; y shifts up because the
        // cropped window's bottom row is `row0 + nr - 1` from the top.
        let bottom_rows_removed = self.nrows - (row0 + nr);
        let origin = [
            self.origin[0] + col0 as f64 * self.cellsize,
            self.origin[1] + bottom_rows_removed as f64 * self.cellsize,
        ];
        Dem::new(nc, nr, self.cellsize, origin, self.nodata, z)
    }

    /// Block-average downsample by an integer `factor` (≥2). NODATA cells are
    /// excluded from each block's average; an all-NODATA block stays NODATA.
    pub fn downsample(&self, factor: usize) -> Option<Dem> {
        if factor < 2 {
            return Some(self.clone());
        }
        let nc = self.ncols / factor;
        let nr = self.nrows / factor;
        if nc == 0 || nr == 0 {
            return None;
        }
        let mut z = Vec::with_capacity(nc * nr);
        for r in 0..nr {
            for c in 0..nc {
                let mut sum = 0.0;
                let mut cnt = 0usize;
                for dr in 0..factor {
                    for dc in 0..factor {
                        let v = self.z[self.idx(c * factor + dc, r * factor + dr)];
                        if !self.is_nodata(v) {
                            sum += v;
                            cnt += 1;
                        }
                    }
                }
                z.push(if cnt > 0 { sum / cnt as f64 } else { self.nodata });
            }
        }
        // Downsample keeps the lower-left corner; the trimmed top/right edges
        // (from integer division) drop a partial block — acceptable for a coarse
        // domain. cellsize grows by `factor`.
        Dem::new(nc, nr, self.cellsize * factor as f64, self.origin, self.nodata, z)
    }
}
