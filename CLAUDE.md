# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GFD (Generalized Fluid Dynamics) — Rust로 작성된 통합 멀티피직스 솔버. CFD(비압축/압축), 열전달(전도/대류/복사), 고체역학(선형 탄성 FEM)을 단일 바이너리로 해석. JSON 입력, VTK 출력(ParaView 호환). 203 Rust 파일, 28,376줄, 18 크레이트, 212 테스트, 0 todo!().

## Build & Test

```bash
cargo build --release                                     # 빌드
cargo test --workspace                                    # 전체 테스트 (212개)
cargo test -p gfd-fluid                                   # 단일 크레이트
cargo test -p gfd-thermal steady_1d                       # 특정 테스트
cargo run --release --bin gfd -- run examples/lid_driven_cavity.json   # 시뮬레이션 실행
cargo run --release --bin gfd -- run examples/heat_conduction.json
cargo run --release --bin gfd-benchmark                   # 벤치마크 (5 케이스, READ-ONLY)
cargo build --release --features gpu                      # GPU 빌드 (CUDA 필요)
```

두 바이너리가 있으므로 `--bin gfd` 또는 `--bin gfd-benchmark`을 명시해야 함.

## Architecture — Crate Dependency Graph

```
Layer 0 (leaf):    gfd-core          gfd-expression
                      ↑                    ↑
Layer 1:     gfd-matrix  gfd-discretize  gfd-boundary  gfd-source  gfd-material
             gfd-turbulence  gfd-coupling  gfd-gpu  gfd-io  gfd-parallel
             gfd-postprocess  gfd-vdb
                      ↑
Layer 2:           gfd-linalg (→ gfd-core + gfd-matrix)
                      ↑
Layer 3 (physics): gfd-fluid  gfd-thermal  gfd-solid
                      ↑
Layer 4 (binary):  src/main.rs
```

## Key Crates

| Crate | Role |
|-------|------|
| **gfd-core** | `UnstructuredMesh`, `StructuredMesh`, `ScalarField`/`VectorField`/`TensorField`, `SparseMatrix` (CSR, optimized unsafe `spmv`), Green-Gauss gradient, linear interpolation |
| **gfd-linalg** | **프로덕션 선형 솔버**: `CG`, `BiCGSTAB`, `GMRES`, `PCG`, `PBiCGSTAB`, `ILU0`/`Jacobi` preconditioner |
| **gfd-matrix** | `Assembler` (COO→CSR, counting sort), `apply_dirichlet`/`apply_neumann`, `CooMatrix` |
| **gfd-expression** | 수학식 파서 (tokenizer→AST→LaTeX/Rust codegen), 심볼릭 미분, 차원 분석, GMN 문법 |
| **gfd-fluid** | SIMPLE/PISO/SIMPLEC, Roe/HLLC/AUSM+, VOF/LevelSet/Euler-Euler, k-ε/k-ω SST/LES |
| **gfd-thermal** | 정상/비정상 열전도, 대류-확산, P-1/DO 복사, 상변화, 공액열전달 |
| **gfd-solid** | Hex8 FEM (2×2×2 Gauss), Von Mises 소성, Newmark-β 동역학, 접촉, 크리프 |
| **gfd-gpu** | CUDA 추상화 (`cudarc`), `GpuCG` (CPU 폴백), AmgX 스텁, feature `cuda` |
| **gfd-io** | JSON config 파싱, Gmsh v2.2 리더, STL 리더, VTK Legacy 출력, 체크포인트, 프로브 |

## Critical Design Decisions

### 1. 이중 LinearSolver 트레이트 (혼동 주의)

- `gfd_core::linalg::solvers::LinearSolver` — `&mut LinearSystem` 인터페이스. **기본 구현** (gfd-core 내장).
- `gfd_linalg::traits::LinearSolverTrait` — `(&SparseMatrix, &[f64], &mut [f64])` 인터페이스. **프로덕션 구현**. 물리 솔버는 반드시 이것을 사용.

### 2. FVM 솔버 패턴 (모든 물리 솔버가 동일)

