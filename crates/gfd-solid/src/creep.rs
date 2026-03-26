//! Creep (time-dependent deformation) models.
//!
//! Implements implicit creep integration coupled with elastic stress computation.
//! The total strain is decomposed: eps_total = eps_elastic + eps_creep.
//! Stress is computed from elastic strain: sigma = C : eps_elastic.

use gfd_core::UnstructuredMesh;
use crate::{SolidState, Result};

/// Creep solver for time-dependent inelastic deformation.
///
/// Supports Norton (power law) creep with implicit time integration.
/// The creep strain evolves according to:
///   d(eps_cr)/dt = A * sigma_eq^n * exp(-Q/(R*T))
///
/// At each time step, the solver:
/// 1. Computes stress from elastic strain: sigma = C : (eps_total - eps_creep)
/// 2. Evaluates the Norton creep rate from the current stress
/// 3. Updates creep strain using implicit integration
/// 4. Recomputes elastic strain and stress for consistency
pub struct CreepSolver {
    /// Norton creep coefficient A.
    pub norton_a: f64,
    /// Norton stress exponent n.
    pub norton_n: f64,
    /// Activation energy / (gas constant) [K].
    pub q_over_r: f64,
    /// Young's modulus [Pa] (for stress computation from elastic strain).
    pub youngs_modulus: f64,
    /// Poisson's ratio [-].
    pub poissons_ratio: f64,
    /// Accumulated creep strain (deviatoric, stored as Voigt 6-component per cell).
    creep_strain: Vec<[[f64; 3]; 3]>,
    /// Current simulation time [s].
    current_time: f64,
    /// Maximum Newton iterations for implicit integration.
    pub max_newton_iter: usize,
    /// Newton iteration tolerance.
    pub newton_tol: f64,
}

impl CreepSolver {
    /// Creates a new Norton creep model with elastic material properties.
    ///
    /// Creep strain rate: d(epsilon_cr)/dt = A * sigma^n * exp(-Q/(R*T))
    pub fn norton(a: f64, n: f64, q_over_r: f64) -> Self {
        Self {
            norton_a: a,
            norton_n: n,
            q_over_r,
            youngs_modulus: 200e9,
            poissons_ratio: 0.3,
            creep_strain: Vec::new(),
            current_time: 0.0,
            max_newton_iter: 20,
            newton_tol: 1e-10,
        }
    }

    /// Sets the elastic material properties for stress computation.
    pub fn with_elastic_properties(mut self, youngs_modulus: f64, poissons_ratio: f64) -> Self {
        self.youngs_modulus = youngs_modulus;
        self.poissons_ratio = poissons_ratio;
        self
    }

    /// Returns the current simulation time.
    pub fn current_time(&self) -> f64 {
        self.current_time
    }

    /// Returns a reference to the accumulated creep strain field.
    pub fn creep_strain(&self) -> &[[[f64; 3]; 3]] {
        &self.creep_strain
    }

    /// Computes the Lame parameters.
    fn lame_parameters(&self) -> (f64, f64) {
        let e = self.youngs_modulus;
        let nu = self.poissons_ratio;
        let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let mu = e / (2.0 * (1.0 + nu));
        (lambda, mu)
    }

    /// Computes stress from elastic strain using Hooke's law.
    /// sigma_ij = lambda * tr(eps_e) * delta_ij + 2 * mu * eps_e_ij
    fn compute_stress(lambda: f64, mu: f64, elastic_strain: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
        let trace = elastic_strain[0][0] + elastic_strain[1][1] + elastic_strain[2][2];
        let mut stress = [[0.0_f64; 3]; 3];
        for a in 0..3 {
            for b in 0..3 {
                stress[a][b] = 2.0 * mu * elastic_strain[a][b];
                if a == b {
                    stress[a][b] += lambda * trace;
                }
            }
        }
        stress
    }

    /// Computes the Von Mises equivalent stress.
    fn von_mises(stress: &[[f64; 3]; 3]) -> f64 {
        let hydrostatic = (stress[0][0] + stress[1][1] + stress[2][2]) / 3.0;
        let mut j2 = 0.0_f64;
        for a in 0..3 {
            for b in 0..3 {
                let s_ab = stress[a][b] - if a == b { hydrostatic } else { 0.0 };
                j2 += if a == b { 0.5 } else { 1.0 } * s_ab * s_ab;
            }
        }
        (3.0 * j2).sqrt()
    }

    /// Computes the deviatoric stress tensor.
    fn deviatoric(stress: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
        let hydrostatic = (stress[0][0] + stress[1][1] + stress[2][2]) / 3.0;
        let mut s = [[0.0_f64; 3]; 3];
        for a in 0..3 {
            for b in 0..3 {
                s[a][b] = stress[a][b] - if a == b { hydrostatic } else { 0.0 };
            }
        }
        s
    }

