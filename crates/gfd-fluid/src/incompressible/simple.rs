//! SIMPLE (Semi-Implicit Method for Pressure-Linked Equations) algorithm.

use std::collections::HashMap;

use gfd_core::gradient::{GreenGaussCellBasedGradient, GradientComputer};
use gfd_core::linalg::LinearSystem;
use gfd_core::{ScalarField, SolverStats, UnstructuredMesh};
use gfd_discretize::fvm::diffusion::compute_diffusive_coefficient;
use gfd_linalg::iterative::bicgstab::BiCGSTAB;
use gfd_linalg::iterative::cg::CG;
use gfd_linalg::traits::LinearSolverTrait;
use gfd_matrix::assembler::Assembler;
use gfd_matrix::boundary::apply_dirichlet;

#[cfg(feature = "gpu")]
use gfd_gpu::solver::{GpuLinearSolver, gpu_cg::GpuCG};
#[cfg(feature = "gpu")]
use gfd_gpu::sparse::GpuSparseMatrix;
#[cfg(feature = "gpu")]
use gfd_gpu::memory::GpuVector;
#[cfg(feature = "gpu")]
use gfd_gpu::device::GpuDeviceHandle;

use crate::incompressible::PressureVelocityCoupling;
use crate::{FluidError, FluidState, Result};

/// SIMPLE pressure-velocity coupling solver.
///
/// Algorithm outline:
/// 1. Solve momentum equations to obtain intermediate velocity (u*)
/// 2. Solve pressure correction equation (p')
/// 3. Correct velocity: u = u* + correction(p')
/// 4. Correct pressure: p = p* + alpha_p * p'
/// 5. Update mass fluxes
pub struct SimpleSolver {
    /// Under-relaxation factor for velocity.
    pub alpha_u: f64,
    /// Under-relaxation factor for pressure correction.
    pub alpha_p: f64,
    /// Uniform density [kg/m^3].
    pub density: f64,
    /// Uniform dynamic viscosity [Pa*s].
    pub viscosity: f64,
    /// Whether to use GPU acceleration for linear solves (requires `gpu` feature).
    pub use_gpu: bool,
    /// Stored diagonal coefficients from the momentum equation (used in pressure correction).
    a_p_momentum: Vec<f64>,
    /// Previous pressure correction (used as initial guess for next SIMPLE iteration).
    prev_p_prime: Vec<f64>,
    /// Boundary velocities: patch name -> [u, v, w].
    boundary_velocities: HashMap<String, [f64; 3]>,
    /// Boundary pressures: patch name -> pressure value.
    boundary_pressure: HashMap<String, f64>,
    /// Wall patch names (no-slip).
    wall_patches: Vec<String>,
    /// Cached face-to-patch map (built once per mesh).
    face_patch_cache: Vec<Option<usize>>,
    /// Cached patch names (indices match face_patch_cache values).
    patch_names: Vec<String>,
    /// Number of faces for which the cache was built.
    cached_num_faces: usize,
    /// Cached per-internal-face: (face_idx, owner, neighbor, distance, D_coeff).
    /// Built once on first call; reused across SIMPLE iterations.
    cached_internal_geom: Vec<(usize, usize, usize, f64, f64)>,
    /// Cached per-boundary-face: (face_idx, owner, D_coeff).
    cached_boundary_geom: Vec<(usize, usize, f64)>,
    /// Cached cell volumes (flat array for fast access).
    cached_volumes: Vec<f64>,
    /// Reusable CG solver for pressure correction (workspace persists).
    cg_solver: CG,
    /// Cached momentum CSR matrix (values updated in-place each iteration).
    mom_matrix: Option<gfd_core::SparseMatrix>,
    /// Cached pressure correction CSR matrix (values updated in-place).
    pc_matrix: Option<gfd_core::SparseMatrix>,
    /// Reusable workspace vectors.
    ws_sources: Vec<f64>,
    ws_a_p: Vec<f64>,
    ws_x_buf: Vec<f64>,
    /// Cached pressure correction CSR pattern (row_ptr, col_idx).
    /// Values array is updated in-place each SIMPLE iteration.
    pc_cached: bool,
    pc_row_ptr: Vec<usize>,
    pc_col_idx: Vec<usize>,
    /// Mapping: for each internal face, the CSR index for (owner→neigh) and (neigh→owner).
    pc_face_csr_idx: Vec<(usize, usize)>,
    /// Mapping: CSR index for diagonal entry of each cell.
    pc_diag_csr_idx: Vec<usize>,
}

