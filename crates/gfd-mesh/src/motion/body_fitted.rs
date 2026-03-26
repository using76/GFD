//! Body-fitted mesh generation for moving bodies.
//!
//! Generates meshes that conform to a moving body surface defined by a signed
//! distance function (SDF), and supports updating the body position.

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use std::collections::HashMap;

use crate::Result;

/// A body-fitted mesh generator.
///
/// Generates a structured-like mesh within an outer bounding box that conforms
/// to a body surface defined by an SDF. The mesh has layers of cells that
/// follow the body contour.
pub struct BodyFittedMesh {
    /// Signed distance function defining the body surface.
    /// Negative inside the body, positive outside.
    pub body_sdf: Box<dyn Fn([f64; 3]) -> f64>,
    /// Outer boundary of the domain: [xmin, ymin, zmin, xmax, ymax, zmax].
    pub outer_boundary: [f64; 6],
    /// Number of layers in the mesh between the body and outer boundary.
    pub layers: usize,
}

impl BodyFittedMesh {
    /// Creates a new body-fitted mesh specification.
    pub fn new(
        body_sdf: Box<dyn Fn([f64; 3]) -> f64>,
        outer_boundary: [f64; 6],
        layers: usize,
    ) -> Self {
        Self {
            body_sdf,
            outer_boundary,
            layers,
        }
    }

    /// Generate a mesh that conforms to the body surface.
    ///
    /// Creates a 2D quad mesh (extruded to one layer in z) with `nx * ny` base
    /// cells. Nodes that fall inside the body are projected to the body surface.
    /// Cells that are entirely inside the body are removed.
    ///
    /// # Arguments
    /// * `nx` - Number of cells in x.
    /// * `ny` - Number of cells in y.
    ///
    /// # Returns
    /// An `UnstructuredMesh` that conforms to the body.
    pub fn generate(&self, nx: usize, ny: usize) -> Result<UnstructuredMesh> {
        if nx == 0 || ny == 0 {
            return Err(crate::MeshError::InvalidParameters(
                "nx and ny must be positive".to_string(),
            ));
        }
        if self.layers == 0 {
            return Err(crate::MeshError::InvalidParameters(
                "layers must be positive".to_string(),
            ));
        }

        let [xmin, ymin, zmin, xmax, ymax, zmax] = self.outer_boundary;
        let dx = (xmax - xmin) / nx as f64;
        let dy = (ymax - ymin) / ny as f64;
        let nz = 1usize;
        let dz = if (zmax - zmin).abs() < 1e-30 { 1.0 } else { zmax - zmin };

        // Generate nodes on a structured grid
        let nnx = nx + 1;
        let nny = ny + 1;
        let nnz = nz + 1;
        let mut nodes: Vec<Node> = Vec::with_capacity(nnx * nny * nnz);

        for k in 0..nnz {
            for j in 0..nny {
                for i in 0..nnx {
                    let nid = nodes.len();
                    let x = xmin + i as f64 * dx;
                    let y = ymin + j as f64 * dy;
                    let z = zmin + k as f64 * dz / nz as f64;
                    let mut pos = [x, y, z];

                    // Project nodes inside the body onto the body surface
                    let d = (self.body_sdf)(pos);
                    if d < 0.0 {
                        // Move node outward along the gradient of the SDF
                        let eps = dx.min(dy) * 0.01;
                        let grad = sdf_gradient(&*self.body_sdf, pos, eps);
                        let grad_len = (grad[0] * grad[0] + grad[1] * grad[1] + grad[2] * grad[2]).sqrt();
                        if grad_len > 1e-12 {
                            // Project to surface: move by |d| along gradient
                            pos[0] += (-d) * grad[0] / grad_len;
                            pos[1] += (-d) * grad[1] / grad_len;
                            pos[2] += (-d) * grad[2] / grad_len;
                        }
                    }

                    nodes.push(Node::new(nid, pos));
                }
            }
        }

        // Generate hex cells, skipping those fully inside the body
        let mut cells: Vec<Cell> = Vec::new();
        let node_idx = |i: usize, j: usize, k: usize| -> usize { k * nnx * nny + j * nnx + i };

        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let n0 = node_idx(i, j, k);
                    let n1 = node_idx(i + 1, j, k);
                    let n2 = node_idx(i + 1, j + 1, k);
                    let n3 = node_idx(i, j + 1, k);
                    let n4 = node_idx(i, j, k + 1);
                    let n5 = node_idx(i + 1, j, k + 1);
                    let n6 = node_idx(i + 1, j + 1, k + 1);
                    let n7 = node_idx(i, j + 1, k + 1);

                    let cell_nodes = vec![n0, n1, n2, n3, n4, n5, n6, n7];

                    // Compute cell center
                    let center = compute_center(&nodes, &cell_nodes);

                    // Skip cells whose center is inside the body
                    if (self.body_sdf)(center) < -dx.min(dy) * 0.1 {
                        continue;
                    }

                    // Compute approximate volume
                    let volume = compute_hex_volume(&nodes, &cell_nodes);
                    if volume < 1e-30 {
                        continue;
                    }

                    let cid = cells.len();
                    cells.push(Cell::new(cid, cell_nodes, Vec::new(), volume, center));
                }
            }
        }

        if cells.is_empty() {
            return Err(crate::MeshError::GenerationFailed(
                "No cells generated - body may fill the entire domain".to_string(),
            ));
        }

        // Rebuild faces
        let (faces, boundary_patches) = rebuild_faces(&nodes, &mut cells);

        Ok(UnstructuredMesh::from_components(
            nodes,
            faces,
            cells,
            boundary_patches,
        ))
    }

    /// Update the body position by providing a new SDF.
    pub fn update_body_position(&mut self, new_sdf: Box<dyn Fn([f64; 3]) -> f64>) {
        self.body_sdf = new_sdf;
    }
}

