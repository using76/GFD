//! # gfd-matrix
//!
//! Matrix assembly, manipulation, and diagnostics for the GFD solver framework.
//! Builds on top of the CSR `SparseMatrix` and `LinearSystem` types from gfd-core,
//! providing higher-level assembly from discrete equations, boundary condition
//! application, and matrix diagnostics.

pub mod sparse;
pub mod assembler;
pub mod block;
pub mod boundary;
pub mod modify;
pub mod diagnostics;

use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during matrix operations.
#[derive(Debug, Error)]
pub enum MatrixError {
    #[error("Index out of bounds: row {row}, col {col}, matrix size {nrows}x{ncols}")]
    IndexOutOfBounds {
        row: usize,
        col: usize,
        nrows: usize,
        ncols: usize,
    },

    #[error("Dimension mismatch: {0}")]
    DimensionMismatch(String),

    #[error("Assembly error: {0}")]
    AssemblyError(String),

    #[error("Singular matrix detected: {0}")]
    SingularMatrix(String),

    #[error("Core error: {0}")]
    Core(#[from] gfd_core::CoreError),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, MatrixError>;

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use sparse::CooMatrix;
pub use assembler::Assembler;
pub use block::{BlockMatrix, BlockAssembler, BlockLinearSystem};
pub use boundary::{apply_dirichlet, apply_neumann};
pub use diagnostics::{DiagnosticReport, check_diagonal_dominance, find_zero_pivots};
