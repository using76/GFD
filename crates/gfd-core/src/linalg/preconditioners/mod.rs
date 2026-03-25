//! Preconditioners for iterative linear solvers.

pub mod jacobi;
pub mod ilu;
pub mod amg;

use crate::linalg::SparseMatrix;
use crate::Result;

/// Trait for preconditioners that accelerate iterative solver convergence.
pub trait Preconditioner {
    /// Sets up the preconditioner from the system matrix.
    ///
    /// This is called once before any `apply` calls and may be expensive
    /// (e.g., computing an incomplete factorization).
    fn setup(&mut self, a: &SparseMatrix) -> Result<()>;

    /// Applies the preconditioner: solves M * z = r approximately.
    ///
    /// Given the residual vector `r`, computes a preconditioned vector `z`.
    fn apply(&self, r: &[f64], z: &mut [f64]) -> Result<()>;

    /// Returns the name of this preconditioner.
    fn name(&self) -> &str;
}