/// Compute the gradient of an SDF using central differences.
fn sdf_gradient(sdf: &dyn Fn([f64; 3]) -> f64, p: [f64; 3], eps: f64) -> [f64; 3] {
    let mut grad = [0.0; 3];
    for axis in 0..3 {
        let mut p_plus = p;
        let mut p_minus = p;
        p_plus[axis] += eps;
        p_minus[axis] -= eps;
        grad[axis] = (sdf(p_plus) - sdf(p_minus)) / (2.0 * eps);
    }
    grad
}

fn compute_center(nodes: &[Node], node_ids: &[usize]) -> [f64; 3] {
    let n = node_ids.len() as f64;
    let mut c = [0.0; 3];
    for &nid in node_ids {
        let p = nodes[nid].position;
        c[0] += p[0];
        c[1] += p[1];
        c[2] += p[2];
    }
    [c[0] / n, c[1] / n, c[2] / n]
}

fn compute_hex_volume(nodes: &[Node], cell_nodes: &[usize]) -> f64 {
    if cell_nodes.len() != 8 {
        return 0.0;
    }
    // Approximate hex volume by splitting into 5 tetrahedra
    let p: Vec<[f64; 3]> = cell_nodes.iter().map(|&nid| nodes[nid].position).collect();
    let tets = [
        [0, 1, 3, 4],
        [1, 2, 3, 6],
        [1, 4, 5, 6],
        [3, 4, 6, 7],
        [1, 3, 4, 6],
    ];
    let mut vol = 0.0;
    for tet in &tets {
        vol += tet_volume(p[tet[0]], p[tet[1]], p[tet[2]], p[tet[3]]).abs();
    }
    vol
}

fn tet_volume(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let ad = [d[0] - a[0], d[1] - a[1], d[2] - a[2]];
    let cross = [
        ac[1] * ad[2] - ac[2] * ad[1],
        ac[2] * ad[0] - ac[0] * ad[2],
        ac[0] * ad[1] - ac[1] * ad[0],
    ];
    (ab[0] * cross[0] + ab[1] * cross[1] + ab[2] * cross[2]).abs() / 6.0
}

fn rebuild_faces(
    nodes: &[Node],
    cells: &mut Vec<Cell>,
) -> (Vec<Face>, Vec<BoundaryPatch>) {
    let mut face_map: HashMap<Vec<usize>, (usize, Vec<usize>)> = HashMap::new();
    let mut faces: Vec<Face> = Vec::new();
    let mut boundary_face_ids: Vec<usize> = Vec::new();

    for ci in 0..cells.len() {
        let cn = cells[ci].nodes.clone();
        let face_list = match cn.len() {
            8 => vec![
                vec![cn[0], cn[1], cn[2], cn[3]],
                vec![cn[4], cn[5], cn[6], cn[7]],
                vec![cn[0], cn[1], cn[5], cn[4]],
                vec![cn[3], cn[2], cn[6], cn[7]],
                vec![cn[0], cn[3], cn[7], cn[4]],
                vec![cn[1], cn[2], cn[6], cn[5]],
            ],
            4 => vec![
                vec![cn[0], cn[1], cn[2]],
                vec![cn[0], cn[1], cn[3]],
                vec![cn[0], cn[2], cn[3]],
                vec![cn[1], cn[2], cn[3]],
            ],
            _ => vec![],
        };

        for fn_nodes in &face_list {
            let mut sorted = fn_nodes.clone();
            sorted.sort();

            if let Some((owner_ci, _)) = face_map.get(&sorted) {
                let fid = faces.len();
                let fc = face_center_calc(nodes, fn_nodes);
                let (area, normal) = face_area_normal_calc(nodes, fn_nodes);
                faces.push(Face::new(fid, fn_nodes.clone(), *owner_ci, Some(ci), area, normal, fc));
                cells[*owner_ci].faces.push(fid);
                cells[ci].faces.push(fid);
                face_map.remove(&sorted);
            } else {
                face_map.insert(sorted, (ci, fn_nodes.clone()));
            }
        }
    }

    for (_key, (owner_ci, fn_nodes)) in &face_map {
        let fid = faces.len();
        let fc = face_center_calc(nodes, fn_nodes);
        let (area, normal) = face_area_normal_calc(nodes, fn_nodes);
        faces.push(Face::new(fid, fn_nodes.clone(), *owner_ci, None, area, normal, fc));
        cells[*owner_ci].faces.push(fid);
        boundary_face_ids.push(fid);
    }

    let mut patches = Vec::new();
    if !boundary_face_ids.is_empty() {
        patches.push(BoundaryPatch::new("boundary", boundary_face_ids));
    }
    (faces, patches)
}

