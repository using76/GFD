use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

use crate::{Direction3, GeomResult, Point3, Surface, Vector3};

/// Finite cylinder: axis from origin in `axis` direction, radius `radius`,
/// parameterized as (u = angle[0,2π], v = axial height[0, height]).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Cylinder {
    pub origin: Point3,
    pub axis: Direction3,
    pub x_ref: Direction3,
    pub radius: f64,
    pub height: f64,
}

impl Cylinder {
    pub fn new(origin: Point3, axis: Direction3, x_ref: Direction3, radius: f64, height: f64) -> Self {
        Self { origin, axis, x_ref, radius, height }
    }

    fn y_ref(&self) -> Vector3 {
        self.axis.as_vec().cross(self.x_ref.as_vec())
    }
}

impl Surface for Cylinder {
    fn u_range(&self) -> (f64, f64) { (0.0, TAU) }
    fn v_range(&self) -> (f64, f64) { (0.0, self.height) }

    fn eval(&self, u: f64, v: f64) -> GeomResult<Point3> {
        let (s, c) = u.sin_cos();
        let y = self.y_ref();
        Ok(Point3::new(
            self.origin.x + self.radius * (c * self.x_ref.x + s * y.x) + v * self.axis.x,
            self.origin.y + self.radius * (c * self.x_ref.y + s * y.y) + v * self.axis.y,
            self.origin.z + self.radius * (c * self.x_ref.z + s * y.z) + v * self.axis.z,
        ))
    }

    fn normal(&self, u: f64, _v: f64) -> GeomResult<Vector3> {
        let (s, c) = u.sin_cos();
        let y = self.y_ref();
        Ok(Vector3::new(
            c * self.x_ref.x + s * y.x,
            c * self.x_ref.y + s * y.y,
            c * self.x_ref.z + s * y.z,
        ))
    }
}
