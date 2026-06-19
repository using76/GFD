//! ESRI ASCII grid (`.asc` / Arc/Info ASCIIGRID) reader — the simplest, most
//! universal DEM exchange format and our first terrain-import target.
//!
//! Layout: up to 6 header lines (`key value`, case-insensitive), then
//! `nrows × ncols` elevation values, whitespace-separated, **top (north) row
//! first**. Supports corner (`xllcorner`/`yllcorner`) or center
//! (`xllcenter`/`yllcenter`) origins; `NODATA_value` is optional (default −9999).

use std::path::Path;

use super::Dem;
use crate::{GeoError, GeoResult};

fn is_header_key(t: &str) -> bool {
    matches!(
        t.to_ascii_lowercase().as_str(),
        "ncols" | "nrows" | "xllcorner" | "yllcorner" | "xllcenter" | "yllcenter"
            | "cellsize" | "dx" | "dy" | "nodata_value" | "nodata"
    )
}

/// Parse an ESRI ASCII grid from a string.
pub fn parse_asc(text: &str) -> GeoResult<Dem> {
    let mut ncols: Option<usize> = None;
    let mut nrows: Option<usize> = None;
    let mut cellsize: Option<f64> = None;
    let mut xll: Option<f64> = None;
    let mut yll: Option<f64> = None;
    let mut x_is_center = false;
    let mut y_is_center = false;
    let mut nodata = -9999.0_f64;

    let mut tokens = text.split_whitespace().peekable();
    // Header: consume `key value` pairs while the next token is a known key.
    while let Some(&tok) = tokens.peek() {
        if !is_header_key(tok) {
            break;
        }
        let key = tokens.next().unwrap().to_ascii_lowercase();
        let val = tokens.next().ok_or_else(|| GeoError::Parse(format!("header '{key}' missing value")))?;
        match key.as_str() {
            "ncols" => ncols = Some(val.parse().map_err(|_| GeoError::Parse(format!("bad ncols '{val}'")))?),
            "nrows" => nrows = Some(val.parse().map_err(|_| GeoError::Parse(format!("bad nrows '{val}'")))?),
            "cellsize" | "dx" => cellsize = Some(val.parse().map_err(|_| GeoError::Parse(format!("bad cellsize '{val}'")))?),
            "dy" => {} // assume square cells; dy ignored
            "xllcorner" => xll = Some(val.parse().map_err(|_| GeoError::Parse(format!("bad xllcorner '{val}'")))?),
            "yllcorner" => yll = Some(val.parse().map_err(|_| GeoError::Parse(format!("bad yllcorner '{val}'")))?),
            "xllcenter" => { xll = Some(val.parse().map_err(|_| GeoError::Parse(format!("bad xllcenter '{val}'")))?); x_is_center = true; }
            "yllcenter" => { yll = Some(val.parse().map_err(|_| GeoError::Parse(format!("bad yllcenter '{val}'")))?); y_is_center = true; }
            "nodata_value" | "nodata" => nodata = val.parse().map_err(|_| GeoError::Parse(format!("bad nodata '{val}'")))?,
            _ => {}
        }
    }

    let ncols = ncols.ok_or_else(|| GeoError::Parse("missing ncols".into()))?;
    let nrows = nrows.ok_or_else(|| GeoError::Parse("missing nrows".into()))?;
    let cellsize = cellsize.ok_or_else(|| GeoError::Parse("missing cellsize".into()))?;
    let mut xll = xll.ok_or_else(|| GeoError::Parse("missing xllcorner/xllcenter".into()))?;
    let mut yll = yll.ok_or_else(|| GeoError::Parse("missing yllcorner/yllcenter".into()))?;
    if cellsize <= 0.0 {
        return Err(GeoError::Parse(format!("cellsize must be > 0 (got {cellsize})")));
    }
    // Normalize a center origin to the lower-left corner.
    if x_is_center { xll -= cellsize * 0.5; }
    if y_is_center { yll -= cellsize * 0.5; }

    let n = ncols * nrows;
    let mut z = Vec::with_capacity(n);
    for _ in 0..n {
        let t = tokens.next().ok_or_else(|| GeoError::Parse(format!("expected {n} values, got {}", z.len())))?;
        z.push(t.parse::<f64>().map_err(|_| GeoError::Parse(format!("bad grid value '{t}'")))?);
    }

    Dem::new(ncols, nrows, cellsize, [xll, yll], nodata, z)
        .ok_or_else(|| GeoError::Parse("invalid grid dimensions".into()))
}

