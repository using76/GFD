//! AutoCAD DXF R12 writer (ASCII flavour, 3DFACE entities).
//! Minimal but broadly compatible — LibreCAD, BricsCAD, Fusion 360 all
//! open it. Useful for handing CAD meshes to legacy 2D/3D workflows.

use std::fs;
use std::io::Write;
use std::path::Path;

use gfd_cad_tessel::TriMesh;

use crate::IoResult;

pub fn write_dxf_3dface(path: &Path, mesh: &TriMesh) -> IoResult<()> {
    let mut f = fs::File::create(path)?;
    writeln!(f, "0\nSECTION\n2\nENTITIES")?;
    let tri = mesh.indices.len() / 3;
    for t in 0..tri {
        let a = mesh.positions[mesh.indices[t * 3] as usize];
        let b = mesh.positions[mesh.indices[t * 3 + 1] as usize];
        let c = mesh.positions[mesh.indices[t * 3 + 2] as usize];
        // 3DFACE has 4 corners; repeat the last vertex for a triangle.
        writeln!(f, "0\n3DFACE\n8\n0")?; // layer 0
        writeln!(f, "10\n{:.6}\n20\n{:.6}\n30\n{:.6}", a[0], a[1], a[2])?;
        writeln!(f, "11\n{:.6}\n21\n{:.6}\n31\n{:.6}", b[0], b[1], b[2])?;
        writeln!(f, "12\n{:.6}\n22\n{:.6}\n32\n{:.6}", c[0], c[1], c[2])?;
        writeln!(f, "13\n{:.6}\n23\n{:.6}\n33\n{:.6}", c[0], c[1], c[2])?;
    }
    writeln!(f, "0\nENDSEC\n0\nEOF")?;
    Ok(())
}
