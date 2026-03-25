# GFD Autoresearch — 자율 솔버 최적화 루프

이것은 AI 에이전트가 GFD 멀티피직스 솔버를 자율적으로 개선하는 실험이다.

## 개요

에이전트는 GFD 솔버 코드를 수정하고, 고정된 벤치마크를 실행하고, 결과를 평가하여 개선/폐기를 반복한다. 사람은 잠을 자도 되고, 에이전트는 자동으로 계속 돌아간다.

## Setup

1. **브랜치 생성**: `git checkout -b autoresearch/<tag>` (예: `autoresearch/mar24`)
2. **프로젝트 파일 읽기**: 아래 파일들을 읽어 전체 컨텍스트를 파악한다:
   - `PROJECT_PLAN.md` — 전체 프로젝트 계획
   - `src/main.rs` — CLI 진입점
   - `crates/gfd-core/src/` — 핵심 자료구조 (mesh, field, linalg)
   - `crates/gfd-fluid/src/incompressible/simple.rs` — SIMPLE 유동 솔버
   - `crates/gfd-thermal/src/conduction.rs` — 열전도 솔버
   - `crates/gfd-linalg/src/iterative/` — 선형 솔버 (CG, BiCGSTAB, GMRES)
   - `crates/gfd-discretize/src/fvm/` — FVM 이산화
   - `crates/gfd-core/src/gradient/mod.rs` — 구배 계산
   - `benches/gfd_benchmark.rs` — 벤치마크 (이 파일이 evaluate 역할)
3. **빌드 확인**: `cargo build --release` 성공 확인
4. **베이스라인 실행**: `cargo run --release -- benchmark` 으로 베이스라인 메트릭 수집
5. **results.tsv 초기화**: 헤더 행만 있는 results.tsv 생성
6. **확인 후 시작**

## 벤치마크 시스템

각 실험은 **고정된 벤치마크 케이스**를 실행한다. 벤치마크는 수정 불가(READ-ONLY):

### 벤치마크 케이스 (benches/gfd_benchmark.rs)

| 케이스 | 설명 | 핵심 메트릭 |
|--------|------|-----------|
| **heat_1d** | 1D 정상 열전도, T_L=100, T_R=200, 50셀 | error_l2 (해석해 대비) |
| **heat_source** | 1D 소스항 열전도, 100셀 | error_l2 |
| **cavity_20** | 20×20 lid-driven cavity, Re=100 | iterations_to_converge, residual |
| **cavity_50** | 50×50 lid-driven cavity, Re=100 | iterations_to_converge, wall_time_ms |
| **cavity_100** | 100×100 lid-driven cavity, Re=100 | iterations_to_converge, wall_time_ms |

### 메트릭

벤치마크 실행 후 다음이 출력된다:

```
---
heat_1d_error:        1.23e-10
heat_source_error:    4.56e-08
cavity_20_iters:      39
cavity_20_residual:   9.62e-05
cavity_50_iters:      150
cavity_50_time_ms:    1234
cavity_100_iters:     500
cavity_100_time_ms:   12345
total_benchmark_ms:   15000
all_tests_pass:       true
---
```

## 수정 가능/불가 범위

**수정 가능 (CAN modify):**
- `crates/gfd-fluid/src/incompressible/simple.rs` — SIMPLE 알고리즘 개선
- `crates/gfd-fluid/src/incompressible/piso.rs` — PISO 알고리즘 구현
- `crates/gfd-linalg/src/iterative/*.rs` — 선형 솔버 개선
- `crates/gfd-linalg/src/preconditioner/*.rs` — 전처리기 구현 (ILU, AMG)
- `crates/gfd-core/src/gradient/mod.rs` — 구배 계산 개선
- `crates/gfd-core/src/interpolation/mod.rs` — 보간 개선
- `crates/gfd-core/src/numerics/*.rs` — 수치 스킴 (TVD, MUSCL 등)
- `crates/gfd-discretize/src/fvm/*.rs` — FVM 이산화 개선
- `crates/gfd-thermal/src/conduction.rs` — 열전도 솔버 개선
- `crates/gfd-matrix/src/*.rs` — 행렬 조립/경계조건 개선
- `crates/gfd-core/src/mesh/*.rs` — 메시 자료구조 개선

