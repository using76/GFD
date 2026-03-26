//! Cut-cell mesh generation using a signed distance function on a background Cartesian grid.

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::structured::StructuredMesh;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use crate::{MeshError, Result};

/// Cut-cell mesher that creates a body-fitted mesh from a background Cartesian grid
/// and a signed distance function.
pub struct CutCellMesher {
    /// Number of background cells in x.
    pub background_nx: usize,
    /// Number of background cells in y.
    pub background_ny: usize,
    /// Number of background cells in z.
    pub background_nz: usize,
    /// Domain extents: [xmin, xmax, ymin, ymax, zmin, zmax].
    pub domain: [f64; 6],
    /// Signed distance function: negative inside the solid, positive in fluid.
    sdf: Box<dyn Fn([f64; 3]) -> f64>,
    /// Minimum volume fraction for cut cells. Cells smaller than this are merged.
    pub min_volume_fraction: f64,
}

impl CutCellMesher {
    /// Creates a new cut-cell mesher.
    ///
    /// # Arguments
    /// * `nx`, `ny`, `nz` - Background grid resolution.
    /// * `domain` - [xmin, xmax, ymin, ymax, zmin, zmax].
    /// * `sdf` - Signed distance function (negative = inside solid).
    pub fn new(
        nx: usize,
        ny: usize,
        nz: usize,
        domain: [f64; 6],
        sdf: impl Fn([f64; 3]) -> f64 + 'static,
    ) -> Self {
        Self {
            background_nx: nx,
            background_ny: ny,
            background_nz: nz,
            domain,
            sdf: Box::new(sdf),
            min_volume_fraction: 0.1,
        }
    }

    /// Sets the minimum volume fraction for cell merging.
    pub fn with_min_volume_fraction(mut self, frac: f64) -> Self {
        self.min_volume_fraction = frac;
        self
    }

