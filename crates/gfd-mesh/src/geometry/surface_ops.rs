//! Surface operations on triangle meshes.
//!
//! Provides offset, stitching, capping, boundary extraction, plane splitting,
//! and surface area computation for triangle meshes.

use crate::geometry::distance_field::Triangle;
use std::collections::HashMap;

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[0]+b[0], a[1]+b[1], a[2]+b[2]] }
fn scale(a: [f64; 3], s: f64) -> [f64; 3] { [a[0]*s, a[1]*s, a[2]*s] }
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 { a[0]*b[0]+a[1]*b[1]+a[2]*b[2] }
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
}
fn length(a: [f64; 3]) -> f64 { dot(a, a).sqrt() }
fn normalize(a: [f64; 3]) -> [f64; 3] {
    let l = length(a); if l < 1e-30 { [0.0,0.0,0.0] } else { scale(a, 1.0/l) }
}

fn vertex_key(v: [f64; 3]) -> [i64; 3] {
    [(v[0]*1e10).round() as i64, (v[1]*1e10).round() as i64, (v[2]*1e10).round() as i64]
}

fn edge_key(a: [f64; 3], b: [f64; 3]) -> ([i64; 3], [i64; 3]) {
    let ka = vertex_key(a); let kb = vertex_key(b);
    if ka < kb { (ka, kb) } else { (kb, ka) }
}

fn triangle_area(tri: &Triangle) -> f64 {
    0.5 * length(cross(sub(tri.v1, tri.v0), sub(tri.v2, tri.v0)))
}

/// Offset all triangles along their unit normals by a given distance.
pub fn offset_surface(triangles: &[Triangle], distance: f64) -> Vec<Triangle> {
    if triangles.is_empty() { return Vec::new(); }
    let mut vnormals: HashMap<[i64;3],[f64;3]> = HashMap::new();
    let mut vcounts: HashMap<[i64;3],usize> = HashMap::new();
    for tri in triangles {
        let n = tri.unit_normal();
        for v in &[tri.v0, tri.v1, tri.v2] {
            let k = vertex_key(*v);
            let e = vnormals.entry(k).or_insert([0.0;3]);
            *e = add(*e, n);
            *vcounts.entry(k).or_insert(0) += 1;
        }
    }
    let mut voff: HashMap<[i64;3],[f64;3]> = HashMap::new();
    for (k, ns) in &vnormals {
        let cnt = vcounts[k] as f64;
        let avg = normalize(scale(*ns, 1.0/cnt));
        voff.insert(*k, scale(avg, distance));
    }
    triangles.iter().map(|tri| {
        let o0 = voff[&vertex_key(tri.v0)];
        let o1 = voff[&vertex_key(tri.v1)];
        let o2 = voff[&vertex_key(tri.v2)];
        Triangle::new(add(tri.v0,o0), add(tri.v1,o1), add(tri.v2,o2))
    }).collect()
}

/// Stitch two sets of triangles by matching boundary edges within tolerance.
pub fn stitch_surfaces(a: &[Triangle], b: &[Triangle], tolerance: f64) -> Vec<Triangle> {
    let mut result: Vec<Triangle> = Vec::new();
    result.extend_from_slice(a);
    result.extend_from_slice(b);
    let ba = extract_boundary_edges(a);
    let bb = extract_boundary_edges(b);
    let tol_sq = tolerance * tolerance;
    let mut used_b = vec![false; bb.len()];
    for (ea0, ea1) in &ba {
        let mid_a = scale(add(*ea0, *ea1), 0.5);
        let mut best_idx = None;
        let mut best_d = f64::MAX;
        for (j, (eb0, eb1)) in bb.iter().enumerate() {
            if used_b[j] { continue; }
            let mid_b = scale(add(*eb0, *eb1), 0.5);
            let d = dot(sub(mid_a, mid_b), sub(mid_a, mid_b));
            if d < best_d && d < tol_sq { best_d = d; best_idx = Some(j); }
        }
        if let Some(j) = best_idx {
            used_b[j] = true;
            let (eb0, eb1) = bb[j];
            result.push(Triangle::new(*ea0, *ea1, eb0));
            result.push(Triangle::new(*ea1, eb1, eb0));
        }
    }
    result
}

