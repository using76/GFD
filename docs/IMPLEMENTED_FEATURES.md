# GFD Implemented Features

29 크레이트 (19 solver + 10 CAD), **911 tests** 기준 구현 목록 (iter 66 시점).

---

## 20-29. gfd-cad-* — Pure-Rust CAD Kernel (FreeCAD-style 재구현)

새 크레이트 10개. 순수 Rust, OCCT 의존 없음. Electron GUI와 JSON-RPC로 연결. 자세한 로드맵은 [`CAD_KERNEL_PLAN.md`](CAD_KERNEL_PLAN.md).

| 크레이트 | 파일 | 구현 내용 |
|---|---|---|
| **gfd-cad-geom** | `src/point.rs`, `vec.rs`, `bbox.rs` | `Point3`, `Vector3`, `Direction3`, `BoundingBox` |
| | `src/curve/{line,circle,bspline}.rs` | Line, Circle, BSplineCurve (Cox-de Boor) + `Curve` trait (eval/tangent/length/closest_point) |
| | `src/surface/{plane,cylinder,sphere,cone,torus}.rs` | 5개 parametric surface + `Surface` trait |
| **gfd-cad-topo** | `src/shape.rs`, `arena.rs`, `iter.rs`, `builder.rs`, `adjacency.rs` | `Shape` enum, `ShapeArena` (stable ids), `HalfEdge`, `build_half_edges`, `EdgeFaceMap::{build, face_neighbors, is_manifold_vertex}`, `collect_by_kind` |
| **gfd-cad-sketch** | `src/lib.rs` | 2D sketcher: Point/Line/Circle entities, 12 constraints (Coincident/Fix/H/V/Distance/Parallel/Perpendicular/PointOnLine/PointOnCircle/Radius/EqualLength/Angle), damped Gauss-Newton + Levenberg solver |
| **gfd-cad-feature** | `src/primitive.rs` | `box_solid`, `sphere_solid`, `cylinder_solid`, `cone_solid`, `torus_solid` |
| | `src/pad.rs` | `pad_polygon_xy`, `pad_polygon_xy_signed` |
| | `src/pocket.rs` | `pocket_polygon_xy` — downward extrusion |
| | `src/revolve.rs` | `revolve_profile_z`, `revolve_profile_z_partial` (auto caps) |
| | `src/chamfer.rs` | `chamfered_box_solid` (1 corner), `chamfered_box_top_edges` (keycap) |
| | `src/fillet.rs` | `filleted_box_solid`, `filleted_box_top_edges`, `filleted_cylinder_solid` |
| | `src/wedge.rs` | `wedge_solid` — 삼각기둥 |
| | `src/pyramid.rs` | `pyramid_solid`, `ngon_prism_solid` (정N각형 프리즘) |
| | `src/mirror.rs` | `mirror_shape` XY/YZ/XZ |
| | `src/transform.rs` | `translate_shape`, `scale_shape` (per-axis), `rotate_shape` (Rodrigues) |
| | `src/array.rs` | `linear_array`, `circular_array`, `rectangular_array` |
| | `src/lib.rs` | `Feature` enum, `FeatureTree`, `execute()` |
| **gfd-cad-bool** | `src/lib.rs` | `compound_merge`, `bbox_overlaps` |
| | `src/mesh.rs` | `mesh_boolean` (union/diff/intersection via Möller-Trumbore), `point_inside_mesh` |
| **gfd-cad-io** | `src/stl.rs` | ASCII + binary STL read/write |
| | `src/brep.rs` | BRep-JSON roundtrip (`read_brep`, `write_brep`) |
| | `src/step.rs` | STEP AP214 writer (10 entity kinds: CARTESIAN_POINT, VERTEX_POINT, EDGE_CURVE, EDGE_LOOP, FACE_OUTER_BOUND, AXIS2_PLACEMENT_3D, PLANE/CYLINDRICAL_SURFACE/SPHERICAL_SURFACE, ADVANCED_FACE, CLOSED_SHELL, MANIFOLD_SOLID_BREP) + points-only reader |
| **gfd-cad-heal** | `src/lib.rs` | `check_validity`, `fix_shape` (sew_vertices + dedup_edges + close_open_wires + remove_small_edges), `shape_stats` |
| **gfd-cad-measure** | `src/lib.rs` | `distance`, `polygon_area`, `bounding_box`, `bbox_volume`, `face_area` (Newell), `surface_area` (planar + analytic sphere/torus/cylinder/cone), `divergence_volume`, `edge_length`, `angle_between_edges` |
| **gfd-cad-tessel** | `src/lib.rs`, `grid.rs`, `earclip.rs` | `TriMesh`, `uv_grid` sampler, per-surface tessellation, Compound→Face 재귀 walker, ear-clipping 2D polygon triangulation, sphere-pole collapse, `auto_uv_steps` + `tessellate_adaptive` (chord-tol 기반 per-face 단계) |
| **gfd-cad** (facade) | `src/document.rs`, `rpc.rs` | `Document { arena, features, sketches }`, RPC types, integration tests |

