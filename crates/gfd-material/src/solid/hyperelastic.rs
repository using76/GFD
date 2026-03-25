//! Hyperelastic constitutive models.

use crate::traits::{ConstitutiveModel, MaterialState};
use crate::Result;

/// Neo-Hookean hyperelastic material.
///
/// Strain energy density: W = (mu/2)*(I1 - 3) - mu*ln(J) + (lambda/2)*(ln(J))^2
#[derive(Debug, Clone)]
pub struct NeoHookean {
    /// Shear modulus mu [Pa].
    pub mu: f64,
    /// First Lame parameter lambda [Pa].
    pub lambda: f64,
}

impl NeoHookean {
    /// Creates a new Neo-Hookean material.
    pub fn new(mu: f64, lambda: f64) -> Self {
        Self { mu, lambda }
    }
}

impl ConstitutiveModel for NeoHookean {
    /// Computes the Cauchy stress for a Neo-Hookean material.
    ///
    /// This is a stub for the full nonlinear formulation which requires
    /// the deformation gradient rather than small-strain tensor.
    fn stress(
        &self,
        _strain: &[[f64; 3]; 3],
        _state: &MaterialState,
    ) -> Result<[[f64; 3]; 3]> {
        // Approximate Neo-Hookean for small strain:
        // sigma = mu * (B - I) + lambda * ln(J) * I
        // In the small-strain limit: sigma ≈ 2*mu*epsilon + lambda*tr(epsilon)*I
        let mu = self.mu;
        let lambda = self.lambda;
        let strain = _strain;

        let trace_eps = strain[0][0] + strain[1][1] + strain[2][2];
        let mut stress = [[0.0_f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                stress[i][j] = 2.0 * mu * strain[i][j];
                if i == j {
                    stress[i][j] += lambda * trace_eps;
                }
            }
        }

        Ok(stress)
    }
}