/// Cap an opening by fan-triangulating from the centroid.
pub fn cap_opening(boundary_vertices: &[[f64; 3]]) -> Vec<Triangle> {
    let n = boundary_vertices.len();
    if n < 3 { return Vec::new(); }
    let mut c = [0.0;3];
    for v in boundary_vertices { c = add(c, *v); }
    c = scale(c, 1.0/n as f64);
    let mut tris = Vec::with_capacity(n);
    for i in 0..n {
        let j = (i+1)%n;
        tris.push(Triangle::new(c, boundary_vertices[i], boundary_vertices[j]));
    }
    tris
}

/// Extract boundary edges (edges in exactly one triangle).
pub fn extract_boundary_edges(triangles: &[Triangle]) -> Vec<([f64;3],[f64;3])> {
    let mut es: HashMap<([i64;3],[i64;3]),([f64;3],[f64;3])> = HashMap::new();
    let mut eu: HashMap<([i64;3],[i64;3]),usize> = HashMap::new();
    for tri in triangles {
        for (a,b) in &[(tri.v0,tri.v1),(tri.v1,tri.v2),(tri.v2,tri.v0)] {
            let k = edge_key(*a,*b);
            es.entry(k).or_insert((*a,*b));
            *eu.entry(k).or_insert(0) += 1;
        }
    }
    es.into_iter().filter(|(k,_)| eu.get(k).copied().unwrap_or(0)==1).map(|(_,v)|v).collect()
}

/// Split triangles at a plane (normal . x = offset).
pub fn split_at_plane(
    triangles: &[Triangle], normal: [f64;3], offset: f64,
) -> (Vec<Triangle>, Vec<Triangle>) {
    let n = normalize(normal);
    let mut positive = Vec::new();
    let mut negative = Vec::new();
    for tri in triangles {
        let d0 = dot(n, tri.v0) - offset;
        let d1 = dot(n, tri.v1) - offset;
        let d2 = dot(n, tri.v2) - offset;
        let pc = (d0 >= 0.0) as u8 + (d1 >= 0.0) as u8 + (d2 >= 0.0) as u8;
        match pc {
            3 => { positive.push(*tri); }
            0 => { negative.push(*tri); }
            _ => {
                let verts = [tri.v0, tri.v1, tri.v2];
                let dists = [d0, d1, d2];
                let mut pv = Vec::new();
                let mut nv = Vec::new();
                for i in 0..3 {
                    let j = (i+1)%3;
                    if dists[i] >= 0.0 { pv.push(verts[i]); } else { nv.push(verts[i]); }
                    if (dists[i] >= 0.0) != (dists[j] >= 0.0) {
                        let t = dists[i] / (dists[i] - dists[j]);
                        let ip = add(verts[i], scale(sub(verts[j], verts[i]), t));
                        pv.push(ip); nv.push(ip);
                    }
                }
                fan_tri(&pv, &mut positive);
                fan_tri(&nv, &mut negative);
            }
        }
    }
    (positive, negative)
}

fn fan_tri(verts: &[[f64;3]], out: &mut Vec<Triangle>) {
    if verts.len() < 3 { return; }
    for i in 1..(verts.len()-1) {
        out.push(Triangle::new(verts[0], verts[i], verts[i+1]));
    }
}

