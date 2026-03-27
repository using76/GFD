# GFD CAD 모듈 — SpaceClaim 완전 재현 설계서

> **참조:** ANSYS SpaceClaim 2024 R2 User Guide
> **스크린샷 분석:** engine_configurations 어셈블리 (alternator, flywheel, manifold 등)
> **목표:** SpaceClaim의 모든 탭/리본/기능을 GFD GUI에 1:1 재현

---

## 1. SpaceClaim UI 구조 분석 (스크린샷 기반)

### 1.1 메뉴바 (Menu Bar) — 16개 탭

```
File | Design | Display | Assembly | Measure | Facets | Repair | Prepare |
Workbench | Detail | Sheet Metal | Tools | KeyShot | RS/Allied | Momentum | Gear
```

GFD에서 구현할 탭 (10개 — CFD 관련 중심):

| SpaceClaim 탭 | GFD 대응 | 우선순위 |
|-------------|---------|---------|
| **File** | File (열기/저장/가져오기/내보내기) | P0 |
| **Design** | Design (핵심 모델링 도구) | P0 |
| **Display** | Display (뷰/렌더링 설정) | P1 |
| **Assembly** | Assembly (파트 조립) | P2 |
| **Measure** | Measure (거리/각도/면적 측정) | P1 |
| **Facets** | Facets (STL/메시 편집) | P1 |
| **Repair** | Repair (형상 수리) | P0 |
| **Prepare** | Prepare (시뮬레이션 전처리) | P0 |
| **Tools** | Tools (설정/스크립팅) | P2 |
| **Detail** | Detail (치수/주석) | P3 |

### 1.2 리본 (Ribbon) 구조 — Design 탭 기준

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Clipboard │ Orient    │ Mode   │ Select│Pull│Move│Fill│ Edit │Intersect│
│           │           │        │       │    │    │    │      │         │
│  Paste    │ Home      │ Sketch │  ↖    │ ↕  │ ↔  │ ▢  │      │SplitBody│
│           │ Plan View │        │       │    │    │    │      │Split    │
│           │ Pan/Spin  │        │       │    │    │    │      │Project  │
│           │ Zoom      │        │       │    │    │    │      │Combine  │
├───────────┴───────────┴────────┴───────┴────┴────┴────┴──────┴─────────┤
│ Create                                                                  │
│ Shell │ Offset │ Mirror │ Equation │ Cylinder │ Sphere │ Body          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 1.3 왼쪽 패널 (Structure Tree)

```
📁 engine_configurations*
  ├── 📦 engine_body
  ├── 📦 distributor
  ├── 📦 intake_manifold
  ├── 📦 flywheel
  ├── 📦 timing_cover
  ├── 📦 aux_drive_1
  ├── 📦 ac_compressor
  ├── 📦 aux_drive_2
  ├── 📦 alternator          ← 선택됨 (하이라이트)
  ├── 📦 exhaust_manifold
  ├── 📦 ps_pump.asm
  ├── 📦 water_bypass_bracket
  └── 📦 water_bypass_line
```

하단 탭: **Structure | Layers | Groups | Selection | Views**

### 1.4 Options 패널 (리본 하단)

```
Options - Pull
⚙ General
  + Add  - Cut  ⊘ No merge
  [아이콘들: 방향 고정, 구속, 각도 등]
```

### 1.5 하단 상태바

```
Properties │ Appearance │ Simulation Structure │ Playback
                                          engine_configurations* ×
Pull 1 face                                              Face ▼
```

---

## 2. 전체 기능 목록 (탭별)

### 2.1 File 탭 — 20개

