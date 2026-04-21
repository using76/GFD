# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GFD (Generalized Fluid Dynamics) — Rust 멀티피직스 솔버 + 순수 Rust CAD 커널 + Electron GUI 워크벤치.

- **Solver**: CFD(비압축/압축), 열전달(전도/대류/복사), 고체역학(선형 탄성 FEM)을 단일 바이너리로 해석
- **CAD kernel** (iter 1–10, FreeCAD-style 재구현): B-Rep 토폴로지, 2D sketcher, primitive/Pad/Revolve features, STL/BRep-JSON I/O, 해석적 measurements, shape healing
- **GUI**: Electron + React + Three.js, `useAppStore` 단일 Zustand 스토어 + `useCadStore` CAD 슬라이스

Workspace: **29 Rust 크레이트** (19 solver + 10 gfd-cad-*), **911 tests passing** (805 legacy + 106 CAD).
CAD 커널은 iter 1–67의 Ralph loop 작업으로 대부분 기본 기능이 작동하는 상태. `docs/CAD_KERNEL_PLAN.md`의 Completion Matrix 참조.

## Build & Test

```bash
# === Rust Solver ===
cargo build --release                                     # 빌드
cargo test --workspace                                    # 전체 테스트 (911개)
cargo test -p gfd-fluid                                   # 단일 크레이트
cargo test -p gfd-thermal steady_1d                       # 특정 테스트
cargo test -p gfd-cad                                     # CAD 통합 테스트 (full_pipeline_smoke)
cargo run --release --bin gfd -- run examples/lid_driven_cavity.json
cargo run --release --bin gfd-benchmark                   # 벤치마크 (READ-ONLY)
cargo build --release --features gpu                      # GPU 빌드 (CUDA 필요)

# === CAD JSON-RPC 서버 ===
cargo run --release --bin gfd-server                      # stdin/stdout JSON-RPC
# 예시: echo '{"id":1,"method":"cad.feature.primitive","params":{"kind":"box"}}' | cargo run --bin gfd-server

# === GUI (Electron + React) ===
cd gui
npm install                                               # 의존성 설치
npm run dev                                               # Vite dev server (http://localhost:5173)
npm run build                                             # 프로덕션 빌드
npx tsc --noEmit                                          # TypeScript 타입 체크 (반드시 0 errors)
npm run electron                                          # Electron 데스크톱 앱
```

세 개의 바이너리가 있으므로 `--bin gfd`, `--bin gfd-benchmark`, `--bin gfd-server`를 명시해야 함.

## Architecture — Rust Crate Dependency Graph

### Solver 계층 (Layer 0–5)

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

### CAD kernel 계층 (독립 스택)

```
gfd-cad-geom ───┬──► gfd-cad-topo ──┬──► gfd-cad-bool
                │                    ├──► gfd-cad-heal
                │                    ├──► gfd-cad-tessel
                │                    └──► gfd-cad-measure
                └──► gfd-cad-sketch ────►
                                    gfd-cad-feature ◄── (topo + sketch + geom)
                                    gfd-cad-io        ◄── (topo + geom)
                                         │
                                         ▼
                                      gfd-cad (facade, Document, rpc types)
                                         │
                                         ▼
                                 src/server.rs handlers
```

## Architecture — GUI

```
gui/
├── electron/main.js              # Electron main process
├── src/
│   ├── App.tsx                   # Main layout (AppMenu, QuickAccess, Ribbon, Tabs)
│   ├── store/
│   │   ├── useAppStore.ts        # Zustand global store (~2000 lines, 레거시 + solver + mesh)
│   │   └── cadStore.ts           # Zustand slice for gfd-cad 결과 (tessellated shapes)
│   ├── components/
│   │   ├── Ribbon.tsx            # SpaceClaim-style 9-tab ribbon
│   │   ├── RibbonCadV2.tsx       # Design/Display/Measure/Repair placeholder ribbons
│   │   ├── LeftPanelStack.tsx    # 탭별 패널 라우팅 (design_v2, display_v2, measure_v2, repair_v2 분기)
│   │   └── ...
│   ├── engine/
│   │   ├── CadScene.tsx          # 레거시 CAD 프리미티브 렌더링
│   │   ├── CadKernelLayer.tsx    # gfd-cad tessellated shape를 Three.js BufferGeometry로 마운트
│   │   ├── MeshRenderer.tsx      # Mesh + field contour 렌더링
│   │   └── Viewport3D.tsx        # Canvas + camera + overlays
│   ├── tabs/
│   │   ├── cad/                  # (레거시) 기존 mesh-based CAD 컴포넌트
│   │   ├── design_v2/            # FeatureTree + primitive/Pad/Revolve 버튼
│   │   ├── display_v2/           # Color picker, visibility
│   │   ├── measure_v2/           # Bulk area/volume/validity
│   │   ├── repair_v2/            # heal.check_validity 이슈 그리드
│   │   ├── mesh/, setup/, calc/, results/
│   ├── ipc/
│   │   ├── gfdClient.ts          # 레거시 솔버 IPC stub (browser simulation)
│   │   └── cadClient.ts          # gfd-cad JSON-RPC client (+ browser sim fallback)
```

**Key state flow (legacy)**: Ribbon button → `useAppStore` action → CadScene/MeshRenderer re-render

**Key state flow (CAD)**: DesignTabV2 button → `cadClient.primitive()` → `cadClient.tessellate()` → `useCadStore.addShape()` → `CadKernelLayer` 렌더

## Key Crates (Rust)

### Solver 크레이트

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

### CAD 커널 크레이트 (Pure Rust, OCCT 의존 없음)

