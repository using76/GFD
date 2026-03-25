//! # gfd-linalg
//!
//! Linear algebra solvers and preconditioners for the GFD solver framework.
//! Provides iterative solvers (CG, BiCGSTAB, GMRES, FGMRES), direct solvers
//! (LU, Cholesky stubs), and preconditioners (Jacobi, ILU, AMG stubs).

pub mod iterative;
pub mod direct;
pub mod preconditioner;
pub mod traits;

use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during linear algebra operations.
#[derive(Debug, Error)]
pub enum LinalgError {
    #[error("Solver did not converge after {iterations} iterations (residual: {residual})")]
    NotConverged { iterations: usize, residual: f64 },

    #[error("Dimension mismatch: {0}")]
    DimensionMismatch(String),

    #[error("Singular or near-singular matrix: {0}")]
    SingularMatrix(String),

    #[error("Preconditioner error: {0}")]
    PreconditionerError(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Core error: {0}")]
    Core(#[from] gfd_core::CoreError),

    #[error("Matrix error: {0}")]
    Matrix(#[from] gfd_matrix::MatrixError),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, LinalgError>;

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use traits::{LinearSolverTrait, PreconditionerTrait};
pub use iterative::cg::CG;
pub use iterative::bicgstab::BiCGSTAB;
pub use iterative::gmres::GMRES;
pub use preconditioner::jacobi::Jacobi;
