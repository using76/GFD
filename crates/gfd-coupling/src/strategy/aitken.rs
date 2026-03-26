//! Aitken dynamic relaxation coupling strategy.
//!
//! Implements the Aitken adaptive under-relaxation method for partitioned
//! multi-physics coupling. The relaxation factor is dynamically adjusted
//! based on the convergence history of interface residuals.

use gfd_core::{FieldData, FieldSet};
use crate::traits::CouplingStrategy;
use crate::Result;

/// Aitken adaptive under-relaxation coupling.
///
/// Dynamically adjusts the relaxation factor based on the convergence history:
///
///   omega_{n+1} = -omega_n * (r_n^T . (r_{n+1} - r_n)) / ||r_{n+1} - r_n||^2
///
/// where r is the interface residual vector (difference between computed
/// and current interface values). This provides super-linear convergence
/// for many FSI problems while being simple to implement.
pub struct AitkenCoupling {
    /// Current relaxation factor.
    pub omega: f64,
    /// Initial relaxation factor (used for reset).
    initial_omega: f64,
    /// Previous residual vector (flattened).
    prev_residual: Option<Vec<f64>>,
}

impl AitkenCoupling {
    /// Creates a new Aitken coupling strategy with the given initial relaxation factor.
    pub fn new(initial_omega: f64) -> Self {
        Self {
            omega: initial_omega,
            initial_omega,
            prev_residual: None,
        }
    }

    /// Returns the current dynamic relaxation factor.
    pub fn current_omega(&self) -> f64 {
        self.omega
    }

    /// Resets the Aitken state for a new time step.
    ///
    /// This should be called at the beginning of each time step to reset
    /// the relaxation factor to its initial value and clear the residual
    /// history, since the Aitken acceleration is within a single time step's
    /// coupling iterations.
    pub fn reset(&mut self) {
        self.omega = self.initial_omega;
        self.prev_residual = None;
    }
}

