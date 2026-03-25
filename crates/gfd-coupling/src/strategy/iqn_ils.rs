//! Interface Quasi-Newton with Inverse Least-Squares (IQN-ILS) coupling.
//!
//! An advanced partitioned coupling strategy for strongly coupled
//! multi-physics problems (e.g., fluid-structure interaction).
//! IQN-ILS builds an approximate inverse Jacobian from previous
//! residuals and interface values, enabling Newton-like convergence
//! rates without requiring explicit Jacobian computation.
//!
//! Reference: Degroote, Bathe & Vierendeels, "Performance of a new
//! partitioned procedure versus a monolithic procedure in
//! fluid-structure interaction", Computers & Structures 87 (2009).

use gfd_core::FieldSet;
use crate::traits::CouplingStrategy;
use crate::Result;

/// IQN-ILS (Interface Quasi-Newton with Inverse Least-Squares) coupling.
///
/// Builds an approximate inverse Jacobian of the coupled system from
/// differences of previous residuals (columns of matrix V) and
/// interface value differences (columns of matrix W).
pub struct IqnIls {
    /// Number of previous time steps whose data is reused.
    pub reuse: usize,
    /// History of residual differences (columns of V matrix).
    residual_diffs: Vec<Vec<f64>>,
    /// History of interface value differences (columns of W matrix).
    interface_diffs: Vec<Vec<f64>>,
    /// Previous residual vector.
    prev_residual: Option<Vec<f64>>,
    /// Previous interface value vector.
    prev_interface: Option<Vec<f64>>,
}

impl IqnIls {
    /// Creates a new IQN-ILS coupling strategy.
    ///
    /// `reuse` controls how many previous coupling iterations are kept
    /// in the V and W matrices.  A value of 0 means only the current
    /// time-step data is used (no reuse across time steps).
    pub fn new(reuse: usize) -> Self {
        Self {
            reuse,
            residual_diffs: Vec::new(),
            interface_diffs: Vec::new(),
            prev_residual: None,
            prev_interface: None,
        }
    }

    /// Updates the inverse Jacobian approximation with a new
    /// residual / interface value pair.
    ///
    /// This appends columns to the V and W matrices and trims old
    /// columns that exceed the reuse window.
    pub fn update(&mut self, _residual: &[f64], _interface_value: &[f64]) {
        // If we have a previous residual/interface, compute differences
        if let (Some(ref prev_r), Some(ref prev_x)) = (&self.prev_residual, &self.prev_interface) {
            if prev_r.len() == _residual.len() && prev_x.len() == _interface_value.len() {
                let delta_r: Vec<f64> = _residual.iter().zip(prev_r.iter()).map(|(a, b)| a - b).collect();
                let delta_x: Vec<f64> = _interface_value.iter().zip(prev_x.iter()).map(|(a, b)| a - b).collect();
                self.residual_diffs.push(delta_r);
                self.interface_diffs.push(delta_x);
            }
        }

        // Store current as previous
        self.prev_residual = Some(_residual.to_vec());
        self.prev_interface = Some(_interface_value.to_vec());

        // Trim by reuse window
        let max_cols = if self.reuse > 0 { self.reuse } else { 50 };
        while self.residual_diffs.len() > max_cols {
            self.residual_diffs.remove(0);
            self.interface_diffs.remove(0);
        }
    }

