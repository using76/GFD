# GFD — Generalized Fluid Dynamics

Rust 기반 통합 멀티피직스 솔버 + **순수 Rust CAD 커널** + Electron GUI 워크벤치

> ANSYS Fluent / SpaceClaim / FreeCAD 수준의 워크플로우를 **단일 오픈소스 패키지**로 제공하는 것이 목표입니다. CAD 커널은 OCCT에 의존하지 않는 **pure-Rust** 구현입니다.

---

## 주요 특징

| 영역 | 내용 |
|------|------|
| **Solver** | SIMPLE / PISO / SIMPLEC 압력-속도 커플링, Roe / HLLC / AUSM+ 리만 솔버 |
| **난류** | k-ε, k-ω SST, Spalart-Allmaras, Realizable k-ε, LES |
| **다상** | VOF + CSF, Level Set, Euler-Euler, Mixture, DPM |
| **열전달** | 전도 / 대류 / 복사 (P-1, DO), 상변화, 공액열전달 |
| **고체역학** | Hex8 FEM, Von Mises 소성, Newmark-β 동역학 |
| **메시** | Cartesian Hex, Tet, Poly, Delaunay 2D/3D, O-grid, Prism layer, Octree AMR, Cut-cell |
| **CAD 커널** | B-Rep 토폴로지, 19 primitive, 13 profile, 17-constraint 2D sketcher, mesh CSG, shape healing |
| **GUI** | Electron + React + Three.js, SpaceClaim 스타일 9-tab 리본 UI |
| **GPU** | CUDA 가속 (`cudarc`), AmgX 지원 (feature flag) |
| **I/O** | STL ASCII/binary, STEP AP214 writer, BRep-JSON, OBJ, OFF, PLY, WRL, XYZ, VTK, DXF, Gmsh |

## 프로젝트 규모

| 항목 | 수치 |
|------|------|
| Rust crate | **29** (19 solver + 10 gfd-cad-*) |
| Rust 테스트 | **911 passed** (805 legacy + 106 CAD) |
| TypeScript 에러 | **0** (strict mode) |
| CAD JSON-RPC 메서드 | **125+** |
| CAD primitive | 19종 (box/sphere/cyl/cone/torus/pyramid/wedge/N-gon prism/stairs/helix/honeycomb/platonic 4종 등) |
| 2D profile | 13종 (rectangle, rounded-rect, slot, ellipse, n-gon, star, gear, airfoil NACA4, I/L/C/T/Z beam) |
| Revolve profile | 5종 (ring, cup, frustum, torus, capsule) |
| Sketcher constraint | 17종 (H/V/Fix/Coincident/Distance/Angle/Parallel/Perpendicular 등) |
| Measure helper | **60+** |
| Export 포맷 | 13종 (disk) + 9종 (in-memory) |
| Import 포맷 | 7종 (STL, STEP points-only, BRep-JSON, OBJ, OFF, PLY, XYZ) |
| GUI 리본 버튼 | 86+ (레거시) + 40+ (Design v2) |

## 빌드 방법

### 요구사항

- Rust 1.75+ (`rustup`)
- Node.js 18+ (`npm`)
- (선택) CUDA Toolkit 12+ (GPU 가속)

### Rust 솔버 + CAD 커널

```bash
cargo build --release
cargo test --workspace                                   # 911 tests
cargo run --release --bin gfd -- run examples/lid_driven_cavity.json
cargo run --release --bin gfd-benchmark                   # READ-ONLY
cargo run --release --bin gfd-server                      # stdin/stdout JSON-RPC
```

바이너리가 셋이므로 `--bin gfd`, `--bin gfd-benchmark`, `--bin gfd-server`를 명시해야 합니다.

### GUI (Electron + React)

```bash
cd gui
npm install
npm run dev            # Vite dev server (http://localhost:5173)
npm run build          # production build
npm run electron       # Electron desktop app (gfd-server spawn)
npx tsc --noEmit       # TypeScript 체크 (0 errors 필수)
```

### GPU 빌드 (선택)

```bash
cargo build --release --features gpu
```

## 아키텍처 — Rust Crate Dependency Graph

### Solver 계층

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
                      |
Layer 6 (GUI):     gui/ (Electron + React + Three.js)
```

### CAD 커널 계층 (독립 스택, pure Rust)

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
                                 src/server.rs handlers (JSON-RPC)
```

## CAD 커널 — 현재 상태 (Phase 1–8 완료)

자세한 Completion Matrix는 [`docs/CAD_KERNEL_PLAN.md`](docs/CAD_KERNEL_PLAN.md) 참조.

