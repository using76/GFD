//! SIMPLEC (SIMPLE-Consistent) algorithm.

use gfd_core::UnstructuredMesh;
use crate::incompressible::PressureVelocityCoupling;
use crate::{FluidState, Result};

/// SIMPLEC pressure-velocity coupling solver.
///
/// A variant of SIMPLE where the velocity correction formula uses
/// (a_P - H_1) instead of a_P, providing better convergence for
/// the pressure correction equation.
pub struct SimplecSolver {
    /// Under-relaxation factor for pressure.
    pub under_relaxation_pressure: f64,
    /// Under-relaxation factor for velocity.
    pub under_relaxation_velocity: f64,
}

impl SimplecSolver {
    /// Creates a new SIMPLEC solver.
    pub fn new(
        under_relaxation_pressure: f64,
        under_relaxation_velocity: f64,
    ) -> Self {
        Self {
            under_relaxation_pressure,
            under_relaxation_velocity,
        }
    }
}

impl PressureVelocityCoupling for SimplecSolver {
    fn solve_step(
        &mut self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<f64> {
        // SIMPLEC is SIMPLE with alpha_p = 1.0 (the consistent pressure correction).
        // We delegate to a SimpleSolver configured with SIMPLEC-style relaxation.
        let mut simple = super::simple::SimpleSolver::new(
            state.density.values()[0], // uniform density
            state.viscosity.values()[0], // uniform viscosity
        );
        simple.alpha_u = self.under_relaxation_velocity;
        // Key SIMPLEC insight: pressure under-relaxation = 1.0
        simple.alpha_p = 1.0;

        simple.solve_step(state, mesh, dt)
    }
}
