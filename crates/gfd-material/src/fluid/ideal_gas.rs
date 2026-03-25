//! Ideal gas density model.

use crate::traits::{MaterialProperty, MaterialState};
use crate::{MaterialError, Result};

/// Ideal gas density: rho = p * M / (R * T).
#[derive(Debug, Clone)]
pub struct IdealGasDensity {
    /// Molecular weight [kg/mol].
    pub molecular_weight: f64,
    /// Universal gas constant [J/(mol*K)].
    pub r: f64,
}

impl IdealGasDensity {
    /// Creates a new ideal gas density model.
    pub fn new(molecular_weight: f64) -> Self {
        Self {
            molecular_weight,
            r: 8.314,
        }
    }

    /// Creates an ideal gas model for air (M = 0.02897 kg/mol).
    pub fn air() -> Self {
        Self::new(0.02897)
    }
}

impl MaterialProperty for IdealGasDensity {
    fn evaluate(&self, state: &MaterialState) -> Result<f64> {
        let t = state.temperature;
        let p = state.pressure;
        if t <= 0.0 {
            return Err(MaterialError::InvalidState(
                "Temperature must be positive for ideal gas law".to_string(),
            ));
        }
        Ok(p * self.molecular_weight / (self.r * t))
    }

    fn derivative_wrt(&self, state: &MaterialState, var: &str) -> Result<f64> {
        let t = state.temperature;
        let p = state.pressure;
        if t <= 0.0 {
            return Err(MaterialError::InvalidState(
                "Temperature must be positive".to_string(),
            ));
        }
        match var {
            "temperature" => {
                // d(rho)/dT = -p * M / (R * T^2)
                Ok(-p * self.molecular_weight / (self.r * t * t))
            }
            "pressure" => {
                // d(rho)/dp = M / (R * T)
                Ok(self.molecular_weight / (self.r * t))
            }
            _ => Ok(0.0),
        }
    }

    fn name(&self) -> &str {
        "ideal_gas_density"
    }

    fn units(&self) -> &str {
        "kg/m^3"
    }
}
