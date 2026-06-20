//! Minimal single-band Float32 GeoTIFF writer (no external crates) for flood
//! raster export (FLOOD_PLAN Phase 9). Emits a little-endian baseline TIFF with
//! `ModelPixelScale` + `ModelTiepoint` + a `GeoKeyDirectory` (projected, metre
//! units) so the result is georeferenced and loads in QGIS / ArcGIS / GDAL.
//!
//! Row order: `values` is row-major **north row first** (matching the DEM/ESRI
//! convention), which is exactly TIFF's top-to-bottom raster order. `origin` is
//! the lower-left corner (xll, yll), so the top-left tiepoint is `yll + nrows·cs`.

use std::path::Path;

use crate::GeoResult;

const N_ENTRIES: usize = 13;

/// Write a georeferenced Float32 GeoTIFF. `values.len()` must be `ncols*nrows`.
pub fn write_geotiff_float32(
    path: &Path,
    ncols: usize,
    nrows: usize,
    cellsize: f64,
    origin: [f64; 2],
    values: &[f64],
) -> GeoResult<()> {
    assert_eq!(values.len(), ncols * nrows, "values length must equal ncols*nrows");

    // Layout offsets (header 8 + IFD, then out-of-line data, then pixels).
    let ifd_size = 2 + N_ENTRIES * 12 + 4;
    let ool = 8 + ifd_size;
    let pixscale_off = ool;
    let tiepoint_off = pixscale_off + 24; // 3 × f64
    let geokey_off = tiepoint_off + 48; // 6 × f64
    let img_off = geokey_off + 32; // 16 × u16
    let strip_bytes = ncols * nrows * 4;

    let mut b: Vec<u8> = Vec::with_capacity(img_off + strip_bytes);
    // --- TIFF header (little-endian) ---
    b.extend_from_slice(b"II");
    b.extend_from_slice(&42u16.to_le_bytes());
    b.extend_from_slice(&8u32.to_le_bytes()); // first IFD at offset 8

    // --- IFD ---
    b.extend_from_slice(&(N_ENTRIES as u16).to_le_bytes());
    let mut entry = |tag: u16, ty: u16, count: u32, val: u32, buf: &mut Vec<u8>| {
        buf.extend_from_slice(&tag.to_le_bytes());
        buf.extend_from_slice(&ty.to_le_bytes());
        buf.extend_from_slice(&count.to_le_bytes());
        buf.extend_from_slice(&val.to_le_bytes());
    };
    const SHORT: u16 = 3;
    const LONG: u16 = 4;
    const DOUBLE: u16 = 12;
    entry(256, LONG, 1, ncols as u32, &mut b); // ImageWidth
    entry(257, LONG, 1, nrows as u32, &mut b); // ImageLength
    entry(258, SHORT, 1, 32, &mut b); // BitsPerSample
    entry(259, SHORT, 1, 1, &mut b); // Compression = none
    entry(262, SHORT, 1, 1, &mut b); // Photometric = BlackIsZero
    entry(273, LONG, 1, img_off as u32, &mut b); // StripOffsets
    entry(277, SHORT, 1, 1, &mut b); // SamplesPerPixel
    entry(278, LONG, 1, nrows as u32, &mut b); // RowsPerStrip (single strip)
    entry(279, LONG, 1, strip_bytes as u32, &mut b); // StripByteCounts
    entry(339, SHORT, 1, 3, &mut b); // SampleFormat = IEEE float
    entry(33550, DOUBLE, 3, pixscale_off as u32, &mut b); // ModelPixelScale
    entry(33922, DOUBLE, 6, tiepoint_off as u32, &mut b); // ModelTiepoint
    entry(34735, SHORT, 16, geokey_off as u32, &mut b); // GeoKeyDirectory
    b.extend_from_slice(&0u32.to_le_bytes()); // next IFD = none

    // --- out-of-line values ---
    debug_assert_eq!(b.len(), pixscale_off);
    for v in [cellsize, cellsize, 0.0] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    debug_assert_eq!(b.len(), tiepoint_off);
    let top_y = origin[1] + nrows as f64 * cellsize;
    for v in [0.0, 0.0, 0.0, origin[0], top_y, 0.0] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    debug_assert_eq!(b.len(), geokey_off);
    // GeoKeyDirectory: v1.1.0, 3 keys → Projected, PixelIsArea, metre units.
    for k in [1u16, 1, 0, 3, 1024, 0, 1, 1, 1025, 0, 1, 1, 3076, 0, 1, 9001] {
        b.extend_from_slice(&k.to_le_bytes());
    }
    // --- image data: Float32 LE, north row first ---
    debug_assert_eq!(b.len(), img_off);
    for &v in values {
        b.extend_from_slice(&(v as f32).to_le_bytes());
    }

    std::fs::write(path, b)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_valid_geotiff_structure() {
        let vals: Vec<f64> = (0..12).map(|i| i as f64 * 0.5).collect();
        let mut path = std::env::temp_dir();
        path.push("gfd_geotiff_test.tif");
        write_geotiff_float32(&path, 4, 3, 10.0, [100.0, 200.0], &vals).unwrap();
        let bytes = std::fs::read(&path).unwrap();

        // Header: "II", 42, IFD@8.
        assert_eq!(&bytes[0..2], b"II");
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 42);
        assert_eq!(u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]), 8);
        // IFD entry count.
        assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), N_ENTRIES as u16);
        // First entry = ImageWidth (tag 256) = 4.
        assert_eq!(u16::from_le_bytes([bytes[10], bytes[11]]), 256);
        assert_eq!(u32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]), 4);
        // Total file size = header + IFD + ool + 12 floats.
        let expected = 8 + (2 + N_ENTRIES * 12 + 4) + 24 + 48 + 32 + 12 * 4;
        assert_eq!(bytes.len(), expected);
        // Last pixel value round-trips as f32 (5.5).
        let last = &bytes[bytes.len() - 4..];
        let v = f32::from_le_bytes([last[0], last[1], last[2], last[3]]);
        assert!((v - 5.5).abs() < 1e-6, "last pixel {v}");
        let _ = std::fs::remove_file(&path);
    }
}
