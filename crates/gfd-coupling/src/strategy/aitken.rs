//! Aitken dynamic relaxation coupling strategy.

use gfd_core::FieldSet;
use crate::traits::CouplingStrategy;
use crate::Result;

/// Aitken adaptive under-relaxation coupling.
///
/// Dynamically adjusts the relaxation factor based on the convergence history:
/// omega_{n+1} = -omega_n * (r_n^T * (r_{n+1} - r_n)) / ||r_{n+1} - r_n||^2
pub struct AitkenCoupling {
    /// Current relaxation factor.
    pub omega: f64,
    /// Previous residual vector (flattened).
    prev_residual: Option<Vec<f64>>,
}

impl AitkenCoupling {
    /// Creates a new Aitken coupling strategy with the given initial relaxation factor.
    pub fn new(initial_omega: f64) -> Self {
        Self {
            omega: initial_omega,
            prev_residual: None,
        }
    }

    /// Returns the current dynamic relaxation factor.
    pub fn current_omega(&self) -> f64 {
        self.omega
    }
}

impl CouplingStrategy for AitkenCoupling {
    fn exchange_data(
        &mut self,
        _fields_from: &FieldSet,
        _fields_to: &mut FieldSet,
    ) -> Result<()> {
        // Aitken algorithm:
        // 1. Compute residual r_{n+1} = phi_computed - phi_old
        // 2. If previous residual exists, update omega:
        //    delta_r = r_{n+1} - r_n
        //    omega = -omega * (r_n . delta_r) / (delta_r . delta_r)
        // 3. Apply relaxation: phi_new = phi_old + omega * r_{n+1}
        // 4. Store current residual as previous
        // Aitken algorithm:
        // 1. Compute residual r = fields_from values - fields_to values
        // 2. If previous residual exists, update omega
        // 3. Apply relaxation
        // 4. Store residual

        // Flatten fields_from and fields_to into vectors for comparison
        let mut residual = Vec::new();
        for (name, field_data) in _fields_from.iter() {
            if let Some(to_data) = _fields_to.get(name) {
                match (field_data, to_data) {
                    (gfd_core::FieldData::Scalar(from), gfd_core::FieldData::Scalar(to)) => {
                        for (f, t) in from.values().iter().zip(to.values().iter()) {
                            residual.push(f - t);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Update omega using Aitken formula if we have a previous residual
        if let Some(ref prev_r) = self.prev_residual {
            if prev_r.len() == residual.len() && !residual.is_empty() {
                let mut delta_r = vec![0.0_f64; residual.len()];
                for i in 0..residual.len() {
                    delta_r[i] = residual[i] - prev_r[i];
                }
                let dot_r_dr: f64 = prev_r.iter().zip(delta_r.iter()).map(|(a, b)| a * b).sum();
                let norm_dr_sq: f64 = delta_r.iter().map(|x| x * x).sum();
                if norm_dr_sq > 1e-30 {
                    self.omega = -self.omega * dot_r_dr / norm_dr_sq;
                    // Clamp omega to reasonable range
                    self.omega = self.omega.clamp(-1.0, 1.0);
                }
            }
        }

        // Apply relaxation: phi_new = phi_old + omega * residual
        let omega = self.omega;
        let mut r_idx = 0;
        for (name, field_data) in _fields_from.iter() {
            if let Some(to_data) = _fields_to.get_mut(name) {
                match (field_data, to_data) {
                    (gfd_core::FieldData::Scalar(_from), gfd_core::FieldData::Scalar(to)) => {
                        let to_vals = to.values_mut();
                        for val in to_vals.iter_mut() {
                            if r_idx < residual.len() {
                                *val += omega * residual[r_idx];
                                r_idx += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        self.prev_residual = Some(residual);

        Ok(())
    }

    fn check_convergence(
        &self,
        _current: &FieldSet,
        _previous: &FieldSet,
    ) -> Result<f64> {
        // Compute L2 norm of difference between current and previous
        let mut norm_sq = 0.0_f64;
        let mut count = 0;

        for (name, curr_data) in _current.iter() {
            if let Some(prev_data) = _previous.get(name) {
                match (curr_data, prev_data) {
                    (gfd_core::FieldData::Scalar(curr), gfd_core::FieldData::Scalar(prev)) => {
                        for (c, p) in curr.values().iter().zip(prev.values().iter()) {
                            norm_sq += (c - p).powi(2);
                            count += 1;
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
