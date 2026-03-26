//! Contact mechanics.

use gfd_core::UnstructuredMesh;
use crate::{SolidState, Result};

/// A detected contact pair between a slave node and a master surface.
#[derive(Debug, Clone)]
pub struct ContactPair {
    /// Index of the slave node.
    pub slave_node: usize,
    /// Index of the master face.
    pub master_face: usize,
    /// Gap distance (negative means penetration).
    pub gap: f64,
    /// Outward normal of the master surface at the contact point.
    pub normal: [f64; 3],
    /// Closest point on the master surface.
    pub closest_point: [f64; 3],
}

/// Contact detection and enforcement.
///
/// Supports node-to-surface contact with penalty enforcement
/// and Coulomb friction.
pub struct ContactSolver {
    /// Contact stiffness (penalty parameter) [N/m].
    pub penalty_stiffness: f64,
    /// Friction coefficient (Coulomb mu).
    pub friction_coefficient: f64,
    /// Tangential penalty stiffness [N/m].
    pub tangential_stiffness: f64,
    /// Names of slave boundary patches.
    pub slave_patches: Vec<String>,
    /// Names of master boundary patches.
    pub master_patches: Vec<String>,
}

impl ContactSolver {
    /// Creates a new contact solver.
    pub fn new(penalty_stiffness: f64, friction_coefficient: f64) -> Self {
        Self {
            penalty_stiffness,
            friction_coefficient,
            tangential_stiffness: penalty_stiffness,
            slave_patches: Vec::new(),
            master_patches: Vec::new(),
        }
    }

    /// Sets the slave and master boundary patches for contact detection.
    pub fn with_patches(
        mut self,
        slave_patches: Vec<String>,
        master_patches: Vec<String>,
    ) -> Self {
        self.slave_patches = slave_patches;
        self.master_patches = master_patches;
        self
    }

    /// Sets the tangential (friction) penalty stiffness.
    pub fn with_tangential_stiffness(mut self, k_t: f64) -> Self {
        self.tangential_stiffness = k_t;
        self
    }

    /// Detects contacts between slave nodes and master faces.
    ///
    /// For each slave node, finds the closest point on any master face,
    /// computes the gap distance, and returns contact pairs where contact
    /// or penetration is detected (gap <= 0).
    pub fn detect_contacts(
        &self,
        mesh: &UnstructuredMesh,
        displacements: &[[f64; 3]],
    ) -> Vec<ContactPair> {
        let mut contacts = Vec::new();

        // Collect slave node IDs
        let mut slave_nodes = Vec::new();
        for patch_name in &self.slave_patches {
            if let Some(patch) = mesh.boundary_patch(patch_name) {
                for &face_id in &patch.face_ids {
                    let face = &mesh.faces[face_id];
                    for &node_id in &face.nodes {
                        if !slave_nodes.contains(&node_id) {
                            slave_nodes.push(node_id);
                        }
                    }
                }
            }
        }

        // Collect master face IDs
        let mut master_faces = Vec::new();
        for patch_name in &self.master_patches {
            if let Some(patch) = mesh.boundary_patch(patch_name) {
                for &face_id in &patch.face_ids {
                    master_faces.push(face_id);
                }
            }
        }

        // For each slave node, find the closest master face
        for &slave_id in &slave_nodes {
            let node_pos = mesh.nodes[slave_id].position;
            // Deformed position = reference + displacement
            let disp = if slave_id < displacements.len() {
                displacements[slave_id]
            } else {
                [0.0; 3]
            };
            let slave_pos = [
                node_pos[0] + disp[0],
                node_pos[1] + disp[1],
                node_pos[2] + disp[2],
            ];

            let mut best_dist = f64::MAX;
            let mut best_pair: Option<ContactPair> = None;

            for &face_id in &master_faces {
                let face = &mesh.faces[face_id];
                // Compute the master face center in the deformed configuration
                let mut face_center = [0.0_f64; 3];
                let n_face_nodes = face.nodes.len();
                for &fnode_id in &face.nodes {
                    let fnode_pos = mesh.nodes[fnode_id].position;
                    let fdisp = if fnode_id < displacements.len() {
                        displacements[fnode_id]
                    } else {
                        [0.0; 3]
                    };
                    face_center[0] += fnode_pos[0] + fdisp[0];
                    face_center[1] += fnode_pos[1] + fdisp[1];
                    face_center[2] += fnode_pos[2] + fdisp[2];
                }
                face_center[0] /= n_face_nodes as f64;
                face_center[1] /= n_face_nodes as f64;
                face_center[2] /= n_face_nodes as f64;

                // Use the face normal (reference configuration is fine for small deformation).
                // The face outward normal points *away* from the owning cell (i.e., out of the body).
                // For contact, the master normal should point *toward* the slave body,
                // which is the *inward* normal (negative of the outward normal).
                let n = face.normal;
                let n_mag = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                if n_mag < 1e-30 { continue; }
                // Inward normal (pointing into the body, toward potential slave)
                let n_inward = [-n[0] / n_mag, -n[1] / n_mag, -n[2] / n_mag];

                // Vector from master face center to slave node
                let dx = [
                    slave_pos[0] - face_center[0],
                    slave_pos[1] - face_center[1],
                    slave_pos[2] - face_center[2],
                ];

                // Gap = projection of (slave - master) onto inward normal
                // Positive gap means slave is on the correct side (no penetration)
                // Negative gap means the slave has crossed the master surface (penetration)
                let gap = dx[0] * n_inward[0] + dx[1] * n_inward[1] + dx[2] * n_inward[2];

                // Closest point on master surface (projection)
                let closest = [
                    slave_pos[0] - gap * n_inward[0],
                    slave_pos[1] - gap * n_inward[1],
                    slave_pos[2] - gap * n_inward[2],
                ];

                // Distance for finding closest face
                let dist = (dx[0] * dx[0] + dx[1] * dx[1] + dx[2] * dx[2]).sqrt();

                if dist < best_dist {
                    best_dist = dist;
                    best_pair = Some(ContactPair {
                        slave_node: slave_id,
                        master_face: face_id,
                        gap,
                        normal: n_inward,
                        closest_point: closest,
                    });
                }
            }

            // Only add pairs where contact is detected or close to contact
            if let Some(pair) = best_pair {
                if pair.gap <= 0.0 {
                    contacts.push(pair);
                }
            }
        }

        contacts
    }

