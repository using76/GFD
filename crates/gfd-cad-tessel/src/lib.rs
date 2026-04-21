//! gfd-cad-tessel — tessellate a B-Rep shape into triangles for Three.js.
//!
//! Iteration 2: each `SurfaceGeom` variant emits a UV grid with configurable
//! resolution. Adaptive chord-tolerance refinement lands in a later iteration.

use gfd_cad_geom::{Surface};
use gfd_cad_topo::{shape::SurfaceGeom, Shape, ShapeArena, ShapeId, TopoError};
use gfd_cad_geom::surface::{Plane, Cylinder, Sphere, Cone, Torus};
#[allow(unused_imports)]
use gfd_cad_geom::{Point3, Vector3};
use serde::{Deserialize, Serialize};

pub mod earclip;
pub mod grid;

pub use earclip::triangulate_polygon;
use grid::uv_grid;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TriMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals:   Vec<[f32; 3]>,
    pub indices:   Vec<u32>,
}

impl TriMesh {
    pub fn merge(&mut self, other: TriMesh) {
        let offset = self.positions.len() as u32;
        self.positions.extend(other.positions);
        self.normals.extend(other.normals);
        self.indices.extend(other.indices.into_iter().map(|i| i + offset));
    }

    /// Removes positions (and matching normals) that aren't referenced by
    /// any triangle index. Re-indexes `indices`. Returns the count of
    /// vertices dropped. Safe to call after `weld` (which may leave
    /// orphans from deleted degenerate triangles).
    pub fn prune_unused_vertices(&mut self) -> usize {
        if self.positions.is_empty() { return 0; }
        let mut used = vec![false; self.positions.len()];
        for &i in &self.indices {
            if let Some(flag) = used.get_mut(i as usize) { *flag = true; }
        }
        let has_normals = self.normals.len() == self.positions.len();
        let mut remap: Vec<u32> = vec![0; self.positions.len()];
        let mut new_positions: Vec<[f32; 3]> = Vec::new();
        let mut new_normals: Vec<[f32; 3]> = Vec::new();
        for (i, u) in used.iter().enumerate() {
            if *u {
                remap[i] = new_positions.len() as u32;
                new_positions.push(self.positions[i]);
                if has_normals { new_normals.push(self.normals[i]); }
            }
        }
        let dropped = self.positions.len() - new_positions.len();
        for idx in self.indices.iter_mut() {
            *idx = remap[*idx as usize];
        }
        self.positions = new_positions;
        self.normals = new_normals;
        dropped
    }

    /// Laplacian mesh smoothing: pulls each vertex toward the mean of its
    /// edge-adjacent neighbours. `factor` ∈ [0, 1] is the blend ratio
    /// (0 = no change, 1 = full replacement). Repeated `iterations` times.
    /// Preserves topology; drops stored normals (caller can re-compute).
    /// Works correctly only on welded meshes — duplicate-corner tessellator
    /// output will not smooth across face boundaries.
    pub fn laplacian_smooth(&mut self, iterations: usize, factor: f32) {
        if iterations == 0 || factor <= 0.0 || self.positions.is_empty() { return; }
        let f = factor.clamp(0.0, 1.0);
        let n = self.positions.len();
        let mut adj: Vec<std::collections::HashSet<u32>> =
            vec![std::collections::HashSet::new(); n];
        for t in 0..(self.indices.len() / 3) {
            let i0 = self.indices[t * 3];
            let i1 = self.indices[t * 3 + 1];
            let i2 = self.indices[t * 3 + 2];
            adj[i0 as usize].insert(i1); adj[i0 as usize].insert(i2);
            adj[i1 as usize].insert(i0); adj[i1 as usize].insert(i2);
            adj[i2 as usize].insert(i0); adj[i2 as usize].insert(i1);
        }
        for _ in 0..iterations {
            let mut new_pos = self.positions.clone();
            for (i, neighbours) in adj.iter().enumerate() {
                if neighbours.is_empty() { continue; }
                let mut sum = [0.0_f32; 3];
                for &j in neighbours {
                    let q = self.positions[j as usize];
                    sum[0] += q[0]; sum[1] += q[1]; sum[2] += q[2];
                }
                let inv = 1.0 / neighbours.len() as f32;
                let avg = [sum[0] * inv, sum[1] * inv, sum[2] * inv];
                let cur = self.positions[i];
                new_pos[i] = [
                    cur[0] + f * (avg[0] - cur[0]),
                    cur[1] + f * (avg[1] - cur[1]),
                    cur[2] + f * (avg[2] - cur[2]),
                ];
            }
            self.positions = new_pos;
        }
        if !self.normals.is_empty() { self.normals.clear(); }
    }

