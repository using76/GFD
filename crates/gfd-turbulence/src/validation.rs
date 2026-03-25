//! Turbulence model validation utilities.

use crate::builtin::TurbulenceModel;

/// Validates a turbulence model, returning a list of warning/error messages.
///
/// Checks performed:
/// - All constants have valid (finite, non-NaN) values.
/// - Constants with defined ranges are within those ranges.
/// - The eddy viscosity expression string is non-empty.
/// - The number of transport equations matches `num_equations`.
/// - Each transport equation has a non-empty variable name and equation string.
pub fn validate_model(model: &dyn TurbulenceModel) -> Vec<String> {
    let mut warnings = Vec::new();
    let definition = model.get_definition();
    let constants = model.get_constants();

    // Check that constant values are valid.
    for (name, constant) in constants {
        if !constant.value.is_finite() {
            warnings.push(format!(
                "Constant '{}' has non-finite value: {}",
                name, constant.value
            ));
        }

        if let Some(min) = constant.min {
            if constant.value < min {
                warnings.push(format!(
                    "Constant '{}' = {} is below minimum {}",
                    name, constant.value, min
                ));
            }
        }

        if let Some(max) = constant.max {
            if constant.value > max {
                warnings.push(format!(
                    "Constant '{}' = {} is above maximum {}",
                    name, constant.value, max
                ));
            }
        }
    }

    // Check eddy viscosity expression.
    if definition.eddy_viscosity.trim().is_empty() {
        warnings.push("Eddy viscosity expression is empty".to_string());
    }

    // Check transport equation count consistency.
    if definition.transport_equations.len() != definition.num_equations {
        warnings.push(format!(
            "Declared num_equations ({}) does not match actual transport equations count ({})",
            definition.num_equations,
            definition.transport_equations.len()
        ));
    }

    // Check each transport equation.
    for (i, eq) in definition.transport_equations.iter().enumerate() {
        if eq.variable_name.trim().is_empty() {
            warnings.push(format!(
                "Transport equation {} has an empty variable name",
                i
            ));
        }
        if eq.equation_str.trim().is_empty() {
            warnings.push(format!(
                "Transport equation {} ('{}') has an empty equation string",
                i, eq.variable_name
            ));
        }
        if eq.diffusion_coeff.trim().is_empty() {
            warnings.push(format!(
                "Transport equation '{}' has an empty diffusion coefficient",
                eq.variable_name
            ));
        }
    }

    warnings
}
