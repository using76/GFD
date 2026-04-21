//! Rigid + non-uniform affine transforms: translate, rotate (axis + angle),
//! scale (per-axis). All operate by deep-cloning the shape tree through a
//! user-provided point-mapping closure, preserving topology structure and
//! rewiring shared sub-shapes via a remap table.

use gfd_cad_geom::{curve::Line, surface::Plane, Direction3, Point3};
use gfd_cad_topo::{
    shape::{CurveGeom, Shape, SurfaceGeom},
    ShapeArena, ShapeId, TopoError, TopoResult,
};

/// Clone `id` through an arbitrary point transform `xform` and direction
/// transform `dir_xform`. Returns the id of the transformed shape.
pub fn transform_shape(
    arena: &mut ShapeArena,
    id: ShapeId,
    xform: impl Fn(Point3) -> Point3,
    dir_xform: impl Fn(Direction3) -> Direction3,
) -> TopoResult<ShapeId> {
    let mut remap: std::collections::HashMap<u32, ShapeId> = std::collections::HashMap::new();
    clone_rec(arena, id, &mut remap, &xform, &dir_xform)
}

fn clone_rec<F, G>(
    arena: &mut ShapeArena,
    id: ShapeId,
    remap: &mut std::collections::HashMap<u32, ShapeId>,
    xform: &F,
    dir_xform: &G,
) -> TopoResult<ShapeId>
where
    F: Fn(Point3) -> Point3,
    G: Fn(Direction3) -> Direction3,
{
    if let Some(&new_id) = remap.get(&id.0) { return Ok(new_id); }
    let cloned: Shape = match arena.get(id)?.clone() {
        Shape::Vertex { point } => Shape::Vertex { point: xform(point) },
        Shape::Edge { curve, vertices, orient } => {
            let nv = [
                clone_rec(arena, vertices[0], remap, xform, dir_xform)?,
                clone_rec(arena, vertices[1], remap, xform, dir_xform)?,
            ];
            let nc = match curve {
                CurveGeom::Line(l) => {
                    let a = xform(l.origin);
                    let b = xform(Point3::new(
                        l.origin.x + l.direction.x * l.length,
                        l.origin.y + l.direction.y * l.length,
                        l.origin.z + l.direction.z * l.length,
                    ));
                    CurveGeom::Line(Line::from_points(a, b).map_err(TopoError::from)?)
                }
                other => other,
            };
            Shape::Edge { curve: nc, vertices: nv, orient }
        }
        Shape::Wire { edges } => {
            let mut ne = Vec::with_capacity(edges.len());
            for (e, o) in edges { ne.push((clone_rec(arena, e, remap, xform, dir_xform)?, o)); }
            Shape::Wire { edges: ne }
        }
        Shape::Face { surface, wires, orient } => {
            let ns = match surface {
                SurfaceGeom::Plane(p) => SurfaceGeom::Plane(Plane::new(
                    xform(p.origin),
                    dir_xform(p.normal),
                    dir_xform(p.x_axis),
                )),
                other => other,
            };
            let mut nw = Vec::with_capacity(wires.len());
            for w in wires { nw.push(clone_rec(arena, w, remap, xform, dir_xform)?); }
            Shape::Face { surface: ns, wires: nw, orient }
        }
        Shape::Shell { faces } => {
            let mut nf = Vec::with_capacity(faces.len());
            for (f, o) in faces { nf.push((clone_rec(arena, f, remap, xform, dir_xform)?, o)); }
            Shape::Shell { faces: nf }
        }
        Shape::Solid { shells } => {
            let mut nsh = Vec::with_capacity(shells.len());
            for s in shells { nsh.push(clone_rec(arena, s, remap, xform, dir_xform)?); }
            Shape::Solid { shells: nsh }
        }
        Shape::Compound { children } => {
            let mut nc = Vec::with_capacity(children.len());
            for c in children { nc.push(clone_rec(arena, c, remap, xform, dir_xform)?); }
            Shape::Compound { children: nc }
        }
    };
    let new_id = arena.push(cloned);
    remap.insert(id.0, new_id);
    Ok(new_id)
}

