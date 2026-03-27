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

        // -- CAD --
        "cad.create_primitive" => handle_cad_create_primitive(state, req.id, &req.params),

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
// Mesh handlers
// ---------------------------------------------------------------------------

fn handle_mesh_generate(state: &mut ServerState, id: u64, params: &Value) -> RpcResponse {
    let nx = params.get("nx").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let ny = params.get("ny").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let nz = params.get("nz").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let lx = params.get("lx").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let ly = params.get("ly").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let lz = params
        .get("lz")
        .and_then(|v| v.as_f64())
        .unwrap_or(if nz > 0 { 1.0 } else { 0.0 });

    let mesh = StructuredMesh::uniform(nx, ny, nz, lx, ly, lz).to_unstructured();

    let n_cells = mesh.num_cells();
    let n_faces = mesh.num_faces();
    let n_nodes = mesh.num_nodes();

    // Compute quality
    let quality = compute_mesh_quality(&mesh);

    state.mesh_params = Some((nx, ny, nz, lx, ly, lz));
    state.mesh = Some(mesh);

    RpcResponse::ok(
        id,
        serde_json::json!({
            "cells": n_cells,
            "faces": n_faces,
            "nodes": n_nodes,
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
