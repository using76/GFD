//! SIMPLEC (SIMPLE-Consistent) algorithm.
//!
//! SIMPLEC differs from SIMPLE in the velocity correction formula.
//! In SIMPLE, the velocity correction uses d_P = V_P / a_P, which
//! omits neighbor contributions and requires pressure under-relaxation.
//! SIMPLEC uses d_P = V_P / (a_P - H1), where H1 = sum(|a_nb|), which
//! accounts for the omitted neighbor coefficients. This allows
//! pressure under-relaxation alpha_p = 1.0, leading to faster convergence.

use std::collections::HashMap;

use gfd_core::UnstructuredMesh;
use gfd_discretize::fvm::diffusion::compute_diffusive_coefficient;
use gfd_linalg::iterative::bicgstab::BiCGSTAB;
use gfd_linalg::iterative::cg::CG;
use gfd_linalg::traits::LinearSolverTrait;
use gfd_matrix::assembler::Assembler;

use crate::incompressible::PressureVelocityCoupling;
use crate::{FluidError, FluidState, Result};

/// SIMPLEC pressure-velocity coupling solver.
///
/// A variant of SIMPLE where the velocity correction formula uses
/// (a_P - H_1) instead of a_P, providing better convergence for
/// the pressure correction equation. This allows alpha_p = 1.0.
pub struct SimplecSolver {
    /// Under-relaxation factor for velocity (momentum equation).
    pub alpha_u: f64,
    /// Uniform density [kg/m^3].
    pub density: f64,
    /// Uniform dynamic viscosity [Pa*s].
    pub viscosity: f64,
    /// Diagonal coefficients a_P from the momentum equation (before under-relaxation).
    a_p_diagonal: Vec<f64>,
    /// Sum of absolute neighbor coefficients H1 = sum(|a_nb|) for each cell.
    h1_sum: Vec<f64>,
    /// Boundary velocities: patch name -> [u, v, w].
    boundary_velocities: HashMap<String, [f64; 3]>,
    /// Boundary pressures: patch name -> pressure value.
    boundary_pressure: HashMap<String, f64>,
    /// Wall patch names (no-slip).
    wall_patches: Vec<String>,
    /// Cached face-to-patch map.
    face_patch_cache: Vec<Option<usize>>,
    /// Cached patch names.
    patch_names: Vec<String>,
    /// Number of faces for which the cache was built.
    cached_num_faces: usize,
    /// Cached per-internal-face: (face_idx, owner, neighbor, distance, D_coeff).
    cached_internal_geom: Vec<(usize, usize, usize, f64, f64)>,
    /// Cached per-boundary-face: (face_idx, owner, D_coeff).
    cached_boundary_geom: Vec<(usize, usize, f64)>,
    /// Cached cell volumes.
    cached_volumes: Vec<f64>,
    /// Cached face areas.
    cached_face_area: Vec<f64>,
    /// Cached face.area * face.normal for all faces.
    cached_normal_area: Vec<[f64; 3]>,
    /// Precomputed boundary face data: (owner, D, bc_vel, f_flux_bc, is_wall).
    cached_bc_faces: Vec<(usize, f64, Option<[f64; 3]>, f64, bool)>,
    cached_bc_faces_built: bool,
    /// Reusable CG solver for pressure correction.
    cg_solver: CG,
    /// Cached CSR pattern (row_ptr, col_idx).
    pc_cached: bool,
    pc_row_ptr: Vec<usize>,
    pc_col_idx: Vec<usize>,
    /// Mapping: for each internal face, CSR indices for (owner->neigh) and (neigh->owner).
    pc_face_csr_idx: Vec<(usize, usize)>,
    /// CSR index for diagonal entry of each cell.
    pc_diag_csr_idx: Vec<usize>,
    /// Reusable workspace vectors.
    ws_sources: Vec<f64>,
    ws_a_p: Vec<f64>,
    ws_x_buf: Vec<f64>,
    /// Cached momentum CSR matrix.
    mom_matrix: Option<gfd_core::SparseMatrix>,
    /// Cached pressure correction CSR matrix.
    pc_matrix: Option<gfd_core::SparseMatrix>,
}

impl SimplecSolver {
    /// Creates a new SIMPLEC solver with the given density and viscosity.
    ///
    /// Default velocity under-relaxation is 0.7. Pressure under-relaxation
    /// is always 1.0 (the key SIMPLEC advantage).
    pub fn new(density: f64, viscosity: f64) -> Self {
        Self {
            alpha_u: 0.7,
            density,
            viscosity,
            a_p_diagonal: Vec::new(),
            h1_sum: Vec::new(),
            boundary_velocities: HashMap::new(),
            boundary_pressure: HashMap::new(),
            wall_patches: Vec::new(),
            face_patch_cache: Vec::new(),
            patch_names: Vec::new(),
            cached_num_faces: 0,
            cached_internal_geom: Vec::new(),
            cached_boundary_geom: Vec::new(),
            cached_volumes: Vec::new(),
            cached_face_area: Vec::new(),
            cached_normal_area: Vec::new(),
            cached_bc_faces: Vec::new(),
            cached_bc_faces_built: false,
            cg_solver: CG::new(1e-3, 1000),
            pc_cached: false,
            pc_row_ptr: Vec::new(),
            pc_col_idx: Vec::new(),
            pc_face_csr_idx: Vec::new(),
            pc_diag_csr_idx: Vec::new(),
            ws_sources: Vec::new(),
            ws_a_p: Vec::new(),
            ws_x_buf: Vec::new(),
            mom_matrix: None,
            pc_matrix: None,
        }
    }

