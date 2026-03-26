//! Von Mises (J2) yield criterion and return-mapping.
//!
//! Implements the classical von Mises yield surface with isotropic
//! hardening and radial return-mapping algorithm.

/// Von Mises yield criterion with isotropic linear hardening.
///
/// The yield function is: f = sigma_vm - sigma_y(eps_p)
/// where sigma_y = sigma_y0 + H * eps_p
pub struct VonMisesYield {
    /// Initial yield stress [Pa].
    pub yield_stress: f64,
    /// Isotropic hardening modulus H [Pa].
    pub hardening_modulus: f64,
}

impl VonMisesYield {
    /// Creates a new Von Mises yield criterion.
    pub fn new(yield_stress: f64, hardening_modulus: f64) -> Self {
        Self {
            yield_stress,
            hardening_modulus,
        }
    }

    /// Computes the current yield stress accounting for isotropic hardening.
    ///
    /// sigma_y = sigma_y0 + H * eps_p
    pub fn current_yield_stress(&self, equivalent_plastic_strain: f64) -> f64 {
        self.yield_stress + self.hardening_modulus * equivalent_plastic_strain
    }

    /// Computes the Von Mises equivalent stress from a 3x3 stress tensor.
    ///
    /// sigma_vm = sqrt(3/2 * s_ij * s_ij)
    ///
    /// where s_ij = sigma_ij - 1/3 * sigma_kk * delta_ij is the deviatoric stress.
    ///
    /// Expanded:
    /// sigma_vm = sqrt(0.5 * ((s11-s22)^2 + (s22-s33)^2 + (s33-s11)^2 + 6*(s12^2+s23^2+s13^2)))
    pub fn compute_von_mises(stress: &[[f64; 3]; 3]) -> f64 {
        // Hydrostatic (mean) stress
        let hydrostatic = (stress[0][0] + stress[1][1] + stress[2][2]) / 3.0;

        // Deviatoric stress components
        let s = [
            [stress[0][0] - hydrostatic, stress[0][1], stress[0][2]],
            [stress[1][0], stress[1][1] - hydrostatic, stress[1][2]],
            [stress[2][0], stress[2][1], stress[2][2] - hydrostatic],
        ];

        // J2 = 0.5 * s_ij * s_ij (using full tensor contraction)
        let j2 = 0.5
            * (s[0][0] * s[0][0]
                + s[1][1] * s[1][1]
                + s[2][2] * s[2][2]
                + 2.0 * s[0][1] * s[0][1]
                + 2.0 * s[1][2] * s[1][2]
                + 2.0 * s[0][2] * s[0][2]);

        // sigma_vm = sqrt(3 * J2)
        (3.0 * j2).sqrt()
    }

    /// Evaluates the yield function.
    ///
    /// f = sigma_vm - sigma_y(eps_p)
    ///
    /// f <= 0: elastic (inside or on yield surface)
    /// f > 0: plastic (requires return mapping)
    pub fn yield_function(&self, stress: &[[f64; 3]; 3], equivalent_plastic_strain: f64) -> f64 {
        let sigma_vm = Self::compute_von_mises(stress);
        let sigma_y = self.current_yield_stress(equivalent_plastic_strain);
        sigma_vm - sigma_y
    }