**수정 불가 (CANNOT modify):**
- `benches/gfd_benchmark.rs` — 벤치마크는 고정 (evaluate 역할)
- `examples/*.json` — 예제 설정 파일
- `Cargo.toml` (루트) — 의존성 추가 금지
- `crates/*/Cargo.toml` — 외부 의존성 추가 금지

**목표: 벤치마크 메트릭을 개선하라.**

우선순위:
1. **정확성**: heat_*_error 감소 (해석해에 더 가까이)
2. **수렴 속도**: cavity_*_iters 감소 (더 적은 반복에 수렴)
3. **성능**: cavity_*_time_ms 감소 (더 빠른 실행)
4. **안정성**: 더 큰 격자(100×100)에서도 안정적 수렴

## 개선 아이디어 카탈로그

에이전트가 시도할 수 있는 구체적 개선 방향:

### 선형 솔버 개선
- ILU(0) 전처리기 구현 → CG/BiCGSTAB 수렴 가속
- AMG 전처리기 구현 → 압력 Poisson 가속
- GMRES 재시작 파라미터 최적화
- 수렴 판정 기준 개선

### SIMPLE 알고리즘 개선
- Under-relaxation factor 자동 조절 (Aitken)
- SIMPLEC 구현 (더 빠른 수렴)
- PISO 구현 (비정상 문제에 유리)
- Rhie-Chow 보간 구현 (체커보드 압력 방지)
- 운동량 보간 개선 (2차 정확도)
- 비직교 보정 (non-orthogonal correction)

### 수치 스킴 개선
- 2차 업윈드 (Second Order Upwind) 대류 스킴
- TVD 스킴 (Van Leer, Minmod, Superbee) 실제 적용
- MUSCL 재구성
- 구배 제한기 (gradient limiter)
- Green-Gauss Node-Based 구배 (더 정확)
- Least Squares 구배 (비직교 메시에 더 강건)

### 행렬/메모리 최적화
- CSR 행렬 연산 최적화 (SIMD 등)
- 행렬 재사용 (행렬이 변하지 않을 때 재조립 회피)
- 메모리 할당 최소화 (벡터 재사용)

### 열전도 솔버 개선
- 조화 평균 열전도율 (harmonic mean) at faces
- 비직교 보정
- 고차 이산화

## results.tsv 형식

```
commit	metric	value	status	description
```

| 열 | 설명 |
|----|------|
| commit | git commit hash (7자) |
| metric | 핵심 메트릭 이름 (cavity_50_iters가 대표) |
| value | 메트릭 값 |
| status | keep / discard / crash |
| description | 이 실험에서 시도한 것 |

## 실험 루프

**영원히 반복:**

1. 현재 git 상태 확인
2. 개선 아이디어 선택 → `train.py` 대신 **솔버 소스 코드** 수정
3. `cargo build --release` — 빌드 실패 시 수정 시도, 3회 실패 시 폐기
4. `cargo test --workspace` — 기존 테스트 깨지면 폐기
5. `cargo run --release -- benchmark > run.log 2>&1` — 벤치마크 실행
6. 결과 파싱: `grep "cavity_50_iters:\|cavity_50_time_ms:\|heat_1d_error:\|all_tests_pass:" run.log`
7. results.tsv에 기록
8. 개선되었으면 **keep** (git commit 유지)
9. 같거나 악화되었으면 **discard** (git reset --hard 이전 커밋으로)
10. 다음 아이디어로 이동

**타임아웃**: 벤치마크가 5분 이상 걸리면 kill하고 crash 처리.

**크래시**: 빌드 실패나 벤치마크 오류 시, 쉽게 고칠 수 있으면 수정. 근본적 문제면 skip.

**절대 멈추지 마라**: 실험 루프가 시작되면, 인간에게 "계속할까요?"를 물어보지 마라. 아이디어가 고갈되면 더 깊이 생각하라 — 코드를 다시 읽고, 논문의 알고리즘을 참조하고, 이전 실패를 조합해보고, 더 급진적인 변경을 시도하라. 인간이 수동으로 중단할 때까지 루프는 계속된다.
