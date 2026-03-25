//! Field mapping between non-matching meshes.

pub mod nearest;
pub mod rbf;
pub mod mortar;

use gfd_core::ScalarField;
use crate::Result;

/// Trait for mapping field values between non-conforming meshes.
pub trait FieldMapper {
    /// Maps a scalar field from a source mesh to a target mesh.
    fn map_field(&self, from: &ScalarField, to: &mut ScalarField) -> Result<()>;
}
