# GFD 차세대 구현 계획서 — GUI + GPU + MPI

> **목표:** Fluent/CFX 수준의 GUI와 고성능 병렬 컴퓨팅
> **프론트엔드:** Electron + React + Three.js
> **백엔드:** Rust (기존 GFD 솔버) + IPC (JSON-RPC over stdin/stdout)
> **참조:** ANSYS Fluent, CFX, OpenFOAM + ParaView, SimScale

---

## 전체 로드맵 요약 (1~8번)

| # | 항목 | 상태 | 이번 구현 |
|---|------|------|----------|
| 1 | **GUI / 시각화** | 🔴 미구현 | ✅ Electron+React+Three.js |
| 2 | 문서화 / 튜토리얼 | 🟡 SOLVER_API.md만 | 향후 |
| 3 | CI/CD | 🟡 미구현 | 향후 |
| 4 | **GPU 가속** | 🔴 스텁만 | ✅ CUDA SpMV 연결 |
| 5 | **MPI 병렬** | 🔴 스텁만 | ✅ 메시 파티셔닝 + 분산 솔버 |
| 6 | 산업 검증 | 🟡 기본 예제만 | 향후 |
| 7 | Python 바인딩 | 🔴 미구현 | 향후 |
| 8 | 패키징/배포 | 🟡 cargo만 | 향후 |

---

## 1. GUI (Electron + React + Three.js)

### 1.1 아키텍처

```
┌─────────────────────────────────────────────────┐
│  Electron App (Node.js + Chromium)               │
│  ┌────────────┐ ┌────────────┐ ┌──────────────┐ │
│  │ React UI   │ │ Three.js   │ │ Monaco Editor│ │
│  │ (설정패널) │ │ (3D뷰어)   │ │ (JSON편집)   │ │
│  └─────┬──────┘ └─────┬──────┘ └──────┬───────┘ │
│        │              │               │          │
│  ┌─────┴──────────────┴───────────────┴───────┐  │
│  │  IPC Bridge (JSON-RPC over child_process)  │  │
│  └─────────────────────┬──────────────────────┘  │
└────────────────────────┼─────────────────────────┘
                         │ stdin/stdout JSON
┌────────────────────────┼─────────────────────────┐
│  GFD Rust Backend      │                          │
│  ┌─────────────────────┴──────────────────────┐  │
│  │  gfd-server (JSON-RPC handler)              │  │
│  │  ├─ mesh_generate(config) → mesh_info       │  │
│  │  ├─ solve_start(config) → job_id            │  │
│  │  ├─ solve_status(job_id) → progress         │  │
│  │  ├─ solve_stop(job_id)                      │  │
│  │  ├─ get_field(job_id, field) → data         │  │
│  │  ├─ get_residuals(job_id) → [iter, res]     │  │
│  │  └─ export_vtk(job_id, path)                │  │
│  └────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────┘
```

### 1.2 GUI 레이아웃 (Fluent 참조)

