//! Gradient computation at cell centers for FVM discretization.

use gfd_core::UnstructuredMesh;

use crate::Result;

/// Trait for FVM gradient computation at cell centers.
///
/// Implementations compute the gradient of a scalar field, returning
/// a vector of 3D gradient vectors (one per cell).
pub trait FvmGradient {
    /// Compute gradients at cell centers.
    ///
    /// # Arguments
    /// * `cell_values` - Scalar field values at cell centers.
    ///
    /// # Returns
    /// A vector of gradient vectors `[dφ/dx, dφ/dy, dφ/dz]` per cell.
    fn compute(&self, cell_values: &[f64]) -> Result<Vec<[f64; 3]>>;
}

/// Green-Gauss cell-based gradient computation.
///
/// Stores a clone of the mesh so that `compute` can access face/cell topology.
/// The algorithm loops over every face, computes a face-averaged scalar value,
/// and accumulates `φ_f * A_f * n_f` into the owning (and neighboring) cell.
/// Each gradient is finally divided by the cell volume.
#[derive(Debug, Clone)]
pub struct GreenGaussCellGradient {
    mesh: UnstructuredMesh,
}

impl GreenGaussCellGradient {
    /// Create a new Green-Gauss cell gradient computer for the given mesh.
    pub fn new(mesh: &UnstructuredMesh) -> Self {
        Self { mesh: mesh.clone() }
    }
}

impl FvmGradient for GreenGaussCellGradient {
    fn compute(&self, cell_values: &[f64]) -> Result<Vec<[f64; 3]>> {
        let num_cells = self.mesh.num_cells();
        let mut gradients = vec![[0.0_f64; 3]; num_cells];

        // Loop over all faces and accumulate φ_f * A_f * n_f
        for face in &self.mesh.faces {
            let owner = face.owner_cell;

            if let Some(neighbor) = face.neighbor_cell {
                // Internal face: φ_f = 0.5 * (φ_owner + φ_neighbor)
                let phi_f_area = 0.5 * (cell_values[owner] + cell_values[neighbor]) * face.area;

                let c0 = phi_f_area * face.normal[0];
                let c1 = phi_f_area * face.normal[1];
                let c2 = phi_f_area * face.normal[2];
                gradients[owner][0] += c0;
                gradients[owner][1] += c1;
                gradients[owner][2] += c2;
                gradients[neighbor][0] -= c0;
                gradients[neighbor][1] -= c1;
                gradients[neighbor][2] -= c2;
            } else {
                // Boundary face: φ_f = φ_owner (zero-gradient extrapolation)
                let phi_f_area = cell_values[owner] * face.area;

                gradients[owner][0] += phi_f_area * face.normal[0];
                gradients[owner][1] += phi_f_area * face.normal[1];
                gradients[owner][2] += phi_f_area * face.normal[2];
            }
        }

        // Divide each gradient by cell volume: grad(φ) ≈ (1/V) Σ φ_f A_f n_f
        for i in 0..num_cells {
            let vol = self.mesh.cells[i].volume;
            for dim in 0..3 {
                gradients[i][dim] /= vol;
            }
        }

        Ok(gradients)
    }
}

/// Least-squares gradient computation.
///
/// Stores a clone of the mesh so that `compute` can access cell centers
/// and neighbor connectivity. For each cell, builds the weighted normal
/// equations A^T W A g = A^T W b where rows correspond to neighbors and
/// solves the 3×3 system via Gauss elimination with partial pivoting.
#[derive(Debug, Clone)]
pub struct LeastSquaresGradient {
    mesh: UnstructuredMesh,
}

impl LeastSquaresGradient {
    /// Create a new least-squares gradient computer for the given mesh.
    pub fn new(mesh: &UnstructuredMesh) -> Self {
        Self { mesh: mesh.clone() }
    }
}

