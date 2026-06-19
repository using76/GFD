//! Non-Uniform Rational B-Spline (NURBS) surface.
//!
//! Extends [`BSplineSurface`] with per-control-point weights, evaluated by the
//! standard projective (homogeneous-coordinate) formulation:
//!
//! S(u,v) = Σ_i Σ_j N_{i,p}(u) N_{j,q}(v) w_{ij} P_{ij}
//!        / Σ_i Σ_j N_{i,p}(u) N_{j,q}(v) w_{ij}
//!
//! With all weights = 1 it reduces to a plain B-spline. Rational surfaces can
//! represent exact quadrics (spheres, cylinders, cones) — the form real CAD
//! systems emit via RATIONAL_B_SPLINE_SURFACE in STEP. Mirrors the existing
//! [`crate::NurbsCurve`] design: a weighted-numerator + scalar-denominator pair
//! of plain B-spline surfaces, divided once at the end.

use serde::{Deserialize, Serialize};

use crate::{BSplineSurface, GeomError, GeomResult, Point3, Surface, Vector3};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NurbsSurface {
    pub u_degree: usize,
    pub v_degree: usize,
    pub u_control_count: usize,
    pub v_control_count: usize,
    /// Control net, row-major: `control_points[i * v_control_count + j]`.
    pub control_points: Vec<Point3>,
    /// Weights, same row-major layout and length as `control_points`, all > 0.
    pub weights: Vec<f64>,
    pub u_knots: Vec<f64>,
    pub v_knots: Vec<f64>,
}

impl NurbsSurface {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        u_degree: usize,
        v_degree: usize,
        u_control_count: usize,
        v_control_count: usize,
        control_points: Vec<Point3>,
        weights: Vec<f64>,
        u_knots: Vec<f64>,
        v_knots: Vec<f64>,
    ) -> GeomResult<Self> {
        let n = u_control_count * v_control_count;
        if control_points.len() != n {
            return Err(GeomError::Degenerate("control net size mismatch"));
        }
        if weights.len() != n {
            return Err(GeomError::Degenerate("weight count ≠ control point count"));
        }
        if weights.iter().any(|w| *w <= 0.0) {
            return Err(GeomError::Degenerate("weights must be strictly positive"));
        }
        // Reuse BSplineSurface's degree/knot validation.
        let _ = BSplineSurface::new(
            u_degree, v_degree, u_control_count, v_control_count,
            control_points.clone(), u_knots.clone(), v_knots.clone(),
        )?;
        Ok(Self { u_degree, v_degree, u_control_count, v_control_count, control_points, weights, u_knots, v_knots })
    }

    /// Promote to a non-rational B-spline by discarding the weights.
    pub fn to_bspline(&self) -> GeomResult<BSplineSurface> {
        BSplineSurface::new(
            self.u_degree, self.v_degree, self.u_control_count, self.v_control_count,
            self.control_points.clone(), self.u_knots.clone(), self.v_knots.clone(),
        )
    }

    /// Numerator surface (weighted control points) and a scalar denominator
    /// surface (weights packed into the x component), so `eval`/partials divide
    /// once at the end — exactly mirroring `NurbsCurve::weighted_bspline`.
    fn weighted_bspline(&self) -> GeomResult<(BSplineSurface, BSplineSurface)> {
        let weighted: Vec<Point3> = self.control_points.iter().zip(&self.weights)
            .map(|(p, w)| Point3::new(w * p.x, w * p.y, w * p.z))
            .collect();
        let denom: Vec<Point3> = self.weights.iter().map(|w| Point3::new(*w, 0.0, 0.0)).collect();
        Ok((
            BSplineSurface::new(self.u_degree, self.v_degree, self.u_control_count, self.v_control_count,
                weighted, self.u_knots.clone(), self.v_knots.clone())?,
            BSplineSurface::new(self.u_degree, self.v_degree, self.u_control_count, self.v_control_count,
                denom, self.u_knots.clone(), self.v_knots.clone())?,
        ))
    }

    /// ∂S/∂u via the quotient rule: S = N/W ⇒ S_u = (N_u − S·W_u)/W.
    pub fn partial_u(&self, u: f64, v: f64) -> GeomResult<Vector3> {
        let (num, den) = self.weighted_bspline()?;
        let s = self.eval(u, v)?;
        let nu = num.partial_u(u, v)?;
        let wu = den.partial_u(u, v)?.x;
        let w = den.eval(u, v)?.x;
        if w.abs() < f64::EPSILON {
            return Err(GeomError::Degenerate("NURBS denominator is zero"));
        }
        Ok(Vector3::new((nu.x - s.x * wu) / w, (nu.y - s.y * wu) / w, (nu.z - s.z * wu) / w))
    }

    /// ∂S/∂v via the quotient rule.
    pub fn partial_v(&self, u: f64, v: f64) -> GeomResult<Vector3> {
        let (num, den) = self.weighted_bspline()?;
        let s = self.eval(u, v)?;
        let nv = num.partial_v(u, v)?;
        let wv = den.partial_v(u, v)?.x;
        let w = den.eval(u, v)?.x;
        if w.abs() < f64::EPSILON {
            return Err(GeomError::Degenerate("NURBS denominator is zero"));
        }
        Ok(Vector3::new((nv.x - s.x * wv) / w, (nv.y - s.y * wv) / w, (nv.z - s.z * wv) / w))
    }
}

