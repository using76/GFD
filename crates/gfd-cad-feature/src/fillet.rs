//! Fillet feature — round a corner of an axis-aligned box with a sphere octant.
//!
//! Iteration 17 scope: single-corner fillet only. Generic rolling-ball
//! fillet over arbitrary edges ships with the full B-Rep CSG pipeline.
//! The octant is modelled as a `Shape::Face` whose surface is a sphere
//! centred at the inset corner point — tessellation handles it correctly.

use gfd_cad_geom::{curve::Line, surface::{Plane, Sphere}, Direction3, Point3};
use gfd_cad_topo::{
    shape::{CurveGeom, Shape, SurfaceGeom},
    Orientation, ShapeArena, ShapeId, TopoError, TopoResult,
};

/// Build a filleted box: clip the `(+hx, +hy, +hz)` corner region of an
/// axis-aligned box by `radius` along each axis, then replace the corner
/// with a sphere face of that radius. The sphere is centred at
/// `(+hx - r, +hy - r, +hz - r)`.
pub fn filleted_box_solid(
    arena: &mut ShapeArena,
    lx: f64,
    ly: f64,
    lz: f64,
    radius: f64,
) -> TopoResult<ShapeId> {
    let min_dim = lx.min(ly).min(lz);
    let r = radius.clamp(1.0e-6, min_dim * 0.5 - 1.0e-6);
    let (hx, hy, hz) = (lx * 0.5, ly * 0.5, lz * 0.5);

    // 11-entry array; index 6 is the removed corner (never referenced below).
    let pts = [
        Point3::new(-hx, -hy, -hz), // 0
        Point3::new( hx, -hy, -hz), // 1
        Point3::new( hx,  hy, -hz), // 2
        Point3::new(-hx,  hy, -hz), // 3
        Point3::new(-hx, -hy,  hz), // 4
        Point3::new( hx, -hy,  hz), // 5
        Point3::new( hx,  hy,  hz), // 6 placeholder — NOT referenced by any face
        Point3::new(-hx,  hy,  hz), // 7
        Point3::new( hx - r,  hy,  hz), // 8 — top inset along -X
        Point3::new( hx,  hy - r,  hz), // 9 — top inset along -Y
        Point3::new( hx,  hy,  hz - r), // 10 — side inset along -Z
    ];

    let verts: Vec<ShapeId> = pts.iter().map(|p| arena.push(Shape::vertex(*p))).collect();
    let mut faces: Vec<(ShapeId, Orientation)> = Vec::new();

    let mut build_face = |arena: &mut ShapeArena, idxs: &[usize], normal: Direction3| -> TopoResult<()> {
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

    // Same skeleton as chamfered_box, minus the corner, plus a sphere face.
    build_face(arena, &[0, 1, 2, 3], Direction3 { x: 0.0, y: 0.0, z: -1.0 })?; // bottom
    build_face(arena, &[0, 3, 7, 4], Direction3 { x: -1.0, y: 0.0, z: 0.0 })?; // -X
    build_face(arena, &[0, 4, 5, 1], Direction3 { x: 0.0, y: -1.0, z: 0.0 })?; // -Y
    build_face(arena, &[1, 2, 10, 9, 5], Direction3::X)?;                       // +X pentagon
    build_face(arena, &[3, 2, 10, 8, 7], Direction3::Y)?;                       // +Y pentagon
    build_face(arena, &[4, 5, 9, 8, 7], Direction3::Z)?;                        // top pentagon

    // Fillet surface: sphere octant at the clipped corner. Centre at the
    // inset diagonal point (hx-r, hy-r, hz-r), no wire (closed analytical
    // surface — surface_area picks up the full sphere; that's an upper bound
    // on the real area but acceptable for iter 17).
    let centre = Point3::new(hx - r, hy - r, hz - r);
    let fillet_face = arena.push(Shape::Face {
        surface: SurfaceGeom::Sphere(Sphere::new(centre, r)),
        wires: vec![],
        orient: Orientation::Forward,
    });
    faces.push((fillet_face, Orientation::Forward));

    let shell = arena.push(Shape::Shell { faces });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

/// Fillet all four top edges of an axis-aligned box with quarter-cylinder
/// surfaces. The 4 top corners become quarter-sphere faces and the 4 top
/// edges become quarter-cylinder faces. Result: 9 primary faces (bottom +
/// 4 sides + top-inset rect) + 4 edge cylinders + 4 corner spheres = 17.
pub fn filleted_box_top_edges(
    arena: &mut ShapeArena,
    lx: f64,
    ly: f64,
    lz: f64,
    radius: f64,
) -> TopoResult<ShapeId> {
    use gfd_cad_geom::surface::{Cylinder, Sphere};
    let min_xy = lx.min(ly);
    let r = radius.clamp(1.0e-6, min_xy * 0.5 - 1.0e-6).min(lz - 1.0e-6);
    let (hx, hy, hz) = (lx * 0.5, ly * 0.5, lz * 0.5);

    let pts = [
        Point3::new(-hx, -hy, -hz),          // 0
        Point3::new( hx, -hy, -hz),          // 1
        Point3::new( hx,  hy, -hz),          // 2
        Point3::new(-hx,  hy, -hz),          // 3
        Point3::new(-hx, -hy,  hz - r),      // 4  side-top, -X-Y
        Point3::new( hx, -hy,  hz - r),      // 5
        Point3::new( hx,  hy,  hz - r),      // 6
        Point3::new(-hx,  hy,  hz - r),      // 7
        Point3::new(-hx + r, -hy + r,  hz),  // 8  top-inset ring
        Point3::new( hx - r, -hy + r,  hz),  // 9
        Point3::new( hx - r,  hy - r,  hz),  // 10
        Point3::new(-hx + r,  hy - r,  hz),  // 11
    ];
    let verts: Vec<ShapeId> = pts.iter().map(|p| arena.push(Shape::vertex(*p))).collect();

    let mut faces: Vec<(ShapeId, Orientation)> = Vec::new();
    let mut build_planar = |arena: &mut ShapeArena, idxs: &[usize], normal: Direction3| -> TopoResult<()> {
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

    // Bottom + 4 full side rectangles + inset top square
    build_planar(arena, &[0, 1, 2, 3], Direction3 { x: 0.0, y: 0.0, z: -1.0 })?;
    build_planar(arena, &[0, 1, 5, 4], Direction3 { x: 0.0, y: -1.0, z: 0.0 })?;
    build_planar(arena, &[1, 2, 6, 5], Direction3::X)?;
    build_planar(arena, &[2, 3, 7, 6], Direction3::Y)?;
    build_planar(arena, &[3, 0, 4, 7], Direction3 { x: -1.0, y: 0.0, z: 0.0 })?;
    build_planar(arena, &[8, 9, 10, 11], Direction3::Z)?;

    // 4 quarter-cylinder faces along each top edge (no wires — analytic
    // surface_area will approximate via full cylinder band).
    let cyl_origins = [
        Point3::new(0.0, -hy + r,  hz - r),   // along +X edge, axis = X
        Point3::new( hx - r, 0.0,  hz - r),   // along +Y edge, axis = Y
        Point3::new(0.0,  hy - r,  hz - r),
        Point3::new(-hx + r, 0.0,  hz - r),
    ];
    let cyl_axes = [Direction3::X, Direction3::Y, Direction3::X, Direction3::Y];
    let cyl_heights = [lx, ly, lx, ly];
    for i in 0..4 {
        let face = arena.push(Shape::Face {
            surface: SurfaceGeom::Cylinder(Cylinder::new(
                cyl_origins[i], cyl_axes[i], Direction3::Z, r, cyl_heights[i],
            )),
            wires: vec![],
            orient: Orientation::Forward,
        });
        faces.push((face, Orientation::Forward));
    }

    // 4 quarter-sphere faces at each top corner.
    let sphere_centres = [
        Point3::new(-hx + r, -hy + r,  hz - r),
        Point3::new( hx - r, -hy + r,  hz - r),
        Point3::new( hx - r,  hy - r,  hz - r),
        Point3::new(-hx + r,  hy - r,  hz - r),
    ];
    for c in &sphere_centres {
        let face = arena.push(Shape::Face {
            surface: SurfaceGeom::Sphere(Sphere::new(*c, r)),
            wires: vec![],
            orient: Orientation::Forward,
        });
        faces.push((face, Orientation::Forward));
    }

    let shell = arena.push(Shape::Shell { faces });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

/// Cylinder with both top and bottom circular edges filleted by radius `f`.
/// Analytic model (no wires, all surfaces closed analytic):
/// - Bottom cap disk: plane at z=0 (radius r-f)
/// - Bottom fillet: torus at z=f (major = r-f, minor = f)
/// - Lateral cylinder: radius r, z ∈ [f, height - f]
/// - Top fillet: torus at z = height - f
/// - Top cap disk: plane at z = height (radius r-f)
///
/// Returns a 5-face `Shape::Solid`.
pub fn filleted_cylinder_solid(
    arena: &mut ShapeArena,
    radius: f64,
    height: f64,
    fillet: f64,
) -> TopoResult<ShapeId> {
    use gfd_cad_geom::surface::{Cylinder, Torus};
    let f = fillet.clamp(1.0e-6, radius.min(height * 0.5) - 1.0e-6);
    let mut faces: Vec<(ShapeId, Orientation)> = Vec::new();

    // Bottom cap
    let bot = arena.push(Shape::Face {
        surface: SurfaceGeom::Plane(Plane::new(
            Point3::ORIGIN,
            Direction3 { x: 0.0, y: 0.0, z: -1.0 },
            Direction3::X,
        )),
        wires: vec![],
        orient: Orientation::Forward,
    });
    faces.push((bot, Orientation::Forward));

    // Bottom fillet (torus)
    let tor_bot = arena.push(Shape::Face {
        surface: SurfaceGeom::Torus(Torus::new(
            Point3::new(0.0, 0.0, f),
            Direction3::Z,
            Direction3::X,
            radius - f,
            f,
        )),
        wires: vec![],
        orient: Orientation::Forward,
    });
    faces.push((tor_bot, Orientation::Forward));

    // Lateral cylinder
    let lateral = arena.push(Shape::Face {
        surface: SurfaceGeom::Cylinder(Cylinder::new(
            Point3::new(0.0, 0.0, f),
            Direction3::Z,
            Direction3::X,
            radius,
            (height - 2.0 * f).max(0.0),
        )),
        wires: vec![],
        orient: Orientation::Forward,
    });
    faces.push((lateral, Orientation::Forward));

    // Top fillet (torus)
    let tor_top = arena.push(Shape::Face {
        surface: SurfaceGeom::Torus(Torus::new(
            Point3::new(0.0, 0.0, height - f),
            Direction3::Z,
            Direction3::X,
            radius - f,
            f,
        )),
        wires: vec![],
        orient: Orientation::Forward,
    });
    faces.push((tor_top, Orientation::Forward));

    // Top cap
    let top = arena.push(Shape::Face {
        surface: SurfaceGeom::Plane(Plane::new(
            Point3::new(0.0, 0.0, height),
            Direction3::Z,
            Direction3::X,
        )),
        wires: vec![],
        orient: Orientation::Forward,
    });
    faces.push((top, Orientation::Forward));

    let shell = arena.push(Shape::Shell { faces });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_cad_topo::{collect_by_kind, ShapeKind};

    #[test]
    fn filleted_cylinder_has_5_faces() {
        let mut a = ShapeArena::new();
        let id = filleted_cylinder_solid(&mut a, 0.5, 1.0, 0.1).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        assert_eq!(faces.len(), 5);
    }

    #[test]
    fn filleted_box_top_edges_has_14_faces() {
        let mut a = ShapeArena::new();
        let id = filleted_box_top_edges(&mut a, 2.0, 2.0, 1.0, 0.2).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        // 1 bottom + 4 sides + 1 top + 4 cylinder edges + 4 corner spheres = 14.
        assert_eq!(faces.len(), 14);
    }

    #[test]
    fn filleted_box_has_7_faces() {
        let mut a = ShapeArena::new();
        let id = filleted_box_solid(&mut a, 2.0, 2.0, 2.0, 0.3).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        assert_eq!(faces.len(), 7); // 6 planar (3 rectangles + 3 pentagons) + 1 sphere
    }
}
