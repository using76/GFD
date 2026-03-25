//! # gfd-material
//!
//! Material property SDK for the GFD solver framework.
//! Provides fluid and solid material models, thermal properties,
//! and a built-in material database.

pub mod traits;
pub mod fluid;
pub mod solid;
pub mod thermal;
pub mod database;

use thiserror::Error;

/// Error type for the material crate.
#[derive(Debug, Error)]
pub enum MaterialError {
    #[error("Invalid material state: {0}")]
    InvalidState(String),

    #[error("Property evaluation failed for '{property}': {reason}")]
    EvaluationFailed { property: String, reason: String },

    #[error("Unsupported derivative variable: {0}")]
    UnsupportedDerivative(String),

    #[error("Material not found: {0}")]
    MaterialNotFound(String),

    #[error("Constitutive model error: {0}")]
    ConstitutiveError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, MaterialError>;

// Re-export key types.
pub use traits::{MaterialState, PropertyValue, MaterialProperty, ConstitutiveModel};
pub use fluid::FluidMaterial;
pub use database::MaterialDatabase;
