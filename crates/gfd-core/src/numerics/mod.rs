//! Numerical schemes for convection, diffusion, and temporal discretization.

pub mod convection;
pub mod diffusion;
pub mod time;

use serde::{Deserialize, Serialize};

/// TVD (Total Variation Diminishing) flux limiters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TVDLimiter {
    /// Van Leer limiter: (r + |r|) / (1 + |r|).
    VanLeer,
    /// MinMod limiter: max(0, min(1, r)).
    MinMod,
    /// Superbee limiter: max(0, min(2r, 1), min(r, 2)).
    Superbee,
    /// Van Albada limiter: (r^2 + r) / (r^2 + 1).
    VanAlbada,
}

/// Convective flux discretization schemes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ConvectionScheme {
    /// First-order upwind (very diffusive, unconditionally stable).
    FirstOrderUpwind,
    /// Second-order upwind with gradient reconstruction.
    SecondOrderUpwind,
    /// Central differencing (second-order, may be unstable).
    Central,
    /// QUICK (Quadratic Upstream Interpolation for Convective Kinematics).
    Quick,
    /// TVD scheme with the specified flux limiter.
    Tvd { limiter: TVDLimiter },
}

/// Diffusive flux discretization schemes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffusionScheme {
    /// Central differencing for diffusion (standard second-order).
    Central,
    /// Over-relaxed correction approach for non-orthogonal meshes.
    OverRelaxed,
    /// Minimum correction approach for non-orthogonal meshes.
    MinimumCorrection,
    /// Orthogonal correction approach.
    OrthogonalCorrection,
}

/// Temporal discretization schemes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalScheme {
    /// Forward Euler (first-order, explicit).
    Euler,
    /// Backward Differentiation Formula, order 2 (implicit, second-order).
    Bdf2,
    /// Crank-Nicolson (implicit, second-order, trapezoidal rule).
    CrankNicolson,
    /// Classical Runge-Kutta 4th order (explicit).
    Rk4,
}

impl TVDLimiter {
    /// Evaluates the limiter function for the given gradient ratio r.
    pub fn evaluate(&self, r: f64) -> f64 {
        match self {
            TVDLimiter::VanLeer => {
                if r <= 0.0 {
                    0.0
                } else {
                    (r + r.abs()) / (1.0 + r.abs())
                }
            }
            TVDLimiter::MinMod => {
                f64::max(0.0, f64::min(1.0, r))
            }
            TVDLimiter::Superbee => {
                let a = f64::min(2.0 * r, 1.0);
                let b = f64::min(r, 2.0);
                f64::max(0.0, f64::max(a, b))
            }
            TVDLimiter::VanAlbada => {
                if r <= 0.0 {
                    0.0
                } else {
                    (r * r + r) / (r * r + 1.0)
                }
            }
        }
    }
}
