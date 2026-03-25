//! Jacobi (diagonal) preconditioner.
//!
//! The Jacobi preconditioner is M = diag(A), so M^{-1} r = r_i / a_ii.

use gfd_core::SparseMatrix;
use crate::{LinalgError, Result};
use crate::traits::PreconditionerTrait;

/// Jacobi (diagonal) preconditioner.
///
/// Stores the inverse of the diagonal entries of the matrix.
#[derive(Debug, Clone)]
pub struct Jacobi {
    /// Inverse diagonal entries: 1 / a_ii.
    inv_diag: Vec<f64>,
}

impl Jacobi {
    /// Creates a new uninitialized Jacobi preconditioner.
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

impl PreconditionerTrait for Jacobi {
    fn setup(&mut self, a: &SparseMatrix) -> Result<()> {
        let diag = a.diagonal();
        self.inv_diag = Vec::with_capacity(diag.len());

        for (i, &d) in diag.iter().enumerate() {
            if d.abs() < 1e-300 {
                return Err(LinalgError::PreconditionerError(format!(
                    "Jacobi: zero diagonal at row {}",
                    i
                )));
            }
            self.inv_diag.push(1.0 / d);
        }

        Ok(())
    }

    fn apply(&self, r: &[f64], z: &mut [f64]) -> Result<()> {
        if r.len() != self.inv_diag.len() || z.len() != self.inv_diag.len() {
            return Err(LinalgError::DimensionMismatch(format!(
                "Jacobi: expected vector of length {}, got r={} z={}",
                self.inv_diag.len(),
                r.len(),
                z.len()
            )));
        }

        for i in 0..r.len() {
            z[i] = self.inv_diag[i] * r[i];
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jacobi_basic() {
        // 3x3 diagonal: [2 0 0; 0 4 0; 0 0 8]
        let row_ptr = vec![0, 1, 2, 3];
        let col_idx = vec![0, 1, 2];
        let values = vec![2.0, 4.0, 8.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();

        let mut jac = Jacobi::new();
        jac.setup(&a).unwrap();

        let r = vec![10.0, 20.0, 40.0];
        let mut z = vec![0.0; 3];
        jac.apply(&r, &mut z).unwrap();

        assert!((z[0] - 5.0).abs() < 1e-12);
        assert!((z[1] - 5.0).abs() < 1e-12);
        assert!((z[2] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn jacobi_with_offdiag() {
        // [4 -1 0; -1 4 -1; 0 -1 4]
        let row_ptr = vec![0, 2, 5, 7];
        let col_idx = vec![0, 1, 0, 1, 2, 1, 2];
        let values = vec![4.0, -1.0, -1.0, 4.0, -1.0, -1.0, 4.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();

        let mut jac = Jacobi::new();
        jac.setup(&a).unwrap();

        let r = vec![4.0, 8.0, 12.0];
        let mut z = vec![0.0; 3];
        jac.apply(&r, &mut z).unwrap();

        assert!((z[0] - 1.0).abs() < 1e-12);
        assert!((z[1] - 2.0).abs() < 1e-12);
        assert!((z[2] - 3.0).abs() < 1e-12);
    }
}
