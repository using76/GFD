//! gfd-cad-topo — B-Rep topology for the GFD CAD kernel.
//!
//! Shape hierarchy follows OCCT / ISO 10303-42 conventions:
//!   Compound ⊃ Solid ⊃ Shell ⊃ Face ⊃ Wire ⊃ Edge ⊃ Vertex
//!
//! Uses a shape arena with stable `ShapeId`s so feature re-execution can
//! preserve references even when downstream geometry changes.

use gfd_cad_geom::{GeomError, Point3};
use serde::{Deserialize, Serialize};

pub mod adjacency;
pub mod arena;
pub mod builder;
pub mod iter;
pub mod orientation;
pub mod shape;

pub use adjacency::{build_half_edges, EdgeFaceMap};
pub use arena::ShapeArena;
pub use iter::collect_by_kind;
pub use orientation::Orientation;
pub use shape::{CurveGeom, HalfEdge, Shape, ShapeKind, SurfaceGeom};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ShapeId(pub u32);

impl ShapeId {
    pub const INVALID: Self = Self(u32::MAX);
}

#[derive(Debug, thiserror::Error)]
pub enum TopoError {
    #[error("invalid shape id {0:?}")]
    InvalidId(ShapeId),
    #[error("expected {expected:?}, got {actual:?}")]
    KindMismatch { expected: ShapeKind, actual: ShapeKind },
    #[error(transparent)]
    Geom(#[from] GeomError),
}

pub type TopoResult<T> = Result<T, TopoError>;

/// Convenience: build a vertex shape directly on an arena.
pub fn make_vertex(arena: &mut ShapeArena, p: Point3) -> ShapeId {
    arena.push(Shape::vertex(p))
}
