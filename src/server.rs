//! GFD JSON-RPC server over stdin/stdout.
//!
//! The Electron GUI spawns this process and communicates via line-delimited
//! JSON-RPC 2.0 messages on stdin (requests) and stdout (responses).
//!
//! Usage:
//!   echo '{"id":1,"method":"system.version","params":{}}' | cargo run --bin gfd-server

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use gfd_core::field::{ScalarField, VectorField};
use gfd_core::mesh::structured::StructuredMesh;
use gfd_core::mesh::unstructured::UnstructuredMesh;
use gfd_fluid::incompressible::simple::SimpleSolver;
use gfd_fluid::FluidState;
use gfd_mesh::quality::metrics::compute_mesh_quality;
use gfd_thermal::conduction::ConductionSolver;
use gfd_thermal::ThermalState;

use gfd_cad::{
    bool_::{compound_merge, mesh_boolean, MeshOp},
    feature::{box_solid, c_channel_profile, t_beam_profile, z_section_profile, chamfered_box_solid, chamfered_box_top_edges, circular_array, cone_solid, capsule_revolve_profile, cup_revolve_profile, cylinder_solid, disc_solid, dodecahedron_solid, ellipse_profile, filleted_box_solid, filleted_box_top_edges, filleted_cylinder_solid, frustum_revolve_profile, gear_profile_simple, airfoil_naca4_profile, archimedean_spiral_path, helix_length, helix_path, honeycomb_pattern_solid, torus_knot_path, i_beam_profile, icosahedron_solid, icosphere_solid, l_angle_profile, linear_array, mirror_shape, ngon_prism_solid, octahedron_solid, offset_polygon_2d, pad_polygon_xy, pocket_polygon_xy, pyramid_solid, rectangle_profile, rectangular_array, regular_ngon_profile, revolve_profile_z, revolve_profile_z_partial, ring_revolve_profile, rotate_shape, rounded_rectangle_profile, scale_shape, slot_profile, sphere_solid, spiral_staircase_solid, stairs_solid, star_profile, tetrahedron_solid, torus_revolve_profile, torus_solid, translate_shape, tube_solid, wedge_solid, FeatureTree, MirrorPlane},
    heal::{check_validity, fix_shape, shape_stats, HealOptions},
    io::{export_step, import_brep, import_step, read_brep, read_obj, read_off, read_ply_ascii, read_stl, read_xyz, summarise_step, write_brep, write_dxf_3dface, write_obj, write_off, write_ply_ascii, write_stl_ascii, write_stl_binary, write_vtk_polydata, write_wrl, write_xyz, StlMesh},
    measure::{bbox_volume, bounding_sphere, center_of_mass, closest_point_on_shape, distance as cad_distance, distance_edge_edge, distance_vertex_edge, divergence_volume, edge_length, edge_length_range, hausdorff_distance_vertex, inertia_tensor_full, is_convex_polygon, is_point_inside_solid, mesh_euler_genus, polygon_area, polygon_area_signed, polygon_centroid, polygon_contains_point, polygon_convex_hull, polygon_perimeter, principal_axes, signed_distance, surface_area, trimesh_aspect_ratio_stats, trimesh_bounding_box, trimesh_boundary_edges, trimesh_center_of_mass, trimesh_closest_point, trimesh_edge_length_stats, trimesh_inertia_tensor, trimesh_is_closed, trimesh_non_manifold_edges, trimesh_point_inside, trimesh_ray_intersect, trimesh_signed_distance, trimesh_surface_area, trimesh_volume},
    sketch::{Constraint as SkCons, EntityId as SkEid, Point2, PointId as SkPid, Sketch},
    tessel::{extract_edges, tessellate, tessellate_adaptive, TessellationOptions, TriMesh},
    topo::ShapeId,
    Document,
};

// ---------------------------------------------------------------------------
// JSON-RPC protocol types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RpcRequest {
    id: u64,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Serialize)]
struct RpcResponse {
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl RpcResponse {
    fn ok(id: u64, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    fn err(id: u64, msg: impl Into<String>) -> Self {
        Self {
            id,
            result: None,
            error: Some(msg.into()),
        }
    }
}

// ---------------------------------------------------------------------------
// Server state
// ---------------------------------------------------------------------------

/// Shared mutable state across RPC calls.
struct ServerState {
    /// Counter for generating unique body IDs.
    next_body_id: u64,
    /// Counter for generating unique job IDs.
    next_job_id: u64,
    /// Currently active mesh (generated from geometry or `mesh.generate`).
    mesh: Option<UnstructuredMesh>,
    /// Mesh generation parameters (nx, ny, nz, lx, ly, lz).
    mesh_params: Option<(usize, usize, usize, f64, f64, f64)>,
    /// Created primitive bodies: id -> (vertices, indices, normals).
    bodies: HashMap<String, PrimitiveBody>,
    /// Active solver jobs.
    jobs: HashMap<String, JobHandle>,
    /// Last solved field data (pressure, velocity, temperature, etc.)
    fields: HashMap<String, Vec<f64>>,
    /// Active CAD document (Phase 7+ of CAD_KERNEL_PLAN.md).
    cad_doc: Document,
    /// Map from GUI-facing shape string id ("shape_N") to arena ShapeId.
    cad_shape_map: HashMap<String, ShapeId>,
    /// Counter for cad shape string ids.
    next_cad_shape_id: u64,
}

struct PrimitiveBody {
    vertices: Vec<f64>,
    indices: Vec<u32>,
    normals: Vec<f64>,
}

struct JobHandle {
    running: Arc<AtomicBool>,
    iteration: Arc<AtomicU64>,
    residual: Arc<Mutex<f64>>,
    start_time: Instant,
    /// Final result once complete, protected by Mutex.
    result: Arc<Mutex<Option<JobResult>>>,
}

struct JobResult {
    status: String,
    iterations: usize,
    residual: f64,
    fields: HashMap<String, Vec<f64>>,
}

impl ServerState {
    fn new() -> Self {
        Self {
            next_body_id: 0,
            next_job_id: 0,
            mesh: None,
            mesh_params: None,
            bodies: HashMap::new(),
            jobs: HashMap::new(),
            fields: HashMap::new(),
            cad_doc: Document::new(),
            cad_shape_map: HashMap::new(),
            next_cad_shape_id: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Primitive geometry generation (triangulated surfaces)
// ---------------------------------------------------------------------------

fn create_box_primitive(size: [f64; 3], position: [f64; 3]) -> PrimitiveBody {
    let hx = size[0] * 0.5;
    let hy = size[1] * 0.5;
    let hz = size[2] * 0.5;
    let px = position[0];
    let py = position[1];
    let pz = position[2];

    // 8 corners of the box
    let corners: [[f64; 3]; 8] = [
        [px - hx, py - hy, pz - hz], // 0: ---
        [px + hx, py - hy, pz - hz], // 1: +--
        [px + hx, py + hy, pz - hz], // 2: ++-
        [px - hx, py + hy, pz - hz], // 3: -+-
        [px - hx, py - hy, pz + hz], // 4: --+
        [px + hx, py - hy, pz + hz], // 5: +-+
        [px + hx, py + hy, pz + hz], // 6: +++
        [px - hx, py + hy, pz + hz], // 7: -++
    ];

    // 6 faces, 2 triangles each = 12 triangles.
    // Each face has its own vertices (with normal) for flat shading.
    let face_defs: [([usize; 4], [f64; 3]); 6] = [
        ([0, 1, 2, 3], [0.0, 0.0, -1.0]), // front  (-z)
        ([5, 4, 7, 6], [0.0, 0.0, 1.0]),  // back   (+z)
        ([4, 0, 3, 7], [-1.0, 0.0, 0.0]), // left   (-x)
        ([1, 5, 6, 2], [1.0, 0.0, 0.0]),  // right  (+x)
        ([4, 5, 1, 0], [0.0, -1.0, 0.0]), // bottom (-y)
        ([3, 2, 6, 7], [0.0, 1.0, 0.0]),  // top    (+y)
    ];

    let mut vertices = Vec::with_capacity(24 * 3);
    let mut normals = Vec::with_capacity(24 * 3);
    let mut indices = Vec::with_capacity(36);

    for (vi, (quad, n)) in face_defs.iter().enumerate() {
        let base = (vi * 4) as u32;
        for &ci in quad {
            let c = corners[ci];
            vertices.extend_from_slice(&c);
            normals.extend_from_slice(n);
        }
        // Two triangles: 0-1-2, 0-2-3
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    PrimitiveBody {
        vertices,
        indices,
        normals,
    }
}

fn create_sphere_primitive(radius: f64, position: [f64; 3]) -> PrimitiveBody {
    let stacks: usize = 16;
    let slices: usize = 24;

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Generate vertices
    for i in 0..=stacks {
        let phi = std::f64::consts::PI * (i as f64) / (stacks as f64);
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();

        for j in 0..=slices {
            let theta = 2.0 * std::f64::consts::PI * (j as f64) / (slices as f64);
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();

            let nx = sin_phi * cos_theta;
            let ny = cos_phi;
            let nz = sin_phi * sin_theta;

            vertices.push(position[0] + radius * nx);
            vertices.push(position[1] + radius * ny);
            vertices.push(position[2] + radius * nz);
            normals.push(nx);
            normals.push(ny);
            normals.push(nz);
        }
    }

    // Generate indices
    for i in 0..stacks {
        for j in 0..slices {
            let row0 = (i * (slices + 1) + j) as u32;
            let row1 = ((i + 1) * (slices + 1) + j) as u32;
            indices.extend_from_slice(&[row0, row1, row0 + 1]);
            indices.extend_from_slice(&[row0 + 1, row1, row1 + 1]);
        }
    }

    PrimitiveBody {
        vertices,
        indices,
        normals,
    }
}

fn create_cylinder_primitive(radius: f64, height: f64, position: [f64; 3]) -> PrimitiveBody {
    let slices: usize = 24;
    let half_h = height * 0.5;

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Side wall
    for i in 0..=slices {
        let theta = 2.0 * std::f64::consts::PI * (i as f64) / (slices as f64);
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        // Bottom ring vertex
        vertices.extend_from_slice(&[
            position[0] + radius * cos_t,
            position[1] - half_h,
            position[2] + radius * sin_t,
        ]);
        normals.extend_from_slice(&[cos_t, 0.0, sin_t]);

        // Top ring vertex
        vertices.extend_from_slice(&[
            position[0] + radius * cos_t,
            position[1] + half_h,
            position[2] + radius * sin_t,
        ]);
        normals.extend_from_slice(&[cos_t, 0.0, sin_t]);
    }

    // Side indices
    for i in 0..slices {
        let b0 = (i * 2) as u32;
        let t0 = b0 + 1;
        let b1 = b0 + 2;
        let t1 = b0 + 3;
        indices.extend_from_slice(&[b0, b1, t0]);
        indices.extend_from_slice(&[t0, b1, t1]);
    }

    // Top and bottom cap centers
    let side_vert_count = (slices + 1) * 2;

    // Bottom cap
    let bc_idx = (vertices.len() / 3) as u32;
    vertices.extend_from_slice(&[position[0], position[1] - half_h, position[2]]);
    normals.extend_from_slice(&[0.0, -1.0, 0.0]);

    for i in 0..=slices {
        let theta = 2.0 * std::f64::consts::PI * (i as f64) / (slices as f64);
        vertices.extend_from_slice(&[
            position[0] + radius * theta.cos(),
            position[1] - half_h,
            position[2] + radius * theta.sin(),
        ]);
        normals.extend_from_slice(&[0.0, -1.0, 0.0]);
    }
    for i in 0..slices {
        let v0 = bc_idx;
        let v1 = bc_idx + 1 + i as u32;
        let v2 = bc_idx + 2 + i as u32;
        indices.extend_from_slice(&[v0, v2, v1]); // reverse winding for bottom face
    }

    // Top cap
    let tc_idx = (vertices.len() / 3) as u32;
    vertices.extend_from_slice(&[position[0], position[1] + half_h, position[2]]);
    normals.extend_from_slice(&[0.0, 1.0, 0.0]);

    for i in 0..=slices {
        let theta = 2.0 * std::f64::consts::PI * (i as f64) / (slices as f64);
        vertices.extend_from_slice(&[
            position[0] + radius * theta.cos(),
            position[1] + half_h,
            position[2] + radius * theta.sin(),
        ]);
        normals.extend_from_slice(&[0.0, 1.0, 0.0]);
    }
    for i in 0..slices {
        let v0 = tc_idx;
        let v1 = tc_idx + 1 + i as u32;
        let v2 = tc_idx + 2 + i as u32;
        indices.extend_from_slice(&[v0, v1, v2]);
    }

    let _ = side_vert_count; // suppress unused warning

    PrimitiveBody {
        vertices,
        indices,
        normals,
    }
}

// ---------------------------------------------------------------------------
// Mesh display data extraction
// ---------------------------------------------------------------------------

fn mesh_display_data(mesh: &UnstructuredMesh) -> Value {
    // Flatten node positions into a flat f64 array for Three.js
    let mut vertices: Vec<f64> = Vec::with_capacity(mesh.nodes.len() * 3);
    for node in &mesh.nodes {
        vertices.extend_from_slice(&node.position);
    }

    // Wireframe edges: collect unique edges from all faces
    let mut edge_set = std::collections::HashSet::new();
    let mut wireframe_indices: Vec<u32> = Vec::new();

    for face in &mesh.faces {
        let n = face.nodes.len();
        for i in 0..n {
            let a = face.nodes[i] as u32;
            let b = face.nodes[(i + 1) % n] as u32;
            let key = if a < b { (a, b) } else { (b, a) };
            if edge_set.insert(key) {
                wireframe_indices.push(a);
                wireframe_indices.push(b);
            }
        }
    }

    // Surface triangles: triangulate boundary faces only
    let mut surface_indices: Vec<u32> = Vec::new();
    for patch in &mesh.boundary_patches {
        for &fid in &patch.face_ids {
            let face = &mesh.faces[fid];
            let ns = &face.nodes;
            // Fan triangulation from first node
            for i in 1..ns.len().saturating_sub(1) {
                surface_indices.push(ns[0] as u32);
                surface_indices.push(ns[i] as u32);
                surface_indices.push(ns[i + 1] as u32);
            }
        }
    }

    serde_json::json!({
        "vertices": vertices,
        "wireframe_indices": wireframe_indices,
        "surface_indices": surface_indices,
    })
}

// ---------------------------------------------------------------------------
// Colormap helpers
// ---------------------------------------------------------------------------

/// Maps a normalized value in [0,1] to an RGB color tuple using a jet-like colormap.
fn jet_color(t: f64) -> [f64; 3] {
    let t = t.clamp(0.0, 1.0);
    let r = (1.5 - (4.0 * t - 3.0).abs()).clamp(0.0, 1.0);
    let g = (1.5 - (4.0 * t - 2.0).abs()).clamp(0.0, 1.0);
    let b = (1.5 - (4.0 * t - 1.0).abs()).clamp(0.0, 1.0);
    [r, g, b]
}

/// Maps a normalized value in [0,1] to an RGB color tuple using a coolwarm colormap.
fn coolwarm_color(t: f64) -> [f64; 3] {
    let t = t.clamp(0.0, 1.0);
    let r = 0.230 + t * 0.770;
    let g = 0.299 + (0.5 - (t - 0.5).abs()) * 1.2;
    let b = 1.0 - t * 0.770;
    [r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)]
}

fn map_color(t: f64, colormap: &str) -> [f64; 3] {
    match colormap {
        "coolwarm" => coolwarm_color(t),
        _ => jet_color(t),
    }
}

// ---------------------------------------------------------------------------
// RPC method dispatch
// ---------------------------------------------------------------------------

fn handle_request(state: &mut ServerState, req: &RpcRequest) -> RpcResponse {
    match req.method.as_str() {
        // -- System --
        "system.version" => handle_system_version(req.id),
        "system.capabilities" => handle_system_capabilities(req.id),

        // -- CAD (legacy mesh-based) --
        "cad.create_primitive" => handle_cad_create_primitive(state, req.id, &req.params),

        // -- CAD kernel (Phase 7+ of CAD_KERNEL_PLAN.md) --
        "cad.document.new"      => handle_cad_document_new(state, req.id),
        "cad.document.stats"    => handle_cad_document_stats(state, req.id),
        "cad.document.save_json" => handle_cad_document_save_json(state, req.id, &req.params),
        "cad.document.load_json" => handle_cad_document_load_json(state, req.id, &req.params),
        "cad.document.to_string" => handle_cad_document_to_string(state, req.id),
        "cad.document.from_string" => handle_cad_document_from_string(state, req.id, &req.params),
        "cad.feature.primitive" => handle_cad_feature_primitive(state, req.id, &req.params),
        "cad.feature.pad"       => handle_cad_feature_pad(state, req.id, &req.params),
        "cad.feature.pad_profile" => handle_cad_feature_pad_profile(state, req.id, &req.params),
        "cad.feature.pocket_profile" => handle_cad_feature_pocket_profile(state, req.id, &req.params),
        "cad.feature.revolve_profile" => handle_cad_feature_revolve_profile(state, req.id, &req.params),
        "cad.feature.tube"            => handle_cad_feature_tube(state, req.id, &req.params),
        "cad.feature.disc"            => handle_cad_feature_disc(state, req.id, &req.params),
        "cad.feature.tetrahedron"     => handle_cad_feature_tetra(state, req.id, &req.params),
        "cad.feature.octahedron"      => handle_cad_feature_octa(state, req.id, &req.params),
        "cad.feature.icosahedron"     => handle_cad_feature_icosa(state, req.id, &req.params),
        "cad.feature.dodecahedron"    => handle_cad_feature_dodeca(state, req.id, &req.params),
        "cad.feature.icosphere"       => handle_cad_feature_icosphere(state, req.id, &req.params),
        "cad.feature.stairs"          => handle_cad_feature_stairs(state, req.id, &req.params),
        "cad.feature.honeycomb"       => handle_cad_feature_honeycomb(state, req.id, &req.params),
        "cad.feature.spiral_staircase" => handle_cad_feature_spiral_staircase(state, req.id, &req.params),
        "cad.profile.helix"         => handle_cad_profile_helix(req.id, &req.params),
        "cad.profile.spiral"        => handle_cad_profile_spiral(req.id, &req.params),
        "cad.profile.torus_knot"    => handle_cad_profile_torus_knot(req.id, &req.params),
        "cad.profile.generate"      => handle_cad_profile_generate(req.id, &req.params),
        "cad.profile.list_kinds"    => handle_cad_profile_list_kinds(req.id),
        "cad.version"               => handle_cad_version(req.id),
        "cad.ping"                  => handle_cad_ping(req.id, &req.params),
        "cad.feature.revolve"   => handle_cad_feature_revolve(state, req.id, &req.params),
        "cad.feature.pocket"    => handle_cad_feature_pocket(state, req.id, &req.params),
        "cad.feature.chamfer_box" => handle_cad_feature_chamfer_box(state, req.id, &req.params),
        "cad.feature.fillet_box"  => handle_cad_feature_fillet_box(state, req.id, &req.params),
        "cad.feature.keycap"      => handle_cad_feature_keycap(state, req.id, &req.params),
        "cad.feature.rounded_top_box" => handle_cad_feature_rounded_top(state, req.id, &req.params),
        "cad.feature.mirror"      => handle_cad_feature_mirror(state, req.id, &req.params),
        "cad.feature.translate"   => handle_cad_feature_translate(state, req.id, &req.params),
        "cad.feature.scale"       => handle_cad_feature_scale(state, req.id, &req.params),
        "cad.feature.rotate"      => handle_cad_feature_rotate(state, req.id, &req.params),
        "cad.feature.linear_array" => handle_cad_feature_linear_array(state, req.id, &req.params),
        "cad.feature.circular_array" => handle_cad_feature_circular_array(state, req.id, &req.params),
        "cad.feature.rectangular_array" => handle_cad_feature_rect_array(state, req.id, &req.params),
        "cad.feature.offset_polygon"    => handle_cad_feature_offset_polygon(req.id, &req.params),
        "cad.feature.offset_pad"        => handle_cad_feature_offset_pad(state, req.id, &req.params),
        "cad.feature.wedge"       => handle_cad_feature_wedge(state, req.id, &req.params),
        "cad.feature.pyramid"     => handle_cad_feature_pyramid(state, req.id, &req.params),
        "cad.feature.ngon_prism"  => handle_cad_feature_ngon_prism(state, req.id, &req.params),
        "cad.measure.closest_point"  => handle_cad_measure_closest(state, req.id, &req.params),
        "cad.measure.point_inside"   => handle_cad_measure_inside(state, req.id, &req.params),
        "cad.measure.signed_distance" => handle_cad_measure_signed(state, req.id, &req.params),
        "cad.measure.bounding_sphere" => handle_cad_measure_bsphere(state, req.id, &req.params),
        "cad.measure.principal_axes"  => handle_cad_measure_pa(state, req.id, &req.params),
        "cad.measure.edge_length_range" => handle_cad_measure_elr(state, req.id, &req.params),
        "cad.feature.filleted_cylinder" => handle_cad_feature_filleted_cylinder(state, req.id, &req.params),
        "cad.measure.edge_length" => handle_cad_measure_edge_length(state, req.id, &req.params),
        "cad.tessellate"        => handle_cad_tessellate(state, req.id, &req.params),
        "cad.tessellate_adaptive" => handle_cad_tessellate_adaptive(state, req.id, &req.params),
        "cad.tessellate_welded" => handle_cad_tessellate_welded(state, req.id, &req.params),
        "cad.mesh.boolean_raw"  => handle_cad_mesh_boolean_raw(req.id, &req.params),
        "cad.mesh.smooth"       => handle_cad_mesh_smooth(req.id, &req.params),
        "cad.mesh.subdivide"    => handle_cad_mesh_subdivide(req.id, &req.params),
        "cad.mesh.weld"         => handle_cad_mesh_weld(req.id, &req.params),
        "cad.mesh.transform"    => handle_cad_mesh_transform(req.id, &req.params),
        "cad.mesh.compute_normals" => handle_cad_mesh_compute_normals(req.id, &req.params),
        "cad.mesh.reverse_winding" => handle_cad_mesh_reverse_winding(req.id, &req.params),
        "cad.mesh.concat"       => handle_cad_mesh_concat(req.id, &req.params),
        "cad.extract_edges"       => handle_cad_extract_edges(state, req.id, &req.params),
        "cad.tree.get"          => handle_cad_tree_get(state, req.id),
        "cad.import.stl"        => handle_cad_import_stl(req.id, &req.params),
        "cad.import.obj"        => handle_cad_import_mesh(req.id, &req.params, "obj"),
        "cad.import.off"        => handle_cad_import_mesh(req.id, &req.params, "off"),
        "cad.import.ply"        => handle_cad_import_mesh(req.id, &req.params, "ply"),
        "cad.import.xyz"        => handle_cad_import_mesh(req.id, &req.params, "xyz"),
        "cad.measure.polygon_area" => handle_cad_measure_polygon_area(req.id, &req.params),
        "cad.measure.polygon_full" => handle_cad_measure_polygon_full(req.id, &req.params),
        "cad.measure.polygon_signed_area" => handle_cad_measure_polygon_signed_area(req.id, &req.params),
        "cad.measure.polygon_convex_hull" => handle_cad_measure_polygon_convex_hull(req.id, &req.params),
        "cad.measure.polygon_contains_point" => handle_cad_measure_polygon_contains_point(req.id, &req.params),
        "cad.measure.trimesh_summary" => handle_cad_measure_trimesh_summary(req.id, &req.params),
        "cad.measure.trimesh_signed_distance" => handle_cad_measure_trimesh_sdf(req.id, &req.params),
        "cad.measure.trimesh_raycast" => handle_cad_measure_trimesh_raycast(req.id, &req.params),
        "cad.arena.list_shapes"    => handle_cad_arena_list_shapes(state, req.id),
        "cad.arena.shape_info"     => handle_cad_arena_shape_info(state, req.id, &req.params),
        "cad.arena.delete_shape"   => handle_cad_arena_delete_shape(state, req.id, &req.params),
        "cad.arena.stats"          => handle_cad_arena_stats(state, req.id),
        "cad.measure.hausdorff"    => handle_cad_measure_hausdorff(state, req.id, &req.params),
        "cad.measure.shape_summary" => handle_cad_measure_shape_summary(state, req.id, &req.params),
        "cad.measure.multi_shape_summary" => handle_cad_measure_multi_shape_summary(state, req.id, &req.params),
        "cad.measure.mesh_quality" => handle_cad_measure_mesh_quality(state, req.id, &req.params),
        "cad.measure.bbox_volume"  => handle_cad_measure_bbox_volume(state, req.id, &req.params),
        "cad.measure.distance"     => handle_cad_measure_distance(state, req.id, &req.params),
        "cad.measure.surface_area" => handle_cad_measure_surface_area(state, req.id, &req.params),
        "cad.measure.volume"       => handle_cad_measure_volume(state, req.id, &req.params),
        "cad.measure.center_of_mass" => handle_cad_measure_com(state, req.id, &req.params),
        "cad.measure.inertia"        => handle_cad_measure_inertia(state, req.id, &req.params),
        "cad.measure.distance_vertex_edge" => handle_cad_measure_dve(state, req.id, &req.params),
        "cad.measure.distance_edge_edge"   => handle_cad_measure_dee(state, req.id, &req.params),
        "cad.export.brep"          => handle_cad_export_brep(state, req.id, &req.params),
        "cad.import.brep"          => handle_cad_import_brep(state, req.id, &req.params),
        "cad.export.step"          => handle_cad_export_step(state, req.id, &req.params),
        "cad.import.step"          => handle_cad_import_step(state, req.id, &req.params),
        "cad.step.summary"         => handle_cad_step_summary(req.id, &req.params),
        "cad.export.stl"           => handle_cad_export_stl(state, req.id, &req.params),
        "cad.export.obj"           => handle_cad_export_obj(state, req.id, &req.params),
        "cad.export.off"           => handle_cad_export_off(state, req.id, &req.params),
        "cad.export.ply"           => handle_cad_export_ply(state, req.id, &req.params),
        "cad.export.wrl"           => handle_cad_export_wrl(state, req.id, &req.params),
        "cad.export.xyz"           => handle_cad_export_xyz(state, req.id, &req.params),
        "cad.export.vtk"           => handle_cad_export_vtk(state, req.id, &req.params),
        "cad.export.dxf"           => handle_cad_export_dxf(state, req.id, &req.params),
        "cad.export.stl_string"    => handle_cad_export_stl_string(state, req.id, &req.params),
        "cad.export.obj_string"    => handle_cad_export_obj_string(state, req.id, &req.params),
        "cad.export.ply_string"    => handle_cad_export_ply_string(state, req.id, &req.params),
        "cad.export.step_string"   => handle_cad_export_step_string(state, req.id, &req.params),
        "cad.export.brep_string"   => handle_cad_export_brep_string(state, req.id, &req.params),
        "cad.export.vtk_string"    => handle_cad_export_vtk_string(state, req.id, &req.params),
        "cad.export.wrl_string"    => handle_cad_export_wrl_string(state, req.id, &req.params),
        "cad.export.dxf_string"    => handle_cad_export_dxf_string(state, req.id, &req.params),
        "cad.export.xyz_string"    => handle_cad_export_xyz_string(state, req.id, &req.params),
        "cad.boolean.union"        => handle_cad_boolean_union(state, req.id, &req.params),
        "cad.boolean.mesh_union"       => handle_cad_boolean_mesh(state, req.id, &req.params, MeshOp::Union),
        "cad.boolean.mesh_difference"  => handle_cad_boolean_mesh(state, req.id, &req.params, MeshOp::Difference),
        "cad.boolean.mesh_intersection"=> handle_cad_boolean_mesh(state, req.id, &req.params, MeshOp::Intersection),
        "cad.heal.check_validity"  => handle_cad_heal_check(state, req.id, &req.params),
        "cad.heal.fix"             => handle_cad_heal_fix(state, req.id, &req.params),
        "cad.heal.stats"           => handle_cad_heal_stats(state, req.id, &req.params),
        "cad.sketch.new"           => handle_cad_sketch_new(state, req.id),
        "cad.sketch.add_point"     => handle_cad_sketch_add_point(state, req.id, &req.params),
        "cad.sketch.add_line"      => handle_cad_sketch_add_line(state, req.id, &req.params),
        "cad.sketch.add_arc"       => handle_cad_sketch_add_arc(state, req.id, &req.params),
        "cad.sketch.add_circle"    => handle_cad_sketch_add_circle(state, req.id, &req.params),
        "cad.sketch.add_constraint" => handle_cad_sketch_add_constraint(state, req.id, &req.params),
        "cad.sketch.solve"         => handle_cad_sketch_solve(state, req.id, &req.params),
        "cad.sketch.get"           => handle_cad_sketch_get(state, req.id, &req.params),
        "cad.sketch.dof"           => handle_cad_sketch_dof(state, req.id, &req.params),
        "cad.sketch.list"          => handle_cad_sketch_list(state, req.id),
        "cad.sketch.delete"        => handle_cad_sketch_delete(state, req.id, &req.params),
        "cad.sketch.add_polyline"  => handle_cad_sketch_add_polyline(state, req.id, &req.params),
        "cad.sketch.add_profile"   => handle_cad_sketch_add_profile(state, req.id, &req.params),

        // -- Mesh --
        "mesh.generate" => handle_mesh_generate(state, req.id, &req.params),
        "mesh.get_display_data" => handle_mesh_get_display_data(state, req.id),
        "mesh.quality" => handle_mesh_quality(state, req.id),

        // -- Solve --
        "solve.start" => handle_solve_start(state, req.id, &req.params),
        "solve.status" => handle_solve_status(state, req.id, &req.params),
        "solve.stop" => handle_solve_stop(state, req.id, &req.params),

        // -- Field / Results --
        "field.get" => handle_field_get(state, req.id, &req.params),
        "field.contour" => handle_field_contour(state, req.id, &req.params),

        _ => RpcResponse::err(req.id, format!("Unknown method: {}", req.method)),
    }
}

// ---------------------------------------------------------------------------
// System handlers
// ---------------------------------------------------------------------------

fn handle_system_version(id: u64) -> RpcResponse {
    RpcResponse::ok(
        id,
        serde_json::json!({
            "name": "GFD Solver",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Generalized Fluid Dynamics multi-physics solver",
            "powered_by": "GFD Solver — https://github.com/using76/GFD",
        }),
    )
}

fn handle_system_capabilities(id: u64) -> RpcResponse {
    RpcResponse::ok(
        id,
        serde_json::json!({
            "solvers": [
                "incompressible_simple",
                "incompressible_piso",
                "incompressible_simplec",
                "compressible_roe",
                "compressible_hllc",
                "compressible_ausm_plus",
                "conduction_steady",
                "conduction_transient",
                "solid_linear_elastic",
            ],
            "turbulence_models": [
                "none",
                "k_epsilon",
                "k_omega_sst",
                "les_smagorinsky",
            ],
            "multiphase": [
                "vof",
                "level_set",
                "euler_euler",
            ],
            "mesh_types": [
                "cartesian",
                "structured",
            ],
            "boundary_conditions": [
                "wall",
                "velocity_inlet",
                "pressure_outlet",
                "symmetry",
                "periodic",
                "fixed_temperature",
            ],
            "output_fields": [
                "pressure",
                "velocity",
                "temperature",
                "velocity_magnitude",
                "vx", "vy", "vz",
            ],
        }),
    )
}

// ---------------------------------------------------------------------------
// CAD handlers
// ---------------------------------------------------------------------------

fn handle_cad_create_primitive(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let ptype = params
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("box");

    let position = parse_f64_array3(params.get("position")).unwrap_or([0.0, 0.0, 0.0]);

    let body = match ptype {
        "box" => {
            let size = parse_f64_array3(params.get("size")).unwrap_or([1.0, 1.0, 1.0]);
            create_box_primitive(size, position)
        }
        "sphere" => {
            let radius = params
                .get("radius")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5);
            create_sphere_primitive(radius, position)
        }
        "cylinder" => {
            let radius = params
                .get("radius")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5);
            let height = params
                .get("height")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            create_cylinder_primitive(radius, height, position)
        }
        _ => {
            return RpcResponse::err(id, format!("Unknown primitive type: {}", ptype));
        }
    };

    state.next_body_id += 1;
    let body_id = format!("body_{}", state.next_body_id);

    let result = serde_json::json!({
        "id": body_id,
        "vertices": &body.vertices,
        "indices": &body.indices,
        "normals": &body.normals,
    });

    state.bodies.insert(body_id, body);

    RpcResponse::ok(id, result)
}

// ---------------------------------------------------------------------------
// CAD kernel handlers (Phase 7 of CAD_KERNEL_PLAN.md)
// ---------------------------------------------------------------------------

fn handle_cad_document_new(state: &mut ServerState, id: u64) -> RpcResponse {
    state.cad_doc = Document::new();
    state.cad_shape_map.clear();
    state.next_cad_shape_id = 0;
    RpcResponse::ok(id, serde_json::json!({ "ok": true }))
}

/// In-memory document → JSON string. Same format as `save_json` but
/// returns the content directly without touching disk.
fn handle_cad_document_to_string(state: &ServerState, id: u64) -> RpcResponse {
    let mut shapes_json: Vec<serde_json::Value> = Vec::new();
    for (str_id, arena_id) in state.cad_shape_map.iter() {
        let tmp = std::env::temp_dir().join(format!("gfd_doc_tostr_{}.brep",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos()).unwrap_or(0)));
        if write_brep(&tmp, &state.cad_doc.arena, Some(*arena_id)).is_err() { continue; }
        let content = std::fs::read_to_string(&tmp).unwrap_or_default();
        let _ = std::fs::remove_file(&tmp);
        shapes_json.push(serde_json::json!({
            "shape_id":  str_id,
            "arena_id":  arena_id.0,
            "brep_json": content,
        }));
    }
    let doc = serde_json::json!({
        "version":       env!("CARGO_PKG_VERSION"),
        "shapes":        shapes_json,
        "next_shape_id": state.next_cad_shape_id,
        "sketch_count":  state.cad_doc.sketches.len(),
    });
    let s = serde_json::to_string(&doc).unwrap_or_default();
    let len = s.len();
    RpcResponse::ok(id, serde_json::json!({ "content": s, "length": len }))
}

/// Restore document from a JSON string (as produced by `to_string`).
fn handle_cad_document_from_string(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(content) = params.get("content").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing content");
    };
    let doc: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(e) => return RpcResponse::err(id, format!("parse failed: {}", e)),
    };
    state.cad_doc = gfd_cad::Document::default();
    state.cad_shape_map.clear();
    state.next_cad_shape_id = 0;
    let Some(arr) = doc.get("shapes").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "doc missing 'shapes'");
    };
    let mut loaded = 0usize;
    for entry in arr {
        let Some(str_id) = entry.get("shape_id").and_then(|v| v.as_str()) else { continue; };
        let Some(brep_str) = entry.get("brep_json").and_then(|v| v.as_str()) else { continue; };
        let tmp = std::env::temp_dir().join(format!("gfd_doc_fromstr_{}.brep",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos()).unwrap_or(0)));
        if std::fs::write(&tmp, brep_str).is_err() { continue; }
        if let Ok(new_id) = import_brep(&tmp, &mut state.cad_doc.arena) {
            state.cad_shape_map.insert(str_id.to_string(), new_id);
            state.next_cad_shape_id = state.next_cad_shape_id.max(
                str_id.strip_prefix("shape_").and_then(|s| s.parse::<u64>().ok()).unwrap_or(0));
            loaded += 1;
        }
        let _ = std::fs::remove_file(&tmp);
    }
    RpcResponse::ok(id, serde_json::json!({ "loaded": loaded }))
}

