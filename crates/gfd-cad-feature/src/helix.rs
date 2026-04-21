//! Helix curve sampler — classic right-handed spiral
//! `x = r cos(θ)`, `y = r sin(θ)`, `z = pitch · (θ / 2π)`.
//! Useful as a sweep path for threads, springs, twisted extrusions.

use gfd_cad_geom::Point3;

/// Samples a helix centered on the Z axis. Returns `turns ·
/// segments_per_turn + 1` points (the extra one closes the last segment).
/// `pitch` is the Z advance per full revolution. Right-handed (CCW
/// looking down −Z); flip `pitch` sign for left-handed.
pub fn helix_path(
    radius: f64,
    pitch: f64,
    turns: f64,
    segments_per_turn: usize,
) -> Vec<Point3> {
    if radius <= 0.0 || turns <= 0.0 || segments_per_turn == 0 {
        return Vec::new();
    }
    let total = (turns * segments_per_turn as f64).ceil() as usize;
    let mut out = Vec::with_capacity(total + 1);
    for i in 0..=total {
        let t = i as f64 / segments_per_turn as f64; // revolutions so far
        let theta = t * 2.0 * std::f64::consts::PI;
        let (s, c) = theta.sin_cos();
        out.push(Point3::new(radius * c, radius * s, pitch * t));
    }
    out
}

/// Archimedean spiral `r = a + b·θ` sampled in 2D (z = 0). `a` is the
/// starting radius, `b` the linear growth per radian, `turns` full
/// revolutions, `segments_per_turn` samples per revolution. Handy
/// sketcher input for volute springs, Tesla coils, spiral staircases, etc.
pub fn archimedean_spiral_path(
    a: f64,
    b: f64,
    turns: f64,
    segments_per_turn: usize,
) -> Vec<Point3> {
    if b == 0.0 || turns <= 0.0 || segments_per_turn == 0 {
        return Vec::new();
    }
    let total = (turns * segments_per_turn as f64).ceil() as usize;
    (0..=total).map(|i| {
        let t = i as f64 / segments_per_turn as f64;
        let theta = t * 2.0 * std::f64::consts::PI;
        let r = a + b * theta;
        Point3::new(r * theta.cos(), r * theta.sin(), 0.0)
    }).collect()
}

/// Torus-knot path — (p, q) knot winding around a torus with major
/// radius `major_r` and minor radius `minor_r`.
/// `x = (R + r cos(qφ)) cos(pφ), y = (R + r cos(qφ)) sin(pφ),
///  z = r sin(qφ)`. Good for decorative pipes and teaching materials.
pub fn torus_knot_path(
    p: u32,
    q: u32,
    major_r: f64,
    minor_r: f64,
    segments: usize,
) -> Vec<Point3> {
    if p == 0 || q == 0 || segments < 3 { return Vec::new(); }
    (0..=segments).map(|i| {
        let phi = (i as f64 / segments as f64) * 2.0 * std::f64::consts::PI;
        let qp = (q as f64) * phi;
        let pp = (p as f64) * phi;
        let (qs, qc) = qp.sin_cos();
        let (ps, pc) = pp.sin_cos();
        let ring = major_r + minor_r * qc;
        Point3::new(ring * pc, ring * ps, minor_r * qs)
    }).collect()
}

/// Arc length of a helix from θ=0 to θ=2π·turns:
/// `L = turns · 2π · √(r² + (pitch/2π)²)`.
pub fn helix_length(radius: f64, pitch: f64, turns: f64) -> f64 {
    let tau = 2.0 * std::f64::consts::PI;
    turns * tau * (radius * radius + (pitch / tau).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helix_single_turn_closes_in_xy() {
        let pts = helix_path(1.0, 2.0, 1.0, 8);
        // 8 segments + 1 closing point.
        assert_eq!(pts.len(), 9);
        // First point at (r, 0, 0), last at (r, 0, pitch).
        assert!((pts[0].x - 1.0).abs() < 1e-9);
        assert!( pts[0].y.abs()        < 1e-9);
        assert!( pts[0].z.abs()        < 1e-9);
        assert!((pts.last().unwrap().x - 1.0).abs() < 1e-9);
        assert!( pts.last().unwrap().y.abs()        < 1e-9);
        assert!((pts.last().unwrap().z - 2.0).abs() < 1e-9);
    }

    #[test]
    fn helix_length_matches_closed_form() {
        // r = 3, pitch = 4, 1 turn. L = 2π · √(9 + (4/2π)²) ≈ 2π · √(9.405) ≈ 19.276.
        let l = helix_length(3.0, 4.0, 1.0);
        let expected = 2.0 * std::f64::consts::PI * (9.0 + (4.0 / (2.0 * std::f64::consts::PI)).powi(2)).sqrt();
        assert!((l - expected).abs() < 1e-9);
    }

    #[test]
    fn helix_zero_turns_is_empty() {
        assert!(helix_path(1.0, 1.0, 0.0, 10).is_empty());
    }
}
