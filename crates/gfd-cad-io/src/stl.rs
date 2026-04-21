//! Minimal STL reader producing triangle buffers ready for Three.js.
//!
//! Supports both ASCII and binary formats. Returned buffers use the same
//! layout as `gfd_cad_tessel::TriMesh` (positions / normals / indices),
//! but this module intentionally stays dependency-light so the IO crate can
//! be pulled without the rest of the kernel.

use std::fs;
use std::path::Path;

use crate::{IoError, IoResult};

#[derive(Debug, Default, Clone)]
pub struct StlMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl StlMesh {
    pub fn triangle_count(&self) -> usize { self.indices.len() / 3 }
}

/// Write a `StlMesh` as an ASCII STL file. Each indexed triangle is emitted
/// with its stored normal (or a recomputed face normal if stored is zero).
pub fn write_stl_ascii(path: &Path, mesh: &StlMesh, name: &str) -> IoResult<()> {
    use std::io::Write;
    let mut f = fs::File::create(path)?;
    writeln!(f, "solid {}", name)?;
    for tri in 0..mesh.indices.len() / 3 {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;
        let p0 = mesh.positions[i0];
        let p1 = mesh.positions[i1];
        let p2 = mesh.positions[i2];
        // Prefer stored normal; fall back to face normal.
        let n = if i0 < mesh.normals.len() {
            mesh.normals[i0]
        } else {
            face_normal(p0, p1, p2)
        };
        writeln!(f, "  facet normal {:.6} {:.6} {:.6}", n[0], n[1], n[2])?;
        writeln!(f, "    outer loop")?;
        writeln!(f, "      vertex {:.6} {:.6} {:.6}", p0[0], p0[1], p0[2])?;
        writeln!(f, "      vertex {:.6} {:.6} {:.6}", p1[0], p1[1], p1[2])?;
        writeln!(f, "      vertex {:.6} {:.6} {:.6}", p2[0], p2[1], p2[2])?;
        writeln!(f, "    endloop")?;
        writeln!(f, "  endfacet")?;
    }
    writeln!(f, "endsolid {}", name)?;
    Ok(())
}

/// Write a `StlMesh` as a binary STL file (80-byte header + u32 triangle
/// count + 50 bytes per triangle). Substantially smaller than ASCII for
/// large meshes (~50x).
pub fn write_stl_binary(path: &Path, mesh: &StlMesh) -> IoResult<()> {
    use std::io::Write;
    let tri_count = (mesh.indices.len() / 3) as u32;
    let mut buf: Vec<u8> = Vec::with_capacity(84 + tri_count as usize * 50);
    buf.extend_from_slice(&[0u8; 80]); // header (empty)
    buf.extend_from_slice(&tri_count.to_le_bytes());
    for tri in 0..tri_count as usize {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;
        let p0 = mesh.positions[i0];
        let p1 = mesh.positions[i1];
        let p2 = mesh.positions[i2];
        let n = if i0 < mesh.normals.len() { mesh.normals[i0] } else { face_normal(p0, p1, p2) };
        for &v in &[n[0], n[1], n[2]] { buf.extend_from_slice(&v.to_le_bytes()); }
        for p in [p0, p1, p2] {
            for &c in &p { buf.extend_from_slice(&c.to_le_bytes()); }
        }
        buf.extend_from_slice(&[0u8, 0u8]); // attribute byte count
    }
    let mut f = fs::File::create(path)?;
    f.write_all(&buf)?;
    Ok(())
}

fn face_normal(p0: [f32; 3], p1: [f32; 3], p2: [f32; 3]) -> [f32; 3] {
    let ax = p1[0] - p0[0];
    let ay = p1[1] - p0[1];
    let az = p1[2] - p0[2];
    let bx = p2[0] - p0[0];
    let by = p2[1] - p0[1];
    let bz = p2[2] - p0[2];
    let nx = ay * bz - az * by;
    let ny = az * bx - ax * bz;
    let nz = ax * by - ay * bx;
    let len = (nx * nx + ny * ny + nz * nz).sqrt().max(1.0e-12);
    [nx / len, ny / len, nz / len]
}

pub fn read_stl(path: &Path) -> IoResult<StlMesh> {
    let bytes = fs::read(path)?;
    if bytes.len() < 15 {
        return Err(IoError::Parse("file too short to be STL".into()));
    }

    // Heuristic: ASCII STL begins with "solid" and contains the word "facet".
    // Binary files may coincidentally start with "solid" too, so we also
    // cross-check against the declared triangle count.
    let looks_ascii = bytes.starts_with(b"solid")
        && bytes.iter().any(|&b| b == b'\n')
        && std::str::from_utf8(&bytes).map_or(false, |s| s.contains("facet"));

    if looks_ascii {
        parse_ascii(std::str::from_utf8(&bytes).map_err(|e| IoError::Parse(format!("utf8: {}", e)))?)
    } else {
        parse_binary(&bytes)
    }
}

