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

    /// Analytical derivative curve: degree-p B-spline → degree-(p-1) B-spline
    /// with control points Q_i = p · (P_{i+1} − P_i) / (u_{i+p+1} − u_{i+1})
    /// and knot vector self.knots\[1..len-1\]. The result's control points are
    /// stored as `Point3` but represent tangent vectors (linear-combination
    /// evaluates to a vector, since the Qi are already position differences).
    pub fn derivative(&self) -> GeomResult<BSplineCurve> {
        if self.degree == 0 {
            return Err(GeomError::Degenerate("cannot differentiate degree-0 B-spline"));
        }
        let p = self.degree;
        let n = self.control_points.len();
        let mut new_cps = Vec::with_capacity(n - 1);
        for i in 0..n - 1 {
            let denom = self.knots[i + p + 1] - self.knots[i + 1];
            let coef = if denom.abs() < f64::EPSILON { 0.0 } else { p as f64 / denom };
            let a = self.control_points[i];
            let b = self.control_points[i + 1];
            new_cps.push(Point3::new(coef * (b.x - a.x), coef * (b.y - a.y), coef * (b.z - a.z)));
        }
        let new_knots = self.knots[1..self.knots.len() - 1].to_vec();
        BSplineCurve::new(p - 1, new_cps, new_knots)
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
        // Analytical derivative via degree reduction. For degree-0 B-splines
        // (piecewise-constant) the tangent is the zero vector — a finite
        // difference fallback approximates it but the analytical result is 0.
        if self.degree == 0 {
            return Ok(Vector3::ZERO);
        }
        let deriv = self.derivative()?;
        let v = deriv.eval(u)?;
        Ok(Vector3::new(v.x, v.y, v.z))
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

    #[test]
    fn degree1_tangent_constant() {
        // Degree-1 clamped uniform between (0,0,0) and (2,0,0): tangent is
        // (2, 0, 0) everywhere (constant derivative of a straight line segment).
        let cps = vec![Point3::new(0.0, 0.0, 0.0), Point3::new(2.0, 0.0, 0.0)];
        let c = BSplineCurve::clamped_uniform(1, cps).unwrap();
        let t = c.tangent(0.5).unwrap();
        assert_abs_diff_eq!(t.x, 2.0, epsilon = 1e-10);
        assert_abs_diff_eq!(t.y, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(t.z, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn analytic_tangent_matches_finite_difference() {
        // For a quadratic clamped B-spline the analytical derivative should
        // match a centered finite difference to high precision.
        let cps = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(3.0, 0.0, 0.0),
        ];
        let c = BSplineCurve::clamped_uniform(2, cps).unwrap();
        let u = 0.37;
        let h = 1.0e-5;
        let pm = c.eval(u - h).unwrap();
        let pp = c.eval(u + h).unwrap();
        let fd = Vector3::new((pp.x - pm.x) / (2.0 * h), (pp.y - pm.y) / (2.0 * h), (pp.z - pm.z) / (2.0 * h));
        let an = c.tangent(u).unwrap();
        assert_abs_diff_eq!(an.x, fd.x, epsilon = 1e-6);
        assert_abs_diff_eq!(an.y, fd.y, epsilon = 1e-6);
        assert_abs_diff_eq!(an.z, fd.z, epsilon = 1e-6);
    }

    #[test]
    fn derivative_curve_reduces_degree() {
        let cps = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(3.0, 0.0, 0.0),
            Point3::new(5.0, 2.0, 0.0),
        ];
        let c = BSplineCurve::clamped_uniform(3, cps).unwrap();
        let d = c.derivative().unwrap();
        assert_eq!(d.degree, 2);
        assert_eq!(d.control_points.len(), 3);
    }
}
