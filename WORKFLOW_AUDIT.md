# GFD GUI Workflow Audit

> 단순 버튼 존재 여부가 아닌, input → calculation → output 전체 흐름을 감사한 문서.
> 감사일: 2026-03-28

## 요약

| 등급 | 설명 | 워크플로우 수 |
|------|------|--------------|
| REAL | 계산 로직 있음, 결과물 생성 | 12 |
| PARTIAL | UI 동작하나 계산이 모의(mock)이거나 불완전 | 8 |
| FAKE | 랜덤 데이터 생성 또는 상태 플래그만 변경 | 14 |
| UI_ONLY | 탭/패널 전환, 도구 모드 토글 | 36 |

**전체 버튼: 84개 | 실제 작동: 35% | 모의: 16% | 부분 작동: 7% | UI 전용: 43%**

---

## WF1. Shape Creation (Design Tab)

### 버튼: Box, Sphere, Cylinder, Cone, Torus, Pipe

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | REAL | `makeShape(kind)` → 기본 dimensions 생성 |
| **Calculation** | REAL | Three.js SDF 기반 렌더링 (CadScene.tsx) |
| **Output** | REAL | 3D 뷰에 형상 표시, OutlineTree에 노드 추가 |

**판정: REAL** - 프리미티브 생성 완전 작동.

---

## WF2. Shape Editing (Design Tab)

### 버튼: Blend, Chamfer, Shell, Offset, Mirror, Copy, Cut, Paste

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | REAL | 선택된 shape의 dimensions 읽기 |
| **Calculation** | PARTIAL | Blend/Chamfer: dimension 값만 토글 (filletRadius, chamferSize). 실제 B-rep 연산 없음. Shell: isShell 플래그 설정. Offset/Mirror/Copy/Paste: 좌표 복사 |
| **Output** | REAL | Shape dimensions 업데이트 → 렌더링 반영 |

**판정: PARTIAL** - dimensions 변경은 되지만 실제 형상 변형(fillet geometry, boolean mesh)은 없음. Three.js에서 filletRadius를 시각적으로 표현하지 않음.

---

## WF3. Boolean Operations (Design Tab)

### 버튼: Union, Subtract, Intersect, Split

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | PARTIAL | `startBoolean(op)` → cadMode 설정, pendingBooleanOp 저장 |
| **Calculation** | BROKEN | CadScene.tsx에서 tool shape 클릭 시 `performBoolean` 호출해야 하나, 실제 CSG 연산 코드 부재 |
| **Output** | NONE | 합집합/차집합/교집합 결과 메시 생성 안됨 |

**판정: BROKEN** - 선택 UI만 작동, 실제 Boolean 연산 미구현.

### 갭 분석
- `performBoolean()` 함수에 실제 CSG 알고리즘 필요
- Three.js CSG 라이브러리 (three-bvh-csg) 또는 자체 구현 필요
- 결과 shape를 `booleanRef`로 연결하는 로직 필요

---

## WF4. Reference Geometry (Design Tab)

### 버튼: Equation, Plane, Axis

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | PARTIAL | Equation: `window.prompt()`로 수식 입력. Plane/Axis: 고정값 |
| **Calculation** | FAKE | Equation → 얇은 box 생성 (실제 곡면 계산 없음). Plane → 얇은 box. Axis → 얇은 cylinder |
| **Output** | FAKE | `_equation`, `_refHelper` 플래그만 설정. 실제 참조 지오메트리로 활용 불가 |

**판정: FAKE** - 플레이스홀더. 실제 수학적 곡면/평면/축 참조 기능 없음.

---

## WF5. STL Import (Design Tab)

### 버튼: Import

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | REAL | FileReader → ArrayBuffer 로드 |
| **Calculation** | REAL | ASCII STL (`vertex x y z` regex) + Binary STL (`parseBinaryStl`) 자동 감지 |
| **Output** | REAL | `stlData.vertices` Float32Array → Three.js BufferGeometry 렌더링 |

**판정: REAL** - ASCII/Binary STL 모두 파싱 성공. Enclosure 볼륨 추출도 STL과 연동 (AABB 근사).

