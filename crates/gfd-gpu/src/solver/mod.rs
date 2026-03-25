//! GPU-accelerated linear solvers.

pub mod amgx;
pub mod gpu_cg;

use crate::memory::GpuVector;
use crate::sparse::GpuSparseMatrix;
use crate::Result;
use gfd_core::linalg::SolverStats;

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// A linear solver that operates on GPU data.
pub trait GpuLinearSolver {
    /// Solve `A x = b`, storing the result in `x`.
    fn solve(
        &mut self,
        a: &GpuSparseMatrix,
        b: &GpuVector,
        x: &mut GpuVector,
    ) -> Result<SolverStats>;
}

// ---------------------------------------------------------------------------
// Backend selector
// ---------------------------------------------------------------------------

/// Selects between CPU and GPU linear-solver backends.
///
/// When the variant is `Cpu` the caller is expected to drive the solve through
/// `gfd-linalg` directly.  When the variant is `Gpu`, the contained solver
/// operates on GPU-resident data.
pub enum SolverBackend {
    /// Caller uses gfd-linalg (or any other CPU solver) directly.
    Cpu,
    /// GPU-accelerated solver.
    Gpu(Box<dyn GpuLinearSolver>),
}