| # | 그룹 | 기능 | 설명 |
|---|------|------|------|
| 1 | New | New Design | 빈 디자인 생성 |
| 2 | | New from Template | 템플릿에서 생성 |
| 3 | Open | Open | 파일 열기 (.scdoc, .step, .stl 등) |
| 4 | | Recent Files | 최근 파일 목록 |
| 5 | Save | Save | 현재 파일 저장 |
| 6 | | Save As | 다른 이름으로 저장 |
| 7 | | Save Copy | 사본 저장 |
| 8 | Import | Import STL | STL 파일 가져오기 |
| 9 | | Import STEP | STEP 파일 가져오기 |
| 10 | | Import IGES | IGES 파일 가져오기 |
| 11 | | Import OBJ | OBJ 파일 가져오기 |
| 12 | | Import Parasolid | Parasolid 파일 가져오기 |
| 13 | Export | Export STL | STL 내보내기 |
| 14 | | Export STEP | STEP 내보내기 |
| 15 | | Export Image | 스크린샷 내보내기 |
| 16 | Print | Print | 인쇄 |
| 17 | Properties | Document Properties | 문서 속성 |
| 18 | | Units | 단위 설정 (mm/m/inch) |
| 19 | | Material Library | 재질 라이브러리 |
| 20 | Close | Close | 파일 닫기 |

### 2.2 Design 탭 — 35개 (핵심)

| # | 그룹 | 기능 | 설명 | 스크린샷 위치 |
|---|------|------|------|-------------|
| 21 | Clipboard | Paste | 붙여넣기 | 좌상단 |
| 22 | | Copy | 복사 | |
| 23 | | Cut | 잘라내기 | |
| 24 | Orient | Home | 홈 뷰 복원 | 리본 |
| 25 | | Plan View | 정면 뷰 | 리본 |
| 26 | | Pan | 화면 이동 | 리본 |
| 27 | | Spin | 화면 회전 | 리본 |
| 28 | | Zoom | 확대/축소 | 리본 |
| 29 | | Zoom Window | 영역 확대 | |
| 30 | | Zoom to Fit | 전체 맞춤 | |
| 31 | Mode | Sketch Mode | 2D 스케치 모드 진입/종료 | 리본 |
| 32 | **Select** | **Select** | 객체 선택 도구 | 리본 핵심 |
| 33 | **Pull** | **Pull** | 면/모서리 당기기 (Extrude) | 리본 핵심 |
| 34 | | Pull Options: Add | 재질 추가 방향 돌출 | Options 패널 |
| 35 | | Pull Options: Cut | 재질 제거 방향 돌출 | Options 패널 |
| 36 | | Pull Options: No merge | 별도 바디로 돌출 | Options 패널 |
| 37 | **Move** | **Move** | 객체 이동/복사 | 리본 핵심 |
| 38 | **Fill** | **Fill** | 구멍/면 채우기 (디피처링 핵심) | 리본 핵심 |
| 39 | Edit | Blend (Fillet) | 모서리 라운딩 | |
| 40 | | Chamfer | 모서리 모따기 | |
| 41 | Intersect | **Split Body** | 평면으로 바디 분할 | 리본 |
| 42 | | **Split** | 면/모서리로 분할 | 리본 |
| 43 | | **Project** | 커브를 면 위에 투영 | 리본 |
| 44 | | **Combine** | 바디 합치기 | 리본 |
| 45 | Create | **Shell** | 솔리드→속이 빈 껍질 | 리본 |
| 46 | | **Offset** | 면 오프셋 | 리본 |
| 47 | | **Mirror** | 대칭 복사 | 리본 |
| 48 | | **Equation** | 수학 방정식 곡면 | 리본 |
| 49 | | **Cylinder** | 실린더 생성 | 리본 |
| 50 | | **Sphere** | 구 생성 | 리본 |
| 51 | | **Body** | 바디 생성 (Box 등) | 리본 |
| 52 | | Cone | 원뿔 생성 | |
| 53 | | Torus | 토러스 생성 | |
| 54 | | Plane | 기준면 생성 | |
| 55 | | Axis | 기준축 생성 | |

### 2.3 Display 탭 — 15개

| # | 기능 | 설명 |
|---|------|------|
| 56 | Wireframe | 와이어프레임 모드 |
| 57 | Shaded | 음영 모드 |
| 58 | Transparent | 반투명 모드 |
| 59 | Hidden Line | 숨은선 모드 |
| 60 | Section View | 단면 뷰 |
| 61 | Exploded View | 분해 뷰 |
| 62 | Show/Hide Bodies | 바디 표시/숨기기 |
| 63 | Show All | 전체 표시 |
| 64 | Appearance | 색상/재질 외관 |
| 65 | Edge Display | 모서리 표시 옵션 |
| 66 | Lighting | 조명 설정 |
| 67 | Background | 배경색 설정 |
| 68 | Perspective/Ortho | 투시/정사 전환 |
| 69 | Shadow | 그림자 On/Off |
| 70 | Grid | 그리드 표시 |