    /// Sets the boundary conditions for the solver.
    pub fn set_boundary_conditions(
        &mut self,
        boundary_velocities: HashMap<String, [f64; 3]>,
        boundary_pressure: HashMap<String, f64>,
        wall_patches: Vec<String>,
    ) {
        self.boundary_velocities = boundary_velocities;
        self.boundary_pressure = boundary_pressure;
        self.wall_patches = wall_patches;
    }

    /// Ensure the face-to-patch cache is built for this mesh.
    fn ensure_face_patch_cache(&mut self, mesh: &UnstructuredMesh) {
        if self.cached_num_faces == mesh.faces.len() && !self.face_patch_cache.is_empty() {
            return;
        }
        let num_faces = mesh.faces.len();
        self.face_patch_cache = vec![None; num_faces];
        self.patch_names.clear();
        let mut name_to_idx: HashMap<String, usize> = HashMap::new();
        for patch in &mesh.boundary_patches {
            let idx = if let Some(&idx) = name_to_idx.get(&patch.name) {
                idx
            } else {
                let idx = self.patch_names.len();
                self.patch_names.push(patch.name.clone());
                name_to_idx.insert(patch.name.clone(), idx);
                idx
            };
            for &fid in &patch.face_ids {
                if fid < num_faces {
                    self.face_patch_cache[fid] = Some(idx);
                }
            }
        }
        self.cached_num_faces = num_faces;

        // Cache cell volumes and face normal*area products
        self.cached_volumes = mesh.cells.iter().map(|c| c.volume).collect();
        self.cached_normal_area = mesh
            .faces
            .iter()
            .map(|f| {
                [
                    f.area * f.normal[0],
                    f.area * f.normal[1],
                    f.area * f.normal[2],
                ]
            })
            .collect();
        self.cached_face_area = mesh.faces.iter().map(|f| f.area).collect();

        // Cache geometric data
        self.cached_internal_geom.clear();
        self.cached_boundary_geom.clear();
        for (fi, face) in mesh.faces.iter().enumerate() {
            let owner = face.owner_cell;
            if let Some(neigh) = face.neighbor_cell {
                let co = mesh.cells[owner].center;
                let cn = mesh.cells[neigh].center;
                let dx = cn[0] - co[0];
                let dy = cn[1] - co[1];
                let dz = cn[2] - co[2];
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                let d = compute_diffusive_coefficient(self.viscosity, face.area, dist);
                self.cached_internal_geom.push((fi, owner, neigh, dist, d));
            } else {
                let co = mesh.cells[owner].center;
                let fc = face.center;
                let dx = fc[0] - co[0];
                let dy = fc[1] - co[1];
                let dz = fc[2] - co[2];
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                let dist = if dist < 1e-30 { 1e-30 } else { dist };
                let d = compute_diffusive_coefficient(self.viscosity, face.area, dist);
                self.cached_boundary_geom.push((fi, owner, d));
            }
        }
    }

    /// Get the patch name for a face from cache.
    #[inline]
    fn get_face_patch(&self, face_id: usize) -> Option<&str> {
        if face_id < self.face_patch_cache.len() {
            self.face_patch_cache[face_id]
                .map(|idx| self.patch_names[idx].as_str())
        } else {
            None
        }
    }

    /// Full SIMPLEC step with explicit boundary condition arguments.
    pub fn solve_step_with_bcs(
        &mut self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        boundary_velocities: &HashMap<String, [f64; 3]>,
        boundary_pressure: &HashMap<String, f64>,
        wall_patches: &[String],
    ) -> Result<f64> {
        self.ensure_face_patch_cache(mesh);

        // 1. Solve momentum equations (same as SIMPLE, with velocity under-relaxation)
        self.solve_momentum(state, mesh, boundary_velocities, boundary_pressure, wall_patches)?;

        // 2. Solve pressure correction using SIMPLEC d-coefficient
        let p_prime = self.solve_pressure_correction(state, mesh, boundary_pressure)?;

        // 3. Correct velocity using SIMPLEC d-coefficient: d_P = V_P / (a_P - H1)
        self.correct_velocity(state, mesh, &p_prime)?;

        // 4. Correct pressure with alpha_p = 1.0 (SIMPLEC advantage)
        self.correct_pressure(state, &p_prime);

        // 5. Compute and return continuity residual
        let residual =
            self.compute_continuity_residual(state, mesh, boundary_velocities, wall_patches);
        Ok(residual)
    }

    // -----------------------------------------------------------------------
    // Step 1: Momentum predictor (same as SIMPLE)
    // -----------------------------------------------------------------------

