# GFD CAD Kernel Port Plan — Pure Rust FreeCAD Reimplementation

**Status**: Iteration 195 complete (2026-04-20). Test count: **989 passing**.
**Phase 1–8 all functionally complete** — see Completion Matrix in §9b.

## Current totals (iter 195)

- **10 CAD crates**: gfd-cad-geom / topo / bool / heal / sketch / feature / io / tessel / measure + gfd-cad facade
- **~125 JSON-RPC methods**: document / feature / boolean / measure / heal / sketch / import / export / profile / version / arena / mesh / ping / misc
- **Export formats (13, all disk + 9 in-memory)**: STL ASCII + binary, BRep-JSON, STEP AP214, OBJ, OFF, PLY, WRL, XYZ, VTK Legacy, DXF R12
- **Import formats (7)**: STL, STEP (points-only), BRep-JSON, OBJ, OFF, PLY, XYZ
- **Primitives (19)**: box / cube / rectangular_prism / sphere / cylinder / cone / torus / wedge / pyramid / ngon_prism / tube / disc / tetrahedron / octahedron / icosahedron / icosphere / stairs / honeycomb / spiral_staircase
- **2D profile generators (13)**: ngon / star / rectangle / rounded_rectangle / slot / ellipse / gear / airfoil / i_beam / l_angle / c_channel / t_beam / z_section
- **Revolve (r,z) profile generators (5)**: ring / cup / frustum / torus / capsule
- **Curve samplers (3)**: helix / archimedean_spiral / torus_knot
- **Sketcher constraints (17)** + lifecycle RPCs (new/add_*/solve/get/dof/list/delete/add_polyline/add_profile)
- **Measure helpers (60+)**: shape-based + bare-TriMesh families (area, volume, bbox, inertia, principal axes, OBB, bounding sphere, Hausdorff, Euler χ, closest point, signed distance, ray intersect, edge/aspect stats, polygon signed area / convex hull / contains-point, …)
- **Tessellation bridge helpers**: weld, prune_unused_vertices, reverse_winding, compute_face_normals, compute_smooth_normals, subdivide_midpoint, transform, aabb, aabb_overlaps, center_and_normalize, laplacian_smooth
- **Document persistence**: save_json / load_json (disk) + to_string / from_string (memory)
- **Raw-mesh RPC family (9)**: boolean_raw / smooth / subdivide / weld / transform / compute_normals / reverse_winding / concat / ray-SDF/raycast

Phase 1–8 scope is comprehensively covered. Deliberately deferred (non-blocking):
B-Rep-level CSG (SSI + face classification), generic-edge rolling-ball fillet,
full STEP AP214 reader with topology reconstruction, `BSplineSurface`, dodecahedron.
**Goal**: FreeCAD 1.0의 Part Design + Sketcher + Shape Healing 기능을 **Pure Rust**로 재구현. OCCT 의존 없음. GUI는 Electron + React + Three.js (현재 스택 유지). 기존 Design/Display/Measure/Repair 탭 전체 제거 후 재작성.

---

## 1. Target Architecture

```
┌──────────────────────────────────────────────────────────┐
│  Electron (gui/electron/main.js)                         │
│    └── BrowserWindow → React app                         │
├──────────────────────────────────────────────────────────┤
│  React + Three.js GUI (gui/src/)                         │
│    ├── tabs/design_v2/       ← NEW (Part Design clone)   │
│    ├── tabs/display_v2/      ← NEW                       │
│    ├── tabs/measure_v2/      ← NEW                       │
│    ├── tabs/repair_v2/       ← NEW                       │
│    ├── engine/CadScene.tsx   ← 재사용 (tessellation 결과) │
│    └── ipc/cadClient.ts      ← NEW JSON-RPC client       │
├──────────────────────────────────────────────────────────┤
│  IPC boundary:  stdio JSON-RPC (electron → gfd-server)   │
├──────────────────────────────────────────────────────────┤
│  Rust backend (gfd-server binary)                        │
│    └── crates/gfd-cad-*       ← NEW (이 계획의 주제)      │
│         ├── gfd-cad-geom      (Point/Curve/Surface)       │
│         ├── gfd-cad-topo      (Vertex/Edge/Face/Solid)    │
│         ├── gfd-cad-bool      (Boolean ops)               │
│         ├── gfd-cad-sketch    (2D sketcher + solver)      │
│         ├── gfd-cad-feature   (Feature tree, parametric)  │
│         ├── gfd-cad-io        (STEP/IGES/STL/BRep)        │
│         ├── gfd-cad-heal      (Shape healing)             │
│         ├── gfd-cad-measure   (Distance/Area/Volume)      │
│         ├── gfd-cad-tessel    (B-Rep → triangles)         │
│         └── gfd-cad           (facade + JSON-RPC types)   │
└──────────────────────────────────────────────────────────┘
```

