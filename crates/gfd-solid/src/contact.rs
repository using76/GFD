//! Contact mechanics.

use gfd_core::UnstructuredMesh;
use crate::{SolidState, Result};

/// Contact detection and enforcement.
///
/// Supports node-to-surface and surface-to-surface contact
/// with penalty or Lagrange multiplier enforcement.
pub struct ContactSolver {
    /// Contact stiffness (penalty parameter).
    pub penalty_stiffness: f64,
    /// Friction coefficient.
    pub friction_coefficient: f64,
}

impl ContactSolver {
    /// Creates a new contact solver.
    pub fn new(penalty_stiffness: f64, friction_coefficient: f64) -> Self {
        Self {
            penalty_stiffness,
            friction_coefficient,
        }
    }

    /// Detects contact and computes contact forces.
    pub fn detect_and_enforce(
        &self,
        _state: &mut SolidState,
        _mesh: &UnstructuredMesh,
    ) -> Result<()> {
        let num_cells = _state.num_cells();
        let k_pen = self.penalty_stiffness;
        let _mu_f = self.friction_coefficient;

        // Simple penalty contact: check each boundary face for penetration
        // For each boundary face, compute gap = displacement . normal
        // If gap < 0 (penetration), apply penalty force F = k_pen * |gap| * normal
        for face in &_mesh.faces {
            if face.neighbor_cell.is_some() {
                continue; // Skip internal faces
            }
            let cell_id = face.owner_cell;
            if cell_id >= num_cells {
                continue;
            }

            let disp = _state.displacement.get(cell_id).unwrap_or([0.0; 3]);
            let n = face.normal;

            // Gap = displacement projected onto outward normal
            let gap = disp[0] * n[0] + disp[1] * n[1] + disp[2] * n[2];

            if gap < 0.0 {
                // Penetration detected: apply penalty force
                let penalty_mag = k_pen * gap.abs();
                let mut current_disp = disp;
                for dim in 0..3 {
                    current_disp[dim] += penalty_mag * n[dim];
                }
                let _ = _state.displacement.set(cell_id, current_disp);
            }
        }

        Ok(())
    }
}