### JSON-RPC API (70+ methods, iter 66 시점)

```
# Document / tree
cad.document.new        cad.tree.get        cad.tessellate        cad.tessellate_adaptive

# Features
cad.feature.primitive           cad.feature.pad          cad.feature.pocket
cad.feature.revolve             cad.feature.chamfer_box   cad.feature.fillet_box
cad.feature.keycap              cad.feature.rounded_top_box  cad.feature.filleted_cylinder
cad.feature.wedge               cad.feature.pyramid       cad.feature.ngon_prism
cad.feature.mirror              cad.feature.translate     cad.feature.scale        cad.feature.rotate
cad.feature.linear_array        cad.feature.circular_array  cad.feature.rectangular_array

# Sketcher
cad.sketch.new  cad.sketch.add_{point, line, arc, circle, constraint}
cad.sketch.solve  cad.sketch.get  cad.sketch.dof

# Boolean
cad.boolean.union  cad.boolean.mesh_{union, difference, intersection}

# I/O
cad.import.{stl, brep, step}  cad.export.{brep, step, stl}

# Measure
cad.measure.{polygon_area, bbox_volume, surface_area, volume, edge_length,
             distance, distance_vertex_edge, distance_edge_edge,
             center_of_mass, inertia, bounding_sphere, principal_axes,
             edge_length_range, closest_point, point_inside, signed_distance}

# Heal
cad.heal.{check_validity, fix, stats}
```

### GUI (React + Three.js)

| 파일 | 역할 |
|---|---|
| `gui/src/ipc/cadClient.ts` | JSON-RPC client (Electron IPC + browser dev sim) |
| `gui/src/store/cadStore.ts` | Zustand slice — tessellated CAD shapes |
| `gui/src/engine/CadKernelLayer.tsx` | Three.js BufferGeometry per shape |
| `gui/src/tabs/design_v2/DesignTabV2.tsx` | primitive + pad + revolve buttons |
| `gui/src/tabs/design_v2/FeatureTree.tsx` | shape list with measure / validity / visibility / delete |
| `gui/src/tabs/display_v2/DisplayTabV2.tsx` | color picker, show/hide all |
| `gui/src/tabs/measure_v2/MeasureTabV2.tsx` | bulk area/volume/validity |
| `gui/src/tabs/repair_v2/RepairTabV2.tsx` | heal.check_validity issue list |

### Test coverage (CAD only, iter 66)

- gfd-cad-geom: 10 tests
- gfd-cad-topo: 5 tests (+ adjacency + half-edge threading)
- gfd-cad-sketch: 12 tests (+ symmetric, tangent, radius, DOF status)
- gfd-cad-feature: 26 tests (primitives + pad + revolve + pocket + chamfer/fillet variants + wedge + pyramid + ngon_prism + mirror + transforms + arrays)
- gfd-cad-bool: 6 tests (+ mesh_boolean x4 + bbox_overlaps)
- gfd-cad-io: 7 tests (STL ASCII + binary + write-roundtrip + BRep + STEP x2 + AXIS2_PLACEMENT entity check)
- gfd-cad-heal: 8 tests (check + fix_remove_small + sew + dedup + close_wires + stats)
- gfd-cad-measure: 22 tests (area + volume + distance all axes + CoM + inertia + bsphere + closest_point + inside + signed_distance)
- gfd-cad-tessel: 4 tests (uv_grid + earclip x3)
- gfd-cad: 4 integration tests (box round-trip, sphere tessellation, full_pipeline_smoke, extended_pipeline_smoke, box_divergence_volume)