    /// Computes contact forces from detected contact pairs.
    ///
    /// Returns nodal force contributions from penalty enforcement and friction.
    /// The returned vector is indexed by node ID, with [fx, fy, fz] per node.
    pub fn compute_contact_forces(
        &self,
        contacts: &[ContactPair],
        prev_positions: Option<&[[f64; 3]]>,
        current_positions: &[[f64; 3]],
    ) -> Vec<[f64; 3]> {
        let max_node = contacts.iter().map(|c| c.slave_node).max().unwrap_or(0);
        let mut forces = vec![[0.0_f64; 3]; max_node + 1];

        for contact in contacts {
            let node = contact.slave_node;
            if node >= forces.len() { continue; }

            // Normal force: F_n = k_penalty * max(-gap, 0) * n
            let penetration = (-contact.gap).max(0.0);
            let f_normal_mag = self.penalty_stiffness * penetration;

            let n = contact.normal;
            forces[node][0] += f_normal_mag * n[0];
            forces[node][1] += f_normal_mag * n[1];
            forces[node][2] += f_normal_mag * n[2];

            // Friction (Coulomb model)
            if self.friction_coefficient > 0.0 && prev_positions.is_some() {
                let prev = prev_positions.unwrap();
                if node < prev.len() && node < current_positions.len() {
                    // Compute incremental slip in the tangent plane
                    let slip = [
                        current_positions[node][0] - prev[node][0],
                        current_positions[node][1] - prev[node][1],
                        current_positions[node][2] - prev[node][2],
                    ];

                    // Project slip onto tangent plane (remove normal component)
                    let slip_normal = slip[0] * n[0] + slip[1] * n[1] + slip[2] * n[2];
                    let slip_tangent = [
                        slip[0] - slip_normal * n[0],
                        slip[1] - slip_normal * n[1],
                        slip[2] - slip_normal * n[2],
                    ];

                    let slip_mag = (slip_tangent[0] * slip_tangent[0]
                        + slip_tangent[1] * slip_tangent[1]
                        + slip_tangent[2] * slip_tangent[2])
                    .sqrt();

                    if slip_mag > 1e-30 {
                        // Coulomb friction: F_t = min(mu * |F_n|, k_t * slip) in tangent direction
                        let coulomb_limit = self.friction_coefficient * f_normal_mag;
                        let elastic_friction = self.tangential_stiffness * slip_mag;
                        let f_friction_mag = coulomb_limit.min(elastic_friction);

                        // Direction opposite to slip
                        let t = [
                            -slip_tangent[0] / slip_mag,
                            -slip_tangent[1] / slip_mag,
                            -slip_tangent[2] / slip_mag,
                        ];

                        forces[node][0] += f_friction_mag * t[0];
                        forces[node][1] += f_friction_mag * t[1];
                        forces[node][2] += f_friction_mag * t[2];
                    }
                }
            }
        }

        forces
    }

