//! Boundary condition application to assembled linear systems.

use gfd_core::LinearSystem;

/// Apply a Dirichlet boundary condition to the linear system.
///
/// Sets the specified row to an identity equation: `1 * x[row] = value`.
/// All off-diagonal entries in the row are zeroed, the diagonal is set to 1,
/// and the RHS is set to the prescribed value.
///
/// # Arguments
/// * `system` - The linear system to modify.
/// * `row` - The DOF/cell index to constrain.
/// * `value` - The prescribed Dirichlet value.
pub fn apply_dirichlet(system: &mut LinearSystem, row: usize, value: f64) {
    let a = &mut system.a;

    // Zero all entries in this row and set diagonal to 1.
    let start = a.row_ptr[row];
    let end = a.row_ptr[row + 1];

    for idx in start..end {
        if a.col_idx[idx] == row {
            a.values[idx] = 1.0;
        } else {
            a.values[idx] = 0.0;
        }
    }

    // Set the RHS to the prescribed value.
    system.b[row] = value;
}

/// Apply a Neumann boundary condition to the linear system.
///
/// Adds the specified flux to the right-hand side of the equation at `row`.
/// This corresponds to a prescribed flux (gradient) boundary condition.
///
/// # Arguments
/// * `system` - The linear system to modify.
/// * `row` - The DOF/cell index adjacent to the Neumann boundary.
/// * `flux` - The prescribed flux value (already scaled by face area).
pub fn apply_neumann(system: &mut LinearSystem, row: usize, flux: f64) {
    system.b[row] += flux;
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::SparseMatrix;

    fn make_test_system() -> LinearSystem {
        // 3x3 tridiagonal: [4 -1 0; -1 4 -1; 0 -1 4], b = [10, 0, 10]
        let row_ptr = vec![0, 2, 5, 7];
        let col_idx = vec![0, 1, 0, 1, 2, 1, 2];
        let values = vec![4.0, -1.0, -1.0, 4.0, -1.0, -1.0, 4.0];
        let a = SparseMatrix::new(3, 3, row_ptr, col_idx, values).unwrap();
        LinearSystem::new(a, vec![10.0, 0.0, 10.0])
    }

    #[test]
    fn dirichlet_sets_identity_row() {
        let mut sys = make_test_system();
        apply_dirichlet(&mut sys, 0, 100.0);

        // Row 0 should now be [1, 0, ...] with b[0] = 100
        assert!((sys.b[0] - 100.0).abs() < 1e-12);

        // Check that diagonal is 1 and off-diag is 0.
        let start = sys.a.row_ptr[0];
        let end = sys.a.row_ptr[1];
        for idx in start..end {
            if sys.a.col_idx[idx] == 0 {
                assert!((sys.a.values[idx] - 1.0).abs() < 1e-12);
            } else {
                assert!((sys.a.values[idx]).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn neumann_adds_flux() {
        let mut sys = make_test_system();
        apply_neumann(&mut sys, 2, 5.0);
        assert!((sys.b[2] - 15.0).abs() < 1e-12);
    }
}