pub fn translate_shape(arena: &mut ShapeArena, id: ShapeId, tx: f64, ty: f64, tz: f64) -> TopoResult<ShapeId> {
    transform_shape(
        arena, id,
        move |p| Point3::new(p.x + tx, p.y + ty, p.z + tz),
        |d| d,
    )
}

pub fn scale_shape(arena: &mut ShapeArena, id: ShapeId, sx: f64, sy: f64, sz: f64) -> TopoResult<ShapeId> {
    transform_shape(
        arena, id,
        move |p| Point3::new(p.x * sx, p.y * sy, p.z * sz),
        move |d| {
            // Directions transform by the inverse-transpose; for axis-scale
            // we just scale each component and renormalise.
            let x = d.x * sx;
            let y = d.y * sy;
            let z = d.z * sz;
            let n = (x * x + y * y + z * z).sqrt().max(f64::EPSILON);
            Direction3 { x: x / n, y: y / n, z: z / n }
        },
    )
}

/// Rotate `id` about an arbitrary axis through the origin by `angle_rad`.
/// Uses Rodrigues' rotation formula. `axis` is normalised.
pub fn rotate_shape(arena: &mut ShapeArena, id: ShapeId, axis: (f64, f64, f64), angle_rad: f64) -> TopoResult<ShapeId> {
    let norm = (axis.0 * axis.0 + axis.1 * axis.1 + axis.2 * axis.2).sqrt().max(f64::EPSILON);
    let (ax, ay, az) = (axis.0 / norm, axis.1 / norm, axis.2 / norm);
    let (s, c) = angle_rad.sin_cos();
    let oc = 1.0 - c;
    // 3×3 rotation matrix rows
    let m = [
        [c + ax*ax*oc,     ax*ay*oc - az*s,  ax*az*oc + ay*s],
        [ay*ax*oc + az*s,  c + ay*ay*oc,     ay*az*oc - ax*s],
        [az*ax*oc - ay*s,  az*ay*oc + ax*s,  c + az*az*oc    ],
    ];
    transform_shape(
        arena, id,
        move |p| Point3::new(
            m[0][0]*p.x + m[0][1]*p.y + m[0][2]*p.z,
            m[1][0]*p.x + m[1][1]*p.y + m[1][2]*p.z,
            m[2][0]*p.x + m[2][1]*p.y + m[2][2]*p.z,
        ),
        move |d| Direction3 {
            x: m[0][0]*d.x + m[0][1]*d.y + m[0][2]*d.z,
            y: m[1][0]*d.x + m[1][1]*d.y + m[1][2]*d.z,
            z: m[2][0]*d.x + m[2][1]*d.y + m[2][2]*d.z,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::box_solid;
    use gfd_cad_measure::bounding_box;

    #[test]
    fn translate_moves_bbox() {
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 2.0, 2.0, 2.0).unwrap();
        let moved = translate_shape(&mut a, id, 5.0, 0.0, 0.0).unwrap();
        let bb = bounding_box(&a, moved).unwrap();
        assert!((bb.min.x - 4.0).abs() < 1e-9);
        assert!((bb.max.x - 6.0).abs() < 1e-9);
    }

    #[test]
    fn scale_nonuniform_stretches_bbox() {
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 2.0, 2.0, 2.0).unwrap();
        let scaled = scale_shape(&mut a, id, 3.0, 1.0, 0.5).unwrap();
        let bb = bounding_box(&a, scaled).unwrap();
        assert!((bb.max.x - bb.min.x - 6.0).abs() < 1e-6);
        assert!((bb.max.y - bb.min.y - 2.0).abs() < 1e-6);
        assert!((bb.max.z - bb.min.z - 1.0).abs() < 1e-6);
    }

    #[test]
    fn rotate_90_z_swaps_x_y_extents() {
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 4.0, 2.0, 1.0).unwrap();
        let rotated = rotate_shape(&mut a, id, (0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_2).unwrap();
        let bb = bounding_box(&a, rotated).unwrap();
        // 90° about Z: X-extent (was 4) becomes Y-extent, Y (was 2) becomes X.
        assert!((bb.max.x - bb.min.x - 2.0).abs() < 1e-6, "x-extent {}", bb.max.x - bb.min.x);
        assert!((bb.max.y - bb.min.y - 4.0).abs() < 1e-6, "y-extent {}", bb.max.y - bb.min.y);
    }
}
