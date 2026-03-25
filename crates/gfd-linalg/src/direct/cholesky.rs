//! Cholesky factorization direct solver (stub).

use gfd_core::{SparseMatrix, SolverStats};
use crate::{LinalgError, Result};
use crate::traits::LinearSolverTrait;

/// Cholesky factorization solver for SPD matrices (stub).
///
/// Requires the matrix to be symmetric positive-definite.
/// This is a placeholder for a future sparse Cholesky implementation.
#[derive(Debug, Clone)]
pub struct Cholesky;

impl Cholesky {
    /// Creates a new Cholesky solver.
    pub fn new() -> Self {
        Self
    }
}

impl Default for Cholesky {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearSolverTrait for Cholesky {
    fn solve(&mut self, _a: &SparseMatrix, _b: &[f64], _x: &mut [f64]) -> Result<SolverStats> {
        Err(LinalgError::NotImplemented(
            "Sparse Cholesky factorization is not yet implemented".to_string(),
        ))
    }
}
