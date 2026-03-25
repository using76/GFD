//! Non-Newtonian viscosity models.

use crate::traits::{MaterialProperty, MaterialState};
use crate::Result;

/// Carreau viscosity model.
///
/// mu(gamma_dot) = mu_inf + (mu_0 - mu_inf) * (1 + (lambda * gamma_dot)^2)^((n-1)/2)
#[derive(Debug, Clone)]
pub struct CarreauModel {
    /// Infinite-shear-rate viscosity [Pa*s].
    pub mu_inf: f64,
    /// Zero-shear-rate viscosity [Pa*s].
    pub mu_0: f64,
    /// Relaxation time constant [s].
    pub lambda: f64,
    /// Power-law index (dimensionless).
    pub n: f64,
}

impl CarreauModel {
    /// Creates a new Carreau model.
    pub fn new(mu_inf: f64, mu_0: f64, lambda: f64, n: f64) -> Self {
        Self { mu_inf, mu_0, lambda, n }
    }
}

impl MaterialProperty for CarreauModel {
    fn evaluate(&self, state: &MaterialState) -> Result<f64> {
        let gamma = state.strain_rate;
        let lg = self.lambda * gamma;
        let factor = (1.0 + lg * lg).powf((self.n - 1.0) / 2.0);
        Ok(self.mu_inf + (self.mu_0 - self.mu_inf) * factor)
    }

    fn derivative_wrt(&self, state: &MaterialState, var: &str) -> Result<f64> {
        match var {
            "strain_rate" => {
                let gamma = state.strain_rate;
                let lg = self.lambda * gamma;
                let inner = 1.0 + lg * lg;
                let exponent = (self.n - 1.0) / 2.0;
                // d/d(gamma) of (mu_0 - mu_inf) * inner^exponent
                // = (mu_0 - mu_inf) * exponent * inner^(exponent-1) * 2*lambda^2*gamma
                let d = (self.mu_0 - self.mu_inf) * exponent
                    * inner.powf(exponent - 1.0)
                    * 2.0 * self.lambda * self.lambda * gamma;
                Ok(d)
            }
            _ => Ok(0.0),
        }
    }

    fn name(&self) -> &str {
        "carreau_viscosity"
    }

    fn units(&self) -> &str {
        "Pa*s"
    }
}

/// Herschel-Bulkley viscosity model (viscoplastic).
///
/// tau = tau_y + K * gamma_dot^n
/// Apparent viscosity: mu = tau_y / gamma_dot + K * gamma_dot^(n-1)
#[derive(Debug, Clone)]
pub struct HerschelBulkleyModel {
    /// Yield stress [Pa].
    pub tau_y: f64,
    /// Consistency index K [Pa*s^n].
    pub k: f64,
    /// Power-law index.
    pub n: f64,
}

impl HerschelBulkleyModel {
    /// Creates a new Herschel-Bulkley model.
    pub fn new(tau_y: f64, k: f64, n: f64) -> Self {
        Self { tau_y, k, n }
    }
}

impl MaterialProperty for HerschelBulkleyModel {
    fn evaluate(&self, state: &MaterialState) -> Result<f64> {
        let gamma = state.strain_rate.max(1.0e-20); // Regularization to avoid division by zero.
        let mu = self.tau_y / gamma + self.k * gamma.powf(self.n - 1.0);
        Ok(mu)
    }

    fn derivative_wrt(&self, state: &MaterialState, var: &str) -> Result<f64> {
        match var {
            "strain_rate" => {
                let gamma = state.strain_rate.max(1.0e-20);
                let d = -self.tau_y / (gamma * gamma)
                    + self.k * (self.n - 1.0) * gamma.powf(self.n - 2.0);
                Ok(d)
            }
            _ => Ok(0.0),
        }
    }

    fn name(&self) -> &str {
        "herschel_bulkley_viscosity"
    }

    fn units(&self) -> &str {
        "Pa*s"
    }
}

/// Cross viscosity model.
///
/// mu(gamma_dot) = mu_inf + (mu_0 - mu_inf) / (1 + (lambda * gamma_dot)^m)
#[derive(Debug, Clone)]
pub struct CrossModel {
    /// Infinite-shear-rate viscosity [Pa*s].
    pub mu_inf: f64,
    /// Zero-shear-rate viscosity [Pa*s].
    pub mu_0: f64,
    /// Time constant [s].
    pub lambda: f64,
    /// Rate constant (dimensionless).
    pub m: f64,
}

impl CrossModel {
    /// Creates a new Cross model.
    pub fn new(mu_inf: f64, mu_0: f64, lambda: f64, m: f64) -> Self {
        Self { mu_inf, mu_0, lambda, m }
    }
}

impl MaterialProperty for CrossModel {
    fn evaluate(&self, state: &MaterialState) -> Result<f64> {
        let gamma = state.strain_rate;
        let lg = self.lambda * gamma;
        let denom = 1.0 + lg.powf(self.m);
        Ok(self.mu_inf + (self.mu_0 - self.mu_inf) / denom)
    }

    fn derivative_wrt(&self, state: &MaterialState, var: &str) -> Result<f64> {
        match var {
            "strain_rate" => {
                let gamma = state.strain_rate;
                let lg = self.lambda * gamma;
                let lg_m = lg.powf(self.m);
                let denom = 1.0 + lg_m;
                // d/d(gamma) of (mu_0 - mu_inf) / (1 + (lambda*gamma)^m)
                // = -(mu_0 - mu_inf) * m * lambda * (lambda*gamma)^(m-1) / denom^2
                let d = -(self.mu_0 - self.mu_inf) * self.m * self.lambda
                    * lg.powf(self.m - 1.0)
                    / (denom * denom);
                Ok(d)
            }
            _ => Ok(0.0),
        }
    }

    fn name(&self) -> &str {
        "cross_viscosity"
    }

    fn units(&self) -> &str {
        "Pa*s"
    }
}
