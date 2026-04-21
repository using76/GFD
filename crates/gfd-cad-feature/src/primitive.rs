//! Primitive shape constructors (Box / Cylinder / Sphere / Cone / Torus).
//!
//! Each function takes an arena plus parameters, constructs every vertex /
//! edge / wire / face / shell needed, and returns the id of the resulting
//! `Shape::Solid`.

use gfd_cad_geom::{
    curve::Line, surface::Cone, surface::Cylinder, surface::Plane, surface::Sphere,
    surface::Torus, Direction3, Point3,
};
use gfd_cad_topo::{
    shape::{CurveGeom, Shape, SurfaceGeom},
    Orientation, ShapeArena, ShapeId, TopoError, TopoResult,
};

fn vertex(arena: &mut ShapeArena, p: Point3) -> ShapeId {
    arena.push(Shape::vertex(p))
}

fn line_edge(arena: &mut ShapeArena, a: Point3, b: Point3, va: ShapeId, vb: ShapeId) -> TopoResult<ShapeId> {
    let line = Line::from_points(a, b).map_err(TopoError::from)?;
    Ok(arena.push(Shape::Edge {
        curve: CurveGeom::Line(line),
        vertices: [va, vb],
        orient: Orientation::Forward,
    }))
}

fn quad_face(arena: &mut ShapeArena, plane: Plane, corners: [Point3; 4]) -> TopoResult<ShapeId> {
    let v: Vec<ShapeId> = corners.iter().map(|p| vertex(arena, *p)).collect();
    let mut edges = Vec::with_capacity(4);
    for i in 0..4 {
        let j = (i + 1) % 4;
        let e = line_edge(arena, corners[i], corners[j], v[i], v[j])?;
        edges.push((e, Orientation::Forward));
    }
    let wire = arena.push(Shape::Wire { edges });
    Ok(arena.push(Shape::Face {
        surface: SurfaceGeom::Plane(plane),
        wires: vec![wire],
        orient: Orientation::Forward,
    }))
}

/// Spiral staircase — `step_count` rectangular treads revolving around
/// the Z axis at `radius`. Each step rotates by `angle_per_step_deg` and
/// rises by `rise_per_step`. Tread dimensions: `tread_len × tread_w ×
/// step_h` (radial length × tangential width × vertical thickness).
/// Returns a `Compound` of all treads.
pub fn spiral_staircase_solid(
    arena: &mut ShapeArena,
    step_count: usize,
    radius: f64,
    tread_len: f64,
    tread_w: f64,
    step_h: f64,
    angle_per_step_deg: f64,
    rise_per_step: f64,
) -> TopoResult<ShapeId> {
    if step_count < 1 || radius <= 0.0 || tread_len <= 0.0
        || tread_w <= 0.0 || step_h <= 0.0 {
        return Err(gfd_cad_topo::TopoError::Geom(
            gfd_cad_geom::GeomError::Degenerate("spiral_staircase: invalid args")));
    }
    let mut children: Vec<ShapeId> = Vec::with_capacity(step_count);
    let step_rad = angle_per_step_deg.to_radians();
    for i in 0..step_count {
        let theta = i as f64 * step_rad;
        let (ts, tc) = theta.sin_cos();
        // Build a tread lying along +X from the axis.
        let tread = box_solid(arena, tread_len, tread_w, step_h)?;
        // 1. Translate so its inner edge touches radius.
        let shifted = crate::translate_shape(
            arena, tread,
            radius + tread_len * 0.5, 0.0, i as f64 * rise_per_step,
        )?;
        // 2. Rotate about Z by theta (via translate then rotate is not
        //    quite right — use rotate_shape(about z) + translate).
        // Simpler: build at origin, rotate, then translate — but the
        // translate above already baked x-offset. Undo + rotate + redo.
        // Here we use a direct vertex remap via `rotate_shape` which
        // rotates about origin, acceptable because our translate sits
        // in the XY plane at theta=0.
        let rotated = crate::rotate_shape(arena, shifted, (0.0, 0.0, 1.0), theta)?;
        // Convert to tangential orientation (rotate about +Z by theta
        // already handled). Final correction for y at theta:
        // actually rotate_shape does the work, no further correction.
        let _ = ts; let _ = tc;
        children.push(rotated);
    }
    Ok(arena.push(gfd_cad_topo::Shape::Compound { children }))
}

