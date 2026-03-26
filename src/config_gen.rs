//! Configuration generation helpers for programmatic use by AI agents.
//!
//! These functions produce [`SimulationConfig`] objects for common simulation
//! setups, ready to be serialized to JSON and fed to the GFD solver.

use std::collections::HashMap;

use gfd_io::json_input::{
    BoundaryConditionConfig, GeneralConfig, InitialConditionsConfig, LinearSolverConfig,
    MaterialConfig, ModelsConfig, RelaxationConfig, ResultsConfig, RunConfig, SetupConfig,
    SimulationConfig, SolverConfig,
};

/// Generate a lid-driven cavity configuration.
///
/// The cavity is a square domain of unit size, with the top wall moving at
/// a velocity that yields the desired Reynolds number: `Re = rho * U * L / mu`.
/// Here `L = 1`, `rho = 1`, `U = 1`, so `mu = 1/Re`.
pub fn lid_driven_cavity(nx: usize, ny: usize, re: f64) -> SimulationConfig {
    let viscosity = 1.0 / re;
    let density = 1.0;

    SimulationConfig {
        setup: SetupConfig {
            general: GeneralConfig {
                name: format!("{}x{}_lid_driven_cavity", nx, ny),
                dimension: 2,
                mesh_file: String::new(),
                mesh_format: "generated".to_string(),
            },
            models: ModelsConfig {
                flow: "incompressible".to_string(),
                turbulence: "none".to_string(),
                energy: false,
                radiation: "none".to_string(),
                solid: "none".to_string(),
                species: "none".to_string(),
            },
            materials: vec![MaterialConfig {
                name: "fluid".to_string(),
                material_type: "fluid".to_string(),
                properties: HashMap::from([
                    ("density".to_string(), density),
                    ("viscosity".to_string(), viscosity),
                ]),
            }],
            boundary_conditions: vec![
                BoundaryConditionConfig {
                    patch: "ymax".to_string(),
                    bc_type: "wall".to_string(),
                    parameters: HashMap::from([
                        ("vx".to_string(), serde_json::json!(1.0)),
                        ("vy".to_string(), serde_json::json!(0.0)),
                        ("vz".to_string(), serde_json::json!(0.0)),
                    ]),
                },
                wall_bc("ymin"),
                wall_bc("xmin"),
                wall_bc("xmax"),
            ],
            initial_conditions: Some(InitialConditionsConfig {
                velocity: Some([0.0, 0.0, 0.0]),
                pressure: Some(0.0),
                ..Default::default()
            }),
        },
        solver: SolverConfig {
            pv_coupling: "SIMPLE".to_string(),
            convection_scheme: "upwind".to_string(),
            relaxation: RelaxationConfig {
                velocity: 0.5,
                pressure: 0.3,
                turbulence: 0.7,
                energy: 0.9,
            },
            linear_solver: LinearSolverConfig {
                solver_type: "bicgstab".to_string(),
                max_iterations: 1000,
                tolerance: 1e-6,
                preconditioner: "jacobi".to_string(),
            },
        },
        run: RunConfig {
            simulation_type: "steady".to_string(),
            max_iterations: 500,
            tolerance: 1e-4,
            ..Default::default()
        },
        results: ResultsConfig {
            output_dir: "results".to_string(),
            format: "vtk".to_string(),
            write_interval: 50,
            fields: vec!["velocity".to_string(), "pressure".to_string()],
        },
    }
}

/// Generate a pipe flow (channel) configuration.
///
/// A rectangular domain with inlet on the left (xmin), outlet on the right (xmax),
/// and walls on top/bottom. Inlet velocity is set so that `Re = rho * U * H / mu`,
/// where `H = 1` is the channel height.
pub fn pipe_flow(nx: usize, ny: usize, re: f64) -> SimulationConfig {
    let density = 1.0;
    let inlet_velocity = 1.0;
    let viscosity = density * inlet_velocity * 1.0 / re; // Re = rho*U*H/mu

    SimulationConfig {
        setup: SetupConfig {
            general: GeneralConfig {
                name: format!("{}x{}_pipe_flow", nx, ny),
                dimension: 2,
                mesh_file: String::new(),
                mesh_format: "generated".to_string(),
            },
            models: ModelsConfig {
                flow: "incompressible".to_string(),
                turbulence: "none".to_string(),
                energy: false,
                radiation: "none".to_string(),
                solid: "none".to_string(),
                species: "none".to_string(),
            },
            materials: vec![MaterialConfig {
                name: "fluid".to_string(),
                material_type: "fluid".to_string(),
                properties: HashMap::from([
                    ("density".to_string(), density),
                    ("viscosity".to_string(), viscosity),
                ]),
            }],
            boundary_conditions: vec![
                BoundaryConditionConfig {
                    patch: "xmin".to_string(),
                    bc_type: "velocity_inlet".to_string(),
                    parameters: HashMap::from([
                        ("vx".to_string(), serde_json::json!(inlet_velocity)),
                        ("vy".to_string(), serde_json::json!(0.0)),
                        ("vz".to_string(), serde_json::json!(0.0)),
                    ]),
                },
                BoundaryConditionConfig {
                    patch: "xmax".to_string(),
                    bc_type: "pressure_outlet".to_string(),
                    parameters: HashMap::from([
                        ("pressure".to_string(), serde_json::json!(0.0)),
                    ]),
                },
                wall_bc("ymin"),
                wall_bc("ymax"),
            ],
            initial_conditions: Some(InitialConditionsConfig {
                velocity: Some([inlet_velocity, 0.0, 0.0]),
                pressure: Some(0.0),
                ..Default::default()
            }),
        },
        solver: SolverConfig {
            pv_coupling: "SIMPLE".to_string(),
            convection_scheme: "upwind".to_string(),
            relaxation: RelaxationConfig {
                velocity: 0.5,
                pressure: 0.3,
                turbulence: 0.7,
                energy: 0.9,
            },
            linear_solver: LinearSolverConfig {
                solver_type: "bicgstab".to_string(),
                max_iterations: 1000,
                tolerance: 1e-6,
                preconditioner: "jacobi".to_string(),
            },
        },
        run: RunConfig {
            simulation_type: "steady".to_string(),
            max_iterations: 500,
            tolerance: 1e-4,
            ..Default::default()
        },
        results: ResultsConfig {
            output_dir: "results".to_string(),
            format: "vtk".to_string(),
            write_interval: 50,
            fields: vec!["velocity".to_string(), "pressure".to_string()],
        },
    }
}