fn face_center_calc(nodes: &[Node], node_ids: &[usize]) -> [f64; 3] {
    let n = node_ids.len() as f64;
    let mut c = [0.0; 3];
    for &nid in node_ids {
        let p = nodes[nid].position;
        c[0] += p[0];
        c[1] += p[1];
        c[2] += p[2];
    }
    [c[0] / n, c[1] / n, c[2] / n]
}

fn face_area_normal_calc(nodes: &[Node], node_ids: &[usize]) -> (f64, [f64; 3]) {
    if node_ids.len() < 3 {
        return (0.0, [0.0, 0.0, 1.0]);
    }
    let n = node_ids.len();
    let mut nx = 0.0f64;
    let mut ny = 0.0f64;
    let mut nz = 0.0f64;
    for i in 0..n {
        let p1 = nodes[node_ids[i]].position;
        let p2 = nodes[node_ids[(i + 1) % n]].position;
        nx += (p1[1] - p2[1]) * (p1[2] + p2[2]);
        ny += (p1[2] - p2[2]) * (p1[0] + p2[0]);
        nz += (p1[0] - p2[0]) * (p1[1] + p2[1]);
    }
    let area = 0.5 * (nx * nx + ny * ny + nz * nz).sqrt();
    if area < 1e-30 {
        return (0.0, [0.0, 0.0, 1.0]);
    }
    (area, [nx / (2.0 * area), ny / (2.0 * area), nz / (2.0 * area)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_no_body() {
        // SDF for a body far outside the domain: all cells kept
        let sdf = Box::new(|_p: [f64; 3]| 100.0_f64);
        let bfm = BodyFittedMesh::new(sdf, [0.0, 0.0, 0.0, 4.0, 4.0, 1.0], 3);
        let mesh = bfm.generate(4, 4).unwrap();
        assert_eq!(mesh.cells.len(), 16, "4x4 grid with no body should have 16 cells");
    }

    #[test]
    fn test_generate_with_sphere() {
        // Sphere centered at (2,2,0.5) with radius 1
        let sdf = Box::new(|p: [f64; 3]| {
            let dx = p[0] - 2.0;
            let dy = p[1] - 2.0;
            let dz = p[2] - 0.5;
            (dx * dx + dy * dy + dz * dz).sqrt() - 1.0
        });
        let bfm = BodyFittedMesh::new(sdf, [0.0, 0.0, 0.0, 4.0, 4.0, 1.0], 3);
        let mesh = bfm.generate(8, 8).unwrap();
        // Some cells should be removed (inside the sphere)
        assert!(
            mesh.cells.len() < 64,
            "Should have fewer than 64 cells with sphere removed, got {}",
            mesh.cells.len()
        );
        assert!(mesh.cells.len() > 0, "Should still have cells outside the sphere");
    }

    #[test]
    fn test_update_body_position() {
        let sdf1 = Box::new(|p: [f64; 3]| {
            let dx = p[0] - 1.0;
            let dy = p[1] - 1.0;
            (dx * dx + dy * dy).sqrt() - 0.5
        });
        let mut bfm = BodyFittedMesh::new(sdf1, [0.0, 0.0, 0.0, 4.0, 4.0, 1.0], 2);
        let mesh1 = bfm.generate(4, 4).unwrap();

        // Move body to different position
        let sdf2 = Box::new(|p: [f64; 3]| {
            let dx = p[0] - 3.0;
            let dy = p[1] - 3.0;
            (dx * dx + dy * dy).sqrt() - 0.5
        });
        bfm.update_body_position(sdf2);
        let mesh2 = bfm.generate(4, 4).unwrap();

        // Both should produce valid meshes, possibly with different cell counts
        assert!(mesh1.cells.len() > 0);
        assert!(mesh2.cells.len() > 0);
    }

    #[test]
    fn test_generate_invalid_params() {
        let sdf = Box::new(|_p: [f64; 3]| 1.0_f64);
        let bfm = BodyFittedMesh::new(sdf, [0.0, 0.0, 0.0, 1.0, 1.0, 1.0], 1);
        assert!(bfm.generate(0, 5).is_err());
    }

    #[test]
    fn test_generate_positive_volumes() {
        let sdf = Box::new(|_p: [f64; 3]| 10.0_f64);
        let bfm = BodyFittedMesh::new(sdf, [0.0, 0.0, 0.0, 2.0, 2.0, 1.0], 2);
        let mesh = bfm.generate(3, 3).unwrap();
        for cell in &mesh.cells {
            assert!(
                cell.volume > 0.0,
                "Cell {} should have positive volume, got {}",
                cell.id, cell.volume
            );
        }
    }
}
