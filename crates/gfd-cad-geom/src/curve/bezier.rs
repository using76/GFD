//! Bezier curve (de Casteljau evaluation).
//!
//! A Bezier curve of degree n is defined by (n+1) control points and
//! parameterised over [0, 1]. Evaluation uses the numerically stable
//! de Casteljau recursion rather than the explicit Bernstein polynomial.

use serde::{Deserialize, Serialize};

use crate::{Curve, GeomError, GeomResult, Point3, Vector3};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bezier {
    pub control_points: Vec<Point3>,
}

impl Bezier {
    /// Construct a Bezier curve from ≥ 2 control points.
    pub fn new(control_points: Vec<Point3>) -> GeomResult<Self> {
        if control_points.len() < 2 {
            return Err(GeomError::Degenerate("bezier: need ≥ 2 control points"));
        }
        Ok(Self { control_points })
    }

    /// Degree n = control_points.len() - 1.
    pub fn degree(&self) -> usize {
        self.control_points.len() - 1
    }

    /// de Casteljau recursion — O(n²) evaluation, numerically stable across
    /// the full t ∈ [0, 1] range.
    fn de_casteljau(&self, t: f64) -> Point3 {
        let mut pts = self.control_points.clone();
        let n = pts.len();
        for r in 1..n {
            for i in 0..(n - r) {
                let a = pts[i];
                let b = pts[i + 1];
                pts[i] = Point3::new(
                    (1.0 - t) * a.x + t * b.x,
                    (1.0 - t) * a.y + t * b.y,
                    (1.0 - t) * a.z + t * b.z,
                );
            }
        }
        pts[0]
    }

    /// Derivative Bezier curve: degree n-1 with control points
    /// Q_i = n · (P_{i+1} − P_i). Evaluates to B'(t) as a tangent vector.
    pub fn derivative(&self) -> GeomResult<Bezier> {
        let n = self.degree();
        if n == 0 {
            return Err(GeomError::Degenerate("bezier: cannot differentiate degree 0"));
        }
        let nf = n as f64;
        let mut q = Vec::with_capacity(n);
        for i in 0..n {
            let a = self.control_points[i];
            let b = self.control_points[i + 1];
            q.push(Point3::new(nf * (b.x - a.x), nf * (b.y - a.y), nf * (b.z - a.z)));
        }
        Bezier::new(q)
    }
}

impl Curve for Bezier {
    fn u_range(&self) -> (f64, f64) { (0.0, 1.0) }

    fn eval(&self, u: f64) -> GeomResult<Point3> {
        if !(-crate::LINEAR_TOL..=1.0 + crate::LINEAR_TOL).contains(&u) {
            return Err(GeomError::OutOfRange(u));
        }
        let t = u.clamp(0.0, 1.0);
        Ok(self.de_casteljau(t))
    }

    fn tangent(&self, u: f64) -> GeomResult<Vector3> {
        let deriv = self.derivative()?;
        let t = u.clamp(0.0, 1.0);
        let v = deriv.de_casteljau(t);
        Ok(Vector3::new(v.x, v.y, v.z))
    }

    fn length(&self) -> f64 {
        // Gauss-Legendre is overkill for GUI labels; sample densely.
        let steps = 64.max(self.control_points.len() * 8);
        let mut prev = self.de_casteljau(0.0);
        let mut len = 0.0;
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let p = self.de_casteljau(t);
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
    fn linear_bezier_interpolates() {
        let b = Bezier::new(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        ]).unwrap();
        let p = b.eval(0.5).unwrap();
        assert_abs_diff_eq!(p.x, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn quadratic_bezier_endpoints_interpolate() {
        let b = Bezier::new(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        ]).unwrap();
        let p0 = b.eval(0.0).unwrap();
        let p1 = b.eval(1.0).unwrap();
        assert_abs_diff_eq!(p0.x, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(p1.x, 2.0, epsilon = 1e-10);
    }

    #[test]
    fn quadratic_bezier_peak_at_half() {
        // Classic symmetric "hill": y-peak at t=0.5 should be 0.5 for this
        // triangle of control points (P1.y=1 is not interpolated).
        let b = Bezier::new(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        ]).unwrap();
        let mid = b.eval(0.5).unwrap();
        assert_abs_diff_eq!(mid.y, 0.5, epsilon = 1e-10);
    }

    #[test]
    fn tangent_matches_finite_difference() {
        let b = Bezier::new(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(3.0, 0.0, 0.0),
        ]).unwrap();
        let t = 0.37;
        let h = 1e-6;
        let pm = b.eval(t - h).unwrap();
        let pp = b.eval(t + h).unwrap();
        let fd = Vector3::new((pp.x - pm.x) / (2.0 * h), (pp.y - pm.y) / (2.0 * h), (pp.z - pm.z) / (2.0 * h));
        let an = b.tangent(t).unwrap();
        assert_abs_diff_eq!(an.x, fd.x, epsilon = 1e-5);
        assert_abs_diff_eq!(an.y, fd.y, epsilon = 1e-5);
    }

    #[test]
    fn linear_bezier_length_equals_distance() {
        let b = Bezier::new(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(3.0, 4.0, 0.0),
        ]).unwrap();
        let len = b.length();
        assert_abs_diff_eq!(len, 5.0, epsilon = 1e-4);
    }
}
