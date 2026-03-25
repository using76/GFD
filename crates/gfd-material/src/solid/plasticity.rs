//! Plasticity models.

/// Von Mises yield criterion parameters.
#[derive(Debug, Clone)]
pub struct VonMises {
    /// Initial yield stress [Pa].
    pub yield_stress: f64,
    /// Isotropic hardening modulus [Pa].
    pub hardening_modulus: f64,
}

impl VonMises {
    /// Creates a new Von Mises plasticity model.
    pub fn new(yield_stress: f64, hardening_modulus: f64) -> Self {
        Self {
            yield_stress,
            hardening_modulus,
        }
    }

    /// Returns the current yield stress accounting for isotropic hardening.
    ///
    /// sigma_y = sigma_y0 + H * eps_p
    pub fn current_yield_stress(&self, equivalent_plastic_strain: f64) -> f64 {
        self.yield_stress + self.hardening_modulus * equivalent_plastic_strain
    }
}

/// Computes the von Mises equivalent stress from a 3x3 Cauchy stress tensor.
///
/// sigma_vm = sqrt(0.5 * ((s11-s22)^2 + (s22-s33)^2 + (s33-s11)^2 + 6*(s12^2+s23^2+s13^2)))
pub fn compute_von_mises_stress(stress: &[[f64; 3]; 3]) -> f64 {
    let s11 = stress[0][0];
    let s22 = stress[1][1];
    let s33 = stress[2][2];
    let s12 = stress[0][1];
    let s23 = stress[1][2];
    let s13 = stress[0][2];

    let term1 = (s11 - s22).powi(2);
    let term2 = (s22 - s33).powi(2);
    let term3 = (s33 - s11).powi(2);
    let shear = 6.0 * (s12 * s12 + s23 * s23 + s13 * s13);

    (0.5 * (term1 + term2 + term3 + shear)).sqrt()
}