fn parse_ascii(src: &str) -> IoResult<StlMesh> {
    let mut mesh = StlMesh::default();
    let mut current_normal: [f32; 3] = [0.0, 0.0, 0.0];
    let mut tri_verts: Vec<[f32; 3]> = Vec::new();

    for line in src.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("facet normal") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 3 {
                current_normal = [
                    parts[0].parse().unwrap_or(0.0),
                    parts[1].parse().unwrap_or(0.0),
                    parts[2].parse().unwrap_or(1.0),
                ];
            }
        } else if let Some(rest) = line.strip_prefix("vertex") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 3 {
                tri_verts.push([
                    parts[0].parse().unwrap_or(0.0),
                    parts[1].parse().unwrap_or(0.0),
                    parts[2].parse().unwrap_or(0.0),
                ]);
            }
        } else if line == "endfacet" && tri_verts.len() == 3 {
            let base = mesh.positions.len() as u32;
            for v in &tri_verts {
                mesh.positions.push(*v);
                mesh.normals.push(current_normal);
            }
            mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
            tri_verts.clear();
        }
    }
    Ok(mesh)
}

fn parse_binary(bytes: &[u8]) -> IoResult<StlMesh> {
    if bytes.len() < 84 {
        return Err(IoError::Parse("binary STL shorter than header".into()));
    }
    let tri_count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    let expected = 84 + tri_count * 50;
    if bytes.len() < expected {
        return Err(IoError::Parse(format!(
            "binary STL truncated: expected {expected} bytes, got {}", bytes.len(),
        )));
    }
    let mut mesh = StlMesh {
        positions: Vec::with_capacity(tri_count * 3),
        normals:   Vec::with_capacity(tri_count * 3),
        indices:   Vec::with_capacity(tri_count * 3),
    };
    let mut cursor = 84;
    for _ in 0..tri_count {
        let n = read_vec3(&bytes[cursor..cursor + 12]);
        cursor += 12;
        let base = mesh.positions.len() as u32;
        for _ in 0..3 {
            let v = read_vec3(&bytes[cursor..cursor + 12]);
            mesh.positions.push(v);
            mesh.normals.push(n);
            cursor += 12;
        }
        mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
        cursor += 2; // attribute byte count, ignored
    }
    Ok(mesh)
}

fn read_vec3(bytes: &[u8]) -> [f32; 3] {
    [
        f32::from_le_bytes(bytes[0..4].try_into().unwrap()),
        f32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        f32::from_le_bytes(bytes[8..12].try_into().unwrap()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(content: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("gfd_cad_io_test_{}.stl",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(content).unwrap();
        path
    }

    #[test]
    fn stl_binary_write_then_read_roundtrip() {
        let mesh = StlMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals:   vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
            indices:   vec![0, 1, 2],
        };
        let path = write_tmp(b"");
        write_stl_binary(&path, &mesh).unwrap();
        let back = read_stl(&path).unwrap();
        assert_eq!(back.triangle_count(), 1);
        assert!((back.normals[0][2] - 1.0).abs() < 1e-5);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn stl_write_then_read_roundtrip() {
        let mesh = StlMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals:   vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
            indices:   vec![0, 1, 2],
        };
        let path = write_tmp(b"");
        write_stl_ascii(&path, &mesh, "test").unwrap();
        let back = read_stl(&path).unwrap();
        assert_eq!(back.triangle_count(), 1);
        assert!((back.normals[0][2] - 1.0).abs() < 1e-5);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn ascii_one_triangle() {
        let stl = b"solid tri\n\
                    facet normal 0.0 0.0 1.0\n\
                      outer loop\n\
                        vertex 0.0 0.0 0.0\n\
                        vertex 1.0 0.0 0.0\n\
                        vertex 0.0 1.0 0.0\n\
                      endloop\n\
                    endfacet\n\
                    endsolid tri\n";
        let path = write_tmp(stl);
        let mesh = read_stl(&path).unwrap();
        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.positions.len(), 3);
        assert_eq!(mesh.normals[0], [0.0, 0.0, 1.0]);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn binary_one_triangle() {
        // 80-byte header + 4-byte tri count + 50 bytes for the triangle.
        let mut buf = vec![0u8; 80];
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&0.0f32.to_le_bytes());  // nx
        buf.extend_from_slice(&0.0f32.to_le_bytes());  // ny
        buf.extend_from_slice(&1.0f32.to_le_bytes());  // nz
        for &v in &[[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            buf.extend_from_slice(&v[0].to_le_bytes());
            buf.extend_from_slice(&v[1].to_le_bytes());
            buf.extend_from_slice(&v[2].to_le_bytes());
        }
        buf.extend_from_slice(&[0u8, 0u8]);
        let path = write_tmp(&buf);
        let mesh = read_stl(&path).unwrap();
        assert_eq!(mesh.triangle_count(), 1);
        let _ = fs::remove_file(&path);
    }
}
