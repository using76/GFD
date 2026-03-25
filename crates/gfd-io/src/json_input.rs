//! JSON simulation configuration loading.

use serde::Deserialize;
use crate::Result;

/// Top-level simulation configuration matching the GFD JSON schema.
#[derive(Debug, Clone, Deserialize)]
pub struct SimulationConfig {
    /// General setup configuration.
    pub setup: SetupConfig,
    /// Solver configuration.
    pub solver: SolverConfig,
    /// Run control configuration.
    pub run: RunConfig,
    /// Results output configuration.
    pub results: ResultsConfig,
}

/// General setup: mesh, physics, materials, etc.
#[derive(Debug, Clone, Deserialize)]
pub struct SetupConfig {
    /// General simulation settings.
    #[serde(default)]
    pub general: GeneralConfig,
    /// Physics models to activate.
    #[serde(default)]
    pub models: ModelsConfig,
    /// Material definitions.
    #[serde(default)]
    pub materials: Vec<MaterialConfig>,
    /// Boundary conditions.
    #[serde(default)]
    pub boundary_conditions: Vec<BoundaryConditionConfig>,
    /// Initial conditions.
    #[serde(default)]
    pub initial_conditions: Option<InitialConditionsConfig>,
}

/// General simulation settings.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GeneralConfig {
    /// Simulation name.
    #[serde(default)]
    pub name: String,
    /// Spatial dimension (2 or 3).
    #[serde(default = "default_dimension")]
    pub dimension: usize,
    /// Path to the mesh file.
    #[serde(default)]
    pub mesh_file: String,
    /// Mesh format: "gmsh", "stl", etc.
    #[serde(default)]
    pub mesh_format: String,
}

fn default_dimension() -> usize {
    3
}

/// Physics models configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ModelsConfig {
    /// Flow model: "incompressible", "compressible", or "none".
    #[serde(default)]
    pub flow: String,
    /// Turbulence model name (e.g., "k-epsilon", "k-omega-sst", "none").
    #[serde(default)]
    pub turbulence: String,
    /// Whether to solve the energy equation.
    #[serde(default)]
    pub energy: bool,
    /// Radiation model: "none", "p1", "dom".
    #[serde(default)]
    pub radiation: String,
    /// Solid mechanics model: "none", "linear_elastic", "hyperelastic".
    #[serde(default)]
    pub solid: String,
    /// Species transport model: "none", "species_transport".
    #[serde(default)]
    pub species: String,
}

/// Material definition.
#[derive(Debug, Clone, Deserialize)]
pub struct MaterialConfig {
    /// Material name.
    pub name: String,
    /// Material type: "fluid" or "solid".
    #[serde(default)]
    pub material_type: String,
    /// Material properties as key-value pairs.
    #[serde(default)]
    pub properties: std::collections::HashMap<String, f64>,
}

/// Boundary condition definition.
#[derive(Debug, Clone, Deserialize)]
pub struct BoundaryConditionConfig {
    /// Name of the boundary patch.
    pub patch: String,
    /// Boundary condition type: "wall", "inlet", "outlet", "symmetry", etc.
    #[serde(rename = "type")]
    pub bc_type: String,
    /// Additional parameters for this boundary condition.
    #[serde(default)]
    pub parameters: std::collections::HashMap<String, serde_json::Value>,
}

/// Initial conditions.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct InitialConditionsConfig {
    /// Uniform initial velocity [m/s].
    #[serde(default)]
    pub velocity: Option<[f64; 3]>,
    /// Uniform initial pressure [Pa].
    #[serde(default)]
    pub pressure: Option<f64>,
    /// Uniform initial temperature [K].
    #[serde(default)]
    pub temperature: Option<f64>,
    /// Uniform initial turbulent kinetic energy [m^2/s^2].
    #[serde(default)]
    pub turb_kinetic_energy: Option<f64>,
    /// Uniform initial turbulence dissipation rate [m^2/s^3].
    #[serde(default)]
    pub turb_dissipation: Option<f64>,
}

/// Solver configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SolverConfig {
    /// Pressure-velocity coupling: "SIMPLE", "PISO", "SIMPLEC".
    #[serde(default = "default_pv_coupling")]
    pub pv_coupling: String,
    /// Convection scheme: "upwind", "linear", "vanLeer", etc.
    #[serde(default = "default_convection_scheme")]
    pub convection_scheme: String,
    /// Under-relaxation factors.
    #[serde(default)]
    pub relaxation: RelaxationConfig,
    /// Linear solver settings.
    #[serde(default)]
    pub linear_solver: LinearSolverConfig,
}

