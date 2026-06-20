# GFD Flood Simulation Plan — Terrain (DEM) + BIM (IFC) → Mesh → Flood

**Status**: Phase 0 (설계 문서) — 2026-06-19 작성. 코드 미착수.
**물리 전략**: 하이브리드 — 2D 천수방정식(SWE)을 메인 엔진으로 신규 구현하고,
필요한 핫스팟만 기존 3D VOF/Euler-Euler로 줌인(one-way coupling).
**BIM 전략**: `gfd-geo`에 네이티브 IFC4 리더를 신규 구현 (외부 변환 도구 의존 없음).

이 문서는 `docs/CAD_KERNEL_PLAN.md`의 phase + completion-matrix 형식을 따른다.
구현을 시작하기 전에 반드시 이 문서의 §3(좌표계·정밀도)와 §4(물리 모델 선택)를 먼저 읽을 것.

---

## 0. 목표 (Goal)

지형 정보(DEM)와 BIM 건축물(IFC)을 **공통 로컬 좌표계**로 정합한 뒤 단일 메시로 합치고,
그 위에서 침수(홍수) 시뮬레이션을 수행해 **침수심·유속·도달시간·hazard 래스터**를 산출한다.

대상 시나리오:
- 도시·유역 규모 침수도 (강우/하천 범람/제방 붕괴) — 2D SWE
- 특정 건물군·교각·여수로 주변 국소 3D 자유수면 디테일 — 3D VOF (줌인)

비목표(현 단계 제외): 토사·debris 수송, 지하 관망(SWMM류) 연계, 실시간 예보 운영.

---

## 1. 기존 GFD 자산 실측 (재사용 가능 / 없음)

### 재사용 가능 (이미 작동, 코드 확인 완료)
| 자산 | 위치 | 용도 |
|---|---|---|
| Cut-cell 임베디드 경계 | `crates/gfd-mesh/src/hybrid/cutcell.rs` | 지형/건물을 배경격자에 SDF로 절단 |
| 삼각형→SDF | `crates/gfd-mesh/src/geometry/distance_field.rs::sdf_from_triangles` | STL→implicit (닫힌 메시 전제, pseudo-normal) |
| SDF primitive/조합 | `crates/gfd-mesh/src/geometry/primitives.rs` (`sdf_union`/`subtract`/`intersection`) | 지형∪건물 합성 SDF |
| Marching cubes | `crates/gfd-mesh/src/geometry/marching_cubes.rs` | SDF→삼각형 |
| Cartesian 격자(grading) | `crates/gfd-mesh/src/structured/cartesian.rs` | DEM→2D SWE 격자 |
| Octree AMR / prism layer | `crates/gfd-mesh/src/hybrid/octree.rs`, `unstructured/prism_layer.rs` | 침수전선 국소 세분 / 경계층 |
| STL 리더 | `gfd-mesh/src/geometry/stl_reader.rs`, `gfd-io/src/mesh_reader/stl.rs` | 지형/건물 메시 입력 |
| 압축성 Riemann + CFL | `crates/gfd-fluid/src/compressible/mod.rs::compute_cfl_timestep` | **SWE HLLC 솔버의 변형 원형** |
| 3D 자유수면 | `gfd-fluid/src/multiphase/{vof,level_set,euler_euler}.rs` | 줌인 트랙(중력 `gravity:[f64;3]` 지원) |
| STEP B-Rep import | `gfd-cad-io/src/lib.rs:67 import_step_brep` → `step.rs:854 read_step_brep` | 보조 BIM 경로(IFC→STEP 변환 시) |
| 아핀 변환 | `crates/gfd-cad-feature/src/transform.rs` | 건물 지오레퍼런싱 배치 |
| 경계조건 프레임워크 | `crates/gfd-boundary/` | inlet/outlet/wall/symmetry → 홍수 BC 래퍼 기반 |
| VTK writer / Gmsh 리더 | `crates/gfd-io/` | 결과 출력 / 외부 메시 입력 |

