//! GFD Benchmark Suite
//!
//! READ-ONLY evaluation harness for the autoresearch loop.
//! Runs fixed test cases and reports metrics (error norms, iteration counts, wall time).

use std::collections::HashMap;
use std::time::Instant;

use gfd_core::mesh::cell::Cell;
use gfd_core::mesh::face::Face;
use gfd_core::mesh::structured::StructuredMesh;
use gfd_core::mesh::unstructured::{BoundaryPatch, UnstructuredMesh};
use gfd_fluid::incompressible::simple::SimpleSolver;
use gfd_fluid::FluidState;
use gfd_thermal::conduction::ConductionSolver;
use gfd_thermal::ThermalState;

// ===========================================================================
// 1-D mesh builder (matches the test helper in gfd-thermal::conduction::tests)
// ===========================================================================

/// Creates a 1D mesh of `nx` cells along x in [0, length], each cell with
/// a 1 m^2 cross-section (dy=dz=1).  Boundary patches are named "left"
/// (x = 0) and "right" (x = length).
fn make_1d_mesh(nx: usize, length: f64) -> UnstructuredMesh {
    let dx = length / nx as f64;
    let cross_area = 1.0;

    let nodes: Vec<gfd_core::mesh::node::Node> = Vec::new();

    let mut cells = Vec::with_capacity(nx);
    for i in 0..nx {
        let cx = (i as f64 + 0.5) * dx;
        cells.push(Cell::new(i, vec![], vec![], dx * 1.0 * 1.0, [cx, 0.5, 0.5]));
    }

    let mut faces: Vec<Face> = Vec::new();
    let mut face_id = 0usize;

    // Left boundary face (x = 0)
    let left_face_id = face_id;
    faces.push(Face::new(
        face_id,
        vec![],
        0,
        None,
        cross_area,
        [-1.0, 0.0, 0.0],
        [0.0, 0.5, 0.5],
    ));
    face_id += 1;

    // Internal faces
    let mut internal_face_ids = Vec::new();
    for i in 0..nx - 1 {
        let fx = (i as f64 + 1.0) * dx;
        internal_face_ids.push(face_id);
        faces.push(Face::new(
            face_id,
            vec![],
            i,
            Some(i + 1),
            cross_area,
            [1.0, 0.0, 0.0],
            [fx, 0.5, 0.5],
        ));
        face_id += 1;
    }

    // Right boundary face (x = length)
    let right_face_id = face_id;
    faces.push(Face::new(
        face_id,
        vec![],
        nx - 1,
        None,
        cross_area,
        [1.0, 0.0, 0.0],
        [length, 0.5, 0.5],
    ));

    // Populate cell face lists
    cells[0].faces.push(left_face_id);
    if nx > 1 {
        cells[0].faces.push(internal_face_ids[0]);
    }
    for i in 1..nx - 1 {
        cells[i].faces.push(internal_face_ids[i - 1]);
        cells[i].faces.push(internal_face_ids[i]);
    }
    if nx > 1 {
        cells[nx - 1].faces.push(internal_face_ids[nx - 2]);
    }
    cells[nx - 1].faces.push(right_face_id);

    let boundary_patches = vec![
        BoundaryPatch::new("left", vec![left_face_id]),
        BoundaryPatch::new("right", vec![right_face_id]),
    ];

    UnstructuredMesh::from_components(nodes, faces, cells, boundary_patches)
}

// ===========================================================================
// Case 1 — heat_1d: 1-D steady conduction, 50 cells
// ===========================================================================