impl FvmGradient for LeastSquaresGradient {
    fn compute(&self, cell_values: &[f64]) -> Result<Vec<[f64; 3]>> {
        let num_cells = self.mesh.num_cells();
        let mut gradients = vec![[0.0_f64; 3]; num_cells];

        for cell_id in 0..num_cells {
            let xp = self.mesh.cells[cell_id].center;

            // Build normal equations: A^T W A g = A^T W b
            // A has rows = (x_N - x_P), b = φ_N - φ_P, w = 1/|d|^2
            let mut ata = [[0.0_f64; 3]; 3];
            let mut atb = [0.0_f64; 3];

            for &face_id in &self.mesh.cells[cell_id].faces {
                let face = &self.mesh.faces[face_id];
                if let Some(neighbor) = face.neighbor_cell {
                    let other = if neighbor == cell_id {
                        face.owner_cell
                    } else {
                        neighbor
                    };
                    let xn = self.mesh.cells[other].center;
                    let dx = [xn[0] - xp[0], xn[1] - xp[1], xn[2] - xp[2]];
                    let dphi = cell_values[other] - cell_values[cell_id];
                    let dist_sq = dx[0] * dx[0] + dx[1] * dx[1] + dx[2] * dx[2];
                    let w = if dist_sq > 1e-30 { 1.0 / dist_sq } else { 1.0 };

                    for i in 0..3 {
                        for j in 0..3 {
                            ata[i][j] += w * dx[i] * dx[j];
                        }
                        atb[i] += w * dx[i] * dphi;
                    }
                }
            }

            // Solve 3×3 system via Gauss elimination with partial pivoting
            let mut aug = [[0.0_f64; 4]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    aug[i][j] = ata[i][j];
                }
                aug[i][3] = atb[i];
            }

            for k in 0..3 {
                // Partial pivoting
                let mut max_val = aug[k][k].abs();
                let mut max_row = k;
                for i in (k + 1)..3 {
                    if aug[i][k].abs() > max_val {
                        max_val = aug[i][k].abs();
                        max_row = i;
                    }
                }
                if max_row != k {
                    aug.swap(k, max_row);
                }

                let pivot = aug[k][k];
                if pivot.abs() < 1e-30 {
                    continue;
                }

                for i in (k + 1)..3 {
                    let factor = aug[i][k] / pivot;
                    for j in k..4 {
                        aug[i][j] -= factor * aug[k][j];
                    }
                }
            }

            let mut g = [0.0_f64; 3];
            for k in (0..3).rev() {
                if aug[k][k].abs() < 1e-30 {
                    continue;
                }
                let mut s = aug[k][3];
                for j in (k + 1)..3 {
                    s -= aug[k][j] * g[j];
                }
                g[k] = s / aug[k][k];
            }

            gradients[cell_id] = g;
        }

        Ok(gradients)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::cell::Cell;
    use gfd_core::mesh::face::Face;

    /// Creates a 3×1×1 hex mesh manually.
    ///
    /// 3 cells along x-axis, each 1×1×1. Domain: [0,3]×[0,1]×[0,1].
    /// Cell centers: (0.5,0.5,0.5), (1.5,0.5,0.5), (2.5,0.5,0.5).
    fn make_3x1x1_mesh() -> UnstructuredMesh {
        let cells = vec![
            Cell::new(0, vec![], vec![0, 4, 5, 8, 9, 12, 13], 1.0, [0.5, 0.5, 0.5]),
            Cell::new(1, vec![], vec![1, 2, 6, 7, 10, 11, 14, 15], 1.0, [1.5, 0.5, 0.5]),
            Cell::new(2, vec![], vec![3, 2, 6, 7, 10, 11, 14, 15], 1.0, [2.5, 0.5, 0.5]),
        ];

        let mut faces = Vec::new();
        let mut fid = 0;

        // X-normal faces
        faces.push(Face::new(fid, vec![], 0, None, 1.0, [-1.0, 0.0, 0.0], [0.0, 0.5, 0.5]));
        fid += 1;
        faces.push(Face::new(fid, vec![], 0, Some(1), 1.0, [1.0, 0.0, 0.0], [1.0, 0.5, 0.5]));
        fid += 1;
        faces.push(Face::new(fid, vec![], 1, Some(2), 1.0, [1.0, 0.0, 0.0], [2.0, 0.5, 0.5]));
        fid += 1;
        faces.push(Face::new(fid, vec![], 2, None, 1.0, [1.0, 0.0, 0.0], [3.0, 0.5, 0.5]));
        fid += 1;

        // Y-normal faces
        for i in 0..3 {
            let cx = i as f64 + 0.5;
            faces.push(Face::new(fid, vec![], i, None, 1.0, [0.0, -1.0, 0.0], [cx, 0.0, 0.5]));
            fid += 1;
            faces.push(Face::new(fid, vec![], i, None, 1.0, [0.0, 1.0, 0.0], [cx, 1.0, 0.5]));
            fid += 1;
        }

        // Z-normal faces
        for i in 0..3 {
            let cx = i as f64 + 0.5;
            faces.push(Face::new(fid, vec![], i, None, 1.0, [0.0, 0.0, -1.0], [cx, 0.5, 0.0]));
            fid += 1;
            faces.push(Face::new(fid, vec![], i, None, 1.0, [0.0, 0.0, 1.0], [cx, 0.5, 1.0]));
            fid += 1;
        }

        UnstructuredMesh::from_components(vec![], faces, cells, vec![])
    }

