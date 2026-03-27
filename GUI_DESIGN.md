# GFD GUI — 모듈형 멀티피직스 워크벤치 설계 문서

> **목표:** Fluent/CFX/STAR-CCM+ 수준의 통합 GUI 워크벤치
> **프레임워크:** Electron + React + Three.js (R3F)
> **설계 원칙:** 모듈 재사용 · 플러그인 확장 · 자동 업데이트 · 컴포넌트 독립성
> **참조:** ANSYS Fluent, SpaceClaim, ICEM CFD, ParaView, FreeCAD, Onshape

---

## 1. 탭 구조 (워크플로우)

```
┌──────────────────────────────────────────────────────────────────┐
│  GFD Workbench                                          ─ □ ×   │
├──────────────────────────────────────────────────────────────────┤
│  [CAD] [MESH] [Numerical Setup] [Calculation] [Results]         │
├──────┬──────────────────────────────────────┬───────────────────┤
│  ... │          3D Viewport                  │   Properties      │
│      │                                       │   Panel           │
└──────┴──────────────────────────────────────┴───────────────────┘
```

각 탭은 **독립 모듈**로 분리되어 별도 배포/업데이트 가능.

| 탭 | 역할 | 참조 |
|---|------|------|
| **CAD** | 형상 생성/편집/불리안/STL 임포트 | SpaceClaim, FreeCAD, Onshape |
| **MESH** | 메시 생성/품질/적응/편집 | Fluent Meshing, ICEM CFD, snappyHexMesh |
| **Numerical Setup** | 물리 모델/재질/경계조건/솔버 설정 | Fluent Setup, CFX-Pre |
| **Calculation** | 실행/모니터/수렴/중지 | Fluent Solution |
| **Results** | 컨투어/벡터/유선/절단면/애니메이션 | Fluent Post, ParaView, CFD-Post |

---

## 2. 재사용 가능 핵심 모듈

### 2.1 모듈 의존 관계

```
                   ┌─────────────┐
                   │  App Shell  │ ← Electron 메인, 탭 라우터, 업데이트
                   └──────┬──────┘
                          │
      ┌───────────────────┼───────────────────────┐
      ▼                   ▼                        ▼
┌──────────┐    ┌──────────────┐    ┌────────────────────┐
│ 3D Engine│    │ Panel System │    │ IPC / State Manager│
│ (공유)   │    │ (공유)       │    │ (공유)             │
└──────────┘    └──────────────┘    └────────────────────┘
      │                   │                        │
      ▼                   ▼                        ▼
┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
│   CAD    │ │  MESH    │ │ Num.Setup│ │  Calc.   │ │ Results  │
│  Module  │ │  Module  │ │  Module  │ │  Module  │ │  Module  │
└──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘
```

### 2.2 공유 모듈 상세

#### A. 3D Engine (모든 탭에서 재사용)

```
packages/3d-engine/
├── src/
│   ├── Viewport.tsx          # Three.js 캔버스 + OrbitControls
│   ├── SceneManager.ts       # 씬 그래프 관리
│   ├── CameraController.ts   # 회전/줌/팬 + 뷰 저장/복원
│   ├── SelectionManager.ts   # 면/모서리/점 선택 (레이캐스팅)
│   ├── TransformGizmo.tsx    # 이동/회전/스케일 기즈모
│   ├── GridHelper.tsx        # 바닥 그리드 + 축 표시
│   ├── renderers/
│   │   ├── SurfaceRenderer.ts    # 삼각형 표면 렌더링
│   │   ├── WireframeRenderer.ts  # 와이어프레임
│   │   ├── PointCloudRenderer.ts # 포인트 클라우드
│   │   ├── ContourRenderer.ts    # 스칼라장 컬러맵
│   │   ├── VectorRenderer.ts     # 화살표/글리프
│   │   ├── StreamlineRenderer.ts # 유선 (입자 추적)
│   │   ├── IsoSurfaceRenderer.ts # 등가면
│   │   └── ClipPlaneRenderer.ts  # 절단면
│   ├── materials/
│   │   ├── ColorMapMaterial.ts   # Jet/Rainbow/Grayscale 컬러맵
│   │   └── TransparentMaterial.ts# 반투명 재질
│   └── utils/
│       ├── BinaryTransfer.ts     # Rust→JS 바이너리 데이터
│       ├── LODManager.ts         # Level of Detail 관리
│       └── PerformanceMonitor.ts # FPS/메모리 모니터
```