| Crate | Role |
|-------|------|
| **gfd-cad-geom** | `Point3`/`Vector3`/`BoundingBox`, Line/Circle/BSplineCurve, Plane/Cylinder/Sphere/Cone/Torus, `Curve`/`Surface` traits |
| **gfd-cad-topo** | `Shape` enum, `ShapeArena`, `HalfEdge`, `build_half_edges`, `EdgeFaceMap::{face_neighbors, is_manifold_vertex}` |
| **gfd-cad-sketch** | 2D sketcher: Point/Line/Circle/Arc, **17 constraints**, damped Gauss-Newton + Levenberg solver, DOF analysis |
| **gfd-cad-feature** | 18+ feature kinds: box/sphere/cyl/cone/torus, pad, pocket, revolve (+ partial), chamfer (corner/top), fillet (corner/top/cyl), wedge, pyramid, ngon_prism, mirror, translate/scale/rotate, linear/circular/rectangular arrays |
| **gfd-cad-bool** | `compound_merge`, `bbox_overlaps`, **mesh CSG** (Möller-Trumbore) — union/diff/intersect on tessellated shapes. Real B-Rep CSG 미구현 |
| **gfd-cad-io** | STL ASCII/binary read+write, BRep-JSON roundtrip, STEP AP214 writer (10 entity kinds), STEP points-only reader |
| **gfd-cad-heal** | `check_validity`, `fix_shape` (sew_vertices + dedup_edges + close_open_wires + remove_small_edges), `shape_stats` |
| **gfd-cad-measure** | 17+ measures: distance (v-v, v-e, e-e, f-f), polygon_area, bbox, surface_area (Newell + analytic), divergence_volume, edge_length + range, angle, center_of_mass, inertia (diag + full 3×3), principal_axes, bounding_sphere, closest_point (boundary + face interior), is_point_inside, signed_distance |
| **gfd-cad-tessel** | `TriMesh`, `uv_grid`, ear-clipping, sphere-pole collapse, `auto_uv_steps` + `tessellate_adaptive` (chord-tolerance based) |
| **gfd-cad** | Facade: `Document { arena, features, sketches }`, RPC types, integration tests `full_pipeline_smoke` / `extended_pipeline_smoke` / `box_divergence_volume_is_one` |

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

### 7. Design/Display/Measure/Repair 탭은 v2 재구축 중

기존 mesh-based Design/Display/Measure/Repair 탭은 2026-04-20에 제거되고 **gfd-cad 커널 위에서 재작성되는 중**. 플로우:

```
DesignTabV2 button → cadClient.primitive/pad/revolve → gfd-server (Rust)
  → ShapeArena mutate → tessellate 반환 → useCadStore.addShape
  → CadKernelLayer가 Three.js로 렌더
```

- 구 레거시는 `gui/src/tabs/cad/`에 남아 있지만 더 이상 라우팅되지 않음 (LeftPanelStack에서 v2만 분기)
- `gui/src/components/RibbonCadV2.tsx` — 4개 탭의 리본은 "CAD kernel 재구축 중" placeholder

### 8. CAD 커널 현재 상태 (iter 67 기준)

`docs/CAD_KERNEL_PLAN.md`의 Completion Matrix 참조.
Phase 1/2/4/6/7/8/9/10/11/12 ✅, Phase 3 ⚠️ (mesh CSG만, B-Rep CSG 미구현), Phase 5 ⚠️ (구체적 primitive/transform/corner-fillet만 구현 — 임의 B-Rep edge에 대한 일반 fillet은 미구현).
- Boolean CSG: 메쉬 레벨은 작동 (`mesh_boolean` union/diff/intersection)
- Pocket/Chamfer/Fillet: 박스 코너 + 상단 edge + 실린더 한정
- STEP: writer는 10가지 entity kind 방출, reader는 points-only
- Sketcher: 17 constraints + SketcherCanvas UI (Point/Line/Arc + H/V/Fix tool) + Extrude/Revolve 버튼
- Repair: `fix_shape`가 실제로 arena 변경 (sew/dedup/close/remove small)

CAD 기능을 확장할 때 이 문서를 먼저 읽고 어떤 phase가 실제로 동작하는지 확인할 것.

## GUI Code Quality Rules

- **`as any` in .tsx files**: 0 allowed. Use proper types or `as never` for Ant Design Select.
- **`Math.random()` in UI code** (components/tabs/engine): 0 allowed. Solver simulation uses seeded deterministic noise.
- **`console.log` in production**: 0 allowed.
- TypeScript strict mode: `npx tsc --noEmit` must pass with **0 errors** before committing.

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
| **CAD 커널 로드맵** | `docs/CAD_KERNEL_PLAN.md` | **12-phase plan, 10 iteration 기록, 각 phase 완성도** |
| Workflow audit | `WORKFLOW_AUDIT.md` | 84 buttons x 17 workflows, gap analysis |
| Technical docs | `docs/TECHNICAL.md` | 19 solver 크레이트, GUI architecture, API, benchmarks |
| **구현 목록** | `docs/IMPLEMENTED_FEATURES.md` | **29 크레이트별 구현 기능 & 소스 파일, 테스트 커버리지** |
| CLI adaptation | `docs/CLI_ADAPTATION_PLAN.md` | Claude Code CLI patterns → GFD CLI |
| UI features | `UI_FEATURES.md` | 168-feature implementation checklist |
| Autoresearch rules | `program.md` | Agent behavior rules |
| Experiment log | `results.tsv` | Optimization experiments (commit, metric, status) |

## Language

사용자와는 한국어로 소통. 기술 용어와 코드 식별자는 영어 유지.