### 2.4 Assembly 탭 — 10개

| # | 기능 | 설명 |
|---|------|------|
| 71 | Component | 컴포넌트 삽입 |
| 72 | Place Component | 위치 지정 배치 |
| 73 | Align | 면/모서리 정렬 |
| 74 | Fix | 고정 (움직이지 않게) |
| 75 | Anchor | 앵커 설정 |
| 76 | Interference Check | 간섭 체크 |
| 77 | Clearance | 클리어런스 확인 |
| 78 | Explode | 분해 |
| 79 | Pattern | 패턴 배치 |
| 80 | Replace | 컴포넌트 교체 |

### 2.5 Measure 탭 — 10개

| # | 기능 | 설명 |
|---|------|------|
| 81 | Distance | 두 점/면/모서리 거리 |
| 82 | Angle | 두 면/모서리 각도 |
| 83 | Area | 면적 계산 |
| 84 | Volume | 체적 계산 |
| 85 | Length | 모서리 길이 |
| 86 | Perimeter | 둘레 길이 |
| 87 | Mass Properties | 질량/관성모멘트 |
| 88 | Minimum Distance | 최소 거리 |
| 89 | Deviation | 면 편차 분석 |
| 90 | Curvature | 곡률 분석 |

### 2.6 Facets 탭 — 12개

| # | 기능 | 설명 |
|---|------|------|
| 91 | Auto Skin | 패싯→B-Rep 자동 변환 |
| 92 | Fit Surface | 패싯에 곡면 피팅 |
| 93 | Reduce | 삼각형 수 줄이기 (Decimate) |
| 94 | Refine | 삼각형 세분화 |
| 95 | Smooth | 패싯 스무딩 |
| 96 | Separate | 연결된 패싯 분리 |
| 97 | Merge | 패싯 합치기 |
| 98 | Fill Holes | 패싯 구멍 메우기 |
| 99 | Detect Features | 특징 자동 감지 |
| 100 | Remesh | 패싯 리메싱 |
| 101 | Offset Mesh | 메시 오프셋 |
| 102 | Boolean | 패싯 불리안 |

### 2.7 Repair 탭 — 15개

| # | 기능 | 설명 |
|---|------|------|
| 103 | Check | 형상 오류 검사 |
| 104 | Fix Errors | 자동 오류 수정 |
| 105 | Missing Faces | 누락된 면 생성 |
| 106 | Extra Edges | 불필요한 모서리 제거 |
| 107 | Split Edges | 분할된 모서리 제거 |
| 108 | Stitch | 면 봉합 |
| 109 | Unstitch | 면 분리 |
| 110 | Gap Fill | 틈새 메우기 |
| 111 | Solidify | 면→솔리드 변환 |
| 112 | Merge Faces | 같은 면 합치기 |
| 113 | Delete Face | 면 삭제 |
| 114 | Replace Face | 면 교체 |
| 115 | Extend Face | 면 연장 |
| 116 | Patch | 패치 면 생성 |
| 117 | Trim | 면 자르기 |

### 2.8 Prepare 탭 — 20개 (시뮬레이션 전처리)

| # | 기능 | 설명 |
|---|------|------|
| 118 | Enclosure | 외부 유동 도메인 생성 |
| 119 | Volume Extract | 유동 영역 추출 |
| 120 | Share Topology | 토폴로지 공유 |
| 121 | Named Selection | 이름 지정 선택 |
| 122 | Suppress | 파트 억제 (해석 제외) |
| 123 | Unsuppress | 파트 억제 해제 |
| 124 | Midsurface | 중간면 추출 (박판) |
| 125 | Beam | 빔 요소 추출 |
| 126 | Thickness | 두께 표시 |
| 127 | Contact | 접촉 영역 감지 |
| 128 | Simplify | 자동 간소화 |
| 129 | Defeaturing | 디피처링 자동 |
| 130 | Remove Fillets | 필렛 제거 |
| 131 | Remove Holes | 구멍 제거 |
| 132 | Remove Chamfers | 챔퍼 제거 |
| 133 | Remove Rounds | 라운드 제거 |
| 134 | Remove Bosses | 보스 제거 |
| 135 | Remove Pockets | 포켓 제거 |
| 136 | Point Mass | 질점 추가 |
| 137 | Coordinate System | 좌표계 생성 |

