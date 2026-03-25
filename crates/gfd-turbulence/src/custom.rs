//! Custom turbulence models loaded from JSON.

use std::collections::HashMap;
use crate::model_template::{TurbulenceModelDef, ModelConstant};
use crate::builtin::TurbulenceModel;
use crate::{TurbulenceError, Result};

/// A user-defined turbulence model loaded from a JSON definition.
#[derive(Debug, Clone)]
pub struct CustomTurbulenceModel {
    /// The underlying model definition.
    definition: TurbulenceModelDef,
}

impl CustomTurbulenceModel {
    /// Creates a custom model from an existing definition.
    pub fn from_definition(definition: TurbulenceModelDef) -> Self {
        Self { definition }
    }
}

/// Loads a custom turbulence model from a JSON string.
///
/// The JSON should deserialize into a `TurbulenceModelDef`.
///
/// # Errors
///
/// Returns `TurbulenceError::JsonError` if deserialization fails,
/// or `TurbulenceError::CustomModelError` if the definition is empty.
pub fn load_custom_model(json: &str) -> Result<CustomTurbulenceModel> {
    let definition: TurbulenceModelDef = serde_json::from_str(json)?;
    if definition.name.is_empty() {
        return Err(TurbulenceError::CustomModelError(
            "Model name must not be empty".to_string(),
        ));
    }
    Ok(CustomTurbulenceModel { definition })
}

impl TurbulenceModel for CustomTurbulenceModel {
    fn name(&self) -> &str {
        &self.definition.name
    }

    fn num_equations(&self) -> usize {
        self.definition.num_equations
    }

    /// Computes eddy viscosity for the custom model.
    ///
    /// Since the expression is user-defined, this requires an expression
    /// evaluator. Currently returns a placeholder value.
    fn compute_eddy_viscosity(&self, _var1: f64, _var2: f64, _rho: f64) -> f64 {
        // Full implementation would parse self.definition.eddy_viscosity
        // through gfd-expression and evaluate at runtime.
        // Placeholder: return a simple k-epsilon-like eddy viscosity
        // mu_t = C_mu * rho * var1^2 / var2  (where var1=k, var2=epsilon)
        let c_mu = 0.09;
        if _var2.abs() > 1e-30 {
            c_mu * _rho * _var1 * _var1 / _var2
        } else {
            0.0
        }
    }

    fn get_definition(&self) -> &TurbulenceModelDef {
        &self.definition
    }

    fn get_constants(&self) -> &HashMap<String, ModelConstant> {
        &self.definition.constants
    }
}
