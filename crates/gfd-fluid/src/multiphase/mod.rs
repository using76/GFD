//! Multiphase flow solvers.

pub mod vof;
pub mod level_set;
pub mod euler_euler;
pub mod dpm;

use gfd_core::{ScalarField, UnstructuredMesh};
use crate::Result;

/// Volume of Fluid (VOF) solver for tracking phase interfaces.
///
/// Solves the advection equation for the volume fraction alpha:
/// d(alpha)/dt + div(alpha * U) = 0
pub struct VofSolver {
    /// Interface compression coefficient (0 = no compression, 1 = full).
    pub compression_coefficient: f64,
}

impl VofSolver {
    /// Creates a new VOF solver.
    pub fn new(compression_coefficient: f64) -> Self {
        Self {
            compression_coefficient,
        }
    }

    /// Advances the volume fraction field by one time step.
    pub fn solve_step(
        &self,
        alpha: &mut ScalarField,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<()> {
        let mut solver_impl = vof::VofSolverImpl::new(
            alpha.clone(),
            self.compression_coefficient,
        );
        // Create a zero velocity field (the caller should provide velocity via VofSolverImpl directly)
        let velocity = gfd_core::VectorField::zeros("velocity", mesh.num_cells());
        solver_impl.solve_transport(&velocity, mesh, dt)?;
        // Copy result back
        let src = solver_impl.alpha.values();
        let dst = alpha.values_mut();
        dst.copy_from_slice(src);
        Ok(())
    }
}

/// Level Set solver for tracking phase interfaces.
///
/// Solves the level set equation: d(phi)/dt + U . grad(phi) = 0
/// followed by a reinitialization step to maintain the signed distance property.
pub struct LevelSetSolver {
    /// Number of reinitialization iterations.
    pub reinit_iterations: usize,
}

impl LevelSetSolver {
    /// Creates a new Level Set solver.
    pub fn new(reinit_iterations: usize) -> Self {
        Self { reinit_iterations }
    }

    /// Advances the level set field by one time step.
    pub fn solve_step(
        &self,
        phi: &mut ScalarField,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<()> {
        let mut solver_impl = level_set::LevelSetSolverImpl::new(
            phi.clone(),
            self.reinit_iterations,
        );
        let velocity = gfd_core::VectorField::zeros("velocity", mesh.num_cells());
        solver_impl.solve_transport(&velocity, mesh, dt)?;
        solver_impl.reinitialize(mesh)?;
        // Copy result back
        let src = solver_impl.phi.values();
        let dst = phi.values_mut();
        dst.copy_from_slice(src);
        Ok(())
    }
}
