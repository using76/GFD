//! Turbulence model template definitions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Wall treatment strategy for a turbulence model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallTreatment {
    /// Standard wall function (log-law).
    StandardWallFunction,
    /// Scalable wall function (avoids y+ < 11.63 issues).
    ScalableWallFunction,
    /// Enhanced wall treatment (blended two-layer approach).
    EnhancedWallTreatment,
    /// Low-Reynolds-number model (integration to the wall).
    LowReynolds,
}

/// A model constant with metadata and optional range bounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConstant {
    /// Numerical value of the constant.
    pub value: f64,
    /// Human-readable description.
    pub description: String,
    /// Optional minimum valid value.
    pub min: Option<f64>,
    /// Optional maximum valid value.
    pub max: Option<f64>,
}

/// Definition of a single transport equation within a turbulence model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportEquationDef {
    /// Name of the transported variable (e.g. "k", "epsilon", "omega").
    pub variable_name: String,
    /// The full equation in GMN string form.
    pub equation_str: String,
    /// Diffusion coefficient expression.
    pub diffusion_coeff: String,
    /// Production term expression.
    pub production: String,
    /// Destruction term expression.
    pub destruction: String,
    /// Default boundary conditions per patch type.
    pub boundary_defaults: HashMap<String, String>,
}

/// Complete definition of a turbulence model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurbulenceModelDef {
    /// Name of the turbulence model.
    pub name: String,
    /// Number of transport equations.
    pub num_equations: usize,
    /// Definitions of each transport equation.
    pub transport_equations: Vec<TransportEquationDef>,
    /// Eddy viscosity expression in GMN string form.
    pub eddy_viscosity: String,
    /// Model constants keyed by name.
    pub constants: HashMap<String, ModelConstant>,
    /// Wall treatment strategy.
    pub wall_treatment: WallTreatment,
}