| Phase | 내용 | 상태 |
|-------|------|------|
| 1 | Geometry primitives (Line, Circle, BSplineCurve, Plane, Cyl, Sphere, Cone, Torus, Ellipse, Polyline) | ✅ |
| 2 | B-Rep Topology (Shape, ShapeArena, HalfEdge, EdgeFaceMap) | ✅ |
| 3 | Boolean CSG (mesh-level Möller-Trumbore) — B-Rep SSI 미구현 | ⚠️ |
| 4 | 2D Sketcher (17 constraint, 해석적 야코비안, damped Gauss-Newton + Levenberg) | ✅ |
| 5 | Features (box/sphere/cyl/cone/torus + pad/pocket/revolve + chamfer/fillet 코너·상단 edge + array) | ✅ |
| 6 | I/O (STL ASCII/binary, BRep-JSON, STEP AP214 writer, OBJ/OFF/PLY/WRL/XYZ/VTK/DXF) | ✅ |
| 7 | Healing (`check_validity`, `fix_shape`: sew_vertices + dedup_edges + close_open_wires) | ✅ |
| 8 | Measures (distance, area, volume, bbox, inertia, principal axes, signed distance, Hausdorff) | ✅ |

**CAD → GUI 플로우**

```
DesignTabV2 버튼 → cadClient.primitive/pad/revolve (JSON-RPC)
                → gfd-server (Rust)
                → ShapeArena mutate → tessellate 반환
                → useCadStore.addShape
                → CadKernelLayer가 Three.js BufferGeometry로 렌더
```

## GUI 워크플로우

```
1. Design (v2) → CAD 커널 기반 shape 생성 / STL·STEP·OBJ Import
2. Prepare    → Enclosure → Volume Extract → Named Selections
3. Mesh       → Settings (Type/Size) → Generate
4. Setup      → Models / Materials / Boundary Conditions / Solver
5. Calculation → Start → Residual Plot → Console
6. Results    → Contours / Vectors / Streamlines / Reports → Export VTK
7. Display v2 → 색상 / 가시성 / Export (13 포맷)
8. Measure v2 → Bulk area / volume / inertia / validity
9. Repair v2  → heal.check_validity 이슈 그리드 + fix_shape
```

## 주요 문서

| 문서 | 위치 | 내용 |
|------|------|------|
| Master plan | [`PROJECT_PLAN.md`](PROJECT_PLAN.md) | 솔버 리스트, 수학, 아키텍처, GPU 계획 |
| CAD 커널 로드맵 | [`docs/CAD_KERNEL_PLAN.md`](docs/CAD_KERNEL_PLAN.md) | 12-phase plan, iteration 기록, 각 phase 완성도 |
| 구현 목록 | [`docs/IMPLEMENTED_FEATURES.md`](docs/IMPLEMENTED_FEATURES.md) | 29 크레이트별 기능, 소스 파일, 테스트 커버리지 |
| Technical docs | [`docs/TECHNICAL.md`](docs/TECHNICAL.md) | 19 solver 크레이트, GUI 아키텍처, API |
| Workflow audit | [`WORKFLOW_AUDIT.md`](WORKFLOW_AUDIT.md) | 84 버튼 × 17 워크플로우, gap analysis |
| CLI adaptation | [`docs/CLI_ADAPTATION_PLAN.md`](docs/CLI_ADAPTATION_PLAN.md) | Claude Code CLI 패턴 → GFD CLI |
| Autoresearch | [`program.md`](program.md) | 자율 최적화 에이전트 규칙 |
| Experiment log | [`results.tsv`](results.tsv) | 최적화 실험 기록 |

## 남은 작업 (Known Limitations)

| 항목 | 심각도 | 상세 |
|------|--------|------|
| **B-Rep-level CSG** | HIGH | 현재 mesh CSG만 동작. SSI (surface-surface intersection) + face classification 미구현 |
| **Generic rolling-ball fillet** | MEDIUM | 박스 코너/상단 edge + cylinder 한정. 임의 B-Rep edge 미지원 |
| **STEP AP214 full reader** | MEDIUM | writer는 10개 entity kind 방출, reader는 points-only |
| **IGES I/O** | LOW | 미구현 |
| **BSplineSurface** | LOW | Phase 1 TODO |
| **Dodecahedron** | LOW | Platonic solid 마지막 하나 |
| **GUI Solver 시뮬레이션** | MEDIUM | GUI solver는 Math.exp 기반 모의. 실제 Rust SIMPLE/PISO는 `cargo run --bin gfd`로 실행 |

## 라이선스

Modified MIT License — 자세한 내용은 [LICENSE](LICENSE) 참조.

## 기여

이슈 및 PR 환영합니다.
