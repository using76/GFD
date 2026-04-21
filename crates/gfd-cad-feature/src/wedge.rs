//! Wedge primitive — triangular prism.
//!
//! Profile: right triangle in XZ plane with legs along +X and +Z; swept
//! along +Y by `width`. Common CAD primitive for ramps / stops / clamps.

use gfd_cad_geom::{curve::Line, surface::Plane, Direction3, Point3};
use gfd_cad_topo::{
    shape::{CurveGeom, Shape, SurfaceGeom},
    Orientation, ShapeArena, ShapeId, TopoError, TopoResult,
};

pub fn wedge_solid(arena: &mut ShapeArena, lx: f64, ly: f64, lz: f64) -> TopoResult<ShapeId> {
    // 6 vertices: two triangles (Y=0 and Y=ly), each with legs lx × lz.
    let pts = [
        Point3::new(0.0, 0.0, 0.0),  // 0
        Point3::new(lx,  0.0, 0.0),  // 1
        Point3::new(0.0, 0.0, lz ),  // 2
        Point3::new(0.0, ly,  0.0),  // 3
        Point3::new(lx,  ly,  0.0),  // 4
        Point3::new(0.0, ly,  lz ),  // 5
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

    // Bottom (y=0) rectangle + back (x=0) rectangle + hypotenuse rectangle.
    build(arena, &[0, 1, 4, 3], Direction3 { x: 0.0, y: 0.0, z: -1.0 })?;  // bottom (−Z facing? original was -Z) — actually bottom of wedge
    build(arena, &[0, 3, 5, 2], Direction3 { x: -1.0, y: 0.0, z: 0.0 })?;  // back (−X)
    // Hypotenuse: points 1, 4, 5, 2 in CCW when viewed from outside.
    let len = (lx * lx + lz * lz).sqrt().max(f64::EPSILON);
    let nx = lz / len;
    let nz = lx / len;
    build(arena, &[1, 4, 5, 2], Direction3 { x: nx, y: 0.0, z: nz })?;
    // Two triangular sides (Y=0 and Y=ly).
    build(arena, &[0, 2, 1], Direction3 { x: 0.0, y: -1.0, z: 0.0 })?;
    build(arena, &[3, 4, 5], Direction3 { x: 0.0, y:  1.0, z: 0.0 })?;

    let shell = arena.push(Shape::Shell { faces });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_cad_topo::{collect_by_kind, ShapeKind};

    #[test]
    fn wedge_has_5_faces() {
        let mut a = ShapeArena::new();
        let id = wedge_solid(&mut a, 1.0, 1.0, 1.0).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        assert_eq!(faces.len(), 5); // 3 quads + 2 triangles
    }
}