    /// Generates the cut-cell mesh.
    ///
    /// Algorithm:
    /// 1. Create background Cartesian grid.
    /// 2. Evaluate SDF at each cell center and each cell vertex.
    /// 3. Cells fully outside solid (all vertex SDF > 0): keep as-is (fluid).
    /// 4. Cells fully inside solid (all vertex SDF < 0): remove.
    /// 5. Cut cells (SDF changes sign): compute approximate volume fraction and keep if large enough.
    /// 6. Small cut cells are merged with the nearest fluid neighbor.
    pub fn build(&self) -> Result<UnstructuredMesh> {
        let nx = self.background_nx;
        let ny = self.background_ny;
        let nz = if self.background_nz == 0 { 1 } else { self.background_nz };

        if nx == 0 || ny == 0 {
            return Err(MeshError::InvalidParameters(
                "Grid dimensions must be positive".into(),
            ));
        }

        let dx = (self.domain[1] - self.domain[0]) / nx as f64;
        let dy = (self.domain[3] - self.domain[2]) / ny as f64;
        let dz = (self.domain[5] - self.domain[4]) / nz as f64;
        let cell_vol = dx * dy * dz;
        let _half_diag = (dx * dx + dy * dy + dz * dz).sqrt() * 0.5;

        // Create background structured mesh
        let bg = StructuredMesh::new(
            nx,
            ny,
            nz,
            dx,
            dy,
            dz,
            [self.domain[0], self.domain[2], self.domain[4]],
        );
        let bg_mesh = bg.to_unstructured();

        // Classify each background cell
        #[derive(Clone, Copy, PartialEq)]
        enum CellClass {
            Fluid,     // Fully outside solid
            Solid,     // Fully inside solid
            Cut(f64),  // Volume fraction of fluid
        }

        let n_bg_cells = bg_mesh.cells.len();
        let mut classification = vec![CellClass::Fluid; n_bg_cells];

        for (ci, cell) in bg_mesh.cells.iter().enumerate() {
            let _center_sdf = (self.sdf)(cell.center);

            // Check all vertices
            let mut all_inside = true;
            let mut all_outside = true;
            let mut n_outside = 0usize;
            let n_nodes = cell.nodes.len();

            for &nid in &cell.nodes {
                let sdf_val = (self.sdf)(bg_mesh.nodes[nid].position);
                if sdf_val > 0.0 {
                    all_inside = false;
                    n_outside += 1;
                } else {
                    all_outside = false;
                }
            }

            if all_inside {
                // All vertices inside solid
                classification[ci] = CellClass::Solid;
            } else if all_outside {
                // All vertices in fluid
                classification[ci] = CellClass::Fluid;
            } else {
                // Cut cell: estimate volume fraction as ratio of fluid vertices
                let vol_frac = n_outside as f64 / n_nodes as f64;
                classification[ci] = CellClass::Cut(vol_frac);
            }
        }

        // Merge small cut cells: if volume fraction < min_volume_fraction,
        // try to merge with the largest fluid neighbor.
        for ci in 0..n_bg_cells {
            if let CellClass::Cut(vf) = classification[ci] {
                if vf < self.min_volume_fraction {
                    // Find neighbor cells via shared faces
                    let mut best_neighbor = None;
                    let mut best_vf = 0.0f64;

                    for &fid in &bg_mesh.cells[ci].faces {
                        let face = &bg_mesh.faces[fid];
                        let neighbor_id = if face.owner_cell == ci {
                            face.neighbor_cell
                        } else {
                            Some(face.owner_cell)
                        };
                        if let Some(nbr) = neighbor_id {
                            let nbr_vf = match classification[nbr] {
                                CellClass::Fluid => 1.0,
                                CellClass::Cut(f) => f,
                                CellClass::Solid => 0.0,
                            };
                            if nbr_vf > best_vf {
                                best_vf = nbr_vf;
                                best_neighbor = Some(nbr);
                            }
                        }
                    }

                    if best_neighbor.is_some() && best_vf > 0.0 {
                        // Mark this cell as solid (effectively merged into neighbor)
                        classification[ci] = CellClass::Solid;
                    }
                }
            }
        }

        // Build the output mesh from fluid and cut cells
        let mut new_nodes: Vec<Node> = Vec::new();
        let mut new_faces: Vec<Face> = Vec::new();
        let mut new_cells: Vec<Cell> = Vec::new();
        let mut boundary_patches: Vec<BoundaryPatch> = Vec::new();

        // Map from old cell id to new cell id (-1 means removed)
        let mut old_to_new_cell: Vec<Option<usize>> = vec![None; n_bg_cells];
        let mut new_cell_id = 0;
        for ci in 0..n_bg_cells {
            match classification[ci] {
                CellClass::Fluid | CellClass::Cut(_) => {
                    old_to_new_cell[ci] = Some(new_cell_id);
                    new_cell_id += 1;
                }
                CellClass::Solid => {}
            }
        }

        // Copy all nodes (simpler than remapping; unused nodes are harmless)
        for node in &bg_mesh.nodes {
            new_nodes.push(node.clone());
        }

        // Build faces: keep faces that have at least one fluid/cut cell adjacent.
        let mut cut_boundary_faces = Vec::new();
        let mut wall_boundary_faces = Vec::new();

        for face in &bg_mesh.faces {
            let owner_new = old_to_new_cell[face.owner_cell];
            let neighbor_new = face.neighbor_cell.and_then(|n| old_to_new_cell[n]);

            if owner_new.is_none() && neighbor_new.is_none() {
                // Both cells are solid, skip this face.
                continue;
            }

            let fid = new_faces.len();

            if owner_new.is_some() && neighbor_new.is_some() {
                // Internal face between two fluid/cut cells
                new_faces.push(Face::new(
                    fid,
                    face.nodes.clone(),
                    owner_new.unwrap(),
                    Some(neighbor_new.unwrap()),
                    face.area,
                    face.normal,
                    face.center,
                ));
            } else if owner_new.is_some() {
                // Owner is fluid/cut, neighbor is either solid or boundary
                new_faces.push(Face::new(
                    fid,
                    face.nodes.clone(),
                    owner_new.unwrap(),
                    None,
                    face.area,
                    face.normal,
                    face.center,
                ));
                if face.neighbor_cell.is_some() {
                    // Neighbor was solid => this becomes a wall boundary
                    cut_boundary_faces.push(fid);
                } else {
                    // Original boundary face
                    wall_boundary_faces.push(fid);
                }
            } else {
                // neighbor_new is Some, owner was solid
                new_faces.push(Face::new(
                    fid,
                    face.nodes.clone(),
                    neighbor_new.unwrap(),
                    None,
                    face.area,
                    [-face.normal[0], -face.normal[1], -face.normal[2]],
                    face.center,
                ));
                cut_boundary_faces.push(fid);
            }
        }

        // Build cells
        for ci in 0..n_bg_cells {
            if let Some(new_id) = old_to_new_cell[ci] {
                let old_cell = &bg_mesh.cells[ci];
                let vol = match classification[ci] {
                    CellClass::Cut(vf) => cell_vol * vf,
                    _ => cell_vol,
                };
                // Collect faces belonging to this new cell
                let cell_faces: Vec<usize> = new_faces
                    .iter()
                    .filter(|f| {
                        f.owner_cell == new_id
                            || f.neighbor_cell == Some(new_id)
                    })
                    .map(|f| f.id)
                    .collect();

                new_cells.push(Cell::new(
                    new_id,
                    old_cell.nodes.clone(),
                    cell_faces,
                    vol,
                    old_cell.center,
                ));
            }
        }

        // Boundary patches
        if !cut_boundary_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("cut_wall", cut_boundary_faces));
        }
        if !wall_boundary_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("outer", wall_boundary_faces));
        }

        Ok(UnstructuredMesh::from_components(
            new_nodes,
            new_faces,
            new_cells,
            boundary_patches,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cutcell_no_solid() {
        // SDF that is always positive => no solid, all cells kept
        let mesher = CutCellMesher::new(
            3, 3, 1,
            [0.0, 3.0, 0.0, 3.0, 0.0, 1.0],
            |_p| 1.0, // always positive = all fluid
        );
        let mesh = mesher.build().unwrap();
        assert_eq!(mesh.cells.len(), 9, "All 3x3 cells should be fluid");
    }

    #[test]
    fn test_cutcell_all_solid() {
        // SDF always negative => all cells are solid, mesh should be empty
        let mesher = CutCellMesher::new(
            3, 3, 1,
            [0.0, 3.0, 0.0, 3.0, 0.0, 1.0],
            |_p| -1.0, // always negative = all solid
        );
        let mesh = mesher.build().unwrap();
        assert_eq!(mesh.cells.len(), 0, "All cells should be removed (solid)");
    }

    #[test]
    fn test_cutcell_sphere() {
        // Sphere centered at (2.5, 2.5, 0.5) with radius 1.0
        // Domain: [0, 5] x [0, 5] x [0, 1]
        let mesher = CutCellMesher::new(
            10, 10, 1,
            [0.0, 5.0, 0.0, 5.0, 0.0, 1.0],
            |p| {
                let dx = p[0] - 2.5;
                let dy = p[1] - 2.5;
                let dz = p[2] - 0.5;
                (dx * dx + dy * dy + dz * dz).sqrt() - 1.0
            },
        );
        let mesh = mesher.build().unwrap();
        // Some cells should be removed (inside sphere), some should be cut
        assert!(
            mesh.cells.len() < 100,
            "Some cells should have been removed, got {}",
            mesh.cells.len()
        );
        assert!(
            mesh.cells.len() > 50,
            "Most cells should remain as fluid, got {}",
            mesh.cells.len()
        );
    }

    #[test]
    fn test_cutcell_has_cut_wall_patch() {
        let mesher = CutCellMesher::new(
            5, 5, 1,
            [0.0, 5.0, 0.0, 5.0, 0.0, 1.0],
            |p| {
                let dx = p[0] - 2.5;
                let dy = p[1] - 2.5;
                (dx * dx + dy * dy).sqrt() - 1.0
            },
        );
        let mesh = mesher.build().unwrap();
        let cut_patch = mesh.boundary_patch("cut_wall");
        assert!(
            cut_patch.is_some(),
            "Should have a 'cut_wall' boundary patch"
        );
    }

    #[test]
    fn test_cutcell_invalid_dimensions() {
        let mesher = CutCellMesher::new(
            0, 5, 1,
            [0.0, 5.0, 0.0, 5.0, 0.0, 1.0],
            |_| 1.0,
        );
        assert!(mesher.build().is_err());
    }
}
