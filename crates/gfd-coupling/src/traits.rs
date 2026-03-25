//! Core coupling traits.

use gfd_core::FieldSet;
use crate::Result;

/// Trait for coupling strategies that transfer data between solvers.
pub trait CouplingStrategy {
    /// Exchange data from one field set to another through the coupling interface.
    fn exchange_data(
        &mut self,
        fields_from: &FieldSet,
        fields_to: &mut FieldSet,
    ) -> Result<()>;

    /// Check convergence between current and previous field sets.
    /// Returns the convergence residual (lower is better).
    fn check_convergence(
        &self,
        current: &FieldSet,
        previous: &FieldSet,
    ) -> Result<f64>;
}