### 완전히 없음 (신규 구축 대상)
- ❌ **2D SWE / Saint-Venant 솔버** — 홍수 본체 (Phase 6)
- ❌ **지리정보 I/O** — GeoTIFF / ESRI ASCII grid(.asc) / LAS·LAZ DEM (Phase 1)
- ❌ **CRS/좌표변환** — UTM/EPSG, 로컬원점 시프트 (Phase 2)
- ❌ **네이티브 IFC4 리더** — BIM (Phase 3)
- ❌ **DEM → 메시 변환** (Phase 4) / **건물 풋프린트 burn-in** (Phase 5)
- ❌ 홍수 전용 BC(유량 hydrograph, 강우, 하류 수위), wetting/drying, Manning 마찰 (Phase 6–7)
- ❌ 비압축/SWE 적응 dt(현재 CFL은 compressible 전용)

---

## 2. Target Architecture

```
 [지형 DEM]                         [BIM]
 GeoTIFF / .asc / LAS·LAZ           IFC4 (Revit/ArchiCAD export)
     │                                  │
     ▼                                  ▼
 ┌──────────────────────────────────────────────────────┐
 │  gfd-geo  (신규 크레이트)                              │
 │   ├── dem/      .asc → GeoTIFF → LAS 리더             │
 │   ├── crs/      EPSG/UTM 재투영 + 로컬원점 시프트     │
 │   ├── ifc/      네이티브 IFC4 STEP-physical 파서       │
 │   │             (IfcWall/Slab/Column/Proxy 외피 추출)  │
 │   └── georef/   DEM+IFC 공통 로컬 카테시안 정합        │
 └──────────────────────────────────────────────────────┘
     │ 공통 로컬 좌표 (f64, 원점 시프트 완료)
     ▼
 ┌────────────────────────┬──────────────────────────────┐
 │ Track A (메인)         │ Track B (국소 줌인)           │
 │ 2D SWE                 │ 3D VOF / Euler-Euler          │
 │  ├ DEM→2D Cartesian    │  ├ 지형∪건물 → 합성 SDF       │
 │  │   + z_b 필드        │  ├ CutCellMesher              │
 │  ├ 건물 풋프린트       │  └ 기존 gfd-fluid multiphase  │
 │  │   burn-in           │                               │
 │  └ gfd-fluid/          │  ▲ SWE 결과를 유입 BC로 수신  │
 │    shallow_water (신규)│  (one-way coupling)           │
 └────────────────────────┴──────────────────────────────┘
     │
     ▼
 후처리: 최대침수심 / 도달시간 / 유속 hazard
     → VTK + GeoTIFF export → GUI(지형 위 수심 컬러맵 + 시간 슬라이더)
```

신규 코드는 두 곳에 집중된다:
1. `crates/gfd-geo/` — 지리정보 I/O + IFC + 정합 (새 크레이트, Layer 1)
2. `crates/gfd-fluid/src/shallow_water/` — 2D SWE 솔버 (기존 크레이트 내 새 모듈)

---

## 3. 좌표계 · 정밀도 (착수 전 필독)

실무에서 가장 자주 깨지는 지점. 모든 Phase의 전제다.

1. **DEM과 IFC는 좌표계가 다르다.**
   - DEM: 투영좌표 (한국이면 EPSG:5186 중부원점 TM 또는 5179 UTM-K, 또는 글로벌 UTM zone).
   - IFC: 자체 로컬 원점 + `IfcMapConversion`(IFC4) 지오레퍼런싱(Eastings/Northings/OrthogonalHeight/XAxisAbscissa·Ordinate 회전). 현장 파일에 누락·오류가 잦아 **수동 배치 보정 경로를 반드시 제공**한다.
