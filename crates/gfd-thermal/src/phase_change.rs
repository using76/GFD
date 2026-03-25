//! Phase change (melting/solidification) models.

use gfd_core::{ScalarField, UnstructuredMesh};
use crate::Result;

/// Enthalpy-porosity method for melting and solidification.
///
/// Models the mushy zone as a porous medium with the liquid fraction
/// varying between 0 (fully solid) and 1 (fully liquid).
pub struct EnthalpyPorosity {
    /// Solidus temperature [K].
    pub solidus_temperature: f64,
    /// Liquidus temperature [K].
    pub liquidus_temperature: f64,
    /// Latent heat of fusion [J/kg].
    pub latent_heat: f64,
    /// Mushy zone constant (Carman-Kozeny parameter).
    pub mushy_constant: f64,
}

impl EnthalpyPorosity {
    /// Creates a new enthalpy-porosity phase change model.
    pub fn new(
        solidus_temperature: f64,
        liquidus_temperature: f64,
        latent_heat: f64,
    ) -> Self {
        Self {
            solidus_temperature,
            liquidus_temperature,
            latent_heat,
            mushy_constant: 1.0e5,
        }
    }

    /// Computes the liquid fraction from the temperature field.
    pub fn compute_liquid_fraction(
        &self,
        temperature: &ScalarField,
        _mesh: &UnstructuredMesh,
    ) -> Result<ScalarField> {
        let values = temperature.values();
        let t_s = self.solidus_temperature;
        let t_l = self.liquidus_temperature;

        let fl: Vec<f64> = values
            .iter()
            .map(|&t| {
                if t_l <= t_s {
                    // Degenerate case: isothermal phase change
                    if t >= t_s { 1.0 } else { 0.0 }
                } else {
                    ((t - t_s) / (t_l - t_s)).clamp(0.0, 1.0)
                }
            })
            .collect();

        Ok(ScalarField::new("liquid_fraction", fl))
    }
}
