//! Wall function computations for near-wall turbulence treatment.

use crate::{TurbulenceError, Result};

/// von Karman constant (default).
pub const KAPPA_DEFAULT: f64 = 0.41;

/// Log-law additive constant (default, smooth wall).
pub const E_DEFAULT: f64 = 9.793;

/// Threshold y+ for the viscous sublayer / log-law crossover.
pub const Y_PLUS_CROSSOVER: f64 = 11.225;

/// Computes y+ from friction velocity.
///
/// y+ = u_tau * y / nu
#[inline]
pub fn compute_y_plus(u_tau: f64, y: f64, nu: f64) -> f64 {
    if nu <= 0.0 {
        return 0.0;
    }
    u_tau * y / nu
}

/// Computes the friction velocity u_tau using Newton's method to solve
/// the implicit log-law equation:
///
///   u_p / u_tau = (1/kappa) * ln(E * u_tau * y_p / nu)
///
/// which can be rewritten as:
///
///   f(u_tau) = u_p / u_tau - (1/kappa) * ln(E * y_p * u_tau / nu) = 0
///
/// # Arguments
///
/// - `u_p`: velocity magnitude at the near-wall cell center
/// - `y_p`: wall-normal distance of the cell center from the wall
/// - `nu`: molecular kinematic viscosity
/// - `kappa`: von Karman constant (0.41)
/// - `e_const`: log-law constant E (9.793 for smooth walls)
///
/// # Errors
///
/// Returns an error if Newton iteration fails to converge.
pub fn compute_u_tau(
    u_p: f64,
    y_p: f64,
    nu: f64,
    kappa: f64,
    e_const: f64,
) -> Result<f64> {
    if u_p <= 0.0 || y_p <= 0.0 || nu <= 0.0 {
        return Ok(0.0);
    }

    // Initial guess: assume viscous sublayer u_tau = sqrt(nu * u_p / y_p)
    let mut u_tau = (nu * u_p / y_p).sqrt();
    if u_tau < 1.0e-15 {
        u_tau = 1.0e-6;
    }

    let max_iter = 50;
    let tolerance = 1.0e-8;

    for _iter in 0..max_iter {
        let y_plus = u_tau * y_p / nu;

        if y_plus < Y_PLUS_CROSSOVER {
            // In the viscous sublayer: u+ = y+, so u_p = u_tau * y+
            // => u_p = u_tau^2 * y_p / nu => u_tau = sqrt(nu * u_p / y_p)
            u_tau = (nu * u_p / y_p).sqrt();
            return Ok(u_tau);
        }

        // f(u_tau) = u_p / u_tau - (1/kappa) * ln(E * y_p * u_tau / nu)
        let f = u_p / u_tau - (1.0 / kappa) * (e_const * y_p * u_tau / nu).ln();

        // f'(u_tau) = -u_p / u_tau^2 - 1 / (kappa * u_tau)
        let df = -u_p / (u_tau * u_tau) - 1.0 / (kappa * u_tau);

        if df.abs() < 1.0e-30 {
            return Err(TurbulenceError::WallFunctionError(
                "Zero derivative in Newton iteration".to_string(),
            ));
        }

        let delta = f / df;
        u_tau -= delta;

        // Ensure u_tau stays positive
        if u_tau <= 0.0 {
            u_tau = 1.0e-6;
        }

        if delta.abs() < tolerance * u_tau {
            return Ok(u_tau);
        }
    }

    Err(TurbulenceError::WallFunctionError(format!(
        "Newton iteration for u_tau did not converge after {} iterations (u_p={}, y_p={}, nu={})",
        max_iter, u_p, y_p, nu
    )))
}

/// Computes u+ from the standard wall function (log-law or viscous sublayer).
///
/// Returns the non-dimensional velocity:
/// - Viscous sublayer (y+ < 11.225): u+ = y+
/// - Log-law region (y+ >= 11.225): u+ = (1/kappa) * ln(E * y+)
pub fn compute_u_plus(y_plus: f64, kappa: f64, e_const: f64) -> f64 {
    if y_plus < Y_PLUS_CROSSOVER {
        y_plus
    } else {
        (1.0 / kappa) * (e_const * y_plus).ln()
    }
}