**3D 공간 조작 — CAD 참조 기능:**

| 기능 | 설명 | 구현 |
|------|------|------|
| **Orbit** | 마우스 중앙 드래그 → 회전 | OrbitControls (Three.js 내장) |
| **Pan** | Shift+중앙 / 우클릭 드래그 → 이동 | OrbitControls |
| **Zoom** | 스크롤 휠 → 확대/축소 | OrbitControls |
| **Zoom to Fit** | 더블클릭 → 전체 모델 맞춤 | camera.lookAt(boundingBox.center) |
| **View Preset** | Front/Back/Top/Bottom/Left/Right/Iso | 카메라 위치 프리셋 |
| **Selection** | 클릭 → 면/모서리/점 선택 | Raycaster + Octree 가속 |
| **Box Select** | 드래그 → 영역 다중 선택 | Frustum culling |
| **Hide/Show** | 선택 객체 숨기기/보이기 | visible 토글 |
| **Transparency** | 선택 객체 반투명 | opacity 조절 |
| **Section Plane** | 절단면으로 내부 관찰 | ClipPlane + stencil buffer |
| **Measure** | 두 점 사이 거리 측정 | 클릭 두 점 → 라인 + 레이블 |
| **Annotation** | 3D 공간에 텍스트 메모 | CSS2DObject 또는 Sprite |
| **Screenshot** | 현재 뷰 PNG 저장 | canvas.toDataURL() |

#### B. Panel System (모든 탭에서 재사용)

```
packages/panel-system/
├── src/
│   ├── SplitLayout.tsx       # 리사이즈 가능한 분할 레이아웃
│   ├── DockablePanel.tsx     # 드래그앤드롭 도킹 패널
│   ├── OutlineTree.tsx       # 계층형 트리 뷰 (Fluent 스타일)
│   ├── PropertyGrid.tsx      # Key-Value 속성 편집기
│   ├── TabBar.tsx            # 워크플로우 탭바
│   ├── Toolbar.tsx           # 아이콘 툴바 (undo/redo 포함)
│   ├── StatusBar.tsx         # 하단 상태바
│   ├── DialogManager.tsx     # 모달/팝업 관리
│   ├── ContextMenu.tsx       # 우클릭 메뉴
│   └── forms/
│       ├── NumberInput.tsx    # 소수점 숫자 입력 (드래그로 값 변경)
│       ├── VectorInput.tsx    # [x, y, z] 벡터 입력
│       ├── Dropdown.tsx       # 드롭다운 선택
│       ├── ColorPicker.tsx    # 색상 선택
│       ├── RangeSlider.tsx    # 범위 슬라이더
│       └── FileSelector.tsx   # 파일 선택 다이얼로그
```

#### C. IPC / State Manager (공유 통신 계층)

```
packages/ipc-bridge/
├── src/
│   ├── GfdClient.ts          # Rust JSON-RPC 클라이언트
│   ├── BinaryProtocol.ts     # 대용량 데이터용 바이너리 프로토콜
│   ├── JobManager.ts         # 비동기 솔버 작업 관리
│   └── EventBus.ts           # 탭 간 이벤트 통신

packages/state-manager/
├── src/
│   ├── projectStore.ts       # 프로젝트 전체 상태 (Zustand)
│   ├── geometryStore.ts      # CAD 형상 데이터
│   ├── meshStore.ts          # 메시 데이터
│   ├── setupStore.ts         # 물리/경계조건 설정
│   ├── solverStore.ts        # 솔버 진행 상태
│   ├── resultStore.ts        # 결과 데이터
│   └── undoManager.ts        # Undo/Redo 스택
```

---

## 3. 탭별 상세 설계

### 3.1 CAD 탭

