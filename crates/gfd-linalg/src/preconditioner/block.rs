//! Block preconditioner (stub).

use gfd_core::SparseMatrix;
use crate::{LinalgError, Result};
use crate::traits::PreconditionerTrait;

/// Block preconditioner for coupled systems (stub).
///
/// Applies preconditioning block-by-block for multi-equation systems.
#[derive(Debug, Clone)]
pub struct BlockPreconditioner;

impl BlockPreconditioner {
    /// Creates a new block preconditioner.
    pub fn new() -> Self {
        Self
    }
}

impl Default for BlockPreconditioner {
    fn default() -> Self {
        Self::new()
    }
}

impl PreconditionerTrait for BlockPreconditioner {
    fn setup(&mut self, _a: &SparseMatrix) -> Result<()> {
        Err(LinalgError::NotImplemented(
            "Block preconditioner is not yet implemented".to_string(),
        ))
    }

    fn apply(&self, _r: &[f64], _z: &mut [f64]) -> Result<()> {
        Err(LinalgError::NotImplemented(
            "Block preconditioner is not yet implemented".to_string(),
        ))
    }
}