2. **공통 로컬 원점으로 시프트한다.** UTM 좌표는 ≥500,000 m → GFD `TriMesh`의 **f32 positions**(`Vec<[f32;3]>`)와 충돌: f32 유효숫자 ~7자리라 500,000 m에서 분해능 ~0.03 m로 붕괴. 따라서:
   - `gfd-geo`가 로컬 원점(도메인 SW 코너 등)을 보관하고, **TriMesh/STL로 내리기 전에 좌표에서 원점을 뺀다.**
   - 내부 계산·SWE 솔버는 전부 **f64** 유지. 출력 시 원점을 다시 더해 지리좌표 복원.
3. **단위 통일.** DEM=m, IFC=mm가 흔함 → import 시 스케일 정규화(`IfcSIUnit`/`IfcConversionBasedUnit` 파싱).
4. **연직 기준(datum) 주의.** DEM 표고(해발 EGM/지오이드)와 IFC 건물 0층 레벨이 다를 수 있음 → georef 단계에서 수직 오프셋 입력.

---

## 4. 물리 모델 — 왜 2D SWE 메인인가 (근거)

| 방식 | 변수 | 규모 적합성 | GFD 현황 |
|---|---|---|---|
| **2D SWE (Saint-Venant)** | h(수심), hu, hv | ✅ km² 침수도 정석 (HEC-RAS 2D, TELEMAC, LISFLOOD-FP) | ❌ 없음 → 신규 |
| 3D NS + 자유수면 (VOF/LS/Euler-Euler) | 전 3D 속도·압력·α | ⚠️ 도시 전체는 셀 폭발 | ✅ 보유 → 줌인 전용 |

**핵심 재사용 포인트**: SWE는 "압력 = ½gh², 음속 = √(gh)"인 2D 쌍곡 보존계로, **2D 압축성 Euler와 수학적으로 동형**이다. 기존 HLLC Riemann + `compute_cfl_timestep` 골격을 변형해 구현 → 백지 구현이 아니다.

SWE 보존형:
```
∂U/∂t + ∂F/∂x + ∂G/∂y = S_b + S_f
U = [ h , hu , hv ]ᵀ
F = [ hu , hu² + ½g h² , huv ]ᵀ
G = [ hv , huv , hv² + ½g h² ]ᵀ
S_b (하상경사) = [ 0 , -g h ∂z_b/∂x , -g h ∂z_b/∂y ]ᵀ
S_f (Manning 마찰) = [ 0 , -g n² u√(u²+v²)/h^{1/3} , -g n² v√(u²+v²)/h^{1/3} ]ᵀ
```
파속(wave speed) λ = u·n ± √(gh), CFL dt = C · Δx / max(|u|+√(gh)).

**솔버 4대 필수 요소** (이게 빠지면 물리적으로 틀린다):
1. **Well-balanced (C-property)** — 정지수면(h+z_b=const, u=0)이 기계 정밀도로 유지되어야 함. → **Audusse hydrostatic reconstruction**(2004) 채택. 미적용 시 평지에 가짜 흐름 발생.
2. **Wetting/drying** — 마른 셀 h→0 허용오차(h_dry ≈ 1e-4 m) + 얕은 셀 유속·마찰 제한(분모 h^{1/3} 폭주 방지).
3. **Manning 마찰** — 반음해(semi-implicit) 처리로 얕은 수심 안정화.
4. **양의 수심 보존(positivity-preserving)** — HLLC flux + h ≥ 0 보장 재구성.

수치 스킴: Godunov FVM(1차) → MUSCL + minmod 제한자(2차)로 확장. 시간적분은 explicit Euler → SSP-RK2.

---

## 5. 건물 처리 방식 (2D SWE)

도시 침수에서 건물은 3가지 방식 중 선택(점진 채택):
| 방식 | 처리 | 장단점 | 채택 순서 |
|---|---|---|---|
| **Block (상승)** | 건물 풋프린트 셀의 `z_b`를 최고수위 이상으로 상향 | 가장 단순, 메시 변경 없음 | **1차(MVP)** |
| **Hole (제거)** | 건물 셀 제거, 벽면을 reflective wall BC | 정확, 흐름 우회 정밀 | 2차 |
| **Roughness/Porosity** | 건물 셀에 높은 Manning n / 공극률 | 군집 건물 통계적 처리 | 3차(선택) |

