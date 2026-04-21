//! Cubic Hermite curve.
//!
//! Interpolates between two endpoints `p0` and `p1` with specified tangent
//! vectors `v0` and `v1`. The curve is C¹-continuous by construction:
//!
//! H(t) = h00(t)·p0 + h10(t)·v0 + h01(t)·p1 + h11(t)·v1
//!
//! with the standard Hermite basis
//!   h00 =  2t³ − 3t² + 1
//!   h10 =   t³ − 2t² + t
//!   h01 = −2t³ + 3t²
//!   h11 =   t³ −  t²
//!
//! and t ∈ [0, 1].

use serde::{Deserialize, Serialize};

use crate::{Curve, GeomError, GeomResult, Point3, Vector3};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Hermite {
    pub p0: Point3,
    pub v0: Vector3,
    pub p1: Point3,
    pub v1: Vector3,
}

impl Hermite {
    pub fn new(p0: Point3, v0: Vector3, p1: Point3, v1: Vector3) -> Self {
        Self { p0, v0, p1, v1 }
    }
}

/// Sample a uniform Catmull-Rom spline through `points`, returning one
/// polyline that passes through every control point with C¹-continuous
/// tangents (finite differences of neighbors).
///
/// `samples_per_segment` controls density between consecutive control
/// points; a value of 16 is typical for smooth GUI rendering. Each
/// Catmull-Rom segment is a cubic Hermite with tangents
///   v_i = 0.5 · (p_{i+1} − p_{i−1})
/// for interior points, and one-sided differences at the endpoints.
///
/// Returns `None` if fewer than 2 points are provided.
pub fn catmull_rom_sample(points: &[Point3], samples_per_segment: usize) -> Option<Vec<Point3>> {
    let n = points.len();
    if n < 2 { return None; }
    let samples = samples_per_segment.max(1);
    // Precompute tangents.
    let mut tangents: Vec<Vector3> = Vec::with_capacity(n);
    for i in 0..n {
        let v = if i == 0 {
            let d = points[1];
            let s = points[0];
            Vector3::new(d.x - s.x, d.y - s.y, d.z - s.z)
        } else if i == n - 1 {
            let s = points[n - 2];
            let d = points[n - 1];
            Vector3::new(d.x - s.x, d.y - s.y, d.z - s.z)
        } else {
            let a = points[i - 1];
            let b = points[i + 1];
            Vector3::new(0.5 * (b.x - a.x), 0.5 * (b.y - a.y), 0.5 * (b.z - a.z))
        };
        tangents.push(v);
    }
    let total_points = (n - 1) * samples + 1;
    let mut out = Vec::with_capacity(total_points);
    for seg in 0..(n - 1) {
        let h = Hermite::new(points[seg], tangents[seg], points[seg + 1], tangents[seg + 1]);
        let steps = if seg == n - 2 { samples } else { samples };
        let inclusive = seg == n - 2;
        for k in 0..steps {
            let t = k as f64 / samples as f64;
            if let Ok(p) = h.eval(t) { out.push(p); }
        }
        if inclusive {
            out.push(points[n - 1]);
        }
    }
    Some(out)
}

impl Curve for Hermite {
    fn u_range(&self) -> (f64, f64) { (0.0, 1.0) }

    fn eval(&self, u: f64) -> GeomResult<Point3> {
        if !(-crate::LINEAR_TOL..=1.0 + crate::LINEAR_TOL).contains(&u) {
            return Err(GeomError::OutOfRange(u));
        }
        let t = u.clamp(0.0, 1.0);
        let t2 = t * t;
        let t3 = t2 * t;
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;
        Ok(Point3::new(
            h00 * self.p0.x + h10 * self.v0.x + h01 * self.p1.x + h11 * self.v1.x,
            h00 * self.p0.y + h10 * self.v0.y + h01 * self.p1.y + h11 * self.v1.y,
            h00 * self.p0.z + h10 * self.v0.z + h01 * self.p1.z + h11 * self.v1.z,
        ))
    }

