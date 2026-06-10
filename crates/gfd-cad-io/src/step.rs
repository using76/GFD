//! Minimal STEP AP203-ish ASCII writer.
//!
//! Iteration 15 scope: emit a syntactically well-formed ISO-10303-21 file
//! containing `CARTESIAN_POINT` entities for every vertex in the arena.
//! Faces/edges are expressed as simple entity references for downstream
//! tools that only need point-cloud import. Full AP214 (Manifold_solid_brep
//! + advanced_face + ...) ships in a later iteration.
//!
//! Reader is NOT implemented — use `cad.import.brep` for roundtrip.

use std::fs;
use std::io::Write;
use std::path::Path;

use gfd_cad_topo::{Shape, ShapeArena, ShapeId};
use gfd_cad_tessel::{triangulate_polygon, TriMesh};

use crate::{IoError, IoResult};

pub fn write_step(path: &Path, arena: &ShapeArena, root: ShapeId) -> IoResult<()> {
    let mut buf = String::new();
    buf.push_str("ISO-10303-21;\n");
    buf.push_str("HEADER;\n");
    buf.push_str("FILE_DESCRIPTION(('GFD CAD export'),'2;1');\n");
    buf.push_str(&format!(
        "FILE_NAME('{}','{}',(''),(''),'','gfd-cad','');\n",
        path.file_name().and_then(|s| s.to_str()).unwrap_or(""),
        "2026-04-20T00:00:00"
    ));
    buf.push_str("FILE_SCHEMA(('CONFIG_CONTROL_DESIGN'));\n");
    buf.push_str("ENDSEC;\n");
    buf.push_str("DATA;\n");

    let mut ctx = StepCtx::default();
    walk(arena, root, &mut buf, &mut ctx);

    // Wrap all collected faces into a CLOSED_SHELL + MANIFOLD_SOLID_BREP so
    // the file tops out at a solid reference (required by AP214).
    if !ctx.face_entities.is_empty() {
        let shell_id = ctx.alloc();
        let refs: Vec<String> = ctx.face_entities.iter().map(|f| format!("#{}", f)).collect();
        buf.push_str(&format!("#{}=CLOSED_SHELL('',({}));\n", shell_id, refs.join(",")));
        let solid_id = ctx.alloc();
        buf.push_str(&format!("#{}=MANIFOLD_SOLID_BREP('',#{});\n", solid_id, shell_id));
    }

    buf.push_str("ENDSEC;\n");
    buf.push_str("END-ISO-10303-21;\n");

    let mut f = fs::File::create(path)?;
    f.write_all(buf.as_bytes())?;
    Ok(())
}

#[derive(Default)]
struct StepCtx {
    next_id: u32,
    /// Map from arena vertex ids to STEP entity ids so edges can back-reference.
    vertex_entity: std::collections::HashMap<u32, u32>,
    /// Entity ids of all emitted EDGE_CURVE entries (used to build EDGE_LOOP).
    edge_entities: Vec<u32>,
    /// Entity ids of all emitted ADVANCED_FACE entries.
    face_entities: Vec<u32>,
}

impl StepCtx {
    fn alloc(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }
}

