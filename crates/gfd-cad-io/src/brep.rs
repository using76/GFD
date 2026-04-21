//! Minimal BRep-JSON format.
//!
//! Iteration 8 roundtrip: serialize a `ShapeArena` (and optional root id)
//! to pretty JSON, and parse it back. This is intentionally internal — it
//! is NOT OCCT's BRep text format, but it gives us a stable, reviewable
//! dump for debugging and CI snapshot tests until STEP AP214 lands.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use gfd_cad_topo::{ShapeArena, ShapeId};

use crate::{IoError, IoResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct BrepJson {
    pub version: u32,
    pub root: Option<u32>,
    pub arena: ShapeArena,
}

impl BrepJson {
    pub fn new(arena: ShapeArena, root: Option<ShapeId>) -> Self {
        Self { version: 1, root: root.map(|r| r.0), arena }
    }
}

pub fn write_brep(path: &Path, arena: &ShapeArena, root: Option<ShapeId>) -> IoResult<()> {
    let payload = BrepJson {
        version: 1,
        root: root.map(|r| r.0),
        arena: arena.clone(),
    };
    let text = serde_json::to_string_pretty(&payload)
        .map_err(|e| IoError::Parse(format!("brep serialize: {}", e)))?;
    fs::write(path, text)?;
    Ok(())
}

pub fn read_brep(path: &Path) -> IoResult<BrepJson> {
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|e| IoError::Parse(format!("brep parse: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_cad_geom::Point3;
    use gfd_cad_topo::Shape;

    #[test]
    fn roundtrip_vertex_arena() {
        let mut arena = ShapeArena::new();
        let v = arena.push(Shape::vertex(Point3::new(1.0, 2.0, 3.0)));
        let path = std::env::temp_dir().join(format!("gfd_brep_test_{}.json",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        write_brep(&path, &arena, Some(v)).unwrap();
        let back = read_brep(&path).unwrap();
        assert_eq!(back.version, 1);
        assert_eq!(back.root, Some(v.0));
        assert_eq!(back.arena.len(), 1);
        let _ = fs::remove_file(&path);
    }
}
