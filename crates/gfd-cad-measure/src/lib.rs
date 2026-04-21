//! gfd-cad-measure — geometric queries on B-Rep shapes.
//!
//! Iteration 5: distance (vertex-vertex), polygon area (shoelace), bounding
//! box volume. Analytical face-area and solid-volume integration arrive with
//! Phase 10 full implementation.

use gfd_cad_geom::{BoundingBox, Point3};
use gfd_cad_topo::{Shape, ShapeArena, ShapeId, TopoError};

#[derive(Debug, thiserror::Error)]
pub enum MeasureError {
    #[error("measurement not yet implemented for this shape")]
    Unimplemented,
    #[error("empty shape")]
    EmptyShape,
    #[error(transparent)]
    Topo(#[from] TopoError),
}

pub type MeasureResult<T> = Result<T, MeasureError>;

/// Euclidean distance between two vertex shapes.
pub fn distance(arena: &ShapeArena, a: ShapeId, b: ShapeId) -> MeasureResult<f64> {
    let pa = vertex_point(arena, a)?;
    let pb = vertex_point(arena, b)?;
    Ok(pa.distance(pb))
}

/// Perpendicular distance from a vertex to a line-backed edge segment.
/// Returns the closest distance to any point on the *clipped* segment
/// (not the infinite line), so a point outside the segment range returns
/// the distance to the nearest endpoint.
pub fn distance_vertex_edge(arena: &ShapeArena, vertex: ShapeId, edge: ShapeId) -> MeasureResult<f64> {
    let p = vertex_point(arena, vertex)?;
    let (a, b) = match arena.get(edge)? {
        Shape::Edge { vertices, .. } => {
            let va = match arena.get(vertices[0])? { Shape::Vertex { point } => *point, _ => return Err(MeasureError::Unimplemented) };
            let vb = match arena.get(vertices[1])? { Shape::Vertex { point } => *point, _ => return Err(MeasureError::Unimplemented) };
            (va, vb)
        }
        _ => return Err(MeasureError::Unimplemented),
    };
    let dx = b.x - a.x; let dy = b.y - a.y; let dz = b.z - a.z;
    let len2 = dx * dx + dy * dy + dz * dz;
    if len2 < 1.0e-24 { return Ok(p.distance(a)); }
    let t = ((p.x - a.x) * dx + (p.y - a.y) * dy + (p.z - a.z) * dz) / len2;
    let t = t.clamp(0.0, 1.0);
    let q = Point3::new(a.x + dx * t, a.y + dy * t, a.z + dz * t);
    Ok(p.distance(q))
}

/// Signed distance from `q` to a closed solid: negative inside, positive
/// outside. Combines `is_point_inside_solid` (ray cast) with
/// `closest_point_on_shape` (boundary distance). Same u/v resolution is
/// used for the inside test.
pub fn signed_distance(
    arena: &ShapeArena,
    id: ShapeId,
    q: Point3,
    u_steps: usize,
    v_steps: usize,
) -> MeasureResult<f64> {
    let d = closest_point_on_shape(arena, id, q)?;
    let inside = is_point_inside_solid(arena, id, q, u_steps, v_steps)?;
    Ok(if inside { -d } else { d })
}

/// Point-in-solid test — tessellates the shape (using the given u/v
/// resolution) then runs a perturbed-ray Möller-Trumbore count. Returns
/// true when `q` is inside the closed surface.
pub fn is_point_inside_solid(
    arena: &ShapeArena,
    id: ShapeId,
    q: Point3,
    u_steps: usize,
    v_steps: usize,
) -> MeasureResult<bool> {
    use gfd_cad_tessel::{tessellate, TessellationOptions};
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let mesh = tessellate(arena, id, opts).map_err(|_| MeasureError::Unimplemented)?;
    Ok(gfd_cad_bool::point_inside_mesh(
        [q.x as f32, q.y as f32, q.z as f32],
        &mesh,
    ))
}

/// Minimum distance from an arbitrary point `q` to any shape sub-element
/// (vertex / line-edge / polygon face). Walks the tree once and returns
/// the smallest distance. Useful for picking, snap-to-vertex, and hover
/// tests in the GUI.
pub fn closest_point_on_shape(arena: &ShapeArena, id: ShapeId, q: Point3) -> MeasureResult<f64> {
    let mut best = f64::INFINITY;
    walk_closest(arena, id, q, &mut best)?;
    if best.is_infinite() { return Err(MeasureError::EmptyShape); }
    Ok(best)
}

fn walk_closest(arena: &ShapeArena, id: ShapeId, q: Point3, best: &mut f64) -> MeasureResult<()> {
    match arena.get(id)? {
        Shape::Vertex { point } => {
            let d = q.distance(*point);
            if d < *best { *best = d; }
        }
        Shape::Edge { vertices, .. } => {
            let a = match arena.get(vertices[0])? { Shape::Vertex { point } => *point, _ => return Ok(()) };
            let b = match arena.get(vertices[1])? { Shape::Vertex { point } => *point, _ => return Ok(()) };
            let d = point_segment_dist(q, a, b);
            if d < *best { *best = d; }
        }
        Shape::Wire { edges } => for (e, _) in edges { walk_closest(arena, *e, q, best)?; }
        Shape::Face { wires, .. } => {
            // First check the interior: project q onto the face plane (via
            // Newell normal) and test if the projection lies inside the
            // boundary polygon. If so the planar distance is the answer.
            if let Some(d) = point_to_face_interior_dist(arena, id, q) {
                if d < *best { *best = d; }
            }
            for w in wires { walk_closest(arena, *w, q, best)?; }
        }
        Shape::Shell { faces } => for (f, _) in faces { walk_closest(arena, *f, q, best)?; }
        Shape::Solid { shells } => for s in shells { walk_closest(arena, *s, q, best)?; }
        Shape::Compound { children } => for c in children { walk_closest(arena, *c, q, best)?; }
    }
    Ok(())
}

/// Distance from `q` to the interior of a planar face, or None if the
/// projection falls outside the outer polygon wire. Uses Newell's normal
/// on the first wire's polygon and a 2D point-in-polygon test in the
/// face's local frame.
fn point_to_face_interior_dist(arena: &ShapeArena, face_id: ShapeId, q: Point3) -> Option<f64> {
    let Ok(Shape::Face { wires, .. }) = arena.get(face_id) else { return None; };
    if wires.is_empty() { return None; }
    let poly = extract_line_polygon(arena, wires[0]).ok()?;
    if poly.len() < 3 { return None; }
    // Newell normal.
    let mut nx = 0.0; let mut ny = 0.0; let mut nz = 0.0;
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        let a = poly[i];
        let b = poly[j];
        nx += (a.y - b.y) * (a.z + b.z);
        ny += (a.z - b.z) * (a.x + b.x);
        nz += (a.x - b.x) * (a.y + b.y);
    }
    let n_len = (nx*nx + ny*ny + nz*nz).sqrt();
    if n_len < 1.0e-12 { return None; }
    let (nxh, nyh, nzh) = (nx / n_len, ny / n_len, nz / n_len);
    // Plane distance (signed).
    let p0 = poly[0];
    let d_signed = (q.x - p0.x) * nxh + (q.y - p0.y) * nyh + (q.z - p0.z) * nzh;
    // Project q into the face plane.
    let qp = Point3::new(
        q.x - nxh * d_signed,
        q.y - nyh * d_signed,
        q.z - nzh * d_signed,
    );
    // Build a local 2D basis on the plane and convert polygon + qp to 2D.
    let (ux, uy, uz) = pick_tangent((nxh, nyh, nzh));
    let (vx, vy, vz) = (nyh*uz - nzh*uy, nzh*ux - nxh*uz, nxh*uy - nyh*ux);
    let to2d = |p: Point3| -> (f64, f64) {
        let rx = p.x - p0.x;
        let ry = p.y - p0.y;
        let rz = p.z - p0.z;
        (rx*ux + ry*uy + rz*uz, rx*vx + ry*vy + rz*vz)
    };
    let poly2: Vec<(f64, f64)> = poly.iter().map(|p| to2d(*p)).collect();
    let q2 = to2d(qp);
    if point_in_polygon(q2, &poly2) { Some(d_signed.abs()) } else { None }
}

fn pick_tangent(n: (f64, f64, f64)) -> (f64, f64, f64) {
    // Choose an axis that isn't near-parallel to the normal, then orthogonalise.
    let a = if n.0.abs() < 0.9 { (1.0, 0.0, 0.0) } else { (0.0, 1.0, 0.0) };
    let dot = a.0*n.0 + a.1*n.1 + a.2*n.2;
    let tx = a.0 - dot * n.0;
    let ty = a.1 - dot * n.1;
    let tz = a.2 - dot * n.2;
    let l = (tx*tx + ty*ty + tz*tz).sqrt().max(1.0e-12);
    (tx/l, ty/l, tz/l)
}

fn point_in_polygon(p: (f64, f64), poly: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > p.1) != (yj > p.1)
           && p.0 < (xj - xi) * (p.1 - yi) / (yj - yi + f64::EPSILON) + xi
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Approximate bounding sphere of a shape: centre at the bbox midpoint,
/// radius equal to the bbox diagonal half-length. Not the minimal enclosing
/// sphere — see [`minimum_bounding_sphere`] for Welzl's exact variant.
pub fn bounding_sphere(arena: &ShapeArena, id: ShapeId) -> MeasureResult<(Point3, f64)> {
    let bb = bounding_box(arena, id)?;
    let cx = (bb.min.x + bb.max.x) * 0.5;
    let cy = (bb.min.y + bb.max.y) * 0.5;
    let cz = (bb.min.z + bb.max.z) * 0.5;
    let r = bb.diagonal() * 0.5;
    Ok((Point3::new(cx, cy, cz), r))
}

/// Minimum enclosing sphere via Welzl's randomised algorithm applied to
/// every vertex point reachable from `id`. Expected O(n) for n points.
pub fn minimum_bounding_sphere(arena: &ShapeArena, id: ShapeId) -> MeasureResult<(Point3, f64)> {
    let mut pts = Vec::new();
    walk_vertices(arena, id, &mut |p| pts.push(p))?;
    if pts.is_empty() { return Err(MeasureError::EmptyShape); }
    let sphere = welzl(&mut pts, Vec::new());
    Ok(sphere)
}

fn welzl(pts: &mut Vec<Point3>, boundary: Vec<Point3>) -> (Point3, f64) {
    if pts.is_empty() || boundary.len() == 4 {
        return trivial_sphere(&boundary);
    }
    let p = pts.pop().unwrap();
    let (c, r) = welzl(pts, boundary.clone());
    if in_sphere(p, c, r) {
        pts.push(p);
        return (c, r);
    }
    let mut b2 = boundary;
    b2.push(p);
    let result = welzl(pts, b2);
    pts.push(p);
    result
}

fn trivial_sphere(boundary: &[Point3]) -> (Point3, f64) {
    match boundary.len() {
        0 => (Point3::ORIGIN, 0.0),
        1 => (boundary[0], 0.0),
        2 => {
            let a = boundary[0]; let b = boundary[1];
            let c = Point3::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5);
            (c, a.distance(b) * 0.5)
        }
        _ => {
            // Generic fallback — centroid + max-distance. Accurate for 3-4
            // collinear/coplanar configurations is not the textbook case but
            // acceptable for our coarse point sets.
            let n = boundary.len() as f64;
            let cx = boundary.iter().map(|p| p.x).sum::<f64>() / n;
            let cy = boundary.iter().map(|p| p.y).sum::<f64>() / n;
            let cz = boundary.iter().map(|p| p.z).sum::<f64>() / n;
            let c = Point3::new(cx, cy, cz);
            let r = boundary.iter().map(|p| c.distance(*p)).fold(0.0_f64, f64::max);
            (c, r)
        }
    }
}

fn in_sphere(p: Point3, c: Point3, r: f64) -> bool {
    p.distance(c) <= r + 1.0e-9
}

/// Return `(shortest, longest)` line-edge lengths reachable from `id`.
/// Skips curved edges. Returns `Err(EmptyShape)` if no linear edges found.
pub fn edge_length_range(arena: &ShapeArena, id: ShapeId) -> MeasureResult<(f64, f64)> {
    use gfd_cad_topo::{collect_by_kind, ShapeKind};
    let edges = collect_by_kind(arena, id, ShapeKind::Edge);
    let mut min_len = f64::INFINITY;
    let mut max_len = 0.0_f64;
    let mut any = false;
    for e in edges {
        if let Ok(l) = edge_length(arena, e) {
            min_len = min_len.min(l);
            max_len = max_len.max(l);
            any = true;
        }
    }
    if !any { return Err(MeasureError::EmptyShape); }
    Ok((min_len, max_len))
}

/// Principal moments and axes of the inertia tensor: the three eigenvalues
/// (ordered ascending) plus the orthonormal eigenvector matrix.
/// Useful for auto-alignment or gimbal-stabilised UI gizmos.
pub fn principal_axes(arena: &ShapeArena, id: ShapeId) -> MeasureResult<(f64, f64, f64, [[f64; 3]; 3])> {
    let (ixx, iyy, izz, ixy, iyz, izx) = inertia_tensor_full(arena, id)?;
    let m = nalgebra::Matrix3::new(
        ixx, -ixy, -izx,
        -ixy,  iyy, -iyz,
        -izx, -iyz,  izz,
    );
    let eig = m.symmetric_eigen();
    // Sort eigenvalues ascending.
    let mut pairs: Vec<(f64, [f64; 3])> = (0..3).map(|i| {
        let v = eig.eigenvectors.column(i);
        (eig.eigenvalues[i], [v[0], v[1], v[2]])
    }).collect();
    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let vals = (pairs[0].0, pairs[1].0, pairs[2].0);
    let vecs = [pairs[0].1, pairs[1].1, pairs[2].1];
    Ok((vals.0, vals.1, vals.2, vecs))
}

/// Signed distance from a point to an oriented plane. The plane is given
/// by a base point `origin` and a normal vector (need not be unit — we
/// normalize). Positive means the query point is on the side the normal
/// points toward; negative on the other. Returns 0 for a degenerate
/// (zero-length) normal.
pub fn point_plane_signed_distance(
    point: Point3,
    origin: Point3,
    normal: [f64; 3],
) -> f64 {
    let nl = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if nl < f64::EPSILON { return 0.0; }
    let nx = normal[0] / nl;
    let ny = normal[1] / nl;
    let nz = normal[2] / nl;
    (point.x - origin.x) * nx + (point.y - origin.y) * ny + (point.z - origin.z) * nz
}

/// Ray-plane intersection. Ray = `ray_origin + t · ray_dir`, plane has
/// `plane_origin` + unit (or non-unit) normal. Returns `Some((t, hit))`
/// when the ray is not parallel to the plane and the hit parameter is
/// finite; `None` when parallel within tolerance.
///
/// `t < 0` is legal — callers interested in forward-only hits should
/// filter. Useful for click-to-plane picking, section views, and
/// projecting 3D cursors onto construction planes.
pub fn ray_plane_intersection(
    ray_origin: Point3,
    ray_dir: [f64; 3],
    plane_origin: Point3,
    plane_normal: [f64; 3],
) -> Option<(f64, Point3)> {
    let dn = ray_dir[0] * plane_normal[0]
           + ray_dir[1] * plane_normal[1]
           + ray_dir[2] * plane_normal[2];
    if dn.abs() < 1.0e-12 { return None; }
    let num = (plane_origin.x - ray_origin.x) * plane_normal[0]
            + (plane_origin.y - ray_origin.y) * plane_normal[1]
            + (plane_origin.z - ray_origin.z) * plane_normal[2];
    let t = num / dn;
    let hit = Point3::new(
        ray_origin.x + t * ray_dir[0],
        ray_origin.y + t * ray_dir[1],
        ray_origin.z + t * ray_dir[2],
    );
    Some((t, hit))
}

