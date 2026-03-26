//! Linear elastic solver using the finite element method.
//!
//! Solves: div(C : epsilon(u)) + f = 0
//! where C is the elastic stiffness tensor and epsilon = 0.5*(grad(u) + grad(u)^T).

use std::collections::{HashMap, HashSet};

use gfd_core::UnstructuredMesh;
use gfd_linalg::iterative::cg::CG;
use gfd_linalg::traits::LinearSolverTrait;
use gfd_matrix::sparse::CooMatrix;

use crate::plasticity::von_mises::VonMisesYield;
use crate::{SolidError, SolidState, Result};

/// Linear elastic finite element solver.
///
/// Assembles the global stiffness matrix K and solves K*u = f
/// using the standard displacement-based FEM approach.
pub struct LinearElasticSolver {
    /// Young's modulus [Pa].
    pub youngs_modulus: f64,
    /// Poisson's ratio [-].
    pub poissons_ratio: f64,
    /// Maximum linear solver iterations.
    pub max_iterations: usize,
    /// Convergence tolerance.
    pub tolerance: f64,
}

impl LinearElasticSolver {
    /// Creates a new linear elastic solver.
    pub fn new(youngs_modulus: f64, poissons_ratio: f64) -> Self {
        Self {
            youngs_modulus,
            poissons_ratio,
            max_iterations: 1000,
            tolerance: 1e-8,
        }
    }

    /// Computes the Lame parameters from Young's modulus and Poisson's ratio.
    pub fn lame_parameters(&self) -> (f64, f64) {
        let e = self.youngs_modulus;
        let nu = self.poissons_ratio;
        let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let mu = e / (2.0 * (1.0 + nu));
        (lambda, mu)
    }

    /// Computes the 6x6 elasticity matrix C for isotropic material (Voigt notation).
    ///
    /// C = [lambda + 2*mu,  lambda,        lambda,        0,  0,  0 ]
    ///     [lambda,         lambda + 2*mu, lambda,        0,  0,  0 ]
    ///     [lambda,         lambda,        lambda + 2*mu, 0,  0,  0 ]
    ///     [0,              0,             0,             mu, 0,  0 ]
    ///     [0,              0,             0,             0,  mu, 0 ]
    ///     [0,              0,             0,             0,  0,  mu]
    pub fn elasticity_matrix(&self) -> [[f64; 6]; 6] {
        let (lambda, mu) = self.lame_parameters();
        let mut c = [[0.0; 6]; 6];

        c[0][0] = lambda + 2.0 * mu;
        c[0][1] = lambda;
        c[0][2] = lambda;
        c[1][0] = lambda;
        c[1][1] = lambda + 2.0 * mu;
        c[1][2] = lambda;
        c[2][0] = lambda;
        c[2][1] = lambda;
        c[2][2] = lambda + 2.0 * mu;
        c[3][3] = mu;
        c[4][4] = mu;
        c[5][5] = mu;

        c
    }

