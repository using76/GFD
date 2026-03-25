//! Algebraic Multigrid (AMG) preconditioner.
//!
//! Implements classical AMG with Ruge-Stueben coarsening.
//! Uses strength-of-connection to build the coarse/fine splitting,
//! constructs interpolation/restriction operators, and performs
//! V-cycle smoothing with Gauss-Seidel at each level.

use gfd_core::SparseMatrix;
use crate::{LinalgError, Result};
use crate::traits::PreconditionerTrait;

/// A single level in the AMG hierarchy.
#[derive(Debug, Clone)]
struct AmgLevel {
    /// System matrix at this level (CSR).
    a_row_ptr: Vec<usize>,
    a_col_idx: Vec<usize>,
    a_values: Vec<f64>,
    n: usize,

    /// Interpolation operator (prolongation): coarse -> fine.
    /// Stored as CSR with dimensions n_fine x n_coarse.
    p_row_ptr: Vec<usize>,
    p_col_idx: Vec<usize>,
    p_values: Vec<f64>,

    /// Restriction operator: fine -> coarse (= P^T).
    /// Stored as CSR with dimensions n_coarse x n_fine.
    r_row_ptr: Vec<usize>,
    r_col_idx: Vec<usize>,
    r_values: Vec<f64>,

    /// Diagonal entries of A for this level (for Gauss-Seidel smoothing).
    diag: Vec<f64>,
}

/// AMG preconditioner using classical Ruge-Stueben coarsening.
///
/// Builds a hierarchy of coarser levels from the system matrix,
/// then applies V-cycle multigrid as a preconditioner.
#[derive(Debug, Clone)]
pub struct AMG {
    /// AMG levels (index 0 = finest).
    levels: Vec<AmgLevel>,
    /// Coarsest-level matrix stored dense for direct solve.
    coarse_dense: Vec<f64>,
    coarse_n: usize,
    /// Number of pre/post smoothing iterations.
    n_smooth: usize,
    /// Strength threshold for coarsening.
    theta: f64,
    /// Maximum number of levels.
    max_levels: usize,
    /// Minimum coarse grid size (below which we stop coarsening).
    min_coarse_size: usize,
}

impl AMG {
    /// Creates a new AMG preconditioner with default parameters.
    pub fn new() -> Self {
        Self {
            levels: Vec::new(),
            coarse_dense: Vec::new(),
            coarse_n: 0,
            n_smooth: 2,
            theta: 0.25,
            max_levels: 10,
            min_coarse_size: 4,
        }
    }

    /// Creates an AMG preconditioner with custom parameters.
    pub fn with_params(n_smooth: usize, theta: f64, max_levels: usize) -> Self {
        Self {
            levels: Vec::new(),
            coarse_dense: Vec::new(),
            coarse_n: 0,
            n_smooth,
            theta,
            max_levels,
            min_coarse_size: 4,
        }
    }

    /// Perform Gauss-Seidel smoothing: A * x ~ b.
    fn smooth(
        row_ptr: &[usize],
        col_idx: &[usize],
        values: &[f64],
        diag: &[f64],
        b: &[f64],
        x: &mut [f64],
        n: usize,
        iterations: usize,
    ) {
        for _ in 0..iterations {
            for i in 0..n {
                let mut sigma = 0.0;
                for idx in row_ptr[i]..row_ptr[i + 1] {
                    let j = col_idx[idx];
                    if j != i {
                        sigma += values[idx] * x[j];
                    }
                }
                if diag[i].abs() > 1e-300 {
                    x[i] = (b[i] - sigma) / diag[i];
                }
            }
        }
    }

    /// Sparse matrix-vector multiply for level data.
    fn level_spmv(
        row_ptr: &[usize],
        col_idx: &[usize],
        values: &[f64],
        x: &[f64],
        y: &mut [f64],
        n: usize,
    ) {
        for i in 0..n {
            let mut sum = 0.0;
            for idx in row_ptr[i]..row_ptr[i + 1] {
                sum += values[idx] * x[col_idx[idx]];
            }
            y[i] = sum;
        }
    }

