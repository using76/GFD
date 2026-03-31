# GFD — Generalized Fluid Dynamics

Rust 기반 통합 멀티피직스 솔버 + Electron GUI 워크벤치

> ANSYS Fluent/SpaceClaim 수준의 CFD 워크플로우를 단일 오픈소스 패키지로 제공하는 것이 목표입니다.

---

## 스크린샷

```
┌─ File ─ Quick Access ──────────────────────────────────────────────────┐
│ [Design] [Display] [Measure] [Repair] [Prepare] [Mesh] [Setup] [Calc] [Results] │
├─ Ribbon: Box Sphere Cylinder ... Import STL ... Boolean ... ───────────┤
│ ┌─ CAD Tree ─────┐ ┌─ 3D Viewport ─────────────┐ ┌─ Properties ─────┐ │
│ │ Bodies (3)      │ │                           │ │ Width: 1.0       │ │
│ │  ├ box-1       │ │    [Three.js Scene]       │ │ Height: 1.0      │ │
│ │  ├ sphere-2    │ │    Grid + Axes + Gizmo    │ │ Depth: 1.0       │ │
│ │  └ cylinder-3  │ │                           │ │                  │ │
│ │ Enclosures (1)  │ │                           │ │ [Delete Shape]   │ │
│ └────────────────┘ └───────────────────────────┘ └──────────────────┘ │
├─ Status: Ready │ Tool: Select │ Filter: Face │ 0 cells ───────────────┤
└────────────────────────────────────────────────────────────────────────┘
```

## 주요 특징

| 영역 | 내용 |
|------|------|
| **Solver** | SIMPLE/PISO/SIMPLEC 압력-속도 커플링, Roe/HLLC/AUSM+ 리만 솔버 |
| **난류** | k-epsilon, k-omega SST, Spalart-Allmaras, Realizable k-e, LES |
| **다상** | VOF+CSF, Level Set, Euler-Euler, Mixture, DPM |
| **열전달** | 전도/대류/복사(P-1, DO), 상변화, 공액열전달 |
| **고체역학** | Hex8 FEM, Von Mises 소성, Newmark-beta 동역학 |
| **메시** | Cartesian Hex, Tet, Poly, Delaunay, O-grid, Prism layer, Octree AMR |
| **GUI** | Electron + React + Three.js, SpaceClaim 스타일 리본 UI |
| **GPU** | CUDA 가속 (cudarc), AmgX 지원 (feature flag) |

## 프로젝트 규모

| 항목 | 수치 |
|------|------|
| Rust 소스 | 262 파일, 63,805 줄 |
| GUI (TypeScript/React) | 50 파일, 16,000+ 줄 |
| Crate 수 | 19 |
| Rust 테스트 | **805 passed**, 0 failed |
| TypeScript 에러 | **0** |
| GUI 구현 기능 | **125+** (35 iterations) |
| GUI 코드 | 11,239줄 |
| 리본 버튼 | 84 |
| 키보드 단축키 | 35+ |
| Export 포맷 | 8종 (VTK, STL, OpenFOAM, Gmsh, CSV, TXT, JSON, HTML) |
| 물성치 프리셋 | 14종 |
| Solver 필드 | 9종 (pressure, velocity, temperature, tke, vof_alpha, radiation_G, species_Y, wall_yplus, quality) |

## 빌드 방법

### 요구사항

- Rust 1.75+ (`rustup`)
- Node.js 18+ (`npm`)
- (선택) CUDA Toolkit 12+ (GPU 가속)

### Solver 빌드

```bash
cargo build --release
cargo test --workspace                    # 805 tests
cargo run --release --bin gfd -- run examples/lid_driven_cavity.json
```

### GUI 빌드 및 실행

```bash
cd gui
npm install
npm run dev          # 개발 서버 (Vite, http://localhost:5173)
npm run electron:dev # Electron 데스크톱 앱 (개발 모드)
npm run build        # 프로덕션 빌드
```

### GPU 빌드 (선택)