    /// Norton creep rate: d(eps_cr)/dt = A * sigma_eq^n * exp(-Q/(R*T))
    pub fn creep_rate(&self, sigma_eq: f64, temperature: f64) -> f64 {
        self.norton_a * sigma_eq.powf(self.norton_n) * (-self.q_over_r / temperature.max(1.0)).exp()
    }

    /// Performs one creep time step with implicit integration.
    ///
    /// The algorithm:
    /// 1. Compute elastic strain: eps_e = eps_total - eps_creep_old
    /// 2. Compute trial stress: sigma_trial = C : eps_e
    /// 3. Compute Norton creep strain rate from trial stress
    /// 4. Implicit integration: solve for delta_eps_cr such that
    ///    sigma_eq(C : (eps_total - eps_creep_old - delta_eps_cr)) gives
    ///    consistent creep rate with delta_eps_cr / dt
    /// 5. Update: eps_creep_new = eps_creep_old + delta_eps_cr
    /// 6. Recompute stress: sigma = C : (eps_total - eps_creep_new)
    ///
    /// Returns the maximum creep strain increment magnitude.
    pub fn solve_step(
        &mut self,
        state: &mut SolidState,
        _mesh: &UnstructuredMesh,
        temperature: &[f64],
        dt: f64,
    ) -> Result<f64> {
        let num_cells = state.num_cells();
        let (lambda, mu) = self.lame_parameters();
        let shear_modulus = mu;

        // Initialize creep strain storage if needed
        if self.creep_strain.len() != num_cells {
            self.creep_strain = vec![[[0.0; 3]; 3]; num_cells];
        }

        let mut max_creep_strain = 0.0_f64;

        for i in 0..num_cells {
            let total_strain = state.strain.get(i).unwrap_or([[0.0; 3]; 3]);
            let eps_cr_old = self.creep_strain[i];

            // Elastic strain: eps_e = eps_total - eps_creep
            let mut elastic_strain = [[0.0_f64; 3]; 3];
            for a in 0..3 {
                for b in 0..3 {
                    elastic_strain[a][b] = total_strain[a][b] - eps_cr_old[a][b];
                }
            }

            // Compute trial stress from elastic strain
            let trial_stress = Self::compute_stress(lambda, mu, &elastic_strain);
            let sigma_vm = Self::von_mises(&trial_stress);

            // Temperature for this cell
            let temp = if i < temperature.len() { temperature[i] } else { 300.0 };

            // If stress is negligible, skip
            if sigma_vm < 1e-30 {
                let _ = state.stress.set(i, trial_stress);
                continue;
            }

            // Implicit creep integration using Newton iteration
            // We solve for the equivalent creep strain increment delta_eps_eq:
            //
            //   delta_eps_eq / dt = A * sigma_eq_new^n * exp(-Q/(R*T))
            //
            // where sigma_eq_new = sigma_eq_trial - 3*G*delta_eps_eq
            // (radial return in deviatoric space, analogous to plasticity)
            //
            // Residual: R(x) = x - dt * A * (sigma_vm - 3*G*x)^n * exp(-Q/(R*T))
            // where x = delta_eps_eq

            let arrhenius = (-self.q_over_r / temp.max(1.0)).exp();
            let a_coeff = self.norton_a;
            let n_exp = self.norton_n;

            // Initial guess from explicit forward Euler
            let mut delta_eps_eq = dt * a_coeff * sigma_vm.powf(n_exp) * arrhenius;
            // Clamp to prevent overshooting past zero stress
            let max_delta = sigma_vm / (3.0 * shear_modulus);
            if delta_eps_eq > 0.9 * max_delta {
                delta_eps_eq = 0.9 * max_delta;
            }

            // Newton iteration for implicit integration
            for _iter in 0..self.max_newton_iter {
                let sigma_new = (sigma_vm - 3.0 * shear_modulus * delta_eps_eq).max(0.0);
                let creep_rate_new = a_coeff * sigma_new.powf(n_exp) * arrhenius;

                let residual = delta_eps_eq - dt * creep_rate_new;

                if residual.abs() < self.newton_tol * (1.0 + delta_eps_eq) {
                    break;
                }

                // Derivative of residual w.r.t. delta_eps_eq
                // dR/dx = 1 + dt * A * n * sigma_new^(n-1) * 3*G * arrhenius
                let d_residual = if sigma_new > 1e-30 {
                    1.0 + dt * a_coeff * n_exp * sigma_new.powf(n_exp - 1.0) * 3.0 * shear_modulus * arrhenius
                } else {
                    1.0
                };

                delta_eps_eq -= residual / d_residual;
                delta_eps_eq = delta_eps_eq.max(0.0);
            }

            if delta_eps_eq > max_creep_strain {
                max_creep_strain = delta_eps_eq;
            }

            // Update creep strain in deviatoric direction
            // d_eps_cr_ij = 1.5 * (delta_eps_eq / sigma_vm) * s_ij
            let dev_stress = Self::deviatoric(&trial_stress);
            let factor = 1.5 * delta_eps_eq / sigma_vm;

            let mut eps_cr_new = eps_cr_old;
            for a in 0..3 {
                for b in 0..3 {
                    eps_cr_new[a][b] += factor * dev_stress[a][b];
                }
            }
            self.creep_strain[i] = eps_cr_new;

            // Recompute elastic strain and stress for consistency
            let mut elastic_strain_new = [[0.0_f64; 3]; 3];
            for a in 0..3 {
                for b in 0..3 {
                    elastic_strain_new[a][b] = total_strain[a][b] - eps_cr_new[a][b];
                }
            }
            let stress_new = Self::compute_stress(lambda, mu, &elastic_strain_new);

            let _ = state.stress.set(i, stress_new);
            let _ = state.strain.set(i, total_strain);
        }

        self.current_time += dt;

        Ok(max_creep_strain)
    }