[ANSYS Fluent GUI Components](https://ansyshelp.ansys.com/public//Views/Secured/corp/v251/en/flu_ug/flu_ug_sec_gui_components.html) 참고:

```
┌─────────────────────────────────────────────────────────────┐
│  Menu Bar: File | Edit | Mesh | Physics | Solve | Results   │
├──────┬───────────────────────────────────────┬──────────────┤
│      │                                       │              │
│  O   │         3D Viewport                   │  Properties  │
│  u   │         (Three.js)                    │  Panel       │
│  t   │                                       │              │
│  l   │  ┌─────────────────────────────────┐  │  ┌────────┐  │
│  i   │  │  메시/유동장/온도장 시각화      │  │  │ 경계   │  │
│  n   │  │  마우스 회전/줌/이동            │  │  │ 조건   │  │
│  e   │  │  컬러맵, 벡터, 등고선, 유선    │  │  │ 설정   │  │
│      │  │                                 │  │  │        │  │
│  T   │  └─────────────────────────────────┘  │  │ 재질   │  │
│  r   │                                       │  │ 속성   │  │
│  e   ├───────────────────────────────────────┤  │        │  │
│  e   │  Residual Monitor / Console           │  │ 솔버   │  │
│      │  ┌────────┐ ┌────────┐ ┌──────────┐  │  │ 설정   │  │
│      │  │잔차그래프│ │로그출력│ │프로브값  │  │  │        │  │
│      │  └────────┘ └────────┘ └──────────┘  │  └────────┘  │
├──────┴───────────────────────────────────────┴──────────────┤
│  Status Bar: Iteration 35/200 | Residual: 9.8e-5 | GPU: ON │
└─────────────────────────────────────────────────────────────┘
```

### 1.3 Outline Tree (왼쪽 패널) — Fluent 스타일

```
📁 Setup
  ├── 📄 General (2D/3D, Steady/Transient)
  ├── 📁 Models
  │   ├── Viscous (k-ε, k-ω SST, SA, LES...)
  │   ├── Energy (On/Off)
  │   ├── Radiation (None, P-1, DOM)
  │   ├── Multiphase (None, VOF, Euler, Mixture, DPM)
  │   └── Species (None, Species Transport, Combustion)
  ├── 📁 Materials
  │   ├── Fluid (density, viscosity, Cp, k)
  │   └── Solid (E, nu, rho)
  ├── 📁 Boundary Conditions
  │   ├── inlet (velocity/pressure)
  │   ├── outlet (pressure)
  │   ├── wall (no-slip/moving/thermal)
  │   └── symmetry
  └── 📁 Mesh
      ├── Quality Check
      ├── Adapt/Refine
      └── Display
📁 Solution
  ├── Methods (SIMPLE/PISO/SIMPLEC)
  ├── Relaxation Factors
  ├── Initialization
  ├── Run Calculation
  └── Monitors
📁 Results
  ├── Contours
  ├── Vectors
  ├── Streamlines
  ├── Probes
  └── Reports
```

### 1.4 React 컴포넌트 구조

```
gui/
├── package.json              # Electron + React + Three.js
├── electron/
│   ├── main.ts               # Electron main process
│   ├── preload.ts            # IPC bridge
│   └── gfd-bridge.ts         # Rust child_process 관리
├── src/
│   ├── App.tsx               # 메인 레이아웃
│   ├── components/
│   │   ├── OutlineTree.tsx    # 왼쪽 트리 (Fluent 스타일)
│   │   ├── Viewport3D.tsx    # Three.js 3D 뷰어
│   │   ├── PropertiesPanel.tsx # 오른쪽 속성 패널
│   │   ├── ResidualPlot.tsx  # 수렴 그래프 (Chart.js)
│   │   ├── ConsolePanel.tsx  # 솔버 로그 출력
│   │   ├── StatusBar.tsx     # 하단 상태바
│   │   └── MenuBar.tsx       # 상단 메뉴
│   ├── viewport/
│   │   ├── MeshRenderer.tsx  # 메시 와이어프레임/서피스
│   │   ├── ContourMap.tsx    # 스칼라장 컬러맵
│   │   ├── VectorField.tsx   # 속도 벡터 화살표
│   │   ├── Streamlines.tsx   # 유선
│   │   └── ColorBar.tsx      # 범례
│   ├── panels/
│   │   ├── MeshPanel.tsx     # 메시 생성/품질
│   │   ├── PhysicsPanel.tsx  # 물리 모델 설정
│   │   ├── BoundaryPanel.tsx # 경계조건 설정
│   │   ├── SolverPanel.tsx   # 솔버 파라미터
│   │   ├── MaterialPanel.tsx # 재질 속성
│   │   └── ResultsPanel.tsx  # 결과 분석
│   ├── store/
│   │   └── simulationStore.ts # Zustand 상태 관리
│   └── ipc/
│       └── gfdClient.ts      # Rust 백엔드 통신
├── public/
│   └── index.html
└── tsconfig.json
```

### 1.5 Rust 백엔드 서버 (gfd-server)

```rust
// src/server.rs — JSON-RPC over stdin/stdout

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct RpcRequest {
    id: u64,
    method: String,
    params: serde_json::Value,
}

#[derive(Serialize)]
struct RpcResponse {
    id: u64,
    result: serde_json::Value,
}

// 지원 메서드:
// "mesh.generate" → 메시 생성, 메시 정보 반환
// "mesh.quality"  → 품질 메트릭 반환
// "mesh.display"  → 노드/셀 좌표 반환 (Three.js용)
// "solve.start"   → 솔버 시작 (비동기)
// "solve.status"  → 진행률/잔차 반환
// "solve.stop"    → 솔버 중단
// "field.get"     → 필드 데이터 반환 (pressure, velocity, temperature)
// "field.slice"   → 절단면 데이터 반환
// "export.vtk"    → VTK 파일 내보내기
// "config.validate" → 설정 유효성 검사
```

### 1.6 문제 발생 가능 부분 + 대응

| 위험 | 원인 | 대응 |
|------|------|------|
| **대용량 메시 전송 느림** | 100K+ 셀 좌표를 JSON으로 보내면 느림 | Binary ArrayBuffer로 전송 (MessagePack 또는 SharedArrayBuffer) |
| **3D 렌더링 성능** | Three.js에서 100만 셀 렌더링 불가 | LOD (Level of Detail) + 절단면만 렌더링 |
| **Electron 메모리** | Chromium + Node + Rust 동시 실행 | 메시 데이터는 Rust에만 보관, 렌더링 필요 부분만 전송 |
| **실시간 잔차 업데이트** | 매 반복마다 IPC 호출 오버헤드 | 10~50ms 간격 배치 업데이트 |
| **Cross-platform** | Windows/Linux/Mac 차이 | Rust 바이너리는 크로스컴파일, Electron은 네이티브 |
| **VTK 파싱** | Three.js가 VTK를 직접 못 읽음 | Rust에서 VTK → JSON/Binary 변환 후 전송 |

---

## 4. GPU 가속 (CUDA)

### 4.1 현재 상태

```
crates/gfd-gpu/
├── src/
│   ├── device.rs      # ✅ CUDA device 선택
│   ├── memory.rs      # ✅ GPU 메모리 관리
│   ├── solver/
│   │   ├── gpu_cg.rs  # ✅ GPU CG 솔버 (cudarc)
│   │   └── amgx.rs    # 🔴 AmgX 스텁
│   └── kernels/       # 🔴 커스텀 커널 스텁
│       ├── flux.rs
│       ├── gradient.rs
│       ├── correction.rs
│       └── reduction.rs
```

### 4.2 구현 계획

| 단계 | 작업 | 효과 |
|------|------|------|
| **1** | GPU SpMV를 SIMPLE 솔버에 실제 연결 | 선형 솔버 10~50x 가속 |
| **2** | cuSPARSE SpMV (CSR→GPU 업로드) | 최적화된 SpMV |
| **3** | GPU CG/BiCGSTAB 전체 경로 | CPU-GPU 전송 최소화 |
| **4** | GPU 커널: 면 플럭스 계산 | 어셈블리 가속 |
| **5** | GPU 커널: Green-Gauss 그래디언트 | 그래디언트 가속 |
| **6** | Multi-GPU 지원 (NCCL) | 대규모 문제 |

### 4.3 기술 스택

```
Rust → cudarc (CUDA Rust 바인딩)
     → cuSPARSE (SpMV)
     → cuBLAS (벡터 연산)
     → 커스텀 PTX 커널 (면 플럭스, 그래디언트)
```

### 4.4 CPU → GPU 전환 전략

```rust
// 기존 CPU 경로
let stats = cg_solver.solve(&matrix, &rhs, &mut solution)?;

// GPU 경로 (feature flag)
#[cfg(feature = "gpu")]
let stats = {
    let gpu_mat = GpuSparseMatrix::from_cpu(&matrix)?;
    let gpu_rhs = GpuVector::from_cpu(&rhs)?;
    let mut gpu_sol = GpuVector::from_cpu(&solution)?;
    let mut gpu_cg = GpuCG::new(tol, max_iter);
    let stats = gpu_cg.solve(&gpu_mat, &gpu_rhs, &mut gpu_sol)?;
    gpu_sol.to_cpu(&mut solution)?;
    stats
};
```

### 4.5 문제 발생 가능 부분

| 위험 | 원인 | 대응 |
|------|------|------|
| **CUDA 미설치 환경** | 모든 사용자가 NVIDIA GPU 없음 | `--features gpu` 조건부 컴파일, CPU 폴백 |
| **GPU 메모리 부족** | 대용량 메시가 GPU VRAM 초과 | 메모리 사용량 사전 추정 + 자동 CPU 폴백 |
| **CPU-GPU 전송 병목** | 매 반복 데이터 업로드/다운로드 | 전체 솔브를 GPU에서 수행, 최종 결과만 다운로드 |
| **cudarc 버전 충돌** | CUDA 12 vs 11 호환성 | cudarc는 런타임 CUDA 로딩, 빌드 시 불필요 |
| **PTX 커널 디버깅** | GPU 커널 에러 추적 어려움 | CPU 폴백으로 동일 로직 검증 후 GPU 이식 |

---

## 5. MPI 병렬 컴퓨팅

### 5.1 구현 계획

| 단계 | 작업 | 설명 |
|------|------|------|
| **1** | 메시 파티셔닝 | 그래프 기반 분할 (Metis/Scotch 스타일) |
| **2** | 헤일로 셀 교환 | 인접 프로세스와 경계 데이터 교환 |
| **3** | 분산 선형 솔버 | 분산 SpMV + 글로벌 내적 (AllReduce) |
| **4** | 분산 SIMPLE 루프 | 각 프로세스가 로컬 부분 풀이 |
| **5** | I/O 병렬화 | 각 프로세스가 자기 파티션 VTK 출력 |

### 5.2 아키텍처

```
mpirun -np 4 gfd solve --config pipe.json --partitions 4

Process 0          Process 1          Process 2          Process 3
┌──────────┐      ┌──────────┐      ┌──────────┐      ┌──────────┐
│ Partition │←────→│ Partition │←────→│ Partition │←────→│ Partition │
│    0      │ halo │    1      │ halo │    2      │ halo │    3      │
│ (n/4 셀) │      │ (n/4 셀) │      │ (n/4 셀) │      │ (n/4 셀) │
└──────────┘      └──────────┘      └──────────┘      └──────────┘
     ↓                  ↓                  ↓                  ↓
   local              local              local              local
   solve              solve              solve              solve
     ↓                  ↓                  ↓                  ↓
  AllReduce(residual) ────────────────────────────────────────→
```

### 5.3 기술 스택

```
Rust → rsmpi (MPI Rust 바인딩)
     → 또는 자체 TCP 소켓 통신 (MPI 없이도 동작)
```

### 5.4 메시 파티셔닝 알고리즘

```rust
// crates/gfd-parallel/src/partitioning.rs

/// 그래프 기반 메시 파티셔닝 (Recursive Bisection)
pub fn partition_mesh(
    mesh: &UnstructuredMesh,
    num_partitions: usize,
) -> Vec<Vec<usize>> {
    // 1. 셀 연결 그래프 구축 (인접 셀 = 면 공유)
    // 2. Recursive Coordinate Bisection:
    //    - x좌표 중앙값으로 2분할
    //    - 재귀적으로 num_partitions가 될 때까지
    // 3. 각 파티션에 헤일로 셀 추가 (1레이어)
}
```

### 5.5 문제 발생 가능 부분

| 위험 | 원인 | 대응 |
|------|------|------|
| **MPI 설치 복잡** | Windows에서 MS-MPI, Linux에서 OpenMPI 필요 | TCP 소켓 폴백 (MPI 없이도 동작) |
| **로드 밸런싱** | 파티션 간 셀 수 불균형 | 가중치 기반 파티셔닝 (계산 비용 추정) |
| **헤일로 동기화 지연** | 네트워크 지연으로 성능 저하 | 비동기 통신 + 계산-통신 오버랩 |
| **파티션 경계 정확도** | 헤일로 셀의 그래디언트 불연속 | 2중 헤일로 또는 보정 스텐실 |
| **디버깅 어려움** | 4+ 프로세스 동시 디버깅 | 단일 프로세스 모드 지원, 재현 가능 시드 |
| **rsmpi 빌드** | C MPI 라이브러리 링킹 필요 | feature flag + 없으면 단일 프로세스 폴백 |

---

## 6. 구현 순서 및 일정 추정

### Phase A: GPU 가속 (4번)
```
A1. GPU SpMV 실제 연결 (gfd-gpu/solver/gpu_cg.rs 수정)
A2. SIMPLE 솔버에 GPU 경로 활성화
A3. GPU 벤치마크 (CPU vs GPU 비교)
A4. 커스텀 PTX 커널 (면 플럭스)
```

### Phase B: MPI 병렬 (5번)
```
B1. 메시 파티셔닝 (Recursive Bisection)
B2. 헤일로 셀 교환 (TCP 소켓)
B3. 분산 선형 솔버 (분산 CG/BiCGSTAB)
B4. 분산 SIMPLE 루프
B5. 병렬 I/O
```

### Phase C: GUI (1번)
```
C1. Electron 프로젝트 생성 + Rust IPC 연결
C2. Outline Tree + Properties Panel
C3. Three.js 3D Viewport (메시 렌더링)
C4. 컬러맵/벡터/유선 시각화
C5. 잔차 모니터 + 콘솔
C6. 메시 생성 GUI
C7. 솔버 제어 (시작/중지/모니터)
```

---

## 7. 종속성 정리

### GUI (Electron/React)
```json
{
  "dependencies": {
    "electron": "^30.0.0",
    "react": "^18.2.0",
    "react-dom": "^18.2.0",
    "three": "^0.170.0",
    "@react-three/fiber": "^8.0.0",
    "@react-three/drei": "^9.0.0",
    "zustand": "^4.0.0",
    "recharts": "^2.0.0",
    "@monaco-editor/react": "^4.0.0",
    "antd": "^5.0.0"
  }
}
```

### GPU (Rust)
```toml
[dependencies]
cudarc = { version = "0.12", optional = true }

[features]
gpu = ["cudarc"]
```

### MPI (Rust)
```toml
[dependencies]
# TCP 소켓 기반 자체 구현 (외부 의존 없음)
# 또는 rsmpi = { version = "0.7", optional = true }

[features]
mpi = []  # 자체 TCP 구현
mpi-native = ["rsmpi"]  # 시스템 MPI 사용
```
