//! PISO (Pressure Implicit with Splitting of Operators) algorithm.

use std::collections::HashMap;

use gfd_core::gradient::{GreenGaussCellBasedGradient, GradientComputer};
use gfd_core::{ScalarField, UnstructuredMesh};
use gfd_discretize::fvm::diffusion::compute_diffusive_coefficient;
use gfd_linalg::iterative::bicgstab::BiCGSTAB;
use gfd_linalg::iterative::cg::CG;
use gfd_linalg::traits::LinearSolverTrait;
use gfd_matrix::assembler::Assembler;
use gfd_matrix::boundary::apply_dirichlet;

use crate::incompressible::PressureVelocityCoupling;
use crate::{FluidError, FluidState, Result};

/// PISO pressure-velocity coupling solver.
///
/// Similar to SIMPLE but with two (or more) corrector steps,
/// allowing larger time steps for transient simulations.
///
/// Algorithm outline:
/// 1. Solve momentum equation (predictor) for u*
/// 2. First pressure correction: solve for p', correct u and p
/// 3. Second pressure correction: solve for p'', correct u and p again
/// 4. (Optional) Additional corrector steps
pub struct PisoSolver {
    /// Number of correction steps (typically 2).
    pub num_correctors: usize,
    /// Under-relaxation factor for pressure (usually 1.0 for PISO).
    pub under_relaxation_pressure: f64,
    /// Uniform density [kg/m^3].
    pub density: f64,
    /// Uniform dynamic viscosity [Pa*s].
    pub viscosity: f64,
    /// Stored diagonal coefficients from the momentum equation.
    a_p_momentum: Vec<f64>,
    /// Boundary velocities: patch name -> [u, v, w].
    boundary_velocities: HashMap<String, [f64; 3]>,
    /// Boundary pressures: patch name -> pressure value.
    boundary_pressure: HashMap<String, f64>,
    /// Wall patch names.
    wall_patches: Vec<String>,
}

impl PisoSolver {
    /// Creates a new PISO solver.
    pub fn new(num_correctors: usize) -> Self {
        Self {
            num_correctors,
            under_relaxation_pressure: 1.0,
            density: 1.0,
            viscosity: 1e-3,
            a_p_momentum: Vec::new(),
            boundary_velocities: HashMap::new(),
            boundary_pressure: HashMap::new(),
            wall_patches: Vec::new(),
        }
    }

