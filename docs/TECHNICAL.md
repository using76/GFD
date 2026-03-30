# GFD Technical Documentation

> Rust 멀티피직스 솔버 + Electron GUI 상세 기술 문서

---

## 1. Rust Solver Architecture

### 1.1 Crate 구조 (19 crates)

```
gfd-core          핵심 타입: UnstructuredMesh, StructuredMesh, ScalarField, VectorField, TensorField, SparseMatrix(CSR)
gfd-matrix        COO→CSR 조립: Assembler, CooMatrix, counting sort, apply_dirichlet/neumann
gfd-linalg        프로덕션 선형 솔버: CG, BiCGSTAB, GMRES, PCG, PBiCGSTAB, ILU0/Jacobi preconditioner
gfd-discretize    유한체적법 이산화: Green-Gauss gradient, 선형 보간
gfd-boundary      경계조건 적용: Dirichlet, Neumann, Robin, 주기적
gfd-source        소스항: 체적열, 중력, Boussinesq, MHD
gfd-material      물성치 관리: 온도 종속, 다상 혼합, 사용자 정의
gfd-turbulence    난류 모델: k-epsilon, k-omega SST, SA, Realizable k-e, Transition SST, LES
gfd-fluid         유동 솔버: SIMPLE/PISO/SIMPLEC, Roe/HLLC/AUSM+, VOF/LevelSet/Euler-Euler/DPM
gfd-thermal       열전달 솔버: 정상/비정상 전도, 대류-확산, P-1/DO 복사, 상변화
gfd-solid         고체역학: Hex8 FEM, Von Mises 소성, Newmark-β, 접촉, 크리프
gfd-coupling      다물리 커플링: 유체-고체, 열-구조, 분할 반복
gfd-mesh          메시 생성: Cartesian, Delaunay 2D/3D, O-grid, Hex sweep, Cut-cell, Octree AMR
gfd-io            입출력: JSON config, Gmsh v2.2 reader, STL reader, VTK Legacy writer, 체크포인트
gfd-gpu           GPU 가속: cudarc CUDA 래핑, GpuCG, AmgX 스텁
gfd-parallel      병렬 컴퓨팅: MPI 추상화, 도메인 분할
gfd-postprocess   후처리: 프로브, 통계, 잔차 수렴
gfd-expression    수학식 SDK: tokenizer→AST→LaTeX/Rust codegen, 심볼릭 미분, 차원 분석
gfd-vdb          VDB 격자: OpenVDB 호환 sparse 격자
```

### 1.2 FVM 솔버 패턴

모든 물리 솔버가 동일한 패턴을 따릅니다:

```rust
// 1. 면 계수 사전 계산
for face in &mesh.faces {
    let D = viscosity * face.area / face.delta;  // diffusion
    let F = density * face.velocity * face.area; // convection
}

// 2. 성분별 조립 + 풀이
for comp in 0..3 {
    let mut assembler = Assembler::with_nnz_estimate(n, n + 2*n_internal);
    // 면 루프: assembler.add_diagonal(), add_neighbor()
    // 소스/BC: assembler.add_source()
    let system = assembler.finalize();  // COO → CSR (counting sort)
    BiCGSTAB::solve(&system.matrix, &system.rhs, &mut solution);
}
```

### 1.3 이중 LinearSolver 트레이트

```
gfd_core::linalg::solvers::LinearSolver    — &mut LinearSystem 인터페이스 (기본 구현)
gfd_linalg::traits::LinearSolverTrait      — (&SparseMatrix, &[f64], &mut [f64]) (프로덕션)
```

물리 솔버는 반드시 `LinearSolverTrait`을 사용해야 합니다.

### 1.4 성능 최적화 결과

| 최적화 | 효과 | 상태 |
|--------|------|------|
| unsafe SpMV (CSR) | **-13.7%** | 적용 |
| COO→CSR counting sort | -5.5% | 적용 |
| Assembler 직접 추가 | -6.8% | 적용 |
| 면 계수 사전 계산 | -3.7% | 적용 |
| ILU(0) 전처리기 | +setup 비용 > 효과 | 폐기 |
| Rhie-Chow 보간 | 수렴 파괴 | 폐기 |

---

## 2. GUI Architecture

### 2.1 기술 스택

