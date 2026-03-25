//! Structured (Cartesian) mesh representation.

use serde::{Deserialize, Serialize};

use super::cell::Cell;
use super::face::Face;
use super::node::Node;
use super::unstructured::{BoundaryPatch, UnstructuredMesh};

/// A structured Cartesian mesh defined by uniform spacing in each direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredMesh {
    /// Number of cells in the x-direction.
    pub nx: usize,
    /// Number of cells in the y-direction.
    pub ny: usize,
    /// Number of cells in the z-direction.
    pub nz: usize,
    /// Cell spacing in the x-direction.
    pub dx: f64,
    /// Cell spacing in the y-direction.
    pub dy: f64,
    /// Cell spacing in the z-direction.
    pub dz: f64,
    /// Origin point of the mesh (minimum corner).
    pub origin: [f64; 3],
}

impl StructuredMesh {
    /// Creates a new structured mesh.
    pub fn new(
        nx: usize,
        ny: usize,
        nz: usize,
        dx: f64,
        dy: f64,
        dz: f64,
        origin: [f64; 3],
    ) -> Self {
        Self {
            nx,
            ny,
            nz,
            dx,
            dy,
            dz,
            origin,
        }
    }

    /// Creates a uniform structured mesh over the given domain extents.
    pub fn uniform(nx: usize, ny: usize, nz: usize, lx: f64, ly: f64, lz: f64) -> Self {
        Self {
            nx,
            ny,
            nz,
            dx: lx / nx as f64,
            dy: ly / ny as f64,
            dz: lz / nz as f64,
            origin: [0.0, 0.0, 0.0],
        }
    }

    /// Returns the total number of cells.
    pub fn num_cells(&self) -> usize {
        self.nx * self.ny * self.nz
    }

    /// Returns the total number of faces.
    pub fn num_faces(&self) -> usize {
        // Internal + boundary faces for a structured grid
        let fx = (self.nx + 1) * self.ny * self.nz; // x-normal faces
        let fy = self.nx * (self.ny + 1) * self.nz; // y-normal faces
        let fz = self.nx * self.ny * (self.nz + 1); // z-normal faces
        fx + fy + fz
    }

    /// Returns the total number of nodes.
    pub fn num_nodes(&self) -> usize {
        (self.nx + 1) * (self.ny + 1) * (self.nz + 1)
    }

    /// Returns the spatial dimensions of the mesh (1, 2, or 3).
    pub fn dimensions(&self) -> usize {
        let mut dims = 0;
        if self.nx > 0 { dims += 1; }
        if self.ny > 0 { dims += 1; }
        if self.nz > 0 { dims += 1; }
        dims
    }

    /// Returns the centroid of the cell at grid indices (i, j, k).
    pub fn cell_center(&self, i: usize, j: usize, k: usize) -> [f64; 3] {
        [
            self.origin[0] + (i as f64 + 0.5) * self.dx,
            self.origin[1] + (j as f64 + 0.5) * self.dy,
            self.origin[2] + (k as f64 + 0.5) * self.dz,
        ]
    }

    /// Returns the volume of a single cell.
    pub fn cell_volume(&self) -> f64 {
        self.dx * self.dy * self.dz
    }

    /// Converts a flat cell index to (i, j, k) grid indices.
    pub fn flat_to_ijk(&self, idx: usize) -> (usize, usize, usize) {
        let k = idx / (self.nx * self.ny);
        let rem = idx % (self.nx * self.ny);
        let j = rem / self.nx;
        let i = rem % self.nx;
        (i, j, k)
    }

    /// Converts (i, j, k) grid indices to a flat cell index.
    pub fn ijk_to_flat(&self, i: usize, j: usize, k: usize) -> usize {
        k * self.nx * self.ny + j * self.nx + i
    }

    /// Computes the flat node index for grid point (i, j, k) given effective_nz.
    fn node_index(&self, i: usize, j: usize, k: usize, effective_nz: usize) -> usize {
        let _ = effective_nz; // effective_nz is only used to document intent; layout uses (nx+1),(ny+1)
        k * (self.ny + 1) * (self.nx + 1) + j * (self.nx + 1) + i
    }

