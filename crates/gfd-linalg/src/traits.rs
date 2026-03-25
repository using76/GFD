//! Traits for linear solvers and preconditioners.

use gfd_core::{SparseMatrix, SolverStats};
use crate::Result;

/// Trait for linear solvers that solve Ax = b.
///
/// Implementations receive the matrix A, RHS vector b, and solution vector x
/// (which may contain an initial guess).
pub trait LinearSolverTrait {
    /// Solve the linear system A * x = b.
    ///
    /// # Arguments
    /// * `a` - The sparse coefficient matrix.
    /// * `b` - The right-hand side vector.
    /// * `x` - On input, the initial guess; on output, the solution.
    ///
    /// # Returns
    /// Solver statistics (iteration count, residual, convergence status).
    fn solve(&mut self, a: &SparseMatrix, b: &[f64], x: &mut [f64]) -> Result<SolverStats>;
}

/// Trait for preconditioners that accelerate iterative solver convergence.
pub trait PreconditionerTrait {
    /// Set up the preconditioner from the system matrix.
    ///
    /// This is called once before `apply` and may be expensive.
    fn setup(&mut self, a: &SparseMatrix) -> Result<()>;

    /// Apply the preconditioner: given residual r, compute z ~ M^{-1} r.
    ///
    /// # Arguments
    /// * `r` - The residual vector.
    /// * `z` - Output: the preconditioned vector.
    fn apply(&self, r: &[f64], z: &mut [f64]) -> Result<()>;
}
