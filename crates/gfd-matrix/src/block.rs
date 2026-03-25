//! Block matrix structures for coupled multi-equation systems.
//!
//! For coupled systems (e.g., velocity-pressure, multi-species transport),
//! each cell has a `block_size x block_size` block of unknowns. The
//! [`BlockAssembler`] collects per-block contributions and produces a
//! [`BlockLinearSystem`] with a properly assembled [`BlockMatrix`].

use gfd_core::SparseMatrix;
use crate::sparse::CooMatrix;

/// A block matrix composed of sub-matrices for coupled equation systems.
///
/// For a system with `n_blocks` coupled equations, the block matrix is:
/// ```text
///   [ A_00  A_01  ...  A_0n ]   [ x_0 ]   [ b_0 ]
///   [ A_10  A_11  ...  A_1n ] * [ x_1 ] = [ b_1 ]
///   [  ...   ...  ...  ... ]   [ ... ]   [ ... ]
///   [ A_n0  A_n1  ...  A_nn ]   [ x_n ]   [ b_n ]
/// ```
#[derive(Debug, Clone)]
pub struct BlockMatrix {
    /// Block sub-matrices stored in row-major order: blocks[i * n_blocks + j] = A_ij.
    pub blocks: Vec<Option<SparseMatrix>>,
    /// Number of block rows (= block columns).
    pub n_blocks: usize,
    /// Size of each block (number of unknowns per equation).
    pub block_size: usize,
}

impl BlockMatrix {
    /// Creates a new block matrix with empty (None) blocks.
    pub fn new(n_blocks: usize, block_size: usize) -> Self {
        Self {
            blocks: vec![None; n_blocks * n_blocks],
            n_blocks,
            block_size,
        }
    }

    /// Sets the block at position (i, j).
    pub fn set_block(&mut self, i: usize, j: usize, matrix: SparseMatrix) {
        self.blocks[i * self.n_blocks + j] = Some(matrix);
    }

    /// Gets a reference to the block at position (i, j).
    pub fn get_block(&self, i: usize, j: usize) -> Option<&SparseMatrix> {
        self.blocks[i * self.n_blocks + j].as_ref()
    }

    /// Performs block matrix-vector multiplication: y = A * x.
    ///
    /// Both `x` and `y` are partitioned into `n_blocks` segments of length `block_size`.
    /// y_i = sum_j A_ij * x_j
    pub fn spmv(&self, x: &[f64], y: &mut [f64]) {
        let bs = self.block_size;
        let nb = self.n_blocks;
        assert_eq!(x.len(), nb * bs);
        assert_eq!(y.len(), nb * bs);

        // Zero output
        for v in y.iter_mut() {
            *v = 0.0;
        }

        let mut tmp = vec![0.0; bs];
        for i in 0..nb {
            for j in 0..nb {
                if let Some(ref mat) = self.blocks[i * nb + j] {
                    let x_j = &x[j * bs..(j + 1) * bs];
                    // Compute tmp = A_ij * x_j
                    for v in tmp.iter_mut() {
                        *v = 0.0;
                    }
                    mat.spmv(x_j, &mut tmp).unwrap();
                    // Accumulate into y_i
                    let y_i = &mut y[i * bs..(i + 1) * bs];
                    for k in 0..bs {
                        y_i[k] += tmp[k];
                    }
                }
            }
        }
    }
}

/// A block linear system: BlockMatrix * x = b.
#[derive(Debug, Clone)]
pub struct BlockLinearSystem {
    /// Block coefficient matrix.
    pub a: BlockMatrix,
    /// Right-hand side vector (length = n_blocks * block_size).
    pub b: Vec<f64>,
    /// Solution vector (length = n_blocks * block_size).
    pub x: Vec<f64>,
}

impl BlockLinearSystem {
    /// Creates a new block linear system with a zero initial guess.
    pub fn new(a: BlockMatrix, b: Vec<f64>) -> Self {
        let n = a.n_blocks * a.block_size;
        assert_eq!(b.len(), n, "RHS length must equal n_blocks * block_size");
        Self {
            a,
            b,
            x: vec![0.0; n],
        }
    }

    /// Returns the total number of unknowns.
    pub fn size(&self) -> usize {
        self.a.n_blocks * self.a.block_size
    }
}

