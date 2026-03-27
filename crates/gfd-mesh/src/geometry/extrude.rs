//! Extrude, revolve, sweep, and loft operations on 2D profiles.

/// Extrude a 2D profile along the Z axis.
pub fn sdf_extrude<F: Fn([f64; 2]) -> f64>(profile: F, height: f64) -> impl Fn([f64; 3]) -> f64 {
    move |p| {
        let d2 = profile([p[0], p[1]]);
        let dz = p[2].abs() - height * 0.5;
        let outside = (d2.max(0.0) * d2.max(0.0) + dz.max(0.0) * dz.max(0.0)).sqrt();
        let inside = d2.max(dz).min(0.0);
        outside + inside
    }
}

/// Revolve a 2D profile (defined in the XY plane, Y>0 side) around the Y axis.
pub fn sdf_revolve<F: Fn([f64; 2]) -> f64>(profile: F) -> impl Fn([f64; 3]) -> f64 {
    move |p| {
        let r = (p[0] * p[0] + p[2] * p[2]).sqrt();
        profile([r, p[1]])
    }
}

/// Sweep a 2D profile along a straight line from start to end.
pub fn sdf_sweep<F: Fn([f64; 2]) -> f64>(
    profile: F,
    start: [f64; 3],
    end: [f64; 3],
) -> impl Fn([f64; 3]) -> f64 {
    let dir = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
    let length = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let n = if length > 1e-30 {
        [dir[0] / length, dir[1] / length, dir[2] / length]
    } else {
        [0.0, 0.0, 1.0]
    };

    move |p| {
        let rel = [p[0] - start[0], p[1] - start[1], p[2] - start[2]];
        let t = rel[0] * n[0] + rel[1] * n[1] + rel[2] * n[2];
        // Local coordinates perpendicular to sweep direction
        let proj = [rel[0] - t * n[0], rel[1] - t * n[1], rel[2] - t * n[2]];
        let r = (proj[0] * proj[0] + proj[1] * proj[1] + proj[2] * proj[2]).sqrt();
        let d2 = profile([r, 0.0]);
        let dt = (t.max(0.0) - t.min(length)).abs();
        let dz = if t >= 0.0 && t <= length { 0.0_f64 } else { dt };
        (d2.max(0.0).powi(2) + dz.max(0.0).powi(2)).sqrt() + d2.max(dz).min(0.0)
    }
}

/// Loft between two circular profiles (linearly interpolate radius).
pub fn sdf_loft_circles(r_bottom: f64, r_top: f64, height: f64) -> impl Fn([f64; 3]) -> f64 {
    move |p| {
        let t = ((p[1] / height) + 0.5).clamp(0.0, 1.0);
        let r = r_bottom * (1.0 - t) + r_top * t;
        let rxy = (p[0] * p[0] + p[2] * p[2]).sqrt();
        let d_radial = rxy - r;
        let d_axial = p[1].abs() - height * 0.5;
        let outside = (d_radial.max(0.0).powi(2) + d_axial.max(0.0).powi(2)).sqrt();
        let inside = d_radial.max(d_axial).min(0.0);
        outside + inside
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::sketch::*;

    #[test]
    fn test_extrude_circle() {
        let disk = sketch_circle([0.0, 0.0], 1.0);
        let cyl = sdf_extrude(disk, 2.0);
        assert!(cyl([0.0, 0.0, 0.0]) < 0.0);    // center
        assert!(cyl([0.0, 0.0, 1.5]) > 0.0);     // above
        assert!(cyl([2.0, 0.0, 0.0]) > 0.0);     // outside
    }

    #[test]
    fn test_extrude_rect() {
        let rect = sketch_rect([-1.0, -0.5], [1.0, 0.5]);
        let bar = sdf_extrude(rect, 3.0);
        assert!(bar([0.0, 0.0, 0.0]) < 0.0);
        assert!(bar([0.0, 0.0, 2.0]) > 0.0);
    }

    #[test]
    fn test_revolve_creates_torus_like() {
        // Profile: circle at (2, 0) with radius 0.5
        let profile = sketch_circle([2.0, 0.0], 0.5);
        let solid = sdf_revolve(profile);
        assert!(solid([2.0, 0.0, 0.0]) < 0.0);
        assert!(solid([0.0, 0.0, 2.0]) < 0.0);
        assert!(solid([0.0, 0.0, 0.0]) > 0.0); // center hole
    }

    #[test]
    fn test_loft_cone() {
        let cone = sdf_loft_circles(1.0, 0.0, 2.0);
        assert!(cone([0.0, -0.9, 0.0]) < 0.0); // near bottom, small radius
        assert!(cone([0.0, 0.9, 0.0]) < 0.0);  // near top
    }

    #[test]
    fn test_loft_cylinder() {
        let cyl = sdf_loft_circles(1.0, 1.0, 2.0);
        assert!(cyl([0.0, 0.0, 0.0]) < 0.0);
        assert!(cyl([1.5, 0.0, 0.0]) > 0.0);
    }
}