    /// Apply restriction: r_coarse = R * r_fine (R = P^T).
    fn restrict(level: &AmgLevel, fine: &[f64], coarse: &mut [f64]) {
        let n_coarse = coarse.len();
        for i in 0..n_coarse {
            let mut sum = 0.0;
            for idx in level.r_row_ptr[i]..level.r_row_ptr[i + 1] {
                sum += level.r_values[idx] * fine[level.r_col_idx[idx]];
            }
            coarse[i] = sum;
        }
    }

    /// Apply prolongation: x_fine += P * x_coarse.
    fn prolongate(level: &AmgLevel, coarse: &[f64], fine: &mut [f64]) {
        let n_fine = fine.len();
        for i in 0..n_fine {
            for idx in level.p_row_ptr[i]..level.p_row_ptr[i + 1] {
                fine[i] += level.p_values[idx] * coarse[level.p_col_idx[idx]];
            }
        }
    }

    /// Solve coarsest level directly using dense LU.
    fn solve_coarse(&self, b: &[f64], x: &mut [f64]) {
        let n = self.coarse_n;
        if n == 0 {
            return;
        }

        // Simple Gaussian elimination with partial pivoting on the dense copy.
        let mut dense = self.coarse_dense.clone();
        let mut rhs = b.to_vec();
        let mut piv: Vec<usize> = (0..n).collect();

        for k in 0..n {
            // Find pivot.
            let mut max_val = 0.0;
            let mut max_row = k;
            for i in k..n {
                let val = dense[piv[i] * n + k].abs();
                if val > max_val {
                    max_val = val;
                    max_row = i;
                }
            }
            piv.swap(k, max_row);

            let pivot_val = dense[piv[k] * n + k];
            if pivot_val.abs() < 1e-300 {
                // Near-singular at coarsest level; zero fill.
                x[k] = 0.0;
                continue;
            }

            for i in (k + 1)..n {
                let factor = dense[piv[i] * n + k] / pivot_val;
                dense[piv[i] * n + k] = factor;
                for j in (k + 1)..n {
                    dense[piv[i] * n + j] -= factor * dense[piv[k] * n + j];
                }
                rhs[piv[i]] -= factor * rhs[piv[k]];
            }
        }

        // Back substitution.
        for i in (0..n).rev() {
            let mut sum = rhs[piv[i]];
            for j in (i + 1)..n {
                sum -= dense[piv[i] * n + j] * x[j];
            }
            let diag = dense[piv[i] * n + i];
            x[i] = if diag.abs() > 1e-300 { sum / diag } else { 0.0 };
        }
    }

    /// V-cycle: recursively apply multigrid.
    fn vcycle(&self, level_idx: usize, b: &[f64], x: &mut [f64]) {
        let n_levels = self.levels.len();

        if level_idx >= n_levels {
            // Coarsest level: solve directly.
            self.solve_coarse(b, x);
            return;
        }

        let level = &self.levels[level_idx];
        let n = level.n;

        // Pre-smoothing.
        Self::smooth(
            &level.a_row_ptr,
            &level.a_col_idx,
            &level.a_values,
            &level.diag,
            b,
            x,
            n,
            self.n_smooth,
        );

        // Compute residual: r = b - A * x.
        let mut r = vec![0.0; n];
        Self::level_spmv(
            &level.a_row_ptr,
            &level.a_col_idx,
            &level.a_values,
            x,
            &mut r,
            n,
        );
        for i in 0..n {
            r[i] = b[i] - r[i];
        }

        // Restrict residual to coarse level.
        let n_coarse = if level_idx + 1 < n_levels {
            self.levels[level_idx + 1].n
        } else {
            self.coarse_n
        };
        let mut r_coarse = vec![0.0; n_coarse];
        Self::restrict(level, &r, &mut r_coarse);

        // Solve on coarse level.
        let mut e_coarse = vec![0.0; n_coarse];
        self.vcycle(level_idx + 1, &r_coarse, &mut e_coarse);

        // Prolongate error correction to fine level: x += P * e_coarse.
        Self::prolongate(level, &e_coarse, x);

        // Post-smoothing.
        Self::smooth(
            &level.a_row_ptr,
            &level.a_col_idx,
            &level.a_values,
            &level.diag,
            b,
            x,
            n,
            self.n_smooth,
        );
    }