/// Assembler for block (coupled) linear systems.
///
/// Collects contributions to each sub-block (i, j) of the block matrix
/// using COO format, then converts all blocks to CSR on finalization.
///
/// # Usage
/// ```rust,ignore
/// let mut asm = BlockAssembler::new(3, 100); // 3 equations, 100 cells
/// // Add diagonal block entry for equation 0, cell 5
/// asm.add_block_diagonal(0, 5, 4.0);
/// // Add off-diagonal neighbor in equation 0: cell 5 to cell 6
/// asm.add_block_neighbor(0, 5, 6, -1.0);
/// // Add coupling from equation 1 to equation 0 at cell 5
/// asm.add_coupling(0, 1, 5, 5, -0.5);
/// // Add source for equation 0, cell 5
/// asm.add_source(0, 5, 10.0);
/// let system = asm.finalize();
/// ```
#[derive(Debug)]
pub struct BlockAssembler {
    /// Number of equations in the coupled system.
    pub n_equations: usize,
    /// Number of cells per equation.
    pub block_size: usize,
    /// COO matrices for each sub-block (i, j), stored row-major.
    /// `coo_blocks[i * n_equations + j]` holds entries for block A_ij.
    coo_blocks: Vec<CooMatrix>,
    /// Right-hand side vectors, one per equation.
    rhs: Vec<Vec<f64>>,
}

impl BlockAssembler {
    /// Creates a new block assembler.
    pub fn new(n_equations: usize, block_size: usize) -> Self {
        let n_total = n_equations * n_equations;
        let mut coo_blocks = Vec::with_capacity(n_total);
        for _ in 0..n_total {
            // Estimate ~7 entries per row for diagonal blocks, fewer for off-diagonal
            coo_blocks.push(CooMatrix::with_capacity(block_size, block_size, block_size * 7));
        }
        let mut rhs = Vec::with_capacity(n_equations);
        for _ in 0..n_equations {
            rhs.push(vec![0.0; block_size]);
        }
        Self {
            n_equations,
            block_size,
            coo_blocks,
            rhs,
        }
    }

    /// Adds a diagonal coefficient to cell `cell_id` in equation `eq`.
    ///
    /// This contributes to block A_{eq, eq} at position (cell_id, cell_id).
    pub fn add_block_diagonal(&mut self, eq: usize, cell_id: usize, value: f64) {
        let idx = eq * self.n_equations + eq;
        self.coo_blocks[idx].add_entry(cell_id, cell_id, value);
    }

    /// Adds an off-diagonal neighbor coefficient within equation `eq`.
    ///
    /// This contributes to block A_{eq, eq} at position (cell_id, neighbor_id).
    pub fn add_block_neighbor(&mut self, eq: usize, cell_id: usize, neighbor_id: usize, coeff: f64) {
        let idx = eq * self.n_equations + eq;
        self.coo_blocks[idx].add_entry(cell_id, neighbor_id, coeff);
    }

    /// Adds a coupling coefficient between equation `eq_row` and equation `eq_col`.
    ///
    /// This contributes to block A_{eq_row, eq_col} at position (row_cell, col_cell).
    pub fn add_coupling(
        &mut self,
        eq_row: usize,
        eq_col: usize,
        row_cell: usize,
        col_cell: usize,
        value: f64,
    ) {
        let idx = eq_row * self.n_equations + eq_col;
        self.coo_blocks[idx].add_entry(row_cell, col_cell, value);
    }

    /// Adds a source (RHS) contribution for equation `eq` at cell `cell_id`.
    pub fn add_source(&mut self, eq: usize, cell_id: usize, value: f64) {
        self.rhs[eq][cell_id] += value;
    }