/// Compute total surface area of a triangle mesh.
pub fn compute_surface_area(triangles: &[Triangle]) -> f64 {
    triangles.iter().map(|t| triangle_area(t)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make_sq() -> Vec<Triangle> {
        vec![
            Triangle::new([0.0,0.0,0.0],[1.0,0.0,0.0],[1.0,1.0,0.0]),
            Triangle::new([0.0,0.0,0.0],[1.0,1.0,0.0],[0.0,1.0,0.0]),
        ]
    }
    #[test] fn test_surface_area() {
        assert!((compute_surface_area(&make_sq()) - 1.0).abs() < 1e-12);
    }
    #[test] fn test_surface_area_single() {
        let t = vec![Triangle::new([0.0,0.0,0.0],[2.0,0.0,0.0],[0.0,3.0,0.0])];
        assert!((compute_surface_area(&t) - 3.0).abs() < 1e-12);
    }
    #[test] fn test_offset() {
        let tris = make_sq();
        let off = offset_surface(&tris, 1.0);
        assert_eq!(off.len(), tris.len());
        for tri in &off {
            assert!((tri.v0[2] - 1.0).abs() < 1e-10);
            assert!((tri.v1[2] - 1.0).abs() < 1e-10);
            assert!((tri.v2[2] - 1.0).abs() < 1e-10);
        }
    }
    #[test] fn test_offset_neg() {
        let off = offset_surface(&make_sq(), -0.5);
        for tri in &off { assert!((tri.v0[2] + 0.5).abs() < 1e-10); }
    }
    #[test] fn test_boundary_edges() {
        let edges = extract_boundary_edges(&make_sq());
        assert_eq!(edges.len(), 4);
    }
    #[test] fn test_boundary_closed() {
        let tris = vec![
            Triangle::new([0.0,0.0,0.0],[1.0,0.0,0.0],[0.5,1.0,0.0]),
            Triangle::new([0.0,0.0,0.0],[0.5,1.0,0.0],[0.5,0.5,1.0]),
            Triangle::new([1.0,0.0,0.0],[0.5,0.5,1.0],[0.5,1.0,0.0]),
            Triangle::new([0.0,0.0,0.0],[0.5,0.5,1.0],[1.0,0.0,0.0]),
        ];
        assert_eq!(extract_boundary_edges(&tris).len(), 0);
    }
    #[test] fn test_split_all_pos() {
        let tris = vec![Triangle::new([0.0,0.0,1.0],[1.0,0.0,1.0],[0.5,1.0,1.0])];
        let (p, n) = split_at_plane(&tris, [0.0,0.0,1.0], 0.0);
        assert_eq!(p.len(), 1); assert_eq!(n.len(), 0);
    }
    #[test] fn test_split_crossing() {
        let tris = vec![Triangle::new([0.0,0.0,-1.0],[1.0,0.0,-1.0],[0.5,0.0,1.0])];
        let (p, n) = split_at_plane(&tris, [0.0,0.0,1.0], 0.0);
        assert!(!p.is_empty()); assert!(!n.is_empty());
        let orig = compute_surface_area(&tris);
        let split = compute_surface_area(&p) + compute_surface_area(&n);
        assert!((orig - split).abs() < 1e-10);
    }
    #[test] fn test_cap_opening() {
        let verts = vec![[0.0,0.0,0.0],[1.0,0.0,0.0],[1.0,1.0,0.0],[0.0,1.0,0.0]];
        let cap = cap_opening(&verts);
        assert_eq!(cap.len(), 4);
        assert!((compute_surface_area(&cap) - 1.0).abs() < 1e-12);
    }
    #[test] fn test_cap_triangle() {
        let verts = vec![[0.0,0.0,0.0],[1.0,0.0,0.0],[0.5,1.0,0.0]];
        assert_eq!(cap_opening(&verts).len(), 3);
    }
    #[test] fn test_stitch() {
        let a = make_sq();
        let b = vec![
            Triangle::new([0.0,0.0,0.01],[1.0,0.0,0.01],[1.0,1.0,0.01]),
            Triangle::new([0.0,0.0,0.01],[1.0,1.0,0.01],[0.0,1.0,0.01]),
        ];
        let s = stitch_surfaces(&a, &b, 0.1);
        assert!(s.len() >= 4);
    }
    #[test] fn test_offset_empty() {
        assert!(offset_surface(&[], 1.0).is_empty());
    }
}
