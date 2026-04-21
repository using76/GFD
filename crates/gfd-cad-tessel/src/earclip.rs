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
    let signed = signed_area(points);
    let mut indices: Vec<u32> = (0..points.len() as u32).collect();
    if signed < 0.0 {
        indices.reverse();
    }

    let mut triangles = Vec::with_capacity(points.len() - 2);
    let mut remaining = indices;
    let mut guard = 0usize;
    while remaining.len() > 3 {
        guard += 1;
        if guard > points.len() * points.len() { break; } // safety
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
}
