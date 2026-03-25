//! # gfd-thermal
//!
//! Thermal solver framework for the GFD solver.
//! Provides conduction, convection, radiation, conjugate heat transfer,
//! and phase change solvers.

pub mod conduction;
pub mod convection;
pub mod radiation;
pub mod conjugate;
pub mod phase_change;

use gfd_core::{ScalarField, VectorField};
use serde::Deserialize;
use thiserror::Error;

/// Error type for the thermal crate.
#[derive(Debug, Error)]
pub enum ThermalError {
    #[error("Thermal solver diverged at iteration {iteration} (residual: {residual})")]
    Diverged { iteration: usize, residual: f64 },

    #[error("Invalid thermal boundary condition: {0}")]
    InvalidBoundaryCondition(String),

    #[error("Material property error: {0}")]
    MaterialError(String),

    #[error("Radiation model error: {0}")]
    RadiationError(String),

    #[error("Core error: {0}")]
    CoreError(#[from] gfd_core::CoreError),
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, ThermalError>;

/// Complete thermal state holding temperature and derived fields.
pub struct ThermalState {
    /// Temperature scalar field [K].
    pub temperature: ScalarField,
    /// Heat flux vector field [W/m^2] (optional, computed from temperature gradient).
    pub heat_flux: Option<VectorField>,
}

impl ThermalState {
    /// Creates a new thermal state with uniform temperature.
    pub fn new(num_cells: usize, initial_temperature: f64) -> Self {
        Self {
            temperature: ScalarField::new(
                "temperature",
                vec![initial_temperature; num_cells],
            ),
            heat_flux: None,
        }
    }

    /// Returns the number of cells in this state.
    pub fn num_cells(&self) -> usize {
        self.temperature.values().len()
    }
}

/// Configuration for the thermal solver.
#[derive(Debug, Clone, Deserialize)]
pub struct ThermalConfig {
    /// Whether to solve for conduction.
    pub conduction: bool,
    /// Whether to include convection (requires velocity field).
    pub convection: bool,
    /// Radiation model: "none", "p1", "discrete_ordinates", "view_factor".
    pub radiation_model: String,
    /// Maximum number of iterations.
    pub max_iterations: usize,
    /// Convergence tolerance.
    pub tolerance: f64,
    /// Under-relaxation factor.
    pub under_relaxation: f64,
}

impl Default for ThermalConfig {
    fn default() -> Self {
        Self {
            conduction: true,
            convection: false,
            radiation_model: "none".to_string(),
            max_iterations: 100,
            tolerance: 1e-6,
            under_relaxation: 0.9,
        }
    }
}
