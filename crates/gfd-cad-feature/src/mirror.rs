//! Mirror feature — duplicate a shape by reflecting through an axis plane.
//!
//! Only vertex positions change; edges / wires / faces / shells keep their
//! topological structure. The reflection inverts orientation of planar
//! faces automatically when the plane normal is flipped; for the common XY
//! / YZ / XZ mirror planes this is straightforward.

use gfd_cad_geom::{curve::Line, surface::Plane, Direction3, Point3};
use gfd_cad_topo::{
    shape::{CurveGeom, Shape, SurfaceGeom},
    Orientation, ShapeArena, ShapeId, ShapeKind, TopoError, TopoResult,
};

#[derive(Debug, Clone, Copy)]
pub enum MirrorPlane { XY, YZ, XZ }

/// Mirror all vertices reachable from `id` through the given axis plane and
/// produce a new shape with the same topology structure. Returns the id of
/// the mirrored top-level shape.
pub fn mirror_shape(arena: &mut ShapeArena, id: ShapeId, plane: MirrorPlane) -> TopoResult<ShapeId> {
    let reflect = |p: Point3| -> Point3 {
        match plane {
            MirrorPlane::XY => Point3::new(p.x, p.y, -p.z),
            MirrorPlane::YZ => Point3::new(-p.x, p.y, p.z),
            MirrorPlane::XZ => Point3::new(p.x, -p.y, p.z),
        }
    };
    let reflect_dir = |d: Direction3| -> Direction3 {
        match plane {
            MirrorPlane::XY => Direction3 { x: d.x,  y: d.y,  z: -d.z },
            MirrorPlane::YZ => Direction3 { x: -d.x, y: d.y,  z: d.z  },
            MirrorPlane::XZ => Direction3 { x: d.x,  y: -d.y, z: d.z  },
        }
    };

    // Map from original id → new id, so shared sub-shapes dedupe correctly.
    let mut remap: std::collections::HashMap<u32, ShapeId> = std::collections::HashMap::new();

    fn clone_shape(
        arena: &mut ShapeArena,
        id: ShapeId,
        remap: &mut std::collections::HashMap<u32, ShapeId>,
        reflect: &impl Fn(Point3) -> Point3,
        reflect_dir: &impl Fn(Direction3) -> Direction3,
    ) -> TopoResult<ShapeId> {
        if let Some(&new_id) = remap.get(&id.0) { return Ok(new_id); }
        let cloned: Shape = match arena.get(id)?.clone() {
            Shape::Vertex { point } => Shape::Vertex { point: reflect(point) },
            Shape::Edge { curve, vertices, orient } => {
                let new_verts = [
                    clone_shape(arena, vertices[0], remap, reflect, reflect_dir)?,
                    clone_shape(arena, vertices[1], remap, reflect, reflect_dir)?,
                ];
                let new_curve = match curve {
                    CurveGeom::Line(l) => {
                        let a = reflect(l.origin);
                        let b = reflect(Point3::new(
                            l.origin.x + l.direction.x * l.length,
                            l.origin.y + l.direction.y * l.length,
                            l.origin.z + l.direction.z * l.length,
                        ));
                        CurveGeom::Line(Line::from_points(a, b).map_err(TopoError::from)?)
                    }
                    other => other, // circles/bsplines unchanged geom-wise (iter 41 limitation)
                };
                Shape::Edge { curve: new_curve, vertices: new_verts, orient: orient.reverse() }
            }
            Shape::Wire { edges } => {
                let mut new_edges = Vec::with_capacity(edges.len());
                for (eid, orient) in edges.iter() {
                    let new_eid = clone_shape(arena, *eid, remap, reflect, reflect_dir)?;
                    new_edges.push((new_eid, orient.reverse()));
                }
                new_edges.reverse();
                Shape::Wire { edges: new_edges }
            }
            Shape::Face { surface, wires, orient } => {
                let new_surface = match surface {
                    SurfaceGeom::Plane(p) => SurfaceGeom::Plane(Plane::new(
                        reflect(p.origin),
                        reflect_dir(p.normal),
                        reflect_dir(p.x_axis),
                    )),
                    other => other,
                };
                let mut new_wires = Vec::with_capacity(wires.len());
                for w in wires { new_wires.push(clone_shape(arena, w, remap, reflect, reflect_dir)?); }
                Shape::Face { surface: new_surface, wires: new_wires, orient: orient.reverse() }
            }
            Shape::Shell { faces } => {
                let mut new_faces = Vec::with_capacity(faces.len());
                for (f, orient) in faces.iter() {
                    let nf = clone_shape(arena, *f, remap, reflect, reflect_dir)?;
                    new_faces.push((nf, orient.reverse()));
                }
                Shape::Shell { faces: new_faces }
            }
            Shape::Solid { shells } => {
                let mut new_shells = Vec::with_capacity(shells.len());
                for s in shells.iter() { new_shells.push(clone_shape(arena, *s, remap, reflect, reflect_dir)?); }
                Shape::Solid { shells: new_shells }
            }
            Shape::Compound { children } => {
                let mut new_children = Vec::with_capacity(children.len());
                for c in children.iter() { new_children.push(clone_shape(arena, *c, remap, reflect, reflect_dir)?); }
                Shape::Compound { children: new_children }
            }
        };
        let new_id = arena.push(cloned);
        remap.insert(id.0, new_id);
        Ok(new_id)
    }

    clone_shape(arena, id, &mut remap, &reflect, &reflect_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pad_polygon_xy;
    use gfd_cad_topo::collect_by_kind;

    #[test]
    fn mirror_pad_preserves_face_count() {
        let mut a = ShapeArena::new();
        let pad = pad_polygon_xy(&mut a, &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], 0.5).unwrap();
        let mirrored = mirror_shape(&mut a, pad, MirrorPlane::XY).unwrap();
        let faces = collect_by_kind(&a, mirrored, ShapeKind::Face);
        assert_eq!(faces.len(), 6); // 4 lateral + 2 caps
    }

    #[test]
    fn mirror_reflects_vertex_positions() {
        let mut a = ShapeArena::new();
        let v = a.push(Shape::Vertex { point: Point3::new(1.0, 2.0, 3.0) });
        let m = mirror_shape(&mut a, v, MirrorPlane::XY).unwrap();
        match a.get(m).unwrap() {
            Shape::Vertex { point } => {
                assert!((point.x - 1.0).abs() < 1e-9);
                assert!((point.y - 2.0).abs() < 1e-9);
                assert!((point.z + 3.0).abs() < 1e-9);
            }
            _ => panic!("expected vertex"),
        }
    }
}