/// Serialise the current arena + shape_map + next_id into a JSON blob and
/// write it to `path`. Sketches are not yet serialised (stored as index
/// references only). BRep-JSON is used per-shape to preserve topology.
fn handle_cad_document_save_json(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    let mut shapes_json: Vec<serde_json::Value> = Vec::new();
    for (str_id, arena_id) in state.cad_shape_map.iter() {
        let tmp = std::env::temp_dir().join(format!("gfd_doc_save_{}.brep",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos()).unwrap_or(0)));
        if write_brep(&tmp, &state.cad_doc.arena, Some(*arena_id)).is_err() { continue; }
        let content = std::fs::read_to_string(&tmp).unwrap_or_default();
        let _ = std::fs::remove_file(&tmp);
        shapes_json.push(serde_json::json!({
            "shape_id":  str_id,
            "arena_id":  arena_id.0,
            "brep_json": content,
        }));
    }
    let doc = serde_json::json!({
        "version":       env!("CARGO_PKG_VERSION"),
        "shapes":        shapes_json,
        "next_shape_id": state.next_cad_shape_id,
        "sketch_count":  state.cad_doc.sketches.len(),
    });
    match std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap_or_default()) {
        Ok(_) => RpcResponse::ok(id, serde_json::json!({
            "ok": true, "path": path, "shape_count": shapes_json.len(),
        })),
        Err(e) => RpcResponse::err(id, format!("doc write failed: {}", e)),
    }
}

/// Reset arena + load shapes from a `cad.document.save_json` blob at `path`.
/// Sketches are reset to empty (not yet round-tripped).
fn handle_cad_document_load_json(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => return RpcResponse::err(id, format!("read failed: {}", e)),
    };
    let doc: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return RpcResponse::err(id, format!("parse failed: {}", e)),
    };
    // Reset document (new arena + clear sketches).
    state.cad_doc = gfd_cad::Document::default();
    state.cad_shape_map.clear();
    state.next_cad_shape_id = 0;
    let Some(arr) = doc.get("shapes").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "doc missing 'shapes'");
    };
    let mut loaded = 0usize;
    for entry in arr {
        let Some(str_id) = entry.get("shape_id").and_then(|v| v.as_str()) else { continue; };
        let Some(brep_str) = entry.get("brep_json").and_then(|v| v.as_str()) else { continue; };
        let tmp = std::env::temp_dir().join(format!("gfd_doc_load_{}.brep",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos()).unwrap_or(0)));
        if std::fs::write(&tmp, brep_str).is_err() { continue; }
        match import_brep(&tmp, &mut state.cad_doc.arena) {
            Ok(new_id) => {
                state.cad_shape_map.insert(str_id.to_string(), new_id);
                state.next_cad_shape_id = state.next_cad_shape_id.max(
                    str_id.strip_prefix("shape_").and_then(|s| s.parse::<u64>().ok()).unwrap_or(0)
                );
                loaded += 1;
            }
            Err(_) => {}
        }
        let _ = std::fs::remove_file(&tmp);
    }
    RpcResponse::ok(id, serde_json::json!({
        "ok": true, "path": path, "loaded": loaded,
    }))
}

fn handle_cad_document_stats(state: &ServerState, id: u64) -> RpcResponse {
    RpcResponse::ok(id, serde_json::json!({
        "arena_len": state.cad_doc.arena.len(),
        "shape_map_len": state.cad_shape_map.len(),
        "feature_count": state.cad_doc.features.len(),
        "sketch_count": state.cad_doc.sketches.len(),
        "next_shape_id": state.next_cad_shape_id,
    }))
}

fn handle_cad_feature_primitive(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let kind = params.get("kind").and_then(|v| v.as_str()).unwrap_or("box");
    let arena = &mut state.cad_doc.arena;
    let result = match kind {
        "box" => {
            let lx = params.get("lx").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let ly = params.get("ly").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let lz = params.get("lz").and_then(|v| v.as_f64()).unwrap_or(1.0);
            box_solid(arena, lx, ly, lz)
        }
        "sphere" => {
            let radius = params.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.5);
            sphere_solid(arena, radius)
        }
        "cylinder" => {
            let radius = params.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let height = params.get("height").and_then(|v| v.as_f64()).unwrap_or(1.0);
            cylinder_solid(arena, radius, height)
        }
        "cone" => {
            let r1 = params.get("r1").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let r2 = params.get("r2").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let h  = params.get("height").and_then(|v| v.as_f64()).unwrap_or(1.0);
            cone_solid(arena, r1, r2, h)
        }
        "torus" => {
            let major = params.get("major").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let minor = params.get("minor").and_then(|v| v.as_f64()).unwrap_or(0.15);
            torus_solid(arena, major, minor)
        }
        other => return RpcResponse::err(id, format!("Unknown primitive kind: {}", other)),
    };

    let shape_id = match result {
        Ok(sid) => sid,
        Err(e) => return RpcResponse::err(id, format!("primitive build failed: {}", e)),
    };

    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);

    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id,
        "arena_id": shape_id.0,
        "kind": kind,
    }))
}

fn handle_cad_extract_edges(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    match extract_edges(&state.cad_doc.arena, aid) {
        Ok(pts) => {
            let flat: Vec<f32> = pts.iter().flat_map(|p| p.iter().copied()).collect();
            RpcResponse::ok(id, serde_json::json!({ "positions": flat, "line_count": pts.len() / 2 }))
        }
        Err(e) => RpcResponse::err(id, format!("extract_edges failed: {}", e)),
    }
}

fn handle_cad_tessellate_adaptive(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let str_id = match params.get("shape_id").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return RpcResponse::err(id, "missing shape_id"),
    };
    let Some(arena_id) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let tol = params.get("chord_tolerance").and_then(|v| v.as_f64()).unwrap_or(0.02);
    match tessellate_adaptive(&state.cad_doc.arena, arena_id, tol) {
        Ok(mesh) => {
            let positions: Vec<f32> = mesh.positions.iter().flat_map(|p| p.iter().copied()).collect();
            let normals:   Vec<f32> = mesh.normals.iter().flat_map(|n| n.iter().copied()).collect();
            RpcResponse::ok(id, serde_json::json!({
                "positions": positions, "normals": normals, "indices": mesh.indices,
                "triangle_count": mesh.indices.len() / 3,
            }))
        }
        Err(e) => RpcResponse::err(id, format!("tessellate_adaptive failed: {}", e)),
    }
}

/// Tessellate + weld (spatial hash dedup) + prune orphan vertices in one
/// RPC. Returns a mesh suitable for Euler-characteristic or topology
/// queries on CAD-kernel output (which normally emits per-face vertices).
fn handle_cad_tessellate_welded(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let tol = params.get("tol").and_then(|v| v.as_f64()).unwrap_or(1.0e-4) as f32;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let mut mesh = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate failed: {}", e)),
    };
    let removed = mesh.weld(tol);
    let pruned = mesh.prune_unused_vertices();
    let positions: Vec<f32> = mesh.positions.iter().flat_map(|p| p.iter().copied()).collect();
    let normals:   Vec<f32> = mesh.normals.iter().flat_map(|n| n.iter().copied()).collect();
    RpcResponse::ok(id, serde_json::json!({
        "positions":       positions,
        "normals":         normals,
        "indices":         mesh.indices,
        "triangle_count":  mesh.indices.len() / 3,
        "vertex_count":    mesh.positions.len(),
        "welded_removed":  removed,
        "pruned":          pruned,
        "tol":             tol,
    }))
}

fn handle_cad_tessellate(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let str_id = match params.get("shape_id").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return RpcResponse::err(id, "missing shape_id"),
    };
    let Some(arena_id) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    match tessellate(&state.cad_doc.arena, arena_id, opts) {
        Ok(mesh) => {
            let positions: Vec<f32> = mesh.positions.iter().flat_map(|p| p.iter().copied()).collect();
            let normals:   Vec<f32> = mesh.normals.iter().flat_map(|n| n.iter().copied()).collect();
            RpcResponse::ok(id, serde_json::json!({
                "positions": positions,
                "normals":   normals,
                "indices":   mesh.indices,
                "triangle_count": mesh.indices.len() / 3,
            }))
        }
        Err(e) => RpcResponse::err(id, format!("tessellate failed: {}", e)),
    }
}

fn handle_cad_tree_get(state: &mut ServerState, id: u64) -> RpcResponse {
    let features: Vec<_> = state.cad_doc.features.features.iter().enumerate().map(|(i, f)| {
        serde_json::json!({
            "id": i,
            "feature": f,
            "result": state.cad_doc.features.results.get(i).copied().flatten().map(|s| s.0),
        })
    }).collect();
    let shapes: Vec<_> = state.cad_shape_map.iter().map(|(k, v)| {
        serde_json::json!({ "shape_id": k, "arena_id": v.0 })
    }).collect();
    RpcResponse::ok(id, serde_json::json!({ "features": features, "shapes": shapes }))
}

#[allow(dead_code)]
fn _silence_feature_tree_warning(_: &FeatureTree) {}