    /// Translates the mesh so its AABB is centered at the origin, then
    /// uniformly scales so the longest AABB side becomes `2 · target_half`
    /// (default 1.0 → mesh fits in `[-1, 1]³`). Returns `(center_before, scale)`
    /// so callers can invert the transform. No-op on empty meshes.
    pub fn center_and_normalize(&mut self, target_half: f32) -> ([f32; 3], f32) {
        let Some((mn, mx)) = self.aabb() else {
            return ([0.0; 3], 1.0);
        };
        let center = [
            0.5 * (mn[0] + mx[0]),
            0.5 * (mn[1] + mx[1]),
            0.5 * (mn[2] + mx[2]),
        ];
        let dx = mx[0] - mn[0];
        let dy = mx[1] - mn[1];
        let dz = mx[2] - mn[2];
        let max_side = dx.max(dy).max(dz);
        let scale = if max_side > 0.0 { 2.0 * target_half / max_side } else { 1.0 };
        for p in self.positions.iter_mut() {
            p[0] = (p[0] - center[0]) * scale;
            p[1] = (p[1] - center[1]) * scale;
            p[2] = (p[2] - center[2]) * scale;
        }
        (center, scale)
    }

    /// Per-triangle face normals, one entry per triangle (length = indices.len()/3).
    /// Zero-area triangles get a `(0, 0, 1)` placeholder.
    pub fn compute_face_normals(&self) -> Vec<[f32; 3]> {
        let n_tri = self.indices.len() / 3;
        let mut out = Vec::with_capacity(n_tri);
        for t in 0..n_tri {
            let a = self.positions[self.indices[t * 3] as usize];
            let b = self.positions[self.indices[t * 3 + 1] as usize];
            let c = self.positions[self.indices[t * 3 + 2] as usize];
            let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let nx = e1[1] * e2[2] - e1[2] * e2[1];
            let ny = e1[2] * e2[0] - e1[0] * e2[2];
            let nz = e1[0] * e2[1] - e1[1] * e2[0];
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            if len > 1.0e-20 {
                out.push([nx / len, ny / len, nz / len]);
            } else {
                out.push([0.0, 0.0, 1.0]);
            }
        }
        out
    }

    /// Axis-aligned bounding box over all positions.
    /// Returns `None` if the mesh is empty.
    pub fn aabb(&self) -> Option<([f32; 3], [f32; 3])> {
        if self.positions.is_empty() { return None; }
        let mut mn = [f32::INFINITY; 3];
        let mut mx = [f32::NEG_INFINITY; 3];
        for p in &self.positions {
            for k in 0..3 {
                if p[k] < mn[k] { mn[k] = p[k]; }
                if p[k] > mx[k] { mx[k] = p[k]; }
            }
        }
        Some((mn, mx))
    }

    /// True iff this mesh's AABB overlaps `other`'s AABB (inclusive).
    /// Separating-axis test on each axis. Used by CSG implementations
    /// as a cheap O(|A|+|B|) reject before O(|A|·|B|) triangle tests.
    pub fn aabb_overlaps(&self, other: &TriMesh) -> bool {
        let Some((a_mn, a_mx)) = self.aabb() else { return false; };
        let Some((b_mn, b_mx)) = other.aabb() else { return false; };
        for k in 0..3 {
            if a_mx[k] < b_mn[k] || b_mx[k] < a_mn[k] { return false; }
        }
        true
    }

