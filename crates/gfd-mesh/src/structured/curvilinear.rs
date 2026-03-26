//! Curvilinear (body-fitted) structured mesh generation.
//!
//! Creates a structured mesh where grid lines follow arbitrary curves in
//! physical space (e.g., around an airfoil). The user provides an (ni x nj)
//! array of node positions; cells are formed as quads mapped to hex cells
//! with a single z-layer.

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use crate::{MeshError, Result};

/// Builder for a curvilinear (body-fitted) structured mesh.
///
/// Grid nodes are specified in physical (x, y) space as a 2D array of size
/// `ni x nj`.  Each cell `(i, j)` is a quad formed by the four corners
/// `(i,j), (i+1,j), (i+1,j+1), (i,j+1)`.  The mesh is extruded as a
/// single hex layer in z for compatibility with the 3D `UnstructuredMesh`.
///
/// # Example
/// ```
/// use gfd_mesh::structured::curvilinear::CurvilinearMeshBuilder;
///
/// // Simple 2x2 quad mesh
/// let nodes = vec![
///     vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]],
///     vec![[0.0, 1.0], [1.0, 1.0], [2.0, 1.0]],
///     vec![[0.0, 2.0], [1.0, 2.0], [2.0, 2.0]],
/// ];
/// let mesh = CurvilinearMeshBuilder::from_nodes(nodes).build().unwrap();
/// assert_eq!(mesh.num_cells(), 4);
/// ```
pub struct CurvilinearMeshBuilder {
    /// Node positions in physical space: nodes_xy[j][i] = [x, y].
    /// Outer index is j (0..nj), inner index is i (0..ni).
    nodes_xy: Vec<Vec<[f64; 2]>>,
    /// Depth in z-direction (single layer).
    depth: f64,
}

impl CurvilinearMeshBuilder {
    /// Create from a 2D array of (nj x ni) node positions.
    ///
    /// `nodes[j][i]` gives the `[x, y]` position of node at structured
    /// index `(i, j)`.  All inner vectors must have the same length.
    pub fn from_nodes(nodes: Vec<Vec<[f64; 2]>>) -> Self {
        Self {
            nodes_xy: nodes,
            depth: 1.0,
        }
    }

    /// Set the z-depth for the single hex layer (default 1.0).
    pub fn with_depth(mut self, depth: f64) -> Self {
        self.depth = depth;
        self
    }