fn handle_cad_feature_pad(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let pts_val = params.get("points");
    let Some(arr) = pts_val.and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "cad.feature.pad: params.points must be an array of [x,y]");
    };
    let mut pts = Vec::with_capacity(arr.len());
    for pair in arr {
        let Some(p) = pair.as_array() else { return RpcResponse::err(id, "point must be [x,y]"); };
        if p.len() < 2 { return RpcResponse::err(id, "point must be [x,y]"); }
        let x = p[0].as_f64().unwrap_or(0.0);
        let y = p[1].as_f64().unwrap_or(0.0);
        pts.push((x, y));
    }
    let height = params.get("height").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let shape_id = match pad_polygon_xy(&mut state.cad_doc.arena, &pts, height) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("pad failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id,
        "arena_id": shape_id.0,
        "kind": "pad",
    }))
}

/// Pad using a parametric 2D profile ("ngon", "star", "rectangle",
/// "rounded_rectangle", "slot", "ellipse", "gear"). Saves the GUI from
/// rebuilding the polygon client-side. Params:
///   profile: string — kind above
///   height: f64
///   + kind-specific: radius / sides / outer_r / inner_r / points /
///     width / height_r / r / corner_segs / length / a / b / segments /
///     tip_r / root_r / teeth / duty
fn handle_cad_feature_pad_profile(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let kind = params.get("profile").and_then(|v| v.as_str()).unwrap_or("");
    let height = params.get("height").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let f = |k: &str, d: f64| params.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
    let u = |k: &str, d: u64| params.get(k).and_then(|v| v.as_u64()).unwrap_or(d) as usize;
    let pts: Vec<(f64, f64)> = match kind {
        "ngon" => regular_ngon_profile(f("radius", 1.0), u("sides", 6), f("start_deg", 0.0)),
        "star" => star_profile(f("outer_r", 1.0), f("inner_r", 0.5), u("points", 5)),
        "rectangle" => rectangle_profile(f("width", 2.0), f("height_r", 1.0)),
        "rounded_rectangle" => rounded_rectangle_profile(
            f("width", 4.0), f("height_r", 2.0), f("r", 0.5), u("corner_segs", 4),
        ),
        "slot" => slot_profile(f("length", 4.0), f("width", 1.0), u("arc_segs", 8)),
        "ellipse" => ellipse_profile(f("a", 2.0), f("b", 1.0), u("segments", 32)),
        "gear" => gear_profile_simple(f("tip_r", 2.0), f("root_r", 1.5), u("teeth", 12), f("duty", 0.5)),
        "airfoil" => airfoil_naca4_profile(f("thickness", 0.12), f("chord", 1.0), u("segments", 32)),
        "i_beam"  => i_beam_profile(f("section_h", 2.0), f("width", 1.0), f("flange_t", 0.1), f("web_t", 0.1)),
        "l_angle" => l_angle_profile(f("length", 1.0), f("section_h", 1.0), f("thickness", 0.1)),
        "c_channel" => c_channel_profile(f("section_h", 2.0), f("depth", 1.0), f("flange_t", 0.1), f("web_t", 0.1)),
        "t_beam"    => t_beam_profile(f("section_h", 1.0), f("width", 1.0), f("flange_t", 0.1), f("web_t", 0.1)),
        "z_section" => z_section_profile(f("section_h", 1.0), f("flange_len", 0.5), f("thickness", 0.1)),
        other => return RpcResponse::err(id, format!("unknown profile kind '{}'", other)),
    };
    if pts.is_empty() {
        return RpcResponse::err(id, "profile generator returned empty polygon (check params)");
    }
    let shape_id = match pad_polygon_xy(&mut state.cad_doc.arena, &pts, height) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("pad failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id,
        "arena_id": shape_id.0,
        "kind": format!("pad_{}", kind),
        "polygon_verts": pts.len(),
    }))
}

fn handle_cad_feature_tetra(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let scale = params.get("scale").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let shape_id = match tetrahedron_solid(&mut state.cad_doc.arena, scale) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("tetrahedron failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({ "shape_id": str_id, "arena_id": shape_id.0, "kind": "tetrahedron" }))
}

fn handle_cad_feature_octa(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let scale = params.get("scale").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let shape_id = match octahedron_solid(&mut state.cad_doc.arena, scale) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("octahedron failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({ "shape_id": str_id, "arena_id": shape_id.0, "kind": "octahedron" }))
}

fn handle_cad_feature_icosa(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let scale = params.get("scale").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let shape_id = match icosahedron_solid(&mut state.cad_doc.arena, scale) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("icosahedron failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({ "shape_id": str_id, "arena_id": shape_id.0, "kind": "icosahedron" }))
}

fn handle_cad_feature_dodeca(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let scale = params.get("scale").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let shape_id = match dodecahedron_solid(&mut state.cad_doc.arena, scale) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("dodecahedron failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({ "shape_id": str_id, "arena_id": shape_id.0, "kind": "dodecahedron" }))
}

fn handle_cad_feature_spiral_staircase(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let count = params.get("step_count").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let radius = params.get("radius").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let len = params.get("tread_len").and_then(|v| v.as_f64()).unwrap_or(0.6);
    let w = params.get("tread_w").and_then(|v| v.as_f64()).unwrap_or(0.3);
    let h = params.get("step_h").and_then(|v| v.as_f64()).unwrap_or(0.08);
    let deg = params.get("angle_per_step_deg").and_then(|v| v.as_f64()).unwrap_or(22.5);
    let rise = params.get("rise_per_step").and_then(|v| v.as_f64()).unwrap_or(0.2);
    let shape_id = match spiral_staircase_solid(&mut state.cad_doc.arena, count, radius, len, w, h, deg, rise) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("spiral_staircase failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id, "arena_id": shape_id.0, "kind": "spiral_staircase",
        "steps": count,
    }))
}

fn handle_cad_feature_honeycomb(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let rows = params.get("rows").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
    let cols = params.get("cols").and_then(|v| v.as_u64()).unwrap_or(4) as usize;
    let hex_r = params.get("hex_r").and_then(|v| v.as_f64()).unwrap_or(0.3);
    let hex_h = params.get("hex_h").and_then(|v| v.as_f64()).unwrap_or(0.2);
    let shape_id = match honeycomb_pattern_solid(&mut state.cad_doc.arena, rows, cols, hex_r, hex_h) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("honeycomb failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id, "arena_id": shape_id.0, "kind": "honeycomb",
        "cells": rows * cols,
    }))
}

fn handle_cad_feature_stairs(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let count = params.get("step_count").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let w = params.get("step_w").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let h = params.get("step_h").and_then(|v| v.as_f64()).unwrap_or(0.2);
    let d = params.get("step_d").and_then(|v| v.as_f64()).unwrap_or(0.3);
    let shape_id = match stairs_solid(&mut state.cad_doc.arena, count, w, h, d) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("stairs failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({ "shape_id": str_id, "arena_id": shape_id.0, "kind": "stairs" }))
}

fn handle_cad_feature_icosphere(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let radius = params.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let subs = params.get("subdivisions").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
    let shape_id = match icosphere_solid(&mut state.cad_doc.arena, radius, subs) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("icosphere failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({ "shape_id": str_id, "arena_id": shape_id.0, "kind": "icosphere" }))
}

fn handle_cad_feature_tube(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let f = |k: &str, d: f64| params.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
    let u = |k: &str, d: u64| params.get(k).and_then(|v| v.as_u64()).unwrap_or(d) as usize;
    let shape_id = match tube_solid(&mut state.cad_doc.arena,
        f("inner_r", 0.4), f("outer_r", 0.6), f("height", 1.0), u("angular_steps", 24)) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("tube failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id, "arena_id": shape_id.0, "kind": "tube",
    }))
}

fn handle_cad_feature_disc(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let f = |k: &str, d: f64| params.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
    let u = |k: &str, d: u64| params.get(k).and_then(|v| v.as_u64()).unwrap_or(d) as usize;
    let shape_id = match disc_solid(&mut state.cad_doc.arena,
        f("radius", 0.5), f("thickness", 0.1), u("angular_steps", 24)) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("disc failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id, "arena_id": shape_id.0, "kind": "disc",
    }))
}

/// Revolve an (r, z) profile generator around the Z axis. Supported
/// kinds: "ring", "cup", "frustum", "torus".
fn handle_cad_feature_revolve_profile(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let kind = params.get("profile").and_then(|v| v.as_str()).unwrap_or("");
    let f = |k: &str, d: f64| params.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
    let u = |k: &str, d: u64| params.get(k).and_then(|v| v.as_u64()).unwrap_or(d) as usize;
    let profile: Vec<(f64, f64)> = match kind {
        "ring"    => ring_revolve_profile(f("inner_r", 0.5), f("outer_r", 1.0), f("thickness", 0.2)),
        "cup"     => cup_revolve_profile(f("outer_r", 1.0), f("wall_thickness", 0.1),
                                         f("height", 1.5), f("bottom_thickness", 0.1)),
        "frustum" => frustum_revolve_profile(f("r1", 0.8), f("r2", 0.4), f("height", 1.0)),
        "torus"   => torus_revolve_profile(f("major_r", 1.0), f("minor_r", 0.3), u("segments", 32)),
        "capsule" => capsule_revolve_profile(f("radius", 0.4), f("cyl_length", 1.0), u("arc_segs", 16)),
        other     => return RpcResponse::err(id, format!("unknown revolve profile kind '{}'", other)),
    };
    if profile.is_empty() {
        return RpcResponse::err(id, "revolve profile returned empty (check params)");
    }
    let angular_steps = params.get("angular_steps").and_then(|v| v.as_u64()).unwrap_or(24) as usize;
    let shape_id = match revolve_profile_z(&mut state.cad_doc.arena, &profile, angular_steps) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("revolve failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id":      str_id,
        "arena_id":      shape_id.0,
        "kind":          format!("revolve_{}", kind),
        "profile_verts": profile.len(),
    }))
}

/// Health-check RPC. Echoes any `payload`, adds server-side nanos-since-
/// UNIX, and the current GUI-registered shape count. Useful for latency
/// probes, resync handshakes, and confirming the server is responsive.
fn handle_cad_ping(id: u64, params: &Value) -> RpcResponse {
    let payload = params.get("payload").cloned().unwrap_or(serde_json::Value::Null);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as i128)
        .unwrap_or(0);
    // `as i128` → serde cannot serialise i128 directly. Downcast to i64 for JSON.
    let ts_ms: i64 = (nanos / 1_000_000) as i64;
    RpcResponse::ok(id, serde_json::json!({
        "pong":        true,
        "payload":     payload,
        "unix_ms":     ts_ms,
    }))
}

/// Kernel version / feature-level meta. Lets clients gracefully adapt
/// to different gfd-cad builds (detect missing RPCs, fallback paths).
fn handle_cad_version(id: u64) -> RpcResponse {
    RpcResponse::ok(id, serde_json::json!({
        "kernel":           "gfd-cad",
        "kernel_version":   env!("CARGO_PKG_VERSION"),
        "server_iteration": 166,
        "features": {
            "boolean_mesh":              true,
            "boolean_brep":              false,
            "generic_edge_fillet":       false,
            "step_reader_topology":      false,
            "step_reader_points_only":   true,
            "step_writer":               true,
            "sketcher_constraints":      17,
            "primitive_count":           13,
            "profile_count":             13,
            "revolve_profile_count":     5,
            "platonic_solids":           ["tetrahedron", "octahedron", "icosahedron"],
        },
        "import_formats": ["stl", "obj", "off", "ply", "xyz", "step_points", "brep"],
        "export_formats": [
            "stl_ascii", "stl_binary", "obj", "off", "ply", "wrl", "xyz",
            "step", "brep", "vtk", "dxf",
        ],
        "memory_export_formats": ["stl", "obj", "ply", "step", "brep"],
        "measure_helpers": 60,
        "rpc_count_approx": 110,
    }))
}

/// Meta-query: returns every profile kind supported by
/// pad_profile / pocket_profile / profile.generate, plus the (r, z)
/// revolve kinds accepted by `cad.feature.revolve_profile`, along with
/// the parameter keys each kind reads. Useful for GUI auto-generated
/// property panels.
fn handle_cad_profile_list_kinds(id: u64) -> RpcResponse {
    RpcResponse::ok(id, serde_json::json!({
        "pad_kinds": [
            {"name": "ngon",              "params": ["radius", "sides", "start_deg"]},
            {"name": "star",              "params": ["outer_r", "inner_r", "points"]},
            {"name": "rectangle",         "params": ["width", "height_r"]},
            {"name": "rounded_rectangle", "params": ["width", "height_r", "r", "corner_segs"]},
            {"name": "slot",              "params": ["length", "width", "arc_segs"]},
            {"name": "ellipse",           "params": ["a", "b", "segments"]},
            {"name": "gear",              "params": ["tip_r", "root_r", "teeth", "duty"]},
            {"name": "airfoil",           "params": ["thickness", "chord", "segments"]},
            {"name": "i_beam",            "params": ["section_h", "width", "flange_t", "web_t"]},
            {"name": "l_angle",           "params": ["length", "section_h", "thickness"]},
            {"name": "c_channel",         "params": ["section_h", "depth", "flange_t", "web_t"]},
            {"name": "t_beam",            "params": ["section_h", "width", "flange_t", "web_t"]},
            {"name": "z_section",         "params": ["section_h", "flange_len", "thickness"]},
        ],
        "revolve_kinds": [
            {"name": "ring",    "params": ["inner_r", "outer_r", "thickness"]},
            {"name": "cup",     "params": ["outer_r", "wall_thickness", "height", "bottom_thickness"]},
            {"name": "frustum", "params": ["r1", "r2", "height"]},
            {"name": "torus",   "params": ["major_r", "minor_r", "segments"]},
            {"name": "capsule", "params": ["radius", "cyl_length", "arc_segs"]},
        ],
        "primitives": [
            "box", "sphere", "cylinder", "cone", "torus",
            "wedge", "pyramid", "ngon_prism", "tube", "disc",
            "tetrahedron", "octahedron", "icosahedron",
        ],
    }))
}

/// Pure-read RPC returning a 2D profile polygon point list. Same 7 kinds
/// as pad_profile / pocket_profile. Useful when the GUI wants to display
/// a preview in a 2D panel, feed a sketcher, or run measurements before
/// committing to a solid.
fn handle_cad_profile_generate(id: u64, params: &Value) -> RpcResponse {
    let kind = params.get("profile").and_then(|v| v.as_str()).unwrap_or("");
    let f = |k: &str, d: f64| params.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
    let u = |k: &str, d: u64| params.get(k).and_then(|v| v.as_u64()).unwrap_or(d) as usize;
    let pts: Vec<(f64, f64)> = match kind {
        "ngon" => regular_ngon_profile(f("radius", 1.0), u("sides", 6), f("start_deg", 0.0)),
        "star" => star_profile(f("outer_r", 1.0), f("inner_r", 0.5), u("points", 5)),
        "rectangle" => rectangle_profile(f("width", 2.0), f("height_r", 1.0)),
        "rounded_rectangle" => rounded_rectangle_profile(
            f("width", 4.0), f("height_r", 2.0), f("r", 0.5), u("corner_segs", 4),
        ),
        "slot" => slot_profile(f("length", 4.0), f("width", 1.0), u("arc_segs", 8)),
        "ellipse" => ellipse_profile(f("a", 2.0), f("b", 1.0), u("segments", 32)),
        "gear" => gear_profile_simple(f("tip_r", 2.0), f("root_r", 1.5), u("teeth", 12), f("duty", 0.5)),
        "airfoil" => airfoil_naca4_profile(f("thickness", 0.12), f("chord", 1.0), u("segments", 32)),
        "i_beam"  => i_beam_profile(f("section_h", 2.0), f("width", 1.0), f("flange_t", 0.1), f("web_t", 0.1)),
        "l_angle" => l_angle_profile(f("length", 1.0), f("section_h", 1.0), f("thickness", 0.1)),
        "c_channel" => c_channel_profile(f("section_h", 2.0), f("depth", 1.0), f("flange_t", 0.1), f("web_t", 0.1)),
        "t_beam"    => t_beam_profile(f("section_h", 1.0), f("width", 1.0), f("flange_t", 0.1), f("web_t", 0.1)),
        "z_section" => z_section_profile(f("section_h", 1.0), f("flange_len", 0.5), f("thickness", 0.1)),
        other => return RpcResponse::err(id, format!("unknown profile kind '{}'", other)),
    };
    if pts.is_empty() {
        return RpcResponse::err(id, "profile returned empty polygon");
    }
    let flat: Vec<f64> = pts.iter().flat_map(|p| [p.0, p.1]).collect();
    RpcResponse::ok(id, serde_json::json!({
        "profile":     kind,
        "points":      flat,
        "vertex_count": pts.len(),
    }))
}

fn handle_cad_profile_spiral(id: u64, params: &Value) -> RpcResponse {
    let a = params.get("a").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let b = params.get("b").and_then(|v| v.as_f64()).unwrap_or(0.2);
    let turns = params.get("turns").and_then(|v| v.as_f64()).unwrap_or(3.0);
    let seg = params.get("segments_per_turn").and_then(|v| v.as_u64()).unwrap_or(48) as usize;
    let pts = archimedean_spiral_path(a, b, turns, seg);
    if pts.is_empty() {
        return RpcResponse::err(id, "spiral: invalid params (b, turns, segments_per_turn must be >0)");
    }
    let flat: Vec<f64> = pts.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
    RpcResponse::ok(id, serde_json::json!({
        "points":      flat,
        "point_count": pts.len(),
        "a":           a,
        "b":           b,
        "turns":       turns,
    }))
}

fn handle_cad_profile_torus_knot(id: u64, params: &Value) -> RpcResponse {
    let p = params.get("p").and_then(|v| v.as_u64()).unwrap_or(2) as u32;
    let q = params.get("q").and_then(|v| v.as_u64()).unwrap_or(3) as u32;
    let major_r = params.get("major_r").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let minor_r = params.get("minor_r").and_then(|v| v.as_f64()).unwrap_or(0.3);
    let segments = params.get("segments").and_then(|v| v.as_u64()).unwrap_or(256) as usize;
    let pts = torus_knot_path(p, q, major_r, minor_r, segments);
    if pts.is_empty() {
        return RpcResponse::err(id, "torus_knot: invalid params");
    }
    let flat: Vec<f64> = pts.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
    RpcResponse::ok(id, serde_json::json!({
        "points":      flat,
        "point_count": pts.len(),
        "p":           p, "q":       q,
        "major_r":     major_r, "minor_r": minor_r,
    }))
}

/// Samples a right-handed helix around the Z axis and returns the point
/// list + analytic arc length. Pure-read RPC — no arena mutation.
fn handle_cad_profile_helix(id: u64, params: &Value) -> RpcResponse {
    let r     = params.get("radius").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let pitch = params.get("pitch").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let turns = params.get("turns").and_then(|v| v.as_f64()).unwrap_or(3.0);
    let seg   = params.get("segments_per_turn").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let pts = helix_path(r, pitch, turns, seg);
    if pts.is_empty() {
        return RpcResponse::err(id, "helix: invalid params (radius/turns/segments must be positive)");
    }
    let length = helix_length(r, pitch, turns);
    let flat: Vec<f64> = pts.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
    RpcResponse::ok(id, serde_json::json!({
        "points":       flat,
        "point_count":  pts.len(),
        "length":       length,
        "radius":       r,
        "pitch":        pitch,
        "turns":        turns,
    }))
}

/// Pocket using a parametric 2D profile — same profile kinds as
/// `cad.feature.pad_profile`, plus a `depth` param routed into
/// `pocket_polygon_xy`. The kind-specific params mirror pad_profile.
fn handle_cad_feature_pocket_profile(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let kind = params.get("profile").and_then(|v| v.as_str()).unwrap_or("");
    let depth = params.get("depth").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let f = |k: &str, d: f64| params.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
    let u = |k: &str, d: u64| params.get(k).and_then(|v| v.as_u64()).unwrap_or(d) as usize;
    let pts: Vec<(f64, f64)> = match kind {
        "ngon" => regular_ngon_profile(f("radius", 1.0), u("sides", 6), f("start_deg", 0.0)),
        "star" => star_profile(f("outer_r", 1.0), f("inner_r", 0.5), u("points", 5)),
        "rectangle" => rectangle_profile(f("width", 2.0), f("height_r", 1.0)),
        "rounded_rectangle" => rounded_rectangle_profile(
            f("width", 4.0), f("height_r", 2.0), f("r", 0.5), u("corner_segs", 4),
        ),
        "slot" => slot_profile(f("length", 4.0), f("width", 1.0), u("arc_segs", 8)),
        "ellipse" => ellipse_profile(f("a", 2.0), f("b", 1.0), u("segments", 32)),
        "gear" => gear_profile_simple(f("tip_r", 2.0), f("root_r", 1.5), u("teeth", 12), f("duty", 0.5)),
        "airfoil" => airfoil_naca4_profile(f("thickness", 0.12), f("chord", 1.0), u("segments", 32)),
        "i_beam"  => i_beam_profile(f("section_h", 2.0), f("width", 1.0), f("flange_t", 0.1), f("web_t", 0.1)),
        "l_angle" => l_angle_profile(f("length", 1.0), f("section_h", 1.0), f("thickness", 0.1)),
        "c_channel" => c_channel_profile(f("section_h", 2.0), f("depth", 1.0), f("flange_t", 0.1), f("web_t", 0.1)),
        "t_beam"    => t_beam_profile(f("section_h", 1.0), f("width", 1.0), f("flange_t", 0.1), f("web_t", 0.1)),
        "z_section" => z_section_profile(f("section_h", 1.0), f("flange_len", 0.5), f("thickness", 0.1)),
        other => return RpcResponse::err(id, format!("unknown profile kind '{}'", other)),
    };
    if pts.is_empty() {
        return RpcResponse::err(id, "profile generator returned empty polygon (check params)");
    }
    let shape_id = match pocket_polygon_xy(&mut state.cad_doc.arena, &pts, depth) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("pocket failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id,
        "arena_id": shape_id.0,
        "kind": format!("pocket_{}", kind),
        "polygon_verts": pts.len(),
    }))
}

/// Shared mesh importer for OBJ / OFF / PLY / XYZ. Returns raw positions +
/// indices (no shape registered in the arena — these formats are mesh-only).
fn handle_cad_import_mesh(id: u64, params: &Value, kind: &str) -> RpcResponse {
    let path = match params.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return RpcResponse::err(id, "missing path"),
    };
    let mesh = match kind {
        "obj" => read_obj(std::path::Path::new(path)),
        "off" => read_off(std::path::Path::new(path)),
        "ply" => read_ply_ascii(std::path::Path::new(path)),
        "xyz" => read_xyz(std::path::Path::new(path)),
        _     => return RpcResponse::err(id, "internal: unknown mesh kind"),
    };
    match mesh {
        Ok(m) => {
            let positions: Vec<f32> = m.positions.iter().flat_map(|p| p.iter().copied()).collect();
            let normals:   Vec<f32> = m.normals.iter().flat_map(|n| n.iter().copied()).collect();
            RpcResponse::ok(id, serde_json::json!({
                "positions":      positions,
                "normals":        normals,
                "indices":        m.indices,
                "triangle_count": m.indices.len() / 3,
                "vertex_count":   m.positions.len(),
                "kind":           kind,
            }))
        }
        Err(e) => RpcResponse::err(id, format!("{} import failed: {}", kind, e)),
    }
}

