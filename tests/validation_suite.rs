//! Integration test suite for the GFD solver.
//!
//! Runs 9 of the 10 example JSON configurations through the solver library
//! (solid_beam is excluded because main.rs does not yet support solid mechanics).
//! Each test verifies convergence and performs basic sanity checks on the solution.

use std::collections::HashMap;

use gfd_core::field::{ScalarField, VectorField};
use gfd_core::mesh::structured::StructuredMesh;
use gfd_core::mesh::unstructured::UnstructuredMesh;
use gfd_fluid::incompressible::simple::SimpleSolver;
use gfd_fluid::FluidState;
use gfd_io::json_input::{load_config, SimulationConfig};
use gfd_thermal::conduction::ConductionSolver;
use gfd_thermal::ThermalState;

// ---------------------------------------------------------------------------
// Helper: parse mesh dimensions from config name (mirrors main.rs logic)
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
// Helper: create mesh from config (mirrors main.rs logic)
// ---------------------------------------------------------------------------

fn create_mesh(config: &SimulationConfig) -> UnstructuredMesh {
    let (nx, ny, nz) = parse_mesh_dims(&config.setup.general.name);
    let sm = StructuredMesh::uniform(nx, ny, nz, 1.0, 1.0, if nz > 0 { 1.0 } else { 0.0 });
    sm.to_unstructured()
}

// ---------------------------------------------------------------------------
// Helper: run heat conduction from config (mirrors main.rs logic)
// ---------------------------------------------------------------------------

fn run_heat_conduction_from_config(config: &SimulationConfig, mesh: &UnstructuredMesh) -> (f64, ThermalState) {
    let conductivity = config
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

    let n = mesh.num_cells();
    let init_temp = config
        .setup
        .initial_conditions
        .as_ref()
        .and_then(|ic| ic.temperature)
        .unwrap_or(300.0);
    let mut state = ThermalState {
        temperature: ScalarField::from_vec("temperature", vec![init_temp; n]),
        heat_flux: None,
    };

    let solver = ConductionSolver::new();
    let residual = solver
        .solve_steady(&mut state, mesh, conductivity, source, &boundary_temps)
        .expect("Conduction solve should not fail");

    (residual, state)
}

// ---------------------------------------------------------------------------
// Helper: run fluid flow from config (mirrors main.rs logic)
// ---------------------------------------------------------------------------

fn run_fluid_flow_from_config(config: &SimulationConfig, mesh: &UnstructuredMesh) -> (f64, FluidState) {
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
                let p = bc.parameters.get("pressure").and_then(|v| v.as_f64()).unwrap_or(0.0);
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

    let n = mesh.num_cells();
    let init_vel = config
        .setup
        .initial_conditions
        .as_ref()
        .and_then(|ic| ic.velocity)
        .unwrap_or([0.0, 0.0, 0.0]);

    let mut state = FluidState {
        velocity: VectorField::from_vec("velocity", vec![init_vel; n]),
        pressure: ScalarField::zeros("pressure", n),
        density: ScalarField::from_vec("density", vec![density; n]),
        viscosity: ScalarField::from_vec("viscosity", vec![viscosity; n]),
        turb_kinetic_energy: None,
        turb_dissipation: None,
        turb_specific_dissipation: None,
        eddy_viscosity: None,
    };

    let mut solver = SimpleSolver::new(density, viscosity);
    solver.alpha_u = config.solver.relaxation.velocity;
    solver.alpha_p = config.solver.relaxation.pressure;

    // Store BCs for pressure correction internal path
    solver.set_boundary_conditions(
        boundary_velocities.clone(),
        boundary_pressure.clone(),
        wall_patches.clone(),
    );

    let max_iter = config.run.max_iterations;
    let tolerance = config.run.tolerance;

    let mut final_residual = f64::MAX;

    for iter in 0..max_iter {
        let residual = solver
            .solve_step_with_bcs(
                &mut state,
                mesh,
                &boundary_velocities,
                &boundary_pressure,
                &wall_patches,
            )
            .unwrap_or_else(|e| panic!("SIMPLE iteration {} failed: {:?}", iter, e));

        final_residual = residual;

        if residual < tolerance {
            break;
        }
    }

    (final_residual, state)
}

// ===========================================================================
// Test 1: Vortex decay — all walls, no-slip, should converge to zero velocity
// ===========================================================================