fn walk(arena: &ShapeArena, id: ShapeId, buf: &mut String, ctx: &mut StepCtx) {
    let Ok(shape) = arena.get(id) else { return; };
    match shape {
        Shape::Vertex { point } => {
            if ctx.vertex_entity.contains_key(&id.0) { return; }
            let cp = ctx.alloc();
            buf.push_str(&format!(
                "#{}=CARTESIAN_POINT('',({:.6},{:.6},{:.6}));\n",
                cp, point.x, point.y, point.z
            ));
            let vp = ctx.alloc();
            buf.push_str(&format!("#{}=VERTEX_POINT('',#{});\n", vp, cp));
            ctx.vertex_entity.insert(id.0, vp);
        }
        Shape::Edge { vertices, .. } => {
            for v in vertices { walk(arena, *v, buf, ctx); }
            let va = ctx.vertex_entity.get(&vertices[0].0).copied();
            let vb = ctx.vertex_entity.get(&vertices[1].0).copied();
            if let (Some(a), Some(b)) = (va, vb) {
                let ec = ctx.alloc();
                buf.push_str(&format!("#{}=EDGE_CURVE('',#{},#{},$,.T.);\n", ec, a, b));
                ctx.edge_entities.push(ec);
            }
        }
        Shape::Wire { edges } => {
            for (e, _) in edges { walk(arena, *e, buf, ctx); }
        }
        Shape::Face { wires, surface, .. } => {
            for w in wires { walk(arena, *w, buf, ctx); }
            if !ctx.edge_entities.is_empty() {
                // Emit an AXIS2_PLACEMENT_3D + surface geometry entity so the
                // ADVANCED_FACE can reference it. For iter 28 only PLANE is
                // emitted; cylindrical/spherical surfaces fall back to $ (none).
                let axis_origin = ctx.alloc();
                buf.push_str(&format!(
                    "#{}=CARTESIAN_POINT('',(0.,0.,0.));\n", axis_origin));
                let axis_z = ctx.alloc();
                buf.push_str(&format!("#{}=DIRECTION('',(0.,0.,1.));\n", axis_z));
                let axis_x = ctx.alloc();
                buf.push_str(&format!("#{}=DIRECTION('',(1.,0.,0.));\n", axis_x));
                let placement = ctx.alloc();
                buf.push_str(&format!(
                    "#{}=AXIS2_PLACEMENT_3D('',#{},#{},#{});\n",
                    placement, axis_origin, axis_z, axis_x));
                let surface_ref = match surface {
                    gfd_cad_topo::SurfaceGeom::Plane(_) => {
                        let plane_id = ctx.alloc();
                        buf.push_str(&format!("#{}=PLANE('',#{});\n", plane_id, placement));
                        format!("#{}", plane_id)
                    }
                    gfd_cad_topo::SurfaceGeom::Cylinder(c) => {
                        let cs_id = ctx.alloc();
                        buf.push_str(&format!(
                            "#{}=CYLINDRICAL_SURFACE('',#{},{:.6});\n",
                            cs_id, placement, c.radius));
                        format!("#{}", cs_id)
                    }
                    gfd_cad_topo::SurfaceGeom::Sphere(s) => {
                        let ss_id = ctx.alloc();
                        buf.push_str(&format!(
                            "#{}=SPHERICAL_SURFACE('',#{},{:.6});\n",
                            ss_id, placement, s.radius));
                        format!("#{}", ss_id)
                    }
                    _ => "$".to_string(),
                };
                let loop_id = ctx.alloc();
                let refs: Vec<String> = ctx.edge_entities.iter().map(|e| format!("#{}", e)).collect();
                buf.push_str(&format!("#{}=EDGE_LOOP('',({}));\n", loop_id, refs.join(",")));
                let bound_id = ctx.alloc();
                buf.push_str(&format!("#{}=FACE_OUTER_BOUND('',#{},.T.);\n", bound_id, loop_id));
                let af = ctx.alloc();
                buf.push_str(&format!("#{}=ADVANCED_FACE('',(#{}),{},.T.);\n", af, bound_id, surface_ref));
                ctx.face_entities.push(af);
                ctx.edge_entities.clear();
            }
        }
        Shape::Shell { faces } => {
            for (f, _) in faces { walk(arena, *f, buf, ctx); }
        }
        Shape::Solid { shells } => {
            for s in shells { walk(arena, *s, buf, ctx); }
        }
        Shape::Compound { children } => {
            for c in children { walk(arena, *c, buf, ctx); }
        }
    }
}

/// Minimal STEP reader — picks up `CARTESIAN_POINT((x,y,z))` entries and
/// returns them as a Vec. Does NOT reconstruct topology.
pub fn read_step_points(path: &Path) -> IoResult<Vec<(f64, f64, f64)>> {
    let text = fs::read_to_string(path)?;
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(start) = line.find("CARTESIAN_POINT('',(") {
            let rest = &line[start + "CARTESIAN_POINT('',(".len()..];
            let end = match rest.find(')') { Some(e) => e, None => continue };
            let nums: Vec<&str> = rest[..end].split(',').collect();
            if nums.len() == 3 {
                let x = nums[0].trim().parse::<f64>().unwrap_or(0.0);
                let y = nums[1].trim().parse::<f64>().unwrap_or(0.0);
                let z = nums[2].trim().parse::<f64>().unwrap_or(0.0);
                out.push((x, y, z));
            }
        }
    }
    Ok(out)
}

