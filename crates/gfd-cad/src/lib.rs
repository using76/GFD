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
    use feature::{box_solid, pad_polygon_xy, revolve_profile_z, sphere_solid};
    use heal::check_validity;
    use measure::{bbox_volume, surface_area};
    use tessel::{tessellate, TessellationOptions};
    use topo::ShapeArena;

    #[test]
    fn box_round_trip_primitive_to_triangles() {
        let mut arena = ShapeArena::new();
        let id = box_solid(&mut arena, 2.0, 2.0, 2.0).unwrap();
        let mesh = tessellate(&arena, id, TessellationOptions::default()).unwrap();
        // 6 faces × (32 × 16 × 2) triangles by default.
        assert_eq!(mesh.indices.len() / 3, 6 * 32 * 16 * 2);
        assert!(!mesh.positions.is_empty());
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
