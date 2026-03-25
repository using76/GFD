//! Post-processing traits.

use gfd_core::{FieldSet, ScalarField};
use crate::Result;

/// Trait for computing derived fields from existing solution data.
pub trait DerivedField {
    /// Computes the derived scalar field from the given field set.
    fn compute(&self, fields: &FieldSet) -> Result<ScalarField>;

    /// Returns the name of this derived field.
    fn name(&self) -> &str;

    /// Returns the SI units of this derived field.
    fn units(&self) -> &str;
}
