//! Overset (Chimera) mesh assembly.
//!
//! Combines a background mesh with one or more component (overlapping) meshes.
//! Provides hole cutting, donor cell identification, and interpolation stencil
//! computation for Chimera-type simulations.

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::node::Node;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use crate::{MeshError, Result};

/// An interpolation stencil describing how a component cell receives data.
#[derive(Debug, Clone)]
pub struct InterpolationStencil {
    /// Index of the component mesh (in `component_meshes`).
    pub component_idx: usize,
    /// Cell index within the component mesh.
    pub component_cell: usize,
    /// Donor cell index in the background mesh.
    pub donor_cell: usize,
    /// Interpolation weight (1.0 = exact match at donor center).
    pub weight: f64,
}

/// Overset (Chimera) mesh that combines a background mesh with component meshes.
///
/// The background mesh covers the entire domain. Component meshes overlap
/// parts of the background mesh. Hole cells in the background are identified
/// (cells covered by component meshes) and interpolation stencils are computed
/// for fringe cells.
pub struct OversetMesh {
    /// The background mesh.
    pub background: UnstructuredMesh,
    /// Component (overlapping) meshes.
    pub component_meshes: Vec<UnstructuredMesh>,
}

impl OversetMesh {
    /// Create a new overset mesh from a background and component meshes.
    pub fn new(background: UnstructuredMesh, component_meshes: Vec<UnstructuredMesh>) -> Self {
        Self {
            background,
            component_meshes,
        }
    }

    /// Find donor cells in the background for each component cell.
    ///
    /// For each cell in each component mesh, finds the nearest background cell
    /// (by cell center distance) and assigns it as the donor.
    pub fn compute_interpolation_stencil(&self) -> Vec<InterpolationStencil> {
        let mut stencils = Vec::new();

        for (comp_idx, comp) in self.component_meshes.iter().enumerate() {
            for (ci, comp_cell) in comp.cells.iter().enumerate() {
                // Find the nearest background cell to this component cell center
                let cc = comp_cell.center;
                let mut best_bg_cell = 0;
                let mut best_dist2 = f64::MAX;

                for (bi, bg_cell) in self.background.cells.iter().enumerate() {
                    let d2 = dist2_3d(cc, bg_cell.center);
                    if d2 < best_dist2 {
                        best_dist2 = d2;
                        best_bg_cell = bi;
                    }
                }

                // Weight based on inverse distance (1.0 at exact match)
                let dist = best_dist2.sqrt();
                let weight = if dist < 1e-30 {
                    1.0
                } else {
                    // Use a characteristic length from the donor cell volume
                    let char_len = self.background.cells[best_bg_cell].volume.cbrt();
                    (1.0 - (dist / char_len).min(1.0)).max(0.0)
                };

                stencils.push(InterpolationStencil {
                    component_idx: comp_idx,
                    component_cell: ci,
                    donor_cell: best_bg_cell,
                    weight,
                });
            }
        }

        stencils
    }

    /// Mark hole cells in the background mesh.
    ///
    /// A background cell is a "hole" if any component cell center lies within
    /// it (approximated by nearest-cell proximity).  Returns a boolean vector
    /// indexed by background cell id; `true` means the cell is a hole.
    pub fn mark_hole_cells(&self) -> Vec<bool> {
        let n_bg = self.background.cells.len();
        let mut is_hole = vec![false; n_bg];

        for comp in &self.component_meshes {
            for comp_cell in &comp.cells {
                let cc = comp_cell.center;

                // Find the background cell whose center is nearest
                let mut best_bg = 0;
                let mut best_d2 = f64::MAX;
                for (bi, bg_cell) in self.background.cells.iter().enumerate() {
                    let d2 = dist2_3d(cc, bg_cell.center);
                    if d2 < best_d2 {
                        best_d2 = d2;
                        best_bg = bi;
                    }
                }

                // Mark as hole if the component cell center is close enough
                // (within the background cell's characteristic length)
                let char_len = self.background.cells[best_bg].volume.cbrt();
                if best_d2.sqrt() < char_len {
                    is_hole[best_bg] = true;
                }
            }
        }

        is_hole
    }

