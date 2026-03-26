//! Turbulence transport equation solver.
//!
//! Provides a dedicated solver for the turbulence transport equations
//! (k, epsilon, omega) that can be called within the fluid solver loop.

use gfd_core::gradient::{GreenGaussCellBasedGradient, GradientComputer};
use gfd_core::{ScalarField, UnstructuredMesh};
use gfd_discretize::fvm::diffusion::compute_diffusive_coefficient;
use gfd_linalg::iterative::bicgstab::BiCGSTAB;
use gfd_linalg::traits::LinearSolverTrait;
use gfd_matrix::assembler::Assembler;

use crate::{FluidError, FluidState, Result};

/// Standard k-epsilon model constants.
const C_MU: f64 = 0.09;
const C1_EPS: f64 = 1.44;
const C2_EPS: f64 = 1.92;
const SIGMA_K: f64 = 1.0;
const SIGMA_EPS: f64 = 1.3;

/// Standard k-omega SST model constants.
const ALPHA_OMEGA: f64 = 5.0 / 9.0;
const BETA_OMEGA: f64 = 0.075;
const SIGMA_OMEGA: f64 = 2.0;

/// Minimum values for turbulence variables (to avoid division by zero).
const K_MIN: f64 = 1e-10;
const EPSILON_MIN: f64 = 1e-10;
const OMEGA_MIN: f64 = 1e-10;

/// Solver for turbulence transport equations.
///
/// Assembles and solves the discretized transport equations for turbulent
/// kinetic energy (k), dissipation rate (epsilon), and/or specific
/// dissipation rate (omega), depending on the active turbulence model.
pub struct TurbulenceTransportSolver {
    /// Under-relaxation factor for k equation.
    pub relax_k: f64,
    /// Under-relaxation factor for the second variable (epsilon or omega).
    pub relax_second: f64,
    /// Maximum sub-iterations for each transport equation.
    pub max_sub_iterations: usize,
    /// Convergence tolerance for transport equation residuals.
    pub tolerance: f64,
}

impl TurbulenceTransportSolver {
    /// Creates a new turbulence transport solver with default settings.
    pub fn new() -> Self {
        Self {
            relax_k: 0.7,
            relax_second: 0.7,
            max_sub_iterations: 20,
            tolerance: 1e-6,
        }
    }

    /// Creates a new solver with custom relaxation factors.
    pub fn with_relaxation(relax_k: f64, relax_second: f64) -> Self {
        Self {
            relax_k,
            relax_second,
            max_sub_iterations: 20,
            tolerance: 1e-6,
        }
    }

    /// Compute the strain rate magnitude S = sqrt(2 * S_ij * S_ij) using
    /// velocity gradients from the Green-Gauss method.
    ///
    /// Returns S^2 per cell (to avoid the sqrt when computing P_k = mu_t * S^2).
    fn compute_strain_rate_sq(
        &self,
        state: &FluidState,
        mesh: &UnstructuredMesh,
    ) -> Result<Vec<f64>> {
        let n = mesh.num_cells();
        let grad_computer = GreenGaussCellBasedGradient;

        // Compute gradient of each velocity component.
        let ux = ScalarField::new(
            "ux",
            state.velocity.values().iter().map(|v| v[0]).collect(),
        );
        let uy = ScalarField::new(
            "uy",
            state.velocity.values().iter().map(|v| v[1]).collect(),
        );
        let uz = ScalarField::new(
            "uz",
            state.velocity.values().iter().map(|v| v[2]).collect(),
        );

        let grad_ux = grad_computer
            .compute(&ux, mesh)
            .map_err(FluidError::CoreError)?;
        let grad_uy = grad_computer
            .compute(&uy, mesh)
            .map_err(FluidError::CoreError)?;
        let grad_uz = grad_computer
            .compute(&uz, mesh)
            .map_err(FluidError::CoreError)?;

        let mut s_sq = vec![0.0; n];
        for i in 0..n {
            let gux = grad_ux.values()[i]; // [dux/dx, dux/dy, dux/dz]
            let guy = grad_uy.values()[i]; // [duy/dx, duy/dy, duy/dz]
            let guz = grad_uz.values()[i]; // [duz/dx, duz/dy, duz/dz]

            // S_ij = 0.5 * (dU_i/dx_j + dU_j/dx_i)
            // 2 * S_ij * S_ij = sum_ij (dU_i/dx_j + dU_j/dx_i)^2 / 2
            // We compute S^2 = 2 * S_ij * S_ij
            let duidxj = [
                [gux[0], gux[1], gux[2]], // row 0 = dU_x/dx_j
                [guy[0], guy[1], guy[2]], // row 1 = dU_y/dx_j
                [guz[0], guz[1], guz[2]], // row 2 = dU_z/dx_j
            ];

            let mut sum = 0.0;
            for ii in 0..3 {
                for jj in 0..3 {
                    let s_ij = 0.5 * (duidxj[ii][jj] + duidxj[jj][ii]);
                    sum += s_ij * s_ij;
                }
            }
            s_sq[i] = 2.0 * sum; // S^2 = 2 * S_ij * S_ij
        }

        Ok(s_sq)
    }

