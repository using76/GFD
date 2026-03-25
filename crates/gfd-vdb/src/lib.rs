//! # gfd-vdb
//!
//! VDB (Volumetric Dynamic B+tree) grid support for the GFD solver.
//! Provides sparse volumetric data storage and I/O compatible with
//! the OpenVDB format.

pub mod tree;
pub mod grid;
pub mod io;
pub mod codec;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error type for the VDB crate.
#[derive(Debug, Error)]
pub enum VdbError {
    #[error("VDB I/O error: {0}")]
    IoError(String),

    #[error("Invalid VDB grid: {0}")]
    InvalidGrid(String),

    #[error("Codec error: {0}")]
    CodecError(String),

    #[error("Core error: {0}")]
    CoreError(#[from] gfd_core::CoreError),
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, VdbError>;

/// A VDB grid storing sparse volumetric data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VdbGrid {
    /// Name of this grid.
    pub name: String,
    /// Flat data storage (active voxel values).
    pub data: Vec<f64>,
    /// Affine transform from index space to world space (4x4 row-major).
    pub transform: [f64; 16],
    /// Arbitrary metadata key-value pairs.
    pub metadata: std::collections::HashMap<String, String>,
}

impl VdbGrid {
    /// Creates a new empty VDB grid.
    pub fn new(name: impl Into<String>) -> Self {
        // Identity transform
        #[rustfmt::skip]
        let identity = [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        Self {
            name: name.into(),
            data: Vec::new(),
            transform: identity,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Creates a new VDB grid with preallocated data.
    pub fn with_data(name: impl Into<String>, data: Vec<f64>) -> Self {
        let mut grid = Self::new(name);
        grid.data = data;
        grid
    }

    /// Sets a metadata key-value pair.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Returns the number of active voxels.
    pub fn num_active_voxels(&self) -> usize {
        self.data.len()
    }
}

/// Writes one or more VDB grids to a file in the simplified GFD-VDB binary format.
pub fn write_vdb(path: &str, grids: &[VdbGrid]) -> Result<()> {
    let bytes = io::write_to_bytes(grids)?;
    std::fs::write(path, bytes).map_err(|e| {
        VdbError::IoError(format!("Failed to write VDB file '{}': {}", path, e))
    })
}

/// Reads VDB grids from a file in the simplified GFD-VDB binary format.
pub fn read_vdb(path: &str) -> Result<Vec<VdbGrid>> {
    let bytes = std::fs::read(path).map_err(|e| {
        VdbError::IoError(format!("Failed to read VDB file '{}': {}", path, e))
    })?;
    io::read_from_bytes(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_roundtrip() {
        let tmp_dir = std::env::temp_dir();
        let path = tmp_dir.join("gfd_vdb_test_roundtrip.vdb");
        let path_str = path.to_str().unwrap();

        let mut grid = VdbGrid::with_data("pressure", vec![1.0, 2.0, 3.0, 4.0]);
        grid.set_metadata("solver", "gfd");

        write_vdb(path_str, &[grid.clone()]).unwrap();
        let read_back = read_vdb(path_str).unwrap();

        assert_eq!(read_back.len(), 1);
        assert_eq!(read_back[0].name, "pressure");
        assert_eq!(read_back[0].data, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(read_back[0].metadata.get("solver").unwrap(), "gfd");
        assert_eq!(read_back[0].transform, grid.transform);

        // Clean up
        let _ = std::fs::remove_file(path_str);
    }

    #[test]
    fn read_nonexistent_file() {
        let result = read_vdb("/tmp/gfd_vdb_nonexistent_12345.vdb");
        assert!(result.is_err());
    }
}
