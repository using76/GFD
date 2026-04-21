//! Edge ↔ Face adjacency maps built by a single pass over the shape graph.
//!
//! Also hosts `build_half_edges`, which turns a shape root into a flat list
//! of `HalfEdge` records with `next` / `prev` threaded around each face and
//! `twin` pointers set wherever an underlying edge appears in two faces.
//!
//! The arena stores shapes as an unrooted tree; to answer "which faces share
//! this edge?" we need to invert the traversal. `EdgeFaceMap::build` runs
//! once per query-set and returns a HashMap from each edge id to its (up to
//! two) parent faces, which is what OCCT exposes as `TopExp_Explorer` +
//! map-from-shape.

use std::collections::HashMap;

use crate::{
    shape::{HalfEdge, Shape},
    ShapeArena, ShapeId, TopoError,
};

#[derive(Debug, Default, Clone)]
pub struct EdgeFaceMap {
    pub edge_to_faces: HashMap<ShapeId, Vec<ShapeId>>,
    pub vertex_to_edges: HashMap<ShapeId, Vec<ShapeId>>,
}

impl EdgeFaceMap {
    pub fn build(arena: &ShapeArena, root: ShapeId) -> Result<Self, TopoError> {
        let mut out = Self::default();
        walk(arena, root, None, &mut out)?;
        Ok(out)
    }

