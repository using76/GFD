//! Built-in turbulence models.

pub mod spalart_allmaras;
pub mod k_epsilon;
pub mod k_omega_sst;
pub mod les;
pub mod rsm;

use std::collections::HashMap;
use crate::model_template::{TurbulenceModelDef, ModelConstant};

/// Trait implemented by all turbulence models.
pub trait TurbulenceModel {
    /// Returns the human-readable name of the model.
    fn name(&self) -> &str;

    /// Returns the number of transport equations solved.
    fn num_equations(&self) -> usize;

    /// Computes the eddy (turbulent) viscosity.
    ///
    /// The meaning of `var1` and `var2` depends on the model:
    /// - k-epsilon: var1 = k, var2 = epsilon
    /// - k-omega:   var1 = k, var2 = omega
    /// - SA:        var1 = nu_tilde, var2 = nu (molecular)
    fn compute_eddy_viscosity(&self, var1: f64, var2: f64, rho: f64) -> f64;

    /// Returns the full model definition.
    fn get_definition(&self) -> &TurbulenceModelDef;

    /// Returns the model constants.
    fn get_constants(&self) -> &HashMap<String, ModelConstant>;
}
