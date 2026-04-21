//! VRML 2.0 (WRL) writer — IndexedFaceSet in a single Shape node.

use std::fs;
use std::io::Write;
use std::path::Path;

use gfd_cad_tessel::TriMesh;

use crate::IoResult;

pub fn write_wrl(path: &Path, mesh: &TriMesh) -> IoResult<()> {
    let mut f = fs::File::create(path)?;
    writeln!(f, "#VRML V2.0 utf8")?;
    writeln!(f, "# gfd-cad-io export")?;
    writeln!(f, "Shape {{")?;
    writeln!(f, "  appearance Appearance {{")?;
    writeln!(f, "    material Material {{ diffuseColor 0.8 0.8 0.8 }}")?;
    writeln!(f, "  }}")?;
    writeln!(f, "  geometry IndexedFaceSet {{")?;
    writeln!(f, "    solid TRUE")?;
    writeln!(f, "    coord Coordinate {{")?;
    writeln!(f, "      point [")?;
    for (i, p) in mesh.positions.iter().enumerate() {
        let sep = if i + 1 == mesh.positions.len() { "" } else { "," };
        writeln!(f, "        {:.6} {:.6} {:.6}{}", p[0], p[1], p[2], sep)?;
    }
    writeln!(f, "      ]")?;
    writeln!(f, "    }}")?;
    writeln!(f, "    coordIndex [")?;
    let tri = mesh.indices.len() / 3;
    for t in 0..tri {
        let i0 = mesh.indices[t * 3];
        let i1 = mesh.indices[t * 3 + 1];
        let i2 = mesh.indices[t * 3 + 2];
        let sep = if t + 1 == tri { "" } else { "," };
        writeln!(f, "      {}, {}, {}, -1{}", i0, i1, i2, sep)?;
    }
    writeln!(f, "    ]")?;
    writeln!(f, "  }}")?;
    writeln!(f, "}}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrl_write_has_header_and_face() {
        let mesh = TriMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![],
            indices: vec![0, 1, 2],
        };
        let path = std::env::temp_dir().join(format!(
            "gfd_wrl_test_{}.wrl",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        write_wrl(&path, &mesh).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("#VRML V2.0 utf8\n"));
        assert!(text.contains("IndexedFaceSet"));
        assert!(text.contains("0, 1, 2, -1"));
        let _ = fs::remove_file(&path);
    }
}
