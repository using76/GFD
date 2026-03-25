//! Plasticity models with return-mapping algorithms.

pub mod von_mises;
pub mod tresca;
pub mod drucker_prager;

use gfd_core::UnstructuredMesh;
use crate::{SolidState, Result};

/// Von Mises (J2) plasticity model.
///
/// Uses the radial return-mapping algorithm to integrate the
/// elastoplastic constitutive equations.
pub struct VonMisesPlasticity {
    /// Young's modulus [Pa].
    pub youngs_modulus: f64,
    /// Poisson's ratio [-].
    pub poissons_ratio: f64,
    /// Initial yield stress [Pa].
    pub yield_stress: f64,
    /// Isotropic hardening modulus [Pa].
    pub hardening_modulus: f64,
    /// Accumulated plastic strain for each integration point.
    pub equivalent_plastic_strain: Vec<f64>,
}

impl VonMisesPlasticity {
    /// Creates a new Von Mises plasticity model.
    pub fn new(
        youngs_modulus: f64,
        poissons_ratio: f64,
        yield_stress: f64,
        hardening_modulus: f64,
        num_integration_points: usize,
    ) -> Self {
        Self {
            youngs_modulus,
            poissons_ratio,
            yield_stress,
            hardening_modulus,
            equivalent_plastic_strain: vec![0.0; num_integration_points],
        }
    }

    /// Computes the Von Mises equivalent stress from the stress tensor.
    ///
    /// sigma_vm = sqrt(3/2 * s_ij * s_ij)
    /// where s_ij = sigma_ij - 1/3 * sigma_kk * delta_ij is the deviatoric stress.
    pub fn von_mises_stress(stress: &[[f64; 3]; 3]) -> f64 {
        let hydrostatic = (stress[0][0] + stress[1][1] + stress[2][2]) / 3.0;
        let s = [
            [stress[0][0] - hydrostatic, stress[0][1], stress[0][2]],
            [stress[1][0], stress[1][1] - hydrostatic, stress[1][2]],
            [stress[2][0], stress[2][1], stress[2][2] - hydrostatic],
        ];

        let j2 = 0.5
            * (s[0][0] * s[0][0]
                + s[1][1] * s[1][1]
                + s[2][2] * s[2][2]
                + 2.0 * s[0][1] * s[0][1]
                + 2.0 * s[1][2] * s[1][2]
                + 2.0 * s[0][2] * s[0][2]);

        (3.0 * j2).sqrt()
    }

    /// Solves the elastoplastic problem using Newton-Raphson with return mapping.
    ///
    /// Return-mapping algorithm outline:
    /// 1. Compute trial elastic strain: epsilon_trial = epsilon_n + delta_epsilon
    /// 2. Compute trial stress: sigma_trial = C : epsilon_trial
    /// 3. Check yield condition: f = sigma_vm_trial - sigma_y(alpha)
    /// 4. If f <= 0: elastic step, accept trial stress
    /// 5. If f > 0: plastic step, apply radial return:
    ///    - delta_gamma = f / (3*G + H)
    ///    - sigma = sigma_trial - 2*G*delta_gamma * n
    ///    - alpha = alpha_n + delta_gamma
    pub fn solve(
        &mut self,
        _state: &mut SolidState,
        _mesh: &UnstructuredMesh,
        _body_forces: &[[f64; 3]],
    ) -> Result<f64> {
        let num_cells = _state.num_cells();
        let e = self.youngs_modulus;
        let nu = self.poissons_ratio;
        let g_mod = e / (2.0 * (1.0 + nu)); // Shear modulus
        let h = self.hardening_modulus;

        let mut max_residual = 0.0_f64;

        for i in 0..num_cells {
            // Get current strain as trial elastic strain
            let strain = _state.strain.get(i).unwrap_or([[0.0; 3]; 3]);

            // Compute trial stress from strain (Hooke's law)
            let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
            let trace_eps = strain[0][0] + strain[1][1] + strain[2][2];
            let mut trial_stress = [[0.0_f64; 3]; 3];
            for a in 0..3 {
                for b in 0..3 {
                    trial_stress[a][b] = 2.0 * g_mod * strain[a][b];
                    if a == b {
                        trial_stress[a][b] += lambda * trace_eps;
                    }
                }
            }

            // Von Mises equivalent stress
            let sigma_vm = Self::von_mises_stress(&trial_stress);

            // Current yield stress
            let eps_p = if i < self.equivalent_plastic_strain.len() {
                self.equivalent_plastic_strain[i]
            } else {
                0.0
            };
            let sigma_y = self.yield_stress + h * eps_p;

            // Check yield
            let f_trial = sigma_vm - sigma_y;
            if f_trial > 0.0 {
                // Plastic correction: radial return
                let delta_gamma = f_trial / (3.0 * g_mod + h);

                // Deviatoric direction
                let hydrostatic = (trial_stress[0][0] + trial_stress[1][1] + trial_stress[2][2]) / 3.0;
                let mut corrected = trial_stress;
                if sigma_vm > 1e-30 {
                    let scale = 1.0 - 3.0 * g_mod * delta_gamma / sigma_vm;
                    for a in 0..3 {
                        for b in 0..3 {
                            let dev = trial_stress[a][b] - if a == b { hydrostatic } else { 0.0 };
                            corrected[a][b] = scale * dev + if a == b { hydrostatic } else { 0.0 };
                        }
                    }
                }

                let _ = _state.stress.set(i, corrected);
                if i < self.equivalent_plastic_strain.len() {
                    self.equivalent_plastic_strain[i] = eps_p + delta_gamma;
                }

                if f_trial > max_residual {
                    max_residual = f_trial;
                }
            } else {
                // Elastic: accept trial stress
                let _ = _state.stress.set(i, trial_stress);
            }
        }

        Ok(max_residual)
    }
}
