//! # gfd-parallel
//!
//! Parallelization utilities for the GFD solver framework.
//! Provides domain decomposition and thread pool management.

pub mod domain_decomp;
pub mod thread_pool;
pub mod mpi_comm;
pub mod gpu;

use thiserror::Error;

/// Error type for the parallel crate.
#[derive(Debug, Error)]
pub enum ParallelError {
    #[error("Domain decomposition failed: {0}")]
    DecompositionError(String),

    #[error("Thread pool error: {0}")]
    ThreadPoolError(String),

    #[error("Invalid partition count: requested {requested} partitions for {num_cells} cells")]
    InvalidPartitionCount { requested: usize, num_cells: usize },

    #[error("Core error: {0}")]
    CoreError(#[from] gfd_core::CoreError),
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, ParallelError>;
