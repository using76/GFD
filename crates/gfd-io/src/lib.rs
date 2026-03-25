//! # gfd-io
//!
//! Input/output utilities for the GFD solver framework.
//! Provides JSON configuration loading, mesh reading, VTK writing,
//! checkpointing, and probe output.

pub mod json_input;
pub mod mesh_reader;
pub mod vtk_writer;
pub mod checkpoint;
pub mod probes;
pub mod vdb_writer;

use thiserror::Error;

/// Error type for the IO crate.
#[derive(Debug, Error)]
pub enum IoError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Write error: {0}")]
    WriteError(String),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("IO error: {0}")]
    StdIo(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Core error: {0}")]
    CoreError(#[from] gfd_core::CoreError),
}

/// Convenience result type for this crate.
pub type Result<T> = std::result::Result<T, IoError>;