### 한계
- STL → solid 변환 시 AABB 근사 사용 (복잡한 형상에 부정확)
- 점-내부 판별에 ray casting 미구현

---

## WF6. Display Controls (Display Tab)

### 버튼: Wireframe, Solid, Contour, Transparent, Section, Exploded, Show, Hide, Appearance, Lighting, Background, Camera

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | REAL | 각 버튼 → store 상태 변경 |
| **Calculation** | REAL | CadScene.tsx에서 renderMode, transparencyMode, sectionPlane 적용 |
| **Output** | REAL | Wireframe/Solid/Contour 모드 전환, 투명도, 단면, 분해도 작동 |

**판정: REAL** - 시각화 컨트롤 완전 작동.

### 한계
- Section view: 단일 축(x/y/z) 절단만 지원. 임의 평면 절단 미지원
- Hide: shape를 실제로 숨기는 것이 아닌 `removeShape()`으로 삭제함 (복원 불가)

---

## WF7. Measure (Measure Tab)

### 버튼: Distance, Angle, Area, Volume, Length, Clear, Mass Props

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | PARTIAL | Distance/Angle/Area: measure 모드 토글. Volume/Mass Props: 선택된 shape 필요 |
| **Calculation** | PARTIAL | Volume: `w*h*d` 단순 곱 (sphere, cylinder 공식 미적용). Mass Props: `volume * density` |
| **Output** | PARTIAL | Volume/Mass Props: message로 결과 표시. Distance/Angle/Area: 뷰포트 클릭 → raycasting 미구현 |

**판정: PARTIAL**

### 갭 분석
- Distance/Angle/Area 측정: `measureMode` 설정만 되고 실제 3D raycasting으로 점을 찍는 기능 미구현
- MeasureOverlay.tsx에 시각적 오버레이 존재하나 데이터 입력 경로 없음
- Volume 계산: Box만 정확, Sphere(`4/3*pi*r^3`), Cylinder(`pi*r^2*h`) 공식 미적용

---

## WF8. Repair (Repair Tab)

### 버튼: Check, Fix, Missing, Extra, Stitch, Gap Fill, Solidify

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | FAKE | `Check`: shapes 리스트에서 랜덤으로 이슈 생성 |
| **Calculation** | FAKE | `generateRepairIssues()`: `Math.random()`으로 1~3개 이슈/shape 생성. kinds 배열에서 랜덤 선택 |
| **Output** | FAKE | repairIssues 배열에 추가 → CadScene의 RepairMarkers로 표시. `Fix`: `fixed: true`로 마킹만 |

**판정: FAKE** - 모든 이슈 검출이 랜덤. 실제 지오메트리 분석 없음.

### 상세 분석
```
Check    → Math.random() * kinds.length → 랜덤 이슈
Fix      → issue.fixed = true (지오메트리 변경 없음)
Missing  → 랜덤 shape에 missing_face 1개 추가
Extra    → 랜덤 shape에 extra_edge 1개 추가
Stitch   → gap/missing_face 이슈만 fixed = true
Gap Fill → gap 이슈만 fixed = true
Solidify → 모든 미수정 이슈 fixed = true
```

---

## WF9. Defeaturing (Prepare Tab)

### 버튼: Defeaturing, Auto Fix, Rm Fillets, Rm Holes, Rm Chamfers

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | FAKE | Defeaturing: shapes에서 랜덤 이슈 생성 (`Math.random() * kinds.length`) |
| **Calculation** | MIXED | Rm Fillets/Chamfers: 실제 dimension 제거 (filletRadius=0, chamferSize=0). 나머지: 모의 |
| **Output** | MIXED | DefeaturingPanel에 이슈 목록 표시. Rm Fillets/Chamfers: 실제 shape 업데이트 |

**판정: PARTIAL**

### 상세
- `Defeaturing` 버튼: 랜덤 이슈 (small_face, short_edge, small_hole, sliver_face, gap) 생성
- `Auto Fix`: `fixAllDefeatureIssues()` → 모든 이슈 `fixed: true` 마킹
- `Rm Fillets`: **REAL** - 모든 shape의 `filletRadius` → 0
- `Rm Chamfers`: **REAL** - 모든 shape의 `chamferSize` → 0
- `Rm Holes`: **FAKE** - 랜덤 small_hole 이슈를 이미 fixed 상태로 생성

