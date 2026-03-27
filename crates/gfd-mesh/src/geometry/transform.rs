//! Geometric transformations for SDF shapes.

/// Translate an SDF by an offset.
pub fn sdf_translate<F: Fn([f64; 3]) -> f64>(sdf: F, offset: [f64; 3]) -> impl Fn([f64; 3]) -> f64 {
    move |p| sdf([p[0] - offset[0], p[1] - offset[1], p[2] - offset[2]])
}

/// Rotate an SDF around the Y axis by angle_rad.
pub fn sdf_rotate_y<F: Fn([f64; 3]) -> f64>(sdf: F, angle_rad: f64) -> impl Fn([f64; 3]) -> f64 {
    let c = angle_rad.cos();
    let s = angle_rad.sin();
    move |p| sdf([c * p[0] + s * p[2], p[1], -s * p[0] + c * p[2]])
}

/// Scale an SDF uniformly.
pub fn sdf_scale<F: Fn([f64; 3]) -> f64>(sdf: F, factor: f64) -> impl Fn([f64; 3]) -> f64 {
    move |p| sdf([p[0] / factor, p[1] / factor, p[2] / factor]) * factor
}

/// Mirror an SDF across a plane defined by normal (must be unit vector) and offset.
pub fn sdf_mirror<F: Fn([f64; 3]) -> f64>(sdf: F, normal: [f64; 3], offset: f64) -> impl Fn([f64; 3]) -> f64 {
    move |p| {
        let d = p[0] * normal[0] + p[1] * normal[1] + p[2] * normal[2] - offset;
        let reflected = [
            p[0] - 2.0 * d * normal[0],
            p[1] - 2.0 * d * normal[1],
            p[2] - 2.0 * d * normal[2],
        ];
        sdf(p).min(sdf(reflected))
    }
}

/// Shell: make a solid hollow with given wall thickness.
pub fn sdf_shell<F: Fn([f64; 3]) -> f64>(sdf: F, thickness: f64) -> impl Fn([f64; 3]) -> f64 {
    move |p| sdf(p).abs() - thickness * 0.5
}

/// Round (fillet) all edges of an SDF by subtracting a radius.
pub fn sdf_round<F: Fn([f64; 3]) -> f64>(sdf: F, radius: f64) -> impl Fn([f64; 3]) -> f64 {
    move |p| sdf(p) - radius
}

/// Linear pattern: repeat SDF n times along a direction with given spacing.
pub fn sdf_linear_pattern<F: Fn([f64; 3]) -> f64>(sdf: F, dir: [f64; 3], count: usize, spacing: f64) -> impl Fn([f64; 3]) -> f64 {
    move |p| {
        let mut min_d = f64::MAX;
        for i in 0..count {
            let offset = i as f64 * spacing;
            let q = [p[0] - dir[0] * offset, p[1] - dir[1] * offset, p[2] - dir[2] * offset];
            min_d = min_d.min(sdf(q));
        }
        min_d
    }
}

/// Circular pattern: repeat SDF around the Y axis.
pub fn sdf_circular_pattern<F: Fn([f64; 3]) -> f64>(sdf: F, count: usize) -> impl Fn([f64; 3]) -> f64 {
    move |p| {
        let mut min_d = f64::MAX;
        for i in 0..count {
            let angle = 2.0 * std::f64::consts::PI * i as f64 / count as f64;
            let c = angle.cos();
            let s = angle.sin();
            let q = [c * p[0] + s * p[2], p[1], -s * p[0] + c * p[2]];
            min_d = min_d.min(sdf(q));
        }
        min_d
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::primitives::*;

    #[test]
    fn test_translate() {
        let sphere = sdf_sphere([0.0, 0.0, 0.0], 1.0);
        let moved = sdf_translate(sphere, [5.0, 0.0, 0.0]);
        assert!(moved([5.0, 0.0, 0.0]) < 0.0); // inside at new center
        assert!(moved([0.0, 0.0, 0.0]) > 0.0); // outside at old center
    }

    #[test]
    fn test_scale() {
        let sphere = sdf_sphere([0.0, 0.0, 0.0], 1.0);
        let big = sdf_scale(sphere, 2.0);
        assert!(big([1.5, 0.0, 0.0]) < 0.0); // inside scaled sphere
    }

    #[test]
    fn test_shell() {
        let sphere = sdf_sphere([0.0, 0.0, 0.0], 1.0);
        let hollow = sdf_shell(sphere, 0.2);
        assert!(hollow([0.0, 0.0, 0.0]) > 0.0); // center is hollow
        assert!(hollow([0.9, 0.0, 0.0]) < 0.0); // wall region (within 0.1 of surface)
    }

    #[test]
    fn test_round() {
        let bx = sdf_box([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let rounded = sdf_round(bx, 0.1);
        // Rounded box should be slightly smaller (edges pushed inward)
        assert!(rounded([0.5, 0.5, 0.5]) < 0.0);
    }

    #[test]
    fn test_linear_pattern() {
        let sphere = sdf_sphere([0.0, 0.0, 0.0], 0.3);
        let row = sdf_linear_pattern(sphere, [1.0, 0.0, 0.0], 3, 1.0);
        assert!(row([0.0, 0.0, 0.0]) < 0.0); // first
        assert!(row([1.0, 0.0, 0.0]) < 0.0); // second
        assert!(row([2.0, 0.0, 0.0]) < 0.0); // third
        assert!(row([0.5, 0.0, 0.0]) > 0.0); // gap
    }

    #[test]
    fn test_circular_pattern() {
        let sphere = sdf_sphere([1.0, 0.0, 0.0], 0.2);
        let ring = sdf_circular_pattern(sphere, 4);
        assert!(ring([1.0, 0.0, 0.0]) < 0.0);
        assert!(ring([0.0, 0.0, 1.0]) < 0.0);
        assert!(ring([-1.0, 0.0, 0.0]) < 0.0);
        assert!(ring([0.0, 0.0, -1.0]) < 0.0);
    }
}
