//! 2D sketch primitives (SDF in 2D).

/// 2D circle SDF.
pub fn sketch_circle(center: [f64; 2], radius: f64) -> impl Fn([f64; 2]) -> f64 {
    move |p| {
        let dx = p[0] - center[0];
        let dy = p[1] - center[1];
        (dx * dx + dy * dy).sqrt() - radius
    }
}

/// 2D rectangle SDF.
pub fn sketch_rect(min: [f64; 2], max: [f64; 2]) -> impl Fn([f64; 2]) -> f64 {
    move |p| {
        let cx = 0.5 * (min[0] + max[0]);
        let cy = 0.5 * (min[1] + max[1]);
        let hx = 0.5 * (max[0] - min[0]);
        let hy = 0.5 * (max[1] - min[1]);
        let dx = (p[0] - cx).abs() - hx;
        let dy = (p[1] - cy).abs() - hy;
        let outside = (dx.max(0.0) * dx.max(0.0) + dy.max(0.0) * dy.max(0.0)).sqrt();
        let inside = dx.max(dy).min(0.0);
        outside + inside
    }
}

/// Distance from a point to a 2D line segment.
pub fn sketch_line_segment(a: [f64; 2], b: [f64; 2]) -> impl Fn([f64; 2]) -> f64 {
    move |p| {
        let ab = [b[0] - a[0], b[1] - a[1]];
        let ap = [p[0] - a[0], p[1] - a[1]];
        let t = (ap[0] * ab[0] + ap[1] * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1] + 1e-30);
        let t = t.clamp(0.0, 1.0);
        let closest = [a[0] + t * ab[0], a[1] + t * ab[1]];
        let dx = p[0] - closest[0];
        let dy = p[1] - closest[1];
        (dx * dx + dy * dy).sqrt()
    }
}

/// 2D arc distance (partial circle).
pub fn sketch_arc(center: [f64; 2], radius: f64, start_angle: f64, end_angle: f64) -> impl Fn([f64; 2]) -> f64 {
    move |p| {
        let dx = p[0] - center[0];
        let dy = p[1] - center[1];
        let r = (dx * dx + dy * dy).sqrt();
        let angle = dy.atan2(dx);
        // Normalize angle to [start, end]
        let mut a = angle;
        while a < start_angle { a += 2.0 * std::f64::consts::PI; }
        while a > start_angle + 2.0 * std::f64::consts::PI { a -= 2.0 * std::f64::consts::PI; }
        if a >= start_angle && a <= end_angle {
            (r - radius).abs()
        } else {
            // Distance to nearest endpoint
            let p1 = [center[0] + radius * start_angle.cos(), center[1] + radius * start_angle.sin()];
            let p2 = [center[0] + radius * end_angle.cos(), center[1] + radius * end_angle.sin()];
            let d1 = ((p[0] - p1[0]).powi(2) + (p[1] - p1[1]).powi(2)).sqrt();
            let d2 = ((p[0] - p2[0]).powi(2) + (p[1] - p2[1]).powi(2)).sqrt();
            d1.min(d2)
        }
    }
}

/// 2D polygon SDF from vertices (convex or concave).
pub fn sketch_polygon(vertices: Vec<[f64; 2]>) -> impl Fn([f64; 2]) -> f64 {
    move |p| {
        let n = vertices.len();
        if n < 3 { return f64::MAX; }
        let mut min_dist = f64::MAX;
        let mut winding = 0i32;
        for i in 0..n {
            let j = (i + 1) % n;
            let a = vertices[i];
            let b = vertices[j];
            // Distance to edge
            let ab = [b[0] - a[0], b[1] - a[1]];
            let ap = [p[0] - a[0], p[1] - a[1]];
            let t = (ap[0] * ab[0] + ap[1] * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1] + 1e-30);
            let t = t.clamp(0.0, 1.0);
            let closest = [a[0] + t * ab[0], a[1] + t * ab[1]];
            let d = ((p[0] - closest[0]).powi(2) + (p[1] - closest[1]).powi(2)).sqrt();
            min_dist = min_dist.min(d);
            // Winding number
            if a[1] <= p[1] {
                if b[1] > p[1] {
                    let cross = ab[0] * ap[1] - ab[1] * ap[0];
                    if cross > 0.0 { winding += 1; }
                }
            } else if b[1] <= p[1] {
                let cross = ab[0] * ap[1] - ab[1] * ap[0];
                if cross < 0.0 { winding -= 1; }
            }
        }
        if winding != 0 { -min_dist } else { min_dist }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circle() {
        let c = sketch_circle([0.0, 0.0], 1.0);
        assert!(c([0.0, 0.0]) < 0.0);
        assert!((c([1.0, 0.0])).abs() < 1e-10);
        assert!(c([2.0, 0.0]) > 0.0);
    }

    #[test]
    fn test_rect() {
        let r = sketch_rect([0.0, 0.0], [2.0, 1.0]);
        assert!(r([1.0, 0.5]) < 0.0);
        assert!(r([3.0, 0.5]) > 0.0);
    }

    #[test]
    fn test_line_segment() {
        let l = sketch_line_segment([0.0, 0.0], [1.0, 0.0]);
        assert!((l([0.5, 0.0]) - 0.0).abs() < 1e-10);
        assert!((l([0.5, 1.0]) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_polygon_square() {
        let sq = sketch_polygon(vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        assert!(sq([0.5, 0.5]) < 0.0);
        assert!(sq([2.0, 0.5]) > 0.0);
    }

    #[test]
    fn test_polygon_triangle() {
        let tri = sketch_polygon(vec![[0.0, 0.0], [2.0, 0.0], [1.0, 2.0]]);
        assert!(tri([1.0, 0.5]) < 0.0);
    }
}