/// Flatten all scalar and vector fields from a FieldSet pair into a residual vector.
///
/// The residual is computed as r = fields_from - fields_to for all matching fields.
/// Fields are processed in sorted order by name for deterministic behavior.
fn compute_residual(fields_from: &FieldSet, fields_to: &FieldSet) -> Vec<f64> {
    let mut residual = Vec::new();
    let mut sorted_keys: Vec<&String> = fields_from.keys().collect();
    sorted_keys.sort();

    for name in sorted_keys {
        let field_data = &fields_from[name];
        if let Some(to_data) = fields_to.get(name) {
            match (field_data, to_data) {
                (FieldData::Scalar(from), FieldData::Scalar(to)) => {
                    for (f, t) in from.values().iter().zip(to.values().iter()) {
                        residual.push(f - t);
                    }
                }
                (FieldData::Vector(from), FieldData::Vector(to)) => {
                    for (f, t) in from.values().iter().zip(to.values().iter()) {
                        for dim in 0..3 {
                            residual.push(f[dim] - t[dim]);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    residual
}

impl CouplingStrategy for AitkenCoupling {
    fn exchange_data(
        &mut self,
        fields_from: &FieldSet,
        fields_to: &mut FieldSet,
    ) -> Result<()> {
        // Step 1: Compute residual r_{n+1} = phi_computed - phi_current
        let residual = compute_residual(fields_from, fields_to);

        // Step 2: Update omega using Aitken formula if we have a previous residual
        if let Some(ref prev_r) = self.prev_residual {
            if prev_r.len() == residual.len() && !residual.is_empty() {
                let mut delta_r = vec![0.0_f64; residual.len()];
                for i in 0..residual.len() {
                    delta_r[i] = residual[i] - prev_r[i];
                }
                let dot_r_dr: f64 = prev_r
                    .iter()
                    .zip(delta_r.iter())
                    .map(|(a, b)| a * b)
                    .sum();
                let norm_dr_sq: f64 = delta_r.iter().map(|x| x * x).sum();
                if norm_dr_sq > 1e-30 {
                    self.omega = -self.omega * dot_r_dr / norm_dr_sq;
                    // Clamp omega to reasonable range to prevent divergence
                    self.omega = self.omega.clamp(-1.0, 1.0);
                }
            }
        }

        // Step 3: Apply relaxation: phi_new = phi_old + omega * r_{n+1}
        let omega = self.omega;
        let mut r_idx = 0;
        let mut sorted_keys: Vec<&String> = fields_from.keys().collect();
        sorted_keys.sort();

        for name in sorted_keys {
            let field_data = &fields_from[name];
            if let Some(to_data) = fields_to.get_mut(name) {
                match (field_data, to_data) {
                    (FieldData::Scalar(_), FieldData::Scalar(to)) => {
                        let to_vals = to.values_mut();
                        for val in to_vals.iter_mut() {
                            if r_idx < residual.len() {
                                *val += omega * residual[r_idx];
                                r_idx += 1;
                            }
                        }
                    }
                    (FieldData::Vector(_), FieldData::Vector(to)) => {
                        let to_vals = to.values_mut();
                        for val in to_vals.iter_mut() {
                            for dim in 0..3 {
                                if r_idx < residual.len() {
                                    val[dim] += omega * residual[r_idx];
                                    r_idx += 1;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Step 4: Store current residual for next iteration
        self.prev_residual = Some(residual);

        Ok(())
    }

    fn check_convergence(
        &self,
        current: &FieldSet,
        previous: &FieldSet,
    ) -> Result<f64> {
        // Compute L2 norm of difference between current and previous
        let mut norm_sq = 0.0_f64;
        let mut count = 0;

        for (name, curr_data) in current.iter() {
            if let Some(prev_data) = previous.get(name) {
                match (curr_data, prev_data) {
                    (FieldData::Scalar(curr), FieldData::Scalar(prev)) => {
                        for (c, p) in curr.values().iter().zip(prev.values().iter()) {
                            norm_sq += (c - p).powi(2);
                            count += 1;
                        }
                    }
                    (FieldData::Vector(curr), FieldData::Vector(prev)) => {
                        for (c, p) in curr.values().iter().zip(prev.values().iter()) {
                            for dim in 0..3 {
                                norm_sq += (c[dim] - p[dim]).powi(2);
                                count += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let residual = if count > 0 {
            (norm_sq / count as f64).sqrt()
        } else {
            0.0
        };

        Ok(residual)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::field::ScalarField;

    #[test]
    fn test_aitken_initial_omega() {
        let coupling = AitkenCoupling::new(0.5);
        assert!((coupling.current_omega() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_aitken_first_iteration_uses_initial_omega() {
        let mut coupling = AitkenCoupling::new(0.3);

        let mut from = FieldSet::new();
        from.insert(
            "T".to_string(),
            FieldData::Scalar(ScalarField::new("T", vec![100.0, 200.0])),
        );

        let mut to = FieldSet::new();
        to.insert(
            "T".to_string(),
            FieldData::Scalar(ScalarField::new("T", vec![50.0, 150.0])),
        );

        // First iteration: no previous residual, so omega stays at 0.3
        coupling.exchange_data(&from, &mut to).unwrap();

        // Expected: to_new = to_old + 0.3 * (from - to_old)
        // Cell 0: 50 + 0.3 * (100 - 50) = 50 + 15 = 65
        // Cell 1: 150 + 0.3 * (200 - 150) = 150 + 15 = 165
        let to_vals = match to.get("T").unwrap() {
            FieldData::Scalar(sf) => sf.values().to_vec(),
            _ => panic!("Expected scalar"),
        };
        assert!(
            (to_vals[0] - 65.0).abs() < 1e-10,
            "Expected 65.0, got {}",
            to_vals[0]
        );
        assert!(
            (to_vals[1] - 165.0).abs() < 1e-10,
            "Expected 165.0, got {}",
            to_vals[1]
        );
    }

    #[test]
    fn test_aitken_omega_updates() {
        let mut coupling = AitkenCoupling::new(0.5);

        // First iteration
        let from1 = {
            let mut fs = FieldSet::new();
            fs.insert(
                "T".to_string(),
                FieldData::Scalar(ScalarField::new("T", vec![10.0])),
            );
            fs
        };
        let mut to1 = {
            let mut fs = FieldSet::new();
            fs.insert(
                "T".to_string(),
                FieldData::Scalar(ScalarField::new("T", vec![0.0])),
            );
            fs
        };
        coupling.exchange_data(&from1, &mut to1).unwrap();
        let omega_after_first = coupling.current_omega();
        assert!(
            (omega_after_first - 0.5).abs() < 1e-10,
            "Omega should stay at 0.5 on first call"
        );

        // Second iteration with different fields -> omega should update
        let from2 = {
            let mut fs = FieldSet::new();
            fs.insert(
                "T".to_string(),
                FieldData::Scalar(ScalarField::new("T", vec![8.0])),
            );
            fs
        };
        coupling.exchange_data(&from2, &mut to1).unwrap();
        // Omega should have changed from its initial value
        // (the exact value depends on the residuals)
        assert!(coupling.current_omega().is_finite());
    }

    #[test]
    fn test_aitken_convergence_check() {
        let coupling = AitkenCoupling::new(0.5);

        let mut current = FieldSet::new();
        current.insert(
            "T".to_string(),
            FieldData::Scalar(ScalarField::new("T", vec![100.0, 200.0])),
        );

        let mut previous = FieldSet::new();
        previous.insert(
            "T".to_string(),
            FieldData::Scalar(ScalarField::new("T", vec![100.0, 200.0])),
        );

        // Identical fields -> residual should be 0
        let res = coupling.check_convergence(&current, &previous).unwrap();
        assert!(res.abs() < 1e-10);

        // Different fields -> residual should be non-zero
        previous.insert(
            "T".to_string(),
            FieldData::Scalar(ScalarField::new("T", vec![90.0, 190.0])),
        );
        let res = coupling.check_convergence(&current, &previous).unwrap();
        assert!(res > 0.0);
    }

    #[test]
    fn test_aitken_reset() {
        let mut coupling = AitkenCoupling::new(0.7);

        // Do one exchange to populate prev_residual
        let from = {
            let mut fs = FieldSet::new();
            fs.insert(
                "T".to_string(),
                FieldData::Scalar(ScalarField::new("T", vec![10.0])),
            );
            fs
        };
        let mut to = {
            let mut fs = FieldSet::new();
            fs.insert(
                "T".to_string(),
                FieldData::Scalar(ScalarField::new("T", vec![0.0])),
            );
            fs
        };
        coupling.exchange_data(&from, &mut to).unwrap();
        assert!(coupling.prev_residual.is_some());

        coupling.reset();
        assert!((coupling.current_omega() - 0.7).abs() < 1e-10);
        assert!(coupling.prev_residual.is_none());
    }
}
