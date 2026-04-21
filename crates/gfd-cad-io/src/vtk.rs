//! VTK Legacy PolyData writer. ParaView / VisIt / VTK toolkits all read
//! this format out of the box, so it's the most reliable bridge between
//! CAD meshes and simulation / visualisation pipelines.

use std::fs;
use std::io::Write;
use std::path::Path;

use gfd_cad_tessel::TriMesh;

use crate::IoResult;

pub fn write_vtk_polydata(path: &Path, mesh: &TriMesh, title: &str) -> IoResult<()> {
    let mut f = fs::File::create(path)?;
    writeln!(f, "# vtk DataFile Version 3.0")?;
    writeln!(f, "{}", title)?;
    writeln!(f, "ASCII")?;
    writeln!(f, "DATASET POLYDATA")?;
    writeln!(f, "POINTS {} float", mesh.positions.len())?;
    for p in &mesh.positions {
        writeln!(f, "{:.6} {:.6} {:.6}", p[0], p[1], p[2])?;
    }
    let tri = mesh.indices.len() / 3;
    // Each polygon is preceded by its vertex count → 4 integers per triangle.
    writeln!(f, "POLYGONS {} {}", tri, tri * 4)?;
    for t in 0..tri {
        let i0 = mesh.indices[t * 3];
        let i1 = mesh.indices[t * 3 + 1];
        let i2 = mesh.indices[t * 3 + 2];
        writeln!(f, "3 {} {} {}", i0, i1, i2)?;
    }
    Ok(())
}