    /// Creates a PISO solver with specified fluid properties.
    pub fn with_properties(num_correctors: usize, density: f64, viscosity: f64) -> Self {
        Self {
            num_correctors,
            under_relaxation_pressure: 1.0,
            density,
            viscosity,
            a_p_momentum: Vec::new(),
            boundary_velocities: HashMap::new(),
            boundary_pressure: HashMap::new(),
            wall_patches: Vec::new(),
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

    /// Predictor step: solve momentum equation for u* with NO under-relaxation.
    ///
    /// This is the key difference from SIMPLE: alpha_u = 1.0.
    /// When `dt` is finite, adds implicit Euler transient term to the momentum equation.
    fn predict_velocity(
        &mut self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<()> {
        let n = mesh.num_cells();
        let face_patch_map = build_face_patch_map(mesh);

        // Compute pressure gradient.
        let grad_computer = GreenGaussCellBasedGradient;
        let grad_p = grad_computer
            .compute(&state.pressure, mesh)
            .map_err(FluidError::CoreError)?;

        // Pre-compute face data.
        struct InternalFace { owner: usize, neigh: usize, d: f64, f_flux: f64 }
        struct BoundaryFace {
            owner: usize,
            d: f64,
            bc_vel: Option<[f64; 3]>,
            f_flux_bc: f64,
            is_wall: bool,
        }

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
                let dist_to_face = distance(&mesh.cells[owner].center, &face.center).max(1e-30);
                let d = compute_diffusive_coefficient(self.viscosity, face.area, dist_to_face);
                let patch = face_patch_map.get(&face.id).map(|s| s.to_string());
                let (bc_vel, f_flux_bc, is_wall) = if let Some(ref pname) = patch {
                    if let Some(bv) = self.boundary_velocities.get(pname.as_str()) {
                        let ff = self.density
                            * (bv[0] * face.normal[0] + bv[1] * face.normal[1] + bv[2] * face.normal[2])
                            * face.area;
                        (Some(*bv), ff, false)
                    } else if self.wall_patches.iter().any(|w| w == pname) {
                        (None, 0.0, true)
                    } else {
                        (None, 0.0, false)
                    }
                } else {
                    (None, 0.0, false)
                };
                boundary_faces.push(BoundaryFace { owner, d, bc_vel, f_flux_bc, is_wall });
            }
        }

        let mut a_p_sum = vec![0.0; n];

        // Solve each velocity component (NO under-relaxation for PISO).
        for comp in 0..3 {
            let mut a_p = vec![0.0; n];
            let mut sources_vec = vec![0.0; n];
            let mut assembler = Assembler::new(n);

            // Internal faces.
            for iface in &internal_faces {
                let f_pos = f64::max(iface.f_flux, 0.0);
                let f_neg = f64::max(-iface.f_flux, 0.0);
                a_p[iface.owner] += iface.d + f_pos;
                a_p[iface.neigh] += iface.d + f_neg;
                assembler.add_neighbor(iface.owner, iface.neigh, iface.d + f_neg);
                assembler.add_neighbor(iface.neigh, iface.owner, iface.d + f_pos);
            }

            // Boundary faces.
            for bface in &boundary_faces {
                if let Some(ref bv) = bface.bc_vel {
                    a_p[bface.owner] += bface.d + f64::max(bface.f_flux_bc, 0.0);
                    sources_vec[bface.owner] += (bface.d + f64::max(-bface.f_flux_bc, 0.0)) * bv[comp];
                } else if bface.is_wall {
                    a_p[bface.owner] += bface.d;
                }
            }

            // Pressure gradient source.
            for i in 0..n {
                let gp = grad_p.values()[i];
                sources_vec[i] -= gp[comp] * mesh.cells[i].volume;
            }

            // Transient term (implicit Euler): a_P += rho*V/dt, source += (rho*V/dt)*u_old
            if dt.is_finite() {
                let vel_values = state.velocity.values();
                for i in 0..n {
                    let temporal_coeff = self.density * mesh.cells[i].volume / dt;
                    a_p[i] += temporal_coeff;
                    sources_vec[i] += temporal_coeff * vel_values[i][comp];
                }
            }

            // NO under-relaxation for PISO (alpha_u = 1.0).
            // Accumulate diagonal.
            for i in 0..n {
                a_p_sum[i] += a_p[i];
            }

            // Assemble and solve.
            for i in 0..n {
                assembler.add_diagonal(i, a_p[i]);
                assembler.add_source(i, sources_vec[i]);
            }
            let mut system = assembler
                .finalize()
                .map_err(|e| FluidError::PressureCorrectionFailed(e.to_string()))?;

            for i in 0..n {
                system.x[i] = state.velocity.values()[i][comp];
            }

            let mut solver = BiCGSTAB::new(1e-6, 1000);
            solver
                .solve(&system.a, &system.b, &mut system.x)
                .map_err(|e| FluidError::SolverFailed(format!("{:?}", e)))?;

            let vel_mut = state.velocity.values_mut();
            for i in 0..n {
                vel_mut[i][comp] = system.x[i];
            }
        }

        self.a_p_momentum = a_p_sum.iter().map(|v| v / 3.0).collect();
        Ok(())
    }

    /// Corrector step: solve pressure correction and update velocity/pressure.
    fn corrector_step(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        _step: usize,
    ) -> Result<()> {
        let n = mesh.num_cells();
        let face_patch_map = build_face_patch_map(mesh);

        // 1. Solve pressure Poisson equation for p'.
        let mut a_p_pc = vec![0.0; n];
        let mut neighbors_pc: Vec<Vec<(usize, f64)>> = vec![vec![]; n];
        let mut sources_pc = vec![0.0; n];

        for face in &mesh.faces {
            let owner = face.owner_cell;

            if let Some(neigh) = face.neighbor_cell {
                let center_o = mesh.cells[owner].center;
                let center_n = mesh.cells[neigh].center;
                let dist = distance(&center_o, &center_n);

                let ra_o = mesh.cells[owner].volume / self.a_p_momentum[owner];
                let ra_n = mesh.cells[neigh].volume / self.a_p_momentum[neigh];
                let ra_f = 0.5 * (ra_o + ra_n);

                let coeff = self.density * ra_f * face.area / dist;

                a_p_pc[owner] += coeff;
                a_p_pc[neigh] += coeff;
                neighbors_pc[owner].push((neigh, coeff));
                neighbors_pc[neigh].push((owner, coeff));

                // Mass imbalance.
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
                let patch_name = face_patch_map.get(&face.id);
                if let Some(pname) = patch_name {
                    if self.boundary_pressure.contains_key(pname) {
                        let vel_o = state.velocity.values()[owner];
                        let mass_flux = self.density
                            * (vel_o[0] * face.normal[0]
                                + vel_o[1] * face.normal[1]
                                + vel_o[2] * face.normal[2])
                            * face.area;
                        sources_pc[owner] -= mass_flux;
                    } else if let Some(bc_vel) = self.boundary_velocities.get(pname) {
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

        // Assemble pressure correction system.
        let mut assembler = Assembler::new(n);
        for i in 0..n {
            assembler.add_cell_equation(i, a_p_pc[i], &neighbors_pc[i], sources_pc[i]);
        }
        let mut system = assembler
            .finalize()
            .map_err(|e| FluidError::PressureCorrectionFailed(e.to_string()))?;

        // Apply Dirichlet p' = 0 for outlet patches.
        for patch in &mesh.boundary_patches {
            if self.boundary_pressure.contains_key(&patch.name) {
                for &fid in &patch.face_ids {
                    let cell_id = mesh.faces[fid].owner_cell;
                    apply_dirichlet(&mut system, cell_id, 0.0);
                }
            }
        }

        // Fix one cell if no pressure BC.
        let has_pressure_bc = self
            .boundary_pressure
            .keys()
            .any(|k| mesh.boundary_patch(k).is_some());
        if !has_pressure_bc {
            apply_dirichlet(&mut system, 0, 0.0);
        }

        // Solve pressure correction (SPD -- try CG first).
        let mut cg_solver = CG::new(1e-6, 1000);
        let stats = cg_solver
            .solve(&system.a, &system.b, &mut system.x)
            .map_err(|e| FluidError::SolverFailed(format!("{:?}", e)))?;

        if !stats.converged {
            for xi in system.x.iter_mut() {
                *xi = 0.0;
            }
            let mut bicg = BiCGSTAB::new(1e-6, 1000);
            bicg.solve(&system.a, &system.b, &mut system.x)
                .map_err(|e| FluidError::SolverFailed(format!("{:?}", e)))?;
        }

        let p_prime = system.x;

        // 2. Correct velocity: u_P = u*_P - (V_P / a_P) * grad(p')_P.
        let grad_computer = GreenGaussCellBasedGradient;
        let p_prime_field = ScalarField::new("p_prime", p_prime.clone());
        let grad_pp = grad_computer
            .compute(&p_prime_field, mesh)
            .map_err(FluidError::CoreError)?;

        let vel_mut = state.velocity.values_mut();
        for i in 0..n {
            let ra = mesh.cells[i].volume / self.a_p_momentum[i];
            let gpp = grad_pp.values()[i];
            vel_mut[i][0] -= ra * gpp[0];
            vel_mut[i][1] -= ra * gpp[1];
            vel_mut[i][2] -= ra * gpp[2];
        }

        // 3. Correct pressure: p = p + p' (no under-relaxation for PISO).
        let p_mut = state.pressure.values_mut();
        for i in 0..n {
            p_mut[i] += self.under_relaxation_pressure * p_prime[i];
        }

        Ok(())
    }

    /// Compute the continuity residual: max mass imbalance over cells.
    fn compute_continuity_residual(
        &self,
        state: &FluidState,
        mesh: &UnstructuredMesh,
    ) -> f64 {
        let n = mesh.num_cells();
        let mut mass_imbalance = vec![0.0; n];
        let face_patch_map = build_face_patch_map(mesh);

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
                let mass_flux = self.density
                    * (u_f[0] * face.normal[0]
                        + u_f[1] * face.normal[1]
                        + u_f[2] * face.normal[2])
                    * face.area;
                mass_imbalance[owner] += mass_flux;
                mass_imbalance[neigh] -= mass_flux;
            } else {
                if let Some(pname) = face_patch_map.get(&face.id) {
                    if let Some(bc_vel) = self.boundary_velocities.get(pname) {
                        let mass_flux = self.density
                            * (bc_vel[0] * face.normal[0]
                                + bc_vel[1] * face.normal[1]
                                + bc_vel[2] * face.normal[2])
                            * face.area;
                        mass_imbalance[owner] += mass_flux;
                    } else if !self.wall_patches.iter().any(|w| w == pname) {
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

impl PressureVelocityCoupling for PisoSolver {
    fn solve_step(
        &mut self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<f64> {
        // 1. Momentum predictor (no under-relaxation).
        self.predict_velocity(state, mesh, dt)?;

        // 2. Corrector loop (typically 2 steps).
        for step in 0..self.num_correctors {
            self.corrector_step(state, mesh, step)?;
        }

        // 3. Compute and return continuity residual.
        let residual = self.compute_continuity_residual(state, mesh);
        Ok(residual)
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

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::cell::Cell;
    use gfd_core::mesh::face::Face;
    use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};
    use gfd_core::VectorField;

    fn make_test_mesh() -> UnstructuredMesh {
        let dx = 1.0;
        let cells = vec![
            Cell::new(0, vec![], vec![], dx, [0.5, 0.5, 0.5]),
            Cell::new(1, vec![], vec![], dx, [1.5, 0.5, 0.5]),
            Cell::new(2, vec![], vec![], dx, [2.5, 0.5, 0.5]),
        ];

        let faces = vec![
            Face::new(0, vec![], 0, None, 1.0, [-1.0, 0.0, 0.0], [0.0, 0.5, 0.5]),
            Face::new(1, vec![], 0, Some(1), 1.0, [1.0, 0.0, 0.0], [1.0, 0.5, 0.5]),
            Face::new(2, vec![], 1, Some(2), 1.0, [1.0, 0.0, 0.0], [2.0, 0.5, 0.5]),
            Face::new(3, vec![], 2, None, 1.0, [1.0, 0.0, 0.0], [3.0, 0.5, 0.5]),
        ];

        let patches = vec![
            BoundaryPatch::new("inlet", vec![0]),
            BoundaryPatch::new("outlet", vec![3]),
        ];

        UnstructuredMesh::from_components(vec![], faces, cells, patches)
    }

    #[test]
    fn piso_solve_step_does_not_panic() {
        let mesh = make_test_mesh();
        let n = mesh.num_cells();
        let mut state = FluidState::new(n);
        state.velocity = VectorField::new("velocity", vec![[1.0, 0.0, 0.0]; n]);

        let mut solver = PisoSolver::with_properties(2, 1.0, 1e-3);
        solver.set_boundary_conditions(
            HashMap::from([("inlet".to_string(), [1.0, 0.0, 0.0])]),
            HashMap::from([("outlet".to_string(), 0.0)]),
            vec![],
        );

        let result = solver.solve_step(&mut state, &mesh, 0.01);
        assert!(result.is_ok());
    }

    #[test]
    fn piso_transient_term_affects_solution() {
        // Test that PISO with a finite dt produces different results than with infinite dt.
        let mesh = make_test_mesh();
        let n = mesh.num_cells();

        // Solve with finite dt (transient)
        let mut state_trans = FluidState::new(n);
        state_trans.velocity = VectorField::new("velocity", vec![[1.0, 0.0, 0.0]; n]);
        let mut solver_trans = PisoSolver::with_properties(2, 1.0, 1e-3);
        solver_trans.set_boundary_conditions(
            HashMap::from([("inlet".to_string(), [1.0, 0.0, 0.0])]),
            HashMap::from([("outlet".to_string(), 0.0)]),
            vec![],
        );
        let res_trans = solver_trans.solve_step(&mut state_trans, &mesh, 0.001);
        assert!(res_trans.is_ok(), "Transient PISO failed: {:?}", res_trans.err());

        // Solve with large dt (quasi-steady: temporal term negligible)
        let mut state_large = FluidState::new(n);
        state_large.velocity = VectorField::new("velocity", vec![[1.0, 0.0, 0.0]; n]);
        let mut solver_large = PisoSolver::with_properties(2, 1.0, 1e-3);
        solver_large.set_boundary_conditions(
            HashMap::from([("inlet".to_string(), [1.0, 0.0, 0.0])]),
            HashMap::from([("outlet".to_string(), 0.0)]),
            vec![],
        );
        let res_large = solver_large.solve_step(&mut state_large, &mesh, 1e6);
        assert!(res_large.is_ok(), "Large-dt PISO failed: {:?}", res_large.err());

        // Solutions should differ due to the transient term
        let vel_t = state_trans.velocity.values();
        let vel_l = state_large.velocity.values();
        let mut max_diff = 0.0_f64;
        for i in 0..n {
            for c in 0..3 {
                max_diff = max_diff.max((vel_t[i][c] - vel_l[i][c]).abs());
            }
        }
        assert!(
            max_diff > 1e-10,
            "Small dt and large dt PISO solutions should differ, max_diff={}",
            max_diff
        );
    }

    #[test]
    fn piso_transient_preserves_initial_velocity() {
        // With a very small dt, the transient term dominates and the velocity
        // should stay closer to the initial condition than with large dt.
        let mesh = make_test_mesh();
        let n = mesh.num_cells();
        let u_init = [2.0, 0.5, 0.0];

        // Solve with very small dt (strong temporal term)
        let mut state_small = FluidState::new(n);
        state_small.velocity = VectorField::new("velocity", vec![u_init; n]);
        let mut solver_small = PisoSolver::with_properties(2, 1.0, 1e-3);
        solver_small.set_boundary_conditions(
            HashMap::from([("inlet".to_string(), [1.0, 0.0, 0.0])]),
            HashMap::from([("outlet".to_string(), 0.0)]),
            vec![],
        );
        let result_small = solver_small.solve_step(&mut state_small, &mesh, 1e-8);
        assert!(result_small.is_ok());

        // Solve with large dt (weak temporal term)
        let mut state_large = FluidState::new(n);
        state_large.velocity = VectorField::new("velocity", vec![u_init; n]);
        let mut solver_large = PisoSolver::with_properties(2, 1.0, 1e-3);
        solver_large.set_boundary_conditions(
            HashMap::from([("inlet".to_string(), [1.0, 0.0, 0.0])]),
            HashMap::from([("outlet".to_string(), 0.0)]),
            vec![],
        );
        let result_large = solver_large.solve_step(&mut state_large, &mesh, 1e6);
        assert!(result_large.is_ok());

        // The small-dt solution should stay closer to the initial velocity
        // than the large-dt solution (transient term anchors to u_old).
        let vel_s = state_small.velocity.values();
        let vel_l = state_large.velocity.values();
        let mut drift_small = 0.0_f64;
        let mut drift_large = 0.0_f64;
        for i in 0..n {
            drift_small += (vel_s[i][0] - u_init[0]).abs();
            drift_large += (vel_l[i][0] - u_init[0]).abs();
        }
        assert!(
            drift_small < drift_large,
            "Small dt should drift less from initial velocity: small_drift={}, large_drift={}",
            drift_small,
            drift_large,
        );
    }
}