### 2.9 Tools 탭 — 8개

| # | 기능 | 설명 |
|---|------|------|
| 138 | Options | 환경 설정 |
| 139 | Customize | 리본 사용자 정의 |
| 140 | Scripting | 스크립트 편집기 |
| 141 | Record | 매크로 녹화 |
| 142 | Playback | 매크로 재생 |
| 143 | Undo History | 실행 취소 이력 |
| 144 | Selection Filter | 선택 필터 (면/모서리/점) |
| 145 | Snap Settings | 스냅 설정 |

---

## 3. 왼쪽 패널 시스템

### 3.1 Structure 탭 (기본)
어셈블리 트리: 파트/바디/면 계층 구조
- 우클릭: Rename, Delete, Suppress, Hide/Show, Properties
- 드래그앤드롭: 파트 재배치
- 더블클릭: 파트 편집 모드 진입

### 3.2 Layers 탭
레이어 관리: 표시/숨기기, 잠금/해제, 색상 지정

### 3.3 Groups 탭
사용자 정의 그룹: 관련 객체 묶기

### 3.4 Selection 탭
선택된 객체 목록, 선택 세트 저장/불러오기

### 3.5 Views 탭
저장된 뷰 목록, 뷰 생성/삭제/적용

---

## 4. Options 패널 (도구별 옵션)

각 도구(Pull, Move, Fill 등) 선택 시 리본 아래에 표시되는 옵션:

### Pull Options
```
⚙ General
  + Add        (재질 추가)
  - Cut        (재질 제거)
  ⊘ No merge   (별도 바디)

  [Up to surface]  [Symmetric]  [Draft angle]
  [Thin]           [Revolve]     [Sweep]
```

### Move Options
```
⚙ General
  [Copy]  [Paste at Origin]
  [Snap to Grid]  [Keep Alignment]

  Pivot: [Face center / Edge midpoint / Point]
```

### Fill Options
```
⚙ General
  [Delete and Fill]  [Delete and Patch]
  [Smooth]           [Extend]

  Fill Mode: [Automatic / Manual]
```

---

## 5. 하단 상태바 / 속성 패널

### 상태바
```
Properties │ Appearance │ Simulation Structure │ Playback
```

### Properties 패널
선택된 객체의 속성:
- 바디: 이름, 재질, 체적, 질량, 무게중심
- 면: 면적, 법선 방향, 소속 바디
- 모서리: 길이, 종류 (직선/원호/스플라인)
- 점: 좌표 (x, y, z)

### Appearance 패널
- 색상, 투명도, 텍스처, 반사도

### Simulation Structure 패널
- Fluent/Mechanical용 파트 매핑
- Named Selections 관리

---

## 6. 3D 뷰포트 기능

### 6.1 마우스 조작
| 입력 | 동작 |
|------|------|
| 좌클릭 | 선택 |
| 좌클릭+드래그 | 면 Pull / 객체 이동 |
| 중앙 드래그 | Spin (회전) |
| Shift+중앙 | Pan (이동) |
| 스크롤 | Zoom |
| 우클릭 | Context Menu |
| 더블클릭 | Zoom to Selection |

### 6.2 뷰 큐브 (View Cube)
3D 뷰포트 우상단에 회전 가능한 큐브:
- 면 클릭: Front/Back/Top/Bottom/Left/Right
- 모서리 클릭: 45도 뷰
- 꼭짓점 클릭: Isometric 뷰
- 드래그: 자유 회전

### 6.3 미니 툴바 (뷰포트 좌측)
```
  [↖] 선택 모드
  [✋] 이동 모드
  [🔄] 회전 모드
  [📐] 측정 모드
  [✂️] 단면 모드
```