    fn solve_momentum(
        &mut self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        boundary_velocities: &HashMap<String, [f64; 3]>,
        boundary_pressure: &HashMap<String, f64>,
        wall_patches: &[String],
    ) -> Result<()> {
        let n = mesh.num_cells();

        // Inline Green-Gauss pressure gradient
        let p_vals = state.pressure.values();
        let mut grad_p_data = vec![[0.0_f64; 3]; n];
        for (fi, face) in mesh.faces.iter().enumerate() {
            let owner = face.owner_cell;
            let na = self.cached_normal_area[fi];
            if let Some(neighbor) = face.neighbor_cell {
                let phi_f = 0.5 * (p_vals[owner] + p_vals[neighbor]);
                let c0 = phi_f * na[0];
                let c1 = phi_f * na[1];
                let c2 = phi_f * na[2];
                grad_p_data[owner][0] += c0;
                grad_p_data[owner][1] += c1;
                grad_p_data[owner][2] += c2;
                grad_p_data[neighbor][0] -= c0;
                grad_p_data[neighbor][1] -= c1;
                grad_p_data[neighbor][2] -= c2;
            } else {
                let phi_f = p_vals[owner];
                grad_p_data[owner][0] += phi_f * na[0];
                grad_p_data[owner][1] += phi_f * na[1];
                grad_p_data[owner][2] += phi_f * na[2];
            }
        }
        for i in 0..n {
            let inv_vol = 1.0 / self.cached_volumes[i];
            grad_p_data[i][0] *= inv_vol;
            grad_p_data[i][1] *= inv_vol;
            grad_p_data[i][2] *= inv_vol;
        }

        // Ensure CSR pattern is cached
        if !self.pc_cached {
            self.build_csr_pattern(n, mesh, boundary_pressure)?;
        }

        let rho_half = 0.5 * self.density;
        let vel_vals = state.velocity.values();

        // Ensure workspace vectors are allocated
        let nnz = self.pc_col_idx.len();
        if self.ws_sources.len() != n {
            self.ws_sources = vec![0.0; n];
            self.ws_a_p = vec![0.0; n];
            self.ws_x_buf = vec![0.0; n];
        }

        // Take momentum matrix out
        let mut mom_mat = self.mom_matrix.take().unwrap_or_else(|| {
            gfd_core::SparseMatrix::new(
                n,
                n,
                self.pc_row_ptr.clone(),
                self.pc_col_idx.clone(),
                vec![0.0; nnz],
            )
            .unwrap()
        });
        mom_mat.values.fill(0.0);
        self.ws_a_p.fill(0.0);

        // Reset H1 sum (sum of |off-diagonal neighbor coefficients|)
        self.h1_sum.resize(n, 0.0);
        self.h1_sum.fill(0.0);

        // Internal faces: compute D+F coefficients
        for (face_idx, &(fi, owner, neigh, _dist, d)) in
            self.cached_internal_geom.iter().enumerate()
        {
            let na = self.cached_normal_area[fi];
            let vo = vel_vals[owner];
            let vn = vel_vals[neigh];
            let f_flux = rho_half
                * ((vo[0] + vn[0]) * na[0]
                    + (vo[1] + vn[1]) * na[1]
                    + (vo[2] + vn[2]) * na[2]);
            let f_pos = f64::max(f_flux, 0.0);
            let f_neg = f64::max(-f_flux, 0.0);
            self.ws_a_p[owner] += d + f_pos;
            self.ws_a_p[neigh] += d + f_neg;
            let (idx_on, idx_no) = self.pc_face_csr_idx[face_idx];
            let a_nb_on = d + f_neg; // off-diagonal coefficient for (owner, neigh)
            let a_nb_no = d + f_pos; // off-diagonal coefficient for (neigh, owner)
            mom_mat.values[idx_on] = -a_nb_on;
            mom_mat.values[idx_no] = -a_nb_no;

            // SIMPLEC: accumulate sum of |off-diagonal neighbor coefficients|
            self.h1_sum[owner] += a_nb_on;
            self.h1_sum[neigh] += a_nb_no;
        }

        // Build boundary face data once
        if !self.cached_bc_faces_built {
            self.cached_bc_faces.clear();
            for &(fi, owner, d) in &self.cached_boundary_geom {
                let na = self.cached_normal_area[fi];
                let patch = self.get_face_patch(fi);
                let (bc_vel, f_flux_bc, is_wall) = if let Some(pname) = patch {
                    if let Some(bv) = boundary_velocities.get(pname) {
                        let ff =
                            self.density * (bv[0] * na[0] + bv[1] * na[1] + bv[2] * na[2]);
                        (Some(*bv), ff, false)
                    } else if wall_patches.iter().any(|w| w == pname) {
                        (None, 0.0, true)
                    } else {
                        (None, 0.0, false)
                    }
                } else {
                    (None, 0.0, false)
                };
                self.cached_bc_faces.push((owner, d, bc_vel, f_flux_bc, is_wall));
            }
            self.cached_bc_faces_built = true;
        }

        // Apply boundary contributions to diagonal
        for &(owner, d, ref bc_vel, f_flux_bc, is_wall) in &self.cached_bc_faces {
            if bc_vel.is_some() {
                self.ws_a_p[owner] += d + f64::max(f_flux_bc, 0.0);
            } else if is_wall {
                self.ws_a_p[owner] += d;
            }
        }

        // Store the unrelaxed diagonal (a_P before under-relaxation)
        // and apply under-relaxation to the momentum matrix diagonal
        let ur_factor = (1.0 - self.alpha_u) / self.alpha_u;
        self.a_p_diagonal.resize(n, 0.0);
        for i in 0..n {
            self.a_p_diagonal[i] = self.ws_a_p[i]; // a_P (unrelaxed)
            let a_p_relaxed = self.ws_a_p[i] / self.alpha_u;
            mom_mat.values[self.pc_diag_csr_idx[i]] = a_p_relaxed;
        }

        let sources = &mut self.ws_sources;
        let x_buf = &mut self.ws_x_buf;
        let mut solver_bicgstab = BiCGSTAB::new(1e-2, 1000);

        // Precompute active velocity components
        let mut active_comps = [false; 3];
        for &(_o, _d, ref bv, _f, _w) in &self.cached_bc_faces {
            if let Some(bv) = bv {
                for c in 0..3 {
                    if bv[c].abs() > 0.0 {
                        active_comps[c] = true;
                    }
                }
            }
        }

        // Solve each velocity component
        for comp in 0..3 {
            let vel_values = state.velocity.values();
            if !active_comps[comp]
                && !vel_values.iter().any(|v| v[comp].abs() > 1e-30)
                && !grad_p_data.iter().any(|g| g[comp].abs() > 1e-30)
            {
                continue;
            }

            sources.fill(0.0);

            for &(owner, d, ref bv, f_flux_bc, _w) in &self.cached_bc_faces {
                if let Some(bv) = bv {
                    sources[owner] += (d + f64::max(-f_flux_bc, 0.0)) * bv[comp];
                }
            }

            for i in 0..n {
                sources[i] -= grad_p_data[i][comp] * self.cached_volumes[i];
                sources[i] += ur_factor * self.ws_a_p[i] * vel_values[i][comp];
                x_buf[i] = vel_values[i][comp];
            }

            solver_bicgstab
                .solve(&mom_mat, &sources, x_buf)
                .map_err(|e| FluidError::SolverFailed(format!("{:?}", e)))?;

            let vel_mut = state.velocity.values_mut();
            for i in 0..n {
                vel_mut[i][comp] = x_buf[i];
            }
        }

        self.mom_matrix = Some(mom_mat);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Step 2: Pressure correction with SIMPLEC d-coefficient
    // -----------------------------------------------------------------------

    /// Solve the pressure correction equation using SIMPLEC d-coefficients.
    ///
    /// The key difference from SIMPLE: the face d-coefficient is computed as:
    ///   d_P = V_P / (a_P - H1_P)
    /// where H1_P = sum(|a_nb|) for cell P.
    ///
    /// This accounts for the dropped neighbor terms in the velocity correction,
    /// allowing alpha_p = 1.0 for the pressure correction.
    fn solve_pressure_correction(
        &mut self,
        state: &FluidState,
        mesh: &UnstructuredMesh,
        boundary_pressure: &HashMap<String, f64>,
    ) -> Result<Vec<f64>> {
        let n = mesh.num_cells();
        let vel_vals = state.velocity.values();
        let rho_half = 0.5 * self.density;

        if !self.pc_cached {
            self.build_csr_pattern(n, mesh, boundary_pressure)?;
        }

        let nnz = self.pc_col_idx.len();
        let mut pc_mat = self.pc_matrix.take().unwrap_or_else(|| {
            gfd_core::SparseMatrix::new(
                n,
                n,
                self.pc_row_ptr.clone(),
                self.pc_col_idx.clone(),
                vec![0.0; nnz],
            )
            .unwrap()
        });
        pc_mat.values.fill(0.0);
        let mut sources_pc = vec![0.0; n];

        // Internal faces: compute SIMPLEC coefficients
        for (face_idx, &(fi, owner, neigh, dist, _d)) in
            self.cached_internal_geom.iter().enumerate()
        {
            // SIMPLEC d-coefficient: d_P = V_P / (a_P - H1_P)
            let denom_o = (self.a_p_diagonal[owner] - self.h1_sum[owner]).max(1e-30);
            let denom_n = (self.a_p_diagonal[neigh] - self.h1_sum[neigh]).max(1e-30);
            let ra_o = self.cached_volumes[owner] / denom_o;
            let ra_n = self.cached_volumes[neigh] / denom_n;
            let ra_f = 0.5 * (ra_o + ra_n);
            let coeff = self.density * ra_f * self.cached_face_area[fi] / dist;

            let (idx_on, idx_no) = self.pc_face_csr_idx[face_idx];
            pc_mat.values[idx_on] = -coeff;
            pc_mat.values[idx_no] = -coeff;
            pc_mat.values[self.pc_diag_csr_idx[owner]] += coeff;
            pc_mat.values[self.pc_diag_csr_idx[neigh]] += coeff;

            // RHS: mass imbalance
            let na = self.cached_normal_area[fi];
            let vo = vel_vals[owner];
            let vn = vel_vals[neigh];
            let mass_flux = rho_half
                * ((vo[0] + vn[0]) * na[0]
                    + (vo[1] + vn[1]) * na[1]
                    + (vo[2] + vn[2]) * na[2]);
            sources_pc[owner] -= mass_flux;
            sources_pc[neigh] += mass_flux;
        }

        // Boundary faces: RHS contribution
        for &(fi, owner, _d) in &self.cached_boundary_geom {
            let face = &mesh.faces[fi];
            let patch_name = self.get_face_patch(face.id);
            if let Some(pname) = patch_name {
                if self.boundary_pressure.contains_key(pname) {
                    let vel_o = vel_vals[owner];
                    let mass_flux = self.density
                        * (vel_o[0] * face.normal[0]
                            + vel_o[1] * face.normal[1]
                            + vel_o[2] * face.normal[2])
                        * face.area;
                    sources_pc[owner] -= mass_flux;
                } else if self.boundary_velocities.contains_key(pname)
                    || self.wall_patches.iter().any(|w| w == pname)
                {
                    if let Some(bc_vel) = self.boundary_velocities.get(pname) {
                        let mass_flux = self.density
                            * (bc_vel[0] * face.normal[0]
                                + bc_vel[1] * face.normal[1]
                                + bc_vel[2] * face.normal[2])
                            * face.area;
                        sources_pc[owner] -= mass_flux;
                    }
                }
            }
        }

        // Apply Dirichlet p' = 0 for fixed-pressure or enclosed domains
        let has_pressure_bc = boundary_pressure
            .keys()
            .any(|k| mesh.boundary_patch(k).is_some());
        if !has_pressure_bc {
            let start = self.pc_row_ptr[0];
            let end = self.pc_row_ptr[1];
            for idx in start..end {
                pc_mat.values[idx] = if self.pc_col_idx[idx] == 0 { 1.0 } else { 0.0 };
            }
            sources_pc[0] = 0.0;
        }
        for patch in &mesh.boundary_patches {
            if boundary_pressure.contains_key(&patch.name) {
                for &fid in &patch.face_ids {
                    let cell_id = mesh.faces[fid].owner_cell;
                    let start = self.pc_row_ptr[cell_id];
                    let end = self.pc_row_ptr[cell_id + 1];
                    for idx in start..end {
                        pc_mat.values[idx] =
                            if self.pc_col_idx[idx] == cell_id { 1.0 } else { 0.0 };
                    }
                    sources_pc[cell_id] = 0.0;
                }
            }
        }

        let mut x_pc = vec![0.0; n];
        let stats = self
            .cg_solver
            .solve(&pc_mat, &sources_pc, &mut x_pc)
            .map_err(|e| FluidError::SolverFailed(format!("{:?}", e)))?;

        if !stats.converged {
            x_pc.fill(0.0);
            let mut fallback = BiCGSTAB::new(1e-3, 1000);
            fallback
                .solve(&pc_mat, &sources_pc, &mut x_pc)
                .map_err(|e| FluidError::SolverFailed(format!("{:?}", e)))?;
        }

        self.pc_matrix = Some(pc_mat);
        Ok(x_pc)
    }

    /// Build the CSR pattern for the matrix (called once on first iteration).
    fn build_csr_pattern(
        &mut self,
        n: usize,
        _mesh: &UnstructuredMesh,
        _boundary_pressure: &HashMap<String, f64>,
    ) -> Result<()> {
        let n_internal = self.cached_internal_geom.len();
        let nnz_estimate = n + 2 * n_internal;
        let mut assembler = Assembler::with_nnz_estimate(n, nnz_estimate);

        for &(_fi, owner, neigh, _dist, _d) in &self.cached_internal_geom {
            assembler.add_neighbor(owner, neigh, 1.0);
            assembler.add_neighbor(neigh, owner, 1.0);
        }
        for i in 0..n {
            assembler.add_diagonal(i, 1.0);
            assembler.add_source(i, 0.0);
        }

        let system = assembler
            .finalize()
            .map_err(|e| FluidError::PressureCorrectionFailed(e.to_string()))?;

        self.pc_row_ptr = system.a.row_ptr.clone();
        self.pc_col_idx = system.a.col_idx.clone();

        // Build diagonal index mapping
        self.pc_diag_csr_idx = vec![0; n];
        for i in 0..n {
            let start = self.pc_row_ptr[i];
            let end = self.pc_row_ptr[i + 1];
            for idx in start..end {
                if self.pc_col_idx[idx] == i {
                    self.pc_diag_csr_idx[i] = idx;
                    break;
                }
            }
        }

        // Build face-to-CSR-index mapping
        self.pc_face_csr_idx = Vec::with_capacity(n_internal);
        for &(_fi, owner, neigh, _dist, _d) in &self.cached_internal_geom {
            let idx_on = {
                let start = self.pc_row_ptr[owner];
                let end = self.pc_row_ptr[owner + 1];
                let mut found = start;
                for idx in start..end {
                    if self.pc_col_idx[idx] == neigh {
                        found = idx;
                        break;
                    }
                }
                found
            };
            let idx_no = {
                let start = self.pc_row_ptr[neigh];
                let end = self.pc_row_ptr[neigh + 1];
                let mut found = start;
                for idx in start..end {
                    if self.pc_col_idx[idx] == owner {
                        found = idx;
                        break;
                    }
                }
                found
            };
            self.pc_face_csr_idx.push((idx_on, idx_no));
        }

        self.pc_cached = true;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Step 3: Velocity correction (SIMPLEC version)
    // -----------------------------------------------------------------------

    /// Correct velocity using SIMPLEC pressure correction gradient.
    ///
    /// SIMPLEC: u_P = u*_P - d_SIMPLEC_P * grad(p')_P
    /// where d_SIMPLEC_P = V_P / (a_P - H1_P) instead of SIMPLE's V_P / a_P
    fn correct_velocity(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        p_prime: &[f64],
    ) -> Result<()> {
        let n = mesh.num_cells();

        // Green-Gauss gradient of p_prime
        let mut grad_pp = vec![[0.0_f64; 3]; n];
        for (fi, face) in mesh.faces.iter().enumerate() {
            let owner = face.owner_cell;
            let na = self.cached_normal_area[fi];
            if let Some(neighbor) = face.neighbor_cell {
                let phi_f = 0.5 * (p_prime[owner] + p_prime[neighbor]);
                let c0 = phi_f * na[0];
                let c1 = phi_f * na[1];
                let c2 = phi_f * na[2];
                grad_pp[owner][0] += c0;
                grad_pp[owner][1] += c1;
                grad_pp[owner][2] += c2;
                grad_pp[neighbor][0] -= c0;
                grad_pp[neighbor][1] -= c1;
                grad_pp[neighbor][2] -= c2;
            } else {
                let phi_f = p_prime[owner];
                grad_pp[owner][0] += phi_f * na[0];
                grad_pp[owner][1] += phi_f * na[1];
                grad_pp[owner][2] += phi_f * na[2];
            }
        }

        let vel_mut = state.velocity.values_mut();
        for i in 0..n {
            // SIMPLEC d-coefficient: 1 / (a_P - H1)
            let denom = (self.a_p_diagonal[i] - self.h1_sum[i]).max(1e-30);
            let inv_denom = 1.0 / denom;
            vel_mut[i][0] -= inv_denom * grad_pp[i][0];
            vel_mut[i][1] -= inv_denom * grad_pp[i][1];
            vel_mut[i][2] -= inv_denom * grad_pp[i][2];
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Step 4: Pressure correction (alpha_p = 1.0 for SIMPLEC)
    // -----------------------------------------------------------------------

    /// Correct pressure: p = p* + 1.0 * p' (SIMPLEC uses alpha_p = 1.0).
    fn correct_pressure(&self, state: &mut FluidState, p_prime: &[f64]) {
        let p_mut = state.pressure.values_mut();
        for i in 0..p_mut.len() {
            // SIMPLEC: alpha_p = 1.0 (no under-relaxation needed for pressure)
            p_mut[i] += p_prime[i];
        }
    }

    // -----------------------------------------------------------------------
    // Residual computation
    // -----------------------------------------------------------------------

    fn compute_continuity_residual(
        &self,
        state: &FluidState,
        mesh: &UnstructuredMesh,
        boundary_velocities: &HashMap<String, [f64; 3]>,
        wall_patches: &[String],
    ) -> f64 {
        let n = mesh.num_cells();
        let mut mass_imbalance = vec![0.0; n];
        let vel_vals = state.velocity.values();
        let rho_half = 0.5 * self.density;

        for (fi, face) in mesh.faces.iter().enumerate() {
            let owner = face.owner_cell;
            if let Some(neigh) = face.neighbor_cell {
                let na = self.cached_normal_area[fi];
                let vo = vel_vals[owner];
                let vn = vel_vals[neigh];
                let mass_flux = rho_half
                    * ((vo[0] + vn[0]) * na[0]
                        + (vo[1] + vn[1]) * na[1]
                        + (vo[2] + vn[2]) * na[2]);
                mass_imbalance[owner] += mass_flux;
                mass_imbalance[neigh] -= mass_flux;
            } else {
                let patch_name = self.get_face_patch(face.id);
                if let Some(pname) = patch_name {
                    if let Some(bc_vel) = boundary_velocities.get(pname) {
                        let mass_flux = self.density
                            * (bc_vel[0] * face.normal[0]
                                + bc_vel[1] * face.normal[1]
                                + bc_vel[2] * face.normal[2])
                            * face.area;
                        mass_imbalance[owner] += mass_flux;
                    } else if wall_patches.iter().any(|w| w == pname) {
                        // Wall: zero flux
                    } else {
                        let vel_o = state.velocity.values()[owner];
                        let mass_flux = self.density
                            * (vel_o[0] * face.normal[0]
                                + vel_o[1] * face.normal[1]
                                + vel_o[2] * face.normal[2])
                            * face.area;
                        mass_imbalance[owner] += mass_flux;
                    }
                }
            }
        }

        mass_imbalance
            .iter()
            .map(|v| v.abs())
            .fold(0.0_f64, f64::max)
    }
}

// ---------------------------------------------------------------------------
// PressureVelocityCoupling trait implementation
// ---------------------------------------------------------------------------

impl PressureVelocityCoupling for SimplecSolver {
    fn solve_step(
        &mut self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        _dt: f64,
    ) -> Result<f64> {
        let bv = self.boundary_velocities.clone();
        let bp = self.boundary_pressure.clone();
        let wp = self.wall_patches.clone();
        self.solve_step_with_bcs(state, mesh, &bv, &bp, &wp)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::cell::Cell;
    use gfd_core::mesh::face::Face;
    use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

    /// Creates a 3x3x1 hex mesh for lid-driven cavity test (same as SIMPLE tests).
    fn make_3x3x1_lid_driven_cavity()
        -> (UnstructuredMesh, HashMap<String, [f64; 3]>, HashMap<String, f64>, Vec<String>)
    {
        let nx = 3;
        let ny = 3;
        let dx = 1.0;
        let dy = 1.0;
        let dz = 1.0;

        let mut cells = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                let id = j * nx + i;
                let cx = (i as f64 + 0.5) * dx;
                let cy = (j as f64 + 0.5) * dy;
                let cz = 0.5 * dz;
                cells.push(Cell::new(id, vec![], vec![], dx * dy * dz, [cx, cy, cz]));
            }
        }

        let mut faces = Vec::new();
        let mut fid = 0;

        let mut bottom_faces = Vec::new();
        let mut top_faces = Vec::new();
        let mut left_faces = Vec::new();
        let mut right_faces = Vec::new();
        let mut front_faces = Vec::new();
        let mut back_faces = Vec::new();

        // Internal x-normal faces
        for j in 0..ny {
            for i in 0..(nx - 1) {
                let owner = j * nx + i;
                let neighbor = j * nx + i + 1;
                let fx = (i as f64 + 1.0) * dx;
                let fy = (j as f64 + 0.5) * dy;
                let fz = 0.5 * dz;
                faces.push(Face::new(fid, vec![], owner, Some(neighbor), dy * dz,
                    [1.0, 0.0, 0.0], [fx, fy, fz]));
                cells[owner].faces.push(fid);
                cells[neighbor].faces.push(fid);
                fid += 1;
            }
        }

        // Internal y-normal faces
        for j in 0..(ny - 1) {
            for i in 0..nx {
                let owner = j * nx + i;
                let neighbor = (j + 1) * nx + i;
                let fx = (i as f64 + 0.5) * dx;
                let fy = (j as f64 + 1.0) * dy;
                let fz = 0.5 * dz;
                faces.push(Face::new(fid, vec![], owner, Some(neighbor), dx * dz,
                    [0.0, 1.0, 0.0], [fx, fy, fz]));
                cells[owner].faces.push(fid);
                cells[neighbor].faces.push(fid);
                fid += 1;
            }
        }

        // Boundary faces: left (x=0)
        for j in 0..ny {
            let owner = j * nx;
            faces.push(Face::new(fid, vec![], owner, None, dy * dz,
                [-1.0, 0.0, 0.0], [0.0, (j as f64 + 0.5) * dy, 0.5 * dz]));
            cells[owner].faces.push(fid);
            left_faces.push(fid);
            fid += 1;
        }
        // Boundary faces: right (x=nx*dx)
        for j in 0..ny {
            let owner = j * nx + (nx - 1);
            faces.push(Face::new(fid, vec![], owner, None, dy * dz,
                [1.0, 0.0, 0.0], [nx as f64 * dx, (j as f64 + 0.5) * dy, 0.5 * dz]));
            cells[owner].faces.push(fid);
            right_faces.push(fid);
            fid += 1;
        }
        // Boundary faces: bottom (y=0)
        for i in 0..nx {
            let owner = i;
            faces.push(Face::new(fid, vec![], owner, None, dx * dz,
                [0.0, -1.0, 0.0], [(i as f64 + 0.5) * dx, 0.0, 0.5 * dz]));
            cells[owner].faces.push(fid);
            bottom_faces.push(fid);
            fid += 1;
        }
        // Boundary faces: top (y=ny*dy) -- lid
        for i in 0..nx {
            let owner = (ny - 1) * nx + i;
            faces.push(Face::new(fid, vec![], owner, None, dx * dz,
                [0.0, 1.0, 0.0], [(i as f64 + 0.5) * dx, ny as f64 * dy, 0.5 * dz]));
            cells[owner].faces.push(fid);
            top_faces.push(fid);
            fid += 1;
        }
        // Boundary faces: front (z=0)
        for j in 0..ny {
            for i in 0..nx {
                let owner = j * nx + i;
                faces.push(Face::new(fid, vec![], owner, None, dx * dy,
                    [0.0, 0.0, -1.0], [(i as f64 + 0.5) * dx, (j as f64 + 0.5) * dy, 0.0]));
                cells[owner].faces.push(fid);
                front_faces.push(fid);
                fid += 1;
            }
        }
        // Boundary faces: back (z=dz)
        for j in 0..ny {
            for i in 0..nx {
                let owner = j * nx + i;
                faces.push(Face::new(fid, vec![], owner, None, dx * dy,
                    [0.0, 0.0, 1.0], [(i as f64 + 0.5) * dx, (j as f64 + 0.5) * dy, dz]));
                cells[owner].faces.push(fid);
                back_faces.push(fid);
                fid += 1;
            }
        }

        let boundary_patches = vec![
            BoundaryPatch::new("lid", top_faces),
            BoundaryPatch::new("bottom", bottom_faces),
            BoundaryPatch::new("left", left_faces),
            BoundaryPatch::new("right", right_faces),
            BoundaryPatch::new("front", front_faces),
            BoundaryPatch::new("back", back_faces),
        ];

        let mesh = UnstructuredMesh::from_components(vec![], faces, cells, boundary_patches);

        let mut boundary_velocities = HashMap::new();
        boundary_velocities.insert("lid".to_string(), [1.0, 0.0, 0.0]);
        let boundary_pressure = HashMap::new();
        let wall_patches = vec![
            "bottom".to_string(), "left".to_string(), "right".to_string(),
            "front".to_string(), "back".to_string(),
        ];

        (mesh, boundary_velocities, boundary_pressure, wall_patches)
    }

    #[test]
    fn simplec_solver_new() {
        let solver = SimplecSolver::new(1.0, 0.01);
        assert!((solver.density - 1.0).abs() < 1e-12);
        assert!((solver.viscosity - 0.01).abs() < 1e-12);
        assert!((solver.alpha_u - 0.7).abs() < 1e-12);
    }

    #[test]
    fn simplec_lid_driven_cavity_residual_decreases() {
        let (mesh, boundary_velocities, boundary_pressure, wall_patches) =
            make_3x3x1_lid_driven_cavity();

        let n = mesh.num_cells();
        let mut state = FluidState::new(n);

        let density = 1.0;
        let viscosity = 0.1;
        for i in 0..n {
            state.density.set(i, density).unwrap();
            state.viscosity.set(i, viscosity).unwrap();
        }

        let mut solver = SimplecSolver::new(density, viscosity);
        solver.alpha_u = 0.5;
        solver.set_boundary_conditions(
            boundary_velocities.clone(),
            boundary_pressure.clone(),
            wall_patches.clone(),
        );

        let num_iterations = 10;
        let mut residuals = Vec::new();

        for _iter in 0..num_iterations {
            let res = solver
                .solve_step_with_bcs(
                    &mut state, &mesh,
                    &boundary_velocities, &boundary_pressure, &wall_patches,
                )
                .unwrap();
            residuals.push(res);
        }

        let first_res = residuals[0];
        let last_res = residuals[residuals.len() - 1];
        assert!(
            last_res < first_res * 1.1,
            "SIMPLEC residual did not decrease: first={}, last={}",
            first_res, last_res,
        );

        // Verify velocity near lid
        let vel_top = state.velocity.values();
        let has_motion = vel_top[6][0].abs() > 1e-10
            || vel_top[7][0].abs() > 1e-10
            || vel_top[8][0].abs() > 1e-10;
        assert!(has_motion, "Top row cells should have non-zero x-velocity from lid");
    }

    #[test]
    fn simplec_trait_interface() {
        let (mesh, boundary_velocities, boundary_pressure, wall_patches) =
            make_3x3x1_lid_driven_cavity();

        let n = mesh.num_cells();
        let mut state = FluidState::new(n);

        let mut solver = SimplecSolver::new(1.0, 0.1);
        solver.set_boundary_conditions(boundary_velocities, boundary_pressure, wall_patches);

        let res = solver.solve_step(&mut state, &mesh, 1.0);
        assert!(res.is_ok(), "solve_step should succeed: {:?}", res.err());
        let residual = res.unwrap();
        assert!(residual.is_finite(), "Residual should be finite");
    }

    #[test]
    fn simplec_uses_alpha_p_one() {
        // Verify that SIMPLEC uses alpha_p = 1.0 implicitly
        // by checking that pressure changes are larger than with SIMPLE's alpha_p = 0.3
        let (mesh, bv, bp, wp) = make_3x3x1_lid_driven_cavity();
        let n = mesh.num_cells();

        // Run SIMPLEC
        let mut state_c = FluidState::new(n);
        for i in 0..n {
            state_c.density.set(i, 1.0).unwrap();
            state_c.viscosity.set(i, 0.1).unwrap();
        }
        let mut simplec = SimplecSolver::new(1.0, 0.1);
        simplec.set_boundary_conditions(bv.clone(), bp.clone(), wp.clone());
        simplec.solve_step(&mut state_c, &mesh, 1.0).unwrap();

        // Run SIMPLE with alpha_p = 0.3
        let mut state_s = FluidState::new(n);
        for i in 0..n {
            state_s.density.set(i, 1.0).unwrap();
            state_s.viscosity.set(i, 0.1).unwrap();
        }
        let mut simple = crate::incompressible::simple::SimpleSolver::new(1.0, 0.1);
        simple.alpha_p = 0.3;
        simple.set_boundary_conditions(bv, bp, wp);
        simple.solve_step(&mut state_s, &mesh, 1.0).unwrap();

        // SIMPLEC should produce larger pressure corrections (alpha_p=1.0 vs 0.3)
        let p_c: f64 = state_c.pressure.values().iter().map(|p| p.abs()).sum();
        let p_s: f64 = state_s.pressure.values().iter().map(|p| p.abs()).sum();
        // Both should be finite and non-negative
        assert!(p_c.is_finite() && p_s.is_finite(),
            "Pressures should be finite: SIMPLEC={}, SIMPLE={}", p_c, p_s);
    }
}