    /// Build interpolation and restriction operators using classical RS coarsening.
    fn build_level(
        a_row_ptr: &[usize],
        a_col_idx: &[usize],
        a_values: &[f64],
        n: usize,
        theta: f64,
    ) -> Option<AmgLevel> {
        if n <= 4 {
            return None;
        }

        // Step 1: Compute strength of connection.
        // For each row i, find max |a_ij| for j != i.
        let mut max_off_diag = vec![0.0f64; n];
        for i in 0..n {
            for idx in a_row_ptr[i]..a_row_ptr[i + 1] {
                let j = a_col_idx[idx];
                if j != i {
                    let val = a_values[idx].abs();
                    if val > max_off_diag[i] {
                        max_off_diag[i] = val;
                    }
                }
            }
        }

        // Step 2: Build strong connection sets and compute "lambda" (influence measure).
        // Strong connection: |a_ij| >= theta * max_off_diag[i]
        // lambda[i] = number of points strongly influenced by i (S^T connections).
        let mut lambda = vec![0usize; n];
        let mut strong_connections: Vec<Vec<usize>> = vec![Vec::new(); n];

        for i in 0..n {
            let threshold = theta * max_off_diag[i];
            for idx in a_row_ptr[i]..a_row_ptr[i + 1] {
                let j = a_col_idx[idx];
                if j != i && a_values[idx].abs() >= threshold {
                    strong_connections[i].push(j);
                    lambda[j] += 1; // j influences i
                }
            }
        }

        // Step 3: C/F splitting (simplified RS).
        // 0 = undecided, 1 = coarse (C), 2 = fine (F)
        let mut cf = vec![0u8; n];
        let mut n_undecided = n;

        while n_undecided > 0 {
            // Pick the undecided point with maximum lambda.
            let mut best_i = n; // invalid
            let mut best_lambda = 0;
            for i in 0..n {
                if cf[i] == 0 && (best_i == n || lambda[i] > best_lambda) {
                    best_i = i;
                    best_lambda = lambda[i];
                }
            }
            if best_i == n {
                break;
            }

            // Mark as coarse.
            cf[best_i] = 1;
            n_undecided -= 1;

            // All undecided points strongly connected to best_i become fine.
            for &j in &strong_connections[best_i] {
                if cf[j] == 0 {
                    cf[j] = 2;
                    n_undecided -= 1;
                    // Points strongly connected to j get their lambda increased
                    // (since j is now fine, its neighbors become more important).
                    for &k in &strong_connections[j] {
                        if cf[k] == 0 {
                            lambda[k] += 1;
                        }
                    }
                }
            }

            // Also check reverse: points that strongly connect to best_i.
            for i in 0..n {
                if cf[i] == 0 && strong_connections[i].contains(&best_i) {
                    cf[i] = 2;
                    n_undecided -= 1;
                    for &k in &strong_connections[i] {
                        if cf[k] == 0 {
                            lambda[k] += 1;
                        }
                    }
                }
            }
        }

        // Count coarse points and create mapping.
        let mut coarse_map = vec![0usize; n]; // fine index -> coarse index
        let mut n_coarse = 0;
        for i in 0..n {
            if cf[i] == 1 {
                coarse_map[i] = n_coarse;
                n_coarse += 1;
            }
        }

        // If coarsening didn't reduce size enough, stop.
        if n_coarse == 0 || n_coarse >= n - 1 {
            return None;
        }

        // Step 4: Build interpolation operator P (n x n_coarse).
        // For coarse points: P(i, coarse_map[i]) = 1.
        // For fine points: interpolate from strong coarse neighbors.
        let mut p_row_ptr = vec![0usize; n + 1];
        let mut p_col_idx = Vec::new();
        let mut p_values = Vec::new();

        for i in 0..n {
            if cf[i] == 1 {
                // Coarse point: identity interpolation.
                p_col_idx.push(coarse_map[i]);
                p_values.push(1.0);
            } else {
                // Fine point: interpolate from coarse neighbors.
                // Collect strong coarse neighbors and their weights.
                let mut coarse_neighbors: Vec<(usize, f64)> = Vec::new();
                let mut sum_strong_coarse = 0.0;
                let mut sum_strong_fine = 0.0;
                let mut diag_val = 0.0;
                let mut sum_weak = 0.0;

                for idx in a_row_ptr[i]..a_row_ptr[i + 1] {
                    let j = a_col_idx[idx];
                    let aij = a_values[idx];
                    if j == i {
                        diag_val = aij;
                    } else if strong_connections[i].contains(&j) {
                        if cf[j] == 1 {
                            coarse_neighbors.push((coarse_map[j], aij));
                            sum_strong_coarse += aij;
                        } else {
                            sum_strong_fine += aij;
                        }
                    } else {
                        sum_weak += aij;
                    }
                }

                if coarse_neighbors.is_empty() || diag_val.abs() < 1e-300 {
                    // No coarse neighbors: use diagonal only (will contribute zero correction).
                    // This is a fallback; ideally every fine point has coarse neighbors.
                } else {
                    // Classical AMG interpolation weight formula:
                    // w_ij = -a_ij / (diag + sum_weak) * (1 + sum_strong_fine / sum_strong_coarse)
                    let denom = diag_val + sum_weak;
                    let alpha = if sum_strong_coarse.abs() > 1e-300 {
                        1.0 + sum_strong_fine / sum_strong_coarse
                    } else {
                        1.0
                    };

                    for (cj, aij) in &coarse_neighbors {
                        let w = -aij * alpha / denom;
                        p_col_idx.push(*cj);
                        p_values.push(w);
                    }
                }
            }
            p_row_ptr[i + 1] = p_col_idx.len();
        }

        // Step 5: Build restriction R = P^T (n_coarse x n).
        // Transpose the CSR P into CSR R.
        let mut r_row_ptr = vec![0usize; n_coarse + 1];
        // Count entries per row of R (= per column of P).
        for &cj in &p_col_idx {
            r_row_ptr[cj + 1] += 1;
        }
        for i in 0..n_coarse {
            r_row_ptr[i + 1] += r_row_ptr[i];
        }
        let nnz_r = r_row_ptr[n_coarse];
        let mut r_col_idx = vec![0usize; nnz_r];
        let mut r_values = vec![0.0; nnz_r];
        let mut r_count = vec![0usize; n_coarse];

        for i in 0..n {
            for idx in p_row_ptr[i]..p_row_ptr[i + 1] {
                let cj = p_col_idx[idx];
                let pos = r_row_ptr[cj] + r_count[cj];
                r_col_idx[pos] = i;
                r_values[pos] = p_values[idx];
                r_count[cj] += 1;
            }
        }

        // Extract diagonal of A.
        let mut diag = vec![0.0; n];
        for i in 0..n {
            for idx in a_row_ptr[i]..a_row_ptr[i + 1] {
                if a_col_idx[idx] == i {
                    diag[i] = a_values[idx];
                    break;
                }
            }
        }

        Some(AmgLevel {
            a_row_ptr: a_row_ptr.to_vec(),
            a_col_idx: a_col_idx.to_vec(),
            a_values: a_values.to_vec(),
            n,
            p_row_ptr,
            p_col_idx,
            p_values,
            r_row_ptr,
            r_col_idx,
            r_values,
            diag,
        })
    }

