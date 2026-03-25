//! Coupling interface definitions.

use serde::{Deserialize, Serialize};

/// Defines a coupling interface between two solvers across a shared surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouplingInterface {
    /// Name of this coupling interface.
    pub name: String,
    /// Indices of the mesh faces on the shared surface.
    pub surface_faces: Vec<usize>,
    /// Name of the source solver.
    pub source_solver: String,
    /// Name of the target solver.
    pub target_solver: String,
    /// Names of the variables to transfer across this interface.
    pub transfer_variables: Vec<String>,
}

impl CouplingInterface {
    /// Creates a new coupling interface.
    pub fn new(
        name: impl Into<String>,
        surface_faces: Vec<usize>,
        source_solver: impl Into<String>,
        target_solver: impl Into<String>,
        transfer_variables: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            surface_faces,
            source_solver: source_solver.into(),
            target_solver: target_solver.into(),
            transfer_variables,
        }
    }

    /// Returns the number of faces on this interface.
    pub fn num_faces(&self) -> usize {
        self.surface_faces.len()
    }

    /// Returns the number of transfer variables.
    pub fn num_variables(&self) -> usize {
        self.transfer_variables.len()
    }
}
