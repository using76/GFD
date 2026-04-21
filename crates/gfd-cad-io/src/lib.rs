//! gfd-cad-io — file-format bridge for STEP / IGES / STL / BRep.
//!
//! Iteration 5: STL (ASCII + binary) reader online. STEP AP214 parsing and
//! IGES ship in later iterations.

use std::path::Path;

use gfd_cad_topo::{ShapeArena, ShapeId};

pub mod brep;
pub mod obj;
pub mod off;
pub mod ply;
pub mod step;
pub mod stl;
pub mod dxf;
pub mod vtk;
pub mod wrl;
pub mod xyz;

pub use brep::{read_brep, write_brep, BrepJson};
pub use obj::{read_obj, write_obj};
pub use off::{read_off, write_off};
pub use ply::{read_ply_ascii, write_ply_ascii};
pub use step::{read_step_points, summarise_step, write_step, StepSummary};
pub use stl::{read_stl, write_stl_ascii, write_stl_binary, StlMesh};
pub use dxf::write_dxf_3dface;
pub use vtk::write_vtk_polydata;
pub use wrl::write_wrl;
pub use xyz::{read_xyz, write_xyz};

#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parser: {0}")]
    Parse(String),
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
}

pub type IoResult<T> = Result<T, IoError>;

/// STEP import: reads `CARTESIAN_POINT` entries only (no topology). Returns
/// a Compound of Vertex shapes so downstream code has something to render.
pub fn import_step(path: &Path, arena: &mut ShapeArena) -> IoResult<ShapeId> {
    use gfd_cad_geom::Point3;
    use gfd_cad_topo::Shape;
    let pts = step::read_step_points(path)?;
    if pts.is_empty() {
        return Err(IoError::Parse("no CARTESIAN_POINT entries found".into()));
    }
    let vertex_ids: Vec<ShapeId> = pts.iter()
        .map(|(x, y, z)| arena.push(Shape::vertex(Point3::new(*x, *y, *z))))
        .collect();
    let compound = arena.push(Shape::Compound { children: vertex_ids });
    Ok(compound)
}

pub fn export_step(path: &Path, arena: &ShapeArena, root: ShapeId) -> IoResult<()> {
    step::write_step(path, arena, root)
}

pub fn import_iges(_path: &Path, _arena: &mut ShapeArena) -> IoResult<ShapeId> {
    Err(IoError::Unsupported("IGES import not yet implemented"))
}

pub fn import_brep(path: &Path, arena: &mut ShapeArena) -> IoResult<ShapeId> {
    let loaded = brep::read_brep(path)?;
    *arena = loaded.arena;
    loaded.root.map(ShapeId).ok_or_else(|| IoError::Parse("brep file has no root".into()))
}
