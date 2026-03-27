//! Mesh-based defeaturing for triangle meshes (STL / surface meshes).
//!
//! Detects and removes small geometric features such as tiny faces,
//! short edges, small holes, and sliver triangles.

use crate::geometry::distance_field::Triangle;
use std::collections::HashMap;

/// Result of a defeaturing operation.
#[derive(Debug, Clone, Default)]
pub struct DefeaturingResult {
    pub removed_faces: usize,
    pub removed_edges: usize,
    pub filled_holes: usize,
    pub removed_fillets: usize,
    pub total_issues_fixed: usize,
}

/// Configuration for defeaturing operations.
#[derive(Debug, Clone)]
pub struct DefeaturingConfig {
    pub min_face_area: f64,
    pub min_edge_length: f64,
    pub max_hole_diameter: f64,
    pub max_fillet_radius: f64,
    pub max_sliver_aspect_ratio: f64,
}

impl Default for DefeaturingConfig {
    fn default() -> Self {
        Self {
            min_face_area: 1e-6,
            min_edge_length: 1e-4,
            max_hole_diameter: 0.01,
            max_fillet_radius: 0.005,
            max_sliver_aspect_ratio: 20.0,
        }
    }
}

/// Result of feature detection on a triangle mesh.
#[derive(Debug, Clone, Default)]
pub struct FeatureDetectionResult {
    pub small_faces: Vec<usize>,
    pub short_edges: Vec<(usize, usize)>,
    pub small_holes: Vec<Vec<usize>>,
    pub sliver_triangles: Vec<usize>,
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { [a[0]+b[0], a[1]+b[1], a[2]+b[2]] }
fn scale(a: [f64; 3], s: f64) -> [f64; 3] { [a[0]*s, a[1]*s, a[2]*s] }
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 { a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
}
fn length(a: [f64; 3]) -> f64 { dot(a, a).sqrt() }
fn midpoint(a: [f64; 3], b: [f64; 3]) -> [f64; 3] { scale(add(a, b), 0.5) }

fn triangle_area(tri: &Triangle) -> f64 {
    0.5 * length(cross(sub(tri.v1, tri.v0), sub(tri.v2, tri.v0)))
}

fn triangle_aspect_ratio(tri: &Triangle) -> f64 {
    let a = length(sub(tri.v1, tri.v0));
    let b = length(sub(tri.v2, tri.v1));
    let c = length(sub(tri.v0, tri.v2));
    let longest = a.max(b).max(c);
    let area = triangle_area(tri);
    if area < 1e-30 { return f64::MAX; }
    longest * longest / (2.0 * area)
}

fn vertex_key(v: [f64; 3]) -> [i64; 3] {
    [(v[0]*1e10).round() as i64, (v[1]*1e10).round() as i64, (v[2]*1e10).round() as i64]
}

fn edge_key(a: [f64; 3], b: [f64; 3]) -> ([i64; 3], [i64; 3]) {
    let ka = vertex_key(a); let kb = vertex_key(b);
    if ka < kb { (ka, kb) } else { (kb, ka) }
}

fn collect_edges(tris: &[Triangle]) -> Vec<([f64; 3], [f64; 3])> {
    let mut seen = std::collections::HashSet::new();
    let mut edges = Vec::new();
    for tri in tris {
        for (a, b) in &[(tri.v0,tri.v1),(tri.v1,tri.v2),(tri.v2,tri.v0)] {
            let key = edge_key(*a, *b);
            if seen.insert(key) { edges.push((*a, *b)); }
        }
    }
    edges
}

fn find_boundary_edges(tris: &[Triangle]) -> Vec<([f64; 3], [f64; 3])> {
    let mut es: HashMap<([i64;3],[i64;3]),([f64;3],[f64;3])> = HashMap::new();
    let mut eu: HashMap<([i64;3],[i64;3]),usize> = HashMap::new();
    for tri in tris {
        for (a,b) in &[(tri.v0,tri.v1),(tri.v1,tri.v2),(tri.v2,tri.v0)] {
            let k = edge_key(*a,*b);
            es.entry(k).or_insert((*a,*b));
            *eu.entry(k).or_insert(0) += 1;
        }
    }
    es.into_iter().filter(|(k,_)| eu.get(k).copied().unwrap_or(0)==1).map(|(_,v)|v).collect()
}

fn build_boundary_loops(be: &[([f64;3],[f64;3])]) -> Vec<Vec<[f64;3]>> {
    if be.is_empty() { return Vec::new(); }
    let mut adj: HashMap<[i64;3],Vec<[f64;3]>> = HashMap::new();
    for (a,b) in be {
        adj.entry(vertex_key(*a)).or_default().push(*b);
        adj.entry(vertex_key(*b)).or_default().push(*a);
    }
    let mut visited = std::collections::HashSet::new();
    let mut loops = Vec::new();
    for (a,b) in be {
        let key = edge_key(*a,*b);
        if visited.contains(&key) { continue; }
        let mut lv = vec![*a];
        let mut cur = *b;
        let sk = vertex_key(*a);
        visited.insert(key);
        loop {
            lv.push(cur);
            let ck = vertex_key(cur);
            if ck == sk { break; }
            let nb = match adj.get(&ck) { Some(n) => n, None => break };
            let prev = *lv.get(lv.len().wrapping_sub(2)).unwrap_or(&cur);
            let pk = vertex_key(prev);
            let mut found = false;
            for n in nb {
                let nk = vertex_key(*n);
                if nk == pk { continue; }
                let ek = edge_key(cur, *n);
                if !visited.contains(&ek) {
                    visited.insert(ek); cur = *n; found = true; break;
                }
            }
            if !found { break; }
        }
        if lv.len() >= 3 { loops.push(lv); }
    }
    loops
}

fn loop_diameter(verts: &[[f64;3]]) -> f64 {
    let mut mx = 0.0f64;
    for i in 0..verts.len() {
        for j in (i+1)..verts.len() {
            let d = length(sub(verts[i], verts[j]));
            if d > mx { mx = d; }
        }
    }
    mx
}

/// Detect small features in a triangle mesh.
pub fn detect_features(tris: &[Triangle], config: &DefeaturingConfig) -> FeatureDetectionResult {
    let mut r = FeatureDetectionResult::default();
    for (i,tri) in tris.iter().enumerate() {
        if triangle_area(tri) < config.min_face_area { r.small_faces.push(i); }
    }
    let edges = collect_edges(tris);
    for (a,b) in &edges {
        if length(sub(*a,*b)) < config.min_edge_length {
            for (i,tri) in tris.iter().enumerate() {
                let ka = vertex_key(*a); let kb = vertex_key(*b);
                let tv = [vertex_key(tri.v0),vertex_key(tri.v1),vertex_key(tri.v2)];
                if tv.contains(&ka) && tv.contains(&kb) {
                    if !r.short_edges.iter().any(|(x,_)| *x==i) { r.short_edges.push((i,i)); }
                }
            }
        }
    }
    let be = find_boundary_edges(tris);
    let loops = build_boundary_loops(&be);
    for lv in &loops {
        if loop_diameter(lv) < config.max_hole_diameter {
            r.small_holes.push((0..lv.len()).collect());
        }
    }
    for (i,tri) in tris.iter().enumerate() {
        let ar = triangle_aspect_ratio(tri);
        if ar > config.max_sliver_aspect_ratio && !r.small_faces.contains(&i) {
            r.sliver_triangles.push(i);
        }
    }
    r
}

/// Remove small faces by collapsing them into their neighbors.
pub fn remove_small_faces(tris: &mut Vec<Triangle>, min_area: f64) -> usize {
    let mut removed = 0;
    loop {
        let idx = tris.iter().position(|t| triangle_area(t) < min_area);
        let idx = match idx { Some(i) => i, None => break };
        let tri = tris[idx];
        let c = scale(add(add(tri.v0,tri.v1),tri.v2), 1.0/3.0);
        let vk: Vec<_> = [tri.v0,tri.v1,tri.v2].iter().map(|v| vertex_key(*v)).collect();
        tris.remove(idx); removed += 1;
        for t in tris.iter_mut() {
            if vk.contains(&vertex_key(t.v0)) { t.v0 = c; }
            if vk.contains(&vertex_key(t.v1)) { t.v1 = c; }
            if vk.contains(&vertex_key(t.v2)) { t.v2 = c; }
        }
        tris.retain(|t| triangle_area(t) > 1e-30);
    }
    removed
}

/// Collapse short edges by merging their two endpoint vertices.
pub fn collapse_short_edges(tris: &mut Vec<Triangle>, min_len: f64) -> usize {
    let mut collapsed = 0;
    loop {
        let mut se: Option<([f64;3],[f64;3])> = None;
        for tri in tris.iter() {
            for (a,b) in &[(tri.v0,tri.v1),(tri.v1,tri.v2),(tri.v2,tri.v0)] {
                let l = length(sub(*a,*b));
                if l < min_len && l > 1e-30 { se = Some((*a,*b)); break; }
            }
            if se.is_some() { break; }
        }
        let (va,vb) = match se { Some(e) => e, None => break };
        let mid = midpoint(va, vb);
        let ka = vertex_key(va); let kb = vertex_key(vb);
        for t in tris.iter_mut() {
            if vertex_key(t.v0)==ka || vertex_key(t.v0)==kb { t.v0 = mid; }
            if vertex_key(t.v1)==ka || vertex_key(t.v1)==kb { t.v1 = mid; }
            if vertex_key(t.v2)==ka || vertex_key(t.v2)==kb { t.v2 = mid; }
        }
        tris.retain(|t| {
            let a=vertex_key(t.v0); let b=vertex_key(t.v1); let c=vertex_key(t.v2);
            a!=b && b!=c && a!=c
        });
        collapsed += 1;
    }
    collapsed
}

/// Fill small holes by finding boundary loops and fan-triangulating them.
pub fn fill_small_holes(tris: &mut Vec<Triangle>, max_diam: f64) -> usize {
    let be = find_boundary_edges(tris);
    let loops = build_boundary_loops(&be);
    let mut filled = 0;
    for lv in &loops {
        if lv.len() < 3 { continue; }
        if loop_diameter(lv) > max_diam { continue; }
        let n = lv.len() as f64;
        let mut c = [0.0;3];
        for v in lv { c = add(c, *v); }
        c = scale(c, 1.0/n);
        for i in 0..lv.len() {
            let j = (i+1) % lv.len();
            if vertex_key(lv[i]) == vertex_key(lv[j]) { continue; }
            tris.push(Triangle::new(c, lv[i], lv[j]));
        }
        filled += 1;
    }
    filled
}

/// Remove sliver triangles (high aspect ratio).
pub fn remove_slivers(tris: &mut Vec<Triangle>, max_ar: f64) -> usize {
    let mut removed = 0;
    loop {
        let idx = tris.iter().position(|t| triangle_aspect_ratio(t) > max_ar);
        let idx = match idx { Some(i) => i, None => break };
        let tri = tris[idx];
        let edges = [
            (tri.v0,tri.v1,length(sub(tri.v0,tri.v1))),
            (tri.v1,tri.v2,length(sub(tri.v1,tri.v2))),
            (tri.v2,tri.v0,length(sub(tri.v2,tri.v0))),
        ];
        let (va,vb,_) = edges.iter().copied()
            .min_by(|a,b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)).unwrap();
        let mid = midpoint(va, vb);
        let ka = vertex_key(va); let kb = vertex_key(vb);
        for t in tris.iter_mut() {
            if vertex_key(t.v0)==ka || vertex_key(t.v0)==kb { t.v0 = mid; }
            if vertex_key(t.v1)==ka || vertex_key(t.v1)==kb { t.v1 = mid; }
            if vertex_key(t.v2)==ka || vertex_key(t.v2)==kb { t.v2 = mid; }
        }
        let before = tris.len();
        tris.retain(|t| {
            let a=vertex_key(t.v0);let b=vertex_key(t.v1);let c=vertex_key(t.v2);
            a!=b && b!=c && a!=c
        });
        removed += before - tris.len();
        if before == tris.len() && idx < tris.len() { tris.remove(idx); removed += 1; }
    }
    removed
}

