//! Gradient computation methods for fields on unstructured meshes.

use serde::{Deserialize, Serialize};

use crate::field::{ScalarField, VectorField};
use crate::mesh::unstructured::UnstructuredMesh;
use crate::Result;

/// Available gradient computation methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GradientMethod {
    /// Green-Gauss cell-based gradient (uses face-averaged values from cell neighbors).
    GreenGaussCellBased,
    /// Green-Gauss node-based gradient (interpolates to nodes first).
    GreenGaussNodeBased,
    /// Least-squares gradient reconstruction.
    LeastSquares,
}

/// Trait for gradient computation on unstructured meshes.
pub trait GradientComputer {
    /// Computes the gradient of a scalar field on the given mesh.
    ///
    /// Returns a VectorField where each entry is the gradient vector at
    /// the corresponding cell center.
    fn compute(&self, field: &ScalarField, mesh: &UnstructuredMesh) -> Result<VectorField>;

    /// Returns the gradient method used by this computer.
    fn method(&self) -> GradientMethod;
}

/// Green-Gauss cell-based gradient computer.
#[derive(Debug, Clone)]
pub struct GreenGaussCellBasedGradient;

impl GradientComputer for GreenGaussCellBasedGradient {
    fn compute(&self, field: &ScalarField, mesh: &UnstructuredMesh) -> Result<VectorField> {
        let num_cells = mesh.num_cells();
        let values = field.values();

        // Initialize gradients to zero for each cell
        let mut gradients = vec![[0.0_f64; 3]; num_cells];

        // Loop over all faces
        for face_id in 0..mesh.num_faces() {
            let face = &mesh.faces[face_id];
            let owner = face.owner_cell;

            if let Some(neighbor) = face.neighbor_cell {
                // Internal face: φ_f = 0.5 * (φ_owner + φ_neighbor)
                let phi_f = 0.5 * (values[owner] + values[neighbor]);

                // contribution = φ_f * area * normal (each component)
                for dim in 0..3 {
                    let contrib = phi_f * face.area * face.normal[dim];
                    gradients[owner][dim] += contrib;
                    gradients[neighbor][dim] -= contrib;
                }
            } else {
                // Boundary face: φ_f = φ_owner (zero-gradient extrapolation)
                let phi_f = values[owner];

                for dim in 0..3 {
                    let contrib = phi_f * face.area * face.normal[dim];
                    gradients[owner][dim] += contrib;
                }
            }
        }

        // Divide each gradient by cell volume
        for i in 0..num_cells {
            let vol = mesh.cells[i].volume;
            for dim in 0..3 {
                gradients[i][dim] /= vol;
            }
        }

        Ok(VectorField::from_vec("gradient", gradients))
    }

    fn method(&self) -> GradientMethod {
        GradientMethod::GreenGaussCellBased
    }
}

/// Green-Gauss node-based gradient computer.
#[derive(Debug, Clone)]
pub struct GreenGaussNodeBasedGradient;

impl GradientComputer for GreenGaussNodeBasedGradient {
    fn compute(&self, field: &ScalarField, mesh: &UnstructuredMesh) -> Result<VectorField> {
        let num_cells = mesh.num_cells();
        let num_nodes = mesh.nodes.len();
        let values = field.values();

        // Step 1: Interpolate cell values to nodes using inverse-distance weighting
        let mut node_values = vec![0.0_f64; num_nodes];
        let mut node_weights = vec![0.0_f64; num_nodes];

        for cell_id in 0..num_cells {
            let cell = &mesh.cells[cell_id];
            for &node_id in &cell.nodes {
                if node_id < num_nodes {
                    let dx = mesh.nodes[node_id].position[0] - cell.center[0];
                    let dy = mesh.nodes[node_id].position[1] - cell.center[1];
                    let dz = mesh.nodes[node_id].position[2] - cell.center[2];
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-30);
                    let w = 1.0 / dist;
                    node_values[node_id] += w * values[cell_id];
                    node_weights[node_id] += w;
                }
            }
        }

        for i in 0..num_nodes {
            if node_weights[i] > 1e-30 {
                node_values[i] /= node_weights[i];
            }
        }

        // Step 2: Compute face values from nodal averages, then apply Green-Gauss
        let mut gradients = vec![[0.0_f64; 3]; num_cells];

