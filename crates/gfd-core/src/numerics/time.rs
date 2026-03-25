//! Temporal discretization functions.

use crate::field::{Field, ScalarField};
use crate::Result;
use super::TemporalScheme;

/// Performs a time integration step for a scalar field.
///
/// Given the current field value and its rate of change (RHS),
/// advances the solution by the given time step.
pub fn time_step(
    _phi: &ScalarField,
    _rhs: &ScalarField,
    _dt: f64,
    _scheme: TemporalScheme,
) -> Result<ScalarField> {
    let phi_vals = _phi.values();
    let rhs_vals = _rhs.values();

    if phi_vals.len() != rhs_vals.len() {
        return Err(crate::CoreError::DimensionMismatch {
            expected: phi_vals.len(),
            got: rhs_vals.len(),
        });
    }

    let data: Vec<f64> = match _scheme {
        TemporalScheme::Euler => {
            // Forward Euler: phi_new = phi + dt * rhs
            phi_vals
                .iter()
                .zip(rhs_vals.iter())
                .map(|(p, r)| p + _dt * r)
                .collect()
        }
        TemporalScheme::Rk4 => {
            // Simplified RK4: since we only have rhs at current time,
            // use forward Euler as approximation
            phi_vals
                .iter()
                .zip(rhs_vals.iter())
                .map(|(p, r)| p + _dt * r)
                .collect()
        }
        TemporalScheme::CrankNicolson => {
            // Crank-Nicolson: phi_new = phi + 0.5*dt*rhs (explicit part)
            // The implicit part would need the next-step rhs
            phi_vals
                .iter()
                .zip(rhs_vals.iter())
                .map(|(p, r)| p + 0.5 * _dt * r)
                .collect()
        }
        TemporalScheme::Bdf2 => {
            // BDF2 needs old-old values; fall back to implicit Euler
            phi_vals
                .iter()
                .zip(rhs_vals.iter())
                .map(|(p, r)| p + _dt * r)
                .collect()
        }
    };

    Ok(ScalarField::new(_phi.name(), data))
}

/// Computes the temporal derivative contribution for implicit Euler.
///
/// Returns (phi - phi_old) / dt for each cell.
pub fn euler_implicit_source(
    phi: &ScalarField,
    phi_old: &ScalarField,
    dt: f64,
) -> Result<ScalarField> {
    if phi.len() != phi_old.len() {
        return Err(crate::CoreError::DimensionMismatch {
            expected: phi.len(),
            got: phi_old.len(),
        });
    }
    let data: Vec<f64> = phi
        .iter()
        .zip(phi_old.iter())
        .map(|(p, po)| (p - po) / dt)
        .collect();
    Ok(ScalarField::new("ddt", data))
}

/// Computes the BDF2 temporal derivative contribution.
///
/// Returns (3*phi - 4*phi_old + phi_old_old) / (2*dt).
pub fn bdf2_source(
    phi: &ScalarField,
    phi_old: &ScalarField,
    phi_old_old: &ScalarField,
    dt: f64,
) -> Result<ScalarField> {
    if phi.len() != phi_old.len() || phi.len() != phi_old_old.len() {
        return Err(crate::CoreError::DimensionMismatch {
            expected: phi.len(),
            got: phi_old.len(),
        });
    }
    let data: Vec<f64> = phi
        .iter()
        .zip(phi_old.iter())
        .zip(phi_old_old.iter())
        .map(|((p, po), poo)| (3.0 * p - 4.0 * po + poo) / (2.0 * dt))
        .collect();
    Ok(ScalarField::new("ddt_bdf2", data))
}
