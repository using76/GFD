//! Jacobi (diagonal) preconditioner.

use crate::linalg::SparseMatrix;
use crate::Result;
use super::Preconditioner;

/// Jacobi preconditioner using the inverse of the diagonal of A.
///
/// M = diag(A), so M^{-1} r = r / diag(A).
#[derive(Debug, Clone)]
pub struct Jacobi {
    /// Inverse of the diagonal entries.
    inv_diag: Vec<f64>,
}

impl Jacobi {
    /// Creates a new (uninitialized) Jacobi preconditioner.
    pub fn new() -> Self {
        Self {
            inv_diag: Vec::new(),
        }
    }
}

impl Default for Jacobi {
    fn default() -> Self {
        Self::new()
    }
}

impl Preconditioner for Jacobi {
    fn setup(&mut self, a: &SparseMatrix) -> Result<()> {
        let diag = a.diagonal();
        self.inv_diag = diag
            .iter()
            .map(|&d| if d.abs() > 1e-15 { 1.0 / d } else { 1.0 })
            .collect();
        Ok(())
    }

    fn apply(&self, r: &[f64], z: &mut [f64]) -> Result<()> {
        if r.len() != self.inv_diag.len() || z.len() != self.inv_diag.len() {
            return Err(crate::CoreError::DimensionMismatch {
                expected: self.inv_diag.len(),
                got: r.len(),
            });
        }
        for i in 0..r.len() {
            z[i] = self.inv_diag[i] * r[i];
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "Jacobi"
    }
}
