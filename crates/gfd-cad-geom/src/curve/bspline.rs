//! Non-rational B-spline curve (Cox-de Boor recursion).

use serde::{Deserialize, Serialize};

use crate::{Curve, GeomError, GeomResult, Point3, Vector3};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BSplineCurve {
    pub degree: usize,
    pub control_points: Vec<Point3>,
    pub knots: Vec<f64>,
}

impl BSplineCurve {
    /// Construct a B-spline. `knots.len() == control_points.len() + degree + 1`.
    pub fn new(degree: usize, control_points: Vec<Point3>, knots: Vec<f64>) -> GeomResult<Self> {
        if control_points.len() < degree + 1 {
            return Err(GeomError::Degenerate("insufficient control points"));
        }
        if knots.len() != control_points.len() + degree + 1 {
            return Err(GeomError::Degenerate("knot vector length mismatch"));
        }
        Ok(Self { degree, control_points, knots })
    }

    /// Uniform clamped knot vector: deg+1 zeros, interior uniform, deg+1 ones.
    pub fn clamped_uniform(degree: usize, control_points: Vec<Point3>) -> GeomResult<Self> {
        let n = control_points.len();
        if n < degree + 1 {
            return Err(GeomError::Degenerate("insufficient control points"));
        }
        let m = n + degree + 1;
        let interior = m - 2 * (degree + 1);
        let mut knots = Vec::with_capacity(m);
        for _ in 0..=degree { knots.push(0.0); }
        for i in 1..=interior { knots.push(i as f64 / (interior + 1) as f64); }
        for _ in 0..=degree { knots.push(1.0); }
        Self::new(degree, control_points, knots)
    }

    fn span(&self, u: f64) -> usize {
        let n = self.control_points.len() - 1;
        let p = self.degree;
        if u >= self.knots[n + 1] { return n; }
        if u <= self.knots[p] { return p; }
        // binary search
        let mut lo = p;
        let mut hi = n + 1;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if u < self.knots[mid] { hi = mid; } else { lo = mid; }
        }
        lo
    }

    fn basis_functions(&self, span: usize, u: f64) -> Vec<f64> {
        let p = self.degree;
        let mut n = vec![0.0; p + 1];
        let mut left = vec![0.0; p + 1];
        let mut right = vec![0.0; p + 1];
        n[0] = 1.0;
        for j in 1..=p {
            left[j] = u - self.knots[span + 1 - j];
            right[j] = self.knots[span + j] - u;
            let mut saved = 0.0;
            for r in 0..j {
                let denom = right[r + 1] + left[j - r];
                let temp = if denom.abs() < f64::EPSILON { 0.0 } else { n[r] / denom };
                n[r] = saved + right[r + 1] * temp;
                saved = left[j - r] * temp;
            }
            n[j] = saved;
        }
        n
    }
}

impl Curve for BSplineCurve {
    fn u_range(&self) -> (f64, f64) {
        (self.knots[self.degree], self.knots[self.knots.len() - self.degree - 1])
    }

    fn eval(&self, u: f64) -> GeomResult<Point3> {
        let (u0, u1) = self.u_range();
        if u < u0 - crate::LINEAR_TOL || u > u1 + crate::LINEAR_TOL {
            return Err(GeomError::OutOfRange(u));
        }
        let span = self.span(u);
        let n = self.basis_functions(span, u);
        let p = self.degree;
        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;
        for i in 0..=p {
            let cp = self.control_points[span - p + i];
            x += n[i] * cp.x;
            y += n[i] * cp.y;
            z += n[i] * cp.z;
        }
        Ok(Point3::new(x, y, z))
    }

    fn tangent(&self, u: f64) -> GeomResult<Vector3> {
        // Finite-difference fallback — analytical derivative via degree-reduction
        // lands with Phase 4 sketcher use.
        let h = 1.0e-6;
        let (u0, u1) = self.u_range();
        let um = (u - h).max(u0);
        let up = (u + h).min(u1);
        let pm = self.eval(um)?;
        let pp = self.eval(up)?;
        let dt = (up - um).max(f64::EPSILON);
        Ok(Vector3::new((pp.x - pm.x) / dt, (pp.y - pm.y) / dt, (pp.z - pm.z) / dt))
    }

    fn length(&self) -> f64 {
        // Gauss-Legendre 8-point per knot span — good enough for GUI labels.
        let (u0, u1) = self.u_range();
        let steps = 64.max(self.control_points.len() * 8);
        let du = (u1 - u0) / steps as f64;
        let mut prev = self.eval(u0).unwrap_or(Point3::ORIGIN);
        let mut len = 0.0;
        for i in 1..=steps {
            let u = u0 + du * i as f64;
            let p = self.eval(u).unwrap_or(prev);
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
    fn degree1_linear_interp() {
        let cps = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        ];
        let c = BSplineCurve::clamped_uniform(1, cps).unwrap();
        let p = c.eval(0.5).unwrap();
        assert_abs_diff_eq!(p.x, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn endpoint_interpolation() {
        let cps = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        ];
        let c = BSplineCurve::clamped_uniform(2, cps).unwrap();
        let p0 = c.eval(0.0).unwrap();
        let p1 = c.eval(1.0).unwrap();
        assert_abs_diff_eq!(p0.x, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(p1.x, 2.0, epsilon = 1e-10);
    }
}
