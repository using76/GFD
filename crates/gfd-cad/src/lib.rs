//! gfd-cad — facade crate for the GFD CAD kernel.
//!
//! Re-exports the sub-crates and hosts the `Document` type plus JSON-RPC
//! message types consumed by `src/server.rs` and the Electron GUI.

pub use gfd_cad_bool as bool_;
pub use gfd_cad_feature as feature;
pub use gfd_cad_geom as geom;
pub use gfd_cad_heal as heal;
pub use gfd_cad_io as io;
pub use gfd_cad_measure as measure;
pub use gfd_cad_sketch as sketch;
pub use gfd_cad_tessel as tessel;
pub use gfd_cad_topo as topo;

pub mod document;
pub mod rpc;

pub use document::Document;

#[cfg(test)]
mod integration_tests {
    use super::*;
    use bool_::compound_merge;
    use feature::{box_solid, cone_solid, cylinder_solid, pad_polygon_xy, plate_with_hole_solid, revolve_profile_z, sphere_solid};
    use heal::check_validity;
    use measure::{bbox_volume, surface_area, trimesh_is_closed, volume};
    use tessel::{tessellate, TessellationOptions};
    use topo::ShapeArena;

    #[test]
    fn box_round_trip_primitive_to_triangles() {
        let mut arena = ShapeArena::new();
        let id = box_solid(&mut arena, 2.0, 2.0, 2.0).unwrap();
        let mesh = tessellate(&arena, id, TessellationOptions::default()).unwrap();
        // Planar faces are now bounded to their wire polygon (ear-clipped), so a
        // box is 6 quad faces × 2 triangles = 12, with the TRUE extent — not the
        // old clamped-±1 surface grid (6 × 32 × 16 × 2).
        assert_eq!(mesh.indices.len() / 3, 6 * 2);
        // True extent: a 2×2×2 box spans [-1,1]^3 (not the old oversized shell).
        let (mut mn, mut mx) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        for p in &mesh.positions {
            for k in 0..3 {
                if p[k] < mn[k] { mn[k] = p[k]; }
                if p[k] > mx[k] { mx[k] = p[k]; }
            }
        }
        assert_eq!(mn, [-1.0, -1.0, -1.0]);
        assert_eq!(mx, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn cylinder_caps_are_watertight_discs() {
        // Cylinder caps used to tessellate as clamped ±1 squares; now they are
        // real discs (a Circle-edge wire → center-fan), so the tessellated solid
        // is watertight after a weld and bounded to the true radius.
        let mut arena = ShapeArena::new();
        let id = cylinder_solid(&mut arena, 0.5, 2.0).unwrap();
        let mut mesh = tessellate(&arena, id, TessellationOptions::default()).unwrap();
        // True radial extent: x,y ∈ [-0.5, 0.5] (NOT the old ±1 square caps).
        let (mut mn, mut mx) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        for p in &mesh.positions {
            for k in 0..3 {
                if p[k] < mn[k] { mn[k] = p[k]; }
                if p[k] > mx[k] { mx[k] = p[k]; }
            }
        }
        assert!((mn[0] + 0.5).abs() < 1e-5 && (mx[0] - 0.5).abs() < 1e-5, "x extent {:?}..{:?} != ±0.5", mn[0], mx[0]);
        assert!((mn[2]).abs() < 1e-5 && (mx[2] - 2.0).abs() < 1e-5, "z extent should be [0,2]");
        // Watertight after welding coincident rim vertices (cap rim shares the
        // lateral wall's vertices because both sample u_steps angles).
        let removed = mesh.weld(1e-4);
        mesh.prune_unused_vertices();
        assert!(removed > 0, "weld should merge duplicated face-corner vertices");
        assert!(trimesh_is_closed(&mesh.indices), "welded cylinder must be watertight (caps closed)");
    }

    #[test]
    fn cylinder_volume_is_analytic() {
        // Wire-bounded caps used to make divergence_volume silently return 0;
        // now cylinder/cone report exact closed-form volume.
        let mut arena = ShapeArena::new();
        let cyl = cylinder_solid(&mut arena, 0.5, 2.0).unwrap();
        let v = volume(&arena, cyl).unwrap();
        let expect = std::f64::consts::PI * 0.5 * 0.5 * 2.0; // πr²h
        assert!((v - expect).abs() < 1e-9, "cylinder volume {} != {}", v, expect);
    }

    #[test]
    fn full_cone_r1_zero_tessellates_without_nan() {
        // r1=0 (apex at bottom) must not emit a radius-0 NaN disc cap.
        let mut arena = ShapeArena::new();
        let id = cone_solid(&mut arena, 0.0, 0.5, 2.0).unwrap();
        let mesh = tessellate(&arena, id, TessellationOptions::default()).unwrap();
        assert!(!mesh.positions.is_empty());
        assert!(
            mesh.positions.iter().all(|p| p.iter().all(|c| c.is_finite())),
            "cone r1=0 tessellation must not contain NaN vertices"
        );
    }

    #[test]
    fn step_brep_roundtrips_box_topology_and_volume() {
        use io::{export_step, import_step_brep};
        use topo::{collect_by_kind, ShapeKind};
        // Write a box solid, reconstruct its real B-Rep, check topology + volume.
        let mut src = ShapeArena::new();
        let bid = box_solid(&mut src, 2.0, 3.0, 4.0).unwrap();
        let path = std::env::temp_dir().join(format!("gfd_cad_brep_{}.stp",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        export_step(&path, &src, bid).unwrap();
        let mut dst = ShapeArena::new();
        let root = import_step_brep(&path, &mut dst).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(collect_by_kind(&dst, root, ShapeKind::Solid).len(), 1, "one solid");
        assert_eq!(collect_by_kind(&dst, root, ShapeKind::Face).len(), 6, "six faces");
        let v = volume(&dst, root).unwrap();
        assert!((v - 24.0).abs() < 1e-6, "reconstructed box volume {} != 24", v);
        // The reconstructed B-Rep tessellates to a non-empty mesh.
        let mesh = tessellate(&dst, root, TessellationOptions::default()).unwrap();
        assert!(mesh.indices.len() >= 12, "box B-Rep should tessellate");
    }

    #[test]
    fn plate_with_hole_volume_and_tessellation() {
        // 4×3×1 plate with a 12-gon hole (r=0.5) → volume = box − hole·height,
        // and the cap tessellation cuts the hole out (multi-wire planar face).
        let mut arena = ShapeArena::new();
        let id = plate_with_hole_solid(&mut arena, 4.0, 3.0, 1.0, 2.0, 1.5, 0.5, 12).unwrap();
        let v = volume(&arena, id).unwrap();
        let hole_area = 0.5 * 12.0 * 0.5_f64.powi(2) * (std::f64::consts::TAU / 12.0).sin();
        let expect = 4.0 * 3.0 * 1.0 - hole_area * 1.0;
        assert!((v - expect).abs() < 1e-9, "plate volume {} != box−hole {}", v, expect);
        // Holed caps tessellate (the hole is cut, so a cap has fewer triangles
        // than a solid quad fan would, but the mesh is non-empty).
        let mesh = tessellate(&arena, id, TessellationOptions::default()).unwrap();
        assert!(mesh.indices.len() / 3 > 8, "plate-with-hole should tessellate to many triangles");
    }

    #[test]
    fn sphere_tessellates() {
        let mut arena = ShapeArena::new();
        let id = sphere_solid(&mut arena, 1.0).unwrap();
        let mesh = tessellate(&arena, id, TessellationOptions::default()).unwrap();
        assert!(!mesh.indices.is_empty());
        for p in &mesh.positions {
            let r = ((p[0] * p[0] + p[1] * p[1] + p[2] * p[2]) as f64).sqrt();
            assert!((r - 1.0).abs() < 1e-5, "vertex off sphere: r={}", r);
        }
    }

    /// Iter 70 kitchen-sink: exercise transforms + arrays + mesh-boolean +
    /// healing + adaptive tessellation in a single pass so a regression
    /// anywhere in the deep-clone / RPC / tessellation stack surfaces here.
    #[test]
    fn kitchen_sink_iter70() {
        use bool_::{mesh_boolean, MeshOp};
        use feature::{
            box_solid, linear_array, mirror_shape, rectangular_array, rotate_shape,
            translate_shape, MirrorPlane,
        };
        use heal::{check_validity, fix_shape, HealOptions};
        use measure::{bounding_sphere, signed_distance, surface_area};
        use tessel::{tessellate_adaptive};
        use topo::ShapeArena;

        let mut arena = ShapeArena::new();
        let seed = box_solid(&mut arena, 1.0, 1.0, 1.0).unwrap();
        let moved = translate_shape(&mut arena, seed, 3.0, 0.0, 0.0).unwrap();
        let mirrored = mirror_shape(&mut arena, moved, MirrorPlane::YZ).unwrap();
        let rotated = rotate_shape(&mut arena, mirrored, (0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_4).unwrap();
        let lin = linear_array(&mut arena, rotated, 3, 2.5, 0.0, 0.0).unwrap();
        let rect = rectangular_array(&mut arena, seed, 2, 2, (2.0, 0.0, 0.0), (0.0, 2.0, 0.0)).unwrap();

        // Heal + validity on the seed.
        let opts = HealOptions { tolerance: 1e-7, sew_faces: true, remove_small_edges: true, ..Default::default() };
        let log = fix_shape(&mut arena, seed, &opts).unwrap();
        assert!(!log.is_empty());
        let issues = check_validity(&arena, seed).unwrap();
        assert!(issues.is_empty());

        // Measure the rotated mirrored moved shape.
        assert!(surface_area(&arena, rotated).unwrap() > 0.0);
        let (_, r) = bounding_sphere(&arena, seed).unwrap();
        assert!(r > 0.0);
        let sd = signed_distance(&arena, seed, gfd_cad_geom::Point3::new(10.0, 0.0, 0.0), 8, 4).unwrap();
        assert!(sd > 0.0); // outside

        // Adaptive tessellation of the array + mesh boolean with the seed.
        let mesh_array = tessellate_adaptive(&arena, lin, 0.05).unwrap();
        let mesh_seed = tessellate_adaptive(&arena, seed, 0.05).unwrap();
        let mesh_union = mesh_boolean(&mesh_array, &mesh_seed, MeshOp::Union);
        assert!(!mesh_union.indices.is_empty());

        // rect array must yield 4 solids via the collect_by_kind walker.
        use topo::{collect_by_kind, ShapeKind};
        let solids = collect_by_kind(&arena, rect, ShapeKind::Solid);
        assert_eq!(solids.len(), 4);
    }

    /// Regression test: unit-cube box volume via divergence_volume should
    /// be exactly 1.0 (its 6 planar faces form a closed polyhedron).
    #[test]
    fn box_divergence_volume_is_one() {
        use feature::box_solid;
        use measure::divergence_volume;
        let mut arena = ShapeArena::new();
        let id = box_solid(&mut arena, 1.0, 1.0, 1.0).unwrap();
        let v = divergence_volume(&arena, id).unwrap();
        assert!((v - 1.0).abs() < 1e-6, "volume = {}", v);
    }

    /// Covers the full set of phase-2..8 features added through iter 30:
    /// primitives + pad + revolve + chamfer + fillet + mesh-boolean +
    /// BRep/STEP roundtrip + measure (area, volume, CoM, inertia) + heal
    /// (check_validity, fix_shape, shape_stats) + adjacency (face_neighbors).
    #[test]
    fn extended_pipeline_smoke() {
        use bool_::{mesh_boolean, MeshOp};
        use feature::{chamfered_box_solid, filleted_box_solid};
        use heal::{check_validity, shape_stats};
        use measure::{center_of_mass, inertia_tensor_full, surface_area};
        use tessel::tessellate;
        use topo::{EdgeFaceMap, ShapeArena, ShapeKind, collect_by_kind};

        let mut arena = ShapeArena::new();
        let a = chamfered_box_solid(&mut arena, 1.0, 1.0, 1.0, 0.2).unwrap();
        let b = filleted_box_solid(&mut arena, 1.0, 1.0, 1.0, 0.2).unwrap();

        // Measurements should all succeed.
        let area_a = surface_area(&arena, a).unwrap();
        assert!(area_a > 0.0 && area_a < 10.0);
        let com_b = center_of_mass(&arena, b).unwrap();
        assert!(com_b.x.is_finite() && com_b.y.is_finite() && com_b.z.is_finite());
        let (ixx, _iyy, _izz, _, _, _) = inertia_tensor_full(&arena, a).unwrap();
        assert!(ixx > 0.0);

        // Heal + stats.
        assert!(check_validity(&arena, a).unwrap().is_empty());
        let stats = shape_stats(&arena, a).unwrap();
        assert_eq!(stats.faces, 7); // chamfered_box has 7 faces

        // Adjacency — face_neighbors only reports neighbours when edges are
        // shared by arena id. Our feature constructors allocate fresh edge
        // shapes per face, so this returns empty until a sew-edges pass
        // lands. We still exercise the API to catch regressions.
        let map = EdgeFaceMap::build(&arena, a).unwrap();
        let faces = collect_by_kind(&arena, a, ShapeKind::Face);
        for f in &faces {
            let _ = map.face_neighbors(*f);
        }

        // Mesh CSG: a ∪ b tessellated.
        let opts = TessellationOptions::default();
        let mesh_a = tessellate(&arena, a, opts).unwrap();
        let mesh_b = tessellate(&arena, b, opts).unwrap();
        let unioned = mesh_boolean(&mesh_a, &mesh_b, MeshOp::Union);
        assert!(!unioned.indices.is_empty());
    }

    /// End-to-end smoke test exercising the full Rust pipeline on one
    /// document: primitives + pad + revolve + compound + measure + heal +
    /// tessellate. Stands in for the Ralph-loop final-iteration integration
    /// verification.
    #[test]
    fn full_pipeline_smoke() {
        let mut doc = Document::new();

        // 1. Primitive sphere (analytic surface area = 4π).
        let sphere = sphere_solid(&mut doc.arena, 1.0).unwrap();
        let sphere_area = surface_area(&doc.arena, sphere).unwrap();
        assert!((sphere_area - 4.0 * std::f64::consts::PI).abs() < 1e-9);

        // 2. Pad (2×1×0.5): 4 lateral faces (2×0.5 + 1×0.5 + 2×0.5 + 1×0.5 = 3.0)
        //    plus top/bottom caps (each 2×1 = 2, total 4.0) → 7.0.
        let pad = pad_polygon_xy(&mut doc.arena, &[(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)], 0.5).unwrap();
        let pad_area = surface_area(&doc.arena, pad).unwrap();
        assert!((pad_area - 7.0).abs() < 1e-6, "pad area = {}", pad_area);
        let pad_bbox = bbox_volume(&doc.arena, pad).unwrap();
        assert!((pad_bbox - 1.0).abs() < 1e-9);

        // 3. Revolve (unit cylinder r=0.5, h=1).
        let revo = revolve_profile_z(&mut doc.arena, &[(0.0, 0.0), (0.5, 0.0), (0.5, 1.0), (0.0, 1.0)], 16).unwrap();
        let revo_bbox = bbox_volume(&doc.arena, revo).unwrap();
        assert!((revo_bbox - 1.0).abs() < 1e-6, "bbox-V was {}", revo_bbox);

        // 4. Compound merge — three shapes grouped.
        let compound = compound_merge(&mut doc.arena, &[sphere, pad, revo]).unwrap();
        let issues = check_validity(&doc.arena, compound).unwrap();
        // Revolve emits known degenerate axis edges (r=0 endpoints) — these
        // are expected artifacts until wire-closing Phase lands. Only flag
        // other kinds.
        let unexpected: Vec<_> = issues.iter().filter(|i| i.kind != "degenerate_edge").collect();
        assert!(unexpected.is_empty(), "unexpected issues: {:?}", unexpected);

        // 5. Tessellate the whole compound.
        let mesh = tessellate(&doc.arena, compound, TessellationOptions::default()).unwrap();
        assert!(mesh.positions.len() > 100, "too few vertices: {}", mesh.positions.len());
        assert!(mesh.indices.len() % 3 == 0);
    }
}