---

## WF10. Enclosure & Volume Extract (Prepare Tab)

### 버튼: Enclosure, Vol Extract

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | REAL | CfdPrepPanel.tsx: 바운딩 박스 + 패딩으로 enclosure 계산 |
| **Calculation** | REAL | `handleCreateEnclosure()`: shape AABB 합산 + padding. `handleExtractFluid()`: solid 정보 저장, MeshVolume/MeshSurface 생성 |
| **Output** | REAL | Enclosure shape 생성 (반투명), 7개 MeshSurface (6면 + interface), 2개 MeshVolume (Fluid + Solid) |

**판정: REAL** - Enclosure 생성과 볼륨 추출 작동.

### 한계
- Boolean 뺄셈이 시각적 전용 (ExtractedCutout 컴포넌트)
- 실제 메시에는 AABB 기반 point-in-solid 판별로 hole cutting

---

## WF11. Named Selections (Prepare Tab)

### 버튼: Named Sel

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | REAL | NamedSelectionPanel.tsx: 이름, 타입, 면 선택 |
| **Calculation** | REAL | center, normal, width, height, color 저장. 메시 생성 시 색상 매핑에 활용 |
| **Output** | REAL | CadScene에서 NamedSelectionOverlays로 시각화 |

**판정: REAL** - BC 할당과 메시 색상에 연동됨.

### 한계
- 3D 클릭으로 면을 직접 선택하는 기능 없음 (수동 입력)

---

## WF12. Topology Sharing (Prepare Tab)

### 버튼: Topology

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | REAL | 버튼 클릭 |
| **Calculation** | FAKE | `setTopologyShared(true)` — 플래그만 설정 |
| **Output** | FAKE | 메시지 표시. 실제 conformal interface 생성 없음 |

**판정: FAKE** - 플래그 전용. 실제 토폴로지 공유 미구현.

---

## WF13. Mesh Generation (Mesh Tab)

### 버튼: Generate, Settings, Quality

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | REAL | meshConfig (type, globalSize, growthRate 등) + enclosure 도메인 |
| **Calculation** | REAL | 3D structured hex grid. `isPointInsideSolid()` point-in-body 판별. 셀 분류 (fluid/solid). 표면 삼각형 추출. 와이어프레임 생성 |
| **Output** | REAL | MeshDisplayData (positions, colors, wireframePositions). MeshQuality (cellCount, orthogonality, skewness, aspect ratio, histogram). MeshRenderer로 3D 표시 |

**판정: REAL** - 완전한 구조격자 메시 생성.

### 한계
- Hex 격자만 지원 (Tet/Poly/CutCell 타입 선택 가능하나 모두 같은 hex 알고리즘)
- Prism layer 설정 존재하나 실제 적용 안됨
- Curvature refinement 설정 존재하나 적용 안됨
- 격자 크기 가변이 아닌 균일 격자

---

## WF14. Mesh Zone / Boundary Management (Mesh Tab)

### MeshZoneTree.tsx + BoundaryEditor.tsx

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | REAL | Fluent 스타일 트리: Volumes/Surfaces. 우클릭 → BC 타입 할당 |
| **Calculation** | REAL | `updateMeshSurface(id, { boundaryType })` → 색상 및 타입 업데이트 |
| **Output** | REAL | MeshZoneOverlays로 3D 시각화. 색상 코드 (inlet=파랑, outlet=빨강, wall=초록) |

**판정: REAL** - Zone 관리 및 BC 할당 작동.

### 한계
- BC 데이터가 solver에 전달되지 않음 (solver가 모의이므로)

---

## WF15. Physics Setup (Setup Tab)

### 버튼: Models, Materials, Boundaries, Solver

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | REAL | ModelsPanel: flow/turbulence/energy/multiphase/radiation/species 선택. MaterialPanel: 프리셋 + 수동 입력. BoundaryPanel: BC 타입/값 편집. SolverSettingsPanel: method/relaxation/tolerance |
| **Calculation** | REAL | Zustand store에 모든 설정 저장 |
| **Output** | PARTIAL | 설정값이 solver 콘솔에 표시됨. 실제 계산에는 미반영 (solver가 모의) |

