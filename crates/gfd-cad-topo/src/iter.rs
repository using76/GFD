//! Traversal helpers for walking the shape hierarchy.

use crate::{shape::Shape, ShapeArena, ShapeId, ShapeKind};

/// Collect every shape id reachable from `root` whose kind matches `kind`.
///
/// Recursive descent; a single shape can appear more than once if the graph
/// contains shared sub-shapes. De-duplicate at the call site if needed.
pub fn collect_by_kind(arena: &ShapeArena, root: ShapeId, kind: ShapeKind) -> Vec<ShapeId> {
    let mut out = Vec::new();
    walk(arena, root, kind, &mut out);
    out
}

fn walk(arena: &ShapeArena, id: ShapeId, kind: ShapeKind, out: &mut Vec<ShapeId>) {
    let Ok(shape) = arena.get(id) else { return; };
    if shape.kind() == kind {
        out.push(id);
    }
    match shape {
        Shape::Compound { children } => {
            for c in children { walk(arena, *c, kind, out); }
        }
        Shape::Solid { shells } => {
            for s in shells { walk(arena, *s, kind, out); }
        }
        Shape::Shell { faces } => {
            for (f, _) in faces { walk(arena, *f, kind, out); }
        }
        Shape::Face { wires, .. } => {
            for w in wires { walk(arena, *w, kind, out); }
        }
        Shape::Wire { edges } => {
            for (e, _) in edges { walk(arena, *e, kind, out); }
        }
        Shape::Edge { vertices, .. } => {
            for v in vertices { walk(arena, *v, kind, out); }
        }
        Shape::Vertex { .. } => {}
    }
}