fn handle_cad_import_stl(id: u64, params: &Value) -> RpcResponse {
    let path = match params.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return RpcResponse::err(id, "missing path"),
    };
    match read_stl(std::path::Path::new(path)) {
        Ok(mesh) => {
            let positions: Vec<f32> = mesh.positions.iter().flat_map(|p| p.iter().copied()).collect();
            let normals:   Vec<f32> = mesh.normals.iter().flat_map(|n| n.iter().copied()).collect();
            RpcResponse::ok(id, serde_json::json!({
                "positions": positions,
                "normals":   normals,
                "indices":   mesh.indices,
                "triangle_count": mesh.triangle_count(),
            }))
        }
        Err(e) => RpcResponse::err(id, format!("stl read failed: {}", e)),
    }
}

fn handle_cad_measure_polygon_area(id: u64, params: &Value) -> RpcResponse {
    let Some(arr) = params.get("points").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing points array");
    };
    let mut pts = Vec::with_capacity(arr.len());
    for p in arr {
        let Some(xy) = p.as_array() else { return RpcResponse::err(id, "point must be [x,y]"); };
        if xy.len() < 2 { return RpcResponse::err(id, "point must be [x,y]"); }
        pts.push((xy[0].as_f64().unwrap_or(0.0), xy[1].as_f64().unwrap_or(0.0)));
    }
    RpcResponse::ok(id, serde_json::json!({ "area": polygon_area(&pts) }))
}

/// Mesh-level quality report on a shape_id. Tessellates with the given
/// UV steps, optionally welds first, then reports edge-length stats,
/// aspect-ratio stats, Euler χ, `is_closed`, boundary / non-manifold
/// counts. Useful for CSG pre-flight and adaptive tessellation tuning.
fn handle_cad_measure_mesh_quality(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let weld_first = params.get("weld").and_then(|v| v.as_bool()).unwrap_or(true);
    let tol = params.get("tol").and_then(|v| v.as_f64()).unwrap_or(1.0e-4) as f32;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let mut mesh = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate failed: {}", e)),
    };
    if weld_first {
        mesh.weld(tol);
        mesh.prune_unused_vertices();
    }
    let edge_stats = trimesh_edge_length_stats(&mesh.positions, &mesh.indices);
    let ar_stats = trimesh_aspect_ratio_stats(&mesh.positions, &mesh.indices);
    let (chi, genus) = mesh_euler_genus(&mesh.positions, &mesh.indices);
    let boundary = trimesh_boundary_edges(&mesh.indices).len();
    let nonman = trimesh_non_manifold_edges(&mesh.indices).len();
    let closed = trimesh_is_closed(&mesh.indices);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id":          str_id,
        "vertex_count":      mesh.positions.len(),
        "triangle_count":    mesh.indices.len() / 3,
        "welded":            weld_first,
        "edge_length_stats":  edge_stats.map(|(mn, mx, m, s)| [mn, mx, m, s]),
        "aspect_ratio_stats": ar_stats.map(|(mn, mx, m)| [mn, mx, m]),
        "euler_chi":         chi,
        "genus_estimate":    genus,
        "boundary_edges":    boundary,
        "non_manifold":      nonman,
        "is_closed":         closed,
    }))
}

/// Batched `shape_summary` — evaluates the same 8 scalars across many
/// shape_ids in a single RPC. Unknown ids become entries with
/// `error: "unknown"` so callers can match by index.
fn handle_cad_measure_multi_shape_summary(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(arr) = params.get("shape_ids").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing shape_ids (array)");
    };
    let mut out: Vec<serde_json::Value> = Vec::with_capacity(arr.len());
    for v in arr {
        let Some(str_id) = v.as_str() else {
            out.push(serde_json::json!({ "error": "bad type" }));
            continue;
        };
        let Some(&aid) = state.cad_shape_map.get(str_id) else {
            out.push(serde_json::json!({ "shape_id": str_id, "error": "unknown" }));
            continue;
        };
        let arena = &state.cad_doc.arena;
        let area = surface_area(arena, aid).ok();
        let bbox_v = bbox_volume(arena, aid).ok();
        let vol = divergence_volume(arena, aid).ok();
        let com = center_of_mass(arena, aid).ok().map(|p| [p.x, p.y, p.z]);
        let inertia = inertia_tensor_full(arena, aid).ok()
            .map(|t| [t.0, t.1, t.2, t.3, t.4, t.5]);
        let bs = bounding_sphere(arena, aid).ok().map(|(c, r)| serde_json::json!({
            "center": [c.x, c.y, c.z], "radius": r,
        }));
        let elr = edge_length_range(arena, aid).ok().map(|(mn, mx)| [mn, mx]);
        let issues = check_validity(arena, aid).ok();
        let valid = issues.as_ref().map(|v| v.is_empty()).unwrap_or(false);
        let issue_count = issues.as_ref().map(|v| v.len()).unwrap_or(0);
        out.push(serde_json::json!({
            "shape_id":           str_id,
            "arena_id":           aid.0,
            "surface_area":       area,
            "bbox_volume":        bbox_v,
            "divergence_volume":  vol,
            "center_of_mass":     com,
            "inertia_tensor":     inertia,
            "bounding_sphere":    bs,
            "edge_length_range":  elr,
            "valid":              valid,
            "issues":             issue_count,
        }));
    }
    RpcResponse::ok(id, serde_json::json!({ "summaries": out, "count": arr.len() }))
}

/// One-call snapshot of every major scalar measurement for a shape.
/// Replaces a typical 8-RPC GUI call chain (surface_area, bbox_volume,
/// volume, com, inertia, bounding_sphere, edge_length_range, validity)
/// with a single round-trip. Missing / unsupported fields are `null`.
fn handle_cad_measure_shape_summary(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(&aid) = state.cad_shape_map.get(str_id) else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let arena = &state.cad_doc.arena;
    let area = surface_area(arena, aid).ok();
    let bbox_v = bbox_volume(arena, aid).ok();
    let vol = divergence_volume(arena, aid).ok();
    let com = center_of_mass(arena, aid).ok().map(|p| [p.x, p.y, p.z]);
    let inertia = inertia_tensor_full(arena, aid).ok().map(|t| [t.0, t.1, t.2, t.3, t.4, t.5]);
    let bs = bounding_sphere(arena, aid).ok().map(|(c, r)| serde_json::json!({
        "center": [c.x, c.y, c.z],
        "radius": r,
    }));
    let elr = edge_length_range(arena, aid).ok().map(|(mn, mx)| [mn, mx]);
    let issues = check_validity(arena, aid).ok();
    let valid = issues.as_ref().map(|v| v.is_empty()).unwrap_or(false);
    let issue_count = issues.as_ref().map(|v| v.len()).unwrap_or(0);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id":           str_id,
        "arena_id":           aid.0,
        "surface_area":       area,
        "bbox_volume":        bbox_v,
        "divergence_volume":  vol,
        "center_of_mass":     com,
        "inertia_tensor":     inertia,
        "bounding_sphere":    bs,
        "edge_length_range":  elr,
        "valid":              valid,
        "issues":             issue_count,
    }))
}

/// Vertex Hausdorff distance between two shapes. Both are tessellated
/// with default UV steps, then positions are fed to
/// `hausdorff_distance_vertex`. Useful for comparing original vs healed
/// vs CSG-resulted shapes.
fn handle_cad_measure_hausdorff(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(sa) = params.get("shape_a").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_a");
    };
    let Some(sb) = params.get("shape_b").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_b");
    };
    let Some(&aa) = state.cad_shape_map.get(sa) else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", sa));
    };
    let Some(&ab) = state.cad_shape_map.get(sb) else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", sb));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let ma = match tessellate(&state.cad_doc.arena, aa, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate A failed: {}", e)),
    };
    let mb = match tessellate(&state.cad_doc.arena, ab, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate B failed: {}", e)),
    };
    let d = hausdorff_distance_vertex(&ma.positions, &mb.positions);
    RpcResponse::ok(id, serde_json::json!({
        "distance":        d,
        "shape_a":         sa,
        "shape_b":         sb,
        "vertex_count_a":  ma.positions.len(),
        "vertex_count_b":  mb.positions.len(),
    }))
}

/// Lists every registered shape (shape_id → arena_id) in the live
/// document. Useful for GUI resync after external imports or on cold
/// reload — the Zustand store can compare against this snapshot.
fn handle_cad_arena_list_shapes(state: &ServerState, id: u64) -> RpcResponse {
    let mut entries: Vec<serde_json::Value> = state.cad_shape_map.iter().map(|(str_id, arena_id)| {
        let kind = match state.cad_doc.arena.get(*arena_id) {
            Ok(gfd_cad::topo::Shape::Compound { .. }) => "compound",
            Ok(gfd_cad::topo::Shape::Solid { .. })    => "solid",
            Ok(gfd_cad::topo::Shape::Shell { .. })    => "shell",
            Ok(gfd_cad::topo::Shape::Face { .. })     => "face",
            Ok(gfd_cad::topo::Shape::Wire { .. })     => "wire",
            Ok(gfd_cad::topo::Shape::Edge { .. })     => "edge",
            Ok(gfd_cad::topo::Shape::Vertex { .. })   => "vertex",
            Err(_) => "tombstoned",
        };
        serde_json::json!({
            "shape_id": str_id,
            "arena_id": arena_id.0,
            "kind":     kind,
        })
    }).collect();
    entries.sort_by(|a, b| a["arena_id"].as_u64().cmp(&b["arena_id"].as_u64()));
    RpcResponse::ok(id, serde_json::json!({
        "shapes": entries,
        "count":  state.cad_shape_map.len(),
    }))
}

/// Arena-wide kind histogram — counts every alive entity (ignoring
/// tombstones). Useful for debugging and scripted diagnostics.
fn handle_cad_arena_stats(state: &ServerState, id: u64) -> RpcResponse {
    let mut v = 0; let mut e = 0; let mut w = 0; let mut f = 0;
    let mut sh = 0; let mut so = 0; let mut cp = 0; let mut alive = 0;
    for i in 0..state.cad_doc.arena.len() {
        let sid = gfd_cad::topo::ShapeId(i as u32);
        match state.cad_doc.arena.get(sid) {
            Ok(gfd_cad::topo::Shape::Vertex { .. })   => { v += 1; alive += 1; }
            Ok(gfd_cad::topo::Shape::Edge { .. })     => { e += 1; alive += 1; }
            Ok(gfd_cad::topo::Shape::Wire { .. })     => { w += 1; alive += 1; }
            Ok(gfd_cad::topo::Shape::Face { .. })     => { f += 1; alive += 1; }
            Ok(gfd_cad::topo::Shape::Shell { .. })    => { sh += 1; alive += 1; }
            Ok(gfd_cad::topo::Shape::Solid { .. })    => { so += 1; alive += 1; }
            Ok(gfd_cad::topo::Shape::Compound { .. }) => { cp += 1; alive += 1; }
            Err(_) => {}
        }
    }
    RpcResponse::ok(id, serde_json::json!({
        "arena_len":       state.cad_doc.arena.len(),
        "alive":           alive,
        "tombstoned":      state.cad_doc.arena.len() - alive,
        "registered":      state.cad_shape_map.len(),
        "histogram": {
            "vertex": v, "edge": e, "wire": w, "face": f,
            "shell": sh, "solid": so, "compound": cp,
        },
    }))
}

/// Removes a shape from the arena (tombstones the slot) and drops the
/// GUI-side shape_id mapping. Safe no-op if the shape_id is unknown.
fn handle_cad_arena_delete_shape(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.remove(str_id) else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    match state.cad_doc.arena.remove(aid) {
        Ok(_) => RpcResponse::ok(id, serde_json::json!({
            "deleted": str_id,
            "arena_id": aid.0,
        })),
        Err(e) => RpcResponse::err(id, format!("arena remove failed: {}", e)),
    }
}

/// Detailed view of one shape: kind + recursive child kind histogram
/// (vertex/edge/wire/face/shell/solid counts).
fn handle_cad_arena_shape_info(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    use gfd_cad::topo::{collect_by_kind, ShapeKind};
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(&aid) = state.cad_shape_map.get(str_id) else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let v = collect_by_kind(&state.cad_doc.arena, aid, ShapeKind::Vertex).len();
    let e = collect_by_kind(&state.cad_doc.arena, aid, ShapeKind::Edge).len();
    let w = collect_by_kind(&state.cad_doc.arena, aid, ShapeKind::Wire).len();
    let f = collect_by_kind(&state.cad_doc.arena, aid, ShapeKind::Face).len();
    let sh = collect_by_kind(&state.cad_doc.arena, aid, ShapeKind::Shell).len();
    let so = collect_by_kind(&state.cad_doc.arena, aid, ShapeKind::Solid).len();
    let cp = collect_by_kind(&state.cad_doc.arena, aid, ShapeKind::Compound).len();
    let root_kind = match state.cad_doc.arena.get(aid) {
        Ok(gfd_cad::topo::Shape::Compound { .. }) => "compound",
        Ok(gfd_cad::topo::Shape::Solid { .. })    => "solid",
        Ok(gfd_cad::topo::Shape::Shell { .. })    => "shell",
        Ok(gfd_cad::topo::Shape::Face { .. })     => "face",
        Ok(gfd_cad::topo::Shape::Wire { .. })     => "wire",
        Ok(gfd_cad::topo::Shape::Edge { .. })     => "edge",
        Ok(gfd_cad::topo::Shape::Vertex { .. })   => "vertex",
        Err(_) => "tombstoned",
    };
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id,
        "arena_id": aid.0,
        "root_kind": root_kind,
        "histogram": {
            "vertex": v, "edge": e, "wire": w, "face": f,
            "shell": sh, "solid": so, "compound": cp,
        },
    }))
}

/// Batched ray-mesh intersection. Inputs: mesh (`positions`, `indices`) +
/// N rays as `origins` (3N flat) and `dirs` (3N flat). Returns per-ray
/// nearest hit `(t, triangle_index, u, v)`. `t = -1` + `tri = -1` if no
/// hit. Brute-force O(rays × tris) — adequate for picking/cursor UI.
fn handle_cad_measure_trimesh_raycast(id: u64, params: &Value) -> RpcResponse {
    let Some(pos_val) = params.get("positions").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing positions");
    };
    let Some(idx_val) = params.get("indices").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing indices");
    };
    let Some(o_val) = params.get("origins").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing origins (flat xyz)");
    };
    let Some(d_val) = params.get("dirs").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing dirs (flat xyz)");
    };
    if pos_val.len() % 3 != 0 || o_val.len() % 3 != 0 || d_val.len() % 3 != 0 {
        return RpcResponse::err(id, "flat xyz arrays must be multiples of 3");
    }
    if o_val.len() != d_val.len() {
        return RpcResponse::err(id, "origins / dirs must have equal length");
    }
    let positions: Vec<[f32; 3]> = pos_val.chunks(3).map(|c| [
        c[0].as_f64().unwrap_or(0.0) as f32,
        c[1].as_f64().unwrap_or(0.0) as f32,
        c[2].as_f64().unwrap_or(0.0) as f32,
    ]).collect();
    let indices: Vec<u32> = idx_val.iter().map(|v| v.as_u64().unwrap_or(0) as u32).collect();
    let n_rays = o_val.len() / 3;
    let mut t_out:  Vec<f64> = Vec::with_capacity(n_rays);
    let mut tri:    Vec<i64> = Vec::with_capacity(n_rays);
    let mut u:      Vec<f64> = Vec::with_capacity(n_rays);
    let mut v:      Vec<f64> = Vec::with_capacity(n_rays);
    for i in 0..n_rays {
        let o = [
            o_val[i * 3].as_f64().unwrap_or(0.0),
            o_val[i * 3 + 1].as_f64().unwrap_or(0.0),
            o_val[i * 3 + 2].as_f64().unwrap_or(0.0),
        ];
        let d = [
            d_val[i * 3].as_f64().unwrap_or(0.0),
            d_val[i * 3 + 1].as_f64().unwrap_or(0.0),
            d_val[i * 3 + 2].as_f64().unwrap_or(0.0),
        ];
        match trimesh_ray_intersect(o, d, &positions, &indices) {
            Some((t, ti, uu, vv)) => {
                t_out.push(t); tri.push(ti as i64); u.push(uu); v.push(vv);
            }
            None => {
                t_out.push(-1.0); tri.push(-1); u.push(0.0); v.push(0.0);
            }
        }
    }
    RpcResponse::ok(id, serde_json::json!({
        "t":  t_out,
        "triangle_index": tri,
        "u":  u,
        "v":  v,
        "ray_count": n_rays,
    }))
}

/// Batched TriMesh SDF evaluator. Accepts flat `positions` + `indices`
/// for the mesh, plus a flat `points` array of query points (3·N floats).
/// Returns per-query `(signed_distance, closest_x, closest_y, closest_z)`.
/// Sign: negative inside, positive outside. O(|query| × |tris|).
fn handle_cad_measure_trimesh_sdf(id: u64, params: &Value) -> RpcResponse {
    let Some(pos_val) = params.get("positions").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing positions");
    };
    let Some(idx_val) = params.get("indices").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing indices");
    };
    let Some(q_val) = params.get("points").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing points (flat xyz floats)");
    };
    if pos_val.len() % 3 != 0 || q_val.len() % 3 != 0 {
        return RpcResponse::err(id, "positions / points length must be multiple of 3");
    }
    let positions: Vec<[f32; 3]> = pos_val.chunks(3).map(|c| [
        c[0].as_f64().unwrap_or(0.0) as f32,
        c[1].as_f64().unwrap_or(0.0) as f32,
        c[2].as_f64().unwrap_or(0.0) as f32,
    ]).collect();
    let indices: Vec<u32> = idx_val.iter().map(|v| v.as_u64().unwrap_or(0) as u32).collect();
    let queries: Vec<[f64; 3]> = q_val.chunks(3).map(|c| [
        c[0].as_f64().unwrap_or(0.0),
        c[1].as_f64().unwrap_or(0.0),
        c[2].as_f64().unwrap_or(0.0),
    ]).collect();
    let mut sdf:       Vec<f64> = Vec::with_capacity(queries.len());
    let mut closest_x: Vec<f64> = Vec::with_capacity(queries.len());
    let mut closest_y: Vec<f64> = Vec::with_capacity(queries.len());
    let mut closest_z: Vec<f64> = Vec::with_capacity(queries.len());
    for q in &queries {
        let d = trimesh_signed_distance(*q, &positions, &indices).unwrap_or(f64::NAN);
        sdf.push(d);
        if let Some((p, _, _)) = trimesh_closest_point(*q, &positions, &indices) {
            closest_x.push(p[0]); closest_y.push(p[1]); closest_z.push(p[2]);
        } else {
            closest_x.push(f64::NAN); closest_y.push(f64::NAN); closest_z.push(f64::NAN);
        }
    }
    // Cheap second use of trimesh_point_inside (suppresses dead-code warning).
    let _ = trimesh_point_inside;
    RpcResponse::ok(id, serde_json::json!({
        "sdf":       sdf,
        "closest_x": closest_x,
        "closest_y": closest_y,
        "closest_z": closest_z,
        "query_count": queries.len(),
    }))
}

/// Batched TriMesh measurement for imported / tessellated meshes. Input:
/// `positions` flat Vec<f32>, `indices` Vec<u32>. Returns area, volume (if
/// closed), AABB, COM (if closed), diagonal inertia, boundary + non-
/// manifold edge counts, Euler χ, `is_closed`.
fn handle_cad_measure_trimesh_summary(id: u64, params: &Value) -> RpcResponse {
    let Some(pos_val) = params.get("positions").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing positions (flat float array)");
    };
    let Some(idx_val) = params.get("indices").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing indices (u32 array)");
    };
    if pos_val.len() % 3 != 0 {
        return RpcResponse::err(id, "positions length must be multiple of 3");
    }
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(pos_val.len() / 3);
    for chunk in pos_val.chunks(3) {
        positions.push([
            chunk[0].as_f64().unwrap_or(0.0) as f32,
            chunk[1].as_f64().unwrap_or(0.0) as f32,
            chunk[2].as_f64().unwrap_or(0.0) as f32,
        ]);
    }
    let indices: Vec<u32> = idx_val.iter()
        .map(|v| v.as_u64().unwrap_or(0) as u32)
        .collect();
    let area = trimesh_surface_area(&positions, &indices);
    let volume = trimesh_volume(&positions, &indices);
    let bbox = trimesh_bounding_box(&positions);
    let com = trimesh_center_of_mass(&positions, &indices);
    let inertia = trimesh_inertia_tensor(&positions, &indices);
    let boundary = trimesh_boundary_edges(&indices).len();
    let nonmanifold = trimesh_non_manifold_edges(&indices).len();
    let closed = trimesh_is_closed(&indices);
    let (chi, genus) = mesh_euler_genus(&positions, &indices);
    let edge_stats = trimesh_edge_length_stats(&positions, &indices);
    let ar_stats = trimesh_aspect_ratio_stats(&positions, &indices);
    let total_k = gfd_cad::measure::trimesh_total_gaussian_curvature(&positions, &indices);
    RpcResponse::ok(id, serde_json::json!({
        "area":           area,
        "volume":         volume,
        "bbox":           bbox.map(|(mn, mx)| [mn, mx]),
        "com":            com,
        "inertia":        inertia.map(|t| [t.0, t.1, t.2, t.3, t.4, t.5]),
        "boundary_edges": boundary,
        "non_manifold":   nonmanifold,
        "is_closed":      closed,
        "euler_chi":      chi,
        "genus_estimate": genus,
        "vertex_count":   positions.len(),
        "triangle_count": indices.len() / 3,
        "edge_length_stats":  edge_stats.map(|(mn, mx, mean, sd)| [mn, mx, mean, sd]),
        "aspect_ratio_stats": ar_stats.map(|(mn, mx, mean)| [mn, mx, mean]),
        "total_gaussian_curvature": total_k,
    }))
}