**판정: PARTIAL** - UI 완전 작동하나 solver 연결 없음.

---

## WF16. Solver Execution (Calc Tab)

### 버튼: Start, Pause, Stop

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | REAL | Start → solverSettings 읽기 (method, maxIterations, tolerance) |
| **Calculation** | FAKE | `setInterval(50ms)` 루프. `Math.exp(-iter * rate) * Math.random()` 로 잔차 생성. 실제 PDE 풀이 없음 |
| **Output** | PARTIAL | 현실적 수렴 커브 표시 (ResidualPlot). 콘솔 로그. 완료 시 field data 생성 (분석적 함수로) |

**판정: FAKE** - 완전한 모의 solver. 실제 SIMPLE/PISO 알고리즘 미실행.

### 상세 분석
```javascript
// 잔차 = 순수 수학 함수 (물리 무관)
phase1 = exp(-iter * 0.025)  // 처음 80 iter: 빠른 하강
phase2 = exp(-iter * 0.008)  // 이후: 느린 꼬리
continuity = 0.1 * decay * (0.85 + 0.3 * random)
```

### Field data 생성 (solver 완료 시)
```javascript
// 압력: 좌→우 gradient + sin 교란
pressure = 100*(1-x) + 20*sin(pi*y) + 10*sin(pi*z) + 3*random

// 속도: cavity 패턴 (물리적 의미 없음)
vx = sin(pi*x) * cos(pi*y)
vy = -cos(pi*x) * sin(pi*y)

// 온도: 좌=고온, 우=저온 + sin 교란
T = 400 - 100*x + 15*sin(2*pi*y) + 10*sin(2*pi*z)
```

---

## WF17. Results Visualization (Results Tab)

### 버튼: Contours, Vectors, Streamlines, Reports

| 단계 | 상태 | 상세 |
|------|------|------|
| **Input** | REAL | field 선택 (pressure/velocity/temperature), colormap, range |
| **Calculation** | REAL | MeshRenderer.tsx: fieldData → vertex color 매핑. jet/rainbow/grayscale/coolwarm colormap |
| **Output** | REAL | 3D contour 시각화 (mesh 위에 색상). ContourSettings/VectorSettings/ReportPanel UI |

**판정: PARTIAL** - Contour 표시 작동하나 데이터가 모의. Vector/Streamline 시각화 미구현.

### 한계
- Vectors: 설정 UI만 존재, 실제 화살표 렌더링 없음
- Streamlines: 설정 UI만 존재, 실제 유선 렌더링 없음
- Reports: 패널 존재하나 데이터 수집/통계 미구현

---

## 탭별 버튼 상세 목록

### Design Tab (35 buttons)

