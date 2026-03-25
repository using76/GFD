//! Scalar field stored as a Vec<f64> with one value per cell.

use serde::{Deserialize, Serialize};

use super::{Field, FieldData};
use crate::{CoreError, Result};

/// A scalar field storing one f64 value per mesh entity (typically cells).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalarField {
    /// Name of this field (e.g., "pressure", "temperature").
    name: String,
    /// The scalar values.
    data: Vec<f64>,
}

impl ScalarField {
    /// Creates a new scalar field with the given name and data.
    pub fn new(name: impl Into<String>, data: Vec<f64>) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }

    /// Creates a scalar field of zeros with the given size.
    pub fn zeros(name: impl Into<String>, size: usize) -> Self {
        Self {
            name: name.into(),
            data: vec![0.0; size],
        }
    }

    /// Creates a scalar field of ones with the given size.
    pub fn ones(name: impl Into<String>, size: usize) -> Self {
        Self {
            name: name.into(),
            data: vec![1.0; size],
        }
    }

    /// Creates a scalar field from an existing vector.
    pub fn from_vec(name: impl Into<String>, data: Vec<f64>) -> Self {
        Self::new(name, data)
    }

    /// Returns the value at the given index.
    pub fn get(&self, index: usize) -> Result<f64> {
        self.data.get(index).copied().ok_or(CoreError::IndexOutOfBounds {
            index,
            size: self.data.len(),
        })
    }

    /// Sets the value at the given index.
    pub fn set(&mut self, index: usize, value: f64) -> Result<()> {
        if index >= self.data.len() {
            return Err(CoreError::IndexOutOfBounds {
                index,
                size: self.data.len(),
            });
        }
        self.data[index] = value;
        Ok(())
    }

    /// Returns an iterator over the values.
    pub fn iter(&self) -> impl Iterator<Item = &f64> {
        self.data.iter()
    }

    /// Returns a mutable iterator over the values.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut f64> {
        self.data.iter_mut()
    }

    /// Applies a function to each value in the field.
    pub fn apply_fn<F: Fn(f64) -> f64>(&mut self, f: F) {
        for val in &mut self.data {
            *val = f(*val);
        }
    }

    /// Returns the L2 norm of the field.
    pub fn norm_l2(&self) -> f64 {
        self.data.iter().map(|v| v * v).sum::<f64>().sqrt()
    }

    /// Returns the maximum value in the field.
    pub fn max(&self) -> Option<f64> {
        self.data.iter().copied().reduce(f64::max)
    }

    /// Returns the minimum value in the field.
    pub fn min(&self) -> Option<f64> {
        self.data.iter().copied().reduce(f64::min)
    }

    /// Returns a reference to the underlying data.
    pub fn values(&self) -> &[f64] {
        &self.data
    }

    /// Returns a mutable reference to the underlying data.
    pub fn values_mut(&mut self) -> &mut [f64] {
        &mut self.data
    }
}

impl Field for ScalarField {
    fn len(&self) -> usize {
        self.data.len()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn clone_data(&self) -> FieldData {
        FieldData::Scalar(self.clone())
    }
}