건물 풋프린트 추출: IFC 솔리드(또는 tessellated TriMesh)를 z-평면에 투영 → 외곽 폴리곤 → 셀 포함 판정(`polygon contains-point`, 기존 measure 헬퍼 재사용).

---

## 6. Phase Plan (iteration 단위)

### Phase 0 — Scaffolding ✅
- [x] `docs/FLOOD_PLAN.md` 작성 (이 문서)
- [x] `crates/gfd-geo/` 크레이트 스캐폴드 + workspace 등록 (Layer 1, deps: gfd-core; tiff/las/proj는 Phase 1+에서 추가)
- [x] `crates/gfd-fluid/src/shallow_water/mod.rs` 모듈 스캐폴드 (SwState `[h,hu,hv]`+z_b, SwParams, wetting/drying velocity, SWE CFL dt)
- [x] `examples/flood_dambreak_1d.json` 설정 스키마 초안 (Ritter 검증 케이스)

### Phase 1 — DEM I/O (`gfd-geo::dem`) ⚠️
- [x] **`.asc`(ESRI ASCII grid) 리더** — 헤더(ncols/nrows/xll·yll corner|center/cellsize/NODATA, 순서·대소문자·NODATA누락 허용) + 격자값. 첫 타깃 완료.
- [x] `Dem { ncols, nrows, cellsize, origin:[f64;2], nodata, z: Vec<f64> }` + bilinear 샘플러(+NODATA 폴백) + crop/downsample + elevation_range/bounds/cell_center
- [ ] **GeoTIFF 리더** (`tiff` crate; GeoKeyDirectory에서 EPSG 추출)
- [ ] **LAS/LAZ 포인트 클라우드** (`las` crate) → 격자화(IDW/최근접) → DEM (선택, 후순위)
- [x] DEM 다운샘플/크롭(해석 도메인 한정) — `Dem::crop`/`Dem::downsample`

### Phase 2 — CRS & Georeferencing (`gfd-geo::crs`, `::georef`) ⬜
- [ ] EPSG↔로컬 재투영 (`proj` PROJ 바인딩 우선, 순수 Rust `proj4rs` 대안 평가)
- [ ] 로컬 원점 시프트 컨테이너 `LocalFrame { epsg, origin:[f64;3], shift()/unshift() }`
- [ ] 한국 좌표계 프리셋 (EPSG:5186, 5179, 5174)
- [ ] §3 정밀도 가드: TriMesh 변환 전 f64→원점차→f32 강제 경로

### Phase 3 — Native IFC Reader (`gfd-geo::ifc`) ⬜
> IFC는 STEP physical file(ISO 10303-21) 문법을 공유 → 기존 `gfd-cad-io/src/step.rs` 파서 토큰화 로직 부분 재사용.
- [ ] STEP-physical 토크나이저 + 엔티티 그래프 (헤더/DATA 섹션, `#id=ENTITY(...)`)
- [ ] `IfcProject`/`IfcSite`/`IfcBuilding`/`IfcBuildingStorey` 공간 위계 파싱
- [ ] **지오레퍼런싱**: `IfcMapConversion` + `IfcProjectedCRS` (IFC4) / `IfcSite.RefLatitude·RefLongitude·RefElevation` (IFC2x3 fallback)
- [ ] 단위: `IfcUnitAssignment` / `IfcSIUnit` / `IfcConversionBasedUnit`
- [ ] **외피(envelope) 추출**: `IfcWall`, `IfcWallStandardCase`, `IfcSlab`, `IfcColumn`, `IfcBeam`, `IfcRoof`, `IfcBuildingElementProxy`만 유지. 가구/MEP/`IfcFurnishing`/`IfcFlowSegment` 제거.
- [ ] **형상 표현 파싱**: `IfcExtrudedAreaSolid`(가장 흔함, 풋프린트+높이 압출), `IfcFacetedBrep`, `IfcPolygonalFaceSet`, `IfcTriangulatedFaceSet`(IFC4) → TriMesh
- [ ] `IfcLocalPlacement` 체인 누적 변환 (건물 → 층 → 부재 좌표계)
- [ ] **수동 배치 보정 API** (지오레퍼런싱 누락 파일 대비): 사용자 지정 origin/rotation 오버라이드
- 보조 경로: 지오레퍼런싱·형상이 너무 복잡한 파일은 외부 IfcOpenShell `IfcConvert`로 STEP/OBJ 변환 후 기존 import 재사용 (fallback 문서화)