**Total CAD tests: 106. Workspace total: 911.**

---

## Legacy solver crates

19 크레이트, 262 Rust 파일, ~63,800줄, 805 solver 테스트.

---

## 1. gfd-core — 핵심 자료구조 & 수치 기법

| 모듈 | 파일 | 구현 내용 |
|------|------|-----------|
| Field | `src/field/scalar.rs` | ScalarField |
| | `src/field/vector.rs` | VectorField |
| | `src/field/tensor.rs` | TensorField |
| Mesh | `src/mesh/structured.rs` | StructuredMesh (Cartesian, `to_unstructured()`) |
| | `src/mesh/unstructured.rs` | UnstructuredMesh |
| | `src/mesh/cell.rs` | Cell 정의 |
| | `src/mesh/face.rs` | Face 정의 |
| | `src/mesh/node.rs` | Node 정의 |
| | `src/mesh/partition.rs` | Mesh partitioning |
| Gradient | `src/gradient/mod.rs` | Green-Gauss cell-based gradient |
| Interpolation | `src/interpolation/mod.rs` | Face interpolation schemes |
| Convection | `src/numerics/convection.rs` | 1st/2nd Upwind, Central, QUICK, TVD (Van Leer, MinMod, Superbee, Van Albada) |
| Diffusion | `src/numerics/diffusion.rs` | Central, Over-relaxed, Minimum, Orthogonal correction |
| Time | `src/numerics/time.rs` | Forward Euler, BDF2, Crank-Nicolson, RK4 |
| LinAlg (basic) | `src/linalg/solvers/cg.rs` | CG (basic) |
| | `src/linalg/solvers/bicgstab.rs` | BiCGSTAB (basic) |
| | `src/linalg/solvers/gmres.rs` | GMRES (basic) |
| Preconditioner (basic) | `src/linalg/preconditioners/jacobi.rs` | Jacobi |
| | `src/linalg/preconditioners/ilu.rs` | ILU |
| | `src/linalg/preconditioners/amg.rs` | AMG |
| Sparse Matrix | `src/linalg/mod.rs` | SparseMatrix (CSR), LinearSystem, SolverStats |

## 2. gfd-linalg — 프로덕션 선형 솔버

| 모듈 | 파일 | 구현 내용 |
|------|------|-----------|
| Iterative | `src/iterative/cg.rs` | CG (Conjugate Gradient) |
| | `src/iterative/bicgstab.rs` | BiCGSTAB |
| | `src/iterative/gmres.rs` | GMRES |
| | `src/iterative/fgmres.rs` | FGMRES (Flexible GMRES) |
| | `src/iterative/pcg.rs` | PCG (Preconditioned CG) |
| | `src/iterative/pbicgstab.rs` | PBiCGSTAB (Preconditioned BiCGSTAB) |
| Direct | `src/direct/lu.rs` | LU factorization |
| | `src/direct/cholesky.rs` | Cholesky factorization |
| Preconditioner | `src/preconditioner/jacobi.rs` | Jacobi |
| | `src/preconditioner/ilu.rs` | ILU (Incomplete LU) |
| | `src/preconditioner/amg.rs` | AMG (Algebraic Multigrid) |
| | `src/preconditioner/block.rs` | Block preconditioner |
| Trait | `src/traits.rs` | `LinearSolverTrait` (production interface) |

## 3. gfd-matrix — 행렬 조립 & 경계조건

| 파일 | 구현 내용 |
|------|-----------|
| `src/assembler.rs` | COO → CSR 조립 (counting sort) |
| `src/sparse.rs` | 희소행렬 연산 |
| `src/block.rs` | Block matrix 지원 |
| `src/boundary.rs` | `apply_dirichlet`, `apply_neumann`, Robin BC |
| `src/modify.rs` | 행렬 수정/변환 |
| `src/diagnostics.rs` | Sparsity 분석, condition number 추정 |

## 4. gfd-fluid — 유동 해석

