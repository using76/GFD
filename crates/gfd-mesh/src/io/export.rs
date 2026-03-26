//! Mesh export to various file formats.
//!
//! Supports exporting to Gmsh MSH v2.2 and VTK Legacy formats.

use gfd_core::mesh::unstructured::UnstructuredMesh;
use crate::Result;

/// Export mesh to Gmsh MSH v2.2 format.
///
/// The output file contains:
/// - `$MeshFormat` section (version 2.2, ASCII)
/// - `$Nodes` section with all mesh nodes
/// - `$Elements` section with all cells as elements
///
/// Supported element types:
/// - 4 nodes: tetrahedron (type 4)
/// - 5 nodes: pyramid (type 7)
/// - 6 nodes: prism/wedge (type 6)
/// - 8 nodes: hexahedron (type 5)
///
/// # Arguments
/// * `mesh` - The mesh to export.
/// * `path` - Output file path.
pub fn export_gmsh_msh(mesh: &UnstructuredMesh, path: &str) -> Result<()> {
    use std::fmt::Write as FmtWrite;

    let mut content = String::new();

    // MeshFormat section
    writeln!(content, "$MeshFormat").unwrap();
    writeln!(content, "2.2 0 8").unwrap();
    writeln!(content, "$EndMeshFormat").unwrap();

    // Nodes section (1-indexed in Gmsh)
    writeln!(content, "$Nodes").unwrap();
    writeln!(content, "{}", mesh.nodes.len()).unwrap();
    for node in &mesh.nodes {
        writeln!(
            content,
            "{} {:.15e} {:.15e} {:.15e}",
            node.id + 1,
            node.position[0],
            node.position[1],
            node.position[2]
        )
        .unwrap();
    }
    writeln!(content, "$EndNodes").unwrap();

    // Elements section
    writeln!(content, "$Elements").unwrap();
    writeln!(content, "{}", mesh.cells.len()).unwrap();
    for (i, cell) in mesh.cells.iter().enumerate() {
        let elem_type = match cell.nodes.len() {
            4 => 4,  // tetrahedron
            5 => 7,  // pyramid
            6 => 6,  // prism
            8 => 5,  // hexahedron
            _ => 4,  // fallback to tet
        };

        // Element: id type num_tags tag... node_ids...
        // 0 tags for simplicity
        let node_str: String = cell
            .nodes
            .iter()
            .map(|&n| (n + 1).to_string())
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(content, "{} {} 0 {}", i + 1, elem_type, node_str).unwrap();
    }
    writeln!(content, "$EndElements").unwrap();

    std::fs::write(path, content).map_err(|e| {
        crate::MeshError::GenerationFailed(format!("Failed to write Gmsh file: {}", e))
    })?;

    Ok(())
}

/// Export mesh to VTK Legacy format (ASCII).
///
/// Produces an unstructured grid file compatible with ParaView.
///
/// VTK cell types used:
/// - 4 nodes: VTK_TETRA (10)
/// - 5 nodes: VTK_PYRAMID (14)
/// - 6 nodes: VTK_WEDGE (13)
/// - 8 nodes: VTK_HEXAHEDRON (12)
///
/// # Arguments
/// * `mesh` - The mesh to export.
/// * `path` - Output file path.
pub fn export_vtk_mesh(mesh: &UnstructuredMesh, path: &str) -> Result<()> {
    use std::fmt::Write as FmtWrite;

    let mut content = String::new();

    // Header
    writeln!(content, "# vtk DataFile Version 3.0").unwrap();
    writeln!(content, "GFD Mesh Export").unwrap();
    writeln!(content, "ASCII").unwrap();
    writeln!(content, "DATASET UNSTRUCTURED_GRID").unwrap();

    // Points
    writeln!(content, "POINTS {} double", mesh.nodes.len()).unwrap();
    for node in &mesh.nodes {
        writeln!(
            content,
            "{:.15e} {:.15e} {:.15e}",
            node.position[0], node.position[1], node.position[2]
        )
        .unwrap();
    }

    // Cells
    let total_cell_data: usize = mesh.cells.iter().map(|c| c.nodes.len() + 1).sum();
    writeln!(content, "CELLS {} {}", mesh.cells.len(), total_cell_data).unwrap();
    for cell in &mesh.cells {
        let node_str: String = cell
            .nodes
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(content, "{} {}", cell.nodes.len(), node_str).unwrap();
    }

    // Cell types
    writeln!(content, "CELL_TYPES {}", mesh.cells.len()).unwrap();
    for cell in &mesh.cells {
        let vtk_type = match cell.nodes.len() {
            4 => 10,  // VTK_TETRA
            5 => 14,  // VTK_PYRAMID
            6 => 13,  // VTK_WEDGE
            8 => 12,  // VTK_HEXAHEDRON
            _ => 7,   // VTK_POLYGON (fallback)
        };
        writeln!(content, "{}", vtk_type).unwrap();
    }

    // Cell data: volumes
    writeln!(content, "CELL_DATA {}", mesh.cells.len()).unwrap();
    writeln!(content, "SCALARS volume double 1").unwrap();
    writeln!(content, "LOOKUP_TABLE default").unwrap();
    for cell in &mesh.cells {
        writeln!(content, "{:.15e}", cell.volume).unwrap();
    }

    std::fs::write(path, content).map_err(|e| {
        crate::MeshError::GenerationFailed(format!("Failed to write VTK file: {}", e))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_core::mesh::structured::StructuredMesh;

    #[test]
    fn test_export_gmsh_msh() {
        let mesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        let path = std::env::temp_dir().join("test_export.msh");
        let path_str = path.to_str().unwrap();

        export_gmsh_msh(&mesh, path_str).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("$MeshFormat"));
        assert!(content.contains("$Nodes"));
        assert!(content.contains("$Elements"));
        assert!(content.contains("$EndMeshFormat"));
        assert!(content.contains("$EndNodes"));
        assert!(content.contains("$EndElements"));

        // Clean up
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_gmsh_node_count() {
        let mesh = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();
        let path = std::env::temp_dir().join("test_gmsh_nodes.msh");
        let path_str = path.to_str().unwrap();

        export_gmsh_msh(&mesh, path_str).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        // The line after $Nodes should contain the node count
        let node_count_line = content
            .lines()
            .skip_while(|l| !l.contains("$Nodes"))
            .nth(1)
            .unwrap();
        let n: usize = node_count_line.trim().parse().unwrap();
        assert_eq!(n, mesh.nodes.len());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_vtk_mesh() {
        let mesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0).to_unstructured();
        let path = std::env::temp_dir().join("test_export.vtk");
        let path_str = path.to_str().unwrap();

        export_vtk_mesh(&mesh, path_str).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("vtk DataFile Version"));
        assert!(content.contains("UNSTRUCTURED_GRID"));
        assert!(content.contains("POINTS"));
        assert!(content.contains("CELLS"));
        assert!(content.contains("CELL_TYPES"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_vtk_cell_count() {
        let mesh = StructuredMesh::uniform(3, 3, 1, 3.0, 3.0, 1.0).to_unstructured();
        let path = std::env::temp_dir().join("test_vtk_cells.vtk");
        let path_str = path.to_str().unwrap();

        export_vtk_mesh(&mesh, path_str).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        // Find CELL_TYPES line and check count
        let ct_line = content
            .lines()
            .find(|l| l.starts_with("CELL_TYPES"))
            .unwrap();
        let count: usize = ct_line.split_whitespace().nth(1).unwrap().parse().unwrap();
        assert_eq!(count, mesh.cells.len());

        let _ = std::fs::remove_file(&path);
    }
}