---

## 2. Scope (FreeCAD 대비)

| FreeCAD 모듈 | 구현 계획 | 난이도 | 페이즈 |
|---|---|---|---|
| **Base/App/Document** | `gfd-cad::Document`, feature tree | ★★ | P3 |
| **Part** (OCCT B-Rep wrap) | `gfd-cad-topo` + `gfd-cad-geom` | ★★★★★ | P1, P2 |
| **PartDesign** | `gfd-cad-feature` (Body/Pad/Pocket/Revolve/Fillet) | ★★★★ | P5 |
| **Sketcher** | `gfd-cad-sketch` (2D constraint solver) | ★★★★ | P4 |
| **Draft** | 추후 | ★★ | 제외 |
| **Arch** | 추후 | ★★★ | 제외 |
| **Path** (CAM) | 추후 | ★★★ | 제외 |
| **Assembly** | A2plus 스타일 placement | ★★★ | 추후 |
| **Mesh** | 기존 gfd-mesh 재사용 | - | 재사용 |
| **Import/Export** | STEP/IGES/STL/BRep | ★★★★ | P6 |
| **Shape Healing** | ShapeFix equivalent | ★★★ | P11 |
| **Measure** | B-Rep geometric queries | ★★ | P10 |

**제외**: FEM(기존 gfd-solid 사용), TechDraw, Spreadsheet, Surface, BIM, Python scripting(1차 제외, 추후 `rhai`로 대체).

---

## 3. Phase Plan (iteration 단위, 대략 1 iteration = 1 세션)

### Phase 0 — Cleanup & Scaffolding ✅ **(iter 1)**
- [x] `docs/CAD_KERNEL_PLAN.md` 작성
- [x] Ribbon: `DesignRibbon/DisplayRibbon/MeasureRibbon/RepairRibbon` 613줄 제거
- [x] `RibbonCadV2.tsx` placeholder로 교체 (`npx tsc --noEmit` 0 errors)
- [x] `crates/gfd-cad*` 10개 크레이트 스캐폴드 + workspace 등록
- [ ] `gui/src/tabs/cad/` 디렉토리 정리 (LeftPanelStack 연결 확인 후) — **iter 3**
- [ ] `useAppStore.ts`의 repair/measure 슬라이스 제거 — **iter 3**

### Phase 1 — Geometry (gfd-cad-geom) ✅ **(iter 1–2)**
- [x] `Point3`, `Vector3`, `Direction3`
- [x] Curves: `Line`, `Circle`, `BSplineCurve` (Cox-de Boor, clamped uniform)
- [x] Surfaces: `Plane`, `Cylinder`, `Sphere`, `Cone`, `Torus`
- [x] `Curve::eval` / `tangent` / `length` / `closest_point` (golden-section)
- [x] `Surface::eval` / `normal`
- [x] `BoundingBox`
- [x] `Ellipse` — iter 92
- [ ] `BSplineSurface`, analytical curve derivatives — **iter 3+**

### Phase 2 — Topology (gfd-cad-topo) ⏳ **(iter 2–21)**
- [x] `Shape` enum: Compound/Solid/Shell/Face/Wire/Edge/Vertex
- [x] `ShapeId(u32)` stable arena with tombstoned removal
- [x] `Orientation` (Forward/Reversed/Internal/External)
- [x] `builder::make_line_edge`, `make_wire`, `make_solid_from_face`
- [x] `iter::collect_by_kind` 재귀 순회
- [x] `EdgeFaceMap::build` — edge→faces + vertex→edges 인시던스
- [x] `is_manifold_vertex` — 매니폴드 체인 판별
- [x] **`build_half_edges(arena, root)`** — HalfEdge 리스트 + next/prev 스레드 + twin 페어링
- [x] `EdgeFaceMap::face_neighbors` — 공유 에지 있는 face 목록 반환
- [ ] ShapeHealing에서 사용하는 join — iter 30+