        for face_id in 0..mesh.num_faces() {
            let face = &mesh.faces[face_id];
            let owner = face.owner_cell;

            // Compute face value from node averages
            let mut phi_f = 0.0_f64;
            if !face.nodes.is_empty() {
                let mut count = 0;
                for &nid in &face.nodes {
                    if nid < num_nodes {
                        phi_f += node_values[nid];
                        count += 1;
                    }
                }
                if count > 0 {
                    phi_f /= count as f64;
                } else {
                    phi_f = values[owner];
                }
            } else {
                // No node info: fall back to cell-based
                if let Some(neighbor) = face.neighbor_cell {
                    phi_f = 0.5 * (values[owner] + values[neighbor]);
                } else {
                    phi_f = values[owner];
                }
            }

            for dim in 0..3 {
                let contrib = phi_f * face.area * face.normal[dim];
                gradients[owner][dim] += contrib;
                if let Some(neighbor) = face.neighbor_cell {
                    gradients[neighbor][dim] -= contrib;
                }
            }
        }

        // Divide by cell volume
        for i in 0..num_cells {
            let vol = mesh.cells[i].volume;
            for dim in 0..3 {
                gradients[i][dim] /= vol;
            }
        }

        Ok(VectorField::from_vec("gradient", gradients))
    }

    fn method(&self) -> GradientMethod {
        GradientMethod::GreenGaussNodeBased
    }
}

/// Least-squares gradient computer.
#[derive(Debug, Clone)]
pub struct LeastSquaresGradient;

