//! # gfd-solid
//!
//! Structural mechanics solver framework for the GFD solver.
//! Provides linear elastic, hyperelastic, plasticity, dynamics,
//! contact, creep, and thermal stress solvers.

pub mod elastic;
pub mod hyperelastic;
pub mod plasticity;
pub mod dynamics;
pub mod contact;
pub mod creep;
pub mod thermal_stress;

use gfd_core::{VectorField, TensorField};
use serde::Deserialize;
use thiserror::Error;

/// Error type for the solid mechanics crate.
#[derive(Debug, Error)]
pub enum SolidError {
    #[error("Solid solver diverged at iteration {iteration} (residual: {residual})")]
    Diverged { iteration: usize, residual: f64 },

    #[error("Negative Jacobian detected in element {element_id}")]
    NegativeJacobian { element_id: usize },

    #[error("Material model error: {0}")]
    MaterialError(String),

    #[error("Contact detection error: {0}")]
    ContactError(String),

    #[error("Core error: {0}")]
    CoreError(#[from] gfd_core::CoreError),
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, SolidError>;

/// Complete solid mechanics state.
pub struct SolidState {
    /// Displacement vector field [m].
    pub displacement: VectorField,
    /// Cauchy stress tensor field [Pa].
    pub stress: TensorField,
    /// Strain tensor field [-].
    pub strain: TensorField,
}

impl SolidState {
    /// Creates a new zero-initialized solid state for the given number of cells.
    pub fn new(num_cells: usize) -> Self {
        Self {
            displacement: VectorField::zeros("displacement", num_cells),
            stress: TensorField::zeros("stress", num_cells),
            strain: TensorField::zeros("strain", num_cells),
        }
    }

    /// Returns the number of cells in this state.
    pub fn num_cells(&self) -> usize {
        self.displacement.values().len()
    }
}

/// Configuration for the solid mechanics solver.
#[derive(Debug, Clone, Deserialize)]
pub struct SolidConfig {
    /// Analysis type: "static", "quasi_static", "dynamic".
    pub analysis_type: String,
    /// Material model: "linear_elastic", "hyperelastic", "elastoplastic".
    pub material_model: String,
    /// Maximum number of Newton-Raphson iterations (for nonlinear problems).
    pub max_iterations: usize,
    /// Convergence tolerance for the Newton-Raphson solver.
    pub tolerance: f64,
}

impl Default for SolidConfig {
    fn default() -> Self {
        Self {
            analysis_type: "static".to_string(),
            material_model: "linear_elastic".to_string(),
            max_iterations: 50,
            tolerance: 1e-8,
        }
    }
}
