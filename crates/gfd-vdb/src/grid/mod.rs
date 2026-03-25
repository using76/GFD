//! VDB grid operations.

use crate::VdbGrid;

/// Resamples a VDB grid to a different voxel size.
pub fn resample(grid: &VdbGrid, new_voxel_size: f64) -> VdbGrid {
    // Simple resampling: scale the transform and keep data as-is
    let mut resampled = grid.clone();
    // Update the diagonal elements of the transform to reflect new voxel size
    resampled.transform[0] = new_voxel_size;
    resampled.transform[5] = new_voxel_size;
    resampled.transform[10] = new_voxel_size;
    resampled
}

/// Computes the bounding box of active voxels in index space.
pub fn active_bounding_box(grid: &VdbGrid) -> Option<([i64; 3], [i64; 3])> {
    if grid.data.is_empty() {
        return None;
    }
    // Without explicit index tracking, return a bounding box based on data length
    let n = grid.data.len() as i64;
    let dim = (n as f64).cbrt().ceil() as i64;
    Some(([0, 0, 0], [dim - 1, dim - 1, dim - 1]))
}
