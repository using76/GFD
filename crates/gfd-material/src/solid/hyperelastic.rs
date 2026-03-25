//! Hyperelastic constitutive models.
//!
//! Provides the compressible Neo-Hookean model suitable for large-deformation
//! solid mechanics. The input `strain` parameter to [`ConstitutiveModel::stress`]
//! is interpreted as the **displacement gradient** H_ij = du_i / dx_j, from
//! which the deformation gradient F = I + H is formed.

use crate::traits::{ConstitutiveModel, MaterialState};
use crate::{MaterialError, Result};

/// Neo-Hookean hyperelastic material.
///
/// Strain energy density (compressible form):
///
///   W = (mu/2) * (I1 - 3) - mu * ln(J) + (lambda/2) * (ln J)^2
///
/// where I1 = tr(C), C = F^T F is the right Cauchy-Green deformation tensor,
/// J = det(F), and F = I + grad(u) is the deformation gradient.
///
/// The second Piola-Kirchhoff stress is:
///
///   S = mu * (I - C^{-1}) + lambda * ln(J) * C^{-1}
///
/// and the Cauchy stress is obtained via the push-forward:
///
///   sigma = (1/J) * F * S * F^T
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

    /// Creates a Neo-Hookean material from Young's modulus E and Poisson's ratio nu.
    pub fn from_engineering(e: f64, nu: f64) -> Self {
        let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let mu = e / (2.0 * (1.0 + nu));
        Self { mu, lambda }
    }
}

// ---------------------------------------------------------------------------
// 3x3 matrix helpers (column-major style with [[f64; 3]; 3])
// ---------------------------------------------------------------------------

