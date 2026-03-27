# GFD CAD 모듈 — SpaceClaim/FreeCAD 수준 형상 처리 설계

> **목표:** CFD 전처리에 필요한 모든 형상 생성/편집/수정/디피처링 기능
> **참조:** ANSYS SpaceClaim, FreeCAD, SolidWorks, Onshape, CATIA
> **핵심:** 메시 생성에 적합한 "깨끗한 형상"을 만드는 것이 최종 목적

---

## 1. 기능 분류 총괄

| 카테고리 | 기능 수 | 설명 |
|---------|--------|------|
| **형상 생성 (Create)** | 15 | 프리미티브, 스케치, 돌출, 회전 |
| **형상 편집 (Edit)** | 12 | 이동, 회전, 스케일, 미러, 패턴 |
| **불리안 연산 (Boolean)** | 4 | 합집합, 차집합, 교집합, 분할 |
| **디피처링 (Defeaturing)** | 14 | 작은 면/모서리/구멍 제거 |
| **면/모서리 처리 (Surface)** | 10 | 필렛, 챔퍼, 면 연장, 스티칭 |
| **형상 분석 (Analysis)** | 8 | 간섭체크, 거리측정, 면적/체적 |
| **형상 가져오기 (Import)** | 6 | STL, STEP, IGES, OBJ, Parasolid |
| **CFD 전용 (CFD Prep)** | 8 | 유동영역 추출, 캡핑, 래핑 |
| **합계** | **77개 기능** | |

---

## 2. 형상 생성 (Create) — 15개

### 2.1 3D 프리미티브 (Primitives)

| # | 기능 | 파라미터 | SpaceClaim 대응 |
|---|------|---------|----------------|
| 1 | **Box** | width, height, depth, position | ✅ Box |
| 2 | **Sphere** | radius, center | ✅ Sphere |
| 3 | **Cylinder** | radius, height, axis | ✅ Cylinder |
| 4 | **Cone** | radius_top, radius_bottom, height | ✅ Cone |
| 5 | **Torus** | major_radius, minor_radius | ✅ Torus |
| 6 | **Wedge** | width, height, depth, x_offset | Wedge/Prism |
| 7 | **Pipe** | inner_radius, outer_radius, height | Hollow cylinder |

### 2.2 스케치 기반 (Sketch-Based)

| # | 기능 | 설명 | SpaceClaim 대응 |
|---|------|------|----------------|
| 8 | **Extrude** | 2D 프로파일을 직선 방향으로 돌출 | ✅ Pull |
| 9 | **Revolve** | 2D 프로파일을 축 중심으로 회전 | ✅ Revolve |
| 10 | **Sweep** | 2D 프로파일을 경로를 따라 이동 | ✅ Sweep |
| 11 | **Loft** | 여러 2D 프로파일을 연결하여 3D 생성 | ✅ Loft |

### 2.3 스케치 도구 (2D Sketch)

| # | 기능 | 설명 |
|---|------|------|
| 12 | **Line** | 두 점을 잇는 직선 |
| 13 | **Arc** | 원호 (3점 또는 중심+반지름+각도) |
| 14 | **Circle** | 원 (중심+반지름) |
| 15 | **Spline** | B-스플라인 커브 (제어점) |

---

## 3. 형상 편집 (Edit) — 12개

| # | 기능 | 설명 | 단축키 |
|---|------|------|--------|
| 16 | **Move** | 선택 객체 이동 (XYZ 방향) | W |
| 17 | **Rotate** | 선택 객체 회전 (축 지정) | E |
| 18 | **Scale** | 균일/비균일 스케일 | R |
| 19 | **Mirror** | 평면 기준 대칭 복사 | M |
| 20 | **Linear Pattern** | X/Y/Z 방향 반복 복사 (배열) | |
| 21 | **Circular Pattern** | 축 중심 원형 반복 복사 | |
| 22 | **Copy** | 객체 복제 | Ctrl+C |
| 23 | **Split Body** | 평면으로 바디 분할 | |
| 24 | **Merge Bodies** | 여러 바디를 하나로 병합 | |
| 25 | **Shell** | 솔리드를 속이 빈 껍질로 변환 (두께 지정) | |
| 26 | **Offset Surface** | 면을 법선 방향으로 오프셋 | |
| 27 | **Thicken** | 면에 두께를 주어 솔리드 생성 | |