    /// Performs the radial return-mapping algorithm.
    ///
    /// Given a trial stress (from a purely elastic predictor), projects
    /// back to the yield surface if the trial state exceeds yield.
    ///
    /// Algorithm:
    /// 1. Compute trial von Mises stress
    /// 2. Check yield condition: f_trial = sigma_vm_trial - sigma_y(eps_p_old)
    /// 3. If f_trial <= 0: elastic, return trial stress unchanged
    /// 4. If f_trial > 0: compute plastic multiplier
    ///    delta_gamma = f_trial / (3*G + H)
    ///    sigma = sigma_trial - 2*G*delta_gamma * n
    ///    eps_p_new = eps_p_old + delta_gamma
    ///
    /// # Arguments
    /// * `trial_stress` - The elastic trial stress tensor.
    /// * `shear_modulus` - Shear modulus G [Pa].
    /// * `eps_p_old` - Previous accumulated equivalent plastic strain.
    ///
    /// # Returns
    /// Tuple of (corrected stress tensor, updated equivalent plastic strain).
    pub fn return_mapping(
        &self,
        _trial_stress: &[[f64; 3]; 3],
        _shear_modulus: f64,
        _eps_p_old: f64,
    ) -> ([[f64; 3]; 3], f64) {
        // 1. Compute trial von Mises stress and deviatoric direction
        // 2. Check yield: f_trial = sigma_vm_trial - sigma_y(eps_p_old)
        // 3. If elastic (f <= 0), return (trial_stress, eps_p_old)
        // 4. Plastic correction:
        //    a. delta_gamma = f_trial / (3*G + H)
        //    b. n_ij = s_trial_ij / (sigma_vm_trial) (flow direction)
        //    c. sigma_ij = sigma_trial_ij - 2*G*delta_gamma * n_ij * sqrt(3/2)
        //    d. eps_p_new = eps_p_old + delta_gamma
        // 5. Return (corrected_stress, eps_p_new)
        let trial_stress = _trial_stress;
        let g = _shear_modulus;
        let eps_p_old = _eps_p_old;

        // 1. Compute hydrostatic and deviatoric parts
        let hydrostatic = (trial_stress[0][0] + trial_stress[1][1] + trial_stress[2][2]) / 3.0;
        let mut s = [[0.0_f64; 3]; 3];
        for a in 0..3 {
            for b in 0..3 {
                s[a][b] = trial_stress[a][b] - if a == b { hydrostatic } else { 0.0 };
            }
        }

        // 2. Compute trial von Mises stress
        let sigma_vm_trial = Self::compute_von_mises(trial_stress);

        // 3. Check yield condition
        let sigma_y = self.current_yield_stress(eps_p_old);
        let f_trial = sigma_vm_trial - sigma_y;

        if f_trial <= 0.0 {
            // Elastic: return trial stress unchanged
            return (*trial_stress, eps_p_old);
        }

        // 4. Plastic correction
        let h = self.hardening_modulus;
        let delta_gamma = f_trial / (3.0 * g + h);

        // 5. Return to yield surface
        let mut corrected = *trial_stress;
        if sigma_vm_trial > 1e-30 {
            let scale = 1.0 - 3.0 * g * delta_gamma / sigma_vm_trial;
            for a in 0..3 {
                for b in 0..3 {
                    let dev = s[a][b];
                    corrected[a][b] = scale * dev + if a == b { hydrostatic } else { 0.0 };
                }
            }
        }

        let eps_p_new = eps_p_old + delta_gamma;

        (corrected, eps_p_new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_von_mises_uniaxial() {
        // Uniaxial stress state: sigma_11 = 100 MPa, all others zero
        let stress = [
            [100.0e6, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let vm = VonMisesYield::compute_von_mises(&stress);
        assert!(
            (vm - 100.0e6).abs() / 100.0e6 < 1e-10,
            "Von Mises stress for uniaxial should equal the applied stress"
        );
    }

    #[test]
    fn test_von_mises_pure_shear() {
        // Pure shear: sigma_12 = sigma_21 = tau
        let tau = 50.0e6;
        let stress = [
            [0.0, tau, 0.0],
            [tau, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let vm = VonMisesYield::compute_von_mises(&stress);
        let expected = tau * 3.0_f64.sqrt();
        assert!(
            (vm - expected).abs() / expected < 1e-10,
            "Von Mises for pure shear should be sqrt(3)*tau"
        );
    }

    #[test]
    fn test_yield_function_elastic() {
        let ym = VonMisesYield::new(250.0e6, 0.0);
        let stress = [
            [100.0e6, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let f = ym.yield_function(&stress, 0.0);
        assert!(f < 0.0, "Should be elastic (below yield)");
    }

    #[test]
    fn test_hydrostatic_gives_zero() {
        // Hydrostatic stress should give zero von Mises stress
        let p = 100.0e6;
        let stress = [
            [p, 0.0, 0.0],
            [0.0, p, 0.0],
            [0.0, 0.0, p],
        ];
        let vm = VonMisesYield::compute_von_mises(&stress);
        assert!(vm.abs() < 1e-6, "Hydrostatic stress should give zero von Mises");
    }

    #[test]
    fn test_return_mapping_elastic() {
        // Trial stress below yield -> return mapping should not modify stress
        let ym = VonMisesYield::new(250e6, 0.0);
        let trial_stress = [
            [100e6, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let g = 80e9; // shear modulus
        let (corrected, eps_p) = ym.return_mapping(&trial_stress, g, 0.0);

        assert!(
            (eps_p - 0.0).abs() < 1e-30,
            "No plastic strain expected in elastic range"
        );
        assert!(
            (corrected[0][0] - trial_stress[0][0]).abs() < 1e-6,
            "Stress should be unchanged in elastic range"
        );
    }

    #[test]
    fn test_return_mapping_plastic() {
        // Trial stress above yield -> return mapping should reduce von Mises stress
        let yield_stress = 250e6;
        let hardening = 1e9;
        let ym = VonMisesYield::new(yield_stress, hardening);

        let trial_stress = [
            [400e6, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let g = 80e9;
        let (corrected, eps_p) = ym.return_mapping(&trial_stress, g, 0.0);

        assert!(eps_p > 0.0, "Should have accumulated plastic strain");

        let vm_corrected = VonMisesYield::compute_von_mises(&corrected);
        let sigma_y = ym.current_yield_stress(eps_p);

        // After return mapping, von Mises stress should be at the yield surface
        assert!(
            (vm_corrected - sigma_y).abs() / sigma_y < 1e-8,
            "Corrected VM stress {:.3e} should equal yield stress {:.3e}",
            vm_corrected,
            sigma_y
        );
    }

    #[test]
    fn test_return_mapping_preserves_hydrostatic() {
        // Return mapping should only affect deviatoric stress, not hydrostatic
        let ym = VonMisesYield::new(100e6, 0.0);

        let trial_stress = [
            [300e6, 50e6, 0.0],
            [50e6, 100e6, 0.0],
            [0.0, 0.0, 200e6],
        ];
        let hydrostatic_before = (trial_stress[0][0] + trial_stress[1][1] + trial_stress[2][2]) / 3.0;

        let g = 80e9;
        let (corrected, _eps_p) = ym.return_mapping(&trial_stress, g, 0.0);

        let hydrostatic_after = (corrected[0][0] + corrected[1][1] + corrected[2][2]) / 3.0;

        assert!(
            (hydrostatic_after - hydrostatic_before).abs() / hydrostatic_before.abs() < 1e-10,
            "Hydrostatic stress should be preserved: before {:.3e}, after {:.3e}",
            hydrostatic_before,
            hydrostatic_after
        );
    }
}