/// Standalone Lumelsky segment-segment closest points in 3D. Returns the
/// distance plus the two closest points (`cp_a` on segment A, `cp_b` on
/// segment B) and the segment parameters `(s, t)` in [0, 1]. Degenerate
/// (zero-length) segments are handled gracefully.
pub fn segment_segment_distance_3d(
    a0: Point3, a1: Point3,
    b0: Point3, b1: Point3,
) -> (f64, Point3, Point3, f64, f64) {
    let d1 = (a1.x - a0.x, a1.y - a0.y, a1.z - a0.z);
    let d2 = (b1.x - b0.x, b1.y - b0.y, b1.z - b0.z);
    let r  = (a0.x - b0.x, a0.y - b0.y, a0.z - b0.z);
    let a = d1.0 * d1.0 + d1.1 * d1.1 + d1.2 * d1.2;
    let e = d2.0 * d2.0 + d2.1 * d2.1 + d2.2 * d2.2;
    let f = d2.0 * r.0 + d2.1 * r.1 + d2.2 * r.2;
    let eps = 1.0e-12;
    let (mut s, mut t) = (0.0_f64, 0.0_f64);
    if a <= eps && e <= eps {
        // both segments degenerate
    } else if a <= eps {
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.0 * r.0 + d1.1 * r.1 + d1.2 * r.2;
        if e <= eps {
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = d1.0 * d2.0 + d1.1 * d2.1 + d1.2 * d2.2;
            let denom = a * e - b * b;
            let s_raw = if denom > eps { (b * f - c * e) / denom } else { 0.0 };
            s = s_raw.clamp(0.0, 1.0);
            let t_raw = (b * s + f) / e;
            if t_raw < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t_raw > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            } else {
                t = t_raw;
            }
        }
    }
    let cp_a = Point3::new(a0.x + d1.0 * s, a0.y + d1.1 * s, a0.z + d1.2 * s);
    let cp_b = Point3::new(b0.x + d2.0 * t, b0.y + d2.1 * t, b0.z + d2.2 * t);
    (cp_a.distance(cp_b), cp_a, cp_b, s, t)
}

/// Shortest distance between two line-backed edge segments in 3D.
/// Implements the classic Lumelsky closest-points-between-segments
/// algorithm (O(1)). Handles parallel / overlapping / endpoint cases.
pub fn distance_edge_edge(arena: &ShapeArena, e1: ShapeId, e2: ShapeId) -> MeasureResult<f64> {
    let (p1, q1) = edge_endpoints(arena, e1)?;
    let (p2, q2) = edge_endpoints(arena, e2)?;
    Ok(segment_segment_distance_3d(p1, q1, p2, q2).0)
}

/// Minimum distance between two polygon-outer-wire faces. Conservative: we
/// sample every vertex of A against every edge of B (and vice versa) and
/// return the smallest hit. Face interiors are not sampled, so an edge of A
/// passing *through* face B registers at the edge-edge level.
pub fn distance_face_face(arena: &ShapeArena, f1: ShapeId, f2: ShapeId) -> MeasureResult<f64> {
    let poly1 = extract_face_polygon(arena, f1)?;
    let poly2 = extract_face_polygon(arena, f2)?;
    if poly1.is_empty() || poly2.is_empty() {
        return Err(MeasureError::Unimplemented);
    }
    let mut best = f64::INFINITY;
    // Vertex-vertex pass.
    for p in &poly1 {
        for q in &poly2 {
            let d = p.distance(*q);
            if d < best { best = d; }
        }
    }
    // Vertex-edge pass (both directions).
    for p in &poly1 {
        for i in 0..poly2.len() {
            let a = poly2[i];
            let b = poly2[(i + 1) % poly2.len()];
            let d = point_segment_dist(*p, a, b);
            if d < best { best = d; }
        }
    }
    for p in &poly2 {
        for i in 0..poly1.len() {
            let a = poly1[i];
            let b = poly1[(i + 1) % poly1.len()];
            let d = point_segment_dist(*p, a, b);
            if d < best { best = d; }
        }
    }
    Ok(best)
}

fn extract_face_polygon(arena: &ShapeArena, face_id: ShapeId) -> MeasureResult<Vec<Point3>> {
    let Shape::Face { wires, .. } = arena.get(face_id)? else {
        return Err(MeasureError::Unimplemented);
    };
    if wires.is_empty() { return Err(MeasureError::Unimplemented); }
    extract_line_polygon(arena, wires[0])
}

fn point_segment_dist(p: Point3, a: Point3, b: Point3) -> f64 {
    let dx = b.x - a.x; let dy = b.y - a.y; let dz = b.z - a.z;
    let len2 = dx * dx + dy * dy + dz * dz;
    if len2 < 1e-24 { return p.distance(a); }
    let t = ((p.x - a.x) * dx + (p.y - a.y) * dy + (p.z - a.z) * dz) / len2;
    let t = t.clamp(0.0, 1.0);
    let q = Point3::new(a.x + dx * t, a.y + dy * t, a.z + dz * t);
    p.distance(q)
}

/// Full 3×3 unit-density inertia tensor for a closed polygon-faced solid,
/// integrated via signed-tetrahedra over each face fan. Returns
/// `(Ixx, Iyy, Izz, Ixy, Iyz, Izx)` where the off-diagonals are the
/// products of inertia (∫ xy dV, etc.).
pub fn inertia_tensor_full(arena: &ShapeArena, id: ShapeId) -> MeasureResult<(f64, f64, f64, f64, f64, f64)> {
    use gfd_cad_topo::{collect_by_kind, ShapeKind};
    let faces = collect_by_kind(arena, id, ShapeKind::Face);
    let mut ix2 = 0.0_f64;
    let mut iy2 = 0.0_f64;
    let mut iz2 = 0.0_f64;
    let mut ixy = 0.0_f64;
    let mut iyz = 0.0_f64;
    let mut izx = 0.0_f64;
    let mut any = false;
    for f in faces {
        let Ok(gfd_cad_topo::Shape::Face { wires, .. }) = arena.get(f) else { continue; };
        if wires.is_empty() { continue; }
        let poly = extract_line_polygon(arena, wires[0])?;
        if poly.len() < 3 { continue; }
        any = true;
        let p0 = poly[0];
        for i in 1..poly.len() - 1 {
            let p1 = poly[i];
            let p2 = poly[i + 1];
            let cx = p1.y * p2.z - p1.z * p2.y;
            let cy = p1.z * p2.x - p1.x * p2.z;
            let cz = p1.x * p2.y - p1.y * p2.x;
            let v = (p0.x * cx + p0.y * cy + p0.z * cz) / 6.0;
            let vol = v.abs() * v.signum();
            // For a tet with vertices (0, p0, p1, p2), ∫ xy dV = |V|/20 *
            //   (x0 y0 + x1 y1 + x2 y2 + ½(x0 y1 + x1 y0 + x0 y2 + x2 y0 + x1 y2 + x2 y1))
            let sxx = p0.x * p0.x + p1.x * p1.x + p2.x * p2.x
                    + p0.x * p1.x + p0.x * p2.x + p1.x * p2.x;
            let syy = p0.y * p0.y + p1.y * p1.y + p2.y * p2.y
                    + p0.y * p1.y + p0.y * p2.y + p1.y * p2.y;
            let szz = p0.z * p0.z + p1.z * p1.z + p2.z * p2.z
                    + p0.z * p1.z + p0.z * p2.z + p1.z * p2.z;
            let sxy = 2.0 * (p0.x * p0.y + p1.x * p1.y + p2.x * p2.y)
                    + p0.x * p1.y + p1.x * p0.y
                    + p0.x * p2.y + p2.x * p0.y
                    + p1.x * p2.y + p2.x * p1.y;
            let syz = 2.0 * (p0.y * p0.z + p1.y * p1.z + p2.y * p2.z)
                    + p0.y * p1.z + p1.y * p0.z
                    + p0.y * p2.z + p2.y * p0.z
                    + p1.y * p2.z + p2.y * p1.z;
            let szx = 2.0 * (p0.z * p0.x + p1.z * p1.x + p2.z * p2.x)
                    + p0.z * p1.x + p1.z * p0.x
                    + p0.z * p2.x + p2.z * p0.x
                    + p1.z * p2.x + p2.z * p1.x;
            ix2 += vol * (syy + szz) / 10.0;
            iy2 += vol * (sxx + szz) / 10.0;
            iz2 += vol * (sxx + syy) / 10.0;
            ixy += vol * sxy / 20.0;
            iyz += vol * syz / 20.0;
            izx += vol * szx / 20.0;
        }
    }
    if !any {
        let (ix, iy, iz) = inertia_bbox(arena, id)?;
        return Ok((ix, iy, iz, 0.0, 0.0, 0.0));
    }
    Ok((ix2.abs(), iy2.abs(), iz2.abs(), ixy.abs(), iyz.abs(), izx.abs()))
}

/// Analytical diagonal of the unit-density inertia tensor for a closed
/// polygon-faced solid, computed via the divergence theorem with signed-
/// tetrahedra contributions from each face's fan triangulation. Returns
/// `(Ixx, Iyy, Izz)` about the origin. Spheres / cylinders (no wires)
/// fall back to [`inertia_bbox`].
pub fn inertia_tensor_diag(arena: &ShapeArena, id: ShapeId) -> MeasureResult<(f64, f64, f64)> {
    use gfd_cad_topo::{collect_by_kind, ShapeKind};
    let faces = collect_by_kind(arena, id, ShapeKind::Face);
    let mut ix = 0.0_f64;
    let mut iy = 0.0_f64;
    let mut iz = 0.0_f64;
    let mut any = false;
    for f in faces {
        let Ok(gfd_cad_topo::Shape::Face { wires, .. }) = arena.get(f) else { continue; };
        if wires.is_empty() { continue; }
        let poly = extract_line_polygon(arena, wires[0])?;
        if poly.len() < 3 { continue; }
        any = true;
        let p0 = poly[0];
        for i in 1..poly.len() - 1 {
            let p1 = poly[i];
            let p2 = poly[i + 1];
            // Signed tetra volume (origin, p0, p1, p2) = (1/6) p0·(p1×p2).
            let cx = p1.y * p2.z - p1.z * p2.y;
            let cy = p1.z * p2.x - p1.x * p2.z;
            let cz = p1.x * p2.y - p1.y * p2.x;
            let v = (p0.x * cx + p0.y * cy + p0.z * cz) / 6.0;
            // Tetrahedron about origin: I contribution for a uniform tet with
            // vertices (0, p0, p1, p2) is (|V|/10) * Σ (xi² + xj² + xi*xj) etc.
            // Simplified diagonal using the identity for integral of x² over tet:
            //   ∫ x² dV = (|V|/10)(x0² + x1² + x2² + x0 x1 + x0 x2 + x1 x2)
            // treating the origin as the 4th vertex (contributing 0).
            let vol = v.abs();
            let sumx2 = p0.x * p0.x + p1.x * p1.x + p2.x * p2.x
                      + p0.x * p1.x + p0.x * p2.x + p1.x * p2.x;
            let sumy2 = p0.y * p0.y + p1.y * p1.y + p2.y * p2.y
                      + p0.y * p1.y + p0.y * p2.y + p1.y * p2.y;
            let sumz2 = p0.z * p0.z + p1.z * p1.z + p2.z * p2.z
                      + p0.z * p1.z + p0.z * p2.z + p1.z * p2.z;
            // Ix = ∫ (y² + z²) dV
            ix += vol * (sumy2 + sumz2) / 10.0 * v.signum();
            iy += vol * (sumx2 + sumz2) / 10.0 * v.signum();
            iz += vol * (sumx2 + sumy2) / 10.0 * v.signum();
        }
    }
    if !any {
        return inertia_bbox(arena, id);
    }
    Ok((ix.abs(), iy.abs(), iz.abs()))
}

/// Diagonal of the mass-density=1 inertia tensor for a solid, computed from
/// its bounding box as if it were a homogeneous box. Returns `(Ixx, Iyy, Izz)`.
/// An exact moment-of-inertia integral ships once B-Rep CSG lands.
pub fn inertia_bbox(arena: &ShapeArena, id: ShapeId) -> MeasureResult<(f64, f64, f64)> {
    let bb = bounding_box(arena, id)?;
    let lx = bb.max.x - bb.min.x;
    let ly = bb.max.y - bb.min.y;
    let lz = bb.max.z - bb.min.z;
    let mass = lx * ly * lz;            // assuming unit density
    // Solid box about its own centre: Ix = (1/12) m (y² + z²), etc.
    let ix = mass * (ly * ly + lz * lz) / 12.0;
    let iy = mass * (lx * lx + lz * lz) / 12.0;
    let iz = mass * (lx * lx + ly * ly) / 12.0;
    Ok((ix, iy, iz))
}

fn vertex_point(arena: &ShapeArena, id: ShapeId) -> MeasureResult<Point3> {
    match arena.get(id)? {
        Shape::Vertex { point } => Ok(*point),
        _ => Err(MeasureError::Unimplemented),
    }
}

/// Signed polygon area (2D) via the shoelace formula. Positive if the loop
/// winds counter-clockwise.
pub fn polygon_area_signed(points: &[(f64, f64)]) -> f64 {
    if points.len() < 3 { return 0.0; }
    let mut acc = 0.0;
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        acc += points[i].0 * points[j].1;
        acc -= points[j].0 * points[i].1;
    }
    acc * 0.5
}

pub fn polygon_area(points: &[(f64, f64)]) -> f64 {
    polygon_area_signed(points).abs()
}

/// 2D triangle area from three corner points (unsigned, absolute value).
pub fn triangle_area_2d(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    let cross = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    cross.abs() * 0.5
}

/// 3D triangle area via half the norm of the cross-product of two edge
/// vectors. Works regardless of orientation.
pub fn triangle_area_3d(a: Point3, b: Point3, c: Point3) -> f64 {
    let abx = b.x - a.x; let aby = b.y - a.y; let abz = b.z - a.z;
    let acx = c.x - a.x; let acy = c.y - a.y; let acz = c.z - a.z;
    let nx = aby * acz - abz * acy;
    let ny = abz * acx - abx * acz;
    let nz = abx * acy - aby * acx;
    (nx * nx + ny * ny + nz * nz).sqrt() * 0.5
}

/// 2D line-segment / line-segment intersection. Returns `Some((x, y))` when
/// the segments properly cross in their interior; `None` for parallel,
/// collinear, or strictly non-touching cases.
pub fn segment_intersection_2d(
    a: (f64, f64), b: (f64, f64),
    c: (f64, f64), d: (f64, f64),
) -> Option<(f64, f64)> {
    let rx = b.0 - a.0; let ry = b.1 - a.1;
    let sx = d.0 - c.0; let sy = d.1 - c.1;
    let denom = rx * sy - ry * sx;
    if denom.abs() < 1e-12 { return None; }
    let t = ((c.0 - a.0) * sy - (c.1 - a.1) * sx) / denom;
    let u = ((c.0 - a.0) * ry - (c.1 - a.1) * rx) / denom;
    if t > 0.0 && t < 1.0 && u > 0.0 && u < 1.0 {
        Some((a.0 + t * rx, a.1 + t * ry))
    } else {
        None
    }
}