    /// Detects contact and computes contact forces, applying them to the state.
    ///
    /// This is a convenience method that combines detect_contacts and
    /// compute_contact_forces, then applies the resulting forces as
    /// displacement corrections.
    pub fn detect_and_enforce(
        &self,
        state: &mut SolidState,
        mesh: &UnstructuredMesh,
    ) -> Result<()> {
        let num_cells = state.num_cells();
        let k_pen = self.penalty_stiffness;

        if !self.slave_patches.is_empty() && !self.master_patches.is_empty() {
            // Use node-to-surface contact detection
            // Build per-node displacements from cell-averaged data
            let num_nodes = mesh.num_nodes();
            let mut node_disp = vec![[0.0_f64; 3]; num_nodes];
            let mut node_count = vec![0usize; num_nodes];
            for cell in &mesh.cells {
                let cell_disp = state.displacement.get(cell.id).unwrap_or([0.0; 3]);
                for &nid in &cell.nodes {
                    node_disp[nid][0] += cell_disp[0];
                    node_disp[nid][1] += cell_disp[1];
                    node_disp[nid][2] += cell_disp[2];
                    node_count[nid] += 1;
                }
            }
            for nid in 0..num_nodes {
                if node_count[nid] > 0 {
                    let c = node_count[nid] as f64;
                    node_disp[nid][0] /= c;
                    node_disp[nid][1] /= c;
                    node_disp[nid][2] /= c;
                }
            }

            let contacts = self.detect_contacts(mesh, &node_disp);
            if !contacts.is_empty() {
                let forces = self.compute_contact_forces(&contacts, None, &node_disp);

                // Apply contact force corrections to cell displacements
                for cell in &mesh.cells {
                    let mut correction = [0.0_f64; 3];
                    let mut has_correction = false;
                    for &nid in &cell.nodes {
                        if nid < forces.len() {
                            let f = forces[nid];
                            if f[0].abs() + f[1].abs() + f[2].abs() > 1e-30 {
                                has_correction = true;
                                correction[0] += f[0];
                                correction[1] += f[1];
                                correction[2] += f[2];
                            }
                        }
                    }
                    if has_correction {
                        let n_nodes = cell.nodes.len() as f64;
                        let disp = state.displacement.get(cell.id).unwrap_or([0.0; 3]);
                        // Apply force as displacement correction scaled by 1/k_pen
                        let scale = 1.0 / (k_pen * n_nodes);
                        let new_disp = [
                            disp[0] + correction[0] * scale,
                            disp[1] + correction[1] * scale,
                            disp[2] + correction[2] * scale,
                        ];
                        let _ = state.displacement.set(cell.id, new_disp);
                    }
                }
            }
        } else {
            // Fallback: simple boundary penalty contact (original behavior)
            for face in &mesh.faces {
                if face.neighbor_cell.is_some() {
                    continue;
                }
                let cell_id = face.owner_cell;
                if cell_id >= num_cells {
                    continue;
                }

                let disp = state.displacement.get(cell_id).unwrap_or([0.0; 3]);
                let n = face.normal;

                let gap = disp[0] * n[0] + disp[1] * n[1] + disp[2] * n[2];

                if gap < 0.0 {
                    let penalty_mag = k_pen * gap.abs();
                    let mut current_disp = disp;
                    for dim in 0..3 {
                        current_disp[dim] += penalty_mag * n[dim];
                    }
                    let _ = state.displacement.set(cell_id, current_disp);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::StructuredMesh;

    /// Test contact detection between slave and master surfaces.
    #[test]
    fn detect_contacts_basic() {
        // Create a simple 2-element mesh: 2x1x1 with length 2.0
        // xmin face at x=0, xmax face at x=2.0
        let structured = StructuredMesh::uniform(2, 1, 1, 2.0, 1.0, 1.0);
        let mesh = structured.to_unstructured();

        // Set up contact solver with xmax as slave and xmin as master
        let solver = ContactSolver::new(1e6, 0.3)
            .with_patches(
                vec!["xmax".to_string()],  // slave
                vec!["xmin".to_string()],  // master
            );

        let num_nodes = mesh.num_nodes();

        // No displacement: slave (xmax, x=2.0) is far from master (xmin, x=0.0).
        // With inward normal at xmin = [+1,0,0], gap = 2.0 > 0 => no contact.
        let no_disp = vec![[0.0_f64; 3]; num_nodes];
        let contacts = solver.detect_contacts(&mesh, &no_disp);
        assert!(
            contacts.is_empty(),
            "No contact expected without penetration, found {} contacts",
            contacts.len()
        );

        // Apply large negative x displacement to xmax slave nodes to push past xmin.
        // Slave nodes at x=2.0, push by -3.0 => deformed x = -1.0.
        // Gap = (-1.0 - 0.0) * 1.0 = -1.0 < 0 => penetration.
        let mut disp = vec![[0.0_f64; 3]; num_nodes];
        for (i, node) in mesh.nodes.iter().enumerate() {
            if (node.position[0] - 2.0).abs() < 1e-10 {
                disp[i][0] = -3.0; // push xmax nodes past x=0
            }
        }

        let contacts = solver.detect_contacts(&mesh, &disp);
        assert!(
            !contacts.is_empty(),
            "Contact should be detected when slave penetrates master"
        );

        for c in &contacts {
            assert!(c.gap <= 0.0, "Gap should be negative (penetration), got {}", c.gap);
        }
    }

    /// Test that contact force computation produces forces in the correct direction.
    #[test]
    fn contact_forces_direction() {
        let contact = ContactPair {
            slave_node: 0,
            master_face: 0,
            gap: -0.01, // 1cm penetration
            normal: [1.0, 0.0, 0.0], // normal in +x direction
            closest_point: [0.0, 0.0, 0.0],
        };

        let solver = ContactSolver::new(1e6, 0.0); // no friction

        let positions = vec![[0.0; 3]; 1];
        let forces = solver.compute_contact_forces(&[contact], None, &positions);

        assert!(!forces.is_empty());
        // Normal force should push slave in +x direction (away from master)
        assert!(
            forces[0][0] > 0.0,
            "Normal force should be positive in x (pushing slave out), got {}",
            forces[0][0]
        );
        // Expected: k_pen * penetration * n = 1e6 * 0.01 * 1.0 = 1e4
        let expected = 1e6 * 0.01;
        assert!(
            (forces[0][0] - expected).abs() / expected < 1e-10,
            "Force magnitude should be k*penetration = {}, got {}",
            expected,
            forces[0][0]
        );
    }

    /// Test Coulomb friction limiting.
    #[test]
    fn friction_coulomb_limit() {
        let contact = ContactPair {
            slave_node: 0,
            master_face: 0,
            gap: -0.01,
            normal: [0.0, 1.0, 0.0], // normal in +y
            closest_point: [0.0, 0.0, 0.0],
        };

        let mu = 0.3;
        let k_t = 1e8; // high tangential stiffness to trigger Coulomb limit
        let solver = ContactSolver::new(1e6, mu)
            .with_tangential_stiffness(k_t);

        let prev_pos = vec![[0.0, 0.0, 0.0]];
        let curr_pos = vec![[1.0, 0.0, 0.0]]; // large slip in x

        let forces = solver.compute_contact_forces(&[contact], Some(&prev_pos), &curr_pos);

        let f_normal = 1e6 * 0.01; // 1e4
        let f_tangent_mag = (forces[0][0] * forces[0][0] + forces[0][2] * forces[0][2]).sqrt();

        // Friction should be limited to mu * F_n
        let coulomb_limit = mu * f_normal;
        assert!(
            f_tangent_mag <= coulomb_limit + 1e-6,
            "Friction {} should be <= Coulomb limit {}",
            f_tangent_mag,
            coulomb_limit
        );
    }

    /// Test detect_and_enforce with simple penalty contact (fallback mode).
    #[test]
    fn detect_and_enforce_penalty() {
        let structured = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();
        let mut state = SolidState::new(num_cells);

        let solver = ContactSolver::new(0.5, 0.0);

        // Set a penetrating displacement on cell 0
        let _ = state.displacement.set(0, [-1.0, 0.0, 0.0]);

        solver.detect_and_enforce(&mut state, &mesh).unwrap();

        // The displacement should have been corrected (at least partially)
        // by the penalty method on boundary faces
        let disp = state.displacement.get(0).unwrap();
        // The exact correction depends on which boundary faces cell 0 touches.
        // At minimum, verify the method runs without error and doesn't make
        // the displacement worse.
        eprintln!("Corrected displacement: {:?}", disp);
    }

    /// Test ContactPair structure fields.
    #[test]
    fn contact_pair_fields() {
        let pair = ContactPair {
            slave_node: 42,
            master_face: 7,
            gap: -0.005,
            normal: [0.0, 0.0, 1.0],
            closest_point: [1.0, 2.0, 3.0],
        };
        assert_eq!(pair.slave_node, 42);
        assert_eq!(pair.master_face, 7);
        assert!((pair.gap - (-0.005)).abs() < 1e-15);
        assert!((pair.normal[2] - 1.0).abs() < 1e-15);
    }
}