    /// Compute the Galerkin coarse operator: A_c = R * A * P.
    /// Returns CSR components and dimension.
    fn compute_coarse_operator(
        level: &AmgLevel,
        n_coarse: usize,
    ) -> (Vec<usize>, Vec<usize>, Vec<f64>) {
        let _n_fine = level.n;

        // Step 1: Compute AP = A * P (n_fine x n_coarse) using a hash map for each row.
        // Then compute R * AP = P^T * AP (n_coarse x n_coarse).

        // We'll compute R * A * P row by row for R (which is n_coarse x n_fine).
        // R(ci, :) * A * P = sum_i R(ci, i) * A(i, :) * P

        // Actually, let's do this as a triple product using dense accumulator per row.
        let mut ac_row_ptr = vec![0usize; n_coarse + 1];
        let mut ac_col_idx = Vec::new();
        let mut ac_values = Vec::new();

        let mut acc = vec![0.0; n_coarse]; // Dense accumulator for one row.

        for ci in 0..n_coarse {
            // Zero the accumulator.
            for v in acc.iter_mut() {
                *v = 0.0;
            }

            // R(ci, :) is stored in r_row_ptr/r_col_idx/r_values.
            for r_idx in level.r_row_ptr[ci]..level.r_row_ptr[ci + 1] {
                let fi = level.r_col_idx[r_idx]; // fine index
                let r_val = level.r_values[r_idx];

                // A(fi, :) * P -> for each entry a_fj in row fi of A:
                for a_idx in level.a_row_ptr[fi]..level.a_row_ptr[fi + 1] {
                    let fj = level.a_col_idx[a_idx];
                    let a_val = level.a_values[a_idx];

                    // P(fj, :) -> for each entry p_val in row fj of P:
                    for p_idx in level.p_row_ptr[fj]..level.p_row_ptr[fj + 1] {
                        let cj = level.p_col_idx[p_idx];
                        let p_val = level.p_values[p_idx];
                        acc[cj] += r_val * a_val * p_val;
                    }
                }
            }

            // Extract non-zeros from accumulator.
            for cj in 0..n_coarse {
                if acc[cj].abs() > 1e-300 {
                    ac_col_idx.push(cj);
                    ac_values.push(acc[cj]);
                }
            }
            ac_row_ptr[ci + 1] = ac_col_idx.len();
        }

        (ac_row_ptr, ac_col_idx, ac_values)
    }
}

