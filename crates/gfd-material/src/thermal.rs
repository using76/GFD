//! Thermal property models.

use crate::traits::{MaterialProperty, MaterialState};
use crate::Result;

/// Constant thermal conductivity.
#[derive(Debug, Clone)]
pub struct ConstantConductivity {
    /// Thermal conductivity [W/(m*K)].
    pub k: f64,
}

impl ConstantConductivity {
    /// Creates a new constant conductivity model.
    pub fn new(k: f64) -> Self {
        Self { k }
    }
}

impl MaterialProperty for ConstantConductivity {
    fn evaluate(&self, _state: &MaterialState) -> Result<f64> {
        Ok(self.k)
    }

    fn derivative_wrt(&self, _state: &MaterialState, _var: &str) -> Result<f64> {
        Ok(0.0)
    }

    fn name(&self) -> &str {
        "constant_conductivity"
    }

    fn units(&self) -> &str {
        "W/(m*K)"
    }
}

/// Polynomial thermal conductivity: k(T) = sum(a_i * T^i).
#[derive(Debug, Clone)]
pub struct PolynomialConductivity {
    /// Polynomial coefficients [a_0, a_1, a_2, ...].
    /// k(T) = a_0 + a_1*T + a_2*T^2 + ...
    pub coefficients: Vec<f64>,
}

impl PolynomialConductivity {
    /// Creates a new polynomial conductivity model.
    pub fn new(coefficients: Vec<f64>) -> Self {
        Self { coefficients }
    }
}

impl MaterialProperty for PolynomialConductivity {
    fn evaluate(&self, state: &MaterialState) -> Result<f64> {
        let t = state.temperature;
        let mut result = 0.0;
        let mut t_power = 1.0;
        for &coeff in &self.coefficients {
            result += coeff * t_power;
            t_power *= t;
        }
        Ok(result)
    }

    fn derivative_wrt(&self, state: &MaterialState, var: &str) -> Result<f64> {
        match var {
            "temperature" => {
                let t = state.temperature;
                let mut result = 0.0;
                for (i, &coeff) in self.coefficients.iter().enumerate().skip(1) {
                    result += coeff * (i as f64) * t.powi(i as i32 - 1);
                }
                Ok(result)
            }
            _ => Ok(0.0),
        }
    }

    fn name(&self) -> &str {
        "polynomial_conductivity"
    }

    fn units(&self) -> &str {
        "W/(m*K)"
    }
}

/// Constant specific heat capacity.
#[derive(Debug, Clone)]
pub struct ConstantSpecificHeat {
    /// Specific heat capacity [J/(kg*K)].
    pub cp: f64,
}

impl ConstantSpecificHeat {
    /// Creates a new constant specific heat model.
    pub fn new(cp: f64) -> Self {
        Self { cp }
    }
}

impl MaterialProperty for ConstantSpecificHeat {
    fn evaluate(&self, _state: &MaterialState) -> Result<f64> {
        Ok(self.cp)
    }

    fn derivative_wrt(&self, _state: &MaterialState, _var: &str) -> Result<f64> {
        Ok(0.0)
    }

    fn name(&self) -> &str {
        "constant_specific_heat"
    }

    fn units(&self) -> &str {
        "J/(kg*K)"
    }
}