    /// Subdivides every triangle into 4 via edge-midpoint insertion
    /// (4-1 loop-style topology, but midpoints are not smoothed). After
    /// one pass: 4× triangles, ~3× vertices (unique midpoints shared
    /// between adjacent faces). Useful for densifying a coarse mesh before
    /// curvature-based smoothing or for mesh CSG stability.
    pub fn subdivide_midpoint(&mut self) {
        if self.indices.is_empty() { return; }
        let mut mid_cache: std::collections::HashMap<(u32, u32), u32> =
            std::collections::HashMap::new();
        let mut new_positions = self.positions.clone();
        let mut new_indices: Vec<u32> = Vec::with_capacity(self.indices.len() * 4);
        let mut edge_mid = |a: u32, b: u32, pos: &mut Vec<[f32; 3]>| -> u32 {
            let key = if a < b { (a, b) } else { (b, a) };
            if let Some(&idx) = mid_cache.get(&key) { return idx; }
            let pa = pos[a as usize];
            let pb = pos[b as usize];
            let mid = [
                0.5 * (pa[0] + pb[0]),
                0.5 * (pa[1] + pb[1]),
                0.5 * (pa[2] + pb[2]),
            ];
            let idx = pos.len() as u32;
            pos.push(mid);
            mid_cache.insert(key, idx);
            idx
        };
        for t in 0..(self.indices.len() / 3) {
            let a = self.indices[t * 3];
            let b = self.indices[t * 3 + 1];
            let c = self.indices[t * 3 + 2];
            let ab = edge_mid(a, b, &mut new_positions);
            let bc = edge_mid(b, c, &mut new_positions);
            let ca = edge_mid(c, a, &mut new_positions);
            new_indices.extend([a, ab, ca]);
            new_indices.extend([b, bc, ab]);
            new_indices.extend([c, ca, bc]);
            new_indices.extend([ab, bc, ca]);
        }
        self.positions = new_positions;
        self.indices = new_indices;
        // Normals become stale — caller can re-run compute_smooth_normals.
        if !self.normals.is_empty() { self.normals.clear(); }
    }

    /// Applies a row-major 4×4 affine transform to positions and rotates
    /// normals by the 3×3 upper-left block (assumes rotation/uniform-scale —
    /// for non-uniform scale, pass inverse-transpose normals separately).
    /// If the transform has a negative determinant (mirror), winding is
    /// also flipped so face normals stay outward.
    pub fn transform(&mut self, m: [[f64; 4]; 4]) {
        let det =
              m[0][0]*(m[1][1]*m[2][2] - m[1][2]*m[2][1])
            - m[0][1]*(m[1][0]*m[2][2] - m[1][2]*m[2][0])
            + m[0][2]*(m[1][0]*m[2][1] - m[1][1]*m[2][0]);
        for p in self.positions.iter_mut() {
            let x = p[0] as f64; let y = p[1] as f64; let z = p[2] as f64;
            let nx = m[0][0]*x + m[0][1]*y + m[0][2]*z + m[0][3];
            let ny = m[1][0]*x + m[1][1]*y + m[1][2]*z + m[1][3];
            let nz = m[2][0]*x + m[2][1]*y + m[2][2]*z + m[2][3];
            *p = [nx as f32, ny as f32, nz as f32];
        }
        for n in self.normals.iter_mut() {
            let x = n[0] as f64; let y = n[1] as f64; let z = n[2] as f64;
            let nx = m[0][0]*x + m[0][1]*y + m[0][2]*z;
            let ny = m[1][0]*x + m[1][1]*y + m[1][2]*z;
            let nz = m[2][0]*x + m[2][1]*y + m[2][2]*z;
            let len = (nx*nx + ny*ny + nz*nz).sqrt();
            if len > 1e-20 {
                *n = [(nx / len) as f32, (ny / len) as f32, (nz / len) as f32];
            }
        }
        if det < 0.0 { self.reverse_winding(); }
    }

    /// Flips triangle winding by swapping the second and third index of each
    /// triangle. Reverses every face normal. Used when a CSG difference
    /// leaves inner surfaces facing the wrong way.
    pub fn reverse_winding(&mut self) {
        for t in 0..(self.indices.len() / 3) {
            self.indices.swap(t * 3 + 1, t * 3 + 2);
        }
    }

    /// Recomputes per-vertex normals from the area-weighted sum of incident
    /// triangle normals, then normalises. Replaces any existing `normals`.
    /// Uses the current `positions`/`indices`; call after `weld` if you
    /// want smoothing across face boundaries.
    pub fn compute_smooth_normals(&mut self) {
        let mut acc = vec![[0.0_f32; 3]; self.positions.len()];
        for t in 0..(self.indices.len() / 3) {
            let i0 = self.indices[t * 3] as usize;
            let i1 = self.indices[t * 3 + 1] as usize;
            let i2 = self.indices[t * 3 + 2] as usize;
            let a = self.positions[i0];
            let b = self.positions[i1];
            let c = self.positions[i2];
            let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            // Un-normalised normal; its length = 2·area, so weighting is automatic.
            for idx in [i0, i1, i2] {
                acc[idx][0] += n[0];
                acc[idx][1] += n[1];
                acc[idx][2] += n[2];
            }
        }
        self.normals = acc.into_iter().map(|v| {
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if len > 1.0e-20 { [v[0] / len, v[1] / len, v[2] / len] } else { [0.0, 0.0, 1.0] }
        }).collect();
    }