    /// Get the combined active mesh.
    ///
    /// Removes hole cells from the background and merges remaining background
    /// cells with all component mesh cells into a single `UnstructuredMesh`.
    pub fn combined_mesh(&self) -> Result<UnstructuredMesh> {
        let hole_mask = self.mark_hole_cells();

        // Count active background cells
        let active_bg_count = hole_mask.iter().filter(|&&h| !h).count();
        let total_comp_cells: usize = self.component_meshes.iter().map(|c| c.cells.len()).sum();

        if active_bg_count + total_comp_cells == 0 {
            return Err(MeshError::GenerationFailed(
                "Combined mesh has no active cells".to_string(),
            ));
        }

        let mut nodes: Vec<Node> = Vec::new();
        let mut faces: Vec<Face> = Vec::new();
        let mut cells: Vec<Cell> = Vec::new();
        let mut boundary_patches: Vec<BoundaryPatch> = Vec::new();

        // --- Add active background cells ---
        // Re-index nodes: copy all background nodes
        let bg_node_offset = 0;
        for node in &self.background.nodes {
            nodes.push(Node::new(bg_node_offset + node.id, node.position));
        }

        // Map old bg cell id -> new cell id
        let mut bg_cell_map: Vec<Option<usize>> = vec![None; self.background.cells.len()];
        let mut new_cell_id = 0;
        for (ci, &is_hole) in hole_mask.iter().enumerate() {
            if !is_hole {
                bg_cell_map[ci] = Some(new_cell_id);
                new_cell_id += 1;
            }
        }

        // Add background faces (only those with at least one active cell)
        let mut bg_boundary_faces = Vec::new();
        for face in &self.background.faces {
            let owner_new = bg_cell_map[face.owner_cell];
            let neighbor_new = face.neighbor_cell.and_then(|n| bg_cell_map[n]);

            if owner_new.is_none() && neighbor_new.is_none() {
                continue;
            }

            let fid = faces.len();

            if owner_new.is_some() && neighbor_new.is_some() {
                faces.push(Face::new(
                    fid,
                    face.nodes.clone(),
                    owner_new.unwrap(),
                    Some(neighbor_new.unwrap()),
                    face.area,
                    face.normal,
                    face.center,
                ));
            } else if let Some(owner) = owner_new {
                faces.push(Face::new(
                    fid,
                    face.nodes.clone(),
                    owner,
                    None,
                    face.area,
                    face.normal,
                    face.center,
                ));
                bg_boundary_faces.push(fid);
            } else if let Some(nbr) = neighbor_new {
                faces.push(Face::new(
                    fid,
                    face.nodes.clone(),
                    nbr,
                    None,
                    face.area,
                    [-face.normal[0], -face.normal[1], -face.normal[2]],
                    face.center,
                ));
                bg_boundary_faces.push(fid);
            }
        }

        // Add active background cells
        for (ci, bg_cell) in self.background.cells.iter().enumerate() {
            if let Some(new_id) = bg_cell_map[ci] {
                let cell_faces: Vec<usize> = faces
                    .iter()
                    .filter(|f| f.owner_cell == new_id || f.neighbor_cell == Some(new_id))
                    .map(|f| f.id)
                    .collect();

                cells.push(Cell::new(
                    new_id,
                    bg_cell.nodes.iter().map(|&n| bg_node_offset + n).collect(),
                    cell_faces,
                    bg_cell.volume,
                    bg_cell.center,
                ));
            }
        }

        // --- Add component mesh cells ---
        for comp in &self.component_meshes {
            let comp_node_offset = nodes.len();

            // Add component nodes
            for node in &comp.nodes {
                nodes.push(Node::new(comp_node_offset + node.id, node.position));
            }

            // Map component cell ids
            let comp_cell_offset = cells.len();
            let mut comp_boundary_faces = Vec::new();

            // Add component faces
            for face in &comp.faces {
                let fid = faces.len();
                let owner = comp_cell_offset + face.owner_cell;
                let neighbor = face.neighbor_cell.map(|n| comp_cell_offset + n);
                let face_nodes: Vec<usize> = face.nodes.iter().map(|&n| comp_node_offset + n).collect();

                faces.push(Face::new(
                    fid,
                    face_nodes,
                    owner,
                    neighbor,
                    face.area,
                    face.normal,
                    face.center,
                ));

                if neighbor.is_none() {
                    comp_boundary_faces.push(fid);
                }
            }

            // Add component cells
            for (ci, comp_cell) in comp.cells.iter().enumerate() {
                let new_id = comp_cell_offset + ci;
                let cell_nodes: Vec<usize> = comp_cell.nodes.iter().map(|&n| comp_node_offset + n).collect();
                let cell_faces: Vec<usize> = faces
                    .iter()
                    .filter(|f| f.owner_cell == new_id || f.neighbor_cell == Some(new_id))
                    .map(|f| f.id)
                    .collect();

                cells.push(Cell::new(
                    new_id,
                    cell_nodes,
                    cell_faces,
                    comp_cell.volume,
                    comp_cell.center,
                ));
            }

            if !comp_boundary_faces.is_empty() {
                boundary_patches.push(BoundaryPatch::new("component_boundary", comp_boundary_faces));
            }
        }

        if !bg_boundary_faces.is_empty() {
            boundary_patches.push(BoundaryPatch::new("background_boundary", bg_boundary_faces));
        }

        Ok(UnstructuredMesh::from_components(
            nodes,
            faces,
            cells,
            boundary_patches,
        ))
    }
}