    /// Performs multiple creep time steps to reach a target time.
    ///
    /// Returns the total accumulated maximum creep strain increment.
    pub fn solve_to_time(
        &mut self,
        state: &mut SolidState,
        mesh: &UnstructuredMesh,
        temperature: &[f64],
        target_time: f64,
        dt: f64,
    ) -> Result<f64> {
        let mut total_max_creep = 0.0_f64;

        while self.current_time < target_time - 1e-30 {
            let step_dt = dt.min(target_time - self.current_time);
            let max_creep = self.solve_step(state, mesh, temperature, step_dt)?;
            if max_creep > total_max_creep {
                total_max_creep = max_creep;
            }
        }

        Ok(total_max_creep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::StructuredMesh;

    /// Test that creep produces strain relaxation under constant total strain.
    #[test]
    fn creep_stress_relaxation() {
        let structured = StructuredMesh::uniform(1, 1, 1, 1.0, 1.0, 1.0);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();
        let mut state = SolidState::new(num_cells);

        // Apply an initial uniaxial strain
        let initial_strain = [[0.001, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
        let _ = state.strain.set(0, initial_strain);

        // Compute initial stress
        let e = 200e9;
        let nu = 0.3;
        let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let mu = e / (2.0 * (1.0 + nu));
        let initial_stress = CreepSolver::compute_stress(lambda, mu, &initial_strain);
        let _ = state.stress.set(0, initial_stress);
        let initial_vm = CreepSolver::von_mises(&initial_stress);

        // Create Norton creep solver: moderate creep parameters
        let mut solver = CreepSolver::norton(1e-20, 3.0, 0.0) // Q/R=0 means no temperature dependence
            .with_elastic_properties(e, nu);

        let temperature = vec![300.0; num_cells];
        let dt = 1.0; // 1 second steps

        // Run several creep steps
        for _ in 0..10 {
            solver.solve_step(&mut state, &mesh, &temperature, dt).unwrap();
        }

        // After creep, the von Mises stress should decrease (relaxation)
        let final_stress = state.stress.get(0).unwrap();
        let final_vm = CreepSolver::von_mises(&final_stress);

        assert!(
            final_vm < initial_vm,
            "Stress should relax due to creep: initial VM = {:.3e}, final VM = {:.3e}",
            initial_vm,
            final_vm
        );

        // Creep strain should have accumulated
        assert!(
            !solver.creep_strain().is_empty(),
            "Creep strain storage should be initialized"
        );
        let eps_cr = solver.creep_strain()[0];
        let eps_cr_mag = (eps_cr[0][0].powi(2) + eps_cr[1][1].powi(2) + eps_cr[2][2].powi(2)
            + 2.0 * (eps_cr[0][1].powi(2) + eps_cr[1][2].powi(2) + eps_cr[0][2].powi(2)))
        .sqrt();
        assert!(
            eps_cr_mag > 0.0,
            "Creep strain should be non-zero"
        );

        eprintln!("Initial VM stress: {:.3e} Pa", initial_vm);
        eprintln!("Final VM stress:   {:.3e} Pa", final_vm);
        eprintln!("Creep strain magnitude: {:.6e}", eps_cr_mag);
    }

    /// Test that zero stress produces zero creep.
    #[test]
    fn creep_zero_stress() {
        let structured = StructuredMesh::uniform(1, 1, 1, 1.0, 1.0, 1.0);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();
        let mut state = SolidState::new(num_cells);

        let mut solver = CreepSolver::norton(1e-15, 3.0, 0.0)
            .with_elastic_properties(200e9, 0.3);

        let temperature = vec![300.0; num_cells];
        let max_creep = solver.solve_step(&mut state, &mesh, &temperature, 1.0).unwrap();

        assert!(
            max_creep < 1e-30,
            "Zero stress should produce zero creep strain, got {}",
            max_creep
        );
    }

    /// Test temperature dependence of creep rate.
    #[test]
    fn creep_temperature_dependence() {
        let structured = StructuredMesh::uniform(2, 1, 1, 2.0, 1.0, 1.0);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();

        // Apply same strain to both cells
        let strain = [[0.001, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]];

        let e = 200e9;
        let nu = 0.3;
        let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let mu = e / (2.0 * (1.0 + nu));
        let stress = CreepSolver::compute_stress(lambda, mu, &strain);

        // Run at two different temperatures
        let q_over_r = 20000.0; // Typical activation parameter

        // Low temperature
        let mut state_low = SolidState::new(num_cells);
        let _ = state_low.strain.set(0, strain);
        let _ = state_low.stress.set(0, stress);
        let _ = state_low.strain.set(1, strain);
        let _ = state_low.stress.set(1, stress);
        let mut solver_low = CreepSolver::norton(1e-15, 3.0, q_over_r)
            .with_elastic_properties(e, nu);
        let temp_low = vec![500.0; num_cells]; // 500 K
        let max_low = solver_low.solve_step(&mut state_low, &mesh, &temp_low, 1.0).unwrap();

        // High temperature
        let mut state_high = SolidState::new(num_cells);
        let _ = state_high.strain.set(0, strain);
        let _ = state_high.stress.set(0, stress);
        let _ = state_high.strain.set(1, strain);
        let _ = state_high.stress.set(1, stress);
        let mut solver_high = CreepSolver::norton(1e-15, 3.0, q_over_r)
            .with_elastic_properties(e, nu);
        let temp_high = vec![1000.0; num_cells]; // 1000 K
        let max_high = solver_high.solve_step(&mut state_high, &mesh, &temp_high, 1.0).unwrap();

        assert!(
            max_high > max_low,
            "Higher temperature should produce more creep: low={:.3e}, high={:.3e}",
            max_low,
            max_high
        );

        eprintln!("Creep at 500K:  {:.6e}", max_low);
        eprintln!("Creep at 1000K: {:.6e}", max_high);
    }

    /// Test solve_to_time for multi-step creep integration.
    #[test]
    fn creep_multi_step() {
        let structured = StructuredMesh::uniform(1, 1, 1, 1.0, 1.0, 1.0);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();
        let mut state = SolidState::new(num_cells);

        let strain = [[0.001, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
        let _ = state.strain.set(0, strain);
        let e = 200e9;
        let nu = 0.3;
        let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let mu = e / (2.0 * (1.0 + nu));
        let stress = CreepSolver::compute_stress(lambda, mu, &strain);
        let _ = state.stress.set(0, stress);

        let mut solver = CreepSolver::norton(1e-20, 3.0, 0.0)
            .with_elastic_properties(e, nu);
        let temperature = vec![300.0; num_cells];

        solver.solve_to_time(&mut state, &mesh, &temperature, 10.0, 1.0).unwrap();

        assert!(
            (solver.current_time() - 10.0).abs() < 1e-10,
            "Should have advanced to target time"
        );
    }

    /// Test that creep strain is deviatoric (trace = 0 for incompressible creep).
    #[test]
    fn creep_strain_deviatoric() {
        let structured = StructuredMesh::uniform(1, 1, 1, 1.0, 1.0, 1.0);
        let mesh = structured.to_unstructured();
        let num_cells = mesh.num_cells();
        let mut state = SolidState::new(num_cells);

        // Biaxial strain state
        let strain = [[0.001, 0.0, 0.0], [0.0, -0.0005, 0.0], [0.0, 0.0, -0.0005]];
        let _ = state.strain.set(0, strain);
        let e = 200e9;
        let nu = 0.3;
        let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let mu = e / (2.0 * (1.0 + nu));
        let stress = CreepSolver::compute_stress(lambda, mu, &strain);
        let _ = state.stress.set(0, stress);

        let mut solver = CreepSolver::norton(1e-20, 3.0, 0.0)
            .with_elastic_properties(e, nu);
        let temperature = vec![300.0; num_cells];

        for _ in 0..5 {
            solver.solve_step(&mut state, &mesh, &temperature, 1.0).unwrap();
        }

        let eps_cr = solver.creep_strain()[0];
        let trace = eps_cr[0][0] + eps_cr[1][1] + eps_cr[2][2];

        assert!(
            trace.abs() < 1e-20,
            "Creep strain should be deviatoric (trace=0), got trace = {:.3e}",
            trace
        );
    }
}
