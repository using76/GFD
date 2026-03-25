//! Linear elastic constitutive model.

use crate::traits::{ConstitutiveModel, MaterialState};
use crate::Result;

/// Isotropic linear elastic material.
///
/// Hooke's law: sigma_ij = lambda * delta_ij * eps_kk + 2 * mu * eps_ij
#[derive(Debug, Clone)]
pub struct LinearElastic {
    /// Young's modulus E [Pa].
    pub e: f64,
    /// Poisson's ratio nu (dimensionless).
    pub nu: f64,
}

impl LinearElastic {
    /// Creates a new linear elastic material.
    pub fn new(e: f64, nu: f64) -> Self {
        Self { e, nu }
    }

    /// Creates a linear elastic model for structural steel.
    pub fn steel() -> Self {
        Self::new(200.0e9, 0.3)
    }

    /// Creates a linear elastic model for aluminum.
    pub fn aluminum() -> Self {
        Self::new(69.0e9, 0.33)
    }

    /// Bulk modulus K = E / (3 * (1 - 2*nu)).
    pub fn bulk_modulus(&self) -> f64 {
        self.e / (3.0 * (1.0 - 2.0 * self.nu))
    }

    /// Shear modulus G = E / (2 * (1 + nu)).
    pub fn shear_modulus(&self) -> f64 {
        self.e / (2.0 * (1.0 + self.nu))
    }

    /// First Lame parameter lambda = E * nu / ((1 + nu) * (1 - 2*nu)).
    pub fn lame_lambda(&self) -> f64 {
        self.e * self.nu / ((1.0 + self.nu) * (1.0 - 2.0 * self.nu))
    }

    /// Second Lame parameter mu = G = E / (2 * (1 + nu)).
    pub fn lame_mu(&self) -> f64 {
        self.shear_modulus()
    }
}

impl ConstitutiveModel for LinearElastic {
    /// Computes the Cauchy stress tensor for isotropic linear elasticity.
    ///
    /// sigma_ij = lambda * delta_ij * eps_kk + 2 * mu * eps_ij
    fn stress(
        &self,
        strain: &[[f64; 3]; 3],
        _state: &MaterialState,
    ) -> Result<[[f64; 3]; 3]> {
        let lambda = self.lame_lambda();
        let mu = self.lame_mu();

        // Volumetric strain: eps_kk = trace(eps)
        let trace = strain[0][0] + strain[1][1] + strain[2][2];

        let mut sigma = [[0.0_f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                let delta_ij = if i == j { 1.0 } else { 0.0 };
                sigma[i][j] = lambda * delta_ij * trace + 2.0 * mu * strain[i][j];
            }
        }

        Ok(sigma)
    }
}