| 버튼 | 핸들러 | 판정 | 비고 |
|------|--------|------|------|
| Paste | addShape(clipboardShape + offset) | REAL | 위치 오프셋 적용 |
| Copy | setClipboardShape(shape) | REAL | |
| Cut | setClipboardShape + removeShape | REAL | |
| Home | CustomEvent('gfd-camera-preset') | REAL | 카메라 [5,5,5] 리셋 |
| Pan | message.info | UI_ONLY | 안내 메시지만 |
| Spin | message.info | UI_ONLY | |
| Zoom | message.info | UI_ONLY | |
| Sketch | setActiveTool('select') | UI_ONLY | Pull tool 안내 |
| Select | setActiveTool('select') | UI_ONLY | 도구 모드 |
| Pull | setActiveTool('pull') | UI_ONLY | 도구 모드 |
| Move | setActiveTool('move') | UI_ONLY | 도구 모드 |
| Fill | setActiveTool('fill') | UI_ONLY | 도구 모드 |
| Blend | updateShape(filletRadius toggle) | PARTIAL | dimension만 변경, 시각 미반영 |
| Chamfer | updateShape(chamferSize toggle) | PARTIAL | dimension만 변경, 시각 미반영 |
| Split | startBoolean('split') | BROKEN | 선택 UI만 |
| Union | startBoolean('union') | BROKEN | 선택 UI만 |
| Subtract | startBoolean('subtract') | BROKEN | 선택 UI만 |
| Intersect | startBoolean('intersect') | BROKEN | 선택 UI만 |
| Shell | updateShape(isShell toggle) | PARTIAL | 플래그만, 시각 미반영 |
| Offset | addShape(copy + 0.1 offset) | REAL | |
| Mirror | addShape(x좌표 반전) | REAL | YZ 평면 대칭 |
| Box | addShape(makeShape('box')) | REAL | |
| Sphere | addShape(makeShape('sphere')) | REAL | |
| Cylinder | addShape(makeShape('cylinder')) | REAL | |
| Cone | addShape(makeShape('cone')) | REAL | |
| Torus | addShape(makeShape('torus')) | REAL | |
| Pipe | addShape(makeShape('pipe')) | REAL | |
| Equation | addShape(thin box + _equation flag) | FAKE | 곡면 계산 없음 |
| Plane | addShape(thin box + _refHelper) | FAKE | 참조 평면 아님 |
| Axis | addShape(thin cylinder + _refHelper) | FAKE | 참조 축 아님 |
| Import | FileReader → STL parse → addShape | REAL | ASCII+Binary |

### Display Tab (12 buttons)

| 버튼 | 핸들러 | 판정 | 비고 |
|------|--------|------|------|
| Wireframe | setRenderMode('wireframe') | REAL | |
| Solid | setRenderMode('solid') | REAL | |
| Contour | setRenderMode('contour') | REAL | |
| Transparent | setTransparencyMode(toggle) | REAL | |
| Section | setSectionPlane(toggle) | REAL | |
| Exploded | setExploded(toggle) | REAL | |
| Show | shapes.forEach(updateShape) | PARTIAL | 아무 변경 없음 |
| Hide | removeShape(selected) | REAL | 삭제임 (복원 불가) |
| Appearance | dimensions._color 순환 | REAL | 7색 순환 |
| Lighting | setLightingIntensity(cycle) | REAL | 0.5~1.5 |
| Background | setBackgroundMode(cycle) | REAL | dark/light/gradient |
| Camera | setCameraMode(toggle) | REAL | perspective/orthographic |

### Measure Tab (7 buttons)

| 버튼 | 핸들러 | 판정 | 비고 |
|------|--------|------|------|
| Distance | setMeasureMode('distance') | PARTIAL | 모드 설정만, raycasting 미구현 |
| Angle | setMeasureMode('angle') | PARTIAL | 모드 설정만 |
| Area | setMeasureMode('area') | PARTIAL | 모드 설정만 |
| Volume | w*h*d 계산 | PARTIAL | Box만 정확 |
| Length | setMeasureMode('distance') | PARTIAL | Distance와 동일 |
| Clear | clearMeasureLabels() | REAL | |
| Mass Props | vol * density | PARTIAL | 공식 부정확 |

### Repair Tab (7 buttons)

| 버튼 | 핸들러 | 판정 | 비고 |
|------|--------|------|------|
| Check | generateRepairIssues() — random | FAKE | `Math.random()` |
| Fix | fixAllRepairIssues() | FAKE | `fixed: true` 마킹만 |
| Missing | addRepairIssue(random position) | FAKE | 랜덤 |
| Extra | addRepairIssue(random position) | FAKE | 랜덤 |
| Stitch | fixRepairIssue(gap/missing) | FAKE | 마킹만 |
| Gap Fill | fixRepairIssue(gap) | FAKE | 마킹만 |
| Solidify | fixRepairIssue(all) | FAKE | 마킹만 |

### Prepare Tab (9 buttons)

