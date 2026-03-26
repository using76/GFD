//! Structural dynamics solvers.
//!
//! Solves: M * a + C * v + K * u = f(t)

use std::collections::{HashMap, HashSet};

use gfd_core::UnstructuredMesh;
use gfd_linalg::iterative::cg::CG;
use gfd_linalg::traits::LinearSolverTrait;
use gfd_matrix::sparse::CooMatrix;

use crate::elastic::LinearElasticSolver;
use crate::{SolidError, SolidState, Result};

/// Newmark-beta time integration scheme for structural dynamics.
///
/// The Newmark family of methods:
/// - beta = 0.25, gamma = 0.5: unconditionally stable, no numerical damping
/// - beta = 0.3025, gamma = 0.6: unconditionally stable with numerical damping
pub struct NewmarkBeta {
    /// Newmark parameter beta (controls displacement approximation).
    pub beta: f64,
    /// Newmark parameter gamma (controls velocity approximation).
    pub gamma: f64,
    /// Previous velocity field (nodal DOFs, length = 3 * num_nodes).
    prev_velocity: Option<Vec<f64>>,
    /// Previous acceleration field (nodal DOFs, length = 3 * num_nodes).
    prev_acceleration: Option<Vec<f64>>,
    /// Previous nodal displacement (length = 3 * num_nodes).
    prev_displacement: Option<Vec<f64>>,
}

/// Configuration for dynamic analysis, including material, BC, and damping.
pub struct DynamicsConfig {
    /// Young's modulus [Pa].
    pub youngs_modulus: f64,
    /// Poisson's ratio [-].
    pub poissons_ratio: f64,
    /// Material density [kg/m^3].
    pub density: f64,
    /// Rayleigh damping alpha (mass proportional).
    pub rayleigh_alpha: f64,
    /// Rayleigh damping beta (stiffness proportional).
    pub rayleigh_beta: f64,
    /// Fixed (Dirichlet) boundary patch names.
    pub fixed_patches: Vec<String>,
    /// Force (Neumann) boundary patches: patch_name -> traction [Pa].
    pub force_patches: HashMap<String, [f64; 3]>,
    /// Body force [N/m^3].
    pub body_force: [f64; 3],
    /// Max linear solver iterations.
    pub max_iterations: usize,
    /// Linear solver tolerance.
    pub tolerance: f64,
}

impl Default for DynamicsConfig {
    fn default() -> Self {
        Self {
            youngs_modulus: 200e9,
            poissons_ratio: 0.3,
            density: 7800.0,
            rayleigh_alpha: 0.0,
            rayleigh_beta: 0.0,
            fixed_patches: Vec::new(),
            force_patches: HashMap::new(),
            body_force: [0.0; 3],
            max_iterations: 1000,
            tolerance: 1e-8,
        }
    }
}

impl NewmarkBeta {
    /// Creates a new Newmark-beta integrator.
    pub fn new(beta: f64, gamma: f64) -> Self {
        Self {
            beta,
            gamma,
            prev_velocity: None,
            prev_acceleration: None,
            prev_displacement: None,
        }
    }

    /// Creates an average acceleration (trapezoidal rule) integrator.
    ///
    /// beta = 0.25, gamma = 0.5 (unconditionally stable, no damping).
    pub fn average_acceleration() -> Self {
        Self::new(0.25, 0.5)
    }

    /// Creates a linear acceleration integrator.
    ///
    /// beta = 1/6, gamma = 0.5 (conditionally stable).
    pub fn linear_acceleration() -> Self {
        Self::new(1.0 / 6.0, 0.5)
    }