/// Honeycomb pattern: `rows × cols` hexagonal prisms tiled without gaps.
/// Each hexagon has circumradius `hex_r`, height `hex_h`. Rows are offset
/// by `hex_r · √3` and alternate columns by `hex_r · 1.5 · √3/2` to
/// achieve tight packing. Returns a `Compound`.
pub fn honeycomb_pattern_solid(
    arena: &mut ShapeArena,
    rows: usize,
    cols: usize,
    hex_r: f64,
    hex_h: f64,
) -> TopoResult<ShapeId> {
    if rows == 0 || cols == 0 || hex_r <= 0.0 || hex_h <= 0.0 {
        return Err(gfd_cad_topo::TopoError::Geom(
            gfd_cad_geom::GeomError::Degenerate("honeycomb: invalid args")));
    }
    let dx = hex_r * 1.5;
    let dy = hex_r * 3f64.sqrt();
    let mut children: Vec<ShapeId> = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            let ox = c as f64 * dx;
            let oy = r as f64 * dy + if c % 2 == 1 { dy * 0.5 } else { 0.0 };
            let base = crate::pyramid::ngon_prism_solid(arena, 6, hex_r, hex_h)?;
            let moved = crate::translate_shape(arena, base, ox, oy, 0.0)?;
            children.push(moved);
        }
    }
    Ok(arena.push(gfd_cad_topo::Shape::Compound { children }))
}

/// Parametric staircase: `step_count` boxes of width × height × depth
/// each, stacked with each step shifted +X by `depth` and +Z by `height`.
/// Returns a `Compound` of the step solids. Useful architectural/demo
/// primitive; also stresses compound rendering.
pub fn stairs_solid(
    arena: &mut ShapeArena,
    step_count: usize,
    step_w: f64,
    step_h: f64,
    step_d: f64,
) -> TopoResult<ShapeId> {
    if step_count < 1 || step_w <= 0.0 || step_h <= 0.0 || step_d <= 0.0 {
        return Err(gfd_cad_topo::TopoError::Geom(
            gfd_cad_geom::GeomError::Degenerate("stairs: invalid args")));
    }
    let mut children: Vec<ShapeId> = Vec::with_capacity(step_count);
    for i in 0..step_count {
        let fi = i as f64;
        // Each step spans x in [i·d, (i+1)·d], z in [0, (i+1)·h], y in [-w/2, w/2].
        let box_id = box_solid(arena, step_d, step_w, (fi + 1.0) * step_h)?;
        let offset_x = fi * step_d + step_d * 0.5;
        let offset_z = (fi + 1.0) * step_h * 0.5;
        let moved = crate::translate_shape(arena, box_id, offset_x, 0.0, offset_z)?;
        children.push(moved);
    }
    Ok(arena.push(gfd_cad_topo::Shape::Compound { children }))
}

/// Unit-style convenience: cube of side `scale`, centered at origin.
pub fn cube_solid(arena: &mut ShapeArena, scale: f64) -> TopoResult<ShapeId> {
    box_solid(arena, scale, scale, scale)
}

/// Alias for `box_solid` — reads better when the user wants a non-cube
/// rectangular prism.
pub fn rectangular_prism_solid(
    arena: &mut ShapeArena, lx: f64, ly: f64, lz: f64,
) -> TopoResult<ShapeId> {
    box_solid(arena, lx, ly, lz)
}