    /// Computes the quasi-Newton relaxation step.
    ///
    /// Solves the least-squares problem  V * c = -r  to obtain the
    /// coefficient vector c, then returns the update
    /// delta_x = W * c + r.
    pub fn compute_relaxation(&self) -> Vec<f64> {
        let r = match &self.prev_residual {
            Some(r) => r,
            None => return Vec::new(),
        };

        let n = r.len();
        let m = self.residual_diffs.len();

        if m == 0 || n == 0 {
            // No history: return simple relaxation (omega=0.5)
            return r.iter().map(|ri| 0.5 * ri).collect();
        }

        // Solve least-squares: V * c = -r
        // V is n x m, solve via normal equations: V^T V c = -V^T r
        let mut vtv = vec![vec![0.0_f64; m]; m];
        let mut vtr = vec![0.0_f64; m];

        for i in 0..m {
            for j in 0..m {
                vtv[i][j] = self.residual_diffs[i]
                    .iter()
                    .zip(self.residual_diffs[j].iter())
                    .map(|(a, b)| a * b)
                    .sum();
            }
            vtr[i] = -self.residual_diffs[i]
                .iter()
                .zip(r.iter())
                .map(|(a, b)| a * b)
                .sum::<f64>();
        }

        // Solve with Gauss elimination
        let mut c = vec![0.0_f64; m];
        for k in 0..m {
            let pivot = vtv[k][k];
            if pivot.abs() < 1e-30 { continue; }
            for i in (k + 1)..m {
                let factor = vtv[i][k] / pivot;
                for j in k..m {
                    vtv[i][j] -= factor * vtv[k][j];
                }
                vtr[i] -= factor * vtr[k];
            }
        }
        for k in (0..m).rev() {
            if vtv[k][k].abs() < 1e-30 { continue; }
            let mut s = vtr[k];
            for j in (k + 1)..m {
                s -= vtv[k][j] * c[j];
            }
            c[k] = s / vtv[k][k];
        }

        // Compute update: delta_x = W * c + r
        let mut delta_x = r.clone();
        for j in 0..m {
            if j < self.interface_diffs.len() {
                for i in 0..n.min(self.interface_diffs[j].len()) {
                    delta_x[i] += c[j] * self.interface_diffs[j][i];
                }
            }
        }

        delta_x
    }

    /// Clears all accumulated history (call at the start of a new time step
    /// when `reuse == 0`).
    pub fn clear_history(&mut self) {
        self.residual_diffs.clear();
        self.interface_diffs.clear();
        self.prev_residual = None;
        self.prev_interface = None;
    }
}

impl CouplingStrategy for IqnIls {
    fn exchange_data(
        &mut self,
        _fields_from: &FieldSet,
        _fields_to: &mut FieldSet,
    ) -> Result<()> {
        // IQN-ILS algorithm per coupling iteration:
        // 1. Evaluate residual r = H(x) - x  where H is the coupled operator.
        // 2. If not first iteration, update V and W matrices.
        // 3. Solve least-squares:  V * c = -r
        // 4. Compute update:  delta_x = W * c + r
        // 5. Apply:  x_{k+1} = x_k + delta_x
        // IQN-ILS coupling iteration:
        // 1. Extract residual from field difference
        // 2. Update V and W matrices
        // 3. Compute relaxation
        // 4. Apply update

        let mut x_from = Vec::new();
        let mut x_to = Vec::new();

        for (name, from_data) in _fields_from.iter() {
            if let Some(to_data) = _fields_to.get(name) {
                match (from_data, to_data) {
                    (gfd_core::FieldData::Scalar(from), gfd_core::FieldData::Scalar(to)) => {
                        x_from.extend_from_slice(from.values());
                        x_to.extend_from_slice(to.values());
                    }
                    _ => {}
                }
            }
        }

        // Residual = H(x) - x = fields_from - fields_to
        let residual: Vec<f64> = x_from.iter().zip(x_to.iter()).map(|(f, t)| f - t).collect();

        // Update V and W matrices
        self.update(&residual, &x_to);

        // Compute relaxation
        let delta_x = self.compute_relaxation();

        // Apply: x_{k+1} = x_k + delta_x
        let mut idx = 0;
        for (name, _from_data) in _fields_from.iter() {
            if let Some(to_data) = _fields_to.get_mut(name) {
                if let gfd_core::FieldData::Scalar(to) = to_data {
                    let to_vals = to.values_mut();
                    for val in to_vals.iter_mut() {
                        if idx < delta_x.len() {
                            *val += delta_x[idx];
                            idx += 1;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn check_convergence(
        &self,
        _current: &FieldSet,
        _previous: &FieldSet,
    ) -> Result<f64> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iqn_ils_creation() {
        let iqn = IqnIls::new(3);
        assert_eq!(iqn.reuse, 3);
        assert!(iqn.prev_residual.is_none());
        assert!(iqn.prev_interface.is_none());
    }

    #[test]
    fn test_iqn_ils_clear_history() {
        let mut iqn = IqnIls::new(2);
        iqn.residual_diffs.push(vec![1.0, 2.0]);
        iqn.clear_history();
        assert!(iqn.residual_diffs.is_empty());
        assert!(iqn.interface_diffs.is_empty());
    }
}