```
┌──────┬───────────────────────────────────────┬──────────────┐
│      │                                       │              │
│  C   │         3D Viewport                   │  Properties  │
│  A   │         (형상 편집 모드)              │              │
│  D   │                                       │  ┌────────┐  │
│      │  ┌─────────────────────────────────┐  │  │ 치수   │  │
│  T   │  │  [이동][회전][스케일] 기즈모    │  │  │ 위치   │  │
│  r   │  │  그리드 + 스냅               │  │  │ 파라미터│ │
│  e   │  │  부울 연산 미리보기             │  │  │        │  │
│  e   │  └─────────────────────────────────┘  │  └────────┘  │
│      │                                       │              │
│  📁  ├───────────────────────────────────────┤  ┌────────┐  │
│  Bo  │  Toolbar:                             │  │ 재질   │  │
│  dy  │  [Box][Sphere][Cylinder][Extrude]     │  │ 색상   │  │
│      │  [Union][Subtract][Intersect]         │  │        │  │
│  📁  │  [Import STL][Import STEP]            │  └────────┘  │
│  Op  │                                       │              │
└──────┴───────────────────────────────────────┴──────────────┘
```

**CAD 모듈 컴포넌트:**

```
packages/cad-module/
├── src/
│   ├── CadTab.tsx              # CAD 탭 메인
│   ├── tools/
│   │   ├── PrimitiveTool.ts    # Box/Sphere/Cylinder 생성
│   │   ├── ExtrudeTool.ts      # 면 돌출
│   │   ├── RevolveTool.ts      # 축 회전 돌출
│   │   ├── BooleanTool.ts      # Union/Subtract/Intersect
│   │   ├── FilletTool.ts       # 모서리 라운딩
│   │   ├── TransformTool.ts    # 이동/회전/스케일
│   │   └── SketchTool.ts       # 2D 스케치 (선/원호/스플라인)
│   ├── importers/
│   │   ├── StlImporter.ts      # STL 파일 임포트
│   │   ├── StepImporter.ts     # STEP 파일 (향후)
│   │   └── ObjImporter.ts      # OBJ 파일
│   ├── geometry/
│   │   ├── BRepKernel.ts       # 경계 표현 (B-Rep) 커널
│   │   ├── SdfKernel.ts        # SDF 기반 형상 (Rust 연동)
│   │   └── ParametricBody.ts   # 파라메트릭 형상
│   └── tree/
│       └── CadTree.tsx         # CAD 히스토리 트리
```

### 3.2 MESH 탭

```
┌──────┬───────────────────────────────────────┬──────────────┐
│      │                                       │              │
│  M   │         3D Viewport                   │  Mesh        │
│  E   │         (메시 미리보기)               │  Settings    │
│  S   │                                       │              │
│  H   │  ┌─────────────────────────────────┐  │  ┌────────┐  │
│      │  │  와이어프레임 / 셀 표면        │  │  │ Type:  │  │
│  T   │  │  품질 히트맵 (빨강=나쁜셀)    │  │  │ [Auto] │  │
│  r   │  │  단면 보기                      │  │  │ [Hex ] │  │
│  e   │  └─────────────────────────────────┘  │  │ [Tet ] │  │
│  e   │                                       │  │ [Poly] │  │
│      ├───────────────────────────────────────┤  │        │  │
│  📁  │  Quality Report:                      │  │ Size:  │  │
│  Sur │  Min Ortho: 0.35  Max Skew: 0.72     │  │ 0.01   │  │
│  fac │  Max AR: 4.2  Bad cells: 12          │  │        │  │
│  📁  │  ████████████████░░░░ 95% good       │  │ Wall:  │  │
│  Vol │                                       │  │ 5 layer│  │
└──────┴───────────────────────────────────────┴──────────────┘
```

**MESH 모듈 컴포넌트:**