    /// Build face-to-patch map for boundary faces.
    fn build_face_patch_map<'a>(&self, mesh: &'a UnstructuredMesh) -> std::collections::HashMap<usize, &'a str> {
        let mut map = std::collections::HashMap::new();
        for patch in &mesh.boundary_patches {
            for &fid in &patch.face_ids {
                map.insert(fid, patch.name.as_str());
            }
        }
        map
    }

    /// Generic scalar transport equation solver.
    ///
    /// Solves: d(rho*phi)/dt + div(rho*U*phi) = div(gamma_eff * grad(phi)) + S_u
    /// with implicit destruction: S_p * phi (added to diagonal).
    ///
    /// Returns the solved field values and the residual norm.
    fn solve_scalar_transport(
        &self,
        phi_old: &[f64],
        state: &FluidState,
        mesh: &UnstructuredMesh,
        gamma_eff: &[f64],  // effective diffusivity per cell
        source_explicit: &[f64], // S_u (explicit source)
        source_implicit: &[f64], // S_p (implicit: added to diagonal, positive = destruction)
        dt: f64,
        relax: f64,
    ) -> Result<(Vec<f64>, f64)> {
        let n = mesh.num_cells();
        let _face_patch_map = self.build_face_patch_map(mesh);

        let mut a_p = vec![0.0_f64; n];
        let mut neighbors_list: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        let mut sources = vec![0.0_f64; n];

        // Loop over faces: convection + diffusion.
        for face in &mesh.faces {
            let owner = face.owner_cell;

            if let Some(neighbor) = face.neighbor_cell {
                // Internal face.
                let c_o = &mesh.cells[owner].center;
                let c_n = &mesh.cells[neighbor].center;
                let dist = ((c_o[0] - c_n[0]).powi(2)
                    + (c_o[1] - c_n[1]).powi(2)
                    + (c_o[2] - c_n[2]).powi(2))
                .sqrt();

                // Effective diffusivity at face: average of owner and neighbor.
                let gamma_f = 0.5 * (gamma_eff[owner] + gamma_eff[neighbor]);
                let d_coeff = compute_diffusive_coefficient(gamma_f, face.area, dist);

                // Convective mass flux.
                let vel_o = state.velocity.values()[owner];
                let vel_n = state.velocity.values()[neighbor];
                let rho_o = state.density.values()[owner];
                let rho_n = state.density.values()[neighbor];
                let rho_f = 0.5 * (rho_o + rho_n);
                let u_f = [
                    0.5 * (vel_o[0] + vel_n[0]),
                    0.5 * (vel_o[1] + vel_n[1]),
                    0.5 * (vel_o[2] + vel_n[2]),
                ];
                let f_flux = rho_f
                    * (u_f[0] * face.normal[0]
                        + u_f[1] * face.normal[1]
                        + u_f[2] * face.normal[2])
                    * face.area;

                // First-order upwind.
                let f_pos = f64::max(f_flux, 0.0);
                let f_neg = f64::max(-f_flux, 0.0);

                a_p[owner] += d_coeff + f_pos;
                neighbors_list[owner].push((neighbor, d_coeff + f_neg));

                a_p[neighbor] += d_coeff + f_neg;
                neighbors_list[neighbor].push((owner, d_coeff + f_pos));
            } else {
                // Boundary face: zero-gradient (Neumann) for turbulence variables.
                // Wall treatment would go here but for now we use zero-gradient
                // which is appropriate for free-stream boundaries.
                // For wall-adjacent cells, the wall function approach sets
                // the boundary value implicitly through the source terms.
            }
        }

        // Temporal term (implicit Euler).
        for i in 0..n {
            let rho = state.density.values()[i];
            let temporal_coeff = rho * mesh.cells[i].volume / dt;
            a_p[i] += temporal_coeff;
            sources[i] += temporal_coeff * phi_old[i];
        }

        // Source terms.
        for i in 0..n {
            sources[i] += source_explicit[i] * mesh.cells[i].volume;
            // Implicit source (positive = destruction, added to diagonal).
            a_p[i] += source_implicit[i] * mesh.cells[i].volume;
        }

        // Under-relaxation.
        for i in 0..n {
            let a_p_orig = a_p[i];
            a_p[i] /= relax;
            sources[i] += (1.0 - relax) / relax * a_p_orig * phi_old[i];
        }

        // Assemble the linear system.
        let mut assembler = Assembler::new(n);
        for i in 0..n {
            assembler.add_cell_equation(i, a_p[i], &neighbors_list[i], sources[i]);
        }
        let mut system = assembler
            .finalize()
            .map_err(|e| FluidError::SolverFailed(e.to_string()))?;

        // Use old values as initial guess.
        for i in 0..n {
            system.x[i] = phi_old[i];
        }

        // Solve with BiCGSTAB.
        let mut solver = BiCGSTAB::new(self.tolerance, self.max_sub_iterations);
        let stats = solver
            .solve(&system.a, &system.b, &mut system.x)
            .map_err(|e| FluidError::SolverFailed(format!("{:?}", e)))?;

        Ok((system.x, stats.final_residual))
    }

    /// Solves the transport equation for turbulent kinetic energy k.
    ///
    /// d(rho*k)/dt + div(rho*U*k) = div((mu + mu_t/sigma_k) * grad(k)) + P_k - rho*epsilon
    ///
    /// where P_k is the production of k due to mean velocity gradients.
    pub fn solve_k_equation(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<f64> {
        let n = mesh.num_cells();

        // Get current k and epsilon fields (initialize if not present).
        if state.turb_kinetic_energy.is_none() {
            state.turb_kinetic_energy = Some(ScalarField::new("k", vec![K_MIN; n]));
        }
        if state.turb_dissipation.is_none() {
            state.turb_dissipation = Some(ScalarField::new("epsilon", vec![EPSILON_MIN; n]));
        }

        let k_old: Vec<f64> = state
            .turb_kinetic_energy
            .as_ref()
            .unwrap()
            .values()
            .to_vec();
        let epsilon_vals: Vec<f64> = state
            .turb_dissipation
            .as_ref()
            .unwrap()
            .values()
            .to_vec();

        // Compute eddy viscosity: mu_t = C_mu * rho * k^2 / epsilon.
        // Apply realizability limiter: mu_t < 1000 * mu_laminar (Fluent practice).
        let mut mu_t = vec![0.0; n];
        for i in 0..n {
            let rho = state.density.values()[i];
            let mu_lam = state.viscosity.values()[i];
            let k_val = k_old[i].max(K_MIN);
            let eps_val = epsilon_vals[i].max(EPSILON_MIN);
            mu_t[i] = (C_MU * rho * k_val * k_val / eps_val).min(1000.0 * mu_lam);
        }

        // Effective diffusivity: gamma_eff = mu + mu_t / sigma_k.
        let gamma_eff: Vec<f64> = (0..n)
            .map(|i| state.viscosity.values()[i] + mu_t[i] / SIGMA_K)
            .collect();

        // Compute production: P_k = mu_t * S^2.
        let s_sq = self.compute_strain_rate_sq(state, mesh)?;
        let mut source_explicit = vec![0.0; n];
        let mut source_implicit = vec![0.0; n];

        for i in 0..n {
            // Production: P_k = mu_t * S^2 with Kato-Launder-style limiter.
            // Clip P_k to max(P_k, 10 * rho * epsilon) to prevent unphysical
            // k spikes at stagnation points (standard Fluent/CFX practice).
            let rho = state.density.values()[i];
            let k_val = k_old[i].max(K_MIN);
            let eps_val = epsilon_vals[i].max(EPSILON_MIN);
            let p_k = mu_t[i] * s_sq[i];
            let p_k_limited = p_k.min(10.0 * rho * eps_val);
            source_explicit[i] = p_k_limited;

            // Destruction: -rho * epsilon.
            // Linearized implicitly: add rho * epsilon / k to the diagonal.
            source_implicit[i] = rho * eps_val / k_val;
        }

        // Solve the scalar transport equation for k.
        let (k_new, residual) = self.solve_scalar_transport(
            &k_old,
            state,
            mesh,
            &gamma_eff,
            &source_explicit,
            &source_implicit,
            dt,
            self.relax_k,
        )?;

        // Clamp k to minimum value and update state.
        let k_field = state.turb_kinetic_energy.as_mut().unwrap();
        for i in 0..n {
            let _ = k_field.set(i, k_new[i].max(K_MIN));
        }

        Ok(residual)
    }

    /// Solves the transport equation for turbulence dissipation rate epsilon.
    ///
    /// d(rho*eps)/dt + div(rho*U*eps) = div((mu + mu_t/sigma_eps)*grad(eps))
    ///                                  + C1*eps/k*P_k - C2*rho*eps^2/k
    pub fn solve_epsilon_equation(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<f64> {
        let n = mesh.num_cells();

        // Ensure fields exist.
        if state.turb_kinetic_energy.is_none() {
            state.turb_kinetic_energy = Some(ScalarField::new("k", vec![K_MIN; n]));
        }
        if state.turb_dissipation.is_none() {
            state.turb_dissipation = Some(ScalarField::new("epsilon", vec![EPSILON_MIN; n]));
        }

        let k_vals: Vec<f64> = state
            .turb_kinetic_energy
            .as_ref()
            .unwrap()
            .values()
            .to_vec();
        let eps_old: Vec<f64> = state
            .turb_dissipation
            .as_ref()
            .unwrap()
            .values()
            .to_vec();

        // Compute eddy viscosity with realizability limiter.
        let mut mu_t = vec![0.0; n];
        for i in 0..n {
            let rho = state.density.values()[i];
            let mu_lam = state.viscosity.values()[i];
            let k_val = k_vals[i].max(K_MIN);
            let eps_val = eps_old[i].max(EPSILON_MIN);
            mu_t[i] = (C_MU * rho * k_val * k_val / eps_val).min(1000.0 * mu_lam);
        }

        // Effective diffusivity: gamma_eff = mu + mu_t / sigma_eps.
        let gamma_eff: Vec<f64> = (0..n)
            .map(|i| state.viscosity.values()[i] + mu_t[i] / SIGMA_EPS)
            .collect();

        // Compute production P_k = mu_t * S^2.
        let s_sq = self.compute_strain_rate_sq(state, mesh)?;

        let mut source_explicit = vec![0.0; n];
        let mut source_implicit = vec![0.0; n];

        for i in 0..n {
            let rho = state.density.values()[i];
            let k_val = k_vals[i].max(K_MIN);
            let eps_val = eps_old[i].max(EPSILON_MIN);
            let p_k = mu_t[i] * s_sq[i];

            // Production: C1 * eps/k * P_k (explicit).
            source_explicit[i] = C1_EPS * eps_val / k_val * p_k;

            // Destruction: C2 * rho * eps^2 / k.
            // Linearized implicitly: C2 * rho * eps / k added to diagonal.
            source_implicit[i] = C2_EPS * rho * eps_val / k_val;
        }

        // Solve the scalar transport equation for epsilon.
        let (eps_new, residual) = self.solve_scalar_transport(
            &eps_old,
            state,
            mesh,
            &gamma_eff,
            &source_explicit,
            &source_implicit,
            dt,
            self.relax_second,
        )?;

        // Clamp epsilon to minimum value and update state.
        let eps_field = state.turb_dissipation.as_mut().unwrap();
        for i in 0..n {
            let _ = eps_field.set(i, eps_new[i].max(EPSILON_MIN));
        }

        Ok(residual)
    }

    /// Solves the transport equation for specific dissipation rate omega.
    ///
    /// d(rho*omega)/dt + div(rho*U*omega) = div((mu + mu_t/sigma_omega)*grad(omega))
    ///                                      + alpha_omega * omega/k * P_k - beta_omega * rho * omega^2
    pub fn solve_omega_equation(
        &self,
        state: &mut FluidState,
        mesh: &UnstructuredMesh,
        dt: f64,
    ) -> Result<f64> {
        let n = mesh.num_cells();

        // Ensure fields exist.
        if state.turb_kinetic_energy.is_none() {
            state.turb_kinetic_energy = Some(ScalarField::new("k", vec![K_MIN; n]));
        }
        if state.turb_specific_dissipation.is_none() {
            state.turb_specific_dissipation = Some(ScalarField::new("omega", vec![OMEGA_MIN; n]));
        }

        let k_vals: Vec<f64> = state
            .turb_kinetic_energy
            .as_ref()
            .unwrap()
            .values()
            .to_vec();
        let omega_old: Vec<f64> = state
            .turb_specific_dissipation
            .as_ref()
            .unwrap()
            .values()
            .to_vec();

        // Eddy viscosity: mu_t = rho * k / omega.
        let mut mu_t = vec![0.0; n];
        for i in 0..n {
            let rho = state.density.values()[i];
            let k_val = k_vals[i].max(K_MIN);
            let omega_val = omega_old[i].max(OMEGA_MIN);
            mu_t[i] = rho * k_val / omega_val;
        }

        // Effective diffusivity: gamma_eff = mu + mu_t / sigma_omega.
        let gamma_eff: Vec<f64> = (0..n)
            .map(|i| state.viscosity.values()[i] + mu_t[i] / SIGMA_OMEGA)
            .collect();

        // Compute production P_k = mu_t * S^2.
        let s_sq = self.compute_strain_rate_sq(state, mesh)?;

        let mut source_explicit = vec![0.0; n];
        let mut source_implicit = vec![0.0; n];

        for i in 0..n {
            let rho = state.density.values()[i];
            let k_val = k_vals[i].max(K_MIN);
            let omega_val = omega_old[i].max(OMEGA_MIN);
            let p_k = mu_t[i] * s_sq[i];

            // Production: alpha_omega * omega/k * P_k.
            source_explicit[i] = ALPHA_OMEGA * omega_val / k_val * p_k;

            // Destruction: beta_omega * rho * omega^2.
            // Linearized: beta_omega * rho * omega added to diagonal.
            source_implicit[i] = BETA_OMEGA * rho * omega_val;
        }

        // Solve the scalar transport equation for omega.
        let (omega_new, residual) = self.solve_scalar_transport(
            &omega_old,
            state,
            mesh,
            &gamma_eff,
            &source_explicit,
            &source_implicit,
            dt,
            self.relax_second,
        )?;

        // Clamp omega and update state.
        let omega_field = state.turb_specific_dissipation.as_mut().unwrap();
        for i in 0..n {
            let _ = omega_field.set(i, omega_new[i].max(OMEGA_MIN));
        }

        Ok(residual)
    }

    /// Computes the eddy viscosity from the current turbulence variables.
    ///
    /// For k-epsilon: mu_t = C_mu * rho * k^2 / epsilon
    /// For k-omega:   mu_t = rho * k / omega (with SST limiter)
    pub fn compute_eddy_viscosity(
        &self,
        state: &FluidState,
        _mesh: &UnstructuredMesh,
    ) -> Result<ScalarField> {
        let n = state.num_cells();

        let mut mu_t_vals = vec![0.0; n];

        if let (Some(k_field), Some(eps_field)) =
            (&state.turb_kinetic_energy, &state.turb_dissipation)
        {
            // k-epsilon model: mu_t = C_mu * rho * k^2 / epsilon.
            for i in 0..n {
                let rho = state.density.values()[i];
                let k_val = k_field.values()[i].max(K_MIN);
                let eps_val = eps_field.values()[i].max(EPSILON_MIN);
                mu_t_vals[i] = C_MU * rho * k_val * k_val / eps_val;
            }
        } else if let (Some(k_field), Some(omega_field)) =
            (&state.turb_kinetic_energy, &state.turb_specific_dissipation)
        {
            // k-omega model: mu_t = rho * k / omega.
            for i in 0..n {
                let rho = state.density.values()[i];
                let k_val = k_field.values()[i].max(K_MIN);
                let omega_val = omega_field.values()[i].max(OMEGA_MIN);
                mu_t_vals[i] = rho * k_val / omega_val;
            }
        }

        Ok(ScalarField::new("eddy_viscosity", mu_t_vals))
    }
}

impl Default for TurbulenceTransportSolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::cell::Cell;
    use gfd_core::mesh::face::Face;
    use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};
    use gfd_core::VectorField;

    /// Creates a simple 3-cell 1D mesh for testing.
    fn make_test_mesh() -> UnstructuredMesh {
        let dx = 1.0;
        let cells = vec![
            Cell::new(0, vec![], vec![], dx, [0.5, 0.5, 0.5]),
            Cell::new(1, vec![], vec![], dx, [1.5, 0.5, 0.5]),
            Cell::new(2, vec![], vec![], dx, [2.5, 0.5, 0.5]),
        ];

        let faces = vec![
            // Left boundary
            Face::new(0, vec![], 0, None, 1.0, [-1.0, 0.0, 0.0], [0.0, 0.5, 0.5]),
            // Internal 0-1
            Face::new(1, vec![], 0, Some(1), 1.0, [1.0, 0.0, 0.0], [1.0, 0.5, 0.5]),
            // Internal 1-2
            Face::new(2, vec![], 1, Some(2), 1.0, [1.0, 0.0, 0.0], [2.0, 0.5, 0.5]),
            // Right boundary
            Face::new(3, vec![], 2, None, 1.0, [1.0, 0.0, 0.0], [3.0, 0.5, 0.5]),
        ];

        let patches = vec![
            BoundaryPatch::new("inlet", vec![0]),
            BoundaryPatch::new("outlet", vec![3]),
        ];

        UnstructuredMesh::from_components(vec![], faces, cells, patches)
    }

    #[test]
    fn solve_k_equation_does_not_panic() {
        let mesh = make_test_mesh();
        let n = mesh.num_cells();
        let mut state = FluidState::new(n);

        // Set a small uniform velocity.
        state.velocity = VectorField::new("velocity", vec![[1.0, 0.0, 0.0]; n]);

        // Initialize turbulence fields.
        state.turb_kinetic_energy = Some(ScalarField::new("k", vec![1.0; n]));
        state.turb_dissipation = Some(ScalarField::new("epsilon", vec![1.0; n]));

        let solver = TurbulenceTransportSolver::new();
        let residual = solver.solve_k_equation(&mut state, &mesh, 0.01);
        assert!(residual.is_ok());
    }

    #[test]
    fn solve_epsilon_equation_does_not_panic() {
        let mesh = make_test_mesh();
        let n = mesh.num_cells();
        let mut state = FluidState::new(n);

        state.velocity = VectorField::new("velocity", vec![[1.0, 0.0, 0.0]; n]);
        state.turb_kinetic_energy = Some(ScalarField::new("k", vec![1.0; n]));
        state.turb_dissipation = Some(ScalarField::new("epsilon", vec![1.0; n]));

        let solver = TurbulenceTransportSolver::new();
        let residual = solver.solve_epsilon_equation(&mut state, &mesh, 0.01);
        assert!(residual.is_ok());
    }

    #[test]
    fn solve_omega_equation_does_not_panic() {
        let mesh = make_test_mesh();
        let n = mesh.num_cells();
        let mut state = FluidState::new(n);

        state.velocity = VectorField::new("velocity", vec![[1.0, 0.0, 0.0]; n]);
        state.turb_kinetic_energy = Some(ScalarField::new("k", vec![1.0; n]));
        state.turb_specific_dissipation = Some(ScalarField::new("omega", vec![1.0; n]));

        let solver = TurbulenceTransportSolver::new();
        let residual = solver.solve_omega_equation(&mut state, &mesh, 0.01);
        assert!(residual.is_ok());
    }

    #[test]
    fn compute_eddy_viscosity_k_epsilon() {
        let mesh = make_test_mesh();
        let n = mesh.num_cells();
        let mut state = FluidState::new(n);

        // k = 1.0, epsilon = 1.0, rho = 1.0
        // mu_t = C_mu * rho * k^2 / epsilon = 0.09
        state.turb_kinetic_energy = Some(ScalarField::new("k", vec![1.0; n]));
        state.turb_dissipation = Some(ScalarField::new("epsilon", vec![1.0; n]));

        let solver = TurbulenceTransportSolver::new();
        let mu_t = solver.compute_eddy_viscosity(&state, &mesh).unwrap();
        for v in mu_t.values() {
            assert!((v - C_MU).abs() < 1e-10);
        }
    }
}
