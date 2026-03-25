//! Core traits and types for material property evaluation.

use crate::Result;

/// Thermomechanical state at which material properties are evaluated.
#[derive(Debug, Clone, Default)]
pub struct MaterialState {
    /// Temperature [K].
    pub temperature: f64,
    /// Pressure [Pa].
    pub pressure: f64,
    /// Strain rate magnitude [1/s] (for non-Newtonian fluids).
    pub strain_rate: f64,
    /// Specific volume [m^3/kg] (optional).
    pub specific_volume: f64,
    /// Species mass fractions (optional).
    pub species: Vec<f64>,
}

/// A property value that can be scalar, vector, or tensor.
#[derive(Debug, Clone)]
pub enum PropertyValue {
    /// Scalar property (e.g. density, viscosity).
    Scalar(f64),
    /// Vector property (3-component).
    Vector([f64; 3]),
    /// Tensor property (3x3 symmetric or full).
    Tensor([[f64; 3]; 3]),
}

/// Trait for evaluating a single material property.
pub trait MaterialProperty: std::fmt::Debug + Send + Sync {
    /// Evaluates the property at the given state. Returns a scalar value.
    fn evaluate(&self, state: &MaterialState) -> Result<f64>;

    /// Computes the derivative of this property with respect to `var`.
    ///
    /// Supported variables: "temperature", "pressure", "strain_rate".
    fn derivative_wrt(&self, state: &MaterialState, var: &str) -> Result<f64>;

    /// Returns the human-readable name of this property.
    fn name(&self) -> &str;

    /// Returns the SI units of this property.
    fn units(&self) -> &str;
}

/// Trait for constitutive (stress-strain) models.
pub trait ConstitutiveModel: std::fmt::Debug + Send + Sync {
    /// Computes the Cauchy stress tensor from the strain tensor and material state.
    fn stress(
        &self,
        strain: &[[f64; 3]; 3],
        state: &MaterialState,
    ) -> Result<[[f64; 3]; 3]>;
}