    /// Finalize and produce a [`BlockLinearSystem`].
    ///
    /// Converts all COO sub-blocks to CSR format and assembles the
    /// block matrix and concatenated RHS vector.
    pub fn finalize(self) -> BlockLinearSystem {
        let n_eq = self.n_equations;
        let bs = self.block_size;

        let mut block_matrix = BlockMatrix::new(n_eq, bs);

        for i in 0..n_eq {
            for j in 0..n_eq {
                let idx = i * n_eq + j;
                let coo = &self.coo_blocks[idx];
                if coo.nnz_stored() > 0 {
                    let csr = coo.to_csr();
                    block_matrix.set_block(i, j, csr);
                }
                // If no entries, leave as None (zero block)
            }
        }

        // Concatenate RHS vectors
        let mut b = Vec::with_capacity(n_eq * bs);
        for eq_rhs in &self.rhs {
            b.extend_from_slice(eq_rhs);
        }

        BlockLinearSystem::new(block_matrix, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_block_assembler() {
        let asm = BlockAssembler::new(2, 5);
        let system = asm.finalize();
        assert_eq!(system.size(), 10);
        assert_eq!(system.b.len(), 10);
        assert_eq!(system.x.len(), 10);
        // All blocks should be None (no entries added)
        assert!(system.a.get_block(0, 0).is_none());
        assert!(system.a.get_block(0, 1).is_none());
        assert!(system.a.get_block(1, 0).is_none());
        assert!(system.a.get_block(1, 1).is_none());
    }

    #[test]
    fn single_equation_block() {
        // Single equation with 3 cells: equivalent to a regular assembler
        let mut asm = BlockAssembler::new(1, 3);

        // Cell 0: 4*x0 - 1*x1 = 10
        asm.add_block_diagonal(0, 0, 4.0);
        asm.add_block_neighbor(0, 0, 1, -1.0);
        asm.add_source(0, 0, 10.0);

        // Cell 1: -1*x0 + 4*x1 - 1*x2 = 0
        asm.add_block_diagonal(0, 1, 4.0);
        asm.add_block_neighbor(0, 1, 0, -1.0);
        asm.add_block_neighbor(0, 1, 2, -1.0);
        asm.add_source(0, 1, 0.0);

        // Cell 2: -1*x1 + 4*x2 = 10
        asm.add_block_diagonal(0, 2, 4.0);
        asm.add_block_neighbor(0, 2, 1, -1.0);
        asm.add_source(0, 2, 10.0);

        let system = asm.finalize();
        assert_eq!(system.size(), 3);

        let a00 = system.a.get_block(0, 0).unwrap();
        assert_eq!(a00.nrows, 3);
        assert_eq!(a00.ncols, 3);
        assert_eq!(a00.nnz(), 7); // 3 diagonal + 4 off-diagonal

        // Check RHS
        assert!((system.b[0] - 10.0).abs() < 1e-12);
        assert!((system.b[1] - 0.0).abs() < 1e-12);
        assert!((system.b[2] - 10.0).abs() < 1e-12);
    }

    #[test]
    fn coupled_two_equation_system() {
        // 2 equations, 4 cells each (e.g., velocity x and y)
        let mut asm = BlockAssembler::new(2, 4);

        // Equation 0: diagonal entries for cells 0-3
        for cell in 0..4 {
            asm.add_block_diagonal(0, cell, 5.0);
            asm.add_source(0, cell, 1.0);
        }
        // Some neighbors in equation 0
        asm.add_block_neighbor(0, 0, 1, -1.0);
        asm.add_block_neighbor(0, 1, 0, -1.0);
        asm.add_block_neighbor(0, 1, 2, -1.0);
        asm.add_block_neighbor(0, 2, 1, -1.0);
        asm.add_block_neighbor(0, 2, 3, -1.0);
        asm.add_block_neighbor(0, 3, 2, -1.0);

        // Equation 1: diagonal entries for cells 0-3
        for cell in 0..4 {
            asm.add_block_diagonal(1, cell, 5.0);
            asm.add_source(1, cell, 2.0);
        }

        // Coupling: equation 0 depends on equation 1 (pressure-velocity coupling)
        for cell in 0..4 {
            asm.add_coupling(0, 1, cell, cell, -0.5);
        }

        let system = asm.finalize();
        assert_eq!(system.size(), 8); // 2 equations * 4 cells

        // Diagonal blocks should exist
        let a00 = system.a.get_block(0, 0).unwrap();
        assert_eq!(a00.nrows, 4);
        assert_eq!(a00.nnz(), 10); // 4 diagonal + 6 off-diagonal

        let a11 = system.a.get_block(1, 1).unwrap();
        assert_eq!(a11.nrows, 4);
        assert_eq!(a11.nnz(), 4); // 4 diagonal only

        // Coupling block (0, 1) should exist
        let a01 = system.a.get_block(0, 1).unwrap();
        assert_eq!(a01.nrows, 4);
        assert_eq!(a01.nnz(), 4); // 4 coupling entries

        // No coupling from eq 1 to eq 0
        assert!(system.a.get_block(1, 0).is_none());

        // Check RHS
        for i in 0..4 {
            assert!((system.b[i] - 1.0).abs() < 1e-12);      // eq 0
            assert!((system.b[4 + i] - 2.0).abs() < 1e-12);  // eq 1
        }
    }

    #[test]
    fn block_spmv() {
        // 2x2 block system, each block is 2x2
        let mut bm = BlockMatrix::new(2, 2);

        // A_00 = [[2, 0], [0, 2]]
        let a00 = SparseMatrix::new(
            2, 2,
            vec![0, 1, 2],
            vec![0, 1],
            vec![2.0, 2.0],
        ).unwrap();
        bm.set_block(0, 0, a00);

        // A_01 = [[1, 0], [0, 1]]
        let a01 = SparseMatrix::new(
            2, 2,
            vec![0, 1, 2],
            vec![0, 1],
            vec![1.0, 1.0],
        ).unwrap();
        bm.set_block(0, 1, a01);

        // A_11 = [[3, 0], [0, 3]]
        let a11 = SparseMatrix::new(
            2, 2,
            vec![0, 1, 2],
            vec![0, 1],
            vec![3.0, 3.0],
        ).unwrap();
        bm.set_block(1, 1, a11);

        // x = [1, 2, 3, 4]
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let mut y = vec![0.0; 4];
        bm.spmv(&x, &mut y);

        // y_0 = A_00 * [1,2] + A_01 * [3,4] = [2,4] + [3,4] = [5, 8]
        // y_1 = A_11 * [3,4] = [9, 12]
        assert!((y[0] - 5.0).abs() < 1e-12);
        assert!((y[1] - 8.0).abs() < 1e-12);
        assert!((y[2] - 9.0).abs() < 1e-12);
        assert!((y[3] - 12.0).abs() < 1e-12);
    }

    #[test]
    fn duplicate_entries_summed_in_block() {
        let mut asm = BlockAssembler::new(1, 2);

        // Add diagonal twice to same cell
        asm.add_block_diagonal(0, 0, 3.0);
        asm.add_block_diagonal(0, 0, 2.0);

        let system = asm.finalize();
        let a00 = system.a.get_block(0, 0).unwrap();
        let diag = a00.diagonal();
        assert!((diag[0] - 5.0).abs() < 1e-12); // 3.0 + 2.0
    }

    #[test]
    fn three_equation_system() {
        // 3 coupled equations (e.g., u, v, p), 2 cells each
        let mut asm = BlockAssembler::new(3, 2);

        for eq in 0..3 {
            for cell in 0..2 {
                asm.add_block_diagonal(eq, cell, 10.0);
                asm.add_source(eq, cell, (eq + 1) as f64);
            }
            // Neighbor connection within each equation
            asm.add_block_neighbor(eq, 0, 1, -2.0);
            asm.add_block_neighbor(eq, 1, 0, -2.0);
        }

        // Coupling: eq 0 <-> eq 2
        asm.add_coupling(0, 2, 0, 0, -1.0);
        asm.add_coupling(2, 0, 0, 0, -1.0);

        let system = asm.finalize();
        assert_eq!(system.size(), 6); // 3 * 2

        // All diagonal blocks should have entries
        for eq in 0..3 {
            let block = system.a.get_block(eq, eq).unwrap();
            assert_eq!(block.nrows, 2);
            assert_eq!(block.nnz(), 4); // 2 diag + 2 off-diag
        }

        // Coupling blocks
        assert!(system.a.get_block(0, 2).is_some());
        assert!(system.a.get_block(2, 0).is_some());
        assert!(system.a.get_block(0, 1).is_none()); // No coupling 0->1
        assert!(system.a.get_block(1, 0).is_none()); // No coupling 1->0

        // RHS: [1,1, 2,2, 3,3]
        assert!((system.b[0] - 1.0).abs() < 1e-12);
        assert!((system.b[1] - 1.0).abs() < 1e-12);
        assert!((system.b[2] - 2.0).abs() < 1e-12);
        assert!((system.b[3] - 2.0).abs() < 1e-12);
        assert!((system.b[4] - 3.0).abs() < 1e-12);
        assert!((system.b[5] - 3.0).abs() < 1e-12);
    }
}
