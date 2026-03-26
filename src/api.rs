//! High-level solver API for programmatic use by AI agents and library consumers.
//!
//! Each function handles mesh generation, boundary condition assembly, and solver
//! setup internally -- callers only provide simple numeric parameters.
//!
//! # Example
//! ```rust,no_run
//! use gfd::api;
//!
//! let result = api::solve_cavity(20, 20, 100.0, 200);
//! println!("Status: {}, iterations: {}", result.status, result.iterations);
//! ```

use std::collections::HashMap;
use std::time::Instant;

use gfd_core::field::{ScalarField, VectorField};
use gfd_core::mesh::structured::StructuredMesh;
use gfd_core::mesh::unstructured::UnstructuredMesh;
use gfd_fluid::FluidState;
use gfd_fluid::incompressible::simple::SimpleSolver;
use gfd_thermal::conduction::ConductionSolver;
use gfd_thermal::ThermalState;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Result from any solver run.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SolveResult {
    /// "converged", "diverged", or "max_iterations".
    pub status: String,
    /// Number of outer iterations performed.
    pub iterations: usize,
    /// Final residual norm.
    pub residual: f64,
    /// Wall-clock time in milliseconds.
    pub wall_time_ms: u64,
    /// Per-field statistics (min, max, mean).
    pub field_stats: HashMap<String, FieldStats>,
    /// Probe point values (empty if no probes configured).
    pub probe_values: Vec<ProbeResult>,
    /// Solver attribution (Modified MIT License requirement).
    pub powered_by: String,
}

/// Statistics for a single field.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldStats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
}

/// A probe measurement at a specific location.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProbeResult {
    pub name: String,
    pub position: [f64; 3],
    pub values: HashMap<String, f64>,
}

// ---------------------------------------------------------------------------
// Helper: compute scalar field stats
// ---------------------------------------------------------------------------

fn scalar_stats(field: &ScalarField) -> FieldStats {
    let vals = field.values();
    if vals.is_empty() {
        return FieldStats {
            min: 0.0,
            max: 0.0,
            mean: 0.0,
        };
    }
    let min = vals.iter().copied().reduce(f64::min).unwrap_or(0.0);
    let max = vals.iter().copied().reduce(f64::max).unwrap_or(0.0);
    let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    FieldStats { min, max, mean }
}

fn velocity_magnitude_stats(field: &VectorField) -> FieldStats {
    let vals = field.values();
    if vals.is_empty() {
        return FieldStats {
            min: 0.0,
            max: 0.0,
            mean: 0.0,
        };
    }
    let mags: Vec<f64> = vals
        .iter()
        .map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
        .collect();
    let min = mags.iter().copied().reduce(f64::min).unwrap_or(0.0);
    let max = mags.iter().copied().reduce(f64::max).unwrap_or(0.0);
    let mean = mags.iter().sum::<f64>() / mags.len() as f64;
    FieldStats { min, max, mean }
}

fn velocity_component_stats(field: &VectorField, comp: usize, name: &str) -> (String, FieldStats) {
    let vals = field.values();
    if vals.is_empty() {
        return (
            name.to_string(),
            FieldStats {
                min: 0.0,
                max: 0.0,
                mean: 0.0,
            },
        );
    }
    let comps: Vec<f64> = vals.iter().map(|v| v[comp]).collect();
    let min = comps.iter().copied().reduce(f64::min).unwrap_or(0.0);
    let max = comps.iter().copied().reduce(f64::max).unwrap_or(0.0);
    let mean = comps.iter().sum::<f64>() / comps.len() as f64;
    (name.to_string(), FieldStats { min, max, mean })
}

// ---------------------------------------------------------------------------
// Helper: generate 2D mesh (nz=0 means single-layer pseudo-2D)
// ---------------------------------------------------------------------------

fn make_mesh_2d(nx: usize, ny: usize, lx: f64, ly: f64) -> UnstructuredMesh {
    StructuredMesh::uniform(nx, ny, 0, lx, ly, 0.0).to_unstructured()
}

// ---------------------------------------------------------------------------
// Fluid Solvers
// ---------------------------------------------------------------------------