/// Auto-defeaturing: apply all fixes iteratively until convergence.
pub fn auto_defeature(tris: &mut Vec<Triangle>, config: &DefeaturingConfig) -> DefeaturingResult {
    let mut result = DefeaturingResult::default();
    for _ in 0..10 {
        let rf = remove_small_faces(tris, config.min_face_area);
        let re = collapse_short_edges(tris, config.min_edge_length);
        let fh = fill_small_holes(tris, config.max_hole_diameter);
        let rs = remove_slivers(tris, config.max_sliver_aspect_ratio);
        result.removed_faces += rf; result.removed_edges += re; result.filled_holes += fh;
        let total = rf + re + fh + rs;
        result.total_issues_fixed += total;
        if total == 0 { break; }
    }
    result
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
    #[test] fn test_tri_area() {
        let t = Triangle::new([0.0,0.0,0.0],[1.0,0.0,0.0],[0.0,1.0,0.0]);
        assert!((triangle_area(&t) - 0.5).abs() < 1e-12);
    }
    #[test] fn test_ar_equilateral() {
        let t = Triangle::new([0.0,0.0,0.0],[1.0,0.0,0.0],[0.5,0.866025403784,0.0]);
        assert!(triangle_aspect_ratio(&t) < 2.0);
    }
    #[test] fn test_ar_sliver() {
        let t = Triangle::new([0.0,0.0,0.0],[10.0,0.0,0.0],[5.0,0.001,0.0]);
        assert!(triangle_aspect_ratio(&t) > 100.0);
    }
    #[test] fn test_detect_small_face() {
        let mut tris = make_sq();
        tris.push(Triangle::new([0.0,0.0,0.0],[1e-8,0.0,0.0],[0.0,1e-8,0.0]));
        let c = DefeaturingConfig{min_face_area:1e-6,..DefeaturingConfig::default()};
        let r = detect_features(&tris, &c);
        assert!(!r.small_faces.is_empty()); assert!(r.small_faces.contains(&2));
    }
    #[test] fn test_detect_sliver() {
        let tris = vec![Triangle::new([0.0,0.0,0.0],[10.0,0.0,0.0],[5.0,0.001,0.0])];
        let c = DefeaturingConfig{max_sliver_aspect_ratio:20.0,min_face_area:1e-10,..DefeaturingConfig::default()};
        assert!(!detect_features(&tris,&c).sliver_triangles.is_empty());
    }
    #[test] fn test_remove_small_faces() {
        let mut tris = make_sq();
        tris.push(Triangle::new([2.0,0.0,0.0],[2.0+1e-8,0.0,0.0],[2.0,1e-8,0.0]));
        let n = tris.len();
        assert!(remove_small_faces(&mut tris, 1e-6) > 0);
        assert!(tris.len() < n);
    }
    #[test] fn test_collapse_short() {
        let mut tris = vec![
            Triangle::new([0.0,0.0,0.0],[1.0,0.0,0.0],[0.5,1.0,0.0]),
            Triangle::new([2.0,0.0,0.0],[2.0001,0.0,0.0],[2.0,1.0,0.0]),
        ];
        assert!(collapse_short_edges(&mut tris, 0.001) > 0);
    }
    #[test] fn test_remove_slivers() {
        let mut tris = vec![
            Triangle::new([0.0,0.0,0.0],[1.0,0.0,0.0],[0.5,0.866,0.0]),
            Triangle::new([3.0,0.0,0.0],[13.0,0.0,0.0],[8.0,0.001,0.0]),
        ];
        assert!(remove_slivers(&mut tris, 20.0) > 0);
        assert!(!tris.is_empty());
    }
    #[test] fn test_auto_defeature() {
        let mut tris = make_sq();
        tris.push(Triangle::new([2.0,0.0,0.0],[2.0+1e-8,0.0,0.0],[2.0,1e-8,0.0]));
        assert!(auto_defeature(&mut tris, &DefeaturingConfig::default()).total_issues_fixed > 0);
    }
    #[test] fn test_auto_clean() {
        let mut tris = make_sq();
        let c = DefeaturingConfig{min_face_area:1e-10,min_edge_length:1e-10,max_hole_diameter:1e-10,max_fillet_radius:1e-10,max_sliver_aspect_ratio:100.0};
        assert_eq!(auto_defeature(&mut tris,&c).total_issues_fixed, 0);
        assert_eq!(tris.len(), 2);
    }
    #[test] fn test_no_issues() {
        let tris = make_sq();
        let c = DefeaturingConfig{min_face_area:1e-10,min_edge_length:1e-10,max_hole_diameter:1e-10,max_fillet_radius:1e-10,max_sliver_aspect_ratio:100.0};
        let r = detect_features(&tris,&c);
        assert!(r.small_faces.is_empty()); assert!(r.sliver_triangles.is_empty());
    }
    #[test] fn test_fill_holes() {
        let mut tris = make_sq();
        assert!(!find_boundary_edges(&tris).is_empty());
        let n = tris.len();
        let f = fill_small_holes(&mut tris, 100.0);
        if f > 0 { assert!(tris.len() > n); }
    }
}