#[test]
fn test_vortex_decay() {
    let config = load_config("examples/vortex_decay.json").expect("Failed to load config");
    let mesh = create_mesh(&config);

    assert_eq!(mesh.num_cells(), 20 * 20, "Expected 400 cells for 20x20 mesh");

    let (residual, state) = run_fluid_flow_from_config(&config, &mesh);

    // With all no-slip walls and zero initial velocity, the steady state is
    // zero velocity everywhere. The solver should converge quickly.
    assert!(
        residual < 1.0,
        "Vortex decay: residual {} should be reasonable (< 1.0)",
        residual
    );

    // Pressure field should exist and have correct size
    assert_eq!(state.pressure.values().len(), 400);

    // With zero forcing, velocity magnitudes should be near zero
    let vel_data = state.velocity.values();
    let max_vel_mag: f64 = vel_data
        .iter()
        .map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
        .fold(0.0_f64, f64::max);
    assert!(
        max_vel_mag < 1.0,
        "Vortex decay: max velocity magnitude {} should be small for quiescent flow",
        max_vel_mag
    );
}

// ===========================================================================
// Test 2: Pipe flow — inlet/outlet with walls
// ===========================================================================

#[test]
fn test_pipe_flow() {
    let config = load_config("examples/pipe_flow.json").expect("Failed to load config");
    let mesh = create_mesh(&config);

    assert_eq!(mesh.num_cells(), 30 * 10, "Expected 300 cells for 30x10 mesh");

    let (residual, state) = run_fluid_flow_from_config(&config, &mesh);

    // Should converge or at least make progress
    assert!(
        residual < 10.0,
        "Pipe flow: residual {} should be reasonable",
        residual
    );

    // Velocity field should have non-zero x-component somewhere (flow driven by inlet)
    let vel_data = state.velocity.values();
    let max_vx: f64 = vel_data.iter().map(|v| v[0]).fold(f64::NEG_INFINITY, f64::max);
    assert!(
        max_vx > 0.0,
        "Pipe flow: max Vx {} should be positive (flow from inlet)",
        max_vx
    );

    // Pressure should not be NaN
    for i in 0..state.pressure.values().len() {
        let p = state.pressure.get(i).unwrap();
        assert!(!p.is_nan(), "Pipe flow: pressure at cell {} is NaN", i);
    }
}

// ===========================================================================
// Test 3: Backward-facing step
// ===========================================================================

#[test]
fn test_backward_step() {
    let config = load_config("examples/backward_step.json").expect("Failed to load config");
    let mesh = create_mesh(&config);

    assert_eq!(mesh.num_cells(), 20 * 10, "Expected 200 cells for 20x10 mesh");

    let (residual, state) = run_fluid_flow_from_config(&config, &mesh);

    // Should converge or at least make progress
    assert!(
        residual < 10.0,
        "Backward step: residual {} should be reasonable",
        residual
    );

    // Velocity field should have correct size and no NaN values
    let n = mesh.num_cells();
    let mut has_nan = false;
    for i in 0..n {
        let v = state.velocity.get(i).unwrap();
        if v[0].is_nan() || v[1].is_nan() || v[2].is_nan() {
            has_nan = true;
            break;
        }
    }
    assert!(!has_nan, "Backward step: velocity field contains NaN");

    // No NaN in pressure
    for i in 0..n {
        let p = state.pressure.get(i).unwrap();
        assert!(!p.is_nan(), "Backward step: pressure at cell {} is NaN", i);
    }
}

// ===========================================================================
// Test 4: Natural convection — flow + energy (flow-only since main.rs ignores energy for flow)
// ===========================================================================

#[test]
fn test_natural_convection() {
    let config = load_config("examples/natural_convection.json").expect("Failed to load config");
    let mesh = create_mesh(&config);

    assert_eq!(mesh.num_cells(), 20 * 20, "Expected 400 cells for 20x20 mesh");

    // The current main.rs runs flow only when has_flow is true (ignoring energy).
    // We mirror that behavior here.
    let (residual, state) = run_fluid_flow_from_config(&config, &mesh);

    assert!(
        residual < 10.0,
        "Natural convection (flow part): residual {} should be reasonable",
        residual
    );

    // Verify no NaN in solution fields
    for i in 0..state.pressure.values().len() {
        let p = state.pressure.get(i).unwrap();
        assert!(!p.is_nan(), "Natural convection: pressure at cell {} is NaN", i);
    }
    let vel_data = state.velocity.values();
    for (i, v) in vel_data.iter().enumerate() {
        assert!(!v[0].is_nan() && !v[1].is_nan() && !v[2].is_nan(),
            "Natural convection: velocity at cell {} is NaN", i);
    }
}