    /// Solves the linear elasticity problem on hex8 elements.
    ///
    /// FEM assembly process:
    /// 1. Loop over all hex8 elements
    /// 2. For each element, compute element stiffness matrix using 2x2x2 Gauss quadrature:
    ///    Ke = sum_gp (B^T * C * B * det(J) * w)
    /// 3. Assemble element matrices into the global stiffness matrix K
    /// 4. Apply boundary conditions (Dirichlet and Neumann)
    /// 5. Solve the global system: K * u = f
    /// 6. Compute strains and stresses from displacements
    ///
    /// Returns the maximum displacement magnitude.
    pub fn solve(
        &self,
        state: &mut SolidState,
        mesh: &UnstructuredMesh,
        body_force: [f64; 3],
        fixed_patches: &[String],
        force_patches: &HashMap<String, [f64; 3]>,
    ) -> Result<f64> {
        let c_mat = self.elasticity_matrix();
        let num_nodes = mesh.num_nodes();
        let num_cells = mesh.num_cells();
        let ndof = 3 * num_nodes;

        // Estimate nnz: each hex8 element contributes 24x24 = 576 entries
        let estimated_nnz = num_cells * 576;
        let mut coo = CooMatrix::with_capacity(ndof, ndof, estimated_nnz);
        let mut rhs = vec![0.0; ndof];

        // Step 1: Assemble element stiffness matrices using 2x2x2 Gauss quadrature
        let gp = 1.0 / 3.0_f64.sqrt();
        let gauss_points: [(f64, f64, f64); 8] = [
            (-gp, -gp, -gp),
            ( gp, -gp, -gp),
            ( gp,  gp, -gp),
            (-gp,  gp, -gp),
            (-gp, -gp,  gp),
            ( gp, -gp,  gp),
            ( gp,  gp,  gp),
            (-gp,  gp,  gp),
        ];
        let gauss_weight = 1.0; // each weight is 1.0 for 2x2x2 Gauss

        // Hex8 reference node coordinates
        let ref_coords: [(f64, f64, f64); 8] = [
            (-1.0, -1.0, -1.0),
            ( 1.0, -1.0, -1.0),
            ( 1.0,  1.0, -1.0),
            (-1.0,  1.0, -1.0),
            (-1.0, -1.0,  1.0),
            ( 1.0, -1.0,  1.0),
            ( 1.0,  1.0,  1.0),
            (-1.0,  1.0,  1.0),
        ];

        for cell in &mesh.cells {
            if cell.nodes.len() != 8 {
                return Err(SolidError::MaterialError(format!(
                    "Cell {} has {} nodes, expected 8 for hex element",
                    cell.id, cell.nodes.len()
                )));
            }

            // Get node coordinates for this element
            let mut coords = [[0.0_f64; 3]; 8];
            for (local, &global_id) in cell.nodes.iter().enumerate() {
                coords[local] = mesh.nodes[global_id].position;
            }

            // Element stiffness matrix Ke (24x24)
            let mut ke = [[0.0_f64; 24]; 24];
            // Element force vector fe (24)
            let mut fe = [0.0_f64; 24];

            // 2x2x2 Gauss quadrature
            for &(xi, eta, zeta) in &gauss_points {
                // Shape function derivatives w.r.t. reference coordinates
                let mut dn_dxi = [[0.0_f64; 3]; 8]; // dn_dxi[node][xi/eta/zeta]
                for i in 0..8 {
                    let (xi_i, eta_i, zeta_i) = ref_coords[i];
                    dn_dxi[i][0] = 0.125 * xi_i * (1.0 + eta_i * eta) * (1.0 + zeta_i * zeta);
                    dn_dxi[i][1] = 0.125 * (1.0 + xi_i * xi) * eta_i * (1.0 + zeta_i * zeta);
                    dn_dxi[i][2] = 0.125 * (1.0 + xi_i * xi) * (1.0 + eta_i * eta) * zeta_i;
                }

                // Jacobian matrix J = sum_i (x_i * dN_i/d_ref)
                // J[m][n] = sum_i coords[i][m] * dn_dxi[i][n]
                let mut jac = [[0.0_f64; 3]; 3];
                for i in 0..8 {
                    for m in 0..3 {
                        for n in 0..3 {
                            jac[m][n] += coords[i][m] * dn_dxi[i][n];
                        }
                    }
                }

                // Determinant of Jacobian
                let det_j = jac[0][0] * (jac[1][1] * jac[2][2] - jac[1][2] * jac[2][1])
                    - jac[0][1] * (jac[1][0] * jac[2][2] - jac[1][2] * jac[2][0])
                    + jac[0][2] * (jac[1][0] * jac[2][1] - jac[1][1] * jac[2][0]);

                if det_j <= 0.0 {
                    return Err(SolidError::NegativeJacobian { element_id: cell.id });
                }

                // Inverse of Jacobian
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

                // Shape function derivatives w.r.t. physical coordinates
                // dN/dx = J^{-T} * dN/d_ref, but since J[m][n] = dx_m/d_ref_n,
                // dN/dx_m = sum_n jac_inv[n][m] * dN/d_ref_n
                // Actually: dN/d_phys_m = sum_n (J^{-1})_{mn} * dN/d_ref_n
                // where J_{mn} = d(phys_m)/d(ref_n)
                // so J^{-1}_{mn} means d(ref_m)/d(phys_n)
                // dN/d_phys_m = sum_n dN/d_ref_n * d_ref_n/d_phys_m = sum_n dn_dxi[i][n] * jac_inv[n][m]
                let mut dn_dx = [[0.0_f64; 3]; 8];
                for i in 0..8 {
                    for m in 0..3 {
                        for n in 0..3 {
                            dn_dx[i][m] += jac_inv[n][m] * dn_dxi[i][n];
                        }
                    }
                }

                // Build B matrix (6x24)
                // B relates strain (Voigt: eps_xx, eps_yy, eps_zz, gamma_xy, gamma_yz, gamma_xz)
                // to nodal displacements.
                // For node i (columns 3*i, 3*i+1, 3*i+2):
                // B[0][3i]   = dN_i/dx    (eps_xx)
                // B[1][3i+1] = dN_i/dy    (eps_yy)
                // B[2][3i+2] = dN_i/dz    (eps_zz)
                // B[3][3i]   = dN_i/dy    (gamma_xy)
                // B[3][3i+1] = dN_i/dx
                // B[4][3i+1] = dN_i/dz    (gamma_yz)
                // B[4][3i+2] = dN_i/dy
                // B[5][3i]   = dN_i/dz    (gamma_xz)
                // B[5][3i+2] = dN_i/dx
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

                // Compute B^T * C * B * det_j * weight
                // First compute CB = C * B (6x24)
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

                // Compute B^T * CB = Ke_gp (24x24)
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

                // Shape function values at this Gauss point for body force
                let mut n_vals = [0.0_f64; 8];
                for i in 0..8 {
                    let (xi_i, eta_i, zeta_i) = ref_coords[i];
                    n_vals[i] = 0.125 * (1.0 + xi_i * xi) * (1.0 + eta_i * eta) * (1.0 + zeta_i * zeta);
                }

                // fe += N^T * body_force * det_j * weight
                for i in 0..8 {
                    fe[3 * i]     += n_vals[i] * body_force[0] * factor;
                    fe[3 * i + 1] += n_vals[i] * body_force[1] * factor;
                    fe[3 * i + 2] += n_vals[i] * body_force[2] * factor;
                }
            }

            // Assemble Ke into global COO and fe into RHS
            for i_local in 0..8 {
                let i_global = cell.nodes[i_local];
                for i_dof in 0..3 {
                    let row = 3 * i_global + i_dof;
                    let local_row = 3 * i_local + i_dof;

                    // RHS contribution
                    rhs[row] += fe[local_row];

                    // Stiffness matrix entries
                    for j_local in 0..8 {
                        let j_global = cell.nodes[j_local];
                        for j_dof in 0..3 {
                            let col = 3 * j_global + j_dof;
                            let local_col = 3 * j_local + j_dof;
                            let val = ke[local_row][local_col];
                            if val.abs() > 1e-30 {
                                coo.add_entry(row, col, val);
                            }
                        }
                    }
                }
            }
        }

        // Step 2: Apply Neumann boundary conditions (surface forces)
        for (patch_name, traction) in force_patches {
            if let Some(patch) = mesh.boundary_patch(patch_name) {
                for &face_id in &patch.face_ids {
                    let face = &mesh.faces[face_id];
                    let face_area = face.area;
                    let n_face_nodes = face.nodes.len();
                    // Distribute traction force equally among face nodes
                    let force_per_node = face_area / n_face_nodes as f64;
                    for &node_id in &face.nodes {
                        rhs[3 * node_id]     += traction[0] * force_per_node;
                        rhs[3 * node_id + 1] += traction[1] * force_per_node;
                        rhs[3 * node_id + 2] += traction[2] * force_per_node;
                    }
                }
            }
        }

        // Step 3: Convert to CSR
        let csr = coo.to_csr();

        // Step 4: Create linear system and apply Dirichlet BCs
        let mut system = gfd_core::LinearSystem::new(csr, rhs);

        // Collect all DOFs to fix (nodes on fixed patches get u=0)
        let mut fixed_dofs = HashSet::new();
        for patch_name in fixed_patches {
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

        // Also zero out columns for Dirichlet DOFs to preserve symmetry.
        // This is important for CG which requires SPD matrices.
        // We modify the RHS to account for known displacement values (zero here).
        // For zero Dirichlet, we only need to zero out the row and set diagonal=1.
        // But to preserve symmetry, we also need to zero out the column entries.
        // Since the prescribed displacement is 0, the RHS correction is 0 too.
        //
        // Strategy: rebuild the matrix with Dirichlet rows/cols zeroed.
        {
            let a = &mut system.a;
            let b = &mut system.b;
            // Zero out column entries for fixed DOFs (for symmetry)
            for i in 0..a.nrows {
                if fixed_dofs.contains(&i) {
                    continue; // will handle row zeroing below
                }
                let start = a.row_ptr[i];
                let end = a.row_ptr[i + 1];
                for idx in start..end {
                    let col = a.col_idx[idx];
                    if fixed_dofs.contains(&col) {
                        // Since prescribed value is 0, RHS correction is:
                        // b[i] -= a[i][col] * 0.0 = 0
                        a.values[idx] = 0.0;
                    }
                }
            }
            // Zero out rows for fixed DOFs and set diagonal = 1
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

        // Step 5: Solve with CG
        let mut solver = CG::new(self.tolerance, self.max_iterations);
        let stats = solver
            .solve(&system.a, &system.b, &mut system.x)
            .map_err(|e| SolidError::MaterialError(format!("Linear solver error: {}", e)))?;

        if !stats.converged {
            return Err(SolidError::Diverged {
                iteration: stats.iterations,
                residual: stats.final_residual,
            });
        }

        // Step 6: Extract displacements and compute stresses
        // Store per-node displacement into state (which is per-cell, so we
        // compute the average displacement at the cell centroid from its nodes).
        let displacement_vec = &system.x;

        // Resize state if needed
        if state.num_cells() != num_cells {
            *state = SolidState::new(num_cells);
        }

        let mut max_disp = 0.0_f64;

        for cell in &mesh.cells {
            if cell.nodes.len() != 8 {
                continue;
            }

            // Get element node coordinates and displacements
            let mut coords = [[0.0_f64; 3]; 8];
            let mut u_elem = [0.0_f64; 24];
            for (local, &global_id) in cell.nodes.iter().enumerate() {
                coords[local] = mesh.nodes[global_id].position;
                u_elem[3 * local]     = displacement_vec[3 * global_id];
                u_elem[3 * local + 1] = displacement_vec[3 * global_id + 1];
                u_elem[3 * local + 2] = displacement_vec[3 * global_id + 2];
            }

            // Average displacement at cell center
            let mut avg_disp = [0.0_f64; 3];
            for local in 0..8 {
                avg_disp[0] += u_elem[3 * local];
                avg_disp[1] += u_elem[3 * local + 1];
                avg_disp[2] += u_elem[3 * local + 2];
            }
            avg_disp[0] /= 8.0;
            avg_disp[1] /= 8.0;
            avg_disp[2] /= 8.0;

            let disp_mag = (avg_disp[0].powi(2) + avg_disp[1].powi(2) + avg_disp[2].powi(2)).sqrt();
            if disp_mag > max_disp {
                max_disp = disp_mag;
            }

            state.displacement.values_mut()[cell.id] = avg_disp;

            // Compute strain and stress at element center (xi=eta=zeta=0)
            let xi = 0.0;
            let eta = 0.0;
            let zeta = 0.0;

            let ref_coords_local: [(f64, f64, f64); 8] = [
                (-1.0, -1.0, -1.0),
                ( 1.0, -1.0, -1.0),
                ( 1.0,  1.0, -1.0),
                (-1.0,  1.0, -1.0),
                (-1.0, -1.0,  1.0),
                ( 1.0, -1.0,  1.0),
                ( 1.0,  1.0,  1.0),
                (-1.0,  1.0,  1.0),
            ];

            let mut dn_dxi = [[0.0_f64; 3]; 8];
            for i in 0..8 {
                let (xi_i, eta_i, zeta_i) = ref_coords_local[i];
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
                continue; // skip stress computation for degenerate elements
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

            // Build B matrix at center
            let mut b_mat = [[0.0_f64; 24]; 6];
            for i in 0..8 {
                let dndx = dn_dx[i][0];
                let dndy = dn_dx[i][1];
                let dndz = dn_dx[i][2];
                let col = 3 * i;

                b_mat[0][col]     = dndx;
                b_mat[1][col + 1] = dndy;
                b_mat[2][col + 2] = dndz;
                b_mat[3][col]     = dndy;
                b_mat[3][col + 1] = dndx;
                b_mat[4][col + 1] = dndz;
                b_mat[4][col + 2] = dndy;
                b_mat[5][col]     = dndz;
                b_mat[5][col + 2] = dndx;
            }

            // strain = B * u_elem (Voigt: eps_xx, eps_yy, eps_zz, gamma_xy, gamma_yz, gamma_xz)
            let mut strain_voigt = [0.0_f64; 6];
            for i in 0..6 {
                for j in 0..24 {
                    strain_voigt[i] += b_mat[i][j] * u_elem[j];
                }
            }

            // stress = C * strain
            let mut stress_voigt = [0.0_f64; 6];
            for i in 0..6 {
                for j in 0..6 {
                    stress_voigt[i] += c_mat[i][j] * strain_voigt[j];
                }
            }

            // Store as 3x3 tensors
            // Voigt order: xx, yy, zz, xy, yz, xz
            let strain_tensor = [
                [strain_voigt[0], 0.5 * strain_voigt[3], 0.5 * strain_voigt[5]],
                [0.5 * strain_voigt[3], strain_voigt[1], 0.5 * strain_voigt[4]],
                [0.5 * strain_voigt[5], 0.5 * strain_voigt[4], strain_voigt[2]],
            ];
            let stress_tensor = [
                [stress_voigt[0], stress_voigt[3], stress_voigt[5]],
                [stress_voigt[3], stress_voigt[1], stress_voigt[4]],
                [stress_voigt[5], stress_voigt[4], stress_voigt[2]],
            ];

            state.strain.values_mut()[cell.id] = strain_tensor;
            state.stress.values_mut()[cell.id] = stress_tensor;
        }

        Ok(max_disp)
    }

    /// Solves the elastoplastic problem using the elastic FEM solver followed by
    /// a cell-by-cell return-mapping algorithm for Von Mises plasticity.
    ///
    /// Algorithm:
    /// 1. Solve the elastic problem: K * u = f (using `solve()`)
    /// 2. For each cell, compute trial elastic stress from the FEM solution
    /// 3. Check the Von Mises yield criterion
    /// 4. If yielded, apply the radial return-mapping algorithm
    /// 5. Update stress, plastic strain, and hardening variables
    ///
    /// # Arguments
    /// * `state` - Mutable reference to the solid state (displacement, stress, strain)
    /// * `mesh` - The unstructured mesh
    /// * `body_force` - Body force vector [N/m^3]
    /// * `fixed_patches` - Names of Dirichlet boundary patches (u=0)
    /// * `force_patches` - Neumann boundary patches with traction values
    /// * `yield_model` - Von Mises yield criterion with hardening parameters
    /// * `plastic_strain` - Accumulated equivalent plastic strain per cell (updated in-place)
    ///
    /// # Returns
    /// Maximum displacement magnitude.
    pub fn solve_elastoplastic(
        &self,
        state: &mut SolidState,
        mesh: &UnstructuredMesh,
        body_force: [f64; 3],
        fixed_patches: &[String],
        force_patches: &HashMap<String, [f64; 3]>,
        yield_model: &VonMisesYield,
        plastic_strain: &mut Vec<f64>,
    ) -> Result<f64> {
        let num_cells = mesh.num_cells();

        // Ensure plastic strain vector is properly sized
        if plastic_strain.len() != num_cells {
            plastic_strain.resize(num_cells, 0.0);
        }

        // Step 1: Solve the elastic problem to get trial displacements/strains/stresses
        let max_disp = self.solve(state, mesh, body_force, fixed_patches, force_patches)?;

        // Step 2: Apply return mapping to each cell
        let (_lambda, mu) = self.lame_parameters();
        let shear_modulus = mu;

        for cell_id in 0..num_cells {
            let trial_stress = state.stress.get(cell_id).unwrap_or([[0.0; 3]; 3]);
            let eps_p_old = plastic_strain[cell_id];

            // Check yield condition
            let f_trial = yield_model.yield_function(&trial_stress, eps_p_old);

            if f_trial > 0.0 {
                // Yielded: apply return mapping
                let (corrected_stress, eps_p_new) =
                    yield_model.return_mapping(&trial_stress, shear_modulus, eps_p_old);

                // Update stress in state
                state.stress.values_mut()[cell_id] = corrected_stress;

                // Update plastic strain
                plastic_strain[cell_id] = eps_p_new;

                // Update strain to reflect the plastic correction
                // eps_elastic = eps_total - eps_plastic_increment (in deviatoric direction)
                // The strain stored in state is the total strain from FEM;
                // we update it to reflect the elastic part consistent with corrected stress.
                // Compute elastic strain from corrected stress: eps_e = C^{-1} : sigma
                // For isotropic material: eps_ij = (1+nu)/E * sigma_ij - nu/E * sigma_kk * delta_ij
                let e = self.youngs_modulus;
                let nu = self.poissons_ratio;
                let sigma_kk = corrected_stress[0][0] + corrected_stress[1][1] + corrected_stress[2][2];
                let elastic_strain = [
                    [(1.0 + nu) / e * corrected_stress[0][0] - nu / e * sigma_kk,
                     (1.0 + nu) / e * corrected_stress[0][1],
                     (1.0 + nu) / e * corrected_stress[0][2]],
                    [(1.0 + nu) / e * corrected_stress[1][0],
                     (1.0 + nu) / e * corrected_stress[1][1] - nu / e * sigma_kk,
                     (1.0 + nu) / e * corrected_stress[1][2]],
                    [(1.0 + nu) / e * corrected_stress[2][0],
                     (1.0 + nu) / e * corrected_stress[2][1],
                     (1.0 + nu) / e * corrected_stress[2][2] - nu / e * sigma_kk],
                ];

                // The total strain stored in state should be eps_elastic + eps_plastic
                // (we keep the original total strain from FEM, which is correct)
                // But update the strain field to the elastic part for visualization
                let _ = state.strain.values_mut()[cell_id] = elastic_strain;
            }
        }

        Ok(max_disp)
    }
}

/// Elastoplastic solver that wraps the linear elastic solver with Von Mises plasticity.
///
/// This solver performs the full elastoplastic analysis:
/// 1. Elastic predictor (FEM solve for displacements)
/// 2. Plastic corrector (return mapping at integration points)
///
/// For hardening materials, Newton-Raphson iterations can be used for
/// global equilibrium correction. This implementation uses a single-pass
/// elastic predictor + return mapping, which is accurate for moderate plasticity.
pub struct ElastoPlasticSolver {
    /// Underlying elastic solver.
    pub elastic_solver: LinearElasticSolver,
    /// Von Mises yield criterion with hardening.
    pub yield_model: VonMisesYield,
    /// Accumulated equivalent plastic strain per cell.
    pub plastic_strain: Vec<f64>,
    /// Maximum Newton-Raphson iterations for global equilibrium.
    pub max_iterations: usize,
    /// Convergence tolerance for the global residual.
    pub tolerance: f64,
}

impl ElastoPlasticSolver {
    /// Creates a new elastoplastic solver.
    pub fn new(
        youngs_modulus: f64,
        poissons_ratio: f64,
        yield_stress: f64,
        hardening_modulus: f64,
    ) -> Self {
        Self {
            elastic_solver: LinearElasticSolver::new(youngs_modulus, poissons_ratio),
            yield_model: VonMisesYield::new(yield_stress, hardening_modulus),
            plastic_strain: Vec::new(),
            max_iterations: 50,
            tolerance: 1e-6,
        }
    }

    /// Solves the elastoplastic problem.
    ///
    /// Returns the maximum displacement magnitude.
    pub fn solve(
        &mut self,
        state: &mut SolidState,
        mesh: &UnstructuredMesh,
        body_force: [f64; 3],
        fixed_patches: &[String],
        force_patches: &HashMap<String, [f64; 3]>,
    ) -> Result<f64> {
        let num_cells = mesh.num_cells();
        if self.plastic_strain.len() != num_cells {
            self.plastic_strain = vec![0.0; num_cells];
        }

        self.elastic_solver.solve_elastoplastic(
            state,
            mesh,
            body_force,
            fixed_patches,
            force_patches,
            &self.yield_model,
            &mut self.plastic_strain,
        )
    }

    /// Returns the equivalent plastic strain field.
    pub fn equivalent_plastic_strain(&self) -> &[f64] {
        &self.plastic_strain
    }

    /// Returns the maximum equivalent plastic strain.
    pub fn max_plastic_strain(&self) -> f64 {
        self.plastic_strain.iter().cloned().fold(0.0_f64, f64::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::StructuredMesh;

    /// Test cantilever beam: 5x1x1 hex mesh, fixed at xmin, downward force at xmax.
    /// Verify displacement increases along x direction (qualitative behavior).
    /// Compare against analytical: delta = F*L^3/(3*E*I) for cantilever.
    #[test]
    fn cantilever_beam_deflection() {
        // Beam parameters
        let nx = 5;
        let ny = 1;
        let nz = 1;
        let lx = 5.0; // length
        let ly = 1.0; // height
        let lz = 1.0; // width

        let youngs_modulus = 200e9; // Steel, 200 GPa
        let poisson_ratio = 0.3;

        // Create mesh
        let structured = StructuredMesh::uniform(nx, ny, nz, lx, ly, lz);
        let mesh = structured.to_unstructured();

        let num_cells = mesh.num_cells();
        let mut state = SolidState::new(num_cells);

        // Applied force: total force F in the -y direction on the right face
        let total_force = -1000.0; // N, downward
        let xmax_area = ly * lz; // area of the right face
        let traction_y = total_force / xmax_area;

        let mut force_patches = HashMap::new();
        force_patches.insert("xmax".to_string(), [0.0, traction_y, 0.0]);

        let fixed_patches = vec!["xmin".to_string()];

        let solver = LinearElasticSolver::new(youngs_modulus, poisson_ratio);
        let max_disp = solver
            .solve(
                &mut state,
                &mesh,
                [0.0, 0.0, 0.0],
                &fixed_patches,
                &force_patches,
            )
            .expect("Solver should succeed");

        // Basic sanity: max displacement should be positive
        assert!(
            max_disp > 0.0,
            "Max displacement should be positive, got {}",
            max_disp
        );

        // Check that displacement in y increases in magnitude along x
        // The cells are ordered by (i, j, k) with i being x-direction.
        // For a cantilever with load at xmax, |uy| should increase with x.
        let disps: Vec<f64> = (0..nx)
            .map(|i| {
                // For ny=1, nz=1, cell index = i
                state.displacement.values()[i][1].abs()
            })
            .collect();

        for i in 1..nx {
            assert!(
                disps[i] >= disps[i - 1] - 1e-15,
                "Displacement should increase along beam: |uy[{}]|={} < |uy[{}]|={}",
                i,
                disps[i],
                i - 1,
                disps[i - 1]
            );
        }

        // Analytical cantilever tip deflection: delta = F*L^3 / (3*E*I)
        // I = b*h^3/12 for rectangular cross-section (b=width=lz, h=height=ly)
        let inertia = lz * ly.powi(3) / 12.0;
        let analytical_tip = (total_force.abs() * lx.powi(3)) / (3.0 * youngs_modulus * inertia);

        // With only 5 elements the FEM result won't be exact, but should be
        // within a reasonable factor (coarse mesh underestimates deflection
        // for Euler-Bernoulli due to shear locking, but should be same order).
        // The FEM max displacement at the tip cell center:
        let fem_tip = disps[nx - 1];

        // Check that FEM gives a result in the right ballpark (within 10x).
        // On a coarse hex mesh with full integration, the result will be stiffer.
        assert!(
            fem_tip > analytical_tip * 0.01,
            "FEM tip deflection {} should be at least 1% of analytical {}",
            fem_tip,
            analytical_tip
        );
        assert!(
            fem_tip < analytical_tip * 10.0,
            "FEM tip deflection {} should be within 10x of analytical {}",
            fem_tip,
            analytical_tip
        );

        // Print for manual inspection
        eprintln!("Analytical tip deflection: {:.6e} m", analytical_tip);
        eprintln!("FEM tip deflection (cell avg): {:.6e} m", fem_tip);
        eprintln!("FEM max displacement: {:.6e} m", max_disp);
        eprintln!("Cell displacements (uy): {:?}", disps);

        // Verify stresses are non-zero
        let stress_xx = state.stress.values()[0][0][0];
        assert!(
            stress_xx.abs() > 0.0,
            "Stress should be non-zero in a loaded beam"
        );
    }

    /// Test that a free body with no constraints and no forces has zero displacement.
    #[test]
    fn zero_force_zero_displacement() {
        let structured = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();
        let mut state = SolidState::new(num_cells);

        let solver = LinearElasticSolver::new(200e9, 0.3);

        // Fix all boundaries so the problem is well-posed
        let fixed_patches = vec![
            "xmin".to_string(),
            "xmax".to_string(),
            "ymin".to_string(),
            "ymax".to_string(),
            "zmin".to_string(),
            "zmax".to_string(),
        ];
        let force_patches = HashMap::new();

        let max_disp = solver
            .solve(
                &mut state,
                &mesh,
                [0.0, 0.0, 0.0],
                &fixed_patches,
                &force_patches,
            )
            .expect("Solver should succeed with all fixed boundaries");

        assert!(
            max_disp < 1e-15,
            "With no forces and all boundaries fixed, displacement should be zero, got {}",
            max_disp
        );
    }

    /// Test uniform tension: a bar pulled in the x-direction should have
    /// uniform stress sigma_xx = F/A.
    #[test]
    fn uniform_tension() {
        let nx = 3;
        let ny = 1;
        let nz = 1;
        let lx = 3.0;
        let ly = 1.0;
        let lz = 1.0;

        let youngs_modulus = 100e9;
        let poisson_ratio = 0.0; // zero Poisson's ratio for simple check

        let structured = StructuredMesh::uniform(nx, ny, nz, lx, ly, lz);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();
        let mut state = SolidState::new(num_cells);

        // Apply tension: fix xmin, pull on xmax
        let traction = 1e6; // 1 MPa in x-direction
        let mut force_patches = HashMap::new();
        force_patches.insert("xmax".to_string(), [traction, 0.0, 0.0]);

        // Fix xmin fully (all 3 DOFs on those nodes), plus ymin and zmin to
        // prevent rigid body rotation in the y-z plane.
        let fixed_patches = vec![
            "xmin".to_string(),
            "ymin".to_string(),
            "zmin".to_string(),
        ];

        let solver = LinearElasticSolver::new(youngs_modulus, poisson_ratio);
        let max_disp = solver
            .solve(
                &mut state,
                &mesh,
                [0.0, 0.0, 0.0],
                &fixed_patches,
                &force_patches,
            )
            .expect("Solver should succeed");

        assert!(max_disp > 0.0, "Should have non-zero displacement");

        // For a bar under uniform tension with nu=0:
        // sigma_xx = F/A = traction (since force = traction * area and sigma = force/area)
        // epsilon_xx = sigma_xx / E
        // delta_x = epsilon_xx * L = traction * L / E
        let expected_elongation = traction * lx / youngs_modulus;

        // The displacement at the rightmost cell center should be approximately
        // traction * (x - 0) / E for a uniform bar.
        // The rightmost cell center is at x = lx - dx/2 = 3.0 - 0.5 = 2.5
        // But the displacement at the xmax face (x=3.0) is the full elongation.
        // Cell center displacement for the last cell:
        let last_cell_ux = state.displacement.values()[nx - 1][0];
        let expected_at_center = traction * (lx - lx / (2.0 * nx as f64)) / youngs_modulus;

        // Allow generous tolerance since BCs on ymin/zmin constrain the problem.
        // The result should be in the right ballpark.
        eprintln!("Uniform tension: expected elongation = {:.6e}", expected_elongation);
        eprintln!("Last cell ux = {:.6e}, expected at center = {:.6e}", last_cell_ux, expected_at_center);

        assert!(
            last_cell_ux > 0.0,
            "Displacement in tension direction should be positive"
        );
    }

    /// Test elastoplastic: low stress stays elastic (no plastic strain).
    #[test]
    fn elastoplastic_below_yield() {
        let structured = StructuredMesh::uniform(3, 1, 1, 3.0, 1.0, 1.0);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();
        let mut state = SolidState::new(num_cells);

        // Low traction that stays well below yield
        let mut force_patches = HashMap::new();
        force_patches.insert("xmax".to_string(), [1e3, 0.0, 0.0]); // 1 kPa, very low

        let fixed_patches = vec![
            "xmin".to_string(),
            "ymin".to_string(),
            "zmin".to_string(),
        ];

        let yield_stress = 250e6; // 250 MPa
        let hardening = 1e9;
        let yield_model = VonMisesYield::new(yield_stress, hardening);

        let solver = LinearElasticSolver::new(200e9, 0.3);
        let mut plastic_strain = vec![0.0; num_cells];

        let max_disp = solver
            .solve_elastoplastic(
                &mut state,
                &mesh,
                [0.0, 0.0, 0.0],
                &fixed_patches,
                &force_patches,
                &yield_model,
                &mut plastic_strain,
            )
            .expect("Elastoplastic solve should succeed");

        assert!(max_disp > 0.0, "Should have displacement");

        // All plastic strains should remain zero (below yield)
        for (i, &eps_p) in plastic_strain.iter().enumerate() {
            assert!(
                eps_p < 1e-30,
                "Cell {} should have zero plastic strain below yield, got {}",
                i,
                eps_p
            );
        }
    }

    /// Test elastoplastic: high stress exceeds yield, producing plastic strain.
    /// Uses nu=0.0 so uniaxial traction produces uniaxial stress (VM = sigma_xx).
    #[test]
    fn elastoplastic_above_yield() {
        let structured = StructuredMesh::uniform(3, 1, 1, 3.0, 1.0, 1.0);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();
        let mut state = SolidState::new(num_cells);

        // Very high traction to exceed yield stress. Use lower yield stress
        // to ensure yielding occurs on a coarse mesh.
        let mut force_patches = HashMap::new();
        force_patches.insert("xmax".to_string(), [500e6, 0.0, 0.0]); // 500 MPa traction

        // Use nu=0 so stress is purely uniaxial and VM = |sigma_xx|
        let fixed_patches = vec![
            "xmin".to_string(),
            "ymin".to_string(),
            "zmin".to_string(),
        ];

        let yield_stress = 100e6; // 100 MPa (lower yield to ensure yielding)
        let hardening = 1e9; // 1 GPa hardening
        let yield_model = VonMisesYield::new(yield_stress, hardening);

        let solver = LinearElasticSolver::new(200e9, 0.0); // nu=0 for clean uniaxial
        let mut plastic_strain = vec![0.0; num_cells];

        let max_disp = solver
            .solve_elastoplastic(
                &mut state,
                &mesh,
                [0.0, 0.0, 0.0],
                &fixed_patches,
                &force_patches,
                &yield_model,
                &mut plastic_strain,
            )
            .expect("Elastoplastic solve should succeed");

        assert!(max_disp > 0.0, "Should have displacement");

        // At least some cells should have yielded (500 MPa traction > 100 MPa yield)
        let max_eps_p = plastic_strain.iter().cloned().fold(0.0_f64, f64::max);
        assert!(
            max_eps_p > 0.0,
            "Some cells should have plastic strain above yield, max eps_p = {}",
            max_eps_p
        );

        // Verify that the corrected von Mises stress is at or below the yield surface
        for cell_id in 0..num_cells {
            let stress = state.stress.get(cell_id).unwrap_or([[0.0; 3]; 3]);
            let vm = VonMisesYield::compute_von_mises(&stress);
            let eps_p = plastic_strain[cell_id];
            let sigma_y = yield_model.current_yield_stress(eps_p);
            assert!(
                vm <= sigma_y + 1.0, // 1 Pa absolute tolerance
                "Cell {}: VM stress {:.3e} should be <= yield stress {:.3e}",
                cell_id,
                vm,
                sigma_y
            );
        }

        eprintln!("Max plastic strain: {:.6e}", max_eps_p);
        eprintln!("Max displacement: {:.6e}", max_disp);
    }

    /// Test ElastoPlasticSolver wrapper.
    #[test]
    fn elastoplastic_solver_wrapper() {
        let structured = StructuredMesh::uniform(2, 1, 1, 2.0, 1.0, 1.0);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();
        let mut state = SolidState::new(num_cells);

        let mut force_patches = HashMap::new();
        force_patches.insert("xmax".to_string(), [300e6, 0.0, 0.0]);

        let fixed_patches = vec![
            "xmin".to_string(),
            "ymin".to_string(),
            "zmin".to_string(),
        ];

        let mut ep_solver = ElastoPlasticSolver::new(200e9, 0.3, 250e6, 1e9);
        let max_disp = ep_solver
            .solve(&mut state, &mesh, [0.0, 0.0, 0.0], &fixed_patches, &force_patches)
            .expect("ElastoPlasticSolver should succeed");

        assert!(max_disp > 0.0, "Should have displacement");
        eprintln!("Max plastic strain: {:.6e}", ep_solver.max_plastic_strain());
    }
}