    /// Converts this structured mesh into an equivalent `UnstructuredMesh`.
    ///
    /// For a 2D mesh (`nz == 0`), a single layer in z is generated with `dz_eff = 1.0`.
    pub fn to_unstructured(&self) -> UnstructuredMesh {
        let effective_nz = if self.nz == 0 { 1 } else { self.nz };
        let dz_eff = if self.nz == 0 { 1.0 } else { self.dz };

        // -----------------------------------------------------------
        // 1. Nodes
        // -----------------------------------------------------------
        let num_nodes = (self.nx + 1) * (self.ny + 1) * (effective_nz + 1);
        let mut nodes = Vec::with_capacity(num_nodes);
        for k in 0..=effective_nz {
            for j in 0..=self.ny {
                for i in 0..=self.nx {
                    let id = self.node_index(i, j, k, effective_nz);
                    let pos = [
                        self.origin[0] + i as f64 * self.dx,
                        self.origin[1] + j as f64 * self.dy,
                        self.origin[2] + k as f64 * dz_eff,
                    ];
                    nodes.push(Node::new(id, pos));
                }
            }
        }

        // -----------------------------------------------------------
        // 2. Cells (faces will be filled in later)
        // -----------------------------------------------------------
        let num_cells = self.nx * self.ny * effective_nz;
        let mut cells = Vec::with_capacity(num_cells);
        let cell_vol = if self.nz == 0 {
            self.dx * self.dy * 1.0
        } else {
            self.cell_volume()
        };

        for k in 0..effective_nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let cell_id = k * self.nx * self.ny + j * self.nx + i;
                    let n0 = self.node_index(i, j, k, effective_nz);
                    let n1 = self.node_index(i + 1, j, k, effective_nz);
                    let n2 = self.node_index(i + 1, j + 1, k, effective_nz);
                    let n3 = self.node_index(i, j + 1, k, effective_nz);
                    let n4 = self.node_index(i, j, k + 1, effective_nz);
                    let n5 = self.node_index(i + 1, j, k + 1, effective_nz);
                    let n6 = self.node_index(i + 1, j + 1, k + 1, effective_nz);
                    let n7 = self.node_index(i, j + 1, k + 1, effective_nz);

                    let center = if self.nz == 0 {
                        [
                            self.origin[0] + (i as f64 + 0.5) * self.dx,
                            self.origin[1] + (j as f64 + 0.5) * self.dy,
                            self.origin[2] + (k as f64 + 0.5) * dz_eff,
                        ]
                    } else {
                        self.cell_center(i, j, k)
                    };

                    cells.push(Cell::new(
                        cell_id,
                        vec![n0, n1, n2, n3, n4, n5, n6, n7],
                        Vec::new(), // faces populated later
                        cell_vol,
                        center,
                    ));
                }
            }
        }

        // Helper closure: cell flat index for the effective grid
        let cell_flat =
            |i: usize, j: usize, k: usize| -> usize { k * self.nx * self.ny + j * self.nx + i };

        // -----------------------------------------------------------
        // 3. Faces
        // -----------------------------------------------------------
        let num_x_faces = (self.nx + 1) * self.ny * effective_nz;
        let num_y_faces = self.nx * (self.ny + 1) * effective_nz;
        let num_z_faces = self.nx * self.ny * (effective_nz + 1);
        let total_faces = num_x_faces + num_y_faces + num_z_faces;

        let mut faces: Vec<Face> = Vec::with_capacity(total_faces);

        // We also collect boundary face ids per patch while generating faces.
        let mut xmin_faces = Vec::new();
        let mut xmax_faces = Vec::new();
        let mut ymin_faces = Vec::new();
        let mut ymax_faces = Vec::new();
        let mut zmin_faces = Vec::new();
        let mut zmax_faces = Vec::new();

        // --- X-direction faces ---
        for k in 0..effective_nz {
            for j in 0..self.ny {
                for i in 0..=self.nx {
                    let face_id = faces.len();
                    let fn0 = self.node_index(i, j, k, effective_nz);
                    let fn1 = self.node_index(i, j + 1, k, effective_nz);
                    let fn2 = self.node_index(i, j + 1, k + 1, effective_nz);
                    let fn3 = self.node_index(i, j, k + 1, effective_nz);
                    let face_nodes = vec![fn0, fn1, fn2, fn3];

                    let area = self.dy * dz_eff;
                    let center = [
                        self.origin[0] + i as f64 * self.dx,
                        self.origin[1] + (j as f64 + 0.5) * self.dy,
                        self.origin[2] + (k as f64 + 0.5) * dz_eff,
                    ];

                    let (owner, neighbor, normal);
                    if i == 0 {
                        // boundary at xmin
                        owner = cell_flat(0, j, k);
                        neighbor = None;
                        normal = [-1.0, 0.0, 0.0];
                        xmin_faces.push(face_id);
                    } else if i == self.nx {
                        // boundary at xmax
                        owner = cell_flat(self.nx - 1, j, k);
                        neighbor = None;
                        normal = [1.0, 0.0, 0.0];
                        xmax_faces.push(face_id);
                    } else {
                        // internal
                        owner = cell_flat(i - 1, j, k);
                        neighbor = Some(cell_flat(i, j, k));
                        normal = [1.0, 0.0, 0.0];
                    }

                    faces.push(Face::new(face_id, face_nodes, owner, neighbor, area, normal, center));
                }
            }
        }

        // --- Y-direction faces ---
        for k in 0..effective_nz {
            for j in 0..=self.ny {
                for i in 0..self.nx {
                    let face_id = faces.len();
                    let fn0 = self.node_index(i, j, k, effective_nz);
                    let fn1 = self.node_index(i + 1, j, k, effective_nz);
                    let fn2 = self.node_index(i + 1, j, k + 1, effective_nz);
                    let fn3 = self.node_index(i, j, k + 1, effective_nz);
                    let face_nodes = vec![fn0, fn1, fn2, fn3];

                    let area = self.dx * dz_eff;
                    let center = [
                        self.origin[0] + (i as f64 + 0.5) * self.dx,
                        self.origin[1] + j as f64 * self.dy,
                        self.origin[2] + (k as f64 + 0.5) * dz_eff,
                    ];

                    let (owner, neighbor, normal);
                    if j == 0 {
                        owner = cell_flat(i, 0, k);
                        neighbor = None;
                        normal = [0.0, -1.0, 0.0];
                        ymin_faces.push(face_id);
                    } else if j == self.ny {
                        owner = cell_flat(i, self.ny - 1, k);
                        neighbor = None;
                        normal = [0.0, 1.0, 0.0];
                        ymax_faces.push(face_id);
                    } else {
                        owner = cell_flat(i, j - 1, k);
                        neighbor = Some(cell_flat(i, j, k));
                        normal = [0.0, 1.0, 0.0];
                    }

                    faces.push(Face::new(face_id, face_nodes, owner, neighbor, area, normal, center));
                }
            }
        }

        // --- Z-direction faces ---
        for k in 0..=effective_nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let face_id = faces.len();
                    let fn0 = self.node_index(i, j, k, effective_nz);
                    let fn1 = self.node_index(i + 1, j, k, effective_nz);
                    let fn2 = self.node_index(i + 1, j + 1, k, effective_nz);
                    let fn3 = self.node_index(i, j + 1, k, effective_nz);
                    let face_nodes = vec![fn0, fn1, fn2, fn3];

                    let area = self.dx * self.dy;
                    let center = [
                        self.origin[0] + (i as f64 + 0.5) * self.dx,
                        self.origin[1] + (j as f64 + 0.5) * self.dy,
                        self.origin[2] + k as f64 * dz_eff,
                    ];

                    let (owner, neighbor, normal);
                    if k == 0 {
                        owner = cell_flat(i, j, 0);
                        neighbor = None;
                        normal = [0.0, 0.0, -1.0];
                        zmin_faces.push(face_id);
                    } else if k == effective_nz {
                        owner = cell_flat(i, j, effective_nz - 1);
                        neighbor = None;
                        normal = [0.0, 0.0, 1.0];
                        zmax_faces.push(face_id);
                    } else {
                        owner = cell_flat(i, j, k - 1);
                        neighbor = Some(cell_flat(i, j, k));
                        normal = [0.0, 0.0, 1.0];
                    }

                    faces.push(Face::new(face_id, face_nodes, owner, neighbor, area, normal, center));
                }
            }
        }

        // -----------------------------------------------------------
        // 4. Boundary patches
        // -----------------------------------------------------------
        let mut boundary_patches = Vec::new();
        if !xmin_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("xmin", xmin_faces));
        }
        if !xmax_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("xmax", xmax_faces));
        }
        if !ymin_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("ymin", ymin_faces));
        }
        if !ymax_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("ymax", ymax_faces));
        }
        if !zmin_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("zmin", zmin_faces));
        }
        if !zmax_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("zmax", zmax_faces));
        }

        // -----------------------------------------------------------
        // 5. Populate cell face lists
        // -----------------------------------------------------------
        for face in &faces {
            cells[face.owner_cell].faces.push(face.id);
            if let Some(nbr) = face.neighbor_cell {
                cells[nbr].faces.push(face.id);
            }
        }

        UnstructuredMesh::from_components(nodes, faces, cells, boundary_patches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 2x2x1 mesh: 4 cells, 12 nodes for the effective grid,
    /// and the correct total number of faces.
    #[test]
    fn test_2x2x1_mesh() {
        let mesh = StructuredMesh::new(2, 2, 1, 1.0, 1.0, 1.0, [0.0, 0.0, 0.0]);
        let um = mesh.to_unstructured();

        assert_eq!(um.num_cells(), 4);
        // Nodes: (2+1)*(2+1)*(1+1) = 18 -- wait, nz=1 so effective_nz=1
        // Actually (nx+1)*(ny+1)*(effective_nz+1) = 3*3*2 = 18
        assert_eq!(um.num_nodes(), 18);

        // Faces: x-faces = (2+1)*2*1 = 6, y-faces = 2*(2+1)*1 = 6, z-faces = 2*2*(1+1) = 8
        // Total = 6 + 6 + 8 = 20
        assert_eq!(um.num_faces(), 20);
    }

    /// Test 2x2 with nz=0 (2D case treated as single layer).
    #[test]
    fn test_2x2_nz0_mesh() {
        let mesh = StructuredMesh::new(2, 2, 0, 1.0, 1.0, 0.0, [0.0, 0.0, 0.0]);
        let um = mesh.to_unstructured();

        // effective_nz = 1, so same topology as 2x2x1
        assert_eq!(um.num_cells(), 4);
        assert_eq!(um.num_nodes(), 18);
        assert_eq!(um.num_faces(), 20);
    }

    /// Test 3x3x1 mesh boundary patch face counts.
    #[test]
    fn test_3x3x1_boundary_patches() {
        let mesh = StructuredMesh::new(3, 3, 1, 1.0, 1.0, 1.0, [0.0, 0.0, 0.0]);
        let um = mesh.to_unstructured();

        assert_eq!(um.num_cells(), 9);

        let xmin = um.boundary_patch("xmin").unwrap();
        let xmax = um.boundary_patch("xmax").unwrap();
        let ymin = um.boundary_patch("ymin").unwrap();
        let ymax = um.boundary_patch("ymax").unwrap();
        let zmin = um.boundary_patch("zmin").unwrap();
        let zmax = um.boundary_patch("zmax").unwrap();

        // xmin/xmax: ny * effective_nz = 3*1 = 3
        assert_eq!(xmin.num_faces(), 3);
        assert_eq!(xmax.num_faces(), 3);
        // ymin/ymax: nx * effective_nz = 3*1 = 3
        assert_eq!(ymin.num_faces(), 3);
        assert_eq!(ymax.num_faces(), 3);
        // zmin/zmax: nx * ny = 3*3 = 9
        assert_eq!(zmin.num_faces(), 9);
        assert_eq!(zmax.num_faces(), 9);
    }

    /// All internal faces must have both owner and neighbor.
    #[test]
    fn test_internal_faces_have_neighbor() {
        let mesh = StructuredMesh::new(3, 3, 2, 1.0, 1.0, 1.0, [0.0, 0.0, 0.0]);
        let um = mesh.to_unstructured();

        for face in &um.faces {
            if face.neighbor_cell.is_some() {
                // Internal face: owner != neighbor
                assert_ne!(face.owner_cell, face.neighbor_cell.unwrap());
            }
        }

        // Count internal vs boundary
        let boundary_count: usize = um.boundary_patches.iter().map(|p| p.num_faces()).sum();
        let internal_count = um.num_faces() - boundary_count;

        for face in &um.faces {
            if face.is_boundary() {
                assert!(face.neighbor_cell.is_none());
            } else {
                assert!(face.neighbor_cell.is_some());
            }
        }

        // Verify counts match
        let actual_boundary = um.faces.iter().filter(|f| f.is_boundary()).count();
        assert_eq!(actual_boundary, boundary_count);
        let actual_internal = um.faces.iter().filter(|f| !f.is_boundary()).count();
        assert_eq!(actual_internal, internal_count);
    }

    /// All boundary faces must have neighbor = None.
    #[test]
    fn test_boundary_faces_no_neighbor() {
        let mesh = StructuredMesh::new(2, 3, 2, 0.5, 0.5, 0.5, [0.0, 0.0, 0.0]);
        let um = mesh.to_unstructured();

        for patch in &um.boundary_patches {
            for &fid in &patch.face_ids {
                let face = &um.faces[fid];
                assert!(
                    face.neighbor_cell.is_none(),
                    "Boundary face {} on patch '{}' should have no neighbor",
                    fid,
                    patch.name
                );
            }
        }
    }

    /// Test face area and normal correctness for a 1x1x1 mesh.
    #[test]
    fn test_1x1x1_face_area_and_normal() {
        let dx = 2.0;
        let dy = 3.0;
        let dz = 5.0;
        let mesh = StructuredMesh::new(1, 1, 1, dx, dy, dz, [0.0, 0.0, 0.0]);
        let um = mesh.to_unstructured();

        // 1 cell, 8 nodes
        assert_eq!(um.num_cells(), 1);
        assert_eq!(um.num_nodes(), 8);
        // x-faces: 2*1*1=2, y-faces: 1*2*1=2, z-faces: 1*1*2=2 => 6 faces (all boundary)
        assert_eq!(um.num_faces(), 6);

        // Every face is a boundary face
        for face in &um.faces {
            assert!(face.is_boundary());
        }

        // Check areas and normals
        // X-faces: area = dy*dz
        let xmin_patch = um.boundary_patch("xmin").unwrap();
        assert_eq!(xmin_patch.num_faces(), 1);
        let xmin_face = &um.faces[xmin_patch.face_ids[0]];
        assert!((xmin_face.area - dy * dz).abs() < 1e-12);
        assert_eq!(xmin_face.normal, [-1.0, 0.0, 0.0]);

        let xmax_patch = um.boundary_patch("xmax").unwrap();
        let xmax_face = &um.faces[xmax_patch.face_ids[0]];
        assert!((xmax_face.area - dy * dz).abs() < 1e-12);
        assert_eq!(xmax_face.normal, [1.0, 0.0, 0.0]);

        // Y-faces: area = dx*dz
        let ymin_patch = um.boundary_patch("ymin").unwrap();
        let ymin_face = &um.faces[ymin_patch.face_ids[0]];
        assert!((ymin_face.area - dx * dz).abs() < 1e-12);
        assert_eq!(ymin_face.normal, [0.0, -1.0, 0.0]);

        let ymax_patch = um.boundary_patch("ymax").unwrap();
        let ymax_face = &um.faces[ymax_patch.face_ids[0]];
        assert!((ymax_face.area - dx * dz).abs() < 1e-12);
        assert_eq!(ymax_face.normal, [0.0, 1.0, 0.0]);

        // Z-faces: area = dx*dy
        let zmin_patch = um.boundary_patch("zmin").unwrap();
        let zmin_face = &um.faces[zmin_patch.face_ids[0]];
        assert!((zmin_face.area - dx * dy).abs() < 1e-12);
        assert_eq!(zmin_face.normal, [0.0, 0.0, -1.0]);

        let zmax_patch = um.boundary_patch("zmax").unwrap();
        let zmax_face = &um.faces[zmax_patch.face_ids[0]];
        assert!((zmax_face.area - dx * dy).abs() < 1e-12);
        assert_eq!(zmax_face.normal, [0.0, 0.0, 1.0]);

        // The single cell should have 6 faces
        assert_eq!(um.cells[0].faces.len(), 6);
    }
}
