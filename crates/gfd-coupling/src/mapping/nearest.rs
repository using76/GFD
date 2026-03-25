//! Nearest-neighbor field mapping.

use gfd_core::ScalarField;
use gfd_core::field::Field;
use crate::{CouplingError, Result};
use super::FieldMapper;

/// Maps field values using nearest-neighbor interpolation.
///
/// Each target point receives the value from the closest source point.
/// Source and target point coordinates are stored as parallel arrays of [f64; 3].
pub struct NearestNeighborMapper {
    /// Coordinates of source points.
    source_points: Vec<[f64; 3]>,
    /// Coordinates of target points.
    target_points: Vec<[f64; 3]>,
    /// Precomputed mapping: target index -> nearest source index.
    mapping: Vec<usize>,
}

impl NearestNeighborMapper {
    /// Creates a new nearest-neighbor mapper from source and target point coordinates.
    ///
    /// Precomputes the mapping from each target point to its nearest source point.
    pub fn new(source_points: Vec<[f64; 3]>, target_points: Vec<[f64; 3]>) -> Self {
        let mapping = target_points
            .iter()
            .map(|tp| {
                source_points
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        let da = distance_squared(tp, a);
                        let db = distance_squared(tp, b);
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(idx, _)| idx)
                    .unwrap_or(0)
            })
            .collect();

        Self {
            source_points,
            target_points,
            mapping,
        }
    }

    /// Returns the number of source points.
    pub fn num_source_points(&self) -> usize {
        self.source_points.len()
    }

    /// Returns the number of target points.
    pub fn num_target_points(&self) -> usize {
        self.target_points.len()
    }
}

impl FieldMapper for NearestNeighborMapper {
    fn map_field(&self, from: &ScalarField, to: &mut ScalarField) -> Result<()> {
        if from.len() != self.source_points.len() {
            return Err(CouplingError::SizeMismatch {
                src_size: from.len(),
                tgt_size: self.source_points.len(),
            });
        }
        if to.len() != self.target_points.len() {
            return Err(CouplingError::SizeMismatch {
                src_size: to.len(),
                tgt_size: self.target_points.len(),
            });
        }

        let from_values = from.values();
        let to_values = to.values_mut();
        for (target_idx, &source_idx) in self.mapping.iter().enumerate() {
            to_values[target_idx] = from_values[source_idx];
        }

        Ok(())
    }
}

/// Computes the squared Euclidean distance between two 3D points.
fn distance_squared(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}
