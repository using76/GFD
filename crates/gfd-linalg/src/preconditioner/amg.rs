//! Algebraic Multigrid (AMG) preconditioner (stub).

use gfd_core::SparseMatrix;
use crate::{LinalgError, Result};
use crate::traits::PreconditionerTrait;

/// AMG preconditioner (stub).
///
/// Algebraic Multigrid uses coarsening and interpolation operators
/// derived from the matrix entries. This is a placeholder for a
/// future implementation.
#[derive(Debug, Clone)]
pub struct AMG;

impl AMG {
    /// Creates a new AMG preconditioner.
    pub fn new() -> Self {
        Self
    }
}

impl Default for AMG {
    fn default() -> Self {
        Self::new()
    }
}

impl PreconditionerTrait for AMG {
    fn setup(&mut self, _a: &SparseMatrix) -> Result<()> {
        Err(LinalgError::NotImplemented(
            "AMG preconditioner is not yet implemented".to_string(),
        ))
    }

    fn apply(&self, _r: &[f64], _z: &mut [f64]) -> Result<()> {
        Err(LinalgError::NotImplemented(
            "AMG preconditioner is not yet implemented".to_string(),
        ))
    }
}