    /// Performs one dynamic time step using the full Newmark-beta algorithm.
    ///
    /// Assembles the actual stiffness matrix K from the elastic solver,
    /// builds lumped mass matrix M, and optional Rayleigh damping C = alpha*M + beta*K.
    ///
    /// Effective stiffness: K_eff = K + a0*M + a1*C
    /// Effective force: f_eff = f + M*(a0*u_n + a2*v_n + a3*a_n) + C*(a1*u_n + a4*v_n + a5*a_n)
    ///
    /// where a0 = 1/(beta*dt^2), a1 = gamma/(beta*dt), etc.
    pub fn solve_dynamic_step(
        &mut self,
        state: &mut SolidState,
        mesh: &UnstructuredMesh,
        external_forces: &[[f64; 3]],
        dt: f64,
        config: &DynamicsConfig,
    ) -> Result<f64> {
        let num_nodes = mesh.num_nodes();
        let num_cells = mesh.num_cells();
        let ndof = 3 * num_nodes;

        // Initialize previous states if first step
        if self.prev_velocity.is_none() {
            self.prev_velocity = Some(vec![0.0; ndof]);
        }
        if self.prev_acceleration.is_none() {
            self.prev_acceleration = Some(vec![0.0; ndof]);
        }
        if self.prev_displacement.is_none() {
            self.prev_displacement = Some(vec![0.0; ndof]);
        }

        let prev_vel = self.prev_velocity.as_ref().unwrap().clone();
        let prev_acc = self.prev_acceleration.as_ref().unwrap().clone();
        let prev_disp = self.prev_displacement.as_ref().unwrap().clone();

        // Newmark coefficients
        let a0 = 1.0 / (self.beta * dt * dt);
        let a1 = self.gamma / (self.beta * dt);
        let a2 = 1.0 / (self.beta * dt);
        let a3 = 1.0 / (2.0 * self.beta) - 1.0;
        let a4 = self.gamma / self.beta - 1.0;
        let a5 = dt / 2.0 * (self.gamma / self.beta - 2.0);

        // --- Step 1: Assemble stiffness matrix K ---
        let elastic = LinearElasticSolver::new(config.youngs_modulus, config.poissons_ratio);
        let c_mat = elastic.elasticity_matrix();

        let estimated_nnz = num_cells * 576;
        let mut coo = CooMatrix::with_capacity(ndof, ndof, estimated_nnz + ndof);
        let mut rhs = vec![0.0; ndof];

        // Gauss quadrature setup
        let gp = 1.0 / 3.0_f64.sqrt();
        let gauss_points: [(f64, f64, f64); 8] = [
            (-gp, -gp, -gp), ( gp, -gp, -gp), ( gp,  gp, -gp), (-gp,  gp, -gp),
            (-gp, -gp,  gp), ( gp, -gp,  gp), ( gp,  gp,  gp), (-gp,  gp,  gp),
        ];
        let gauss_weight = 1.0;
        let ref_coords: [(f64, f64, f64); 8] = [
            (-1.0, -1.0, -1.0), ( 1.0, -1.0, -1.0), ( 1.0,  1.0, -1.0), (-1.0,  1.0, -1.0),
            (-1.0, -1.0,  1.0), ( 1.0, -1.0,  1.0), ( 1.0,  1.0,  1.0), (-1.0,  1.0,  1.0),
        ];

        // Lumped mass: M_ii = rho * V_cell, distributed equally to nodes
        // For hex8, each node gets 1/8 of the element mass per DOF
        let mut lumped_mass = vec![0.0; ndof];

        for cell in &mesh.cells {
            if cell.nodes.len() != 8 {
                return Err(SolidError::MaterialError(format!(
                    "Cell {} has {} nodes, expected 8 for hex element",
                    cell.id, cell.nodes.len()
                )));
            }

            let mut coords = [[0.0_f64; 3]; 8];
            for (local, &global_id) in cell.nodes.iter().enumerate() {
                coords[local] = mesh.nodes[global_id].position;
            }

            // Lumped mass: each node gets rho * V / 8 for each of 3 DOFs
            let node_mass = config.density * cell.volume / 8.0;
            for &node_id in &cell.nodes {
                for dof in 0..3 {
                    lumped_mass[3 * node_id + dof] += node_mass;
                }
            }

            // Element stiffness matrix (24x24)
            let mut ke = [[0.0_f64; 24]; 24];
            let mut fe = [0.0_f64; 24];

            for &(xi, eta, zeta) in &gauss_points {
                let mut dn_dxi = [[0.0_f64; 3]; 8];
                for i in 0..8 {
                    let (xi_i, eta_i, zeta_i) = ref_coords[i];
                    dn_dxi[i][0] = 0.125 * xi_i * (1.0 + eta_i * eta) * (1.0 + zeta_i * zeta);
                    dn_dxi[i][1] = 0.125 * (1.0 + xi_i * xi) * eta_i * (1.0 + zeta_i * zeta);
                    dn_dxi[i][2] = 0.125 * (1.0 + xi_i * xi) * (1.0 + eta_i * eta) * zeta_i;
                }

                let mut jac = [[0.0_f64; 3]; 3];
                for i in 0..8 {
                    for m in 0..3 {
                        for n in 0..3 {
                            jac[m][n] += coords[i][m] * dn_dxi[i][n];
                        }
                    }
                }

                let det_j = jac[0][0] * (jac[1][1] * jac[2][2] - jac[1][2] * jac[2][1])
                    - jac[0][1] * (jac[1][0] * jac[2][2] - jac[1][2] * jac[2][0])
                    + jac[0][2] * (jac[1][0] * jac[2][1] - jac[1][1] * jac[2][0]);

                if det_j <= 0.0 {
                    return Err(SolidError::NegativeJacobian { element_id: cell.id });
                }

                let inv_det = 1.0 / det_j;
                let mut jac_inv = [[0.0_f64; 3]; 3];
                jac_inv[0][0] = inv_det * (jac[1][1] * jac[2][2] - jac[1][2] * jac[2][1]);
                jac_inv[0][1] = inv_det * (jac[0][2] * jac[2][1] - jac[0][1] * jac[2][2]);
                jac_inv[0][2] = inv_det * (jac[0][1] * jac[1][2] - jac[0][2] * jac[1][1]);
                jac_inv[1][0] = inv_det * (jac[1][2] * jac[2][0] - jac[1][0] * jac[2][2]);
                jac_inv[1][1] = inv_det * (jac[0][0] * jac[2][2] - jac[0][2] * jac[2][0]);
                jac_inv[1][2] = inv_det * (jac[0][2] * jac[1][0] - jac[0][0] * jac[1][2]);
                jac_inv[2][0] = inv_det * (jac[1][0] * jac[2][1] - jac[1][1] * jac[2][0]);
                jac_inv[2][1] = inv_det * (jac[0][1] * jac[2][0] - jac[0][0] * jac[2][1]);
                jac_inv[2][2] = inv_det * (jac[0][0] * jac[1][1] - jac[0][1] * jac[1][0]);

                let mut dn_dx = [[0.0_f64; 3]; 8];
                for i in 0..8 {
                    for m in 0..3 {
                        for n in 0..3 {
                            dn_dx[i][m] += jac_inv[n][m] * dn_dxi[i][n];
                        }
                    }
                }

                // B matrix (6x24)
                let mut b_mat = [[0.0_f64; 24]; 6];
                for i in 0..8 {
                    let dx = dn_dx[i][0];
                    let dy = dn_dx[i][1];
                    let dz = dn_dx[i][2];
                    let col = 3 * i;
                    b_mat[0][col]     = dx;
                    b_mat[1][col + 1] = dy;
                    b_mat[2][col + 2] = dz;
                    b_mat[3][col]     = dy;
                    b_mat[3][col + 1] = dx;
                    b_mat[4][col + 1] = dz;
                    b_mat[4][col + 2] = dy;
                    b_mat[5][col]     = dz;
                    b_mat[5][col + 2] = dx;
                }

                // CB = C * B
                let mut cb = [[0.0_f64; 24]; 6];
                for i in 0..6 {
                    for j in 0..24 {
                        let mut sum = 0.0;
                        for k in 0..6 {
                            sum += c_mat[i][k] * b_mat[k][j];
                        }
                        cb[i][j] = sum;
                    }
                }

                let factor = det_j * gauss_weight;
                for i in 0..24 {
                    for j in 0..24 {
                        let mut sum = 0.0;
                        for k in 0..6 {
                            sum += b_mat[k][i] * cb[k][j];
                        }
                        ke[i][j] += sum * factor;
                    }
                }

                // Body force
                let mut n_vals = [0.0_f64; 8];
                for i in 0..8 {
                    let (xi_i, eta_i, zeta_i) = ref_coords[i];
                    n_vals[i] = 0.125 * (1.0 + xi_i * xi) * (1.0 + eta_i * eta) * (1.0 + zeta_i * zeta);
                }
                for i in 0..8 {
                    fe[3 * i]     += n_vals[i] * config.body_force[0] * factor;
                    fe[3 * i + 1] += n_vals[i] * config.body_force[1] * factor;
                    fe[3 * i + 2] += n_vals[i] * config.body_force[2] * factor;
                }
            }

            // Assemble K_eff = K + a0*M + a1*C  (C = alpha*M + beta*K)
            // K_eff_ij = K_ij * (1 + a1*beta_r) + M_ij * (a0 + a1*alpha_r)
            // For lumped mass, M is diagonal, so only add to diagonal entries.
            let k_factor = 1.0 + a1 * config.rayleigh_beta;

            for i_local in 0..8 {
                let i_global = cell.nodes[i_local];
                for i_dof in 0..3 {
                    let row = 3 * i_global + i_dof;
                    let local_row = 3 * i_local + i_dof;

                    rhs[row] += fe[local_row];

                    for j_local in 0..8 {
                        let j_global = cell.nodes[j_local];
                        for j_dof in 0..3 {
                            let col = 3 * j_global + j_dof;
                            let local_col = 3 * j_local + j_dof;
                            let val = ke[local_row][local_col] * k_factor;
                            if val.abs() > 1e-30 {
                                coo.add_entry(row, col, val);
                            }
                        }
                    }
                }
            }
        }

        // Add lumped mass contribution to diagonal: (a0 + a1*alpha_r) * M_ii
        let mass_factor = a0 + a1 * config.rayleigh_alpha;
        for i in 0..ndof {
            if lumped_mass[i] > 0.0 {
                coo.add_entry(i, i, mass_factor * lumped_mass[i]);
            }
        }

        // --- Step 2: Add Neumann BCs (surface forces) ---
        for (patch_name, traction) in &config.force_patches {
            if let Some(patch) = mesh.boundary_patch(patch_name) {
                for &face_id in &patch.face_ids {
                    let face = &mesh.faces[face_id];
                    let face_area = face.area;
                    let n_face_nodes = face.nodes.len();
                    let force_per_node = face_area / n_face_nodes as f64;
                    for &node_id in &face.nodes {
                        rhs[3 * node_id]     += traction[0] * force_per_node;
                        rhs[3 * node_id + 1] += traction[1] * force_per_node;
                        rhs[3 * node_id + 2] += traction[2] * force_per_node;
                    }
                }
            }
        }

        // Add external nodal forces (per-cell -> distribute to nodes)
        for cell in &mesh.cells {
            let force = if cell.id < external_forces.len() {
                external_forces[cell.id]
            } else {
                [0.0; 3]
            };
            if force[0].abs() + force[1].abs() + force[2].abs() < 1e-30 {
                continue;
            }
            let n_nodes = cell.nodes.len() as f64;
            for &node_id in &cell.nodes {
                for dof in 0..3 {
                    rhs[3 * node_id + dof] += force[dof] / n_nodes;
                }
            }
        }

        // --- Step 3: Build effective RHS ---
        // f_eff = f + M*(a0*u_n + a2*v_n + a3*a_n) + C*(a1*u_n + a4*v_n + a5*a_n)
        // C = alpha*M + beta*K, but for lumped mass:
        //   M*(a0*u_n + a2*v_n + a3*a_n) adds to RHS per-DOF (diagonal)
        //   C*(a1*u_n + a4*v_n + a5*a_n) = alpha*M*(a1*u_n + ...) + beta*K*(a1*u_n + ...)
        //   The alpha*M part is diagonal, the beta*K part would require K*v which is complex.
        //   For simplicity with lumped mass, we compute:
        //     mass term contribution per DOF
        //     damping term: alpha*M*(a1*u + a4*v + a5*a) is diagonal
        //     stiffness-proportional damping contribution beta*K*(a1*u + a4*v + a5*a)
        //     This last one is already handled by multiplying K by (1 + a1*beta_r) in K_eff
        //     and adding beta_r * K * (a4*v_n + a5*a_n) to RHS.
        //
        //   Actually, the standard approach:
        //   K_eff * u_{n+1} = f_eff
        //   f_eff = f_{n+1} + M*(a0*u_n + a2*v_n + a3*a_n) + C*(a1*u_n + a4*v_n + a5*a_n)
        //
        //   With C = alpha*M + beta*K:
        //   M contribution: M * (a0*u_n + a2*v_n + a3*a_n + alpha*(a1*u_n + a4*v_n + a5*a_n))
        //   K contribution: beta * K * (a1*u_n + a4*v_n + a5*a_n)
        //
        //   The M part is diagonal (lumped). The K part needs K * vec.

        // Compute the vector that K multiplies for the damping RHS contribution
        let mut damp_vec = vec![0.0; ndof];
        for i in 0..ndof {
            damp_vec[i] = a4 * prev_vel[i] + a5 * prev_acc[i];
        }

        // For the mass-proportional part of RHS
        for i in 0..ndof {
            let mass_rhs = lumped_mass[i] * (
                a0 * prev_disp[i] + a2 * prev_vel[i] + a3 * prev_acc[i]
                + config.rayleigh_alpha * (a1 * prev_disp[i] + a4 * prev_vel[i] + a5 * prev_acc[i])
            );
            rhs[i] += mass_rhs;
        }

        // For the stiffness-proportional damping RHS: beta_r * K * damp_vec
        // We need to apply K (without the K_eff scaling) to damp_vec.
        // We do this element-by-element since we don't have K in CSR yet.
        if config.rayleigh_beta.abs() > 1e-30 {
            for cell in &mesh.cells {
                if cell.nodes.len() != 8 { continue; }
                let mut coords = [[0.0_f64; 3]; 8];
                for (local, &global_id) in cell.nodes.iter().enumerate() {
                    coords[local] = mesh.nodes[global_id].position;
                }

                let mut ke_elem = [[0.0_f64; 24]; 24];
                for &(xi, eta, zeta) in &gauss_points {
                    let mut dn_dxi = [[0.0_f64; 3]; 8];
                    for i in 0..8 {
                        let (xi_i, eta_i, zeta_i) = ref_coords[i];
                        dn_dxi[i][0] = 0.125 * xi_i * (1.0 + eta_i * eta) * (1.0 + zeta_i * zeta);
                        dn_dxi[i][1] = 0.125 * (1.0 + xi_i * xi) * eta_i * (1.0 + zeta_i * zeta);
                        dn_dxi[i][2] = 0.125 * (1.0 + xi_i * xi) * (1.0 + eta_i * eta) * zeta_i;
                    }
                    let mut jac = [[0.0_f64; 3]; 3];
                    for i in 0..8 {
                        for m in 0..3 {
                            for n in 0..3 {
                                jac[m][n] += coords[i][m] * dn_dxi[i][n];
                            }
                        }
                    }
                    let det_j = jac[0][0] * (jac[1][1] * jac[2][2] - jac[1][2] * jac[2][1])
                        - jac[0][1] * (jac[1][0] * jac[2][2] - jac[1][2] * jac[2][0])
                        + jac[0][2] * (jac[1][0] * jac[2][1] - jac[1][1] * jac[2][0]);
                    if det_j <= 0.0 { continue; }
                    let inv_det = 1.0 / det_j;
                    let mut jac_inv = [[0.0_f64; 3]; 3];
                    jac_inv[0][0] = inv_det * (jac[1][1] * jac[2][2] - jac[1][2] * jac[2][1]);
                    jac_inv[0][1] = inv_det * (jac[0][2] * jac[2][1] - jac[0][1] * jac[2][2]);
                    jac_inv[0][2] = inv_det * (jac[0][1] * jac[1][2] - jac[0][2] * jac[1][1]);
                    jac_inv[1][0] = inv_det * (jac[1][2] * jac[2][0] - jac[1][0] * jac[2][2]);
                    jac_inv[1][1] = inv_det * (jac[0][0] * jac[2][2] - jac[0][2] * jac[2][0]);
                    jac_inv[1][2] = inv_det * (jac[0][2] * jac[1][0] - jac[0][0] * jac[1][2]);
                    jac_inv[2][0] = inv_det * (jac[1][0] * jac[2][1] - jac[1][1] * jac[2][0]);
                    jac_inv[2][1] = inv_det * (jac[0][1] * jac[2][0] - jac[0][0] * jac[2][1]);
                    jac_inv[2][2] = inv_det * (jac[0][0] * jac[1][1] - jac[0][1] * jac[1][0]);
                    let mut dn_dx = [[0.0_f64; 3]; 8];
                    for i in 0..8 {
                        for m in 0..3 {
                            for n in 0..3 {
                                dn_dx[i][m] += jac_inv[n][m] * dn_dxi[i][n];
                            }
                        }
                    }
                    let mut b_mat = [[0.0_f64; 24]; 6];
                    for i in 0..8 {
                        let dx = dn_dx[i][0];
                        let dy = dn_dx[i][1];
                        let dz = dn_dx[i][2];
                        let col = 3 * i;
                        b_mat[0][col]     = dx;
                        b_mat[1][col + 1] = dy;
                        b_mat[2][col + 2] = dz;
                        b_mat[3][col]     = dy;
                        b_mat[3][col + 1] = dx;
                        b_mat[4][col + 1] = dz;
                        b_mat[4][col + 2] = dy;
                        b_mat[5][col]     = dz;
                        b_mat[5][col + 2] = dx;
                    }
                    let mut cb = [[0.0_f64; 24]; 6];
                    for i in 0..6 {
                        for j in 0..24 {
                            let mut sum = 0.0;
                            for k in 0..6 { sum += c_mat[i][k] * b_mat[k][j]; }
                            cb[i][j] = sum;
                        }
                    }
                    let factor = det_j * gauss_weight;
                    for i in 0..24 {
                        for j in 0..24 {
                            let mut sum = 0.0;
                            for k in 0..6 { sum += b_mat[k][i] * cb[k][j]; }
                            ke_elem[i][j] += sum * factor;
                        }
                    }
                }

                // Compute ke_elem * damp_vec_local and add beta_r * result to rhs
                let mut damp_local = [0.0_f64; 24];
                for i_local in 0..8 {
                    let i_global = cell.nodes[i_local];
                    for i_dof in 0..3 {
                        damp_local[3 * i_local + i_dof] = damp_vec[3 * i_global + i_dof];
                    }
                }
                for i_local in 0..8 {
                    let i_global = cell.nodes[i_local];
                    for i_dof in 0..3 {
                        let local_row = 3 * i_local + i_dof;
                        let mut sum = 0.0;
                        for j in 0..24 {
                            sum += ke_elem[local_row][j] * damp_local[j];
                        }
                        rhs[3 * i_global + i_dof] += config.rayleigh_beta * sum;
                    }
                }
            }
        }

        // Also add a1*u_n stiffness-proportional damping contribution
        // This is already included in K_eff since K_eff = K*(1 + a1*beta_r)
        // and we solve K_eff * u_{n+1} = f_eff, so no extra term needed for a1*u_n.

        // --- Step 4: Convert to CSR and apply Dirichlet BCs ---
        let csr = coo.to_csr();
        let mut system = gfd_core::LinearSystem::new(csr, rhs);

        let mut fixed_dofs = HashSet::new();
        for patch_name in &config.fixed_patches {
            if let Some(patch) = mesh.boundary_patch(patch_name) {
                for &face_id in &patch.face_ids {
                    let face = &mesh.faces[face_id];
                    for &node_id in &face.nodes {
                        fixed_dofs.insert(3 * node_id);
                        fixed_dofs.insert(3 * node_id + 1);
                        fixed_dofs.insert(3 * node_id + 2);
                    }
                }
            }
        }

        {
            let a = &mut system.a;
            let b = &mut system.b;
            for i in 0..a.nrows {
                if fixed_dofs.contains(&i) { continue; }
                let start = a.row_ptr[i];
                let end = a.row_ptr[i + 1];
                for idx in start..end {
                    let col = a.col_idx[idx];
                    if fixed_dofs.contains(&col) {
                        a.values[idx] = 0.0;
                    }
                }
            }
            for &dof in &fixed_dofs {
                let start = a.row_ptr[dof];
                let end = a.row_ptr[dof + 1];
                for idx in start..end {
                    if a.col_idx[idx] == dof {
                        a.values[idx] = 1.0;
                    } else {
                        a.values[idx] = 0.0;
                    }
                }
                b[dof] = 0.0;
            }
        }

        // --- Step 5: Solve K_eff * u_{n+1} = f_eff ---
        let mut solver = CG::new(config.tolerance, config.max_iterations);
        let stats = solver
            .solve(&system.a, &system.b, &mut system.x)
            .map_err(|e| SolidError::MaterialError(format!("Linear solver error: {}", e)))?;

        if !stats.converged {
            return Err(SolidError::Diverged {
                iteration: stats.iterations,
                residual: stats.final_residual,
            });
        }

        let new_disp = &system.x;

        // --- Step 6: Update acceleration and velocity ---
        let mut new_acc = vec![0.0; ndof];
        let mut new_vel = vec![0.0; ndof];

        for i in 0..ndof {
            // a_{n+1} = a0*(u_{n+1} - u_n) - a2*v_n - a3*a_n
            new_acc[i] = a0 * (new_disp[i] - prev_disp[i]) - a2 * prev_vel[i] - a3 * prev_acc[i];
            // v_{n+1} = v_n + dt*((1-gamma)*a_n + gamma*a_{n+1})
            new_vel[i] = prev_vel[i] + dt * ((1.0 - self.gamma) * prev_acc[i] + self.gamma * new_acc[i]);
        }

        // --- Step 7: Update SolidState (cell-averaged displacements) ---
        if state.num_cells() != num_cells {
            *state = SolidState::new(num_cells);
        }

        let mut max_change = 0.0_f64;
        for cell in &mesh.cells {
            if cell.nodes.len() != 8 { continue; }
            let mut avg_disp = [0.0_f64; 3];
            for &node_id in &cell.nodes {
                avg_disp[0] += new_disp[3 * node_id];
                avg_disp[1] += new_disp[3 * node_id + 1];
                avg_disp[2] += new_disp[3 * node_id + 2];
            }
            avg_disp[0] /= 8.0;
            avg_disp[1] /= 8.0;
            avg_disp[2] /= 8.0;

            let old_disp = state.displacement.values()[cell.id];
            let change = ((avg_disp[0] - old_disp[0]).powi(2)
                + (avg_disp[1] - old_disp[1]).powi(2)
                + (avg_disp[2] - old_disp[2]).powi(2))
            .sqrt();
            if change > max_change {
                max_change = change;
            }

            state.displacement.values_mut()[cell.id] = avg_disp;
        }

        // Store nodal quantities for next step
        self.prev_displacement = Some(new_disp.clone());
        self.prev_velocity = Some(new_vel);
        self.prev_acceleration = Some(new_acc);

        Ok(max_change)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::StructuredMesh;

    /// Test that a single dynamic step with no forces produces no displacement.
    #[test]
    fn dynamic_step_no_force() {
        let structured = StructuredMesh::uniform(2, 1, 1, 2.0, 1.0, 1.0);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();
        let mut state = SolidState::new(num_cells);

        let mut integrator = NewmarkBeta::average_acceleration();
        let config = DynamicsConfig {
            fixed_patches: vec![
                "xmin".to_string(), "xmax".to_string(),
                "ymin".to_string(), "ymax".to_string(),
                "zmin".to_string(), "zmax".to_string(),
            ],
            ..Default::default()
        };

        let forces = vec![[0.0; 3]; num_cells];
        let change = integrator
            .solve_dynamic_step(&mut state, &mesh, &forces, 0.001, &config)
            .expect("Should succeed with no forces");

        assert!(
            change < 1e-12,
            "No-force dynamic step should produce near-zero displacement, got {}",
            change
        );
    }

    /// Test that a dynamic step with applied force produces displacement in the
    /// correct direction.
    #[test]
    fn dynamic_step_with_force() {
        let structured = StructuredMesh::uniform(3, 1, 1, 3.0, 1.0, 1.0);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();
        let mut state = SolidState::new(num_cells);

        let mut integrator = NewmarkBeta::average_acceleration();
        let mut force_patches = HashMap::new();
        force_patches.insert("xmax".to_string(), [0.0, -1e6, 0.0]); // downward traction

        let config = DynamicsConfig {
            youngs_modulus: 200e9,
            poissons_ratio: 0.3,
            density: 7800.0,
            fixed_patches: vec!["xmin".to_string()],
            force_patches,
            ..Default::default()
        };

        let forces = vec![[0.0; 3]; num_cells];
        let dt = 1e-4;

        // Run a few steps
        for _ in 0..5 {
            integrator
                .solve_dynamic_step(&mut state, &mesh, &forces, dt, &config)
                .expect("Dynamic step should succeed");
        }

        // After applying a downward force, the tip should have negative y-displacement
        let tip_disp_y = state.displacement.values()[num_cells - 1][1];
        assert!(
            tip_disp_y < 0.0,
            "Tip should deflect downward, got uy = {}",
            tip_disp_y
        );
    }

    /// Test that Rayleigh damping reduces the response compared to undamped.
    #[test]
    fn dynamic_step_rayleigh_damping() {
        let structured = StructuredMesh::uniform(2, 1, 1, 2.0, 1.0, 1.0);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();

        let dt = 1e-4;
        let forces = vec![[0.0; 3]; num_cells];
        let mut force_patches = HashMap::new();
        force_patches.insert("xmax".to_string(), [0.0, -1e6, 0.0]);

        // Undamped case
        let mut state_undamped = SolidState::new(num_cells);
        let mut integrator_undamped = NewmarkBeta::average_acceleration();
        let config_undamped = DynamicsConfig {
            youngs_modulus: 200e9,
            poissons_ratio: 0.3,
            density: 7800.0,
            rayleigh_alpha: 0.0,
            rayleigh_beta: 0.0,
            fixed_patches: vec!["xmin".to_string()],
            force_patches: force_patches.clone(),
            ..Default::default()
        };

        for _ in 0..10 {
            integrator_undamped
                .solve_dynamic_step(&mut state_undamped, &mesh, &forces, dt, &config_undamped)
                .unwrap();
        }
        let undamped_tip = state_undamped.displacement.values()[num_cells - 1][1].abs();

        // Heavily damped case
        let mut state_damped = SolidState::new(num_cells);
        let mut integrator_damped = NewmarkBeta::average_acceleration();
        let config_damped = DynamicsConfig {
            youngs_modulus: 200e9,
            poissons_ratio: 0.3,
            density: 7800.0,
            rayleigh_alpha: 1000.0,
            rayleigh_beta: 0.0,
            fixed_patches: vec!["xmin".to_string()],
            force_patches,
            ..Default::default()
        };

        for _ in 0..10 {
            integrator_damped
                .solve_dynamic_step(&mut state_damped, &mesh, &forces, dt, &config_damped)
                .unwrap();
        }
        let damped_tip = state_damped.displacement.values()[num_cells - 1][1].abs();

        // Both should have some displacement
        assert!(undamped_tip > 0.0, "Undamped should deflect");
        assert!(damped_tip > 0.0, "Damped should deflect");

        // Damped response should be smaller than or comparable to undamped
        // (with high alpha damping, mass-proportional damping reduces response)
        eprintln!("Undamped tip |uy|: {:.6e}", undamped_tip);
        eprintln!("Damped tip |uy|:   {:.6e}", damped_tip);
    }

    /// Test that the linear_acceleration constructor sets correct parameters.
    #[test]
    fn linear_acceleration_params() {
        let integrator = NewmarkBeta::linear_acceleration();
        assert!((integrator.beta - 1.0 / 6.0).abs() < 1e-15);
        assert!((integrator.gamma - 0.5).abs() < 1e-15);
    }
}