---

## 4. 불리안 연산 (Boolean) — 4개

| # | 기능 | 수식 | 설명 |
|---|------|------|------|
| 28 | **Union (합집합)** | A ∪ B | 두 바디를 하나로 합침 |
| 29 | **Subtract (차집합)** | A - B | A에서 B 형상을 빼냄 |
| 30 | **Intersect (교집합)** | A ∩ B | 두 바디의 겹치는 부분만 남김 |
| 31 | **Split (분할)** | A / B | B의 면으로 A를 잘라서 두 개로 분리 |

---

## 5. 디피처링 (Defeaturing) — 14개 ⭐ CFD 핵심

> **목적:** CAD 모델에는 메시 생성을 어렵게 하는 작은 형상들이 많음.
> 이를 자동/반자동으로 제거하여 메시 품질을 높임.

### 5.1 작은 형상 제거

| # | 기능 | 설명 | 판단 기준 |
|---|------|------|----------|
| 32 | **Remove Small Faces** | 면적이 threshold 이하인 면 제거 | area < min_area |
| 33 | **Remove Short Edges** | 길이가 threshold 이하인 모서리 제거 | length < min_edge |
| 34 | **Remove Slivers** | 가늘고 긴 삼각형/면 제거 | aspect_ratio > max_ar |
| 35 | **Remove Small Holes** | 작은 구멍(관통/비관통) 메우기 | diameter < max_hole_dia |
| 36 | **Remove Fillets** | 작은 라운딩(필렛) 제거하여 직각으로 | radius < max_fillet_r |
| 37 | **Remove Chamfers** | 작은 챔퍼 제거 | size < max_chamfer |
| 38 | **Remove Bosses** | 돌출된 작은 돌기 제거 | height < threshold |
| 39 | **Remove Pockets** | 작은 오목한 홈 제거 | depth < threshold |

### 5.2 형상 단순화

| # | 기능 | 설명 |
|---|------|------|
| 40 | **Collapse Short Edges** | 짧은 모서리의 두 꼭짓점을 하나로 합침 |
| 41 | **Merge Adjacent Faces** | 같은 평면/곡면 위의 인접 면들을 하나로 합침 |
| 42 | **Simplify Curves** | 복잡한 스플라인을 직선/원호로 근사 |
| 43 | **Replace with Primitive** | 복잡한 형상을 가장 가까운 프리미티브로 교체 |
| 44 | **Auto Defeaturing** | 위 모든 디피처링을 자동 적용 (threshold 기반) |
| 45 | **Interactive Defeaturing** | 문제 형상을 하이라이트 → 클릭으로 하나씩 수정 |

### 5.3 디피처링 워크플로우

```
1. [Analyze] 형상 분석 → 문제 형상 목록 생성
   - 작은 면: 23개 (< 0.1 mm²)
   - 짧은 모서리: 45개 (< 0.05 mm)
   - 작은 구멍: 8개 (직경 < 2 mm)
   - 가느다란 면: 12개 (AR > 100)

2. [Preview] 문제 형상을 3D에서 빨간색으로 하이라이트

3. [Select] 수정할 항목 선택 (전체/부분)

4. [Apply] 선택된 디피처링 적용

5. [Verify] 결과 확인 → 문제 남아있으면 반복
```

---

## 6. 면/모서리 처리 (Surface Operations) — 10개

| # | 기능 | 설명 |
|---|------|------|
| 46 | **Fillet (라운딩)** | 모서리에 라운드 적용 (반지름 지정) |
| 47 | **Chamfer (모따기)** | 모서리에 직선 모따기 (거리 지정) |
| 48 | **Extend Face** | 면의 경계를 연장 |
| 49 | **Trim Face** | 면을 커브/면으로 잘라냄 |
| 50 | **Stitch Surfaces** | 분리된 면들을 하나의 솔리드로 봉합 |
| 51 | **Extract Surface** | 솔리드에서 특정 면만 추출 |
| 52 | **Imprint** | 바디 위에 커브/면의 흔적을 새김 (면 분할용) |
| 53 | **Project Curve** | 커브를 면 위에 투영 |
| 54 | **Delete Face** | 특정 면 삭제 (열린 면 생성) |
| 55 | **Cap Opening** | 열린 면의 구멍을 막음 |