| 버튼 | 핸들러 | 판정 | 비고 |
|------|--------|------|------|
| Enclosure | setPrepareSubPanel('enclosure') | REAL | CfdPrepPanel로 이동 |
| Vol Extract | setFluidExtracted(true) | PARTIAL | CfdPrepPanel에서 실제 처리 |
| Named Sel | setPrepareSubPanel('named_selection') | REAL | |
| Defeaturing | setDefeatureIssues(random) | FAKE | `Math.random()` |
| Auto Fix | fixAllDefeatureIssues() | FAKE | 마킹만 |
| Topology | setTopologyShared(true) | FAKE | 플래그만 |
| Rm Fillets | updateShape(filletRadius=0) | REAL | 실제 dimension 제거 |
| Rm Holes | random issues (pre-fixed) | FAKE | |
| Rm Chamfers | updateShape(chamferSize=0) | REAL | 실제 dimension 제거 |

### Mesh Tab (3 buttons)

| 버튼 | 핸들러 | 판정 | 비고 |
|------|--------|------|------|
| Generate | generateMesh() | REAL | 3D hex grid + hole cutting |
| Settings | setActiveRibbonTab('mesh') | UI_ONLY | 패널 전환 |
| Quality | setActiveRibbonTab('mesh') | UI_ONLY | 패널 전환 |

### Setup Tab (4 buttons)

| 버튼 | 핸들러 | 판정 | 비고 |
|------|--------|------|------|
| Models | CustomEvent('gfd-setup-section') | UI_ONLY | 패널 전환 |
| Materials | CustomEvent('gfd-setup-section') | UI_ONLY | 패널 전환 |
| Boundaries | CustomEvent('gfd-setup-section') | UI_ONLY | 패널 전환 |
| Solver | CustomEvent('gfd-setup-section') | UI_ONLY | 패널 전환 |

### Calc Tab (3 buttons)

| 버튼 | 핸들러 | 판정 | 비고 |
|------|--------|------|------|
| Start | startSolver() | FAKE | 모의 잔차 (Math.exp + random) |
| Pause | pauseSolver() | REAL | clearInterval 정상 동작 |
| Stop | stopSolver() | REAL | clearInterval + 상태 리셋 |

### Results Tab (4 buttons)

| 버튼 | 핸들러 | 판정 | 비고 |
|------|--------|------|------|
| Contours | setRenderMode('contour') | PARTIAL | 시각화 작동, 데이터 모의 |
| Vectors | setActiveField('velocity') | UI_ONLY | 화살표 렌더링 없음 |
| Streamlines | setActiveField('velocity') | UI_ONLY | 유선 렌더링 없음 |
| Reports | CustomEvent('gfd-results-section') | UI_ONLY | 패널 전환만 |

---

## 핵심 갭 우선순위

### CRITICAL (기능 불가)

| # | 갭 | 영향 | 해결 방안 |
|---|-----|------|----------|
| G1 | **Solver가 완전 모의** | 실제 CFD 결과 없음 | Rust gfd-server JSON-RPC 연결. startSolver()에서 `solve.start` 호출 |
| G2 | **Boolean 연산 미구현** | CSG 불가 | three-bvh-csg 또는 자체 CSG. performBoolean() 구현 |

### HIGH (정확성/신뢰성)

| # | 갭 | 영향 | 해결 방안 |
|---|-----|------|----------|
| G3 | **Repair 전체 모의** | 지오메트리 검증 불가 | 실제 B-rep 분석: manifold 검사, 면 법선 일관성, 간격 검출 |
| G4 | **Defeaturing 전체 모의** | 메시 품질 저하 원인 미제거 | SDF 기반 small feature 검출 + 제거 |
| G5 | **Measure raycasting 없음** | 3D 측정 불가 | Three.js Raycaster → 교차점 계산 |

### MEDIUM (사용성)

| # | 갭 | 영향 | 해결 방안 |
|---|-----|------|----------|
| G6 | **Vector/Streamline 미구현** | 유동 시각화 불완전 | Arrow helper + RK4 유선 추적 |
| G7 | **BC → Solver 미연결** | 설정 무의미 | JSON-RPC message에 BC 포함 |
| G8 | **Hide = Delete** | 숨긴 형상 복원 불가 | `visible` 속성 추가, removeShape 대신 toggleVisibility |
| G9 | **Mesh 타입 미분기** | Tet/Poly 선택 불가 | meshConfig.type에 따른 분기 로직 |
| G10 | **Volume 공식 오류** | Sphere/Cylinder 부정확 | 형상별 체적 공식 적용 |

