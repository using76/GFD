//! Vector field stored as Vec<[f64; 3]> with one 3D vector per cell.

use serde::{Deserialize, Serialize};

use super::scalar::ScalarField;
use super::{Field, FieldData};
use crate::{CoreError, Result};

/// A vector field storing one 3D vector per mesh entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorField {
    /// Name of this field (e.g., "velocity", "gradient_p").
    name: String,
    /// The vector values.
    data: Vec<[f64; 3]>,
}

impl VectorField {
    /// Creates a new vector field with the given name and data.
    pub fn new(name: impl Into<String>, data: Vec<[f64; 3]>) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }

    /// Creates a vector field of zero vectors with the given size.
    pub fn zeros(name: impl Into<String>, size: usize) -> Self {
        Self {
            name: name.into(),
            data: vec![[0.0, 0.0, 0.0]; size],
        }
    }

    /// Creates a vector field from an existing vector of 3-component arrays.
    pub fn from_vec(name: impl Into<String>, data: Vec<[f64; 3]>) -> Self {
        Self::new(name, data)
    }

    /// Returns the vector at the given index.
    pub fn get(&self, index: usize) -> Result<[f64; 3]> {
        self.data.get(index).copied().ok_or(CoreError::IndexOutOfBounds {
            index,
            size: self.data.len(),
        })
    }

    /// Sets the vector at the given index.
    pub fn set(&mut self, index: usize, value: [f64; 3]) -> Result<()> {
        if index >= self.data.len() {
            return Err(CoreError::IndexOutOfBounds {
                index,
                size: self.data.len(),
            });
        }
        self.data[index] = value;
        Ok(())
    }

    /// Returns an iterator over the vectors.
    pub fn iter(&self) -> impl Iterator<Item = &[f64; 3]> {
        self.data.iter()
    }

    /// Returns a mutable iterator over the vectors.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut [f64; 3]> {
        self.data.iter_mut()
    }

    /// Returns a ScalarField containing the magnitude of each vector.
    pub fn magnitude(&self) -> ScalarField {
        let magnitudes: Vec<f64> = self
            .data
            .iter()
            .map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
            .collect();
        ScalarField::new(format!("{}_magnitude", self.name), magnitudes)
    }

    /// Extracts a single component (0=x, 1=y, 2=z) as a ScalarField.
    pub fn component(&self, idx: usize) -> Result<ScalarField> {
        if idx >= 3 {
            return Err(CoreError::IndexOutOfBounds { index: idx, size: 3 });
        }
        let component_name = match idx {
            0 => "x",
            1 => "y",
            2 => "z",
            _ => unreachable!(),
        };
        let values: Vec<f64> = self.data.iter().map(|v| v[idx]).collect();
        Ok(ScalarField::new(
            format!("{}_{}", self.name, component_name),
            values,
        ))
    }

    /// Returns a reference to the underlying data.
    pub fn values(&self) -> &[[f64; 3]] {
        &self.data
    }

    /// Returns a mutable reference to the underlying data.
    pub fn values_mut(&mut self) -> &mut [[f64; 3]] {
        &mut self.data
    }
}

impl Field for VectorField {
    fn len(&self) -> usize {
        self.data.len()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn clone_data(&self) -> FieldData {
        FieldData::Vector(self.clone())
    }
}
