//! Field data structures for storing physical quantities on meshes.

pub mod scalar;
pub mod vector;
pub mod tensor;

use std::collections::HashMap;

pub use scalar::ScalarField;
pub use vector::VectorField;
pub use tensor::TensorField;

/// Trait for common field operations.
pub trait Field {
    /// Returns the number of elements in this field.
    fn len(&self) -> usize;

    /// Returns true if this field has no elements.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the name of this field.
    fn name(&self) -> &str;

    /// Clones the data of this field into a new boxed `FieldData`.
    fn clone_data(&self) -> FieldData;
}

/// Enum that can hold any field type.
#[derive(Debug, Clone)]
pub enum FieldData {
    Scalar(ScalarField),
    Vector(VectorField),
    Tensor(TensorField),
}

impl FieldData {
    /// Returns the number of elements in the field data.
    pub fn len(&self) -> usize {
        match self {
            FieldData::Scalar(f) => f.len(),
            FieldData::Vector(f) => f.len(),
            FieldData::Tensor(f) => f.len(),
        }
    }

    /// Returns true if the field data has no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the name of the field data.
    pub fn name(&self) -> &str {
        match self {
            FieldData::Scalar(f) => f.name(),
            FieldData::Vector(f) => f.name(),
            FieldData::Tensor(f) => f.name(),
        }
    }
}

/// A collection of named fields, used to store the full solution state.
pub type FieldSet = HashMap<String, FieldData>;
