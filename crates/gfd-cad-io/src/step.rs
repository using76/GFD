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
