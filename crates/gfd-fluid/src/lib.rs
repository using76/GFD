//! # gfd-fluid
//!
//! Fluid dynamics solver framework for the GFD solver.
//! Provides incompressible/compressible flow, turbulence integration,
//! multiphase, and combustion solvers.

pub mod incompressible;
pub mod compressible;
pub mod turbulence;
pub mod multiphase;
pub mod combustion;
pub mod eos;
pub mod source;

use gfd_core::{ScalarField, VectorField};
use serde::Deserialize;
use thiserror::Error;

/// Error type for the fluid crate.
#[derive(Debug, Error)]
pub enum FluidError {
    #[error("Momentum equation diverged at iteration {iteration} (residual: {residual})")]
    MomentumDiverged { iteration: usize, residual: f64 },

    #[error("Pressure correction failed: {0}")]
    PressureCorrectionFailed(String),

    #[error("Turbulence model error: {0}")]
    TurbulenceError(String),

    #[error("Linear solver failed: {0}")]
    SolverFailed(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Core error: {0}")]
    CoreError(#[from] gfd_core::CoreError),
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, FluidError>;

/// Complete fluid state holding all solution fields.
pub struct FluidState {
    /// Velocity vector field [m/s].
    pub velocity: VectorField,
    /// Pressure scalar field [Pa].
    pub pressure: ScalarField,
    /// Density scalar field [kg/m^3].
    pub density: ScalarField,
    /// Dynamic viscosity scalar field [Pa*s].
    pub viscosity: ScalarField,
    /// Turbulent kinetic energy k [m^2/s^2] (RANS models).
    pub turb_kinetic_energy: Option<ScalarField>,
    /// Turbulence dissipation rate epsilon [m^2/s^3] (k-epsilon model).
    pub turb_dissipation: Option<ScalarField>,
    /// Specific dissipation rate omega [1/s] (k-omega model).
    pub turb_specific_dissipation: Option<ScalarField>,
    /// Eddy (turbulent) viscosity [Pa*s].
    pub eddy_viscosity: Option<ScalarField>,
}

impl FluidState {
    /// Creates a new fluid state with fields sized for the given number of cells.
    pub fn new(num_cells: usize) -> Self {
        Self {
            velocity: VectorField::zeros("velocity", num_cells),
            pressure: ScalarField::zeros("pressure", num_cells),
            density: ScalarField::ones("density", num_cells),
            viscosity: ScalarField::new("viscosity", vec![1.0e-3; num_cells]),
            turb_kinetic_energy: None,
            turb_dissipation: None,
            turb_specific_dissipation: None,
            eddy_viscosity: None,
        }
    }

    /// Returns the number of cells in this state.
    pub fn num_cells(&self) -> usize {
        self.pressure.values().len()
    }
}

/// Configuration for the fluid solver.
#[derive(Debug, Clone, Deserialize)]
pub struct FluidConfig {
    /// Type of flow: "incompressible" or "compressible".
    pub flow_type: String,
    /// Pressure-velocity coupling algorithm: "SIMPLE", "PISO", "SIMPLEC".
    pub pv_coupling: String,
    /// Under-relaxation factor for velocity.
    pub relax_velocity: f64,
    /// Under-relaxation factor for pressure.
    pub relax_pressure: f64,
    /// Maximum number of outer iterations per time step.
    pub max_iterations: usize,
    /// Convergence tolerance.
    pub tolerance: f64,
}

impl Default for FluidConfig {
    fn default() -> Self {
        Self {
            flow_type: "incompressible".to_string(),
            pv_coupling: "SIMPLE".to_string(),
            relax_velocity: 0.7,
            relax_pressure: 0.3,
            max_iterations: 100,
            tolerance: 1e-6,
        }
    }
}
