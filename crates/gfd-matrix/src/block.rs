//! Block matrix structures for coupled multi-equation systems.

use gfd_core::SparseMatrix;

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
}

/// Assembler for block (coupled) linear systems (stub).
#[derive(Debug)]
pub struct BlockAssembler {
    /// Number of equations in the coupled system.
    pub n_equations: usize,
    /// Number of cells per equation.
    pub block_size: usize,
}

impl BlockAssembler {
    /// Creates a new block assembler.
    pub fn new(n_equations: usize, block_size: usize) -> Self {
        Self {
            n_equations,
            block_size,
        }
    }

    /// Finalize and produce a BlockMatrix (stub).
    pub fn finalize(self) -> BlockMatrix {
        BlockMatrix::new(self.n_equations, self.block_size)
    }
}
