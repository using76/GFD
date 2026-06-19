//! Pad feature — extrude a planar 2D polygon along its normal.
//!
//! Iteration 5 scope: convex polygon → prismatic solid. Points are given in
//! order around the boundary (any orientation); the z-axis is the extrusion
//! direction. Arbitrary sketch planes and non-convex / multi-loop polygons
//! follow in later iterations.

use gfd_cad_geom::{curve::Line, surface::Plane, Direction3, Point3};
use gfd_cad_topo::{
    shape::{CurveGeom, Shape, SurfaceGeom},
    Orientation, ShapeArena, ShapeId, TopoError, TopoResult,
};

/// Extrude a planar 2D polygon (given in XY) along +Z by `height`.
/// Returns a `Shape::Solid` with top / bottom / lateral faces.
pub fn pad_polygon_xy(
    arena: &mut ShapeArena,
    points: &[(f64, f64)],
    height: f64,
) -> TopoResult<ShapeId> {
    pad_polygon_xy_signed(arena, points, height.abs())
}

/// Pad an XY polygon along an arbitrary unit direction (not just +Z). If
/// the direction has near-zero Z component, the caller's profile is still
/// treated as XY but the resulting solid tilts accordingly. Useful for
/// slanted extrusions and rib features.
pub fn pad_polygon_along(
    arena: &mut ShapeArena,
    points: &[(f64, f64)],
    height: f64,
    dir_x: f64,
    dir_y: f64,
    dir_z: f64,
) -> TopoResult<ShapeId> {
    let norm = (dir_x * dir_x + dir_y * dir_y + dir_z * dir_z).sqrt().max(1e-12);
    let (dx, dy, dz) = (dir_x / norm, dir_y / norm, dir_z / norm);
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        let h_signed = if dz >= 0.0 { height.abs() } else { -height.abs() };
        return pad_polygon_xy_signed(arena, points, h_signed);
    }
    // For off-axis directions, build explicitly.
    let bottom: Vec<Point3> = points.iter().map(|(x, y)| Point3::new(*x, *y, 0.0)).collect();
    let offs = height.abs();
    let top: Vec<Point3> = points
        .iter()
        .map(|(x, y)| Point3::new(*x + dx * offs, *y + dy * offs, dz * offs))
        .collect();
    let mut faces: Vec<(ShapeId, Orientation)> = Vec::with_capacity(points.len() + 2);
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        let a = bottom[i]; let b = bottom[j]; let c = top[j]; let d = top[i];
        let v0 = arena.push(Shape::vertex(a));
        let v1 = arena.push(Shape::vertex(b));
        let v2 = arena.push(Shape::vertex(c));
        let v3 = arena.push(Shape::vertex(d));
        let e0 = push_line_edge(arena, a, b, v0, v1)?;
        let e1 = push_line_edge(arena, b, c, v1, v2)?;
        let e2 = push_line_edge(arena, c, d, v2, v3)?;
        let e3 = push_line_edge(arena, d, a, v3, v0)?;
        let wire = arena.push(Shape::Wire { edges: vec![
            (e0, Orientation::Forward), (e1, Orientation::Forward),
            (e2, Orientation::Forward), (e3, Orientation::Forward),
        ]});
        let plane = Plane::new(a, Direction3::Z, Direction3::X);
        let face = arena.push(Shape::Face {
            surface: SurfaceGeom::Plane(plane),
            wires: vec![wire],
            orient: Orientation::Forward,
        });
        faces.push((face, Orientation::Forward));
    }
    faces.push((build_cap(arena, &bottom, true)?, Orientation::Forward));
    faces.push((build_cap(arena, &top,    false)?, Orientation::Forward));
    let shell = arena.push(Shape::Shell { faces });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