fn default_pv_coupling() -> String {
    "SIMPLE".to_string()
}

fn default_convection_scheme() -> String {
    "upwind".to_string()
}

/// Under-relaxation factors for each equation.
#[derive(Debug, Clone, Deserialize)]
pub struct RelaxationConfig {
    /// Velocity under-relaxation.
    #[serde(default = "default_relax_velocity")]
    pub velocity: f64,
    /// Pressure under-relaxation.
    #[serde(default = "default_relax_pressure")]
    pub pressure: f64,
    /// Turbulence under-relaxation.
    #[serde(default = "default_relax_turbulence")]
    pub turbulence: f64,
    /// Energy under-relaxation.
    #[serde(default = "default_relax_energy")]
    pub energy: f64,
}

fn default_relax_velocity() -> f64 {
    0.7
}
fn default_relax_pressure() -> f64 {
    0.3
}
fn default_relax_turbulence() -> f64 {
    0.7
}
fn default_relax_energy() -> f64 {
    0.9
}

impl Default for RelaxationConfig {
    fn default() -> Self {
        Self {
            velocity: default_relax_velocity(),
            pressure: default_relax_pressure(),
            turbulence: default_relax_turbulence(),
            energy: default_relax_energy(),
        }
    }
}

/// Linear solver settings.
#[derive(Debug, Clone, Deserialize)]
pub struct LinearSolverConfig {
    /// Solver type: "cg", "bicgstab", "gmres".
    #[serde(default = "default_solver_type")]
    pub solver_type: String,
    /// Maximum iterations.
    #[serde(default = "default_max_iter")]
    pub max_iterations: usize,
    /// Convergence tolerance.
    #[serde(default = "default_linear_tolerance")]
    pub tolerance: f64,
    /// Preconditioner: "none", "jacobi", "ilu".
    #[serde(default = "default_preconditioner")]
    pub preconditioner: String,
}

fn default_solver_type() -> String {
    "bicgstab".to_string()
}
fn default_max_iter() -> usize {
    1000
}
fn default_linear_tolerance() -> f64 {
    1e-8
}
fn default_preconditioner() -> String {
    "ilu".to_string()
}

impl Default for LinearSolverConfig {
    fn default() -> Self {
        Self {
            solver_type: default_solver_type(),
            max_iterations: default_max_iter(),
            tolerance: default_linear_tolerance(),
            preconditioner: default_preconditioner(),
        }
    }
}

/// Run control configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct RunConfig {
    /// Simulation type: "steady" or "transient".
    #[serde(default = "default_sim_type")]
    pub simulation_type: String,
    /// Time step size [s] (for transient).
    #[serde(default)]
    pub time_step: Option<f64>,
    /// End time [s] (for transient).
    #[serde(default)]
    pub end_time: Option<f64>,
    /// Maximum number of outer iterations per time step (or total for steady).
    #[serde(default = "default_max_outer")]
    pub max_iterations: usize,
    /// Convergence tolerance for outer iterations.
    #[serde(default = "default_outer_tolerance")]
    pub tolerance: f64,
}

fn default_sim_type() -> String {
    "steady".to_string()
}
fn default_max_outer() -> usize {
    1000
}
fn default_outer_tolerance() -> f64 {
    1e-6
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            simulation_type: default_sim_type(),
            time_step: None,
            end_time: None,
            max_iterations: default_max_outer(),
            tolerance: default_outer_tolerance(),
        }
    }
}

/// Results output configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ResultsConfig {
    /// Output directory.
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
    /// Output format: "vtk", "csv", etc.
    #[serde(default = "default_output_format")]
    pub format: String,
    /// Write interval (every N iterations or time steps).
    #[serde(default = "default_write_interval")]
    pub write_interval: usize,
    /// Fields to write.
    #[serde(default)]
    pub fields: Vec<String>,
}

fn default_output_dir() -> String {
    "results".to_string()
}
fn default_output_format() -> String {
    "vtk".to_string()
}
fn default_write_interval() -> usize {
    100
}

impl Default for ResultsConfig {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
            format: default_output_format(),
            write_interval: default_write_interval(),
            fields: Vec::new(),
        }
    }
}

/// Loads a simulation configuration from a JSON file.
pub fn load_config(path: &str) -> Result<SimulationConfig> {
    let contents = std::fs::read_to_string(path).map_err(|e| {
        crate::IoError::FileNotFound(format!("{}: {}", path, e))
    })?;
    let config: SimulationConfig =
        serde_json::from_str(&contents).map_err(crate::IoError::JsonError)?;
    Ok(config)
}
