# GFD Mesher — 통합 메시 생성기 구현 계획서

> **목표:** Fluent Meshing / ANSYS Meshing / snappyHexMesh 수준의 자동 메시 생성기를 Rust로 구현
> **위치:** `crates/gfd-mesh/` (새 크레이트)
> **참조:** Gmsh, CGAL, TetGen, snappyHexMesh, Pointwise, Fluent Meshing

---

## 1. 아키텍처

```
crates/gfd-mesh/
├── src/
│   ├── lib.rs                    # 메셔 크레이트 진입점
│   ├── structured/               # Phase 1: 정렬격자
│   │   ├── mod.rs
│   │   ├── cartesian.rs          # FDS식 직교 정렬격자
│   │   ├── curvilinear.rs        # 곡선 좌표계 격자
│   │   ├── o_grid.rs             # O-grid (원통/터빈)
│   │   └── grading.rs            # 벽면 clustering, geometric ratio
│   ├── unstructured/             # Phase 2: 비정렬격자
│   │   ├── mod.rs
│   │   ├── delaunay.rs           # Delaunay 삼각분할 (2D/3D)
│   │   ├── tetrahedral.rs        # 4면체 메시 (TetGen식)
│   │   ├── hexahedral.rs         # 6면체 메시 (매핑/스위핑)
│   │   ├── polyhedral.rs         # 다면체 (12면체 등, Voronoi 듀얼)
│   │   └── prism_layer.rs        # 벽면 프리즘 레이어
│   ├── hybrid/                   # Phase 3: 하이브리드
│   │   ├── mod.rs
│   │   ├── cutcell.rs            # Cut-cell (IBM식)
│   │   ├── overset.rs            # Overset/Chimera 메시
│   │   └── octree.rs             # Octree 기반 적응격자 (AMR)
│   ├── adaptation/               # Phase 4: 적응형
│   │   ├── mod.rs
│   │   ├── refinement.rs         # h-refinement (셀 분할)
│   │   ├── coarsening.rs         # 셀 병합
│   │   ├── solution_adaptive.rs  # 유동인식형 (gradient/error 기반)
│   │   └── feature_adaptive.rs   # 형상인식형 (곡률/근접도)
│   ├── motion/                   # Phase 5: 동적 메시
│   │   ├── mod.rs
│   │   ├── moving_mesh.rs        # ALE 메시 이동
│   │   ├── deformation.rs        # 스프링/확산 기반 변형
│   │   ├── remeshing.rs          # 리메싱 (품질 저하 시)
│   │   └── body_fitted.rs        # 물체 추종형 메시
│   ├── quality/                  # 품질 관리
│   │   ├── mod.rs
│   │   ├── metrics.rs            # 품질 메트릭 (aspect ratio, skewness, orthogonality)
│   │   ├── smoother.rs           # Laplacian/Optimization 스무딩
│   │   └── repair.rs             # 불량 셀 수정
│   ├── geometry/                 # 형상 입력
│   │   ├── mod.rs
│   │   ├── stl_reader.rs         # STL 형상 읽기
│   │   ├── primitives.rs         # Box, Sphere, Cylinder 등
│   │   ├── boolean_ops.rs        # Union, Subtract, Intersect
│   │   └── distance_field.rs     # Signed Distance Function
│   └── io/                       # 입출력
│       ├── mod.rs
│       ├── export.rs             # 내부 → UnstructuredMesh 변환
│       └── formats.rs            # Gmsh, CGNS, Fluent .msh 포맷
```

---

## 2. Phase별 구현 계획

### Phase 1: 정렬격자 (Structured Grid)

**참조:** FDS (NIST), Plot3D, OpenFOAM blockMesh

| 기능 | 설명 | 난이도 |
|------|------|--------|
| **직교 정렬격자** | nx×ny×nz 균일/비균일 직교격자. FDS 스타일 | 쉬움 |
| **Geometric grading** | 벽면 근처 셀 크기를 기하급수적으로 줄임. `first_cell_height`, `growth_ratio`, `num_layers` | 쉬움 |
| **Hyperbolic tangent grading** | tanh 분포로 양 끝 집중. 채널/파이프 유동 | 쉬움 |
| **Multi-block** | 여러 블록을 연결하여 L자, T자 형상 | 중간 |
| **O-grid** | 원통 주위 O형 격자 (터빈 블레이드, 파이프) | 중간 |
| **Curvilinear** | 곡선 좌표계 (airfoil 주위) | 어려움 |

**핵심 수식:**
```
Geometric grading: h_i = h_1 * r^(i-1), where r = growth_ratio
Tanh grading:      s(xi) = 1 + tanh(delta*(xi - 0.5)) / tanh(delta/2)
```

### Phase 2: 비정렬격자 (Unstructured Grid)

**참조:** Gmsh, TetGen, CGAL, Netgen

