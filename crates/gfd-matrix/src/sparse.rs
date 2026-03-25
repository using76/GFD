//! Coordinate (COO) sparse matrix format and conversion to CSR.
//!
//! The CSR `SparseMatrix` type lives in `gfd_core::linalg`. This module
//! provides a COO builder that accumulates triplets and converts to CSR.

use gfd_core::SparseMatrix;

/// A sparse matrix in COOrdinate (triplet) format.
///
/// Entries can be added incrementally; duplicate (i, j) entries are summed
/// when converting to CSR.
#[derive(Debug, Clone)]
pub struct CooMatrix {
    /// Triplets: (row, col, value).
    pub triplets: Vec<(usize, usize, f64)>,
    /// Number of rows.
    pub nrows: usize,
    /// Number of columns.
    pub ncols: usize,
}

impl CooMatrix {
    /// Creates a new empty COO matrix with the given dimensions.
    pub fn new(nrows: usize, ncols: usize) -> Self {
        Self {
            triplets: Vec::new(),
            nrows,
            ncols,
        }
    }

    /// Creates a COO matrix with pre-allocated capacity.
    pub fn with_capacity(nrows: usize, ncols: usize, capacity: usize) -> Self {
        Self {
            triplets: Vec::with_capacity(capacity),
            nrows,
            ncols,
        }
    }

    /// Adds a single entry to the matrix. Duplicate entries are summed
    /// during conversion to CSR.
    pub fn add_entry(&mut self, row: usize, col: usize, value: f64) {
        debug_assert!(row < self.nrows, "row {} out of bounds (nrows={})", row, self.nrows);
        debug_assert!(col < self.ncols, "col {} out of bounds (ncols={})", col, self.ncols);
        self.triplets.push((row, col, value));
    }

    /// Returns the number of stored triplets (including duplicates).
    pub fn nnz_stored(&self) -> usize {
        self.triplets.len()
    }

    /// Converts this COO matrix to CSR format (`gfd_core::SparseMatrix`).
    ///
    /// Duplicate entries at the same (row, col) are summed.
    /// Uses counting sort by row for O(nnz + nrows) performance.
    pub fn to_csr(&self) -> SparseMatrix {
        if self.triplets.is_empty() {
            return SparseMatrix::zeros(self.nrows, self.ncols);
        }

        let nt = self.triplets.len();

        // Step 1: Count entries per row
        let mut row_counts = vec![0usize; self.nrows];
        for &(r, _, _) in &self.triplets {
            row_counts[r] += 1;
        }

        // Step 2: Build row_ptr from counts
        let mut row_ptr = vec![0usize; self.nrows + 1];
        for i in 0..self.nrows {
            row_ptr[i + 1] = row_ptr[i] + row_counts[i];
        }

        // Step 3: Place triplets into row-ordered arrays using counting sort
        let mut col_raw = vec![0usize; nt];
        let mut val_raw = vec![0.0f64; nt];
        let mut cursor = row_ptr[..self.nrows].to_vec(); // write cursor per row
        for &(r, c, v) in &self.triplets {
            let pos = cursor[r];
            col_raw[pos] = c;
            val_raw[pos] = v;
            cursor[r] += 1;
        }

        // Step 4: Sort within each row by column and sum duplicates
        let mut col_idx = Vec::with_capacity(nt);
        let mut values = Vec::with_capacity(nt);
        let mut new_row_ptr = vec![0usize; self.nrows + 1];

        for i in 0..self.nrows {
            let start = row_ptr[i];
            let end = row_ptr[i + 1];
            if start == end {
                new_row_ptr[i + 1] = col_idx.len();
                continue;
            }

            // Sort this row's entries by column
            let row_slice = &mut col_raw[start..end];
            let val_slice = &mut val_raw[start..end];
            // Simple insertion sort (rows are typically small, ~5-7 entries)
            for j in 1..(end - start) {
                let key_c = row_slice[j];
                let key_v = val_slice[j];
                let mut k = j;
                while k > 0 && row_slice[k - 1] > key_c {
                    row_slice[k] = row_slice[k - 1];
                    val_slice[k] = val_slice[k - 1];
                    k -= 1;
                }
                row_slice[k] = key_c;
                val_slice[k] = key_v;
            }

            // Merge duplicates
            let mut prev_col = row_slice[0];
            let mut prev_val = val_slice[0];
            for j in 1..(end - start) {
                if row_slice[j] == prev_col {
                    prev_val += val_slice[j];
                } else {
                    col_idx.push(prev_col);
                    values.push(prev_val);
                    prev_col = row_slice[j];
                    prev_val = val_slice[j];
                }
            }
            col_idx.push(prev_col);
            values.push(prev_val);
            new_row_ptr[i + 1] = col_idx.len();
        }

        SparseMatrix::new(self.nrows, self.ncols, new_row_ptr, col_idx, values)
            .expect("COO to CSR conversion produced invalid CSR data")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_coo_to_csr() {
        let mut coo = CooMatrix::new(3, 3);
        coo.add_entry(0, 0, 4.0);
        coo.add_entry(0, 1, -1.0);
        coo.add_entry(1, 0, -1.0);
        coo.add_entry(1, 1, 4.0);
        coo.add_entry(1, 2, -1.0);
        coo.add_entry(2, 1, -1.0);
        coo.add_entry(2, 2, 4.0);

        let csr = coo.to_csr();
        assert_eq!(csr.nrows, 3);
        assert_eq!(csr.ncols, 3);
        assert_eq!(csr.nnz(), 7);
    }

    #[test]
    fn duplicate_entries_summed() {
        let mut coo = CooMatrix::new(2, 2);
        coo.add_entry(0, 0, 1.0);
        coo.add_entry(0, 0, 2.0);
        coo.add_entry(1, 1, 5.0);

        let csr = coo.to_csr();
        assert_eq!(csr.nnz(), 2);
        // Check value at (0,0) = 3.0
        let diag = csr.diagonal();
        assert!((diag[0] - 3.0).abs() < 1e-12);
        assert!((diag[1] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn empty_matrix() {
        let coo = CooMatrix::new(5, 5);
        let csr = coo.to_csr();
        assert_eq!(csr.nnz(), 0);
        assert_eq!(csr.nrows, 5);
    }
}
