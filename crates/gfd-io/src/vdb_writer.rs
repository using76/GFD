//! OpenVDB file writer for volumetric data export.
//!
//! Exports simulation fields to the OpenVDB format (.vdb), enabling
//! high-quality volume rendering in tools such as Houdini, Blender, and
//! ParaView (with the VDB plugin).

use gfd_core::{FieldSet, UnstructuredMesh};
use crate::Result;

/// Configuration for VDB export.
#[derive(Debug, Clone)]
pub struct VdbExportConfig {
    /// Names of fields to export (empty = export all).
    pub fields_to_export: Vec<String>,
    /// Voxel size for rasterisation from the unstructured mesh.
    pub voxel_size: f64,
    /// Enable BLOSC compression inside VDB grids.
    pub compression: bool,
    /// Half-bandwidth (in voxels) for narrow-band level-set grids.
    pub half_bandwidth: f64,
    /// Write as half-float (FP16) to reduce file size.
    pub half_float: bool,
}

impl Default for VdbExportConfig {
    fn default() -> Self {
        Self {
            fields_to_export: Vec::new(),
            voxel_size: 0.01,
            compression: true,
            half_bandwidth: 3.0,
            half_float: false,
        }
    }
}

/// Writes mesh and field data to an OpenVDB file.
///
/// The unstructured cell data is rasterised onto a sparse VDB grid
/// using the configured voxel size.  Each exported field becomes a
/// separate named grid inside the VDB file.
///
/// # Arguments
/// * `path`          - Output file path (should end in `.vdb`).
/// * `mesh`          - The computational mesh.
/// * `fields`        - Field data to export.
/// * `time_step`     - Current time-step index (written as metadata).
/// * `physical_time` - Current physical time (written as metadata).
pub fn write_vdb(
    _path: &str,
    _mesh: &UnstructuredMesh,
    _fields: &FieldSet,
    _time_step: usize,
    _physical_time: f64,
) -> Result<()> {
    // VDB export pipeline:
    // 1. Determine bounding box of the mesh.
    // 2. Create a VDB grid transform with the desired voxel size.
    // 3. For each field:
    //    a. Rasterise cell-centred data onto the VDB grid.
    //    b. Apply BLOSC compression if enabled.
    //    c. Attach metadata (field name, time, units).
    // 4. Write all grids to a single .vdb file.
    Err(crate::IoError::WriteError(
        "VDB export requires OpenVDB library support which is not yet linked. \
         Use VTK export as an alternative.".to_string(),
    ))
}

/// Writes mesh and field data to an OpenVDB file using a custom config.
pub fn write_vdb_with_config(
    _path: &str,
    _mesh: &UnstructuredMesh,
    _fields: &FieldSet,
    _time_step: usize,
    _physical_time: f64,
    _config: &VdbExportConfig,
) -> Result<()> {
    Err(crate::IoError::WriteError(
        "VDB export requires OpenVDB library support which is not yet linked. \
         Use VTK export as an alternative.".to_string(),
    ))
}
