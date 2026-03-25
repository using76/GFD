//! # gfd-core
//!
//! The foundational crate for the GFD solver framework. Provides core types,
//! traits, and abstractions that all other crates depend on.

pub mod mesh;
pub mod field;
pub mod linalg;
pub mod interpolation;
pub mod gradient;
pub mod numerics;

use thiserror::Error;

/// Core error type for the gfd-core crate.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Index out of bounds: index {index}, size {size}")]
    IndexOutOfBounds { index: usize, size: usize },

    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    #[error("Linear solver failed to converge after {iterations} iterations (residual: {residual})")]
    SolverNotConverged { iterations: usize, residual: f64 },

    #[error("Invalid mesh: {0}")]
    InvalidMesh(String),

    #[error("Invalid field operation: {0}")]
    InvalidFieldOperation(String),

    #[error("Sparse matrix error: {0}")]
    SparseMatrixError(String),

    #[error("Preconditioner error: {0}")]
    PreconditionerError(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

/// Convenience type alias for Results using [`CoreError`].
pub type Result<T> = std::result::Result<T, CoreError>;

// Re-export key types for convenience.
pub use mesh::{Mesh, MeshInfo};
pub use mesh::node::Node;
pub use mesh::face::{Face, FaceType};
pub use mesh::cell::{Cell, CellType};
pub use mesh::unstructured::{UnstructuredMesh, BoundaryPatch};
pub use mesh::structured::StructuredMesh;
pub use mesh::partition::Partition;

pub use field::{ScalarField, VectorField, TensorField, Field, FieldData, FieldSet};

pub use linalg::{SparseMatrix, LinearSystem, SolverStats, SolverConfig};
pub use linalg::solvers::LinearSolver;
pub use linalg::preconditioners::Preconditioner;

pub use gradient::{GradientMethod, GradientComputer};
pub use interpolation::{InterpolationScheme, Interpolator};
pub use numerics::{ConvectionScheme, DiffusionScheme, TemporalScheme, TVDLimiter};
