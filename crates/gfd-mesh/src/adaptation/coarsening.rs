//! Cell coarsening (merging) for adaptive mesh refinement.
//!
//! Merges groups of cells into larger cells, combining volumes, computing
//! new centers, and removing internal faces within each merge group.

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};

use std::collections::{HashMap, HashSet};

use crate::Result;

/// Merge groups of cells into larger cells.
///
/// For each group: combine volumes, compute the new center as the volume-weighted
/// average, merge face lists (remove internal faces within the group, keep external).
///
/// # Arguments
/// * `mesh` - The original mesh.
/// * `cells_to_merge` - Each inner `Vec<usize>` contains the indices of cells to merge
///   into a single cell. Cells not in any group are kept unchanged.
///
/// # Returns
/// A new `UnstructuredMesh` with merged cells.
pub fn coarsen_cells(
    mesh: &UnstructuredMesh,
    cells_to_merge: &[Vec<usize>],
) -> Result<UnstructuredMesh> {
    // Validate input
    let n_cells = mesh.num_cells();
    let mut cell_to_group: HashMap<usize, usize> = HashMap::new();
    for (gi, group) in cells_to_merge.iter().enumerate() {
        if group.is_empty() {
            return Err(crate::MeshError::InvalidParameters(
                "Empty merge group".to_string(),
            ));
        }
        for &ci in group {
            if ci >= n_cells {
                return Err(crate::MeshError::InvalidParameters(
                    format!("Cell index {} out of range (num_cells={})", ci, n_cells),
                ));
            }
            if cell_to_group.contains_key(&ci) {
                return Err(crate::MeshError::InvalidParameters(
                    format!("Cell {} appears in multiple merge groups", ci),
                ));
            }
            cell_to_group.insert(ci, gi);
        }
    }

    // Build new cells
    let mut new_cells: Vec<Cell> = Vec::new();

    // Track the old-cell-id -> new-cell-id mapping
    let mut old_to_new: HashMap<usize, usize> = HashMap::new();

    // First, add merged cells
    for group in cells_to_merge {
        let new_id = new_cells.len();
        let mut total_volume = 0.0;
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut cz = 0.0;
        let mut all_nodes: Vec<usize> = Vec::new();
        let mut node_set: HashSet<usize> = HashSet::new();

        for &ci in group {
            let cell = &mesh.cells[ci];
            total_volume += cell.volume;
            cx += cell.center[0] * cell.volume;
            cy += cell.center[1] * cell.volume;
            cz += cell.center[2] * cell.volume;

            for &nid in &cell.nodes {
                if node_set.insert(nid) {
                    all_nodes.push(nid);
                }
            }

            old_to_new.insert(ci, new_id);
        }

        if total_volume > 1e-30 {
            cx /= total_volume;
            cy /= total_volume;
            cz /= total_volume;
        }

        new_cells.push(Cell::new(
            new_id,
            all_nodes,
            Vec::new(), // faces rebuilt later
            total_volume,
            [cx, cy, cz],
        ));
    }

    // Then, add unchanged cells
    for ci in 0..n_cells {
        if cell_to_group.contains_key(&ci) {
            continue;
        }
        let new_id = new_cells.len();
        let cell = &mesh.cells[ci];
        old_to_new.insert(ci, new_id);
        new_cells.push(Cell::new(
            new_id,
            cell.nodes.clone(),
            Vec::new(),
            cell.volume,
            cell.center,
        ));
    }

    // Rebuild faces: skip faces that are internal to a merge group
    let mut new_faces: Vec<Face> = Vec::new();
    let mut boundary_face_ids: Vec<usize> = Vec::new();

    // Track which faces we've already processed (to avoid duplicates after remapping)
    let mut seen_face_pairs: HashSet<(usize, usize)> = HashSet::new();

    for face in &mesh.faces {
        let old_owner = face.owner_cell;
        let new_owner = old_to_new[&old_owner];

        let new_neighbor = face.neighbor_cell.map(|nb| old_to_new[&nb]);

        // If both owner and neighbor map to the same new cell, this face is internal
        // to the merge group — skip it.
        if let Some(new_nb) = new_neighbor {
            if new_owner == new_nb {
                continue;
            }

            // Avoid duplicate internal faces (same pair can appear from different old faces)
            let pair = if new_owner < new_nb {
                (new_owner, new_nb)
            } else {
                (new_nb, new_owner)
            };
            if !seen_face_pairs.insert(pair) {
                continue;
            }
        }

        let fid = new_faces.len();
        let mut new_face = Face::new(
            fid,
            face.nodes.clone(),
            new_owner,
            new_neighbor,
            face.area,
            face.normal,
            face.center,
        );
        let _ = &mut new_face; // suppress unused

        new_cells[new_owner].faces.push(fid);
        if let Some(new_nb) = new_neighbor {
            new_cells[new_nb].faces.push(fid);
        } else {
            boundary_face_ids.push(fid);
        }

        new_faces.push(Face::new(
            fid,
            face.nodes.clone(),
            new_owner,
            new_neighbor,
            face.area,
            face.normal,
            face.center,
        ));
    }

    // Build boundary patches
    let mut boundary_patches = Vec::new();
    if !boundary_face_ids.is_empty() {
        boundary_patches.push(BoundaryPatch::new("boundary", boundary_face_ids));
    }

    Ok(UnstructuredMesh::from_components(
        mesh.nodes.clone(),
        new_faces,
        new_cells,
        boundary_patches,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test]
    fn test_coarsen_no_groups() {
        let mesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        let result = coarsen_cells(&mesh, &[]).unwrap();
        assert_eq!(result.cells.len(), mesh.cells.len());
    }

    #[test]
    fn test_coarsen_two_cells() {
        let mesh = StructuredMesh::uniform(2, 1, 1, 2.0, 1.0, 1.0).to_unstructured();
        assert_eq!(mesh.cells.len(), 2);

        let result = coarsen_cells(&mesh, &[vec![0, 1]]).unwrap();
        assert_eq!(result.cells.len(), 1, "Merging 2 cells should produce 1 cell");
    }

    #[test]
    fn test_coarsen_preserves_volume() {
        let mesh = StructuredMesh::uniform(4, 1, 1, 4.0, 1.0, 1.0).to_unstructured();
        let original_volume: f64 = mesh.cells.iter().map(|c| c.volume).sum();

        // Merge cells 0+1 and 2+3
        let result = coarsen_cells(&mesh, &[vec![0, 1], vec![2, 3]]).unwrap();
        let coarsened_volume: f64 = result.cells.iter().map(|c| c.volume).sum();

        assert!(
            (original_volume - coarsened_volume).abs() < 1e-10,
            "Volume should be conserved: original={original_volume}, coarsened={coarsened_volume}"
        );
    }

    #[test]
    fn test_coarsen_partial() {
        let mesh = StructuredMesh::uniform(3, 1, 1, 3.0, 1.0, 1.0).to_unstructured();
        assert_eq!(mesh.cells.len(), 3);

        // Merge first two, keep third
        let result = coarsen_cells(&mesh, &[vec![0, 1]]).unwrap();
        assert_eq!(result.cells.len(), 2, "Should have 1 merged + 1 unchanged = 2 cells");
    }

    #[test]
    fn test_coarsen_invalid_duplicate() {
        let mesh = StructuredMesh::uniform(2, 1, 1, 2.0, 1.0, 1.0).to_unstructured();
        let result = coarsen_cells(&mesh, &[vec![0], vec![0]]);
        assert!(result.is_err(), "Duplicate cell in different groups should fail");
    }

    #[test]
    fn test_coarsen_invalid_index() {
        let mesh = StructuredMesh::uniform(2, 1, 1, 2.0, 1.0, 1.0).to_unstructured();
        let result = coarsen_cells(&mesh, &[vec![999]]);
        assert!(result.is_err(), "Out-of-range cell index should fail");
    }
}
