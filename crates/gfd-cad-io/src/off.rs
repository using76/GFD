//! Object File Format (Geomview OFF) — the simplest possible mesh
//! container. Widely accepted by academic mesh tools. Reader + writer.

use std::fs;
use std::io::Write;
use std::path::Path;

use gfd_cad_tessel::TriMesh;

use crate::{IoError, IoResult};

pub fn write_off(path: &Path, mesh: &TriMesh) -> IoResult<()> {
    let mut f = fs::File::create(path)?;
    let v = mesh.positions.len();
    let tri_count = mesh.indices.len() / 3;
    writeln!(f, "OFF")?;
    writeln!(f, "{} {} 0", v, tri_count)?;
    for p in &mesh.positions {
        writeln!(f, "{:.6} {:.6} {:.6}", p[0], p[1], p[2])?;
    }
    for t in 0..tri_count {
        let i0 = mesh.indices[t * 3];
        let i1 = mesh.indices[t * 3 + 1];
        let i2 = mesh.indices[t * 3 + 2];
        writeln!(f, "3 {} {} {}", i0, i1, i2)?;
    }
    Ok(())
}

/// Parses a plain OFF file. Expects a leading `OFF` magic (optional for
/// "NOFF"/"COFF" variants which we silently accept), then `nV nF nE`,
/// then `nV` vertex lines of 3 floats, then `nF` face lines of
/// `count idx idx … [color]`. Polygonal faces are fan-triangulated.
/// Skips blank lines and `#` comments.
pub fn read_off(path: &Path) -> IoResult<TriMesh> {
    let text = fs::read_to_string(path)?;
    let mut tokens = text
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .flat_map(|l| l.split_whitespace())
        .peekable();
    let magic = tokens.next().ok_or_else(|| IoError::Parse("empty off".into()))?;
    let accepted = ["OFF", "COFF", "NOFF", "NCOFF", "STOFF", "CNOFF"];
    if !accepted.contains(&magic) {
        return Err(IoError::Parse(format!("not an OFF file (magic '{}')", magic)));
    }
    let nv: usize = tokens.next().and_then(|s| s.parse().ok())
        .ok_or_else(|| IoError::Parse("off: missing vertex count".into()))?;
    let nf: usize = tokens.next().and_then(|s| s.parse().ok())
        .ok_or_else(|| IoError::Parse("off: missing face count".into()))?;
    let _ne: usize = tokens.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(nv);
    for _ in 0..nv {
        let x: f32 = tokens.next().and_then(|s| s.parse().ok())
            .ok_or_else(|| IoError::Parse("off: bad vx".into()))?;
        let y: f32 = tokens.next().and_then(|s| s.parse().ok())
            .ok_or_else(|| IoError::Parse("off: bad vy".into()))?;
        let z: f32 = tokens.next().and_then(|s| s.parse().ok())
            .ok_or_else(|| IoError::Parse("off: bad vz".into()))?;
        positions.push([x, y, z]);
    }
    let mut indices: Vec<u32> = Vec::with_capacity(nf * 3);
    for _ in 0..nf {
        let cnt: usize = tokens.next().and_then(|s| s.parse().ok())
            .ok_or_else(|| IoError::Parse("off: missing face size".into()))?;
        let mut ring = Vec::with_capacity(cnt);
        for _ in 0..cnt {
            let idx: u32 = tokens.next().and_then(|s| s.parse().ok())
                .ok_or_else(|| IoError::Parse("off: bad face idx".into()))?;
            ring.push(idx);
        }
        if cnt < 3 { continue; }
        for k in 1..cnt - 1 {
            indices.push(ring[0]);
            indices.push(ring[k]);
            indices.push(ring[k + 1]);
        }
    }
    Ok(TriMesh { positions, normals: vec![], indices })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_write_header_and_triangle() {
        let mesh = TriMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![],
            indices: vec![0, 1, 2],
        };
        let path = std::env::temp_dir().join(format!("gfd_off_test_{}.off",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        write_off(&path, &mesh).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("OFF"));
        assert!(text.contains("3 1 0")); // 3 vertices, 1 face, 0 edges
        assert!(text.contains("3 0 1 2"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn off_read_roundtrip() {
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
            "gfd_off_roundtrip_{}.off",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        write_off(&path, &mesh).unwrap();
        let loaded = read_off(&path).unwrap();
        assert_eq!(loaded.positions.len(), 4);
        assert_eq!(loaded.indices, vec![0, 1, 2, 0, 2, 3]);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn off_fan_triangulates_quad() {
        let src = "OFF\n# comment\n4 1 0\n0 0 0\n1 0 0\n1 1 0\n0 1 0\n4 0 1 2 3\n";
        let path = std::env::temp_dir().join(format!(
            "gfd_off_quad_{}.off",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, src).unwrap();
        let mesh = read_off(&path).unwrap();
        assert_eq!(mesh.positions.len(), 4);
        assert_eq!(mesh.indices, vec![0, 1, 2, 0, 2, 3]);
        let _ = fs::remove_file(&path);
    }
}
