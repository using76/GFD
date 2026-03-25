//! # gfd-source
//!
//! Source term SDK for the GFD solver framework.
//! Provides volume sources, momentum sources, porous media,
//! and buoyancy models.

pub mod traits;
pub mod volume_source;
pub mod momentum_source;
pub mod porous;
pub mod buoyancy;

use thiserror::Error;

/// Error type for the source crate.
#[derive(Debug, Error)]
pub enum SourceError {
    #[error("Source term evaluation failed: {0}")]
    EvaluationFailed(String),

    #[error("Invalid source configuration: {0}")]
    InvalidConfig(String),

    #[error("Expression error: {0}")]
    ExpressionError(String),

    #[error("Zone not found: {0}")]
    ZoneNotFound(String),
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, SourceError>;

// Re-export key types.
pub use traits::{EquationId, LinearizedSource, ZoneFilter, SourceTerm};
pub use volume_source::{ConstantVolumeSource, ExpressionSource};
pub use momentum_source::BodyForce;
pub use porous::DarcyForchheimer;
pub use buoyancy::{BoussinesqBuoyancy, FullBuoyancy};
