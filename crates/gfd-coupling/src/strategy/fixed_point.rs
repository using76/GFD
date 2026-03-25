//! Fixed-point (constant relaxation) coupling strategy.

use gfd_core::{FieldData, FieldSet};
use crate::traits::CouplingStrategy;
use crate::Result;

/// Fixed-point coupling with constant under-relaxation.
///
/// Updates fields using: phi_new = omega * phi_computed + (1 - omega) * phi_old
pub struct FixedPointCoupling {
    /// Under-relaxation factor (0 < omega <= 1).
    pub relaxation: f64,
}

impl FixedPointCoupling {
    /// Creates a new fixed-point coupling strategy with the given relaxation factor.
    pub fn new(relaxation: f64) -> Self {
        assert!(
            relaxation > 0.0 && relaxation <= 1.0,
            "Relaxation factor must be in (0, 1], got {}",
            relaxation
        );
        Self { relaxation }
    }
}

impl CouplingStrategy for FixedPointCoupling {
    fn exchange_data(
        &mut self,
        fields_from: &FieldSet,
        fields_to: &mut FieldSet,
    ) -> Result<()> {
        let omega = self.relaxation;

        for (name, new_data) in fields_from.iter() {
            if let Some(old_data) = fields_to.get_mut(name) {
                match (new_data, &mut *old_data) {
                    (FieldData::Scalar(new_field), FieldData::Scalar(old_field)) => {
                        let new_vals = new_field.values();
                        let old_vals = old_field.values_mut();
                        for (old, &new) in old_vals.iter_mut().zip(new_vals.iter()) {
                            // phi_new = omega * phi_computed + (1 - omega) * phi_old
                            *old = omega * new + (1.0 - omega) * (*old);
                        }
                    }
                    (FieldData::Vector(new_field), FieldData::Vector(old_field)) => {
                        let new_vals = new_field.values();
                        let old_vals = old_field.values_mut();
                        for (old, new) in old_vals.iter_mut().zip(new_vals.iter()) {
                            for c in 0..3 {
                                old[c] = omega * new[c] + (1.0 - omega) * old[c];
                            }
                        }
                    }
                    (new, old) => {
                        // For tensor or mismatched types, just copy directly
                        *old = new.clone();
                    }
                }
            }
        }

        Ok(())
    }

    fn check_convergence(
        &self,
        current: &FieldSet,
        previous: &FieldSet,
    ) -> Result<f64> {
        let mut max_residual = 0.0_f64;

        for (name, curr_data) in current.iter() {
            if let Some(prev_data) = previous.get(name) {
                let residual = match (curr_data, prev_data) {
                    (FieldData::Scalar(curr), FieldData::Scalar(prev)) => {
                        let curr_vals = curr.values();
                        let prev_vals = prev.values();
                        let diff_sq: f64 = curr_vals
                            .iter()
                            .zip(prev_vals.iter())
                            .map(|(c, p)| (c - p) * (c - p))
                            .sum();
                        let norm: f64 = curr_vals.iter().map(|v| v * v).sum::<f64>().max(1e-30);
                        (diff_sq / norm).sqrt()
                    }
                    (FieldData::Vector(curr), FieldData::Vector(prev)) => {
                        let curr_vals = curr.values();
                        let prev_vals = prev.values();
                        let diff_sq: f64 = curr_vals
                            .iter()
                            .zip(prev_vals.iter())
                            .map(|(c, p)| {
                                (c[0] - p[0]).powi(2)
                                    + (c[1] - p[1]).powi(2)
                                    + (c[2] - p[2]).powi(2)
                            })
                            .sum();
                        let norm: f64 = curr_vals
                            .iter()
                            .map(|v| v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
                            .sum::<f64>()
                            .max(1e-30);
                        (diff_sq / norm).sqrt()
                    }
                    _ => 0.0,
                };
                max_residual = max_residual.max(residual);
            }
        }

        Ok(max_residual)
    }
}