| 기능 | 설명 | 난이도 |
|------|------|--------|
| **2D Delaunay 삼각분할** | Bowyer-Watson 알고리즘. 점 삽입 + 플립 | 중간 |
| **3D Delaunay 테트라** | Bowyer-Watson 3D 확장. 4면체 생성 | 어려움 |
| **Advancing Front** | 경계에서 안쪽으로 성장하는 메시 | 어려움 |
| **Voronoi → Polyhedral** | Delaunay의 듀얼 → 다면체 메시 (Fluent poly mesh) | 중간 |
| **Prism Layer** | 벽면에 프리즘(쐐기) 레이어 삽입. y+ 제어 | 중간 |
| **Size field** | 공간별 셀 크기 지정 (proximity, curvature) | 중간 |

**핵심 알고리즘:**
```
Bowyer-Watson:
1. 초기 super-triangle/tetrahedron 생성
2. 점 P 삽입
3. P를 포함하는 circumsphere를 가진 모든 셀 제거 → cavity
4. Cavity 경계와 P를 연결하여 새 셀 생성
5. 반복

Prism Layer:
1. 벽면 법선 방향으로 첫 번째 셀 높이 h1 배치
2. Growth ratio r로 n_layers개 레이어 생성
3. 내부 메시와 연결
```

### Phase 3: 하이브리드/특수 메시

**참조:** snappyHexMesh, AMReX, IBAMR

| 기능 | 설명 | 난이도 |
|------|------|--------|
| **Cut-cell (IBM)** | 직교격자 + STL 형상으로 잘린 셀 생성 | 어려움 |
| **Octree AMR** | 8진트리 기반 적응격자. 국소 세분화 | 어려움 |
| **Overset/Chimera** | 겹치는 격자. 배경 + 물체 격자 보간 | 매우 어려움 |

**Cut-cell 알고리즘:**
```
1. 배경 직교격자 생성
2. STL 형상의 SDF (Signed Distance Field) 계산
3. SDF < 0인 셀: solid (제거 또는 비활성화)
4. SDF 부호가 변하는 셀: cut-cell
   - 형상과 셀 변의 교차점 계산
   - 잘린 면적/부피 계산
   - 셀 중심 재계산
5. 작은 cut-cell은 이웃과 병합 (cell merging)
```

### Phase 4: 적응형 메시

**참조:** libMesh, deal.II, p4est

| 기능 | 설명 | 난이도 |
|------|------|--------|
| **h-refinement** | 셀을 2^d개로 분할 (isotropic) | 중간 |
| **Anisotropic refinement** | 특정 방향으로만 분할 (경계층 등) | 어려움 |
| **유동인식형 (Solution-adaptive)** | gradient, error estimator 기반 세분화 | 중간 |
| **형상인식형 (Feature-adaptive)** | 곡률, 근접도, 사용자 지정 영역 세분화 | 중간 |
| **Coarsening** | 셀 병합 (uniform 영역에서) | 중간 |

**세분화 기준:**
```
Gradient-based:  refine if |grad(phi)| * h > threshold
Error-based:     refine if ||e_h||_cell > theta * max(||e_h||)
Curvature-based: refine if kappa * h > threshold
Proximity-based: refine if distance_to_wall < n_layers * h
```

### Phase 5: 동적 메시 (Moving/Deforming Mesh)

**참조:** OpenFOAM dynamicMesh, Fluent sliding/deforming

| 기능 | 설명 | 난이도 |
|------|------|--------|
| **Spring-based smoothing** | 스프링 유추법으로 메시 변형 | 중간 |
| **Diffusion-based smoothing** | 라플라스 방정식으로 변위 확산 | 중간 |
| **Remeshing** | 품질 저하 시 국소 리메싱 | 어려움 |
| **물체 추종형** | 움직이는 물체 주위 메시 갱신 | 어려움 |
| **ALE (Arbitrary Lagrangian-Eulerian)** | 메시 속도 포함한 수송방정식 | 어려움 |

**스프링 기반 스무딩:**
```
각 내부 노드 i에 대해:
  x_i^new = x_i + omega * sum_j(k_ij * (x_j - x_i)) / sum_j(k_ij)

k_ij = 1/|x_i - x_j|  (inverse distance spring stiffness)
omega = under-relaxation factor (0.5 ~ 0.8)
```

---

## 3. 품질 메트릭

Fluent/ICEM에서 사용하는 메시 품질 지표:

| 메트릭 | 수식 | 좋은 값 |
|--------|------|---------|
| **Aspect Ratio** | max_edge / min_edge | < 5 (이상적: 1) |
| **Skewness** | (optimal_size - actual_size) / optimal_size | < 0.85 |
| **Orthogonality** | cos(angle between face normal and cell-center vector) | > 0.1 |
| **Volume Ratio** | V_cell / V_neighbor | 0.5 ~ 2.0 |
| **Minimum Angle** | min interior angle of faces | > 18° |
| **Jacobian** | det(J) of element mapping | > 0 |

```rust
pub struct MeshQuality {
    pub aspect_ratio: f64,
    pub skewness: f64,
    pub orthogonality: f64,
    pub min_angle: f64,
    pub volume_ratio: f64,
}
```

---

## 4. 구현 우선순위