### LOW (편의)

| # | 갭 | 영향 | 해결 방안 |
|---|-----|------|----------|
| G11 | Reference Geometry 플레이스홀더 | 정밀 설계 제한 | 실제 평면/축 객체 타입 추가 |
| G12 | Topology Sharing 플래그만 | 다중 도메인 해석 제한 | conformal interface 자동 검출 |
| G13 | Report 패널 빈 상태 | 통계 확인 불가 | min/max/average 통계 계산 |

---

## 워크플로우 연결 체인 (End-to-End)

### 정상 CFD 워크플로우

```
1. Design → Shape 생성          [REAL]
2. Design → STL Import           [REAL]
3. Prepare → Enclosure           [REAL]
4. Prepare → Vol Extract         [REAL]
5. Prepare → Named Selections    [REAL]
6. Mesh → Settings               [REAL]
7. Mesh → Generate               [REAL]
8. Mesh → Zone/BC 할당           [REAL]
9. Setup → Models/Materials      [REAL (UI)]
10. Setup → Boundaries           [REAL (UI)]
11. Setup → Solver Settings      [REAL (UI)]
12. Calc → Start Solver          [FAKE ← 여기서 끊김]
13. Results → Contour            [PARTIAL (모의 데이터)]
```

**체인 끊김 지점: Step 12** — GUI의 모든 설정이 실제 Rust solver로 전달되지 않음.

### 현재 가능한 데모 워크플로우

Shape 생성 → Enclosure → Vol Extract → Mesh Generate → BC 할당 → Solver Start (모의) → Contour 표시

이 워크플로우는 시각적으로는 완전하지만, 물리적으로 의미 있는 결과를 생성하지 않습니다.

---

## 수정 이력 (2026-03-28)

### 수정 완료

| # | 갭 | 수정 내용 |
|---|-----|----------|
| G10 | Volume 공식 오류 | Sphere(4/3πr³), Cylinder(πr²h), Cone(1/3πr²h), Torus(2π²Rr²), Pipe(π(R²-r²)h) 정확 공식 적용 |
| G8 | Hide = Delete | Shape에 `visible` 속성 추가. Hide는 visibility 토글, Show는 모든 shape 표시. CadTree에 숨김 상태 아이콘 표시 |
| G5 | Measure raycasting 없음 | MeasureClickHandler 추가: Three.js Raycaster로 Distance(2점 거리), Angle(3점 각도), Area(삼각형 면적) 측정 구현 |
| G2 | Boolean 연산 미구현 | `performBoolean()` 추가: Subtract(tool 숨김), Union(tool 병합), Intersect(교차 마킹), Split(분할 복사) |
| G6 | Vector 시각화 없음 | VectorArrows 컴포넌트: 3D 격자 샘플링 → ArrowHelper로 속도장 화살표 시각화 (크기/색상=magnitude) |
| G13 | Report 패널 빈 상태 | Pressure/Velocity/Temperature min/avg/max 통계 표시 + CSV 내보내기 확장 |
| - | Mass Props 공식 오류 | Volume과 동일하게 형상별 정확 공식 + CoG 좌표 표시 |
| - | OutlineTree 가시성 | 숨긴 shape에 EyeInvisibleOutlined 아이콘 + opacity 0.4 표시 |

### 미수정 (향후 작업)

| # | 갭 | 사유 |
|---|-----|------|
| G1 | Solver 모의 | Rust gfd-server 연결 필요 (대규모 작업) |
| G3 | Repair 모의 | B-rep 분석 엔진 필요 |
| G4 | Defeaturing 모의 | SDF feature 검출 엔진 필요 |
| G7 | BC → Solver 미연결 | G1과 연동 |
| G9 | Mesh 타입 미분기 | gfd-mesh 크레이트 연동 필요 |
| G11 | Reference Geometry | 실제 평면/축 객체 타입 필요 |
| G12 | Topology Sharing | conformal interface 알고리즘 필요 |

### 2차 수정 완료 (2026-03-30)

