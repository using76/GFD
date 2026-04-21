//! Revolve feature — rotate a 2D profile about the Z-axis.
//!
//! Iteration 9 scope: full 360° sweep of a polygon defined in the XZ plane
//! (x ≥ 0 for the profile's "radial" coordinate, z for axial height). The
//! output is a `Shape::Solid` whose lateral faces are a collection of
//! planar quads obtained by discretising the rotation into `angular_steps`
//! segments. Partial (<360°) sweeps land in a later iteration.

use gfd_cad_geom::{surface::Plane, Direction3, Point3};
use gfd_cad_topo::{
    shape::{Shape, SurfaceGeom},
    Orientation, ShapeArena, ShapeId, TopoError, TopoResult,
};

/// Revolve a 2D profile by an arbitrary angle (radians) about the Z axis.
/// When `angle_rad >= 2π` this is a full revolve; otherwise a partial sweep
/// that leaves the start/end azimuths exposed (no end caps in iter 34).
pub fn revolve_profile_z_partial(
    arena: &mut ShapeArena,
    profile_rz: &[(f64, f64)],
    angular_steps: usize,
    angle_rad: f64,
) -> TopoResult<ShapeId> {
    if profile_rz.len() < 2 {
        return Err(TopoError::Geom(gfd_cad_geom::GeomError::Degenerate(
            "revolve profile needs >= 2 points",
        )));
    }
    if angular_steps < 3 {
        return Err(TopoError::Geom(gfd_cad_geom::GeomError::Degenerate(
            "revolve angular_steps must be >= 3",
        )));
    }
    let mut faces: Vec<(ShapeId, Orientation)> = Vec::with_capacity(angular_steps * profile_rz.len());
    for step in 0..angular_steps {
        let a0 = angle_rad * step as f64 / angular_steps as f64;
        let a1 = angle_rad * (step + 1) as f64 / angular_steps as f64;
        for i in 0..profile_rz.len() - 1 {
            let (r0, z0) = profile_rz[i];
            let (r1, z1) = profile_rz[i + 1];
            let p_a0 = Point3::new(r0 * a0.cos(), r0 * a0.sin(), z0);
            let p_b0 = Point3::new(r1 * a0.cos(), r1 * a0.sin(), z1);
            let p_b1 = Point3::new(r1 * a1.cos(), r1 * a1.sin(), z1);
            let p_a1 = Point3::new(r0 * a1.cos(), r0 * a1.sin(), z0);
            let v0 = arena.push(Shape::vertex(p_a0));
            let v1 = arena.push(Shape::vertex(p_b0));
            let v2 = arena.push(Shape::vertex(p_b1));
            let v3 = arena.push(Shape::vertex(p_a1));
            let e0 = push_line_edge(arena, p_a0, p_b0, v0, v1)?;
            let e1 = push_line_edge(arena, p_b0, p_b1, v1, v2)?;
            let e2 = push_line_edge(arena, p_b1, p_a1, v2, v3)?;
            let e3 = push_line_edge(arena, p_a1, p_a0, v3, v0)?;
            let wire = arena.push(Shape::Wire {
                edges: vec![
                    (e0, Orientation::Forward),
                    (e1, Orientation::Forward),
                    (e2, Orientation::Forward),
                    (e3, Orientation::Forward),
                ],
            });
            let plane = Plane::new(p_a0, Direction3::Z, Direction3::X);
            let face = arena.push(Shape::Face {
                surface: SurfaceGeom::Plane(plane),
                wires: vec![wire],
                orient: Orientation::Forward,
            });
            faces.push((face, Orientation::Forward));
        }
    }
    // End caps for partial sweeps: the profile polygon rotated to the start
    // and end azimuths, closed by dropping a segment along the Z-axis if the
    // profile doesn't already touch it.
    let is_partial = angle_rad < std::f64::consts::TAU - 1.0e-9;
    if is_partial {
        let mut build_cap = |arena: &mut ShapeArena, az: f64, normal_sign: f64| -> TopoResult<()> {
            // Ensure the profile encloses an area by adding the axis points
            // (r=0 at the same z) if it doesn't already start/end on the axis.
            let mut loop_pts = profile_rz.to_vec();
            if loop_pts.first().unwrap().0 > 1.0e-9 {
                let (_, z_first) = *loop_pts.first().unwrap();
                loop_pts.insert(0, (0.0, z_first));
            }
            if loop_pts.last().unwrap().0 > 1.0e-9 {
                let (_, z_last) = *loop_pts.last().unwrap();
                loop_pts.push((0.0, z_last));
            }
            // Rotate to the target azimuth.
            let (sa, ca) = az.sin_cos();
            let poly_3d: Vec<Point3> = loop_pts.iter()
                .map(|(r, z)| Point3::new(r * ca, r * sa, *z))
                .collect();
            let verts: Vec<ShapeId> = poly_3d.iter().map(|p| arena.push(Shape::vertex(*p))).collect();
            let mut edges = Vec::with_capacity(poly_3d.len());
            for i in 0..poly_3d.len() {
                let j = (i + 1) % poly_3d.len();
                if poly_3d[i].distance(poly_3d[j]) < gfd_cad_geom::LINEAR_TOL { continue; }
                let e = push_line_edge(arena, poly_3d[i], poly_3d[j], verts[i], verts[j])?;
                edges.push((e, Orientation::Forward));
            }
            if edges.is_empty() { return Ok(()); }
            let wire = arena.push(Shape::Wire { edges });
            // Cap normal: +sign → pointing towards negative sweep direction
            // (opposite for end cap). Use the Z-axis cross (azimuth direction).
            let normal = Direction3 { x: -sa * normal_sign, y: ca * normal_sign, z: 0.0 };
            let plane = Plane::new(poly_3d[0], normal, Direction3::Z);
            let face = arena.push(Shape::Face {
                surface: SurfaceGeom::Plane(plane),
                wires: vec![wire],
                orient: Orientation::Forward,
            });
            faces.push((face, Orientation::Forward));
            Ok(())
        };
        build_cap(arena, 0.0, 1.0)?;
        build_cap(arena, angle_rad, -1.0)?;
    }

    let shell = arena.push(Shape::Shell { faces });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

/// Revolve a 2D profile (given as (r, z) pairs, r ≥ 0) about the Z axis.
/// `angular_steps` must be ≥ 3.
pub fn revolve_profile_z(
    arena: &mut ShapeArena,
    profile_rz: &[(f64, f64)],
    angular_steps: usize,
) -> TopoResult<ShapeId> {
    if profile_rz.len() < 2 {
        return Err(TopoError::Geom(gfd_cad_geom::GeomError::Degenerate(
            "revolve profile needs >= 2 points",
        )));
    }
    if angular_steps < 3 {
        return Err(TopoError::Geom(gfd_cad_geom::GeomError::Degenerate(
            "revolve angular_steps must be >= 3",
        )));
    }
    let mut faces: Vec<(ShapeId, Orientation)> = Vec::with_capacity(angular_steps * profile_rz.len());
    let two_pi = std::f64::consts::TAU;
    for step in 0..angular_steps {
        let a0 = two_pi * step as f64 / angular_steps as f64;
        let a1 = two_pi * (step + 1) as f64 / angular_steps as f64;
        for i in 0..profile_rz.len() - 1 {
            let (r0, z0) = profile_rz[i];
            let (r1, z1) = profile_rz[i + 1];
            let p_a0 = Point3::new(r0 * a0.cos(), r0 * a0.sin(), z0);
            let p_b0 = Point3::new(r1 * a0.cos(), r1 * a0.sin(), z1);
            let p_b1 = Point3::new(r1 * a1.cos(), r1 * a1.sin(), z1);
            let p_a1 = Point3::new(r0 * a1.cos(), r0 * a1.sin(), z0);
            // Register 4 vertices so downstream measurements (bbox, volume,
            // area) can traverse the shape. A proper wire with line edges
            // ships later.
            let v0 = arena.push(Shape::vertex(p_a0));
            let v1 = arena.push(Shape::vertex(p_b0));
            let v2 = arena.push(Shape::vertex(p_b1));
            let v3 = arena.push(Shape::vertex(p_a1));
            let e0 = push_line_edge(arena, p_a0, p_b0, v0, v1)?;
            let e1 = push_line_edge(arena, p_b0, p_b1, v1, v2)?;
            let e2 = push_line_edge(arena, p_b1, p_a1, v2, v3)?;
            let e3 = push_line_edge(arena, p_a1, p_a0, v3, v0)?;
            let wire = arena.push(Shape::Wire {
                edges: vec![
                    (e0, Orientation::Forward),
                    (e1, Orientation::Forward),
                    (e2, Orientation::Forward),
                    (e3, Orientation::Forward),
                ],
            });
            let plane = Plane::new(p_a0, Direction3::Z, Direction3::X);
            let face = arena.push(Shape::Face {
                surface: SurfaceGeom::Plane(plane),
                wires: vec![wire],
                orient: Orientation::Forward,
            });
            faces.push((face, Orientation::Forward));
        }
    }
    let shell = arena.push(Shape::Shell { faces });
    Ok(arena.push(Shape::Solid { shells: vec![shell] }))
}

fn push_line_edge(
    arena: &mut ShapeArena,
    a: Point3,
    b: Point3,
    va: ShapeId,
    vb: ShapeId,
) -> TopoResult<ShapeId> {
    use gfd_cad_geom::curve::Line;
    use gfd_cad_topo::shape::CurveGeom;
    // Skip degenerate edges (when r0 == 0 the swept "edge" collapses onto
    // the axis). The heal/check layer will complain but bbox/volume work.
    if a.distance(b) < gfd_cad_geom::LINEAR_TOL {
        let edge = arena.push(Shape::Edge {
            curve: CurveGeom::Line(Line::from_points(
                Point3::new(a.x, a.y, a.z),
                Point3::new(a.x + 1.0e-6, a.y, a.z),
            ).map_err(TopoError::from)?),
            vertices: [va, vb],
            orient: Orientation::Forward,
        });
        return Ok(edge);
    }
    let line = Line::from_points(a, b).map_err(TopoError::from)?;
    Ok(arena.push(Shape::Edge {
        curve: CurveGeom::Line(line),
        vertices: [va, vb],
        orient: Orientation::Forward,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_cad_topo::{collect_by_kind, ShapeKind};

    #[test]
    fn partial_revolve_180deg_with_caps() {
        let mut a = ShapeArena::new();
        let id = revolve_profile_z_partial(
            &mut a,
            &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            8,
            std::f64::consts::PI,
        ).unwrap();
        let faces = gfd_cad_topo::collect_by_kind(&a, id, gfd_cad_topo::ShapeKind::Face);
        // 3 profile edges × 8 angular steps = 24 lateral + 2 end caps = 26.
        assert_eq!(faces.len(), 26);
    }

    #[test]
    fn revolve_triangle_creates_faces() {
        let mut a = ShapeArena::new();
        // A right triangle profile (radius, z): (0,0) → (1,0) → (0,1) forms a cone.
        let id = revolve_profile_z(&mut a, &[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)], 8).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        // 2 edges × 8 angular steps = 16 lateral quads.
        assert_eq!(faces.len(), 2 * 8);
    }
}