/// Axis-aligned box centered at origin.
pub fn box_solid(arena: &mut ShapeArena, lx: f64, ly: f64, lz: f64) -> TopoResult<ShapeId> {
    let (hx, hy, hz) = (lx * 0.5, ly * 0.5, lz * 0.5);
    // Corner coords indexed by (i,j,k) ∈ {0,1}^3 → sign (+/-).
    let c = |i: u8, j: u8, k: u8| Point3::new(
        if i == 0 { -hx } else { hx },
        if j == 0 { -hy } else { hy },
        if k == 0 { -hz } else { hz },
    );
    let mut faces = Vec::with_capacity(6);
    // +Z / -Z
    faces.push(quad_face(arena, Plane::new(Point3::new(0.0,0.0, hz), Direction3::Z, Direction3::X),
        [c(0,0,1), c(1,0,1), c(1,1,1), c(0,1,1)])?);
    faces.push(quad_face(arena, Plane::new(Point3::new(0.0,0.0,-hz),
        Direction3 { x: 0.0, y: 0.0, z: -1.0 }, Direction3::X),
        [c(0,1,0), c(1,1,0), c(1,0,0), c(0,0,0)])?);
    // +X / -X
    faces.push(quad_face(arena, Plane::new(Point3::new( hx,0.0,0.0), Direction3::X, Direction3::Y),
        [c(1,0,0), c(1,1,0), c(1,1,1), c(1,0,1)])?);
    faces.push(quad_face(arena, Plane::new(Point3::new(-hx,0.0,0.0),
        Direction3 { x: -1.0, y: 0.0, z: 0.0 }, Direction3::Y),
        [c(0,0,1), c(0,1,1), c(0,1,0), c(0,0,0)])?);
    // +Y / -Y
    faces.push(quad_face(arena, Plane::new(Point3::new(0.0, hy,0.0), Direction3::Y, Direction3::Z),
        [c(0,1,0), c(0,1,1), c(1,1,1), c(1,1,0)])?);
    faces.push(quad_face(arena, Plane::new(Point3::new(0.0,-hy,0.0),
        Direction3 { x: 0.0, y: -1.0, z: 0.0 }, Direction3::Z),
        [c(1,0,0), c(1,0,1), c(0,0,1), c(0,0,0)])?);
    let shell = arena.push(Shape::Shell {
        faces: faces.into_iter().map(|f| (f, Orientation::Forward)).collect(),
    });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

/// Sphere centered at origin. Single face, degenerate wire.
pub fn sphere_solid(arena: &mut ShapeArena, radius: f64) -> TopoResult<ShapeId> {
    let face = arena.push(Shape::Face {
        surface: SurfaceGeom::Sphere(Sphere::new(Point3::ORIGIN, radius)),
        wires: vec![],
        orient: Orientation::Forward,
    });
    let shell = arena.push(Shape::Shell { faces: vec![(face, Orientation::Forward)] });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

/// Right cylinder along +Z with base at z=0, top at z=height.
pub fn cylinder_solid(arena: &mut ShapeArena, radius: f64, height: f64) -> TopoResult<ShapeId> {
    let lateral = arena.push(Shape::Face {
        surface: SurfaceGeom::Cylinder(Cylinder::new(
            Point3::ORIGIN, Direction3::Z, Direction3::X, radius, height)),
        wires: vec![],
        orient: Orientation::Forward,
    });
    let top = arena.push(Shape::Face {
        surface: SurfaceGeom::Plane(Plane::new(
            Point3::new(0.0, 0.0, height), Direction3::Z, Direction3::X)),
        wires: vec![],
        orient: Orientation::Forward,
    });
    let bottom = arena.push(Shape::Face {
        surface: SurfaceGeom::Plane(Plane::new(
            Point3::ORIGIN,
            Direction3 { x: 0.0, y: 0.0, z: -1.0 },
            Direction3::X)),
        wires: vec![],
        orient: Orientation::Forward,
    });
    let shell = arena.push(Shape::Shell {
        faces: vec![
            (lateral, Orientation::Forward),
            (top, Orientation::Forward),
            (bottom, Orientation::Forward),
        ],
    });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

/// Truncated cone (frustum) or full cone (r2=0).
pub fn cone_solid(arena: &mut ShapeArena, r1: f64, r2: f64, height: f64) -> TopoResult<ShapeId> {
    let lateral = arena.push(Shape::Face {
        surface: SurfaceGeom::Cone(Cone::new(
            Point3::ORIGIN, Direction3::Z, Direction3::X, r1, r2, height)),
        wires: vec![],
        orient: Orientation::Forward,
    });
    let bottom = arena.push(Shape::Face {
        surface: SurfaceGeom::Plane(Plane::new(
            Point3::ORIGIN,
            Direction3 { x: 0.0, y: 0.0, z: -1.0 },
            Direction3::X)),
        wires: vec![],
        orient: Orientation::Forward,
    });
    let mut faces = vec![
        (lateral, Orientation::Forward),
        (bottom, Orientation::Forward),
    ];
    if r2.abs() > f64::EPSILON {
        let top = arena.push(Shape::Face {
            surface: SurfaceGeom::Plane(Plane::new(
                Point3::new(0.0, 0.0, height), Direction3::Z, Direction3::X)),
            wires: vec![],
            orient: Orientation::Forward,
        });
        faces.push((top, Orientation::Forward));
    }
    let shell = arena.push(Shape::Shell { faces });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

/// Torus with Z-aligned axis of revolution.
pub fn torus_solid(arena: &mut ShapeArena, major: f64, minor: f64) -> TopoResult<ShapeId> {
    let face = arena.push(Shape::Face {
        surface: SurfaceGeom::Torus(Torus::new(
            Point3::ORIGIN, Direction3::Z, Direction3::X, major, minor)),
        wires: vec![],
        orient: Orientation::Forward,
    });
    let shell = arena.push(Shape::Shell { faces: vec![(face, Orientation::Forward)] });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_cad_topo::{collect_by_kind, ShapeKind};

    #[test]
    fn box_has_six_faces() {
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 1.0, 1.0, 1.0).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        assert_eq!(faces.len(), 6);
    }

    #[test]
    fn sphere_has_one_face() {
        let mut a = ShapeArena::new();
        let id = sphere_solid(&mut a, 0.5).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        assert_eq!(faces.len(), 1);
    }

    #[test]
    fn cylinder_has_three_faces() {
        let mut a = ShapeArena::new();
        let id = cylinder_solid(&mut a, 0.3, 1.0).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        assert_eq!(faces.len(), 3);
    }
}
