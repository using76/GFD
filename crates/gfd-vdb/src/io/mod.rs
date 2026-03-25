//! VDB file I/O utilities.
//!
//! Implements a simplified GFD-VDB binary format:
//! - Header: magic "GFD_VDB\0" (8 bytes), version (u32 LE), grid count (u32 LE)
//! - Per grid:
//!     - name_len (u32 LE), name (UTF-8 bytes)
//!     - data_type (u32 LE): 0 = f64
//!     - bbox: 6 x f64 LE (min_x, min_y, min_z, max_x, max_y, max_z)
//!     - voxel_count (u64 LE)
//!     - data values: voxel_count x f64 LE
//!     - metadata_count (u32 LE)
//!     - per metadata entry: key_len (u32), key (UTF-8), val_len (u32), val (UTF-8)
//!     - transform: 16 x f64 LE (4x4 row-major affine)

use crate::{VdbGrid, Result, VdbError};

/// Magic number for the GFD-VDB binary format.
const MAGIC: &[u8; 8] = b"GFD_VDB\0";

/// Current format version.
const VERSION: u32 = 1;

/// Data type tag for f64 values.
const DATA_TYPE_F64: u32 = 0;

/// Writes VDB grids to a binary stream.
pub fn write_to_bytes(grids: &[VdbGrid]) -> Result<Vec<u8>> {
    let mut buf = Vec::new();

    // Header
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&VERSION.to_le_bytes());
    buf.extend_from_slice(&(grids.len() as u32).to_le_bytes());

    for grid in grids {
        // Name
        let name_bytes = grid.name.as_bytes();
        buf.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        buf.extend_from_slice(name_bytes);

        // Data type
        buf.extend_from_slice(&DATA_TYPE_F64.to_le_bytes());

        // Bounding box: compute from data length (cube root approximation)
        let n = grid.data.len();
        if n == 0 {
            // Empty grid: zero bbox
            for _ in 0..6 {
                buf.extend_from_slice(&0.0_f64.to_le_bytes());
            }
        } else {
            let dim = (n as f64).cbrt().ceil();
            let voxel_size_x = grid.transform[0];
            let voxel_size_y = grid.transform[5];
            let voxel_size_z = grid.transform[10];
            let origin_x = grid.transform[3];
            let origin_y = grid.transform[7];
            let origin_z = grid.transform[11];
            // min
            buf.extend_from_slice(&origin_x.to_le_bytes());
            buf.extend_from_slice(&origin_y.to_le_bytes());
            buf.extend_from_slice(&origin_z.to_le_bytes());
            // max
            buf.extend_from_slice(&(origin_x + dim * voxel_size_x).to_le_bytes());
            buf.extend_from_slice(&(origin_y + dim * voxel_size_y).to_le_bytes());
            buf.extend_from_slice(&(origin_z + dim * voxel_size_z).to_le_bytes());
        }

        // Voxel count and data
        buf.extend_from_slice(&(n as u64).to_le_bytes());
        for &val in &grid.data {
            buf.extend_from_slice(&val.to_le_bytes());
        }

        // Metadata
        buf.extend_from_slice(&(grid.metadata.len() as u32).to_le_bytes());
        for (key, value) in &grid.metadata {
            let kb = key.as_bytes();
            buf.extend_from_slice(&(kb.len() as u32).to_le_bytes());
            buf.extend_from_slice(kb);
            let vb = value.as_bytes();
            buf.extend_from_slice(&(vb.len() as u32).to_le_bytes());
            buf.extend_from_slice(vb);
        }

        // Transform (4x4, 16 f64s)
        for &t in &grid.transform {
            buf.extend_from_slice(&t.to_le_bytes());
        }
    }

    Ok(buf)
}