---

## 7. 형상 분석 (Analysis) — 8개

| # | 기능 | 출력 |
|---|------|------|
| 56 | **Interference Check** | 두 바디의 간섭(겹침) 영역 탐지 |
| 57 | **Gap Detection** | 바디 사이의 틈새 탐지 (거리 측정) |
| 58 | **Distance Measure** | 두 점/면/모서리 사이 최소 거리 |
| 59 | **Angle Measure** | 두 면/모서리 사이 각도 |
| 60 | **Area Measure** | 선택 면의 면적 |
| 61 | **Volume Measure** | 선택 바디의 체적 |
| 62 | **Mass Properties** | 질량, 무게중심, 관성모멘트 |
| 63 | **Surface Quality** | 면 곡률 분포, 법선 일관성 체크 |

---

## 8. 형상 가져오기/내보내기 (Import/Export) — 6개

| # | 포맷 | 방향 | 설명 |
|---|------|------|------|
| 64 | **STL** | Import/Export | 삼각형 메시 (가장 흔함) |
| 65 | **STEP (AP214)** | Import/Export | B-Rep 표준 포맷 |
| 66 | **IGES** | Import | 구형 CAD 호환 포맷 |
| 67 | **OBJ** | Import/Export | 폴리곤 메시 |
| 68 | **Parasolid (.x_t)** | Import | Siemens NX 호환 |
| 69 | **BREP (.brep)** | Import/Export | OpenCascade 네이티브 |

---

## 9. CFD 전용 기능 (CFD Prep) — 8개

| # | 기능 | 설명 |
|---|------|------|
| 70 | **Extract Fluid Region** | 솔리드 바디 주변의 유동 영역(음각) 자동 생성 |
| 71 | **Create Enclosure** | 형상 주위에 외부 유동 도메인 생성 (Box/Sphere 바운딩) |
| 72 | **Cap Inlets/Outlets** | 파이프 열린 끝을 평면으로 막기 (경계면 생성) |
| 73 | **Surface Wrap** | 복잡한 형상을 단순한 닫힌 표면으로 감싸기 |
| 74 | **Name Regions** | 면/바디에 이름 부여 (경계조건 매핑용) |
| 75 | **Split by Angle** | 면을 곡률 각도 기준으로 분할 (wall/inlet/outlet 구분) |
| 76 | **Symmetry Cut** | 대칭면으로 절단하여 반만 사용 (계산량 절감) |
| 77 | **Interference Region** | 두 바디 사이의 접촉/간섭 영역에 면 생성 (CHT용) |

---

## 10. 구현 우선순위

### Phase 1 (필수 — CFD 최소 요구)
| 우선순위 | 기능 | 번호 |
|---------|------|------|
| P0 | Box, Sphere, Cylinder, Cone | 1-4 |
| P0 | Move, Rotate, Scale | 16-18 |
| P0 | Union, Subtract, Intersect | 28-30 |
| P0 | STL Import/Export | 64 |
| P0 | Create Enclosure | 71 |
| P0 | Name Regions | 74 |

### Phase 2 (디피처링 — 실무 필수)
| 우선순위 | 기능 | 번호 |
|---------|------|------|
| P1 | Remove Small Faces/Edges | 32-33 |
| P1 | Remove Small Holes | 35 |
| P1 | Remove Fillets/Chamfers | 36-37 |
| P1 | Auto Defeaturing | 44 |
| P1 | Interactive Defeaturing | 45 |
| P1 | Cap Openings | 55, 72 |

### Phase 3 (고급 형상)
| 우선순위 | 기능 | 번호 |
|---------|------|------|
| P2 | Extrude, Revolve, Sweep, Loft | 8-11 |
| P2 | Sketch tools (Line, Arc, Circle) | 12-14 |
| P2 | Fillet, Chamfer | 46-47 |
| P2 | Mirror, Pattern | 19-21 |
| P2 | Shell, Thicken | 25, 27 |