| 모듈 | 파일 | 구현 내용 |
|------|------|-----------|
| **비압축성** | `src/incompressible/simple.rs` | SIMPLE 알고리즘 |
| | `src/incompressible/piso.rs` | PISO 알고리즘 |
| | `src/incompressible/simplec.rs` | SIMPLEC 알고리즘 |
| **압축성** | `src/compressible/roe.rs` | Roe Riemann 솔버 |
| | `src/compressible/hllc.rs` | HLLC Riemann 솔버 |
| | `src/compressible/ausm.rs` | AUSM+ Riemann 솔버 |
| **다상유동** | `src/multiphase/vof.rs` | VOF (Volume of Fluid) + 계면 압축 |
| | `src/multiphase/level_set.rs` | Level Set + 재초기화 |
| | `src/multiphase/euler_euler.rs` | Euler-Euler 이류체 모델 |
| | `src/multiphase/dpm.rs` | DPM (Discrete Particle Method) |
| | `src/multiphase/mixture.rs` | Mixture 모델 |
| **연소** | `src/combustion/species.rs` | 종 질량분율 수송 |
| | `src/combustion/reaction.rs` | Arrhenius 유한속도, EDM, Eddy-Breakup |
| **난류 커플링** | `src/turbulence/transport_solver.rs` | 난류 수송방정식 솔버 어댑터 |
| | `src/turbulence/wall_treatment.rs` | 벽면 처리 |
| | `src/turbulence/les/mod.rs` | LES 인터페이스 |
| **소스항** | `src/source/porous.rs` | 다공성 매체 (Darcy, Forchheimer) |
| **EOS** | `src/eos.rs` | Ideal Gas, Stiffened Gas, Barotropic, Incompressible |

## 5. gfd-thermal — 열전달

| 파일 | 구현 내용 |
|------|-----------|
| `src/conduction.rs` | 정상/비정상 열전도 |
| `src/convection.rs` | 대류 열전달 |
| `src/phase_change.rs` | 상변화 (용융/응고) |
| `src/conjugate.rs` | Conjugate heat transfer (유체-고체 커플링) |
| `src/radiation/p1.rs` | P-1 복사 모델 |
| `src/radiation/discrete_ordinates.rs` | Discrete Ordinates (DO) 복사 |
| `src/radiation/view_factor.rs` | View Factor 복사 |

## 6. gfd-solid — 고체역학

| 파일 | 구현 내용 |
|------|-----------|
| `src/elastic.rs` | 선형 탄성 FEM (Hex8) |
| `src/hyperelastic.rs` | 초탄성 (Neo-Hookean 등) |
| `src/plasticity/von_mises.rs` | Von Mises (J2) 소성 + return-mapping |
| `src/plasticity/tresca.rs` | Tresca 항복 기준 |
| `src/plasticity/drucker_prager.rs` | Drucker-Prager 모델 |
| `src/dynamics.rs` | 동적 해석 (Newmark-beta) |
| `src/contact.rs` | 접촉 역학 |
| `src/creep.rs` | 크리프 (power-law, exponential) |
| `src/thermal_stress.rs` | 열응력 커플링 |

## 7. gfd-turbulence — 난류 모델

| 파일 | 구현 내용 |
|------|-----------|
| `src/builtin/k_epsilon.rs` | k-epsilon 모델 |
| `src/builtin/k_omega_sst.rs` | k-omega SST 모델 |
| `src/builtin/spalart_allmaras.rs` | Spalart-Allmaras 모델 |
| `src/builtin/rsm.rs` | Reynolds Stress Model (RSM) |
| `src/builtin/les.rs` | LES (Dynamic Smagorinsky 등) |
| `src/wall_functions.rs` | 벽함수 (log-law) |
| `src/custom.rs` | 커스텀 난류 모델 (JSON/Expression) |
| `src/model_template.rs` | 모델 템플릿 SDK |
| `src/validation.rs` | 모델 검증 도구 |

## 8. gfd-mesh — 메쉬 생성 & 적응