| 계층 | 기술 |
|------|------|
| 데스크톱 쉘 | Electron 28+ |
| 프레임워크 | React 18 + TypeScript |
| 3D 엔진 | Three.js (React Three Fiber + Drei) |
| UI 컴포넌트 | Ant Design 5 |
| 상태 관리 | Zustand |
| 차트 | Recharts |
| 빌드 | Vite 6 |
| IPC | JSON-RPC over stdin/stdout (계획) |

### 2.2 파일 구조

```
gui/
├── electron/
│   └── main.js              # Electron 메인 프로세스
├── src/
│   ├── App.tsx              # 메인 레이아웃 (AppMenu, QuickAccess, Ribbon, Tabs)
│   ├── main.tsx             # React 엔트리포인트
│   ├── store/
│   │   └── useAppStore.ts   # Zustand 전역 상태 (1600+ 줄)
│   ├── components/
│   │   ├── Ribbon.tsx       # SpaceClaim 스타일 9탭 리본 (84 버튼)
│   │   ├── ContextMenu3D.tsx # 3D 뷰포트 우클릭 메뉴
│   │   ├── LeftPanelStack.tsx # 동적 좌측 패널 라우팅
│   │   ├── MeasureOverlay.tsx # 측정 결과 HTML 오버레이
│   │   ├── MenuBar.tsx      # 상단 메뉴바
│   │   ├── MiniToolbar.tsx  # 카메라 프리셋 버튼
│   │   ├── OutlineTree.tsx  # 재사용 가능 트리 뷰
│   │   ├── PropertyGrid.tsx # 속성 편집 그리드
│   │   ├── SelectionFilter.tsx # 엔티티 선택 필터
│   │   ├── SplitLayout.tsx  # 3분할 레이아웃
│   │   ├── StatusBar.tsx    # 하단 상태바
│   │   └── ToolOptions.tsx  # 도구별 옵션 패널
│   ├── engine/
│   │   ├── CadScene.tsx     # Three.js CAD 렌더링 (1700+ 줄)
│   │   ├── MeshRenderer.tsx # 메시 + 필드 데이터 렌더링
│   │   ├── Viewport3D.tsx   # Canvas + 조명 + 카메라
│   │   ├── CameraControls.tsx # 카메라 뷰 프리셋
│   │   └── SelectionManager.tsx # 엔티티 선택
│   ├── tabs/
│   │   ├── CadTab.tsx       # Design/Display/Measure/Repair/Prepare
│   │   ├── MeshTab.tsx      # Mesh 탭
│   │   ├── SetupTab.tsx     # Setup 탭
│   │   ├── CalcTab.tsx      # Calculation 탭
│   │   ├── ResultsTab.tsx   # Results 탭
│   │   ├── cad/             # CAD 하위 패널들
│   │   ├── mesh/            # Mesh 하위 패널들
│   │   ├── setup/           # Setup 하위 패널들
│   │   ├── calc/            # Calc 하위 패널들
│   │   └── results/         # Results 하위 패널들
│   └── ipc/
│       └── gfdClient.ts     # Rust backend IPC 클라이언트 (스텁)
```

### 2.3 상태 관리 (Zustand Store)

```typescript
interface AppState {
  // 탭/리본/도구 (UI 상태)
  activeTab, activeRibbonTab, activeTool, selectionFilter

  // CAD (형상 데이터)
  shapes: Shape[]           // 모든 3D 형상
  booleanOps                // Boolean 연산 기록
  defeatureIssues           // Defeaturing 이슈
  namedSelections           // Named Selections (CFD prep)

  // Undo/Redo (히스토리)
  undoStack: Shape[][]      // 최대 30단계
  redoStack: Shape[][]

  // 메시 (격자 데이터)
  meshConfig                // 격자 설정 (type, size, growth rate)
  meshDisplayData           // 렌더링용 Float32Array (positions, colors, wireframe)
  meshQuality               // 품질 메트릭
  meshVolumes, meshSurfaces // Fluent 스타일 zone 관리

  // 물리 설정
  physicsModels             // Flow/Turbulence/Energy/Multiphase/Radiation
  material                  // 물성치 (density, viscosity, cp, conductivity)
  boundaries                // 경계조건 리스트
  solverSettings            // SIMPLE/PISO, relaxation, tolerance

  // 계산
  solverStatus              // idle/running/paused/finished
  residuals                 // 잔차 히스토리
  fieldData                 // 압력/속도/온도 필드

  // 결과
  contourConfig             // Colormap, range, opacity
  vectorConfig              // Scale, density
  showVectors, showStreamlines
}
```

### 2.4 3D 렌더링 파이프라인

