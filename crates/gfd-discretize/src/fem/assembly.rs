//! Element-level assembly for the Finite Element Method.

use serde::{Deserialize, Serialize};

/// Element-level matrix and vector contributions.
///
/// After integrating the weak form over an element, the result is a local
/// stiffness matrix `ke` and force vector `fe`, along with the global DOF
/// indices they map to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementMatrix {
    /// Element stiffness matrix (n_dof x n_dof).
    pub ke: Vec<Vec<f64>>,
    /// Element force (load) vector (n_dof).
    pub fe: Vec<f64>,
    /// Global DOF indices corresponding to the local DOFs.
    pub dof_indices: Vec<usize>,
}

impl ElementMatrix {
    /// Creates a new zero-initialized element matrix.
    ///
    /// # Arguments
    /// * `n_dof` - Number of degrees of freedom for this element.
    /// * `dof_indices` - Global DOF indices.
    pub fn new(n_dof: usize, dof_indices: Vec<usize>) -> Self {
        Self {
            ke: vec![vec![0.0; n_dof]; n_dof],
            fe: vec![0.0; n_dof],
            dof_indices,
        }
    }

    /// Returns the number of local degrees of freedom.
    pub fn n_dof(&self) -> usize {
        self.dof_indices.len()
    }
}