// ===========================================================================
// Test 5: Forced convection — flow + energy (flow-only since main.rs ignores energy for flow)
// ===========================================================================

#[test]
fn test_forced_convection() {
    let config = load_config("examples/forced_convection.json").expect("Failed to load config");
    let mesh = create_mesh(&config);

    assert_eq!(mesh.num_cells(), 30 * 10, "Expected 300 cells for 30x10 mesh");

    // Flow-only (energy is not solved in the coupled flow path of main.rs)
    let (residual, state) = run_fluid_flow_from_config(&config, &mesh);

    assert!(
        residual < 10.0,
        "Forced convection (flow part): residual {} should be reasonable",
        residual
    );

    // Inlet-driven flow should produce positive x-velocity
    let vel_data = state.velocity.values();
    let max_vx: f64 = vel_data.iter().map(|v| v[0]).fold(f64::NEG_INFINITY, f64::max);
    assert!(
        max_vx > 0.0,
        "Forced convection: max Vx {} should be positive",
        max_vx
    );
}

// ===========================================================================
// Test 6: Heat sink — energy-only with volumetric source, T=300 on all walls
// ===========================================================================

#[test]
fn test_heat_sink() {
    let config = load_config("examples/heat_sink.json").expect("Failed to load config");
    let mesh = create_mesh(&config);

    assert_eq!(mesh.num_cells(), 20 * 20, "Expected 400 cells for 20x20 mesh");

    let (residual, state) = run_heat_conduction_from_config(&config, &mesh);

    assert!(
        residual < 1e-4,
        "Heat sink: linear solver residual {} should be small",
        residual
    );

    // With T=300 on all boundaries and positive source, interior should be > 300
    let n = mesh.num_cells();
    let mut max_temp = f64::NEG_INFINITY;
    let mut min_temp = f64::INFINITY;
    for i in 0..n {
        let t = state.temperature.get(i).unwrap();
        assert!(!t.is_nan(), "Heat sink: temperature at cell {} is NaN", i);
        if t > max_temp { max_temp = t; }
        if t < min_temp { min_temp = t; }
    }

    assert!(
        max_temp > 300.0,
        "Heat sink: max temperature {} should exceed boundary T=300 due to source term",
        max_temp
    );
    assert!(
        min_temp >= 299.0,
        "Heat sink: min temperature {} should not drop far below boundary T=300",
        min_temp
    );

    // Peak temperature should be at center (symmetric BCs + uniform source)
    // For a 20x20 unit square with S=1000, k=10: analytical peak ~ 300 + S*L^2/(8k) = 300 + 12.5
    // 2D gives a slightly different formula but temperature should be in a reasonable range
    assert!(
        max_temp < 400.0,
        "Heat sink: max temperature {} should be bounded above",
        max_temp
    );
}

// ===========================================================================
// Test 7: Heat transfer fins — linear temperature profile (xmin=500, xmax=300)
// ===========================================================================

#[test]
fn test_heat_transfer_fins() {
    let config = load_config("examples/heat_transfer_fins.json").expect("Failed to load config");
    let mesh = create_mesh(&config);

    assert_eq!(mesh.num_cells(), 30 * 10, "Expected 300 cells for 30x10 mesh");

    let (residual, state) = run_heat_conduction_from_config(&config, &mesh);

    assert!(
        residual < 1e-4,
        "Heat transfer fins: linear solver residual {} should be small",
        residual
    );

    // With Dirichlet BCs at xmin=500 and xmax=300 and no source,
    // temperature should be a linear profile in x.
    // ymin/ymax are zero-gradient by default (no BC specified).
    let n = mesh.num_cells();
    for i in 0..n {
        let t = state.temperature.get(i).unwrap();
        assert!(!t.is_nan(), "Heat transfer fins: temperature at cell {} is NaN", i);
        assert!(
            t >= 290.0 && t <= 510.0,
            "Heat transfer fins: temperature {} at cell {} should be in [290, 510]",
            t, i
        );
    }

    // Check that temperature decreases from left to right (approximately)
    // Get temperatures for cells in the first column (low x) vs last column (high x)
    let cells = &mesh.cells;
    let mut left_temps = Vec::new();
    let mut right_temps = Vec::new();
    for i in 0..n {
        let cx = cells[i].center[0];
        let t = state.temperature.get(i).unwrap();
        if cx < 0.1 {
            left_temps.push(t);
        } else if cx > 0.9 {
            right_temps.push(t);
        }
    }

    if !left_temps.is_empty() && !right_temps.is_empty() {
        let avg_left: f64 = left_temps.iter().sum::<f64>() / left_temps.len() as f64;
        let avg_right: f64 = right_temps.iter().sum::<f64>() / right_temps.len() as f64;
        assert!(
            avg_left > avg_right,
            "Heat transfer fins: average left T ({}) should be > average right T ({})",
            avg_left, avg_right
        );
    }
}

