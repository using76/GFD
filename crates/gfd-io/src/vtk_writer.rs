//! VTK file writer for visualization.

use std::fs::File;
use std::io::{BufWriter, Write};

use gfd_core::{CellType, Field, FieldData, FieldSet, UnstructuredMesh};
use crate::Result;

/// Maps a `CellType` to the corresponding VTK cell type integer.
fn vtk_cell_type(ct: CellType) -> u8 {
    match ct {
        CellType::Tetrahedron => 10,
        CellType::Hexahedron => 12,
        CellType::Wedge => 13,
        CellType::Pyramid => 14,
        CellType::Polyhedron => 7,
    }
}

/// Writes mesh and field data in VTK legacy format (.vtk).
///
/// Produces unstructured grid files compatible with ParaView and other
/// VTK-based visualization tools.
pub fn write_vtk(
    path: &str,
    mesh: &UnstructuredMesh,
    fields: &FieldSet,
) -> Result<()> {
    let file = File::create(path)?;
    let mut w = BufWriter::new(file);

    // Header
    writeln!(w, "# vtk DataFile Version 3.0")?;
    writeln!(w, "GFD Solver Output")?;
    writeln!(w, "ASCII")?;
    writeln!(w, "DATASET UNSTRUCTURED_GRID")?;

    // POINTS
    let num_nodes = mesh.num_nodes();
    writeln!(w)?;
    writeln!(w, "POINTS {} double", num_nodes)?;
    for node in &mesh.nodes {
        writeln!(w, "{} {} {}", node.position[0], node.position[1], node.position[2])?;
    }

    // CELLS
    let num_cells = mesh.num_cells();
    let total_size: usize = mesh.cells.iter().map(|c| 1 + c.nodes.len()).sum();
    writeln!(w)?;
    writeln!(w, "CELLS {} {}", num_cells, total_size)?;
    for cell in &mesh.cells {
        let mut line = format!("{}", cell.nodes.len());
        for &nid in &cell.nodes {
            line.push_str(&format!(" {}", nid));
        }
        writeln!(w, "{}", line)?;
    }

    // CELL_TYPES
    writeln!(w)?;
    writeln!(w, "CELL_TYPES {}", num_cells)?;
    for cell in &mesh.cells {
        writeln!(w, "{}", vtk_cell_type(cell.cell_type()))?;
    }

    // CELL_DATA
    if !fields.is_empty() {
        writeln!(w)?;
        writeln!(w, "CELL_DATA {}", num_cells)?;

        // Sort keys for deterministic output
        let mut keys: Vec<&String> = fields.keys().collect();
        keys.sort();

        for key in keys {
            let field = &fields[key];
            match field {
                FieldData::Scalar(sf) => {
                    writeln!(w, "SCALARS {} double 1", sf.name())?;
                    writeln!(w, "LOOKUP_TABLE default")?;
                    for val in sf.values() {
                        writeln!(w, "{}", val)?;
                    }
                }
                FieldData::Vector(vf) => {
                    writeln!(w, "VECTORS {} double", vf.name())?;
                    for v in vf.values() {
                        writeln!(w, "{} {} {}", v[0], v[1], v[2])?;
                    }
                }
                FieldData::Tensor(_tf) => {
                    // Tensor fields are not yet supported in VTK legacy output;
                    // skip silently.
                }
            }
        }
    }

    w.flush()?;
    Ok(())
}

/// Writes mesh and field data in VTK XML format (.vtu).
///
/// Currently delegates to `write_vtk` as a convenience wrapper.
pub fn write_vtu(
    path: &str,
    mesh: &UnstructuredMesh,
    fields: &FieldSet,
) -> Result<()> {
    // Replace .vtu extension with .vtk if present, otherwise just use as-is.
    let vtk_path = if path.ends_with(".vtu") {
        format!("{}.vtk", &path[..path.len() - 4])
    } else {
        path.to_string()
    };
    write_vtk(&vtk_path, mesh, fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use gfd_core::{FieldData, ScalarField, StructuredMesh};

    #[test]
    fn test_write_vtk_2x2x1() {
        // 1. Create a 2x2x1 mesh
        let smesh = StructuredMesh::uniform(2, 2, 1, 2.0, 2.0, 1.0);
        let mesh = smesh.to_unstructured();

        // 2. Create a ScalarField "temperature" with 4 values (one per cell)
        let temperature = ScalarField::new("temperature", vec![100.0, 200.0, 300.0, 400.0]);

        // 3. Build the FieldSet
        let mut fields: FieldSet = HashMap::new();
        fields.insert("temperature".to_string(), FieldData::Scalar(temperature));

        // 4. Write to a temp file
        let dir = std::env::temp_dir();
        let path = dir.join("gfd_test_output.vtk");
        let path_str = path.to_str().unwrap();

        write_vtk(path_str, &mesh, &fields).expect("write_vtk should succeed");

        // 5. Read the file back and verify contents
        let contents = std::fs::read_to_string(&path).expect("should read the vtk file");

        assert!(
            contents.starts_with("# vtk DataFile Version"),
            "File should start with VTK header"
        );

        // 6. Verify POINTS 18 (for 2x2x1: (2+1)*(2+1)*(1+1) = 18 nodes)
        assert!(
            contents.contains("POINTS 18 double"),
            "Should contain POINTS 18; got:\n{}",
            &contents[..contents.len().min(500)]
        );

        // 7. Verify CELLS 4
        assert!(
            contents.contains("CELLS 4 "),
            "Should contain CELLS 4"
        );

        // 8. Verify SCALARS temperature
        assert!(
            contents.contains("SCALARS temperature double 1"),
            "Should contain SCALARS temperature"
        );

        // Clean up
        let _ = std::fs::remove_file(&path);
    }
}
