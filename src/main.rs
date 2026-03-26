use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

use gfd_core::field::{Field, FieldData, ScalarField, VectorField};
use gfd_core::mesh::structured::StructuredMesh;
use gfd_io::json_input::{load_config, SimulationConfig};

pub mod config_gen;

// ---------------------------------------------------------------------------
// JSON output data structures
// ---------------------------------------------------------------------------

/// Summary of a single field's statistics.
#[derive(Debug, Clone, Serialize)]
pub struct FieldStats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
}

/// A probe result containing extracted field values at a specific location.
#[derive(Debug, Clone, Serialize)]
pub struct ProbeResult {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pressure: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub velocity: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
}

/// Probe point definition in the JSON config.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProbeConfig {
    pub name: String,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub z: f64,
}

/// Structured JSON output from a simulation run.
#[derive(Debug, Clone, Serialize)]
pub struct SimulationResult {
    pub status: String,
    pub iterations: usize,
    pub final_residual: f64,
    pub wall_time_ms: u64,
    pub fields: HashMap<String, FieldStats>,
    pub probes: Vec<ProbeResult>,
    pub vtk_path: String,
}

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

/// GFD -- Generalized Finite Difference multi-physics solver
const GFD_BANNER: &str = r#"
  ╔═══════════════════════════════════════════════════╗
  ║  GFD — Generalized Fluid Dynamics Solver          ║
  ║  Powered by GFD  |  https://github.com/using76/GFD║
  ║  Licensed under Modified MIT License              ║
  ╚═══════════════════════════════════════════════════╝
"#;

