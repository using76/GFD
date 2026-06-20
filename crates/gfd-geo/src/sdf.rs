//! Signed-distance primitives for the flood 3D track (FLOOD_PLAN Phase 5) — used
//! to union terrain (DEM heightfield) with building prisms (IFC footprints
//! extruded over [base_z, base_z+height]) into one SDF closure for the cut-cell
//! mesher. Convention: **negative = inside the solid**, positive = fluid.

/// Distance from `p` to segment `a`–`b` (2D).
fn point_seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (abx, aby) = (b[0] - a[0], b[1] - a[1]);
    let (apx, apy) = (p[0] - a[0], p[1] - a[1]);
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 1e-30 { ((apx * abx + apy * aby) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (qx, qy) = (a[0] + t * abx, a[1] + t * aby);
    ((p[0] - qx).powi(2) + (p[1] - qy).powi(2)).sqrt()
}

/// Even-odd point-in-polygon test (2D).
fn inside_polygon(p: [f64; 2], poly: &[[f64; 2]]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut hit = false;
    let mut k = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = (poly[i][0], poly[i][1]);
        let (xk, yk) = (poly[k][0], poly[k][1]);
        if ((yi > p[1]) != (yk > p[1])) && (p[0] < (xk - xi) * (p[1] - yi) / (yk - yi) + xi) {
            hit = !hit;
        }
        k = i;
    }
    hit
}

/// Signed distance from a 2D point to a closed polygon boundary: negative inside,
/// positive outside, magnitude = distance to the nearest edge.
pub fn sdf_polygon_2d(p: [f64; 2], poly: &[[f64; 2]]) -> f64 {
    if poly.len() < 3 {
        return f64::INFINITY;
    }
    let mut d = f64::INFINITY;
    let mut k = poly.len() - 1;
    for i in 0..poly.len() {
        d = d.min(point_seg_dist(p, poly[k], poly[i]));
        k = i;
    }
    if inside_polygon(p, poly) {
        -d
    } else {
        d
    }
}

/// Signed distance to a vertical prism = footprint polygon × [base_z, base_z+height].
/// Negative only when the point is inside the footprint AND within the height slab.
pub fn sdf_building_prism(x: f64, y: f64, z: f64, footprint: &[[f64; 2]], base_z: f64, height: f64) -> f64 {
    let top = base_z + height.max(0.0);
    let d_xy = sdf_polygon_2d([x, y], footprint);
    let d_z = (base_z - z).max(z - top); // slab SDF: negative inside [base_z, top]
    // Intersection of two half/region SDFs = max (conservative outside,
    // exact for axis-aligned interiors).
    d_xy.max(d_z)
}

/// SDF of the union of building prisms at a point (negative inside any prism).
pub fn sdf_buildings(x: f64, y: f64, z: f64, prisms: &[(Vec<[f64; 2]>, f64, f64)]) -> f64 {
    let mut best = f64::INFINITY;
    for (fp, base_z, h) in prisms {
        best = best.min(sdf_building_prism(x, y, z, fp, *base_z, *h));
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<[f64; 2]> {
        vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]]
    }

    #[test]
    fn polygon_sdf_sign_and_distance() {
        let s = square();
        assert!(sdf_polygon_2d([2.0, 2.0], &s) < 0.0, "center is inside");
        assert!((sdf_polygon_2d([2.0, 2.0], &s) + 2.0).abs() < 1e-9, "center dist 2 to nearest edge");
        assert!(sdf_polygon_2d([6.0, 2.0], &s) > 0.0, "outside is positive");
        assert!((sdf_polygon_2d([6.0, 2.0], &s) - 2.0).abs() < 1e-9, "2 m outside right edge");
        assert!(sdf_polygon_2d([0.0, 2.0], &s).abs() < 1e-9, "on edge ~ 0");
    }

    #[test]
    fn prism_sdf_inside_and_outside() {
        let s = square();
        // inside footprint AND in slab [10, 13] → negative.
        assert!(sdf_building_prism(2.0, 2.0, 11.0, &s, 10.0, 3.0) < 0.0);
        // inside footprint but above the slab → positive (vertical distance).
        let above = sdf_building_prism(2.0, 2.0, 15.0, &s, 10.0, 3.0);
        assert!(above > 0.0 && (above - 2.0).abs() < 1e-9, "2 m above top, got {above}");
        // in slab but outside footprint → positive (in-plane distance).
        let beside = sdf_building_prism(6.0, 2.0, 11.0, &s, 10.0, 3.0);
        assert!(beside > 0.0 && (beside - 2.0).abs() < 1e-9, "2 m beside, got {beside}");
    }

    #[test]
    fn union_takes_nearest_solid() {
        let prisms = vec![
            (square(), 0.0, 5.0),
            (vec![[10.0, 0.0], [12.0, 0.0], [12.0, 2.0], [10.0, 2.0]], 0.0, 5.0),
        ];
        assert!(sdf_buildings(2.0, 2.0, 1.0, &prisms) < 0.0, "inside first prism");
        assert!(sdf_buildings(11.0, 1.0, 1.0, &prisms) < 0.0, "inside second prism");
        assert!(sdf_buildings(7.0, 1.0, 1.0, &prisms) > 0.0, "between prisms is fluid");
    }
}