### 6.4 선택 필터 (상태바 우측)
```
  [Face ▼]  ← 현재 선택 모드
  옵션: Face / Edge / Vertex / Body / Component
```

---

## 7. 구현 우선순위

### Phase 1 — 기본 뼈대 (P0)

| 기능 | 수 | 상태 |
|------|---|------|
| File: New/Open/Save/Import STL | 5 | 🔴 |
| Design: Select/Pull/Move/Fill | 4 | 🟡 부분 |
| Design: Box/Sphere/Cylinder/Cone/Torus | 5 | ✅ 완료 |
| Design: Shell/Offset/Mirror | 3 | 🟡 부분 |
| Design: Split Body/Combine | 2 | 🟡 부분 |
| Display: Wireframe/Shaded/Transparent | 3 | ✅ 완료 |
| Repair: Check/Fix/Stitch | 3 | 🔴 |
| Prepare: Enclosure/Extract/Named Selection | 3 | ✅ 완료 |
| Structure Tree | 1 | ✅ 완료 |
| Properties Panel | 1 | ✅ 완료 |
| Selection Filter (Face/Edge/Body) | 1 | 🔴 |
| **소계** | **31** | |

### Phase 2 — 핵심 편집 (P1)

| 기능 | 수 |
|------|---|
| Pull (Extrude/Cut/Draft) | 5 |
| Fill (디피처링 핵심) | 3 |
| Blend/Chamfer | 2 |
| Measure (거리/각도/면적) | 5 |
| Facets (Reduce/Smooth/Fill Holes) | 4 |
| Display (Section View/Exploded) | 2 |
| **소계** | **21** |

### Phase 3 — 고급 (P2)

| 기능 | 수 |
|------|---|
| Sketch Mode (Line/Arc/Circle/Spline) | 5 |
| Sweep/Loft | 2 |
| Assembly (Place/Align/Fix) | 5 |
| Repair (Missing Faces/Gap Fill/Solidify) | 5 |
| Prepare (Midsurface/Beam/Contact) | 3 |
| Views/Layers/Groups | 3 |
| **소계** | **23** |

### Phase 4 — 완성 (P3)

| 기능 | 수 |
|------|---|
| Equation surfaces | 1 |
| Sheet Metal | 5 |
| Scripting/Record/Playback | 3 |
| Detail (Dimensions/Annotations) | 5 |
| Undo History | 1 |
| Custom Coordinate System | 1 |
| Point Mass | 1 |
| **소계** | **17** |

---

## 8. GFD 리본 레이아웃

SpaceClaim과 동일한 리본 구조를 GFD GUI에 구현:

```
┌─────────────────────────────────────────────────────────────────────┐
│  File │ Design │ Display │ Measure │ Facets │ Repair │ Prepare     │
├─────────────────────────────────────────────────────────────────────┤
│                        Design Tab Ribbon                            │
├──────┬───────┬──────┬──────────────────┬─────────┬──────────────────┤
│Clip  │Orient │Mode  │    Edit          │Intersect│    Create        │
│board │       │      │                  │         │                  │
│Paste │Home   │Sketch│Select Pull Move  │SplitBody│Shell  Offset     │
│Copy  │PlanV  │      │       Fill       │Split    │Mirror Cylinder   │
│Cut   │Pan    │      │       Blend      │Project  │Sphere Body       │
│      │Spin   │      │       Chamfer    │Combine  │Cone   Torus      │
│      │Zoom   │      │                  │         │Plane  Axis       │
└──────┴───────┴──────┴──────────────────┴─────────┴──────────────────┘
```

---

## 9. 총 기능 수 요약

| 카테고리 | 기능 수 |
|---------|--------|
| File | 20 |
| Design | 35 |
| Display | 15 |
| Assembly | 10 |
| Measure | 10 |
| Facets | 12 |
| Repair | 15 |
| Prepare | 20 |
| Tools | 8 |
| 3D Viewport | 15 |
| Panels (Structure/Properties/Options) | 10 |
| **총합** | **170개** |