/// Batched point-in-polygon: returns a bool[] for each query point,
/// with `true` = inside the polygon (even-odd crossing rule).
fn handle_cad_measure_polygon_contains_point(id: u64, params: &Value) -> RpcResponse {
    let Some(arr) = params.get("polygon").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing polygon array");
    };
    let mut poly = Vec::with_capacity(arr.len());
    for p in arr {
        let Some(xy) = p.as_array() else { return RpcResponse::err(id, "point must be [x,y]"); };
        if xy.len() < 2 { return RpcResponse::err(id, "point must be [x,y]"); }
        poly.push((xy[0].as_f64().unwrap_or(0.0), xy[1].as_f64().unwrap_or(0.0)));
    }
    let Some(q_val) = params.get("points").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing points array (flat [x, y, x, y, ...])");
    };
    if q_val.len() % 2 != 0 {
        return RpcResponse::err(id, "points length must be even");
    }
    let results: Vec<bool> = q_val.chunks(2).map(|c| {
        let p = (c[0].as_f64().unwrap_or(0.0), c[1].as_f64().unwrap_or(0.0));
        polygon_contains_point(&poly, p)
    }).collect();
    RpcResponse::ok(id, serde_json::json!({
        "inside":     results,
        "query_count": q_val.len() / 2,
    }))
}

/// Returns the 2D convex hull vertices of the input polygon (Andrew's
/// monotone chain). Output `hull` is `[x, y, x, y, ...]` flat.
fn handle_cad_measure_polygon_convex_hull(id: u64, params: &Value) -> RpcResponse {
    let Some(arr) = params.get("points").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing points array");
    };
    let mut pts = Vec::with_capacity(arr.len());
    for p in arr {
        let Some(xy) = p.as_array() else { return RpcResponse::err(id, "point must be [x,y]"); };
        if xy.len() < 2 { return RpcResponse::err(id, "point must be [x,y]"); }
        pts.push((xy[0].as_f64().unwrap_or(0.0), xy[1].as_f64().unwrap_or(0.0)));
    }
    let hull = polygon_convex_hull(&pts);
    let flat: Vec<f64> = hull.iter().flat_map(|p| [p.0, p.1]).collect();
    RpcResponse::ok(id, serde_json::json!({
        "hull":       flat,
        "vertex_count": hull.len(),
    }))
}

/// Signed polygon area — positive = CCW, negative = CW. Useful for
/// orientation checks (`orientation: "ccw" | "cw" | "degenerate"`).
fn handle_cad_measure_polygon_signed_area(id: u64, params: &Value) -> RpcResponse {
    let Some(arr) = params.get("points").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing points array");
    };
    let mut pts = Vec::with_capacity(arr.len());
    for p in arr {
        let Some(xy) = p.as_array() else { return RpcResponse::err(id, "point must be [x,y]"); };
        if xy.len() < 2 { return RpcResponse::err(id, "point must be [x,y]"); }
        pts.push((xy[0].as_f64().unwrap_or(0.0), xy[1].as_f64().unwrap_or(0.0)));
    }
    let signed = polygon_area_signed(&pts);
    let orientation = if signed.abs() < 1e-12 { "degenerate" }
        else if signed > 0.0 { "ccw" } else { "cw" };
    RpcResponse::ok(id, serde_json::json!({
        "signed_area": signed,
        "area":        signed.abs(),
        "orientation": orientation,
    }))
}

/// Full 2D polygon measurement pack: area, perimeter, centroid, convex,
/// bbox, plus convex-hull vertex count. Single round-trip for dashboard-
/// style UI panels.
fn handle_cad_measure_polygon_full(id: u64, params: &Value) -> RpcResponse {
    let Some(arr) = params.get("points").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing points array");
    };
    let mut pts = Vec::with_capacity(arr.len());
    for p in arr {
        let Some(xy) = p.as_array() else { return RpcResponse::err(id, "point must be [x,y]"); };
        if xy.len() < 2 { return RpcResponse::err(id, "point must be [x,y]"); }
        pts.push((xy[0].as_f64().unwrap_or(0.0), xy[1].as_f64().unwrap_or(0.0)));
    }
    if pts.len() < 3 {
        return RpcResponse::err(id, "polygon needs at least 3 vertices");
    }
    let area = polygon_area(&pts);
    let perim = polygon_perimeter(&pts);
    let (cx, cy) = polygon_centroid(&pts);
    let convex = is_convex_polygon(&pts);
    let hull = polygon_convex_hull(&pts);
    let xs: Vec<f64> = pts.iter().map(|p| p.0).collect();
    let ys: Vec<f64> = pts.iter().map(|p| p.1).collect();
    let xmin = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let xmax = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let ymin = ys.iter().cloned().fold(f64::INFINITY, f64::min);
    let ymax = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    RpcResponse::ok(id, serde_json::json!({
        "area":         area,
        "perimeter":    perim,
        "centroid":     [cx, cy],
        "convex":       convex,
        "bbox":         [[xmin, ymin], [xmax, ymax]],
        "hull_vertex_count": hull.len(),
    }))
}

fn handle_cad_measure_bbox_volume(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(arena_id) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    match bbox_volume(&state.cad_doc.arena, arena_id) {
        Ok(v) => RpcResponse::ok(id, serde_json::json!({ "volume": v })),
        Err(e) => RpcResponse::err(id, format!("bbox_volume failed: {}", e)),
    }
}

fn sketch_mut<'a>(state: &'a mut ServerState, idx: usize) -> Option<&'a mut Sketch> {
    state.cad_doc.sketches.get_mut(idx)
}

fn handle_cad_sketch_new(state: &mut ServerState, id: u64) -> RpcResponse {
    state.cad_doc.sketches.push(Sketch::new());
    let idx = state.cad_doc.sketches.len() - 1;
    RpcResponse::ok(id, serde_json::json!({ "sketch_idx": idx }))
}

fn sketch_idx_from(params: &Value) -> Option<usize> {
    params.get("sketch_idx").and_then(|v| v.as_u64()).map(|u| u as usize)
}

fn handle_cad_sketch_add_point(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(idx) = sketch_idx_from(params) else { return RpcResponse::err(id, "missing sketch_idx"); };
    let x = params.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let y = params.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let Some(sk) = sketch_mut(state, idx) else { return RpcResponse::err(id, format!("sketch {} not found", idx)); };
    let pid = sk.add_point(Point2::new(x, y));
    RpcResponse::ok(id, serde_json::json!({ "point_id": pid.0 }))
}

/// Inject a profile-generator output directly into a sketch. Accepts
/// all 13 pad_profile kinds; the resulting polygon is pushed as
/// N points + N closing line entities.
fn handle_cad_sketch_add_profile(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(idx) = sketch_idx_from(params) else { return RpcResponse::err(id, "missing sketch_idx"); };
    let kind = params.get("profile").and_then(|v| v.as_str()).unwrap_or("");
    let f = |k: &str, d: f64| params.get(k).and_then(|v| v.as_f64()).unwrap_or(d);
    let u = |k: &str, d: u64| params.get(k).and_then(|v| v.as_u64()).unwrap_or(d) as usize;
    let pts: Vec<(f64, f64)> = match kind {
        "ngon" => regular_ngon_profile(f("radius", 1.0), u("sides", 6), f("start_deg", 0.0)),
        "star" => star_profile(f("outer_r", 1.0), f("inner_r", 0.5), u("points", 5)),
        "rectangle" => rectangle_profile(f("width", 2.0), f("height_r", 1.0)),
        "rounded_rectangle" => rounded_rectangle_profile(
            f("width", 4.0), f("height_r", 2.0), f("r", 0.5), u("corner_segs", 4),
        ),
        "slot" => slot_profile(f("length", 4.0), f("width", 1.0), u("arc_segs", 8)),
        "ellipse" => ellipse_profile(f("a", 2.0), f("b", 1.0), u("segments", 32)),
        "gear" => gear_profile_simple(f("tip_r", 2.0), f("root_r", 1.5), u("teeth", 12), f("duty", 0.5)),
        "airfoil" => airfoil_naca4_profile(f("thickness", 0.12), f("chord", 1.0), u("segments", 32)),
        "i_beam"  => i_beam_profile(f("section_h", 2.0), f("width", 1.0), f("flange_t", 0.1), f("web_t", 0.1)),
        "l_angle" => l_angle_profile(f("length", 1.0), f("section_h", 1.0), f("thickness", 0.1)),
        "c_channel" => c_channel_profile(f("section_h", 2.0), f("depth", 1.0), f("flange_t", 0.1), f("web_t", 0.1)),
        "t_beam" => t_beam_profile(f("section_h", 1.0), f("width", 1.0), f("flange_t", 0.1), f("web_t", 0.1)),
        "z_section" => z_section_profile(f("section_h", 1.0), f("flange_len", 0.5), f("thickness", 0.1)),
        other => return RpcResponse::err(id, format!("unknown profile kind '{}'", other)),
    };
    if pts.is_empty() {
        return RpcResponse::err(id, "profile returned empty polygon");
    }
    let Some(sk) = sketch_mut(state, idx) else { return RpcResponse::err(id, format!("sketch {} not found", idx)); };
    let mut point_ids: Vec<u32> = Vec::with_capacity(pts.len());
    for (x, y) in &pts {
        let pid = sk.add_point(gfd_cad::sketch::Point2 { x: *x, y: *y });
        point_ids.push(pid.0);
    }
    let mut entity_ids: Vec<u32> = Vec::new();
    for i in 0..pts.len() {
        let j = (i + 1) % pts.len();
        let eid = sk.add_line(SkPid(point_ids[i]), SkPid(point_ids[j]));
        entity_ids.push(eid.0);
    }
    RpcResponse::ok(id, serde_json::json!({
        "profile":    kind,
        "point_ids":  point_ids,
        "entity_ids": entity_ids,
        "vertex_count": pts.len(),
    }))
}

/// Bulk add a polyline to a sketch: given `points` (flat [x0, y0, x1, y1,
/// ...]), creates a sketcher point for each XY pair and then adds line
/// entities connecting consecutive points. If `closed = true`, a final
/// segment joins the last to the first. Returns the new point_ids and
/// entity_ids so callers can reference them in subsequent constraints.
fn handle_cad_sketch_add_polyline(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(idx) = sketch_idx_from(params) else { return RpcResponse::err(id, "missing sketch_idx"); };
    let Some(arr) = params.get("points").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing points (flat [x, y, x, y, ...])");
    };
    if arr.len() % 2 != 0 || arr.len() < 4 {
        return RpcResponse::err(id, "points must have an even length ≥ 4");
    }
    let closed = params.get("closed").and_then(|v| v.as_bool()).unwrap_or(false);
    let Some(sk) = sketch_mut(state, idx) else { return RpcResponse::err(id, format!("sketch {} not found", idx)); };
    let n = arr.len() / 2;
    let mut point_ids: Vec<u32> = Vec::with_capacity(n);
    for i in 0..n {
        let x = arr[i * 2].as_f64().unwrap_or(0.0);
        let y = arr[i * 2 + 1].as_f64().unwrap_or(0.0);
        let pid = sk.add_point(gfd_cad::sketch::Point2 { x, y });
        point_ids.push(pid.0);
    }
    let mut entity_ids: Vec<u32> = Vec::new();
    for i in 0..n - 1 {
        let eid = sk.add_line(SkPid(point_ids[i]), SkPid(point_ids[i + 1]));
        entity_ids.push(eid.0);
    }
    if closed {
        let eid = sk.add_line(SkPid(point_ids[n - 1]), SkPid(point_ids[0]));
        entity_ids.push(eid.0);
    }
    RpcResponse::ok(id, serde_json::json!({
        "point_ids":  point_ids,
        "entity_ids": entity_ids,
        "closed":     closed,
    }))
}

fn handle_cad_sketch_add_line(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(idx) = sketch_idx_from(params) else { return RpcResponse::err(id, "missing sketch_idx"); };
    let a = params.get("a").and_then(|v| v.as_u64()).map(|u| SkPid(u as u32));
    let b = params.get("b").and_then(|v| v.as_u64()).map(|u| SkPid(u as u32));
    let (Some(a), Some(b)) = (a, b) else { return RpcResponse::err(id, "missing a or b point id"); };
    let Some(sk) = sketch_mut(state, idx) else { return RpcResponse::err(id, format!("sketch {} not found", idx)); };
    let eid = sk.add_line(a, b);
    RpcResponse::ok(id, serde_json::json!({ "entity_id": eid.0 }))
}

fn handle_cad_sketch_add_arc(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(idx) = sketch_idx_from(params) else { return RpcResponse::err(id, "missing sketch_idx"); };
    let center = params.get("center").and_then(|v| v.as_u64()).map(|u| SkPid(u as u32));
    let start  = params.get("start").and_then(|v| v.as_u64()).map(|u| SkPid(u as u32));
    let end    = params.get("end").and_then(|v| v.as_u64()).map(|u| SkPid(u as u32));
    let (Some(c), Some(s), Some(e)) = (center, start, end) else { return RpcResponse::err(id, "need center/start/end point ids"); };
    let Some(sk) = sketch_mut(state, idx) else { return RpcResponse::err(id, format!("sketch {} not found", idx)); };
    let eid = sk.add_arc(c, s, e);
    RpcResponse::ok(id, serde_json::json!({ "entity_id": eid.0 }))
}

fn handle_cad_sketch_add_circle(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(idx) = sketch_idx_from(params) else { return RpcResponse::err(id, "missing sketch_idx"); };
    let center = params.get("center").and_then(|v| v.as_u64()).map(|u| SkPid(u as u32));
    let radius = params.get("radius").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let Some(c) = center else { return RpcResponse::err(id, "need center point id"); };
    let Some(sk) = sketch_mut(state, idx) else { return RpcResponse::err(id, format!("sketch {} not found", idx)); };
    let eid = sk.add_circle(c, radius);
    RpcResponse::ok(id, serde_json::json!({ "entity_id": eid.0 }))
}

fn handle_cad_sketch_add_constraint(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(idx) = sketch_idx_from(params) else { return RpcResponse::err(id, "missing sketch_idx"); };
    let kind = params.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let get_pid = |k: &str| params.get(k).and_then(|v| v.as_u64()).map(|u| SkPid(u as u32));
    let get_eid = |k: &str| params.get(k).and_then(|v| v.as_u64()).map(|u| SkEid(u as u32));
    let get_f64 = |k: &str| params.get(k).and_then(|v| v.as_f64());
    let c = match kind {
        "coincident"   => if let (Some(a), Some(b)) = (get_pid("a"), get_pid("b")) { SkCons::Coincident(a, b) } else { return RpcResponse::err(id, "need a,b"); },
        "fix"          => if let (Some(p), Some(x), Some(y)) = (get_pid("point"), get_f64("x"), get_f64("y")) { SkCons::Fix { point: p, x, y } } else { return RpcResponse::err(id, "need point,x,y"); },
        "horizontal"   => if let Some(l) = get_eid("line") { SkCons::Horizontal { line: l } } else { return RpcResponse::err(id, "need line"); },
        "vertical"     => if let Some(l) = get_eid("line") { SkCons::Vertical { line: l } } else { return RpcResponse::err(id, "need line"); },
        "distance"     => if let (Some(a), Some(b), Some(v)) = (get_pid("a"), get_pid("b"), get_f64("value")) { SkCons::Distance { a, b, value: v } } else { return RpcResponse::err(id, "need a,b,value"); },
        "parallel"     => if let (Some(l1), Some(l2)) = (get_eid("l1"), get_eid("l2")) { SkCons::Parallel { l1, l2 } } else { return RpcResponse::err(id, "need l1,l2"); },
        "perpendicular"=> if let (Some(l1), Some(l2)) = (get_eid("l1"), get_eid("l2")) { SkCons::Perpendicular { l1, l2 } } else { return RpcResponse::err(id, "need l1,l2"); },
        "point_on_line"=> if let (Some(p), Some(l)) = (get_pid("point"), get_eid("line")) { SkCons::PointOnLine { point: p, line: l } } else { return RpcResponse::err(id, "need point,line"); },
        "point_on_circle" => if let (Some(p), Some(c)) = (get_pid("point"), get_eid("circle")) { SkCons::PointOnCircle { point: p, circle: c } } else { return RpcResponse::err(id, "need point,circle"); },
        "radius"        => if let (Some(c), Some(v)) = (get_eid("circle"), get_f64("value")) { SkCons::Radius { circle: c, value: v } } else { return RpcResponse::err(id, "need circle,value"); },
        "equal_length"  => if let (Some(a1), Some(b1), Some(a2), Some(b2)) = (get_pid("a1"), get_pid("b1"), get_pid("a2"), get_pid("b2")) { SkCons::EqualLength { a1, b1, a2, b2 } } else { return RpcResponse::err(id, "need a1,b1,a2,b2"); },
        "angle"         => if let (Some(l1), Some(l2), Some(v)) = (get_eid("l1"), get_eid("l2"), get_f64("value")) { SkCons::Angle { l1, l2, value: v } } else { return RpcResponse::err(id, "need l1,l2,value"); },
        "arc_closed"    => if let Some(a) = get_eid("arc") { SkCons::ArcClosed { arc: a } } else { return RpcResponse::err(id, "need arc"); },
        "arc_length"    => if let (Some(a), Some(v)) = (get_eid("arc"), get_f64("value")) { SkCons::ArcLength { arc: a, value: v } } else { return RpcResponse::err(id, "need arc,value"); },
        "tangent_line_circle" => if let (Some(l), Some(c)) = (get_eid("line"), get_eid("circle")) { SkCons::TangentLineCircle { line: l, circle: c } } else { return RpcResponse::err(id, "need line,circle"); },
        "symmetric"     => if let (Some(a), Some(b), Some(m)) = (get_pid("a"), get_pid("b"), get_pid("midpoint")) { SkCons::Symmetric { a, b, midpoint: m } } else { return RpcResponse::err(id, "need a,b,midpoint"); },
        "distance_point_line" => if let (Some(p), Some(l), Some(v)) = (get_pid("point"), get_eid("line"), get_f64("value")) { SkCons::DistancePointLine { point: p, line: l, value: v } } else { return RpcResponse::err(id, "need point,line,value"); },
        other => return RpcResponse::err(id, format!("unknown constraint kind: {}", other)),
    };
    let Some(sk) = sketch_mut(state, idx) else { return RpcResponse::err(id, format!("sketch {} not found", idx)); };
    sk.add_constraint(c);
    RpcResponse::ok(id, serde_json::json!({ "ok": true, "constraint_count": sk.constraints.len() }))
}

fn handle_cad_sketch_solve(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(idx) = sketch_idx_from(params) else { return RpcResponse::err(id, "missing sketch_idx"); };
    let tol = params.get("tolerance").and_then(|v| v.as_f64()).unwrap_or(1.0e-8);
    let max_iters = params.get("max_iters").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
    let Some(sk) = sketch_mut(state, idx) else { return RpcResponse::err(id, format!("sketch {} not found", idx)); };
    match sk.solve(tol, max_iters) {
        Ok(norm) => RpcResponse::ok(id, serde_json::json!({
            "residual": norm,
            "points": sk.points.iter().map(|p| serde_json::json!([p.x, p.y])).collect::<Vec<_>>(),
        })),
        Err(e) => RpcResponse::err(id, format!("solve failed: {}", e)),
    }
}

fn handle_cad_sketch_dof(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(idx) = sketch_idx_from(params) else { return RpcResponse::err(id, "missing sketch_idx"); };
    let Some(sk) = state.cad_doc.sketches.get(idx) else { return RpcResponse::err(id, format!("sketch {} not found", idx)); };
    let status = sk.dof_status();
    let (kind, dof, residuals) = match status {
        gfd_cad::sketch::DofStatus::UnderConstrained { dof, residuals } => ("under", dof, residuals),
        gfd_cad::sketch::DofStatus::WellConstrained { dof } => ("well", dof, dof),
        gfd_cad::sketch::DofStatus::OverConstrained { dof, residuals } => ("over", dof, residuals),
    };
    RpcResponse::ok(id, serde_json::json!({
        "status": kind,
        "dof": dof,
        "residuals": residuals,
    }))
}

/// List all sketches in the document. Returns a compact summary array.
fn handle_cad_sketch_list(state: &ServerState, id: u64) -> RpcResponse {
    let rows: Vec<serde_json::Value> = state.cad_doc.sketches.iter().enumerate().map(|(i, sk)| {
        serde_json::json!({
            "index":            i,
            "point_count":      sk.points.len(),
            "entity_count":     sk.entities.len(),
            "constraint_count": sk.constraints.len(),
        })
    }).collect();
    RpcResponse::ok(id, serde_json::json!({
        "sketches": rows,
        "count":    state.cad_doc.sketches.len(),
    }))
}

/// Removes the sketch at `sketch_idx` (by value). Subsequent indices
/// shift down — callers should treat sketch ids as transient.
fn handle_cad_sketch_delete(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(idx) = params.get("sketch_idx").and_then(|v| v.as_u64()).map(|v| v as usize) else {
        return RpcResponse::err(id, "missing sketch_idx");
    };
    if idx >= state.cad_doc.sketches.len() {
        return RpcResponse::err(id, format!("sketch_idx {} out of range (len={})", idx, state.cad_doc.sketches.len()));
    }
    state.cad_doc.sketches.remove(idx);
    RpcResponse::ok(id, serde_json::json!({
        "deleted": idx,
        "remaining": state.cad_doc.sketches.len(),
    }))
}

fn handle_cad_sketch_get(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(idx) = sketch_idx_from(params) else { return RpcResponse::err(id, "missing sketch_idx"); };
    let Some(sk) = state.cad_doc.sketches.get(idx) else { return RpcResponse::err(id, format!("sketch {} not found", idx)); };
    RpcResponse::ok(id, serde_json::json!({
        "points": sk.points.iter().map(|p| serde_json::json!([p.x, p.y])).collect::<Vec<_>>(),
        "entity_count": sk.entities.len(),
        "constraint_count": sk.constraints.len(),
    }))
}

fn parse_raw_trimesh(params: &Value) -> Result<TriMesh, String> {
    let pos = params.get("positions").and_then(|v| v.as_array())
        .ok_or_else(|| "missing positions".to_string())?;
    let idx = params.get("indices").and_then(|v| v.as_array())
        .ok_or_else(|| "missing indices".to_string())?;
    if pos.len() % 3 != 0 {
        return Err("positions length must be multiple of 3".into());
    }
    let positions: Vec<[f32; 3]> = pos.chunks(3).map(|c| [
        c[0].as_f64().unwrap_or(0.0) as f32,
        c[1].as_f64().unwrap_or(0.0) as f32,
        c[2].as_f64().unwrap_or(0.0) as f32,
    ]).collect();
    let indices: Vec<u32> = idx.iter().map(|v| v.as_u64().unwrap_or(0) as u32).collect();
    Ok(TriMesh { positions, normals: vec![], indices })
}