impl GradientComputer for LeastSquaresGradient {
    fn compute(&self, field: &ScalarField, mesh: &UnstructuredMesh) -> Result<VectorField> {
        let num_cells = mesh.num_cells();
        let values = field.values();
        let mut gradients = vec![[0.0_f64; 3]; num_cells];

        // For each cell, solve min ||A*g - b||^2 using normal equations
        // A has rows = (x_N - x_P), b = phi_N - phi_P
        for cell_id in 0..num_cells {
            let xp = mesh.cells[cell_id].center;

            // Build the normal equation: A^T A g = A^T b
            // A^T A is 3x3, A^T b is 3x1
            let mut ata = [[0.0_f64; 3]; 3];
            let mut atb = [0.0_f64; 3];

            for &face_id in &mesh.cells[cell_id].faces {
                let face = &mesh.faces[face_id];
                if let Some(neighbor) = face.neighbor_cell {
                    let other = if neighbor == cell_id {
                        face.owner_cell
                    } else {
                        neighbor
                    };
                    let xn = mesh.cells[other].center;
                    let dx = [xn[0] - xp[0], xn[1] - xp[1], xn[2] - xp[2]];
                    let dphi = values[other] - values[cell_id];
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

            // Solve 3x3 system using Cramer's rule or Gauss elimination
            // Simple Gauss elimination on 3x3:
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
                if pivot.abs() < 1e-30 { continue; }

                for i in (k + 1)..3 {
                    let factor = aug[i][k] / pivot;
                    for j in k..4 {
                        aug[i][j] -= factor * aug[k][j];
                    }
                }
            }

            let mut g = [0.0_f64; 3];
            for k in (0..3).rev() {
                if aug[k][k].abs() < 1e-30 { continue; }
                let mut s = aug[k][3];
                for j in (k + 1)..3 {
                    s -= aug[k][j] * g[j];
                }
                g[k] = s / aug[k][k];
            }

            gradients[cell_id] = g;
        }

        Ok(VectorField::from_vec("gradient", gradients))
    }

    fn method(&self) -> GradientMethod {
        GradientMethod::LeastSquares
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::cell::Cell;
    use crate::mesh::face::Face;
    use crate::mesh::unstructured::UnstructuredMesh;

    /// Creates a 3x1x1 hex mesh manually.
    ///
    /// 3 cells along x-axis, each 1x1x1. Domain: [0,3] x [0,1] x [0,1].
    /// Cell centers: (0.5,0.5,0.5), (1.5,0.5,0.5), (2.5,0.5,0.5).
    fn make_3x1x1_mesh() -> UnstructuredMesh {
        // Cells
        let cells = vec![
            Cell::new(0, vec![], vec![0, 4, 5, 8, 9, 12, 13], 1.0, [0.5, 0.5, 0.5]),
            Cell::new(1, vec![], vec![1, 2, 6, 7, 10, 11, 14, 15], 1.0, [1.5, 0.5, 0.5]),
            Cell::new(2, vec![], vec![3, 2, 6, 7, 10, 11, 14, 15], 1.0, [2.5, 0.5, 0.5]),
        ];

        // Faces: 4 x-normal + 6 y-normal + 6 z-normal = 16
        let mut faces = Vec::new();
        let mut fid = 0;

        // X-normal faces (area = dy*dz = 1.0, normal = [1,0,0] or [-1,0,0])
        // Face at x=0 (boundary, owner=cell0, normal=[-1,0,0] outward from domain)
        faces.push(Face::new(fid, vec![], 0, None, 1.0, [-1.0, 0.0, 0.0], [0.0, 0.5, 0.5]));
        fid += 1;
        // Face at x=1 (internal, owner=cell0, neighbor=cell1, normal=[1,0,0])
        faces.push(Face::new(fid, vec![], 0, Some(1), 1.0, [1.0, 0.0, 0.0], [1.0, 0.5, 0.5]));
        fid += 1;
        // Face at x=2 (internal, owner=cell1, neighbor=cell2, normal=[1,0,0])
        faces.push(Face::new(fid, vec![], 1, Some(2), 1.0, [1.0, 0.0, 0.0], [2.0, 0.5, 0.5]));
        fid += 1;
        // Face at x=3 (boundary, owner=cell2, normal=[1,0,0])
        faces.push(Face::new(fid, vec![], 2, None, 1.0, [1.0, 0.0, 0.0], [3.0, 0.5, 0.5]));
        fid += 1;

        // Y-normal faces (area = dx*dz = 1.0)
        // For each cell: bottom face (y=0, normal=[0,-1,0]) and top face (y=1, normal=[0,1,0])
        for i in 0..3 {
            let cx = i as f64 + 0.5;
            // y=0 boundary
            faces.push(Face::new(fid, vec![], i, None, 1.0, [0.0, -1.0, 0.0], [cx, 0.0, 0.5]));
            fid += 1;
            // y=1 boundary
            faces.push(Face::new(fid, vec![], i, None, 1.0, [0.0, 1.0, 0.0], [cx, 1.0, 0.5]));
            fid += 1;
        }

        // Z-normal faces (area = dx*dy = 1.0)
        // For each cell: front face (z=0, normal=[0,0,-1]) and back face (z=1, normal=[0,0,1])
        for i in 0..3 {
            let cx = i as f64 + 0.5;
            // z=0 boundary
            faces.push(Face::new(fid, vec![], i, None, 1.0, [0.0, 0.0, -1.0], [cx, 0.5, 0.0]));
            fid += 1;
            // z=1 boundary
            faces.push(Face::new(fid, vec![], i, None, 1.0, [0.0, 0.0, 1.0], [cx, 0.5, 1.0]));
            fid += 1;
        }

        UnstructuredMesh::from_components(vec![], faces, cells, vec![])
    }

    #[test]
    fn test_green_gauss_constant_field() {
        let mesh = make_3x1x1_mesh();
        // Constant field: phi = 5.0 everywhere
        let field = ScalarField::new("phi", vec![5.0, 5.0, 5.0]);

        let grad_computer = GreenGaussCellBasedGradient;
        let result = grad_computer.compute(&field, &mesh).unwrap();

        // Gradient of a constant field should be zero
        for i in 0..3 {
            let g = result.get(i).unwrap();
            assert!(g[0].abs() < 1e-10, "cell {} grad_x = {} (expected 0)", i, g[0]);
            assert!(g[1].abs() < 1e-10, "cell {} grad_y = {} (expected 0)", i, g[1]);
            assert!(g[2].abs() < 1e-10, "cell {} grad_z = {} (expected 0)", i, g[2]);
        }
    }

    #[test]
    fn test_green_gauss_linear_field() {
        let mesh = make_3x1x1_mesh();
        // Linear field phi = x (using cell centers: 0.5, 1.5, 2.5)
        let field = ScalarField::new("phi", vec![0.5, 1.5, 2.5]);

        let grad_computer = GreenGaussCellBasedGradient;
        let result = grad_computer.compute(&field, &mesh).unwrap();

        // For the interior cell (cell 1), gradient should be approximately [1, 0, 0]
        let g1 = result.get(1).unwrap();
        assert!(
            (g1[0] - 1.0).abs() < 1e-10,
            "interior cell grad_x = {} (expected ~1.0)",
            g1[0]
        );
        assert!(g1[1].abs() < 1e-10, "interior cell grad_y = {} (expected 0)", g1[1]);
        assert!(g1[2].abs() < 1e-10, "interior cell grad_z = {} (expected 0)", g1[2]);

        // Boundary cells may have less accurate gradients due to zero-gradient BC
        // but y and z components should still be zero
        for i in [0, 2] {
            let g = result.get(i).unwrap();
            assert!(g[1].abs() < 1e-10, "cell {} grad_y = {} (expected 0)", i, g[1]);
            assert!(g[2].abs() < 1e-10, "cell {} grad_z = {} (expected 0)", i, g[2]);
        }
    }
}