/// Same as [`pad_polygon_xy`] but preserves the sign of `height` so negative
/// values extrude downward. Used by `pocket_polygon_xy` to emit a solid that
/// lives below the sketch plane.
pub fn pad_polygon_xy_signed(
    arena: &mut ShapeArena,
    points: &[(f64, f64)],
    height: f64,
) -> TopoResult<ShapeId> {
    if points.len() < 3 {
        return Err(TopoError::Geom(gfd_cad_geom::GeomError::Degenerate(
            "pad polygon needs >= 3 points",
        )));
    }

    // Build 3D points for top and bottom loops.
    let bottom: Vec<Point3> = points.iter().map(|(x, y)| Point3::new(*x, *y, 0.0)).collect();
    let top: Vec<Point3>    = points.iter().map(|(x, y)| Point3::new(*x, *y, height)).collect();

    let mut faces: Vec<(ShapeId, Orientation)> = Vec::with_capacity(points.len() + 2);

    // Lateral quad faces (one per edge).
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        let a = bottom[i];
        let b = bottom[j];
        let c = top[j];
        let d = top[i];
        let v0 = arena.push(Shape::vertex(a));
        let v1 = arena.push(Shape::vertex(b));
        let v2 = arena.push(Shape::vertex(c));
        let v3 = arena.push(Shape::vertex(d));
        let e0 = push_line_edge(arena, a, b, v0, v1)?;
        let e1 = push_line_edge(arena, b, c, v1, v2)?;
        let e2 = push_line_edge(arena, c, d, v2, v3)?;
        let e3 = push_line_edge(arena, d, a, v3, v0)?;
        let wire = arena.push(Shape::Wire {
            edges: vec![
                (e0, Orientation::Forward),
                (e1, Orientation::Forward),
                (e2, Orientation::Forward),
                (e3, Orientation::Forward),
            ],
        });
        // Outward-facing plane: normal ∝ (b - a) × (d - a).
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let nx = dy;
        let ny = -dx;
        let n_len = (nx * nx + ny * ny).sqrt().max(f64::EPSILON);
        let normal = Direction3 { x: nx / n_len, y: ny / n_len, z: 0.0 };
        let x_axis = Direction3 {
            x: dx / n_len.max(f64::EPSILON),
            y: dy / n_len.max(f64::EPSILON),
            z: 0.0,
        };
        let plane = Plane::new(a, normal, x_axis);
        let face = arena.push(Shape::Face {
            surface: SurfaceGeom::Plane(plane),
            wires: vec![wire],
            orient: Orientation::Forward,
        });
        faces.push((face, Orientation::Forward));
    }

    // Bottom cap (normal -Z) and top cap (normal +Z) with real polygon wires
    // so `measure::face_area` via Newell's method picks them up.
    let cap_bottom = build_cap(arena, &bottom, /*downward=*/true)?;
    let cap_top = build_cap(arena, &top, /*downward=*/false)?;
    faces.push((cap_bottom, Orientation::Forward));
    faces.push((cap_top, Orientation::Forward));

    let shell = arena.push(Shape::Shell { faces });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

/// Build the lateral (side-wall) quad faces for one closed XY loop extruded
/// from z=0 to z=`height`, pushing them onto `faces`. Outward normal ∝ edge
/// direction rotated −90°, so a CCW loop faces outward and a CW loop (a hole)
/// faces inward — both "outward from the solid material", keeping the divergence
/// volume sign consistent across outer wall + hole wall.
fn push_lateral_faces(
    arena: &mut ShapeArena,
    loop_xy: &[(f64, f64)],
    height: f64,
    faces: &mut Vec<(ShapeId, Orientation)>,
) -> TopoResult<()> {
    let bottom: Vec<Point3> = loop_xy.iter().map(|(x, y)| Point3::new(*x, *y, 0.0)).collect();
    let top: Vec<Point3> = loop_xy.iter().map(|(x, y)| Point3::new(*x, *y, height)).collect();
    let n = loop_xy.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let (a, b, c, d) = (bottom[i], bottom[j], top[j], top[i]);
        let v0 = arena.push(Shape::vertex(a));
        let v1 = arena.push(Shape::vertex(b));
        let v2 = arena.push(Shape::vertex(c));
        let v3 = arena.push(Shape::vertex(d));
        let e0 = push_line_edge(arena, a, b, v0, v1)?;
        let e1 = push_line_edge(arena, b, c, v1, v2)?;
        let e2 = push_line_edge(arena, c, d, v2, v3)?;
        let e3 = push_line_edge(arena, d, a, v3, v0)?;
        let wire = arena.push(Shape::Wire { edges: vec![
            (e0, Orientation::Forward), (e1, Orientation::Forward),
            (e2, Orientation::Forward), (e3, Orientation::Forward),
        ]});
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let nlen = (dx * dx + dy * dy).sqrt().max(f64::EPSILON);
        let normal = Direction3 { x: dy / nlen, y: -dx / nlen, z: 0.0 };
        let x_axis = Direction3 { x: dx / nlen, y: dy / nlen, z: 0.0 };
        let face = arena.push(Shape::Face {
            surface: SurfaceGeom::Plane(Plane::new(a, normal, x_axis)),
            wires: vec![wire],
            orient: Orientation::Forward,
        });
        faces.push((face, Orientation::Forward));
    }
    Ok(())
}

/// Build a planar cap face carrying an outer loop plus one or more hole loops
/// (`wires = [outer, hole0, ...]`). `downward` flips the normal to −Z.
fn build_cap_with_holes(
    arena: &mut ShapeArena,
    outer: &[Point3],
    holes: &[Vec<Point3>],
    downward: bool,
) -> TopoResult<ShapeId> {
    let make_wire = |arena: &mut ShapeArena, lp: &[Point3]| -> TopoResult<ShapeId> {
        let verts: Vec<ShapeId> = lp.iter().map(|p| arena.push(Shape::vertex(*p))).collect();
        let mut edges = Vec::with_capacity(lp.len());
        for i in 0..lp.len() {
            let j = (i + 1) % lp.len();
            let e = push_line_edge(arena, lp[i], lp[j], verts[i], verts[j])?;
            edges.push((e, Orientation::Forward));
        }
        Ok(arena.push(Shape::Wire { edges }))
    };
    let mut wires = vec![make_wire(arena, outer)?];
    for h in holes {
        wires.push(make_wire(arena, h)?);
    }
    let normal = if downward { Direction3 { x: 0.0, y: 0.0, z: -1.0 } } else { Direction3::Z };
    Ok(arena.push(Shape::Face {
        surface: SurfaceGeom::Plane(Plane::new(outer[0], normal, Direction3::X)),
        wires,
        orient: Orientation::Forward,
    }))
}