| 모듈 | 파일 | 구현 내용 |
|------|------|-----------|
| **구조격자** | `src/structured/cartesian.rs` | Cartesian 메쉬 |
| | `src/structured/curvilinear.rs` | Curvilinear 격자 |
| | `src/structured/o_grid.rs` | O-grid (원통/극좌표) |
| | `src/structured/grading.rs` | 비균일 간격 제어 |
| **비구조격자** | `src/unstructured/delaunay.rs` | Delaunay (Bowyer-Watson) |
| | `src/unstructured/tetrahedral.rs` | 사면체 메쉬 |
| | `src/unstructured/hexahedral.rs` | 육면체 메쉬 |
| | `src/unstructured/polyhedral.rs` | 다면체 메쉬 |
| | `src/unstructured/prism_layer.rs` | 경계층 프리즘 |
| **하이브리드** | `src/hybrid/cutcell.rs` | Cut-cell |
| | `src/hybrid/octree.rs` | Octree AMR |
| | `src/hybrid/overset.rs` | Overset (chimera) |
| **적응** | `src/adaptation/refinement.rs` | h-refinement |
| | `src/adaptation/coarsening.rs` | Coarsening |
| | `src/adaptation/solution_adaptive.rs` | Solution-adaptive |
| | `src/adaptation/feature_adaptive.rs` | Feature-based (곡률 감지) |
| **격자 이동** | `src/motion/body_fitted.rs` | Body-fitted 이동격자 |
| | `src/motion/deformation.rs` | 격자 변형 |
| | `src/motion/moving_mesh.rs` | 이동 메쉬 |
| | `src/motion/remeshing.rs` | ALE 리메싱 |
| **품질** | `src/quality/metrics.rs` | 품질 메트릭 (aspect ratio, skewness 등) |
| | `src/quality/repair.rs` | 메쉬 수리 |
| | `src/quality/smoother.rs` | Laplacian, Taubin 스무딩 |
| **형상** | `src/geometry/boolean_ops.rs` | Boolean 연산 (union/intersection/difference) |
| | `src/geometry/defeaturing.rs` | Defeaturing (CAD 정리) |
| | `src/geometry/distance_field.rs` | 거리장 계산 |
| | `src/geometry/marching_cubes.rs` | Marching Cubes (등값면 추출) |
| | `src/geometry/extrude.rs` | 돌출 |
| | `src/geometry/sketch.rs` | 스케치 프리미티브 |
| | `src/geometry/stl_reader.rs` | STL 리더 |
| | `src/geometry/surface_ops.rs` | 표면 연산 |
| | `src/geometry/transform.rs` | 변환 |
| | `src/geometry/analysis.rs` | 형상 분석 |
| | `src/geometry/cfd_prep.rs` | CFD 전처리 |
| | `src/geometry/primitives.rs` | 기본 형상 |
| **I/O** | `src/io/export.rs` | 메쉬 내보내기 |
| | `src/io/formats.rs` | 포맷 자동 감지 |

## 9. gfd-discretize — 이산화

| 모듈 | 파일 | 구현 내용 |
|------|------|-----------|
| **FVM** | `src/fvm/convection.rs` | 대류 이산화 |
| | `src/fvm/diffusion.rs` | 확산 이산화 |
| | `src/fvm/gradient.rs` | 구배 계산 |
| | `src/fvm/interpolation.rs` | 면 보간 |
| | `src/fvm/source.rs` | 소스항 처리 |
| | `src/fvm/temporal.rs` | 시간 이산화 |
| **FEM** | `src/fem/assembly.rs` | FEM 조립 |
| | `src/fem/quadrature.rs` | Quadrature 규칙 |
| | `src/fem/shape_fn.rs` | Shape functions |
| | `src/fem/weak_form.rs` | Weak form 구성 |
| Pipeline | `src/pipeline.rs` | Expression → 이산 방정식 변환 |

## 10. gfd-boundary — 경계조건

| 파일 | 구현 내용 |
|------|-----------|
| `src/standard/mod.rs` | Fixed Value (Dirichlet), Fixed Gradient (Neumann), Zero Gradient, No-Slip, Symmetry, Robin, Convective |
| `src/custom.rs` | Expression 기반 커스텀 BC |
| `src/profiles.rs` | 시간 의존 프로파일 |
| `src/synthetic_turbulence.rs` | Synthetic 난류 생성 (입구 BC) |
| `src/traits.rs` | BC trait 인터페이스 |

