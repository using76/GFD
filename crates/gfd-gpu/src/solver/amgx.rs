//! NVIDIA AmgX solver integration (stub).
//!
//! AmgX is NVIDIA's algebraic multigrid library that provides GPU-accelerated
//! solvers and preconditioners. This module will wrap the AmgX C API through
//! Rust FFI bindings.
//!
//! ## Integration plan
//!
//! 1. Link against `libamgxsh.so` / `amgxsh.dll` at build time via a
//!    `build.rs` script that locates the AmgX installation.
//! 2. Expose safe Rust wrappers for the AmgX resource, config, matrix,
//!    vector, and solver handles.
//! 3. Implement the `GpuLinearSolver` trait so that AmgX can be used as a
//!    drop-in replacement for GPU CG when available.
//!
//! The `amgx` feature flag (which implies `cuda`) gates this entire module.

use crate::memory::GpuVector;
use crate::sparse::GpuSparseMatrix;
use crate::Result;
use gfd_core::linalg::SolverStats;

use super::GpuLinearSolver;

// ---------------------------------------------------------------------------
// Configuration presets
// ---------------------------------------------------------------------------

/// Pre-configured AmgX solver profiles.
#[derive(Debug, Clone)]
pub enum AmgxConfig {
    /// Tuned for pressure-Poisson equations (symmetric, well-conditioned).
    PressureSolver,
    /// Tuned for momentum equations (non-symmetric, potentially stiff).
    MomentumSolver,
    /// User-supplied JSON configuration string.
    Custom(String),
}

// ---------------------------------------------------------------------------
// Solver
// ---------------------------------------------------------------------------

/// AmgX-based linear solver.
///
/// This is currently a **stub** — calling `solve` will panic. The struct
/// exists so that downstream code can be written against the `GpuLinearSolver`
/// trait and tested with the `GpuCG` fallback until AmgX bindings are ready.
pub struct AmgxSolver {
    /// The AmgX configuration to use.
    pub config: AmgxConfig,
}

impl AmgxSolver {
    /// Create a new AmgX solver with the given configuration preset.
    pub fn new(config: AmgxConfig) -> Self {
        Self { config }
    }
}

impl GpuLinearSolver for AmgxSolver {
    fn solve(
        &mut self,
        _a: &GpuSparseMatrix,
        _b: &GpuVector,
        _x: &mut GpuVector,
    ) -> Result<SolverStats> {
        // AmgX integration is not yet implemented.
        // When ready, this method will:
        //   1. Initialize AmgX resources (once, cached).
        //   2. Upload the matrix/vectors via AmgX API.
        //   3. Run the configured solver.
        //   4. Read back the solution.
        Err(crate::GpuError::SolverFailed(
            "AmgX solver integration not yet available. \
             Please use GpuCG or a CPU solver backend instead.".to_string(),
        ))
    }
}
