//! Pyramid primitive — polygon base + apex.
//!
//! Currently supports a square base (rectangular frustum with `r2=0` is
//! an alternative but this is more natural for users); a regular N-gon
//! base variant sits in [`crate::ngon_prism_solid`].

use gfd_cad_geom::{curve::Line, surface::Plane, Direction3, Point3};
use gfd_cad_topo::{
    shape::{CurveGeom, Shape, SurfaceGeom},
    Orientation, ShapeArena, ShapeId, TopoError, TopoResult,
};

/// Square-based pyramid: base (lx × ly) at z=0, apex at (0, 0, height).
pub fn pyramid_solid(arena: &mut ShapeArena, lx: f64, ly: f64, height: f64) -> TopoResult<ShapeId> {
    let (hx, hy) = (lx * 0.5, ly * 0.5);
    let pts = [
        Point3::new(-hx, -hy, 0.0),    // 0
        Point3::new( hx, -hy, 0.0),    // 1
        Point3::new( hx,  hy, 0.0),    // 2
        Point3::new(-hx,  hy, 0.0),    // 3
        Point3::new(0.0, 0.0, height), // 4 apex
    ];
    let verts: Vec<ShapeId> = pts.iter().map(|p| arena.push(Shape::vertex(*p))).collect();
    let mut faces: Vec<(ShapeId, Orientation)> = Vec::new();
    let mut build = |arena: &mut ShapeArena, idxs: &[usize], normal: Direction3| -> TopoResult<()> {
        let mut edges = Vec::with_capacity(idxs.len());
        for i in 0..idxs.len() {
            let j = (i + 1) % idxs.len();
            let line = Line::from_points(pts[idxs[i]], pts[idxs[j]]).map_err(TopoError::from)?;
            let e = arena.push(Shape::Edge {
                curve: CurveGeom::Line(line),
                vertices: [verts[idxs[i]], verts[idxs[j]]],
                orient: Orientation::Forward,
            });
            edges.push((e, Orientation::Forward));
        }
        let wire = arena.push(Shape::Wire { edges });
        let plane = Plane::new(pts[idxs[0]], normal, Direction3::X);
        let face = arena.push(Shape::Face {
            surface: SurfaceGeom::Plane(plane),
            wires: vec![wire],
            orient: Orientation::Forward,
        });
        faces.push((face, Orientation::Forward));
        Ok(())
    };

    build(arena, &[0, 1, 2, 3], Direction3 { x: 0.0, y: 0.0, z: -1.0 })?;  // base
    build(arena, &[0, 1, 4], Direction3 { x: 0.0, y: -1.0, z: 0.0 })?;     // -Y triangle
    build(arena, &[1, 2, 4], Direction3::X)?;                                // +X
    build(arena, &[2, 3, 4], Direction3::Y)?;                                // +Y
    build(arena, &[3, 0, 4], Direction3 { x: -1.0, y: 0.0, z: 0.0 })?;      // -X

    let shell = arena.push(Shape::Shell { faces });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

/// Regular N-gon prism: flat N-sided polygon base (inscribed in circle of
/// `radius`) at z=0, straight walls extending to z=height.
pub fn ngon_prism_solid(arena: &mut ShapeArena, sides: usize, radius: f64, height: f64) -> TopoResult<ShapeId> {
    use std::f64::consts::TAU;
    if sides < 3 {
        return Err(TopoError::Geom(gfd_cad_geom::GeomError::Degenerate("ngon prism needs >= 3 sides")));
    }
    // Use the existing Pad helper — build polygon around origin and extrude.
    let polygon: Vec<(f64, f64)> = (0..sides)
        .map(|i| {
            let a = TAU * i as f64 / sides as f64;
            (radius * a.cos(), radius * a.sin())
        })
        .collect();
    crate::pad::pad_polygon_xy(arena, &polygon, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_cad_topo::{collect_by_kind, ShapeKind};

    #[test]
    fn pyramid_has_5_faces() {
        let mut a = ShapeArena::new();
        let id = pyramid_solid(&mut a, 2.0, 2.0, 1.0).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        assert_eq!(faces.len(), 5);
    }

    #[test]
    fn hexagonal_prism_has_8_faces() {
        let mut a = ShapeArena::new();
        let id = ngon_prism_solid(&mut a, 6, 1.0, 1.0).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        assert_eq!(faces.len(), 8); // 6 lateral + 2 caps
    }

    #[test]
    fn ngon_prism_rejects_small_n() {
        let mut a = ShapeArena::new();
        assert!(ngon_prism_solid(&mut a, 2, 1.0, 1.0).is_err());
    }
}
