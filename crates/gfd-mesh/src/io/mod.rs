//! Mesh I/O — export generated meshes to various formats.

pub mod export;
pub mod formats;

use gfd_core::mesh::unstructured::UnstructuredMesh;
use crate::Result;

/// Export mesh statistics as a summary string.
pub fn mesh_summary(mesh: &UnstructuredMesh) -> String {
    let n_cells = mesh.num_cells();
    let n_faces = mesh.num_faces();
    let n_nodes = mesh.num_nodes();
    let n_patches = mesh.boundary_patches.len();

    let internal_faces = mesh.faces.iter().filter(|f| f.neighbor_cell.is_some()).count();
    let boundary_faces = n_faces - internal_faces;

    format!(
        "Mesh Summary:\n  Cells:           {}\n  Faces:           {} ({} internal, {} boundary)\n  Nodes:           {}\n  Boundary patches: {}\n  Patches: {}",
        n_cells, n_faces, internal_faces, boundary_faces, n_nodes, n_patches,
        mesh.boundary_patches.iter().map(|p| format!("{} ({})", p.name, p.face_ids.len()))
            .collect::<Vec<_>>().join(", ")
    )
}

/// Verify mesh consistency (no dangling faces, volumes > 0, etc.).
pub fn verify_mesh(mesh: &UnstructuredMesh) -> Result<()> {
    // Check cell volumes
    for (i, cell) in mesh.cells.iter().enumerate() {
        if cell.volume <= 0.0 {
            return Err(crate::MeshError::QualityFailed(
                format!("Cell {} has non-positive volume: {}", i, cell.volume)));
        }
    }

    // Check face areas
    for (i, face) in mesh.faces.iter().enumerate() {
        if face.area <= 0.0 {
            return Err(crate::MeshError::QualityFailed(
                format!("Face {} has non-positive area: {}", i, face.area)));
        }
    }

    // Check owner cells are valid
    for (i, face) in mesh.faces.iter().enumerate() {
        if face.owner_cell >= mesh.num_cells() {
            return Err(crate::MeshError::QualityFailed(
                format!("Face {} has invalid owner cell: {}", i, face.owner_cell)));
        }
        if let Some(nb) = face.neighbor_cell {
            if nb >= mesh.num_cells() {
                return Err(crate::MeshError::QualityFailed(
                    format!("Face {} has invalid neighbor cell: {}", i, nb)));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test]
    fn test_mesh_summary() {
        let sm = StructuredMesh::uniform(5, 5, 0, 1.0, 1.0, 0.0);
        let mesh = sm.to_unstructured();
        let summary = mesh_summary(&mesh);
        assert!(summary.contains("Cells:"));
        assert!(summary.contains("25")); // 5x5 = 25 cells
    }

    #[test]
    fn test_verify_valid_mesh() {
        let sm = StructuredMesh::uniform(3, 3, 0, 1.0, 1.0, 0.0);
        let mesh = sm.to_unstructured();
        assert!(verify_mesh(&mesh).is_ok());
    }
}
