use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::path::PathBuf;

use gfd_core::field::{Field, FieldData, ScalarField, VectorField};
use gfd_core::mesh::structured::StructuredMesh;
use gfd_io::json_input::{load_config, SimulationConfig};

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

/// GFD -- Generalized Finite Difference multi-physics solver
#[derive(Parser, Debug)]
#[command(name = "gfd", version, about, long_about = None)]
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

    match cli.command {
        Commands::Run { config, threads, gpu } => {
            tracing::info!(?config, ?threads, gpu, "Starting simulation");
            run_simulation(&config, threads, gpu)?;
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
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Run simulation
// ---------------------------------------------------------------------------

fn run_simulation(config_path: &PathBuf, threads: Option<usize>, gpu: bool) -> Result<()> {
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
    let has_flow = config.setup.models.flow == "incompressible";
    let has_energy = config.setup.models.energy;

    if gpu {
        tracing::info!("GPU acceleration enabled for linear solves");
    }

    if has_energy && !has_flow {
        run_heat_conduction(&config, &mesh)?;
    } else if has_flow {
        run_fluid_flow(&config, &mesh, has_energy, gpu)?;
    } else {
        anyhow::bail!("No physics model enabled. Set models.flow or models.energy in config.");
    }

    Ok(())
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
) -> Result<()> {
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
    write_results(config, mesh, &state.temperature, None)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Fluid flow solver (SIMPLE)
// ---------------------------------------------------------------------------

fn run_fluid_flow(
    config: &SimulationConfig,
    mesh: &gfd_core::mesh::unstructured::UnstructuredMesh,
    _has_energy: bool,
    gpu: bool,
) -> Result<()> {
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

    // Iteration loop
    let max_iter = config.run.max_iterations;
    let tolerance = config.run.tolerance;

    tracing::info!(max_iter, tolerance, "Starting SIMPLE iterations");

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

        if iter % 10 == 0 || residual < tolerance {
            tracing::info!(iter, residual, "SIMPLE iteration");
        }

        if residual < tolerance {
            tracing::info!(iter, residual, "Converged!");
            break;
        }
    }

    // Write output
    write_results(config, mesh, &state.pressure, Some(&state.velocity))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn write_results(
    config: &SimulationConfig,
    mesh: &gfd_core::mesh::unstructured::UnstructuredMesh,
    scalar: &ScalarField,
    vector: Option<&VectorField>,
) -> Result<()> {
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
    Ok(())
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
