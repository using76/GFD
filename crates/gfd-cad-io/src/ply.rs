//! Stanford PLY (Polygon File Format) ASCII reader + writer.

use std::fs;
use std::io::Write;
use std::path::Path;

use gfd_cad_tessel::TriMesh;

use crate::{IoError, IoResult};

pub fn write_ply_ascii(path: &Path, mesh: &TriMesh) -> IoResult<()> {
    let mut f = fs::File::create(path)?;
    let v = mesh.positions.len();
    let tri = mesh.indices.len() / 3;
    writeln!(f, "ply")?;
    writeln!(f, "format ascii 1.0")?;
    writeln!(f, "comment gfd-cad-io export")?;
    writeln!(f, "element vertex {}", v)?;
    writeln!(f, "property float x")?;
    writeln!(f, "property float y")?;
    writeln!(f, "property float z")?;
    writeln!(f, "element face {}", tri)?;
    writeln!(f, "property list uchar uint vertex_indices")?;
    writeln!(f, "end_header")?;
    for p in &mesh.positions {
        writeln!(f, "{:.6} {:.6} {:.6}", p[0], p[1], p[2])?;
    }
    for t in 0..tri {
        let i0 = mesh.indices[t * 3];
        let i1 = mesh.indices[t * 3 + 1];
        let i2 = mesh.indices[t * 3 + 2];
        writeln!(f, "3 {} {} {}", i0, i1, i2)?;
    }
    Ok(())
}

/// ASCII PLY reader. Supports the common `float x/y/z` vertex layout and
/// `list uchar uint vertex_indices` face layout. Non-triangle faces are
/// fan-triangulated. Ignores extra properties silently.
pub fn read_ply_ascii(path: &Path) -> IoResult<TriMesh> {
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines();
    let magic = lines.next().ok_or_else(|| IoError::Parse("empty ply".into()))?;
    if magic.trim() != "ply" {
        return Err(IoError::Parse("not a PLY file (missing 'ply' magic)".into()));
    }
    let fmt = lines.next().ok_or_else(|| IoError::Parse("missing format line".into()))?;
    if !fmt.trim_start().starts_with("format ascii") {
        return Err(IoError::Unsupported("only ascii PLY is supported"));
    }
    let mut n_vert: usize = 0;
    let mut n_face: usize = 0;
    let mut vert_props = 0usize;
    let mut in_vert = false;
    for line in lines.by_ref() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("comment") || t.starts_with("obj_info") { continue; }
        if let Some(rest) = t.strip_prefix("element ") {
            let mut it = rest.split_whitespace();
            let kind = it.next().unwrap_or("");
            let count: usize = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            match kind {
                "vertex" => { n_vert = count; in_vert = true; }
                "face"   => { n_face = count; in_vert = false; }
                _        => { in_vert = false; }
            }
        } else if t.starts_with("property ") {
            if in_vert { vert_props += 1; }
        } else if t == "end_header" {
            break;
        }
    }
    if vert_props < 3 {
        return Err(IoError::Parse("vertex needs at least x/y/z".into()));
    }
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n_vert);
    for _ in 0..n_vert {
        let line = lines.next().ok_or_else(|| IoError::Parse("unexpected eof in verts".into()))?;
        let mut it = line.split_whitespace();
        let x: f32 = it.next().and_then(|s| s.parse().ok()).ok_or_else(|| IoError::Parse("bad x".into()))?;
        let y: f32 = it.next().and_then(|s| s.parse().ok()).ok_or_else(|| IoError::Parse("bad y".into()))?;
        let z: f32 = it.next().and_then(|s| s.parse().ok()).ok_or_else(|| IoError::Parse("bad z".into()))?;
        positions.push([x, y, z]);
    }
    let mut indices: Vec<u32> = Vec::with_capacity(n_face * 3);
    for _ in 0..n_face {
        let line = lines.next().ok_or_else(|| IoError::Parse("unexpected eof in faces".into()))?;
        let mut it = line.split_whitespace();
        let n: usize = it.next().and_then(|s| s.parse().ok()).ok_or_else(|| IoError::Parse("bad face count".into()))?;
        let verts: Vec<u32> = (0..n)
            .map(|_| it.next().and_then(|s| s.parse().ok()).ok_or_else(|| IoError::Parse("bad idx".into())))
            .collect::<IoResult<Vec<u32>>>()?;
        if n < 3 { continue; }
        for k in 1..n - 1 {
            indices.push(verts[0]);
            indices.push(verts[k]);
            indices.push(verts[k + 1]);
        }
    }
    Ok(TriMesh { positions, normals: vec![], indices })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ply_write_header_and_face() {
        let mesh = TriMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![],
            indices: vec![0, 1, 2],
        };
        let path = std::env::temp_dir().join(format!("gfd_ply_test_{}.ply",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        write_ply_ascii(&path, &mesh).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("ply\n"));
        assert!(text.contains("element vertex 3"));
        assert!(text.contains("element face 1"));
        assert!(text.contains("3 0 1 2"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn ply_read_roundtrip() {
        let mesh = TriMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            normals: vec![],
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        let path = std::env::temp_dir().join(format!(
            "gfd_ply_roundtrip_{}.ply",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        write_ply_ascii(&path, &mesh).unwrap();
        let loaded = read_ply_ascii(&path).unwrap();
        assert_eq!(loaded.positions.len(), 4);
        assert_eq!(loaded.indices.len(), 6);
        assert_eq!(loaded.indices, vec![0, 1, 2, 0, 2, 3]);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn ply_fan_triangulates_quad_face() {
        let src = "ply\nformat ascii 1.0\nelement vertex 4\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar uint vertex_indices\nend_header\n0 0 0\n1 0 0\n1 1 0\n0 1 0\n4 0 1 2 3\n";
        let path = std::env::temp_dir().join(format!(
            "gfd_ply_quad_{}.ply",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, src).unwrap();
        let loaded = read_ply_ascii(&path).unwrap();
        assert_eq!(loaded.positions.len(), 4);
        assert_eq!(loaded.indices, vec![0, 1, 2, 0, 2, 3]);
        let _ = fs::remove_file(&path);
    }
}