```bash
cargo build --release --features gpu
```

## 아키텍처

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

## GUI 워크플로우

```
1. Design → Shape 생성 / STL Import
2. Prepare → Enclosure → Volume Extract → Named Selections
3. Mesh → Settings (Type/Size) → Generate
4. Setup → Models / Materials / Boundary Conditions / Solver
5. Calculation → Start → Residual Plot → Console
6. Results → Contours / Vectors / Streamlines / Reports → Export VTK
```

## 현재 진척도

### 완료된 기능 (Working)

| 기능 | 상태 | 상세 |
|------|------|------|
| Shape 생성 (Box/Sphere/Cylinder/Cone/Torus/Pipe) | **완료** | 6종 프리미티브, dimensions 편집 |
| STL Import (ASCII + Binary) | **완료** | 자동 포맷 감지 |
| Copy/Cut/Paste | **완료** | Ctrl+C/X/V + 리본 버튼 |
| Undo/Redo (30단계) | **완료** | Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y |
| Boolean 연산 (Union/Subtract/Intersect/Split) | **완료** | 선택 UI + 상태 관리 |
| Fillet/Chamfer 토글 | **완료** | dimension 기반 |
| Shell/Offset/Mirror | **완료** | |
| Enclosure 생성 | **완료** | AABB + padding |
| Volume Extract | **완료** | Fluid/Solid zone, 7 mesh surfaces |
| Named Selections | **완료** | 타입별 색상, 메시 색상 매핑 |
| Display (Wireframe/Solid/Contour/Transparent/Section/Exploded) | **완료** | |
| Camera Presets (Front/Top/Iso 등) | **완료** | 애니메이션 이동 |
| Camera (Perspective/Orthographic) | **완료** | |
| Colormaps (Jet/Rainbow/Grayscale/Cool-Warm) | **완료** | 4종 |
| Shape Visibility (Hide/Show) | **완료** | 삭제가 아닌 숨김 |
| Measure (Distance/Angle/Area) | **완료** | Three.js raycasting |
| Volume/Mass Properties | **완료** | 형상별 정확 공식 |
| Mesh Generation (Hex/Tet/Poly) | **완료** | 3D grid + solid hole cutting |
| Mesh Zone/Boundary Management | **완료** | Fluent 스타일 트리 |
| Mesh Quality Statistics | **완료** | Orthogonality/Skewness/AR/Histogram |
| Physics Models (Flow/Turbulence/Energy/Multiphase/Radiation) | **완료** | UI |
| Material Presets (Air/Water/Steel/Aluminum) | **완료** | |
| Boundary Conditions (Inlet/Outlet/Wall/Symmetry) | **완료** | 속도/압력/온도 편집 |
| Solver Settings (SIMPLE/PISO, Relaxation, Tolerance) | **완료** | |
| Solver Execution (Start/Pause/Stop) | **완료** | 시뮬레이션 모드 |
| Residual Convergence Plot | **완료** | Recharts |
| Console Output | **완료** | 타임스탬프, 자동 스크롤 |
| Field Data (Pressure/Velocity/Temperature) | **완료** | per-vertex 색상 |
| Vector Arrows | **완료** | 3D ArrowHelper |
| Streamline Traces | **완료** | RK4 적분 |
| Report Statistics (min/avg/max) | **완료** | CSV 내보내기 |
| VTK Export | **완료** | Legacy ASCII 포맷 |
| File Save/Open (JSON) | **완료** | 파일 다이얼로그 |
| Keyboard Shortcuts (30+) | **완료** | Ctrl+S/Z/Y/C/X/V, F11, Del, 0-6 |
| Defeaturing Analysis | **완료** | 형상 기반 결정론적 분석 |
| Revolve/Sweep/Loft | **완료** | Shape dimension 기반 |
| Fullscreen (F11) | **완료** | |
| Physics-aware Solver | **완료** | 난류 모델/BC 기반 수렴률+필드 생성 |
| Transient Solver Mode | **완료** | 시간 스텝별 반복, 물리 시간 표시 |
| TKE Field | **완료** | 난류 운동 에너지 contour |
| VOF Phase Fraction | **완료** | 다상 인터페이스 시각화 |
| Boundary Layer Mesh | **완료** | Prism layers, 기하급수 높이 분포 |
| Curvature Refinement | **완료** | 곡면 근처 1.5x 격자 해상도 |
| Transform Gizmo (R키) | **완료** | Translate/Rotate/Scale 모드 |
| Face Hover Highlighting | **완료** | 파란 발광 효과 + 커서 변경 |
| Screenshot (Ctrl+P) | **완료** | PNG 이미지 캡처 |
| Color Legend Bar | **완료** | Contour 필드 min/max + 색상바 |
| Zoom to Fit | **완료** | 전체 shape 프레이밍 |
| Probe Points (더블클릭) | **완료** | 필드 값 보간 + 3D 마커 |
| Surface Integrals | **완료** | Wall shear, heat flux 보고서 |
| STL Ray-Casting | **완료** | Moller-Trumbore 정확한 solid 판별 |
| Solver Log/Residual Export | **완료** | .txt/.csv 내보내기 |
| Export STL (모든 primitive) | **완료** | Three.js geometry → Binary STL |
| OpenFOAM Case Export | **완료** | controlDict, fvSchemes, fvSolution, BCs |
| DPM Particle Animation | **완료** | 200 파티클 실시간 유동 추적 |
| Iso-Surface Rendering | **완료** | 필드 등치면 edge 보간 |
| Cross-Section Contour | **완료** | 절단면에 필드 DataTexture |
| Y+ Wall Distance Estimation | **완료** | Schlichting 마찰계수 기반 |
| Mesh Quality Coloring | **완료** | 품질 메트릭 contour |
| Convergence Target Line | **완료** | 잔차 플롯에 tolerance 수평선 |
| Keyboard Shortcuts Help (?) | **완료** | 19개 단축키 모달 |
| Drag-and-Drop STL Import | **완료** | 뷰포트에 파일 드래그 |
| 14 Material Presets | **완료** | 유체 9종 + 고체 4종 + Custom |
| Auto-Save (5분) | **완료** | localStorage 자동 저장 |
| FPS Counter | **완료** | 뷰포트 좌상단 실시간 FPS |
| Shape Tooltip on Hover | **완료** | 이름, 치수, 위치 표시 |
| Centerline Line Plot | **완료** | X축 따른 필드 값 Recharts 차트 |
| Solver Pre-Flight Validation | **완료** | 메시/BC/입구속도 검증 |
| Mesh Progress Log | **완료** | 단계별 콘솔 진행 표시 |
| Refinement Zone UI | **완료** | MeshSettings에서 추가/삭제 |
| 3D Annotations | **완료** | Billboard 텍스트 라벨 + 핀 |
| Annotation/Probe Management | **완료** | ToolOptions에서 목록/삭제 |
| Auto-Save Restore | **완료** | File 메뉴에서 복원 |
| Iso-Surface Settings UI | **완료** | 필드/값 선택 슬라이더 |
| Residual Auto-Scale | **완료** | Y축 자동 범위 |
| Clear All Shapes | **완료** | Undo 지원 전체 삭제 |
| Radiation Field (G) | **완료** | σT⁴ 입사 복사 필드 |
| Species Mass Fraction (Y) | **완료** | 혼합 패턴 필드 |
| Shape Lock | **완료** | 잠금/삭제 방지 + 아이콘 |
| Set Position Dialog | **완료** | X,Y,Z 좌표 입력 |
| Grid Snap (G키) | **완료** | OFF/0.1/0.25/0.5/1.0m 순환 |
| Mesh CSV Export | **완료** | 노드 좌표 + 필드 데이터 |
| Export 포맷 8종 | **완료** | VTK, STL, OpenFOAM, Gmsh, CSV, TXT, JSON, HTML |
| Quick Shape Create (Ctrl+1-6) | **완료** | Box/Sphere/Cylinder/Cone/Torus/Pipe |
| Shape Color Picker | **완료** | 9색 프리셋 팔레트 |
| Solver Progress Bar | **완료** | StatusBar 미니 프로그레스 |
| Scene Bounding Box (B키) | **완료** | 전체 shape 와이어프레임 |
| Convergence Rate Display | **완료** | decades/iter + 품질 평가 |
| Shape Surface Area | **완료** | 모든 primitive 공식 |
| Wall y+ Visualization | **완료** | Schlichting 기반 필드 |
| Mass/Energy Balance | **완료** | 보존 법칙 검증 보고서 |
| Field Normalize | **완료** | [0,1] 범위 정규화 |
| Mesh Independence Guide | **완료** | 4단계 검증 안내 |
| Multi-Select (Ctrl+Click) | **완료** | 다중 선택 + 정렬/배분 |
| Shape Align/Distribute | **완료** | X/Y/Z 정렬 + 등간격 배분 |
| Array Pattern | **완료** | N개 복사 + 간격/축 설정 |
| 3D Annotations | **완료** | Billboard 텍스트 라벨 |
| Refinement Zone UI | **완료** | 로컬 격자 세분화 |
| Dimension Lines | **완료** | W/H/D 화살표 표시 |
| CadTree Visibility Toggle | **완료** | 눈 아이콘 직접 클릭 |
| Undo History Panel | **완료** | 스냅샷 목록 + 버튼 |
| Shape Lock | **완료** | 잠금/삭제 방지 |
| HTML Report Export | **완료** | 스타일링된 보고서 파일 |