| 순위 | 항목 | Phase | 이유 |
|------|------|-------|------|
| **1** | 정렬격자 + Grading | 1 | 기존 StructuredMesh 대체, 가장 기본 |
| **2** | 품질 메트릭 + 스무딩 | Quality | 모든 메셔에 필요 |
| **3** | 2D Delaunay 삼각분할 | 2 | 비정렬의 기초 |
| **4** | Prism Layer | 2 | CFD 필수 (y+ 제어) |
| **5** | 3D Tetrahedral | 2 | 범용 비정렬 |
| **6** | Polyhedral (Voronoi) | 2 | Fluent 스타일 |
| **7** | Cut-cell | 3 | 복잡 형상 자동 메싱 |
| **8** | Octree AMR | 3 | 적응형 메시 |
| **9** | h-refinement | 4 | 유동인식형 기초 |
| **10** | Solution-adaptive | 4 | gradient 기반 세분화 |
| **11** | Spring smoothing | 5 | 동적 메시 기초 |
| **12** | Overset | 3 | 고급 기능 |

---

## 5. API 설계

```rust
use gfd_mesh::*;

// 1. 정렬격자 + 벽면 grading
let mesh = CartesianMesh::new(100, 50, 1)
    .domain(10.0, 5.0, 0.1)
    .wall_grading("ymin", GradingSpec::geometric(1e-4, 1.2, 20))
    .wall_grading("ymax", GradingSpec::geometric(1e-4, 1.2, 20))
    .build()?;

// 2. 비정렬 테트라 메시
let mesh = TetMesher::new()
    .geometry(Geometry::stl("wing.stl"))
    .max_cell_size(0.1)
    .min_cell_size(0.001)
    .prism_layers(PrismSpec::new(1e-5, 1.2, 15))
    .curvature_refinement(true)
    .build()?;

// 3. Cut-cell 메시
let mesh = CutCellMesher::new()
    .background(CartesianMesh::uniform(200, 100, 50))
    .subtract(Geometry::stl("car.stl"))
    .min_cut_volume_fraction(0.1)
    .build()?;

// 4. 적응형 세분화
let refined = mesh.refine_where(|cell| {
    gradient_magnitude(cell) > threshold
})?;

// 5. 품질 확인
let quality = mesh.compute_quality();
println!("Min orthogonality: {}", quality.min_orthogonality);
println!("Max skewness: {}", quality.max_skewness);
println!("Max aspect ratio: {}", quality.max_aspect_ratio);
```

---

## 6. 참조 오픈소스

| 프로젝트 | 언어 | 핵심 기능 | 참조 포인트 |
|---------|------|----------|-----------|
| **Gmsh** | C++ | Delaunay, Frontal, 형상엔진 | 알고리즘, API 디자인 |
| **TetGen** | C++ | Constrained Delaunay 3D | 4면체 알고리즘 |
| **CGAL** | C++ | Delaunay, Voronoi, 메시 최적화 | 수학적 정확성 |
| **snappyHexMesh** | C++ | Cut-cell + refinement | Cut-cell 전략 |
| **Netgen** | C++ | Advancing front 3D | AF 알고리즘 |
| **p4est** | C | Octree AMR (분산 병렬) | AMR 데이터 구조 |
| **Triangle** | C | 2D CDT (Shewchuk) | 2D Delaunay 최적화 |

---

## 7. 예상 코드량

| Phase | 파일 수 | 예상 줄 수 | 테스트 수 |
|-------|--------|-----------|----------|
| Phase 1 (정렬) | 6 | ~2,000 | ~20 |
| Phase 2 (비정렬) | 7 | ~5,000 | ~30 |
| Phase 3 (하이브리드) | 4 | ~3,000 | ~15 |
| Phase 4 (적응형) | 5 | ~2,000 | ~15 |
| Phase 5 (동적) | 5 | ~2,000 | ~10 |
| Quality | 3 | ~1,000 | ~15 |
| Geometry | 4 | ~1,500 | ~10 |
| I/O | 3 | ~500 | ~5 |
| **합계** | **~37** | **~17,000** | **~120** |

---

## 8. 기존 코드와의 통합

현재 GFD는 `gfd-core`의 `StructuredMesh::uniform().to_unstructured()`를 사용합니다.

새 메셔는 `gfd-mesh` 크레이트로 독립 구현하되, 최종 출력은 `gfd_core::UnstructuredMesh`로 변환합니다:

```rust
// gfd-mesh의 모든 메셔는 이 trait를 구현
pub trait MeshGenerator {
    fn build(&self) -> Result<gfd_core::UnstructuredMesh>;
    fn quality(&self) -> MeshQuality;
}
```

`main.rs`에서 `create_mesh()` 함수를 확장하여 JSON config로 메시 유형을 선택:

```json
{
  "mesh": {
    "type": "cartesian",
    "nx": 100, "ny": 50,
    "grading": {
      "ymin": {"type": "geometric", "first_height": 1e-4, "ratio": 1.2, "layers": 20}
    }
  }
}
```