```
packages/mesh-module/
├── src/
│   ├── MeshTab.tsx              # MESH 탭 메인
│   ├── generators/
│   │   ├── AutoMeshPanel.tsx    # 자동 메싱 (형상→메시)
│   │   ├── StructuredPanel.tsx  # 정렬격자 설정
│   │   ├── CutCellPanel.tsx     # Cut-cell 설정
│   │   └── RefinementPanel.tsx  # 적응형 세분화
│   ├── sizing/
│   │   ├── GlobalSize.tsx       # 전역 셀 크기
│   │   ├── LocalSize.tsx        # 국소 크기 (면/볼륨/BOI)
│   │   ├── PrismLayer.tsx       # 벽면 프리즘 레이어
│   │   └── GrowthRate.tsx       # 성장률 설정
│   ├── quality/
│   │   ├── QualityReport.tsx    # 품질 통계 + 히스토그램
│   │   ├── QualityHeatmap.tsx   # 3D 품질 컬러맵
│   │   └── CellInspector.tsx    # 개별 셀 검사
│   └── tree/
│       └── MeshTree.tsx         # 메시 존/영역 트리
```

### 3.3 Numerical Setup 탭

```
┌──────┬───────────────────────────────────────┬──────────────┐
│      │                                       │              │
│  S   │         3D Viewport                   │  Settings    │
│  E   │         (경계조건 시각화)             │  Detail      │
│  T   │                                       │              │
│  U   │  ┌─────────────────────────────────┐  │  ┌────────┐  │
│  P   │  │  경계면 색상 코딩:              │  │  │Selected│  │
│      │  │  🔴 Inlet  🔵 Outlet           │  │  │ inlet  │  │
│  T   │  │  🟢 Wall   🟡 Symmetry         │  │  │        │  │
│  r   │  │  클릭하면 경계조건 편집         │  │  │ Type:  │  │
│  e   │  └─────────────────────────────────┘  │  │velocity│  │
│  e   │                                       │  │ Vx: 1.0│  │
│      ├───────────────────────────────────────┤  │ Vy: 0.0│  │
│  📁  │  JSON Config Preview:                 │  │ Vz: 0.0│  │
│  Mod │  {                                    │  │        │  │
│  els │    "flow": "incompressible",          │  │ T: 300 │  │
│  📁  │    "turbulence": "k-omega-sst",       │  │        │  │
│  BCs │    "energy": true                     │  └────────┘  │
└──────┴───────────────────────────────────────┴──────────────┘
```

**Setup 모듈 컴포넌트:**

```
packages/setup-module/
├── src/
│   ├── SetupTab.tsx             # Setup 탭 메인
│   ├── models/
│   │   ├── FlowModelPanel.tsx   # 유동 모델 (비압축/압축)
│   │   ├── TurbulencePanel.tsx  # 난류 모델 선택
│   │   ├── EnergyPanel.tsx      # 에너지 방정식 On/Off
│   │   ├── RadiationPanel.tsx   # 복사 모델
│   │   ├── MultiphasePanel.tsx  # 다상유동 모델
│   │   └── SpeciesPanel.tsx     # 종/연소 모델
│   ├── boundary/
│   │   ├── BoundaryList.tsx     # 경계 목록 + 색상 코딩
│   │   ├── BoundaryEditor.tsx   # 경계조건 편집 폼
│   │   ├── InletEditor.tsx      # 속도/질량유량 입구
│   │   ├── OutletEditor.tsx     # 압력 출구
│   │   ├── WallEditor.tsx       # 벽면 (열/이동)
│   │   └── SymmetryEditor.tsx   # 대칭면
│   ├── materials/
│   │   ├── MaterialLibrary.tsx  # 재질 라이브러리 (물/공기/강철...)
│   │   └── MaterialEditor.tsx   # 재질 속성 편집
│   ├── solver/
│   │   ├── MethodPanel.tsx      # SIMPLE/PISO/SIMPLEC
│   │   ├── RelaxationPanel.tsx  # Under-relaxation 설정
│   │   └── InitialPanel.tsx     # 초기 조건
│   └── tree/
│       └── SetupTree.tsx        # 설정 트리 (Fluent Outline 스타일)
```

### 3.4 Calculation 탭

