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

/// Writes one or more VDB grids to a file.
pub fn write_vdb(_path: &str, _grids: &[VdbGrid]) -> Result<()> {
    // OpenVDB file format is complex; return error for now.
    Err(VdbError::IoError(
        "VDB file writing is not yet implemented. Use gfd-io VTK export instead.".to_string(),
    ))
}

/// Reads VDB grids from a file.
pub fn read_vdb(_path: &str) -> Result<Vec<VdbGrid>> {
    Err(VdbError::IoError(
        "VDB file reading is not yet implemented.".to_string(),
    ))
}