/// Squared Euclidean distance between two 3D points.
fn dist2_3d(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Helper: create a simple hex mesh for testing.
#[cfg(test)]
fn make_test_mesh(
    nx: usize,
    ny: usize,
    lx: f64,
    ly: f64,
    lz: f64,
    origin: [f64; 3],
) -> UnstructuredMesh {
    use gfd_core::mesh::structured::StructuredMesh;

    let dx = lx / nx as f64;
    let dy = ly / ny as f64;
    let sm = StructuredMesh::new(nx, ny, 1, dx, dy, lz, origin);
    sm.to_unstructured()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overset_no_overlap() {
        // Background covers [0,4]x[0,4], component covers [5,6]x[5,6]
        // No overlap => no hole cells
        let bg = make_test_mesh(4, 4, 4.0, 4.0, 1.0, [0.0, 0.0, 0.0]);
        let comp = make_test_mesh(2, 2, 1.0, 1.0, 1.0, [5.0, 5.0, 0.0]);
        let overset = OversetMesh::new(bg, vec![comp]);

        let holes = overset.mark_hole_cells();
        assert!(
            holes.iter().all(|&h| !h),
            "No holes expected when meshes don't overlap",
        );
    }

    #[test]
    fn test_overset_with_overlap() {
        // Background covers [0,4]x[0,4], component covers [1,3]x[1,3]
        let bg = make_test_mesh(4, 4, 4.0, 4.0, 1.0, [0.0, 0.0, 0.0]);
        let comp = make_test_mesh(2, 2, 2.0, 2.0, 1.0, [1.0, 1.0, 0.0]);
        let overset = OversetMesh::new(bg, vec![comp]);

        let holes = overset.mark_hole_cells();
        let n_holes = holes.iter().filter(|&&h| h).count();
        assert!(
            n_holes > 0,
            "Should have hole cells when component overlaps background",
        );
    }

    #[test]
    fn test_interpolation_stencil() {
        let bg = make_test_mesh(4, 4, 4.0, 4.0, 1.0, [0.0, 0.0, 0.0]);
        let comp = make_test_mesh(2, 2, 2.0, 2.0, 1.0, [1.0, 1.0, 0.0]);
        let overset = OversetMesh::new(bg, vec![comp]);

        let stencils = overset.compute_interpolation_stencil();
        // Should have one stencil per component cell
        assert_eq!(stencils.len(), 4, "4 component cells => 4 stencils");

        for s in &stencils {
            assert_eq!(s.component_idx, 0);
            assert!(s.donor_cell < 16, "donor should be in background (16 cells)");
        }
    }

    #[test]
    fn test_combined_mesh() {
        let bg = make_test_mesh(4, 4, 4.0, 4.0, 1.0, [0.0, 0.0, 0.0]);
        let comp = make_test_mesh(2, 2, 2.0, 2.0, 1.0, [1.0, 1.0, 0.0]);

        let n_bg = bg.cells.len();
        let n_comp = comp.cells.len();

        let overset = OversetMesh::new(bg, vec![comp]);
        let holes = overset.mark_hole_cells();
        let n_holes = holes.iter().filter(|&&h| h).count();

        let combined = overset.combined_mesh().unwrap();

        // Combined = active background + all component cells
        let expected = n_bg - n_holes + n_comp;
        assert_eq!(
            combined.num_cells(),
            expected,
            "combined cells {} expected {} (bg={} - holes={} + comp={})",
            combined.num_cells(),
            expected,
            n_bg,
            n_holes,
            n_comp,
        );
    }

    #[test]
    fn test_combined_mesh_all_positive_volume() {
        let bg = make_test_mesh(3, 3, 3.0, 3.0, 1.0, [0.0, 0.0, 0.0]);
        let comp = make_test_mesh(2, 2, 1.0, 1.0, 1.0, [1.0, 1.0, 0.0]);
        let overset = OversetMesh::new(bg, vec![comp]);
        let combined = overset.combined_mesh().unwrap();

        for cell in &combined.cells {
            assert!(
                cell.volume > 0.0,
                "cell {} has non-positive volume {}",
                cell.id,
                cell.volume,
            );
        }
    }

    #[test]
    fn test_multiple_components() {
        let bg = make_test_mesh(6, 6, 6.0, 6.0, 1.0, [0.0, 0.0, 0.0]);
        let comp1 = make_test_mesh(2, 2, 1.0, 1.0, 1.0, [1.0, 1.0, 0.0]);
        let comp2 = make_test_mesh(2, 2, 1.0, 1.0, 1.0, [4.0, 4.0, 0.0]);
        let overset = OversetMesh::new(bg, vec![comp1, comp2]);

        let stencils = overset.compute_interpolation_stencil();
        // 4 cells from each component = 8 stencils
        assert_eq!(stencils.len(), 8);

        let combined = overset.combined_mesh().unwrap();
        assert!(combined.num_cells() > 0);
    }

    #[test]
    fn test_hole_marking_no_false_positives() {
        // Component mesh placed far away from background
        let bg = make_test_mesh(3, 3, 3.0, 3.0, 1.0, [0.0, 0.0, 0.0]);
        let comp = make_test_mesh(2, 2, 1.0, 1.0, 1.0, [100.0, 100.0, 0.0]);
        let overset = OversetMesh::new(bg, vec![comp]);

        let holes = overset.mark_hole_cells();
        let n_holes = holes.iter().filter(|&&h| h).count();
        assert_eq!(
            n_holes, 0,
            "No holes expected when component is far away",
        );
    }
}