```rust
// 1. 면 계수 사전 계산 (comp 루프 밖)
for face in &mesh.faces { /* D, F 계산 */ }

// 2. 성분별 조립 + 풀이
for comp in 0..3 {
    let mut assembler = Assembler::with_nnz_estimate(n, n + 2*n_internal);
    // 면 루프: assembler.add_diagonal(), add_neighbor()
    // 소스/BC: assembler.add_source()
    assembler.finalize() → LinearSystem
    BiCGSTAB::solve() or CG::solve()
}
```

### 3. 메시 생성

`StructuredMesh::uniform(nx, ny, nz, lx, ly, lz).to_unstructured()` — 구조격자를 비정렬 메시로 변환하여 단일 FVM 코드 경로 사용. `nz=0`이면 2D (실제로는 nz=1 단층 3D hex).

### 4. GPU 통합

`--features gpu` (Cargo feature flag). `gfd-gpu` 크레이트가 `cudarc`로 CUDA 래핑. `cuda` feature 없으면 모든 GPU 경로가 CPU 폴백. `simple.rs`의 `solve_linear_system()` 헬퍼가 CPU/GPU 디스패치.

## Autoresearch System

자율 솔버 최적화 루프 (`program.md` 참조):
1. 솔버 코드 수정 → `cargo build --release`
2. `cargo test --workspace` (기존 테스트 깨지면 폐기)
3. `cargo run --release --bin gfd-benchmark` → 메트릭 비교
4. 개선되면 keep, 아니면 discard
5. `results.tsv`에 기록

벤치마크 (`benches/gfd_benchmark.rs`)는 READ-ONLY:
- `heat_1d`: 50셀 1D 열전도 (해석해 비교, 오차 ~2e-12)
- `heat_source`: 100셀 소스항 열전도
- `cavity_20/50/100`: lid-driven cavity Re=100

### 확인된 최적화 결과 (results.tsv)

| 성공 | 실패 |
|------|------|
| 면 계수 사전 계산 (-3.7%) | ILU(0) 전처리기 (setup 비용 > 효과) |
| COO→CSR counting sort (-5.5%) | Rhie-Chow 보간 (수렴 파괴) |
| unsafe SpMV (-13.7%, **최대 효과**) | p' warm start (+33% 느림) |
| 4-way unrolled dot (-0.3%) | |
| Assembler 직접 추가 (-6.8%) | |

## Long-Term Memory

프로젝트별 장기기억이 대화 간 컨텍스트를 유지:

```
~/.claude/projects/C--Users-sdd32-OneDrive----GitHub-GFD-DEV/memory/
├── MEMORY.md                          # 인덱스 (먼저 읽기)
├── user_profile.md                    # 사용자: CFD 엔지니어, 한국어, 대규모 위임 스타일
├── feedback_coding_style.md           # 피드백: 병렬 에이전트 선호, 최적화 실험 성패
├── project_gfd_overview.md            # 프로젝트: 203파일, 동작하는 솔버 5개
├── project_architecture_decisions.md  # 설계: 이중 LinearSolver, Assembler 패턴
├── reference_gpu_plan.md              # 참조: cudarc/AmgX GPU 통합
└── reference_autoresearch.md          # 참조: autoresearch 루프 사용법
```

새 대화 시작 시 MEMORY.md → 관련 기억 파일 순서로 읽어 이전 작업 컨텍스트를 복원할 것. 새 결정/피드백은 해당 .md에 즉시 반영.

## Key Documents

| 문서 | 위치 | 내용 |
|------|------|------|
| 종합 계획서 | `PROJECT_PLAN.md` | 4,734줄. 솔버 목록, 수학식, 아키텍처, Fluent 워크플로우, SDK, GPU 계획 |
| 자율 연구 지침 | `program.md` | autoresearch 에이전트 행동 규칙 |
| 실험 결과 | `results.tsv` | 최적화 실험 기록 (commit, metric, status) |
| 열전도 예제 | `examples/heat_conduction.json` | 20×1 정상 열전도 |
| 유동 예제 | `examples/lid_driven_cavity.json` | 20×20 lid-driven cavity Re=100 |

## Language

사용자와는 한국어로 소통. 기술 용어와 코드 식별자는 영어 유지.