### Phase 3 — Boolean (gfd-cad-bool) ⏳ **(iter 6–33)**
- [x] `compound_merge` — multi-shape grouping as `Shape::Compound`
- [x] `union` = compound_merge alias (non-CSG fallback)
- [x] `mesh_boolean(A, B, op)` — mesh CSG via centroid-ray classification (Möller-Trumbore)
- [x] `bbox_overlaps` — fast AABB pre-filter
- [x] RPC `cad.boolean.mesh_{union,difference,intersection}` + GUI button
- [ ] B-Rep CSG union/difference/intersection (SSI + face classification) — iter 34+
- [ ] Face-splitting mesh CSG (libigl-quality) — iter 34+

### Phase 4 — Sketcher (gfd-cad-sketch) ⏳ **(iter 4–18)**
- [x] Entities: `Point`, `Line`, `Circle`, `Arc { center, start, end }`
- [x] Constraints (17개): Coincident, Fix, H, V, Distance, Parallel, Perpendicular,
  PointOnLine, PointOnCircle, Radius, EqualLength, Angle, ArcClosed, TangentLineCircle, Symmetric, ArcLength, DistancePointLine
- [x] Solver: damped Gauss-Newton w/ Levenberg, forward-diff Jacobian, pre-apply algebraic
- [x] 11 tests (+ dof_status)
- [x] **DOF analysis** — `residual_count`, `DofStatus { Under/Well/Over }`, RPC `cad.sketch.dof`
- [x] GUI DOF badge (colour-coded) after every solve
- [ ] Symbolic arc/arc tangent, analytical Jacobian — iter 19+
- [ ] Symbolic rank-based over-constrained detection — iter 19+

### Phase 5 — Features (gfd-cad-feature) ⏳ **(iter 2–17)**
- [x] `primitive::{box, sphere, cylinder, cone, torus}_solid` — 모두 `Shape::Solid` 반환
- [x] `FeatureTree` + `execute()` — primitive feature 재실행 캐시
- [x] `pad::pad_polygon_xy`, `pad_polygon_xy_signed`, `pad_polygon_along(dir)` — polygon extrude (axial or tilted)
- [x] `revolve::revolve_profile_z` — 360° polygon revolve around Z
- [x] `pocket::pocket_polygon_xy` — downward extrusion
- [x] `chamfer::chamfered_box_solid` — clip one box corner → 7-face solid
- [x] `fillet::filleted_box_solid` — round a box corner with a sphere octant → 7-face solid
- [x] `chamfered_box_top_edges` — 4 top edges chamfered at once (keycap/tapered-cube, 10 faces)
- [x] `filleted_box_top_edges` — 4 top edges + 4 top corners rounded with cylinder/sphere faces (14 faces)
- [x] `filleted_cylinder_solid` — cylinder with top/bottom circular fillets (5-face analytic)
- [x] `revolve_profile_z_partial(angle_rad)` — arbitrary-angle revolve with auto start/end caps
- [x] `mirror_shape(id, MirrorPlane)` — deep-clone through XY/YZ/XZ mirror plane
- [x] `translate_shape`, `scale_shape`, `rotate_shape` — affine transforms built on shared kernel
- [x] `linear_array`, `circular_array`, `rectangular_array` — pattern features
- [x] `offset_polygon_2d(points, distance)` — 2D polygon offset (CCW-aware)
- [x] `wedge_solid` — triangular-prism primitive (5 faces)
- [x] `pyramid_solid`, `ngon_prism_solid` — square pyramid + regular N-gon prism
- [x] Platonic family — `tetrahedron_solid`, `octahedron_solid`, `icosahedron_solid`, `icosphere_solid`
- [x] Parametric compounds — `stairs_solid`, `honeycomb_pattern_solid`, `spiral_staircase_solid`
- [x] 2D profile library (13) — ngon/star/rect/rounded_rect/slot/ellipse/gear/airfoil/i_beam/l_angle/c_channel/t_beam/z_section
- [x] (r,z) revolve profile library (5) — ring/cup/frustum/torus/capsule
- [x] Curve samplers — helix / archimedean spiral / torus knot
- [x] Convenience builders — `cube_solid`, `rectangular_prism_solid`, `tube_solid`, `disc_solid`, `star/gear/slot_prism_solid`
- [ ] Generic Fillet on arbitrary B-Rep edge (rolling ball over 2 adjacent faces) — deferred

