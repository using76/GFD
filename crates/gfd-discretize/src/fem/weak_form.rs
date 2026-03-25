//! Automatic weak form generation from strong-form PDEs.

/// Trait for weak form representations of PDEs.
///
/// A weak form transforms the strong-form PDE into an integral statement
/// suitable for finite element discretization. Implementations generate
/// element-level integrals from the governing equations.
pub trait WeakForm {
    /// The type of the element stiffness contribution.
    type Output;

    /// Evaluate the weak form at a single quadrature point.
    ///
    /// # Arguments
    /// * `shape_values` - Shape function values at the quadrature point.
    /// * `shape_gradients` - Shape function gradients at the quadrature point.
    /// * `weight` - Quadrature weight times the Jacobian determinant.
    fn evaluate(
        &self,
        shape_values: &[f64],
        shape_gradients: &[[f64; 3]],
        weight: f64,
    ) -> Self::Output;
}
