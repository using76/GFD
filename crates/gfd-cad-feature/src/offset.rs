//! 2D polygon offset (parallel curve at distance `d`).
//!
//! For each edge, compute the outward normal and shift by `d`; for each
//! vertex, intersect adjacent shifted edges to find the new corner point.
//! Convex-only input for iter 80 — reflex vertices need arc/bevel joins
//! that ship later.

/// Offset a closed 2D polygon outward by `distance` (positive) or inward
/// (negative). Returns the new polygon's vertex list. Same length as input.
pub fn offset_polygon_2d(points: &[(f64, f64)], distance: f64) -> Vec<(f64, f64)> {
    let n = points.len();
    if n < 3 { return points.to_vec(); }

    // Per-edge unit outward normal (points to the right of the edge direction
    // if the polygon winds CCW). For a CCW polygon, +distance = outward.
    let mut normals = Vec::with_capacity(n);
    for i in 0..n {
        let j = (i + 1) % n;
        let dx = points[j].0 - points[i].0;
        let dy = points[j].1 - points[i].1;
        let len = (dx * dx + dy * dy).sqrt().max(f64::EPSILON);
        // Right-hand perpendicular = (dy, -dx), which points outward for CW
        // winding. Swap for CCW via winding test.
        normals.push((dy / len, -dx / len));
    }

    // Detect winding by signed area; flip normals if CCW.
    let mut signed = 0.0_f64;
    for i in 0..n {
        let j = (i + 1) % n;
        signed += points[i].0 * points[j].1 - points[j].0 * points[i].1;
    }
    // `(dy, -dx)` is right-of-edge-direction — outward for CCW (positive
    // signed area). For CW input we flip so +distance always means outward.
    if signed < 0.0 {
        for n_vec in normals.iter_mut() { n_vec.0 = -n_vec.0; n_vec.1 = -n_vec.1; }
    }

    // For each vertex, intersect the two shifted edges around it.
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let prev = (i + n - 1) % n;
        let n1 = normals[prev];
        let n2 = normals[i];
        // p1 = points[i] + n1*d (shifted point on previous edge heading into vertex)
        // p2 = points[i] + n2*d (shifted point on next edge leaving vertex)
        // Direction of prev-edge: points[i] - points[prev]
        let d1_x = points[i].0 - points[prev].0;
        let d1_y = points[i].1 - points[prev].1;
        // Direction of next-edge: points[(i+1)%n] - points[i]
        let d2_x = points[(i + 1) % n].0 - points[i].0;
        let d2_y = points[(i + 1) % n].1 - points[i].1;
        // Solve for intersection of (p1 + t*d1) and (p2 - s*d2), i.e. a 2×2.
        let p1_x = points[i].0 + n1.0 * distance;
        let p1_y = points[i].1 + n1.1 * distance;
        let p2_x = points[i].0 + n2.0 * distance;
        let p2_y = points[i].1 + n2.1 * distance;
        // Line A: (x, y) = (p1_x, p1_y) + t * (d1_x, d1_y)
        // Line B: (x, y) = (p2_x, p2_y) - s * (d2_x, d2_y)   (parameterised back)
        // Setting equal: p1 + t*d1 = p2 - s*d2 → [d1 | d2] [t s]^T = (p2 - p1)
        let det = d1_x * d2_y - d1_y * d2_x;
        if det.abs() < 1e-12 {
            // Near-parallel adjacent edges: fall back to simple vertex shift.
            let avg_n_x = (n1.0 + n2.0) * 0.5;
            let avg_n_y = (n1.1 + n2.1) * 0.5;
            out.push((points[i].0 + avg_n_x * distance, points[i].1 + avg_n_y * distance));
            continue;
        }
        let rhs_x = p2_x - p1_x;
        let rhs_y = p2_y - p1_y;
        let t = (rhs_x * d2_y - rhs_y * d2_x) / det;
        out.push((p1_x + t * d1_x, p1_y + t * d1_y));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_unit_square_outward_by_one_gives_3x3_square() {
        let square = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let off = offset_polygon_2d(&square, 1.0);
        // Each side should extend outward by 1, corners clamp to ±1 from the
        // original corner. Resulting bbox: (-1..2) × (-1..2).
        let minx = off.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let maxx = off.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
        assert!((minx - -1.0).abs() < 1e-6, "minx {}", minx);
        assert!((maxx - 2.0).abs() < 1e-6, "maxx {}", maxx);
    }

    #[test]
    fn offset_inward_shrinks_square() {
        let square = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let off = offset_polygon_2d(&square, -1.0);
        let minx = off.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
        let maxx = off.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
        assert!((minx - 1.0).abs() < 1e-6);
        assert!((maxx - 3.0).abs() < 1e-6);
    }
}