    /// Merge vertices whose positions coincide within `tol` (per-axis grid
    /// quantisation). Rewrites `indices` to reference the welded set and
    /// drops duplicate entries from `positions` (and `normals` if present).
    /// Returns the count of vertices removed.
    pub fn weld(&mut self, tol: f32) -> usize {
        if self.positions.is_empty() { return 0; }
        let inv = if tol > 0.0 { 1.0 / tol } else { 0.0 };
        let key = |p: [f32; 3]| -> (i64, i64, i64) {
            ((p[0] * inv).round() as i64,
             (p[1] * inv).round() as i64,
             (p[2] * inv).round() as i64)
        };
        let mut map: std::collections::HashMap<(i64, i64, i64), u32> =
            std::collections::HashMap::new();
        let mut new_positions: Vec<[f32; 3]> = Vec::new();
        let mut new_normals: Vec<[f32; 3]> = Vec::new();
        let has_normals = self.normals.len() == self.positions.len();
        let mut remap: Vec<u32> = Vec::with_capacity(self.positions.len());
        for (i, p) in self.positions.iter().enumerate() {
            let k = key(*p);
            let new_idx = *map.entry(k).or_insert_with(|| {
                let idx = new_positions.len() as u32;
                new_positions.push(*p);
                if has_normals { new_normals.push(self.normals[i]); }
                idx
            });
            remap.push(new_idx);
        }
        let removed = self.positions.len() - new_positions.len();
        for i in self.indices.iter_mut() {
            *i = remap[*i as usize];
        }
        self.positions = new_positions;
        self.normals = new_normals;
        // Drop degenerate triangles (two or more identical corners after weld).
        let mut kept: Vec<u32> = Vec::with_capacity(self.indices.len());
        for t in 0..(self.indices.len() / 3) {
            let a = self.indices[t * 3];
            let b = self.indices[t * 3 + 1];
            let c = self.indices[t * 3 + 2];
            if a != b && b != c && a != c {
                kept.push(a); kept.push(b); kept.push(c);
            }
        }
        self.indices = kept;
        removed
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TessellationOptions {
    pub u_steps: usize,
    pub v_steps: usize,
    pub chord_tolerance: f64,
    pub angular_tolerance: f64,
}

impl Default for TessellationOptions {
    fn default() -> Self {
        Self { u_steps: 32, v_steps: 16, chord_tolerance: 0.01, angular_tolerance: 0.5 }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TessellError {
    #[error("tessellation not yet implemented for this shape")]
    Unimplemented,
    #[error(transparent)]
    Topo(#[from] TopoError),
    #[error("geometry: {0}")]
    Geom(String),
}

pub type TessellResult<T> = Result<T, TessellError>;

/// Tessellate the shape tree rooted at `id`. Compounds / Solids / Shells fan
/// out recursively. Faces call into [`tessellate_surface`].
pub fn tessellate(arena: &ShapeArena, id: ShapeId, opts: TessellationOptions) -> TessellResult<TriMesh> {
    let mut mesh = TriMesh::default();
    walk(arena, id, opts, &mut mesh)?;
    Ok(mesh)
}

/// Adaptive tessellation: each face picks its own u/v step counts based on
/// `chord_tolerance` via [`auto_uv_steps`]. Planar faces use a minimal grid.
/// Extract a line-segment wireframe from every line-edge reachable from
/// `id`. Output is a flat `Vec<[f32; 3]>` of alternating start/end vertex
/// positions — ready for `THREE.LineSegments` consumption.
pub fn extract_edges(arena: &ShapeArena, id: ShapeId) -> TessellResult<Vec<[f32; 3]>> {
    let mut out = Vec::new();
    walk_edges(arena, id, &mut out)?;
    Ok(out)
}

fn walk_edges(arena: &ShapeArena, id: ShapeId, out: &mut Vec<[f32; 3]>) -> TessellResult<()> {
    match arena.get(id)? {
        Shape::Compound { children } => for c in children { walk_edges(arena, *c, out)?; },
        Shape::Solid { shells }      => for s in shells { walk_edges(arena, *s, out)?; },
        Shape::Shell { faces }       => for (f, _) in faces { walk_edges(arena, *f, out)?; },
        Shape::Face { wires, .. }    => for w in wires { walk_edges(arena, *w, out)?; },
        Shape::Wire { edges }        => for (e, _) in edges { walk_edges(arena, *e, out)?; },
        Shape::Edge { vertices, .. } => {
            let a = match arena.get(vertices[0])? { Shape::Vertex { point } => *point, _ => return Ok(()) };
            let b = match arena.get(vertices[1])? { Shape::Vertex { point } => *point, _ => return Ok(()) };
            out.push([a.x as f32, a.y as f32, a.z as f32]);
            out.push([b.x as f32, b.y as f32, b.z as f32]);
        }
        Shape::Vertex { .. } => {}
    }
    Ok(())
}

pub fn tessellate_adaptive(arena: &ShapeArena, id: ShapeId, chord_tolerance: f64) -> TessellResult<TriMesh> {
    let mut mesh = TriMesh::default();
    walk_adaptive(arena, id, chord_tolerance, &mut mesh)?;
    Ok(mesh)
}

fn walk_adaptive(arena: &ShapeArena, id: ShapeId, chord_tol: f64, out: &mut TriMesh) -> TessellResult<()> {
    match arena.get(id)? {
        Shape::Compound { children } => for c in children { walk_adaptive(arena, *c, chord_tol, out)?; },
        Shape::Solid { shells }      => for s in shells   { walk_adaptive(arena, *s, chord_tol, out)?; },
        Shape::Shell { faces }       => for (f, _) in faces { walk_adaptive(arena, *f, chord_tol, out)?; },
        Shape::Face { surface, .. }  => {
            let (u, v) = auto_uv_steps(surface, chord_tol);
            let opts = TessellationOptions { u_steps: u, v_steps: v, chord_tolerance: chord_tol, ..Default::default() };
            let face_mesh = tessellate_surface(surface, opts)?;
            out.merge(face_mesh);
        }
        _ => {}
    }
    Ok(())
}

fn walk(arena: &ShapeArena, id: ShapeId, opts: TessellationOptions, out: &mut TriMesh) -> TessellResult<()> {
    match arena.get(id)? {
        Shape::Compound { children } => {
            for c in children { walk(arena, *c, opts, out)?; }
        }
        Shape::Solid { shells } => {
            for s in shells { walk(arena, *s, opts, out)?; }
        }
        Shape::Shell { faces } => {
            for (f, _) in faces { walk(arena, *f, opts, out)?; }
        }
        Shape::Face { surface, .. } => {
            let face_mesh = tessellate_surface(surface, opts)?;
            out.merge(face_mesh);
        }
        _ => { /* Vertex / Edge / Wire do not contribute triangles */ }
    }
    Ok(())
}

/// Tessellate a single surface over its natural parameter range.
/// Pick reasonable u/v step counts for a surface so the chord error stays
/// below `chord_tolerance`. A circular/spherical surface needs roughly
/// `π * r / chord_tolerance` × 2 segments around its equator; this helper
/// returns a clamped estimate between 4 and 128 steps per axis.
pub fn auto_uv_steps(surface: &SurfaceGeom, chord_tolerance: f64) -> (usize, usize) {
    let clamp = |n: f64| n.clamp(4.0, 128.0) as usize;
    match surface {
        SurfaceGeom::Plane(_) => (4, 4),
        SurfaceGeom::Cylinder(c) => (clamp(std::f64::consts::PI / (chord_tolerance / c.radius).max(1e-3)), 4),
        SurfaceGeom::Cone(c) => {
            let r = c.r1.max(c.r2).max(c.height);
            (clamp(std::f64::consts::PI / (chord_tolerance / r).max(1e-3)), 8)
        }
        SurfaceGeom::Sphere(s) => {
            let n = clamp(std::f64::consts::PI / (chord_tolerance / s.radius).max(1e-3));
            (n, n / 2)
        }
        SurfaceGeom::Torus(t) => {
            let nu = clamp(std::f64::consts::PI / (chord_tolerance / t.major).max(1e-3));
            let nv = clamp(std::f64::consts::PI / (chord_tolerance / t.minor).max(1e-3));
            (nu, nv)
        }
    }
}

pub fn tessellate_surface(surface: &SurfaceGeom, opts: TessellationOptions) -> TessellResult<TriMesh> {
    match surface {
        SurfaceGeom::Plane(p)    => sample(p, opts, 1.0, 1.0),
        SurfaceGeom::Cylinder(c) => sample(c, opts, 1.0, 1.0),
        // Spheres: collapse pole rings onto shared vertices to avoid
        // slivers and visible seams at the poles.
        SurfaceGeom::Sphere(_)   => {
            let mut mesh = sample(surface_ref(surface), opts, 1.0, 1.0)?;
            collapse_pole_rings(&mut mesh, opts.u_steps + 1, opts.v_steps + 1);
            Ok(mesh)
        }
        SurfaceGeom::Cone(c)     => sample(c, opts, 1.0, 1.0),
        SurfaceGeom::Torus(t)    => sample(t, opts, 1.0, 1.0),
    }
}

fn surface_ref(s: &SurfaceGeom) -> &impl Surface {
    // Only called for Sphere; coerce through the match above.
    if let SurfaceGeom::Sphere(sp) = s { sp } else { panic!("surface_ref: expected Sphere") }
}

/// Merge the first and last UV rows of a UV-grid tessellation onto single
/// representative vertices (the poles of a sphere). Keeps the same index
/// buffer layout — duplicate indices just reference one point now.
fn collapse_pole_rings(mesh: &mut TriMesh, nu: usize, nv: usize) {
    if mesh.positions.len() != nu * nv { return; }
    // v = 0 (south pole) → collapse to first entry of that ring.
    let south_pos = mesh.positions[0];
    let south_norm = mesh.normals[0];
    for i in 1..nu {
        mesh.positions[i] = south_pos;
        mesh.normals[i] = south_norm;
    }
    // v = last (north pole).
    let north_idx = (nv - 1) * nu;
    let north_pos = mesh.positions[north_idx];
    let north_norm = mesh.normals[north_idx];
    for i in 1..nu {
        mesh.positions[north_idx + i] = north_pos;
        mesh.normals[north_idx + i] = north_norm;
    }
}

pub fn sample<S: Surface>(s: &S, opts: TessellationOptions, _u_scale: f64, _v_scale: f64) -> TessellResult<TriMesh> {
    let (u0, u1) = s.u_range();
    let (v0, v1) = s.v_range();
    // Clamp infinite planes to a finite sampling window for visualization.
    let (u0, u1) = if u0.is_infinite() || u1.is_infinite() { (-1.0, 1.0) } else { (u0, u1) };
    let (v0, v1) = if v0.is_infinite() || v1.is_infinite() { (-1.0, 1.0) } else { (v0, v1) };
    let mesh = uv_grid(opts.u_steps, opts.v_steps, u0, u1, v0, v1, |u, v| {
        let p = s.eval(u, v).map_err(|e| TessellError::Geom(e.to_string()))?;
        let n = s.normal(u, v).map_err(|e| TessellError::Geom(e.to_string()))?;
        Ok(([p.x as f32, p.y as f32, p.z as f32],
            [n.x as f32, n.y as f32, n.z as f32]))
    })?;
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_drops_unreferenced_vertices() {
        // positions[3] is never indexed — should be dropped.
        let mut m = TriMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [9.0, 9.0, 9.0],
            ],
            normals: vec![],
            indices: vec![0, 1, 2],
        };
        let dropped = m.prune_unused_vertices();
        assert_eq!(dropped, 1);
        assert_eq!(m.positions.len(), 3);
        assert_eq!(m.indices, vec![0, 1, 2]);
    }

    #[test]
    fn prune_remaps_indices_correctly() {
        // Drop middle vertex (idx 1) by not referencing it.
        let mut m = TriMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [9.0, 9.0, 9.0], // unused
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            normals: vec![],
            indices: vec![0, 2, 3],
        };
        m.prune_unused_vertices();
        assert_eq!(m.positions.len(), 3);
        assert_eq!(m.indices, vec![0, 1, 2]);
        assert_eq!(m.positions[1], [1.0, 0.0, 0.0]); // old [2] is now [1]
    }

    #[test]
    fn laplacian_smooth_relaxes_spiked_vertex() {
        // Four vertices in z=0 plane + one spike above.
        let mut m = TriMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.5, 0.5, 10.0], // central spike
            ],
            normals: vec![],
            indices: vec![
                0, 1, 4,
                1, 3, 4,
                3, 2, 4,
                2, 0, 4,
            ],
        };
        let spike_before = m.positions[4][2];
        m.laplacian_smooth(1, 1.0);
        let spike_after = m.positions[4][2];
        // After one full smoothing pass, spike pulled toward neighbours (z=0 avg).
        assert!(spike_after < spike_before);
        assert!(spike_after.abs() < 0.1);
    }

    #[test]
    fn center_and_normalize_fits_unit_cube() {
        let mut m = TriMesh {
            positions: vec![
                [5.0, 5.0, 5.0],
                [11.0, 5.0, 5.0],   // span 6 in X (longest)
                [5.0, 7.0, 5.0],
                [5.0, 5.0, 9.0],
            ],
            normals: vec![],
            indices: vec![],
        };
        let (center, scale) = m.center_and_normalize(1.0);
        assert!((center[0] - 8.0).abs() < 1e-6);
        assert!((center[1] - 6.0).abs() < 1e-6);
        assert!((center[2] - 7.0).abs() < 1e-6);
        // After normalisation the longest AABB side (6) becomes 2.
        assert!((scale - 2.0 / 6.0).abs() < 1e-6);
        let (mn, mx) = m.aabb().unwrap();
        assert!((mx[0] - mn[0] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn face_normals_on_xy_and_yz_quads() {
        let m = TriMesh {
            positions: vec![
                [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], // z=0
                [0.0, 0.0, 1.0], [0.0, 1.0, 1.0], [0.0, 1.0, 0.0], // x=0
            ],
            normals: vec![],
            indices: vec![0, 1, 2, 3, 4, 5],
        };
        let fn_ = m.compute_face_normals();
        assert_eq!(fn_.len(), 2);
        // Triangle 0 in z=0 plane, CCW → +Z.
        assert!((fn_[0][2] - 1.0).abs() < 1e-6);
        // Triangle 1 in x=0 plane; whichever sign, |n|=1.
        let l = (fn_[1][0].powi(2) + fn_[1][1].powi(2) + fn_[1][2].powi(2)).sqrt();
        assert!((l - 1.0).abs() < 1e-6);
    }

    #[test]
    fn aabb_and_overlap_checks() {
        let a = TriMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]],
            normals: vec![],
            indices: vec![],
        };
        let (mn, mx) = a.aabb().unwrap();
        assert_eq!(mn, [0.0, 0.0, 0.0]);
        assert_eq!(mx, [1.0, 2.0, 3.0]);

        let b_overlap = TriMesh {
            positions: vec![[0.5, 0.5, 0.5], [2.0, 2.0, 2.0]],
            normals: vec![], indices: vec![],
        };
        assert!(a.aabb_overlaps(&b_overlap));

        let b_disjoint = TriMesh {
            positions: vec![[10.0, 10.0, 10.0], [11.0, 11.0, 11.0]],
            normals: vec![], indices: vec![],
        };
        assert!(!a.aabb_overlaps(&b_disjoint));

        let empty = TriMesh::default();
        assert!(empty.aabb().is_none());
        assert!(!empty.aabb_overlaps(&a));
    }

    #[test]
    fn subdivide_triangle_becomes_four() {
        let mut m = TriMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![],
            indices: vec![0, 1, 2],
        };
        m.subdivide_midpoint();
        // 3 original corners + 3 unique midpoints = 6 vertices, 4 triangles.
        assert_eq!(m.positions.len(), 6);
        assert_eq!(m.indices.len() / 3, 4);
        // Midpoints exactly halfway between corner pairs.
        assert!(m.positions.iter().any(|p| (p[0] - 0.5).abs() < 1e-6 && p[1].abs() < 1e-6));
        assert!(m.positions.iter().any(|p| (p[0] - 0.5).abs() < 1e-6 && (p[1] - 0.5).abs() < 1e-6));
    }

    #[test]
    fn subdivide_shares_midpoints_between_triangles() {
        // A quad split into two triangles shares an edge. After subdivide,
        // the shared edge's midpoint must be reused → 5 unique verts, not 6.
        let mut m = TriMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            normals: vec![],
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        m.subdivide_midpoint();
        // 4 corners + 5 unique midpoints (4 edges + 1 diagonal) = 9 vertices.
        assert_eq!(m.positions.len(), 9);
        assert_eq!(m.indices.len() / 3, 8);
    }

    #[test]
    fn transform_translation_moves_vertices() {
        let mut m = TriMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![],
            indices: vec![0, 1, 2],
        };
        let tx = [
            [1.0, 0.0, 0.0, 5.0],
            [0.0, 1.0, 0.0, 3.0],
            [0.0, 0.0, 1.0, 7.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        m.transform(tx);
        assert_eq!(m.positions[0], [5.0, 3.0, 7.0]);
        assert_eq!(m.positions[1], [6.0, 3.0, 7.0]);
        assert_eq!(m.positions[2], [5.0, 4.0, 7.0]);
        assert_eq!(m.indices, vec![0, 1, 2]);
    }

    #[test]
    fn transform_mirror_flips_winding() {
        let mut m = TriMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![],
            indices: vec![0, 1, 2],
        };
        // Mirror X: det = -1.
        let mx = [
            [-1.0, 0.0, 0.0, 0.0],
            [ 0.0, 1.0, 0.0, 0.0],
            [ 0.0, 0.0, 1.0, 0.0],
            [ 0.0, 0.0, 0.0, 1.0],
        ];
        m.transform(mx);
        assert_eq!(m.indices, vec![0, 2, 1]);
    }

    #[test]
    fn reverse_winding_swaps_indices() {
        let mut m = TriMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![],
            indices: vec![0, 1, 2],
        };
        m.reverse_winding();
        assert_eq!(m.indices, vec![0, 2, 1]);
    }

    #[test]
    fn smooth_normals_on_flat_quad_point_up() {
        let mut m = TriMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            normals: vec![],
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        m.compute_smooth_normals();
        assert_eq!(m.normals.len(), 4);
        for n in &m.normals {
            assert!((n[0].abs() + n[1].abs()) < 1e-5);
            assert!((n[2] - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn weld_collapses_duplicate_vertices() {
        let mut m = TriMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],   // duplicate of 0
                [1.0, 0.0, 0.0],   // duplicate of 1
                [1.0, 1.0, 0.0],
            ],
            normals: vec![],
            indices: vec![0, 1, 2, 3, 4, 5],
        };
        let removed = m.weld(1e-4);
        assert_eq!(removed, 2);
        assert_eq!(m.positions.len(), 4);
        assert_eq!(m.indices, vec![0, 1, 2, 0, 1, 3]);
    }

    #[test]
    fn weld_closed_box_yields_eight_unique_vertices() {
        // Build a per-face-unshared box (6 faces × 4 verts = 24 verts before weld).
        // After weld at 1e-4 the 8 unique box corners should remain.
        let c = [
            [0.0_f32, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        // 6 faces as quads, each independently listing its 4 corners.
        let faces = [
            [0, 1, 2, 3], // z=0
            [4, 5, 6, 7], // z=1
            [0, 1, 5, 4], // y=0
            [3, 2, 6, 7], // y=1
            [0, 3, 7, 4], // x=0
            [1, 2, 6, 5], // x=1
        ];
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        for f in &faces {
            let base = positions.len() as u32;
            for &v in f { positions.push(c[v]); }
            indices.extend([base, base + 1, base + 2]);
            indices.extend([base, base + 2, base + 3]);
        }
        let mut mesh = TriMesh { positions, normals: vec![], indices };
        assert_eq!(mesh.positions.len(), 24);
        mesh.weld(1e-4);
        assert_eq!(mesh.positions.len(), 8);
        // With shared vertices, χ = 8 − 18 + 12 = 2.
        let mut edges: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
        for t in 0..(mesh.indices.len() / 3) {
            for (a, b) in [
                (mesh.indices[t * 3], mesh.indices[t * 3 + 1]),
                (mesh.indices[t * 3 + 1], mesh.indices[t * 3 + 2]),
                (mesh.indices[t * 3 + 2], mesh.indices[t * 3]),
            ] {
                let k = if a < b { (a, b) } else { (b, a) };
                edges.insert(k);
            }
        }
        let chi = 8_i64 - edges.len() as i64 + (mesh.indices.len() / 3) as i64;
        assert_eq!(chi, 2);
    }
}