```
Shape (store) → makeGeometry() → Three.js BufferGeometry
                                       ↓
                               ShapeMesh component
                                       ↓
                               meshStandardMaterial (color, opacity, emissive)
                                       ↓
                               Edges (outline)
                                       ↓
                               CadScene group → Canvas render
```

**메시 렌더링:**
```
generateMesh() → MeshDisplayData {positions, colors, wireframePositions}
                        ↓
                 MeshRenderer → BufferGeometry (non-indexed)
                        ↓
                 vertexColors (boundary type 색상 또는 field contour)
                        ↓
                 wireframe overlay (lineSegments)
```

### 2.5 메시 생성 알고리즘

```
1. 도메인 결정: Enclosure → (xMin, yMin, zMin) ~ (xMax, yMax, zMax)
2. 셀 크기 계산: nx = round(Lx / globalSize), ny, nz
3. 셀 분류: isPointInsideSolid(cx, cy, cz) → fluid(0) / solid(1)
   - Sphere: distance < radius
   - Cylinder: radial distance < radius && |y| < h/2
   - STL: AABB containment
   - Box: axis-aligned containment
4. 표면 추출: 6방향 인접 셀 검사 → boundary/interface face
5. Tet 변형: center-point 4분할 (quad → 4 triangles)
6. Poly 변형: 0.85 shrink factor
7. 색상: boundary type → RGB 코드
8. 와이어프레임: quad 또는 tet edge 쌍
```

### 2.6 키보드 단축키

| 키 | 동작 |
|-----|------|
| Ctrl+S | 프로젝트 저장 |
| Ctrl+Z | Undo |
| Ctrl+Shift+Z, Ctrl+Y | Redo |
| Ctrl+C | 형상 복사 |
| Ctrl+X | 형상 잘라내기 |
| Ctrl+V | 형상 붙여넣기 |
| S | Select 도구 |
| P | Pull 도구 |
| M | Move 도구 |
| F | Fill 도구 |
| H | Home 카메라 |
| Delete | 선택 삭제 |
| Escape | 선택 해제 |
| F11 | 전체화면 토글 |
| 0-6 | 카메라 프리셋 (Iso, Front, Back, Top, Bottom, Left, Right) |

---

## 3. JSON-RPC Server

### 3.1 지원 메서드

```json
// Rust → gfd-server (src/server.rs)
{
  "system.version": {},
  "system.capabilities": {},
  "cad.create_primitive": { "kind": "box", "dimensions": {...} },
  "mesh.generate": { "nx": 20, "ny": 20, "nz": 20 },
  "solve.start": { "method": "SIMPLE", "max_iterations": 500 },
  "solve.status": {},
  "solve.stop": {},
  "field.get": { "name": "pressure" },
  "field.contour": { "name": "velocity", "colormap": "jet" }
}
```

### 3.2 IPC 통신 (계획)

```
Electron Main Process
    ↓ spawn
Rust gfd-server (stdin/stdout)
    ↓ JSON-RPC
GUI Renderer (gfdClient.ts)
```

현재는 `gfdClient.ts`가 브라우저 시뮬레이션 모드로 동작합니다.

---

## 4. 벤치마크

### 4.1 테스트 케이스

| 케이스 | 설명 | 격자 |
|--------|------|------|
| heat_1d | 1D 정상 열전도 (해석해 비교) | 50 셀 |
| heat_source | 소스항 열전도 | 100 셀 |
| cavity_20 | Lid-driven cavity Re=100 | 20×20 |
| cavity_50 | Lid-driven cavity Re=100 | 50×50 |
| cavity_100 | Lid-driven cavity Re=100 | 100×100 |

### 4.2 성능 이력

903ms → 856ms → 810ms → 720ms → 320ms → **232ms** (최종, 74% 감소)

---

## 5. 예제 실행

### Lid-Driven Cavity

```bash
cargo run --release --bin gfd -- run examples/lid_driven_cavity.json
```

```json
{
  "mesh": { "type": "structured", "nx": 20, "ny": 20 },
  "fluid": { "density": 1.0, "viscosity": 0.01 },
  "solver": { "method": "SIMPLE", "max_iterations": 500, "tolerance": 1e-6 },
  "boundary": {
    "top": { "type": "moving_wall", "velocity": [1, 0] },
    "bottom": { "type": "wall" },
    "left": { "type": "wall" },
    "right": { "type": "wall" }
  }
}
```

### 열전도

```bash
cargo run --release --bin gfd -- run examples/heat_conduction.json
```