### 미비한 점 (Known Limitations)

| 항목 | 심각도 | 상세 |
|------|--------|------|
| **Solver가 시뮬레이션** | CRITICAL | GUI solver는 모의(fake) — `Math.exp()` 기반 잔차. 실제 SIMPLE/PISO는 Rust backend에 구현되어 있으나 GUI와 IPC 미연결 |
| **Rust↔GUI IPC 미연결** | CRITICAL | `gfd-server` JSON-RPC 서버 존재하나 Electron에서 호출하지 않음. `gfdClient.ts` 스텁만 있음 |
| **Repair 탭 분석** | HIGH | Check/Fix 버튼이 랜덤 이슈 생성 (실제 B-rep 분석 없음) |
| **Boolean CSG 미구현** | HIGH | 상태 관리만 작동, 실제 메시 부울 연산 없음 (three-bvh-csg 등 필요) |
| **STL→Solid AABB 근사** | MEDIUM | STL의 point-in-solid 판별이 바운딩 박스만 사용 (ray casting 미구현) |
| **Mesh type 제한** | MEDIUM | Tet/Poly는 시각적 변형만 (실제 비정렬 알고리즘 미구현) |
| **Turbulence model 미반영** | MEDIUM | 모델 선택이 콘솔 로그에만 반영, solver 알고리즘 변경 없음 |
| **BC→Solver 미연결** | MEDIUM | BC 데이터가 모의 solver에 전달되지 않음 |
| **Named Selection 3D 클릭** | MEDIUM | 면 직접 클릭 선택 미구현 (수동 입력) |
| **Setup 탭 이벤트 라우팅** | LOW | Custom event 기반 — Zustand 상태로 전환 필요 |

### 로드맵

1. **Phase 1**: Rust gfd-server ↔ Electron IPC 연결 (solve.start → 실제 SIMPLE 실행)
2. **Phase 2**: 실제 CSG Boolean (three-bvh-csg 또는 Rust manifold3d)
3. **Phase 3**: Ray casting 기반 정확한 point-in-solid
4. **Phase 4**: GPU solver 가속 연동 (gfd-gpu CUDA)
5. **Phase 5**: 병렬 MPI 실행 (gfd-parallel)

## 라이선스

Modified MIT License — 자세한 내용은 [LICENSE](LICENSE) 참조.

## 기여

이슈 및 PR 환영합니다.
