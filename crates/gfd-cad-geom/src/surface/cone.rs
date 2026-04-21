use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

use crate::{Direction3, GeomResult, Point3, Surface, Vector3};

/// Right circular cone (frustum). Radius linearly interpolates from `r1` (at
/// v=0) to `r2` (at v=height). Parameterization: u = angle[0,2π], v ∈ [0, h].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Cone {
    pub origin: Point3,
    pub axis: Direction3,
    pub x_ref: Direction3,
    pub r1: f64,
    pub r2: f64,
    pub height: f64,
}

impl Cone {
    pub fn new(origin: Point3, axis: Direction3, x_ref: Direction3, r1: f64, r2: f64, height: f64) -> Self {
        Self { origin, axis, x_ref, r1, r2, height }
    }

    fn y_ref(&self) -> Vector3 {
        self.axis.as_vec().cross(self.x_ref.as_vec())
    }
}

impl Surface for Cone {
    fn u_range(&self) -> (f64, f64) { (0.0, TAU) }
    fn v_range(&self) -> (f64, f64) { (0.0, self.height) }

    fn eval(&self, u: f64, v: f64) -> GeomResult<Point3> {
        let t = if self.height.abs() < f64::EPSILON { 0.0 } else { v / self.height };
        let r = self.r1 * (1.0 - t) + self.r2 * t;
        let (s, c) = u.sin_cos();
        let y = self.y_ref();
        Ok(Point3::new(
            self.origin.x + r * (c * self.x_ref.x + s * y.x) + v * self.axis.x,
            self.origin.y + r * (c * self.x_ref.y + s * y.y) + v * self.axis.y,
            self.origin.z + r * (c * self.x_ref.z + s * y.z) + v * self.axis.z,
        ))
    }

    fn normal(&self, u: f64, _v: f64) -> GeomResult<Vector3> {
        // Slant component blended with radial — coarse but sufficient for tessellation.
        let (s, c) = u.sin_cos();
        let y = self.y_ref();
        let radial = Vector3::new(
            c * self.x_ref.x + s * y.x,
            c * self.x_ref.y + s * y.y,
            c * self.x_ref.z + s * y.z,
        );
        let slope = (self.r1 - self.r2) / self.height.max(f64::EPSILON);
        let ax = self.axis.as_vec();
        let mut n = Vector3::new(
            radial.x + slope * ax.x,
            radial.y + slope * ax.y,
            radial.z + slope * ax.z,
        );
        let m = n.norm().max(f64::EPSILON);
        n.x /= m; n.y /= m; n.z /= m;
        Ok(n)
    }
}
