//! Tensor field stored as Vec<[[f64; 3]; 3]> with one 3x3 tensor per cell.

use serde::{Deserialize, Serialize};

use super::{Field, FieldData};
use crate::{CoreError, Result};

/// A tensor field storing one 3x3 tensor per mesh entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorField {
    /// Name of this field (e.g., "stress", "strain_rate").
    name: String,
    /// The tensor values (row-major 3x3 matrices).
    data: Vec<[[f64; 3]; 3]>,
}

impl TensorField {
    /// Creates a new tensor field with the given name and data.
    pub fn new(name: impl Into<String>, data: Vec<[[f64; 3]; 3]>) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }

    /// Creates a tensor field of zero tensors with the given size.
    pub fn zeros(name: impl Into<String>, size: usize) -> Self {
        Self {
            name: name.into(),
            data: vec![[[0.0; 3]; 3]; size],
        }
    }

    /// Creates a tensor field from an existing vector of 3x3 arrays.
    pub fn from_vec(name: impl Into<String>, data: Vec<[[f64; 3]; 3]>) -> Self {
        Self::new(name, data)
    }

    /// Returns the tensor at the given index.
    pub fn get(&self, index: usize) -> Result<[[f64; 3]; 3]> {
        self.data.get(index).copied().ok_or(CoreError::IndexOutOfBounds {
            index,
            size: self.data.len(),
        })
    }

    /// Sets the tensor at the given index.
    pub fn set(&mut self, index: usize, value: [[f64; 3]; 3]) -> Result<()> {
        if index >= self.data.len() {
            return Err(CoreError::IndexOutOfBounds {
                index,
                size: self.data.len(),
            });
        }
        self.data[index] = value;
        Ok(())
    }

    /// Returns an iterator over the tensors.
    pub fn iter(&self) -> impl Iterator<Item = &[[f64; 3]; 3]> {
        self.data.iter()
    }

    /// Returns a mutable iterator over the tensors.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut [[f64; 3]; 3]> {
        self.data.iter_mut()
    }

    /// Returns a reference to the underlying data.
    pub fn values(&self) -> &[[[f64; 3]; 3]] {
        &self.data
    }

    /// Returns a mutable reference to the underlying data.
    pub fn values_mut(&mut self) -> &mut [[[f64; 3]; 3]] {
        &mut self.data
    }

    /// Computes the trace of each tensor and returns it as a ScalarField.
    pub fn trace(&self) -> super::scalar::ScalarField {
        let traces: Vec<f64> = self
            .data
            .iter()
            .map(|t| t[0][0] + t[1][1] + t[2][2])
            .collect();
        super::scalar::ScalarField::new(format!("{}_trace", self.name), traces)
    }
}

impl Field for TensorField {
    fn len(&self) -> usize {
        self.data.len()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn clone_data(&self) -> FieldData {
        FieldData::Tensor(self.clone())
    }
}