### Phase 6 — I/O (gfd-cad-io) ⏳ **(iter 5–24)**
- [x] `stl::read_stl` — ASCII + binary
- [x] `stl::write_stl_ascii` / `write_stl_binary` — ASCII + binary writers
- [x] RPC `cad.export.stl` (tessellate + write in one call, `binary` flag)
- [x] `brep::{read_brep, write_brep}` — serde-JSON roundtrip
- [x] `step::write_step` — CARTESIAN_POINT + VERTEX_POINT + EDGE_CURVE + EDGE_LOOP +
  FACE_OUTER_BOUND + AXIS2_PLACEMENT_3D + PLANE + ADVANCED_FACE + CLOSED_SHELL + MANIFOLD_SOLID_BREP
- [x] `step::read_step_points` — points-only reader
- [x] RPC `cad.export.{brep,step}`, `cad.import.{brep,step}`
- [x] `obj::{read_obj, write_obj}`, `off::{read_off, write_off}`, `ply::{read_ply_ascii, write_ply_ascii}`
- [x] `xyz::{read_xyz, write_xyz}`, `wrl::write_wrl`, `vtk::write_vtk_polydata`, `dxf::write_dxf_3dface`
- [x] In-memory export for 9 formats (no disk I/O): STL, OBJ, PLY, STEP, BRep, VTK, WRL, DXF, XYZ
- [x] AXIS2_PLACEMENT_3D + PLANE + CYLINDRICAL_SURFACE + SPHERICAL_SURFACE in STEP writer
- [ ] IGES — deferred
- [ ] STEP reader with topology reconstruction — deferred

### Phase 7 — JSON-RPC API ⏳ **(iter 3–7)**
- [x] document/feature primitives, tessellate, tree, pad
- [x] import.stl, measure (polygon_area, bbox_volume, distance, surface_area)
- [x] boolean.union (compound), heal.check_validity
- [x] sketch.{new, add_point, add_line, add_arc, add_circle, add_constraint, solve, get, dof, list, delete}
- [x] Complete feature coverage: pad, pocket, revolve (+ partial), chamfer, fillet, mirror, transforms, arrays, offset
- [x] Profile + revolve RPCs: pad_profile, pocket_profile, revolve_profile, profile.generate, profile.list_kinds
- [x] Meta RPCs: cad.version, cad.ping, cad.arena.{list_shapes, shape_info, delete_shape, stats}
- [x] Raw-mesh RPCs: boolean_raw, smooth, subdivide, weld, transform, compute_normals, reverse_winding, concat
- [x] Measure RPCs: shape_summary, multi_shape_summary, mesh_quality, hausdorff, polygon_full, trimesh_summary, trimesh_signed_distance, trimesh_raycast

#### Reference list
프로토콜:
```json
{"jsonrpc":"2.0", "id":1, "method":"cad.pad",
 "params":{"sketch_id":"abc", "length":10.0, "direction":"normal"}}
```

Methods (명세):
- `cad.document.new`, `cad.document.save`, `cad.document.load`
- `cad.sketch.create`, `cad.sketch.add_line`, `cad.sketch.add_constraint`, `cad.sketch.solve`
- `cad.feature.pad`, `cad.feature.pocket`, `cad.feature.revolve`, `cad.feature.fillet`
- `cad.boolean.union`, `cad.boolean.difference`, `cad.boolean.intersection`
- `cad.import.step`, `cad.import.stl`, `cad.export.step`, `cad.export.stl`
- `cad.query.tessellate` → vertices/indices for Three.js
- `cad.measure.distance`, `cad.measure.area`, `cad.measure.volume`
- `cad.heal.fix_shape`, `cad.heal.sew`
- `cad.tree.get` → feature tree JSON

