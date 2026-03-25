//! # gfd-postprocess
//!
//! Post-processing utilities for the GFD solver framework.
//! Provides derived field computation, integral quantities, and field statistics.

pub mod derived_field;
pub mod integrals;
pub mod statistics;
pub mod traits;

use thiserror::Error;

/// Error type for the postprocess crate.
#[derive(Debug, Error)]
pub enum PostProcessError {
    #[error("Field not found: {0}")]
    FieldNotFound(String),

    #[error("Invalid computation: {0}")]
    InvalidComputation(String),

    #[error("Empty field: cannot compute statistics on an empty field")]
    EmptyField,

    #[error("Mesh mismatch: field size {field_size} does not match mesh cells {mesh_cells}")]
    MeshMismatch { field_size: usize, mesh_cells: usize },

    #[error("Core error: {0}")]
    CoreError(#[from] gfd_core::CoreError),
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, PostProcessError>;
