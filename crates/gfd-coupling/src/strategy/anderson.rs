//! Anderson acceleration coupling strategy.

use gfd_core::FieldSet;
use crate::traits::CouplingStrategy;
use crate::Result;

/// Anderson acceleration for coupling iteration.
///
/// Stores a history of previous iterates and residuals to compute an
/// optimal linear combination for the next iterate.
pub struct AndersonCoupling {
    /// Maximum number of previous iterates to store.
    pub depth: usize,
    /// History of residual vectors.
    residual_history: Vec<Vec<f64>>,
    /// History of iterate vectors.
    iterate_history: Vec<Vec<f64>>,
}

impl AndersonCoupling {
    /// Creates a new Anderson acceleration coupling with the given history depth.
    pub fn new(depth: usize) -> Self {
        Self {
            depth,
            residual_history: Vec::new(),
            iterate_history: Vec::new(),
        }
    }
}

impl CouplingStrategy for AndersonCoupling {
    fn exchange_data(
        &mut self,
        _fields_from: &FieldSet,
        _fields_to: &mut FieldSet,
    ) -> Result<()> {
        // Anderson acceleration:
        // 1. Compute g(x_k) from fields_from, x_k from fields_to
        // 2. Compute residual r_k = g(x_k) - x_k
        // 3. Store in history
        // 4. Solve least-squares for mixing coefficients
        // 5. Compute accelerated iterate

        // Flatten current iterate and fixed-point evaluation
        let mut x_k = Vec::new();
        let mut g_k = Vec::new();
        for (name, from_data) in _fields_from.iter() {
            if let Some(to_data) = _fields_to.get(name) {
                match (from_data, to_data) {
                    (gfd_core::FieldData::Scalar(from), gfd_core::FieldData::Scalar(to)) => {
                        g_k.extend_from_slice(from.values());
                        x_k.extend_from_slice(to.values());
                    }
                    _ => {}
                }
            }
        }

        // Residual
        let residual: Vec<f64> = g_k.iter().zip(x_k.iter()).map(|(g, x)| g - x).collect();

        // Store in history
        self.residual_history.push(residual.clone());
        self.iterate_history.push(x_k.clone());

        // Trim history to depth
        while self.residual_history.len() > self.depth + 1 {
            self.residual_history.remove(0);
            self.iterate_history.remove(0);
        }

        let m = self.residual_history.len();
        let n = residual.len();

        // If not enough history, use simple mixing
        let x_new = if m < 2 || n == 0 {
            // Simple relaxation: x_{k+1} = x_k + beta * r_k with beta=0.5
            let beta = 0.5;
            x_k.iter().zip(residual.iter()).map(|(x, r)| x + beta * r).collect::<Vec<f64>>()
        } else {
            // Build difference matrices
            let num_cols = m - 1;
            // delta_r_j = r_{j+1} - r_j
            let mut delta_r: Vec<Vec<f64>> = Vec::new();
            let mut delta_x: Vec<Vec<f64>> = Vec::new();
            for j in 0..num_cols {
                let dr: Vec<f64> = self.residual_history[j + 1].iter()
                    .zip(self.residual_history[j].iter())
                    .map(|(a, b)| a - b).collect();
                let dx: Vec<f64> = self.iterate_history[j + 1].iter()
                    .zip(self.iterate_history[j].iter())
                    .map(|(a, b)| a - b).collect();
                delta_r.push(dr);
                delta_x.push(dx);
            }

            // Solve least-squares: min ||r_m - sum(alpha_j * delta_r_j)||^2
            // Simple: if 1 column, alpha = (delta_r^T * r_m) / (delta_r^T * delta_r)
            let r_m = &self.residual_history[m - 1];
            let mut alphas = vec![0.0_f64; num_cols];

            if num_cols == 1 {
                let dot_dr_r: f64 = delta_r[0].iter().zip(r_m.iter()).map(|(a, b)| a * b).sum();
                let dot_dr_dr: f64 = delta_r[0].iter().map(|a| a * a).sum();
                if dot_dr_dr > 1e-30 {
                    alphas[0] = dot_dr_r / dot_dr_dr;
                }
            } else {
                // Multi-column: use simple least-squares via normal equations
                let nc = num_cols;
                let mut ata = vec![vec![0.0_f64; nc]; nc];
                let mut atb = vec![0.0_f64; nc];
                for i in 0..nc {
                    for j in 0..nc {
                        ata[i][j] = delta_r[i].iter().zip(delta_r[j].iter()).map(|(a, b)| a * b).sum();
                    }
                    atb[i] = delta_r[i].iter().zip(r_m.iter()).map(|(a, b)| a * b).sum();
                }
                // Solve with simple Gauss elimination
                for k in 0..nc {
                    let pivot = ata[k][k];
                    if pivot.abs() < 1e-30 { continue; }
                    for i in (k + 1)..nc {
                        let factor = ata[i][k] / pivot;
                        for j in k..nc {
                            ata[i][j] -= factor * ata[k][j];
                        }
                        atb[i] -= factor * atb[k];
                    }
                }
                for k in (0..nc).rev() {
                    if ata[k][k].abs() < 1e-30 { continue; }
                    let mut s = atb[k];
                    for j in (k + 1)..nc {
                        s -= ata[k][j] * alphas[j];
                    }
                    alphas[k] = s / ata[k][k];
                }
            }

            // Compute x_{k+1} = g_k - sum(alpha_j * (delta_x_j + delta_r_j))
            let mut x_new = g_k.clone();
            for j in 0..num_cols {
                for i in 0..n {
                    x_new[i] -= alphas[j] * (delta_x[j][i] + delta_r[j][i]);
                }
            }
            x_new
        };

        // Write result back to fields_to
        let mut idx = 0;
        for (name, _from_data) in _fields_from.iter() {
            if let Some(to_data) = _fields_to.get_mut(name) {
                if let gfd_core::FieldData::Scalar(to) = to_data {
                    let to_vals = to.values_mut();
                    for val in to_vals.iter_mut() {
                        if idx < x_new.len() {
                            *val = x_new[idx];
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