fn dump_trimesh(id: u64, mesh: &TriMesh, extra: serde_json::Value) -> RpcResponse {
    let positions: Vec<f32> = mesh.positions.iter().flat_map(|p| p.iter().copied()).collect();
    let normals:   Vec<f32> = mesh.normals.iter().flat_map(|n| n.iter().copied()).collect();
    let mut v = serde_json::json!({
        "positions":      positions,
        "normals":        normals,
        "indices":        mesh.indices,
        "triangle_count": mesh.indices.len() / 3,
        "vertex_count":   mesh.positions.len(),
    });
    if let serde_json::Value::Object(ref mut map) = v {
        if let serde_json::Value::Object(ex) = extra {
            for (k, val) in ex { map.insert(k, val); }
        }
    }
    RpcResponse::ok(id, v)
}

fn handle_cad_mesh_smooth(id: u64, params: &Value) -> RpcResponse {
    let mut m = match parse_raw_trimesh(params) { Ok(m) => m, Err(e) => return RpcResponse::err(id, e) };
    let iters = params.get("iterations").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
    let factor = params.get("factor").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
    m.laplacian_smooth(iters, factor);
    dump_trimesh(id, &m, serde_json::json!({ "iterations": iters, "factor": factor }))
}

fn handle_cad_mesh_subdivide(id: u64, params: &Value) -> RpcResponse {
    let mut m = match parse_raw_trimesh(params) { Ok(m) => m, Err(e) => return RpcResponse::err(id, e) };
    m.subdivide_midpoint();
    dump_trimesh(id, &m, serde_json::json!({}))
}

/// Concatenate multiple raw TriMesh inputs into one. Input: `meshes`
/// array, each element `{positions, indices}`. Indices are re-offset.
fn handle_cad_mesh_concat(id: u64, params: &Value) -> RpcResponse {
    let Some(arr) = params.get("meshes").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing meshes (array of {positions, indices})");
    };
    let mut out_positions: Vec<[f32; 3]> = Vec::new();
    let mut out_indices:   Vec<u32> = Vec::new();
    for entry in arr {
        let pos = entry.get("positions").and_then(|v| v.as_array());
        let idx = entry.get("indices").and_then(|v| v.as_array());
        let (Some(pos), Some(idx)) = (pos, idx) else { continue; };
        if pos.len() % 3 != 0 { continue; }
        let offset = out_positions.len() as u32;
        for c in pos.chunks(3) {
            out_positions.push([
                c[0].as_f64().unwrap_or(0.0) as f32,
                c[1].as_f64().unwrap_or(0.0) as f32,
                c[2].as_f64().unwrap_or(0.0) as f32,
            ]);
        }
        for v in idx {
            out_indices.push(v.as_u64().unwrap_or(0) as u32 + offset);
        }
    }
    let mesh = TriMesh { positions: out_positions, normals: vec![], indices: out_indices };
    dump_trimesh(id, &mesh, serde_json::json!({ "merged_count": arr.len() }))
}

fn handle_cad_mesh_transform(id: u64, params: &Value) -> RpcResponse {
    let mut m = match parse_raw_trimesh(params) { Ok(m) => m, Err(e) => return RpcResponse::err(id, e) };
    let Some(arr) = params.get("matrix").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing matrix (flat 16 floats, row-major)");
    };
    if arr.len() != 16 {
        return RpcResponse::err(id, "matrix must have 16 entries (4x4 row-major)");
    }
    let mut mat = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            mat[i][j] = arr[i * 4 + j].as_f64().unwrap_or(0.0);
        }
    }
    m.transform(mat);
    dump_trimesh(id, &m, serde_json::json!({}))
}

fn handle_cad_mesh_compute_normals(id: u64, params: &Value) -> RpcResponse {
    let mut m = match parse_raw_trimesh(params) { Ok(m) => m, Err(e) => return RpcResponse::err(id, e) };
    m.compute_smooth_normals();
    dump_trimesh(id, &m, serde_json::json!({}))
}

fn handle_cad_mesh_reverse_winding(id: u64, params: &Value) -> RpcResponse {
    let mut m = match parse_raw_trimesh(params) { Ok(m) => m, Err(e) => return RpcResponse::err(id, e) };
    m.reverse_winding();
    dump_trimesh(id, &m, serde_json::json!({}))
}

fn handle_cad_mesh_weld(id: u64, params: &Value) -> RpcResponse {
    let mut m = match parse_raw_trimesh(params) { Ok(m) => m, Err(e) => return RpcResponse::err(id, e) };
    let tol = params.get("tol").and_then(|v| v.as_f64()).unwrap_or(1.0e-4) as f32;
    let removed = m.weld(tol);
    let pruned = m.prune_unused_vertices();
    dump_trimesh(id, &m, serde_json::json!({
        "welded_removed": removed,
        "pruned":         pruned,
        "tol":            tol,
    }))
}

/// Mesh boolean on raw TriMesh inputs (imported STL/OBJ/PLY etc.) —
/// bypasses the arena. Params: `a_positions` + `a_indices`, `b_positions`
/// + `b_indices`, `op` ∈ {"union","difference","intersection"}.
fn handle_cad_mesh_boolean_raw(id: u64, params: &Value) -> RpcResponse {
    let parse_mesh = |pos_key: &str, idx_key: &str| -> Result<TriMesh, String> {
        let pos = params.get(pos_key).and_then(|v| v.as_array())
            .ok_or_else(|| format!("missing {}", pos_key))?;
        let idx = params.get(idx_key).and_then(|v| v.as_array())
            .ok_or_else(|| format!("missing {}", idx_key))?;
        if pos.len() % 3 != 0 {
            return Err(format!("{} length must be multiple of 3", pos_key));
        }
        let positions: Vec<[f32; 3]> = pos.chunks(3).map(|c| [
            c[0].as_f64().unwrap_or(0.0) as f32,
            c[1].as_f64().unwrap_or(0.0) as f32,
            c[2].as_f64().unwrap_or(0.0) as f32,
        ]).collect();
        let indices: Vec<u32> = idx.iter().map(|v| v.as_u64().unwrap_or(0) as u32).collect();
        Ok(TriMesh { positions, normals: vec![], indices })
    };
    let a = match parse_mesh("a_positions", "a_indices") { Ok(m) => m, Err(e) => return RpcResponse::err(id, e) };
    let b = match parse_mesh("b_positions", "b_indices") { Ok(m) => m, Err(e) => return RpcResponse::err(id, e) };
    let op = match params.get("op").and_then(|v| v.as_str()).unwrap_or("") {
        "union"        => MeshOp::Union,
        "difference"   => MeshOp::Difference,
        "intersection" => MeshOp::Intersection,
        other          => return RpcResponse::err(id, format!("unknown op '{}'", other)),
    };
    let out = mesh_boolean(&a, &b, op);
    let positions: Vec<f32> = out.positions.iter().flat_map(|p| p.iter().copied()).collect();
    let normals:   Vec<f32> = out.normals.iter().flat_map(|n| n.iter().copied()).collect();
    RpcResponse::ok(id, serde_json::json!({
        "positions":      positions,
        "normals":        normals,
        "indices":        out.indices,
        "triangle_count": out.indices.len() / 3,
        "vertex_count":   out.positions.len(),
    }))
}

fn handle_cad_boolean_mesh(state: &mut ServerState, id: u64, params: &Value, op: MeshOp) -> RpcResponse {
    let Some(a_id) = params.get("a").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing a (shape_id)");
    };
    let Some(b_id) = params.get("b").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing b (shape_id)");
    };
    let Some(aa) = state.cad_shape_map.get(a_id).copied() else {
        return RpcResponse::err(id, format!("unknown a: {}", a_id));
    };
    let Some(bb) = state.cad_shape_map.get(b_id).copied() else {
        return RpcResponse::err(id, format!("unknown b: {}", b_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(8) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let mesh_a = match tessellate(&state.cad_doc.arena, aa, opts) {
        Ok(m) => m, Err(e) => return RpcResponse::err(id, format!("tess A: {}", e)),
    };
    let mesh_b = match tessellate(&state.cad_doc.arena, bb, opts) {
        Ok(m) => m, Err(e) => return RpcResponse::err(id, format!("tess B: {}", e)),
    };
    let out = mesh_boolean(&mesh_a, &mesh_b, op);
    let positions: Vec<f32> = out.positions.iter().flat_map(|p| p.iter().copied()).collect();
    let normals:   Vec<f32> = out.normals.iter().flat_map(|n| n.iter().copied()).collect();
    RpcResponse::ok(id, serde_json::json!({
        "positions": positions,
        "normals":   normals,
        "indices":   out.indices,
        "triangle_count": out.indices.len() / 3,
    }))
}

fn handle_cad_boolean_union(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(arr) = params.get("shape_ids").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing shape_ids (array of strings)");
    };
    let mut arena_ids = Vec::with_capacity(arr.len());
    for s in arr {
        let Some(str_id) = s.as_str() else { return RpcResponse::err(id, "shape id must be string"); };
        let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
            return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
        };
        arena_ids.push(aid);
    }
    let merged = match compound_merge(&mut state.cad_doc.arena, &arena_ids) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("union failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), merged);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id,
        "arena_id": merged.0,
        "kind": "compound",
    }))
}

fn handle_cad_heal_fix(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(arena_id) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let tol = params.get("tolerance").and_then(|v| v.as_f64()).unwrap_or(1.0e-7);
    let sew = params.get("sew").and_then(|v| v.as_bool()).unwrap_or(true);
    let fix_wires = params.get("fix_wires").and_then(|v| v.as_bool()).unwrap_or(false);
    let remove_small = params.get("remove_small").and_then(|v| v.as_bool()).unwrap_or(true);
    let remove_dup_faces = params.get("remove_duplicate_faces").and_then(|v| v.as_bool()).unwrap_or(false);
    let opts = HealOptions {
        tolerance: tol,
        sew_faces: sew,
        fix_wires,
        remove_small_edges: remove_small,
        unify_tolerances: false,
        remove_duplicate_faces: remove_dup_faces,
    };
    match fix_shape(&mut state.cad_doc.arena, arena_id, &opts) {
        Ok(log) => RpcResponse::ok(id, serde_json::json!({ "log": log })),
        Err(e) => RpcResponse::err(id, format!("heal fix failed: {}", e)),
    }
}

fn handle_cad_heal_stats(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    match shape_stats(&state.cad_doc.arena, aid) {
        Ok(s) => RpcResponse::ok(id, serde_json::json!({
            "vertices": s.vertices,
            "edges":    s.edges,
            "wires":    s.wires,
            "faces":    s.faces,
            "shells":   s.shells,
            "solids":   s.solids,
        })),
        Err(e) => RpcResponse::err(id, format!("shape_stats failed: {}", e)),
    }
}

fn handle_cad_heal_check(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(arena_id) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    match check_validity(&state.cad_doc.arena, arena_id) {
        Ok(issues) => {
            let list: Vec<_> = issues.iter().map(|i| serde_json::json!({
                "arena_id": i.shape_id,
                "kind": i.kind,
                "detail": i.detail,
            })).collect();
            RpcResponse::ok(id, serde_json::json!({ "issues": list, "valid": issues.is_empty() }))
        }
        Err(e) => RpcResponse::err(id, format!("heal check failed: {}", e)),
    }
}

fn resolve_shape_id(state: &ServerState, params: &Value) -> Result<ShapeId, String> {
    let str_id = params.get("shape_id").and_then(|v| v.as_str()).ok_or("missing shape_id")?;
    state.cad_shape_map.get(str_id).copied().ok_or_else(|| format!("unknown shape_id: {}", str_id))
}

fn register_cad(state: &mut ServerState, arena_id: ShapeId, kind: &str) -> (String, ShapeId) {
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), arena_id);
    let _ = kind;
    (str_id, arena_id)
}

fn handle_cad_feature_pyramid(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let lx = params.get("lx").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let ly = params.get("ly").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let h  = params.get("height").and_then(|v| v.as_f64()).unwrap_or(1.0);
    match pyramid_solid(&mut state.cad_doc.arena, lx, ly, h) {
        Ok(new_id) => {
            let (s, _) = register_cad(state, new_id, "pyramid");
            RpcResponse::ok(id, serde_json::json!({ "shape_id": s, "arena_id": new_id.0, "kind": "pyramid" }))
        }
        Err(e) => RpcResponse::err(id, format!("pyramid failed: {}", e)),
    }
}

fn handle_cad_feature_ngon_prism(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let sides = params.get("sides").and_then(|v| v.as_u64()).unwrap_or(6) as usize;
    let r = params.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let h = params.get("height").and_then(|v| v.as_f64()).unwrap_or(1.0);
    match ngon_prism_solid(&mut state.cad_doc.arena, sides, r, h) {
        Ok(new_id) => {
            let (s, _) = register_cad(state, new_id, "ngon_prism");
            RpcResponse::ok(id, serde_json::json!({ "shape_id": s, "arena_id": new_id.0, "kind": "ngon_prism" }))
        }
        Err(e) => RpcResponse::err(id, format!("ngon_prism failed: {}", e)),
    }
}

fn handle_cad_feature_wedge(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let lx = params.get("lx").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let ly = params.get("ly").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let lz = params.get("lz").and_then(|v| v.as_f64()).unwrap_or(1.0);
    match wedge_solid(&mut state.cad_doc.arena, lx, ly, lz) {
        Ok(new_id) => {
            let (s, _) = register_cad(state, new_id, "wedge");
            RpcResponse::ok(id, serde_json::json!({ "shape_id": s, "arena_id": new_id.0, "kind": "wedge" }))
        }
        Err(e) => RpcResponse::err(id, format!("wedge failed: {}", e)),
    }
}

fn handle_cad_measure_bsphere(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let aid = match resolve_shape_id(state, params) { Ok(x) => x, Err(e) => return RpcResponse::err(id, e) };
    match bounding_sphere(&state.cad_doc.arena, aid) {
        Ok((c, r)) => RpcResponse::ok(id, serde_json::json!({ "x": c.x, "y": c.y, "z": c.z, "radius": r })),
        Err(e) => RpcResponse::err(id, format!("bounding_sphere failed: {}", e)),
    }
}

fn handle_cad_measure_pa(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let aid = match resolve_shape_id(state, params) { Ok(x) => x, Err(e) => return RpcResponse::err(id, e) };
    match principal_axes(&state.cad_doc.arena, aid) {
        Ok((i1, i2, i3, vecs)) => RpcResponse::ok(id, serde_json::json!({
            "moments": [i1, i2, i3],
            "axes": vecs,
        })),
        Err(e) => RpcResponse::err(id, format!("principal_axes failed: {}", e)),
    }
}

fn handle_cad_measure_elr(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let aid = match resolve_shape_id(state, params) { Ok(x) => x, Err(e) => return RpcResponse::err(id, e) };
    match edge_length_range(&state.cad_doc.arena, aid) {
        Ok((mn, mx)) => RpcResponse::ok(id, serde_json::json!({ "min": mn, "max": mx })),
        Err(e) => RpcResponse::err(id, format!("edge_length_range failed: {}", e)),
    }
}

fn handle_cad_measure_signed(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let aid = match resolve_shape_id(state, params) { Ok(x) => x, Err(e) => return RpcResponse::err(id, e) };
    let x = params.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let y = params.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let z = params.get("z").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(8) as usize;
    match signed_distance(&state.cad_doc.arena, aid, gfd_cad::geom::Point3::new(x, y, z), u_steps, v_steps) {
        Ok(d) => RpcResponse::ok(id, serde_json::json!({ "signed_distance": d, "inside": d < 0.0 })),
        Err(e) => RpcResponse::err(id, format!("signed_distance failed: {}", e)),
    }
}

fn handle_cad_feature_filleted_cylinder(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let r = params.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let h = params.get("height").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let f = params.get("fillet").and_then(|v| v.as_f64()).unwrap_or(0.1);
    match filleted_cylinder_solid(&mut state.cad_doc.arena, r, h, f) {
        Ok(new_id) => {
            let (s, _) = register_cad(state, new_id, "filleted_cylinder");
            RpcResponse::ok(id, serde_json::json!({ "shape_id": s, "arena_id": new_id.0, "kind": "filleted_cylinder" }))
        }
        Err(e) => RpcResponse::err(id, format!("filleted_cylinder failed: {}", e)),
    }
}

fn handle_cad_measure_closest(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let aid = match resolve_shape_id(state, params) { Ok(x) => x, Err(e) => return RpcResponse::err(id, e) };
    let x = params.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let y = params.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let z = params.get("z").and_then(|v| v.as_f64()).unwrap_or(0.0);
    match closest_point_on_shape(&state.cad_doc.arena, aid, gfd_cad::geom::Point3::new(x, y, z)) {
        Ok(d) => RpcResponse::ok(id, serde_json::json!({ "distance": d })),
        Err(e) => RpcResponse::err(id, format!("closest_point failed: {}", e)),
    }
}

fn handle_cad_measure_inside(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let aid = match resolve_shape_id(state, params) { Ok(x) => x, Err(e) => return RpcResponse::err(id, e) };
    let x = params.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let y = params.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let z = params.get("z").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(8) as usize;
    match is_point_inside_solid(&state.cad_doc.arena, aid, gfd_cad::geom::Point3::new(x, y, z), u_steps, v_steps) {
        Ok(b) => RpcResponse::ok(id, serde_json::json!({ "inside": b })),
        Err(e) => RpcResponse::err(id, format!("point_inside failed: {}", e)),
    }
}

fn handle_cad_feature_linear_array(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let aid = match resolve_shape_id(state, params) { Ok(x) => x, Err(e) => return RpcResponse::err(id, e) };
    let count = params.get("count").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
    let dx = params.get("dx").and_then(|v| v.as_f64()).unwrap_or(2.0);
    let dy = params.get("dy").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let dz = params.get("dz").and_then(|v| v.as_f64()).unwrap_or(0.0);
    match linear_array(&mut state.cad_doc.arena, aid, count, dx, dy, dz) {
        Ok(new_id) => {
            let (s, _) = register_cad(state, new_id, "linear_array");
            RpcResponse::ok(id, serde_json::json!({ "shape_id": s, "arena_id": new_id.0, "kind": "linear_array" }))
        }
        Err(e) => RpcResponse::err(id, format!("linear_array failed: {}", e)),
    }
}

fn handle_cad_feature_offset_polygon(id: u64, params: &Value) -> RpcResponse {
    let Some(arr) = params.get("points").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing points");
    };
    let mut pts = Vec::with_capacity(arr.len());
    for p in arr {
        let Some(xy) = p.as_array() else { return RpcResponse::err(id, "point must be [x,y]"); };
        if xy.len() < 2 { return RpcResponse::err(id, "point must be [x,y]"); }
        pts.push((xy[0].as_f64().unwrap_or(0.0), xy[1].as_f64().unwrap_or(0.0)));
    }
    let d = params.get("distance").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let out = offset_polygon_2d(&pts, d);
    let as_json: Vec<Value> = out.iter().map(|(x, y)| serde_json::json!([*x, *y])).collect();
    RpcResponse::ok(id, serde_json::json!({ "points": as_json }))
}

fn handle_cad_feature_offset_pad(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(arr) = params.get("points").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing points");
    };
    let mut pts = Vec::with_capacity(arr.len());
    for p in arr {
        let Some(xy) = p.as_array() else { return RpcResponse::err(id, "point must be [x,y]"); };
        if xy.len() < 2 { return RpcResponse::err(id, "point must be [x,y]"); }
        pts.push((xy[0].as_f64().unwrap_or(0.0), xy[1].as_f64().unwrap_or(0.0)));
    }
    let offset = params.get("offset").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let height = params.get("height").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let offset_pts = if offset.abs() > 1e-9 { offset_polygon_2d(&pts, offset) } else { pts };
    match pad_polygon_xy(&mut state.cad_doc.arena, &offset_pts, height) {
        Ok(sid) => {
            let (s, _) = register_cad(state, sid, "offset_pad");
            RpcResponse::ok(id, serde_json::json!({ "shape_id": s, "arena_id": sid.0, "kind": "offset_pad" }))
        }
        Err(e) => RpcResponse::err(id, format!("offset_pad failed: {}", e)),
    }
}

fn handle_cad_feature_rect_array(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let aid = match resolve_shape_id(state, params) { Ok(x) => x, Err(e) => return RpcResponse::err(id, e) };
    let cu = params.get("count_u").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
    let cv = params.get("count_v").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
    let du = (
        params.get("du_x").and_then(|v| v.as_f64()).unwrap_or(2.0),
        params.get("du_y").and_then(|v| v.as_f64()).unwrap_or(0.0),
        params.get("du_z").and_then(|v| v.as_f64()).unwrap_or(0.0),
    );
    let dv = (
        params.get("dv_x").and_then(|v| v.as_f64()).unwrap_or(0.0),
        params.get("dv_y").and_then(|v| v.as_f64()).unwrap_or(2.0),
        params.get("dv_z").and_then(|v| v.as_f64()).unwrap_or(0.0),
    );
    match rectangular_array(&mut state.cad_doc.arena, aid, cu, cv, du, dv) {
        Ok(new_id) => {
            let (s, _) = register_cad(state, new_id, "rectangular_array");
            RpcResponse::ok(id, serde_json::json!({ "shape_id": s, "arena_id": new_id.0, "kind": "rectangular_array" }))
        }
        Err(e) => RpcResponse::err(id, format!("rect array failed: {}", e)),
    }
}

fn handle_cad_feature_circular_array(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let aid = match resolve_shape_id(state, params) { Ok(x) => x, Err(e) => return RpcResponse::err(id, e) };
    let count = params.get("count").and_then(|v| v.as_u64()).unwrap_or(6) as usize;
    let ax = params.get("ax").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let ay = params.get("ay").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let az = params.get("az").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let total_deg = params.get("total_deg").and_then(|v| v.as_f64()).unwrap_or(360.0);
    match circular_array(&mut state.cad_doc.arena, aid, count, (ax, ay, az), total_deg.to_radians()) {
        Ok(new_id) => {
            let (s, _) = register_cad(state, new_id, "circular_array");
            RpcResponse::ok(id, serde_json::json!({ "shape_id": s, "arena_id": new_id.0, "kind": "circular_array" }))
        }
        Err(e) => RpcResponse::err(id, format!("circular_array failed: {}", e)),
    }
}