## 11. gfd-coupling — 다물리 커플링

| 모듈 | 파일 | 구현 내용 |
|------|------|-----------|
| **전략** | `src/strategy/fixed_point.rs` | Fixed-point 반복 |
| | `src/strategy/aitken.rs` | Aitken 가속 |
| | `src/strategy/anderson.rs` | Anderson 가속 |
| | `src/strategy/iqn_ils.rs` | IQN-ILS (Interface Quasi-Newton) |
| **매핑** | `src/mapping/nearest.rs` | Nearest-neighbor |
| | `src/mapping/rbf.rs` | RBF (Radial Basis Function) |
| | `src/mapping/mortar.rs` | Mortar element |
| Interface | `src/interface.rs` | 인터페이스 정의 & 수렴 모니터링 |
| Trait | `src/traits.rs` | 커플링 trait |

## 12. gfd-expression — 수식 엔진

| 파일 | 구현 내용 |
|------|-----------|
| `src/tokenizer.rs` | 토크나이저/렉서 |
| `src/parser.rs` | 파서 (AST 빌드) |
| `src/ast.rs` | AST 표현 & 조작 |
| `src/simplify.rs` | 대수적 단순화, 상수 폴딩 |
| `src/differentiate.rs` | 기호 미분 (chain/product/quotient rule) |
| `src/linearize.rs` | 소스 분해: S(phi) → Sc + Sp*phi |
| `src/dimension.rs` | 차원/단위 검사 (SI) |
| `src/validate.rs` | 다중 패스 검증 |
| `src/codegen_rust.rs` | Rust 코드 생성 |
| `src/codegen_latex.rs` | LaTeX 출력 |
| `src/codegen_json.rs` | JSON 직렬화 |

## 13. gfd-material — 물성치 데이터베이스

| 모듈 | 파일 | 구현 내용 |
|------|------|-----------|
| **유체** | `src/fluid/newtonian.rs` | Newtonian 유체 |
| | `src/fluid/non_newtonian.rs` | Non-Newtonian (power-law 등) |
| | `src/fluid/ideal_gas.rs` | 이상기체 모델 |
| **고체** | `src/solid/elastic.rs` | 탄성 물성 |
| | `src/solid/hyperelastic.rs` | 초탄성 물성 |
| | `src/solid/plasticity.rs` | 소성 물성 |
| 열물성 | `src/thermal.rs` | 열전도율, 비열, 방사율 |
| 데이터베이스 | `src/database.rs` | 내장 물성치 라이브러리 |
| Trait | `src/traits.rs` | 물성치 trait |

## 14. gfd-gpu — GPU 가속

| 모듈 | 파일 | 구현 내용 |
|------|------|-----------|
| Device | `src/device.rs` | CUDA 디바이스 감지 & 관리 |
| Memory | `src/memory.rs` | GPU 메모리 할당/해제 |
| Transfer | `src/transfer.rs` | Host-Device 전송 |
| Sparse | `src/sparse.rs` | GPU CSR 희소행렬 |
| **커널** | `src/kernels/gradient.rs` | Gradient 커널 |
| | `src/kernels/flux.rs` | Flux 커널 |
| | `src/kernels/correction.rs` | Correction 커널 |
| | `src/kernels/reduction.rs` | Reduction (norm, dot) 커널 |
| **솔버** | `src/solver/gpu_cg.rs` | GPU CG 솔버 |
| | `src/solver/amgx.rs` | AmgX 인터페이스 (stub) |

## 15. gfd-io — 입출력

| 파일 | 구현 내용 |
|------|-----------|
| `src/json_input.rs` | JSON 설정 파일 파서 |
| `src/mesh_reader/gmsh.rs` | Gmsh v2.2 리더 |
| `src/mesh_reader/stl.rs` | STL 리더 |
| `src/mesh_reader/cgns.rs` | CGNS 리더 |
| `src/vtk_writer.rs` | VTK Legacy 출력 |
| `src/vdb_writer.rs` | VDB 출력 |
| `src/checkpoint.rs` | Checkpoint/Restart |
| `src/probes.rs` | Probe 샘플링 (포인트 모니터) |

