//! Turbulence solver integration for the fluid solver.
//!
//! Re-exports from gfd-turbulence and provides an adapter for solving
//! turbulence transport equations within the fluid solver loop.

pub mod transport_solver;
pub mod wall_treatment;
pub mod les;

pub use gfd_turbulence::TurbulenceModel;
pub use gfd_turbulence::builtin::{
    self,
    spalart_allmaras::SpalartAllmaras,
    k_epsilon::KEpsilon,
    k_omega_sst::KOmegaSST,
};

use gfd_core::UnstructuredMesh;
use crate::{FluidState, Result};

/// Adapter that wraps a turbulence model and solves its transport equations
/// as part of the fluid solver iteration loop.
pub struct TurbulenceSolverAdapter {
    /// Name of the active turbulence model.
    pub model_name: String,
    /// Under-relaxation factor for turbulence variables.
    pub under_relaxation: f64,
    /// Maximum iterations for the turbulence sub-solver.
    pub max_iterations: usize,
}

impl TurbulenceSolverAdapter {
    /// Creates a new turbulence solver adapter.
    pub fn new(model_name: impl Into<String>, under_relaxation: f64) -> Self {
        Self {
            model_name: model_name.into(),
            under_relaxation,
            max_iterations: 50,
        }
    }

    /// Solves the turbulence transport equations and updates the fluid state.
    ///
    /// This method discretizes and solves the transport equations for the
    /// turbulence variables (e.g., k and epsilon, or k and omega),
    /// then updates the eddy viscosity field.
    pub fn solve_transport_equations(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<f64> {
        let solver = transport_solver::TurbulenceTransportSolver::with_relaxation(
            self.under_relaxation,
            self.under_relaxation,
        );

        // Determine which model is active and solve accordingly
        let residual_k = solver.solve_k_equation(state, mesh, dt)?;

        let residual_second = if state.turb_dissipation.is_some() || self.model_name.contains("epsilon") {
            solver.solve_epsilon_equation(state, mesh, dt)?
        } else if state.turb_specific_dissipation.is_some() || self.model_name.contains("omega") || self.model_name.contains("sst") {
            solver.solve_omega_equation(state, mesh, dt)?
        } else {
            // Default to k-epsilon
            solver.solve_epsilon_equation(state, mesh, dt)?
        };

        // Update eddy viscosity
        let mu_t = solver.compute_eddy_viscosity(state, mesh)?;
        state.eddy_viscosity = Some(mu_t);

        // Return the maximum residual
        Ok(residual_k.max(residual_second))
    }
}
