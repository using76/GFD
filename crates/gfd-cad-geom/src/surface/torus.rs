use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

use crate::{Direction3, GeomResult, Point3, Surface, Vector3};

/// Torus centered at `origin` with axis `axis`, major radius R and minor radius r.
/// u = longitude [0,2π], v = latitude [0,2π].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Torus {
    pub origin: Point3,
    pub axis: Direction3,
    pub x_ref: Direction3,
    pub major: f64,
    pub minor: f64,
}

impl Torus {
    pub fn new(origin: Point3, axis: Direction3, x_ref: Direction3, major: f64, minor: f64) -> Self {
        Self { origin, axis, x_ref, major, minor }
    }

    fn y_ref(&self) -> Vector3 {
        self.axis.as_vec().cross(self.x_ref.as_vec())
    }
}

impl Surface for Torus {
    fn u_range(&self) -> (f64, f64) { (0.0, TAU) }
    fn v_range(&self) -> (f64, f64) { (0.0, TAU) }

    fn eval(&self, u: f64, v: f64) -> GeomResult<Point3> {
        let (su, cu) = u.sin_cos();
        let (sv, cv) = v.sin_cos();
        let y = self.y_ref();
        let ax = self.axis.as_vec();
        let ring = self.major + self.minor * cv;
        Ok(Point3::new(
            self.origin.x + ring * (cu * self.x_ref.x + su * y.x) + self.minor * sv * ax.x,
            self.origin.y + ring * (cu * self.x_ref.y + su * y.y) + self.minor * sv * ax.y,
            self.origin.z + ring * (cu * self.x_ref.z + su * y.z) + self.minor * sv * ax.z,
        ))
    }

    fn normal(&self, u: f64, v: f64) -> GeomResult<Vector3> {
        let (su, cu) = u.sin_cos();
        let (sv, cv) = v.sin_cos();
        let y = self.y_ref();
        let ax = self.axis.as_vec();
        Ok(Vector3::new(
            cv * (cu * self.x_ref.x + su * y.x) + sv * ax.x,
            cv * (cu * self.x_ref.y + su * y.y) + sv * ax.y,
            cv * (cu * self.x_ref.z + su * y.z) + sv * ax.z,
        ))
    }
}
