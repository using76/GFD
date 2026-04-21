//! Non-Uniform Rational B-Spline (NURBS) curve.
//!
//! Extends `BSplineCurve` with per-control-point weights. Evaluation uses
//! the standard projective formulation:
//!
//! C(u) = Σ_i N_{i,p}(u) · w_i · P_i  /  Σ_i N_{i,p}(u) · w_i
//!
//! When all weights equal 1 this reduces to a plain non-rational B-spline.
//! Conic sections (circle, ellipse, hyperbola, parabola) can be represented
//! exactly by a degree-2 NURBS — something no polynomial B-spline can do.

use serde::{Deserialize, Serialize};

use crate::{BSplineCurve, Curve, GeomError, GeomResult, Point3, Vector3};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NurbsCurve {
    pub degree: usize,
    pub control_points: Vec<Point3>,
    pub weights: Vec<f64>,
    pub knots: Vec<f64>,
}

impl NurbsCurve {
    /// Construct a NURBS curve. `knots.len() == control_points.len() + degree + 1`
    /// and `weights.len() == control_points.len()` with strictly positive weights.
    pub fn new(
        degree: usize,
        control_points: Vec<Point3>,
        weights: Vec<f64>,
        knots: Vec<f64>,
    ) -> GeomResult<Self> {
        if control_points.len() < degree + 1 {
            return Err(GeomError::Degenerate("insufficient control points"));
        }
        if weights.len() != control_points.len() {
            return Err(GeomError::Degenerate("weight count ≠ control point count"));
        }
        if weights.iter().any(|w| *w <= 0.0) {
            return Err(GeomError::Degenerate("weights must be strictly positive"));
        }
        if knots.len() != control_points.len() + degree + 1 {
            return Err(GeomError::Degenerate("knot vector length mismatch"));
        }
        Ok(Self { degree, control_points, weights, knots })
    }

    /// Construct a NURBS with clamped-uniform knots (same convention as
    /// `BSplineCurve::clamped_uniform`) and explicit weights.
    pub fn clamped_uniform(
        degree: usize,
        control_points: Vec<Point3>,
        weights: Vec<f64>,
    ) -> GeomResult<Self> {
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
        Self::new(degree, control_points, weights, knots)
    }

    /// Promote to a (non-rational) BSpline curve by discarding weights.
    pub fn to_bspline(&self) -> GeomResult<BSplineCurve> {
        BSplineCurve::new(self.degree, self.control_points.clone(), self.knots.clone())
    }

    /// Build the numerator curve (weighted control points) and a parallel
    /// weight array so eval can divide once at the end.
    fn weighted_bspline(&self) -> GeomResult<(BSplineCurve, BSplineCurve)> {
        let weighted: Vec<Point3> = self.control_points.iter().zip(&self.weights)
            .map(|(p, w)| Point3::new(w * p.x, w * p.y, w * p.z))
            .collect();
        // The denominator is a scalar B-spline; encode it as a B-spline of
        // "points" whose x component is the weight (y, z ignored).
        let denom_cps: Vec<Point3> = self.weights.iter()
            .map(|w| Point3::new(*w, 0.0, 0.0))
            .collect();
        Ok((
            BSplineCurve::new(self.degree, weighted, self.knots.clone())?,
            BSplineCurve::new(self.degree, denom_cps, self.knots.clone())?,
        ))
    }
}

impl Curve for NurbsCurve {
    fn u_range(&self) -> (f64, f64) {
        (
            self.knots[self.degree],
            self.knots[self.knots.len() - self.degree - 1],
        )
    }

    fn eval(&self, u: f64) -> GeomResult<Point3> {
        let (u0, u1) = self.u_range();
        if u < u0 - crate::LINEAR_TOL || u > u1 + crate::LINEAR_TOL {
            return Err(GeomError::OutOfRange(u));
        }
        let (num_curve, den_curve) = self.weighted_bspline()?;
        let num = num_curve.eval(u)?;
        let den = den_curve.eval(u)?.x;
        if den.abs() < f64::EPSILON {
            return Err(GeomError::Degenerate("NURBS denominator is zero"));
        }
        Ok(Point3::new(num.x / den, num.y / den, num.z / den))
    }

    fn tangent(&self, u: f64) -> GeomResult<Vector3> {
        // Quotient-rule derivative: C = N/W implies
        //   C'(u) = (N'(u) - C(u)·W'(u)) / W(u)
        let (num_curve, den_curve) = self.weighted_bspline()?;
        let num = num_curve.eval(u)?;
        let den = den_curve.eval(u)?.x;
        if den.abs() < f64::EPSILON {
            return Err(GeomError::Degenerate("NURBS denominator is zero"));
        }
        let c = Point3::new(num.x / den, num.y / den, num.z / den);
        let num_t = num_curve.tangent(u)?;
        let den_t = den_curve.tangent(u)?.x;
        Ok(Vector3::new(
            (num_t.x - c.x * den_t) / den,
            (num_t.y - c.y * den_t) / den,
            (num_t.z - c.z * den_t) / den,
        ))
    }

    fn length(&self) -> f64 {
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
    fn uniform_weights_reduce_to_bspline() {
        let cps = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        ];
        let ws = vec![1.0, 1.0, 1.0];
        let n = NurbsCurve::clamped_uniform(2, cps.clone(), ws).unwrap();
        let b = BSplineCurve::clamped_uniform(2, cps).unwrap();
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let pn = n.eval(t).unwrap();
            let pb = b.eval(t).unwrap();
            assert_abs_diff_eq!(pn.x, pb.x, epsilon = 1e-10);
            assert_abs_diff_eq!(pn.y, pb.y, epsilon = 1e-10);
        }
    }

    #[test]
    fn endpoint_interpolation_with_weights() {
        // Clamped NURBS interpolates endpoints independent of weights.
        let cps = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        ];
        let ws = vec![1.0, 3.0, 1.0]; // middle weight inflated
        let n = NurbsCurve::clamped_uniform(2, cps, ws).unwrap();
        let p0 = n.eval(0.0).unwrap();
        let p1 = n.eval(1.0).unwrap();
        assert_abs_diff_eq!(p0.x, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(p1.x, 2.0, epsilon = 1e-10);
    }

    #[test]
    fn weighted_middle_pulls_curve_higher() {
        // With w1 >> w0=w2 the curve should pass closer to P1 (0, 2, 0)
        // than the equal-weight case.
        let cps = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        ];
        let equal_weights = NurbsCurve::clamped_uniform(2, cps.clone(), vec![1.0, 1.0, 1.0]).unwrap();
        let heavy_middle  = NurbsCurve::clamped_uniform(2, cps,         vec![1.0, 5.0, 1.0]).unwrap();
        let mid_equal  = equal_weights.eval(0.5).unwrap();
        let mid_heavy  = heavy_middle.eval(0.5).unwrap();
        assert!(mid_heavy.y > mid_equal.y,
            "heavy middle weight should pull curve up (equal={}, heavy={})",
            mid_equal.y, mid_heavy.y);
    }

    #[test]
    fn weights_must_be_positive() {
        let cps = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        ];
        assert!(NurbsCurve::clamped_uniform(2, cps, vec![1.0, 0.0, 1.0]).is_err());
    }
}