fn handle_cad_feature_translate(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let aid = match resolve_shape_id(state, params) { Ok(x) => x, Err(e) => return RpcResponse::err(id, e) };
    let tx = params.get("tx").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let ty = params.get("ty").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let tz = params.get("tz").and_then(|v| v.as_f64()).unwrap_or(0.0);
    match translate_shape(&mut state.cad_doc.arena, aid, tx, ty, tz) {
        Ok(new_id) => {
            let (s, _) = register_cad(state, new_id, "translate");
            RpcResponse::ok(id, serde_json::json!({ "shape_id": s, "arena_id": new_id.0, "kind": "translate" }))
        }
        Err(e) => RpcResponse::err(id, format!("translate failed: {}", e)),
    }
}

fn handle_cad_feature_scale(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let aid = match resolve_shape_id(state, params) { Ok(x) => x, Err(e) => return RpcResponse::err(id, e) };
    let sx = params.get("sx").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let sy = params.get("sy").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let sz = params.get("sz").and_then(|v| v.as_f64()).unwrap_or(1.0);
    match scale_shape(&mut state.cad_doc.arena, aid, sx, sy, sz) {
        Ok(new_id) => {
            let (s, _) = register_cad(state, new_id, "scale");
            RpcResponse::ok(id, serde_json::json!({ "shape_id": s, "arena_id": new_id.0, "kind": "scale" }))
        }
        Err(e) => RpcResponse::err(id, format!("scale failed: {}", e)),
    }
}

fn handle_cad_feature_rotate(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let aid = match resolve_shape_id(state, params) { Ok(x) => x, Err(e) => return RpcResponse::err(id, e) };
    let ax = params.get("ax").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let ay = params.get("ay").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let az = params.get("az").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let angle_deg = params.get("angle_deg").and_then(|v| v.as_f64()).unwrap_or(90.0);
    match rotate_shape(&mut state.cad_doc.arena, aid, (ax, ay, az), angle_deg.to_radians()) {
        Ok(new_id) => {
            let (s, _) = register_cad(state, new_id, "rotate");
            RpcResponse::ok(id, serde_json::json!({ "shape_id": s, "arena_id": new_id.0, "kind": "rotate" }))
        }
        Err(e) => RpcResponse::err(id, format!("rotate failed: {}", e)),
    }
}

fn handle_cad_feature_mirror(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let plane_str = params.get("plane").and_then(|v| v.as_str()).unwrap_or("xy");
    let plane = match plane_str.to_lowercase().as_str() {
        "xy" => MirrorPlane::XY,
        "yz" => MirrorPlane::YZ,
        "xz" => MirrorPlane::XZ,
        other => return RpcResponse::err(id, format!("unknown mirror plane: {}", other)),
    };
    let new_shape = match mirror_shape(&mut state.cad_doc.arena, aid, plane) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("mirror failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let new_str = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(new_str.clone(), new_shape);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": new_str, "arena_id": new_shape.0, "kind": "mirror",
    }))
}

fn handle_cad_feature_rounded_top(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let lx = params.get("lx").and_then(|v| v.as_f64()).unwrap_or(1.5);
    let ly = params.get("ly").and_then(|v| v.as_f64()).unwrap_or(1.5);
    let lz = params.get("lz").and_then(|v| v.as_f64()).unwrap_or(0.8);
    let r  = params.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.2);
    let shape_id = match filleted_box_top_edges(&mut state.cad_doc.arena, lx, ly, lz, r) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("rounded_top_box failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id, "arena_id": shape_id.0, "kind": "rounded_top_box",
    }))
}

fn handle_cad_feature_keycap(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let lx = params.get("lx").and_then(|v| v.as_f64()).unwrap_or(1.5);
    let ly = params.get("ly").and_then(|v| v.as_f64()).unwrap_or(1.5);
    let lz = params.get("lz").and_then(|v| v.as_f64()).unwrap_or(0.8);
    let d  = params.get("distance").and_then(|v| v.as_f64()).unwrap_or(0.2);
    let shape_id = match chamfered_box_top_edges(&mut state.cad_doc.arena, lx, ly, lz, d) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("keycap failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id, "arena_id": shape_id.0, "kind": "keycap",
    }))
}

fn handle_cad_feature_fillet_box(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let lx = params.get("lx").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let ly = params.get("ly").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let lz = params.get("lz").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let r  = params.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.2);
    let shape_id = match filleted_box_solid(&mut state.cad_doc.arena, lx, ly, lz, r) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("fillet_box failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id, "arena_id": shape_id.0, "kind": "fillet_box",
    }))
}

fn handle_cad_feature_chamfer_box(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let lx = params.get("lx").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let ly = params.get("ly").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let lz = params.get("lz").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let d  = params.get("distance").and_then(|v| v.as_f64()).unwrap_or(0.2);
    let shape_id = match chamfered_box_solid(&mut state.cad_doc.arena, lx, ly, lz, d) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("chamfer_box failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id, "arena_id": shape_id.0, "kind": "chamfer_box",
    }))
}

fn handle_cad_feature_pocket(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(arr) = params.get("points").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "cad.feature.pocket: missing points");
    };
    let mut pts = Vec::with_capacity(arr.len());
    for p in arr {
        let Some(xy) = p.as_array() else { return RpcResponse::err(id, "point must be [x,y]"); };
        if xy.len() < 2 { return RpcResponse::err(id, "point must be [x,y]"); }
        pts.push((xy[0].as_f64().unwrap_or(0.0), xy[1].as_f64().unwrap_or(0.0)));
    }
    let depth = params.get("depth").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let shape_id = match pocket_polygon_xy(&mut state.cad_doc.arena, &pts, depth) {
        Ok(s) => s,
        Err(e) => return RpcResponse::err(id, format!("pocket failed: {}", e)),
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id,
        "arena_id": shape_id.0,
        "kind": "pocket",
    }))
}

fn handle_cad_feature_revolve(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(arr) = params.get("profile").and_then(|v| v.as_array()) else {
        return RpcResponse::err(id, "missing profile array of [r,z]");
    };
    let mut pts = Vec::with_capacity(arr.len());
    for p in arr {
        let Some(rz) = p.as_array() else { return RpcResponse::err(id, "profile point must be [r,z]"); };
        if rz.len() < 2 { return RpcResponse::err(id, "profile point must be [r,z]"); }
        pts.push((rz[0].as_f64().unwrap_or(0.0), rz[1].as_f64().unwrap_or(0.0)));
    }
    let steps = params.get("angular_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let angle_deg = params.get("angle_deg").and_then(|v| v.as_f64());
    let shape_id = match angle_deg {
        Some(a) if a < 360.0 - 1e-6 => {
            let angle_rad = a.to_radians();
            match revolve_profile_z_partial(&mut state.cad_doc.arena, &pts, steps, angle_rad) {
                Ok(s) => s,
                Err(e) => return RpcResponse::err(id, format!("partial revolve failed: {}", e)),
            }
        }
        _ => match revolve_profile_z(&mut state.cad_doc.arena, &pts, steps) {
            Ok(s) => s,
            Err(e) => return RpcResponse::err(id, format!("revolve failed: {}", e)),
        },
    };
    state.next_cad_shape_id += 1;
    let str_id = format!("shape_{}", state.next_cad_shape_id);
    state.cad_shape_map.insert(str_id.clone(), shape_id);
    RpcResponse::ok(id, serde_json::json!({
        "shape_id": str_id,
        "arena_id": shape_id.0,
        "kind": "revolve",
    }))
}

fn handle_cad_measure_edge_length(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let arena_id = params.get("arena_id").and_then(|v| v.as_u64()).map(|u| ShapeId(u as u32));
    let Some(eid) = arena_id else { return RpcResponse::err(id, "missing arena_id"); };
    match edge_length(&state.cad_doc.arena, eid) {
        Ok(l) => RpcResponse::ok(id, serde_json::json!({ "length": l })),
        Err(e) => RpcResponse::err(id, format!("edge_length failed: {}", e)),
    }
}

fn handle_cad_measure_inertia(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    match inertia_tensor_full(&state.cad_doc.arena, aid) {
        Ok((ixx, iyy, izz, ixy, iyz, izx)) => RpcResponse::ok(id, serde_json::json!({
            "ixx": ixx, "iyy": iyy, "izz": izz,
            "ixy": ixy, "iyz": iyz, "izx": izx,
        })),
        Err(e) => RpcResponse::err(id, format!("inertia failed: {}", e)),
    }
}

fn handle_cad_measure_dve(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let v = params.get("vertex").and_then(|v| v.as_u64()).map(|u| ShapeId(u as u32));
    let e = params.get("edge").and_then(|v| v.as_u64()).map(|u| ShapeId(u as u32));
    let (Some(v), Some(e)) = (v, e) else { return RpcResponse::err(id, "need vertex+edge arena ids"); };
    match distance_vertex_edge(&state.cad_doc.arena, v, e) {
        Ok(d) => RpcResponse::ok(id, serde_json::json!({ "distance": d })),
        Err(err) => RpcResponse::err(id, format!("distance_vertex_edge failed: {}", err)),
    }
}

fn handle_cad_measure_dee(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let a = params.get("a").and_then(|v| v.as_u64()).map(|u| ShapeId(u as u32));
    let b = params.get("b").and_then(|v| v.as_u64()).map(|u| ShapeId(u as u32));
    let (Some(a), Some(b)) = (a, b) else { return RpcResponse::err(id, "need a+b arena ids"); };
    match distance_edge_edge(&state.cad_doc.arena, a, b) {
        Ok(d) => RpcResponse::ok(id, serde_json::json!({ "distance": d })),
        Err(err) => RpcResponse::err(id, format!("distance_edge_edge failed: {}", err)),
    }
}

fn handle_cad_measure_com(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    match center_of_mass(&state.cad_doc.arena, aid) {
        Ok(p) => RpcResponse::ok(id, serde_json::json!({ "x": p.x, "y": p.y, "z": p.z })),
        Err(e) => RpcResponse::err(id, format!("center_of_mass failed: {}", e)),
    }
}

fn handle_cad_measure_volume(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(arena_id) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    match divergence_volume(&state.cad_doc.arena, arena_id) {
        Ok(v) => RpcResponse::ok(id, serde_json::json!({ "volume": v })),
        Err(e) => RpcResponse::err(id, format!("volume failed: {}", e)),
    }
}

fn handle_cad_export_brep(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    let shape_id = params.get("shape_id").and_then(|v| v.as_str()).and_then(|s| state.cad_shape_map.get(s).copied());
    match write_brep(std::path::Path::new(path), &state.cad_doc.arena, shape_id) {
        Ok(()) => RpcResponse::ok(id, serde_json::json!({ "ok": true, "path": path })),
        Err(e) => RpcResponse::err(id, format!("brep export failed: {}", e)),
    }
}

fn handle_cad_export_ply(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate for ply failed: {}", e)),
    };
    match write_ply_ascii(std::path::Path::new(path), &tri) {
        Ok(()) => RpcResponse::ok(id, serde_json::json!({ "ok": true, "path": path, "triangle_count": tri.indices.len() / 3 })),
        Err(e) => RpcResponse::err(id, format!("ply write failed: {}", e)),
    }
}

fn handle_cad_export_dxf_string(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate failed: {}", e)),
    };
    let tmp = std::env::temp_dir().join(format!("gfd_dxf_{}_{}.dxf",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos()).unwrap_or(0),
    ));
    if let Err(e) = write_dxf_3dface(&tmp, &tri) {
        return RpcResponse::err(id, format!("dxf write failed: {}", e));
    }
    let content = std::fs::read_to_string(&tmp).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);
    let len = content.len();
    RpcResponse::ok(id, serde_json::json!({ "content": content, "length": len }))
}

fn handle_cad_export_xyz_string(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate failed: {}", e)),
    };
    let mut buf = String::with_capacity(tri.positions.len() * 30);
    for p in &tri.positions {
        buf.push_str(&format!("{:.6} {:.6} {:.6}\n", p[0], p[1], p[2]));
    }
    let len = buf.len();
    RpcResponse::ok(id, serde_json::json!({ "content": buf, "length": len, "point_count": tri.positions.len() }))
}

fn handle_cad_export_vtk_string(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let tmp = std::env::temp_dir().join(format!("gfd_vtk_{}_{}.vtk",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos()).unwrap_or(0),
    ));
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate for vtk failed: {}", e)),
    };
    if let Err(e) = write_vtk_polydata(&tmp, &tri, "gfd-cad export") {
        return RpcResponse::err(id, format!("vtk write failed: {}", e));
    }
    let content = std::fs::read_to_string(&tmp).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);
    let len = content.len();
    RpcResponse::ok(id, serde_json::json!({ "content": content, "length": len }))
}

fn handle_cad_export_wrl_string(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate for wrl failed: {}", e)),
    };
    let tmp = std::env::temp_dir().join(format!("gfd_wrl_{}_{}.wrl",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos()).unwrap_or(0),
    ));
    if let Err(e) = write_wrl(&tmp, &tri) {
        return RpcResponse::err(id, format!("wrl write failed: {}", e));
    }
    let content = std::fs::read_to_string(&tmp).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);
    let len = content.len();
    RpcResponse::ok(id, serde_json::json!({ "content": content, "length": len }))
}

/// In-memory STEP AP214 export. Writes to a tempfile then reads it back
/// (`write_step` wants a `Path`), deletes the temp, returns the string.
fn handle_cad_export_step_string(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let tmp = std::env::temp_dir().join(format!("gfd_step_{}_{}.stp",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos()).unwrap_or(0),
    ));
    if let Err(e) = export_step(&tmp, &state.cad_doc.arena, aid) {
        return RpcResponse::err(id, format!("step export failed: {}", e));
    }
    let content = match std::fs::read_to_string(&tmp) {
        Ok(c) => c,
        Err(e) => return RpcResponse::err(id, format!("reading temp step failed: {}", e)),
    };
    let _ = std::fs::remove_file(&tmp);
    let len = content.len();
    RpcResponse::ok(id, serde_json::json!({ "content": content, "length": len }))
}

/// In-memory BRep-JSON export (iter 6 brep writer → string).
fn handle_cad_export_brep_string(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let tmp = std::env::temp_dir().join(format!("gfd_brep_{}_{}.brep",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos()).unwrap_or(0),
    ));
    if let Err(e) = write_brep(&tmp, &state.cad_doc.arena, Some(aid)) {
        return RpcResponse::err(id, format!("brep export failed: {}", e));
    }
    let content = match std::fs::read_to_string(&tmp) {
        Ok(c) => c,
        Err(e) => return RpcResponse::err(id, format!("reading temp brep failed: {}", e)),
    };
    let _ = std::fs::remove_file(&tmp);
    let len = content.len();
    RpcResponse::ok(id, serde_json::json!({ "content": content, "length": len }))
}

fn handle_cad_export_obj_string(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate for obj failed: {}", e)),
    };
    let mut buf = String::with_capacity(tri.indices.len() * 50);
    buf.push_str("# gfd-cad-io export\no ");
    buf.push_str(str_id);
    buf.push('\n');
    for p in &tri.positions {
        buf.push_str(&format!("v {:.6} {:.6} {:.6}\n", p[0], p[1], p[2]));
    }
    for t in 0..(tri.indices.len() / 3) {
        let i0 = tri.indices[t * 3] as usize + 1;
        let i1 = tri.indices[t * 3 + 1] as usize + 1;
        let i2 = tri.indices[t * 3 + 2] as usize + 1;
        buf.push_str(&format!("f {} {} {}\n", i0, i1, i2));
    }
    RpcResponse::ok(id, serde_json::json!({
        "content":        buf,
        "triangle_count": tri.indices.len() / 3,
        "length":         buf.len(),
    }))
}

fn handle_cad_export_ply_string(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate for ply failed: {}", e)),
    };
    let v = tri.positions.len();
    let t_count = tri.indices.len() / 3;
    let mut buf = String::with_capacity(t_count * 30 + v * 24);
    buf.push_str("ply\nformat ascii 1.0\ncomment gfd-cad-io export\n");
    buf.push_str(&format!("element vertex {}\nproperty float x\nproperty float y\nproperty float z\n", v));
    buf.push_str(&format!("element face {}\nproperty list uchar uint vertex_indices\nend_header\n", t_count));
    for p in &tri.positions {
        buf.push_str(&format!("{:.6} {:.6} {:.6}\n", p[0], p[1], p[2]));
    }
    for t in 0..t_count {
        buf.push_str(&format!("3 {} {} {}\n",
            tri.indices[t * 3], tri.indices[t * 3 + 1], tri.indices[t * 3 + 2]));
    }
    RpcResponse::ok(id, serde_json::json!({
        "content":        buf,
        "triangle_count": t_count,
        "length":         buf.len(),
    }))
}

/// In-memory STL ASCII export. Returns the file content as a string
/// (no disk write). Useful for browser-side downloads, preview, etc.
fn handle_cad_export_stl_string(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate for stl failed: {}", e)),
    };
    let mut buf = String::with_capacity(tri.indices.len() * 100);
    buf.push_str("solid gfd_cad\n");
    for t in 0..(tri.indices.len() / 3) {
        let a = tri.positions[tri.indices[t * 3] as usize];
        let b = tri.positions[tri.indices[t * 3 + 1] as usize];
        let c = tri.positions[tri.indices[t * 3 + 2] as usize];
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let nx = e1[1] * e2[2] - e1[2] * e2[1];
        let ny = e1[2] * e2[0] - e1[0] * e2[2];
        let nz = e1[0] * e2[1] - e1[1] * e2[0];
        let l = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-30);
        buf.push_str(&format!("  facet normal {:e} {:e} {:e}\n    outer loop\n", nx / l, ny / l, nz / l));
        buf.push_str(&format!("      vertex {:.6} {:.6} {:.6}\n", a[0], a[1], a[2]));
        buf.push_str(&format!("      vertex {:.6} {:.6} {:.6}\n", b[0], b[1], b[2]));
        buf.push_str(&format!("      vertex {:.6} {:.6} {:.6}\n", c[0], c[1], c[2]));
        buf.push_str("    endloop\n  endfacet\n");
    }
    buf.push_str("endsolid gfd_cad\n");
    RpcResponse::ok(id, serde_json::json!({
        "content":        buf,
        "triangle_count": tri.indices.len() / 3,
        "length":         buf.len(),
    }))
}

fn handle_cad_export_dxf(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate for dxf failed: {}", e)),
    };
    match write_dxf_3dface(std::path::Path::new(path), &tri) {
        Ok(()) => RpcResponse::ok(id, serde_json::json!({
            "ok": true, "path": path,
            "triangle_count": tri.indices.len() / 3,
        })),
        Err(e) => RpcResponse::err(id, format!("dxf write failed: {}", e)),
    }
}

fn handle_cad_export_vtk(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("gfd-cad export");
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate for vtk failed: {}", e)),
    };
    match write_vtk_polydata(std::path::Path::new(path), &tri, title) {
        Ok(()) => RpcResponse::ok(id, serde_json::json!({
            "ok": true, "path": path,
            "triangle_count": tri.indices.len() / 3,
            "vertex_count":   tri.positions.len(),
        })),
        Err(e) => RpcResponse::err(id, format!("vtk write failed: {}", e)),
    }
}

fn handle_cad_export_xyz(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate for xyz failed: {}", e)),
    };
    match write_xyz(std::path::Path::new(path), &tri) {
        Ok(()) => RpcResponse::ok(id, serde_json::json!({ "ok": true, "path": path, "point_count": tri.positions.len() })),
        Err(e) => RpcResponse::err(id, format!("xyz write failed: {}", e)),
    }
}

fn handle_cad_export_wrl(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate for wrl failed: {}", e)),
    };
    match write_wrl(std::path::Path::new(path), &tri) {
        Ok(()) => RpcResponse::ok(id, serde_json::json!({ "ok": true, "path": path, "triangle_count": tri.indices.len() / 3 })),
        Err(e) => RpcResponse::err(id, format!("wrl write failed: {}", e)),
    }
}

fn handle_cad_export_off(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate for off failed: {}", e)),
    };
    match write_off(std::path::Path::new(path), &tri) {
        Ok(()) => RpcResponse::ok(id, serde_json::json!({ "ok": true, "path": path, "triangle_count": tri.indices.len() / 3 })),
        Err(e) => RpcResponse::err(id, format!("off write failed: {}", e)),
    }
}

fn handle_cad_export_obj(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate for obj failed: {}", e)),
    };
    match write_obj(std::path::Path::new(path), &tri, str_id) {
        Ok(()) => RpcResponse::ok(id, serde_json::json!({ "ok": true, "path": path, "triangle_count": tri.indices.len() / 3 })),
        Err(e) => RpcResponse::err(id, format!("obj write failed: {}", e)),
    }
}

fn handle_cad_export_stl(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    let u_steps = params.get("u_steps").and_then(|v| v.as_u64()).unwrap_or(32) as usize;
    let v_steps = params.get("v_steps").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
    let opts = TessellationOptions { u_steps, v_steps, ..Default::default() };
    let tri = match tessellate(&state.cad_doc.arena, aid, opts) {
        Ok(m) => m,
        Err(e) => return RpcResponse::err(id, format!("tessellate for stl failed: {}", e)),
    };
    let stl = StlMesh {
        positions: tri.positions,
        normals:   tri.normals,
        indices:   tri.indices,
    };
    let binary = params.get("binary").and_then(|v| v.as_bool()).unwrap_or(false);
    let result = if binary {
        write_stl_binary(std::path::Path::new(path), &stl)
    } else {
        write_stl_ascii(std::path::Path::new(path), &stl, str_id)
    };
    match result {
        Ok(()) => RpcResponse::ok(id, serde_json::json!({
            "ok": true, "path": path, "triangle_count": stl.triangle_count(), "binary": binary,
        })),
        Err(e) => RpcResponse::err(id, format!("stl write failed: {}", e)),
    }
}

fn handle_cad_export_step(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(aid) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    match export_step(std::path::Path::new(path), &state.cad_doc.arena, aid) {
        Ok(()) => RpcResponse::ok(id, serde_json::json!({ "ok": true, "path": path })),
        Err(e) => RpcResponse::err(id, format!("step export failed: {}", e)),
    }
}

