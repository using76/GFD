//! Mesh-based approximate Boolean on triangle buffers.
//!
//! This is **not** a B-Rep CSG — it classifies triangles of A as inside or
//! outside B (via Möller-Trumbore ray-casting from each triangle's centroid)
//! and then assembles the output per operation:
//!
//! - **Union**      = `keep(A outside B) ∪ keep(B outside A)`
//! - **Difference** = `keep(A outside B) ∪ flip(B inside A)`
//! - **Intersection** = `keep(A inside B) ∪ keep(B inside A)`
//!
//! Centroid-only classification introduces aliasing on boundaries (a
//! triangle straddling the other shape can be kept or dropped based on its
//! centroid). Good enough for GUI previews; a face-splitting CSG ships
//! later.

use gfd_cad_tessel::TriMesh;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshOp {
    Union,
    Difference,
    Intersection,
}

pub fn mesh_boolean(a: &TriMesh, b: &TriMesh, op: MeshOp) -> TriMesh {
    let a_classes = classify_triangles(a, b);
    let b_classes = classify_triangles(b, a);
    let mut out = TriMesh::default();
    match op {
        MeshOp::Union => {
            append_filtered(&mut out, a, &a_classes, /*want_inside=*/false, /*flip=*/false);
            append_filtered(&mut out, b, &b_classes, false, false);
        }
        MeshOp::Difference => {
            append_filtered(&mut out, a, &a_classes, /*want_inside=*/false, /*flip=*/false);
            append_filtered(&mut out, b, &b_classes, /*want_inside=*/true, /*flip=*/true);
        }
        MeshOp::Intersection => {
            append_filtered(&mut out, a, &a_classes, /*want_inside=*/true, false);
            append_filtered(&mut out, b, &b_classes, /*want_inside=*/true, false);
        }
    }
    out
}

/// For each triangle of `a`, return true if its centroid is inside `b`.
fn classify_triangles(a: &TriMesh, b: &TriMesh) -> Vec<bool> {
    let n = a.indices.len() / 3;
    let mut out = Vec::with_capacity(n);
    for t in 0..n {
        let i0 = a.indices[t * 3] as usize;
        let i1 = a.indices[t * 3 + 1] as usize;
        let i2 = a.indices[t * 3 + 2] as usize;
        let p0 = a.positions[i0];
        let p1 = a.positions[i1];
        let p2 = a.positions[i2];
        let centroid = [
            (p0[0] + p1[0] + p2[0]) / 3.0,
            (p0[1] + p1[1] + p2[1]) / 3.0,
            (p0[2] + p1[2] + p2[2]) / 3.0,
        ];
        out.push(point_inside_mesh(centroid, b));
    }
    out
}

fn append_filtered(out: &mut TriMesh, src: &TriMesh, classes: &[bool], want_inside: bool, flip: bool) {
    let offset = out.positions.len() as u32;
    out.positions.extend_from_slice(&src.positions);
    out.normals.extend_from_slice(&src.normals);
    for (t, &inside) in classes.iter().enumerate() {
        if inside != want_inside { continue; }
        let i0 = src.indices[t * 3] + offset;
        let i1 = src.indices[t * 3 + 1] + offset;
        let i2 = src.indices[t * 3 + 2] + offset;
        if flip {
            out.indices.extend_from_slice(&[i0, i2, i1]);
        } else {
            out.indices.extend_from_slice(&[i0, i1, i2]);
        }
    }
}

/// Ray-cast from `p` along a perturbed +X direction and count crossings with
/// `mesh` triangles. Odd count ⇒ inside. The tiny y/z perturbation avoids
/// the degenerate case where the ray grazes a shared triangle edge.
pub fn point_inside_mesh(p: [f32; 3], mesh: &TriMesh) -> bool {
    let origin = [p[0] as f64, p[1] as f64, p[2] as f64];
    let dir = [1.0, 0.7131234e-5, 0.3917891e-5];
    let mut count = 0usize;
    let n = mesh.indices.len() / 3;
    for t in 0..n {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;
        let v0 = mesh.positions[i0];
        let v1 = mesh.positions[i1];
        let v2 = mesh.positions[i2];
        if moller_trumbore(origin, dir, v0, v1, v2) { count += 1; }
    }
    count % 2 == 1
}

fn moller_trumbore(origin: [f64; 3], dir: [f64; 3], v0: [f32; 3], v1: [f32; 3], v2: [f32; 3]) -> bool {
    let v0 = [v0[0] as f64, v0[1] as f64, v0[2] as f64];
    let v1 = [v1[0] as f64, v1[1] as f64, v1[2] as f64];
    let v2 = [v2[0] as f64, v2[1] as f64, v2[2] as f64];
    let eps = 1.0e-10;
    let edge1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
    let edge2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
    let h = cross(dir, edge2);
    let a = dot(edge1, h);
    if a.abs() < eps { return false; }
    let f = 1.0 / a;
    let s = [origin[0] - v0[0], origin[1] - v0[1], origin[2] - v0[2]];
    let u = f * dot(s, h);
    if u < 0.0 || u > 1.0 { return false; }
    let q = cross(s, edge1);
    let v = f * dot(dir, q);
    if v < 0.0 || u + v > 1.0 { return false; }
    let t = f * dot(edge2, q);
    t > eps
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1]*b[2] - a[2]*b[1], a[2]*b[0] - a[0]*b[2], a[0]*b[1] - a[1]*b[0]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 { a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_cube_mesh() -> TriMesh {
        // Axis-aligned unit cube centered at origin, 12 triangles.
        let v = [
            [-0.5f32, -0.5, -0.5], [ 0.5, -0.5, -0.5], [ 0.5,  0.5, -0.5], [-0.5,  0.5, -0.5],
            [-0.5, -0.5,  0.5], [ 0.5, -0.5,  0.5], [ 0.5,  0.5,  0.5], [-0.5,  0.5,  0.5],
        ];
        let idx: Vec<u32> = vec![
            0,1,2, 0,2,3,    // -Z
            4,6,5, 4,7,6,    // +Z
            0,4,5, 0,5,1,    // -Y
            3,2,6, 3,6,7,    // +Y
            0,3,7, 0,7,4,    // -X
            1,5,6, 1,6,2,    // +X
        ];
        TriMesh {
            positions: v.to_vec(),
            normals: vec![[0.0, 0.0, 1.0]; 8],
            indices: idx,
        }
    }

    #[test]
    fn centroid_inside_origin_of_unit_cube() {
        let cube = unit_cube_mesh();
        assert!(point_inside_mesh([0.0, 0.0, 0.0], &cube));
    }

    #[test]
    fn point_outside_unit_cube() {
        let cube = unit_cube_mesh();
        assert!(!point_inside_mesh([5.0, 0.0, 0.0], &cube));
    }

    #[test]
    fn intersection_of_overlapping_cubes_nonempty() {
        // Cube A at origin, cube B offset by (0.3, 0, 0) — they overlap along X.
        let a = unit_cube_mesh();
        let mut b = unit_cube_mesh();
        for p in &mut b.positions { p[0] += 0.3; }
        let inter = mesh_boolean(&a, &b, MeshOp::Intersection);
        assert!(!inter.indices.is_empty());
    }

    #[test]
    fn difference_of_fully_disjoint_cubes_equals_a() {
        let a = unit_cube_mesh();
        let mut b = unit_cube_mesh();
        for p in &mut b.positions { p[0] += 5.0; } // far away
        let diff = mesh_boolean(&a, &b, MeshOp::Difference);
        // All 12 triangles of A should survive (none inside B).
        assert_eq!(diff.indices.len() / 3, 12);
    }
}