### Phase 4 (분석/고급)
| 우선순위 | 기능 | 번호 |
|---------|------|------|
| P3 | Interference/Gap Detection | 56-57 |
| P3 | Measurements | 58-61 |
| P3 | Surface Wrap | 73 |
| P3 | STEP Import | 65 |
| P3 | Extract Fluid Region | 70 |

---

## 11. 기술 스택 및 구현 방식

### B-Rep (경계 표현) 커널
```
Option A: OpenCascade (OCCT) — C++ 라이브러리, Rust FFI
  장점: 산업 표준, STEP/IGES 지원, 정확한 불리안
  단점: 거대한 의존성, FFI 복잡

Option B: 자체 SDF 기반 — 순수 Rust
  장점: 의존성 없음, 빠름, 메시 생성에 최적
  단점: 정확한 B-Rep 없음, STEP 불가

Option C: 하이브리드
  - 프리미티브/불리안: SDF 기반 (Rust)
  - STEP/디피처링: OpenCascade FFI (optional feature)
  - STL: 직접 구현 (이미 있음)

→ 추천: Option C (하이브리드)
```

### 디피처링 알고리즘
```
Small Face Detection:
  for each face in model:
    if face.area() < threshold:
      candidates.push(face)

Short Edge Detection:
  for each edge in model:
    if edge.length() < threshold:
      candidates.push(edge)

Small Hole Detection:
  for each closed_loop in model.internal_loops():
    if loop.is_circular() and loop.diameter() < threshold:
      holes.push(loop)

Fillet Detection:
  for each face in model:
    if face.is_cylindrical() and face.radius() < threshold:
      fillets.push(face)

Auto Defeaturing:
  1. Detect all small features
  2. Sort by size (smallest first)
  3. Remove iteratively, checking topology after each
  4. Stop when no more features below threshold
```

---

## 12. GUI 통합

### CAD 탭 레이아웃 (개선)

```
┌──────────────────────────────────────────────────────────────┐
│ [Create ▼][Edit ▼][Boolean ▼][Defeaturing ▼][Analysis ▼]    │
├──────┬──────────────────────────────────────┬───────────────┤
│      │                                      │               │
│  B   │         3D Viewport                  │  Properties   │
│  o   │                                      │               │
│  d   │  ┌──────────────────────────────┐    │  Shape: Box-1 │
│  y   │  │                              │    │  Width: 1.0   │
│      │  │   [기즈모] 형상 편집 모드   │    │  Height: 1.0  │
│  T   │  │   불리안 미리보기            │    │  Depth: 1.0   │
│  r   │  │   디피처링 하이라이트       │    │  Pos: 0,0,0   │
│  e   │  │                              │    │               │
│  e   │  └──────────────────────────────┘    │  ┌─────────┐  │
│      │                                      │  │Defeaturing│ │
│  📁  ├──────────────────────────────────────┤  │Small faces│ │
│  Body│  Issues: ⚠ 23 small faces           │  │: 23      │ │
│  📁  │          ⚠ 45 short edges           │  │Short edges│ │
│  Feat│          ⚠ 8 small holes            │  │: 45      │ │
│  ure │  [Auto Fix All] [Fix Selected]       │  │[Fix All] │ │
└──────┴──────────────────────────────────────┴───────────────┘
```

### 디피처링 패널 상세

```
┌─ Defeaturing Analysis ───────────────────┐
│                                           │
│  Threshold Settings:                      │
│  ├─ Min Face Area:    [0.1    ] mm²      │
│  ├─ Min Edge Length:  [0.05   ] mm       │
│  ├─ Max Hole Diameter:[2.0    ] mm       │
│  └─ Max Fillet Radius:[1.0    ] mm       │
│                                           │
│  [🔍 Analyze]                            │
│                                           │
│  Found Issues:          Actions:          │
│  ├─ 🔴 Small faces: 23  [Fix All]       │
│  ├─ 🟡 Short edges: 45  [Fix All]       │
│  ├─ 🔵 Small holes:  8  [Fill All]      │
│  ├─ 🟢 Fillets:     12  [Remove All]    │
│  └─ 🟣 Chamfers:     3  [Remove All]    │
│                                           │
│  Total: 91 issues                         │
│  [Auto Fix All] [Preview] [Undo]         │
└───────────────────────────────────────────┘
```