/// Generate a heat conduction configuration.
///
/// A 1D-like slab with fixed temperatures on the left and right boundaries.
pub fn heat_conduction(
    nx: usize,
    ny: usize,
    k: f64,
    t_left: f64,
    t_right: f64,
) -> SimulationConfig {
    SimulationConfig {
        setup: SetupConfig {
            general: GeneralConfig {
                name: format!("{}x{}_heat_conduction", nx, ny),
                dimension: 2,
                mesh_file: String::new(),
                mesh_format: "generated".to_string(),
            },
            models: ModelsConfig {
                flow: "none".to_string(),
                turbulence: "none".to_string(),
                energy: true,
                radiation: "none".to_string(),
                solid: "none".to_string(),
                species: "none".to_string(),
            },
            materials: vec![MaterialConfig {
                name: "solid_material".to_string(),
                material_type: "solid".to_string(),
                properties: HashMap::from([
                    ("conductivity".to_string(), k),
                    ("heat_source".to_string(), 0.0),
                ]),
            }],
            boundary_conditions: vec![
                BoundaryConditionConfig {
                    patch: "xmin".to_string(),
                    bc_type: "fixed_temperature".to_string(),
                    parameters: HashMap::from([
                        ("temperature".to_string(), serde_json::json!(t_left)),
                    ]),
                },
                BoundaryConditionConfig {
                    patch: "xmax".to_string(),
                    bc_type: "fixed_temperature".to_string(),
                    parameters: HashMap::from([
                        ("temperature".to_string(), serde_json::json!(t_right)),
                    ]),
                },
            ],
            initial_conditions: Some(InitialConditionsConfig {
                temperature: Some((t_left + t_right) / 2.0),
                ..Default::default()
            }),
        },
        solver: SolverConfig {
            pv_coupling: "SIMPLE".to_string(),
            convection_scheme: "upwind".to_string(),
            relaxation: RelaxationConfig::default(),
            linear_solver: LinearSolverConfig {
                solver_type: "cg".to_string(),
                max_iterations: 5000,
                tolerance: 1e-10,
                preconditioner: "jacobi".to_string(),
            },
        },
        run: RunConfig {
            simulation_type: "steady".to_string(),
            max_iterations: 1,
            tolerance: 1e-6,
            ..Default::default()
        },
        results: ResultsConfig {
            output_dir: "results".to_string(),
            format: "vtk".to_string(),
            write_interval: 1,
            fields: vec!["temperature".to_string()],
        },
    }
}

/// Helper: create a no-slip wall BC with no parameters.
fn wall_bc(patch: &str) -> BoundaryConditionConfig {
    BoundaryConditionConfig {
        patch: patch.to_string(),
        bc_type: "wall".to_string(),
        parameters: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lid_driven_cavity_config() {
        let config = lid_driven_cavity(20, 20, 100.0);
        assert_eq!(config.setup.general.name, "20x20_lid_driven_cavity");
        assert_eq!(config.setup.models.flow, "incompressible");
        let mu = config.setup.materials[0].properties["viscosity"];
        assert!((mu - 0.01).abs() < 1e-12);
        // Should serialize to JSON without errors
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("lid_driven_cavity"));
    }

    #[test]
    fn test_pipe_flow_config() {
        let config = pipe_flow(30, 10, 50.0);
        assert_eq!(config.setup.general.name, "30x10_pipe_flow");
        assert_eq!(config.setup.boundary_conditions[0].bc_type, "velocity_inlet");
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("pipe_flow"));
    }

    #[test]
    fn test_heat_conduction_config() {
        let config = heat_conduction(20, 1, 1.0, 100.0, 200.0);
        assert_eq!(config.setup.general.name, "20x1_heat_conduction");
        assert!(config.setup.models.energy);
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("heat_conduction"));
    }
}
