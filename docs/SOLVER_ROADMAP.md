# GFD Solver — AI-control improvements & open-source feature gap analysis

Researched against OpenFOAM, SU2, PyFR, Nek5000, Palabos, preCICE, DAFoam,
LBM-DEM frameworks, and agentic-CFD work (PhyNiKCE). Sources at the bottom.

## What GFD already has (O)
| Area | Capability | Status |
|---|---|:--:|
| Incompressible | SIMPLE / PISO / SIMPLEC | O |
| Compressible | Roe / HLLC / AUSM+ | O |
| Turbulence | k-ε, k-ω SST, Spalart–Allmaras, LES (Smagorinsky/WALE) | O |
| Multiphase | VOF, Level-Set, Euler–Euler, mixture, DPM | O |
| Thermal | conduction, convection-diffusion, P-1/DO radiation, phase change, CHT | O |
| Solid | linear-elastic, von-Mises plasticity, Newmark dynamics, contact | O |
| Mesh | Cartesian, Delaunay, O-grid, cut-cell, octree AMR, quality | O |
| Pluggable physics | gfd-expression terms → **affect the real solve** (this PR) | O |
| AI control | MCP tools, residual stream, expression editing | O |
| Output | VTK, **OpenVDB**, **OpenUSD** (Omniverse/Isaac) | O |

## Missing vs leading open-source solvers (X = not implemented)
| Capability | Reference solver | Status | Why it matters for an AI-controlled CAE |
|---|---|:--:|---|
| **Discrete adjoint / gradient sensitivities** | SU2, DAFoam | X | Lets the AI run gradient-based shape optimization in 1 solve/step. Highest-leverage gap. |
| **Runtime solution-adaptive AMR** (coupled to the solver) | OpenFOAM, AMReX | △ | AMR meshing exists; not driven by live error estimates during a solve. |
| **High-order spectral element / flux reconstruction** | Nek5000, PyFR | X | High-accuracy DNS/LES; GPU-friendly DSL (PyFR). |
| **Lattice Boltzmann (LBM)** | Palabos | X | Trivially parallel, great on GPU, easy complex geometry. |
| **DEM + CFD–DEM coupling** | LEDDS (LBM-DEM) | X | Particle-laden flows, fluidized beds, granular. |
| **Partitioned FSI / multi-code coupling** | preCICE | △ | Have CHT coupling; no general partitioned FSI coupling library. |
| **Immersed boundary method** | IB-LBM frameworks | X | Moving/complex bodies without body-fitted remeshing — ideal for AI geometry edits. |
| **Full GPU solver path** (not just CG fallback) | PyFR, LBM-GPU | △ | gfd-gpu has GpuCG; full assembled-system GPU solve is not wired. |
| **Combustion / reacting flow / species sources** | OpenFOAM | △ | Species transport partial; no finite-rate chemistry. |
| **Acoustics (FW-H), MHD / electromagnetics** | OpenFOAM modules | X | Aeroacoustics, plasma/MHD. |
| **Overset / Chimera meshes** | OpenFOAM, SU2 | X | Relative body motion (rotors, valves). |

## AI-control improvements (roadmap)
1. **Expression physics is now live** (this PR): `physics.set_term` with an
   expression body force changes the solved field (verified: cavity 0 → 4.6 mean
   velocity under `fx=100*y`). Next: expression **viscosity/density** in the
   constitutive path, and per-equation expression diffusion/convection terms.
2. **Per-equation residual stream** — backend currently returns one combined
   residual; expose per-equation residuals so the AI can diagnose which equation
   stalls and auto-tune that relaxation factor.
3. **Auto-tuning loop** — on divergence, the agent lowers `relaxVelocity`/
   `relaxPressure` (commands exist) and re-runs; expose a "diverged because…"
   structured reason.
4. **Adjoint sensitivities → AI shape optimization** — biggest win: expose
   `solve.adjoint` returning d(objective)/d(geometry) so the agent closes a
   gradient-based design loop with the geometry commands.
5. **AI-driven adaptive refinement** — return field-gradient/error estimates so
   the agent requests local refinement zones before re-solving.
6. **Parametric sweeps** — an MCP tool to launch batched solves over a parameter
   grid and return a results table for the agent to rank.
7. **Natural-language → config** already scoped in `docs/CLI_ADAPTATION_PLAN.md`;
   now realizable since every setup action is a typed command/MCP tool.
8. **Neurosymbolic/agentic autonomy** (cf. PhyNiKCE) — the command-core +
   journal + consent is exactly the substrate for an autonomous CFD agent with
   auditable, reversible actions.

## Suggested next implementations (priority)
1. Expression **constitutive** (viscosity/density) in the solver — small, high
   value, completes "AI edits any physics → result changes".
2. Per-equation residuals + structured divergence diagnostics (cheap, big AI UX).
3. Discrete adjoint for a scalar objective (drag/pressure-drop) — enables AI
   shape optimization; medium-large.
4. Lattice Boltzmann mini-solver (GPU-friendly, complex geometry) — new physics
   with low coupling to the FVM core.
5. Immersed boundary for moving AI-edited geometry without remeshing.

## Sources
- [Comparing Open Source CFD Platforms — Resolved Analytics](https://www.resolvedanalytics.com/theflux/comparing-cfd-software-part-2-open-source-cfd-software-packages)
- [Code4CFD — curated CFD repositories](https://github.com/thw1021/Code4CFD)
- [awesome-fluid-dynamics](https://github.com/lento234/awesome-fluid-dynamics)
- [list-lattice-Boltzmann-codes](https://github.com/sthavishtha/list-lattice-Boltzmann-codes)
- [PhyNiKCE: Neurosymbolic Agentic Framework for Autonomous CFD (arXiv)](https://arxiv.org/pdf/2602.11666)
- [LEDDS: Portable LBM-DEM on GPUs (arXiv)](https://arxiv.org/pdf/2512.04997)
- [preCICE coupling library](https://precice.org/)
- [DAFoam — discrete adjoint with OpenFOAM](https://dafoam.github.io/)
- [SU2](https://su2code.github.io/) · [PyFR](https://www.pyfr.org/) · [Nek5000](https://nek5000.mcs.anl.gov/) · [Palabos](https://palabos.unige.ch/)