/// Read an ESRI ASCII grid DEM from a file.
pub fn read_asc(path: &Path) -> GeoResult<Dem> {
    let text = std::fs::read_to_string(path)?;
    parse_asc(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
ncols 3
nrows 2
xllcorner 500000.0
yllcorner 4000000.0
cellsize 10.0
NODATA_value -9999
1 2 3
4 5 6";

    #[test]
    fn parses_header_and_grid() {
        let d = parse_asc(SAMPLE).unwrap();
        assert_eq!((d.ncols, d.nrows), (3, 2));
        assert_eq!(d.cellsize, 10.0);
        assert_eq!(d.origin, [500000.0, 4000000.0]);
        assert_eq!(d.nodata, -9999.0);
        // Top (north) row first: z[0..3] = 1,2,3 ; z[3..6] = 4,5,6.
        assert_eq!(d.z, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        // Top-left cell value via at(col=0,row=0).
        assert_eq!(d.at(0, 0), Some(1.0));
        assert_eq!(d.at(2, 1), Some(6.0));
    }

    #[test]
    fn cell_center_and_bounds() {
        let d = parse_asc(SAMPLE).unwrap();
        // Bounds: x 5e5..5e5+30, y 4e6..4e6+20.
        assert_eq!(d.bounds(), [500000.0, 4000000.0, 500030.0, 4000020.0]);
        // Bottom-left cell (col 0, row 1, the '4') center = (5,5) above origin.
        let c = d.cell_center(0, 1);
        assert!((c[0] - 500005.0).abs() < 1e-9 && (c[1] - 4000005.0).abs() < 1e-9);
    }

    #[test]
    fn bilinear_sample_at_cell_centers_returns_cell_values() {
        let d = parse_asc(SAMPLE).unwrap();
        // Sampling at a cell center returns that cell's value.
        let c = d.cell_center(1, 0); // the '2'
        assert!((d.sample(c[0], c[1]).unwrap() - 2.0).abs() < 1e-9);
        let c = d.cell_center(2, 1); // the '6'
        assert!((d.sample(c[0], c[1]).unwrap() - 6.0).abs() < 1e-9);
        // Midpoint between the '1'(NW) and '2' cells ≈ 1.5.
        let a = d.cell_center(0, 0);
        let b = d.cell_center(1, 0);
        let mid = d.sample((a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5).unwrap();
        assert!((mid - 1.5).abs() < 1e-9, "mid={mid}");
        // Outside the grid → None.
        assert!(d.sample(499000.0, 4000000.0).is_none());
    }

    #[test]
    fn handles_center_origin_and_missing_nodata() {
        let txt = "ncols 2\nnrows 1\nxllcenter 105.0\nyllcenter 205.0\ncellsize 10.0\n7 8";
        let d = parse_asc(txt).unwrap();
        // center→corner: 105 - 5 = 100.
        assert_eq!(d.origin, [100.0, 200.0]);
        assert_eq!(d.nodata, -9999.0); // default
        assert_eq!(d.z, vec![7.0, 8.0]);
    }

    #[test]
    fn to_bed_field_walls_off_nodata() {
        let txt = "ncols 2\nnrows 1\nxllcorner 0\nyllcorner 0\ncellsize 5\nNODATA_value -9999\n3 -9999";
        let d = parse_asc(txt).unwrap();
        let (nc, nr, cs, zb) = d.to_bed_field(None);
        assert_eq!((nc, nr, cs), (2, 1, 5.0));
        assert_eq!(zb[0], 3.0);
        assert!(zb[1] > 1000.0, "NODATA cell should become a high wall, got {}", zb[1]);
    }

    #[test]
    fn nodata_excluded_from_range_and_downsample() {
        let txt = "ncols 2\nnrows 2\nxllcorner 0\nyllcorner 0\ncellsize 1\nNODATA_value -9999\n10 -9999\n20 30";
        let d = parse_asc(txt).unwrap();
        assert_eq!(d.elevation_range(), Some((10.0, 30.0)));
        // 2×2 → 1×1 block average over the 3 valid cells = 20.
        let ds = d.downsample(2).unwrap();
        assert_eq!((ds.ncols, ds.nrows), (1, 1));
        assert!((ds.z[0] - 20.0).abs() < 1e-9);
    }
}
