//! Pocket feature — extrude a 2D polygon **downward** by `depth` to produce
//! a "tool solid" representing material that would be removed from a target.
//!
//! Iteration 11 scope: geometric construction of the pocket solid only.
//! True CSG subtraction against a target shape lands with mesh-based boolean
//! in a later iteration; today's pocket is renderable and measurable but
//! does not yet mutate the target.

use crate::pad::pad_polygon_xy_signed;
use gfd_cad_topo::{ShapeArena, ShapeId, TopoResult};

pub fn pocket_polygon_xy(
    arena: &mut ShapeArena,
    points: &[(f64, f64)],
    depth: f64,
) -> TopoResult<ShapeId> {
    // Pocket extrudes the negative Z direction so it's "below" the sketch
    // plane: top cap at z=0, bottom at z=-depth.
    pad_polygon_xy_signed(arena, points, -depth.abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_cad_topo::{collect_by_kind, ShapeKind};

    #[test]
    fn pocket_square_makes_6_faces() {
        let mut a = ShapeArena::new();
        let id = pocket_polygon_xy(&mut a, &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], 0.5).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        assert_eq!(faces.len(), 6);
    }
}