### Phase 4 — DEM → Mesh ⚠️
- [x] **2D(Track A)**: `Dem::to_bed_field()` → `SwGrid` + 셀별 `z_b` (NODATA→고지대 wall). end-to-end 테스트 `tests/flood_dem_to_swe.rs`(다운슬로프 흐름·질량보존·NODATA wall 검증)
- [ ] **3D(Track B)**: DEM 격자당 삼각형 2개 → 하이트필드 STL → `sdf_from_triangles` → `CutCellMesher` (줌인 트랙, 후순위)
- [x] NODATA·경계 처리 (NODATA→wall), 도메인 클립(`Dem::crop`)

### Phase 5 — 지형+건물 결합 ⬜
- [ ] 건물 풋프린트 추출 (z투영 → 외곽 폴리곤)
- [ ] **2D burn-in**: Block 방식(`z_b` 상향) MVP → Hole(reflective wall) → Roughness(Manning n 맵)
- [ ] **3D**: `sdf_union(지형, 건물들)` 합성 SDF → cut-cell (메시 CSG보다 SDF union이 boundary 견고)
- [ ] 건물-지형 접지면(foundation) 처리, 셀 분류 검증

### Phase 6 — 2D SWE 솔버 (`gfd-fluid/src/shallow_water/`) ⚠️ **(핵심·거의완료)**
- [x] 보존변수 `[h, hu, hv]` 상태 + bed `z_b` (`SwState`, `SwGrid`, `SwParams`)
- [x] **HLLC Riemann flux** (`hllc.rs`, 파속 √(gh), Einfeldt+dry-bed, 횡방향 contact 업윈드)
- [x] **Well-balanced**: Audusse hydrostatic reconstruction — lake-at-rest C-property 머신정밀도(1e-10) 검증
- [x] **Wetting/drying** + positivity 보존 (h_dry, 마른 셀 속도 0, 음수 깊이 클램프)
- [x] **Manning 마찰** 소스항 (semi-implicit, `friction`)
- [x] **CFL 적응 dt** (`cfl_timestep`, smax=|u|+√(gh))
- [x] 1차 Godunov + SSP-RK2 (시간 2차). Ritter 댐붕괴·질량보존 검증
- [x] **2차 MUSCL+minmod 공간 재구성** (`SwParams.order=2`) — η(자유수면) 재구성으로
  well-balanced 유지(C-property 1e-10), bed-기울기 중앙 소스(평지에서 0). Ritter
  dam 깊이 오차 1차 0.100 → 2차 0.027로 개선

### Phase 7 — 홍수 경계조건·소스 ⬜
- [ ] **유입 hydrograph** Q(t) inlet (시간보간 테이블)
- [ ] **하류 outflow**: normal-depth / critical / 지정 수위(stage)
- [ ] **면적 강우** 소스항 (rain-on-grid) + 침투(infiltration, 선택)
- [ ] reflective wall(건물·제방) BC
- [ ] `gfd-boundary` 위 홍수 BC 래퍼

