//! LU factorization direct solver (stub).

use gfd_core::{SparseMatrix, SolverStats};
use crate::{LinalgError, Result};
use crate::traits::LinearSolverTrait;

/// LU factorization solver (stub).
///
/// For sparse LU, a fill-reducing ordering and symbolic/numeric
/// factorization would be needed. This is a placeholder.
#[derive(Debug, Clone)]
pub struct LU;

impl LU {
    /// Creates a new LU solver.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LU {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearSolverTrait for LU {
    fn solve(&mut self, _a: &SparseMatrix, _b: &[f64], _x: &mut [f64]) -> Result<SolverStats> {
        Err(LinalgError::NotImplemented(
            "Sparse LU factorization is not yet implemented".to_string(),
        ))
    }
}
