//! Temporal discretization coefficients for FVM.

use serde::{Deserialize, Serialize};

/// Coefficients produced by temporal discretization.
///
/// These are added to the linear system:
///   a_P += a_p_time   (diagonal contribution)
///   b_P += source_time (source / RHS contribution)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TemporalCoefficients {
    /// Contribution to the diagonal coefficient a_P.
    pub a_p_time: f64,
    /// Contribution to the source term.
    pub source_time: f64,
}

/// Compute temporal coefficients using the implicit (backward) Euler scheme.
///
/// For ddt(rho * phi):
///   a_P_time   = rho * V / dt
///   source_time = rho * V / dt * phi_old
///
/// # Arguments
/// * `rho` - density
/// * `volume` - cell volume V
/// * `dt` - time step size
///
/// Note: `phi_old` is not passed here; it is folded into the source term
/// externally. The returned `source_time` represents the coefficient that
/// multiplies `phi_old`.
pub fn euler_implicit(rho: f64, volume: f64, dt: f64) -> TemporalCoefficients {
    let coeff = rho * volume / dt;
    TemporalCoefficients {
        a_p_time: coeff,
        source_time: coeff, // multiply by phi_old externally: b += coeff * phi_old
    }
}

/// Compute temporal coefficients using the second-order BDF2 scheme.
///
/// For ddt(rho * phi) with BDF2:
///   a_P_time   = 3/2 * rho * V / dt
///   source_time = rho * V / dt * (2 * phi_old - 0.5 * phi_old_old)
///
/// # Arguments
/// * `rho` - density
/// * `volume` - cell volume V
/// * `dt` - time step size
/// * `phi_old` - solution at previous time step (t - dt)
/// * `phi_old_old` - solution at time step (t - 2*dt)
pub fn bdf2(
    rho: f64,
    volume: f64,
    dt: f64,
    phi_old: f64,
    phi_old_old: f64,
) -> TemporalCoefficients {
    let base = rho * volume / dt;
    TemporalCoefficients {
        a_p_time: 1.5 * base,
        source_time: base * (2.0 * phi_old - 0.5 * phi_old_old),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_implicit_basic() {
        let tc = euler_implicit(1.0, 0.1, 0.01);
        assert!((tc.a_p_time - 10.0).abs() < 1e-12);
        assert!((tc.source_time - 10.0).abs() < 1e-12);
    }

    #[test]
    fn bdf2_basic() {
        let tc = bdf2(1.0, 0.1, 0.01, 300.0, 299.0);
        let base = 1.0 * 0.1 / 0.01; // 10.0
        assert!((tc.a_p_time - 15.0).abs() < 1e-12);
        let expected_src = base * (2.0 * 300.0 - 0.5 * 299.0);
        assert!((tc.source_time - expected_src).abs() < 1e-10);
    }
}