### Phase 8 — 하이브리드 커플링 (one-way) ⬜
- [ ] SWE 결과(수심·평면유속)를 3D 줌인 도메인 경계의 유입 프로파일로 매핑
- [ ] VOF 초기 자유수면(α) 초기화 from SWE h
- [ ] 줌인 영역 자동 추출(고유속/고수심 hotspot 탐지)

### Phase 9 — 후처리 · GUI ⬜
- [ ] 최대침수심 / 도달시간(arrival time) / 유속·hazard(v·h, v·√h) 래스터 산출
- [ ] VTK + **GeoTIFF export**(원점 unshift 복원)
- [ ] GUI: 지형 위 수심 컬러맵 + 시간 슬라이더 (`CadKernelLayer`/`MeshRenderer` 확장, `cadStore`에 flood 결과 슬라이스)
- [ ] gfd-server JSON-RPC: `flood.load_dem` / `flood.load_ifc` / `flood.build_mesh` / `flood.run` / `flood.result`

### Phase 10 — 검증 (Validation) ⬜
- [ ] **Ritter/Stoker 댐붕괴** 해석해 (1D, well-balanced·wetting/drying 1차 검증)
- [ ] **Lake-at-rest** C-property (정지수면 보존, 머신 정밀도)
- [ ] **Malpasset 댐 붕괴** (고전 2D SWE 벤치, 실측 도달시간·최고수위)
- [ ] **Toce River** 물리모형(건물 배열 도시 침수)
- [ ] **Carrier-Greenspan** 처오름(wetting/drying 동적)
- [ ] DEM+IFC 정합 회귀 테스트(좌표 라운드트립 오차 < 1e-6 상대)

---

## 7. 제외 / 추후 (Deferred)

- 토사·debris·세굴 수송, 부유물
- 2-way SWE↔3D 강결합 커플링 (현 단계는 one-way)
- 지하 우수관망(SWMM류) 연계
- 실시간·앙상블 예보 운영, 비정상 강우 nowcasting
- B-Rep CSG 기반 정밀 건물-지형 절단 (mesh/SDF 수준으로 충분)
- IFC 의미정보(재질·속성) 활용 — 형상 외피만 사용

---

## 8. File Layout (예정)

```
crates/
  gfd-geo/                         ← 신규 (Layer 1)
    Cargo.toml                     deps: gfd-core, gfd-cad-io, gfd-mesh, tiff, las, proj(or proj4rs)
    src/
      lib.rs
      dem/      mod.rs  asc.rs  geotiff.rs  las.rs  sampler.rs
      crs/      mod.rs  epsg.rs  reproject.rs  local_frame.rs
      ifc/      mod.rs  tokenizer.rs  entity.rs  spatial.rs
                geometry.rs  placement.rs  units.rs  georef.rs
      georef/   mod.rs  align.rs        (DEM+IFC 공통 정합)
      mesh/     mod.rs  heightfield.rs  footprint.rs  burn_in.rs
  gfd-fluid/
    src/
      shallow_water/               ← 신규 모듈
        mod.rs        state.rs      (U=[h,hu,hv])
        hllc.rs                     (Riemann flux, √(gh) 파속)
        well_balanced.rs            (Audusse reconstruction)
        wetting_drying.rs
        friction.rs                 (Manning, semi-implicit)
        cfl.rs                      (적응 dt)
        muscl.rs                    (2차 + minmod)
        bc.rs                       (hydrograph / outflow / rainfall / wall)
        solver.rs                   (시간적분 SSP-RK2)
src/
  server.rs                        ← flood.* JSON-RPC 핸들러 추가
examples/
  flood_dambreak_1d.json           검증용
  flood_city.json                  DEM+IFC 통합 시나리오
gui/src/
  tabs/flood/  FloodTab.tsx  DemPanel.tsx  RunPanel.tsx  ResultPanel.tsx
  engine/      FloodLayer.tsx       (지형 위 수심 컬러맵)
  store/       floodStore.ts
docs/
  FLOOD_PLAN.md                    (이 문서)
```