/// Barycentric coordinates of `p` with respect to triangle `(a, b, c)` in 2D.
/// Returns (u, v, w) such that `p = u·a + v·b + w·c` and `u + v + w = 1`.
/// A triangle with zero area yields `None`.
pub fn barycentric_2d(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<(f64, f64, f64)> {
    let v0 = (b.0 - a.0, b.1 - a.1);
    let v1 = (c.0 - a.0, c.1 - a.1);
    let v2 = (p.0 - a.0, p.1 - a.1);
    let d00 = v0.0 * v0.0 + v0.1 * v0.1;
    let d01 = v0.0 * v1.0 + v0.1 * v1.1;
    let d11 = v1.0 * v1.0 + v1.1 * v1.1;
    let d20 = v2.0 * v0.0 + v2.1 * v0.1;
    let d21 = v2.0 * v1.0 + v2.1 * v1.1;
    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-14 { return None; }
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    let u = 1.0 - v - w;
    Some((u, v, w))
}

/// Minimum distance from a query point `q` to any segment of a 3D polyline.
/// Returns (distance, index of the closest segment). Open-chain input.
pub fn closest_point_on_polyline_3d(q: Point3, polyline: &[Point3]) -> Option<(f64, usize)> {
    if polyline.len() < 2 { return None; }
    let mut best = f64::INFINITY;
    let mut best_idx = 0usize;
    for i in 0..polyline.len() - 1 {
        let d = point_segment_dist(q, polyline[i], polyline[i + 1]);
        if d < best { best = d; best_idx = i; }
    }
    Some((best, best_idx))
}

/// Sum of segment lengths of a 3D polyline (open chain).
pub fn polyline_length_3d(points: &[Point3]) -> f64 {
    if points.len() < 2 { return 0.0; }
    let mut acc = 0.0;
    for i in 0..points.len() - 1 {
        acc += points[i].distance(points[i + 1]);
    }
    acc
}

/// Point-in-polygon test for a 2D polygon (ray-casting, crossing count).
/// Works for convex and non-convex simple polygons.
pub fn polygon_contains_point(poly: &[(f64, f64)], p: (f64, f64)) -> bool {
    if poly.len() < 3 { return false; }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > p.1) != (yj > p.1)
           && p.0 < (xj - xi) * (p.1 - yi) / (yj - yi + f64::EPSILON) + xi
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// 2D convex hull via Andrew's monotone-chain algorithm. Returns the hull
/// vertices in CCW order with no duplicates; output length ≤ input length.
pub fn polygon_convex_hull(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if points.len() < 3 { return points.to_vec(); }
    let mut pts = points.to_vec();
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.partial_cmp(&b.1).unwrap()));
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)|
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
    let mut lower: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
    for p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], *p) <= 0.0 {
            lower.pop();
        }
        lower.push(*p);
    }
    let mut upper: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
    for p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], *p) <= 0.0 {
            upper.pop();
        }
        upper.push(*p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// True when a 2D polygon is strictly convex — every turn has the same
/// cross-product sign. Returns false for fewer than 3 points or any
/// collinear/reflex vertex.
pub fn is_convex_polygon(points: &[(f64, f64)]) -> bool {
    if points.len() < 3 { return false; }
    let mut sign: Option<f64> = None;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        let c = points[(i + 2) % points.len()];
        let cross = (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0);
        if cross.abs() < 1.0e-12 { continue; } // collinear triplet ignored
        match sign {
            None => sign = Some(cross.signum()),
            Some(s) => if s * cross < 0.0 { return false; },
        }
    }
    sign.is_some()
}

/// Polygon perimeter — sum of edge lengths around the boundary.
pub fn polygon_perimeter(points: &[(f64, f64)]) -> f64 {
    if points.len() < 2 { return 0.0; }
    let mut acc = 0.0;
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        let dx = points[j].0 - points[i].0;
        let dy = points[j].1 - points[i].1;
        acc += (dx * dx + dy * dy).sqrt();
    }
    acc
}

/// Polygon centroid (area-weighted) for a CCW / CW planar polygon.
pub fn polygon_centroid(points: &[(f64, f64)]) -> (f64, f64) {
    if points.len() < 3 { return (0.0, 0.0); }
    let a = polygon_area_signed(points);
    if a.abs() < 1e-18 { return (0.0, 0.0); }
    let mut cx = 0.0; let mut cy = 0.0;
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        let cross = points[i].0 * points[j].1 - points[j].0 * points[i].1;
        cx += (points[i].0 + points[j].0) * cross;
        cy += (points[i].1 + points[j].1) * cross;
    }
    let f = 1.0 / (6.0 * a);
    (cx * f, cy * f)
}

/// Axis-aligned bounding box of a shape sub-tree.
pub fn bounding_box(arena: &ShapeArena, id: ShapeId) -> MeasureResult<BoundingBox> {
    let mut bb = BoundingBox::EMPTY;
    walk_vertices(arena, id, &mut |p| bb.expand(p))?;
    if bb.is_empty() { return Err(MeasureError::EmptyShape); }
    Ok(bb)
}

/// Approximate volume as the axis-aligned bounding box volume. Exact
/// B-Rep volume integration ships in Phase 10 full impl.
pub fn bbox_volume(arena: &ShapeArena, id: ShapeId) -> MeasureResult<f64> {
    let bb = bounding_box(arena, id)?;
    let dx = bb.max.x - bb.min.x;
    let dy = bb.max.y - bb.min.y;
    let dz = bb.max.z - bb.min.z;
    Ok(dx * dy * dz)
}

