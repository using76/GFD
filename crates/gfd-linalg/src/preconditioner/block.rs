//! Block Jacobi preconditioner.
//!
//! Applies block-diagonal scaling where each block is inverted independently.
//! For block size 1, this reduces to standard Jacobi (diagonal) preconditioning.

use gfd_core::SparseMatrix;
use crate::{LinalgError, Result};
use crate::traits::PreconditionerTrait;

/// Block Jacobi preconditioner for coupled systems.
///
/// Partitions the matrix into square diagonal blocks and inverts each block
/// independently. The inverse blocks are stored dense since blocks are
/// typically small. For a system of size n with block_size b, there are
/// ceil(n/b) blocks.
#[derive(Debug, Clone)]
pub struct BlockPreconditioner {
    /// Block size (number of rows/columns per block).
    block_size: usize,
    /// Inverted diagonal blocks, stored row-major.
    /// blocks[k] contains the dense inverse of the k-th diagonal block.
    inv_blocks: Vec<Vec<f64>>,
    /// Total matrix dimension.
    n: usize,
}

impl BlockPreconditioner {
    /// Creates a new block preconditioner with block size 1 (equivalent to Jacobi).
    pub fn new() -> Self {
        Self {
            block_size: 1,
            inv_blocks: Vec::new(),
            n: 0,
        }
    }

    /// Creates a new block preconditioner with the specified block size.
    pub fn with_block_size(block_size: usize) -> Self {
        Self {
            block_size: if block_size == 0 { 1 } else { block_size },
            inv_blocks: Vec::new(),
            n: 0,
        }
    }

    /// Invert a small dense matrix in-place using Gauss-Jordan elimination
    /// with partial pivoting. Returns false if singular.
    fn invert_dense(mat: &mut [f64], size: usize) -> bool {
        // Build augmented matrix [A | I] stored as mat (left) and inv (right).
        let mut inv = vec![0.0; size * size];
        for i in 0..size {
            inv[i * size + i] = 1.0;
        }

        for k in 0..size {
            // Partial pivoting: find row with largest |a(i,k)| for i >= k.
            let mut max_val = mat[k * size + k].abs();
            let mut max_row = k;
            for i in (k + 1)..size {
                let val = mat[i * size + k].abs();
                if val > max_val {
                    max_val = val;
                    max_row = i;
                }
            }

            if max_val < 1e-300 {
                return false; // Singular block.
            }

            // Swap rows k and max_row in both mat and inv.
            if max_row != k {
                for j in 0..size {
                    mat.swap(k * size + j, max_row * size + j);
                    inv.swap(k * size + j, max_row * size + j);
                }
            }

            // Scale pivot row.
            let pivot = mat[k * size + k];
            for j in 0..size {
                mat[k * size + j] /= pivot;
                inv[k * size + j] /= pivot;
            }

            // Eliminate column k in all other rows.
            for i in 0..size {
                if i == k {
                    continue;
                }
                let factor = mat[i * size + k];
                for j in 0..size {
                    mat[i * size + j] -= factor * mat[k * size + j];
                    inv[i * size + j] -= factor * inv[k * size + j];
                }
            }
        }

        // Copy inverse back to mat.
        mat.copy_from_slice(&inv);
        true
    }
}

impl Default for BlockPreconditioner {
    fn default() -> Self {
        Self::new()
    }
}

impl PreconditionerTrait for BlockPreconditioner {
    fn setup(&mut self, a: &SparseMatrix) -> Result<()> {
        let n = a.nrows;
        if n != a.ncols {
            return Err(LinalgError::DimensionMismatch(format!(
                "Block preconditioner requires square matrix, got {}x{}",
                a.nrows, a.ncols
            )));
        }

        self.n = n;
        let bs = self.block_size;
        let n_blocks = (n + bs - 1) / bs; // ceil(n / bs)
        self.inv_blocks = Vec::with_capacity(n_blocks);

        for blk in 0..n_blocks {
            let row_start = blk * bs;
            let row_end = (row_start + bs).min(n);
            let actual_bs = row_end - row_start;

            // Extract the diagonal block as a dense matrix.
            let mut block = vec![0.0; actual_bs * actual_bs];

            for i in 0..actual_bs {
                let global_row = row_start + i;
                for idx in a.row_ptr[global_row]..a.row_ptr[global_row + 1] {
                    let global_col = a.col_idx[idx];
                    if global_col >= row_start && global_col < row_end {
                        let j = global_col - row_start;
                        block[i * actual_bs + j] = a.values[idx];
                    }
                }
            }

            // Invert the block.
            if !Self::invert_dense(&mut block, actual_bs) {
                return Err(LinalgError::PreconditionerError(format!(
                    "Block Jacobi: singular diagonal block at block {}",
                    blk
                )));
            }

            self.inv_blocks.push(block);
        }

        Ok(())
    }