---

## 9. Completion Matrix

| Phase | Status | Summary |
|---|---|---|
| 0 Scaffolding | ✅ 완료 | gfd-geo 크레이트(workspace 등록) + `shallow_water` 모듈 + `examples/flood_dambreak_1d.json` 스키마 초안 |
| 1 DEM I/O | ⚠️ 부분 | `.asc` 리더 + `Dem`(bilinear 샘플러·crop·downsample) ✅ / GeoTIFF·LAS·proj는 후순위 ⬜ |
| 2 CRS/Georef | ⬜ | EPSG 재투영 + 로컬원점 시프트 + 한국 좌표계 프리셋 + f32 정밀도 가드 |
| 3 IFC Reader | ⬜ | 네이티브 IFC4 파서, 외피 추출, MapConversion 지오레퍼런싱, ExtrudedAreaSolid/Brep 형상 |
| 4 DEM→Mesh | ⚠️ 부분 | **2D Track A 완료**: `Dem::to_bed_field`→SwGrid+z_b (NODATA→wall), end-to-end 테스트 `tests/flood_dem_to_swe.rs` / 3D Track B(하이트필드 STL→cut-cell) ⬜ |
| 5 결합 | ⬜ | 풋프린트 추출 + Block/Hole/Roughness burn-in + SDF union |
| 6 SWE 솔버 | ✅ 완료 | HLLC + Audusse well-balanced + wetting/drying + positivity + Manning(semi-impl) + 적응 CFL + SSP-RK2 + **MUSCL 2차(minmod η-재구성 + 중앙 bed 소스)**. lake-at-rest C-property 1e-10(1·2차), Ritter 댐붕괴(MUSCL 오차 0.027 vs 1차 0.100), 질량보존 |
| 7 홍수 BC | ⬜ | hydrograph / outflow / rainfall / wall |
| 8 하이브리드 | ⬜ | SWE→3D VOF one-way coupling |
| 9 후처리/GUI | ⬜ | 침수심/도달시간/hazard + VTK/GeoTIFF + GUI 컬러맵 + flood.* RPC |
| 10 검증 | ⬜ | Ritter/Stoker, lake-at-rest, Malpasset, Toce, Carrier-Greenspan |

범례: ⬜ 미착수 · 🔨 진행 중 · ⚠️ 부분 · ✅ 완료

---

## 10. 의존 크레이트 후보 (외부)

| 목적 | 후보 | 비고 |
|---|---|---|
| GeoTIFF 읽기 | `tiff` / `geotiff` | GeoKeyDirectory에서 EPSG |
| LAS/LAZ | `las` (+ `laz`) | LiDAR 포인트 클라우드 |
| CRS 재투영 | `proj`(PROJ FFI) / `proj4rs`(pure-Rust) | PROJ은 시스템 의존, proj4rs는 순수 Rust지만 변환 적음 → 평가 필요 |
| (IFC) | 신규 자작 | `ifc-rs` 미성숙 → 네이티브 구현 결정 |

> 신규 외부 크레이트 추가 전 OCCT 미의존 원칙(CAD 커널)과 별개로, solver 측은 실용적 crate 도입 허용. 단 PROJ 시스템 의존성은 빌드 문서화 필요(기존 `gfd-gmsh`의 `--features gmsh` 패턴 참고).

---

## 11. 참고 — 검증 벤치마크 출처

- Stoker (1957) — 댐붕괴 해석해
- Audusse et al. (2004) — hydrostatic reconstruction (well-balanced SWE)
- Malpasset dam break — CADAM/IMPACT 프로젝트 실측 데이터
- Toce River physical model — IMPACT WP3 (건물 배열 도시 침수)
- Carrier & Greenspan (1958) — 해안 처오름 wetting/drying
- 비교 도구: HEC-RAS 2D, TELEMAC-2D, LISFLOOD-FP, BASEMENT, SRH-2D