```
┌──────┬───────────────────────────────────────┬──────────────┐
│      │  Residual Plot                        │  Controls    │
│  C   │  ┌─────────────────────────────────┐  │              │
│  A   │  │  1e+0 ┤                         │  │  [▶ Start]  │
│  L   │  │  1e-1 ┤  ╲                      │  │  [⏸ Pause]  │
│  C   │  │  1e-2 ┤   ╲__                   │  │  [⏹ Stop]   │
│      │  │  1e-3 ┤      ╲___               │  │              │
│  T   │  │  1e-4 ┤──────────╲──── target  │  │  Iterations: │
│  r   │  │  1e-5 ┤            ╲            │  │  [500]       │
│  e   │  │       └─────────────────────────│  │  Tolerance:  │
│  e   │  │       0    100   200   300  iter│  │  [1e-4]      │
│      │  └─────────────────────────────────┘  │              │
│      ├───────────────────────────────────────┤  Progress:   │
│  📁  │  Console:                             │  ████░ 72%   │
│  Mon │  iter  35: residual = 9.78e-5         │              │
│  itor│  Converged at iteration 36            │  GPU: ✅ ON  │
│  📁  │  Wall time: 42 ms                    │  MPI: 4 proc │
│  Log │  Results written to result.vtk        │              │
└──────┴───────────────────────────────────────┴──────────────┘
```

**Calculation 모듈 컴포넌트:**

```
packages/calc-module/
├── src/
│   ├── CalcTab.tsx              # Calculation 탭 메인
│   ├── controls/
│   │   ├── RunControls.tsx      # 시작/중지/일시정지
│   │   ├── IterationSettings.tsx# 반복/허용오차
│   │   └── HardwarePanel.tsx    # GPU/MPI 설정
│   ├── monitors/
│   │   ├── ResidualPlot.tsx     # 잔차 수렴 그래프 (Recharts)
│   │   ├── ProbeMonitor.tsx     # 프로브 값 실시간 그래프
│   │   ├── ForceMonitor.tsx     # 항력/양력 모니터
│   │   └── ConvergenceTable.tsx # 반복별 잔차 표
│   ├── console/
│   │   └── SolverConsole.tsx    # 솔버 로그 터미널
│   └── progress/
│       ├── ProgressBar.tsx      # 진행률 바
│       └── TimeEstimator.ts     # 남은 시간 추정
```

### 3.5 Results 탭

```
┌──────┬───────────────────────────────────────┬──────────────┐
│      │                                       │              │
│  R   │         3D Viewport                   │  Display     │
│  E   │         (결과 시각화)                 │  Options     │
│  S   │                                       │              │
│  U   │  ┌─────────────────────────────────┐  │  ┌────────┐  │
│  L   │  │                                 │  │  │ Field: │  │
│  T   │  │   압력 컨투어 + 속도 벡터      │  │  │[press.]│  │
│  S   │  │   절단면에 유선 표시            │  │  │        │  │
│      │  │   컬러바: Min ████████ Max      │  │  │ Style: │  │
│  T   │  │                                 │  │  │[contour│  │
│  r   │  └─────────────────────────────────┘  │  │ vector]│  │
│  e   │                                       │  │ [stream│  │
│  e   ├───────────────────────────────────────┤  │ lines] │  │
│      │  Reports:                             │  │        │  │
│  📁  │  ┌────────┐ ┌────────┐ ┌──────────┐  │  │ Range: │  │
│  Con │  │ Forces │ │ Flux   │ │ Average  │  │  │[auto]  │  │
│  tour│  │ Cd=0.32│ │ 2.1e-3 │ │ T=342K   │  │  │[custom]│  │
│  📁  │  └────────┘ └────────┘ └──────────┘  │  └────────┘  │
│  Vec │                                       │              │
└──────┴───────────────────────────────────────┴──────────────┘
```

**Results 모듈 컴포넌트:**

```
packages/results-module/
├── src/
│   ├── ResultsTab.tsx           # Results 탭 메인
│   ├── visualization/
│   │   ├── ContourPanel.tsx     # 컨투어 설정 (필드/범위/컬러맵)
│   │   ├── VectorPanel.tsx      # 벡터 화살표 (크기/밀도/색상)
│   │   ├── StreamlinePanel.tsx  # 유선 (시드점/개수/색상)
│   │   ├── IsoSurfacePanel.tsx  # 등가면 (값/투명도)
│   │   ├── ClipPlanePanel.tsx   # 절단면 (위치/방향)
│   │   └── AnimationPanel.tsx   # 시간별 애니메이션
│   ├── reports/
│   │   ├── ForceReport.tsx      # 항력/양력 계산
│   │   ├── FluxReport.tsx       # 면 플럭스 (질량/열)
│   │   ├── AverageReport.tsx    # 면/체적 평균값
│   │   ├── MinMaxReport.tsx     # 최대/최소 값 + 위치
│   │   └── ProbeReport.tsx      # 프로브 포인트 값
│   ├── export/
│   │   ├── ScreenshotExport.tsx # PNG/JPEG 내보내기
│   │   ├── DataExport.tsx       # CSV/Excel 데이터 내보내기
│   │   └── AnimationExport.tsx  # MP4/GIF 애니메이션 내보내기
│   └── tree/
│       └── ResultsTree.tsx      # 결과 객체 트리
```