    /// Build the mesh: cells are quads formed by (i,j), (i+1,j), (i+1,j+1), (i,j+1).
    ///
    /// Cell volumes are computed via the cross product of the diagonals.
    /// Face areas are computed from edge lengths.
    pub fn build(&self) -> Result<UnstructuredMesh> {
        let nj = self.nodes_xy.len();
        if nj < 2 {
            return Err(MeshError::InvalidParameters(
                "Need at least 2 rows (nj >= 2) of nodes".to_string(),
            ));
        }
        let ni = self.nodes_xy[0].len();
        if ni < 2 {
            return Err(MeshError::InvalidParameters(
                "Need at least 2 columns (ni >= 2) of nodes".to_string(),
            ));
        }
        // Validate all rows have same length
        for (j, row) in self.nodes_xy.iter().enumerate() {
            if row.len() != ni {
                return Err(MeshError::InvalidParameters(format!(
                    "Row {} has {} nodes, expected {}",
                    j,
                    row.len(),
                    ni,
                )));
            }
        }
        if self.depth <= 0.0 {
            return Err(MeshError::InvalidParameters(
                "depth must be > 0".to_string(),
            ));
        }

        let dz = self.depth;
        let ncells_i = ni - 1;
        let ncells_j = nj - 1;

        // Node indexing: (i, j, k) where k = 0 (bottom) or 1 (top)
        let node_idx = |i: usize, j: usize, k: usize| -> usize {
            k * nj * ni + j * ni + i
        };

        // 1. Build nodes (two z-layers)
        let num_nodes = 2 * nj * ni;
        let mut nodes = Vec::with_capacity(num_nodes);
        for k in 0..2_usize {
            let z = k as f64 * dz;
            for j in 0..nj {
                for i in 0..ni {
                    let id = node_idx(i, j, k);
                    let xy = &self.nodes_xy[j][i];
                    nodes.push(Node::new(id, [xy[0], xy[1], z]));
                }
            }
        }

        // Cell indexing: (i, j) -> flat
        let cell_flat = |i: usize, j: usize| -> usize { j * ncells_i + i };

        // 2. Build cells
        let num_cells = ncells_i * ncells_j;
        let mut cells = Vec::with_capacity(num_cells);
        for j in 0..ncells_j {
            for i in 0..ncells_i {
                let cell_id = cell_flat(i, j);

                // Bottom layer nodes (CCW when viewed from +z)
                let n0 = node_idx(i, j, 0);
                let n1 = node_idx(i + 1, j, 0);
                let n2 = node_idx(i + 1, j + 1, 0);
                let n3 = node_idx(i, j + 1, 0);
                // Top layer nodes
                let n4 = node_idx(i, j, 1);
                let n5 = node_idx(i + 1, j, 1);
                let n6 = node_idx(i + 1, j + 1, 1);
                let n7 = node_idx(i, j + 1, 1);

                let p0 = &self.nodes_xy[j][i];
                let p1 = &self.nodes_xy[j][i + 1];
                let p2 = &self.nodes_xy[j + 1][i + 1];
                let p3 = &self.nodes_xy[j + 1][i];

                // Cell center
                let cx = (p0[0] + p1[0] + p2[0] + p3[0]) / 4.0;
                let cy = (p0[1] + p1[1] + p2[1] + p3[1]) / 4.0;
                let cz = dz / 2.0;

                // Quad area via cross product of diagonals: |d1 x d2| / 2
                let d1x = p2[0] - p0[0];
                let d1y = p2[1] - p0[1];
                let d2x = p3[0] - p1[0];
                let d2y = p3[1] - p1[1];
                let quad_area = (d1x * d2y - d1y * d2x).abs() / 2.0;
                let vol = quad_area * dz;

                cells.push(Cell::new(
                    cell_id,
                    vec![n0, n1, n2, n3, n4, n5, n6, n7],
                    Vec::new(),
                    vol,
                    [cx, cy, cz],
                ));
            }
        }

        // 3. Build faces
        let mut faces: Vec<Face> = Vec::new();
        let mut imin_faces = Vec::new();
        let mut imax_faces = Vec::new();
        let mut jmin_faces = Vec::new();
        let mut jmax_faces = Vec::new();
        let mut zmin_faces = Vec::new();
        let mut zmax_faces = Vec::new();

        // I-direction faces (between cells at i-1 and i)
        for j in 0..ncells_j {
            for i in 0..=ncells_i {
                let face_id = faces.len();
                let fn0 = node_idx(i, j, 0);
                let fn1 = node_idx(i, j + 1, 0);
                let fn2 = node_idx(i, j + 1, 1);
                let fn3 = node_idx(i, j, 1);
                let face_nodes = vec![fn0, fn1, fn2, fn3];

                // Edge length along j-direction at this i
                let pa = &self.nodes_xy[j][i];
                let pb = &self.nodes_xy[j + 1][i];
                let edge_len = ((pb[0] - pa[0]).powi(2) + (pb[1] - pa[1]).powi(2)).sqrt();
                let area = edge_len * dz;

                // Face center
                let center = [
                    (pa[0] + pb[0]) / 2.0,
                    (pa[1] + pb[1]) / 2.0,
                    dz / 2.0,
                ];

                // Normal: rotate the edge vector 90 degrees (pointing in +i direction)
                let normal = if edge_len > 1e-30 {
                    let dx = pb[0] - pa[0];
                    let dy = pb[1] - pa[1];
                    // Rotate (dx, dy) by -90 degrees: (dy, -dx)
                    [dy / edge_len, -dx / edge_len, 0.0]
                } else {
                    [1.0, 0.0, 0.0]
                };

                let (owner, neighbor);
                if i == 0 {
                    owner = cell_flat(0, j);
                    neighbor = None;
                    // Flip normal to point outward (in -i direction)
                    let normal_out = [-normal[0], -normal[1], -normal[2]];
                    imin_faces.push(face_id);
                    faces.push(Face::new(face_id, face_nodes, owner, neighbor, area, normal_out, center));
                    continue;
                } else if i == ncells_i {
                    owner = cell_flat(ncells_i - 1, j);
                    neighbor = None;
                    imax_faces.push(face_id);
                    faces.push(Face::new(face_id, face_nodes, owner, neighbor, area, normal, center));
                    continue;
                } else {
                    owner = cell_flat(i - 1, j);
                    neighbor = Some(cell_flat(i, j));
                }

                faces.push(Face::new(face_id, face_nodes, owner, neighbor, area, normal, center));
            }
        }

        // J-direction faces (between cells at j-1 and j)
        for j in 0..=ncells_j {
            for i in 0..ncells_i {
                let face_id = faces.len();
                let fn0 = node_idx(i, j, 0);
                let fn1 = node_idx(i + 1, j, 0);
                let fn2 = node_idx(i + 1, j, 1);
                let fn3 = node_idx(i, j, 1);
                let face_nodes = vec![fn0, fn1, fn2, fn3];

                // Edge length along i-direction at this j
                let pa = &self.nodes_xy[j][i];
                let pb = &self.nodes_xy[j][i + 1];
                let edge_len = ((pb[0] - pa[0]).powi(2) + (pb[1] - pa[1]).powi(2)).sqrt();
                let area = edge_len * dz;

                let center = [
                    (pa[0] + pb[0]) / 2.0,
                    (pa[1] + pb[1]) / 2.0,
                    dz / 2.0,
                ];

                // Normal: rotate the edge vector 90 degrees (pointing in +j direction)
                let normal = if edge_len > 1e-30 {
                    let dx = pb[0] - pa[0];
                    let dy = pb[1] - pa[1];
                    // Rotate (dx, dy) by +90 degrees: (-dy, dx)
                    [-dy / edge_len, dx / edge_len, 0.0]
                } else {
                    [0.0, 1.0, 0.0]
                };

                let (owner, neighbor);
                if j == 0 {
                    owner = cell_flat(i, 0);
                    neighbor = None;
                    let normal_out = [-normal[0], -normal[1], -normal[2]];
                    jmin_faces.push(face_id);
                    faces.push(Face::new(face_id, face_nodes, owner, neighbor, area, normal_out, center));
                    continue;
                } else if j == ncells_j {
                    owner = cell_flat(i, ncells_j - 1);
                    neighbor = None;
                    jmax_faces.push(face_id);
                    faces.push(Face::new(face_id, face_nodes, owner, neighbor, area, normal, center));
                    continue;
                } else {
                    owner = cell_flat(i, j - 1);
                    neighbor = Some(cell_flat(i, j));
                }

                faces.push(Face::new(face_id, face_nodes, owner, neighbor, area, normal, center));
            }
        }

        // Z-direction faces (bottom and top)
        for j in 0..ncells_j {
            for i in 0..ncells_i {
                let p0 = &self.nodes_xy[j][i];
                let p1 = &self.nodes_xy[j][i + 1];
                let p2 = &self.nodes_xy[j + 1][i + 1];
                let p3 = &self.nodes_xy[j + 1][i];

                // Quad area via cross product of diagonals
                let d1x = p2[0] - p0[0];
                let d1y = p2[1] - p0[1];
                let d2x = p3[0] - p1[0];
                let d2y = p3[1] - p1[1];
                let quad_area = (d1x * d2y - d1y * d2x).abs() / 2.0;

                let cx = (p0[0] + p1[0] + p2[0] + p3[0]) / 4.0;
                let cy = (p0[1] + p1[1] + p2[1] + p3[1]) / 4.0;
                let cell_id = cell_flat(i, j);

                // Bottom face (z=0)
                {
                    let face_id = faces.len();
                    let face_nodes = vec![
                        node_idx(i, j, 0),
                        node_idx(i + 1, j, 0),
                        node_idx(i + 1, j + 1, 0),
                        node_idx(i, j + 1, 0),
                    ];
                    zmin_faces.push(face_id);
                    faces.push(Face::new(
                        face_id, face_nodes, cell_id, None,
                        quad_area, [0.0, 0.0, -1.0], [cx, cy, 0.0],
                    ));
                }

                // Top face (z=dz)
                {
                    let face_id = faces.len();
                    let face_nodes = vec![
                        node_idx(i, j, 1),
                        node_idx(i + 1, j, 1),
                        node_idx(i + 1, j + 1, 1),
                        node_idx(i, j + 1, 1),
                    ];
                    zmax_faces.push(face_id);
                    faces.push(Face::new(
                        face_id, face_nodes, cell_id, None,
                        quad_area, [0.0, 0.0, 1.0], [cx, cy, dz],
                    ));
                }
            }
        }

        // 4. Boundary patches
        let mut boundary_patches = Vec::new();
        if !imin_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("imin", imin_faces));
        }
        if !imax_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("imax", imax_faces));
        }
        if !jmin_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("jmin", jmin_faces));
        }
        if !jmax_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("jmax", jmax_faces));
        }
        if !zmin_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("zmin", zmin_faces));
        }
        if !zmax_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("zmax", zmax_faces));
        }

        // 5. Populate cell face lists
        for face in &faces {
            cells[face.owner_cell].faces.push(face.id);
            if let Some(nbr) = face.neighbor_cell {
                cells[nbr].faces.push(face.id);
            }
        }

        Ok(UnstructuredMesh::from_components(
            nodes,
            faces,
            cells,
            boundary_patches,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a simple 3x3 uniform grid of nodes (2x2 cells).
    fn uniform_3x3() -> Vec<Vec<[f64; 2]>> {
        vec![
            vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]],
            vec![[0.0, 1.0], [1.0, 1.0], [2.0, 1.0]],
            vec![[0.0, 2.0], [1.0, 2.0], [2.0, 2.0]],
        ]
    }

    #[test]
    fn test_basic_curvilinear_mesh() {
        let mesh = CurvilinearMeshBuilder::from_nodes(uniform_3x3())
            .build()
            .unwrap();
        assert_eq!(mesh.num_cells(), 4); // 2x2
        assert_eq!(mesh.num_nodes(), 2 * 9); // 9 nodes * 2 z-layers
    }

    #[test]
    fn test_total_volume_uniform() {
        let mesh = CurvilinearMeshBuilder::from_nodes(uniform_3x3())
            .build()
            .unwrap();
        let total_vol: f64 = mesh.cells.iter().map(|c| c.volume).sum();
        // 2x2 domain with depth 1.0
        assert!(
            (total_vol - 4.0).abs() < 1e-12,
            "total vol {} expected 4.0",
            total_vol,
        );
    }

    #[test]
    fn test_curvilinear_skewed_cells() {
        // Create a parallelogram grid (skewed)
        let nodes = vec![
            vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]],
            vec![[0.5, 1.0], [1.5, 1.0], [2.5, 1.0]],
            vec![[1.0, 2.0], [2.0, 2.0], [3.0, 2.0]],
        ];
        let mesh = CurvilinearMeshBuilder::from_nodes(nodes).build().unwrap();
        assert_eq!(mesh.num_cells(), 4);

        // All parallelogram cells should have the same area = 1.0
        for cell in &mesh.cells {
            assert!(
                (cell.volume - 1.0).abs() < 1e-12,
                "cell {} vol={} expected 1.0",
                cell.id,
                cell.volume,
            );
        }
    }

    #[test]
    fn test_curvilinear_annular_sector() {
        // Quarter-annulus: r=[1,2], theta=[0, pi/2]
        let nr = 3; // radial nodes
        let nt = 5; // circumferential nodes
        let mut nodes = Vec::with_capacity(nt);
        for j in 0..nt {
            let theta = std::f64::consts::FRAC_PI_2 * j as f64 / (nt - 1) as f64;
            let mut row = Vec::with_capacity(nr);
            for i in 0..nr {
                let r = 1.0 + i as f64 / (nr - 1) as f64;
                row.push([r * theta.cos(), r * theta.sin()]);
            }
            nodes.push(row);
        }
        let mesh = CurvilinearMeshBuilder::from_nodes(nodes).build().unwrap();
        assert_eq!(mesh.num_cells(), (nr - 1) * (nt - 1));
        for cell in &mesh.cells {
            assert!(cell.volume > 0.0, "cell {} has non-positive vol", cell.id);
        }
    }

    #[test]
    fn test_boundary_patches() {
        let mesh = CurvilinearMeshBuilder::from_nodes(uniform_3x3())
            .build()
            .unwrap();
        assert!(mesh.boundary_patch("imin").is_some());
        assert!(mesh.boundary_patch("imax").is_some());
        assert!(mesh.boundary_patch("jmin").is_some());
        assert!(mesh.boundary_patch("jmax").is_some());
        assert!(mesh.boundary_patch("zmin").is_some());
        assert!(mesh.boundary_patch("zmax").is_some());

        // 2x2 cells: each boundary direction has 2 faces
        let imin = mesh.boundary_patch("imin").unwrap();
        let imax = mesh.boundary_patch("imax").unwrap();
        let jmin = mesh.boundary_patch("jmin").unwrap();
        let jmax = mesh.boundary_patch("jmax").unwrap();
        assert_eq!(imin.num_faces(), 2);
        assert_eq!(imax.num_faces(), 2);
        assert_eq!(jmin.num_faces(), 2);
        assert_eq!(jmax.num_faces(), 2);
    }

    #[test]
    fn test_all_faces_positive_area() {
        let mesh = CurvilinearMeshBuilder::from_nodes(uniform_3x3())
            .build()
            .unwrap();
        for face in &mesh.faces {
            assert!(face.area > 0.0, "face {} has non-positive area", face.id);
        }
    }

    #[test]
    fn test_invalid_parameters() {
        // Too few rows
        let result = CurvilinearMeshBuilder::from_nodes(vec![
            vec![[0.0, 0.0], [1.0, 0.0]],
        ])
        .build();
        assert!(result.is_err());

        // Too few columns
        let result = CurvilinearMeshBuilder::from_nodes(vec![
            vec![[0.0, 0.0]],
            vec![[0.0, 1.0]],
        ])
        .build();
        assert!(result.is_err());

        // Inconsistent row lengths
        let result = CurvilinearMeshBuilder::from_nodes(vec![
            vec![[0.0, 0.0], [1.0, 0.0]],
            vec![[0.0, 1.0]],
        ])
        .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_cell_faces_populated() {
        let mesh = CurvilinearMeshBuilder::from_nodes(uniform_3x3())
            .build()
            .unwrap();
        for cell in &mesh.cells {
            assert_eq!(
                cell.faces.len(),
                6,
                "cell {} has {} faces, expected 6",
                cell.id,
                cell.faces.len(),
            );
        }
    }

    #[test]
    fn test_custom_depth() {
        let depth = 0.5;
        let mesh = CurvilinearMeshBuilder::from_nodes(uniform_3x3())
            .with_depth(depth)
            .build()
            .unwrap();
        let total_vol: f64 = mesh.cells.iter().map(|c| c.volume).sum();
        assert!(
            (total_vol - 4.0 * depth).abs() < 1e-12,
            "total vol {} expected {}",
            total_vol,
            4.0 * depth,
        );
    }
}