/// Reads VDB grids from a binary stream.
pub fn read_from_bytes(data: &[u8]) -> Result<Vec<VdbGrid>> {
    let mut pos = 0;

    // Helper closures for reading
    let read_bytes = |pos: &mut usize, n: usize| -> Result<&[u8]> {
        if *pos + n > data.len() {
            return Err(VdbError::IoError(format!(
                "Unexpected end of data at offset {} (need {} bytes, have {})",
                *pos, n, data.len()
            )));
        }
        let slice = &data[*pos..*pos + n];
        *pos += n;
        Ok(slice)
    };

    let read_u32 = |pos: &mut usize| -> Result<u32> {
        let bytes = read_bytes(pos, 4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    };

    let read_u64 = |pos: &mut usize| -> Result<u64> {
        let bytes = read_bytes(pos, 8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    };

    let read_f64 = |pos: &mut usize| -> Result<f64> {
        let bytes = read_bytes(pos, 8)?;
        Ok(f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    };

    // Read and validate magic
    let magic = read_bytes(&mut pos, 8)?;
    if magic != MAGIC {
        return Err(VdbError::IoError(
            "Invalid magic number: not a GFD_VDB file".to_string(),
        ));
    }

    // Read and validate version
    let version = read_u32(&mut pos)?;
    if version != VERSION {
        return Err(VdbError::IoError(format!(
            "Unsupported VDB format version: {} (expected {})",
            version, VERSION
        )));
    }

    // Grid count
    let grid_count = read_u32(&mut pos)? as usize;
    let mut grids = Vec::with_capacity(grid_count);

    for _ in 0..grid_count {
        // Name
        let name_len = read_u32(&mut pos)? as usize;
        let name_bytes = read_bytes(&mut pos, name_len)?;
        let name = String::from_utf8(name_bytes.to_vec()).map_err(|e| {
            VdbError::IoError(format!("Invalid UTF-8 in grid name: {}", e))
        })?;

        // Data type
        let data_type = read_u32(&mut pos)?;
        if data_type != DATA_TYPE_F64 {
            return Err(VdbError::IoError(format!(
                "Unsupported data type: {} (only f64 = 0 is supported)",
                data_type
            )));
        }

        // Bounding box (read but not stored in VdbGrid directly; skip 6 f64s)
        let _bbox_min_x = read_f64(&mut pos)?;
        let _bbox_min_y = read_f64(&mut pos)?;
        let _bbox_min_z = read_f64(&mut pos)?;
        let _bbox_max_x = read_f64(&mut pos)?;
        let _bbox_max_y = read_f64(&mut pos)?;
        let _bbox_max_z = read_f64(&mut pos)?;

        // Voxel data
        let voxel_count = read_u64(&mut pos)? as usize;
        let mut grid_data = Vec::with_capacity(voxel_count);
        for _ in 0..voxel_count {
            grid_data.push(read_f64(&mut pos)?);
        }

        // Metadata
        let metadata_count = read_u32(&mut pos)? as usize;
        let mut metadata = std::collections::HashMap::new();
        for _ in 0..metadata_count {
            let key_len = read_u32(&mut pos)? as usize;
            let key_bytes = read_bytes(&mut pos, key_len)?;
            let key = String::from_utf8(key_bytes.to_vec()).map_err(|e| {
                VdbError::IoError(format!("Invalid UTF-8 in metadata key: {}", e))
            })?;
            let val_len = read_u32(&mut pos)? as usize;
            let val_bytes = read_bytes(&mut pos, val_len)?;
            let value = String::from_utf8(val_bytes.to_vec()).map_err(|e| {
                VdbError::IoError(format!("Invalid UTF-8 in metadata value: {}", e))
            })?;
            metadata.insert(key, value);
        }

        // Transform (16 f64s)
        let mut transform = [0.0_f64; 16];
        for t in &mut transform {
            *t = read_f64(&mut pos)?;
        }

        grids.push(VdbGrid {
            name,
            data: grid_data,
            transform,
            metadata,
        });
    }

    Ok(grids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_empty_grids() {
        let grids: Vec<VdbGrid> = vec![];
        let bytes = write_to_bytes(&grids).unwrap();
        let read_back = read_from_bytes(&bytes).unwrap();
        assert_eq!(read_back.len(), 0);
    }

    #[test]
    fn roundtrip_single_empty_grid() {
        let grid = VdbGrid::new("test_grid");
        let bytes = write_to_bytes(&[grid]).unwrap();
        let read_back = read_from_bytes(&bytes).unwrap();
        assert_eq!(read_back.len(), 1);
        assert_eq!(read_back[0].name, "test_grid");
        assert!(read_back[0].data.is_empty());
    }

    #[test]
    fn roundtrip_grid_with_data() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let grid = VdbGrid::with_data("density", data.clone());
        let bytes = write_to_bytes(&[grid]).unwrap();
        let read_back = read_from_bytes(&bytes).unwrap();
        assert_eq!(read_back.len(), 1);
        assert_eq!(read_back[0].name, "density");
        assert_eq!(read_back[0].data, data);
    }

    #[test]
    fn roundtrip_grid_with_metadata() {
        let mut grid = VdbGrid::new("temperature");
        grid.data = vec![100.0, 200.0, 300.0];
        grid.set_metadata("units", "Kelvin");
        grid.set_metadata("solver", "gfd-thermal");

        let bytes = write_to_bytes(&[grid]).unwrap();
        let read_back = read_from_bytes(&bytes).unwrap();
        assert_eq!(read_back.len(), 1);
        assert_eq!(read_back[0].metadata.get("units").unwrap(), "Kelvin");
        assert_eq!(read_back[0].metadata.get("solver").unwrap(), "gfd-thermal");
    }

    #[test]
    fn roundtrip_preserves_transform() {
        let mut grid = VdbGrid::new("velocity");
        grid.transform[0] = 0.5;  // voxel size x
        grid.transform[5] = 0.5;  // voxel size y
        grid.transform[10] = 0.5; // voxel size z
        grid.transform[3] = 1.0;  // origin x
        grid.transform[7] = 2.0;  // origin y
        grid.transform[11] = 3.0; // origin z
        grid.data = vec![1.0; 27]; // 3x3x3

        let bytes = write_to_bytes(&[grid.clone()]).unwrap();
        let read_back = read_from_bytes(&bytes).unwrap();
        assert_eq!(read_back[0].transform, grid.transform);
    }

    #[test]
    fn roundtrip_multiple_grids() {
        let g1 = VdbGrid::with_data("pressure", vec![101325.0; 10]);
        let mut g2 = VdbGrid::with_data("temperature", vec![300.0; 5]);
        g2.set_metadata("unit", "K");

        let bytes = write_to_bytes(&[g1, g2]).unwrap();
        let read_back = read_from_bytes(&bytes).unwrap();
        assert_eq!(read_back.len(), 2);
        assert_eq!(read_back[0].name, "pressure");
        assert_eq!(read_back[0].data.len(), 10);
        assert_eq!(read_back[1].name, "temperature");
        assert_eq!(read_back[1].data.len(), 5);
        assert_eq!(read_back[1].metadata.get("unit").unwrap(), "K");
    }

    #[test]
    fn invalid_magic_rejected() {
        let bytes = b"INVALID\0\x01\x00\x00\x00\x00\x00\x00\x00";
        let result = read_from_bytes(bytes);
        assert!(result.is_err());
    }

    #[test]
    fn truncated_data_rejected() {
        // Just the magic, no version or grid count
        let result = read_from_bytes(MAGIC);
        assert!(result.is_err());
    }

    #[test]
    fn unsupported_version_rejected() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&99u32.to_le_bytes()); // bad version
        bytes.extend_from_slice(&0u32.to_le_bytes());
        let result = read_from_bytes(&bytes);
        assert!(result.is_err());
    }
}
