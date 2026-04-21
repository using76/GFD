use nalgebra::Vector3 as NaVector3;
use serde::{Deserialize, Serialize};

use crate::{GeomError, GeomResult};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vector3 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };
    pub const X: Self = Self { x: 1.0, y: 0.0, z: 0.0 };
    pub const Y: Self = Self { x: 0.0, y: 1.0, z: 0.0 };
    pub const Z: Self = Self { x: 0.0, y: 0.0, z: 1.0 };

    #[inline]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn norm(self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    #[inline]
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    #[inline]
    pub fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    pub fn to_direction(self) -> GeomResult<Direction3> {
        let n = self.norm();
        if n < crate::LINEAR_TOL {
            return Err(GeomError::Degenerate("zero-length vector"));
        }
        Ok(Direction3 { x: self.x / n, y: self.y / n, z: self.z / n })
    }

    #[inline]
    pub fn to_na(self) -> NaVector3<f64> {
        NaVector3::new(self.x, self.y, self.z)
    }
}

/// Unit-length vector enforced by construction.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Direction3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Direction3 {
    pub const X: Self = Self { x: 1.0, y: 0.0, z: 0.0 };
    pub const Y: Self = Self { x: 0.0, y: 1.0, z: 0.0 };
    pub const Z: Self = Self { x: 0.0, y: 0.0, z: 1.0 };

    pub fn as_vec(self) -> Vector3 {
        Vector3 { x: self.x, y: self.y, z: self.z }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn cross_orthogonal() {
        let c = Vector3::X.cross(Vector3::Y);
        assert_abs_diff_eq!(c.x, Vector3::Z.x, epsilon = 1e-12);
        assert_abs_diff_eq!(c.y, Vector3::Z.y, epsilon = 1e-12);
        assert_abs_diff_eq!(c.z, Vector3::Z.z, epsilon = 1e-12);
    }

    #[test]
    fn normalize_zero_fails() {
        assert!(Vector3::ZERO.to_direction().is_err());
    }
}