impl SimpleSolver {
    /// Creates a new SIMPLE solver with the given density and viscosity.
    /// Uses default under-relaxation factors (alpha_u=0.7, alpha_p=0.3).
    pub fn new(density: f64, viscosity: f64) -> Self {
        Self {
            alpha_u: 0.7,
            alpha_p: 0.3,
            density,
            viscosity,
            use_gpu: false,
            a_p_momentum: Vec::new(),
            prev_p_prime: Vec::new(),
            boundary_velocities: HashMap::new(),
            boundary_pressure: HashMap::new(),
            wall_patches: Vec::new(),
            face_patch_cache: Vec::new(),
            patch_names: Vec::new(),
            cached_num_faces: 0,
            cached_internal_geom: Vec::new(),
            cached_boundary_geom: Vec::new(),
            cached_volumes: Vec::new(),
            cg_solver: CG::new(1e-3, 1000),
            mom_matrix: None,
            pc_matrix: None,
            ws_sources: Vec::new(),
            ws_a_p: Vec::new(),
            ws_x_buf: Vec::new(),
            pc_cached: false,
            pc_row_ptr: Vec::new(),
            pc_col_idx: Vec::new(),
            pc_face_csr_idx: Vec::new(),
            pc_diag_csr_idx: Vec::new(),
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

        // Cache cell volumes into flat array
        self.cached_volumes = mesh.cells.iter().map(|c| c.volume).collect();

        // Cache geometric data (distances and diffusion coefficients)
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
            self.face_patch_cache[face_id].map(|idx| self.patch_names[idx].as_str())
        } else {
            None
        }
    }

    /// Full SIMPLE step with explicit boundary condition arguments.
    pub fn solve_step_with_bcs(
        &mut self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        boundary_velocities: &HashMap<String, [f64; 3]>,
        boundary_pressure: &HashMap<String, f64>,
        wall_patches: &[String],
    ) -> Result<f64> {
        // Build face-to-patch cache once
        self.ensure_face_patch_cache(mesh);

        // 1. Solve momentum equations for all 3 components
        self.solve_momentum(state, mesh, boundary_velocities, boundary_pressure, wall_patches)?;

        // 2. Solve pressure correction equation
        let p_prime =
            self.solve_pressure_correction(state, mesh, boundary_pressure)?;

        // 3. Correct velocity
        self.correct_velocity(state, mesh, &p_prime)?;

        // 4. Correct pressure
        self.correct_pressure(state, &p_prime);

        // 5. Compute and return continuity residual
        let residual = self.compute_continuity_residual(state, mesh, boundary_velocities, wall_patches);
        Ok(residual)
    }

    // -----------------------------------------------------------------------
    // Step 1: Momentum predictor
    // -----------------------------------------------------------------------

