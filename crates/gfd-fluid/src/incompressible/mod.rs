//! Incompressible flow solvers using pressure-velocity coupling.

pub mod simple;
pub mod piso;
pub mod simplec;

use gfd_core::UnstructuredMesh;
use crate::{FluidState, Result};

/// Trait for pressure-velocity coupling algorithms.
pub trait PressureVelocityCoupling {
    /// Performs one coupling iteration step.
    ///
    /// Returns the residual norm for convergence monitoring.
    fn solve_step(
        &mut self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<f64>;
}