/// A rectangular plate `width`×`depth`×`height` with a regular `sides`-gon
/// through-hole of radius `hole_r` centered at (`cx`,`cy`). The hole is a
/// genuine multi-wire face on both caps (so it renders cut-out and measures
/// volume = box − hole·height), with inner side walls.
#[allow(clippy::too_many_arguments)]
pub fn plate_with_hole_solid(
    arena: &mut ShapeArena,
    width: f64,
    depth: f64,
    height: f64,
    cx: f64,
    cy: f64,
    hole_r: f64,
    sides: usize,
) -> TopoResult<ShapeId> {
    if sides < 3 || hole_r <= 0.0 || width <= 0.0 || depth <= 0.0 {
        return Err(TopoError::Geom(gfd_cad_geom::GeomError::Degenerate("invalid plate/hole dimensions")));
    }
    // Outer rectangle CCW.
    let outer_xy = vec![(0.0, 0.0), (width, 0.0), (width, depth), (0.0, depth)];
    // Hole regular polygon wound CW (opposite the CCW outer) so it cuts out.
    let hole_xy: Vec<(f64, f64)> = (0..sides)
        .map(|k| {
            let t = -(k as f64) / sides as f64 * std::f64::consts::TAU; // negative = CW
            (cx + hole_r * t.cos(), cy + hole_r * t.sin())
        })
        .collect();

    let mut faces: Vec<(ShapeId, Orientation)> = Vec::new();
    push_lateral_faces(arena, &outer_xy, height, &mut faces)?;
    push_lateral_faces(arena, &hole_xy, height, &mut faces)?;
    let to3 = |loop_xy: &[(f64, f64)], z: f64| -> Vec<Point3> {
        loop_xy.iter().map(|(x, y)| Point3::new(*x, *y, z)).collect()
    };
    let bottom = build_cap_with_holes(arena, &to3(&outer_xy, 0.0), &[to3(&hole_xy, 0.0)], true)?;
    let top = build_cap_with_holes(arena, &to3(&outer_xy, height), &[to3(&hole_xy, height)], false)?;
    faces.push((bottom, Orientation::Forward));
    faces.push((top, Orientation::Forward));
    let shell = arena.push(Shape::Shell { faces });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

fn push_line_edge(arena: &mut ShapeArena, a: Point3, b: Point3, va: ShapeId, vb: ShapeId) -> TopoResult<ShapeId> {
    let line = Line::from_points(a, b).map_err(TopoError::from)?;
    Ok(arena.push(Shape::Edge {
        curve: CurveGeom::Line(line),
        vertices: [va, vb],
        orient: Orientation::Forward,
    }))
}

fn build_cap(arena: &mut ShapeArena, loop_pts: &[Point3], downward: bool) -> TopoResult<ShapeId> {
    // Register vertices + edges around the polygon boundary, bound into a wire.
    let verts: Vec<ShapeId> = loop_pts.iter().map(|p| arena.push(Shape::vertex(*p))).collect();
    let mut edges = Vec::with_capacity(loop_pts.len());
    for i in 0..loop_pts.len() {
        let j = (i + 1) % loop_pts.len();
        let e = push_line_edge(arena, loop_pts[i], loop_pts[j], verts[i], verts[j])?;
        edges.push((e, Orientation::Forward));
    }
    let wire = arena.push(Shape::Wire { edges });
    let normal = if downward {
        Direction3 { x: 0.0, y: 0.0, z: -1.0 }
    } else {
        Direction3::Z
    };
    let face = arena.push(Shape::Face {
        surface: SurfaceGeom::Plane(Plane::new(loop_pts[0], normal, Direction3::X)),
        wires: vec![wire],
        orient: Orientation::Forward,
    });
    Ok(face)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_cad_topo::{collect_by_kind, ShapeKind};

    #[test]
    fn pad_along_tilted_direction_has_6_faces() {
        let mut a = ShapeArena::new();
        let poly = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let id = pad_polygon_along(&mut a, &poly, 1.0, 0.5, 0.0, 0.866).unwrap();
        let faces = gfd_cad_topo::collect_by_kind(&a, id, gfd_cad_topo::ShapeKind::Face);
        assert_eq!(faces.len(), 6);
    }

    #[test]
    fn pad_square_has_6_faces() {
        let mut a = ShapeArena::new();
        let id = pad_polygon_xy(&mut a, &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], 0.5).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        assert_eq!(faces.len(), 6); // 4 lateral + 2 caps
    }

    #[test]
    fn pad_triangle_has_5_faces() {
        let mut a = ShapeArena::new();
        let id = pad_polygon_xy(&mut a, &[(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)], 0.3).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        assert_eq!(faces.len(), 5); // 3 lateral + 2 caps
    }
}
