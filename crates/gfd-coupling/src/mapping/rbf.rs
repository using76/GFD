//! Radial Basis Function (RBF) field mapping.

use gfd_core::ScalarField;
use crate::Result;
use super::FieldMapper;

/// Maps field values using Radial Basis Function interpolation.
///
/// Provides smooth, high-order interpolation between non-matching meshes.
pub struct RbfMapper {
    /// Coordinates of source points.
    pub source_points: Vec<[f64; 3]>,
    /// Coordinates of target points.
    pub target_points: Vec<[f64; 3]>,
    /// RBF shape parameter controlling the basis function width.
    pub shape_parameter: f64,
}

impl RbfMapper {
    /// Creates a new RBF mapper.
    pub fn new(
        source_points: Vec<[f64; 3]>,
        target_points: Vec<[f64; 3]>,
        shape_parameter: f64,
    ) -> Self {
        Self {
            source_points,
            target_points,
            shape_parameter,
        }
    }
}

impl FieldMapper for RbfMapper {
    fn map_field(&self, _from: &ScalarField, _to: &mut ScalarField) -> Result<()> {
        // RBF mapping algorithm:
        // 1. Build interpolation matrix A_ij = phi(||x_i - x_j||) for source points
        // 2. Solve A * w = f for weights w
        // 3. Evaluate at target points: f_target_i = sum_j(w_j * phi(||y_i - x_j||))
        let n_src = self.source_points.len();
        let n_tgt = self.target_points.len();
        let c = self.shape_parameter;

        if n_src == 0 {
            return Ok(());
        }

        let from_values = _from.values();

        // Multiquadric RBF: phi(r) = sqrt(r^2 + c^2)
        let rbf = |r: f64| -> f64 { (r * r + c * c).sqrt() };

        let dist = |a: &[f64; 3], b: &[f64; 3]| -> f64 {
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        };

        // 1. Build interpolation matrix A_ij = phi(||x_i - x_j||) for source points
        // 2. Solve A * w = f for weights
        // Use simple Gauss elimination for small systems, Jacobi for larger

        let n = n_src.min(from_values.len());
        let mut a_mat = vec![vec![0.0_f64; n]; n];
        for i in 0..n {
            for j in 0..n {
                a_mat[i][j] = rbf(dist(&self.source_points[i], &self.source_points[j]));
            }
        }

        // Solve A * w = f using Gauss elimination with partial pivoting
        let mut aug = vec![vec![0.0_f64; n + 1]; n];
        for i in 0..n {
            for j in 0..n {
                aug[i][j] = a_mat[i][j];
            }
            aug[i][n] = from_values[i];
        }

        for k in 0..n {
            // Partial pivoting
            let mut max_val = aug[k][k].abs();
            let mut max_row = k;
            for i in (k + 1)..n {
                if aug[i][k].abs() > max_val {
                    max_val = aug[i][k].abs();
                    max_row = i;
                }
            }
            if max_row != k {
                aug.swap(k, max_row);
            }

            let pivot = aug[k][k];
            if pivot.abs() < 1e-30 { continue; }

            for i in (k + 1)..n {
                let factor = aug[i][k] / pivot;
                for j in k..=n {
                    aug[i][j] -= factor * aug[k][j];
                }
            }
        }

        let mut weights = vec![0.0_f64; n];
        for k in (0..n).rev() {
            if aug[k][k].abs() < 1e-30 { continue; }
            let mut s = aug[k][n];
            for j in (k + 1)..n {
                s -= aug[k][j] * weights[j];
            }
            weights[k] = s / aug[k][k];
        }

        // 3. Evaluate at target points
        let to_values = _to.values_mut();
        for i in 0..n_tgt.min(to_values.len()) {
            let mut val = 0.0_f64;
            for j in 0..n {
                val += weights[j] * rbf(dist(&self.target_points[i], &self.source_points[j]));
            }
            to_values[i] = val;
        }

        Ok(())
    }
}