## 16. gfd-source — 소스항 & 체적력

| 파일 | 구현 내용 |
|------|-----------|
| `src/volume_source.rs` | 체적 소스항 |
| `src/momentum_source.rs` | 운동량 소스항 |
| `src/porous.rs` | 다공성 매체 (Darcy, Forchheimer) |
| `src/buoyancy.rs` | 부력 (Boussinesq) |
| `src/traits.rs` | 소스항 trait |

## 17. gfd-postprocess — 후처리

| 파일 | 구현 내용 |
|------|-----------|
| `src/derived_field.rs` | 유도 필드: Q-criterion, lambda-2, vorticity, Mach, strain rate, entropy |
| `src/integrals.rs` | 체적/면적 적분 (힘, 플럭스, 에너지, 질량, 운동량) |
| `src/statistics.rs` | 필드 통계 (min, max, mean, RMS, variance) |
| `src/traits.rs` | 후처리 trait |

## 18. gfd-parallel — 병렬화

| 파일 | 구현 내용 |
|------|-----------|
| `src/domain_decomp.rs` | 영역 분할 |
| `src/thread_pool.rs` | 공유 메모리 스레드 풀 |
| `src/mpi_comm.rs` | MPI 래퍼 & 통신 |
| `src/gpu/mod.rs` | GPU 병렬 통신 |

## 19. gfd-vdb — OpenVDB 지원

| 파일 | 구현 내용 |
|------|-----------|
| `src/tree/mod.rs` | VDB 트리 구조 (B+ tree 계층 격자) |
| `src/grid/mod.rs` | 희소 격자 표현 |
| `src/io/mod.rs` | VDB 파일 읽기/쓰기 |
| `src/codec/mod.rs` | 압축 코덱 |

## 20. Binary — 메인 바이너리

| 파일 | 구현 내용 |
|------|-----------|
| `src/main.rs` | CLI 엔트리포인트 (`cargo run --bin gfd`) |
| `src/server.rs` | JSON-RPC 서버 (GUI 연동) |
| `src/api.rs` | API 정의 |
| `src/config_gen.rs` | 설정 생성 유틸리티 |
| `src/lib.rs` | 라이브러리 인터페이스 |

---

## 요약 통계

| 분류 | 항목 수 |
|------|---------|
| 비압축 솔버 | 3 (SIMPLE, PISO, SIMPLEC) |
| 압축 솔버 | 3 (Roe, HLLC, AUSM+) |
| 다상 모델 | 5 (VOF, Level Set, Euler-Euler, DPM, Mixture) |
| 연소 모델 | 3 (Finite-rate, EDM, Eddy-Breakup) |
| 난류 모델 | 5 RANS + LES (k-e, k-w SST, SA, RSM, LES) |
| 열전달 | 7 (전도, 대류, 상변화, CHT, P1, DO, View Factor) |
| 고체역학 | 9 (탄성, 초탄성, Von Mises, Tresca, D-P, 동적, 접촉, 크리프, 열응력) |
| 선형 솔버 | 8 (CG, BiCGSTAB, GMRES, FGMRES, PCG, PBiCGSTAB, LU, Cholesky) |
| 전처리기 | 4 (Jacobi, ILU, AMG, Block) |
| 대류 스킴 | 6 (1st/2nd Upwind, Central, QUICK, TVD limiters) |
| 시간 스킴 | 4 (Euler, BDF2, Crank-Nicolson, RK4) |
| 메쉬 타입 | 8+ (Cartesian, Curvilinear, O-grid, Delaunay, Tet, Hex, Poly, Prism) |
| 적응 기법 | 4 (refinement, coarsening, solution, feature) |
| 격자 이동 | 4 (body-fitted, deformation, moving, ALE) |
| 커플링 전략 | 4 (Fixed-point, Aitken, Anderson, IQN-ILS) |
| 필드 매핑 | 3 (Nearest, RBF, Mortar) |
| 경계조건 | 7+ (Dirichlet, Neumann, Zero-grad, No-slip, Symmetry, Robin, Convective) |
| I/O 포맷 | 6 (JSON, Gmsh, STL, CGNS, VTK, VDB) |