impl Surface for NurbsSurface {
    fn u_range(&self) -> (f64, f64) {
        (self.u_knots[self.u_degree], self.u_knots[self.u_knots.len() - self.u_degree - 1])
    }

    fn v_range(&self) -> (f64, f64) {
        (self.v_knots[self.v_degree], self.v_knots[self.v_knots.len() - self.v_degree - 1])
    }

    fn eval(&self, u: f64, v: f64) -> GeomResult<Point3> {
        let (num, den) = self.weighted_bspline()?;
        let n = num.eval(u, v)?;
        let w = den.eval(u, v)?.x;
        if w.abs() < f64::EPSILON {
            return Err(GeomError::Degenerate("NURBS denominator is zero"));
        }
        Ok(Point3::new(n.x / w, n.y / w, n.z / w))
    }

    fn normal(&self, u: f64, v: f64) -> GeomResult<Vector3> {
        let su = self.partial_u(u, v)?;
        let sv = self.partial_v(u, v)?;
        let nrm = su.cross(sv);
        let len = nrm.norm();
        if len < f64::EPSILON {
            return Err(GeomError::Degenerate("degenerate surface normal"));
        }
        Ok(Vector3::new(nrm.x / len, nrm.y / len, nrm.z / len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    fn flat_cps() -> Vec<Point3> {
        vec![
            Point3::new(0.0, 0.0, 0.0), Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0), Point3::new(1.0, 1.0, 0.0),
        ]
    }

    #[test]
    fn uniform_weights_reduce_to_bspline() {
        let cps = flat_cps();
        let knots = vec![0.0, 0.0, 1.0, 1.0];
        let nurbs = NurbsSurface::new(1, 1, 2, 2, cps.clone(), vec![1.0; 4], knots.clone(), knots.clone()).unwrap();
        let bs = BSplineSurface::new(1, 1, 2, 2, cps, knots.clone(), knots).unwrap();
        for u in [0.0, 0.3, 0.5, 1.0] {
            for v in [0.0, 0.4, 1.0] {
                let pn = nurbs.eval(u, v).unwrap();
                let pb = bs.eval(u, v).unwrap();
                assert_abs_diff_eq!(pn.x, pb.x, epsilon = 1e-10);
                assert_abs_diff_eq!(pn.y, pb.y, epsilon = 1e-10);
                assert_abs_diff_eq!(pn.z, pb.z, epsilon = 1e-10);
            }
        }
    }

    #[test]
    fn weighted_corner_pulls_surface() {
        // Lift one corner out of plane and inflate its weight → the patch center
        // is pulled toward that corner more than with equal weights.
        let mut cps = flat_cps();
        cps[3] = Point3::new(1.0, 1.0, 1.0); // lift corner (1,1)
        let knots = vec![0.0, 0.0, 1.0, 1.0];
        let equal = NurbsSurface::new(1, 1, 2, 2, cps.clone(), vec![1.0; 4], knots.clone(), knots.clone()).unwrap();
        let heavy = NurbsSurface::new(1, 1, 2, 2, cps, vec![1.0, 1.0, 1.0, 5.0], knots.clone(), knots).unwrap();
        let ze = equal.eval(0.5, 0.5).unwrap().z;
        let zh = heavy.eval(0.5, 0.5).unwrap().z;
        assert!(zh > ze, "heavy corner weight should pull center up (equal z={}, heavy z={})", ze, zh);
    }

    #[test]
    fn partials_match_finite_difference() {
        let mut cps = flat_cps();
        cps[3] = Point3::new(1.0, 1.0, 1.0);
        let knots = vec![0.0, 0.0, 1.0, 1.0];
        let s = NurbsSurface::new(1, 1, 2, 2, cps, vec![1.0, 2.0, 1.5, 3.0], knots.clone(), knots).unwrap();
        let (u, v, h) = (0.4, 0.6, 1e-6);
        let su = s.partial_u(u, v).unwrap();
        let fd_u = {
            let a = s.eval(u + h, v).unwrap();
            let b = s.eval(u - h, v).unwrap();
            Vector3::new((a.x - b.x) / (2.0 * h), (a.y - b.y) / (2.0 * h), (a.z - b.z) / (2.0 * h))
        };
        assert_abs_diff_eq!(su.x, fd_u.x, epsilon = 1e-5);
        assert_abs_diff_eq!(su.z, fd_u.z, epsilon = 1e-5);
        let sv = s.partial_v(u, v).unwrap();
        let fd_v = {
            let a = s.eval(u, v + h).unwrap();
            let b = s.eval(u, v - h).unwrap();
            Vector3::new((a.x - b.x) / (2.0 * h), (a.y - b.y) / (2.0 * h), (a.z - b.z) / (2.0 * h))
        };
        assert_abs_diff_eq!(sv.y, fd_v.y, epsilon = 1e-5);
        assert_abs_diff_eq!(sv.z, fd_v.z, epsilon = 1e-5);
    }

    #[test]
    fn weights_must_be_positive() {
        let knots = vec![0.0, 0.0, 1.0, 1.0];
        assert!(NurbsSurface::new(1, 1, 2, 2, flat_cps(), vec![1.0, 0.0, 1.0, 1.0], knots.clone(), knots).is_err());
    }
}