impl Default for AMG {
    fn default() -> Self {
        Self::new()
    }
}

impl PreconditionerTrait for AMG {
    fn setup(&mut self, a: &SparseMatrix) -> Result<()> {
        let n = a.nrows;
        if n != a.ncols {
            return Err(LinalgError::DimensionMismatch(format!(
                "AMG requires square matrix, got {}x{}",
                a.nrows, a.ncols
            )));
        }

        self.levels.clear();

        // Build hierarchy.
        let mut cur_row_ptr = a.row_ptr.clone();
        let mut cur_col_idx = a.col_idx.clone();
        let mut cur_values = a.values.clone();
        let mut cur_n = n;

        for _ in 0..self.max_levels {
            if cur_n <= self.min_coarse_size {
                break;
            }

            let level = Self::build_level(
                &cur_row_ptr,
                &cur_col_idx,
                &cur_values,
                cur_n,
                self.theta,
            );

            match level {
                Some(lev) => {
                    // Count coarse points.
                    let n_coarse = lev.r_row_ptr.len() - 1;

                    // Compute coarse operator: A_c = R * A * P.
                    let (ac_rp, ac_ci, ac_val) =
                        Self::compute_coarse_operator(&lev, n_coarse);

                    self.levels.push(lev);

                    cur_row_ptr = ac_rp;
                    cur_col_idx = ac_ci;
                    cur_values = ac_val;
                    cur_n = n_coarse;
                }
                None => break,
            }
        }

        // Store coarsest level as dense for direct solve.
        self.coarse_n = cur_n;
        self.coarse_dense = vec![0.0; cur_n * cur_n];
        for i in 0..cur_n {
            for idx in cur_row_ptr[i]..cur_row_ptr[i + 1] {
                self.coarse_dense[i * cur_n + cur_col_idx[idx]] = cur_values[idx];
            }
        }

        Ok(())
    }

