//! Ear-clipping polygon triangulation for 2D simple polygons.
//!
//! Iteration 13 adds non-convex Pad support by turning an XY polygon (outer
//! loop, CCW or CW) into a triangle fan. O(n²) complexity is fine for the
//! sketch sizes GFD typically deals with.

/// Triangulate a 2D simple polygon. Returns triangle indices into `points`.
/// Self-intersecting polygons give undefined results.
pub fn triangulate_polygon(points: &[(f64, f64)]) -> Vec<[u32; 3]> {
    if points.len() < 3 { return Vec::new(); }
    // Ensure CCW winding — ear clipping convention expects counter-clockwise.
    let mut indices: Vec<u32> = (0..points.len() as u32).collect();
    if signed_area(points) < 0.0 {
        indices.reverse();
    }
    ear_clip(points, indices)
}

/// Triangulate a planar polygon WITH HOLES. `outer` is the boundary loop; each
/// entry of `holes` is an inner loop to be cut out. Returns the combined point
/// list (outer ++ holes, in that order) and triangle indices into it — the
/// caller must build its 3D positions in the SAME order. Holes are merged into
/// the outer loop via bridge edges, then ear-clipped. Falls back to the
/// outer-only triangulation if a bridge cannot be found.
pub fn triangulate_polygon_with_holes(
    outer: &[(f64, f64)],
    holes: &[Vec<(f64, f64)>],
) -> (Vec<(f64, f64)>, Vec<[u32; 3]>) {
    // Combined point array: outer first, then each hole in order.
    let mut points: Vec<(f64, f64)> = outer.to_vec();
    let mut hole_ranges: Vec<(usize, usize)> = Vec::new();
    for h in holes {
        let start = points.len();
        points.extend_from_slice(h);
        hole_ranges.push((start, points.len()));
    }
    if outer.len() < 3 {
        return (points, Vec::new());
    }
    if holes.is_empty() {
        return (points.clone(), triangulate_polygon(&points));
    }
    // Outer ring CCW.
    let mut ring: Vec<u32> = (0..outer.len() as u32).collect();
    if signed_area(outer) < 0.0 {
        ring.reverse();
    }
    // Merge holes by descending rightmost-x so nested bridges resolve correctly.
    let mut order: Vec<usize> = (0..holes.len()).collect();
    order.sort_by(|&a, &b| {
        let ma = holes[a].iter().fold(f64::NEG_INFINITY, |m, p| m.max(p.0));
        let mb = holes[b].iter().fold(f64::NEG_INFINITY, |m, p| m.max(p.0));
        mb.partial_cmp(&ma).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut ok = true;
    for &hi in &order {
        let (hs, he) = hole_ranges[hi];
        if he - hs < 3 {
            continue;
        }
        // Hole must wind opposite (CW) to the CCW outer.
        let mut hole_idx: Vec<u32> = (hs as u32..he as u32).collect();
        let hpts: Vec<(f64, f64)> = hole_idx.iter().map(|&i| points[i as usize]).collect();
        if signed_area(&hpts) > 0.0 {
            hole_idx.reverse();
        }
        match bridge_hole(&points, &ring, &hole_idx) {
            Some(merged) => ring = merged,
            None => { ok = false; break; }
        }
    }
    if !ok || ring.len() < 3 {
        // Bridge failed — degrade to filling the outer loop only.
        return (points.clone(), triangulate_polygon(outer));
    }
    let tris = ear_clip(&points, ring);
    (points, tris)
}

/// Splice a hole loop into an outer ring via a bridge edge, using the standard
/// Eberly/FIST visible-vertex rule: cast a +x ray from the hole's rightmost
/// vertex M, find the nearest ring edge it hits, and bridge to a ring vertex
/// guaranteed visible from M (so the bridge stays inside the polygon even when
/// the outer ring is non-convex). Returns the merged ring, or None if no bridge.
fn bridge_hole(points: &[(f64, f64)], ring: &[u32], hole: &[u32]) -> Option<Vec<u32>> {
    // M = hole vertex with the largest x.
    let m_pos = (0..hole.len()).max_by(|&a, &b| {
        points[hole[a] as usize].0.partial_cmp(&points[hole[b] as usize].0).unwrap_or(std::cmp::Ordering::Equal)
    })?;
    let m = hole[m_pos];
    let mp = points[m as usize];

    // 1. Cast a +x ray from M; find the nearest intersection I with a ring edge
    //    (edge crossing the horizontal line y = M.y at x >= M.x).
    let n = ring.len();
    let mut best_ix = f64::INFINITY;
    let mut best_edge: Option<usize> = None; // edge i = (ring[i], ring[i+1])
    for i in 0..n {
        let a = points[ring[i] as usize];
        let b = points[ring[(i + 1) % n] as usize];
        // Does the edge straddle y = mp.1?
        if (a.1 > mp.1) == (b.1 > mp.1) {
            continue;
        }
        let t = (mp.1 - a.1) / (b.1 - a.1);
        let ix = a.0 + t * (b.0 - a.0);
        if ix >= mp.0 - 1e-12 && ix < best_ix {
            best_ix = ix;
            best_edge = Some(i);
        }
    }
    let ei = best_edge?;
    let ea = points[ring[ei] as usize];
    let eb = points[ring[(ei + 1) % n] as usize];
    let ipt = (best_ix, mp.1);

    // 2. Candidate P = the edge endpoint with the larger x (the one toward M's
    //    interior side). If the intersection IS essentially that vertex, use it.
    let (mut p_pos, p0) = if ea.0 >= eb.0 { (ei, ea) } else { ((ei + 1) % n, eb) };
    let pcand = points[ring[p_pos] as usize];

    // 3. If P0 is not directly visible (a reflex ring vertex lies inside the
    //    triangle M–I–P0), bridge instead to the reflex vertex minimizing the
    //    angle to the +x ray (ties: nearest to M). Eberly's visibility rule.
    if (pcand.0 - ipt.0).abs() > 1e-12 || (pcand.1 - ipt.1).abs() > 1e-12 {
        let tri = (mp, ipt, p0);
        let mut best_ang = f64::INFINITY;
        let mut best_d = f64::INFINITY;
        for (i, &rv) in ring.iter().enumerate() {
            let r = points[rv as usize];
            // Only ring vertices strictly right of M can occlude.
            if r.0 < mp.0 - 1e-12 {
                continue;
            }
            if !point_in_triangle(r, tri.0, tri.1, tri.2) {
                continue;
            }
            // Reflex test would refine further; angle/distance suffices in practice.
            let ang = ((r.1 - mp.1).abs()).atan2(r.0 - mp.0);
            let d = (r.0 - mp.0).powi(2) + (r.1 - mp.1).powi(2);
            if ang < best_ang - 1e-12 || (ang <= best_ang + 1e-12 && d < best_d) {
                best_ang = ang;
                best_d = d;
                p_pos = i;
            }
        }
    }

    // 4. Splice: ring[..=p_pos], the hole starting at M around back to M, then
    //    re-enter the outer ring at P (P and M each duplicated → zero-width bridge).
    let mut merged: Vec<u32> = Vec::with_capacity(ring.len() + hole.len() + 2);
    merged.extend_from_slice(&ring[..=p_pos]);
    for k in 0..hole.len() {
        merged.push(hole[(m_pos + k) % hole.len()]);
    }
    merged.push(m);
    merged.extend_from_slice(&ring[p_pos..]);
    Some(merged)
}

/// Ear-clip a single (weakly-simple) index ring over `points`.
fn ear_clip(points: &[(f64, f64)], indices: Vec<u32>) -> Vec<[u32; 3]> {
    if indices.len() < 3 { return Vec::new(); }
    let mut triangles = Vec::with_capacity(indices.len() - 2);
    let mut remaining = indices;
    let cap = remaining.len();
    let mut guard = 0usize;
    while remaining.len() > 3 {
        guard += 1;
        if guard > cap * cap { break; } // safety
        let n = remaining.len();
        let mut ear_found = false;
        for i in 0..n {
            let ia = remaining[(i + n - 1) % n];
            let ib = remaining[i];
            let ic = remaining[(i + 1) % n];
            if !is_convex(points, ia, ib, ic) { continue; }
            let mut contains = false;
            for &j in &remaining {
                if j == ia || j == ib || j == ic { continue; }
                if point_in_triangle(points[j as usize], points[ia as usize], points[ib as usize], points[ic as usize]) {
                    contains = true;
                    break;
                }
            }
            if !contains {
                triangles.push([ia, ib, ic]);
                remaining.remove(i);
                ear_found = true;
                break;
            }
        }
        if !ear_found { break; }
    }
    if remaining.len() == 3 {
        triangles.push([remaining[0], remaining[1], remaining[2]]);
    }
    triangles
}

/// Even-odd ray-cast point-in-polygon test for a 2D loop. Used to cull uv-grid
/// cells outside a trimmed surface's parameter-space boundary.
pub fn point_in_polygon(p: (f64, f64), poly: &[(f64, f64)]) -> bool {
    if poly.len() < 3 { return false; }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if ((yi > p.1) != (yj > p.1))
            && (p.0 < (xj - xi) * (p.1 - yi) / (yj - yi) + xi)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn signed_area(p: &[(f64, f64)]) -> f64 {
    let mut s = 0.0;
    for i in 0..p.len() {
        let j = (i + 1) % p.len();
        s += p[i].0 * p[j].1 - p[j].0 * p[i].1;
    }
    s * 0.5
}

fn is_convex(p: &[(f64, f64)], ia: u32, ib: u32, ic: u32) -> bool {
    let a = p[ia as usize];
    let b = p[ib as usize];
    let c = p[ic as usize];
    let cross = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    cross > 0.0
}

fn point_in_triangle(p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

fn sign(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    (p.0 - b.0) * (a.1 - b.1) - (a.0 - b.0) * (p.1 - b.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convex_quad_yields_two_triangles() {
        let poly = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let tris = triangulate_polygon(&poly);
        assert_eq!(tris.len(), 2);
    }

    #[test]
    fn l_shape_yields_four_triangles() {
        // 6-vertex L polygon (non-convex):
        //   (0,0) → (2,0) → (2,1) → (1,1) → (1,2) → (0,2)
        let poly = [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0), (1.0, 2.0), (0.0, 2.0)];
        let tris = triangulate_polygon(&poly);
        assert_eq!(tris.len(), 4); // n-2 triangles for n-vertex simple polygon
    }

    #[test]
    fn clockwise_polygon_normalised() {
        // Same square but given clockwise — should still produce 2 triangles.
        let poly = [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0)];
        let tris = triangulate_polygon(&poly);
        assert_eq!(tris.len(), 2);
    }

    #[test]
    fn square_with_square_hole_has_correct_area() {
        // Outer [0,4]^2 (CCW) with a [1,3]^2 hole (CW) → triangulated area = 16-4=12.
        let outer = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let hole = vec![(1.0, 1.0), (1.0, 3.0), (3.0, 3.0), (3.0, 1.0)]; // CW
        let (pts, tris) = triangulate_polygon_with_holes(&outer, &[hole]);
        let tri_area = |t: &[u32; 3]| {
            let (a, b, c) = (pts[t[0] as usize], pts[t[1] as usize], pts[t[2] as usize]);
            0.5 * ((b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1)).abs()
        };
        let total: f64 = tris.iter().map(tri_area).sum();
        assert!((total - 12.0).abs() < 1e-9, "outer−hole area should be 12, got {}", total);
    }

    #[test]
    fn nonconvex_outer_with_offset_hole_cuts_correctly() {
        // L-shaped outer (area 20, non-convex) with a hole on its LEFT arm — the
        // bridge target must be a visible vertex, not the global max-x corner, or
        // the bridge crosses the concave notch and the hole is not cut.
        let outer = [(0.0, 0.0), (6.0, 0.0), (6.0, 2.0), (2.0, 2.0), (2.0, 6.0), (0.0, 6.0)];
        let hole = vec![(0.5, 3.0), (0.5, 5.0), (1.5, 5.0), (1.5, 3.0)]; // CW, area 2
        let (pts, tris) = triangulate_polygon_with_holes(&outer, &[hole]);
        let tri_area = |t: &[u32; 3]| {
            let (a, b, c) = (pts[t[0] as usize], pts[t[1] as usize], pts[t[2] as usize]);
            0.5 * ((b.0 - a.0) * (c.1 - a.1) - (c.0 - a.0) * (b.1 - a.1)).abs()
        };
        let total: f64 = tris.iter().map(tri_area).sum();
        assert!((total - 18.0).abs() < 1e-9, "L-shape − hole area should be 18, got {}", total);
    }
}