    fn apply(&self, r: &[f64], z: &mut [f64]) -> Result<()> {
        if r.len() != self.n || z.len() != self.n {
            return Err(LinalgError::DimensionMismatch(format!(
                "Block preconditioner apply: expected vector of length {}, got r={} z={}",
                self.n,
                r.len(),
                z.len()
            )));
        }

        let bs = self.block_size;

        for (blk, inv_block) in self.inv_blocks.iter().enumerate() {
            let row_start = blk * bs;
            let row_end = (row_start + bs).min(self.n);
            let actual_bs = row_end - row_start;

            // z_block = inv_block * r_block
            for i in 0..actual_bs {
                let mut sum = 0.0;
                for j in 0..actual_bs {
                    sum += inv_block[i * actual_bs + j] * r[row_start + j];
                }
                z[row_start + i] = sum;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::PreconditionerTrait;

    #[test]
    fn block_jacobi_size1_matches_jacobi() {
        // Block size 1 should behave like standard Jacobi.
        let row_ptr = vec![0, 2, 5, 7];
        let col_idx = vec![0, 1, 0, 1, 2, 1, 2];
        let values = vec![4.0, -1.0, -1.0, 4.0, -1.0, -1.0, 4.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();

        let mut block_prec = BlockPreconditioner::with_block_size(1);
        block_prec.setup(&a).unwrap();

        let r = vec![4.0, 8.0, 12.0];
        let mut z = vec![0.0; 3];
        block_prec.apply(&r, &mut z).unwrap();

        // With block size 1, z[i] = r[i] / a[i][i]
        assert!((z[0] - 1.0).abs() < 1e-12);
        assert!((z[1] - 2.0).abs() < 1e-12);
        assert!((z[2] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn block_jacobi_size2() {
        // 4x4 matrix with 2x2 blocks.
        // [4 -1  0  0]
        // [-1  4  0  0]
        // [ 0  0  4 -1]
        // [ 0  0 -1  4]
        let row_ptr = vec![0, 2, 4, 6, 8];
        let col_idx = vec![0, 1, 0, 1, 2, 3, 2, 3];
        let values = vec![4.0, -1.0, -1.0, 4.0, 4.0, -1.0, -1.0, 4.0];
        let a = SparseMatrix::new(4, 4, row_ptr, col_idx, values).unwrap();

        let mut block_prec = BlockPreconditioner::with_block_size(2);
        block_prec.setup(&a).unwrap();
        assert_eq!(block_prec.inv_blocks.len(), 2);

        // r = [3, 2, 3, 2]
        // Block 0: [4 -1; -1 4]^{-1} * [3; 2]
        // inv = [4/15, 1/15; 1/15, 4/15]
        // z[0] = 4/15*3 + 1/15*2 = 14/15
        // z[1] = 1/15*3 + 4/15*2 = 11/15
        let r = vec![3.0, 2.0, 3.0, 2.0];
        let mut z = vec![0.0; 4];
        block_prec.apply(&r, &mut z).unwrap();

        assert!((z[0] - 14.0 / 15.0).abs() < 1e-12);
        assert!((z[1] - 11.0 / 15.0).abs() < 1e-12);
        assert!((z[2] - 14.0 / 15.0).abs() < 1e-12);
        assert!((z[3] - 11.0 / 15.0).abs() < 1e-12);
    }

    #[test]
    fn block_jacobi_full_block() {
        // Full-size block = exact inverse for the diagonal block.
        // 3x3 diagonal: [2 0 0; 0 4 0; 0 0 8]
        let row_ptr = vec![0, 1, 2, 3];
        let col_idx = vec![0, 1, 2];
        let values = vec![2.0, 4.0, 8.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();

        let mut block_prec = BlockPreconditioner::with_block_size(3);
        block_prec.setup(&a).unwrap();
        assert_eq!(block_prec.inv_blocks.len(), 1);

        let r = vec![10.0, 20.0, 40.0];
        let mut z = vec![0.0; 3];
        block_prec.apply(&r, &mut z).unwrap();

        assert!((z[0] - 5.0).abs() < 1e-12);
        assert!((z[1] - 5.0).abs() < 1e-12);
        assert!((z[2] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn block_jacobi_uneven_blocks() {
        // 5x5 matrix with block size 2: blocks of sizes [2, 2, 1].
        let row_ptr = vec![0, 1, 2, 3, 4, 5];
        let col_idx = vec![0, 1, 2, 3, 4];
        let values = vec![2.0, 3.0, 4.0, 5.0, 6.0];
        let a = SparseMatrix::new(5, 5, row_ptr, col_idx, values).unwrap();

        let mut block_prec = BlockPreconditioner::with_block_size(2);
        block_prec.setup(&a).unwrap();
        assert_eq!(block_prec.inv_blocks.len(), 3);

        let r = vec![2.0, 6.0, 12.0, 20.0, 30.0];
        let mut z = vec![0.0; 5];
        block_prec.apply(&r, &mut z).unwrap();

        assert!((z[0] - 1.0).abs() < 1e-12);
        assert!((z[1] - 2.0).abs() < 1e-12);
        assert!((z[2] - 3.0).abs() < 1e-12);
        assert!((z[3] - 4.0).abs() < 1e-12);
        assert!((z[4] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn block_jacobi_singular_block() {
        // Singular block should return error.
        let row_ptr = vec![0, 2, 4];
        let col_idx = vec![0, 1, 0, 1];
        let values = vec![1.0, 1.0, 1.0, 1.0]; // Singular 2x2 block.
        let a = SparseMatrix::new(2, 2, row_ptr, col_idx, values).unwrap();

        let mut block_prec = BlockPreconditioner::with_block_size(2);
        assert!(block_prec.setup(&a).is_err());
    }
}