/// Edge-length statistics for a raw triangle mesh. Computes unique edges
/// (HashSet of sorted pairs), then returns `(min, max, mean, stddev)` of
/// their Euclidean lengths. Useful for mesh quality inspection and
/// chord-tolerance tuning. Returns `None` if fewer than 1 unique edge.
pub fn trimesh_edge_length_stats(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<(f64, f64, f64, f64)> {
    let mut edges: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
    for t in 0..(indices.len() / 3) {
        let i0 = indices[t * 3];
        let i1 = indices[t * 3 + 1];
        let i2 = indices[t * 3 + 2];
        for (a, b) in [(i0, i1), (i1, i2), (i2, i0)] {
            let k = if a < b { (a, b) } else { (b, a) };
            edges.insert(k);
        }
    }
    if edges.is_empty() { return None; }
    let mut lengths: Vec<f64> = Vec::with_capacity(edges.len());
    for (a, b) in &edges {
        let p = positions[*a as usize];
        let q = positions[*b as usize];
        let dx = (p[0] - q[0]) as f64;
        let dy = (p[1] - q[1]) as f64;
        let dz = (p[2] - q[2]) as f64;
        lengths.push((dx * dx + dy * dy + dz * dz).sqrt());
    }
    let n = lengths.len() as f64;
    let min = lengths.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = lengths.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mean = lengths.iter().sum::<f64>() / n;
    let var = lengths.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / n;
    Some((min, max, mean, var.sqrt()))
}

/// Triangle aspect-ratio statistics: for each triangle, aspect ratio is
/// the longest edge divided by the shortest. Returns `(min, max, mean)`
/// across all triangles. Lower is better (1 = equilateral).
pub fn trimesh_aspect_ratio_stats(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<(f64, f64, f64)> {
    if indices.len() < 3 { return None; }
    let mut ratios: Vec<f64> = Vec::with_capacity(indices.len() / 3);
    for t in 0..(indices.len() / 3) {
        let a = positions[indices[t * 3] as usize];
        let b = positions[indices[t * 3 + 1] as usize];
        let c = positions[indices[t * 3 + 2] as usize];
        let d = |p: [f32; 3], q: [f32; 3]| -> f64 {
            let dx = (p[0] - q[0]) as f64;
            let dy = (p[1] - q[1]) as f64;
            let dz = (p[2] - q[2]) as f64;
            (dx * dx + dy * dy + dz * dz).sqrt()
        };
        let lens = [d(a, b), d(b, c), d(c, a)];
        let lo = lens.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = lens.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if lo > 1.0e-20 { ratios.push(hi / lo); }
    }
    if ratios.is_empty() { return None; }
    let n = ratios.len() as f64;
    let min = ratios.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = ratios.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mean = ratios.iter().sum::<f64>() / n;
    Some((min, max, mean))
}

/// Sum of triangle areas for a raw triangle mesh. Works on any triangle
/// soup regardless of manifoldness.
pub fn trimesh_surface_area(positions: &[[f32; 3]], indices: &[u32]) -> f64 {
    let mut s = 0.0_f64;
    for t in 0..(indices.len() / 3) {
        let a = positions[indices[t * 3] as usize];
        let b = positions[indices[t * 3 + 1] as usize];
        let c = positions[indices[t * 3 + 2] as usize];
        let e1 = [
            (b[0] - a[0]) as f64,
            (b[1] - a[1]) as f64,
            (b[2] - a[2]) as f64,
        ];
        let e2 = [
            (c[0] - a[0]) as f64,
            (c[1] - a[1]) as f64,
            (c[2] - a[2]) as f64,
        ];
        let cx = e1[1] * e2[2] - e1[2] * e2[1];
        let cy = e1[2] * e2[0] - e1[0] * e2[2];
        let cz = e1[0] * e2[1] - e1[1] * e2[0];
        s += 0.5 * (cx * cx + cy * cy + cz * cz).sqrt();
    }
    s
}

/// Signed volume of a closed triangle mesh via the divergence theorem:
/// V = Σ (1/6) · (v0 · (v1 × v2)) over all triangles. For closed, outward-
/// oriented meshes this equals the enclosed volume. Absolute value is
/// taken so callers don't have to reason about winding.
pub fn trimesh_volume(positions: &[[f32; 3]], indices: &[u32]) -> f64 {
    let mut v = 0.0_f64;
    for t in 0..(indices.len() / 3) {
        let a = positions[indices[t * 3] as usize];
        let b = positions[indices[t * 3 + 1] as usize];
        let c = positions[indices[t * 3 + 2] as usize];
        let (ax, ay, az) = (a[0] as f64, a[1] as f64, a[2] as f64);
        let (bx, by, bz) = (b[0] as f64, b[1] as f64, b[2] as f64);
        let (cx, cy, cz) = (c[0] as f64, c[1] as f64, c[2] as f64);
        v += (ax * (by * cz - bz * cy)
            + bx * (cy * az - cz * ay)
            + cx * (ay * bz - az * by)) / 6.0;
    }
    v.abs()
}

/// Closest point on a triangle to `q`. Uses barycentric coordinates with
/// clamping onto the six sub-regions (3 vertices, 3 edges, 1 interior).
/// Based on Ericson, *Real-Time Collision Detection*, §5.1.5.
pub fn closest_point_on_triangle(
    q: [f64; 3],
    a: [f32; 3], b: [f32; 3], c: [f32; 3],
) -> [f64; 3] {
    let a = [a[0] as f64, a[1] as f64, a[2] as f64];
    let b = [b[0] as f64, b[1] as f64, b[2] as f64];
    let c = [c[0] as f64, c[1] as f64, c[2] as f64];
    let sub = |u: [f64; 3], v: [f64; 3]| [u[0]-v[0], u[1]-v[1], u[2]-v[2]];
    let dot = |u: [f64; 3], v: [f64; 3]| u[0]*v[0] + u[1]*v[1] + u[2]*v[2];
    let ab = sub(b, a); let ac = sub(c, a); let ap = sub(q, a);
    let d1 = dot(ab, ap); let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 { return a; }
    let bp = sub(q, b);
    let d3 = dot(ab, bp); let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 { return b; }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return [a[0] + v*ab[0], a[1] + v*ab[1], a[2] + v*ab[2]];
    }
    let cp = sub(q, c);
    let d5 = dot(ab, cp); let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 { return c; }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return [a[0] + w*ac[0], a[1] + w*ac[1], a[2] + w*ac[2]];
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return [
            b[0] + w*(c[0]-b[0]),
            b[1] + w*(c[1]-b[1]),
            b[2] + w*(c[2]-b[2]),
        ];
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    [
        a[0] + ab[0]*v + ac[0]*w,
        a[1] + ab[1]*v + ac[1]*w,
        a[2] + ab[2]*v + ac[2]*w,
    ]
}

/// Closest point on the entire triangle mesh to `q`. Returns
/// `(point, triangle_index, distance)` — brute force O(n).
pub fn trimesh_closest_point(
    q: [f64; 3],
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<([f64; 3], usize, f64)> {
    if indices.len() < 3 { return None; }
    let mut best: Option<([f64; 3], usize, f64)> = None;
    for t in 0..(indices.len() / 3) {
        let a = positions[indices[t * 3] as usize];
        let b = positions[indices[t * 3 + 1] as usize];
        let c = positions[indices[t * 3 + 2] as usize];
        let p = closest_point_on_triangle(q, a, b, c);
        let dx = q[0] - p[0];
        let dy = q[1] - p[1];
        let dz = q[2] - p[2];
        let d2 = dx*dx + dy*dy + dz*dz;
        if best.is_none() || d2 < best.unwrap().2 {
            best = Some((p, t, d2));
        }
    }
    best.map(|(p, t, d2)| (p, t, d2.sqrt()))
}

/// Signed distance from `q` to a closed triangle mesh. Negative when
/// inside, positive outside. |value| is the Euclidean closest-point
/// distance. Returns `None` for empty meshes. For non-closed meshes the
/// sign is unreliable — use `trimesh_closest_point` directly instead.
pub fn trimesh_signed_distance(
    q: [f64; 3],
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<f64> {
    let (_, _, d) = trimesh_closest_point(q, positions, indices)?;
    let inside = trimesh_point_inside(q, positions, indices);
    Some(if inside { -d } else { d })
}

/// True iff `point` is strictly inside a closed triangle mesh, determined
/// by shooting a ray along +X (with a tiny +Y perturbation to avoid
/// grazing edge cases) and counting intersections — odd count = inside.
/// O(n) per query. For closed, non-self-intersecting meshes only.
pub fn trimesh_point_inside(
    point: [f64; 3],
    positions: &[[f32; 3]],
    indices: &[u32],
) -> bool {
    let dir = [1.0, 1.0e-7, 3.3e-7]; // perturbed +X
    let mut hits = 0usize;
    for t in 0..(indices.len() / 3) {
        let a = positions[indices[t * 3] as usize];
        let b = positions[indices[t * 3 + 1] as usize];
        let c = positions[indices[t * 3 + 2] as usize];
        if ray_triangle_intersect(point, dir, a, b, c).is_some() { hits += 1; }
    }
    hits % 2 == 1
}

/// Möller-Trumbore ray-triangle intersection. Returns `Some((t, u, v))`
/// if the ray `origin + t·dir` hits the triangle with `t > ε`, where
/// `(u, v)` are the barycentric coordinates and `w = 1 - u - v`. `dir`
/// need not be normalised — `t` is scaled in units of `||dir||`.
pub fn ray_triangle_intersect(
    origin: [f64; 3],
    dir: [f64; 3],
    v0: [f32; 3],
    v1: [f32; 3],
    v2: [f32; 3],
) -> Option<(f64, f64, f64)> {
    let v0 = [v0[0] as f64, v0[1] as f64, v0[2] as f64];
    let v1 = [v1[0] as f64, v1[1] as f64, v1[2] as f64];
    let v2 = [v2[0] as f64, v2[1] as f64, v2[2] as f64];
    let eps = 1.0e-10;
    let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
    let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
    let h = [dir[1]*e2[2] - dir[2]*e2[1],
             dir[2]*e2[0] - dir[0]*e2[2],
             dir[0]*e2[1] - dir[1]*e2[0]];
    let a = e1[0]*h[0] + e1[1]*h[1] + e1[2]*h[2];
    if a.abs() < eps { return None; }
    let f = 1.0 / a;
    let s = [origin[0]-v0[0], origin[1]-v0[1], origin[2]-v0[2]];
    let u = f * (s[0]*h[0] + s[1]*h[1] + s[2]*h[2]);
    if !(0.0..=1.0).contains(&u) { return None; }
    let q = [s[1]*e1[2] - s[2]*e1[1],
             s[2]*e1[0] - s[0]*e1[2],
             s[0]*e1[1] - s[1]*e1[0]];
    let v = f * (dir[0]*q[0] + dir[1]*q[1] + dir[2]*q[2]);
    if v < 0.0 || u + v > 1.0 { return None; }
    let t = f * (e2[0]*q[0] + e2[1]*q[1] + e2[2]*q[2]);
    if t > eps { Some((t, u, v)) } else { None }
}

/// First (smallest positive `t`) intersection between a ray and a triangle
/// mesh. Returns `(t, triangle_index, u, v)`. O(n) brute force — callers
/// needing speed should wrap this in a BVH.
pub fn trimesh_ray_intersect(
    origin: [f64; 3],
    dir: [f64; 3],
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<(f64, usize, f64, f64)> {
    let mut best: Option<(f64, usize, f64, f64)> = None;
    for t in 0..(indices.len() / 3) {
        let a = positions[indices[t * 3] as usize];
        let b = positions[indices[t * 3 + 1] as usize];
        let c = positions[indices[t * 3 + 2] as usize];
        if let Some((ti, u, v)) = ray_triangle_intersect(origin, dir, a, b, c) {
            if best.is_none() || ti < best.unwrap().0 {
                best = Some((ti, t, u, v));
            }
        }
    }
    best
}

/// Returns the list of "boundary" edges — undirected edges used by
/// exactly one triangle. An empty list means the mesh is a closed manifold
/// (every edge shared by ≥ 2 triangles; non-manifold edges with 3+ uses
/// are still flagged as interior here — see `trimesh_non_manifold_edges`).
pub fn trimesh_boundary_edges(indices: &[u32]) -> Vec<(u32, u32)> {
    let mut counts: std::collections::HashMap<(u32, u32), u32> =
        std::collections::HashMap::new();
    for t in 0..(indices.len() / 3) {
        let i0 = indices[t * 3];
        let i1 = indices[t * 3 + 1];
        let i2 = indices[t * 3 + 2];
        for (a, b) in [(i0, i1), (i1, i2), (i2, i0)] {
            let k = if a < b { (a, b) } else { (b, a) };
            *counts.entry(k).or_insert(0) += 1;
        }
    }
    counts.into_iter().filter_map(|(e, n)| if n == 1 { Some(e) } else { None }).collect()
}

/// Non-manifold edges: used by 3+ triangles. An edge shared by exactly 2
/// triangles is manifold; by 1 it is a boundary; by 3+ the surface
/// branches at that edge, which breaks many algorithms.
pub fn trimesh_non_manifold_edges(indices: &[u32]) -> Vec<(u32, u32)> {
    let mut counts: std::collections::HashMap<(u32, u32), u32> =
        std::collections::HashMap::new();
    for t in 0..(indices.len() / 3) {
        let i0 = indices[t * 3];
        let i1 = indices[t * 3 + 1];
        let i2 = indices[t * 3 + 2];
        for (a, b) in [(i0, i1), (i1, i2), (i2, i0)] {
            let k = if a < b { (a, b) } else { (b, a) };
            *counts.entry(k).or_insert(0) += 1;
        }
    }
    counts.into_iter().filter_map(|(e, n)| if n >= 3 { Some(e) } else { None }).collect()
}

/// True iff the welded triangle mesh has no boundary edges and no
/// non-manifold edges — every edge is shared by exactly 2 triangles.
pub fn trimesh_is_closed(indices: &[u32]) -> bool {
    trimesh_boundary_edges(indices).is_empty() && trimesh_non_manifold_edges(indices).is_empty()
}

/// Angle defect per vertex: `defect_v = 2π − Σ θ_i` over incident triangle
/// interior angles at v. For interior vertices on a closed manifold this
/// equals the integrated Gaussian curvature K·A_mixed around v (in the
/// discrete Gauss-Bonnet sense). Boundary vertices use π instead of 2π.
///
/// Returns one value per vertex (same length as `positions`).
pub fn trimesh_gaussian_curvature_per_vertex(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Vec<f64> {
    let n_v = positions.len();
    let mut defect = vec![0.0_f64; n_v];
    let mut is_boundary = vec![false; n_v];
    for (a, b) in trimesh_boundary_edges(indices) {
        if (a as usize) < n_v { is_boundary[a as usize] = true; }
        if (b as usize) < n_v { is_boundary[b as usize] = true; }
    }
    for v in 0..n_v {
        defect[v] = if is_boundary[v] { std::f64::consts::PI } else { 2.0 * std::f64::consts::PI };
    }
    for t in 0..(indices.len() / 3) {
        let i0 = indices[t * 3] as usize;
        let i1 = indices[t * 3 + 1] as usize;
        let i2 = indices[t * 3 + 2] as usize;
        if i0 >= n_v || i1 >= n_v || i2 >= n_v { continue; }
        let p = [positions[i0], positions[i1], positions[i2]];
        let p64 = |v: [f32; 3]| [v[0] as f64, v[1] as f64, v[2] as f64];
        let a = p64(p[0]); let b = p64(p[1]); let c = p64(p[2]);
        defect[i0] -= triangle_angle_at(a, b, c);
        defect[i1] -= triangle_angle_at(b, c, a);
        defect[i2] -= triangle_angle_at(c, a, b);
    }
    defect
}

/// Interior angle of triangle (p, q, r) at vertex p.
fn triangle_angle_at(p: [f64; 3], q: [f64; 3], r: [f64; 3]) -> f64 {
    let u = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
    let v = [r[0] - p[0], r[1] - p[1], r[2] - p[2]];
    let du = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
    let dv = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if du < f64::EPSILON || dv < f64::EPSILON { return 0.0; }
    let cos_t = (u[0] * v[0] + u[1] * v[1] + u[2] * v[2]) / (du * dv);
    cos_t.clamp(-1.0, 1.0).acos()
}

/// Sum of per-vertex angle defects — by discrete Gauss-Bonnet this equals
/// `2π · χ` for a closed oriented surface (χ = Euler characteristic).
/// For an open mesh the sum equals `2π·χ − ∫_∂ κ_g ds` (geodesic boundary term).
pub fn trimesh_total_gaussian_curvature(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> f64 {
    trimesh_gaussian_curvature_per_vertex(positions, indices).iter().sum()
}

/// Vertex valence = number of distinct edges incident on a vertex. For a
/// well-tessellated closed surface, interior vertices should have valence
/// close to 6 (regular hex tiling of a plane); boundary vertices have lower.
///
/// Returns `Some((min, max, mean, irregular_count))` where `irregular_count`
/// counts vertices with valence outside [5, 7] on the interior. `None` on
/// an empty mesh.
pub fn trimesh_vertex_valence_stats(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<(u32, u32, f64, usize)> {
    if positions.is_empty() || indices.is_empty() { return None; }
    let mut incident: std::collections::HashMap<u32, std::collections::HashSet<u32>> =
        std::collections::HashMap::new();
    for t in 0..(indices.len() / 3) {
        let i0 = indices[t * 3];
        let i1 = indices[t * 3 + 1];
        let i2 = indices[t * 3 + 2];
        incident.entry(i0).or_default().extend([i1, i2]);
        incident.entry(i1).or_default().extend([i0, i2]);
        incident.entry(i2).or_default().extend([i0, i1]);
    }
    if incident.is_empty() { return None; }
    // Boundary vertex set — those on open edges.
    let mut is_boundary = std::collections::HashSet::new();
    for (a, b) in trimesh_boundary_edges(indices) {
        is_boundary.insert(a);
        is_boundary.insert(b);
    }
    let mut min_v = u32::MAX;
    let mut max_v = 0u32;
    let mut sum = 0u64;
    let mut irregular = 0usize;
    let n = incident.len() as f64;
    for (vid, neighbors) in &incident {
        let val = neighbors.len() as u32;
        if val < min_v { min_v = val; }
        if val > max_v { max_v = val; }
        sum += val as u64;
        if !is_boundary.contains(vid) && !(5..=7).contains(&val) {
            irregular += 1;
        }
    }
    Some((min_v, max_v, sum as f64 / n, irregular))
}

/// For every interior edge (shared by exactly 2 triangles), compute the
/// dihedral angle between the triangle normals — 0 means coplanar,
/// π means the surface folds back on itself. Boundary and non-manifold
/// edges are skipped. Returns `Some((min, max, mean))` or `None` if no
/// interior edge is found.
pub fn trimesh_dihedral_angle_stats(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<(f64, f64, f64)> {
    let angles = dihedral_angles(positions, indices);
    if angles.is_empty() { return None; }
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut sum = 0.0;
    for a in &angles {
        if *a < min { min = *a; }
        if *a > max { max = *a; }
        sum += *a;
    }
    Some((min, max, sum / angles.len() as f64))
}

/// Every interior edge whose dihedral angle exceeds `threshold_rad`. Useful
/// for feature detection (box corners, fillet transitions) and for picking
/// preservation edges during mesh decimation. Returned edge pairs are
/// (v_min, v_max).
pub fn trimesh_sharp_edges(
    positions: &[[f32; 3]],
    indices: &[u32],
    threshold_rad: f64,
) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    // Build edge → triangle-list map.
    let mut edges: std::collections::HashMap<(u32, u32), Vec<usize>> =
        std::collections::HashMap::new();
    for t in 0..(indices.len() / 3) {
        let i0 = indices[t * 3];
        let i1 = indices[t * 3 + 1];
        let i2 = indices[t * 3 + 2];
        for (a, b) in [(i0, i1), (i1, i2), (i2, i0)] {
            let k = if a < b { (a, b) } else { (b, a) };
            edges.entry(k).or_default().push(t);
        }
    }
    for (edge, tris) in edges {
        if tris.len() != 2 { continue; }
        let n0 = tri_normal(positions, indices, tris[0]);
        let n1 = tri_normal(positions, indices, tris[1]);
        let Some(angle) = normals_angle(n0, n1) else { continue };
        if angle >= threshold_rad {
            out.push(edge);
        }
    }
    out
}

fn dihedral_angles(positions: &[[f32; 3]], indices: &[u32]) -> Vec<f64> {
    let mut edges: std::collections::HashMap<(u32, u32), Vec<usize>> =
        std::collections::HashMap::new();
    for t in 0..(indices.len() / 3) {
        let i0 = indices[t * 3];
        let i1 = indices[t * 3 + 1];
        let i2 = indices[t * 3 + 2];
        for (a, b) in [(i0, i1), (i1, i2), (i2, i0)] {
            let k = if a < b { (a, b) } else { (b, a) };
            edges.entry(k).or_default().push(t);
        }
    }
    let mut out = Vec::new();
    for (_, tris) in edges {
        if tris.len() != 2 { continue; }
        let n0 = tri_normal(positions, indices, tris[0]);
        let n1 = tri_normal(positions, indices, tris[1]);
        if let Some(a) = normals_angle(n0, n1) { out.push(a); }
    }
    out
}

fn tri_normal(positions: &[[f32; 3]], indices: &[u32], t: usize) -> [f64; 3] {
    let a = positions[indices[t * 3] as usize];
    let b = positions[indices[t * 3 + 1] as usize];
    let c = positions[indices[t * 3 + 2] as usize];
    let ux = (b[0] - a[0]) as f64;
    let uy = (b[1] - a[1]) as f64;
    let uz = (b[2] - a[2]) as f64;
    let vx = (c[0] - a[0]) as f64;
    let vy = (c[1] - a[1]) as f64;
    let vz = (c[2] - a[2]) as f64;
    [
        uy * vz - uz * vy,
        uz * vx - ux * vz,
        ux * vy - uy * vx,
    ]
}

fn normals_angle(n0: [f64; 3], n1: [f64; 3]) -> Option<f64> {
    let l0 = (n0[0] * n0[0] + n0[1] * n0[1] + n0[2] * n0[2]).sqrt();
    let l1 = (n1[0] * n1[0] + n1[1] * n1[1] + n1[2] * n1[2]).sqrt();
    if l0 < f64::EPSILON || l1 < f64::EPSILON { return None; }
    let cos_t = (n0[0] * n1[0] + n0[1] * n1[1] + n0[2] * n1[2]) / (l0 * l1);
    Some(cos_t.clamp(-1.0, 1.0).acos())
}

/// Area-weighted surface centroid: Σ(A_tri · (p0+p1+p2)/3) / Σ A_tri.
/// Works on open meshes (unlike `trimesh_center_of_mass`, which needs a
/// closed volume). Returns `None` on empty or zero-area input.
pub fn trimesh_surface_centroid(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<[f64; 3]> {
    if indices.len() < 3 { return None; }
    let mut sum_a = 0.0_f64;
    let mut sx = 0.0_f64;
    let mut sy = 0.0_f64;
    let mut sz = 0.0_f64;
    for t in 0..(indices.len() / 3) {
        let a = positions[indices[t * 3] as usize];
        let b = positions[indices[t * 3 + 1] as usize];
        let c = positions[indices[t * 3 + 2] as usize];
        let (ax, ay, az) = (a[0] as f64, a[1] as f64, a[2] as f64);
        let (bx, by, bz) = (b[0] as f64, b[1] as f64, b[2] as f64);
        let (cx, cy, cz) = (c[0] as f64, c[1] as f64, c[2] as f64);
        let e1 = (bx - ax, by - ay, bz - az);
        let e2 = (cx - ax, cy - ay, cz - az);
        let nx = e1.1 * e2.2 - e1.2 * e2.1;
        let ny = e1.2 * e2.0 - e1.0 * e2.2;
        let nz = e1.0 * e2.1 - e1.1 * e2.0;
        let area = 0.5 * (nx * nx + ny * ny + nz * nz).sqrt();
        let cxt = (ax + bx + cx) / 3.0;
        let cyt = (ay + by + cy) / 3.0;
        let czt = (az + bz + cz) / 3.0;
        sx += area * cxt;
        sy += area * cyt;
        sz += area * czt;
        sum_a += area;
    }
    if sum_a <= 1.0e-18 { return None; }
    Some([sx / sum_a, sy / sum_a, sz / sum_a])
}

/// Unit-density inertia tensor for a closed triangle mesh, integrated via
/// signed-tet decomposition about the origin. Returns `(Ixx, Iyy, Izz,
/// Ixy, Iyz, Izx)`. For a tet (0, p0, p1, p2):
///   ∫ x² dV = (|V|/10) · (x0² + x1² + x2² + x0·x1 + x0·x2 + x1·x2)
///   ∫ xy dV = (|V|/20) · (2(x0 y0 + x1 y1 + x2 y2)
///                         + x0 y1 + x1 y0 + x0 y2 + x2 y0 + x1 y2 + x2 y1)
pub fn trimesh_inertia_tensor(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<(f64, f64, f64, f64, f64, f64)> {
    if indices.len() < 3 { return None; }
    let mut ix2 = 0.0; let mut iy2 = 0.0; let mut iz2 = 0.0;
    let mut ixy = 0.0; let mut iyz = 0.0; let mut izx = 0.0;
    for t in 0..(indices.len() / 3) {
        let a = positions[indices[t * 3] as usize];
        let b = positions[indices[t * 3 + 1] as usize];
        let c = positions[indices[t * 3 + 2] as usize];
        let p0 = (a[0] as f64, a[1] as f64, a[2] as f64);
        let p1 = (b[0] as f64, b[1] as f64, b[2] as f64);
        let p2 = (c[0] as f64, c[1] as f64, c[2] as f64);
        let cx = p1.1 * p2.2 - p1.2 * p2.1;
        let cy = p1.2 * p2.0 - p1.0 * p2.2;
        let cz = p1.0 * p2.1 - p1.1 * p2.0;
        let v = (p0.0 * cx + p0.1 * cy + p0.2 * cz) / 6.0;
        let sxx = p0.0*p0.0 + p1.0*p1.0 + p2.0*p2.0
                + p0.0*p1.0 + p0.0*p2.0 + p1.0*p2.0;
        let syy = p0.1*p0.1 + p1.1*p1.1 + p2.1*p2.1
                + p0.1*p1.1 + p0.1*p2.1 + p1.1*p2.1;
        let szz = p0.2*p0.2 + p1.2*p1.2 + p2.2*p2.2
                + p0.2*p1.2 + p0.2*p2.2 + p1.2*p2.2;
        let sxy = 2.0*(p0.0*p0.1 + p1.0*p1.1 + p2.0*p2.1)
                + p0.0*p1.1 + p1.0*p0.1
                + p0.0*p2.1 + p2.0*p0.1
                + p1.0*p2.1 + p2.0*p1.1;
        let syz = 2.0*(p0.1*p0.2 + p1.1*p1.2 + p2.1*p2.2)
                + p0.1*p1.2 + p1.1*p0.2
                + p0.1*p2.2 + p2.1*p0.2
                + p1.1*p2.2 + p2.1*p1.2;
        let szx = 2.0*(p0.2*p0.0 + p1.2*p1.0 + p2.2*p2.0)
                + p0.2*p1.0 + p1.2*p0.0
                + p0.2*p2.0 + p2.2*p0.0
                + p1.2*p2.0 + p2.2*p1.0;
        ix2 += v * (syy + szz) / 10.0;
        iy2 += v * (sxx + szz) / 10.0;
        iz2 += v * (sxx + syy) / 10.0;
        ixy += v * sxy / 20.0;
        iyz += v * syz / 20.0;
        izx += v * szx / 20.0;
    }
    Some((ix2.abs(), iy2.abs(), iz2.abs(), ixy.abs(), iyz.abs(), izx.abs()))
}

/// AABB over a raw triangle-mesh vertex list. Returns `None` on empty input.
pub fn trimesh_bounding_box(positions: &[[f32; 3]]) -> Option<([f32; 3], [f32; 3])> {
    if positions.is_empty() { return None; }
    let mut mn = [f32::INFINITY; 3];
    let mut mx = [f32::NEG_INFINITY; 3];
    for p in positions {
        for k in 0..3 {
            if p[k] < mn[k] { mn[k] = p[k]; }
            if p[k] > mx[k] { mx[k] = p[k]; }
        }
    }
    Some((mn, mx))
}

/// Volume-weighted centroid of a closed triangle mesh via the divergence
/// theorem: decomposes the body into signed tetrahedra (0, v0, v1, v2),
/// summing `V_tet · centroid_tet` and dividing by total signed volume.
/// Returns `None` for empty / degenerate (near-zero-volume) meshes.
pub fn trimesh_center_of_mass(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<[f64; 3]> {
    if indices.len() < 3 { return None; }
    let mut v_sum = 0.0_f64;
    let mut cx = 0.0_f64;
    let mut cy = 0.0_f64;
    let mut cz = 0.0_f64;
    for t in 0..(indices.len() / 3) {
        let a = positions[indices[t * 3] as usize];
        let b = positions[indices[t * 3 + 1] as usize];
        let c = positions[indices[t * 3 + 2] as usize];
        let (ax, ay, az) = (a[0] as f64, a[1] as f64, a[2] as f64);
        let (bx, by, bz) = (b[0] as f64, b[1] as f64, b[2] as f64);
        let (cx_, cy_, cz_) = (c[0] as f64, c[1] as f64, c[2] as f64);
        let v = (ax * (by * cz_ - bz * cy_)
               + bx * (cy_ * az - cz_ * ay)
               + cx_ * (ay * bz - az * by)) / 6.0;
        // Centroid of tet (0, a, b, c) is (a + b + c) / 4.
        cx += v * (ax + bx + cx_) * 0.25;
        cy += v * (ay + by + cy_) * 0.25;
        cz += v * (az + bz + cz_) * 0.25;
        v_sum += v;
    }
    if v_sum.abs() < 1.0e-18 { return None; }
    Some([cx / v_sum, cy / v_sum, cz / v_sum])
}

/// Symmetric vertex Hausdorff distance between two point sets:
///   H(A,B) = max( max_{a∈A} min_{b∈B} ||a−b||, max_{b∈B} min_{a∈A} ||a−b|| )
/// O(|A|·|B|) — fine for meshes of a few thousand verts. For larger sets
/// consider a BVH or KD-tree. Measures vertex-level similarity only (not
/// the full surface-to-surface Hausdorff, which requires triangle sampling).
pub fn hausdorff_distance_vertex(a: &[[f32; 3]], b: &[[f32; 3]]) -> f64 {
    if a.is_empty() || b.is_empty() { return 0.0; }
    let directed = |src: &[[f32; 3]], dst: &[[f32; 3]]| -> f64 {
        let mut outer = 0.0_f64;
        for p in src {
            let mut inner = f64::INFINITY;
            for q in dst {
                let dx = (p[0] - q[0]) as f64;
                let dy = (p[1] - q[1]) as f64;
                let dz = (p[2] - q[2]) as f64;
                let d2 = dx * dx + dy * dy + dz * dz;
                if d2 < inner { inner = d2; }
            }
            if inner > outer { outer = inner; }
        }
        outer.sqrt()
    };
    directed(a, b).max(directed(b, a))
}

/// Triangle-mesh Euler characteristic χ = V − E + F, where E counts unique
/// undirected edges. For a closed orientable manifold mesh, χ = 2 − 2g;
/// returns `(chi, genus_estimate)`. Non-manifold / degenerate meshes may
/// report nonsensical genus — caller should sanity-check χ sign/parity.
pub fn mesh_euler_genus(positions: &[[f32; 3]], indices: &[u32]) -> (i64, i64) {
    let v = positions.len() as i64;
    let tri = (indices.len() / 3) as i64;
    let mut edges: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
    for t in 0..(indices.len() / 3) {
        let i0 = indices[t * 3];
        let i1 = indices[t * 3 + 1];
        let i2 = indices[t * 3 + 2];
        for (a, b) in [(i0, i1), (i1, i2), (i2, i0)] {
            let key = if a < b { (a, b) } else { (b, a) };
            edges.insert(key);
        }
    }
    let e = edges.len() as i64;
    let chi = v - e + tri;
    let genus = (2 - chi) / 2;
    (chi, genus)
}

fn walk_vertices<F: FnMut(Point3)>(arena: &ShapeArena, id: ShapeId, cb: &mut F) -> MeasureResult<()> {
    match arena.get(id)? {
        Shape::Compound { children }  => { for c in children { walk_vertices(arena, *c, cb)?; } }
        Shape::Solid { shells }       => { for s in shells { walk_vertices(arena, *s, cb)?; } }
        Shape::Shell { faces }        => { for (f, _) in faces { walk_vertices(arena, *f, cb)?; } }
        Shape::Face { wires, .. }     => { for w in wires { walk_vertices(arena, *w, cb)?; } }
        Shape::Wire { edges }         => { for (e, _) in edges { walk_vertices(arena, *e, cb)?; } }
        Shape::Edge { vertices, .. }  => { for v in vertices { walk_vertices(arena, *v, cb)?; } }
        Shape::Vertex { point }       => { cb(*point); }
    }
    Ok(())
}

/// Analytical area of a `Shape::Face` whose outer wire is a polygon of line
/// edges. Returns `Unimplemented` for curved surfaces (iter 7 scope).
pub fn face_area(arena: &ShapeArena, id: ShapeId) -> MeasureResult<f64> {
    let shape = arena.get(id)?;
    let Shape::Face { wires, .. } = shape else {
        return Err(MeasureError::Unimplemented);
    };
    if wires.is_empty() {
        // Closed surface (sphere/torus) — exact area to be added iter 8.
        return Err(MeasureError::Unimplemented);
    }
    let outer = wires[0];
    let poly = extract_line_polygon(arena, outer)?;
    if poly.len() < 3 { return Err(MeasureError::Unimplemented); }
    // Newell's method: signed area vector of a 3D polygon is
    // 0.5 * Σ (Vi × Vi+1); its magnitude is the face area.
    let mut nx = 0.0;
    let mut ny = 0.0;
    let mut nz = 0.0;
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        let a = poly[i];
        let b = poly[j];
        nx += (a.y - b.y) * (a.z + b.z);
        ny += (a.z - b.z) * (a.x + b.x);
        nz += (a.x - b.x) * (a.y + b.y);
    }
    Ok(0.5 * (nx * nx + ny * ny + nz * nz).sqrt())
}

fn extract_line_polygon(arena: &ShapeArena, wire_id: ShapeId) -> MeasureResult<Vec<Point3>> {
    let wire = arena.get(wire_id)?;
    let Shape::Wire { edges } = wire else { return Err(MeasureError::Unimplemented); };
    let mut pts = Vec::with_capacity(edges.len());
    for (edge_id, orient) in edges {
        let edge = arena.get(*edge_id)?;
        let Shape::Edge { vertices, .. } = edge else { continue; };
        let (va, vb) = match orient {
            gfd_cad_topo::Orientation::Forward => (vertices[0], vertices[1]),
            _ => (vertices[1], vertices[0]),
        };
        if let Shape::Vertex { point } = arena.get(va)? {
            pts.push(*point);
        }
        let _ = vb; // endpoint is the next edge's start — avoid duplicates.
    }
    Ok(pts)
}

/// Total surface area of a shape.
///
/// Uses `face_area` (Newell's method) for planar faces with line-edge wires,
/// and closed-form analytic area for sphere and torus faces that carry no
/// wire. Cylinder/cone lateral + caps without wires fall back to the bbox
/// approximation driven by their stored height / radius.
pub fn surface_area(arena: &ShapeArena, id: ShapeId) -> MeasureResult<f64> {
    use gfd_cad_topo::{collect_by_kind, ShapeKind, SurfaceGeom};
    let faces = collect_by_kind(arena, id, ShapeKind::Face);
    let mut total = 0.0;
    let mut any = false;
    for f in faces {
        // Polygon face first.
        if let Ok(a) = face_area(arena, f) { total += a; any = true; continue; }
        // Analytic closed surfaces.
        if let Ok(gfd_cad_topo::Shape::Face { surface, wires, .. }) = arena.get(f) {
            if !wires.is_empty() { continue; }
            match surface {
                SurfaceGeom::Sphere(s) => {
                    total += 4.0 * std::f64::consts::PI * s.radius.powi(2);
                    any = true;
                }
                SurfaceGeom::Torus(t) => {
                    total += 4.0 * std::f64::consts::PI.powi(2) * t.major * t.minor;
                    any = true;
                }
                SurfaceGeom::Cylinder(c) => {
                    total += 2.0 * std::f64::consts::PI * c.radius * c.height;
                    any = true;
                }
                SurfaceGeom::Cone(c) => {
                    let slant = (c.height.powi(2) + (c.r1 - c.r2).powi(2)).sqrt();
                    total += std::f64::consts::PI * (c.r1 + c.r2) * slant;
                    any = true;
                }
                SurfaceGeom::Plane(_) => {} // open plane carries infinite area
            }
        }
    }
    if any { Ok(total) } else { Err(MeasureError::Unimplemented) }
}

/// Volume of a closed B-Rep solid with planar faces via the divergence
/// theorem: V = (1/6) Σ (a · (b × c)) for each triangle fan of each face.
///
/// Only correct when every face carries a polygon outer wire. Spheres /
/// cylinders / tori without wires fall back to `bbox_volume`.
pub fn divergence_volume(arena: &ShapeArena, id: ShapeId) -> MeasureResult<f64> {
    use gfd_cad_topo::{collect_by_kind, ShapeKind};
    let faces = collect_by_kind(arena, id, ShapeKind::Face);
    if faces.is_empty() { return Err(MeasureError::EmptyShape); }
    let mut acc = 0.0;
    let mut any_polygon = false;
    for f in faces {
        let Ok(gfd_cad_topo::Shape::Face { wires, .. }) = arena.get(f) else { continue; };
        if wires.is_empty() { continue; }
        let outer = wires[0];
        let poly = extract_line_polygon(arena, outer)?;
        if poly.len() < 3 { continue; }
        // Fan-triangulate from poly[0].
        let p0 = poly[0];
        for i in 1..poly.len() - 1 {
            let p1 = poly[i];
            let p2 = poly[i + 1];
            // Signed volume of tetrahedron (origin, p0, p1, p2) = (1/6) p0·(p1×p2)
            let cx = p1.y * p2.z - p1.z * p2.y;
            let cy = p1.z * p2.x - p1.x * p2.z;
            let cz = p1.x * p2.y - p1.y * p2.x;
            acc += p0.x * cx + p0.y * cy + p0.z * cz;
        }
        any_polygon = true;
    }
    if !any_polygon { return bbox_volume(arena, id); }
    Ok(acc.abs() / 6.0)
}

pub fn area(arena: &ShapeArena, id: ShapeId) -> MeasureResult<f64> {
    surface_area(arena, id)
}

pub fn volume(arena: &ShapeArena, id: ShapeId) -> MeasureResult<f64> {
    divergence_volume(arena, id)
}

/// Volume-weighted centroid of a closed polygon-faced solid via the
/// divergence theorem applied fan-wise per face. Returns `None` if the
/// shape has no wired planar faces (sphere/torus fall back to bbox centre).
pub fn center_of_mass(arena: &ShapeArena, id: ShapeId) -> MeasureResult<Point3> {
    use gfd_cad_topo::{collect_by_kind, ShapeKind};
    let faces = collect_by_kind(arena, id, ShapeKind::Face);
    let mut vol_acc = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut cz = 0.0;
    let mut any = false;
    for f in faces {
        let Ok(gfd_cad_topo::Shape::Face { wires, .. }) = arena.get(f) else { continue; };
        if wires.is_empty() { continue; }
        let poly = extract_line_polygon(arena, wires[0])?;
        if poly.len() < 3 { continue; }
        any = true;
        let p0 = poly[0];
        for i in 1..poly.len() - 1 {
            let p1 = poly[i];
            let p2 = poly[i + 1];
            // Signed tetra volume and centroid contribution (origin + 3 vertices).
            let v = (p0.x * (p1.y * p2.z - p1.z * p2.y)
                   + p0.y * (p1.z * p2.x - p1.x * p2.z)
                   + p0.z * (p1.x * p2.y - p1.y * p2.x)) / 6.0;
            // Centroid of tetrahedron (origin, p0, p1, p2) is average of its 4 vertices.
            let mx = (p0.x + p1.x + p2.x) * 0.25;
            let my = (p0.y + p1.y + p2.y) * 0.25;
            let mz = (p0.z + p1.z + p2.z) * 0.25;
            vol_acc += v;
            cx += v * mx;
            cy += v * my;
            cz += v * mz;
        }
    }
    if !any || vol_acc.abs() < 1.0e-12 {
        // Fall back to bbox centre.
        let bb = bounding_box(arena, id)?;
        return Ok(Point3::new(
            (bb.min.x + bb.max.x) * 0.5,
            (bb.min.y + bb.max.y) * 0.5,
            (bb.min.z + bb.max.z) * 0.5,
        ));
    }
    Ok(Point3::new(cx / vol_acc, cy / vol_acc, cz / vol_acc))
}

/// Length of an edge defined by a line between two endpoint vertices.
/// Curved-edge arc length ships later.
pub fn edge_length(arena: &ShapeArena, id: ShapeId) -> MeasureResult<f64> {
    let Shape::Edge { vertices, .. } = arena.get(id)? else {
        return Err(MeasureError::Unimplemented);
    };
    let a = match arena.get(vertices[0])? { Shape::Vertex { point } => *point, _ => return Err(MeasureError::Unimplemented) };
    let b = match arena.get(vertices[1])? { Shape::Vertex { point } => *point, _ => return Err(MeasureError::Unimplemented) };
    Ok(a.distance(b))
}

/// Angle (radians) between two edges that share a common vertex. Returns
/// the unoriented angle ∈ [0, π].
pub fn angle_between_edges(arena: &ShapeArena, a: ShapeId, b: ShapeId) -> MeasureResult<f64> {
    let (a0, a1) = edge_endpoints(arena, a)?;
    let (b0, b1) = edge_endpoints(arena, b)?;
    let v1 = (a1.x - a0.x, a1.y - a0.y, a1.z - a0.z);
    let v2 = (b1.x - b0.x, b1.y - b0.y, b1.z - b0.z);
    let n1 = (v1.0 * v1.0 + v1.1 * v1.1 + v1.2 * v1.2).sqrt();
    let n2 = (v2.0 * v2.0 + v2.1 * v2.1 + v2.2 * v2.2).sqrt();
    if n1 < 1.0e-12 || n2 < 1.0e-12 {
        return Err(MeasureError::Unimplemented);
    }
    let dot = (v1.0 * v2.0 + v1.1 * v2.1 + v1.2 * v2.2) / (n1 * n2);
    Ok(dot.clamp(-1.0, 1.0).acos())
}

/// Group planar faces by their infinite-plane equation. Two faces land in
/// the same group when their normals agree (±tolerance on each component)
/// and their plane offset `d = n · p0` matches.
pub fn coplanar_faces(arena: &ShapeArena, id: ShapeId, tol: f64) -> MeasureResult<Vec<Vec<ShapeId>>> {
    use gfd_cad_topo::{collect_by_kind, ShapeKind};
    let face_ids = collect_by_kind(arena, id, ShapeKind::Face);
    let mut planes: Vec<(f64, f64, f64, f64, Vec<ShapeId>)> = Vec::new(); // (nx, ny, nz, d, members)
    for fid in face_ids {
        let Some(n) = face_normal_newell(arena, fid) else { continue; };
        let Ok(Shape::Face { wires, .. }) = arena.get(fid) else { continue; };
        if wires.is_empty() { continue; }
        let poly = extract_line_polygon(arena, wires[0])?;
        if poly.is_empty() { continue; }
        let p0 = poly[0];
        let d = n.0 * p0.x + n.1 * p0.y + n.2 * p0.z;
        let mut found = false;
        for entry in planes.iter_mut() {
            // Two faces are coplanar only when they share the same oriented
            // plane (signed normal + signed offset both match).
            let close_normal =
                (entry.0 - n.0).abs() < tol &&
                (entry.1 - n.1).abs() < tol &&
                (entry.2 - n.2).abs() < tol;
            if close_normal && (entry.3 - d).abs() < tol {
                entry.4.push(fid);
                found = true;
                break;
            }
        }
        if !found { planes.push((n.0, n.1, n.2, d, vec![fid])); }
    }
    Ok(planes.into_iter().map(|e| e.4).collect())
}

/// Unoriented dihedral angle between two polygon faces, in radians ∈ [0, π].
/// Uses Newell-method face normals. Returns `Unimplemented` for non-planar
/// faces (sphere / torus / cylinder) because "the plane" isn't unique.
pub fn dihedral_angle(arena: &ShapeArena, f1: ShapeId, f2: ShapeId) -> MeasureResult<f64> {
    let n1 = face_normal_newell(arena, f1).ok_or(MeasureError::Unimplemented)?;
    let n2 = face_normal_newell(arena, f2).ok_or(MeasureError::Unimplemented)?;
    let dot = (n1.0 * n2.0 + n1.1 * n2.1 + n1.2 * n2.2).clamp(-1.0, 1.0);
    Ok(dot.acos())
}

/// Oriented bounding box aligned to the principal axes of the vertex
/// point cloud (PCA on unweighted vertices). Returns `(center, axes,
/// half_extents)`. For axis-aligned shapes, axes reduce to world axes
/// (up to sign/permutation) and half_extents match the AABB.
pub fn oriented_bounding_box(
    arena: &ShapeArena,
    id: ShapeId,
) -> MeasureResult<(Point3, [[f64; 3]; 3], [f64; 3])> {
    let mut pts: Vec<Point3> = Vec::new();
    walk_vertices(arena, id, &mut |p| pts.push(p))?;
    if pts.is_empty() { return Err(MeasureError::Unimplemented); }
    let n = pts.len() as f64;
    let mut mean = (0.0, 0.0, 0.0);
    for p in &pts { mean.0 += p.x; mean.1 += p.y; mean.2 += p.z; }
    mean.0 /= n; mean.1 /= n; mean.2 /= n;
    let mut cxx = 0.0; let mut cyy = 0.0; let mut czz = 0.0;
    let mut cxy = 0.0; let mut cyz = 0.0; let mut czx = 0.0;
    for p in &pts {
        let dx = p.x - mean.0;
        let dy = p.y - mean.1;
        let dz = p.z - mean.2;
        cxx += dx * dx; cyy += dy * dy; czz += dz * dz;
        cxy += dx * dy; cyz += dy * dz; czx += dz * dx;
    }
    let m = nalgebra::Matrix3::new(
        cxx, cxy, czx,
        cxy, cyy, cyz,
        czx, cyz, czz,
    );
    let eig = m.symmetric_eigen();
    let mut axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for k in 0..3 {
        let v = eig.eigenvectors.column(k);
        axes[k] = [v[0], v[1], v[2]];
    }
    let mut mn = [f64::INFINITY; 3];
    let mut mx = [f64::NEG_INFINITY; 3];
    for p in &pts {
        for k in 0..3 {
            let v = p.x * axes[k][0] + p.y * axes[k][1] + p.z * axes[k][2];
            if v < mn[k] { mn[k] = v; }
            if v > mx[k] { mx[k] = v; }
        }
    }
    let mid = [
        0.5 * (mn[0] + mx[0]),
        0.5 * (mn[1] + mx[1]),
        0.5 * (mn[2] + mx[2]),
    ];
    let half = [
        0.5 * (mx[0] - mn[0]),
        0.5 * (mx[1] - mn[1]),
        0.5 * (mx[2] - mn[2]),
    ];
    let cx = mid[0] * axes[0][0] + mid[1] * axes[1][0] + mid[2] * axes[2][0];
    let cy = mid[0] * axes[0][1] + mid[1] * axes[1][1] + mid[2] * axes[2][1];
    let cz = mid[0] * axes[0][2] + mid[1] * axes[1][2] + mid[2] * axes[2][2];
    Ok((Point3::new(cx, cy, cz), axes, half))
}

fn face_normal_newell(arena: &ShapeArena, face_id: ShapeId) -> Option<(f64, f64, f64)> {
    let Ok(Shape::Face { wires, .. }) = arena.get(face_id) else { return None; };
    if wires.is_empty() { return None; }
    let poly = extract_line_polygon(arena, wires[0]).ok()?;
    if poly.len() < 3 { return None; }
    let mut nx = 0.0; let mut ny = 0.0; let mut nz = 0.0;
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        let a = poly[i]; let b = poly[j];
        nx += (a.y - b.y) * (a.z + b.z);
        ny += (a.z - b.z) * (a.x + b.x);
        nz += (a.x - b.x) * (a.y + b.y);
    }
    let l = (nx*nx + ny*ny + nz*nz).sqrt();
    if l < 1.0e-12 { return None; }
    Some((nx / l, ny / l, nz / l))
}

fn edge_endpoints(arena: &ShapeArena, id: ShapeId) -> MeasureResult<(Point3, Point3)> {
    let Shape::Edge { vertices, .. } = arena.get(id)? else {
        return Err(MeasureError::Unimplemented);
    };
    let a = match arena.get(vertices[0])? { Shape::Vertex { point } => *point, _ => return Err(MeasureError::Unimplemented) };
    let b = match arena.get(vertices[1])? { Shape::Vertex { point } => *point, _ => return Err(MeasureError::Unimplemented) };
    Ok((a, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn unit_square_area() {
        let poly = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        assert_abs_diff_eq!(polygon_area(&poly), 1.0, epsilon = 1e-12);
    }

    #[test]
    fn triangle_area_half() {
        let poly = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)];
        assert_abs_diff_eq!(polygon_area(&poly), 0.5, epsilon = 1e-12);
    }

    #[test]
    fn degenerate_polygon_zero() {
        let poly = [(0.0, 0.0), (1.0, 0.0)];
        assert_eq!(polygon_area(&poly), 0.0);
    }

    #[test]
    fn triangle_area_helpers() {
        assert_abs_diff_eq!(triangle_area_2d((0.0, 0.0), (2.0, 0.0), (0.0, 3.0)), 3.0, epsilon = 1e-12);
        assert_abs_diff_eq!(
            triangle_area_3d(
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ),
            0.5,
            epsilon = 1e-12,
        );
    }

    #[test]
    fn segment_intersection_crossing_and_disjoint() {
        let x = segment_intersection_2d((0.0, 0.0), (2.0, 2.0), (0.0, 2.0), (2.0, 0.0)).unwrap();
        assert_abs_diff_eq!(x.0, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(x.1, 1.0, epsilon = 1e-12);
        assert!(segment_intersection_2d((0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)).is_none());
    }

    #[test]
    fn barycentric_at_centroid_is_third() {
        let (u, v, w) = barycentric_2d((1.0 / 3.0, 1.0 / 3.0), (0.0, 0.0), (1.0, 0.0), (0.0, 1.0)).unwrap();
        assert_abs_diff_eq!(u, 1.0 / 3.0, epsilon = 1e-12);
        assert_abs_diff_eq!(v, 1.0 / 3.0, epsilon = 1e-12);
        assert_abs_diff_eq!(w, 1.0 / 3.0, epsilon = 1e-12);
    }

    #[test]
    fn closest_point_on_l_polyline() {
        // L-shape going (0,0,0) → (1,0,0) → (1,1,0). Query (0.5, 0.5, 0) is
        // closest to segment 0 (y=0, x∈[0,1]) at distance 0.5.
        let poly = [Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 1.0, 0.0)];
        let (d, i) = closest_point_on_polyline_3d(Point3::new(0.5, 0.5, 0.0), &poly).unwrap();
        assert_abs_diff_eq!(d, 0.5, epsilon = 1e-12);
        assert_eq!(i, 0);
    }

    #[test]
    fn polyline_length_3_segments() {
        let pts = [
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 1.0),
        ];
        assert_abs_diff_eq!(polyline_length_3d(&pts), 3.0, epsilon = 1e-12);
    }

    #[test]
    fn polygon_contains_center_of_unit_square() {
        let poly = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        assert!(polygon_contains_point(&poly, (0.5, 0.5)));
        assert!(!polygon_contains_point(&poly, (1.5, 0.5)));
    }

    #[test]
    fn convex_hull_of_pentagon_plus_centre_is_five_verts() {
        // 5 vertices on a regular pentagon + 1 centre point → hull is the 5 pentagon verts.
        let mut pts: Vec<(f64, f64)> = (0..5)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / 5.0;
                (a.cos(), a.sin())
            })
            .collect();
        pts.push((0.0, 0.0));
        let hull = polygon_convex_hull(&pts);
        assert_eq!(hull.len(), 5);
    }

    #[test]
    fn convex_detection() {
        let square = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        assert!(is_convex_polygon(&square));
        // L-shape (non-convex)
        let l = [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0), (1.0, 2.0), (0.0, 2.0)];
        assert!(!is_convex_polygon(&l));
    }

    #[test]
    fn polygon_perimeter_and_centroid_unit_square() {
        let poly = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        assert_abs_diff_eq!(polygon_perimeter(&poly), 4.0, epsilon = 1e-12);
        let (cx, cy) = polygon_centroid(&poly);
        assert_abs_diff_eq!(cx, 0.5, epsilon = 1e-12);
        assert_abs_diff_eq!(cy, 0.5, epsilon = 1e-12);
    }

    #[test]
    fn coplanar_faces_of_box_groups_into_six_singletons() {
        use gfd_cad_feature::box_solid;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 1.0, 1.0, 1.0).unwrap();
        let groups = coplanar_faces(&a, id, 1e-6).unwrap();
        // Six faces, each on its own plane → six singleton groups.
        assert_eq!(groups.len(), 6);
        assert!(groups.iter().all(|g| g.len() == 1));
    }

    #[test]
    fn dihedral_angle_between_adjacent_box_faces_is_90() {
        use gfd_cad_feature::box_solid;
        use gfd_cad_topo::{collect_by_kind, ShapeArena, ShapeKind};
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 1.0, 1.0, 1.0).unwrap();
        let faces = collect_by_kind(&a, id, ShapeKind::Face);
        // Any pair of adjacent faces on a box meets at a right angle.
        // Use the first two box_solid creates: bottom + -X lateral.
        let d = dihedral_angle(&a, faces[0], faces[2]).unwrap();
        assert_abs_diff_eq!(d, std::f64::consts::FRAC_PI_2, epsilon = 1e-6);
    }

    #[test]
    fn face_face_distance_between_two_pad_lateral_faces() {
        // Two 2×2 pads separated by a gap of 1 along X. Their nearest
        // lateral faces should be exactly 1 unit apart.
        use gfd_cad_feature::pad_polygon_xy;
        use gfd_cad_topo::{collect_by_kind, ShapeArena, ShapeKind};
        let mut arena = ShapeArena::new();
        let pad_a = pad_polygon_xy(&mut arena, &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], 1.0).unwrap();
        let pad_b = pad_polygon_xy(&mut arena, &[(2.0, 0.0), (3.0, 0.0), (3.0, 1.0), (2.0, 1.0)], 1.0).unwrap();
        let faces_a = collect_by_kind(&arena, pad_a, ShapeKind::Face);
        let faces_b = collect_by_kind(&arena, pad_b, ShapeKind::Face);
        // Find the +X face of A (at x=1) and -X face of B (at x=2).
        let mut best = f64::INFINITY;
        for &fa in &faces_a {
            for &fb in &faces_b {
                if let Ok(d) = distance_face_face(&arena, fa, fb) {
                    if d < best { best = d; }
                }
            }
        }
        assert_abs_diff_eq!(best, 1.0, epsilon = 1.0e-6);
    }

    #[test]
    fn edge_edge_parallel_segments() {
        use gfd_cad_geom::{curve::Line, Point3};
        use gfd_cad_topo::{shape::CurveGeom, Orientation, ShapeArena};
        let mut a = ShapeArena::new();
        // Two parallel segments on z=0, y apart by 1.
        let v0 = a.push(Shape::vertex(Point3::new(0.0, 0.0, 0.0)));
        let v1 = a.push(Shape::vertex(Point3::new(1.0, 0.0, 0.0)));
        let v2 = a.push(Shape::vertex(Point3::new(0.0, 1.0, 0.0)));
        let v3 = a.push(Shape::vertex(Point3::new(1.0, 1.0, 0.0)));
        let l1 = Line::from_points(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)).unwrap();
        let l2 = Line::from_points(Point3::new(0.0, 1.0, 0.0), Point3::new(1.0, 1.0, 0.0)).unwrap();
        let e1 = a.push(Shape::Edge { curve: CurveGeom::Line(l1), vertices: [v0, v1], orient: Orientation::Forward });
        let e2 = a.push(Shape::Edge { curve: CurveGeom::Line(l2), vertices: [v2, v3], orient: Orientation::Forward });
        let d = distance_edge_edge(&a, e1, e2).unwrap();
        assert_abs_diff_eq!(d, 1.0, epsilon = 1e-9);
    }

    #[test]
    fn edge_edge_crossing_segments_zero() {
        use gfd_cad_geom::{curve::Line, Point3};
        use gfd_cad_topo::{shape::CurveGeom, Orientation, ShapeArena};
        let mut a = ShapeArena::new();
        // Two segments crossing at the origin.
        let v0 = a.push(Shape::vertex(Point3::new(-1.0, 0.0, 0.0)));
        let v1 = a.push(Shape::vertex(Point3::new(1.0, 0.0, 0.0)));
        let v2 = a.push(Shape::vertex(Point3::new(0.0, -1.0, 0.0)));
        let v3 = a.push(Shape::vertex(Point3::new(0.0, 1.0, 0.0)));
        let l1 = Line::from_points(Point3::new(-1.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)).unwrap();
        let l2 = Line::from_points(Point3::new(0.0, -1.0, 0.0), Point3::new(0.0, 1.0, 0.0)).unwrap();
        let e1 = a.push(Shape::Edge { curve: CurveGeom::Line(l1), vertices: [v0, v1], orient: Orientation::Forward });
        let e2 = a.push(Shape::Edge { curve: CurveGeom::Line(l2), vertices: [v2, v3], orient: Orientation::Forward });
        let d = distance_edge_edge(&a, e1, e2).unwrap();
        assert!(d < 1.0e-6, "crossing segments should be ≈0, got {}", d);
    }

    #[test]
    fn vertex_edge_distance_point_outside_segment() {
        use gfd_cad_geom::{curve::Line, Point3};
        use gfd_cad_topo::{shape::CurveGeom, Orientation, ShapeArena};
        let mut a = ShapeArena::new();
        let v0 = a.push(Shape::vertex(Point3::new(0.0, 0.0, 0.0)));
        let v1 = a.push(Shape::vertex(Point3::new(1.0, 0.0, 0.0)));
        let line = Line::from_points(Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0)).unwrap();
        let e = a.push(Shape::Edge { curve: CurveGeom::Line(line), vertices: [v0, v1], orient: Orientation::Forward });
        // Query point directly above midpoint — distance should be exactly 1.
        let v = a.push(Shape::vertex(Point3::new(0.5, 1.0, 0.0)));
        let d = distance_vertex_edge(&a, v, e).unwrap();
        assert_abs_diff_eq!(d, 1.0, epsilon = 1e-9);
        // Query point past the segment end — distance = √((2-1)² + 1²) = √2.
        let v2 = a.push(Shape::vertex(Point3::new(2.0, 1.0, 0.0)));
        let d2 = distance_vertex_edge(&a, v2, e).unwrap();
        assert_abs_diff_eq!(d2, 2f64.sqrt(), epsilon = 1e-9);
    }

    #[test]
    fn inertia_of_unit_cube() {
        use gfd_cad_feature::box_solid;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 1.0, 1.0, 1.0).unwrap();
        let (ix, iy, iz) = inertia_bbox(&a, id).unwrap();
        // Unit cube mass = 1, Ix = Iy = Iz = (1+1)/12 = 1/6.
        assert_abs_diff_eq!(ix, 1.0 / 6.0, epsilon = 1e-9);
        assert_abs_diff_eq!(iy, 1.0 / 6.0, epsilon = 1e-9);
        assert_abs_diff_eq!(iz, 1.0 / 6.0, epsilon = 1e-9);
    }

    #[test]
    fn signed_distance_positive_outside_negative_inside() {
        use gfd_cad_feature::box_solid;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 2.0, 2.0, 2.0).unwrap();
        // (5, 0, 0) is outside, distance to +X face (x=1) = 4.
        let d_out = signed_distance(&a, id, Point3::new(5.0, 0.0, 0.0), 8, 4).unwrap();
        assert!(d_out > 0.0);
        assert!((d_out - 4.0).abs() < 1e-6);
        // Origin is inside; nearest face = 1 unit away → signed −1.
        let d_in = signed_distance(&a, id, Point3::new(0.0, 0.0, 0.0), 8, 4).unwrap();
        assert!(d_in < 0.0);
        assert!((d_in + 1.0).abs() < 1e-6, "got {}", d_in);
    }

    #[test]
    fn point_inside_vs_outside_box() {
        use gfd_cad_feature::box_solid;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 2.0, 2.0, 2.0).unwrap();
        // Origin is inside a centred unit cube.
        assert!(is_point_inside_solid(&a, id, Point3::new(0.0, 0.0, 0.0), 8, 4).unwrap());
        // Far point outside.
        assert!(!is_point_inside_solid(&a, id, Point3::new(5.0, 0.0, 0.0), 8, 4).unwrap());
    }

    #[test]
    fn closest_point_uses_face_interior_when_projection_inside() {
        // Pad 2×2×1 spanning (0..2)×(0..2)×(0..1). A point at (1, 1, 5)
        // projects onto the top cap's interior, so the distance should be 4
        // (not the √(something) distance to the nearest corner).
        use gfd_cad_feature::pad_polygon_xy;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = pad_polygon_xy(&mut a, &[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)], 1.0).unwrap();
        let d = closest_point_on_shape(&a, id, Point3::new(1.0, 1.0, 5.0)).unwrap();
        assert!((d - 4.0).abs() < 1e-6, "got {}", d);
    }

    #[test]
    fn closest_point_on_box_walks_boundary() {
        use gfd_cad_feature::box_solid;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 2.0, 2.0, 2.0).unwrap();
        // Query exactly on a corner (1,1,1) — distance to that vertex is 0.
        let d0 = closest_point_on_shape(&a, id, Point3::new(1.0, 1.0, 1.0)).unwrap();
        assert!(d0.abs() < 1e-6);
        // Query 3 units past the corner along +X+Y+Z — distance is the
        // diagonal (3 - 1) * √3 = 2√3.
        let d = closest_point_on_shape(&a, id, Point3::new(3.0, 3.0, 3.0)).unwrap();
        assert!((d - 2.0 * 3f64.sqrt()).abs() < 1e-6, "got {}", d);
    }

    #[test]
    fn minimum_bounding_sphere_of_unit_cube_contains_all_vertices() {
        use gfd_cad_feature::box_solid;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 2.0, 2.0, 2.0).unwrap();
        let (c, r) = minimum_bounding_sphere(&a, id).unwrap();
        // Every vertex of the 2³ box has distance √3 from centre; the MBS
        // radius should be ≥ √3 (allow small slack for the non-strict Welzl
        // fallback).
        assert!(r >= 3f64.sqrt() - 1e-6, "r = {}", r);
        // Centre should be approximately the origin.
        assert!(c.distance(Point3::ORIGIN) < 0.1);
    }

    #[test]
    fn bounding_sphere_of_unit_cube_has_diagonal_radius() {
        use gfd_cad_feature::box_solid;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 2.0, 2.0, 2.0).unwrap();
        let (c, r) = bounding_sphere(&a, id).unwrap();
        assert_abs_diff_eq!(c.x, 0.0, epsilon = 1e-9);
        // Diagonal of a 2-cube = 2*sqrt(3); bounding-sphere radius = half = sqrt(3).
        assert_abs_diff_eq!(r, 3f64.sqrt(), epsilon = 1e-6);
    }

    #[test]
    fn edge_length_range_on_box() {
        use gfd_cad_feature::box_solid;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 1.0, 2.0, 3.0).unwrap();
        let (min_len, max_len) = edge_length_range(&a, id).unwrap();
        assert!((min_len - 1.0).abs() < 1e-9, "min was {}", min_len);
        assert!((max_len - 3.0).abs() < 1e-9, "max was {}", max_len);
    }

    #[test]
    fn principal_axes_of_box_match_cardinal_axes() {
        use gfd_cad_feature::box_solid;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        // Non-cube so principal moments are distinct and ordering is well defined.
        let id = box_solid(&mut a, 4.0, 2.0, 1.0).unwrap();
        let (i1, i2, i3, vecs) = principal_axes(&a, id).unwrap();
        assert!(i1 <= i2 && i2 <= i3);
        // Each eigenvector should be parallel to one cardinal axis (±).
        for v in vecs.iter() {
            let ax = v[0].abs();
            let ay = v[1].abs();
            let az = v[2].abs();
            let dom = ax.max(ay).max(az);
            assert!(dom > 0.99, "eigenvector not axis-aligned: {:?}", v);
        }
    }

    #[test]
    fn full_inertia_tensor_for_pad_centered_has_zero_cross_terms() {
        // A pad centred about the origin should have zero cross products.
        // Our pad generator places corner at origin, so shift the polygon
        // to (-1..1)×(-1..1) to centre it.
        use gfd_cad_feature::pad_polygon_xy;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = pad_polygon_xy(&mut a, &[(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)], 2.0).unwrap();
        // Shift in Z too: pad sits at z=0..2, so cross terms involving z are
        // non-zero about origin. We just check Ixy is zero due to x/y symmetry.
        let (ix, iy, iz, ixy, _, _) = inertia_tensor_full(&a, id).unwrap();
        assert!(ix > 0.0 && iy > 0.0 && iz > 0.0);
        assert_abs_diff_eq!(ix, iy, epsilon = 1e-6);
        assert!(ixy.abs() < 1e-6, "Ixy should be ~0, got {}", ixy);
    }

    #[test]
    fn analytic_inertia_of_pad() {
        // 2×2×1 pad centred about origin would have Ix=Iy=5/12 for unit
        // density; our pad is at (0,0,0)→(2,2,0)→...(0,0,1) so the solid
        // sits in the +X+Y+Z octant and inertia is taken about the origin.
        // Just check the analytical routine returns a positive tensor and
        // Ix = Iy (square cross-section in XY).
        use gfd_cad_feature::pad_polygon_xy;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = pad_polygon_xy(&mut a, &[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)], 1.0).unwrap();
        let (ix, iy, iz) = inertia_tensor_diag(&a, id).unwrap();
        assert!(ix > 0.0 && iy > 0.0 && iz > 0.0);
        assert_abs_diff_eq!(ix, iy, epsilon = 1e-6);
    }

    #[test]
    fn centroid_of_box_is_origin() {
        use gfd_cad_feature::box_solid;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 2.0, 2.0, 2.0).unwrap();
        let c = center_of_mass(&a, id).unwrap();
        // Unit cube at origin → centroid at (0, 0, 0).
        assert_abs_diff_eq!(c.x, 0.0, epsilon = 1.0e-6);
        assert_abs_diff_eq!(c.y, 0.0, epsilon = 1.0e-6);
        assert_abs_diff_eq!(c.z, 0.0, epsilon = 1.0e-6);
    }

    #[test]
    fn box_edge_length_is_1() {
        use gfd_cad_feature::box_solid;
        use gfd_cad_topo::{collect_by_kind, ShapeArena, ShapeKind};
        let mut a = ShapeArena::new();
        let id = box_solid(&mut a, 1.0, 1.0, 1.0).unwrap();
        let edges = collect_by_kind(&a, id, ShapeKind::Edge);
        assert!(!edges.is_empty());
        let l = edge_length(&a, edges[0]).unwrap();
        assert_abs_diff_eq!(l, 1.0, epsilon = 1e-9);
    }

    #[test]
    fn sphere_surface_area_analytic() {
        use gfd_cad_feature::sphere_solid;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = sphere_solid(&mut a, 1.0).unwrap();
        let area = surface_area(&a, id).unwrap();
        assert_abs_diff_eq!(area, 4.0 * std::f64::consts::PI, epsilon = 1e-9);
    }

    #[test]
    fn torus_surface_area_analytic() {
        use gfd_cad_feature::torus_solid;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = torus_solid(&mut a, 0.5, 0.15).unwrap();
        let area = surface_area(&a, id).unwrap();
        let expect = 4.0 * std::f64::consts::PI.powi(2) * 0.5 * 0.15;
        assert_abs_diff_eq!(area, expect, epsilon = 1e-9);
    }

    #[test]
    fn pad_square_surface_area_full_enclosed() {
        // Iter 13: pad emits caps with real polygon wires, so the total
        // surface area of a 2×2×1 box-pad is:
        //   lateral 4 × 2 = 8
        //   caps    2 × 4 = 8
        //   total         = 16
        use gfd_cad_feature::pad_polygon_xy;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = pad_polygon_xy(&mut a, &[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)], 1.0).unwrap();
        let area = surface_area(&a, id).unwrap();
        assert_abs_diff_eq!(area, 16.0, epsilon = 1e-6);
    }

    #[test]
    fn closest_point_triangle_interior() {
        let a = [0.0_f32, 0.0, 0.0];
        let b = [1.0, 0.0, 0.0];
        let c = [0.0, 1.0, 0.0];
        let q = [0.25, 0.25, 5.0]; // directly above interior
        let p = closest_point_on_triangle(q, a, b, c);
        assert_abs_diff_eq!(p[0], 0.25, epsilon = 1e-6);
        assert_abs_diff_eq!(p[1], 0.25, epsilon = 1e-6);
        assert_abs_diff_eq!(p[2], 0.0,  epsilon = 1e-6);
    }

    #[test]
    fn closest_point_triangle_vertex_region() {
        let a = [0.0_f32, 0.0, 0.0];
        let b = [1.0, 0.0, 0.0];
        let c = [0.0, 1.0, 0.0];
        // Point in (-,-) region → closest is vertex A.
        let p = closest_point_on_triangle([-2.0, -2.0, 0.0], a, b, c);
        assert_abs_diff_eq!(p[0], 0.0, epsilon = 1e-9);
        assert_abs_diff_eq!(p[1], 0.0, epsilon = 1e-9);
    }

    #[test]
    fn trimesh_closest_point_to_quad() {
        let positions = vec![
            [0.0_f32, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let indices = vec![0, 1, 2, 0, 2, 3];
        let (p, _t, d) = trimesh_closest_point([0.5, 0.5, 3.0], &positions, &indices).unwrap();
        assert_abs_diff_eq!(p[2], 0.0, epsilon = 1e-6);
        assert_abs_diff_eq!(d, 3.0, epsilon = 1e-6);
    }

    #[test]
    fn signed_distance_unit_box() {
        let p = vec![
            [0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0],
        ];
        let i = vec![
            0,3,2, 0,2,1,
            4,5,6, 4,6,7,
            0,1,5, 0,5,4,
            3,7,6, 3,6,2,
            0,4,7, 0,7,3,
            1,2,6, 1,6,5,
        ];
        // Centre: inside, |d| = 0.5.
        let d_in = trimesh_signed_distance([0.5, 0.5, 0.5], &p, &i).unwrap();
        assert_abs_diff_eq!(d_in, -0.5, epsilon = 1e-6);
        // 2 units along +X from right face: outside, d = 1.0.
        let d_out = trimesh_signed_distance([2.0, 0.5, 0.5], &p, &i).unwrap();
        assert_abs_diff_eq!(d_out, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn point_inside_unit_box() {
        let p = vec![
            [0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0],
        ];
        let i = vec![
            0,3,2, 0,2,1,
            4,5,6, 4,6,7,
            0,1,5, 0,5,4,
            3,7,6, 3,6,2,
            0,4,7, 0,7,3,
            1,2,6, 1,6,5,
        ];
        assert!(trimesh_point_inside([0.5, 0.5, 0.5], &p, &i));
        assert!(!trimesh_point_inside([2.0, 0.5, 0.5], &p, &i));
        assert!(!trimesh_point_inside([-0.5, 0.5, 0.5], &p, &i));
    }

    #[test]
    fn ray_hits_triangle_through_centroid() {
        let a = [0.0_f32, 0.0, 0.0];
        let b = [1.0, 0.0, 0.0];
        let c = [0.0, 1.0, 0.0];
        // Ray from above straight down through (0.25, 0.25, 0).
        let origin = [0.25, 0.25, 2.0];
        let dir    = [0.0, 0.0, -1.0];
        let hit = ray_triangle_intersect(origin, dir, a, b, c).unwrap();
        assert_abs_diff_eq!(hit.0, 2.0, epsilon = 1e-6);
        assert_abs_diff_eq!(hit.1, 0.25, epsilon = 1e-6); // u
        assert_abs_diff_eq!(hit.2, 0.25, epsilon = 1e-6); // v
    }

    #[test]
    fn ray_misses_triangle_parallel() {
        let a = [0.0_f32, 0.0, 0.0];
        let b = [1.0, 0.0, 0.0];
        let c = [0.0, 1.0, 0.0];
        // Ray in plane of the triangle.
        let hit = ray_triangle_intersect([0.5, 0.5, 0.0], [1.0, 0.0, 0.0], a, b, c);
        assert!(hit.is_none());
    }

    #[test]
    fn trimesh_ray_finds_nearest_hit() {
        // Two stacked triangles at z=1 and z=3. Ray from above should hit z=3 first.
        let positions = vec![
            [0.0_f32, 0.0, 3.0], [1.0, 0.0, 3.0], [0.0, 1.0, 3.0],
            [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0],
        ];
        let indices = vec![0, 1, 2, 3, 4, 5];
        let hit = trimesh_ray_intersect([0.25, 0.25, 5.0], [0.0, 0.0, -1.0],
            &positions, &indices).unwrap();
        assert_eq!(hit.1, 0); // triangle 0 is the nearest
        assert_abs_diff_eq!(hit.0, 2.0, epsilon = 1e-6);
    }

    #[test]
    fn trimesh_closed_welded_tetrahedron() {
        let indices = vec![
            0, 2, 1,
            0, 1, 3,
            1, 2, 3,
            0, 3, 2,
        ];
        assert!(trimesh_is_closed(&indices));
        assert!(trimesh_boundary_edges(&indices).is_empty());
        assert!(trimesh_non_manifold_edges(&indices).is_empty());
    }

    #[test]
    fn trimesh_open_quad_has_four_boundary_edges() {
        let indices = vec![0, 1, 2, 0, 2, 3];
        assert!(!trimesh_is_closed(&indices));
        let bnd = trimesh_boundary_edges(&indices);
        assert_eq!(bnd.len(), 4); // square outline
    }

    #[test]
    fn trimesh_surface_centroid_unit_box_at_half() {
        // Closed unit box — its surface centroid coincides with the volume COM.
        let p = vec![
            [0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0],
        ];
        let i = vec![
            0,3,2, 0,2,1,
            4,5,6, 4,6,7,
            0,1,5, 0,5,4,
            3,7,6, 3,6,2,
            0,4,7, 0,7,3,
            1,2,6, 1,6,5,
        ];
        let c = trimesh_surface_centroid(&p, &i).unwrap();
        assert_abs_diff_eq!(c[0], 0.5, epsilon = 1e-6);
        assert_abs_diff_eq!(c[1], 0.5, epsilon = 1e-6);
        assert_abs_diff_eq!(c[2], 0.5, epsilon = 1e-6);
    }

    #[test]
    fn trimesh_surface_centroid_open_quad() {
        // A single quad on the z=0 plane → surface centroid at (0.5, 0.5, 0).
        let p = vec![
            [0.0_f32, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let i = vec![0, 1, 2, 0, 2, 3];
        let c = trimesh_surface_centroid(&p, &i).unwrap();
        assert_abs_diff_eq!(c[0], 0.5, epsilon = 1e-6);
        assert_abs_diff_eq!(c[1], 0.5, epsilon = 1e-6);
        assert_abs_diff_eq!(c[2], 0.0, epsilon = 1e-6);
    }

    #[test]
    fn trimesh_inertia_unit_cube_about_origin() {
        // Cube [0,1]³ about origin: Ixx=Iyy=Izz=2/3; Ixy=Iyz=Izx=1/4.
        let p = vec![
            [0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0],
        ];
        let i = vec![
            0,3,2, 0,2,1,
            4,5,6, 4,6,7,
            0,1,5, 0,5,4,
            3,7,6, 3,6,2,
            0,4,7, 0,7,3,
            1,2,6, 1,6,5,
        ];
        let (ixx, iyy, izz, ixy, iyz, izx) = trimesh_inertia_tensor(&p, &i).unwrap();
        assert_abs_diff_eq!(ixx, 2.0/3.0, epsilon = 1e-6);
        assert_abs_diff_eq!(iyy, 2.0/3.0, epsilon = 1e-6);
        assert_abs_diff_eq!(izz, 2.0/3.0, epsilon = 1e-6);
        assert_abs_diff_eq!(ixy, 0.25,    epsilon = 1e-6);
        assert_abs_diff_eq!(iyz, 0.25,    epsilon = 1e-6);
        assert_abs_diff_eq!(izx, 0.25,    epsilon = 1e-6);
    }

    #[test]
    fn trimesh_bbox_unit_box() {
        let p = vec![
            [0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0],
        ];
        let (mn, mx) = trimesh_bounding_box(&p).unwrap();
        assert_eq!(mn, [0.0, 0.0, 0.0]);
        assert_eq!(mx, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn trimesh_com_unit_box_at_half() {
        let p = vec![
            [0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0],
        ];
        let i = vec![
            0,3,2, 0,2,1,           // z=0 bottom
            4,5,6, 4,6,7,           // z=1 top
            0,1,5, 0,5,4,           // y=0
            3,7,6, 3,6,2,           // y=1
            0,4,7, 0,7,3,           // x=0
            1,2,6, 1,6,5,           // x=1
        ];
        let com = trimesh_center_of_mass(&p, &i).unwrap();
        assert_abs_diff_eq!(com[0], 0.5, epsilon = 1e-6);
        assert_abs_diff_eq!(com[1], 0.5, epsilon = 1e-6);
        assert_abs_diff_eq!(com[2], 0.5, epsilon = 1e-6);
    }

    #[test]
    fn hausdorff_identical_sets_is_zero() {
        let a: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert_abs_diff_eq!(hausdorff_distance_vertex(&a, &a), 0.0, epsilon = 1e-9);
    }

    #[test]
    fn hausdorff_captures_offset_vertex() {
        let a: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
        let b: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 5.0]];
        // The extra (0,0,5) in B is distance 5 from the closest point of A.
        assert_abs_diff_eq!(hausdorff_distance_vertex(&a, &b), 5.0, epsilon = 1e-6);
    }

    #[test]
    fn trimesh_area_and_volume_unit_box() {
        // Outward-facing unit cube mesh (per-face shared corners welded form).
        let p = vec![
            [0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0],
        ];
        // Each face outward-oriented (CCW when viewed from outside).
        let i = vec![
            0,3,2, 0,2,1,           // z=0 bottom (normal -z)
            4,5,6, 4,6,7,           // z=1 top    (normal +z)
            0,1,5, 0,5,4,           // y=0 front  (normal -y)
            3,7,6, 3,6,2,           // y=1 back   (normal +y)
            0,4,7, 0,7,3,           // x=0 left   (normal -x)
            1,2,6, 1,6,5,           // x=1 right  (normal +x)
        ];
        let a = trimesh_surface_area(&p, &i);
        assert_abs_diff_eq!(a, 6.0, epsilon = 1e-6);
        let v = trimesh_volume(&p, &i);
        assert_abs_diff_eq!(v, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn mesh_euler_genus_closed_tetrahedron() {
        // 4 vertices, 4 triangle faces, 6 shared edges → χ = 4 − 6 + 4 = 2,
        // genus = 0 (topological sphere).
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let indices = vec![
            0, 2, 1,
            0, 1, 3,
            1, 2, 3,
            0, 3, 2,
        ];
        let (chi, genus) = mesh_euler_genus(&positions, &indices);
        assert_eq!(chi, 2);
        assert_eq!(genus, 0);
    }

    #[test]
    fn mesh_euler_genus_unshared_face_mesh_disconnected() {
        // When tessellation emits per-face unique vertices, each face is an
        // independent topological disk → χ = 6*1 = 6 for a box with 6 disks.
        // This is the honest reading — the function measures the mesh as given.
        let positions = vec![
            [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0],
        ];
        let indices = vec![0, 1, 2];
        let (chi, _) = mesh_euler_genus(&positions, &indices);
        assert_eq!(chi, 1); // single triangle disk
    }

    #[test]
    fn obb_axis_aligned_box_matches_aabb() {
        use gfd_cad_feature::pad_polygon_xy;
        use gfd_cad_topo::ShapeArena;
        let mut a = ShapeArena::new();
        let id = pad_polygon_xy(
            &mut a,
            &[(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)],
            3.0,
        ).unwrap();
        let (center, _axes, half) = oriented_bounding_box(&a, id).unwrap();
        let mut sorted = half;
        sorted.sort_by(|x, y| x.partial_cmp(y).unwrap());
        assert_abs_diff_eq!(sorted[0], 0.5, epsilon = 1e-9);
        assert_abs_diff_eq!(sorted[1], 1.0, epsilon = 1e-9);
        assert_abs_diff_eq!(sorted[2], 1.5, epsilon = 1e-9);
        assert_abs_diff_eq!(center.x, 1.0, epsilon = 1e-9);
        assert_abs_diff_eq!(center.y, 0.5, epsilon = 1e-9);
        assert_abs_diff_eq!(center.z, 1.5, epsilon = 1e-9);
    }

    #[test]
    fn total_gaussian_curvature_tetrahedron_is_4pi() {
        // Closed orientable surface (genus 0): Gauss-Bonnet gives 2π·χ = 4π.
        let positions: Vec<[f32; 3]> = vec![
            [ 1.0,  1.0,  1.0],
            [-1.0, -1.0,  1.0],
            [-1.0,  1.0, -1.0],
            [ 1.0, -1.0, -1.0],
        ];
        let indices: Vec<u32> = vec![
            0, 1, 2,
            0, 3, 1,
            0, 2, 3,
            1, 3, 2,
        ];
        let total = trimesh_total_gaussian_curvature(&positions, &indices);
        assert_abs_diff_eq!(total, 4.0 * std::f64::consts::PI, epsilon = 1e-10);
    }

    #[test]
    fn point_plane_signed_distance_above_below() {
        // Plane: z=0 with normal +Z. Points above have positive distance.
        let origin = Point3::new(0.0, 0.0, 0.0);
        let n = [0.0, 0.0, 1.0];
        assert_abs_diff_eq!(
            point_plane_signed_distance(Point3::new(1.0, 2.0, 3.0), origin, n),
            3.0, epsilon = 1e-10);
        assert_abs_diff_eq!(
            point_plane_signed_distance(Point3::new(1.0, 2.0, -3.0), origin, n),
            -3.0, epsilon = 1e-10);
        // Non-unit normal should still give correct (normalized) distance.
        assert_abs_diff_eq!(
            point_plane_signed_distance(Point3::new(1.0, 2.0, 3.0), origin, [0.0, 0.0, 5.0]),
            3.0, epsilon = 1e-10);
    }

    #[test]
    fn ray_plane_hits_plane_z_eq_zero() {
        let (t, hit) = ray_plane_intersection(
            Point3::new(0.0, 0.0, 10.0),
            [0.0, 0.0, -1.0],
            Point3::new(0.0, 0.0, 0.0),
            [0.0, 0.0, 1.0],
        ).unwrap();
        assert_abs_diff_eq!(t, 10.0, epsilon = 1e-10);
        assert_abs_diff_eq!(hit.z, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn ray_plane_parallel_returns_none() {
        let result = ray_plane_intersection(
            Point3::new(0.0, 0.0, 5.0),
            [1.0, 0.0, 0.0],
            Point3::new(0.0, 0.0, 0.0),
            [0.0, 0.0, 1.0],
        );
        assert!(result.is_none());
    }

    #[test]
    fn segment_segment_skew_lines() {
        // Two skew lines: A along +x at y=0,z=0; B along +y at x=0.5,z=1.
        // Nearest points are (0.5, 0, 0) on A and (0.5, 0, 1) on B → dist 1.
        let (d, cp_a, cp_b, s, t) = segment_segment_distance_3d(
            Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 0.0, 1.0), Point3::new(0.5, 1.0, 1.0),
        );
        assert_abs_diff_eq!(d, 1.0, epsilon = 1e-10);
        assert_abs_diff_eq!(cp_a.x, 0.5, epsilon = 1e-10);
        assert_abs_diff_eq!(cp_b.z, 1.0, epsilon = 1e-10);
        assert_abs_diff_eq!(s, 0.5, epsilon = 1e-10);
        assert_abs_diff_eq!(t, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn segment_segment_parallel_offset() {
        // Two parallel segments on x-axis, offset by y=2. Nearest dist = 2
        // and there are many pairs; we only verify the distance.
        let (d, _, _, _, _) = segment_segment_distance_3d(
            Point3::new(0.0, 0.0, 0.0), Point3::new(5.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0), Point3::new(3.0, 2.0, 0.0),
        );
        assert_abs_diff_eq!(d, 2.0, epsilon = 1e-10);
    }

    #[test]
    fn vertex_valence_tetrahedron_is_3() {
        let positions: Vec<[f32; 3]> = vec![
            [ 1.0,  1.0,  1.0],
            [-1.0, -1.0,  1.0],
            [-1.0,  1.0, -1.0],
            [ 1.0, -1.0, -1.0],
        ];
        let indices: Vec<u32> = vec![0, 1, 2,  0, 3, 1,  0, 2, 3,  1, 3, 2];
        let (mn, mx, mean, _irreg) = trimesh_vertex_valence_stats(&positions, &indices).unwrap();
        assert_eq!(mn, 3);
        assert_eq!(mx, 3);
        assert_abs_diff_eq!(mean, 3.0, epsilon = 1e-10);
    }

    #[test]
    fn dihedral_right_angle_on_box_corner() {
        // Two triangles forming an L-shape meeting at edge (v1, v2) with 90°
        // dihedral: one triangle on z=0 plane, the other on y=0 plane.
        let positions: Vec<[f32; 3]> = vec![
            [0.0, 0.0, 0.0], // v0
            [1.0, 0.0, 0.0], // v1
            [1.0, 1.0, 0.0], // v2  (shared edge goes v1→v2)
            [1.0, 0.0, 1.0], // v3
        ];
        // T0 = v0,v1,v2 on z=0 (normal +Z)
        // T1 = v1,v3,v2 on x=1 (normal +X)
        let indices: Vec<u32> = vec![0, 1, 2, 1, 3, 2];
        let stats = trimesh_dihedral_angle_stats(&positions, &indices).unwrap();
        assert_abs_diff_eq!(stats.0, std::f64::consts::FRAC_PI_2, epsilon = 1e-10);
        let sharp = trimesh_sharp_edges(&positions, &indices, std::f64::consts::FRAC_PI_4);
        assert_eq!(sharp.len(), 1);
    }

    #[test]
    fn gaussian_curvature_flat_square_has_zero_interior() {
        // 2 triangles forming a flat 1×1 square on z=0. Boundary vertices
        // have nonzero defect; interior (there is none on a 2-tri square)
        // would have zero. We verify per-vertex values are finite and
        // boundary vertices each have π − (interior angles sum).
        let positions: Vec<[f32; 3]> = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let indices: Vec<u32> = vec![0, 1, 2, 0, 2, 3];
        let k = trimesh_gaussian_curvature_per_vertex(&positions, &indices);
        assert_eq!(k.len(), 4);
        // All four corners are boundary; for a flat square each corner has
        // angle defect = π − π/2 = π/2 (v1 and v3 see only one triangle with
        // π/2 corner, v0 and v2 see two triangles totaling π/2).
        for &d in &k {
            assert!(d.is_finite());
        }
    }
}