fn run_heat_1d() -> (f64, u128) {
    let start = Instant::now();

    let nx = 50;
    let length = 1.0;
    let mesh = make_1d_mesh(nx, length);

    let conductivity = 1.0;
    let source = 0.0;
    let t_left = 100.0;
    let t_right = 200.0;

    let mut boundary_temps = HashMap::new();
    boundary_temps.insert("left".to_string(), t_left);
    boundary_temps.insert("right".to_string(), t_right);

    let mut state = ThermalState::new(nx, 150.0);
    let solver = ConductionSolver::new();

    solver
        .solve_steady(&mut state, &mesh, conductivity, source, &boundary_temps)
        .expect("heat_1d: solver failed");

    // Analytical: T(x) = 100 + 100*x
    let dx = length / nx as f64;
    let mut sum_sq = 0.0;
    for i in 0..nx {
        let x = (i as f64 + 0.5) * dx;
        let t_analytical = t_left + (t_right - t_left) * x / length;
        let t_computed = state.temperature.get(i).unwrap();
        let diff = t_computed - t_analytical;
        sum_sq += diff * diff;
    }
    let error_l2 = (sum_sq / nx as f64).sqrt();

    let elapsed = start.elapsed().as_millis();
    (error_l2, elapsed)
}

// ===========================================================================
// Case 2 — heat_source: 1-D conduction with volumetric source, 100 cells
// ===========================================================================

fn run_heat_source() -> (f64, u128) {
    let start = Instant::now();

    let nx = 100;
    let length = 1.0;
    let mesh = make_1d_mesh(nx, length);

    let conductivity = 1.0;
    let source_val = 100.0; // W/m^3

    let mut boundary_temps = HashMap::new();
    boundary_temps.insert("left".to_string(), 0.0);
    boundary_temps.insert("right".to_string(), 0.0);

    let mut state = ThermalState::new(nx, 0.0);
    let solver = ConductionSolver::new();

    solver
        .solve_steady(&mut state, &mesh, conductivity, source_val, &boundary_temps)
        .expect("heat_source: solver failed");

    // Analytical: T(x) = (S / (2k)) * x * (L - x)
    let dx = length / nx as f64;
    let mut sum_sq = 0.0;
    for i in 0..nx {
        let x = (i as f64 + 0.5) * dx;
        let t_analytical = (source_val / (2.0 * conductivity)) * x * (length - x);
        let t_computed = state.temperature.get(i).unwrap();
        let diff = t_computed - t_analytical;
        sum_sq += diff * diff;
    }
    let error_l2 = (sum_sq / nx as f64).sqrt();

    let elapsed = start.elapsed().as_millis();
    (error_l2, elapsed)
}

// ===========================================================================
// Cavity helper — builds an NxN lid-driven cavity mesh using StructuredMesh
// ===========================================================================

fn run_cavity(n: usize, max_iterations: usize) -> (usize, f64, u128) {
    let start = Instant::now();

    // Build mesh via StructuredMesh (2-D: nz=0)
    let sm = StructuredMesh::uniform(n, n, 0, 1.0, 1.0, 0.0);
    let mesh = sm.to_unstructured();

    let num_cells = mesh.num_cells();

    // Fluid state
    let mut state = FluidState::new(num_cells);

    let density = 1.0;
    let viscosity = 0.01; // Re = rho * U * L / mu = 1 * 1 * 1 / 0.01 = 100

    // Set uniform density and viscosity
    for i in 0..num_cells {
        state.density.set(i, density).unwrap();
        state.viscosity.set(i, viscosity).unwrap();
    }

    // SIMPLE solver
    let mut solver = SimpleSolver::new(density, viscosity);
    solver.alpha_u = 0.5;
    solver.alpha_p = 0.3;

    // Boundary conditions
    //   ymax -> lid velocity (1, 0, 0)
    //   xmin, xmax, ymin -> no-slip walls
    //   zmin, zmax -> treated as empty (zero gradient, listed as walls)
    let mut boundary_velocities: HashMap<String, [f64; 3]> = HashMap::new();
    boundary_velocities.insert("ymax".to_string(), [1.0, 0.0, 0.0]);

    let boundary_pressure: HashMap<String, f64> = HashMap::new();

    let wall_patches: Vec<String> = vec![
        "xmin".to_string(),
        "xmax".to_string(),
        "ymin".to_string(),
        "zmin".to_string(),
        "zmax".to_string(),
    ];

    // Store BCs for the pressure correction internal path
    solver.set_boundary_conditions(
        boundary_velocities.clone(),
        boundary_pressure.clone(),
        wall_patches.clone(),
    );

    let tolerance = 1e-4;
    let mut final_residual = f64::MAX;
    let mut iters_to_converge = max_iterations;

    for iter in 0..max_iterations {
        let residual = solver
            .solve_step_with_bcs(
                &mut state,
                &mesh,
                &boundary_velocities,
                &boundary_pressure,
                &wall_patches,
            )
            .expect(&format!("cavity_{}: iteration {} failed", n, iter));

        final_residual = residual;

        if iter % 50 == 0 {
            eprintln!(
                "  cavity_{}: iter {:4}, residual = {:.6e}",
                n, iter, residual
            );
        }

        if residual < tolerance {
            iters_to_converge = iter + 1;
            eprintln!(
                "  cavity_{}: converged at iter {} (residual = {:.6e})",
                n, iters_to_converge, residual
            );
            break;
        }
    }

    let elapsed = start.elapsed().as_millis();
    (iters_to_converge, final_residual, elapsed)
}

