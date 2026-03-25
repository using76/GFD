//! Flexible GMRES (FGMRES) solver stub.
//!
//! FGMRES allows a different preconditioner at each iteration,
//! which is useful for variable preconditioning strategies.

use gfd_core::{SparseMatrix, SolverStats};
use crate::{LinalgError, Result};
use crate::traits::LinearSolverTrait;

/// Flexible GMRES solver (stub).
#[derive(Debug, Clone)]
pub struct FGMRES {
    /// Convergence tolerance.
    pub tol: f64,
    /// Maximum iterations.
    pub max_iter: usize,
    /// Restart parameter.
    pub restart: usize,
}

impl FGMRES {
    /// Creates a new FGMRES solver.
    pub fn new(tol: f64, max_iter: usize, restart: usize) -> Self {
        Self {
            tol,
            max_iter,
            restart,
        }
    }
}

impl Default for FGMRES {
    fn default() -> Self {
        Self {
            tol: 1e-6,
            max_iter: 1000,
            restart: 30,
        }
    }
}

impl LinearSolverTrait for FGMRES {
    fn solve(&mut self, _a: &SparseMatrix, _b: &[f64], _x: &mut [f64]) -> Result<SolverStats> {
        Err(LinalgError::NotImplemented(
            "FGMRES is not yet implemented".to_string(),
        ))
    }
}
