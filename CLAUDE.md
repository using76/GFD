# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GFD (Generalized Fluid Dynamics) — Rust 멀티피직스 솔버 + Electron GUI 워크벤치. CFD(비압축/압축), 열전달(전도/대류/복사), 고체역학(선형 탄성 FEM)을 단일 바이너리로 해석. 262 Rust 파일, 63,805줄, 19 크레이트, 805 테스트. GUI: ~11,500줄 TypeScript/React/Three.js, 147+ 기능.

## Build & Test

```bash
# === Rust Solver ===
cargo build --release                                     # 빌드
cargo test --workspace                                    # 전체 테스트 (805개)
cargo test -p gfd-fluid                                   # 단일 크레이트
cargo test -p gfd-thermal steady_1d                       # 특정 테스트
cargo run --release --bin gfd -- run examples/lid_driven_cavity.json
cargo run --release --bin gfd-benchmark                   # 벤치마크 (READ-ONLY)
cargo build --release --features gpu                      # GPU 빌드 (CUDA 필요)

# === GUI (Electron + React) ===
cd gui
npm install                                               # 의존성 설치
npm run dev                                               # Vite dev server (http://localhost:5173)
npm run build                                             # 프로덕션 빌드
npx tsc --noEmit                                          # TypeScript 타입 체크
npm run electron                                          # Electron 데스크톱 앱
```

두 바이너리가 있으므로 `--bin gfd` 또는 `--bin gfd-benchmark`을 명시해야 함.

## Architecture — Rust Crate Dependency Graph

```
Layer 0 (leaf):    gfd-core          gfd-expression
                      |                    |
Layer 1:     gfd-matrix  gfd-discretize  gfd-boundary  gfd-source  gfd-material
             gfd-turbulence  gfd-coupling  gfd-gpu  gfd-io  gfd-parallel
             gfd-postprocess  gfd-vdb
                      |
Layer 2:           gfd-linalg (-> gfd-core + gfd-matrix)
                      |
Layer 3 (physics): gfd-fluid  gfd-thermal  gfd-solid
                      |
Layer 4:           gfd-mesh (Cartesian, Delaunay, O-grid, Cut-cell, AMR)
                      |
Layer 5 (binary):  src/main.rs  src/server.rs (JSON-RPC)
```

## Architecture — GUI

```
gui/
├── electron/main.js              # Electron main process
├── src/
│   ├── App.tsx                   # Main layout (AppMenu, QuickAccess, Ribbon, Tabs)
│   ├── store/useAppStore.ts      # Zustand global store (~2000 lines, ALL app state)
│   ├── components/
│   │   ├── Ribbon.tsx            # SpaceClaim-style 9-tab ribbon (86+ buttons)
│   │   ├── ContextMenu3D.tsx     # Right-click menu with shape/viewport actions
│   │   ├── StatusBar.tsx         # Bottom bar (solver progress, mesh stats, tools)
│   │   ├── ToolOptions.tsx       # Per-tool options (Pull/Move/Fill/Measure/Section)
│   │   └── LeftPanelStack.tsx    # Dynamic left panel routing per tab
│   ├── engine/
│   │   ├── CadScene.tsx          # Three.js CAD rendering (~2000 lines, core 3D)
│   │   ├── MeshRenderer.tsx      # Mesh + field contour rendering (4 colormaps)
│   │   └── Viewport3D.tsx        # Canvas + camera + grid + overlays
│   ├── tabs/                     # Tab-specific panels (cad/, mesh/, setup/, calc/, results/)
│   └── ipc/gfdClient.ts          # Rust backend IPC stub (browser simulation mode)
```

**Key state flow**: Ribbon button → `useAppStore` action → CadScene/MeshRenderer re-render

**Store (`useAppStore.ts`)** is the single source of truth for: shapes, mesh, solver, physics models, boundaries, materials, fields, undo/redo, UI state. Every component reads from and writes to this store via Zustand selectors.

## Key Crates (Rust)