#[derive(Parser, Debug)]
#[command(
    name = "gfd",
    version,
    about = "GFD — Universal multi-physics solver (CFD, thermal, structural)\nPowered by GFD | https://github.com/using76/GFD",
    long_about = None,
    after_help = "Powered by GFD Solver — Modified MIT License\nhttps://github.com/using76/GFD"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run a simulation from a JSON configuration file
    Run {
        /// Path to the simulation JSON configuration
        #[arg(value_name = "CONFIG")]
        config: PathBuf,

        /// Number of worker threads (defaults to all available cores)
        #[arg(short, long)]
        threads: Option<usize>,

        /// Enable GPU acceleration for linear solves (requires CUDA and the `gpu` feature)
        #[arg(long)]
        gpu: bool,

        /// Output a structured JSON result summary to stdout
        #[arg(long)]
        json_output: bool,

        /// JSON file or inline JSON string with probe point definitions
        /// (array of {"name", "x", "y", "z"} objects)
        #[arg(long)]
        probes: Option<String>,
    },

    /// Run multiple simulations from JSON configuration files (batch mode)
    Batch {
        /// Paths to the simulation JSON configurations
        #[arg(value_name = "CONFIGS")]
        configs: Vec<PathBuf>,

        /// Number of worker threads (defaults to all available cores)
        #[arg(short, long)]
        threads: Option<usize>,

        /// Enable GPU acceleration for linear solves
        #[arg(long)]
        gpu: bool,

        /// Output a structured JSON result array to stdout
        #[arg(long)]
        json_output: bool,

        /// JSON file or inline JSON string with probe point definitions
        #[arg(long)]
        probes: Option<String>,
    },

    /// Validate a simulation JSON configuration without running
    Validate {
        /// Path to the simulation JSON configuration
        #[arg(value_name = "CONFIG")]
        config: PathBuf,
    },

    /// Check / inspect a mesh file
    CheckMesh {
        /// Path to the mesh file
        #[arg(value_name = "MESH")]
        mesh: PathBuf,
    },

    /// Convert between supported file formats
    Convert {
        /// Path to the input file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Target output format (e.g. "vtk", "json", "csv")
        #[arg(short, long, default_value = "vtk")]
        format: String,
    },

    /// Run the benchmark suite and report metrics
    Benchmark,

    /// Generate a JSON config for a standard simulation template
    GenerateConfig {
        /// Template name: "lid_driven_cavity", "pipe_flow", "heat_conduction"
        #[arg(value_name = "TEMPLATE")]
        template: String,

        /// Grid size in X direction
        #[arg(long, default_value = "20")]
        nx: usize,

        /// Grid size in Y direction
        #[arg(long, default_value = "20")]
        ny: usize,

        /// Reynolds number (for flow templates)
        #[arg(long, default_value = "100")]
        re: f64,

        /// Thermal conductivity (for heat conduction)
        #[arg(long, default_value = "1.0")]
        conductivity: f64,

        /// Left boundary temperature (for heat conduction)
        #[arg(long, default_value = "100.0")]
        t_left: f64,

        /// Right boundary temperature (for heat conduction)
        #[arg(long, default_value = "200.0")]
        t_right: f64,
    },
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    // Print branding banner (required by Modified MIT License)
    eprintln!("{}", GFD_BANNER);

    match cli.command {
        Commands::Run {
            config,
            threads,
            gpu,
            json_output,
            probes,
        } => {
            tracing::info!(?config, ?threads, gpu, json_output, "Starting simulation");
            let probe_configs = parse_probes(&probes)?;
            let result = run_simulation(&config, threads, gpu, &probe_configs)?;
            if json_output {
                let json = serde_json::to_string_pretty(&result)
                    .context("Failed to serialize result to JSON")?;
                println!("{}", json);
            }
        }
        Commands::Batch {
            configs,
            threads,
            gpu,
            json_output,
            probes,
        } => {
            tracing::info!(count = configs.len(), "Starting batch simulation");
            let probe_configs = parse_probes(&probes)?;
            let mut results = Vec::new();
            for config_path in &configs {
                tracing::info!(?config_path, "Running batch item");
                match run_simulation(config_path, threads, gpu, &probe_configs) {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        tracing::error!(?config_path, error = %e, "Batch item failed");
                        results.push(SimulationResult {
                            status: format!("error: {}", e),
                            iterations: 0,
                            final_residual: f64::NAN,
                            wall_time_ms: 0,
                            fields: HashMap::new(),
                            probes: Vec::new(),
                            vtk_path: String::new(),
                        });
                    }
                }
            }
            if json_output {
                let json = serde_json::to_string_pretty(&results)
                    .context("Failed to serialize batch results to JSON")?;
                println!("{}", json);
            }
        }
        Commands::Validate { config } => {
            tracing::info!(?config, "Validating configuration");
            validate_config(&config)?;
        }
        Commands::CheckMesh { mesh } => {
            tracing::info!(?mesh, "Checking mesh");
            check_mesh(&mesh)?;
        }
        Commands::Convert { input, format } => {
            tracing::info!(?input, %format, "Converting file");
            convert_file(&input, &format)?;
        }
        Commands::Benchmark => {
            tracing::info!("Running benchmark suite...");
            let status = std::process::Command::new("cargo")
                .args(["run", "--release", "--bin", "gfd-benchmark"])
                .status()
                .context("Failed to run benchmark")?;
            if !status.success() {
                anyhow::bail!("Benchmark failed with exit code: {:?}", status.code());
            }
        }
        Commands::GenerateConfig {
            template,
            nx,
            ny,
            re,
            conductivity,
            t_left,
            t_right,
        } => {
            let config = match template.as_str() {
                "lid_driven_cavity" => config_gen::lid_driven_cavity(nx, ny, re),
                "pipe_flow" => config_gen::pipe_flow(nx, ny, re),
                "heat_conduction" => config_gen::heat_conduction(nx, ny, conductivity, t_left, t_right),
                _ => anyhow::bail!(
                    "Unknown template '{}'. Available: lid_driven_cavity, pipe_flow, heat_conduction",
                    template
                ),
            };
            let json = serde_json::to_string_pretty(&config)
                .context("Failed to serialize config to JSON")?;
            println!("{}", json);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Probe parsing
// ---------------------------------------------------------------------------

/// Parse probe definitions from a CLI argument.
///
/// The argument can be either:
/// - A path to a JSON file containing an array of probe objects
/// - An inline JSON string (array of `{"name", "x", "y", "z"}`)
fn parse_probes(probes_arg: &Option<String>) -> Result<Vec<ProbeConfig>> {
    let Some(arg) = probes_arg else {
        return Ok(Vec::new());
    };

    // Try to parse as inline JSON first
    if let Ok(probes) = serde_json::from_str::<Vec<ProbeConfig>>(arg) {
        return Ok(probes);
    }

    // Try to read as a file path
    let contents = std::fs::read_to_string(arg)
        .with_context(|| format!("Failed to read probes file: {}", arg))?;
    let probes: Vec<ProbeConfig> = serde_json::from_str(&contents)
        .with_context(|| "Failed to parse probes JSON")?;
    Ok(probes)
}

/// Find the nearest cell to a given point in the mesh.
fn find_nearest_cell(
    mesh: &gfd_core::mesh::unstructured::UnstructuredMesh,
    x: f64,
    y: f64,
    z: f64,
) -> usize {
    let mut best_cell = 0;
    let mut best_dist_sq = f64::MAX;
    for (i, cell) in mesh.cells.iter().enumerate() {
        let dx = cell.center[0] - x;
        let dy = cell.center[1] - y;
        let dz = cell.center[2] - z;
        let dist_sq = dx * dx + dy * dy + dz * dz;
        if dist_sq < best_dist_sq {
            best_dist_sq = dist_sq;
            best_cell = i;
        }
    }
    best_cell
}

/// Extract probe values from the current field state.
fn extract_probes(
    mesh: &gfd_core::mesh::unstructured::UnstructuredMesh,
    probe_configs: &[ProbeConfig],
    pressure: Option<&ScalarField>,
    velocity: Option<&VectorField>,
    temperature: Option<&ScalarField>,
) -> Vec<ProbeResult> {
    probe_configs
        .iter()
        .map(|pc| {
            let cell_id = find_nearest_cell(mesh, pc.x, pc.y, pc.z);
            ProbeResult {
                name: pc.name.clone(),
                x: pc.x,
                y: pc.y,
                z: pc.z,
                pressure: pressure.and_then(|p| p.get(cell_id).ok()),
                velocity: velocity.and_then(|v| v.get(cell_id).ok()),
                temperature: temperature.and_then(|t| t.get(cell_id).ok()),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Field statistics
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
    let mut min = f64::MAX;
    let mut max = f64::MIN;
    let mut sum = 0.0;
    for &v in vals {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
        sum += v;
    }
    FieldStats {
        min,
        max,
        mean: sum / vals.len() as f64,
    }
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
    let mut min = f64::MAX;
    let mut max = f64::MIN;
    let mut sum = 0.0;
    for v in vals {
        let mag = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if mag < min {
            min = mag;
        }
        if mag > max {
            max = mag;
        }
        sum += mag;
    }
    FieldStats {
        min,
        max,
        mean: sum / vals.len() as f64,
    }
}

// ---------------------------------------------------------------------------
// Run simulation
// ---------------------------------------------------------------------------

fn run_simulation(
    config_path: &PathBuf,
    threads: Option<usize>,
    gpu: bool,
    probe_configs: &[ProbeConfig],
) -> Result<SimulationResult> {
    // 1. Thread pool
    if let Some(n) = threads {
        gfd_parallel::thread_pool::configure_thread_pool(n)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        tracing::info!(threads = n, "Thread pool configured");
    }

    // 2. Load config
    let config = load_config(config_path.to_str().unwrap_or("simulation.json"))
        .with_context(|| "Failed to load simulation config")?;
    tracing::info!(name = %config.setup.general.name, "Configuration loaded");

    // 3. Generate mesh (structured grid from config dimensions)
    let mesh = create_mesh(&config)?;
    tracing::info!(
        cells = mesh.num_cells(),
        faces = mesh.num_faces(),
        nodes = mesh.num_nodes(),
        "Mesh created"
    );

    // 4. Determine physics and run
    let has_flow = config.setup.models.flow == "incompressible"
        || config.setup.models.flow == "compressible";
    let has_energy = config.setup.models.energy;
    let has_solid = config.setup.models.solid == "linear_elastic"
        || config.setup.models.solid == "hyperelastic";

    if gpu {
        tracing::info!("GPU acceleration enabled for linear solves");
    }

    let start = std::time::Instant::now();

    let result = if has_solid {
        run_solid_mechanics(&config, &mesh, probe_configs)?
    } else if has_energy && !has_flow {
        run_heat_conduction(&config, &mesh, probe_configs)?
    } else if has_flow {
        run_fluid_flow(&config, &mesh, has_energy, gpu, probe_configs)?
    } else {
        anyhow::bail!("No physics model enabled. Set models.flow, models.energy, or models.solid in config.");
    };

    // Patch wall_time_ms with the actual elapsed time
    let wall_time_ms = start.elapsed().as_millis() as u64;
    let result = SimulationResult {
        wall_time_ms,
        ..result
    };

    Ok(result)
}

/// Create a mesh from config. Uses structured mesh generation.
fn create_mesh(
    config: &SimulationConfig,
) -> Result<gfd_core::mesh::unstructured::UnstructuredMesh> {
    // Extract mesh dimensions from config general section or material properties
    // Default: 10x10x1 unit domain
    let nx = config
        .setup
        .general
        .dimension
        .max(2);
    let mesh_size = nx * nx; // reasonable default
    let _ = mesh_size;

    // Look for mesh parameters in the config name or defaults
    let (nx, ny, nz) = parse_mesh_dims(&config.setup.general.name);
    let sm = StructuredMesh::uniform(nx, ny, nz, 1.0, 1.0, if nz > 0 { 1.0 } else { 0.0 });
    Ok(sm.to_unstructured())
}

/// Parse mesh dimensions from simulation name convention "NxMxK" or use defaults.
fn parse_mesh_dims(name: &str) -> (usize, usize, usize) {
    // Try to find patterns like "20x20" or "10x10x1" in the name
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
        (10, 10, 0) // default 2D 10x10
    }
}

// ---------------------------------------------------------------------------
// Heat conduction solver
// ---------------------------------------------------------------------------

fn run_heat_conduction(
    config: &SimulationConfig,
    mesh: &gfd_core::mesh::unstructured::UnstructuredMesh,
    probe_configs: &[ProbeConfig],
) -> Result<SimulationResult> {
    use gfd_thermal::ThermalState;
    use gfd_thermal::conduction::ConductionSolver;

    // Extract material properties
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

    tracing::info!(k = conductivity, source = source, "Heat conduction setup");

    // Build boundary conditions: patch_name -> temperature
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

    // Initialize state
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

    // Solve
    let solver = ConductionSolver::new();
    let residual = solver
        .solve_steady(&mut state, mesh, conductivity, source, &boundary_temps)
        .map_err(|e| anyhow::anyhow!("Conduction solve failed: {:?}", e))?;

    tracing::info!(residual = residual, "Conduction solved");

    // Write output
    let vtk_path = write_results(config, mesh, &state.temperature, None)?;

    // Build field stats
    let mut fields = HashMap::new();
    fields.insert("temperature".to_string(), scalar_stats(&state.temperature));

    // Extract probes
    let probes = extract_probes(mesh, probe_configs, None, None, Some(&state.temperature));

    let status = if residual < config.run.tolerance {
        "converged"
    } else {
        "max_iterations"
    };

    Ok(SimulationResult {
        status: status.to_string(),
        iterations: 1,
        final_residual: residual,
        wall_time_ms: 0, // will be overwritten by caller
        fields,
        probes,
        vtk_path,
    })
}

// ---------------------------------------------------------------------------
// Fluid flow solver (SIMPLE)
// ---------------------------------------------------------------------------

fn run_fluid_flow(
    config: &SimulationConfig,
    mesh: &gfd_core::mesh::unstructured::UnstructuredMesh,
    has_energy: bool,
    gpu: bool,
    probe_configs: &[ProbeConfig],
) -> Result<SimulationResult> {
    use gfd_fluid::FluidState;
    use gfd_fluid::incompressible::simple::SimpleSolver;

    // Extract material properties
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

    tracing::info!(rho = density, mu = viscosity, "Fluid flow setup");

    // Build boundary conditions
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
                // Check if wall has a velocity (moving wall like lid-driven cavity)
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

    // Auto-add unlisted boundary patches as no-slip walls (e.g., zmin/zmax for 2D meshes)
    for patch in &mesh.boundary_patches {
        let name = &patch.name;
        if !boundary_velocities.contains_key(name)
            && !boundary_pressure.contains_key(name)
            && !wall_patches.contains(name)
        {
            wall_patches.push(name.clone());
        }
    }

    // Initialize state
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

    // Create SIMPLE solver
    let mut solver = SimpleSolver::new(density, viscosity);
    solver.alpha_u = config.solver.relaxation.velocity;
    solver.alpha_p = config.solver.relaxation.pressure;
    solver.use_gpu = gpu;

    // Store BCs in solver for pressure correction (required for inlet mass flux)
    solver.set_boundary_conditions(
        boundary_velocities.clone(),
        boundary_pressure.clone(),
        wall_patches.clone(),
    );

    // Energy equation setup (if enabled)
    let mut thermal_state = if has_energy {
        use gfd_thermal::ThermalState;
        let init_temp = config.setup.initial_conditions.as_ref()
            .and_then(|ic| ic.temperature).unwrap_or(300.0);
        Some(ThermalState::new(n, init_temp))
    } else {
        None
    };

    let mut energy_solver = if has_energy {
        use gfd_thermal::convection::ConvectionDiffusionSolver;
        let conductivity = config.setup.materials.first()
            .and_then(|m| m.properties.get("conductivity").copied()).unwrap_or(1.0);
        let specific_heat = config.setup.materials.first()
            .and_then(|m| m.properties.get("specific_heat").copied()).unwrap_or(1000.0);
        let source = config.setup.materials.first()
            .and_then(|m| m.properties.get("heat_source").copied()).unwrap_or(0.0);
        let mut s = ConvectionDiffusionSolver::with_properties(density, specific_heat, conductivity, source);
        // Set thermal BCs
        let mut thermal_bcs = HashMap::new();
        for bc in &config.setup.boundary_conditions {
            if let Some(t) = bc.parameters.get("temperature").and_then(|v| v.as_f64()) {
                thermal_bcs.insert(bc.patch.clone(), t);
            }
        }
        s.set_boundary_temps(thermal_bcs);
        tracing::info!(k = conductivity, cp = specific_heat, "Energy equation enabled");
        Some(s)
    } else {
        None
    };

    // Iteration loop
    let max_iter = config.run.max_iterations;
    let tolerance = config.run.tolerance;

    tracing::info!(max_iter, tolerance, "Starting SIMPLE iterations");

    let mut final_residual = f64::MAX;
    let mut final_iter = 0;
    let mut converged = false;

    for iter in 0..max_iter {
        let residual = solver
            .solve_step_with_bcs(
                &mut state,
                mesh,
                &boundary_velocities,
                &boundary_pressure,
                &wall_patches,
            )
            .map_err(|e| anyhow::anyhow!("SIMPLE iteration {} failed: {:?}", iter, e))?;

        // Solve energy equation if enabled (coupled with velocity field)
        if let (Some(ref mut e_solver), Some(ref mut t_state)) = (&mut energy_solver, &mut thermal_state) {
            let dt = 1.0; // pseudo time step for steady
            e_solver.solve_step(t_state, &state.velocity, mesh, dt)
                .map_err(|e| anyhow::anyhow!("Energy equation failed: {:?}", e))?;
        }

        final_residual = residual;
        final_iter = iter + 1;

        if iter % 10 == 0 || residual < tolerance {
            tracing::info!(iter, residual, "SIMPLE iteration");
        }

        if residual < tolerance {
            tracing::info!(iter, residual, "Converged!");
            converged = true;
            break;
        }
    }

    // Write output
    let vtk_path = write_results(config, mesh, &state.pressure, Some(&state.velocity))?;

    // Build field stats
    let mut fields = HashMap::new();
    fields.insert("pressure".to_string(), scalar_stats(&state.pressure));
    fields.insert(
        "velocity_magnitude".to_string(),
        velocity_magnitude_stats(&state.velocity),
    );
    if let Some(ref t_state) = thermal_state {
        fields.insert(
            "temperature".to_string(),
            scalar_stats(&t_state.temperature),
        );
    }

    // Extract probes
    let temp_field = thermal_state.as_ref().map(|ts| &ts.temperature);
    let probes = extract_probes(
        mesh,
        probe_configs,
        Some(&state.pressure),
        Some(&state.velocity),
        temp_field,
    );

    let status = if converged {
        "converged"
    } else {
        "max_iterations"
    };

    Ok(SimulationResult {
        status: status.to_string(),
        iterations: final_iter,
        final_residual,
        wall_time_ms: 0, // will be overwritten by caller
        fields,
        probes,
        vtk_path,
    })
}

// ---------------------------------------------------------------------------
// Solid mechanics solver (FEM)
// ---------------------------------------------------------------------------

fn run_solid_mechanics(
    config: &SimulationConfig,
    mesh: &gfd_core::mesh::unstructured::UnstructuredMesh,
    probe_configs: &[ProbeConfig],
) -> Result<SimulationResult> {
    use gfd_solid::elastic::LinearElasticSolver;
    use gfd_solid::SolidState;

    let youngs_modulus = config
        .setup
        .materials
        .first()
        .and_then(|m| m.properties.get("youngs_modulus").copied())
        .unwrap_or(200e9);
    let poisson_ratio = config
        .setup
        .materials
        .first()
        .and_then(|m| m.properties.get("poisson_ratio").copied())
        .unwrap_or(0.3);

    tracing::info!(E = youngs_modulus, nu = poisson_ratio, "Solid mechanics setup");

    let solver = LinearElasticSolver::new(youngs_modulus, poisson_ratio);

    let mut fixed_patches: Vec<String> = Vec::new();
    let mut force_patches: HashMap<String, [f64; 3]> = HashMap::new();

    for bc in &config.setup.boundary_conditions {
        match bc.bc_type.as_str() {
            "fixed" | "clamped" => {
                fixed_patches.push(bc.patch.clone());
            }
            "force" | "traction" => {
                let fx = bc.parameters.get("fx").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let fy = bc.parameters.get("fy").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let fz = bc.parameters.get("fz").and_then(|v| v.as_f64()).unwrap_or(0.0);
                force_patches.insert(bc.patch.clone(), [fx, fy, fz]);
            }
            _ => {}
        }
    }

    let n = mesh.num_cells();
    let mut state = SolidState {
        displacement: VectorField::from_vec("displacement", vec![[0.0; 3]; n]),
        stress: gfd_core::field::TensorField::zeros("stress", n),
        strain: gfd_core::field::TensorField::zeros("strain", n),
    };

    let body_force = [0.0, 0.0, 0.0];
    let max_disp = solver
        .solve(&mut state, mesh, body_force, &fixed_patches, &force_patches)
        .map_err(|e| anyhow::anyhow!("Solid solver failed: {:?}", e))?;

    tracing::info!(max_displacement = max_disp, "Solid mechanics solved");

    let disp_mag = ScalarField::from_vec(
        "displacement_magnitude",
        state
            .displacement
            .values()
            .iter()
            .map(|d| (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt())
            .collect(),
    );
    let vtk_path = write_results(config, mesh, &disp_mag, Some(&state.displacement))?;

    // Build field stats
    let mut fields = HashMap::new();
    fields.insert(
        "displacement_magnitude".to_string(),
        scalar_stats(&disp_mag),
    );

    // Extract probes (pressure/temperature not available for solid)
    let probes = extract_probes(mesh, probe_configs, None, None, None);

    Ok(SimulationResult {
        status: "converged".to_string(),
        iterations: 1,
        final_residual: max_disp,
        wall_time_ms: 0,
        fields,
        probes,
        vtk_path,
    })
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn write_results(
    config: &SimulationConfig,
    mesh: &gfd_core::mesh::unstructured::UnstructuredMesh,
    scalar: &ScalarField,
    vector: Option<&VectorField>,
) -> Result<String> {
    let output_dir = &config.results.output_dir;
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create output dir: {}", output_dir))?;

    let output_path = format!("{}/result.vtk", output_dir);

    let mut fields: HashMap<String, FieldData> = HashMap::new();
    fields.insert(scalar.name().to_string(), FieldData::Scalar(scalar.clone()));
    if let Some(v) = vector {
        fields.insert(v.name().to_string(), FieldData::Vector(v.clone()));
    }

    gfd_io::vtk_writer::write_vtk(&output_path, mesh, &fields)
        .map_err(|e| anyhow::anyhow!("VTK write failed: {:?}", e))?;

    tracing::info!(path = %output_path, "Results written");
    Ok(output_path)
}

// ---------------------------------------------------------------------------
// Other subcommands
// ---------------------------------------------------------------------------

fn validate_config(config_path: &PathBuf) -> Result<()> {
    let _config = load_config(config_path.to_str().unwrap_or(""))
        .with_context(|| "Failed to parse config")?;
    tracing::info!("Configuration is valid");
    Ok(())
}

fn check_mesh(mesh_path: &PathBuf) -> Result<()> {
    let _mesh_text = std::fs::read_to_string(mesh_path)
        .with_context(|| format!("Failed to read mesh file: {}", mesh_path.display()))?;
    tracing::info!("Mesh check complete (stub)");
    Ok(())
}

fn convert_file(input_path: &PathBuf, format: &str) -> Result<()> {
    let _input_text = std::fs::read_to_string(input_path)
        .with_context(|| format!("Failed to read input file: {}", input_path.display()))?;
    tracing::info!(%format, "Conversion complete (stub)");
    Ok(())
}