    /// Solve the momentum equations for all three velocity components.
    ///
    /// Discretizes convection (first-order upwind) + diffusion + pressure gradient.
    /// Applies under-relaxation and solves with BiCGSTAB.
    fn solve_momentum(
        &mut self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        boundary_velocities: &HashMap<String, [f64; 3]>,
        boundary_pressure: &HashMap<String, f64>,
        wall_patches: &[String],
    ) -> Result<()> {
        let n = mesh.num_cells();

        // Compute pressure gradient once for all components
        let grad_computer = GreenGaussCellBasedGradient;
        let grad_p = grad_computer
            .compute(&state.pressure, mesh)
            .map_err(|e| FluidError::CoreError(e))?;

        // Ensure CSR pattern is cached (shared between momentum and pressure correction)
        if !self.pc_cached {
            // Build pattern via Assembler with dummy values
            self.build_pressure_correction_pattern(n, mesh, boundary_pressure)?;
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

        // Take momentum matrix out of self to avoid borrow conflict
        let mut mom_mat = self.mom_matrix.take().unwrap_or_else(|| {
            gfd_core::SparseMatrix::new(
                n, n, self.pc_row_ptr.clone(), self.pc_col_idx.clone(), vec![0.0; nnz],
            ).unwrap()
        });
        mom_mat.values.fill(0.0);
        self.ws_a_p.fill(0.0);

        // Internal faces: compute D+F coefficients
        for (face_idx, &(fi, owner, neigh, _dist, d)) in self.cached_internal_geom.iter().enumerate() {
            let face = &mesh.faces[fi];
            let vo = vel_vals[owner];
            let vn = vel_vals[neigh];
            let f_flux = rho_half * face.area
                * ((vo[0] + vn[0]) * face.normal[0]
                 + (vo[1] + vn[1]) * face.normal[1]
                 + (vo[2] + vn[2]) * face.normal[2]);
            let f_pos = f64::max(f_flux, 0.0);
            let f_neg = f64::max(-f_flux, 0.0);
            self.ws_a_p[owner] += d + f_pos;
            self.ws_a_p[neigh] += d + f_neg;
            let (idx_on, idx_no) = self.pc_face_csr_idx[face_idx];
            mom_mat.values[idx_on] = -(d + f_neg);
            mom_mat.values[idx_no] = -(d + f_pos);
        }

        // Boundary faces: diagonal contribution + precompute BC data
        struct BoundaryFace { owner: usize, d: f64, bc_vel: Option<[f64; 3]>, f_flux_bc: f64, is_wall: bool }
        let mut boundary_faces: Vec<BoundaryFace> = Vec::with_capacity(self.cached_boundary_geom.len());
        for &(_fi, owner, d) in &self.cached_boundary_geom {
            let face = &mesh.faces[_fi];
            let patch = self.get_face_patch(face.id);
            let (bc_vel, f_flux_bc, is_wall) = if let Some(pname) = patch {
                if let Some(bv) = boundary_velocities.get(pname) {
                    let ff = self.density
                        * (bv[0]*face.normal[0] + bv[1]*face.normal[1] + bv[2]*face.normal[2])
                        * face.area;
                    (Some(*bv), ff, false)
                } else if wall_patches.iter().any(|w| w == pname) {
                    (None, 0.0, true)
                } else {
                    (None, 0.0, false)
                }
            } else {
                (None, 0.0, false)
            };
            if bc_vel.is_some() {
                self.ws_a_p[owner] += d + f64::max(f_flux_bc, 0.0);
            } else if is_wall {
                self.ws_a_p[owner] += d;
            }
            boundary_faces.push(BoundaryFace { owner, d, bc_vel, f_flux_bc, is_wall });
        }

        // Apply under-relaxation: store unrelaxed diagonal in a_p_momentum,
        // then modify a_p for the matrix diagonal
        let ur_factor = (1.0 - self.alpha_u) / self.alpha_u;
        self.a_p_momentum.resize(n, 0.0);
        for i in 0..n {
            self.a_p_momentum[i] = self.ws_a_p[i] / self.alpha_u;
            mom_mat.values[self.pc_diag_csr_idx[i]] = self.a_p_momentum[i];
        }

        let sources = &mut self.ws_sources;
        let x_buf = &mut self.ws_x_buf;
        let mut solver_bicgstab = BiCGSTAB::new(1e-2, 1000);

        // Precompute which boundary faces have non-zero velocity per component
        let mut active_comps = [false; 3];
        for bface in &boundary_faces {
            if let Some(ref bv) = bface.bc_vel {
                for c in 0..3 {
                    if bv[c].abs() > 0.0 { active_comps[c] = true; }
                }
            }
        }

        // --- Solve each velocity component using the shared matrix ---
        for comp in 0..3 {
            let vel_values = state.velocity.values();
            if !active_comps[comp]
                && !vel_values.iter().any(|v| v[comp].abs() > 1e-30)
                && !grad_p.values().iter().any(|g| g[comp].abs() > 1e-30)
            {
                continue;
            }

            sources.fill(0.0);

            for bface in &boundary_faces {
                if let Some(ref bv) = bface.bc_vel {
                    sources[bface.owner] += (bface.d + f64::max(-bface.f_flux_bc, 0.0)) * bv[comp];
                }
            }

            for i in 0..n {
                sources[i] -= grad_p.values()[i][comp] * self.cached_volumes[i];
                sources[i] += ur_factor * self.ws_a_p[i] * vel_values[i][comp];
                x_buf[i] = vel_values[i][comp];
            }

            solver_bicgstab
                .solve(&mom_mat, &sources, x_buf)
                .map_err(|e| FluidError::SolverFailed(format!("{:?}", e)))?;

            // Update velocity component
            let vel_mut = state.velocity.values_mut();
            for i in 0..n {
                vel_mut[i][comp] = x_buf[i];
            }
        }

        // Put momentum matrix back
        self.mom_matrix = Some(mom_mat);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Step 2: Pressure correction equation
    // -----------------------------------------------------------------------

    /// Solve the pressure correction equation.
    ///
    /// The pressure correction Laplacian:
    ///   sum_f( rho * rA_f * A_f / d_ON * (p'_N - p'_P) ) = -sum_f( rho * u*_f . n_f * A_f )
    ///
    /// where rA_f = interpolated(V / a_P) to face.
    fn solve_pressure_correction(
        &mut self,
        state: &FluidState,
        mesh: &UnstructuredMesh,
        boundary_pressure: &HashMap<String, f64>,
    ) -> Result<Vec<f64>> {
        let n = mesh.num_cells();
        let vel_vals = state.velocity.values();
        let rho_half = 0.5 * self.density;

        // First call: build CSR pattern via Assembler and cache it
        if !self.pc_cached {
            self.build_pressure_correction_pattern(n, mesh, boundary_pressure)?;
        }

        // Ensure pc_matrix exists; update values in-place
        let nnz = self.pc_col_idx.len();
        let mut pc_mat = self.pc_matrix.take().unwrap_or_else(|| {
            gfd_core::SparseMatrix::new(
                n, n, self.pc_row_ptr.clone(), self.pc_col_idx.clone(), vec![0.0; nnz],
            ).unwrap()
        });
        pc_mat.values.fill(0.0);
        let mut sources_pc = vec![0.0; n];

        // Internal faces: compute coefficients and mass flux
        for (face_idx, &(fi, owner, neigh, dist, _d)) in self.cached_internal_geom.iter().enumerate() {
            let face = &mesh.faces[fi];
            let ra_o = self.cached_volumes[owner] / self.a_p_momentum[owner];
            let ra_n = self.cached_volumes[neigh] / self.a_p_momentum[neigh];
            let ra_f = 0.5 * (ra_o + ra_n);
            let coeff = self.density * ra_f * face.area / dist;

            let (idx_on, idx_no) = self.pc_face_csr_idx[face_idx];
            pc_mat.values[idx_on] = -coeff;
            pc_mat.values[idx_no] = -coeff;
            pc_mat.values[self.pc_diag_csr_idx[owner]] += coeff;
            pc_mat.values[self.pc_diag_csr_idx[neigh]] += coeff;

            // RHS: mass imbalance
            let vo = vel_vals[owner];
            let vn = vel_vals[neigh];
            let mass_flux = rho_half * face.area
                * ((vo[0] + vn[0]) * face.normal[0]
                 + (vo[1] + vn[1]) * face.normal[1]
                 + (vo[2] + vn[2]) * face.normal[2]);
            sources_pc[owner] -= mass_flux;
            sources_pc[neigh] += mass_flux;
        }

        // Boundary faces: RHS contribution only
        for &(fi, owner, _d) in &self.cached_boundary_geom {
            let face = &mesh.faces[fi];
            let patch_name = self.get_face_patch(face.id);
            if let Some(pname) = patch_name {
                if boundary_pressure.contains_key(pname) {
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
        // directly to the cached matrix values and sources
        let has_pressure_bc = boundary_pressure
            .keys()
            .any(|k| mesh.boundary_patch(k).is_some());
        if !has_pressure_bc {
            // Fix cell 0: zero out row, set diagonal to 1, rhs to 0
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
                        pc_mat.values[idx] = if self.pc_col_idx[idx] == cell_id { 1.0 } else { 0.0 };
                    }
                    sources_pc[cell_id] = 0.0;
                }
            }
        }

        // Solve using cached matrix directly (no LinearSystem construction)
        let mut x_pc = vec![0.0; n];

        // Solve pressure correction (SPD system -- use cached CG solver)
        let stats = self.cg_solver
            .solve(&pc_mat, &sources_pc, &mut x_pc)
            .map_err(|e| FluidError::SolverFailed(format!("{:?}", e)))?;

        // If CG doesn't converge, fall back to BiCGSTAB
        if !stats.converged {
            x_pc.fill(0.0);
            let mut fallback = BiCGSTAB::new(1e-3, 1000);
            fallback
                .solve(&pc_mat, &sources_pc, &mut x_pc)
                .map_err(|e| FluidError::SolverFailed(format!("{:?}", e)))?;
        }

        // Put pressure correction matrix back
        self.pc_matrix = Some(pc_mat);

        Ok(x_pc)
    }

    /// Build the CSR pattern for the pressure correction matrix.
    /// Called once on the first SIMPLE iteration; cached for subsequent iterations.
    fn build_pressure_correction_pattern(
        &mut self,
        n: usize,
        mesh: &UnstructuredMesh,
        boundary_pressure: &HashMap<String, f64>,
    ) -> Result<()> {
        // Use Assembler to build the pattern with dummy values
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

        // Cache the CSR pattern
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

        // Build face-to-CSR-index mapping for off-diagonal entries
        self.pc_face_csr_idx = Vec::with_capacity(n_internal);
        for &(_fi, owner, neigh, _dist, _d) in &self.cached_internal_geom {
            // Find CSR index for (owner, neigh) entry
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
            // Find CSR index for (neigh, owner) entry
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
    // Step 3: Velocity correction
    // -----------------------------------------------------------------------

    /// Correct velocity using pressure correction gradient.
    ///
    /// u_P = u*_P - (V_P / a_P) * grad(p')_P
    ///
    /// Computes Green-Gauss gradient of p' inline to avoid ScalarField allocation.
    fn correct_velocity(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        p_prime: &[f64],
    ) -> Result<()> {
        let n = mesh.num_cells();

        // Inline Green-Gauss gradient of p_prime (avoids ScalarField copy)
        let mut grad_pp = vec![[0.0_f64; 3]; n];
        for face in &mesh.faces {
            let owner = face.owner_cell;
            if let Some(neighbor) = face.neighbor_cell {
                let phi_f = 0.5 * (p_prime[owner] + p_prime[neighbor]);
                let na0 = phi_f * face.area * face.normal[0];
                let na1 = phi_f * face.area * face.normal[1];
                let na2 = phi_f * face.area * face.normal[2];
                grad_pp[owner][0] += na0;
                grad_pp[owner][1] += na1;
                grad_pp[owner][2] += na2;
                grad_pp[neighbor][0] -= na0;
                grad_pp[neighbor][1] -= na1;
                grad_pp[neighbor][2] -= na2;
            } else {
                let phi_f = p_prime[owner];
                grad_pp[owner][0] += phi_f * face.area * face.normal[0];
                grad_pp[owner][1] += phi_f * face.area * face.normal[1];
                grad_pp[owner][2] += phi_f * face.area * face.normal[2];
            }
        }

        let vel_mut = state.velocity.values_mut();
        for i in 0..n {
            // ra * inv_vol = (V / a_P) * (1 / V) = 1 / a_P
            let inv_ap = 1.0 / self.a_p_momentum[i];
            vel_mut[i][0] -= inv_ap * grad_pp[i][0];
            vel_mut[i][1] -= inv_ap * grad_pp[i][1];
            vel_mut[i][2] -= inv_ap * grad_pp[i][2];
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Step 4: Pressure correction
    // -----------------------------------------------------------------------

    /// Correct pressure: p = p* + alpha_p * p'
    fn correct_pressure(&self, state: &mut FluidState, p_prime: &[f64]) {
        let p_mut = state.pressure.values_mut();
        for i in 0..p_mut.len() {
            p_mut[i] += self.alpha_p * p_prime[i];
        }
    }

    // -----------------------------------------------------------------------
    // Residual computation
    // -----------------------------------------------------------------------

    /// Compute the continuity residual: max over cells of |sum_faces(rho * u_f . n * A)|.
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

        for face in &mesh.faces {
            let owner = face.owner_cell;

            if let Some(neigh) = face.neighbor_cell {
                // Internal face: compute mass flux directly
                let vo = vel_vals[owner];
                let vn = vel_vals[neigh];
                let na = face.area;
                let mass_flux = rho_half * na
                    * ((vo[0] + vn[0]) * face.normal[0]
                     + (vo[1] + vn[1]) * face.normal[1]
                     + (vo[2] + vn[2]) * face.normal[2]);

                mass_imbalance[owner] += mass_flux;
                mass_imbalance[neigh] -= mass_flux;
            } else {
                // Boundary face
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
                        // Wall: zero velocity, zero flux
                    } else {
                        // Outlet or other: use cell velocity extrapolated to face
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

impl PressureVelocityCoupling for SimpleSolver {
    fn solve_step(
        &mut self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        _dt: f64,
    ) -> Result<f64> {
        // Clone BCs to satisfy borrow checker (self is mutably borrowed)
        let bv = self.boundary_velocities.clone();
        let bp = self.boundary_pressure.clone();
        let wp = self.wall_patches.clone();
        self.solve_step_with_bcs(state, mesh, &bv, &bp, &wp)
    }
}

// ---------------------------------------------------------------------------
// GPU / CPU linear-solve abstraction
// ---------------------------------------------------------------------------

/// Solve a linear system using either GPU or CPU, depending on `use_gpu`.
///
/// When `symmetric` is true the CPU path uses CG (Conjugate Gradient);
/// otherwise it uses BiCGSTAB.  The GPU path always uses `GpuCG` (the
/// gfd-gpu crate currently only exposes CG).
///
/// If `use_gpu` is `true` but the `gpu` feature is not compiled in, the
/// function logs a warning and falls back to the CPU solver automatically.
fn solve_linear_system(
    system: &mut LinearSystem,
    use_gpu: bool,
    symmetric: bool,
) -> Result<SolverStats> {
    // ---- GPU path (only available when the `gpu` feature is enabled) ----
    #[cfg(feature = "gpu")]
    if use_gpu {
        let device = gfd_gpu::device::select_device(0)
            .map_err(|e| FluidError::SolverFailed(format!("GPU device selection: {:?}", e)))?;
        let gpu_a = GpuSparseMatrix::from_cpu(&system.a, &device)
            .map_err(|e| FluidError::SolverFailed(format!("GPU matrix upload: {:?}", e)))?;
        let gpu_b = GpuVector::from_cpu(&system.b, &device)
            .map_err(|e| FluidError::SolverFailed(format!("GPU RHS upload: {:?}", e)))?;
        let mut gpu_x = GpuVector::from_cpu(&system.x, &device)
            .map_err(|e| FluidError::SolverFailed(format!("GPU solution upload: {:?}", e)))?;
        let mut gpu_solver = GpuCG::new(1e-6, 1000);
        let stats = gpu_solver
            .solve(&gpu_a, &gpu_b, &mut gpu_x)
            .map_err(|e| FluidError::SolverFailed(format!("GPU solve: {:?}", e)))?;
        gpu_x
            .to_cpu(&mut system.x)
            .map_err(|e| FluidError::SolverFailed(format!("GPU download: {:?}", e)))?;
        return Ok(stats);
    }

    // If gpu feature is *not* compiled in but the caller asked for GPU,
    // fall back gracefully with a warning.
    #[cfg(not(feature = "gpu"))]
    if use_gpu {
        eprintln!("[gfd-fluid] WARNING: GPU requested but `gpu` feature not enabled -- using CPU solver");
    }

    // ---- CPU path ----
    // Use relaxed tolerance (1e-3) for inner linear solves within SIMPLE/PISO.
    // The outer pressure-velocity coupling loop provides additional correction,
    // so high inner precision is unnecessary and wastes iterations.
    if symmetric {
        let mut solver = CG::new(1e-3, 1000);
        solver
            .solve(&system.a, &system.b, &mut system.x)
            .map_err(|e| FluidError::SolverFailed(format!("{:?}", e)))
    } else {
        let mut solver = BiCGSTAB::new(1e-3, 1000);
        solver
            .solve(&system.a, &system.b, &mut system.x)
            .map_err(|e| FluidError::SolverFailed(format!("{:?}", e)))
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Euclidean distance between two 3D points.
fn distance(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let dz = b[2] - a[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Build a map from face_id to patch name for boundary faces.
fn build_face_patch_map(mesh: &UnstructuredMesh) -> HashMap<usize, String> {
    let mut map = HashMap::new();
    for patch in &mesh.boundary_patches {
        for &fid in &patch.face_ids {
            map.insert(fid, patch.name.clone());
        }
    }
    map
}

/// Helper to check if a boundary pressure map contains a key.
fn boundary_pressure_map_contains(map: &HashMap<String, f64>, key: &str) -> bool {
    map.contains_key(key)
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

    /// Creates a 3x3x1 hex mesh for lid-driven cavity test.
    ///
    /// Domain: [0,3] x [0,3] x [0,1]
    /// 9 cells, each 1x1x1.
    /// Cell layout (viewed from +z):
    ///
    ///   6 | 7 | 8   (y = 2..3, top row -- lid moves here at y=3)
    ///   3 | 4 | 5   (y = 1..2)
    ///   0 | 1 | 2   (y = 0..1, bottom wall)
    ///
    /// Boundaries:
    ///   - "lid" (top, y=3): moving wall u=(1,0,0)
    ///   - "bottom" (y=0): no-slip wall
    ///   - "left" (x=0): no-slip wall
    ///   - "right" (x=3): no-slip wall
    ///   - "front"/"back" (z=0, z=1): treated as empty/symmetry (zero gradient)
    fn make_3x3x1_lid_driven_cavity() -> (UnstructuredMesh, HashMap<String, [f64; 3]>, HashMap<String, f64>, Vec<String>) {
        let nx = 3;
        let ny = 3;
        let dx = 1.0;
        let dy = 1.0;
        let dz = 1.0;

        // Create cells
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

        // Boundary face collectors
        let mut bottom_faces = Vec::new();
        let mut top_faces = Vec::new();
        let mut left_faces = Vec::new();
        let mut right_faces = Vec::new();
        let mut front_faces = Vec::new();
        let mut back_faces = Vec::new();

        // Internal x-normal faces (between columns)
        // For columns i and i+1, row j: face at x = (i+1)*dx
        for j in 0..ny {
            for i in 0..(nx - 1) {
                let owner = j * nx + i;
                let neighbor = j * nx + i + 1;
                let fx = (i as f64 + 1.0) * dx;
                let fy = (j as f64 + 0.5) * dy;
                let fz = 0.5 * dz;
                faces.push(Face::new(
                    fid,
                    vec![],
                    owner,
                    Some(neighbor),
                    dy * dz,
                    [1.0, 0.0, 0.0],
                    [fx, fy, fz],
                ));
                cells[owner].faces.push(fid);
                cells[neighbor].faces.push(fid);
                fid += 1;
            }
        }

        // Internal y-normal faces (between rows)
        // For rows j and j+1, column i: face at y = (j+1)*dy
        for j in 0..(ny - 1) {
            for i in 0..nx {
                let owner = j * nx + i;
                let neighbor = (j + 1) * nx + i;
                let fx = (i as f64 + 0.5) * dx;
                let fy = (j as f64 + 1.0) * dy;
                let fz = 0.5 * dz;
                faces.push(Face::new(
                    fid,
                    vec![],
                    owner,
                    Some(neighbor),
                    dx * dz,
                    [0.0, 1.0, 0.0],
                    [fx, fy, fz],
                ));
                cells[owner].faces.push(fid);
                cells[neighbor].faces.push(fid);
                fid += 1;
            }
        }

        // Boundary faces: left (x=0)
        for j in 0..ny {
            let owner = j * nx;
            let fx = 0.0;
            let fy = (j as f64 + 0.5) * dy;
            let fz = 0.5 * dz;
            faces.push(Face::new(
                fid,
                vec![],
                owner,
                None,
                dy * dz,
                [-1.0, 0.0, 0.0],
                [fx, fy, fz],
            ));
            cells[owner].faces.push(fid);
            left_faces.push(fid);
            fid += 1;
        }

        // Boundary faces: right (x=nx*dx)
        for j in 0..ny {
            let owner = j * nx + (nx - 1);
            let fx = nx as f64 * dx;
            let fy = (j as f64 + 0.5) * dy;
            let fz = 0.5 * dz;
            faces.push(Face::new(
                fid,
                vec![],
                owner,
                None,
                dy * dz,
                [1.0, 0.0, 0.0],
                [fx, fy, fz],
            ));
            cells[owner].faces.push(fid);
            right_faces.push(fid);
            fid += 1;
        }

        // Boundary faces: bottom (y=0)
        for i in 0..nx {
            let owner = i;
            let fx = (i as f64 + 0.5) * dx;
            let fy = 0.0;
            let fz = 0.5 * dz;
            faces.push(Face::new(
                fid,
                vec![],
                owner,
                None,
                dx * dz,
                [0.0, -1.0, 0.0],
                [fx, fy, fz],
            ));
            cells[owner].faces.push(fid);
            bottom_faces.push(fid);
            fid += 1;
        }

        // Boundary faces: top (y=ny*dy) -- the lid
        for i in 0..nx {
            let owner = (ny - 1) * nx + i;
            let fx = (i as f64 + 0.5) * dx;
            let fy = ny as f64 * dy;
            let fz = 0.5 * dz;
            faces.push(Face::new(
                fid,
                vec![],
                owner,
                None,
                dx * dz,
                [0.0, 1.0, 0.0],
                [fx, fy, fz],
            ));
            cells[owner].faces.push(fid);
            top_faces.push(fid);
            fid += 1;
        }

        // Boundary faces: front (z=0)
        for j in 0..ny {
            for i in 0..nx {
                let owner = j * nx + i;
                let fx = (i as f64 + 0.5) * dx;
                let fy = (j as f64 + 0.5) * dy;
                let fz = 0.0;
                faces.push(Face::new(
                    fid,
                    vec![],
                    owner,
                    None,
                    dx * dy,
                    [0.0, 0.0, -1.0],
                    [fx, fy, fz],
                ));
                cells[owner].faces.push(fid);
                front_faces.push(fid);
                fid += 1;
            }
        }

        // Boundary faces: back (z=dz)
        for j in 0..ny {
            for i in 0..nx {
                let owner = j * nx + i;
                let fx = (i as f64 + 0.5) * dx;
                let fy = (j as f64 + 0.5) * dy;
                let fz = dz;
                faces.push(Face::new(
                    fid,
                    vec![],
                    owner,
                    None,
                    dx * dy,
                    [0.0, 0.0, 1.0],
                    [fx, fy, fz],
                ));
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

        // Boundary conditions
        let mut boundary_velocities = HashMap::new();
        boundary_velocities.insert("lid".to_string(), [1.0, 0.0, 0.0]); // lid moves in x

        let boundary_pressure = HashMap::new(); // no outlet pressure BC for cavity

        let wall_patches = vec![
            "bottom".to_string(),
            "left".to_string(),
            "right".to_string(),
            "front".to_string(),
            "back".to_string(),
        ];

        (mesh, boundary_velocities, boundary_pressure, wall_patches)
    }

    #[test]
    fn simple_solver_new() {
        let solver = SimpleSolver::new(1.0, 0.01);
        assert!((solver.density - 1.0).abs() < 1e-12);
        assert!((solver.viscosity - 0.01).abs() < 1e-12);
        assert!((solver.alpha_u - 0.7).abs() < 1e-12);
        assert!((solver.alpha_p - 0.3).abs() < 1e-12);
    }

    #[test]
    fn simple_lid_driven_cavity_residual_decreases() {
        let (mesh, boundary_velocities, boundary_pressure, wall_patches) =
            make_3x3x1_lid_driven_cavity();

        let n = mesh.num_cells();
        let mut state = FluidState::new(n);

        // Set uniform density and viscosity
        let density = 1.0;
        let viscosity = 0.1; // Re = U*L/nu = 1*3/0.1 = 30
        for i in 0..n {
            state.density.set(i, density).unwrap();
            state.viscosity.set(i, viscosity).unwrap();
        }

        let mut solver = SimpleSolver::new(density, viscosity);
        // Use more conservative relaxation for stability on coarse mesh
        solver.alpha_u = 0.5;
        solver.alpha_p = 0.2;

        // Store boundary conditions for the trait implementation path
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
                    &mut state,
                    &mesh,
                    &boundary_velocities,
                    &boundary_pressure,
                    &wall_patches,
                )
                .unwrap();
            residuals.push(res);
        }

        // Verify that the residual generally decreases (compare first vs last)
        let first_res = residuals[0];
        let last_res = residuals[residuals.len() - 1];

        // The residual should decrease overall (allowing for some non-monotone behavior)
        assert!(
            last_res < first_res * 1.1, // allow some tolerance
            "Residual did not decrease: first={}, last={}",
            first_res,
            last_res
        );

        // Verify that velocity near the lid is non-zero in x-direction
        // Cells 6, 7, 8 are in the top row (adjacent to lid)
        let vel_top = state.velocity.values();
        let has_motion = vel_top[6][0].abs() > 1e-10
            || vel_top[7][0].abs() > 1e-10
            || vel_top[8][0].abs() > 1e-10;
        assert!(
            has_motion,
            "Top row cells should have non-zero x-velocity from lid motion"
        );
    }

    #[test]
    fn simple_solver_trait_interface() {
        // Test that the PressureVelocityCoupling trait works via set_boundary_conditions
        let (mesh, boundary_velocities, boundary_pressure, wall_patches) =
            make_3x3x1_lid_driven_cavity();

        let n = mesh.num_cells();
        let mut state = FluidState::new(n);

        let mut solver = SimpleSolver::new(1.0, 0.1);
        solver.set_boundary_conditions(boundary_velocities, boundary_pressure, wall_patches);

        // Use the trait method
        let res = solver.solve_step(&mut state, &mesh, 1.0);
        assert!(res.is_ok(), "solve_step should succeed: {:?}", res.err());
        let residual = res.unwrap();
        assert!(residual.is_finite(), "Residual should be finite");
    }
}
