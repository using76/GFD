//! Gradient computation at cell centers for FVM discretization.

use crate::Result;

/// Trait for FVM gradient computation at cell centers.
///
/// Implementations compute the gradient of a scalar field, returning
/// a vector of 3D gradient vectors (one per cell).
pub trait FvmGradient {
    /// Compute gradients at cell centers.
    ///
    /// # Arguments
    /// * `cell_values` - Scalar field values at cell centers.
    ///
    /// # Returns
    /// A vector of gradient vectors `[dφ/dx, dφ/dy, dφ/dz]` per cell.
    fn compute(&self, cell_values: &[f64]) -> Result<Vec<[f64; 3]>>;
}

/// Green-Gauss cell-based gradient computation (stub).
#[derive(Debug, Clone)]
pub struct GreenGaussCellGradient;

impl FvmGradient for GreenGaussCellGradient {
    fn compute(&self, cell_values: &[f64]) -> Result<Vec<[f64; 3]>> {
        // Without mesh connectivity, return zero gradients.
        // Full implementation requires mesh faces/cells for Green-Gauss summation.
        // The gfd-core GreenGaussCellBasedGradient provides the real implementation.
        let n = cell_values.len();
        Ok(vec![[0.0, 0.0, 0.0]; n])
    }
}

/// Least-squares gradient computation (stub).
#[derive(Debug, Clone)]
pub struct LeastSquaresGradient;

impl FvmGradient for LeastSquaresGradient {
    fn compute(&self, cell_values: &[f64]) -> Result<Vec<[f64; 3]>> {
        // Without mesh connectivity, return zero gradients.
        // Full implementation requires mesh neighbor info and cell center coordinates.
        // The gfd-core LeastSquaresGradient provides the mesh-aware implementation.
        let n = cell_values.len();
        Ok(vec![[0.0, 0.0, 0.0]; n])
    }
}