/// Solve lid-driven cavity flow.
///
/// Creates an `nx * ny` 2D mesh on a unit square domain. The top wall (ymax) moves
/// at velocity `u_lid = 1.0` and the Reynolds number is set via `re = rho * u_lid * L / mu`.
/// All other walls are stationary no-slip.
///
/// Returns a [`SolveResult`] with fields: `pressure`, `velocity_magnitude`, `vx`, `vy`.
pub fn solve_cavity(nx: usize, ny: usize, re: f64, max_iter: usize) -> SolveResult {
    let start = Instant::now();

    // Physical parameters: unit domain, u_lid = 1.0, rho = 1.0 => mu = 1/Re
    let density = 1.0;
    let u_lid = 1.0;
    let viscosity = density * u_lid * 1.0 / re; // mu = rho * U * L / Re

    let mesh = make_mesh_2d(nx, ny, 1.0, 1.0);
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
    solver.alpha_u = 0.5;
    solver.alpha_p = 0.3;

    let boundary_velocities: HashMap<String, [f64; 3]> = [
        ("ymax".to_string(), [u_lid, 0.0, 0.0]),
    ]
    .into_iter()
    .collect();
    let boundary_pressure: HashMap<String, f64> = HashMap::new();
    let wall_patches: Vec<String> = ["xmin", "xmax", "ymin", "zmin", "zmax"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    solver.set_boundary_conditions(
        boundary_velocities.clone(),
        boundary_pressure.clone(),
        wall_patches.clone(),
    );

    let tolerance = 1e-4;
    let mut final_residual = f64::MAX;
    let mut final_iter = 0;
    let mut status = "max_iterations".to_string();

    for iter in 0..max_iter {
        match solver.solve_step_with_bcs(
            &mut state,
            &mesh,
            &boundary_velocities,
            &boundary_pressure,
            &wall_patches,
        ) {
            Ok(residual) => {
                final_residual = residual;
                final_iter = iter + 1;
                if residual < tolerance {
                    status = "converged".to_string();
                    break;
                }
            }
            Err(_e) => {
                final_iter = iter + 1;
                status = "diverged".to_string();
                break;
            }
        }
    }

    let wall_time_ms = start.elapsed().as_millis() as u64;

    let mut field_stats = HashMap::new();
    field_stats.insert("pressure".to_string(), scalar_stats(&state.pressure));
    field_stats.insert(
        "velocity_magnitude".to_string(),
        velocity_magnitude_stats(&state.velocity),
    );
    let (vx_name, vx_stats) = velocity_component_stats(&state.velocity, 0, "vx");
    let (vy_name, vy_stats) = velocity_component_stats(&state.velocity, 1, "vy");
    field_stats.insert(vx_name, vx_stats);
    field_stats.insert(vy_name, vy_stats);

    SolveResult {
        status,
        iterations: final_iter,
        residual: final_residual,
        wall_time_ms,
        field_stats,
        probe_values: Vec::new(),
        powered_by: "GFD Solver — https://github.com/using76/GFD".to_string(),
    }
}

/// Solve pipe/channel flow with an inlet velocity and zero-pressure outlet.
///
/// Creates an `nx * ny` 2D mesh on a rectangular domain (length `nx/ny * 1.0`, height `1.0`).
/// Left boundary (xmin) is a velocity inlet, right (xmax) is a pressure outlet,
/// top/bottom (ymin, ymax) are no-slip walls.
/// Reynolds number is based on channel height: `Re = rho * u_inlet * H / mu`.
///
/// Returns a [`SolveResult`] with fields: `pressure`, `velocity_magnitude`, `vx`, `vy`.
pub fn solve_pipe_flow(nx: usize, ny: usize, re: f64, u_inlet: f64) -> SolveResult {
    let start = Instant::now();

    let density = 1.0;
    let h = 1.0; // channel height
    let viscosity = density * u_inlet * h / re;
    let lx = (nx as f64 / ny as f64).max(1.0) * h; // aspect ratio from grid

    let mesh = make_mesh_2d(nx, ny, lx, h);
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
    solver.alpha_u = 0.7;
    solver.alpha_p = 0.3;

    let boundary_velocities: HashMap<String, [f64; 3]> = [
        ("xmin".to_string(), [u_inlet, 0.0, 0.0]),
    ]
    .into_iter()
    .collect();
    let boundary_pressure: HashMap<String, f64> = [
        ("xmax".to_string(), 0.0),
    ]
    .into_iter()
    .collect();
    let wall_patches: Vec<String> = ["ymin", "ymax", "zmin", "zmax"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    solver.set_boundary_conditions(
        boundary_velocities.clone(),
        boundary_pressure.clone(),
        wall_patches.clone(),
    );

    let max_iter = 500;
    let tolerance = 1e-4;
    let mut final_residual = f64::MAX;
    let mut final_iter = 0;
    let mut status = "max_iterations".to_string();

    for iter in 0..max_iter {
        match solver.solve_step_with_bcs(
            &mut state,
            &mesh,
            &boundary_velocities,
            &boundary_pressure,
            &wall_patches,
        ) {
            Ok(residual) => {
                final_residual = residual;
                final_iter = iter + 1;
                if residual < tolerance {
                    status = "converged".to_string();
                    break;
                }
            }
            Err(_e) => {
                final_iter = iter + 1;
                status = "diverged".to_string();
                break;
            }
        }
    }

    let wall_time_ms = start.elapsed().as_millis() as u64;

    let mut field_stats = HashMap::new();
    field_stats.insert("pressure".to_string(), scalar_stats(&state.pressure));
    field_stats.insert(
        "velocity_magnitude".to_string(),
        velocity_magnitude_stats(&state.velocity),
    );
    let (vx_name, vx_stats) = velocity_component_stats(&state.velocity, 0, "vx");
    let (vy_name, vy_stats) = velocity_component_stats(&state.velocity, 1, "vy");
    field_stats.insert(vx_name, vx_stats);
    field_stats.insert(vy_name, vy_stats);

    SolveResult {
        status,
        iterations: final_iter,
        residual: final_residual,
        wall_time_ms,
        field_stats,
        probe_values: Vec::new(),
        powered_by: "GFD Solver — https://github.com/using76/GFD".to_string(),
    }
}

/// Solve flow from an arbitrary JSON configuration string.
///
/// The JSON must conform to the GFD `SimulationConfig` schema (see `examples/`).
/// Mesh is generated from the `general.name` field (e.g., `"20x20_lid_driven_cavity"`).
pub fn solve_flow(config_json: &str) -> Result<SolveResult, String> {
    let start = Instant::now();

    let config: gfd_io::json_input::SimulationConfig =
        serde_json::from_str(config_json).map_err(|e| format!("JSON parse error: {}", e))?;

    let (nx, ny, nz) = parse_mesh_dims(&config.setup.general.name);
    let mesh = StructuredMesh::uniform(nx, ny, nz, 1.0, 1.0, if nz > 0 { 1.0 } else { 0.0 })
        .to_unstructured();
    let n = mesh.num_cells();

    let density = config
        .setup
        .materials
        .first()
        .and_then(|m| m.properties.get("density").copied())
        .unwrap_or(1.0);
    let viscosity = config
        .setup
        .materials
        .first()
        .and_then(|m| m.properties.get("viscosity").copied())
        .unwrap_or(0.01);

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

    // Parse BCs
    let mut boundary_velocities: HashMap<String, [f64; 3]> = HashMap::new();
    let mut boundary_pressure: HashMap<String, f64> = HashMap::new();
    let mut wall_patches: Vec<String> = Vec::new();

    for bc in &config.setup.boundary_conditions {
        match bc.bc_type.as_str() {
            "inlet" | "velocity_inlet" => {
                let vx = bc.parameters.get("vx").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let vy = bc.parameters.get("vy").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let vz = bc.parameters.get("vz").and_then(|v| v.as_f64()).unwrap_or(0.0);
                boundary_velocities.insert(bc.patch.clone(), [vx, vy, vz]);
            }
            "outlet" | "pressure_outlet" => {
                let p = bc
                    .parameters
                    .get("pressure")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                boundary_pressure.insert(bc.patch.clone(), p);
            }
            "wall" | "no_slip" => {
                let vx = bc.parameters.get("vx").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let vy = bc.parameters.get("vy").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let vz = bc.parameters.get("vz").and_then(|v| v.as_f64()).unwrap_or(0.0);
                if vx.abs() > 1e-15 || vy.abs() > 1e-15 || vz.abs() > 1e-15 {
                    boundary_velocities.insert(bc.patch.clone(), [vx, vy, vz]);
                } else {
                    wall_patches.push(bc.patch.clone());
                }
            }
            _ => {}
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

    let mut solver = SimpleSolver::new(density, viscosity);
    solver.alpha_u = config.solver.relaxation.velocity;
    solver.alpha_p = config.solver.relaxation.pressure;
    solver.set_boundary_conditions(
        boundary_velocities.clone(),
        boundary_pressure.clone(),
        wall_patches.clone(),
    );

    let max_iter = config.run.max_iterations;
    let tolerance = config.run.tolerance;
    let mut final_residual = f64::MAX;
    let mut final_iter = 0;
    let mut status = "max_iterations".to_string();

    for iter in 0..max_iter {
        match solver.solve_step_with_bcs(
            &mut state,
            &mesh,
            &boundary_velocities,
            &boundary_pressure,
            &wall_patches,
        ) {
            Ok(residual) => {
                final_residual = residual;
                final_iter = iter + 1;
                if residual < tolerance {
                    status = "converged".to_string();
                    break;
                }
            }
            Err(e) => {
                final_residual = f64::NAN;
                final_iter = iter + 1;
                status = format!("diverged: {:?}", e);
                break;
            }
        }
    }

    let wall_time_ms = start.elapsed().as_millis() as u64;

    let mut field_stats = HashMap::new();
    field_stats.insert("pressure".to_string(), scalar_stats(&state.pressure));
    field_stats.insert(
        "velocity_magnitude".to_string(),
        velocity_magnitude_stats(&state.velocity),
    );
    let (vx_name, vx_stats) = velocity_component_stats(&state.velocity, 0, "vx");
    let (vy_name, vy_stats) = velocity_component_stats(&state.velocity, 1, "vy");
    field_stats.insert(vx_name, vx_stats);
    field_stats.insert(vy_name, vy_stats);

    Ok(SolveResult {
        status,
        iterations: final_iter,
        residual: final_residual,
        wall_time_ms,
        field_stats,
        probe_values: Vec::new(),
        powered_by: "GFD Solver — https://github.com/using76/GFD".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Thermal Solvers
// ---------------------------------------------------------------------------

/// Solve 1D steady-state heat conduction.
///
/// Creates a 1D mesh of `nx` cells along x on `[0, 1]`.
/// Boundary conditions: fixed temperature `t_left` at x=0, `t_right` at x=1.
/// Uniform conductivity `k` and volumetric heat source `source` [W/m^3].
///
/// Returns a [`SolveResult`] with field: `temperature`.
pub fn solve_conduction_1d(
    nx: usize,
    k: f64,
    t_left: f64,
    t_right: f64,
    source: f64,
) -> SolveResult {
    let start = Instant::now();

    // Use a single row of cells (nx x 1 x 0 = pseudo-1D)
    let mesh = make_mesh_2d(nx, 1, 1.0, 1.0);
    let n = mesh.num_cells();

    let mut state = ThermalState::new(n, (t_left + t_right) / 2.0);

    let mut boundary_temps: HashMap<String, f64> = HashMap::new();
    boundary_temps.insert("xmin".to_string(), t_left);
    boundary_temps.insert("xmax".to_string(), t_right);

    let solver = ConductionSolver::new();
    let result = solver.solve_steady(&mut state, &mesh, k, source, &boundary_temps);

    let wall_time_ms = start.elapsed().as_millis() as u64;

    let (status, residual, iterations) = match result {
        Ok(r) => ("converged".to_string(), r, 1),
        Err(e) => (format!("diverged: {:?}", e), f64::NAN, 0),
    };

    let mut field_stats = HashMap::new();
    field_stats.insert("temperature".to_string(), scalar_stats(&state.temperature));

    SolveResult {
        status,
        iterations,
        residual,
        wall_time_ms,
        field_stats,
        probe_values: Vec::new(),
        powered_by: "GFD Solver — https://github.com/using76/GFD".to_string(),
    }
}

/// Solve 2D steady-state heat conduction.
///
/// Creates an `nx * ny` 2D mesh on `[0, 1] x [0, 1]`.
/// Boundary temperatures are specified by patch name: "xmin", "xmax", "ymin", "ymax".
/// Patches not listed in `boundary_temps` default to zero-gradient (insulated).
/// Uniform conductivity `k` and volumetric source `source` [W/m^3].
///
/// Returns a [`SolveResult`] with field: `temperature`.
pub fn solve_conduction_2d(
    nx: usize,
    ny: usize,
    k: f64,
    boundary_temps: HashMap<String, f64>,
    source: f64,
) -> SolveResult {
    let start = Instant::now();

    let mesh = make_mesh_2d(nx, ny, 1.0, 1.0);
    let n = mesh.num_cells();

    // Use average of specified temperatures as initial guess, or 300 K
    let init_temp = if boundary_temps.is_empty() {
        300.0
    } else {
        boundary_temps.values().sum::<f64>() / boundary_temps.len() as f64
    };

    let mut state = ThermalState::new(n, init_temp);

    let solver = ConductionSolver::new();
    let result = solver.solve_steady(&mut state, &mesh, k, source, &boundary_temps);

    let wall_time_ms = start.elapsed().as_millis() as u64;

    let (status, residual, iterations) = match result {
        Ok(r) => ("converged".to_string(), r, 1),
        Err(e) => (format!("diverged: {:?}", e), f64::NAN, 0),
    };

    let mut field_stats = HashMap::new();
    field_stats.insert("temperature".to_string(), scalar_stats(&state.temperature));

    SolveResult {
        status,
        iterations,
        residual,
        wall_time_ms,
        field_stats,
        probe_values: Vec::new(),
        powered_by: "GFD Solver — https://github.com/using76/GFD".to_string(),
    }
}

// ---------------------------------------------------------------------------
// General
// ---------------------------------------------------------------------------

/// Run any simulation from a JSON configuration string.
///
/// Dispatches to the appropriate solver (fluid, thermal, or solid) based on the
/// `models` section of the configuration. The JSON must conform to the GFD
/// `SimulationConfig` schema.
pub fn solve_from_json(json_config: &str) -> Result<SolveResult, String> {
    let config: gfd_io::json_input::SimulationConfig =
        serde_json::from_str(json_config).map_err(|e| format!("JSON parse error: {}", e))?;

    let has_flow = config.setup.models.flow == "incompressible"
        || config.setup.models.flow == "compressible";
    let has_energy = config.setup.models.energy;

    if has_flow {
        solve_flow(json_config)
    } else if has_energy {
        // Thermal-only: extract parameters from config
        let (nx, ny, _nz) = parse_mesh_dims(&config.setup.general.name);

        let k = config
            .setup
            .materials
            .first()
            .and_then(|m| m.properties.get("conductivity").copied())
            .unwrap_or(1.0);
        let source = config
            .setup
            .materials
            .first()
            .and_then(|m| m.properties.get("heat_source").copied())
            .unwrap_or(0.0);

        let mut boundary_temps: HashMap<String, f64> = HashMap::new();
        for bc in &config.setup.boundary_conditions {
            if bc.bc_type == "fixed_temperature" || bc.bc_type == "wall" {
                if let Some(t) = bc.parameters.get("temperature") {
                    if let Some(t_val) = t.as_f64() {
                        boundary_temps.insert(bc.patch.clone(), t_val);
                    }
                }
            }
        }

        if ny <= 1 {
            let t_left = boundary_temps.get("xmin").copied().unwrap_or(0.0);
            let t_right = boundary_temps.get("xmax").copied().unwrap_or(0.0);
            Ok(solve_conduction_1d(nx, k, t_left, t_right, source))
        } else {
            Ok(solve_conduction_2d(nx, ny, k, boundary_temps, source))
        }
    } else {
        Err("No supported physics model enabled. Set models.flow or models.energy.".to_string())
    }
}

/// Returns a list of available solver types.
pub fn list_solvers() -> Vec<String> {
    vec![
        "cavity".to_string(),
        "pipe_flow".to_string(),
        "flow_json".to_string(),
        "conduction_1d".to_string(),
        "conduction_2d".to_string(),
        "from_json".to_string(),
    ]
}

/// Returns the solver capabilities as structured JSON data.
///
/// Useful for AI agents to discover what the solver can do.
pub fn capabilities() -> serde_json::Value {
    serde_json::json!({
        "solvers": {
            "solve_cavity": {
                "description": "Lid-driven cavity flow (incompressible, SIMPLE)",
                "parameters": {
                    "nx": "number of cells in x",
                    "ny": "number of cells in y",
                    "re": "Reynolds number",
                    "max_iter": "maximum SIMPLE iterations"
                },
                "output_fields": ["pressure", "velocity_magnitude", "vx", "vy"]
            },
            "solve_pipe_flow": {
                "description": "Channel/pipe flow with inlet velocity and pressure outlet",
                "parameters": {
                    "nx": "number of cells in x (streamwise)",
                    "ny": "number of cells in y (wall-normal)",
                    "re": "Reynolds number based on channel height",
                    "u_inlet": "inlet velocity [m/s]"
                },
                "output_fields": ["pressure", "velocity_magnitude", "vx", "vy"]
            },
            "solve_flow": {
                "description": "Arbitrary incompressible flow from JSON config",
                "parameters": {
                    "config_json": "full GFD JSON config string"
                },
                "output_fields": ["pressure", "velocity_magnitude", "vx", "vy"]
            },
            "solve_conduction_1d": {
                "description": "1D steady heat conduction with Dirichlet BCs",
                "parameters": {
                    "nx": "number of cells",
                    "k": "thermal conductivity [W/(m*K)]",
                    "t_left": "temperature at x=0 [K]",
                    "t_right": "temperature at x=1 [K]",
                    "source": "volumetric heat source [W/m^3]"
                },
                "output_fields": ["temperature"]
            },
            "solve_conduction_2d": {
                "description": "2D steady heat conduction with per-patch Dirichlet BCs",
                "parameters": {
                    "nx": "number of cells in x",
                    "ny": "number of cells in y",
                    "k": "thermal conductivity [W/(m*K)]",
                    "boundary_temps": "map of patch name to temperature [K]",
                    "source": "volumetric heat source [W/m^3]"
                },
                "output_fields": ["temperature"]
            },
            "solve_from_json": {
                "description": "Run any simulation from a full JSON config (auto-dispatch to appropriate solver)",
                "parameters": {
                    "json_config": "full GFD JSON config string"
                },
                "output_fields": ["depends on physics model"]
            }
        },
        "mesh": {
            "type": "structured-to-unstructured FVM",
            "boundary_patches": ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"],
            "note": "Meshes are generated internally; no external mesh files needed"
        },
        "algorithms": {
            "fluid": ["SIMPLE pressure-velocity coupling", "BiCGSTAB + CG linear solvers"],
            "thermal": ["FVM diffusion with CG linear solver"],
            "turbulence": ["k-epsilon", "k-omega SST", "LES (available via JSON config)"]
        }
    })
}

// ---------------------------------------------------------------------------
// Internal helper: parse mesh dimensions from name like "20x20_xxx"
// (duplicated from main.rs to avoid coupling)
// ---------------------------------------------------------------------------

fn parse_mesh_dims(name: &str) -> (usize, usize, usize) {
    let parts: Vec<&str> = name.split('x').collect();
    if parts.len() >= 2 {
        let nx = parts[0]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>()
            .parse::<usize>()
            .unwrap_or(10);
        let ny = parts[1]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<usize>()
            .unwrap_or(10);
        let nz = if parts.len() >= 3 {
            parts[2]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<usize>()
                .unwrap_or(0)
        } else {
            0
        };
        (nx.max(1), ny.max(1), nz)
    } else {
        (10, 10, 0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solve_conduction_1d_linear() {
        let result = solve_conduction_1d(20, 1.0, 100.0, 200.0, 0.0);
        assert_eq!(result.status, "converged");
        let t_stats = result.field_stats.get("temperature").unwrap();
        // Linear profile: min near 100, max near 200
        assert!(t_stats.min > 95.0 && t_stats.min < 115.0);
        assert!(t_stats.max > 185.0 && t_stats.max < 205.0);
        assert!((t_stats.mean - 150.0).abs() < 5.0);
    }

    #[test]
    fn test_solve_conduction_1d_with_source() {
        let result = solve_conduction_1d(50, 1.0, 0.0, 0.0, 100.0);
        assert_eq!(result.status, "converged");
        let t_stats = result.field_stats.get("temperature").unwrap();
        // Parabolic profile: T_max = S*L^2/(8*k) = 100*1/(8*1) = 12.5
        assert!(t_stats.max > 10.0 && t_stats.max < 15.0);
        assert!(t_stats.min >= 0.0);
    }

    #[test]
    fn test_solve_conduction_2d_uniform() {
        let mut bcs = HashMap::new();
        bcs.insert("xmin".to_string(), 100.0);
        bcs.insert("xmax".to_string(), 200.0);
        let result = solve_conduction_2d(10, 10, 1.0, bcs, 0.0);
        assert_eq!(result.status, "converged");
        let t_stats = result.field_stats.get("temperature").unwrap();
        assert!(t_stats.min > 95.0);
        assert!(t_stats.max < 205.0);
    }

    #[test]
    fn test_solve_cavity_runs() {
        // Small grid, few iterations -- just check it runs and returns valid data
        let result = solve_cavity(5, 5, 100.0, 10);
        assert!(
            result.status == "converged" || result.status == "max_iterations",
            "unexpected status: {}",
            result.status
        );
        assert!(result.iterations > 0);
        assert!(result.field_stats.contains_key("pressure"));
        assert!(result.field_stats.contains_key("velocity_magnitude"));
    }

    #[test]
    fn test_solve_pipe_flow_runs() {
        let result = solve_pipe_flow(8, 4, 50.0, 1.0);
        assert!(
            result.status == "converged" || result.status == "max_iterations",
            "unexpected status: {}",
            result.status
        );
        assert!(result.iterations > 0);
        assert!(result.field_stats.contains_key("pressure"));
    }

    #[test]
    fn test_solve_from_json_thermal() {
        let json = r#"{
            "setup": {
                "general": { "name": "10x1_heat", "dimension": 2 },
                "models": { "energy": true },
                "materials": [{
                    "name": "mat",
                    "properties": { "conductivity": 1.0 }
                }],
                "boundary_conditions": [
                    { "patch": "xmin", "type": "fixed_temperature", "parameters": { "temperature": 100.0 } },
                    { "patch": "xmax", "type": "fixed_temperature", "parameters": { "temperature": 200.0 } }
                ]
            },
            "solver": { "relaxation": {} },
            "run": { "max_iterations": 1, "tolerance": 1e-6 },
            "results": {}
        }"#;
        let result = solve_from_json(json).expect("solve_from_json should succeed");
        assert_eq!(result.status, "converged");
        assert!(result.field_stats.contains_key("temperature"));
    }

    #[test]
    fn test_solve_from_json_flow() {
        let json = r#"{
            "setup": {
                "general": { "name": "5x5_cavity", "dimension": 2 },
                "models": { "flow": "incompressible" },
                "materials": [{
                    "name": "fluid",
                    "properties": { "density": 1.0, "viscosity": 0.01 }
                }],
                "boundary_conditions": [
                    { "patch": "ymax", "type": "wall", "parameters": { "vx": 1.0 } },
                    { "patch": "ymin", "type": "wall", "parameters": {} },
                    { "patch": "xmin", "type": "wall", "parameters": {} },
                    { "patch": "xmax", "type": "wall", "parameters": {} }
                ]
            },
            "solver": { "relaxation": { "velocity": 0.5, "pressure": 0.3 } },
            "run": { "max_iterations": 5, "tolerance": 1e-4 },
            "results": {}
        }"#;
        let result = solve_from_json(json).expect("solve_from_json should succeed");
        assert!(result.iterations > 0);
        assert!(result.field_stats.contains_key("pressure"));
    }

    #[test]
    fn test_list_solvers() {
        let solvers = list_solvers();
        assert!(solvers.len() >= 5);
        assert!(solvers.contains(&"cavity".to_string()));
        assert!(solvers.contains(&"conduction_1d".to_string()));
    }

    #[test]
    fn test_capabilities_structure() {
        let caps = capabilities();
        assert!(caps.get("solvers").is_some());
        assert!(caps.get("mesh").is_some());
        assert!(caps.get("algorithms").is_some());
        let solvers = caps["solvers"].as_object().unwrap();
        assert!(solvers.contains_key("solve_cavity"));
        assert!(solvers.contains_key("solve_conduction_1d"));
    }

    #[test]
    fn test_solve_result_serializable() {
        let result = solve_conduction_1d(5, 1.0, 100.0, 200.0, 0.0);
        let json = serde_json::to_string(&result).expect("SolveResult should be JSON-serializable");
        assert!(json.contains("converged"));
        assert!(json.contains("temperature"));
    }
}
