//! Ergonomic helpers that construct valid multi-level shapes on an arena.

use gfd_cad_geom::{curve::Line, Point3};

use crate::{
    shape::{CurveGeom, Shape},
    Orientation, ShapeArena, ShapeId,
};

/// Build a straight edge between two points, registering the two endpoint
/// vertices and the edge itself. Returns the edge's `ShapeId`.
pub fn make_line_edge(arena: &mut ShapeArena, a: Point3, b: Point3) -> crate::TopoResult<ShapeId> {
    let v0 = arena.push(Shape::vertex(a));
    let v1 = arena.push(Shape::vertex(b));
    let line = Line::from_points(a, b)?;
    Ok(arena.push(Shape::Edge {
        curve: CurveGeom::Line(line),
        vertices: [v0, v1],
        orient: Orientation::Forward,
    }))
}

/// Build a closed wire from a list of edge ids (with orientations).
pub fn make_wire(arena: &mut ShapeArena, edges: Vec<(ShapeId, Orientation)>) -> ShapeId {
    arena.push(Shape::Wire { edges })
}

/// Wrap a single face (surface + outer wire) into a shell, then into a solid.
pub fn make_solid_from_face(arena: &mut ShapeArena, face: ShapeId) -> ShapeId {
    let shell = arena.push(Shape::Shell {
        faces: vec![(face, Orientation::Forward)],
    });
    arena.push(Shape::Solid { shells: vec![shell] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_edge_creates_three_shapes() {
        let mut arena = ShapeArena::new();
        let before = arena.len();
        let _ = make_line_edge(&mut arena, Point3::ORIGIN, Point3::new(1.0, 0.0, 0.0)).unwrap();
        assert_eq!(arena.len() - before, 3); // 2 vertices + 1 edge
    }
}
