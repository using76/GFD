//! Non-rational tensor-product B-spline surface.
//!
//! S(u, v) = Σ_i Σ_j N_{i,p}(u) · N_{j,q}(v) · P_{i,j}
//!
//! Control net is stored row-major: `control_points[i * n_v + j]` where
//! `i ∈ [0, n_u)`, `j ∈ [0, n_v)` and `n_u = u_control_count`, `n_v = v_control_count`.

use serde::{Deserialize, Serialize};

use crate::{GeomError, GeomResult, Point3, Surface, Vector3};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BSplineSurface {
    pub u_degree: usize,
    pub v_degree: usize,
    pub u_control_count: usize,
    pub v_control_count: usize,
    pub control_points: Vec<Point3>,
    pub u_knots: Vec<f64>,
    pub v_knots: Vec<f64>,
}

impl BSplineSurface {
    pub fn new(
        u_degree: usize,
        v_degree: usize,
        u_control_count: usize,
        v_control_count: usize,
        control_points: Vec<Point3>,
        u_knots: Vec<f64>,
        v_knots: Vec<f64>,
    ) -> GeomResult<Self> {
        if u_control_count < u_degree + 1 || v_control_count < v_degree + 1 {
            return Err(GeomError::Degenerate("insufficient control points"));
        }
        if control_points.len() != u_control_count * v_control_count {
            return Err(GeomError::Degenerate("control net size mismatch"));
        }
        if u_knots.len() != u_control_count + u_degree + 1 {
            return Err(GeomError::Degenerate("u knot vector length mismatch"));
        }
        if v_knots.len() != v_control_count + v_degree + 1 {
            return Err(GeomError::Degenerate("v knot vector length mismatch"));
        }
        Ok(Self {
            u_degree,
            v_degree,
            u_control_count,
            v_control_count,
            control_points,
            u_knots,
            v_knots,
        })
    }

    /// Build a surface with clamped uniform knot vectors in both directions.
    /// `control_points` is row-major as described in the type docs.
    pub fn clamped_uniform(
        u_degree: usize,
        v_degree: usize,
        u_control_count: usize,
        v_control_count: usize,
        control_points: Vec<Point3>,
    ) -> GeomResult<Self> {
        let u_knots = clamped_uniform_knots(u_degree, u_control_count)?;
        let v_knots = clamped_uniform_knots(v_degree, v_control_count)?;
        Self::new(
            u_degree,
            v_degree,
            u_control_count,
            v_control_count,
            control_points,
            u_knots,
            v_knots,
        )
    }

    #[inline]
    fn cp(&self, i: usize, j: usize) -> Point3 {
        self.control_points[i * self.v_control_count + j]
    }

