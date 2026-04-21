//! Parametric 2D profile generators. Output is a CCW polygon in the XY
//! plane, directly consumable by `pad_polygon_xy`, `pocket_polygon_xy`,
//! `revolve_profile_z`, and sketch-based features.

use std::f64::consts::TAU;

/// Regular N-gon inscribed in a circle of `radius`, centered at origin.
/// Starts at angle `start_deg` measured CCW from +X (default 0 = first
/// vertex on the +X axis). Returns `n` points in CCW order.
pub fn regular_ngon_profile(radius: f64, n: usize, start_deg: f64) -> Vec<(f64, f64)> {
    if n < 3 || radius <= 0.0 { return Vec::new(); }
    let theta0 = start_deg.to_radians();
    (0..n).map(|i| {
        let a = theta0 + (i as f64) * TAU / (n as f64);
        (radius * a.cos(), radius * a.sin())
    }).collect()
}

/// Star polygon with alternating `outer_r` and `inner_r` radii,
/// `points` tips, centered at origin. Returns `2·points` vertices CCW.
pub fn star_profile(outer_r: f64, inner_r: f64, points: usize) -> Vec<(f64, f64)> {
    if points < 3 || outer_r <= 0.0 || inner_r <= 0.0 { return Vec::new(); }
    let n = points * 2;
    (0..n).map(|i| {
        let a = (i as f64) * TAU / (n as f64);
        let r = if i % 2 == 0 { outer_r } else { inner_r };
        (r * a.cos(), r * a.sin())
    }).collect()
}

/// Rectangle profile centered at origin with full width `w` and height
/// `h`. CCW order: (-w/2, -h/2), (+w/2, -h/2), (+w/2, +h/2), (-w/2, +h/2).
pub fn rectangle_profile(w: f64, h: f64) -> Vec<(f64, f64)> {
    if w <= 0.0 || h <= 0.0 { return Vec::new(); }
    let wx = w * 0.5;
    let hy = h * 0.5;
    vec![(-wx, -hy), (wx, -hy), (wx, hy), (-wx, hy)]
}

/// Simple straight-sided gear profile (square teeth — not an involute).
/// `teeth` pairs of 4 vertices each form alternating tooth tops and
/// valleys. Tooth top at radius `tip_r`, valley at radius `root_r`.
/// Ratio of tooth angular width to pitch is `duty` (0 < duty < 1).
/// Returns a CCW polygon centered at origin. Good enough for
/// visualisation, Pad demos, or mesh CSG stress tests.
pub fn gear_profile_simple(
    tip_r: f64,
    root_r: f64,
    teeth: usize,
    duty: f64,
) -> Vec<(f64, f64)> {
    if teeth < 3 || tip_r <= root_r || root_r <= 0.0 || duty <= 0.0 || duty >= 1.0 {
        return Vec::new();
    }
    let step = TAU / (teeth as f64);
    let tooth_half = step * duty * 0.5;
    let mut out = Vec::with_capacity(teeth * 4);
    for i in 0..teeth {
        let c = (i as f64) * step;
        // Tooth leading (rising) edge: root → tip.
        let a0 = c - tooth_half;
        out.push((root_r * a0.cos(), root_r * a0.sin()));
        out.push((tip_r  * a0.cos(), tip_r  * a0.sin()));
        // Tooth trailing (falling) edge: tip → root.
        let a1 = c + tooth_half;
        out.push((tip_r  * a1.cos(), tip_r  * a1.sin()));
        out.push((root_r * a1.cos(), root_r * a1.sin()));
    }
    out
}