// ===========================================================================
// Test 8: Solid beam — SKIPPED (solid mechanics not yet supported in main.rs)
// ===========================================================================

#[test]
fn test_solid_beam_config_loads() {
    // Only verify the config loads correctly; the solver does not support solid mechanics yet.
    let config = load_config("examples/solid_beam.json").expect("Failed to load solid_beam config");
    assert_eq!(config.setup.models.solid, "linear_elastic");
    assert_eq!(config.setup.models.flow, "none");
    assert!(!config.setup.models.energy);
    // TODO: Run the actual solid mechanics solver when it is integrated into main.rs
}

// ===========================================================================
// Test 9: Diffusion reaction — energy-only, source=100, T=0 on all boundaries
// ===========================================================================

#[test]
fn test_diffusion_reaction() {
    let config = load_config("examples/diffusion_reaction.json").expect("Failed to load config");
    let mesh = create_mesh(&config);

    assert_eq!(mesh.num_cells(), 20 * 20, "Expected 400 cells for 20x20 mesh");

    let (residual, state) = run_heat_conduction_from_config(&config, &mesh);

    assert!(
        residual < 1e-4,
        "Diffusion reaction: linear solver residual {} should be small",
        residual
    );

    // With T=0 on all boundaries and positive source (S=100, k=1),
    // interior temperatures should be positive.
    let n = mesh.num_cells();
    let mut max_temp = f64::NEG_INFINITY;
    for i in 0..n {
        let t = state.temperature.get(i).unwrap();
        assert!(!t.is_nan(), "Diffusion reaction: temperature at cell {} is NaN", i);
        assert!(
            t >= -0.1,
            "Diffusion reaction: temperature {} at cell {} should be non-negative",
            t, i
        );
        if t > max_temp { max_temp = t; }
    }

    // Maximum temperature should be at the center.
    // For 2D Poisson with S=100, k=1. L=1: peak around S*L^2/8k ~ 12.5 (1D estimate).
    // 2D gives a somewhat smaller peak but should still be significantly positive.
    assert!(
        max_temp > 1.0,
        "Diffusion reaction: max temperature {} should be significantly positive",
        max_temp
    );
    assert!(
        max_temp < 100.0,
        "Diffusion reaction: max temperature {} should be bounded",
        max_temp
    );
}

// ===========================================================================
// Test 10: Multi-region conduction — linear profile, high conductivity
// ===========================================================================

#[test]
fn test_multi_region_conduction() {
    let config = load_config("examples/multi_region_conduction.json").expect("Failed to load config");
    let mesh = create_mesh(&config);

    assert_eq!(mesh.num_cells(), 20 * 10, "Expected 200 cells for 20x10 mesh");

    let (residual, state) = run_heat_conduction_from_config(&config, &mesh);

    assert!(
        residual < 1e-4,
        "Multi-region conduction: residual {} should be small",
        residual
    );

    // With xmin=1000 and xmax=300, no source, the temperature should form
    // a linear profile in x: T(x) = 1000 - 700*x (domain is [0,1]).
    // ymin/ymax are zero-gradient (insulated by default).
    let n = mesh.num_cells();
    let cells = &mesh.cells;

    for i in 0..n {
        let t = state.temperature.get(i).unwrap();
        assert!(!t.is_nan(), "Multi-region conduction: temperature at cell {} is NaN", i);
        assert!(
            t >= 290.0 && t <= 1010.0,
            "Multi-region conduction: temperature {} at cell {} outside expected range",
            t, i
        );
    }

    // Verify approximate linearity: check a few cells and compare with analytical
    let mut max_error = 0.0_f64;
    for i in 0..n {
        let x = cells[i].center[0];
        let t_computed = state.temperature.get(i).unwrap();
        let t_analytical = 1000.0 - 700.0 * x;
        let error = (t_computed - t_analytical).abs();
        if error > max_error {
            max_error = error;
        }
    }

    // With high conductivity and no source, the profile should be very close to linear.
    // Allow some FVM discretization error.
    assert!(
        max_error < 50.0,
        "Multi-region conduction: max deviation from linear profile {} should be small",
        max_error
    );
}