    fn span(knots: &[f64], n_last: usize, degree: usize, u: f64) -> usize {
        if u >= knots[n_last + 1] {
            return n_last;
        }
        if u <= knots[degree] {
            return degree;
        }
        let mut lo = degree;
        let mut hi = n_last + 1;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if u < knots[mid] {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        lo
    }

    fn basis(knots: &[f64], degree: usize, span: usize, u: f64) -> Vec<f64> {
        let p = degree;
        let mut n = vec![0.0; p + 1];
        let mut left = vec![0.0; p + 1];
        let mut right = vec![0.0; p + 1];
        n[0] = 1.0;
        for j in 1..=p {
            left[j] = u - knots[span + 1 - j];
            right[j] = knots[span + j] - u;
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

fn clamped_uniform_knots(degree: usize, control_count: usize) -> GeomResult<Vec<f64>> {
    if control_count < degree + 1 {
        return Err(GeomError::Degenerate("insufficient control points"));
    }
    let m = control_count + degree + 1;
    let interior = m - 2 * (degree + 1);
    let mut knots = Vec::with_capacity(m);
    for _ in 0..=degree {
        knots.push(0.0);
    }
    for i in 1..=interior {
        knots.push(i as f64 / (interior + 1) as f64);
    }
    for _ in 0..=degree {
        knots.push(1.0);
    }
    Ok(knots)
}

impl Surface for BSplineSurface {
    fn u_range(&self) -> (f64, f64) {
        (
            self.u_knots[self.u_degree],
            self.u_knots[self.u_knots.len() - self.u_degree - 1],
        )
    }

    fn v_range(&self) -> (f64, f64) {
        (
            self.v_knots[self.v_degree],
            self.v_knots[self.v_knots.len() - self.v_degree - 1],
        )
    }

    fn eval(&self, u: f64, v: f64) -> GeomResult<Point3> {
        let (u0, u1) = self.u_range();
        let (v0, v1) = self.v_range();
        if u < u0 - crate::LINEAR_TOL || u > u1 + crate::LINEAR_TOL {
            return Err(GeomError::OutOfRange(u));
        }
        if v < v0 - crate::LINEAR_TOL || v > v1 + crate::LINEAR_TOL {
            return Err(GeomError::OutOfRange(v));
        }
        let p = self.u_degree;
        let q = self.v_degree;
        let u_span = Self::span(&self.u_knots, self.u_control_count - 1, p, u);
        let v_span = Self::span(&self.v_knots, self.v_control_count - 1, q, v);
        let nu = Self::basis(&self.u_knots, p, u_span, u);
        let nv = Self::basis(&self.v_knots, q, v_span, v);
        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;
        for i in 0..=p {
            for j in 0..=q {
                let w = nu[i] * nv[j];
                let cp = self.cp(u_span - p + i, v_span - q + j);
                x += w * cp.x;
                y += w * cp.y;
                z += w * cp.z;
            }
        }
        Ok(Point3::new(x, y, z))
    }

    fn normal(&self, u: f64, v: f64) -> GeomResult<Vector3> {
        // Finite-difference partial derivatives — matches BSplineCurve::tangent
        // approach. Analytical derivative lands with later Phase 1 work.
        let h = 1.0e-6;
        let (u0, u1) = self.u_range();
        let (v0, v1) = self.v_range();
        let um = (u - h).max(u0);
        let up = (u + h).min(u1);
        let vm = (v - h).max(v0);
        let vp = (v + h).min(v1);
        let pu_m = self.eval(um, v)?;
        let pu_p = self.eval(up, v)?;
        let pv_m = self.eval(u, vm)?;
        let pv_p = self.eval(u, vp)?;
        let du = (up - um).max(f64::EPSILON);
        let dv = (vp - vm).max(f64::EPSILON);
        let su = Vector3::new((pu_p.x - pu_m.x) / du, (pu_p.y - pu_m.y) / du, (pu_p.z - pu_m.z) / du);
        let sv = Vector3::new((pv_p.x - pv_m.x) / dv, (pv_p.y - pv_m.y) / dv, (pv_p.z - pv_m.z) / dv);
        let n = su.cross(sv);
        let len = n.norm();
        if len < f64::EPSILON {
            return Err(GeomError::Degenerate("degenerate surface normal"));
        }
        Ok(Vector3::new(n.x / len, n.y / len, n.z / len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    fn flat_patch() -> BSplineSurface {
        // Bilinear 2x2 control net on z=0, 1x1 unit square.
        let cps = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
        ];
        BSplineSurface::clamped_uniform(1, 1, 2, 2, cps).unwrap()
    }

    #[test]
    fn bilinear_eval_center() {
        let s = flat_patch();
        let p = s.eval(0.5, 0.5).unwrap();
        assert_abs_diff_eq!(p.x, 0.5, epsilon = 1e-10);
        assert_abs_diff_eq!(p.y, 0.5, epsilon = 1e-10);
        assert_abs_diff_eq!(p.z, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn bilinear_corner_interpolation() {
        let s = flat_patch();
        let p00 = s.eval(0.0, 0.0).unwrap();
        let p11 = s.eval(1.0, 1.0).unwrap();
        assert_abs_diff_eq!(p00.x, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(p00.y, 0.0, epsilon = 1e-10);
        assert_abs_diff_eq!(p11.x, 1.0, epsilon = 1e-10);
        assert_abs_diff_eq!(p11.y, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn bilinear_normal_is_z() {
        let s = flat_patch();
        let n = s.normal(0.5, 0.5).unwrap();
        assert_abs_diff_eq!(n.x, 0.0, epsilon = 1e-5);
        assert_abs_diff_eq!(n.y, 0.0, epsilon = 1e-5);
        assert_abs_diff_eq!(n.z.abs(), 1.0, epsilon = 1e-5);
    }

    #[test]
    fn quadratic_patch_shape() {
        // 3x3 net with middle row raised to z=1 (bump)
        let cps = vec![
            Point3::new(0.0, 0.0, 0.0), Point3::new(0.0, 0.5, 0.0), Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.5, 0.0, 0.0), Point3::new(0.5, 0.5, 1.0), Point3::new(0.5, 1.0, 0.0),
            Point3::new(1.0, 0.0, 0.0), Point3::new(1.0, 0.5, 0.0), Point3::new(1.0, 1.0, 0.0),
        ];
        let s = BSplineSurface::clamped_uniform(2, 2, 3, 3, cps).unwrap();
        let center = s.eval(0.5, 0.5).unwrap();
        // Quadratic B-spline at interior evaluates to control point only when
        // control net isn't interpolated — center z should be between 0 and 1.
        assert!(center.z > 0.0 && center.z < 1.0);
    }
}
