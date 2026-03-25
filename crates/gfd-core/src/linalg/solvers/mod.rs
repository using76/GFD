//! Iterative linear solvers.

pub mod cg;
pub mod gmres;
pub mod bicgstab;

use crate::Result;
use super::{LinearSystem, SolverStats};

/// Trait for iterative linear solvers that solve Ax = b.
pub trait LinearSolver {
    /// Solves the linear system in-place, updating `system.x`.
    ///
    /// Returns statistics about the solve including iteration count,
    /// final residual, and whether convergence was achieved.
    fn solve(&mut self, system: &mut LinearSystem) -> Result<SolverStats>;

    /// Returns the name of this solver.
    fn name(&self) -> &str;
}