/// Identity matrix.
const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Determinant of a 3x3 matrix.
fn det3(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Inverse of a 3x3 matrix. Returns `None` if the matrix is singular.
fn inv3(m: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let d = det3(m);
    if d.abs() < 1e-30 {
        return None;
    }
    let inv_d = 1.0 / d;

    Some([
        [
            inv_d * (m[1][1] * m[2][2] - m[1][2] * m[2][1]),
            inv_d * (m[0][2] * m[2][1] - m[0][1] * m[2][2]),
            inv_d * (m[0][1] * m[1][2] - m[0][2] * m[1][1]),
        ],
        [
            inv_d * (m[1][2] * m[2][0] - m[1][0] * m[2][2]),
            inv_d * (m[0][0] * m[2][2] - m[0][2] * m[2][0]),
            inv_d * (m[0][2] * m[1][0] - m[0][0] * m[1][2]),
        ],
        [
            inv_d * (m[1][0] * m[2][1] - m[1][1] * m[2][0]),
            inv_d * (m[0][1] * m[2][0] - m[0][0] * m[2][1]),
            inv_d * (m[0][0] * m[1][1] - m[0][1] * m[1][0]),
        ],
    ])
}

/// Matrix transpose: B = A^T.
fn transpose3(a: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut b = [[0.0_f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            b[i][j] = a[j][i];
        }
    }
    b
}

/// Matrix multiplication: C = A * B.
fn matmul3(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut c = [[0.0_f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let mut sum = 0.0;
            for k in 0..3 {
                sum += a[i][k] * b[k][j];
            }
            c[i][j] = sum;
        }
    }
    c
}

impl ConstitutiveModel for NeoHookean {
    /// Computes the Cauchy stress tensor for a compressible Neo-Hookean material.
    ///
    /// # Input
    ///
    /// The `strain` parameter is interpreted as the **displacement gradient**:
    ///
    ///   H_ij = du_i / dx_j
    ///
    /// From which the deformation gradient is formed as F = I + H.
    ///
    /// # Formulation
    ///
    /// 1. F = I + H  (deformation gradient)
    /// 2. J = det(F)
    /// 3. C = F^T F  (right Cauchy-Green tensor)
    /// 4. C^{-1}     (inverse of C)
    /// 5. S = mu * (I - C^{-1}) + lambda * ln(J) * C^{-1}  (2nd Piola-Kirchhoff)
    /// 6. sigma = (1/J) * F * S * F^T   (Cauchy stress via push-forward)
    fn stress(
        &self,
        strain: &[[f64; 3]; 3],
        _state: &MaterialState,
    ) -> Result<[[f64; 3]; 3]> {
        let mu = self.mu;
        let lambda = self.lambda;
        let h = strain; // displacement gradient

        // F = I + H
        let mut f = IDENTITY;
        for i in 0..3 {
            for j in 0..3 {
                f[i][j] += h[i][j];
            }
        }

        // J = det(F)
        let j_det = det3(&f);
        if j_det <= 0.0 {
            return Err(MaterialError::ConstitutiveError(
                format!(
                    "Non-physical deformation gradient: det(F) = {} <= 0. \
                     The element is likely inverted.",
                    j_det
                ),
            ));
        }

        // C = F^T F (right Cauchy-Green deformation tensor)
        let ft = transpose3(&f);
        let c = matmul3(&ft, &f);

        // C^{-1}
        let c_inv = inv3(&c).ok_or_else(|| {
            MaterialError::ConstitutiveError(
                "Right Cauchy-Green tensor C is singular".to_string(),
            )
        })?;

        // Second Piola-Kirchhoff stress:
        // S = mu * (I - C^{-1}) + lambda * ln(J) * C^{-1}
        let ln_j = j_det.ln();
        let mut s_pk2 = [[0.0_f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                let delta = if i == j { 1.0 } else { 0.0 };
                s_pk2[i][j] = mu * (delta - c_inv[i][j]) + lambda * ln_j * c_inv[i][j];
            }
        }

        // Cauchy stress: sigma = (1/J) * F * S * F^T
        let f_s = matmul3(&f, &s_pk2);
        let sigma_full = matmul3(&f_s, &ft);

        let inv_j = 1.0 / j_det;
        let mut sigma = [[0.0_f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                sigma[i][j] = inv_j * sigma_full[i][j];
            }
        }

        Ok(sigma)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::MaterialState;

    fn default_state() -> MaterialState {
        MaterialState::default()
    }

    #[test]
    fn test_zero_displacement_gradient() {
        // H = 0 => F = I => C = I => J = 1 => ln(J) = 0
        // S = mu*(I - I) + lambda*0*I = 0
        // sigma = 0
        let mat = NeoHookean::new(80.0e9, 120.0e9);
        let h = [[0.0; 3]; 3];
        let sigma = mat.stress(&h, &default_state()).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                assert!(
                    sigma[i][j].abs() < 1e-6,
                    "sigma[{}][{}] = {} should be zero",
                    i, j, sigma[i][j]
                );
            }
        }
    }

    #[test]
    fn test_uniaxial_stretch_symmetry() {
        // Uniaxial stretch: F = diag(1.1, 1, 1), H = diag(0.1, 0, 0)
        let mat = NeoHookean::new(80.0e9, 120.0e9);
        let mut h = [[0.0_f64; 3]; 3];
        h[0][0] = 0.1;
        let sigma = mat.stress(&h, &default_state()).unwrap();

        // sigma should be diagonal (no shear for diagonal F).
        assert!(sigma[0][1].abs() < 1e-3, "off-diagonal should be ~0");
        assert!(sigma[0][2].abs() < 1e-3, "off-diagonal should be ~0");
        assert!(sigma[1][2].abs() < 1e-3, "off-diagonal should be ~0");

        // sigma_11 > 0 (tension in stretch direction)
        assert!(sigma[0][0] > 0.0, "sigma_11 should be positive for tension");

        // sigma_22 = sigma_33 (transverse isotropy)
        assert!(
            (sigma[1][1] - sigma[2][2]).abs() < 1e-3,
            "sigma_22 should equal sigma_33"
        );
    }

    #[test]
    fn test_hydrostatic_compression() {
        // Uniform compression: F = lambda_s * I where lambda_s < 1.
        // H = (lambda_s - 1) * I
        let mat = NeoHookean::new(80.0e9, 120.0e9);
        let stretch = 0.95; // 5% compression
        let mut h = [[0.0_f64; 3]; 3];
        for i in 0..3 {
            h[i][i] = stretch - 1.0; // -0.05
        }
        let sigma = mat.stress(&h, &default_state()).unwrap();

        // Should be approximately hydrostatic (all normal stresses equal).
        assert!(
            (sigma[0][0] - sigma[1][1]).abs() < 1e-3,
            "should be hydrostatic"
        );
        assert!(
            (sigma[1][1] - sigma[2][2]).abs() < 1e-3,
            "should be hydrostatic"
        );

        // All normal stresses should be compressive (negative).
        assert!(sigma[0][0] < 0.0, "should be compressive");

        // Off-diagonal should be zero.
        for i in 0..3 {
            for j in 0..3 {
                if i != j {
                    assert!(
                        sigma[i][j].abs() < 1e-3,
                        "off-diagonal sigma[{}][{}] = {} should be zero",
                        i, j, sigma[i][j]
                    );
                }
            }
        }
    }

    #[test]
    fn test_small_strain_limit() {
        // For infinitesimal strains, Neo-Hookean should reduce to linear
        // elasticity: sigma ≈ 2*mu*eps + lambda*tr(eps)*I
        let mu = 80.0e9;
        let lambda = 120.0e9;
        let mat = NeoHookean::new(mu, lambda);

        // Very small uniaxial strain: eps_11 = 1e-6
        let eps = 1e-6;
        let mut h = [[0.0_f64; 3]; 3];
        h[0][0] = eps;

        let sigma = mat.stress(&h, &default_state()).unwrap();

        // Linear prediction: sigma_11 = 2*mu*eps + lambda*eps = (2*mu + lambda)*eps
        let sigma_11_linear = (2.0 * mu + lambda) * eps;
        // sigma_22 = sigma_33 = lambda * eps
        let sigma_22_linear = lambda * eps;

        let tol = 1e-3; // relative tolerance (nonlinear terms are O(eps^2))
        assert!(
            ((sigma[0][0] - sigma_11_linear) / sigma_11_linear).abs() < tol,
            "sigma_11: got {}, expected {}",
            sigma[0][0], sigma_11_linear
        );
        assert!(
            ((sigma[1][1] - sigma_22_linear) / sigma_22_linear).abs() < tol,
            "sigma_22: got {}, expected {}",
            sigma[1][1], sigma_22_linear
        );
    }

    #[test]
    fn test_simple_shear() {
        // Simple shear: F = [[1, gamma, 0], [0, 1, 0], [0, 0, 1]]
        let mu = 1.0e6;
        let lambda = 2.0e6;
        let mat = NeoHookean::new(mu, lambda);

        let gamma = 0.1;
        let mut h = [[0.0_f64; 3]; 3];
        h[0][1] = gamma; // du/dy

        let sigma = mat.stress(&h, &default_state()).unwrap();

        // For simple shear, J = 1, so ln(J) = 0.
        // B = F * F^T, and sigma = (mu/J)*(B - I) + (lambda*ln(J)/J)*I
        // With J=1: sigma = mu*(B - I)
        // B = [[1+gamma^2, gamma, 0], [gamma, 1, 0], [0, 0, 1]]
        // sigma_12 = mu * gamma
        assert!(
            (sigma[0][1] - mu * gamma).abs() / (mu * gamma).abs() < 0.01,
            "sigma_12: got {}, expected {}",
            sigma[0][1], mu * gamma
        );
    }

    #[test]
    fn test_negative_jacobian_error() {
        // Inverted element: det(F) < 0
        let mat = NeoHookean::new(1.0e6, 2.0e6);
        let mut h = [[0.0_f64; 3]; 3];
        h[0][0] = -2.0; // F_11 = -1.0 => det(F) = -1
        let result = mat.stress(&h, &default_state());
        assert!(result.is_err(), "should error on negative J");
    }

    #[test]
    fn test_from_engineering() {
        let e = 200.0e9;
        let nu = 0.3;
        let mat = NeoHookean::from_engineering(e, nu);

        let expected_lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let expected_mu = e / (2.0 * (1.0 + nu));

        assert!((mat.lambda - expected_lambda).abs() < 1.0);
        assert!((mat.mu - expected_mu).abs() < 1.0);
    }

    #[test]
    fn test_det3() {
        assert!((det3(&IDENTITY) - 1.0).abs() < 1e-15);
        let m = [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 4.0]];
        assert!((det3(&m) - 24.0).abs() < 1e-12);
    }

    #[test]
    fn test_inv3_identity() {
        let inv = inv3(&IDENTITY).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (inv[i][j] - expected).abs() < 1e-15,
                    "inv[{}][{}] = {}",
                    i, j, inv[i][j]
                );
            }
        }
    }

    #[test]
    fn test_inv3_roundtrip() {
        let m = [[1.0, 2.0, 0.5], [0.3, 3.0, 0.1], [0.2, 0.4, 2.0]];
        let m_inv = inv3(&m).unwrap();
        let product = matmul3(&m, &m_inv);
        for i in 0..3 {
            for j in 0..3 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (product[i][j] - expected).abs() < 1e-10,
                    "M * M^-1 [{},{}] = {} (expected {})",
                    i, j, product[i][j], expected
                );
            }
        }
    }
}