    fn apply(&self, r: &[f64], z: &mut [f64]) -> Result<()> {
        let n = r.len();
        if z.len() != n {
            return Err(LinalgError::DimensionMismatch(format!(
                "AMG apply: r has {} elements, z has {} elements",
                r.len(),
                z.len()
            )));
        }

        // If no levels were built, fall back to simple diagonal scaling.
        if self.levels.is_empty() {
            // Just copy r to z (identity preconditioner).
            z.copy_from_slice(r);
            return Ok(());
        }

        // Check dimension matches finest level.
        if n != self.levels[0].n {
            return Err(LinalgError::DimensionMismatch(format!(
                "AMG apply: expected vector of length {}, got {}",
                self.levels[0].n, n
            )));
        }

        // Initialize z to zero and apply V-cycle.
        for zi in z.iter_mut() {
            *zi = 0.0;
        }
        self.vcycle(0, r, z);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::PreconditionerTrait;

    fn make_tridiagonal(n: usize) -> SparseMatrix {
        let mut row_ptr = vec![0usize];
        let mut col_idx = Vec::new();
        let mut values = Vec::new();

        for i in 0..n {
            if i > 0 {
                col_idx.push(i - 1);
                values.push(-1.0);
            }
            col_idx.push(i);
            values.push(4.0);
            if i < n - 1 {
                col_idx.push(i + 1);
                values.push(-1.0);
            }
            row_ptr.push(col_idx.len());
        }

        SparseMatrix::new(n, n, row_ptr, col_idx, values).unwrap()
    }

    #[test]
    fn amg_setup_succeeds() {
        let a = make_tridiagonal(20);
        let mut amg = AMG::new();
        amg.setup(&a).unwrap();
        // Should have at least one level.
        assert!(!amg.levels.is_empty());
    }

    #[test]
    fn amg_apply_basic() {
        let a = make_tridiagonal(20);
        let mut amg = AMG::new();
        amg.setup(&a).unwrap();

        let r = vec![1.0; 20];
        let mut z = vec![0.0; 20];
        amg.apply(&r, &mut z).unwrap();

        // z should not be all zeros (preconditioner should produce something).
        let z_norm: f64 = z.iter().map(|v| v * v).sum::<f64>().sqrt();
        assert!(z_norm > 1e-10, "AMG produced zero output");
    }

    #[test]
    fn amg_reduces_residual() {
        // Use AMG as a preconditioner inside a simple iterative loop.
        let n = 20;
        let a = make_tridiagonal(n);
        let mut amg = AMG::new();
        amg.setup(&a).unwrap();

        let b = vec![1.0; n];
        let mut x = vec![0.0; n];

        // A few preconditioned Richardson iterations: x += M^{-1}(b - Ax)
        for _ in 0..20 {
            let mut ax = vec![0.0; n];
            a.spmv(&x, &mut ax).unwrap();
            let r: Vec<f64> = b.iter().zip(ax.iter()).map(|(bi, axi)| bi - axi).collect();
            let mut z = vec![0.0; n];
            amg.apply(&r, &mut z).unwrap();
            for i in 0..n {
                x[i] += z[i];
            }
        }

        // Check residual is reduced.
        let mut ax = vec![0.0; n];
        a.spmv(&x, &mut ax).unwrap();
        let residual: f64 = b
            .iter()
            .zip(ax.iter())
            .map(|(bi, axi)| (bi - axi).powi(2))
            .sum::<f64>()
            .sqrt();
        let b_norm: f64 = b.iter().map(|v| v * v).sum::<f64>().sqrt();
        assert!(
            residual / b_norm < 0.1,
            "AMG Richardson did not reduce residual sufficiently: {}",
            residual / b_norm
        );
    }

    #[test]
    fn amg_small_matrix() {
        // Very small matrix: should fall back gracefully.
        let a = make_tridiagonal(3);
        let mut amg = AMG::new();
        amg.setup(&a).unwrap();

        let r = vec![1.0, 2.0, 3.0];
        let mut z = vec![0.0; 3];
        amg.apply(&r, &mut z).unwrap();
        // Should not crash.
    }

    #[test]
    fn amg_diagonal_matrix() {
        let row_ptr = vec![0, 1, 2, 3, 4, 5];
        let col_idx = vec![0, 1, 2, 3, 4];
        let values = vec![2.0, 3.0, 4.0, 5.0, 6.0];
        let a = SparseMatrix::new(5, 5, row_ptr, col_idx, values).unwrap();

        let mut amg = AMG::new();
        amg.setup(&a).unwrap();

        let r = vec![2.0, 6.0, 12.0, 20.0, 30.0];
        let mut z = vec![0.0; 5];
        amg.apply(&r, &mut z).unwrap();

        // For diagonal matrix, AMG should produce reasonable approximation.
        let z_norm: f64 = z.iter().map(|v| v * v).sum::<f64>().sqrt();
        assert!(z_norm > 1e-10, "AMG produced zero output for diagonal matrix");
    }
}