| Crate | Role |
|-------|------|
| **gfd-core** | `UnstructuredMesh`, `ScalarField`/`VectorField`, `SparseMatrix` (CSR, unsafe `spmv`), Green-Gauss gradient |
| **gfd-linalg** | **Production solvers**: `CG`, `BiCGSTAB`, `GMRES`, `PCG`, `PBiCGSTAB`, `ILU0`/`Jacobi` preconditioner |
| **gfd-matrix** | `Assembler` (COO→CSR, counting sort), `apply_dirichlet`/`apply_neumann` |
| **gfd-fluid** | SIMPLE/PISO/SIMPLEC, Roe/HLLC/AUSM+, VOF/LevelSet/Euler-Euler, k-e/k-w SST/LES |
| **gfd-thermal** | Steady/transient conduction, convection-diffusion, P-1/DO radiation, phase change |
| **gfd-solid** | Hex8 FEM, Von Mises plasticity, Newmark-beta dynamics |
| **gfd-mesh** | Cartesian, Delaunay 2D/3D, O-grid, Cut-cell, Octree AMR, quality metrics |
| **gfd-gpu** | CUDA via `cudarc`, `GpuCG` (CPU fallback), feature `cuda` |
| **gfd-io** | JSON config, Gmsh v2.2, STL reader, VTK Legacy writer, checkpoints |

## Critical Design Decisions

### 1. Dual LinearSolver Traits (beware confusion)

- `gfd_core::linalg::solvers::LinearSolver` — `&mut LinearSystem` interface. **Basic impl** (in gfd-core).
- `gfd_linalg::traits::LinearSolverTrait` — `(&SparseMatrix, &[f64], &mut [f64])` interface. **Production impl**. Physics solvers must use this one.

### 2. FVM Solver Pattern (all physics solvers follow this)

```rust
// 1. Face coefficient precompute (outside comp loop)
for face in &mesh.faces { /* D, F */ }
// 2. Per-component assemble + solve
for comp in 0..3 {
    let mut assembler = Assembler::with_nnz_estimate(n, n + 2*n_internal);
    assembler.finalize() -> LinearSystem
    BiCGSTAB::solve()
}
```

### 3. Mesh: Structured → Unstructured

`StructuredMesh::uniform(nx, ny, nz, lx, ly, lz).to_unstructured()` — single FVM code path. `nz=0` means 2D (actually nz=1 single-layer 3D hex).

### 4. GPU: Feature-gated

`--features gpu`. Without `cuda` feature, all GPU paths fall back to CPU. `simple.rs::solve_linear_system()` dispatches CPU/GPU.

### 5. GUI Solver is Simulated

The GUI's `startSolver()` in `useAppStore.ts` runs a **simulated** solver (Math.exp decay + seeded noise). It does NOT call the Rust backend. Real solver runs via `cargo run --bin gfd`. The GUI generates physics-aware field data (pressure, velocity, temperature, TKE, VOF, radiation, species, y+) based on boundary conditions and physics model selections.

### 6. GUI Mesh Generation

`generateMesh()` in `useAppStore.ts` creates a real 3D structured hex grid with:
- Point-in-solid test (sphere, cylinder, cone, torus, pipe, STL ray-casting via Moller-Trumbore)
- Boundary layer prism cells (geometric progression)
- Curvature refinement (1.5x near curved bodies)
- Tet/Poly visual variants
- Per-vertex colors by boundary type

## GUI Code Quality Rules

- **`as any` in .tsx files**: 0 allowed. Use proper types or `as never` for Ant Design Select.
- **`Math.random()` in UI code** (components/tabs/engine): 0 allowed. Solver simulation uses seeded deterministic noise.
- **`console.log` in production**: 0 allowed.
- TypeScript strict mode: `npx tsc --noEmit` must pass with 0 errors before committing.

## Autoresearch System

Autonomous solver optimization loop (`program.md`):
1. Modify solver code → `cargo build --release`
2. `cargo test --workspace` (existing tests must pass)
3. `cargo run --release --bin gfd-benchmark` → compare metrics
4. Keep if improved, discard otherwise
5. Record in `results.tsv`

Benchmark (`benches/gfd_benchmark.rs`) is **READ-ONLY**:
- `heat_1d`: 50-cell 1D conduction (analytical, error ~2e-12)
- `heat_source`: 100-cell source term
- `cavity_20/50/100`: lid-driven cavity Re=100

## Key Documents

| Document | Location | Content |
|----------|----------|---------|
| Master plan | `PROJECT_PLAN.md` | 4,734 lines. Solver list, math, architecture, GPU plan |
| Workflow audit | `WORKFLOW_AUDIT.md` | 84 buttons x 17 workflows, gap analysis |
| Technical docs | `docs/TECHNICAL.md` | 19 crates, GUI architecture, API, benchmarks |
| CLI adaptation | `docs/CLI_ADAPTATION_PLAN.md` | Claude Code CLI patterns → GFD CLI |
| UI features | `UI_FEATURES.md` | 168-feature implementation checklist |
| Autoresearch rules | `program.md` | Agent behavior rules |
| Experiment log | `results.tsv` | Optimization experiments (commit, metric, status) |

## Language

사용자와는 한국어로 소통. 기술 용어와 코드 식별자는 영어 유지.
