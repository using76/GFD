//! Mortar method for conservative field mapping.

use gfd_core::ScalarField;
use crate::Result;
use super::FieldMapper;

/// Maps field values using the mortar element method.
///
/// Provides conservative and consistent interpolation by computing
/// overlap integrals between source and target surface elements.
pub struct MortarMapper {
    /// Precomputed mortar integration weights.
    pub weights: Vec<Vec<(usize, f64)>>,
}

impl MortarMapper {
    /// Creates a new mortar mapper (requires pre-computed overlap integrals).
    pub fn new(weights: Vec<Vec<(usize, f64)>>) -> Self {
        Self { weights }
    }
}

impl FieldMapper for MortarMapper {
    fn map_field(&self, _from: &ScalarField, _to: &mut ScalarField) -> Result<()> {
        // Apply precomputed mortar weights to transfer field values
        let from_values = _from.values();
        let to_values = _to.values_mut();

        for (target_idx, weight_pairs) in self.weights.iter().enumerate() {
            if target_idx >= to_values.len() {
                break;
            }
            let mut val = 0.0_f64;
            for &(src_idx, weight) in weight_pairs {
                if src_idx < from_values.len() {
                    val += weight * from_values[src_idx];
                }
            }
            to_values[target_idx] = val;
        }

        Ok(())
    }
}