/// All `#nnn` entity references inside an argument string.
fn extract_refs(s: &str) -> Vec<u32> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'#' {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 {
                if let Ok(n) = s[i + 1..j].parse::<u32>() {
                    out.push(n);
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// All numeric literals inside an argument string (STEP reals/ints).
fn extract_floats(s: &str) -> Vec<f64> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'-' || c == b'+' || c == b'.' || c.is_ascii_digit() {
            let start = i;
            let mut j = i;
            while j < b.len() {
                let cj = b[j];
                if cj.is_ascii_digit() || matches!(cj, b'.' | b'-' | b'+' | b'e' | b'E') {
                    j += 1;
                } else {
                    break;
                }
            }
            if let Ok(f) = s[start..j].parse::<f64>() {
                out.push(f);
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

/// Triangulate a 3D planar polygon. Projects onto its plane (Newell normal) and
/// ear-clips in 2D so NON-convex faces (L-shapes, brackets) triangulate
/// correctly; falls back to a fan for degenerate/failed cases.
fn triangulate_face(poly: &[[f32; 3]]) -> Vec<[u32; 3]> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    if n == 3 {
        return vec![[0, 1, 2]];
    }
    let fan = || -> Vec<[u32; 3]> { (1..n - 1).map(|i| [0u32, i as u32, i as u32 + 1]).collect() };
    // Newell normal.
    let (mut nx, mut ny, mut nz) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        nx += ((a[1] - b[1]) * (a[2] + b[2])) as f64;
        ny += ((a[2] - b[2]) * (a[0] + b[0])) as f64;
        nz += ((a[0] - b[0]) * (a[1] + b[1])) as f64;
    }
    let nlen = (nx * nx + ny * ny + nz * nz).sqrt();
    if nlen < 1e-20 {
        return fan();
    }
    let nrm = [nx / nlen, ny / nlen, nz / nlen];
    // In-plane basis (u, v).
    let t = if nrm[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let mut u = [
        t[1] * nrm[2] - t[2] * nrm[1],
        t[2] * nrm[0] - t[0] * nrm[2],
        t[0] * nrm[1] - t[1] * nrm[0],
    ];
    let ul = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
    if ul < 1e-20 {
        return fan();
    }
    for c in &mut u {
        *c /= ul;
    }
    let v = [
        nrm[1] * u[2] - nrm[2] * u[1],
        nrm[2] * u[0] - nrm[0] * u[2],
        nrm[0] * u[1] - nrm[1] * u[0],
    ];
    let mut pts2: Vec<(f64, f64)> = poly
        .iter()
        .map(|p| {
            let pd = [p[0] as f64, p[1] as f64, p[2] as f64];
            (
                pd[0] * u[0] + pd[1] * u[1] + pd[2] * u[2],
                pd[0] * v[0] + pd[1] * v[1] + pd[2] * v[2],
            )
        })
        .collect();
    // Ear clipping expects CCW; flip if the projected loop is CW.
    let mut area = 0.0;
    for i in 0..n {
        let (x0, y0) = pts2[i];
        let (x1, y1) = pts2[(i + 1) % n];
        area += x0 * y1 - x1 * y0;
    }
    if area < 0.0 {
        for p in &mut pts2 {
            p.1 = -p.1;
        }
    }
    let tris = triangulate_polygon(&pts2);
    if tris.len() == n - 2 {
        tris
    } else {
        fan()
    }
}

/// A curved underlying surface a STEP face references — used to refine the
/// reconstructed polygon back onto the true geometry.
enum CurvedSurf {
    /// (center, radius)
    Sphere([f64; 3], f64),
    /// (axis origin, unit axis direction, radius)
    Cylinder([f64; 3], [f64; 3], f64),
}

/// Project a point onto the curved surface (radially). Degenerate points (on
/// the sphere center / cylinder axis) are returned unchanged.
fn project_to_surf(p: [f32; 3], s: &CurvedSurf) -> [f32; 3] {
    let pd = [p[0] as f64, p[1] as f64, p[2] as f64];
    match s {
        CurvedSurf::Sphere(c, r) => {
            let w = [pd[0] - c[0], pd[1] - c[1], pd[2] - c[2]];
            let len = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
            if len < 1e-12 {
                return p;
            }
            let k = r / len;
            [(c[0] + w[0] * k) as f32, (c[1] + w[1] * k) as f32, (c[2] + w[2] * k) as f32]
        }
        CurvedSurf::Cylinder(o, d, r) => {
            let w = [pd[0] - o[0], pd[1] - o[1], pd[2] - o[2]];
            let ax = w[0] * d[0] + w[1] * d[1] + w[2] * d[2];
            let rad = [w[0] - ax * d[0], w[1] - ax * d[1], w[2] - ax * d[2]];
            let len = (rad[0] * rad[0] + rad[1] * rad[1] + rad[2] * rad[2]).sqrt();
            if len < 1e-12 {
                return p;
            }
            let k = r / len;
            [
                (o[0] + ax * d[0] + rad[0] * k) as f32,
                (o[1] + ax * d[1] + rad[1] * k) as f32,
                (o[2] + ax * d[2] + rad[2] * k) as f32,
            ]
        }
    }
}

/// One level of midpoint subdivision on a local triangle soup (1 tri → 4).
/// No welding — duplicate midpoints are fine for rendering.
fn subdivide_local(positions: &mut Vec<[f32; 3]>, tris: &mut Vec<[u32; 3]>) {
    let mid = |a: [f32; 3], b: [f32; 3]| [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0];
    let mut out = Vec::with_capacity(tris.len() * 4);
    for t in tris.iter() {
        let (pa, pb, pc) = (
            positions[t[0] as usize],
            positions[t[1] as usize],
            positions[t[2] as usize],
        );
        let iab = positions.len() as u32;
        positions.push(mid(pa, pb));
        let ibc = positions.len() as u32;
        positions.push(mid(pb, pc));
        let ica = positions.len() as u32;
        positions.push(mid(pc, pa));
        out.push([t[0], iab, ica]);
        out.push([iab, t[1], ibc]);
        out.push([ica, ibc, t[2]]);
        out.push([iab, ibc, ica]);
    }
    *tris = out;
}

/// STEP reader that reconstructs polygon faces into a `TriMesh`.
///
/// Walks ADVANCED_FACE → FACE_OUTER_BOUND → EDGE_LOOP → EDGE_CURVE (or
/// ORIENTED_EDGE) → VERTEX_POINT → CARTESIAN_POINT, collecting each loop's
/// ordered vertices and ear-clipping the polygon. Faces referencing a
/// SPHERICAL_SURFACE / CYLINDRICAL_SURFACE are refined: the triangulation is
/// midpoint-subdivided twice and every vertex is projected back onto the true
/// curved surface, so cylinders and spheres import round instead of faceted.
pub fn read_step_trimesh(path: &Path) -> IoResult<TriMesh> {
    use std::collections::HashMap;
    let text = fs::read_to_string(path)?;

    let mut cps: HashMap<u32, [f32; 3]> = HashMap::new();
    let mut vps: HashMap<u32, u32> = HashMap::new(); // vertex_point -> cartesian_point
    let mut edges: HashMap<u32, (u32, u32)> = HashMap::new(); // edge_curve -> (vp_a, vp_b)
    let mut oriented: HashMap<u32, u32> = HashMap::new(); // oriented_edge -> edge_curve
    let mut loops: HashMap<u32, Vec<u32>> = HashMap::new(); // edge_loop -> elems
    let mut bounds: HashMap<u32, u32> = HashMap::new(); // face_bound -> loop
    let mut faces: Vec<Vec<u32>> = Vec::new(); // advanced_face -> refs
    let mut directions: HashMap<u32, [f64; 3]> = HashMap::new(); // direction -> unit vec
    let mut placements: HashMap<u32, (u32, u32)> = HashMap::new(); // axis2_placement -> (point, z-dir)
    let mut surf_sph: HashMap<u32, (u32, f64)> = HashMap::new(); // sphere -> (placement, radius)
    let mut surf_cyl: HashMap<u32, (u32, f64)> = HashMap::new(); // cylinder -> (placement, radius)

    for record in text.split(';') {
        let record = record.trim();
        if !record.starts_with('#') {
            continue;
        }
        let Some(eq) = record.find('=') else { continue };
        let Ok(id) = record[1..eq].trim().parse::<u32>() else { continue };
        let rhs = record[eq + 1..].trim();
        let Some(paren) = rhs.find('(') else { continue };
        let typ = rhs[..paren].trim();
        let args = &rhs[paren..];
        match typ {
            "CARTESIAN_POINT" => {
                let f = extract_floats(args);
                if f.len() >= 3 {
                    cps.insert(id, [f[0] as f32, f[1] as f32, f[2] as f32]);
                }
            }
            "VERTEX_POINT" => {
                if let Some(&cp) = extract_refs(args).first() {
                    vps.insert(id, cp);
                }
            }
            "EDGE_CURVE" => {
                let r = extract_refs(args);
                if r.len() >= 2 {
                    edges.insert(id, (r[0], r[1]));
                }
            }
            "ORIENTED_EDGE" => {
                if let Some(&e) = extract_refs(args).first() {
                    oriented.insert(id, e);
                }
            }
            "EDGE_LOOP" => {
                loops.insert(id, extract_refs(args));
            }
            "FACE_OUTER_BOUND" | "FACE_BOUND" => {
                if let Some(&l) = extract_refs(args).first() {
                    bounds.insert(id, l);
                }
            }
            "ADVANCED_FACE" | "FACE_SURFACE" => {
                faces.push(extract_refs(args));
            }
            "DIRECTION" => {
                let f = extract_floats(args);
                if f.len() >= 3 {
                    let len = (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt();
                    if len > 1e-12 {
                        directions.insert(id, [f[0] / len, f[1] / len, f[2] / len]);
                    }
                }
            }
            "AXIS2_PLACEMENT_3D" => {
                let r = extract_refs(args);
                if r.len() >= 2 {
                    placements.insert(id, (r[0], r[1]));
                }
            }
            "SPHERICAL_SURFACE" => {
                let r = extract_refs(args);
                // extract_floats also picks up the "#nnn" digits — radius is last.
                if let (Some(&pl), Some(&rad)) = (r.first(), extract_floats(args).last()) {
                    surf_sph.insert(id, (pl, rad));
                }
            }
            "CYLINDRICAL_SURFACE" => {
                let r = extract_refs(args);
                if let (Some(&pl), Some(&rad)) = (r.first(), extract_floats(args).last()) {
                    surf_cyl.insert(id, (pl, rad));
                }
            }
            _ => {}
        }
    }

    // Resolve the curved surface a face references (if any) to concrete geometry.
    let resolve_surf = |frefs: &[u32]| -> Option<CurvedSurf> {
        for r in frefs {
            if let Some(&(pl, rad)) = surf_sph.get(r) {
                let &(cp_ref, _) = placements.get(&pl)?;
                let c = cps.get(&cp_ref)?;
                return Some(CurvedSurf::Sphere([c[0] as f64, c[1] as f64, c[2] as f64], rad));
            }
            if let Some(&(pl, rad)) = surf_cyl.get(r) {
                let &(cp_ref, zd_ref) = placements.get(&pl)?;
                let o = cps.get(&cp_ref)?;
                let d = directions.get(&zd_ref)?;
                return Some(CurvedSurf::Cylinder([o[0] as f64, o[1] as f64, o[2] as f64], *d, rad));
            }
        }
        None
    };

    let dist2 = |a: &[f32; 3], b: &[f32; 3]| {
        let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
        dx * dx + dy * dy + dz * dz
    };
    let mut mesh = TriMesh::default();
    for frefs in &faces {
        let curved = resolve_surf(frefs);
        for br in frefs {
            let Some(&loop_id) = bounds.get(br) else { continue };
            let Some(elems) = loops.get(&loop_id) else { continue };
            let mut poly: Vec<[f32; 3]> = Vec::new();
            for el in elems {
                let edge_id = if edges.contains_key(el) {
                    *el
                } else if let Some(&e) = oriented.get(el) {
                    e
                } else {
                    continue;
                };
                let Some(&(va, _vb)) = edges.get(&edge_id) else { continue };
                if let Some(&cp) = vps.get(&va) {
                    if let Some(&p) = cps.get(&cp) {
                        if poly.last().map_or(true, |q| dist2(q, &p) > 1e-12) {
                            poly.push(p);
                        }
                    }
                }
            }
            if poly.len() >= 3 && dist2(&poly[0], poly.last().unwrap()) < 1e-12 {
                poly.pop();
            }
            if poly.len() < 3 {
                continue;
            }
            let mut local_pos = poly.clone();
            let mut tris = triangulate_face(&local_pos);
            if let Some(surf) = &curved {
                // Refine the flat triangulation back onto the curved surface.
                for _ in 0..2 {
                    subdivide_local(&mut local_pos, &mut tris);
                }
                for p in &mut local_pos {
                    *p = project_to_surf(*p, surf);
                }
            }
            let base = mesh.positions.len() as u32;
            mesh.positions.extend_from_slice(&local_pos);
            for tri in tris {
                mesh.indices.push(base + tri[0]);
                mesh.indices.push(base + tri[1]);
                mesh.indices.push(base + tri[2]);
            }
        }
    }
    if mesh.positions.is_empty() {
        return Err(IoError::Parse("STEP file has no reconstructable planar faces".into()));
    }
    Ok(mesh)
}

/// Summary of a STEP file's contents — counts of each entity kind we know
/// how to recognise. Used for import sanity checks and UI preview.
#[derive(Debug, Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct StepSummary {
    pub cartesian_points: usize,
    pub vertex_points: usize,
    pub edge_curves: usize,
    pub edge_loops: usize,
    pub face_outer_bounds: usize,
    pub advanced_faces: usize,
    pub closed_shells: usize,
    pub manifold_solid_breps: usize,
    pub axis2_placements: usize,
    pub planes: usize,
    pub cylindrical_surfaces: usize,
    pub spherical_surfaces: usize,
}

/// Scan a STEP file and count how many of each recognised entity it contains.
/// No topology is reconstructed — this is purely descriptive.
pub fn summarise_step(path: &Path) -> IoResult<StepSummary> {
    let text = fs::read_to_string(path)?;
    let mut s = StepSummary::default();
    for line in text.lines() {
        if line.contains("CARTESIAN_POINT") { s.cartesian_points += 1; }
        if line.contains("VERTEX_POINT") { s.vertex_points += 1; }
        if line.contains("EDGE_CURVE") { s.edge_curves += 1; }
        if line.contains("EDGE_LOOP") { s.edge_loops += 1; }
        if line.contains("FACE_OUTER_BOUND") { s.face_outer_bounds += 1; }
        if line.contains("ADVANCED_FACE") { s.advanced_faces += 1; }
        if line.contains("CLOSED_SHELL") { s.closed_shells += 1; }
        if line.contains("MANIFOLD_SOLID_BREP") { s.manifold_solid_breps += 1; }
        if line.contains("AXIS2_PLACEMENT_3D") { s.axis2_placements += 1; }
        if line.contains("PLANE(") { s.planes += 1; }
        if line.contains("CYLINDRICAL_SURFACE") { s.cylindrical_surfaces += 1; }
        if line.contains("SPHERICAL_SURFACE") { s.spherical_surfaces += 1; }
    }
    Ok(s)
}

pub fn write_step_generic(path: &Path, arena: &ShapeArena, root: ShapeId) -> IoResult<()> {
    write_step(path, arena, root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gfd_cad_geom::Point3;

    #[test]
    fn step_exports_edges_and_faces_for_nontrivial_shape() {
        use gfd_cad_geom::{curve::Line, surface::Plane, Direction3, Point3};
        use gfd_cad_topo::{shape::{CurveGeom, SurfaceGeom}, Orientation};
        let mut arena = ShapeArena::new();
        // 3 vertices + 3 edges + 1 wire + 1 face = a triangle.
        let v = [
            arena.push(Shape::vertex(Point3::new(0.0, 0.0, 0.0))),
            arena.push(Shape::vertex(Point3::new(1.0, 0.0, 0.0))),
            arena.push(Shape::vertex(Point3::new(0.0, 1.0, 0.0))),
        ];
        let mut edges = Vec::new();
        for i in 0..3 {
            let j = (i + 1) % 3;
            let pi = match arena.get(v[i]).unwrap() { Shape::Vertex { point } => *point, _ => unreachable!() };
            let pj = match arena.get(v[j]).unwrap() { Shape::Vertex { point } => *point, _ => unreachable!() };
            let line = Line::from_points(pi, pj).unwrap();
            let e = arena.push(Shape::Edge { curve: CurveGeom::Line(line), vertices: [v[i], v[j]], orient: Orientation::Forward });
            edges.push((e, Orientation::Forward));
        }
        let wire = arena.push(Shape::Wire { edges });
        let face = arena.push(Shape::Face {
            surface: SurfaceGeom::Plane(Plane::new(Point3::ORIGIN, Direction3::Z, Direction3::X)),
            wires: vec![wire],
            orient: Orientation::Forward,
        });
        let path = std::env::temp_dir().join(format!("gfd_step_face_{}.stp",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        write_step(&path, &arena, face).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("CARTESIAN_POINT"), "no CARTESIAN_POINT");
        assert!(text.contains("VERTEX_POINT"), "no VERTEX_POINT");
        assert!(text.contains("EDGE_CURVE"), "no EDGE_CURVE");
        assert!(text.contains("EDGE_LOOP"), "no EDGE_LOOP");
        assert!(text.contains("FACE_OUTER_BOUND"), "no FACE_OUTER_BOUND");
        assert!(text.contains("ADVANCED_FACE"), "no ADVANCED_FACE");
        assert!(text.contains("CLOSED_SHELL"), "no CLOSED_SHELL");
        assert!(text.contains("MANIFOLD_SOLID_BREP"), "no MANIFOLD_SOLID_BREP");
        assert!(text.contains("AXIS2_PLACEMENT_3D"), "no AXIS2_PLACEMENT_3D");
        assert!(text.contains("PLANE("), "no PLANE entity");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn triangulate_face_handles_a_nonconvex_l_shape() {
        // CCW L-shape (z=0): a fan would spill outside; ear-clipping must not.
        let l: Vec<[f32; 3]> = vec![
            [0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 1.0, 0.0],
            [1.0, 1.0, 0.0], [1.0, 2.0, 0.0], [0.0, 2.0, 0.0],
        ];
        let tris = triangulate_face(&l);
        assert_eq!(tris.len(), 4, "6-gon → 4 triangles");
        // Total triangle area must equal the L-shape area (3.0): proof no overlap/gap.
        let area = |a: [f32; 3], b: [f32; 3], c: [f32; 3]| {
            0.5 * (((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])) as f64).abs()
        };
        let total: f64 = tris.iter().map(|t| area(l[t[0] as usize], l[t[1] as usize], l[t[2] as usize])).sum();
        assert!((total - 3.0).abs() < 1e-6, "triangulated area {} != L-shape area 3.0", total);
    }

    /// Minimal STEP body around handcrafted entity records.
    fn step_doc(entities: &str) -> String {
        format!("ISO-10303-21;\nDATA;\n{}\nENDSEC;\nEND-ISO-10303-21;\n", entities)
    }

    #[test]
    fn read_step_trimesh_refines_a_spherical_face_onto_the_sphere() {
        // A triangle whose corners lie on the unit sphere, referencing
        // SPHERICAL_SURFACE(r=1) — vertices must be subdivided + projected.
        let entities = "\
#1=CARTESIAN_POINT('',(0.,0.,0.));
#2=DIRECTION('',(0.,0.,1.));
#3=DIRECTION('',(1.,0.,0.));
#4=AXIS2_PLACEMENT_3D('',#1,#2,#3);
#5=SPHERICAL_SURFACE('',#4,1.0);
#10=CARTESIAN_POINT('',(1.,0.,0.));
#11=VERTEX_POINT('',#10);
#12=CARTESIAN_POINT('',(0.,1.,0.));
#13=VERTEX_POINT('',#12);
#14=CARTESIAN_POINT('',(0.,0.,1.));
#15=VERTEX_POINT('',#14);
#20=EDGE_CURVE('',#11,#13,$,.T.);
#21=EDGE_CURVE('',#13,#15,$,.T.);
#22=EDGE_CURVE('',#15,#11,$,.T.);
#23=EDGE_LOOP('',(#20,#21,#22));
#24=FACE_OUTER_BOUND('',#23,.T.);
#25=ADVANCED_FACE('',(#24),#5,.T.);";
        let path = std::env::temp_dir().join(format!("gfd_step_sph_{}.stp",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::write(&path, step_doc(entities)).unwrap();
        let mesh = read_step_trimesh(&path).unwrap();
        let _ = fs::remove_file(&path);
        // 1 flat triangle, 2 subdivision levels → 16 triangles.
        assert_eq!(mesh.indices.len() / 3, 16, "expected 16 refined triangles");
        for p in &mesh.positions {
            let r = ((p[0] as f64).powi(2) + (p[1] as f64).powi(2) + (p[2] as f64).powi(2)).sqrt();
            assert!((r - 1.0).abs() < 1e-4, "vertex {:?} not on the unit sphere (r={})", p, r);
        }
    }

    #[test]
    fn read_step_trimesh_refines_a_cylindrical_face_onto_the_cylinder() {
        // A quarter-cylinder patch (axis Z, r=1): vertices at 0° and 90°,
        // z ∈ {0, 1}. The flat quad's interior must be pushed out to radius 1.
        let entities = "\
#1=CARTESIAN_POINT('',(0.,0.,0.));
#2=DIRECTION('',(0.,0.,1.));
#3=DIRECTION('',(1.,0.,0.));
#4=AXIS2_PLACEMENT_3D('',#1,#2,#3);
#5=CYLINDRICAL_SURFACE('',#4,1.0);
#10=CARTESIAN_POINT('',(1.,0.,0.));
#11=VERTEX_POINT('',#10);
#12=CARTESIAN_POINT('',(0.,1.,0.));
#13=VERTEX_POINT('',#12);
#14=CARTESIAN_POINT('',(0.,1.,1.));
#15=VERTEX_POINT('',#14);
#16=CARTESIAN_POINT('',(1.,0.,1.));
#17=VERTEX_POINT('',#16);
#20=EDGE_CURVE('',#11,#13,$,.T.);
#21=EDGE_CURVE('',#13,#15,$,.T.);
#22=EDGE_CURVE('',#15,#17,$,.T.);
#23=EDGE_CURVE('',#17,#11,$,.T.);
#24=EDGE_LOOP('',(#20,#21,#22,#23));
#25=FACE_OUTER_BOUND('',#24,.T.);
#26=ADVANCED_FACE('',(#25),#5,.T.);";
        let path = std::env::temp_dir().join(format!("gfd_step_cyl_{}.stp",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::write(&path, step_doc(entities)).unwrap();
        let mesh = read_step_trimesh(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert!(mesh.indices.len() / 3 > 4, "expected refined triangles, got {}", mesh.indices.len() / 3);
        for p in &mesh.positions {
            let r = ((p[0] as f64).powi(2) + (p[1] as f64).powi(2)).sqrt();
            assert!((r - 1.0).abs() < 1e-4, "vertex {:?} not on the unit cylinder (r={})", p, r);
            assert!(p[2] >= -1e-4 && p[2] <= 1.0 + 1e-4, "vertex z out of patch range");
        }
    }

    #[test]
    fn read_step_trimesh_reconstructs_a_face() {
        use gfd_cad_geom::{curve::Line, surface::Plane, Direction3, Point3};
        use gfd_cad_topo::{shape::{CurveGeom, SurfaceGeom}, Orientation};
        let mut arena = ShapeArena::new();
        let pts = [Point3::new(0.0, 0.0, 0.0), Point3::new(2.0, 0.0, 0.0), Point3::new(0.0, 3.0, 0.0)];
        let v = [
            arena.push(Shape::vertex(pts[0])),
            arena.push(Shape::vertex(pts[1])),
            arena.push(Shape::vertex(pts[2])),
        ];
        let mut edges = Vec::new();
        for i in 0..3 {
            let j = (i + 1) % 3;
            let line = Line::from_points(pts[i], pts[j]).unwrap();
            let e = arena.push(Shape::Edge { curve: CurveGeom::Line(line), vertices: [v[i], v[j]], orient: Orientation::Forward });
            edges.push((e, Orientation::Forward));
        }
        let wire = arena.push(Shape::Wire { edges });
        let face = arena.push(Shape::Face {
            surface: SurfaceGeom::Plane(Plane::new(Point3::ORIGIN, Direction3::Z, Direction3::X)),
            wires: vec![wire],
            orient: Orientation::Forward,
        });
        let path = std::env::temp_dir().join(format!("gfd_step_tri_{}.stp",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        write_step(&path, &arena, face).unwrap();
        let mesh = read_step_trimesh(&path).unwrap();
        let _ = fs::remove_file(&path);
        // A single triangular face → 3 positions, 1 triangle.
        assert_eq!(mesh.positions.len(), 3, "expected 3 reconstructed vertices");
        assert_eq!(mesh.indices.len(), 3, "expected one triangle");
        // Every reconstructed point must be one of the original triangle corners.
        for p in &mesh.positions {
            let matched = pts.iter().any(|q| {
                (q.x as f32 - p[0]).abs() < 1e-4 && (q.y as f32 - p[1]).abs() < 1e-4 && (q.z as f32 - p[2]).abs() < 1e-4
            });
            assert!(matched, "reconstructed point {:?} not on the original face", p);
        }
    }

    #[test]
    fn step_summary_counts_entities() {
        use gfd_cad_geom::{curve::Line, surface::Plane, Direction3, Point3};
        use gfd_cad_topo::{shape::{CurveGeom, SurfaceGeom}, Orientation};
        let mut arena = ShapeArena::new();
        let v = [
            arena.push(Shape::vertex(Point3::new(0.0, 0.0, 0.0))),
            arena.push(Shape::vertex(Point3::new(1.0, 0.0, 0.0))),
            arena.push(Shape::vertex(Point3::new(0.0, 1.0, 0.0))),
        ];
        let mut edges = Vec::new();
        for i in 0..3 {
            let j = (i + 1) % 3;
            let pi = match arena.get(v[i]).unwrap() { Shape::Vertex { point } => *point, _ => unreachable!() };
            let pj = match arena.get(v[j]).unwrap() { Shape::Vertex { point } => *point, _ => unreachable!() };
            let line = Line::from_points(pi, pj).unwrap();
            let e = arena.push(Shape::Edge { curve: CurveGeom::Line(line), vertices: [v[i], v[j]], orient: Orientation::Forward });
            edges.push((e, Orientation::Forward));
        }
        let wire = arena.push(Shape::Wire { edges });
        let face = arena.push(Shape::Face {
            surface: SurfaceGeom::Plane(Plane::new(Point3::ORIGIN, Direction3::Z, Direction3::X)),
            wires: vec![wire],
            orient: Orientation::Forward,
        });
        let path = std::env::temp_dir().join(format!("gfd_step_summary_{}.stp",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        write_step(&path, &arena, face).unwrap();
        let s = summarise_step(&path).unwrap();
        assert!(s.cartesian_points >= 3);
        assert!(s.edge_curves >= 3);
        assert!(s.advanced_faces >= 1);
        assert!(s.manifold_solid_breps >= 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn step_roundtrip_single_vertex() {
        let mut arena = ShapeArena::new();
        let v = arena.push(Shape::vertex(Point3::new(1.5, 2.5, 3.5)));
        let path = std::env::temp_dir().join(format!("gfd_step_test_{}.stp",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        write_step(&path, &arena, v).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("ISO-10303-21;"));
        assert!(text.contains("CARTESIAN_POINT"));
        assert!(text.contains("1.500000"));
        let pts = read_step_points(&path).unwrap();
        assert_eq!(pts.len(), 1);
        assert!((pts[0].0 - 1.5).abs() < 1e-9);
        let _ = fs::remove_file(&path);
    }
}