    #[test]
    fn green_gauss_constant_field() {
        let mesh = make_3x1x1_mesh();
        let grad = GreenGaussCellGradient::new(&mesh);
        let result = grad.compute(&[5.0, 5.0, 5.0]).unwrap();

        // Gradient of a constant field should be zero
        for i in 0..3 {
            assert!(result[i][0].abs() < 1e-10, "cell {} grad_x = {}", i, result[i][0]);
            assert!(result[i][1].abs() < 1e-10, "cell {} grad_y = {}", i, result[i][1]);
            assert!(result[i][2].abs() < 1e-10, "cell {} grad_z = {}", i, result[i][2]);
        }
    }

    #[test]
    fn green_gauss_linear_field() {
        let mesh = make_3x1x1_mesh();
        let grad = GreenGaussCellGradient::new(&mesh);
        // phi = x => cell values: 0.5, 1.5, 2.5
        let result = grad.compute(&[0.5, 1.5, 2.5]).unwrap();

        // Interior cell (cell 1) gradient should be ~[1, 0, 0]
        assert!(
            (result[1][0] - 1.0).abs() < 1e-10,
            "interior cell grad_x = {} (expected ~1.0)",
            result[1][0]
        );
        assert!(result[1][1].abs() < 1e-10);
        assert!(result[1][2].abs() < 1e-10);

        // Boundary cells: y and z components should be zero
        for i in [0, 2] {
            assert!(result[i][1].abs() < 1e-10);
            assert!(result[i][2].abs() < 1e-10);
        }
    }

    #[test]
    fn least_squares_constant_field() {
        let mesh = make_3x1x1_mesh();
        let grad = LeastSquaresGradient::new(&mesh);
        let result = grad.compute(&[3.0, 3.0, 3.0]).unwrap();

        for i in 0..3 {
            assert!(result[i][0].abs() < 1e-10, "cell {} grad_x = {}", i, result[i][0]);
            assert!(result[i][1].abs() < 1e-10, "cell {} grad_y = {}", i, result[i][1]);
            assert!(result[i][2].abs() < 1e-10, "cell {} grad_z = {}", i, result[i][2]);
        }
    }

    #[test]
    fn least_squares_linear_field() {
        let mesh = make_3x1x1_mesh();
        let grad = LeastSquaresGradient::new(&mesh);
        // phi = x => cell values: 0.5, 1.5, 2.5
        let result = grad.compute(&[0.5, 1.5, 2.5]).unwrap();

        // Interior cell (cell 1) should have gradient ≈ [1, 0, 0]
        assert!(
            (result[1][0] - 1.0).abs() < 1e-10,
            "interior cell grad_x = {} (expected ~1.0)",
            result[1][0]
        );
        assert!(result[1][1].abs() < 1e-10);
        assert!(result[1][2].abs() < 1e-10);
    }
}
