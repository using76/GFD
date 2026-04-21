//! gfd-cad-bool — Boolean operations.
//!
//! Iteration 6: `compound_merge` wraps multiple shapes into a `Shape::Compound`
//! so the GUI can treat two primitives as a single selectable group. A proper
//! B-Rep Boolean (SSI + face classification + stitching) lands in a later
//! iteration — see CAD_KERNEL_PLAN.md Phase 3.

use gfd_cad_topo::{Shape, ShapeArena, ShapeId, TopoError};

pub mod mesh;

pub use mesh::{mesh_boolean, point_inside_mesh, MeshOp};

#[derive(Debug, thiserror::Error)]
pub enum BoolError {
    #[error("boolean not yet implemented for this shape combination")]
    Unimplemented,
    #[error(transparent)]
    Topo(#[from] TopoError),
}

pub type BoolResult<T> = Result<T, BoolError>;

/// Wrap `ids` into a `Shape::Compound`, returning its id. This is the
/// "group" semantic — all children stay intact and re-tessellating the
/// compound includes every child.
pub fn compound_merge(arena: &mut ShapeArena, ids: &[ShapeId]) -> BoolResult<ShapeId> {
    Ok(arena.push(Shape::Compound { children: ids.to_vec() }))
}

/// Fast axis-aligned bounding box overlap test — useful as a pre-filter
/// before running expensive mesh CSG. Returns true if A's bbox touches B's
/// bbox at any point. Empty shapes return false.
pub fn bbox_overlaps(arena: &ShapeArena, a: ShapeId, b: ShapeId) -> BoolResult<bool> {
    use gfd_cad_geom::BoundingBox;
    fn walk(arena: &ShapeArena, id: ShapeId, bb: &mut BoundingBox) -> Result<(), TopoError> {
        match arena.get(id)? {
            Shape::Vertex { point } => { bb.expand(*point); }
            Shape::Edge { vertices, .. } => { for v in vertices { walk(arena, *v, bb)?; } }
            Shape::Wire { edges } => { for (e, _) in edges { walk(arena, *e, bb)?; } }
            Shape::Face { wires, .. } => { for w in wires { walk(arena, *w, bb)?; } }
            Shape::Shell { faces } => { for (f, _) in faces { walk(arena, *f, bb)?; } }
            Shape::Solid { shells } => { for s in shells { walk(arena, *s, bb)?; } }
            Shape::Compound { children } => { for c in children { walk(arena, *c, bb)?; } }
        }
        Ok(())
    }
    let mut bba = BoundingBox::EMPTY;
    let mut bbb = BoundingBox::EMPTY;
    walk(arena, a, &mut bba)?;
    walk(arena, b, &mut bbb)?;
    if bba.is_empty() || bbb.is_empty() { return Ok(false); }
    Ok(bba.min.x <= bbb.max.x && bba.max.x >= bbb.min.x
    && bba.min.y <= bbb.max.y && bba.max.y >= bbb.min.y
    && bba.min.z <= bbb.max.z && bba.max.z >= bbb.min.z)
}

pub fn union(arena: &mut ShapeArena, a: ShapeId, b: ShapeId) -> BoolResult<ShapeId> {
    // Iter 6: fall back to a compound. True B-Rep union ships later.
    compound_merge(arena, &[a, b])
}

pub fn difference(_arena: &mut ShapeArena, _a: ShapeId, _b: ShapeId) -> BoolResult<ShapeId> {
    Err(BoolError::Unimplemented)
}

pub fn intersection(_arena: &mut ShapeArena, _a: ShapeId, _b: ShapeId) -> BoolResult<ShapeId> {
    Err(BoolError::Unimplemented)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_cad_geom::Point3;
    use gfd_cad_topo::collect_by_kind;
    use gfd_cad_topo::ShapeKind;

    #[test]
    fn bbox_overlap_detects_separated_vs_touching_boxes() {
        use gfd_cad_geom::Point3;
        let mut arena = ShapeArena::new();
        let a = arena.push(Shape::vertex(Point3::new(0.0, 0.0, 0.0)));
        let b = arena.push(Shape::vertex(Point3::new(10.0, 10.0, 10.0)));
        // Single points — bboxes are degenerate but each contains the point;
        // they only overlap if the two points coincide, so far apart → false.
        assert!(!bbox_overlaps(&arena, a, b).unwrap());
        // Same point at origin — bboxes coincide → true.
        let c = arena.push(Shape::vertex(Point3::new(0.0, 0.0, 0.0)));
        assert!(bbox_overlaps(&arena, a, c).unwrap());
    }

    #[test]
    fn compound_groups_two_vertices() {
        let mut arena = ShapeArena::new();
        let a = arena.push(Shape::vertex(Point3::new(0.0, 0.0, 0.0)));
        let b = arena.push(Shape::vertex(Point3::new(1.0, 0.0, 0.0)));
        let c = compound_merge(&mut arena, &[a, b]).unwrap();
        let verts = collect_by_kind(&arena, c, ShapeKind::Vertex);
        assert_eq!(verts.len(), 2);
    }
}
