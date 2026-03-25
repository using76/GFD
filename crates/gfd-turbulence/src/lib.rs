//! # gfd-turbulence
//!
//! Turbulence modelling SDK for the GFD solver framework.
//! Provides RANS, LES, and custom turbulence model definitions.

pub mod model_template;
pub mod builtin;
pub mod custom;
pub mod wall_functions;
pub mod validation;

use thiserror::Error;

/// Error type for the turbulence crate.
#[derive(Debug, Error)]
pub enum TurbulenceError {
    #[error("Invalid model constant '{name}': {reason}")]
    InvalidConstant { name: String, reason: String },

    #[error("Invalid eddy viscosity expression: {0}")]
    InvalidEddyViscosity(String),

    #[error("Wall function computation failed: {0}")]
    WallFunctionError(String),

    #[error("Custom model error: {0}")]
    CustomModelError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("JSON deserialization error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, TurbulenceError>;

// Re-export key types.
pub use model_template::{TurbulenceModelDef, TransportEquationDef, ModelConstant, WallTreatment};
pub use builtin::TurbulenceModel;
pub use builtin::spalart_allmaras::SpalartAllmaras;
pub use builtin::k_epsilon::KEpsilon;
pub use builtin::k_omega_sst::KOmegaSST;
pub use builtin::les::LesModel;
pub use builtin::rsm::ReynoldsStressModel;
pub use custom::CustomTurbulenceModel;
