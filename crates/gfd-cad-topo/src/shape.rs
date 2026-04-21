use gfd_cad_geom::{
    curve::BSplineCurve, curve::Circle, curve::Line, surface::Cone, surface::Cylinder,
    surface::Plane, surface::Sphere, surface::Torus, Point3,
};
use serde::{Deserialize, Serialize};

use crate::{Orientation, ShapeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Vertex,
    Edge,
    Wire,
    Face,
    Shell,
    Solid,
    Compound,
}

/// Geometry attached to an edge. Tagged enum for now — later iterations may
/// hoist curves into their own arena for cross-shape sharing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CurveGeom {
    Line(Line),
    Circle(Circle),
    BSpline(BSplineCurve),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SurfaceGeom {
    Plane(Plane),
    Cylinder(Cylinder),
    Sphere(Sphere),
    Cone(Cone),
    Torus(Torus),
}

/// Vertex ↔ edge connectivity. `next`/`prev` walk around a face wire; `twin`
/// is the mate on the opposite face (or `None` for boundary edges).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HalfEdge {
    pub origin: ShapeId,      // vertex at the tail
    pub edge: ShapeId,        // underlying geometric edge
    pub twin: Option<u32>,    // index into ShapeArena half_edges
    pub next: u32,
    pub prev: u32,
    pub face: Option<ShapeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Shape {
    Vertex { point: Point3 },
    Edge { curve: CurveGeom, vertices: [ShapeId; 2], orient: Orientation },
    Wire { edges: Vec<(ShapeId, Orientation)> },
    Face { surface: SurfaceGeom, wires: Vec<ShapeId>, orient: Orientation },
    Shell { faces: Vec<(ShapeId, Orientation)> },
    Solid { shells: Vec<ShapeId> },
    Compound { children: Vec<ShapeId> },
}

impl Shape {
    pub fn kind(&self) -> ShapeKind {
        match self {
            Self::Vertex { .. } => ShapeKind::Vertex,
            Self::Edge { .. } => ShapeKind::Edge,
            Self::Wire { .. } => ShapeKind::Wire,
            Self::Face { .. } => ShapeKind::Face,
            Self::Shell { .. } => ShapeKind::Shell,
            Self::Solid { .. } => ShapeKind::Solid,
            Self::Compound { .. } => ShapeKind::Compound,
        }
    }

    pub fn vertex(p: Point3) -> Self { Self::Vertex { point: p } }
}
