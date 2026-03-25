//! Newtonian viscosity models.

use crate::traits::{MaterialProperty, MaterialState};
use crate::{MaterialError, Result};

/// Constant dynamic viscosity.
#[derive(Debug, Clone)]
pub struct ConstantViscosity {
    /// Dynamic viscosity [Pa*s].
    pub mu: f64,
}

impl ConstantViscosity {
    /// Creates a new constant viscosity model.
    pub fn new(mu: f64) -> Self {
        Self { mu }
    }
}

impl MaterialProperty for ConstantViscosity {
    fn evaluate(&self, _state: &MaterialState) -> Result<f64> {
        Ok(self.mu)
    }

    fn derivative_wrt(&self, _state: &MaterialState, _var: &str) -> Result<f64> {
        Ok(0.0) // Constant: all derivatives are zero.
    }

    fn name(&self) -> &str {
        "constant_viscosity"
    }

    fn units(&self) -> &str {
        "Pa*s"
    }
}

/// Sutherland's law for temperature-dependent viscosity.
///
/// mu(T) = mu_ref * (T / T_ref)^(3/2) * (T_ref + S) / (T + S)
#[derive(Debug, Clone)]
pub struct SutherlandViscosity {
    /// Reference viscosity [Pa*s].
    pub mu_ref: f64,
    /// Reference temperature [K].
    pub t_ref: f64,
    /// Sutherland constant [K].
    pub s: f64,
}

impl SutherlandViscosity {
    /// Creates a new Sutherland viscosity model.
    pub fn new(mu_ref: f64, t_ref: f64, s: f64) -> Self {
        Self { mu_ref, t_ref, s }
    }

    /// Creates the Sutherland model for air.
    pub fn air() -> Self {
        Self {
            mu_ref: 1.716e-5,
            t_ref: 273.15,
            s: 110.4,
        }
    }
}

impl MaterialProperty for SutherlandViscosity {
    fn evaluate(&self, state: &MaterialState) -> Result<f64> {
        let t = state.temperature;
        if t <= 0.0 {
            return Err(MaterialError::InvalidState(
                "Temperature must be positive for Sutherland's law".to_string(),
            ));
        }
        let ratio = t / self.t_ref;
        let mu = self.mu_ref * ratio.powf(1.5) * (self.t_ref + self.s) / (t + self.s);
        Ok(mu)
    }

    fn derivative_wrt(&self, state: &MaterialState, var: &str) -> Result<f64> {
        match var {
            "temperature" => {
                let t = state.temperature;
                if t <= 0.0 {
                    return Err(MaterialError::InvalidState(
                        "Temperature must be positive".to_string(),
                    ));
                }
                // d/dT [mu_ref * (T/T_ref)^1.5 * (T_ref+S)/(T+S)]
                // Using product rule: let f = (T/T_ref)^1.5, g = (T_ref+S)/(T+S)
                // f' = 1.5 * (T/T_ref)^0.5 / T_ref
                // g' = -(T_ref+S)/(T+S)^2
                let ratio = t / self.t_ref;
                let f = ratio.powf(1.5);
                let g = (self.t_ref + self.s) / (t + self.s);
                let fp = 1.5 * ratio.powf(0.5) / self.t_ref;
                let gp = -(self.t_ref + self.s) / ((t + self.s) * (t + self.s));
                Ok(self.mu_ref * (fp * g + f * gp))
            }
            _ => Ok(0.0),
        }
    }

    fn name(&self) -> &str {
        "sutherland_viscosity"
    }

    fn units(&self) -> &str {
        "Pa*s"
    }
}

/// Power-law viscosity model.
///
/// mu = K * strain_rate^(n-1)
#[derive(Debug, Clone)]
pub struct PowerLawViscosity {
    /// Consistency index K [Pa*s^n].
    pub k: f64,
    /// Power-law index n (dimensionless).
    pub n: f64,
}

impl PowerLawViscosity {
    /// Creates a new power-law viscosity model.
    pub fn new(k: f64, n: f64) -> Self {
        Self { k, n }
    }
}

impl MaterialProperty for PowerLawViscosity {
    fn evaluate(&self, state: &MaterialState) -> Result<f64> {
        let gamma = state.strain_rate.max(1.0e-20); // Avoid zero strain rate.
        Ok(self.k * gamma.powf(self.n - 1.0))
    }

    fn derivative_wrt(&self, state: &MaterialState, var: &str) -> Result<f64> {
        match var {
            "strain_rate" => {
                let gamma = state.strain_rate.max(1.0e-20);
                Ok(self.k * (self.n - 1.0) * gamma.powf(self.n - 2.0))
            }
            _ => Ok(0.0),
        }
    }

    fn name(&self) -> &str {
        "power_law_viscosity"
    }

    fn units(&self) -> &str {
        "Pa*s"
    }
}