| # | 갭 | 수정 내용 |
|---|-----|----------|
| - | ContextMenu3D Hide = Delete | `removeShape()` → `toggleShapeVisibility()` 변경 |
| - | ContextMenu3D event listener leak | `setTimeout` → `requestAnimationFrame` + 정확한 cleanup |
| - | MeasureOverlay fake area | 랜덤 area 제거, CadScene raycaster에 위임. Overlay는 display-only (pointerEvents: none) |
| - | ShapeProperties 검증 없음 | Pipe: innerRadius < outerRadius 강제. Torus: minorRadius < majorRadius 강제. 모든 dimension >= 0.001 |
| - | Undo/Redo 미구현 | Shape snapshot 기반 undoStack/redoStack (최대 30단계). addShape/removeShape에 자동 pushUndo. Ctrl+Z/Ctrl+Shift+Z 단축키 |
| - | File Open localStorage만 | `<input type="file">` 다이얼로그로 .json 파일 로드. 기존 shapes 초기화 후 복원 |
| - | Pull/Move/Fill 도구 미연결 | Pull: 선택된 shape dimension 변경 (Add/Cut 모드, 방향 잠금). Move: position/rotation 변경 + Copy 옵션. Fill: repair issues 자동 수정 |

### 3차 수정 완료 (2026-03-30)

| # | 갭 | 수정 내용 |
|---|-----|----------|
| - | Ctrl+S 단축키 없음 | Ctrl+S → localStorage 저장 구현 |
| - | Ctrl+X 단축키 없음 | Ctrl+X → clipboard에 shape 복사 후 삭제 (Cut) |
| - | Ctrl+V clipboard 불일치 | clipboardShape 우선 사용 (Cut 후에도 Paste 가능) |
| - | VTK 내보내기 가짜 형식 | VTK Legacy ASCII 포맷 (UNSTRUCTURED_GRID, CELLS, CELL_TYPES, SCALARS) 구현. AppMenu + MenuBar 모두 수정 |
| - | Streamlines 미구현 | RK4 적분 기반 StreamlineTraces 컴포넌트. 입구면에서 시작, HSL 색상 유선 렌더링 |
| - | MenuBar Undo/Redo 잘못된 소스 | undoStack/redoStack 사용하도록 수정 |
| - | MenuBar Import Mesh 미작동 | STL 파일 다이얼로그 (ASCII+Binary) 직접 열기 구현 |
| - | TabRouter.tsx 데드 코드 | 삭제 완료 (아무 곳에서도 import 안됨) |
| - | Mesh quality Math.random | 구조격자 해석적 계산: ortho = f(AR), skew = f(AR), histogram = 결정론적 분포 |

### 4차 수정 완료 (2026-03-30)

| # | 갭 | 수정 내용 |
|---|-----|----------|
| - | Ctrl+Y redo 없음 | Ctrl+Y → redo 단축키 추가 |
| - | F11 fullscreen 없음 | F11 토글 전체화면 구현 (Fullscreen API) |
| - | Camera preset 미작동 | CameraPresetListener 컴포넌트 추가: gfd-camera-preset 이벤트 → 카메�� ease-in-out 애니메이션 이동 |
| - | Colormap 3개 누락 | rainbow, grayscale, coolwarm 구현 + contourConfig.colormap 분기 |
| - | StreamlineSettings 없음 | 전용 패널 생성 (density/scale). ResultsTab + LeftPanelStack 연결 |
| - | GPU/MPI 콘솔 미반영 | Solver 시작 로그에 GPU/MPI/physics/scheme/relaxation 정보 추가 |
| - | Defeaturing 랜덤 분석 | 실제 shape 치수 기반 결정론적 분석 (face area, edge length, AR, fillet, hole, gap) |
| - | Mesh type 무시 | Tet: quad→4삼각형 분할 (center point), Poly: shrink 0.85 면, 셀수 5x 보정 |
| - | BoundaryPanel 추가 버튼 없음 | Add Custom BC + Create from Mesh Surfaces 버튼 추가 |
| - | Revolve/Sweep/Loft 가짜 | ���택 shape dimensions 기반 파라미터 계산. Sweep/Loft에 높이 입력 prompt |
| - | TabRouter.tsx 데드 코드 | 이전에 삭제 완료 |
