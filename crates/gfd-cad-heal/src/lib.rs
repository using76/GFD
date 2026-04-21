//! gfd-cad-heal — shape healing routines (OCCT ShapeFix equivalent).
//!
//! Iteration 6: validity checker that surfaces obvious topology problems
//! (missing vertex, degenerate edge, empty wire, self-referencing compound).
//! Sewing / fixing / remove-small operations ship in later iterations.

use gfd_cad_geom::LINEAR_TOL;
use gfd_cad_topo::{Shape, ShapeArena, ShapeId, TopoError};

#[derive(Debug, thiserror::Error)]
pub enum HealError {
    #[error("healing operation not yet implemented")]
    Unimplemented,
    #[error(transparent)]
    Topo(#[from] TopoError),
}

pub type HealResult<T> = Result<T, HealError>;

#[derive(Debug, Default, Clone)]
pub struct HealOptions {
    pub tolerance: f64,
    pub sew_faces: bool,
    pub fix_wires: bool,
    pub remove_small_edges: bool,
    pub unify_tolerances: bool,
    pub remove_duplicate_faces: bool,
}

#[derive(Debug, Clone)]
pub struct ValidityIssue {
    pub shape_id: u32,
    pub kind: &'static str,
    pub detail: String,
}

/// Walk the shape tree rooted at `id` and return every structural issue found.
///
/// Iter 6 checks:
/// - degenerate edge (endpoints coincident within `LINEAR_TOL`)
/// - empty wire
/// - shell with zero faces
/// - solid with zero shells
/// - compound referencing itself
/// Count every shape kind reachable from `id`. Useful for the Repair tab
/// summary bar (e.g. "12 faces, 36 edges, 24 vertices").
pub fn shape_stats(arena: &ShapeArena, id: ShapeId) -> HealResult<ShapeStats> {
    use gfd_cad_topo::{collect_by_kind, ShapeKind};
    Ok(ShapeStats {
        vertices: collect_by_kind(arena, id, ShapeKind::Vertex).len(),
        edges:    collect_by_kind(arena, id, ShapeKind::Edge).len(),
        wires:    collect_by_kind(arena, id, ShapeKind::Wire).len(),
        faces:    collect_by_kind(arena, id, ShapeKind::Face).len(),
        shells:   collect_by_kind(arena, id, ShapeKind::Shell).len(),
        solids:   collect_by_kind(arena, id, ShapeKind::Solid).len(),
    })
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ShapeStats {
    pub vertices: usize,
    pub edges: usize,
    pub wires: usize,
    pub faces: usize,
    pub shells: usize,
    pub solids: usize,
}

pub fn check_validity(arena: &ShapeArena, id: ShapeId) -> HealResult<Vec<ValidityIssue>> {
    let mut issues = Vec::new();
    walk(arena, id, &mut issues)?;
    detect_non_manifold_edges(arena, id, &mut issues)?;
    detect_duplicate_faces(arena, id, &mut issues)?;
    Ok(issues)
}

/// Flag every edge that is referenced by 3 or more faces. Two is the normal
/// case for a closed manifold; a lone edge belonging to one face is a
/// boundary (not a defect). This uses `EdgeFaceMap` for O(n) walk.
fn detect_non_manifold_edges(
    arena: &ShapeArena,
    root: ShapeId,
    issues: &mut Vec<ValidityIssue>,
) -> HealResult<()> {
    use gfd_cad_topo::adjacency::EdgeFaceMap;
    let map = match EdgeFaceMap::build(arena, root) {
        Ok(m) => m,
        Err(_) => return Ok(()),
    };
    for (edge, faces) in &map.edge_to_faces {
        if faces.len() > 2 {
            issues.push(ValidityIssue {
                shape_id: edge.0,
                kind: "non_manifold_edge",
                detail: format!("edge {:?} shared by {} faces", edge, faces.len()),
            });
        }
    }
    Ok(())
}

/// Two faces with identical wire sets (order-independent) are a topology
/// defect: either one should be removed or they should be collapsed. We
/// sort wire ids per face and compare.
fn detect_duplicate_faces(
    arena: &ShapeArena,
    root: ShapeId,
    issues: &mut Vec<ValidityIssue>,
) -> HealResult<()> {
    use gfd_cad_topo::{collect_by_kind, ShapeKind};
    let faces = collect_by_kind(arena, root, ShapeKind::Face);
    let mut seen: std::collections::HashMap<Vec<u32>, ShapeId> = std::collections::HashMap::new();
    for fid in &faces {
        let key = match arena.get(*fid)? {
            Shape::Face { wires, .. } => {
                let mut k: Vec<u32> = wires.iter().map(|w| w.0).collect();
                k.sort();
                k
            }
            _ => continue,
        };
        if key.is_empty() { continue; }
        if let Some(prev) = seen.get(&key) {
            issues.push(ValidityIssue {
                shape_id: fid.0,
                kind: "duplicate_face",
                detail: format!("face {:?} shares wire set with face {:?}", fid, prev),
            });
        } else {
            seen.insert(key, *fid);
        }
    }
    Ok(())
}

fn walk(arena: &ShapeArena, id: ShapeId, issues: &mut Vec<ValidityIssue>) -> HealResult<()> {
    match arena.get(id)? {
        Shape::Vertex { .. } => {}
        Shape::Edge { vertices, .. } => {
            let v0 = arena.get(vertices[0])?;
            let v1 = arena.get(vertices[1])?;
            if let (Shape::Vertex { point: p0 }, Shape::Vertex { point: p1 }) = (v0, v1) {
                if p0.distance(*p1) < LINEAR_TOL {
                    issues.push(ValidityIssue {
                        shape_id: id.0,
                        kind: "degenerate_edge",
                        detail: format!("edge {:?} has coincident endpoints", id),
                    });
                }
            }
            for v in vertices { walk(arena, *v, issues)?; }
        }
        Shape::Wire { edges } => {
            if edges.is_empty() {
                issues.push(ValidityIssue {
                    shape_id: id.0,
                    kind: "empty_wire",
                    detail: format!("wire {:?} has no edges", id),
                });
            }
            for (e, _) in edges { walk(arena, *e, issues)?; }
        }
        Shape::Face { wires, .. } => {
            // Faces without wires are legal for closed surfaces (sphere/torus).
            for w in wires { walk(arena, *w, issues)?; }
        }
        Shape::Shell { faces } => {
            if faces.is_empty() {
                issues.push(ValidityIssue {
                    shape_id: id.0,
                    kind: "empty_shell",
                    detail: format!("shell {:?} has no faces", id),
                });
            }
            for (f, _) in faces { walk(arena, *f, issues)?; }
        }
        Shape::Solid { shells } => {
            if shells.is_empty() {
                issues.push(ValidityIssue {
                    shape_id: id.0,
                    kind: "empty_solid",
                    detail: format!("solid {:?} has no shells", id),
                });
            }
            for s in shells { walk(arena, *s, issues)?; }
        }
        Shape::Compound { children } => {
            for c in children {
                if *c == id {
                    issues.push(ValidityIssue {
                        shape_id: id.0,
                        kind: "self_reference",
                        detail: format!("compound {:?} references itself", id),
                    });
                    continue;
                }
                walk(arena, *c, issues)?;
            }
        }
    }
    Ok(())
}

/// Apply a sequence of non-destructive heal passes and return a log of
/// what changed. Iteration 12 implements:
/// - `remove_small_edges`: strip edges whose endpoints are closer than
///   `options.tolerance`; the parent wire drops the entry.
/// - `unify_tolerances`: no-op placeholder (kept to preserve the API shape).
///
/// The routine walks the shape graph in a single pass and logs one entry
/// per arena mutation.
pub fn fix_shape(arena: &mut ShapeArena, id: ShapeId, opts: &HealOptions) -> HealResult<Vec<String>> {
    let tol = if opts.tolerance > 0.0 { opts.tolerance } else { gfd_cad_geom::LINEAR_TOL };
    let mut log: Vec<String> = Vec::new();

    if opts.sew_faces {
        let merged = sew_vertices(arena, id, tol)?;
        if merged > 0 { log.push(format!("sewed {} coincident vertex pair(s) (tol={})", merged, tol)); }
        let deduped = dedup_edges(arena, id)?;
        if deduped > 0 { log.push(format!("deduplicated {} duplicate edge ref(s)", deduped)); }
    }
    if opts.fix_wires {
        let closed = close_open_wires(arena, id, tol)?;
        if closed > 0 { log.push(format!("closed {} open wire gap(s) (tol={})", closed, tol)); }
    }
    if opts.remove_small_edges {
        let removed = remove_small_edges_in(arena, id, tol)?;
        if removed > 0 {
            log.push(format!("removed {} small edge(s) (tol={})", removed, tol));
        }
    }
    if opts.remove_duplicate_faces {
        let removed = remove_duplicate_faces(arena, id)?;
        if removed > 0 {
            log.push(format!("removed {} duplicate face ref(s) from shells", removed));
        }
    }

    let remaining = check_validity(arena, id)?;
    log.push(format!("post-fix validity: {} issue(s) remain", remaining.len()));
    Ok(log)
}

/// Deduplicate coincident vertices: for every vertex find others within
/// `tol` and rewrite all edge vertex references to the earliest id. Returns
/// the number of edge-side rewrites.
/// Walk every wire and, for each pair of open endpoints that lie within
/// `tol`, insert a linear closing edge. Returns the number of gaps closed.
fn close_open_wires(arena: &mut ShapeArena, id: ShapeId, tol: f64) -> HealResult<usize> {
    use gfd_cad_geom::{curve::Line, Point3};
    use gfd_cad_topo::{shape::CurveGeom, Orientation};
    let wires = collect_all_wires(arena, id)?;
    let mut closed = 0usize;
    for wid in wires {
        // Collect current edge list.
        let entries: Vec<(ShapeId, Orientation)> = match arena.get(wid)? {
            Shape::Wire { edges } => edges.clone(),
            _ => continue,
        };
        if entries.len() < 2 { continue; }
        // Find overall start vertex and end vertex.
        let first = &entries[0];
        let last = &entries[entries.len() - 1];
        let first_start = match arena.get(first.0)? {
            Shape::Edge { vertices, .. } => {
                if matches!(first.1, Orientation::Forward) { vertices[0] } else { vertices[1] }
            }
            _ => continue,
        };
        let last_end = match arena.get(last.0)? {
            Shape::Edge { vertices, .. } => {
                if matches!(last.1, Orientation::Forward) { vertices[1] } else { vertices[0] }
            }
            _ => continue,
        };
        if first_start == last_end { continue; } // already closed
        let p_start = match arena.get(first_start)? {
            Shape::Vertex { point } => *point,
            _ => continue,
        };
        let p_end = match arena.get(last_end)? {
            Shape::Vertex { point } => *point,
            _ => continue,
        };
        let d = p_start.distance(p_end);
        if d > 0.0 && d < tol {
            // Already within tolerance — just rewire last edge's end vertex
            // to the wire's start vertex. No new edge needed.
            if let Shape::Edge { vertices, .. } = arena.get_mut(last.0)? {
                if matches!(last.1, Orientation::Forward) {
                    vertices[1] = first_start;
                } else {
                    vertices[0] = first_start;
                }
            }
            closed += 1;
        } else if d >= tol && d < tol * 10.0 {
            // Slight gap — insert a bridging edge.
            let line = match Line::from_points(p_end, p_start) {
                Ok(l) => l,
                Err(_) => continue,
            };
            let new_edge = arena.push(Shape::Edge {
                curve: CurveGeom::Line(line),
                vertices: [last_end, first_start],
                orient: Orientation::Forward,
            });
            let mut new_entries = entries.clone();
            new_entries.push((new_edge, Orientation::Forward));
            if let Shape::Wire { edges } = arena.get_mut(wid)? {
                *edges = new_entries;
            }
            closed += 1;
            let _ = Point3::ORIGIN; // silence unused import when line construction fails
        }
    }
    Ok(closed)
}

/// After `sew_vertices`, two distinct edges may reference the same pair of
/// canonical vertices. This pass finds such duplicates and rewrites wire
/// edge references to point at the earliest edge id. Returns the number of
/// rewrites performed (not the number of edges collapsed).
fn dedup_edges(arena: &mut ShapeArena, id: ShapeId) -> HealResult<usize> {
    use gfd_cad_topo::ShapeKind;
    let edge_ids = gfd_cad_topo::collect_by_kind(arena, id, ShapeKind::Edge);
    // Map from sorted (v_min, v_max) pair to the canonical edge id.
    let mut canonical: std::collections::HashMap<(u32, u32), ShapeId> = std::collections::HashMap::new();
    let mut remap: std::collections::HashMap<u32, ShapeId> = std::collections::HashMap::new();
    for eid in &edge_ids {
        let pair = match arena.get(*eid)? {
            Shape::Edge { vertices, .. } => {
                let a = vertices[0].0;
                let b = vertices[1].0;
                if a < b { (a, b) } else { (b, a) }
            }
            _ => continue,
        };
        let canon = *canonical.entry(pair).or_insert(*eid);
        if canon != *eid {
            remap.insert(eid.0, canon);
        }
    }
    if remap.is_empty() { return Ok(0); }
    // Rewrite every wire's edge references.
    let wire_ids = collect_all_wires(arena, id)?;
    let mut rewrites = 0usize;
    for wid in wire_ids {
        if let Ok(Shape::Wire { edges }) = arena.get(wid) {
            let mut changed = false;
            let new_edges: Vec<_> = edges.iter().map(|(eid, orient)| {
                if let Some(canon) = remap.get(&eid.0) {
                    changed = true;
                    rewrites += 1;
                    (*canon, *orient)
                } else {
                    (*eid, *orient)
                }
            }).collect();
            if changed {
                if let Shape::Wire { edges } = arena.get_mut(wid)? {
                    *edges = new_edges;
                }
            }
        }
    }
    Ok(rewrites)
}

fn sew_vertices(arena: &mut ShapeArena, id: ShapeId, tol: f64) -> HealResult<usize> {
    use gfd_cad_topo::ShapeKind;
    let vertex_ids = gfd_cad_topo::collect_by_kind(arena, id, ShapeKind::Vertex);
    let mut canonical: std::collections::HashMap<u32, ShapeId> = std::collections::HashMap::new();
    let mut positions: Vec<(ShapeId, gfd_cad_geom::Point3)> = Vec::with_capacity(vertex_ids.len());
    for vid in &vertex_ids {
        if let Shape::Vertex { point } = arena.get(*vid)? {
            positions.push((*vid, *point));
        }
    }
    for (i, (vi, pi)) in positions.iter().enumerate() {
        if canonical.contains_key(&vi.0) { continue; }
        canonical.insert(vi.0, *vi);
        for (vj, pj) in &positions[i + 1..] {
            if canonical.contains_key(&vj.0) { continue; }
            if pi.distance(*pj) < tol {
                canonical.insert(vj.0, *vi);
            }
        }
    }
    // Rewrite edges.
    let edge_ids = gfd_cad_topo::collect_by_kind(arena, id, ShapeKind::Edge);
    let mut rewrites = 0usize;
    for eid in edge_ids {
        let mut new_vs: Option<[ShapeId; 2]> = None;
        if let Shape::Edge { vertices, .. } = arena.get(eid)? {
            let new_a = canonical.get(&vertices[0].0).copied().unwrap_or(vertices[0]);
            let new_b = canonical.get(&vertices[1].0).copied().unwrap_or(vertices[1]);
            if new_a != vertices[0] || new_b != vertices[1] {
                rewrites += (new_a != vertices[0]) as usize + (new_b != vertices[1]) as usize;
                new_vs = Some([new_a, new_b]);
            }
        }
        if let Some(nv) = new_vs {
            if let Shape::Edge { vertices, .. } = arena.get_mut(eid)? {
                *vertices = nv;
            }
        }
    }
    Ok(rewrites)
}

/// For every pair of faces with identical wire sets, rewrite shell face
/// references so only the first (canonical) face remains. Orientation of the
/// dropped entry is discarded. Returns the number of shell entries dropped.
fn remove_duplicate_faces(arena: &mut ShapeArena, id: ShapeId) -> HealResult<usize> {
    use gfd_cad_topo::{collect_by_kind, ShapeKind};
    // Build canonical map from wire-set key → first face id.
    let faces = collect_by_kind(arena, id, ShapeKind::Face);
    let mut canonical: std::collections::HashMap<Vec<u32>, ShapeId> = std::collections::HashMap::new();
    let mut drop_face: std::collections::HashSet<ShapeId> = std::collections::HashSet::new();
    for fid in &faces {
        let key = match arena.get(*fid)? {
            Shape::Face { wires, .. } => {
                let mut k: Vec<u32> = wires.iter().map(|w| w.0).collect();
                k.sort();
                k
            }
            _ => continue,
        };
        if key.is_empty() { continue; }
        if canonical.contains_key(&key) {
            drop_face.insert(*fid);
        } else {
            canonical.insert(key, *fid);
        }
    }
    if drop_face.is_empty() { return Ok(0); }
    // Walk every shell and drop entries referencing flagged faces.
    let shells = collect_by_kind(arena, id, ShapeKind::Shell);
    let mut removed = 0usize;
    for sid in shells {
        if let Ok(Shape::Shell { faces }) = arena.get(sid) {
            let len_before = faces.len();
            let filtered: Vec<_> = faces.iter()
                .filter(|(fid, _)| !drop_face.contains(fid))
                .cloned()
                .collect();
            if filtered.len() != len_before {
                removed += len_before - filtered.len();
                if let Shape::Shell { faces } = arena.get_mut(sid)? {
                    *faces = filtered;
                }
            }
        }
    }
    Ok(removed)
}

fn remove_small_edges_in(arena: &mut ShapeArena, id: ShapeId, tol: f64) -> HealResult<usize> {
    let mut degenerate: Vec<ShapeId> = Vec::new();
    collect_small_edges(arena, id, tol, &mut degenerate)?;
    // Walk every wire and drop references to the flagged edges.
    let mut removed = 0usize;
    let wire_ids = collect_all_wires(arena, id)?;
    for wid in wire_ids {
        if let Ok(Shape::Wire { edges }) = arena.get(wid) {
            let len_before = edges.len();
            let filtered: Vec<_> = edges.iter().filter(|(eid, _)| !degenerate.contains(eid)).cloned().collect();
            if filtered.len() != len_before {
                removed += len_before - filtered.len();
                if let Shape::Wire { edges } = arena.get_mut(wid)? {
                    *edges = filtered;
                }
            }
        }
    }
    Ok(removed)
}

fn collect_small_edges(arena: &ShapeArena, id: ShapeId, tol: f64, out: &mut Vec<ShapeId>) -> HealResult<()> {
    match arena.get(id)? {
        Shape::Edge { vertices, .. } => {
            let a = match arena.get(vertices[0])? {
                Shape::Vertex { point } => *point,
                _ => return Ok(()),
            };
            let b = match arena.get(vertices[1])? {
                Shape::Vertex { point } => *point,
                _ => return Ok(()),
            };
            if a.distance(b) < tol {
                out.push(id);
            }
        }
        Shape::Wire { edges } => for (e, _) in edges { collect_small_edges(arena, *e, tol, out)?; }
        Shape::Face { wires, .. } => for w in wires { collect_small_edges(arena, *w, tol, out)?; }
        Shape::Shell { faces } => for (f, _) in faces { collect_small_edges(arena, *f, tol, out)?; }
        Shape::Solid { shells } => for s in shells { collect_small_edges(arena, *s, tol, out)?; }
        Shape::Compound { children } => for c in children { collect_small_edges(arena, *c, tol, out)?; }
        _ => {}
    }
    Ok(())
}

fn collect_all_wires(arena: &ShapeArena, id: ShapeId) -> HealResult<Vec<ShapeId>> {
    let mut out = Vec::new();
    fn walk(arena: &ShapeArena, id: ShapeId, out: &mut Vec<ShapeId>) -> Result<(), TopoError> {
        match arena.get(id)? {
            Shape::Wire { .. } => out.push(id),
            Shape::Face { wires, .. } => for w in wires { walk(arena, *w, out)?; }
            Shape::Shell { faces } => for (f, _) in faces { walk(arena, *f, out)?; }
            Shape::Solid { shells } => for s in shells { walk(arena, *s, out)?; }
            Shape::Compound { children } => for c in children { walk(arena, *c, out)?; }
            _ => {}
        }
        Ok(())
    }
    walk(arena, id, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_cad_geom::Point3;
    use gfd_cad_topo::{shape::CurveGeom, Orientation};
    use gfd_cad_geom::curve::Line;

    #[test]
    fn box_solid_has_no_issues() {
        use gfd_cad_feature::box_solid;
        let mut arena = ShapeArena::new();
        let id = box_solid(&mut arena, 1.0, 1.0, 1.0).unwrap();
        let issues = check_validity(&arena, id).unwrap();
        assert!(issues.is_empty(), "unexpected issues: {:?}", issues);
    }

    #[test]
    fn empty_shell_reported() {
        let mut arena = ShapeArena::new();
        let shell = arena.push(Shape::Shell { faces: vec![] });
        let issues = check_validity(&arena, shell).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind, "empty_shell");
    }

    #[test]
    fn shape_stats_on_box() {
        use gfd_cad_feature::box_solid;
        let mut arena = ShapeArena::new();
        let id = box_solid(&mut arena, 1.0, 1.0, 1.0).unwrap();
        let stats = shape_stats(&arena, id).unwrap();
        assert_eq!(stats.faces, 6);
        assert_eq!(stats.solids, 1);
        assert_eq!(stats.shells, 1);
        assert!(stats.vertices >= 8);     // 8 unique corners, but pad emits vertex-per-face
    }

    #[test]
    fn close_wires_snaps_near_coincident_endpoints() {
        use gfd_cad_geom::{curve::Line, Point3};
        use gfd_cad_topo::{shape::CurveGeom, Orientation};
        let mut arena = ShapeArena::new();
        // Two edges: (0,0,0)→(1,0,0), (1,0,0)→(0.0000001, 0, 0). The wire's
        // last endpoint is 1e-7 away from its start; close_wires should
        // rewire the last edge's end vertex to match the start vertex.
        let v0 = arena.push(Shape::vertex(Point3::new(0.0, 0.0, 0.0)));
        let v1 = arena.push(Shape::vertex(Point3::new(1.0, 0.0, 0.0)));
        let v2 = arena.push(Shape::vertex(Point3::new(0.5e-8, 0.0, 0.0)));
        let l1 = Line::from_points(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)).unwrap();
        let l2 = Line::from_points(Point3::new(1.0, 0.0, 0.0), Point3::new(0.5e-8, 0.0, 0.0)).unwrap();
        let e1 = arena.push(Shape::Edge { curve: CurveGeom::Line(l1), vertices: [v0, v1], orient: Orientation::Forward });
        let e2 = arena.push(Shape::Edge { curve: CurveGeom::Line(l2), vertices: [v1, v2], orient: Orientation::Forward });
        let w = arena.push(Shape::Wire { edges: vec![(e1, Orientation::Forward), (e2, Orientation::Forward)] });
        let opts = HealOptions { tolerance: 1.0e-6, fix_wires: true, ..Default::default() };
        let log = fix_shape(&mut arena, w, &opts).unwrap();
        assert!(log.iter().any(|l| l.contains("closed 1")));
        // After close_wires, e2's end vertex should equal v0 (the wire's start).
        if let Shape::Edge { vertices, .. } = arena.get(e2).unwrap() {
            assert_eq!(vertices[1], v0);
        }
    }

    #[test]
    fn sew_plus_dedup_collapses_duplicate_edges() {
        use gfd_cad_geom::{curve::Line, Point3};
        use gfd_cad_topo::{shape::CurveGeom, Orientation};
        let mut arena = ShapeArena::new();
        // Two vertices at the same point → sew_vertices collapses them into
        // v0. Two edges from v0 to v_end then refer to the same pair
        // (v0, v_end), and dedup_edges should rewrite the second to the
        // first.
        let p = Point3::new(0.0, 0.0, 0.0);
        let q = Point3::new(1.0, 0.0, 0.0);
        let v0 = arena.push(Shape::vertex(p));
        let v0_dup = arena.push(Shape::vertex(p));
        let v1 = arena.push(Shape::vertex(q));
        let line_a = Line::from_points(p, q).unwrap();
        let line_b = Line::from_points(p, q).unwrap();
        let ea = arena.push(Shape::Edge { curve: CurveGeom::Line(line_a), vertices: [v0, v1], orient: Orientation::Forward });
        let eb = arena.push(Shape::Edge { curve: CurveGeom::Line(line_b), vertices: [v0_dup, v1], orient: Orientation::Forward });
        let w = arena.push(Shape::Wire { edges: vec![(ea, Orientation::Forward), (eb, Orientation::Forward)] });
        let opts = HealOptions { tolerance: 1.0e-6, sew_faces: true, ..Default::default() };
        let log = fix_shape(&mut arena, w, &opts).unwrap();
        assert!(log.iter().any(|l| l.contains("sewed")));
        assert!(log.iter().any(|l| l.contains("deduplicated")));
        // After fix, the wire's second edge ref should equal the first.
        if let Shape::Wire { edges } = arena.get(w).unwrap() {
            assert_eq!(edges[0].0, edges[1].0);
        }
    }

    #[test]
    fn sew_collapses_coincident_vertices() {
        use gfd_cad_geom::{curve::Line, Point3};
        use gfd_cad_topo::{shape::CurveGeom, Orientation};
        let mut arena = ShapeArena::new();
        // Two vertices at the exact same point — should collapse when sewed.
        let p = Point3::new(0.5, 0.0, 0.0);
        let v0 = arena.push(Shape::vertex(p));
        let v1 = arena.push(Shape::vertex(p));
        let v2 = arena.push(Shape::vertex(Point3::new(1.0, 0.0, 0.0)));
        let line01 = Line::from_points(Point3::new(0.0, 0.0, 0.0), p).unwrap();
        let line12 = Line::from_points(p, Point3::new(1.0, 0.0, 0.0)).unwrap();
        // Edge B references the *second* coincident vertex (v1); after sew
        // it should reference v0.
        let e0 = arena.push(Shape::Edge { curve: CurveGeom::Line(line01), vertices: [v0, v1], orient: Orientation::Forward });
        let e1 = arena.push(Shape::Edge { curve: CurveGeom::Line(line12), vertices: [v1, v2], orient: Orientation::Forward });
        let w = arena.push(Shape::Wire { edges: vec![(e0, Orientation::Forward), (e1, Orientation::Forward)] });
        let opts = HealOptions { tolerance: 1.0e-6, sew_faces: true, remove_small_edges: false, ..Default::default() };
        let log = fix_shape(&mut arena, w, &opts).unwrap();
        assert!(log.iter().any(|l| l.contains("sewed")));
        // After sew, e1 should point at v0, not v1.
        if let Shape::Edge { vertices, .. } = arena.get(e1).unwrap() {
            assert_eq!(vertices[0], v0);
        }
    }

    #[test]
    fn fix_shape_reports_no_changes_on_clean_box() {
        use gfd_cad_feature::box_solid;
        let mut arena = ShapeArena::new();
        let id = box_solid(&mut arena, 1.0, 1.0, 1.0).unwrap();
        let opts = HealOptions { tolerance: 1.0e-7, remove_small_edges: true, ..Default::default() };
        let log = fix_shape(&mut arena, id, &opts).unwrap();
        // Only the "post-fix validity" entry should remain.
        assert_eq!(log.len(), 1);
        assert!(log[0].contains("0 issue(s) remain"));
    }

    #[test]
    fn degenerate_edge_reported() {
        let mut arena = ShapeArena::new();
        let p = Point3::new(1.0, 2.0, 3.0);
        let v0 = arena.push(Shape::vertex(p));
        let v1 = arena.push(Shape::vertex(p));
        // Constructing a degenerate Line via from_points would fail, so we
        // build a minimal Line directly using a non-degenerate pair of points
        // and then pretend its vertices are both the same id — this models
        // the kind of mismatch that arises from a bad import.
        let line = Line::from_points(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)).unwrap();
        let edge = arena.push(Shape::Edge {
            curve: CurveGeom::Line(line),
            vertices: [v0, v1],
            orient: Orientation::Forward,
        });
        let issues = check_validity(&arena, edge).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind, "degenerate_edge");
    }

    #[test]
    fn remove_duplicate_faces_collapses_shell() {
        use gfd_cad_geom::{curve::Line, surface::Plane, Direction3, Point3};
        use gfd_cad_topo::{shape::{CurveGeom, SurfaceGeom}, Orientation};
        let mut arena = ShapeArena::new();
        let v0 = arena.push(Shape::vertex(Point3::new(0.0, 0.0, 0.0)));
        let v1 = arena.push(Shape::vertex(Point3::new(1.0, 0.0, 0.0)));
        let v2 = arena.push(Shape::vertex(Point3::new(1.0, 1.0, 0.0)));
        let v3 = arena.push(Shape::vertex(Point3::new(0.0, 1.0, 0.0)));
        let mk = |a, b, arena: &mut ShapeArena, pa, pb| arena.push(Shape::Edge {
            curve: CurveGeom::Line(Line::from_points(pa, pb).unwrap()),
            vertices: [a, b],
            orient: Orientation::Forward,
        });
        let e0 = mk(v0, v1, &mut arena, Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0));
        let e1 = mk(v1, v2, &mut arena, Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0));
        let e2 = mk(v2, v3, &mut arena, Point3::new(1.0, 1.0, 0.0), Point3::new(0.0, 1.0, 0.0));
        let e3 = mk(v3, v0, &mut arena, Point3::new(0.0, 1.0, 0.0), Point3::new(0.0, 0.0, 0.0));
        let wire = arena.push(Shape::Wire { edges: vec![
            (e0, Orientation::Forward), (e1, Orientation::Forward),
            (e2, Orientation::Forward), (e3, Orientation::Forward),
        ]});
        let plane = Plane::new(Point3::ORIGIN, Direction3::Z, Direction3::X);
        let f1 = arena.push(Shape::Face { surface: SurfaceGeom::Plane(plane), wires: vec![wire], orient: Orientation::Forward });
        let f2 = arena.push(Shape::Face { surface: SurfaceGeom::Plane(plane), wires: vec![wire], orient: Orientation::Forward });
        let shell = arena.push(Shape::Shell { faces: vec![(f1, Orientation::Forward), (f2, Orientation::Forward)] });
        let opts = HealOptions { remove_duplicate_faces: true, ..Default::default() };
        let log = fix_shape(&mut arena, shell, &opts).unwrap();
        assert!(log.iter().any(|l| l.contains("removed 1 duplicate face")));
        if let Shape::Shell { faces } = arena.get(shell).unwrap() {
            assert_eq!(faces.len(), 1);
            assert_eq!(faces[0].0, f1);
        }
    }

    #[test]
    fn duplicate_face_reported() {
        // Two faces that share the exact same wire set — classic import defect.
        use gfd_cad_geom::{curve::Line, surface::Plane, Direction3, Point3};
        use gfd_cad_topo::{shape::{CurveGeom, SurfaceGeom}, Orientation};
        let mut arena = ShapeArena::new();
        let v0 = arena.push(Shape::vertex(Point3::new(0.0, 0.0, 0.0)));
        let v1 = arena.push(Shape::vertex(Point3::new(1.0, 0.0, 0.0)));
        let v2 = arena.push(Shape::vertex(Point3::new(1.0, 1.0, 0.0)));
        let v3 = arena.push(Shape::vertex(Point3::new(0.0, 1.0, 0.0)));
        let mk_edge = |a, b, arena: &mut ShapeArena, pa, pb| {
            arena.push(Shape::Edge {
                curve: CurveGeom::Line(Line::from_points(pa, pb).unwrap()),
                vertices: [a, b],
                orient: Orientation::Forward,
            })
        };
        let e0 = mk_edge(v0, v1, &mut arena, Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0));
        let e1 = mk_edge(v1, v2, &mut arena, Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0));
        let e2 = mk_edge(v2, v3, &mut arena, Point3::new(1.0, 1.0, 0.0), Point3::new(0.0, 1.0, 0.0));
        let e3 = mk_edge(v3, v0, &mut arena, Point3::new(0.0, 1.0, 0.0), Point3::new(0.0, 0.0, 0.0));
        let wire = arena.push(Shape::Wire { edges: vec![
            (e0, Orientation::Forward), (e1, Orientation::Forward),
            (e2, Orientation::Forward), (e3, Orientation::Forward),
        ]});
        let plane = Plane::new(Point3::ORIGIN, Direction3::Z, Direction3::X);
        let f1 = arena.push(Shape::Face { surface: SurfaceGeom::Plane(plane), wires: vec![wire], orient: Orientation::Forward });
        let f2 = arena.push(Shape::Face { surface: SurfaceGeom::Plane(plane), wires: vec![wire], orient: Orientation::Forward });
        let shell = arena.push(Shape::Shell { faces: vec![(f1, Orientation::Forward), (f2, Orientation::Forward)] });
        let issues = check_validity(&arena, shell).unwrap();
        assert!(issues.iter().any(|i| i.kind == "duplicate_face"));
    }
}