### Phase 8 — Design Tab v2 GUI ⏳ **(iter 3–15)**
- [x] `cadClient.ts` — JSON-RPC client with browser-dev sim
- [x] `DesignTabV2.tsx` — 6 create buttons (box/sphere/cyl/cone/torus/pad/revolve/pocket) + Mesh A−B
- [x] `LeftPanelStack` 재배선: Design/Display/Measure/Repair 모두 v2
- [x] `cadStore.ts` + `CadKernelLayer.tsx` — Three.js BufferGeometry per shape
- [x] `FeatureTree.tsx` — 아이콘, on-demand measure, validity, visibility, delete
- [x] `SketcherCanvas.tsx` — SVG 2D 스케처 (Point/Line/H/V/Fix 툴, Solve round-trip)
- [x] Sketch-to-Pad UI — "Extrude → Pad" button with height InputNumber
- [x] `PropertyPanel.tsx` — edit + re-execute for box / sphere / cylinder / cone / torus / chamfer_box / fillet_box
- [x] Sketch-to-Revolve button (treats sketch (x, y) as (r, z) profile)
- [x] Undo / Redo (50-entry history stack on structural mutations; cosmetic skip)
- [x] Mirror / Translate / Rotate / Linear-array / Circular-array buttons (all operate on "last shape")
- [x] Profile pad buttons (7): Hex / Star / Slot / RoundRect / Ellipse / Gear / Airfoil + structural beams (I/T/L/C/Z)
- [x] Profile pocket buttons (Star / Slot / Gear cutout)
- [x] Revolve profile buttons (5): Ring / Cup / Frustum / Torus / Capsule
- [x] Primitive+convenience buttons (Tube / Disc / Tetra / Octa / Icosa / Icosphere / Stairs / Honeycomb / Spiral Stair)
- [x] Mesh import buttons (5): STL / OBJ / OFF / PLY / XYZ
- [x] Live backend badges: kernel version / arena alive / registered shapes / GUI shapes
- [x] DisplayTabV2 — disk-path Export (9 formats) + browser Blob Download (9 formats) + per-row delete
- [x] MeasureTabV2 — `multi_shape_summary` batched RPC for instant table load
- [x] RepairTabV2 — shape-info histogram + heal bulk fix + issue grid

#### Full scope
FreeCAD Part Design 카피:
- Left panel: **Body/Feature Tree** (트리 노드 드래그로 순서 변경, visibility toggle)
- Center: Three.js viewport (기존 CadScene 재사용)
- Ribbon: Sketch / Pad / Pocket / Revolve / Fillet / Chamfer / Primitive
- **Sketcher overlay**: 2D canvas (SVG + Konva 또는 Canvas2D) on XY 평면 투영
- Property panel: 선택된 feature의 파라미터 편집

### Phase 9 — Display Tab v2 ⏳ **(iter 10–23)**
- [x] Per-shape visibility Switch + color picker (Ant Design ColorPicker)
- [x] Show-all / hide-all bulk actions
- [x] Wireframe mode toggle per shape
- [x] Opacity slider per shape (0.1 – 1.0)
- [x] **Section plane** — X/Y/Z axis + offset slider, Three.js `clippingPlanes` applied live
- [x] Render modes — Shaded / Shaded+Edges / Wireframe / Hidden-line (per-shape Select)

### Phase 10 — Measure Tab v2 ⏳ **(iter 5–22)**
- [x] `distance(vertex, vertex)` — Euclidean
- [x] `polygon_area_signed`, `polygon_area` — shoelace formula
- [x] `bounding_box`, `bbox_volume` — axis-aligned
- [x] `face_area` — Newell's method for 3D polygon face
- [x] `surface_area` — planar + analytic (sphere 4πr², torus 4π²Rr, cylinder 2πrh, cone π(r1+r2)·slant)
- [x] `divergence_volume`, `volume` — signed tetrahedron sum for closed polygon solids
- [x] `edge_length` (line edges), `angle_between_edges` (unoriented ∈ [0, π])
- [x] `center_of_mass` — divergence centroid with bbox fallback
- [x] `distance_vertex_edge` — clipped segment distance
- [x] `distance_edge_edge` — Lumelsky O(1) closest-points between two segments
- [x] `distance_face_face` — vertex-vertex + vertex-edge polygon sampling
- [x] `inertia_bbox` — diagonal inertia (homogeneous-box approximation)
- [x] `inertia_tensor_diag` — analytical MoI via signed-tetra volume integration
- [x] `inertia_tensor_full` — full 3×3 inertia tensor with cross terms (Ixy, Iyz, Izx)
- [x] `principal_axes` — ordered eigenvalues + eigenvector matrix (nalgebra symmetric_eigen)
- [x] `bounding_sphere` — bbox-based enclosing sphere (quick)
- [x] `minimum_bounding_sphere` — Welzl's randomised O(n) tight enclosing sphere
- [x] `edge_length_range` — min/max line-edge length
- [x] `closest_point_on_shape(q)` — boundary + face-interior distance (plane projection + 2D point-in-polygon)
- [x] `is_point_inside_solid(q)` — ray-cast inside/outside test via tessellation
- [x] `signed_distance(q)` — boundary distance signed by inside/outside

