//! Polyline curve — a 3D piecewise-linear curve, parameterised by cumulative
//! arc length. Useful as a sweep path (helix samples, imported DXF etc.)
//! and as a universal output shape for curve samplers.

use serde::{Deserialize, Serialize};

use crate::{Curve, GeomError, GeomResult, Point3, Vector3};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Polyline {
    pub points: Vec<Point3>,
    /// Cumulative arc length at each sample — `cum[0] = 0`,
    /// `cum[i] = cum[i-1] + |p_i - p_{i-1}|`.
    cum: Vec<f64>,
}

impl Polyline {
    pub fn new(points: Vec<Point3>) -> GeomResult<Self> {
        if points.len() < 2 {
            return Err(GeomError::Degenerate("polyline needs >= 2 points"));
        }
        let mut cum = Vec::with_capacity(points.len());
        cum.push(0.0);
        for i in 1..points.len() {
            let d = points[i - 1].distance(points[i]);
            cum.push(cum[i - 1] + d);
        }
        Ok(Self { points, cum })
    }

    /// Map `u` ∈ [0, L] to (segment_index, local_fraction ∈ [0, 1]).
    fn locate(&self, u: f64) -> (usize, f64) {
        let l = *self.cum.last().unwrap_or(&0.0);
        if l <= 0.0 { return (0, 0.0); }
        let uc = u.clamp(0.0, l);
        // Linear scan — polylines rarely exceed a few hundred segments.
        for i in 1..self.cum.len() {
            if uc <= self.cum[i] {
                let seg = self.cum[i] - self.cum[i - 1];
                if seg <= 0.0 { return (i - 1, 0.0); }
                return (i - 1, (uc - self.cum[i - 1]) / seg);
            }
        }
        (self.points.len() - 2, 1.0)
    }
}

impl Curve for Polyline {
    fn u_range(&self) -> (f64, f64) { (0.0, *self.cum.last().unwrap_or(&0.0)) }

    fn eval(&self, u: f64) -> GeomResult<Point3> {
        let (i, t) = self.locate(u);
        let a = self.points[i];
        let b = self.points[i + 1];
        Ok(Point3::new(
            a.x + t * (b.x - a.x),
            a.y + t * (b.y - a.y),
            a.z + t * (b.z - a.z),
        ))
    }

    fn tangent(&self, u: f64) -> GeomResult<Vector3> {
        let (i, _) = self.locate(u);
        let a = self.points[i];
        let b = self.points[i + 1];
        let v = Vector3::new(b.x - a.x, b.y - a.y, b.z - a.z);
        let l = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
        if l < 1e-20 {
            return Err(GeomError::Degenerate("polyline segment has zero length"));
        }
        Ok(Vector3::new(v.x / l, v.y / l, v.z / l))
    }

    fn length(&self) -> f64 { *self.cum.last().unwrap_or(&0.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polyline_length_and_eval() {
        let p = Polyline::new(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
        ]).unwrap();
        assert!((p.length() - 3.0).abs() < 1e-9);
        // Halfway along first segment → (0.5, 0, 0).
        let mid = p.eval(0.5).unwrap();
        assert!((mid.x - 0.5).abs() < 1e-9 && mid.y.abs() < 1e-9);
        // Midway along second segment (arc length 1 + 1 = 2) → (1, 1, 0).
        let q = p.eval(2.0).unwrap();
        assert!((q.x - 1.0).abs() < 1e-9 && (q.y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn polyline_tangent_unit_vector() {
        let p = Polyline::new(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(3.0, 4.0, 0.0),
        ]).unwrap();
        let t = p.tangent(0.0).unwrap();
        assert!((t.x - 0.6).abs() < 1e-9);
        assert!((t.y - 0.8).abs() < 1e-9);
    }
}
