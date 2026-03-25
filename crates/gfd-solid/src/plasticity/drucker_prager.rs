//! Drucker-Prager yield criterion.
//!
//! A pressure-dependent yield criterion commonly used for geomaterials
//! (soils, rocks, concrete) where the yield stress depends on the
//! hydrostatic pressure.

/// Drucker-Prager yield criterion.
///
/// The yield function is: f = alpha * I1 + sqrt(J2) - k
///
/// where:
/// - I1 = trace(sigma) is the first stress invariant
/// - J2 = 0.5 * s_ij * s_ij is the second deviatoric stress invariant
/// - alpha and k are material constants derived from the friction angle
///   and cohesion:
///   - alpha = 2*sin(phi) / (sqrt(3) * (3 - sin(phi)))
///   - k = 6*c*cos(phi) / (sqrt(3) * (3 - sin(phi)))
///
/// This formulation matches the Mohr-Coulomb criterion on the
/// compressive meridian (outer cone approximation).
pub struct DruckerPrager {
    /// Internal friction angle phi [radians].
    pub friction_angle: f64,
    /// Cohesion c [Pa].
    pub cohesion: f64,
    /// Derived parameter alpha = 2*sin(phi) / (sqrt(3)*(3-sin(phi))).
    alpha: f64,
    /// Derived parameter k = 6*c*cos(phi) / (sqrt(3)*(3-sin(phi))).
    k: f64,
}

impl DruckerPrager {
    /// Creates a new Drucker-Prager criterion from friction angle and cohesion.
    ///
    /// # Arguments
    /// * `friction_angle` - Internal friction angle in radians.
    /// * `cohesion` - Cohesion in Pascals [Pa].
    pub fn new(friction_angle: f64, cohesion: f64) -> Self {
        let sin_phi = friction_angle.sin();
        let cos_phi = friction_angle.cos();
        let sqrt3 = 3.0_f64.sqrt();
        let denom = sqrt3 * (3.0 - sin_phi);

        let alpha = 2.0 * sin_phi / denom;
        let k = 6.0 * cohesion * cos_phi / denom;

        Self {
            friction_angle,
            cohesion,
            alpha,
            k,
        }
    }

    /// Creates a new Drucker-Prager criterion from friction angle in degrees and cohesion.
    pub fn from_degrees(friction_angle_deg: f64, cohesion: f64) -> Self {
        Self::new(friction_angle_deg.to_radians(), cohesion)
    }

    /// Returns the derived alpha parameter.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Returns the derived k parameter.
    pub fn k(&self) -> f64 {
        self.k
    }

    /// Computes the first stress invariant I1 = trace(sigma).
    pub fn compute_i1(stress: &[[f64; 3]; 3]) -> f64 {
        stress[0][0] + stress[1][1] + stress[2][2]
    }

    /// Computes the second deviatoric stress invariant J2.
    ///
    /// J2 = 0.5 * s_ij * s_ij
    /// where s_ij = sigma_ij - (I1/3) * delta_ij
    pub fn compute_j2(stress: &[[f64; 3]; 3]) -> f64 {
        let mean = (stress[0][0] + stress[1][1] + stress[2][2]) / 3.0;

        let s = [
            [stress[0][0] - mean, stress[0][1], stress[0][2]],
            [stress[1][0], stress[1][1] - mean, stress[1][2]],
            [stress[2][0], stress[2][1], stress[2][2] - mean],
        ];

        0.5 * (s[0][0] * s[0][0]
            + s[1][1] * s[1][1]
            + s[2][2] * s[2][2]
            + 2.0 * s[0][1] * s[0][1]
            + 2.0 * s[1][2] * s[1][2]
            + 2.0 * s[0][2] * s[0][2])
    }

    /// Evaluates the Drucker-Prager yield function.
    ///
    /// f = alpha * I1 + sqrt(J2) - k
    ///
    /// f <= 0: elastic (inside or on yield surface)
    /// f > 0: plastic (violates yield criterion)
    ///
    /// # Arguments
    /// * `stress` - The full 3x3 Cauchy stress tensor.
    pub fn yield_function(&self, stress: &[[f64; 3]; 3]) -> f64 {
        let i1 = Self::compute_i1(stress);
        let j2 = Self::compute_j2(stress);
        self.alpha * i1 + j2.sqrt() - self.k
    }

    /// Evaluates the yield function from pre-computed invariants.
    ///
    /// f = alpha * I1 + sqrt(J2) - k
    ///
    /// # Arguments
    /// * `i1` - First stress invariant (trace of stress).
    /// * `j2` - Second deviatoric stress invariant.
    pub fn yield_function_from_invariants(&self, i1: f64, j2: f64) -> f64 {
        self.alpha * i1 + j2.sqrt() - self.k
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drucker_prager_parameters() {
        // Typical soil: phi = 30 degrees, c = 50 kPa
        let dp = DruckerPrager::from_degrees(30.0, 50_000.0);
        let sin30 = 0.5_f64;
        let cos30 = (3.0_f64 / 4.0).sqrt();
        let sqrt3 = 3.0_f64.sqrt();
        let denom = sqrt3 * (3.0 - sin30);

        let expected_alpha = 2.0 * sin30 / denom;
        let expected_k = 6.0 * 50_000.0 * cos30 / denom;

        assert!(
            (dp.alpha() - expected_alpha).abs() < 1e-10,
            "Alpha parameter mismatch"
        );
        assert!(
            (dp.k() - expected_k).abs() < 1e-6,
            "k parameter mismatch"
        );
    }

    #[test]
    fn test_hydrostatic_tension_yields() {
        // Under hydrostatic tension, Drucker-Prager should eventually yield
        // (unlike von Mises which is insensitive to hydrostatic stress)
        let dp = DruckerPrager::from_degrees(30.0, 50_000.0);
        let p = 1.0e9; // Very large hydrostatic tension
        let stress = [
            [p, 0.0, 0.0],
            [0.0, p, 0.0],
            [0.0, 0.0, p],
        ];
        let f = dp.yield_function(&stress);
        assert!(
            f > 0.0,
            "Large hydrostatic tension should cause yielding in Drucker-Prager"
        );
    }

    #[test]
    fn test_hydrostatic_compression_elastic() {
        // Under hydrostatic compression, the material should remain elastic
        // (compressive mean stress helps resist yielding)
        let dp = DruckerPrager::from_degrees(30.0, 50_000.0);
        let p = -1.0e6; // Moderate hydrostatic compression
        let stress = [
            [p, 0.0, 0.0],
            [0.0, p, 0.0],
            [0.0, 0.0, p],
        ];
        let f = dp.yield_function(&stress);
        assert!(
            f < 0.0,
            "Hydrostatic compression should be elastic"
        );
    }

    #[test]
    fn test_invariant_computation() {
        let stress = [
            [100.0, 20.0, 0.0],
            [20.0, -50.0, 10.0],
            [0.0, 10.0, 30.0],
        ];
        let i1 = DruckerPrager::compute_i1(&stress);
        assert!((i1 - 80.0).abs() < 1e-10, "I1 = trace should be 80");

        let j2 = DruckerPrager::compute_j2(&stress);
        assert!(j2 > 0.0, "J2 should be positive for non-hydrostatic stress");
    }
}
