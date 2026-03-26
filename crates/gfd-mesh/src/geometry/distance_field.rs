//! Signed distance field computation from triangle meshes (e.g., STL surfaces).

/// A triangle defined by three vertices in 3D space.
#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    /// Vertices of the triangle.
    pub v0: [f64; 3],
    pub v1: [f64; 3],
    pub v2: [f64; 3],
}

impl Triangle {
    /// Creates a new triangle from three vertices.
    pub fn new(v0: [f64; 3], v1: [f64; 3], v2: [f64; 3]) -> Self {
        Self { v0, v1, v2 }
    }

    /// Returns the face normal (not necessarily unit length).
    pub fn normal(&self) -> [f64; 3] {
        let e1 = sub(self.v1, self.v0);
        let e2 = sub(self.v2, self.v0);
        cross(e1, e2)
    }

    /// Returns the unit normal of the triangle.
    pub fn unit_normal(&self) -> [f64; 3] {
        let n = self.normal();
        let len = dot(n, n).sqrt();
        if len < 1e-30 {
            return [0.0, 0.0, 0.0];
        }
        [n[0] / len, n[1] / len, n[2] / len]
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Computes the closest point on a triangle to a query point.
///
/// Returns `(closest_point, squared_distance)`.
fn closest_point_on_triangle(tri: &Triangle, p: [f64; 3]) -> ([f64; 3], f64) {
    let a = tri.v0;
    let b = tri.v1;
    let c = tri.v2;

    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);

    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        let d = sub(p, a);
        return (a, dot(d, d));
    }

    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        let d = sub(p, b);
        return (b, dot(d, d));
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        let closest = [a[0] + v * ab[0], a[1] + v * ab[1], a[2] + v * ab[2]];
        let d = sub(p, closest);
        return (closest, dot(d, d));
    }

    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        let d = sub(p, c);
        return (c, dot(d, d));
    }

    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        let closest = [a[0] + w * ac[0], a[1] + w * ac[1], a[2] + w * ac[2]];
        let d = sub(p, closest);
        return (closest, dot(d, d));
    }

    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let bc = sub(c, b);
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let closest = [b[0] + w * bc[0], b[1] + w * bc[1], b[2] + w * bc[2]];
        let d = sub(p, closest);
        return (closest, dot(d, d));
    }

    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    let closest = [
        a[0] + ab[0] * v + ac[0] * w,
        a[1] + ab[1] * v + ac[1] * w,
        a[2] + ab[2] * v + ac[2] * w,
    ];
    let d = sub(p, closest);
    (closest, dot(d, d))
}

