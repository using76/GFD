//! Wavefront OBJ reader + writer. Writes positions, normals (optional), and
//! triangle faces. ASCII text format; widely supported by mesh viewers.

use std::fs;
use std::io::Write;
use std::path::Path;

use gfd_cad_tessel::TriMesh;

use crate::{IoError, IoResult};

pub fn write_obj(path: &Path, mesh: &TriMesh, name: &str) -> IoResult<()> {
    let mut f = fs::File::create(path)?;
    writeln!(f, "# gfd-cad-io export")?;
    writeln!(f, "o {}", name)?;
    for p in &mesh.positions {
        writeln!(f, "v {:.6} {:.6} {:.6}", p[0], p[1], p[2])?;
    }
    for n in &mesh.normals {
        writeln!(f, "vn {:.6} {:.6} {:.6}", n[0], n[1], n[2])?;
    }
    let tri_count = mesh.indices.len() / 3;
    let has_normals = mesh.normals.len() == mesh.positions.len();
    for t in 0..tri_count {
        let i0 = mesh.indices[t * 3] as usize + 1;       // OBJ is 1-indexed
        let i1 = mesh.indices[t * 3 + 1] as usize + 1;
        let i2 = mesh.indices[t * 3 + 2] as usize + 1;
        if has_normals {
            writeln!(f, "f {}//{}  {}//{}  {}//{}", i0, i0, i1, i1, i2, i2)?;
        } else {
            writeln!(f, "f {} {} {}", i0, i1, i2)?;
        }
    }
    let _ = IoError::Io;
    Ok(())
}

/// Reads a Wavefront OBJ, returning a `TriMesh` with positions and
/// triangle-fan indices. Ignores textures / normals / material libraries.
/// Face tokens like `1/2/3` → keeps the position index. Polygons are
/// fan-triangulated. Negative (relative) indices are resolved.
pub fn read_obj(path: &Path) -> IoResult<TriMesh> {
    let text = fs::read_to_string(path)?;
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') { continue; }
        let mut it = t.split_whitespace();
        let tag = it.next().unwrap_or("");
        match tag {
            "v" => {
                let x: f32 = it.next().and_then(|s| s.parse().ok())
                    .ok_or_else(|| IoError::Parse("obj: bad v.x".into()))?;
                let y: f32 = it.next().and_then(|s| s.parse().ok())
                    .ok_or_else(|| IoError::Parse("obj: bad v.y".into()))?;
                let z: f32 = it.next().and_then(|s| s.parse().ok())
                    .ok_or_else(|| IoError::Parse("obj: bad v.z".into()))?;
                positions.push([x, y, z]);
            }
            "f" => {
                let toks: Vec<&str> = it.collect();
                if toks.len() < 3 { continue; }
                let resolve = |s: &str| -> IoResult<u32> {
                    let pos = s.split('/').next().unwrap_or("");
                    let n: i64 = pos.parse()
                        .map_err(|_| IoError::Parse(format!("obj: bad face idx '{}'", s)))?;
                    let idx = if n > 0 {
                        (n - 1) as i64
                    } else {
                        positions.len() as i64 + n
                    };
                    if idx < 0 || idx >= positions.len() as i64 {
                        return Err(IoError::Parse(format!("obj: face idx out of range {}", n)));
                    }
                    Ok(idx as u32)
                };
                let mut ring: Vec<u32> = Vec::with_capacity(toks.len());
                for t in toks { ring.push(resolve(t)?); }
                for k in 1..ring.len() - 1 {
                    indices.push(ring[0]);
                    indices.push(ring[k]);
                    indices.push(ring[k + 1]);
                }
            }
            _ => { /* skip vt, vn, vp, o, g, s, mtllib, usemtl, etc. */ }
        }
    }
    Ok(TriMesh { positions, normals: vec![], indices })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_write_roundtrip_content() {
        let mesh = TriMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            indices: vec![0, 1, 2],
        };
        let path = std::env::temp_dir().join(format!("gfd_obj_test_{}.obj",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        write_obj(&path, &mesh, "tri").unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("v 0.000000 0.000000 0.000000"));
        assert!(text.contains("vn 0.000000 0.000000 1.000000"));
        assert!(text.contains("f 1//1  2//2  3//3"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn obj_read_simple_triangle() {
        let src = "# test\no tri\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        let path = std::env::temp_dir().join(format!(
            "gfd_obj_read_{}.obj",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, src).unwrap();
        let mesh = read_obj(&path).unwrap();
        assert_eq!(mesh.positions.len(), 3);
        assert_eq!(mesh.indices, vec![0, 1, 2]);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn obj_read_quad_fan_and_slashes() {
        let src = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1/1/1 2/2/2 3/3/3 4/4/4\n";
        let path = std::env::temp_dir().join(format!(
            "gfd_obj_quad_{}.obj",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, src).unwrap();
        let mesh = read_obj(&path).unwrap();
        assert_eq!(mesh.positions.len(), 4);
        assert_eq!(mesh.indices, vec![0, 1, 2, 0, 2, 3]);
        let _ = fs::remove_file(&path);
    }
}
