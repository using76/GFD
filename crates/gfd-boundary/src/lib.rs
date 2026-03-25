//! # gfd-boundary
//!
//! Boundary condition SDK for the GFD solver framework.
//! Provides standard and custom boundary conditions, profiles, and traits.

pub mod traits;
pub mod standard;
pub mod custom;
pub mod profiles;
pub mod synthetic_turbulence;

use thiserror::Error;

/// Error type for the boundary crate.
#[derive(Debug, Error)]
pub enum BoundaryError {
    #[error("Invalid boundary condition: {0}")]
    InvalidCondition(String),

    #[error("Expression evaluation failed: {0}")]
    ExpressionError(String),

    #[error("Profile evaluation failed: {0}")]
    ProfileError(String),

    #[error("Boundary face not found: face_id={0}")]
    FaceNotFound(usize),
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, BoundaryError>;

// Re-export key types.
pub use traits::{BoundaryConditionType, BoundaryCondition};
pub use standard::{FixedValue, FixedGradient, ZeroGradient, NoSlip, Symmetry, RobinBC, ConvectiveBC};
pub use custom::ExpressionBC;
pub use profiles::TimeProfile;