---

## 4. 3D 공간 조작 상세 (CAD 참조)

### 4.1 마우스/키보드 조작 매핑

| 입력 | CAD 모드 | Mesh/Results 모드 |
|------|---------|-------------------|
| **좌클릭** | 객체 선택 | 면/셀 선택 |
| **좌클릭+드래그** | Box Select | Box Select |
| **중앙 드래그** | Orbit (회전) | Orbit (회전) |
| **Shift+중앙** | Pan (이동) | Pan (이동) |
| **스크롤** | Zoom | Zoom |
| **우클릭** | Context Menu | Context Menu |
| **더블클릭** | Zoom to Selection | Zoom to Selection |
| **F** | Zoom to Fit | Zoom to Fit |
| **1-6** | Front/Back/Top/Bottom/Left/Right | 동일 |
| **7** | Isometric View | 동일 |
| **W** | 이동 기즈모 | — |
| **E** | 회전 기즈모 | — |
| **R** | 스케일 기즈모 | — |
| **X/Y/Z** | 축 고정 | 절단면 축 |
| **Delete** | 객체 삭제 | — |
| **Ctrl+Z/Y** | Undo/Redo | Undo/Redo |
| **Ctrl+S** | 프로젝트 저장 | 프로젝트 저장 |
| **Space** | — | 솔버 시작/중지 토글 |

### 4.2 구현해야 할 3D 기능 목록

| 카테고리 | 기능 | 난이도 | 우선순위 |
|---------|------|--------|---------|
| **카메라** | Orbit/Pan/Zoom | 쉬움 | P0 |
| | View presets (6면 + iso) | 쉬움 | P0 |
| | Zoom to fit / selection | 쉬움 | P0 |
| | Perspective ↔ Orthographic 토글 | 쉬움 | P1 |
| | Camera animation (smooth transition) | 중간 | P2 |
| **선택** | 단일 객체 클릭 선택 | 중간 | P0 |
| | Box/Lasso 다중 선택 | 중간 | P1 |
| | 면/모서리/꼭짓점 선택 모드 전환 | 중간 | P1 |
| | 선택 하이라이트 (외곽선/색상) | 쉬움 | P0 |
| **변환** | 이동 기즈모 (3축) | 중간 | P0 |
| | 회전 기즈모 (3축) | 중간 | P1 |
| | 스케일 기즈모 | 중간 | P2 |
| | 그리드 스냅 | 중간 | P1 |
| **형상** | Box/Sphere/Cylinder 프리미티브 | 쉬움 | P0 |
| | STL/OBJ 임포트 + 렌더링 | 중간 | P0 |
| | Boolean 미리보기 (CSG) | 어려움 | P1 |
| | 면 돌출/회전 (Extrude/Revolve) | 어려움 | P2 |
| **시각화** | 와이어프레임 / 솔리드 토글 | 쉬움 | P0 |
| | 스칼라장 컬러맵 | 중간 | P0 |
| | 벡터 화살표 (Glyph) | 중간 | P0 |
| | 유선 (Streamline) | 어려움 | P1 |
| | 등가면 (Iso-surface) | 어려움 | P2 |
| | 절단면 (Clip plane) | 중간 | P0 |
| | 컬러바 / 범례 | 쉬움 | P0 |
| **기타** | 바닥 그리드 + 축 | 쉬움 | P0 |
| | 측정 도구 (거리/면적) | 중간 | P1 |
| | 스크린샷 내보내기 | 쉬움 | P0 |
| | 반투명 렌더링 | 중간 | P1 |
| | 조명 설정 (ambient/directional) | 쉬움 | P1 |