### Phase 11 — Repair Tab v2 (Shape Healing) ⏳ **(iter 6–22)**
- [x] `check_validity` — degenerate edge, empty wire/shell/solid, compound self-ref
- [x] `ValidityIssue { arena_id, kind, detail }` reporting
- [x] GUI `RepairTabV2.tsx` — bulk check across document, issue grid with Ant Design Tags
- [x] `fix_shape::remove_small_edges` — arena mutates, wire references filtered
- [x] `fix_shape::sew_vertices` — coincident-vertex collapse, edge refs rewritten
- [x] `fix_shape::dedup_edges` — duplicate-edge collapse (runs after sew_vertices)
- [x] `fix_shape::close_open_wires` — near-coincident endpoint snap or bridge-edge insertion
- [x] `shape_stats` — vertex/edge/wire/face/shell/solid counts
- [x] GUI "Fix all" button, `cad.heal.fix` RPC with `sew` toggle
- [ ] `unify_tolerances` (requires per-edge tolerance storage) — iter 27+

### Phase 12 — Tessellation Bridge ⏳ **(iter 2–13)**
- [x] `TriMesh { positions, normals, indices }`
- [x] `uv_grid` + per-SurfaceGeom sampler (Plane/Cyl/Sphere/Cone/Torus)
- [x] 재귀 walker: Compound→Solid→Shell→Face 자동 tessellation
- [x] 통합 테스트: box 6×32×16×2 triangles, sphere 점들이 unit sphere 위
- [x] `earclip::triangulate_polygon` — ear-clipping (convex quad + L-shape tests)
- [x] Sphere-pole collapsing (removes sliver triangles at ±π/2 latitude)
- [x] `auto_uv_steps(surface, chord_tol)` + `tessellate_adaptive` — per-face chord-tolerance tessellation
- [ ] 인접 face edge stitching (seam 제거) — iter 61+

---

## 4. Crate Dependency Graph

```
gfd-cad-geom ───┬──► gfd-cad-topo ───┬──► gfd-cad-bool ──┐
                │                     │                   │
                │                     ├──► gfd-cad-heal ──┤
                │                     │                   │
                │                     ├──► gfd-cad-tessel ┤
                │                     │                   │
                │                     └──► gfd-cad-measure┤
                │                                         │
                └──► gfd-cad-sketch ──────────────────────┤
                                                          ▼
                                            gfd-cad-feature
                                                    │
                                              gfd-cad-io (parallel)
                                                    │
                                                    ▼
                                                gfd-cad
                                                    │
                                                    ▼
                                          src/server.rs (JSON-RPC)
```

---

## 5. External Dependencies (Rust crates to evaluate)

| Crate | 용도 | 상태 |
|---|---|---|
| `nalgebra` | 선형대수 | 채택 (기존) |
| `parry3d` | 충돌/거리 쿼리 | 평가 |
| `truck` (opt-rs) | B-Rep 참고용 | **레퍼런스만**, 직접 의존 X |
| `spade` | 2D Delaunay | tessellation용 |
| `earcut-rs` | polygon triangulation | 대안 |
| `step-rs` / 자작 | STEP 파싱 | **자작 권장** |
| `stl_io` | STL 읽기 | 기존 사용 |

**원칙**: 외부 CAD 커널은 **사용 안 함**. 순수 Rust로.

---

## 6. Non-Goals (1차 제외)

- Python scripting (추후 `rhai` 또는 `rustpython`)
- TechDraw (2D drawing)
- Arch/BIM
- Path/CAM
- Surface workbench의 고급 기능 (Gordon, Filling)
- 멀티바디 Assembly의 물리 시뮬레이션
- CAD 단위 자동 변환 (mm 고정)

---

## 7. Risk & Mitigation