/// Slot profile — rectangle of length `length` and width `width` with
/// full semicircular caps on the short sides. `arc_segs` samples per cap.
/// Returns a CCW polygon centered at the origin.
pub fn slot_profile(length: f64, width: f64, arc_segs: usize) -> Vec<(f64, f64)> {
    if length <= width || width <= 0.0 || arc_segs == 0 { return Vec::new(); }
    let r = width * 0.5;
    let x = length * 0.5 - r; // center offset of the two arc centers
    let mut out = Vec::with_capacity(2 * arc_segs + 2);
    // Right cap: centered at (x, 0), sweeping −π/2 → π/2.
    for s in 0..=arc_segs {
        let a = -std::f64::consts::FRAC_PI_2
              + (s as f64 / arc_segs as f64) * std::f64::consts::PI;
        out.push((x + r * a.cos(), r * a.sin()));
    }
    // Left cap: centered at (-x, 0), sweeping π/2 → 3π/2.
    for s in 0..=arc_segs {
        let a = std::f64::consts::FRAC_PI_2
              + (s as f64 / arc_segs as f64) * std::f64::consts::PI;
        out.push((-x + r * a.cos(), r * a.sin()));
    }
    out
}

/// Ellipse profile sampled uniformly in angle with `segments` points,
/// centered at the origin with semi-axes (`a`, `b`). CCW order.
pub fn ellipse_profile(a: f64, b: f64, segments: usize) -> Vec<(f64, f64)> {
    if a <= 0.0 || b <= 0.0 || segments < 3 { return Vec::new(); }
    (0..segments).map(|i| {
        let t = (i as f64) * TAU / (segments as f64);
        (a * t.cos(), b * t.sin())
    }).collect()
}

