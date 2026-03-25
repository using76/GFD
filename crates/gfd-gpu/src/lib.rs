//! # gfd-gpu
//!
//! GPU acceleration layer for the GFD solver. This crate is **optional** — when
//! compiled without the `cuda` feature every GPU operation transparently falls
//! back to a CPU implementation so the rest of the solver continues to work
//! without CUDA hardware or drivers.
//!
//! ## Feature flags
//!
//! * `cuda`  — enables NVIDIA CUDA support via the `cudarc` crate.
//! * `amgx` — (implies `cuda`) enables the NVIDIA AmgX solver backend (stub).

pub mod device;
pub mod kernels;
pub mod memory;
pub mod solver;
pub mod sparse;
pub mod transfer;

use thiserror::Error;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur in GPU operations.
#[derive(Debug, Error)]
pub enum GpuError {
    #[error("CUDA runtime not available")]
    NoCudaRuntime,

    #[error("GPU device not found (requested id {0})")]
    DeviceNotFound(usize),

    #[error("GPU memory allocation failed: {0}")]
    MemoryAllocation(String),

    #[error("GPU kernel launch failed: {0}")]
    KernelLaunch(String),

    #[error("GPU solver failed: {0}")]
    SolverFailed(String),

    #[error("Host/device transfer failed: {0}")]
    TransferFailed(String),

    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
}

/// Convenience type alias.
pub type Result<T> = std::result::Result<T, GpuError>;

// ---------------------------------------------------------------------------
// Runtime query
// ---------------------------------------------------------------------------

/// Returns `true` when a usable CUDA device is detected at runtime.
///
/// When compiled without the `cuda` feature this always returns `false`.
#[cfg(feature = "cuda")]
pub fn is_gpu_available() -> bool {
    cudarc::driver::result::device::get_count().map_or(false, |n| n > 0)
}

#[cfg(not(feature = "cuda"))]
pub fn is_gpu_available() -> bool {
    false
}

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use device::{detect_devices, GpuDevice, GpuDeviceHandle};
pub use memory::GpuVector;
pub use solver::{GpuLinearSolver, SolverBackend};
pub use sparse::GpuSparseMatrix;
pub use transfer::MeshGpuData;