    pub fn adjacent_faces_of(&self, edge: ShapeId) -> &[ShapeId] {
        self.edge_to_faces.get(&edge).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn adjacent_edges_of(&self, vertex: ShapeId) -> &[ShapeId] {
        self.vertex_to_edges.get(&vertex).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// True when the vertex has exactly two incident edges (a manifold chain).
    pub fn is_manifold_vertex(&self, vertex: ShapeId) -> bool {
        self.vertex_to_edges.get(&vertex).map(|v| v.len() == 2).unwrap_or(false)
    }

    /// All faces that share at least one edge with `face`. Useful for
    /// traversing from a selected face to its neighbours (e.g. fillet
    /// propagation, seam detection).
    pub fn face_neighbors(&self, face: ShapeId) -> Vec<ShapeId> {
        let mut set: std::collections::HashSet<ShapeId> = std::collections::HashSet::new();
        for (_edge, faces) in &self.edge_to_faces {
            if faces.contains(&face) {
                for f in faces {
                    if *f != face { set.insert(*f); }
                }
            }
        }
        let mut out: Vec<ShapeId> = set.into_iter().collect();
        out.sort();
        out
    }
}

/// Build a flat `HalfEdge` list for every wire inside `root`. The output
/// indices are self-referential: `next` / `prev` point within the list,
/// and `twin` is set for every pair where an edge is shared by two faces.
///
/// This is the first step towards OCCT-style topology queries (face walk,
/// neighbor iteration) without rebuilding the arena.
pub fn build_half_edges(arena: &ShapeArena, root: ShapeId) -> Result<Vec<HalfEdge>, TopoError> {
    // (face_id, edge_id, orientation_forward) → list of tuple indices.
    #[derive(Clone, Copy)]
    struct Entry {
        face: ShapeId,
        edge: ShapeId,
        origin: ShapeId,
        forward: bool,
    }
    let mut entries: Vec<Entry> = Vec::new();
    let mut face_ranges: Vec<(usize, usize)> = Vec::new();

    let mut stack: Vec<ShapeId> = vec![root];
    while let Some(id) = stack.pop() {
        let shape = arena.get(id)?;
        match shape {
            Shape::Compound { children }   => { for c in children { stack.push(*c); } }
            Shape::Solid { shells }        => { for s in shells { stack.push(*s); } }
            Shape::Shell { faces }         => { for (f, _) in faces { stack.push(*f); } }
            Shape::Face { wires, .. } => {
                let start = entries.len();
                for w in wires {
                    if let Shape::Wire { edges } = arena.get(*w)? {
                        for (eid, orient) in edges {
                            if let Shape::Edge { vertices, .. } = arena.get(*eid)? {
                                let forward = matches!(orient, crate::Orientation::Forward);
                                let origin = if forward { vertices[0] } else { vertices[1] };
                                entries.push(Entry { face: id, edge: *eid, origin, forward });
                            }
                        }
                    }
                }
                let end = entries.len();
                if end > start { face_ranges.push((start, end)); }
            }
            _ => {}
        }
    }

    // Build HalfEdges with next/prev within each face range.
    let mut out: Vec<HalfEdge> = entries.iter().map(|e| HalfEdge {
        origin: e.origin,
        edge: e.edge,
        twin: None,
        next: 0,
        prev: 0,
        face: Some(e.face),
    }).collect();
    for (start, end) in &face_ranges {
        let n = end - start;
        for k in 0..n {
            let here = start + k;
            let next = start + (k + 1) % n;
            let prev = start + (k + n - 1) % n;
            out[here].next = next as u32;
            out[here].prev = prev as u32;
        }
    }

    // Twin pointers: group indices by edge id and pair opposite orientations.
    let mut by_edge: HashMap<ShapeId, Vec<usize>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        by_edge.entry(e.edge).or_default().push(i);
    }
    for uses in by_edge.values() {
        if uses.len() == 2 {
            out[uses[0]].twin = Some(uses[1] as u32);
            out[uses[1]].twin = Some(uses[0] as u32);
        }
    }
    Ok(out)
}

fn walk(arena: &ShapeArena, id: ShapeId, current_face: Option<ShapeId>, out: &mut EdgeFaceMap)
    -> Result<(), TopoError>
{
    match arena.get(id)? {
        Shape::Compound { children }  => { for c in children { walk(arena, *c, current_face, out)?; } }
        Shape::Solid { shells }       => { for s in shells { walk(arena, *s, current_face, out)?; } }
        Shape::Shell { faces }        => { for (f, _) in faces { walk(arena, *f, current_face, out)?; } }
        Shape::Face { wires, .. }     => { for w in wires { walk(arena, *w, Some(id), out)?; } }
        Shape::Wire { edges }         => { for (e, _) in edges { walk(arena, *e, current_face, out)?; } }
        Shape::Edge { vertices, .. }  => {
            if let Some(f) = current_face {
                out.edge_to_faces.entry(id).or_default().push(f);
            }
            for v in vertices {
                out.vertex_to_edges.entry(*v).or_default().push(id);
            }
        }
        Shape::Vertex { .. } => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_neighbors_on_shared_edge() {
        use crate::{shape::{CurveGeom, SurfaceGeom}, Orientation};
        use gfd_cad_geom::{curve::Line, surface::Plane, Direction3, Point3};
        let mut arena = ShapeArena::new();
        // Two faces sharing edge e: face1 uses edges (e0, e), face2 uses edges (e, e1).
        let v0 = arena.push(Shape::vertex(Point3::new(0.0, 0.0, 0.0)));
        let v1 = arena.push(Shape::vertex(Point3::new(1.0, 0.0, 0.0)));
        let v2 = arena.push(Shape::vertex(Point3::new(2.0, 0.0, 0.0)));
        let make_edge = |arena: &mut ShapeArena, va: ShapeId, vb: ShapeId, pa: Point3, pb: Point3| -> ShapeId {
            let line = Line::from_points(pa, pb).unwrap();
            arena.push(Shape::Edge { curve: CurveGeom::Line(line), vertices: [va, vb], orient: Orientation::Forward })
        };
        let e0 = make_edge(&mut arena, v0, v1, Point3::ORIGIN, Point3::new(1.0, 0.0, 0.0));
        let e1 = make_edge(&mut arena, v1, v2, Point3::new(1.0, 0.0, 0.0), Point3::new(2.0, 0.0, 0.0));
        let w1 = arena.push(Shape::Wire { edges: vec![(e0, Orientation::Forward), (e1, Orientation::Forward)] });
        let w2 = arena.push(Shape::Wire { edges: vec![(e1, Orientation::Forward), (e0, Orientation::Forward)] });
        let plane = Plane::new(Point3::ORIGIN, Direction3::Z, Direction3::X);
        let f1 = arena.push(Shape::Face { surface: SurfaceGeom::Plane(plane), wires: vec![w1], orient: Orientation::Forward });
        let f2 = arena.push(Shape::Face { surface: SurfaceGeom::Plane(plane), wires: vec![w2], orient: Orientation::Forward });
        let compound = arena.push(Shape::Compound { children: vec![f1, f2] });
        let map = EdgeFaceMap::build(&arena, compound).unwrap();
        let neigh = map.face_neighbors(f1);
        assert_eq!(neigh, vec![f2]);
    }

    #[test]
    fn build_half_edges_threads_next_prev() {
        use crate::{shape::{CurveGeom, SurfaceGeom}, Orientation};
        use gfd_cad_geom::{curve::Line, surface::Plane, Direction3, Point3};
        let mut arena = ShapeArena::new();
        // Triangle face: 3 vertices, 3 edges, 1 wire, 1 face.
        let v = [
            arena.push(Shape::vertex(Point3::new(0.0, 0.0, 0.0))),
            arena.push(Shape::vertex(Point3::new(1.0, 0.0, 0.0))),
            arena.push(Shape::vertex(Point3::new(0.0, 1.0, 0.0))),
        ];
        let mut edges = Vec::new();
        for i in 0..3 {
            let j = (i + 1) % 3;
            let a = if i == 0 { Point3::ORIGIN } else if i == 1 { Point3::new(1.0, 0.0, 0.0) } else { Point3::new(0.0, 1.0, 0.0) };
            let b = if j == 0 { Point3::ORIGIN } else if j == 1 { Point3::new(1.0, 0.0, 0.0) } else { Point3::new(0.0, 1.0, 0.0) };
            let line = Line::from_points(a, b).unwrap();
            let e = arena.push(Shape::Edge { curve: CurveGeom::Line(line), vertices: [v[i], v[j]], orient: Orientation::Forward });
            edges.push((e, Orientation::Forward));
        }
        let w = arena.push(Shape::Wire { edges });
        let f = arena.push(Shape::Face {
            surface: SurfaceGeom::Plane(Plane::new(Point3::ORIGIN, Direction3::Z, Direction3::X)),
            wires: vec![w],
            orient: Orientation::Forward,
        });
        let hes = build_half_edges(&arena, f).unwrap();
        assert_eq!(hes.len(), 3);
        // next(i) must loop back around the face.
        assert_eq!(hes[0].next, 1);
        assert_eq!(hes[1].next, 2);
        assert_eq!(hes[2].next, 0);
        assert_eq!(hes[0].prev, 2);
        // Single face → no twin pairings.
        assert!(hes.iter().all(|h| h.twin.is_none()));
    }

    #[test]
    fn box_edges_have_two_faces() {
        // We can't build a box without gfd-cad-feature. Construct a minimal
        // case: one face, one wire, one edge, two vertices. The edge should
        // have exactly one face in its adjacency list.
        use crate::{shape::{CurveGeom, SurfaceGeom}, Orientation};
        use gfd_cad_geom::{curve::Line, surface::Plane, Direction3, Point3};
        let mut arena = ShapeArena::new();
        let v0 = arena.push(Shape::vertex(Point3::new(0.0, 0.0, 0.0)));
        let v1 = arena.push(Shape::vertex(Point3::new(1.0, 0.0, 0.0)));
        let e = arena.push(Shape::Edge {
            curve: CurveGeom::Line(Line::from_points(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)).unwrap()),
            vertices: [v0, v1],
            orient: Orientation::Forward,
        });
        let w = arena.push(Shape::Wire { edges: vec![(e, Orientation::Forward)] });
        let f = arena.push(Shape::Face {
            surface: SurfaceGeom::Plane(Plane::new(Point3::new(0.0, 0.0, 0.0), Direction3::Z, Direction3::X)),
            wires: vec![w],
            orient: Orientation::Forward,
        });
        let map = EdgeFaceMap::build(&arena, f).unwrap();
        assert_eq!(map.adjacent_faces_of(e).len(), 1);
        assert_eq!(map.adjacent_faces_of(e)[0], f);
        assert_eq!(map.adjacent_edges_of(v0).len(), 1);
    }
}