---

## 5. Rust 백엔드 서버 (gfd-server)

### 5.1 JSON-RPC 프로토콜

```rust
// 요청
{"id": 1, "method": "mesh.generate", "params": {"type": "cartesian", "nx": 20, "ny": 20}}

// 응답
{"id": 1, "result": {"cells": 400, "faces": 1640, "quality": {"min_ortho": 1.0}}}

// 이벤트 (서버 → 클라이언트, 비동기)
{"event": "solve.progress", "data": {"iteration": 35, "residual": 9.78e-5}}
```

### 5.2 메서드 목록

| 카테고리 | 메서드 | 설명 |
|---------|--------|------|
| **Geometry** | `cad.create_primitive` | Box/Sphere/Cylinder 생성 |
| | `cad.boolean` | Union/Subtract/Intersect |
| | `cad.import_stl` | STL 파일 임포트 |
| | `cad.get_geometry` | 렌더링용 삼각형 데이터 |
| **Mesh** | `mesh.generate` | 메시 생성 |
| | `mesh.quality` | 품질 메트릭 |
| | `mesh.refine` | 적응형 세분화 |
| | `mesh.get_display_data` | 렌더링용 메시 데이터 |
| **Setup** | `setup.validate` | 설정 유효성 검사 |
| | `setup.get_patches` | 경계 패치 목록 |
| **Solve** | `solve.start` | 솔버 시작 (비동기) |
| | `solve.stop` | 솔버 중지 |
| | `solve.status` | 진행 상태 |
| **Results** | `field.get` | 필드 데이터 (바이너리) |
| | `field.slice` | 절단면 데이터 |
| | `field.streamlines` | 유선 데이터 |
| | `report.forces` | 항력/양력 계산 |

---

## 6. 프로젝트 구조 (monorepo)

```
gui/
├── package.json                 # Workspace root
├── electron/                    # Electron 메인 프로세스
│   ├── main.ts
│   ├── preload.ts
│   └── updater.ts               # 자동 업데이트 (electron-updater)
├── packages/
│   ├── 3d-engine/               # 공유: Three.js 3D 엔진
│   ├── panel-system/            # 공유: 패널/트리/폼 UI
│   ├── ipc-bridge/              # 공유: Rust 통신
│   ├── state-manager/           # 공유: Zustand 상태
│   ├── cad-module/              # 탭: CAD
│   ├── mesh-module/             # 탭: Mesh
│   ├── setup-module/            # 탭: Numerical Setup
│   ├── calc-module/             # 탭: Calculation
│   └── results-module/          # 탭: Results
├── scripts/
│   ├── build.ts                 # 빌드 스크립트
│   └── package.ts               # 패키징 (Windows/Mac/Linux)
└── tsconfig.json
```

### 6.1 업데이트 전략

| 업데이트 대상 | 방법 |
|-------------|------|
| **Electron Shell** | electron-updater (GitHub Releases) |
| **개별 모듈** | 각 패키지 독립 빌드 → 동적 로딩 |
| **Rust 백엔드** | 별도 바이너리 업데이트 (자동 다운로드) |
| **솔버 모델** | 플러그인 형태로 동적 추가 |

---

## 7. 구현 우선순위

| 순서 | 작업 | 예상 규모 |
|------|------|----------|
| **1** | Electron + React 프로젝트 생성 | 1일 |
| **2** | 3D Engine (Viewport + Camera + Grid) | 2일 |
| **3** | Panel System (SplitLayout + Tree + PropertyGrid) | 2일 |
| **4** | IPC Bridge (JSON-RPC + gfd-server) | 1일 |
| **5** | Mesh 탭 (기본 메시 생성 + 품질) | 2일 |
| **6** | Setup 탭 (모델/BC/재질 편집) | 2일 |
| **7** | Calculation 탭 (실행/잔차 모니터) | 1일 |
| **8** | Results 탭 (컨투어/벡터/절단면) | 3일 |
| **9** | CAD 탭 (프리미티브/불리안/STL) | 3일 |
| **10** | 자동 업데이트 + 패키징 | 1일 |
