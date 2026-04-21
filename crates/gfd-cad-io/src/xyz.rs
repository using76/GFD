//! XYZ point-cloud writer/reader. Plain ASCII, one vertex per line:
//! `x y z`. Optional `r g b` triples ignored on read; not emitted on write.
//! Widely consumed by LiDAR tools and scanner pipelines.

use std::fs;
use std::io::Write;
use std::path::Path;

use gfd_cad_tessel::TriMesh;

use crate::{IoError, IoResult};

pub fn write_xyz(path: &Path, mesh: &TriMesh) -> IoResult<()> {
    let mut f = fs::File::create(path)?;
    for p in &mesh.positions {
        writeln!(f, "{:.6} {:.6} {:.6}", p[0], p[1], p[2])?;
    }
    Ok(())
}

pub fn read_xyz(path: &Path) -> IoResult<TriMesh> {
    let text = fs::read_to_string(path)?;
    let mut positions: Vec<[f32; 3]> = Vec::new();
    for (lineno, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') { continue; }
        let mut it = t.split_whitespace();
        let x: f32 = it.next().and_then(|s| s.parse().ok())
            .ok_or_else(|| IoError::Parse(format!("xyz: bad x on line {}", lineno + 1)))?;
        let y: f32 = it.next().and_then(|s| s.parse().ok())
            .ok_or_else(|| IoError::Parse(format!("xyz: bad y on line {}", lineno + 1)))?;
        let z: f32 = it.next().and_then(|s| s.parse().ok())
            .ok_or_else(|| IoError::Parse(format!("xyz: bad z on line {}", lineno + 1)))?;
        positions.push([x, y, z]);
    }
    Ok(TriMesh { positions, normals: vec![], indices: vec![] })
}