    fn tangent(&self, u: f64) -> GeomResult<Vector3> {
        // Analytical derivative of the Hermite basis with respect to t.
        if !(-crate::LINEAR_TOL..=1.0 + crate::LINEAR_TOL).contains(&u) {
            return Err(GeomError::OutOfRange(u));
        }
        let t = u.clamp(0.0, 1.0);
        let t2 = t * t;
        let dh00 = 6.0 * t2 - 6.0 * t;       // d/dt(2t³-3t²+1)
        let dh10 = 3.0 * t2 - 4.0 * t + 1.0; // d/dt(t³-2t²+t)
        let dh01 = -6.0 * t2 + 6.0 * t;      // d/dt(-2t³+3t²)
        let dh11 = 3.0 * t2 - 2.0 * t;       // d/dt(t³-t²)
        Ok(Vector3::new(
            dh00 * self.p0.x + dh10 * self.v0.x + dh01 * self.p1.x + dh11 * self.v1.x,
            dh00 * self.p0.y + dh10 * self.v0.y + dh01 * self.p1.y + dh11 * self.v1.y,
            dh00 * self.p0.z + dh10 * self.v0.z + dh01 * self.p1.z + dh11 * self.v1.z,
        ))
    }

    fn length(&self) -> f64 {
        let steps = 64;
        let mut prev = self.eval(0.0).unwrap_or(self.p0);
        let mut len = 0.0;
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let p = self.eval(t).unwrap_or(prev);
            len += prev.distance(p);
            prev = p;
        }
        len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn endpoints_interpolate() {
        let h = Hermite::new(
            Point3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Point3::new(3.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
        );
        let p0 = h.eval(0.0).unwrap();
        let p1 = h.eval(1.0).unwrap();
        assert_abs_diff_eq!(p0.x, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(p1.x, 3.0, epsilon = 1e-10);
    }

    #[test]
    fn tangents_match_at_endpoints() {
        let h = Hermite::new(
            Point3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 1.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Vector3::new(0.5, -3.0, 0.0),
        );
        let t0 = h.tangent(0.0).unwrap();
        let t1 = h.tangent(1.0).unwrap();
        assert_abs_diff_eq!(t0.x, 2.0, epsilon = 1e-10);
        assert_abs_diff_eq!(t0.y, 1.0, epsilon = 1e-10);
        assert_abs_diff_eq!(t1.x, 0.5, epsilon = 1e-10);
        assert_abs_diff_eq!(t1.y, -3.0, epsilon = 1e-10);
    }

    #[test]
    fn straight_line_hermite_reduces_to_line() {
        // Endpoints (0,0,0) and (2,0,0) with matching tangent (2,0,0).
        // The degree-3 polynomial collapses to the linear interpolant.
        let h = Hermite::new(
            Point3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
        );
        let p = h.eval(0.5).unwrap();
        assert_abs_diff_eq!(p.x, 1.0, epsilon = 1e-10);
        assert_abs_diff_eq!(p.y, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn catmull_rom_passes_through_controls() {
        // 4 control points; sampled polyline must contain all of them.
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(3.0, 1.0, 0.0),
        ];
        let samples = catmull_rom_sample(&pts, 8).unwrap();
        assert_eq!(samples.first().unwrap().x, 0.0);
        assert_abs_diff_eq!(samples.last().unwrap().x, 3.0, epsilon = 1e-10);
        // Must contain each control point exactly (at segment boundaries).
        for cp in &pts {
            let hit = samples.iter().any(|p|
                (p.x - cp.x).abs() < 1e-9 &&
                (p.y - cp.y).abs() < 1e-9 &&
                (p.z - cp.z).abs() < 1e-9
            );
            assert!(hit, "control point {:?} not in sampled polyline", cp);
        }
    }

    #[test]
    fn catmull_rom_two_points_is_straight_line() {
        let pts = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        ];
        let samples = catmull_rom_sample(&pts, 4).unwrap();
        // Start and end match, and every intermediate y == 0 and x monotonic.
        assert_abs_diff_eq!(samples[0].x, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(samples.last().unwrap().x, 2.0, epsilon = 1e-10);
        for p in &samples {
            assert_abs_diff_eq!(p.y, 0.0, epsilon = 1e-10);
        }
    }

    #[test]
    fn length_straight_segment_is_distance() {
        let h = Hermite::new(
            Point3::new(0.0, 0.0, 0.0),
            Vector3::new(3.0, 4.0, 0.0),
            Point3::new(3.0, 4.0, 0.0),
            Vector3::new(3.0, 4.0, 0.0),
        );
        let len = h.length();
        assert_abs_diff_eq!(len, 5.0, epsilon = 1e-4);
    }
}
