//! Basic shape primitives defined as signed distance functions (SDFs).
//!
//! Negative values indicate the interior of the shape, positive values the exterior,
//! and zero represents the surface.

/// Returns an SDF for a sphere with the given center and radius.
pub fn sdf_sphere(center: [f64; 3], radius: f64) -> impl Fn([f64; 3]) -> f64 {
    move |p: [f64; 3]| {
        let dx = p[0] - center[0];
        let dy = p[1] - center[1];
        let dz = p[2] - center[2];
        (dx * dx + dy * dy + dz * dz).sqrt() - radius
    }
}

/// Returns an SDF for an axis-aligned box defined by its min and max corners.
pub fn sdf_box(min: [f64; 3], max: [f64; 3]) -> impl Fn([f64; 3]) -> f64 {
    move |p: [f64; 3]| {
        // For each axis, compute the distance outside the box (clamped to 0 if inside).
        let half = [
            (max[0] - min[0]) * 0.5,
            (max[1] - min[1]) * 0.5,
            (max[2] - min[2]) * 0.5,
        ];
        let center = [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ];
        let qx = (p[0] - center[0]).abs() - half[0];
        let qy = (p[1] - center[1]).abs() - half[1];
        let qz = (p[2] - center[2]).abs() - half[2];

        let outside_len = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2) + qz.max(0.0).powi(2)).sqrt();
        let inside_dist = qx.max(qy).max(qz).min(0.0);
        outside_len + inside_dist
    }
}

/// Returns an SDF for a cylinder with the given center, axis direction, radius, and height.
///
/// The cylinder extends from `center - axis * height/2` to `center + axis * height/2`.
pub fn sdf_cylinder(
    center: [f64; 3],
    axis: [f64; 3],
    radius: f64,
    height: f64,
) -> impl Fn([f64; 3]) -> f64 {
    // Normalize the axis
    let axis_len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let ax = [axis[0] / axis_len, axis[1] / axis_len, axis[2] / axis_len];

    move |p: [f64; 3]| {
        let dp = [p[0] - center[0], p[1] - center[1], p[2] - center[2]];
        // Project onto axis
        let along = dp[0] * ax[0] + dp[1] * ax[1] + dp[2] * ax[2];
        // Radial component
        let rx = dp[0] - along * ax[0];
        let ry = dp[1] - along * ax[1];
        let rz = dp[2] - along * ax[2];
        let radial_dist = (rx * rx + ry * ry + rz * rz).sqrt();

        // 2D SDF of a rectangle in (radial, axial) space
        let half_h = height * 0.5;
        let dr = radial_dist - radius;
        let da = along.abs() - half_h;

        let outside_len = (dr.max(0.0).powi(2) + da.max(0.0).powi(2)).sqrt();
        let inside_dist = dr.max(da).min(0.0);
        outside_len + inside_dist
    }
}

/// Returns an SDF representing the union of two shapes: `min(a, b)`.
pub fn sdf_union<A, B>(a: A, b: B) -> impl Fn([f64; 3]) -> f64
where
    A: Fn([f64; 3]) -> f64,
    B: Fn([f64; 3]) -> f64,
{
    move |p: [f64; 3]| a(p).min(b(p))
}

/// Returns an SDF representing shape `a` with shape `b` subtracted: `max(a, -b)`.
pub fn sdf_subtract<A, B>(a: A, b: B) -> impl Fn([f64; 3]) -> f64
where
    A: Fn([f64; 3]) -> f64,
    B: Fn([f64; 3]) -> f64,
{
    move |p: [f64; 3]| a(p).max(-b(p))
}

/// Returns an SDF representing the intersection of two shapes: `max(a, b)`.
pub fn sdf_intersection<A, B>(a: A, b: B) -> impl Fn([f64; 3]) -> f64
where
    A: Fn([f64; 3]) -> f64,
    B: Fn([f64; 3]) -> f64,
{
    move |p: [f64; 3]| a(p).max(b(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdf_sphere() {
        let sdf = sdf_sphere([0.0, 0.0, 0.0], 1.0);
        // Center: distance = -1.0
        assert!((sdf([0.0, 0.0, 0.0]) - (-1.0)).abs() < 1e-12);
        // On surface
        assert!((sdf([1.0, 0.0, 0.0])).abs() < 1e-12);
        // Outside
        assert!((sdf([2.0, 0.0, 0.0]) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_sdf_box() {
        let sdf = sdf_box([0.0, 0.0, 0.0], [2.0, 2.0, 2.0]);
        // Center: inside
        assert!(sdf([1.0, 1.0, 1.0]) < 0.0);
        // On face center
        assert!((sdf([2.0, 1.0, 1.0])).abs() < 1e-12);
        // Outside
        assert!(sdf([3.0, 1.0, 1.0]) > 0.0);
        assert!((sdf([3.0, 1.0, 1.0]) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_sdf_cylinder() {
        let sdf = sdf_cylinder([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1.0, 2.0);
        // Center
        assert!(sdf([0.0, 0.0, 0.0]) < 0.0);
        // On the radial surface at mid-height
        assert!((sdf([1.0, 0.0, 0.0])).abs() < 1e-12);
        // On the top cap center
        assert!((sdf([0.0, 0.0, 1.0])).abs() < 1e-12);
        // Outside radially
        assert!(sdf([2.0, 0.0, 0.0]) > 0.0);
    }

    #[test]
    fn test_sdf_union() {
        let s1 = sdf_sphere([0.0, 0.0, 0.0], 1.0);
        let s2 = sdf_sphere([1.5, 0.0, 0.0], 1.0);
        let u = sdf_union(s1, s2);
        // At origin, inside s1
        assert!(u([0.0, 0.0, 0.0]) < 0.0);
        // At [1.5, 0, 0], inside s2
        assert!(u([1.5, 0.0, 0.0]) < 0.0);
        // Far away, outside both
        assert!(u([5.0, 0.0, 0.0]) > 0.0);
    }

    #[test]
    fn test_sdf_subtract() {
        let big = sdf_sphere([0.0, 0.0, 0.0], 2.0);
        let small = sdf_sphere([0.0, 0.0, 0.0], 1.0);
        let sub = sdf_subtract(big, small);
        // Center: inside small, so subtracted => outside
        assert!(sub([0.0, 0.0, 0.0]) > 0.0);
        // Between radii: inside big but outside small => inside result
        assert!(sub([1.5, 0.0, 0.0]) < 0.0);
    }
}
