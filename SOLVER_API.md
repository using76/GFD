# GFD Solver API — AI Tool Interface

GFD provides a universal multi-physics solver that AI agents can use as tools.

## Quick Start

```bash
# Run a simulation from JSON config
gfd run examples/pipe_flow.json --json-output

# Output: structured JSON with results
{
  "status": "converged",
  "iterations": 679,
  "residual": 3.0e-4,
  "wall_time_ms": 45,
  "fields": {
    "pressure": {"min": -1.2, "max": 7.2, "mean": 3.1},
    "velocity_magnitude": {"min": 0.0, "max": 1.3, "mean": 0.65}
  }
}
```

## Available Solvers

### Fluid Dynamics
| Solver | Type | Use Case |
|--------|------|----------|
| `SIMPLE` | Steady incompressible | Cavity, pipe, external flow |
| `PISO` | Transient incompressible | Vortex shedding, pulsatile |
| `SIMPLEC` | Faster SIMPLE variant | Same as SIMPLE, fewer iterations |
| `Roe/HLLC/AUSM+` | Compressible | Shock tubes, nozzles, supersonic |

### Turbulence Models
| Model | Equations | Best For |
|-------|-----------|----------|
| `k-epsilon` | 2-eq RANS | General industrial |
| `k-omega-sst` | 2-eq RANS | Adverse pressure gradient |
| `spalart-allmaras` | 1-eq RANS | Aerospace external |
| `realizable-ke` | 2-eq RANS | Swirling, separated flows |
| `smagorinsky` | LES | Resolved turbulence |
| `wale` | LES | Near-wall LES |

### Multiphase
| Model | Type | Use Case |
|-------|------|----------|
| `vof` | Volume of Fluid | Free surface, sloshing |
| `level-set` | Level Set | Interface tracking |
| `euler-euler` | Two-fluid | Bubbly, fluidized bed |
| `mixture` | Mixture model | Slurry, settling |
| `dpm` | Lagrangian | Spray, particle transport |

### Thermal
| Model | Type | Use Case |
|-------|------|----------|
| `conduction` | Steady/transient | Solid heat transfer |
| `convection-diffusion` | Coupled flow+T | Heated channels |
| `radiation-p1` | P-1 approximation | Participating media |
| `radiation-dom` | Discrete Ordinates | Directional radiation |
| `phase-change` | Enthalpy-porosity | Melting/solidification |
| `conjugate` | CHT | Electronics cooling |

### Structural
| Model | Type | Use Case |
|-------|------|----------|
| `linear-elastic` | FEM Hex8 | Static stress |
| `elastoplastic` | Von Mises | Metal forming |
| `dynamics` | Newmark-beta | Vibration, impact |
| `contact` | Penalty method | Assembly mechanics |

## JSON Configuration Format

```json
{
  "setup": {
    "general": {"name": "20x20_cavity", "dimension": 2},
    "models": {
      "flow": "incompressible",
      "turbulence": "none",
      "energy": false,
      "solid": "none"
    },
    "materials": [{
      "name": "fluid",
      "material_type": "fluid",
      "properties": {"density": 1.0, "viscosity": 0.01}
    }],
    "boundary_conditions": [
      {"patch": "ymax", "type": "wall", "parameters": {"vx": 1.0}},
      {"patch": "ymin", "type": "wall", "parameters": {}},
      {"patch": "xmin", "type": "wall", "parameters": {}},
      {"patch": "xmax", "type": "wall", "parameters": {}}
    ]
  },
  "solver": {
    "pv_coupling": "SIMPLE",
    "relaxation": {"velocity": 0.5, "pressure": 0.3}
  },
  "run": {"max_iterations": 500, "tolerance": 1e-4},
  "probes": [
    {"name": "center", "x": 0.5, "y": 0.5}
  ]
}
```

## Programmatic API (Rust)

```rust
use gfd::api::*;

// Solve lid-driven cavity
let result = solve_cavity(20, 20, 100.0, 500);
println!("Status: {}", result.status);
println!("Iterations: {}", result.iterations);

// Solve from JSON string
let config = r#"{"setup": {...}, "solver": {...}, "run": {...}}"#;
let result = solve_from_json(config)?;

// Get solver capabilities
let caps = capabilities();
```

## Boundary Condition Types

| Type | Parameters | Description |
|------|-----------|-------------|
| `wall` | `vx, vy, vz` (optional) | No-slip (or moving) wall |
| `velocity_inlet` | `vx, vy, vz` | Fixed velocity inlet |
| `pressure_outlet` | `pressure` | Fixed pressure outlet |
| `fixed_temperature` | `temperature` | Dirichlet thermal BC |
| `fixed` | (none) | Clamped (solid) |
| `force` | `fx, fy, fz` | Applied traction (solid) |

## Mesh Generation

Meshes are auto-generated from the `name` field pattern:
- `"20x20_cavity"` → 20x20 structured 2D mesh
- `"10x3x3_beam"` → 10x3x3 structured 3D mesh
- Domain size: 1.0 x 1.0 (x 1.0 for 3D)

Boundary patches: `xmin`, `xmax`, `ymin`, `ymax`, `zmin`, `zmax`
