//! Linear system assembler: collects cell equations and produces a LinearSystem.

use gfd_core::LinearSystem;
use crate::sparse::CooMatrix;
use crate::Result;

/// Assembles discrete cell equations into a global linear system.
///
/// Usage:
/// 1. Create with `Assembler::new(num_cells)`.
/// 2. Call `add_cell_equation` for each cell.
/// 3. Call `finalize()` to obtain the `LinearSystem`.
#[derive(Debug)]
pub struct Assembler {
    coo: CooMatrix,
    rhs: Vec<f64>,
    num_cells: usize,
}

impl Assembler {
    /// Creates a new assembler for a system with `num_cells` unknowns.
    pub fn new(num_cells: usize) -> Self {
        // Estimate ~7 non-zeros per row (typical for 3D hex meshes).
        let estimated_nnz = num_cells * 7;
        Self {
            coo: CooMatrix::with_capacity(num_cells, num_cells, estimated_nnz),
            rhs: vec![0.0; num_cells],
            num_cells,
        }
    }

    /// Creates a new assembler with a precise NNZ estimate.
    /// Use `num_cells + 2 * num_internal_faces` for FVM on unstructured meshes.
    pub fn with_nnz_estimate(num_cells: usize, nnz_estimate: usize) -> Self {
        Self {
            coo: CooMatrix::with_capacity(num_cells, num_cells, nnz_estimate),
            rhs: vec![0.0; num_cells],
            num_cells,
        }
    }

    /// Adds a single cell equation to the system.
    ///
    /// # Arguments
    /// * `cell_id` - Row index (cell index).
    /// * `a_p` - Diagonal coefficient for this cell.
    /// * `neighbors` - Slice of (neighbor_cell_id, coefficient) pairs.
    /// * `source` - Right-hand side contribution for this cell.
    pub fn add_cell_equation(
        &mut self,
        cell_id: usize,
        a_p: f64,
        neighbors: &[(usize, f64)],
        source: f64,
    ) {
        // Diagonal entry.
        self.coo.add_entry(cell_id, cell_id, a_p);

        // Off-diagonal neighbor entries.
        for &(nb_id, coeff) in neighbors {
            self.coo.add_entry(cell_id, nb_id, -coeff);
        }

        // Right-hand side.
        self.rhs[cell_id] += source;
    }

    /// Adds a diagonal coefficient to a cell.
    pub fn add_diagonal(&mut self, cell_id: usize, value: f64) {
        self.coo.add_entry(cell_id, cell_id, value);
    }

    /// Adds an off-diagonal coefficient (negative sign applied internally).
    pub fn add_neighbor(&mut self, cell_id: usize, neighbor_id: usize, coeff: f64) {
        self.coo.add_entry(cell_id, neighbor_id, -coeff);
    }

    /// Adds to the right-hand side of a cell.
    pub fn add_source(&mut self, cell_id: usize, value: f64) {
        self.rhs[cell_id] += value;
    }

    /// Returns the number of cells (unknowns).
    pub fn num_cells(&self) -> usize {
        self.num_cells
    }

    /// Finalize assembly and return the linear system.
    ///
    /// Converts the accumulated COO entries to CSR format and packages
    /// them into a `LinearSystem`.
    pub fn finalize(self) -> Result<LinearSystem> {
        let csr = self.coo.to_csr();
        Ok(LinearSystem::new(csr, self.rhs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assemble_simple_system() {
        let mut asm = Assembler::new(3);

        // Cell 0: 4*phi_0 - 1*phi_1 = 10
        asm.add_cell_equation(0, 4.0, &[(1, 1.0)], 10.0);
        // Cell 1: -1*phi_0 + 4*phi_1 - 1*phi_2 = 0
        asm.add_cell_equation(1, 4.0, &[(0, 1.0), (2, 1.0)], 0.0);
        // Cell 2: -1*phi_1 + 4*phi_2 = 10
        asm.add_cell_equation(2, 4.0, &[(1, 1.0)], 10.0);

        let system = asm.finalize().unwrap();
        assert_eq!(system.size(), 3);
        assert_eq!(system.a.nnz(), 7);
    }
}