/// Computes the signed distance from a query point to a triangle mesh (brute-force).
///
/// The sign is determined using the pseudo-normal method: the sign is negative
/// if the point is on the same side as the inward-facing normal of the closest triangle,
/// i.e., inside the surface.
///
/// # Arguments
/// * `triangles` - Slice of triangles defining a closed surface
/// * `query` - The 3D query point
///
/// # Returns
/// Signed distance: negative inside, positive outside.
pub fn sdf_from_triangles(triangles: &[Triangle], query: [f64; 3]) -> f64 {
    if triangles.is_empty() {
        return f64::MAX;
    }

    let mut min_dist_sq = f64::MAX;
    let mut closest_tri_idx = 0;
    let mut _closest_pt = [0.0f64; 3];

    for (i, tri) in triangles.iter().enumerate() {
        let (cp, dist_sq) = closest_point_on_triangle(tri, query);
        if dist_sq < min_dist_sq {
            min_dist_sq = dist_sq;
            closest_tri_idx = i;
            _closest_pt = cp;
        }
    }

    let dist = min_dist_sq.sqrt();

    // Determine sign using pseudo-normal: project (query - closest_point) onto triangle normal
    let tri = &triangles[closest_tri_idx];
    let n = tri.normal();
    let n_len = dot(n, n).sqrt();
    if n_len < 1e-30 {
        return dist;
    }
    let to_query = sub(query, _closest_pt);
    let sign_val = dot(to_query, n);

    if sign_val < 0.0 {
        -dist // Inside
    } else {
        dist // Outside
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_unit_cube_triangles() -> Vec<Triangle> {
        // A simple unit cube [0,1]^3 composed of 12 triangles (2 per face).
        // Normals point outward.
        let mut tris = Vec::new();

        // -Z face (z=0), normal [0,0,-1]
        tris.push(Triangle::new([0.0,0.0,0.0], [1.0,1.0,0.0], [1.0,0.0,0.0]));
        tris.push(Triangle::new([0.0,0.0,0.0], [0.0,1.0,0.0], [1.0,1.0,0.0]));
        // +Z face (z=1), normal [0,0,1]
        tris.push(Triangle::new([0.0,0.0,1.0], [1.0,0.0,1.0], [1.0,1.0,1.0]));
        tris.push(Triangle::new([0.0,0.0,1.0], [1.0,1.0,1.0], [0.0,1.0,1.0]));
        // -X face (x=0), normal [-1,0,0]
        tris.push(Triangle::new([0.0,0.0,0.0], [0.0,0.0,1.0], [0.0,1.0,1.0]));
        tris.push(Triangle::new([0.0,0.0,0.0], [0.0,1.0,1.0], [0.0,1.0,0.0]));
        // +X face (x=1), normal [1,0,0]
        tris.push(Triangle::new([1.0,0.0,0.0], [1.0,1.0,1.0], [1.0,0.0,1.0]));
        tris.push(Triangle::new([1.0,0.0,0.0], [1.0,1.0,0.0], [1.0,1.0,1.0]));
        // -Y face (y=0), normal [0,-1,0]
        tris.push(Triangle::new([0.0,0.0,0.0], [1.0,0.0,0.0], [1.0,0.0,1.0]));
        tris.push(Triangle::new([0.0,0.0,0.0], [1.0,0.0,1.0], [0.0,0.0,1.0]));
        // +Y face (y=1), normal [0,1,0]
        tris.push(Triangle::new([0.0,1.0,0.0], [0.0,1.0,1.0], [1.0,1.0,1.0]));
        tris.push(Triangle::new([0.0,1.0,0.0], [1.0,1.0,1.0], [1.0,1.0,0.0]));

        tris
    }

    #[test]
    fn test_sdf_inside_cube() {
        let tris = make_unit_cube_triangles();
        let d = sdf_from_triangles(&tris, [0.5, 0.5, 0.5]);
        // Center of unit cube: closest face at distance 0.5, inside => negative
        assert!(d < 0.0, "Center of cube should be inside (negative), got {d}");
        assert!((d.abs() - 0.5).abs() < 1e-10, "Distance should be 0.5, got {}", d.abs());
    }

    #[test]
    fn test_sdf_outside_cube() {
        let tris = make_unit_cube_triangles();
        let d = sdf_from_triangles(&tris, [2.0, 0.5, 0.5]);
        // Outside the cube, closest face at x=1 => distance 1.0
        assert!(d > 0.0, "Point outside cube should be positive, got {d}");
        assert!((d - 1.0).abs() < 1e-10, "Distance should be 1.0, got {d}");
    }

    #[test]
    fn test_sdf_on_surface() {
        let tris = make_unit_cube_triangles();
        let d = sdf_from_triangles(&tris, [1.0, 0.5, 0.5]);
        assert!(d.abs() < 1e-10, "On surface should be ~0, got {d}");
    }

    #[test]
    fn test_triangle_normal() {
        let tri = Triangle::new([0.0,0.0,0.0], [1.0,0.0,0.0], [0.0,1.0,0.0]);
        let n = tri.unit_normal();
        assert!((n[0]).abs() < 1e-12);
        assert!((n[1]).abs() < 1e-12);
        assert!((n[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_empty_triangles() {
        let d = sdf_from_triangles(&[], [0.0, 0.0, 0.0]);
        assert_eq!(d, f64::MAX);
    }
}