// ===========================================================================
// Main — run all benchmarks and print parseable summary
// ===========================================================================

fn main() {
    println!("GFD Benchmark Suite");
    println!("===================");

    let mut all_pass = true;

    // --- Thermal benchmarks ---
    println!("\n[1/5] heat_1d (50 cells, linear profile)");
    let (h1_error, h1_time) = run_heat_1d();
    println!("       error_l2 = {:.6e}, time = {} ms", h1_error, h1_time);

    println!("\n[2/5] heat_source (100 cells, volumetric source)");
    let (hs_error, hs_time) = run_heat_source();
    println!("       error_l2 = {:.6e}, time = {} ms", hs_error, hs_time);

    // --- Cavity benchmarks ---
    println!("\n[3/5] cavity_20 (20x20, Re=100, max 500 iters)");
    let (c20_iters, c20_residual, c20_time) = run_cavity(20, 500);
    println!(
        "       iters = {}, residual = {:.6e}, time = {} ms",
        c20_iters, c20_residual, c20_time
    );

    println!("\n[4/5] cavity_50 (50x50, Re=100, max 1000 iters)");
    let (c50_iters, c50_residual, c50_time) = run_cavity(50, 1000);
    println!(
        "       iters = {}, residual = {:.6e}, time = {} ms",
        c50_iters, c50_residual, c50_time
    );

    println!("\n[5/5] cavity_100 (100x100, Re=100, max 2000 iters)");
    let (c100_iters, c100_residual, c100_time) = run_cavity(100, 2000);
    println!(
        "       iters = {}, residual = {:.6e}, time = {} ms",
        c100_iters, c100_residual, c100_time
    );

    // --- Pass/fail checks ---
    if h1_error > 1e-4 {
        all_pass = false;
    }
    if hs_error > 1e-2 {
        all_pass = false;
    }
    if c20_iters >= 500 {
        all_pass = false;
    } // didn't converge

    // --- Parseable summary ---
    println!("\n---");
    println!("heat_1d_error:        {:.6e}", h1_error);
    println!("heat_1d_time_ms:      {}", h1_time);
    println!("heat_source_error:    {:.6e}", hs_error);
    println!("heat_source_time_ms:  {}", hs_time);
    println!("cavity_20_iters:      {}", c20_iters);
    println!("cavity_20_residual:   {:.6e}", c20_residual);
    println!("cavity_20_time_ms:    {}", c20_time);
    println!("cavity_50_iters:      {}", c50_iters);
    println!("cavity_50_residual:   {:.6e}", c50_residual);
    println!("cavity_50_time_ms:    {}", c50_time);
    println!("cavity_100_iters:     {}", c100_iters);
    println!("cavity_100_residual:  {:.6e}", c100_residual);
    println!("cavity_100_time_ms:   {}", c100_time);
    println!(
        "total_benchmark_ms:   {}",
        h1_time + hs_time + c20_time + c50_time + c100_time
    );
    println!("all_tests_pass:       {}", all_pass);
    println!("---");
}