/// Rounded-rectangle profile: a rectangle of `w × h` with corner fillets
/// of radius `r`, sampled with `corner_segs` segments per 90° arc.
pub fn rounded_rectangle_profile(w: f64, h: f64, r: f64, corner_segs: usize) -> Vec<(f64, f64)> {
    if w <= 2.0 * r || h <= 2.0 * r || r < 0.0 || corner_segs == 0 {
        return rectangle_profile(w, h);
    }
    let wx = w * 0.5;
    let hy = h * 0.5;
    let mut out = Vec::with_capacity(corner_segs * 4 + 8);
    // Arc centers.
    let centers = [
        ( wx - r, -hy + r, -std::f64::consts::FRAC_PI_2),     // bottom-right
        ( wx - r,  hy - r, 0.0),                              // top-right
        (-wx + r,  hy - r,  std::f64::consts::FRAC_PI_2),     // top-left
        (-wx + r, -hy + r,  std::f64::consts::PI),            // bottom-left
    ];
    for (cx, cy, a0) in centers {
        for s in 0..=corner_segs {
            let a = a0 + (s as f64 / corner_segs as f64) * std::f64::consts::FRAC_PI_2;
            out.push((cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    out
}

/// I-beam (H-section) cross-section — origin-centered. Outer bounds
/// `height × width`, flange thickness `flange_t`, web thickness `web_t`.
/// Returns CCW 12-vertex polygon.
pub fn i_beam_profile(height: f64, width: f64, flange_t: f64, web_t: f64) -> Vec<(f64, f64)> {
    if height <= 2.0 * flange_t || width <= web_t || flange_t <= 0.0 || web_t <= 0.0 {
        return Vec::new();
    }
    let hx = width * 0.5;
    let hy = height * 0.5;
    let wx = web_t * 0.5;
    let fy_lo = -hy + flange_t;
    let fy_hi =  hy - flange_t;
    vec![
        (-hx, -hy), ( hx, -hy), ( hx, fy_lo), ( wx, fy_lo),
        ( wx, fy_hi), ( hx, fy_hi), ( hx,  hy), (-hx,  hy),
        (-hx, fy_hi), (-wx, fy_hi), (-wx, fy_lo), (-hx, fy_lo),
    ]
}

/// L-angle cross-section, corner at origin, arms along +X and +Y.
/// `length` (horizontal arm), `height` (vertical arm), `thickness`
/// (both arm thicknesses). Returns 6-vertex CCW polygon.
pub fn l_angle_profile(length: f64, height: f64, thickness: f64) -> Vec<(f64, f64)> {
    if length <= thickness || height <= thickness || thickness <= 0.0 { return Vec::new(); }
    vec![
        (0.0, 0.0),
        (length, 0.0),
        (length, thickness),
        (thickness, thickness),
        (thickness, height),
        (0.0, height),
    ]
}

/// Z-section (Z-purlin) steel cross-section. Top flange extends to +X,
/// bottom flange extends to −X, connected by a vertical web of height
/// `section_h`. `flange_len` is the flange length (one side), `thickness`
/// is constant flange + web thickness. Returns 8-vertex CCW polygon.
pub fn z_section_profile(section_h: f64, flange_len: f64, thickness: f64) -> Vec<(f64, f64)> {
    if section_h <= 2.0 * thickness || flange_len <= thickness || thickness <= 0.0 {
        return Vec::new();
    }
    let hy = section_h * 0.5;
    let t = thickness;
    vec![
        (0.0,               -hy),
        ( flange_len,       -hy),
        ( flange_len,       -hy + t),
        ( t,                -hy + t),
        ( t,                 hy),
        (-flange_len,        hy),
        (-flange_len,        hy - t),
        (0.0,                hy - t),
    ]
}

/// T-beam cross-section. Web stands upright from y=0 to y=height−flange_t;
/// flange spans the full `width` at the top. Returns 8-vertex CCW polygon.
pub fn t_beam_profile(height: f64, width: f64, flange_t: f64, web_t: f64) -> Vec<(f64, f64)> {
    if height <= flange_t || width <= web_t || flange_t <= 0.0 || web_t <= 0.0 {
        return Vec::new();
    }
    let wx = web_t * 0.5;
    let fx = width * 0.5;
    let fy = height - flange_t;
    vec![
        (-wx, 0.0),
        ( wx, 0.0),
        ( wx, fy),
        ( fx, fy),
        ( fx, height),
        (-fx, height),
        (-fx, fy),
        (-wx, fy),
    ]
}

/// C-channel (U-section), origin-centered vertically, opening to +X.
/// `height × depth`, `flange_t` (top/bottom), `web_t` (back wall).
pub fn c_channel_profile(height: f64, depth: f64, flange_t: f64, web_t: f64) -> Vec<(f64, f64)> {
    if height <= 2.0 * flange_t || depth <= web_t || flange_t <= 0.0 || web_t <= 0.0 {
        return Vec::new();
    }
    let hy = height * 0.5;
    let fy_lo = -hy + flange_t;
    let fy_hi =  hy - flange_t;
    vec![
        (0.0, -hy),
        (depth, -hy),
        (depth, fy_lo),
        (web_t, fy_lo),
        (web_t, fy_hi),
        (depth, fy_hi),
        (depth,  hy),
        (0.0,   hy),
    ]
}

/// NACA 4-digit symmetric airfoil profile (NACA 00XX family). `thickness`
/// is the `XX` digits as a fraction (e.g. 0.12 for NACA 0012). `chord`
/// scales the resulting outline. `segments` samples per surface side;
/// returns a CCW closed polygon (upper then lower surface).
/// Formula (Jacobs, 1933):
///   y_t(x) = 5·t·(0.2969√x − 0.1260x − 0.3516x² + 0.2843x³ − 0.1036x⁴)
/// Last coefficient −0.1036 closes the trailing edge exactly.
pub fn airfoil_naca4_profile(thickness: f64, chord: f64, segments: usize) -> Vec<(f64, f64)> {
    if thickness <= 0.0 || chord <= 0.0 || segments < 4 { return Vec::new(); }
    let y_t = |x: f64| -> f64 {
        5.0 * thickness * (0.2969 * x.sqrt()
            - 0.1260 * x
            - 0.3516 * x * x
            + 0.2843 * x.powi(3)
            - 0.1036 * x.powi(4))
    };
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(segments * 2);
    // Upper surface: trailing edge (x=1) back to leading edge (x=0).
    for i in 0..=segments {
        let x = 1.0 - (i as f64 / segments as f64);
        pts.push((x * chord, y_t(x) * chord));
    }
    // Lower surface: leading edge (x=0) forward to trailing edge (x=1).
    for i in 1..segments {
        let x = i as f64 / segments as f64;
        pts.push((x * chord, -y_t(x) * chord));
    }
    pts
}

// ===== Convenience prism builders (profile → pad in one call) =====

use gfd_cad_topo::{ShapeArena, ShapeId, TopoResult};

/// Star-shaped prism: `star_profile(outer_r, inner_r, points)` padded by
/// `height` along +Z. Returns the resulting solid's shape id.
pub fn star_prism_solid(
    arena: &mut ShapeArena,
    outer_r: f64, inner_r: f64, points: usize, height: f64,
) -> TopoResult<ShapeId> {
    let pts = star_profile(outer_r, inner_r, points);
    if pts.is_empty() {
        return Err(gfd_cad_topo::TopoError::Geom(
            gfd_cad_geom::GeomError::Degenerate("star_prism: empty profile")));
    }
    crate::pad_polygon_xy(arena, &pts, height)
}

/// Gear-shaped prism: `gear_profile_simple(tip_r, root_r, teeth, duty)`
/// padded by `height`.
pub fn gear_prism_solid(
    arena: &mut ShapeArena,
    tip_r: f64, root_r: f64, teeth: usize, duty: f64, height: f64,
) -> TopoResult<ShapeId> {
    let pts = gear_profile_simple(tip_r, root_r, teeth, duty);
    if pts.is_empty() {
        return Err(gfd_cad_topo::TopoError::Geom(
            gfd_cad_geom::GeomError::Degenerate("gear_prism: empty profile")));
    }
    crate::pad_polygon_xy(arena, &pts, height)
}

/// Slot-shaped prism (elongated hole body): `slot_profile(length, width, arc_segs)`
/// padded by `height`.
pub fn slot_prism_solid(
    arena: &mut ShapeArena,
    length: f64, width: f64, arc_segs: usize, height: f64,
) -> TopoResult<ShapeId> {
    let pts = slot_profile(length, width, arc_segs);
    if pts.is_empty() {
        return Err(gfd_cad_topo::TopoError::Geom(
            gfd_cad_geom::GeomError::Degenerate("slot_prism: empty profile")));
    }
    crate::pad_polygon_xy(arena, &pts, height)
}

/// Hollow cylinder (tube / pipe). Revolves a `ring_revolve_profile`.
/// `angular_steps` controls the lateral segment count (default 24 in GUI).
pub fn tube_solid(
    arena: &mut ShapeArena,
    inner_r: f64, outer_r: f64, height: f64, angular_steps: usize,
) -> TopoResult<ShapeId> {
    let prof = ring_revolve_profile(inner_r, outer_r, height);
    if prof.is_empty() {
        return Err(gfd_cad_topo::TopoError::Geom(
            gfd_cad_geom::GeomError::Degenerate("tube: invalid radii")));
    }
    crate::revolve_profile_z(arena, &prof, angular_steps)
}

/// Solid disc / washer: revolve a frustum with `r1 == r2 == radius`.
/// Simply a thin cylinder, but handy as a named primitive for tests.
pub fn disc_solid(
    arena: &mut ShapeArena,
    radius: f64, thickness: f64, angular_steps: usize,
) -> TopoResult<ShapeId> {
    if radius <= 0.0 || thickness <= 0.0 {
        return Err(gfd_cad_topo::TopoError::Geom(
            gfd_cad_geom::GeomError::Degenerate("disc: radius / thickness must be positive")));
    }
    let prof = vec![(0.0, 0.0), (radius, 0.0), (radius, thickness), (0.0, thickness)];
    crate::revolve_profile_z(arena, &prof, angular_steps)
}

// ===== Revolve (r, z) profiles — consumed by revolve_profile_z =====

/// Ring (washer) revolve profile: solid annulus obtained by revolving a
/// rectangle in (r, z) from `inner_r` to `outer_r` over `thickness`.
pub fn ring_revolve_profile(inner_r: f64, outer_r: f64, thickness: f64) -> Vec<(f64, f64)> {
    if inner_r < 0.0 || outer_r <= inner_r || thickness <= 0.0 { return Vec::new(); }
    vec![
        (inner_r, 0.0),
        (outer_r, 0.0),
        (outer_r, thickness),
        (inner_r, thickness),
    ]
}

/// Cup/bowl revolve profile: cylindrical wall of `wall_thickness`, with a
/// solid bottom disc of `bottom_thickness`. `outer_r` is the outside wall
/// radius, `height` is the total cup height.
pub fn cup_revolve_profile(
    outer_r: f64,
    wall_thickness: f64,
    height: f64,
    bottom_thickness: f64,
) -> Vec<(f64, f64)> {
    let inner_r = outer_r - wall_thickness;
    if outer_r <= 0.0 || wall_thickness <= 0.0 || inner_r <= 0.0
        || height <= bottom_thickness || bottom_thickness <= 0.0 {
        return Vec::new();
    }
    vec![
        (0.0,       0.0),
        (outer_r,   0.0),
        (outer_r,   height),
        (inner_r,   height),
        (inner_r,   bottom_thickness),
        (0.0,       bottom_thickness),
    ]
}

/// Cone revolve profile (frustum). Revolving gives a truncated cone.
pub fn frustum_revolve_profile(r1: f64, r2: f64, height: f64) -> Vec<(f64, f64)> {
    if r1 < 0.0 || r2 < 0.0 || height <= 0.0 || (r1 == 0.0 && r2 == 0.0) { return Vec::new(); }
    vec![(0.0, 0.0), (r1, 0.0), (r2, height), (0.0, height)]
}

/// Capsule revolve profile: cylinder of `radius × cyl_length` with two
/// hemispherical caps of `radius` on top and bottom. Revolving yields a
/// pill-shaped solid. `arc_segs` samples per cap hemisphere (half-circle).
/// Returns a CCW (r, z) polygon.
pub fn capsule_revolve_profile(radius: f64, cyl_length: f64, arc_segs: usize) -> Vec<(f64, f64)> {
    if radius <= 0.0 || cyl_length <= 0.0 || arc_segs < 2 { return Vec::new(); }
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(arc_segs * 2 + 2);
    let z0 = -cyl_length * 0.5;
    let z1 =  cyl_length * 0.5;
    // Bottom cap: arc from (0, z0-r) to (radius, z0), sweeping −π/2 → 0.
    for i in 0..=arc_segs {
        let a = -std::f64::consts::FRAC_PI_2
              + (i as f64 / arc_segs as f64) * std::f64::consts::FRAC_PI_2;
        pts.push((radius * a.cos(), z0 + radius * a.sin()));
    }
    // Top cap: arc from (radius, z1) to (0, z1+r), sweeping 0 → π/2.
    for i in 0..=arc_segs {
        let a = (i as f64 / arc_segs as f64) * std::f64::consts::FRAC_PI_2;
        pts.push((radius * a.cos(), z1 + radius * a.sin()));
    }
    pts
}

/// Torus revolve profile: circle of `minor_r` centered at `(major_r, 0)`
/// on the (r, z) plane, sampled with `segments` points. Revolving yields
/// a torus.
pub fn torus_revolve_profile(major_r: f64, minor_r: f64, segments: usize) -> Vec<(f64, f64)> {
    if major_r <= minor_r || minor_r <= 0.0 || segments < 3 { return Vec::new(); }
    (0..segments).map(|i| {
        let t = (i as f64) * TAU / (segments as f64);
        (major_r + minor_r * t.cos(), minor_r * t.sin())
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ngon_hexagon_has_six_points_on_circle() {
        let pts = regular_ngon_profile(2.0, 6, 0.0);
        assert_eq!(pts.len(), 6);
        for (x, y) in &pts {
            assert!(((x*x + y*y).sqrt() - 2.0).abs() < 1e-9);
        }
    }

    #[test]
    fn ngon_too_few_sides_is_empty() {
        assert!(regular_ngon_profile(1.0, 2, 0.0).is_empty());
    }

    #[test]
    fn star_alternates_radii() {
        let pts = star_profile(3.0, 1.0, 5);
        assert_eq!(pts.len(), 10);
        let r0 = (pts[0].0.powi(2) + pts[0].1.powi(2)).sqrt();
        let r1 = (pts[1].0.powi(2) + pts[1].1.powi(2)).sqrt();
        assert!((r0 - 3.0).abs() < 1e-9);
        assert!((r1 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn rectangle_profile_four_corners() {
        let p = rectangle_profile(4.0, 2.0);
        assert_eq!(p.len(), 4);
        assert!((p[0].0 - -2.0).abs() < 1e-12 && (p[0].1 - -1.0).abs() < 1e-12);
        assert!((p[2].0 -  2.0).abs() < 1e-12 && (p[2].1 -  1.0).abs() < 1e-12);
    }

    #[test]
    fn gear_simple_has_four_verts_per_tooth() {
        let p = gear_profile_simple(5.0, 3.0, 12, 0.5);
        assert_eq!(p.len(), 12 * 4);
        // All points within [root_r, tip_r] radii band.
        for (x, y) in &p {
            let r = (x*x + y*y).sqrt();
            assert!(r >= 3.0 - 1e-9 && r <= 5.0 + 1e-9);
        }
    }

    #[test]
    fn gear_rejects_invalid_args() {
        assert!(gear_profile_simple(3.0, 5.0, 10, 0.5).is_empty()); // tip ≤ root
        assert!(gear_profile_simple(5.0, 3.0, 2, 0.5).is_empty());  // too few teeth
        assert!(gear_profile_simple(5.0, 3.0, 10, 0.0).is_empty()); // duty = 0
        assert!(gear_profile_simple(5.0, 3.0, 10, 1.0).is_empty()); // duty = 1
    }

    #[test]
    fn slot_has_correct_total_length() {
        // length=10, width=4 → bbox_x span = 10, bbox_y span = 4 (= width).
        let p = slot_profile(10.0, 4.0, 8);
        let xmin = p.iter().map(|q| q.0).fold(f64::INFINITY, f64::min);
        let xmax = p.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max);
        let ymin = p.iter().map(|q| q.1).fold(f64::INFINITY, f64::min);
        let ymax = p.iter().map(|q| q.1).fold(f64::NEG_INFINITY, f64::max);
        assert!((xmax - xmin - 10.0).abs() < 1e-9);
        assert!((ymax - ymin - 4.0).abs()  < 1e-9);
    }

    #[test]
    fn slot_rejects_invalid_args() {
        assert!(slot_profile(4.0, 4.0, 8).is_empty()); // length ≤ width
        assert!(slot_profile(5.0, 0.0, 8).is_empty());
    }

    #[test]
    fn ellipse_axis_lengths() {
        let p = ellipse_profile(3.0, 1.0, 64);
        assert_eq!(p.len(), 64);
        let xmax = p.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max);
        let ymax = p.iter().map(|q| q.1).fold(f64::NEG_INFINITY, f64::max);
        assert!((xmax - 3.0).abs() < 1e-6);
        assert!((ymax - 1.0).abs() < 1e-6);
    }

    #[test]
    fn rounded_rectangle_samples_four_arcs() {
        let p = rounded_rectangle_profile(6.0, 4.0, 1.0, 4);
        // 4 corners × (corner_segs+1) = 4 × 5 = 20 points.
        assert_eq!(p.len(), 20);
    }
}