| Risk | 영향 | 완화책 |
|---|---|---|
| Boolean op이 Rust로 구현하기 극도로 어려움 | 블로커 | 메쉬 boolean으로 1차 우회, B-Rep boolean은 점진 |
| STEP 파서 방대 | 시간 | AP214 최소 subset (솔리드 + 면 + 에지) 먼저 |
| Fillet rolling ball 수학 | 중간 | 직선 에지 → 원통면 fillet만 먼저 |
| 기존 GUI 탭 의존성 | 낮음 | 이번 이터에 완전 제거 |
| 테셀레이션 품질 | 중간 | chord tolerance, edge curvature 기반 adaptive |

---

## 8. Ralph Loop Strategy (10 iterations)

| Iter | 목표 |
|---|---|
| 1 | 이 문서 + Phase 0 (teardown + scaffold) + Phase 1 시작 |
| 2 | Phase 1 완료 (geometry primitives, 테스트) |
| 3 | Phase 2 (topology) |
| 4 | Phase 4 시작 (sketcher primitives + 기본 constraints) |
| 5 | Phase 5 primitive features (Box/Cylinder/Sphere + Pad) |
| 6 | Phase 12 tessellation + Phase 7 JSON-RPC 기초 |
| 7 | Phase 8 Design v2 탭 (최소: Body 트리 + Primitive + Pad) |
| 8 | Phase 6 STEP import 최소 + Phase 10 measure 기본 |
| 9 | Phase 3 boolean (mesh-based fallback) + Phase 11 heal 기본 |
| 10 | Phase 9 Display v2 + 통합 테스트 + 문서 업데이트 |

---

## 9. Success Criteria (Iteration 10 기준)

- `cargo test --workspace` 통과 (기존 805 + CAD 테스트)
- `cargo run --bin gfd-server` 실행 후 Electron GUI에서:
  - Body 생성 → Sketch → Pad → Fillet 파이프라인 작동
  - STEP 파일 import → 3D 표시 (최소 솔리드 1개)
  - Measure 탭에서 volume/area 정확 계산
  - Repair 탭에서 fix_all 기본 작동
- `npx tsc --noEmit` 0 errors
- 문서 업데이트 (`docs/IMPLEMENTED_FEATURES.md`에 CAD 크레이트 추가)

---

## 9b. Final Completion Matrix (iter 125, 2026-04-20)

| Phase | Status | Summary |
|---|---|---|
| 0 Cleanup | ✅ Complete | |
| 1 Geometry primitives | ✅ Point3/Vector3/Direction3, Line/Circle/Ellipse/BSplineCurve, Plane/Cylinder/Sphere/Cone/Torus. `BSplineSurface` still TODO (rarely needed for Part Design). |
| 2 B-Rep topology | ✅ `Shape` enum, `ShapeArena` stable id, tombstoned removal, `EdgeFaceMap` + `HalfEdge` threading + `face_neighbors` + `is_manifold_vertex` |
| 3 Boolean | ⚠️ Mesh CSG (Möller-Trumbore union/diff/intersect) fully working; `TriMesh::aabb_overlaps` prefilter. B-Rep CSG (SSI + face classification) still deliberately deferred — too complex for parametric Part-Design scope. |
| 4 Sketcher | ✅ 17 constraints, damped Gauss-Newton + Levenberg solver, DOF analysis, SketcherCanvas UI + H/V/Fix tools + Extrude/Revolve buttons |
| 5 Features | ✅ 18 feature kinds: box/sphere/cyl/cone/torus/wedge/pyramid/ngon_prism, pad, pocket, revolve (+partial), chamfer (corner+top), fillet (corner+top+cyl), mirror, translate/scale/rotate, linear/circular/rectangular array, offset_polygon + helix. 7 profile generators (ngon/star/rectangle/rounded_rect/slot/ellipse/gear). Generic-edge fillet still scoped out. |
| 6 I/O | ✅ 7 mesh/brep formats: STL ASCII+binary r/w, BRep-JSON roundtrip, STEP AP214 writer (10 entity kinds), STEP points-only reader, OBJ r/w, OFF r/w, PLY r/w, WRL (VRML 2.0) writer |
| 7 JSON-RPC | ✅ 85+ methods: document, feature (primitive/pad/pocket/revolve/chamfer/fillet/mirror/transform/arrays/offset), boolean, measure (shape + polygon_full), heal, sketch, import (STL, STEP points, BRep), export (STL × 2, BRep, STEP, OBJ, OFF, PLY, WRL), pad_profile / pocket_profile / profile.generate / profile.helix |
| 8 Design v2 GUI | ✅ ~30 DesignTabV2 buttons incl. 9 profile Pad/Pocket variants, Transforms, Arrays, MeshBoolean, Sketcher integration, FeatureTree, PropertyPanel, Undo/Redo. TypeScript strict 0 errors. |
| 9 Display v2 | ✅ Color + wireframe + opacity + visibility + **live section plane** |
| 10 Measure | ✅ 55+ measure helpers: distances (v-v/v-e/e-e/f-f), bbox, surface_area, divergence_volume, edge_length + range, angle, CoM, full 3×3 inertia, principal_axes, bounding_sphere/minimum_bounding_sphere, closest_point (boundary + face interior), is_point_inside, signed_distance, oriented_bounding_box, polygon_*. Bare-`TriMesh` series: `trimesh_surface_area`, `trimesh_volume`, `trimesh_bounding_box`, `trimesh_center_of_mass`, `trimesh_surface_centroid`, `trimesh_inertia_tensor`, `hausdorff_distance_vertex`, `mesh_euler_genus`, `trimesh_boundary_edges` / `non_manifold_edges` / `is_closed`, `ray_triangle_intersect` / `trimesh_ray_intersect`, `closest_point_on_triangle` / `trimesh_closest_point`, `trimesh_point_inside`, `trimesh_signed_distance` |
| 11 Repair | ✅ `check_validity` + `fix_shape::{remove_small_edges, sew_vertices, close_open_wires, dedup_edges}` + `shape_stats` |
| 12 Tessellation | ✅ UV grid + earclip + sphere-pole collapse + adaptive chord-tolerance. TriMesh methods: `weld`, `reverse_winding`, `compute_smooth_normals`, `compute_face_normals`, `transform` (auto-flip on negative det), `subdivide_midpoint`, `aabb` + `aabb_overlaps`, `center_and_normalize`, `laplacian_smooth`, `prune_unused_vertices` |