fn handle_cad_step_summary(id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    match summarise_step(std::path::Path::new(path)) {
        Ok(s) => RpcResponse::ok(id, serde_json::json!({
            "cartesian_points": s.cartesian_points,
            "vertex_points": s.vertex_points,
            "edge_curves": s.edge_curves,
            "edge_loops": s.edge_loops,
            "face_outer_bounds": s.face_outer_bounds,
            "advanced_faces": s.advanced_faces,
            "closed_shells": s.closed_shells,
            "manifold_solid_breps": s.manifold_solid_breps,
            "axis2_placements": s.axis2_placements,
            "planes": s.planes,
            "cylindrical_surfaces": s.cylindrical_surfaces,
            "spherical_surfaces": s.spherical_surfaces,
        })),
        Err(e) => RpcResponse::err(id, format!("step summary failed: {}", e)),
    }
}

fn handle_cad_import_step(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    match import_step(std::path::Path::new(path), &mut state.cad_doc.arena) {
        Ok(shape_id) => {
            state.next_cad_shape_id += 1;
            let str_id = format!("shape_{}", state.next_cad_shape_id);
            state.cad_shape_map.insert(str_id.clone(), shape_id);
            RpcResponse::ok(id, serde_json::json!({ "shape_id": str_id, "arena_id": shape_id.0 }))
        }
        Err(e) => RpcResponse::err(id, format!("step import failed: {}", e)),
    }
}

fn handle_cad_import_brep(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(path) = params.get("path").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing path");
    };
    match read_brep(std::path::Path::new(path)) {
        Ok(loaded) => {
            state.cad_doc.arena = loaded.arena;
            let shape_id = match loaded.root {
                Some(r) => {
                    state.next_cad_shape_id += 1;
                    let str_id = format!("shape_{}", state.next_cad_shape_id);
                    state.cad_shape_map.insert(str_id.clone(), ShapeId(r));
                    Some(str_id)
                }
                None => None,
            };
            RpcResponse::ok(id, serde_json::json!({ "shape_id": shape_id }))
        }
        Err(e) => RpcResponse::err(id, format!("brep import failed: {}", e)),
    }
}

fn handle_cad_measure_surface_area(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let Some(str_id) = params.get("shape_id").and_then(|v| v.as_str()) else {
        return RpcResponse::err(id, "missing shape_id");
    };
    let Some(arena_id) = state.cad_shape_map.get(str_id).copied() else {
        return RpcResponse::err(id, format!("unknown shape_id: {}", str_id));
    };
    match surface_area(&state.cad_doc.arena, arena_id) {
        Ok(a) => RpcResponse::ok(id, serde_json::json!({ "area": a })),
        Err(e) => RpcResponse::err(id, format!("surface_area failed: {}", e)),
    }
}

fn handle_cad_measure_distance(state: &ServerState, id: u64, params: &Value) -> RpcResponse {
    let arena = &state.cad_doc.arena;
    let get = |key: &str| -> Option<ShapeId> {
        params.get(key).and_then(|v| v.as_u64()).map(|u| ShapeId(u as u32))
    };
    let Some(a) = get("a") else { return RpcResponse::err(id, "missing a (arena_id)"); };
    let Some(b) = get("b") else { return RpcResponse::err(id, "missing b (arena_id)"); };
    match cad_distance(arena, a, b) {
        Ok(d) => RpcResponse::ok(id, serde_json::json!({ "distance": d })),
        Err(e) => RpcResponse::err(id, format!("distance failed: {}", e)),
    }
}

// ---------------------------------------------------------------------------
// Mesh handlers
// ---------------------------------------------------------------------------

fn handle_mesh_generate(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let nx = params.get("nx").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let ny = params.get("ny").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let nz = params.get("nz").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

    // Accept domain bounds from GUI (new format) or fall back to lx/ly/lz
    let (lx, ly, lz, origin_x, origin_y, origin_z) =
        if let Some(domain) = params.get("domain") {
            let xmin = domain.get("xmin").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let xmax = domain.get("xmax").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let ymin = domain.get("ymin").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let ymax = domain.get("ymax").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let zmin = domain.get("zmin").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let zmax = domain.get("zmax").and_then(|v| v.as_f64()).unwrap_or(if nz > 0 { 1.0 } else { 0.0 });
            (xmax - xmin, ymax - ymin, zmax - zmin, xmin, ymin, zmin)
        } else {
            let lx = params.get("lx").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let ly = params.get("ly").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let lz = params
                .get("lz")
                .and_then(|v| v.as_f64())
                .unwrap_or(if nz > 0 { 1.0 } else { 0.0 });
            (lx, ly, lz, 0.0, 0.0, 0.0)
        };

    let mesh = StructuredMesh::uniform(nx, ny, nz, lx, ly, lz).to_unstructured();

    let n_cells = mesh.num_cells();
    let n_faces = mesh.num_faces();
    let n_nodes = mesh.num_nodes();

    // Compute quality
    let quality = compute_mesh_quality(&mesh);

    state.mesh_params = Some((nx, ny, nz, lx, ly, lz));
    state.mesh = Some(mesh);

    // Return enhanced response with domain info
    RpcResponse::ok(
        id,
        serde_json::json!({
            "cells": n_cells,
            "faces": n_faces,
            "nodes": n_nodes,
            "nx": nx,
            "ny": ny,
            "nz": nz,
            "domain": {
                "xmin": origin_x,
                "xmax": origin_x + lx,
                "ymin": origin_y,
                "ymax": origin_y + ly,
                "zmin": origin_z,
                "zmax": origin_z + lz,
            },
            "quality": {
                "min_ortho": quality.min_orthogonality,
                "max_skew": quality.max_skewness,
                "max_ar": quality.max_aspect_ratio,
                "bad_cells": quality.num_bad_cells,
            },
        }),
    )
}

fn handle_mesh_get_display_data(state: &mut ServerState, id: u64) -> RpcResponse {
    match &state.mesh {
        Some(mesh) => RpcResponse::ok(id, mesh_display_data(mesh)),
        None => RpcResponse::err(id, "No mesh generated yet. Call mesh.generate first."),
    }
}

fn handle_mesh_quality(state: &mut ServerState, id: u64) -> RpcResponse {
    match &state.mesh {
        Some(mesh) => {
            let q = compute_mesh_quality(mesh);
            RpcResponse::ok(
                id,
                serde_json::json!({
                    "min_ortho": q.min_orthogonality,
                    "max_skew": q.max_skewness,
                    "max_ar": q.max_aspect_ratio,
                    "bad_cells": q.num_bad_cells,
                }),
            )
        }
        None => RpcResponse::err(id, "No mesh generated yet. Call mesh.generate first."),
    }
}

// ---------------------------------------------------------------------------
// Solve handlers
// ---------------------------------------------------------------------------

fn handle_solve_start(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    // We need a mesh to solve on
    let mesh = match &state.mesh {
        Some(m) => m.clone(),
        None => {
            return RpcResponse::err(id, "No mesh generated yet. Call mesh.generate first.");
        }
    };

    let flow = params
        .get("flow")
        .and_then(|v| v.as_str())
        .unwrap_or("incompressible");
    let _turbulence = params
        .get("turbulence")
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    let max_iter = params
        .get("max_iterations")
        .and_then(|v| v.as_u64())
        .unwrap_or(200) as usize;
    let tolerance = params
        .get("tolerance")
        .and_then(|v| v.as_f64())
        .unwrap_or(1e-4);

    // Physical parameters
    let density = params
        .get("density")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    let viscosity = params
        .get("viscosity")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.01);
    let conductivity = params
        .get("conductivity")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    let alpha_u = params
        .get("alpha_u")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);
    let alpha_p = params
        .get("alpha_p")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.3);

    let physics = params
        .get("physics")
        .and_then(|v| v.as_str())
        .unwrap_or(if flow == "none" { "thermal" } else { "fluid" });

    // Parse boundary conditions from params
    let bcs = params.get("boundary_conditions");

    state.next_job_id += 1;
    let job_id = format!("job_{}", state.next_job_id);

    let running = Arc::new(AtomicBool::new(true));
    let iteration = Arc::new(AtomicU64::new(0));
    let residual = Arc::new(Mutex::new(f64::MAX));
    let result_holder: Arc<Mutex<Option<JobResult>>> = Arc::new(Mutex::new(None));

    let handle = JobHandle {
        running: Arc::clone(&running),
        iteration: Arc::clone(&iteration),
        residual: Arc::clone(&residual),
        start_time: Instant::now(),
        result: Arc::clone(&result_holder),
    };

    state.jobs.insert(job_id.clone(), handle);

    // Clone values for the background thread
    let running_t = Arc::clone(&running);
    let iteration_t = Arc::clone(&iteration);
    let residual_t = Arc::clone(&residual);
    let result_t = Arc::clone(&result_holder);
    let bcs_val = bcs.cloned().unwrap_or(Value::Array(Vec::new()));
    let physics = physics.to_string();

    thread::spawn(move || {
        if physics == "thermal" {
            // Thermal solve
            run_thermal_solve(
                &mesh,
                conductivity,
                max_iter,
                tolerance,
                &bcs_val,
                &running_t,
                &iteration_t,
                &residual_t,
                &result_t,
            );
        } else {
            // Fluid solve
            run_fluid_solve(
                &mesh,
                density,
                viscosity,
                alpha_u,
                alpha_p,
                max_iter,
                tolerance,
                &bcs_val,
                &running_t,
                &iteration_t,
                &residual_t,
                &result_t,
            );
        }
        running_t.store(false, Ordering::SeqCst);
    });

    RpcResponse::ok(id, serde_json::json!({ "job_id": job_id }))
}

fn run_fluid_solve(
    mesh: &UnstructuredMesh,
    density: f64,
    viscosity: f64,
    alpha_u: f64,
    alpha_p: f64,
    max_iter: usize,
    tolerance: f64,
    bcs_val: &Value,
    running: &AtomicBool,
    iteration: &AtomicU64,
    residual: &Mutex<f64>,
    result_holder: &Mutex<Option<JobResult>>,
) {
    let n = mesh.num_cells();

    let mut state = FluidState {
        velocity: VectorField::zeros("velocity", n),
        pressure: ScalarField::zeros("pressure", n),
        density: ScalarField::from_vec("density", vec![density; n]),
        viscosity: ScalarField::from_vec("viscosity", vec![viscosity; n]),
        turb_kinetic_energy: None,
        turb_dissipation: None,
        turb_specific_dissipation: None,
        eddy_viscosity: None,
    };

    let mut solver = SimpleSolver::new(density, viscosity);
    solver.alpha_u = alpha_u;
    solver.alpha_p = alpha_p;

    // Parse boundary conditions
    let (boundary_velocities, boundary_pressure, wall_patches) =
        parse_fluid_bcs(bcs_val, mesh);

    solver.set_boundary_conditions(
        boundary_velocities.clone(),
        boundary_pressure.clone(),
        wall_patches.clone(),
    );

    let mut final_status = "max_iterations".to_string();
    let mut final_residual = f64::MAX;
    let mut final_iter = 0;

    for iter in 0..max_iter {
        if !running.load(Ordering::SeqCst) {
            final_status = "stopped".to_string();
            final_iter = iter;
            break;
        }

        match solver.solve_step_with_bcs(
            &mut state,
            mesh,
            &boundary_velocities,
            &boundary_pressure,
            &wall_patches,
        ) {
            Ok(r) => {
                final_residual = r;
                final_iter = iter + 1;
                iteration.store(final_iter as u64, Ordering::SeqCst);
                if let Ok(mut guard) = residual.lock() {
                    *guard = r;
                }
                if r < tolerance {
                    final_status = "converged".to_string();
                    break;
                }
            }
            Err(_e) => {
                final_status = "diverged".to_string();
                final_iter = iter + 1;
                break;
            }
        }
    }

    // Collect field data
    let mut fields: HashMap<String, Vec<f64>> = HashMap::new();
    fields.insert("pressure".to_string(), state.pressure.values().to_vec());

    let vel_vals = state.velocity.values();
    let vx: Vec<f64> = vel_vals.iter().map(|v| v[0]).collect();
    let vy: Vec<f64> = vel_vals.iter().map(|v| v[1]).collect();
    let vz: Vec<f64> = vel_vals.iter().map(|v| v[2]).collect();
    let vmag: Vec<f64> = vel_vals
        .iter()
        .map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
        .collect();
    fields.insert("vx".to_string(), vx);
    fields.insert("vy".to_string(), vy);
    fields.insert("vz".to_string(), vz);
    fields.insert("velocity_magnitude".to_string(), vmag);

    if let Ok(mut guard) = result_holder.lock() {
        *guard = Some(JobResult {
            status: final_status,
            iterations: final_iter,
            residual: final_residual,
            fields,
        });
    }
}

fn run_thermal_solve(
    mesh: &UnstructuredMesh,
    conductivity: f64,
    _max_iter: usize,
    _tolerance: f64,
    bcs_val: &Value,
    _running: &AtomicBool,
    iteration: &AtomicU64,
    residual_out: &Mutex<f64>,
    result_holder: &Mutex<Option<JobResult>>,
) {
    let n = mesh.num_cells();

    // Parse thermal BCs
    let mut boundary_temps: HashMap<String, f64> = HashMap::new();
    let source = 0.0;

    if let Some(arr) = bcs_val.as_array() {
        for bc in arr {
            let patch = bc.get("patch").and_then(|v| v.as_str()).unwrap_or("");
            let bc_type = bc.get("type").and_then(|v| v.as_str()).unwrap_or("wall");
            if bc_type == "fixed_temperature" || bc_type == "wall" {
                if let Some(t) = bc.get("temperature").and_then(|v| v.as_f64()) {
                    boundary_temps.insert(patch.to_string(), t);
                }
            }
        }
    }

    let init_temp = if boundary_temps.is_empty() {
        300.0
    } else {
        boundary_temps.values().sum::<f64>() / boundary_temps.len() as f64
    };

    let mut thermal_state = ThermalState::new(n, init_temp);
    let solver = ConductionSolver::new();

    let result = solver.solve_steady(&mut thermal_state, mesh, conductivity, source, &boundary_temps);

    iteration.store(1, Ordering::SeqCst);

    let (status, final_res) = match result {
        Ok(r) => {
            if let Ok(mut guard) = residual_out.lock() {
                *guard = r;
            }
            ("converged".to_string(), r)
        }
        Err(_e) => ("diverged".to_string(), f64::NAN),
    };

    let mut fields: HashMap<String, Vec<f64>> = HashMap::new();
    fields.insert(
        "temperature".to_string(),
        thermal_state.temperature.values().to_vec(),
    );

    if let Ok(mut guard) = result_holder.lock() {
        *guard = Some(JobResult {
            status,
            iterations: 1,
            residual: final_res,
            fields,
        });
    }
}

fn parse_fluid_bcs(
    bcs_val: &Value,
    mesh: &UnstructuredMesh,
) -> (HashMap<String, [f64; 3]>, HashMap<String, f64>, Vec<String>) {
    let mut boundary_velocities: HashMap<String, [f64; 3]> = HashMap::new();
    let mut boundary_pressure: HashMap<String, f64> = HashMap::new();
    let mut wall_patches: Vec<String> = Vec::new();

    if let Some(arr) = bcs_val.as_array() {
        for bc in arr {
            let patch = bc
                .get("patch")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let bc_type = bc
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("wall");

            match bc_type {
                "inlet" | "velocity_inlet" => {
                    let vx = bc.get("vx").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let vy = bc.get("vy").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let vz = bc.get("vz").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    boundary_velocities.insert(patch, [vx, vy, vz]);
                }
                "outlet" | "pressure_outlet" => {
                    let p = bc.get("pressure").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    boundary_pressure.insert(patch, p);
                }
                "wall" | "no_slip" => {
                    let vx = bc.get("vx").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let vy = bc.get("vy").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let vz = bc.get("vz").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    if vx.abs() > 1e-15 || vy.abs() > 1e-15 || vz.abs() > 1e-15 {
                        boundary_velocities.insert(patch, [vx, vy, vz]);
                    } else {
                        wall_patches.push(patch);
                    }
                }
                _ => {}
            }
        }
    }

    // Auto-add unlisted boundary patches as walls
    for patch in &mesh.boundary_patches {
        let name = &patch.name;
        if !boundary_velocities.contains_key(name)
            && !boundary_pressure.contains_key(name)
            && !wall_patches.contains(name)
        {
            wall_patches.push(name.clone());
        }
    }

    (boundary_velocities, boundary_pressure, wall_patches)
}

fn handle_solve_status(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let job_id = params
        .get("job_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match state.jobs.get(job_id) {
        Some(handle) => {
            let is_running = handle.running.load(Ordering::SeqCst);
            let iter = handle.iteration.load(Ordering::SeqCst);
            let res = handle
                .residual
                .lock()
                .map(|g| *g)
                .unwrap_or(f64::MAX);
            let elapsed_ms = handle.start_time.elapsed().as_millis() as u64;

            let mut resp = serde_json::json!({
                "running": is_running,
                "iteration": iter,
                "residual": res,
                "elapsed_ms": elapsed_ms,
            });

            // If finished, include the final status, iterations and residual
            if !is_running {
                if let Ok(guard) = handle.result.lock() {
                    if let Some(ref jr) = *guard {
                        resp["status"] = Value::String(jr.status.clone());
                        resp["iteration"] = serde_json::json!(jr.iterations);
                        resp["residual"] = serde_json::json!(jr.residual);
                    }
                }
            }

            RpcResponse::ok(id, resp)
        }
        None => RpcResponse::err(id, format!("Unknown job: {}", job_id)),
    }
}

fn handle_solve_stop(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let job_id = params
        .get("job_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match state.jobs.get(job_id) {
        Some(handle) => {
            handle.running.store(false, Ordering::SeqCst);
            RpcResponse::ok(id, serde_json::json!({ "stopped": true }))
        }
        None => RpcResponse::err(id, format!("Unknown job: {}", job_id)),
    }
}

// ---------------------------------------------------------------------------
// Field handlers
// ---------------------------------------------------------------------------

fn handle_field_get(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let field_name = params
        .get("field")
        .and_then(|v| v.as_str())
        .unwrap_or("pressure");

    // First try to find the field from finished jobs
    collect_finished_job_fields(state);

    match state.fields.get(field_name) {
        Some(values) => {
            let min = values
                .iter()
                .copied()
                .reduce(f64::min)
                .unwrap_or(0.0);
            let max = values
                .iter()
                .copied()
                .reduce(f64::max)
                .unwrap_or(0.0);
            let mean = if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            };
            RpcResponse::ok(
                id,
                serde_json::json!({
                    "values": values,
                    "min": min,
                    "max": max,
                    "mean": mean,
                }),
            )
        }
        None => RpcResponse::err(
            id,
            format!(
                "Field '{}' not found. Available fields: {:?}",
                field_name,
                state.fields.keys().collect::<Vec<_>>()
            ),
        ),
    }
}

fn handle_field_contour(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let field_name = params
        .get("field")
        .and_then(|v| v.as_str())
        .unwrap_or("pressure");
    let colormap = params
        .get("colormap")
        .and_then(|v| v.as_str())
        .unwrap_or("jet");
    let range = params.get("range");

    // Collect any finished job fields
    collect_finished_job_fields(state);

    let mesh = match &state.mesh {
        Some(m) => m,
        None => {
            return RpcResponse::err(id, "No mesh generated yet.");
        }
    };

    let values = match state.fields.get(field_name) {
        Some(v) => v,
        None => {
            return RpcResponse::err(
                id,
                format!(
                    "Field '{}' not found. Available: {:?}",
                    field_name,
                    state.fields.keys().collect::<Vec<_>>()
                ),
            );
        }
    };

    // Determine value range
    let (vmin, vmax) = if let Some(r) = range.and_then(|v| v.as_array()) {
        let lo = r.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
        let hi = r.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0);
        (lo, hi)
    } else {
        let lo = values.iter().copied().reduce(f64::min).unwrap_or(0.0);
        let hi = values.iter().copied().reduce(f64::max).unwrap_or(1.0);
        (lo, hi)
    };

    let range_span = if (vmax - vmin).abs() < 1e-30 {
        1.0
    } else {
        vmax - vmin
    };

    // Build per-vertex colors from cell values via boundary face triangulation.
    // For each boundary triangle, assign the owning cell's field value.
    let mut out_vertices: Vec<f64> = Vec::new();
    let mut out_colors: Vec<f64> = Vec::new();

    for patch in &mesh.boundary_patches {
        for &fid in &patch.face_ids {
            let face = &mesh.faces[fid];
            let cell_id = face.owner_cell;
            let val = if cell_id < values.len() {
                values[cell_id]
            } else {
                0.0
            };
            let t = (val - vmin) / range_span;
            let [r, g, b] = map_color(t, colormap);

            let ns = &face.nodes;
            for i in 1..ns.len().saturating_sub(1) {
                // Triangle vertices
                for &ni in &[ns[0], ns[i], ns[i + 1]] {
                    let pos = mesh.nodes[ni].position;
                    out_vertices.extend_from_slice(&pos);
                    out_colors.extend_from_slice(&[r, g, b]);
                }
            }
        }
    }

    RpcResponse::ok(
        id,
        serde_json::json!({
            "vertices": out_vertices,
            "colors": out_colors,
        }),
    )
}

/// Collect fields from any finished jobs into the global fields map.
fn collect_finished_job_fields(state: &mut ServerState) {
    let finished_ids: Vec<String> = state
        .jobs
        .iter()
        .filter(|(_, h)| !h.running.load(Ordering::SeqCst))
        .map(|(id, _)| id.clone())
        .collect();

    for jid in finished_ids {
        if let Some(handle) = state.jobs.get(&jid) {
            if let Ok(guard) = handle.result.lock() {
                if let Some(ref jr) = *guard {
                    for (fname, fvals) in &jr.fields {
                        state.fields.insert(fname.clone(), fvals.clone());
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_f64_array3(val: Option<&Value>) -> Option<[f64; 3]> {
    let arr = val?.as_array()?;
    if arr.len() < 3 {
        return None;
    }
    Some([
        arr[0].as_f64()?,
        arr[1].as_f64()?,
        arr[2].as_f64()?,
    ])
}

// ---------------------------------------------------------------------------
// Main: read stdin line by line, dispatch, write response to stdout
// ---------------------------------------------------------------------------

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let mut state = ServerState::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let req: RpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let resp = RpcResponse {
                    id: 0,
                    result: None,
                    error: Some(format!("JSON parse error: {}", e)),
                };
                let _ = writeln!(out, "{}", serde_json::to_string(&resp).unwrap());
                let _ = out.flush();
                continue;
            }
        };

        let resp = handle_request(&mut state, &req);
        let _ = writeln!(out, "{}", serde_json::to_string(&resp).unwrap());
        let _ = out.flush();
    }
}
