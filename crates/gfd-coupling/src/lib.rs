//! # gfd-coupling
//!
//! Multi-physics coupling framework for the GFD solver.
//! Provides coupling strategies, field mapping, and interface definitions.

pub mod interface;
pub mod mapping;
pub mod strategy;
pub mod traits;

use thiserror::Error;

/// Error type for the coupling crate.
#[derive(Debug, Error)]
pub enum CouplingError {
    #[error("Coupling convergence failed after {iterations} iterations (residual: {residual})")]
    ConvergenceFailed { iterations: usize, residual: f64 },

    #[error("Field mapping error: {0}")]
    MappingError(String),

    #[error("Interface error: {0}")]
    InterfaceError(String),

    #[error("Mismatched field sizes: source has {src_size}, target has {tgt_size}")]
    SizeMismatch { src_size: usize, tgt_size: usize },

    #[error("Core error: {0}")]
    CoreError(#[from] gfd_core::CoreError),
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, CouplingError>;
