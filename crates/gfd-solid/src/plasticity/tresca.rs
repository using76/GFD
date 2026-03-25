//! Tresca (maximum shear stress) yield criterion.
//!
//! The Tresca criterion states that yielding occurs when the maximum
//! shear stress reaches a critical value equal to half the yield stress.

/// Tresca yield criterion.
///
/// f = max(|s1-s2|, |s2-s3|, |s1-s3|) - sigma_y
///
/// where s1, s2, s3 are the principal stresses.
/// Equivalent to: the maximum shear stress >= sigma_y / 2.
pub struct TrescaYield {
    /// Yield stress in uniaxial tension [Pa].
    pub yield_stress: f64,
}

impl TrescaYield {
    /// Creates a new Tresca yield criterion.
    pub fn new(yield_stress: f64) -> Self {
        Self { yield_stress }
    }

    /// Evaluates the Tresca yield function from principal stresses.
    ///
    /// f = max(|s1 - s2|, |s2 - s3|, |s1 - s3|) - sigma_y
    ///
    /// f <= 0: elastic (inside or on yield surface)
    /// f > 0: plastic (violates yield criterion)
    ///
    /// # Arguments
    /// * `principal_stresses` - The three principal stresses [s1, s2, s3] in any order.
    pub fn yield_function(&self, principal_stresses: &[f64; 3]) -> f64 {
        let s1 = principal_stresses[0];
        let s2 = principal_stresses[1];
        let s3 = principal_stresses[2];

        let d12 = (s1 - s2).abs();
        let d23 = (s2 - s3).abs();
        let d13 = (s1 - s3).abs();

        let max_diff = d12.max(d23).max(d13);

        max_diff - self.yield_stress
    }

    /// Computes the principal stresses from a symmetric 3x3 stress tensor.
    ///
    /// Uses the analytical solution for eigenvalues of a 3x3 symmetric matrix
    /// based on the invariants I1, I2, I3.
    pub fn compute_principal_stresses(stress: &[[f64; 3]; 3]) -> [f64; 3] {
        // First invariant: I1 = trace(sigma)
        let i1 = stress[0][0] + stress[1][1] + stress[2][2];

        // Second invariant: I2 = 0.5*(trace(sigma)^2 - trace(sigma^2))
        let trace_sq = stress[0][0] * stress[0][0]
            + stress[1][1] * stress[1][1]
            + stress[2][2] * stress[2][2]
            + 2.0 * stress[0][1] * stress[0][1]
            + 2.0 * stress[1][2] * stress[1][2]
            + 2.0 * stress[0][2] * stress[0][2];
        let i2 = 0.5 * (i1 * i1 - trace_sq);

        // Third invariant: I3 = det(sigma)
        let i3 = stress[0][0] * (stress[1][1] * stress[2][2] - stress[1][2] * stress[2][1])
            - stress[0][1] * (stress[1][0] * stress[2][2] - stress[1][2] * stress[2][0])
            + stress[0][2] * (stress[1][0] * stress[2][1] - stress[1][1] * stress[2][0]);

        // Use Cardano's method for the characteristic equation:
        // lambda^3 - I1*lambda^2 + I2*lambda - I3 = 0
        let p = i1 / 3.0;
        let q = (2.0 * i1 * i1 * i1 - 9.0 * i1 * i2 + 27.0 * i3) / 54.0;
        let r_sq = (i1 * i1 - 3.0 * i2) / 9.0;
        let r = if r_sq >= 0.0 { r_sq.sqrt() } else { 0.0 };

        if r.abs() < 1e-30 {
            // All eigenvalues are equal
            return [p, p, p];
        }

        let cos_arg = (q / (r * r * r)).clamp(-1.0, 1.0);
        let theta = cos_arg.acos();

        let s1 = p + 2.0 * r * (theta / 3.0).cos();
        let s2 = p + 2.0 * r * ((theta + 2.0 * std::f64::consts::PI) / 3.0).cos();
        let s3 = p + 2.0 * r * ((theta + 4.0 * std::f64::consts::PI) / 3.0).cos();

        // Sort in descending order
        let mut principals = [s1, s2, s3];
        principals.sort_by(|a, b| b.partial_cmp(a).unwrap());
        principals
    }

    /// Evaluates the Tresca yield function from a full stress tensor.
    ///
    /// Computes principal stresses first, then evaluates the yield function.
    pub fn yield_function_from_tensor(&self, stress: &[[f64; 3]; 3]) -> f64 {
        let principals = Self::compute_principal_stresses(stress);
        self.yield_function(&principals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tresca_uniaxial() {
        let tresca = TrescaYield::new(250.0e6);
        // Uniaxial stress: s1=200 MPa, s2=s3=0
        let f = tresca.yield_function(&[200.0e6, 0.0, 0.0]);
        assert!(f < 0.0, "Should be elastic below yield");

        let f = tresca.yield_function(&[300.0e6, 0.0, 0.0]);
        assert!(f > 0.0, "Should be plastic above yield");
    }

    #[test]
    fn test_tresca_at_yield() {
        let tresca = TrescaYield::new(250.0e6);
        let f = tresca.yield_function(&[250.0e6, 0.0, 0.0]);
        assert!(f.abs() < 1e-6, "Should be exactly at yield");
    }

    #[test]
    fn test_tresca_hydrostatic() {
        let tresca = TrescaYield::new(250.0e6);
        // Hydrostatic stress: all principal stresses equal
        let f = tresca.yield_function(&[500.0e6, 500.0e6, 500.0e6]);
        assert!(
            (f - (-250.0e6)).abs() < 1e-6,
            "Hydrostatic stress should not cause yielding"
        );
    }

    #[test]
    fn test_principal_stress_uniaxial() {
        let stress = [
            [100.0e6, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let principals = TrescaYield::compute_principal_stresses(&stress);
        assert!((principals[0] - 100.0e6).abs() < 1.0, "First principal should be 100 MPa");
        assert!(principals[1].abs() < 1.0, "Second principal should be ~0");
        assert!(principals[2].abs() < 1.0, "Third principal should be ~0");
    }
}
