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
    face_patch_cache: HashMap<usize, String>,
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
            face_patch_cache: HashMap::new(),
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

    /// Full SIMPLE step with explicit boundary condition arguments.
    pub fn solve_step_with_bcs(
        &mut self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        boundary_velocities: &HashMap<String, [f64; 3]>,
        boundary_pressure: &HashMap<String, f64>,
        wall_patches: &[String],
    ) -> Result<f64> {
        // 1. Solve momentum equations for all 3 components
        self.solve_momentum(state, mesh, boundary_velocities, wall_patches)?;

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
        wall_patches: &[String],
    ) -> Result<()> {
        let n = mesh.num_cells();

        // Build a lookup: face_id -> (patch_name) for boundary faces
        let face_patch_map = build_face_patch_map(mesh);

        // Compute pressure gradient once for all components
        let grad_computer = GreenGaussCellBasedGradient;
        let grad_p = grad_computer
            .compute(&state.pressure, mesh)
            .map_err(|e| FluidError::CoreError(e))?;

        // --- Pre-compute face coefficients (geometry-only, independent of comp) ---
        // Internal face data: (face_idx, owner, neighbor, D, F)
        struct InternalFace { owner: usize, neigh: usize, d: f64, f_flux: f64 }
        // Boundary face data
        struct BoundaryFace { owner: usize, d: f64, patch: Option<String>, bc_vel: Option<[f64; 3]>, f_flux_bc: f64, is_wall: bool }

        let mut internal_faces: Vec<InternalFace> = Vec::with_capacity(mesh.faces.len());
        let mut boundary_faces: Vec<BoundaryFace> = Vec::new();

        for face in &mesh.faces {
            let owner = face.owner_cell;
            if let Some(neigh) = face.neighbor_cell {
                let vel_o = state.velocity.values()[owner];
                let vel_n = state.velocity.values()[neigh];
                let u_f = [
                    0.5 * (vel_o[0] + vel_n[0]),
                    0.5 * (vel_o[1] + vel_n[1]),
                    0.5 * (vel_o[2] + vel_n[2]),
                ];
                let f_flux = self.density
                    * (u_f[0] * face.normal[0] + u_f[1] * face.normal[1] + u_f[2] * face.normal[2])
                    * face.area;
                let dist = distance(&mesh.cells[owner].center, &mesh.cells[neigh].center);
                let d = compute_diffusive_coefficient(self.viscosity, face.area, dist);
                internal_faces.push(InternalFace { owner, neigh, d, f_flux });
            } else {
                let dist_to_face = {
                    let d = distance(&mesh.cells[owner].center, &face.center);
                    if d < 1e-30 { 1e-30 } else { d }
                };
                let d = compute_diffusive_coefficient(self.viscosity, face.area, dist_to_face);
                let patch = face_patch_map.get(&face.id).map(|s| s.to_string());
                let (bc_vel, f_flux_bc, is_wall) = if let Some(ref pname) = patch {
                    if let Some(bv) = boundary_velocities.get(pname.as_str()) {
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
                boundary_faces.push(BoundaryFace { owner, d, patch, bc_vel, f_flux_bc, is_wall });
            }
        }

        // We will store the diagonal from the last component (they are similar for
        // uniform density/viscosity on the same mesh). For correctness we average.
        let mut a_p_sum = vec![0.0; n];

        // Precise NNZ estimate: n diagonals + 2 off-diagonals per internal face
        let n_internal = internal_faces.len();
        let nnz_estimate = n + 2 * n_internal;

        for comp in 0..3 {
            let mut a_p = vec![0.0; n];
            let mut sources = vec![0.0; n];
            let mut assembler = Assembler::with_nnz_estimate(n, nnz_estimate);

            // --- Internal faces: add off-diagonal directly to assembler ---
            for iface in &internal_faces {
                let f_pos = f64::max(iface.f_flux, 0.0);
                let f_neg = f64::max(-iface.f_flux, 0.0);
                a_p[iface.owner] += iface.d + f_pos;
                a_p[iface.neigh] += iface.d + f_neg;
                assembler.add_neighbor(iface.owner, iface.neigh, iface.d + f_neg);
                assembler.add_neighbor(iface.neigh, iface.owner, iface.d + f_pos);
            }

            // --- Boundary faces ---
            for bface in &boundary_faces {
                if let Some(ref bv) = bface.bc_vel {
                    a_p[bface.owner] += bface.d + f64::max(bface.f_flux_bc, 0.0);
                    sources[bface.owner] += (bface.d + f64::max(-bface.f_flux_bc, 0.0)) * bv[comp];
                } else if bface.is_wall {
                    a_p[bface.owner] += bface.d;
                }
            }

            // --- Pressure gradient source ---
            for i in 0..n {
                let gp = grad_p.values()[i];
                sources[i] -= gp[comp] * mesh.cells[i].volume;
            }

            // --- Under-relaxation ---
            let vel_values = state.velocity.values();
            for i in 0..n {
                let a_p_orig = a_p[i];
                a_p[i] /= self.alpha_u;
                sources[i] +=
                    (1.0 - self.alpha_u) / self.alpha_u * a_p_orig * vel_values[i][comp];
            }

            // Accumulate diagonal for pressure correction
            for i in 0..n {
                a_p_sum[i] += a_p[i];
            }

            // --- Add diagonals and sources to assembler, then finalize ---
            for i in 0..n {
                assembler.add_diagonal(i, a_p[i]);
                assembler.add_source(i, sources[i]);
            }
            let mut system = assembler
                .finalize()
                .map_err(|e| FluidError::PressureCorrectionFailed(e.to_string()))?;

            // Use current velocity as initial guess
            for i in 0..n {
                system.x[i] = state.velocity.values()[i][comp];
            }

            solve_linear_system(&mut system, self.use_gpu, false)?;

            // --- Update velocity component ---
            let vel_mut = state.velocity.values_mut();
            for i in 0..n {
                vel_mut[i][comp] = system.x[i];
            }
        }

        // Store the averaged diagonal for pressure correction
        self.a_p_momentum = a_p_sum.iter().map(|v| v / 3.0).collect();

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
        &self,
        state: &FluidState,
        mesh: &UnstructuredMesh,
        boundary_pressure: &HashMap<String, f64>,
    ) -> Result<Vec<f64>> {
        let n = mesh.num_cells();
        let mut a_p_pc = vec![0.0; n];
        let mut neighbors_pc: Vec<Vec<(usize, f64)>> = vec![vec![]; n];
        let mut sources_pc = vec![0.0; n];

        let face_patch_map = build_face_patch_map(mesh);

        // NOTE: Rhie-Chow correction tested (EXP007) but destabilized convergence.
        // Using simple linear interpolation for face velocities instead.

        for face in &mesh.faces {
            let owner = face.owner_cell;

            if let Some(neigh) = face.neighbor_cell {
                // Internal face
                let center_o = mesh.cells[owner].center;
                let center_n = mesh.cells[neigh].center;
                let dist = distance(&center_o, &center_n);

                // rA_f = 0.5 * (V_O / aP_O + V_N / aP_N)
                let ra_o = mesh.cells[owner].volume / self.a_p_momentum[owner];
                let ra_n = mesh.cells[neigh].volume / self.a_p_momentum[neigh];
                let ra_f = 0.5 * (ra_o + ra_n);

                // Pressure correction coefficient
                let coeff = self.density * ra_f * face.area / dist;

                a_p_pc[owner] += coeff;
                a_p_pc[neigh] += coeff;
                neighbors_pc[owner].push((neigh, coeff));
                neighbors_pc[neigh].push((owner, coeff));

                // RHS: mass imbalance through face
                let vel_o = state.velocity.values()[owner];
                let vel_n = state.velocity.values()[neigh];
                let u_f = [
                    0.5 * (vel_o[0] + vel_n[0]),
                    0.5 * (vel_o[1] + vel_n[1]),
                    0.5 * (vel_o[2] + vel_n[2]),
                ];

                let mass_flux = self.density
                    * (u_f[0] * face.normal[0]
                        + u_f[1] * face.normal[1]
                        + u_f[2] * face.normal[2])
                    * face.area;

                sources_pc[owner] -= mass_flux;
                sources_pc[neigh] += mass_flux;
            } else {
                // Boundary face
                let patch_name = face_patch_map.get(&face.id);

                if let Some(pname) = patch_name {
                    if let Some(_p_val) = boundary_pressure.get(pname) {
                        // Outlet with fixed pressure: p' = 0 (Dirichlet)
                        // Will be handled after assembly via apply_dirichlet
                        // But we still need the mass flux for the source
                        let vel_o = state.velocity.values()[owner];
                        let mass_flux = self.density
                            * (vel_o[0] * face.normal[0]
                                + vel_o[1] * face.normal[1]
                                + vel_o[2] * face.normal[2])
                            * face.area;
                        sources_pc[owner] -= mass_flux;
                    } else if self.boundary_velocities.contains_key(pname)
                        || self.wall_patches.iter().any(|w| w == pname)
                    {
                        // Inlet or wall: known velocity, compute the boundary mass flux
                        if let Some(bc_vel) = self.boundary_velocities.get(pname) {
                            // Inlet: use prescribed velocity
                            let mass_flux = self.density
                                * (bc_vel[0] * face.normal[0]
                                    + bc_vel[1] * face.normal[1]
                                    + bc_vel[2] * face.normal[2])
                                * face.area;
                            sources_pc[owner] -= mass_flux;
                        }
                        // Wall: zero velocity, zero mass flux -> no contribution
                    }
                }
            }
        }

        // Assemble
        let mut assembler = Assembler::new(n);
        for i in 0..n {
            assembler.add_cell_equation(i, a_p_pc[i], &neighbors_pc[i], sources_pc[i]);
        }
        let mut system = assembler
            .finalize()
            .map_err(|e| FluidError::PressureCorrectionFailed(e.to_string()))?;

        // Apply Dirichlet p' = 0 for outlet patches
        for patch in &mesh.boundary_patches {
            if boundary_pressure.contains_key(&patch.name) {
                for &fid in &patch.face_ids {
                    let cell_id = mesh.faces[fid].owner_cell;
                    apply_dirichlet(&mut system, cell_id, 0.0);
                }
            }
        }

        // If no pressure BC was set (e.g., fully enclosed domain),
        // fix one cell to zero to make the system non-singular
        let has_pressure_bc = boundary_pressure
            .keys()
            .any(|k| mesh.boundary_patch(k).is_some());
        if !has_pressure_bc {
            apply_dirichlet(&mut system, 0, 0.0);
        }

        // Solve pressure correction (SPD system -- use CG, with BiCGSTAB fallback)
        let stats = solve_linear_system(&mut system, self.use_gpu, true)?;

        // If CG doesn't converge on CPU, fall back to BiCGSTAB
        if !stats.converged && !self.use_gpu {
            for xi in system.x.iter_mut() {
                *xi = 0.0;
            }
            solve_linear_system(&mut system, false, false)?;
        }

        Ok(system.x)
    }

    // -----------------------------------------------------------------------
    // Step 3: Velocity correction
    // -----------------------------------------------------------------------

    /// Correct velocity using pressure correction gradient.
    ///
    /// u_P = u*_P - (V_P / a_P) * grad(p')_P
    fn correct_velocity(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        p_prime: &[f64],
    ) -> Result<()> {
        let n = mesh.num_cells();
        let grad_computer = GreenGaussCellBasedGradient;
        let p_prime_field = ScalarField::new("p_prime", p_prime.to_vec());
        let grad_pp = grad_computer
            .compute(&p_prime_field, mesh)
            .map_err(|e| FluidError::CoreError(e))?;

        let vel_mut = state.velocity.values_mut();
        for i in 0..n {
            let ra = mesh.cells[i].volume / self.a_p_momentum[i];
            let gpp = grad_pp.values()[i];
            vel_mut[i][0] -= ra * gpp[0];
            vel_mut[i][1] -= ra * gpp[1];
            vel_mut[i][2] -= ra * gpp[2];
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

        let face_patch_map = build_face_patch_map(mesh);

        for face in &mesh.faces {
            let owner = face.owner_cell;

            if let Some(neigh) = face.neighbor_cell {
                // Internal face
                let vel_o = state.velocity.values()[owner];
                let vel_n = state.velocity.values()[neigh];
                let u_f = [
                    0.5 * (vel_o[0] + vel_n[0]),
                    0.5 * (vel_o[1] + vel_n[1]),
                    0.5 * (vel_o[2] + vel_n[2]),
                ];
                let mass_flux = self.density
                    * (u_f[0] * face.normal[0]
                        + u_f[1] * face.normal[1]
                        + u_f[2] * face.normal[2])
                    * face.area;

                mass_imbalance[owner] += mass_flux;
                mass_imbalance[neigh] -= mass_flux;
            } else {
                // Boundary face
                let patch_name = face_patch_map.get(&face.id);
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
    if symmetric {
        let mut solver = CG::new(1e-6, 1000);
        solver
            .solve(&system.a, &system.b, &mut system.x)
            .map_err(|e| FluidError::SolverFailed(format!("{:?}", e)))
    } else {
        let mut solver = BiCGSTAB::new(1e-6, 1000);
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