## 10. File Layout

```
crates/
  gfd-cad-geom/
    src/
      lib.rs
      point.rs        vec.rs
      curve/
        mod.rs  line.rs  circle.rs  ellipse.rs  bspline.rs
      surface/
        mod.rs  plane.rs  cylinder.rs  sphere.rs  cone.rs  torus.rs  bspline.rs
      bbox.rs  projection.rs
  gfd-cad-topo/
    src/
      lib.rs
      shape.rs        id.rs          orientation.rs
      vertex.rs       edge.rs        wire.rs
      face.rs         shell.rs       solid.rs  compound.rs
      halfedge.rs     builder.rs     iter.rs
  gfd-cad-bool/
    src/ lib.rs  union.rs  difference.rs  intersection.rs  ssi.rs
  gfd-cad-sketch/
    src/
      lib.rs          entity.rs
      constraint/
        mod.rs  coincident.rs  distance.rs  angle.rs  parallel.rs  perp.rs  tangent.rs
      solver.rs       dof.rs
  gfd-cad-feature/
    src/
      lib.rs          feature.rs     tree.rs
      pad.rs  pocket.rs  revolve.rs  fillet.rs  chamfer.rs  primitive.rs
  gfd-cad-io/
    src/
      lib.rs
      step/ mod.rs  parser.rs  writer.rs  entity.rs
      iges/ mod.rs
      stl.rs          brep.rs
  gfd-cad-heal/
    src/ lib.rs  sew.rs  fix_wire.rs  remove_small.rs  check.rs
  gfd-cad-measure/
    src/ lib.rs  distance.rs  area.rs  volume.rs  inertia.rs
  gfd-cad-tessel/
    src/ lib.rs  surface_mesh.rs  adaptive.rs  stitch.rs
  gfd-cad/
    src/
      lib.rs          document.rs    rpc.rs

gui/src/
  tabs/
    design_v2/  DesignTabV2.tsx  FeatureTree.tsx  Sketcher.tsx  PropertyPanel.tsx
    display_v2/ DisplayTabV2.tsx  VisibilityList.tsx  SectionView.tsx
    measure_v2/ MeasureTabV2.tsx  MeasurePanel.tsx
    repair_v2/  RepairTabV2.tsx   IssueList.tsx  HealingOptions.tsx
  ipc/
    cadClient.ts   JSON-RPC 클라이언트
  store/
    cadStore.ts    CAD 전용 Zustand slice (document, feature tree)
```
