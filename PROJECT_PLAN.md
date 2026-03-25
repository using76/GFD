# GFD (General Fluid Dynamics) - 통합 멀티피직스 솔버 개발 계획서

> **프로젝트명:** GFD_DEV
> **목표:** 유체역학(CFD), 열전달, 고체 변형 해석을 통합하는 단일 Rust 기반 솔버
> **최종 산출물:** `gfd.exe` (Windows), `gfd` (Linux/macOS)
> **작성일:** 2026-03-24
> **입력 형식:** JSON
> **출력 형식:** OpenVDB
> **개발 언어:** Rust

---

## 목차

1. [프로젝트 개요](#1-프로젝트-개요)
2. [Phase 1: 오픈소스 솔버 수집 및 분류](#2-phase-1-오픈소스-솔버-수집-및-분류)
3. [Phase 2: 수학적 기반 정리](#3-phase-2-수학적-기반-정리)
4. [Phase 3: 소스코드-수학식 매핑](#4-phase-3-소스코드-수학식-매핑)
5. [Phase 4: Input/Output 및 저장 형식 정의](#5-phase-4-inputoutput-및-저장-형식-정의)
6. [Phase 5: Rust 단일 솔버 아키텍처 설계](#6-phase-5-rust-단일-솔버-아키텍처-설계)
7. [Phase 6: 핵심 모듈 구현](#7-phase-6-핵심-모듈-구현)
8. [Phase 7: 테스트 프레임워크](#8-phase-7-테스트-프레임워크)
9. [Phase 8: 빌드 및 배포](#9-phase-8-빌드-및-배포)
10. [일정 및 마일스톤](#10-일정-및-마일스톤)
11. [위험 요소 및 대응 방안](#11-위험-요소-및-대응-방안)
12. [**GFD 솔버 워크플로우 구조 (Fluent 기반)**](#12-gfd-솔버-워크플로우-구조-fluent-기반)
13. [**사용자 정의 수학식 편집 시스템 및 SDK 설계**](#13-사용자-정의-수학식-편집-시스템-및-sdk-설계)
14. [**GPU 가속 유동해석 통합 (NVIDIA CUDA)**](#14-gpu-가속-유동해석-통합-nvidia-cuda)

---

## 1. 프로젝트 개요

### 1.1 비전

현존하는 주요 오픈소스 CFD/FEA/열전달 솔버들의 핵심 알고리즘을 분석하고, 이를 **Rust 언어로 단일 통합 솔버**로 재작성한다. 기존 솔버들이 C/C++/Fortran으로 분산되어 있고, 각각 다른 입출력 형식을 사용하는 문제를 해결하여, **JSON 기반 통합 입력**과 **OpenVDB 기반 통합 출력**을 갖는 범용 솔버를 만든다.

### 1.2 핵심 해석 영역

| 영역 | 세부 분야 |
|------|----------|
| **유체역학 (CFD)** | 비압축성/압축성 유동, 난류 (RANS, LES, DNS), 다상유동, 반응유동, 자유표면 |
| **열전달** | 전도, 대류, 복사, 공액열전달 (CHT), 상변화 |
| **고체역학 (CSM)** | 선형/비선형 탄성, 소성, 크리프, 접촉, 동적 충격 |
| **멀티피직스** | FSI (유체-구조 연성), CHT (공액열전달), 전자기-열 연성 |

### 1.3 기술 스택

| 구성 요소 | 선택 기술 |
|-----------|----------|
| 핵심 언어 | **Rust** (안전성, 성능, 병렬성) |
| 입력 형식 | **JSON** (serde_json) |
| 출력 형식 | **OpenVDB** (vdb-rs) + **VTK** (vtkio) |
| 선형대수 | nalgebra, faer, PETSc FFI |
| CPU 병렬화 | rayon (스레드), MPI (분산) |
| **GPU 가속** | **NVIDIA CUDA** — cudarc (Rust 바인딩) |
| **GPU 선형 솔버** | **AmgX** (AMG+CG/BiCGSTAB), cuSOLVER, cuDSS |
| **GPU 행렬 연산** | **cuSPARSE** (SpMV), **cuBLAS** (벡터 연산) |
| **GPU 커스텀 커널** | Rust-CUDA / PTX 커널 (면 플럭스, 구배, 보정) |
| 메시 | 자체 구현 + Gmsh 연동 |
| 시각화 호환 | ParaView (VDB/VTK) |

---

## 2. Phase 1: 오픈소스 솔버 수집 및 분류

### 2.1 다운로드 대상 솔버 목록 (25개 + α)

> ⚠️ **제외 대상:** SPH 기반 솔버, 상용 솔버 (ANSYS Fluent/CFX, STAR-CCM+는 소스 비공개)
> ⚠️ **참고:** Fluent, CFX, STAR-CCM+는 상용 소프트웨어로 소스코드 접근 불가. 대신 해당 솔버들의 핵심 알고리즘(k-ε, k-ω, SST 등)은 공개 논문과 오픈소스 구현체에서 확보.

---

#### A. 범용 CFD 솔버 (7개)

| # | 솔버명 | 언어 | 라이선스 | 저장소 | 핵심 기능 |
|---|--------|------|----------|--------|----------|
| 1 | **OpenFOAM** | C++ | GPL-3.0 | [OpenFOAM/OpenFOAM-dev](https://github.com/OpenFOAM/OpenFOAM-dev) | 비압축/압축, 다상, 난류, CHT, 연소 |
| 2 | **SU2** | C++ | LGPL-2.1 | [su2code/SU2](https://github.com/su2code/SU2) | 항공 CFD, 최적화, CHT, 압축성 |
| 3 | **Code_Saturne** | C/Fortran | GPL-2.0 | [code-saturne/code_saturne](https://github.com/code-saturne/code_saturne) | 비압축/팽창, 난류, 연소, 복사, MHD |
| 4 | **FDS** | Fortran | Public Domain | [firemodels/fds](https://github.com/firemodels/fds) | 화재 LES, 연기/열 수송, 복사 |
| 5 | **CFL3D** | Fortran | Apache-2.0 | [nasa/CFL3D](https://github.com/nasa/CFL3D) | 구조격자 RANS, 항공 CFD |
| 6 | **Lethe** | C++ | LGPL-2.1 | [lethe-cfd/lethe](https://github.com/lethe-cfd/lethe) | 고차 비압축 N-S, 입자유동 |
| 7 | **NextFOAM** | C++ | GPL-3.0 | [nextfoam/nextfoam-cfd](https://github.com/nextfoam/nextfoam-cfd) | OpenFOAM 기반 확장 솔버 |

#### B. 고차/스펙트럴 CFD 솔버 (5개)

| # | 솔버명 | 언어 | 라이선스 | 저장소 | 핵심 기능 |
|---|--------|------|----------|--------|----------|
| 8 | **Nek5000** | Fortran | BSD-3 | [Nek5000/Nek5000](https://github.com/Nek5000/Nek5000) | 스펙트럴 요소법, DNS/LES |
| 9 | **Nektar++** | C++ | MIT | [nektar/nektar](https://gitlab.nektar.info/nektar/nektar) | 스펙트럴/hp 요소, 비압축/압축 |
| 10 | **Xcompact3d (x3d2)** | Fortran | BSD-3 | [xcompact3d/x3d2](https://github.com/xcompact3d/x3d2) | 고차 컴팩트 유한차분, GPU |
| 11 | **UCNS3D** | Fortran | GPL-3.0 | [ucns3d-team/UCNS3D](https://github.com/ucns3d-team/UCNS3D) | 비정렬 고차 압축성 유동 |
| 12 | **PyFR** | Python/C | BSD-3 | [PyFR/PyFR](https://github.com/PyFR/PyFR) | Flux Reconstruction, GPU 가속 |

#### C. 격자 볼츠만 (LBM) 솔버 (2개)

| # | 솔버명 | 언어 | 라이선스 | 저장소 | 핵심 기능 |
|---|--------|------|----------|--------|----------|
| 13 | **Palabos** | C++ | AGPL-3.0 | [FlowKit/palabos](https://gitlab.com/unibs/palabos) | 격자 볼츠만, 다상, 열전달 |
| 14 | **OpenLB** | C++ | GPL-2.0 | [OpenLB](https://www.openlb.net/) | 격자 볼츠만, 병렬, 다상 |

#### D. 다상/압축성 특화 (2개)

| # | 솔버명 | 언어 | 라이선스 | 저장소 | 핵심 기능 |
|---|--------|------|----------|--------|----------|
| 15 | **MFC** | Fortran/C | MIT | [MFlowCode/MFC](https://github.com/MFlowCode/MFC) | 압축성 다상, 엑사스케일, GPU |
| 16 | **Basilisk** | C | GPL-2.0 | [basilisk.fr](http://basilisk.fr/) | 적응격자, 자유표면, VOF |

#### E. 구조/고체역학 솔버 (3개)

| # | 솔버명 | 언어 | 라이선스 | 저장소 | 핵심 기능 |
|---|--------|------|----------|--------|----------|
| 17 | **CalculiX** | C/Fortran | GPL-2.0 | [Dhondtguido/CalculiX](https://github.com/Dhondtguido/CalculiX) | 선형/비선형 FEA, 열, 동적 |
| 18 | **OpenRadioss** | C/Fortran | AGPL-3.0 | [OpenRadioss/OpenRadioss](https://github.com/OpenRadioss/OpenRadioss) | 충격, 충돌, 동적 비선형 |
| 19 | **FreeFEM** | C++ | LGPL-3.0 | [FreeFem/FreeFem-sources](https://github.com/FreeFem/FreeFem-sources) | 범용 PDE, 유체-구조 |

#### F. 유한요소 라이브러리/프레임워크 (4개)

| # | 솔버명 | 언어 | 라이선스 | 저장소 | 핵심 기능 |
|---|--------|------|----------|--------|----------|
| 20 | **deal.II** | C++ | LGPL-2.1 | [dealii/dealii](https://github.com/dealii/dealii) | 적응 유한요소, hp-FEM |
| 21 | **MFEM** | C++ | BSD-3 | [mfem/mfem](https://github.com/mfem/mfem) | 고차 FEM, GPU, AMR |
| 22 | **FEniCSx** | C++/Python | LGPL-3.0 | [FEniCS/dolfinx](https://github.com/FEniCS/dolfinx) | 자동 PDE 이산화, 병렬 |
| 23 | **NGSolve** | C++ | LGPL-2.1 | [NGSolve/ngsolve](https://github.com/NGSolve/ngsolve) | 고성능 멀티피직스 FEM |

#### G. 멀티피직스 프레임워크 (4개)

| # | 솔버명 | 언어 | 라이선스 | 저장소 | 핵심 기능 |
|---|--------|------|----------|--------|----------|
| 24 | **MOOSE** | C++ | LGPL-2.1 | [idaholab/moose](https://github.com/idaholab/moose) | 범용 멀티피직스 FEM 프레임워크 |
| 25 | **Elmer FEM** | Fortran/C | GPL-2.0 | [ElmerCSC/elmerfem](https://github.com/ElmerCSC/elmerfem) | 유체, 구조, 전자기, 열, 음향 |
| 26 | **Kratos Multiphysics** | C++/Python | BSD-3 | [KratosMultiphysics/Kratos](https://github.com/KratosMultiphysics/Kratos) | 모듈식 멀티피직스 |
| 27 | **preCICE** | C++ | LGPL-3.0 | [precice/precice](https://github.com/precice/precice) | 멀티피직스 커플링 라이브러리 |

### 2.2 다운로드 및 정리 절차

```
GFD_DEV/
├── references/
│   ├── solvers/
│   │   ├── 01_openfoam/          # git clone
│   │   ├── 02_su2/               # git clone
│   │   ├── 03_code_saturne/      # git clone
│   │   ├── 04_fds/               # git clone
│   │   ├── 05_cfl3d/             # git clone
│   │   ├── 06_lethe/             # git clone
│   │   ├── 07_nextfoam/          # git clone
│   │   ├── 08_nek5000/           # git clone
│   │   ├── 09_nektar/            # git clone
│   │   ├── 10_xcompact3d/        # git clone
│   │   ├── 11_ucns3d/            # git clone
│   │   ├── 12_pyfr/              # git clone
│   │   ├── 13_palabos/           # git clone
│   │   ├── 14_openlb/            # download
│   │   ├── 15_mfc/               # git clone
│   │   ├── 16_basilisk/          # download
│   │   ├── 17_calculix/          # git clone
│   │   ├── 18_openradioss/       # git clone
│   │   ├── 19_freefem/           # git clone
│   │   ├── 20_dealii/            # git clone
│   │   ├── 21_mfem/              # git clone
│   │   ├── 22_fenicsx/           # git clone
│   │   ├── 23_ngsolve/           # git clone
│   │   ├── 24_moose/             # git clone
│   │   ├── 25_elmer/             # git clone
│   │   ├── 26_kratos/            # git clone
│   │   └── 27_precice/           # git clone
│   └── papers/                   # 핵심 논문 PDF
├── analysis/                     # 솔버 분석 문서
│   ├── solver_comparison.md
│   ├── algorithm_extraction.md
│   └── equation_catalog.md
```

### 2.3 각 솔버 분석 체크리스트

각 솔버에 대해 다음 항목을 분석/문서화:

- [ ] 지배 방정식 (Governing Equations)
- [ ] 이산화 방법 (FVM, FEM, FDM, LBM, Spectral)
- [ ] 난류 모델 (k-ε, k-ω, SST, SA, LES, DNS)
- [ ] 시간 적분 방법 (Explicit, Implicit, Dual time)
- [ ] 선형 솔버 (Direct, Iterative, Multigrid)
- [ ] 메시 타입 (정렬/비정렬/적응)
- [ ] 경계 조건 처리 방법
- [ ] 병렬화 전략 (OpenMP, MPI, GPU)
- [ ] 입출력 형식
- [ ] 핵심 소스 파일 위치

---

## 3. Phase 2: 수학적 기반 정리

### 3.1 지배 방정식 체계

#### A. 유체역학 (Navier-Stokes)

**연속 방정식 (질량 보존):**
```
∂ρ/∂t + ∇·(ρu) = 0
```

**운동량 방정식:**
```
∂(ρu)/∂t + ∇·(ρu⊗u) = -∇p + ∇·τ + ρg + F
```
여기서 τ = μ(∇u + (∇u)ᵀ) - (2/3)μ(∇·u)I

**에너지 방정식:**
```
∂(ρE)/∂t + ∇·((ρE + p)u) = ∇·(k∇T) + ∇·(τ·u) + Q
```

#### B. 난류 모델

**k-ε 모델:**
```
∂(ρk)/∂t + ∇·(ρuk) = ∇·((μ + μₜ/σₖ)∇k) + Pₖ - ρε
∂(ρε)/∂t + ∇·(ρuε) = ∇·((μ + μₜ/σε)∇ε) + C₁ε(ε/k)Pₖ - C₂ερ(ε²/k)
μₜ = ρCμk²/ε
```

**k-ω SST 모델:**
```
∂(ρk)/∂t + ∇·(ρuk) = ∇·((μ + σₖμₜ)∇k) + P̃ₖ - β*ρkω
∂(ρω)/∂t + ∇·(ρuω) = ∇·((μ + σωμₜ)∇ω) + αS² - βρω² + 2(1-F₁)ρσω₂(1/ω)(∇k·∇ω)
```

**Spalart-Allmaras 모델:**
```
∂ν̃/∂t + u·∇ν̃ = Cᵦ₁S̃ν̃ + (1/σ)[∇·((ν + ν̃)∇ν̃) + Cᵦ₂(∇ν̃)²] - Cw₁fw(ν̃/d)²
```

**LES (Large Eddy Simulation):**
```
Smagorinsky: νₛ = (CₛΔ)²|S̄|
Dynamic Smagorinsky: Cₛ 동적 계산 (Germano identity)
WALE: νₛ = (CwΔ)² (SᵈᵢⱼSᵈᵢⱼ)^(3/2) / ((S̄ᵢⱼS̄ᵢⱼ)^(5/2) + (SᵈᵢⱼSᵈᵢⱼ)^(5/4))
```

#### C. 열전달

**전도 (Fourier's Law):**
```
ρcₚ ∂T/∂t = ∇·(k∇T) + Q
```

**대류-전도 결합:**
```
ρcₚ(∂T/∂t + u·∇T) = ∇·(k∇T) + Φ + Q
```

**복사 (P-1 근사, DO 방법):**
```
P-1: ∇·(Γ∇G) - aG + 4aσT⁴ = 0, Γ = 1/(3a + 3σₛ - Cσₛ)
DO:  (s·∇)I(r,s) = -(a + σₛ)I + aσT⁴/π + (σₛ/4π)∫₄π I(r,s')Φ(s·s')dΩ'
```

**공액열전달 (CHT) 인터페이스:**
```
T_fluid = T_solid  (온도 연속)
k_fluid(∂T/∂n)_fluid = k_solid(∂T/∂n)_solid  (열유속 연속)
```

#### D. 고체역학

**선형 탄성:**
```
∇·σ + ρb = ρü
σ = C:ε
ε = (1/2)(∇u + (∇u)ᵀ)
```

**비선형 (대변형, von Mises 소성):**
```
∇·P + ρ₀b = ρ₀ü  (라그랑주 기술)
F = I + ∇u  (변형 구배)
J = det(F)
σᵥₘ = √(3/2 s:s) ≤ σᵧ  (항복 조건)
```

**크리프 (Norton 법칙):**
```
ε̇ₖᵣ = A·σⁿ·exp(-Q/RT)
```

#### E. 멀티피직스 커플링

**FSI (유체-구조 연성):**
```
유체 → 구조: 힘(압력, 전단) 전달
구조 → 유체: 변위(격자 이동) 전달
ALE 방법: ∂(ρu)/∂t|_χ + ∇·(ρ(u-w)⊗u) = ...  (w: 격자속도)
```

### 3.2 이산화 방법 카탈로그

| 방법 | 약어 | 적용 솔버 | 주요 용도 |
|------|------|----------|----------|
| 유한체적법 | FVM | OpenFOAM, SU2, Code_Saturne, FDS | 보존형 CFD |
| 유한요소법 | FEM | deal.II, MFEM, FEniCSx, CalculiX, Elmer | 구조, 열전달 |
| 유한차분법 | FDM | CFL3D, Xcompact3d | 구조격자 CFD |
| 스펙트럴요소법 | SEM | Nek5000, Nektar++ | 고정밀 유동 |
| 격자볼츠만법 | LBM | Palabos, OpenLB | 복잡 유동 |
| 불연속갈레르킨 | DG | UCNS3D, PyFR | 고차 압축성 |

### 3.3 시간 적분법

| 방법 | 유형 | 특징 |
|------|------|------|
| Forward Euler | 명시적 | 1차, 조건부 안정 |
| RK4 | 명시적 | 4차, 조건부 안정 |
| Backward Euler | 음해적 | 1차, 무조건 안정 |
| Crank-Nicolson | 음해적 | 2차, 무조건 안정 |
| BDF2 | 음해적 | 2차, A-안정 |
| PISO/SIMPLE/SIMPLEC | 압력-속도 커플링 | 비압축성 전용 |
| Dual Time Stepping | 음해적 | 비정상 문제 |
| Newmark-β | 음해적 | 구조 동역학 |
| Generalized-α | 음해적 | 구조 동역학, 수치 감쇠 제어 |

### 3.4 선형 솔버

| 솔버 | 유형 | 용도 |
|------|------|------|
| CG (Conjugate Gradient) | 반복법 | 대칭 양정치 |
| GMRES | 반복법 | 비대칭 범용 |
| BiCGSTAB | 반복법 | 비대칭, 안정적 |
| AMG (Algebraic Multigrid) | 멀티그리드 | 전처리/독립 솔버 |
| LU (SuperLU, MUMPS) | 직접법 | 소규모 정밀 |
| ILU | 전처리기 | 반복법 가속 |

---

## 4. Phase 3: 소스코드-수학식 매핑

### 4.1 솔버별 핵심 알고리즘 추출 매트릭스

| 수학식/알고리즘 | OpenFOAM | SU2 | Code_Saturne | CalculiX | Elmer | MOOSE |
|----------------|----------|-----|-------------|----------|-------|-------|
| 비압축 N-S (SIMPLE) | `simpleFoam/` | `SU2_CFD/src/` | `src/alge/` | - | `FlowSolve/` | `modules/navier_stokes/` |
| 압축성 N-S | `rhoCentralFoam/` | `SU2_CFD/src/solver/` | `src/cfbl/` | - | - | - |
| k-ε 난류 | `turbulenceModels/RAS/kEpsilon/` | `turbulence/` | `src/turb/` | - | `KESolver/` | - |
| k-ω SST | `turbulenceModels/RAS/kOmegaSST/` | `turbulence/` | `src/turb/` | - | `KWSolver/` | - |
| LES Smagorinsky | `turbulenceModels/LES/` | - | `src/turb/` | - | - | - |
| 열전도 | `laplacianFoam/` | `heat/` | `src/base/` | `ccx_2.20/` | `HeatSolve/` | `modules/heat_transfer/` |
| 공액열전달 | `chtMultiRegionFoam/` | `SU2_CFD/src/solver/CHeat*` | - | `ccx_2.20/` | 모듈결합 | `modules/heat_transfer/` |
| 복사 | `radiation/` | - | `src/rayt/` | `ccx_2.20/` | - | - |
| 선형 탄성 | `solidDisplacementFoam/` | `elasticity/` | - | `ccx_2.20/` | `ElasticSolve/` | `modules/solid_mechanics/` |
| 비선형 구조 | - | - | - | `ccx_2.20/nonlinear/` | - | `modules/solid_mechanics/` |
| FSI | `solidFoam + pimpleFoam` | `SU2_CFD/src/solver/CFEM*` | - | via preCICE | via preCICE | `modules/fsi/` |
| VOF (다상) | `interFoam/` | - | `src/vof/` | - | `FreeSurface/` | - |
| LBM | - | - | - | - | - | - |

### 4.2 분석 작업 절차

각 솔버에 대해:

1. **입구 파일 분석** → 어떤 물리량을 입력으로 받는지
2. **메인 루프 추적** → 시간 진행 / 반복 수렴 구조
3. **이산화 코드 추출** → 공간 이산화 구현부
4. **선형시스템 조립** → 행렬 조립 방법
5. **솔버 호출** → 어떤 선형 솔버를 사용하는지
6. **후처리/출력** → 결과 데이터 구조

---

## 5. Phase 4: Input/Output 및 저장 형식 정의

### 5.1 입력 형식 (JSON)

#### 메인 구성 파일: `simulation.json`

```json
{
  "simulation": {
    "name": "cavity_flow_2d",
    "type": "steady",
    "physics": ["fluid", "heat"],
    "dimensions": 3,
    "time": {
      "start": 0.0,
      "end": 10.0,
      "dt": 0.001,
      "max_iterations": 1000,
      "convergence": 1e-6
    }
  },
  "mesh": {
    "type": "file",
    "format": "gmsh",
    "path": "mesh/cavity.msh",
    "refinement": {
      "adaptive": true,
      "max_level": 3,
      "criterion": "gradient",
      "field": "velocity"
    }
  },
  "fluid": {
    "model": "incompressible_navier_stokes",
    "solver": "SIMPLE",
    "properties": {
      "density": 1.225,
      "viscosity": 1.789e-5
    },
    "turbulence": {
      "model": "k_omega_sst",
      "wall_treatment": "automatic",
      "parameters": {
        "intensity": 0.05,
        "viscosity_ratio": 10
      }
    },
    "schemes": {
      "convection": "second_order_upwind",
      "diffusion": "central",
      "time": "bdf2"
    }
  },
  "heat": {
    "model": "convection_diffusion",
    "properties": {
      "conductivity": 0.0242,
      "specific_heat": 1006.43
    },
    "radiation": {
      "model": "p1",
      "absorption_coefficient": 0.5,
      "scattering_coefficient": 0.0
    }
  },
  "solid": {
    "model": "linear_elastic",
    "properties": {
      "youngs_modulus": 210e9,
      "poisson_ratio": 0.3,
      "density": 7850,
      "thermal_expansion": 12e-6
    },
    "nonlinear": {
      "plasticity": "von_mises",
      "yield_stress": 250e6,
      "hardening": "isotropic"
    }
  },
  "coupling": {
    "type": "fsi",
    "method": "partitioned",
    "relaxation": 0.7,
    "max_coupling_iterations": 50,
    "convergence": 1e-5
  },
  "boundary_conditions": [
    {
      "name": "inlet",
      "patch": "inlet",
      "type": {
        "velocity": { "type": "fixed", "value": [1.0, 0.0, 0.0] },
        "pressure": { "type": "zero_gradient" },
        "temperature": { "type": "fixed", "value": 300.0 },
        "k": { "type": "fixed", "value": 0.375 },
        "omega": { "type": "fixed", "value": 100.0 }
      }
    },
    {
      "name": "outlet",
      "patch": "outlet",
      "type": {
        "velocity": { "type": "zero_gradient" },
        "pressure": { "type": "fixed", "value": 0.0 },
        "temperature": { "type": "zero_gradient" }
      }
    },
    {
      "name": "wall",
      "patch": "walls",
      "type": {
        "velocity": { "type": "no_slip" },
        "temperature": { "type": "fixed", "value": 350.0 },
        "displacement": { "type": "fixed", "value": [0, 0, 0] }
      }
    }
  ],
  "initial_conditions": {
    "velocity": [0.0, 0.0, 0.0],
    "pressure": 0.0,
    "temperature": 300.0,
    "k": 0.1,
    "omega": 10.0
  },
  "output": {
    "format": "openvdb",
    "path": "results/",
    "fields": ["velocity", "pressure", "temperature", "stress", "strain"],
    "interval": {
      "time_step": 100,
      "write_last": true
    },
    "probes": [
      { "name": "center", "location": [0.5, 0.5, 0.0], "fields": ["velocity", "pressure"] }
    ],
    "surfaces": [
      { "name": "wall_heat_flux", "patch": "walls", "fields": ["heat_flux", "y_plus"] }
    ]
  },
  "linear_solver": {
    "default": {
      "type": "gmres",
      "preconditioner": "amg",
      "tolerance": 1e-8,
      "max_iterations": 500
    },
    "pressure": {
      "type": "cg",
      "preconditioner": "amg",
      "tolerance": 1e-10,
      "max_iterations": 1000
    }
  },
  "parallel": {
    "method": "auto",
    "decomposition": "metis",
    "num_threads": 8
  }
}
```

### 5.2 출력 형식 (OpenVDB)

#### OpenVDB 저장 전략

```
results/
├── cavity_flow_2d_t0000.vdb     # 초기 조건
├── cavity_flow_2d_t0100.vdb     # t=100 스텝
├── cavity_flow_2d_t0200.vdb     # t=200 스텝
├── ...
├── cavity_flow_2d_final.vdb     # 최종 결과
├── probes/
│   └── center_history.json      # 프로브 시계열 (JSON)
├── surfaces/
│   └── wall_heat_flux.json      # 표면 데이터 (JSON)
└── residuals.json               # 수렴 이력
```

#### VDB 그리드 구조

```
각 .vdb 파일 내부:
├── velocity_x (FloatGrid)
├── velocity_y (FloatGrid)
├── velocity_z (FloatGrid)
├── pressure (FloatGrid)
├── temperature (FloatGrid)
├── k (FloatGrid)            # 난류 운동에너지
├── omega (FloatGrid)        # 비산일률
├── stress_xx (FloatGrid)    # 응력 텐서
├── stress_yy (FloatGrid)
├── stress_zz (FloatGrid)
├── stress_xy (FloatGrid)
├── stress_xz (FloatGrid)
├── stress_yz (FloatGrid)
├── displacement_x (FloatGrid)
├── displacement_y (FloatGrid)
├── displacement_z (FloatGrid)
└── metadata:
    ├── time_step (int)
    ├── physical_time (float)
    ├── residuals (dict)
    └── solver_info (dict)
```

#### Rust OpenVDB 라이브러리 선택

| 라이브러리 | 상태 | 기능 | 선택 |
|-----------|------|------|------|
| `vdb-rs` | 활발 개발 | 읽기 전용 (쓰기 계획중) | 감시 대상 |
| `openvdb-sys` | 불안정 | C++ FFI 바인딩 | 리스크 높음 |
| **자체 구현** | - | 완전 제어 | **1차 선택** |

> **결정:** VDB 파일 쓰기를 위한 순수 Rust 라이브러리를 자체 구현한다. `vdb-rs`의 읽기 구현을 참조하되, 쓰기 기능을 추가 개발한다. 대안으로 VTK 형식도 병렬 지원한다.

### 5.3 추가 입출력

| 용도 | 형식 | 라이브러리 |
|------|------|-----------|
| 메시 입력 | Gmsh (.msh), CGNS, STL | `gmsh` crate, 자체 파서 |
| 설정 입력 | JSON | `serde_json` |
| 시계열 출력 | JSON | `serde_json` |
| 체적 데이터 출력 | OpenVDB (.vdb) | 자체 Rust 구현 |
| 호환 출력 | VTK (.vtu/.vtm) | `vtkio` crate |
| 로그 | stdout + .log | `tracing` crate |
| 체크포인트 | 바이너리 | `bincode` / `rkyv` |

---

## 6. Phase 5: Rust 단일 솔버 아키텍처 설계

### 6.1 프로젝트 구조

```
gfd/
├── Cargo.toml                    # 워크스페이스 루트
├── Cargo.lock
├── crates/
│   ├── gfd-core/                 # 핵심 수학/자료구조
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── mesh/             # 메시 자료구조
│   │   │   │   ├── mod.rs
│   │   │   │   ├── structured.rs
│   │   │   │   ├── unstructured.rs
│   │   │   │   ├── cell.rs
│   │   │   │   ├── face.rs
│   │   │   │   ├── node.rs
│   │   │   │   └── partition.rs
│   │   │   ├── field/            # 물리장 (스칼라, 벡터, 텐서)
│   │   │   │   ├── mod.rs
│   │   │   │   ├── scalar.rs
│   │   │   │   ├── vector.rs
│   │   │   │   └── tensor.rs
│   │   │   ├── linalg/           # 선형대수
│   │   │   │   ├── mod.rs
│   │   │   │   ├── sparse.rs     # CSR/CSC 행렬
│   │   │   │   ├── solvers/
│   │   │   │   │   ├── cg.rs
│   │   │   │   │   ├── gmres.rs
│   │   │   │   │   ├── bicgstab.rs
│   │   │   │   │   └── amg.rs
│   │   │   │   └── preconditioners/
│   │   │   │       ├── ilu.rs
│   │   │   │       ├── jacobi.rs
│   │   │   │       └── amg.rs
│   │   │   ├── interpolation/    # 보간
│   │   │   ├── gradient/         # 구배 계산
│   │   │   └── numerics/         # 수치 스킴
│   │   │       ├── convection.rs # 대류 스킴 (upwind, central, TVD)
│   │   │       ├── diffusion.rs  # 확산 스킴
│   │   │       └── time.rs       # 시간 적분 (Euler, RK4, BDF)
│   │   └── Cargo.toml
│   │
│   ├── gfd-fluid/                # 유체역학 솔버
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── incompressible/   # 비압축성
│   │   │   │   ├── simple.rs     # SIMPLE 알고리즘
│   │   │   │   ├── piso.rs       # PISO 알고리즘
│   │   │   │   └── simplec.rs    # SIMPLEC
│   │   │   ├── compressible/     # 압축성
│   │   │   │   ├── roe.rs        # Roe 스킴
│   │   │   │   ├── hllc.rs       # HLLC 스킴
│   │   │   │   └── ausm.rs       # AUSM 스킴
│   │   │   ├── turbulence/       # 난류 모델
│   │   │   │   ├── mod.rs
│   │   │   │   ├── k_epsilon.rs
│   │   │   │   ├── k_omega.rs
│   │   │   │   ├── k_omega_sst.rs
│   │   │   │   ├── spalart_allmaras.rs
│   │   │   │   ├── les/
│   │   │   │   │   ├── smagorinsky.rs
│   │   │   │   │   ├── dynamic_smagorinsky.rs
│   │   │   │   │   └── wale.rs
│   │   │   │   └── wall_functions.rs
│   │   │   ├── multiphase/       # 다상 유동
│   │   │   │   ├── vof.rs        # Volume of Fluid
│   │   │   │   ├── level_set.rs
│   │   │   │   └── euler_euler.rs
│   │   │   └── combustion/       # 연소
│   │   │       ├── species.rs
│   │   │       └── reaction.rs
│   │   └── Cargo.toml
│   │
│   ├── gfd-thermal/              # 열전달 솔버
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── conduction.rs     # 전도
│   │   │   ├── convection.rs     # 대류열전달
│   │   │   ├── radiation/        # 복사
│   │   │   │   ├── p1.rs
│   │   │   │   ├── discrete_ordinates.rs
│   │   │   │   └── view_factor.rs
│   │   │   ├── conjugate.rs      # 공액열전달 (CHT)
│   │   │   └── phase_change.rs   # 상변화
│   │   └── Cargo.toml
│   │
│   ├── gfd-solid/                # 고체역학 솔버
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── elastic.rs        # 선형 탄성
│   │   │   ├── hyperelastic.rs   # 초탄성
│   │   │   ├── plasticity/       # 소성
│   │   │   │   ├── von_mises.rs
│   │   │   │   ├── tresca.rs
│   │   │   │   └── drucker_prager.rs
│   │   │   ├── dynamics.rs       # 동역학 (Newmark, HHT)
│   │   │   ├── contact.rs        # 접촉
│   │   │   ├── creep.rs          # 크리프
│   │   │   └── thermal_stress.rs # 열응력
│   │   └── Cargo.toml
│   │
│   ├── gfd-coupling/             # 멀티피직스 커플링
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── fsi.rs            # 유체-구조 연성
│   │   │   ├── cht.rs            # 공액열전달
│   │   │   ├── thermo_mechanical.rs  # 열-기계 연성
│   │   │   ├── interface.rs      # 인터페이스 매핑
│   │   │   └── relaxation.rs     # Aitken, 고정점 반복
│   │   └── Cargo.toml
│   │
│   ├── gfd-io/                   # 입출력
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── json_input.rs     # JSON 입력 파서
│   │   │   ├── mesh_reader/      # 메시 리더
│   │   │   │   ├── gmsh.rs
│   │   │   │   ├── cgns.rs
│   │   │   │   └── stl.rs
│   │   │   ├── vdb_writer.rs     # OpenVDB 출력
│   │   │   ├── vtk_writer.rs     # VTK 호환 출력
│   │   │   ├── checkpoint.rs     # 체크포인트
│   │   │   └── probes.rs         # 프로브 출력
│   │   └── Cargo.toml
│   │
│   ├── gfd-parallel/             # 병렬화
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── domain_decomp.rs  # 영역 분할
│   │   │   ├── mpi_comm.rs       # MPI 통신
│   │   │   ├── thread_pool.rs    # 스레드 풀 (rayon)
│   │   │   └── gpu/              # GPU 가속 (향후)
│   │   └── Cargo.toml
│   │
│   └── gfd-vdb/                  # 순수 Rust OpenVDB 구현
│       ├── src/
│       │   ├── lib.rs
│       │   ├── tree.rs           # VDB 트리 구조
│       │   ├── grid.rs           # 그리드
│       │   ├── io.rs             # 파일 읽기/쓰기
│       │   └── codec.rs          # 압축 (blosc, zip)
│       └── Cargo.toml
│
├── src/
│   └── main.rs                   # CLI 진입점
├── tests/                        # 통합 테스트
├── benches/                      # 벤치마크
└── examples/                     # 예제 케이스
```

### 6.2 핵심 Trait 설계

```rust
/// 물리 솔버 공통 인터페이스
pub trait PhysicsSolver {
    type Config: DeserializeOwned;
    type State;
    type Residual;

    fn initialize(&mut self, config: &Self::Config, mesh: &Mesh) -> Result<Self::State>;
    fn assemble(&mut self, state: &Self::State) -> Result<LinearSystem>;
    fn solve_step(&mut self, state: &mut Self::State, dt: f64) -> Result<Self::Residual>;
    fn is_converged(&self, residual: &Self::Residual, tolerance: f64) -> bool;
    fn post_process(&self, state: &Self::State) -> Result<FieldSet>;
}

/// 시간 적분기
pub trait TimeIntegrator {
    fn advance(&mut self, solver: &mut dyn PhysicsSolver, dt: f64) -> Result<()>;
}

/// 선형 솔버
pub trait LinearSolverTrait {
    fn solve(&mut self, system: &LinearSystem, x: &mut Vector) -> Result<SolverStats>;
}

/// 멀티피직스 커플링
pub trait CouplingStrategy {
    fn couple(&mut self, solvers: &mut [Box<dyn PhysicsSolver>]) -> Result<()>;
    fn transfer_fields(&mut self, from: usize, to: usize) -> Result<()>;
}

/// 난류 모델
pub trait TurbulenceModel {
    fn compute_eddy_viscosity(&self, state: &FluidState) -> Result<ScalarField>;
    fn solve_transport(&mut self, state: &mut FluidState, dt: f64) -> Result<()>;
}

/// 경계 조건
pub trait BoundaryCondition {
    fn apply(&self, field: &mut Field, mesh: &Mesh, patch: &Patch) -> Result<()>;
}
```

### 6.3 메인 솔버 루프

```rust
fn main_solver_loop(config: &SimulationConfig) -> Result<()> {
    // 1. 메시 로드
    let mesh = load_mesh(&config.mesh)?;

    // 2. 물리 솔버 초기화
    let mut solvers = initialize_physics_solvers(config, &mesh)?;

    // 3. 초기 조건 적용
    apply_initial_conditions(&mut solvers, &config.initial_conditions)?;

    // 4. 시간 루프
    let mut time = config.simulation.time.start;
    let mut step = 0;

    while time < config.simulation.time.end {
        let dt = compute_timestep(config, &solvers)?;

        // 5. 멀티피직스 커플링 반복
        for coupling_iter in 0..config.coupling.max_iterations {
            // 각 물리 솔버 전진
            for solver in &mut solvers {
                solver.solve_step(dt)?;
            }

            // 커플링 잔차 확인
            if check_coupling_convergence(&solvers, config.coupling.convergence)? {
                break;
            }

            // 인터페이스 데이터 교환
            exchange_interface_data(&mut solvers)?;
        }

        // 6. 출력
        if should_write_output(step, config) {
            write_vdb_output(&solvers, step, time, config)?;
        }

        time += dt;
        step += 1;
    }

    Ok(())
}
```

### 6.4 의존성 (Cargo.toml 주요 crate)

```toml
[workspace.dependencies]
# 직렬화
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# 선형대수
nalgebra = "0.33"
nalgebra-sparse = "0.10"
faer = "0.20"              # 고성능 선형대수

# 병렬화
rayon = "1.10"
mpi = { version = "0.8", optional = true }

# I/O
vtkio = "0.7"              # VTK 출력
gmsh = "0.3"               # Gmsh 메시 읽기
bincode = "1"              # 체크포인트

# 유틸리티
tracing = "0.1"            # 로깅
tracing-subscriber = "0.3"
clap = { version = "4", features = ["derive"] }  # CLI
anyhow = "1"               # 에러 처리
thiserror = "2"            # 에러 타입

# 테스트
approx = "0.5"             # 부동소수점 비교
criterion = "0.5"          # 벤치마크
```

---

## 7. Phase 6: 핵심 모듈 구현

### 7.1 구현 순서 (의존성 기반)

```
Level 0: gfd-core (mesh, field, linalg, numerics)
   ↓
Level 1: gfd-io (JSON input, mesh reader)
   ↓
Level 2: gfd-fluid (incompressible first) + gfd-thermal (conduction first) + gfd-solid (linear elastic first)
   ↓
Level 3: gfd-coupling (CHT first, then FSI)
   ↓
Level 4: gfd-vdb (OpenVDB writer)
   ↓
Level 5: gfd (main executable, CLI)
   ↓
Level 6: 고급 기능 (LES, 다상, 비선형 구조, GPU)
```

### 7.2 구현 마일스톤

#### M1: 기반 (gfd-core + gfd-io)
- 비정렬 메시 자료구조 (노드, 셀, 면)
- 스칼라/벡터/텐서 필드
- CSR 희소 행렬
- CG, GMRES, BiCGSTAB 선형 솔버
- ILU, Jacobi 전처리기
- JSON 파서 (serde)
- Gmsh .msh 리더

#### M2: 기본 유체 (gfd-fluid 1차)
- SIMPLE/PISO 알고리즘 (비압축)
- 1차/2차 업윈드, 중심차분
- Euler/BDF2 시간 적분
- 기본 경계조건 (inlet, outlet, wall, symmetry)
- k-ε, k-ω SST 난류 모델
- 벽함수

#### M3: 기본 열전달 (gfd-thermal 1차)
- 열전도 솔버
- 대류-확산 솔버
- 온도 경계조건

#### M4: 기본 구조 (gfd-solid 1차)
- 선형 탄성 FEM
- Newmark-β 시간 적분
- 변위/힘 경계조건

#### M5: 출력 (gfd-vdb + gfd-io 확장)
- OpenVDB 파일 쓰기
- VTK 호환 출력
- 프로브/표면 데이터 출력

#### M6: 커플링 (gfd-coupling)
- CHT (공액열전달)
- FSI (유체-구조 연성, 단순 파티셔닝)
- 인터페이스 보간

#### M7: 고급 유체
- 압축성 (Roe, HLLC)
- LES (Smagorinsky, WALE)
- Spalart-Allmaras
- VOF 다상유동

#### M8: 고급 구조
- 비선형 (대변형, 소성)
- 접촉
- 크리프
- 열응력

#### M9: 고급 열전달
- 복사 (P-1, DO)
- 상변화
- 공액열전달 고급 (다영역)

#### M10: 성능 최적화
- AMG 멀티그리드
- rayon 병렬화 최적화
- MPI 분산 (선택)
- 메모리 최적화

---

## 8. Phase 7: 테스트 프레임워크

### 8.1 테스트 계층 구조

```
tests/
├── unit/                           # 단위 테스트
│   ├── core/
│   │   ├── test_mesh.rs
│   │   ├── test_sparse_matrix.rs
│   │   ├── test_linear_solvers.rs
│   │   └── test_gradient.rs
│   ├── fluid/
│   │   ├── test_simple.rs
│   │   ├── test_turbulence_models.rs
│   │   └── test_convection_schemes.rs
│   ├── thermal/
│   │   └── test_conduction.rs
│   └── solid/
│       └── test_linear_elastic.rs
│
├── verification/                   # 검증 (해석해 비교)
│   ├── fluid/
│   │   ├── couette_flow/           # Couette 유동 (해석해)
│   │   │   ├── simulation.json
│   │   │   ├── mesh/
│   │   │   └── expected/
│   │   ├── poiseuille_flow/        # Poiseuille 유동 (해석해)
│   │   ├── lid_driven_cavity/      # 뚜껑 구동 공동 (Ghia 벤치마크)
│   │   ├── backward_facing_step/   # 후면 계단 유동
│   │   ├── cylinder_flow_re100/    # 원주 주위 유동 Re=100
│   │   ├── taylor_green_vortex/    # Taylor-Green 와류 (DNS 검증)
│   │   └── turbulent_channel/      # 난류 채널 (DNS 데이터 비교)
│   ├── thermal/
│   │   ├── 1d_conduction/          # 1D 정상 전도 (해석해)
│   │   ├── fin_convection/         # 핀 대류냉각 (해석해)
│   │   ├── natural_convection/     # 자연대류 (De Vahl Davis)
│   │   └── conjugate_ht/           # 공액열전달 벤치마크
│   └── solid/
│       ├── cantilever_beam/        # 외팔보 (해석해)
│       ├── patch_test/             # 패치 테스트
│       ├── cook_membrane/          # Cook 멤브레인
│       └── hertz_contact/          # Hertz 접촉 (해석해)
│
├── validation/                     # 타당성 검증 (실험/타솔버 비교)
│   ├── ahmed_body/                 # Ahmed body (실험 비교)
│   ├── naca0012/                   # NACA 0012 에어포일
│   └── heated_pipe/                # 가열 파이프
│
├── regression/                     # 회귀 테스트
│   └── (이전 결과 대비 변경 감지)
│
├── integration/                    # 통합 테스트
│   ├── test_json_to_result.rs      # 전체 파이프라인
│   ├── test_cht_workflow.rs        # CHT 워크플로우
│   ├── test_fsi_workflow.rs        # FSI 워크플로우
│   └── test_checkpoint_restart.rs  # 체크포인트/재시작
│
└── benchmark/                      # 성능 벤치마크
    ├── scaling/                    # 스케일링 테스트
    ├── memory/                     # 메모리 사용량
    └── comparison/                 # 타 솔버 대비 성능
```

### 8.2 검증 기준

| 테스트 케이스 | 비교 대상 | 허용 오차 |
|--------------|----------|----------|
| Couette Flow | 해석해 | < 1e-6 (L2 norm) |
| Poiseuille Flow | 해석해 | < 1e-6 (L2 norm) |
| Lid-Driven Cavity | Ghia et al. (1982) | < 1% (속도 프로파일) |
| 1D Conduction | 해석해 | < 1e-8 |
| Cantilever Beam | 해석해 | < 0.1% (처짐) |
| k-ε 채널 유동 | DNS 데이터 | < 5% (벽 전단응력) |
| Natural Convection | De Vahl Davis | < 1% (Nusselt 수) |
| NACA 0012 | 실험 데이터 | < 5% (양력/항력 계수) |

### 8.3 테스트 실행 방법

```bash
# 전체 단위 테스트
cargo test

# 특정 모듈 테스트
cargo test -p gfd-fluid

# 검증 테스트 (시간 소요)
cargo test --release -- verification

# 벤치마크
cargo bench

# 회귀 테스트
cargo test -- regression
```

### 8.4 CI/CD 파이프라인

```yaml
# .github/workflows/ci.yml
stages:
  - lint:     cargo clippy + cargo fmt --check
  - unit:     cargo test (모든 단위 테스트)
  - verify:   검증 테스트 (해석해 비교, 릴리스 빌드)
  - bench:    성능 벤치마크 (이전 대비)
  - build:    cargo build --release (Windows/Linux/macOS)
  - package:  .exe / 바이너리 패키징
```

---

## 9. Phase 8: 빌드 및 배포

### 9.1 빌드 명령

```bash
# 개발 빌드
cargo build

# 릴리스 빌드 (최적화)
cargo build --release

# Windows .exe 생성
cargo build --release --target x86_64-pc-windows-msvc

# Linux 바이너리
cargo build --release --target x86_64-unknown-linux-gnu

# macOS 바이너리
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
```

### 9.2 실행 방법

```bash
# 기본 실행
gfd run simulation.json

# 병렬 실행
gfd run simulation.json --threads 16

# MPI 분산 실행
mpirun -np 4 gfd run simulation.json --mpi

# 메시만 검사
gfd check-mesh mesh/cavity.msh

# 입력 파일 검증
gfd validate simulation.json

# 결과 변환 (VDB → VTK)
gfd convert results/output.vdb --format vtk
```

### 9.3 배포 산출물

```
dist/
├── windows/
│   ├── gfd.exe
│   └── README.txt
├── linux/
│   ├── gfd
│   └── README.txt
└── macos/
    ├── gfd
    └── README.txt
```

---

## 10. 일정 및 마일스톤

### 전체 로드맵

```
Phase 1: 솔버 수집 및 분류 ──────────── [2주]
  ├─ 27개 솔버 다운로드
  ├─ 각 솔버 구조 분석
  └─ 핵심 알고리즘 목록화

Phase 2: 수학식 정리 ────────────────── [3주]
  ├─ 지배방정식 카탈로그
  ├─ 이산화 방법 정리
  └─ 솔버별 수학식 매핑

Phase 3: 소스코드-수학식 매핑 ────────── [4주]
  ├─ 각 솔버 핵심 코드 추적
  ├─ 알고리즘-코드 매핑표 작성
  └─ 최적 구현 방식 선정

Phase 4: I/O 정의 ───────────────────── [2주]
  ├─ JSON 스키마 확정
  ├─ OpenVDB 저장 전략 확정
  └─ 프로토타입 I/O 구현

Phase 5: 아키텍처 설계 ──────────────── [2주]
  ├─ Rust 모듈 구조 확정
  ├─ Trait/인터페이스 설계
  └─ 의존성 확정

Phase 6: 핵심 구현 ──────────────────── [수개월 단위]
  ├─ M1: gfd-core        [4주]
  ├─ M2: gfd-fluid 기본  [6주]
  ├─ M3: gfd-thermal 기본 [3주]
  ├─ M4: gfd-solid 기본   [4주]
  ├─ M5: gfd-vdb 출력     [3주]
  ├─ M6: 커플링           [4주]
  ├─ M7: 고급 유체        [6주]
  ├─ M8: 고급 구조        [4주]
  ├─ M9: 고급 열전달      [3주]
  └─ M10: 최적화          [4주]

Phase 7: 테스트 ─────────────────────── [Phase 6과 병행]
  ├─ 단위 테스트 (각 모듈과 함께)
  ├─ 검증 테스트 (해석해 비교)
  └─ 타당성 검증 (실험 비교)

Phase 8: 빌드/배포 ──────────────────── [1주]
  ├─ 크로스 컴파일 설정
  ├─ CI/CD 파이프라인
  └─ .exe 패키징
```

---

## 11. 위험 요소 및 대응 방안

| 위험 요소 | 심각도 | 대응 방안 |
|----------|--------|----------|
| OpenVDB Rust 라이브러리 미성숙 | 높음 | 순수 Rust VDB 구현 + VTK 폴백 |
| 선형 솔버 성능 부족 | 높음 | PETSc FFI 바인딩, faer 활용 |
| 상용 솔버 소스 접근 불가 | 중간 | 공개 논문 + 오픈소스 대안 활용 |
| 멀티피직스 커플링 안정성 | 높음 | preCICE 알고리즘 참조, 점진적 구현 |
| Fortran→Rust 변환 난이도 | 높음 | 알고리즘 수준 재구현 (1:1 변환 X) |
| 메시 호환성 | 중간 | Gmsh를 기본으로, 추후 확장 |
| 검증 데이터 확보 | 중간 | NASA TMR, 학술 벤치마크 데이터 활용 |
| 단일 개발자 범위 초과 | 높음 | 핵심 기능 우선, 모듈화로 점진 확장 |

---

## 12. GFD 솔버 워크플로우 구조 (Fluent 기반)

> ANSYS Fluent의 워크플로우 트리를 참조하여, GFD 솔버의 전체 사용자 인터페이스/JSON 구성 구조를 정의한다.
> 아래 트리의 각 항목은 JSON 입력 파일의 섹션 또는 CLI 서브커맨드에 1:1 대응된다.

### 12.1 전체 워크플로우 트리 개요

```
GFD Solver
├── 1. SETUP (물리 설정)
│   ├── 1.1 General
│   ├── 1.2 Models
│   ├── 1.3 Materials
│   ├── 1.4 Cell Zone Conditions
│   ├── 1.5 Boundary Conditions
│   ├── 1.6 Dynamic Mesh
│   ├── 1.7 Reference Values
│   └── 1.8 Named Expressions
│
├── 2. SOLVER (수치 해법 설정)
│   ├── 2.1 Solution Methods
│   ├── 2.2 Solution Controls
│   ├── 2.3 Monitors
│   ├── 2.4 Initialization
│   └── 2.5 Calculation Activities (Save/Export/Commands)
│
├── 3. RUN CALCULATION (계산 실행)
│   ├── 3.1 Run Settings
│   ├── 3.2 Schedule / Batch
│   └── 3.3 Check & Run
│
└── 4. RESULTS (결과 후처리)
    ├── 4.1 Graphics
    ├── 4.2 Plots
    ├── 4.3 Reports
    ├── 4.4 Animations
    └── 4.5 Data Export
```

---

### 12.2 SETUP (물리 설정) — 상세 구조

#### 1.1 General (일반 설정)

```
General
├── Solver Type
│   ├── Pressure-Based          # 비압축성/약압축성 유동 (기본값)
│   └── Density-Based           # 고속 압축성 유동 (초음속/극초음속)
│
├── Time
│   ├── Steady                  # 정상 상태
│   └── Transient               # 비정상 (시간 의존)
│       ├── Time Step Size      # Δt
│       ├── Max Iterations/Step # 타임스텝당 최대 반복
│       └── Time Stepping Method
│           ├── Fixed            # 고정 시간 간격
│           └── Adaptive         # CFL 기반 자동 조절
│
├── Space
│   ├── 2D                      # 2차원
│   │   ├── Planar              # 평면
│   │   └── Axisymmetric        # 축대칭
│   │       └── Swirl           # 선회 유동 포함
│   └── 3D                      # 3차원
│
├── Velocity Formulation
│   ├── Absolute                # 절대 속도
│   └── Relative                # 상대 속도 (회전 기준좌표)
│
├── Gravity
│   ├── Enabled (true/false)
│   ├── X (m/s²)                # 기본값: 0
│   ├── Y (m/s²)                # 기본값: -9.81
│   └── Z (m/s²)                # 기본값: 0
│
└── Operating Conditions
    ├── Operating Pressure (Pa)  # 기본값: 101325
    ├── Reference Pressure Location (x, y, z)
    ├── Gravity Reference (관련 유무)
    └── Variable-Density Parameters
        ├── Specified Operating Density (true/false)
        └── Operating Density (kg/m³)
```

**JSON 매핑:**
```json
{
  "setup": {
    "general": {
      "solver_type": "pressure_based",
      "time": "steady",
      "space": { "dimensions": 3 },
      "velocity_formulation": "absolute",
      "gravity": { "enabled": true, "vector": [0, -9.81, 0] },
      "operating_conditions": {
        "operating_pressure": 101325,
        "reference_pressure_location": [0, 0, 0]
      }
    }
  }
}
```

---

#### 1.2 Models (물리 모델)

```
Models
├── Multiphase
│   ├── Off (기본값)
│   ├── Volume of Fluid (VOF)
│   │   ├── Scheme: Explicit / Implicit
│   │   ├── Courant Number
│   │   ├── Interface Modeling
│   │   │   ├── Sharp
│   │   │   ├── Sharp/Dispersed
│   │   │   └── Dispersed
│   │   ├── Body Force Formulation
│   │   │   ├── Implicit Body Force
│   │   │   └── Gravity Direction
│   │   └── Phases (N개 정의)
│   │       ├── Phase Name
│   │       ├── Phase Material
│   │       └── Surface Tension (상간)
│   ├── Mixture
│   │   ├── Slip Velocity
│   │   ├── Implicit Body Force
│   │   └── Phases
│   ├── Eulerian
│   │   ├── Number of Phases
│   │   ├── Interfacial Area Concentration
│   │   ├── Drag Model (Schiller-Naumann, Morsi-Alexander, ...)
│   │   ├── Lift Model (Saffman-Mei, Tomiyama, ...)
│   │   ├── Turbulence Interaction
│   │   ├── Heat Transfer (Ranz-Marshall, ...)
│   │   └── Mass Transfer (Evaporation-Condensation, Cavitation)
│   └── Wet Steam
│
├── Energy
│   ├── Off / On
│   ├── Viscous Dissipation
│   ├── Pressure Work
│   ├── Kinetic Energy
│   └── Diffusion Energy Source
│
├── Viscous (난류 모델)
│   ├── Inviscid                    # 비점성
│   ├── Laminar                     # 층류
│   ├── Spalart-Allmaras (1-eq)     # SA 1방정식
│   │   ├── Strain/Vorticity-Based
│   │   └── Low-Re Damping
│   ├── k-epsilon (2-eq)            # k-ε 2방정식
│   │   ├── Standard
│   │   ├── RNG
│   │   └── Realizable
│   │   [공통 옵션]
│   │   ├── Near-Wall Treatment
│   │   │   ├── Standard Wall Functions
│   │   │   ├── Scalable Wall Functions
│   │   │   ├── Non-Equilibrium Wall Functions
│   │   │   └── Enhanced Wall Treatment (2-layer)
│   │   ├── Viscous Heating
│   │   └── Buoyancy Effects
│   ├── k-omega (2-eq)              # k-ω 2방정식
│   │   ├── Standard
│   │   ├── BSL (Baseline)
│   │   └── SST (Shear Stress Transport)
│   │   [공통 옵션]
│   │   ├── Low-Re Corrections
│   │   ├── Shear Flow Corrections
│   │   ├── Compressibility Effects
│   │   └── Production Limiter
│   ├── Transition
│   │   ├── Transition SST (4-eq)
│   │   └── Transition k-kl-omega (3-eq)
│   ├── Reynolds Stress Model (RSM)  # 레이놀즈 응력 모델
│   │   ├── Linear Pressure-Strain
│   │   ├── Quadratic Pressure-Strain
│   │   ├── Stress-Omega
│   │   └── Stress-BSL
│   │   [공통 옵션]
│   │   ├── Wall Reflection Effects
│   │   └── Near-Wall Treatment
│   ├── Scale-Adaptive Simulation (SAS)
│   ├── Detached Eddy Simulation (DES)
│   │   ├── DES-SA
│   │   ├── DES-Realizable k-ε
│   │   └── DES-SST k-ω
│   ├── Large Eddy Simulation (LES)
│   │   ├── Smagorinsky-Lilly
│   │   ├── Dynamic Smagorinsky
│   │   ├── WALE
│   │   ├── Dynamic Kinetic Energy (1-eq)
│   │   └── WMLES (Wall-Modeled LES)
│   │   [공통 옵션]
│   │   ├── Subgrid-Scale Model
│   │   └── Near-Wall Modeling
│   └── Direct Numerical Simulation (DNS)
│
├── Radiation (복사)
│   ├── Off (기본값)
│   ├── Discrete Ordinates (DO)
│   │   ├── Angular Discretization (Theta/Phi Divisions, Pixels)
│   │   ├── Non-Gray Model (Bands)
│   │   ├── Solar Load
│   │   │   ├── Sun Direction Vector
│   │   │   ├── Direct Solar Irradiation (W/m²)
│   │   │   ├── Diffuse Solar Irradiation (W/m²)
│   │   │   └── Solar Calculator (위도, 경도, 시간대, 날짜)
│   │   └── Iteration Parameters
│   │       ├── Flow Iterations per Radiation Iteration
│   │       └── Convergence Criterion
│   ├── P-1
│   │   ├── Absorption Coefficient
│   │   ├── Scattering Coefficient
│   │   └── Phase Function (Isotropic / Linear-Anisotropic)
│   ├── Rosseland
│   │   └── Optically Thick Media Only
│   ├── Surface-to-Surface (S2S)
│   │   ├── View Factor Calculation
│   │   ├── Clustering
│   │   └── Emissivity per Surface
│   └── Monte Carlo
│       ├── Number of Histories
│       └── Angular Resolution
│
├── Heat Exchanger
│   ├── Off (기본값)
│   ├── Dual Cell Heat Exchanger
│   │   ├── Hot Side Zone
│   │   ├── Cold Side Zone
│   │   ├── NTU Correlation
│   │   └── Heat Rejection
│   ├── Macro Heat Exchanger
│   │   ├── Core Geometry
│   │   ├── Auxiliary Fluid Properties
│   │   ├── NTU Parameters
│   │   └── Pressure Drop
│   └── Ungrouped
│
├── Species (화학종)
│   ├── Off (기본값)
│   ├── Species Transport
│   │   ├── Mixture Material
│   │   ├── Reactions
│   │   │   ├── Volumetric (체적 반응)
│   │   │   ├── Wall Surface (벽면 반응)
│   │   │   └── Particle Surface (입자 표면 반응)
│   │   ├── Reaction Mechanism
│   │   │   ├── Laminar Finite-Rate
│   │   │   ├── Eddy-Dissipation
│   │   │   ├── Eddy-Dissipation Concept (EDC)
│   │   │   └── Finite-Rate/Eddy-Dissipation
│   │   ├── Inlet Diffusion
│   │   ├── Diffusion Energy Source
│   │   └── Thermal Diffusion (Soret Effect)
│   ├── Non-Premixed Combustion (PDF)
│   │   ├── Fuel Stream Composition
│   │   ├── Oxidizer Stream Composition
│   │   ├── Flamelet Model
│   │   └── PDF Table
│   ├── Premixed Combustion
│   │   ├── Laminar Flame Speed
│   │   ├── Turbulent Flame Speed Model
│   │   └── Flame Surface Density
│   └── Partially Premixed
│
├── Discrete Phase (DPM)
│   ├── Off (기본값)
│   ├── Interaction with Continuous Phase
│   │   ├── Update Frequency
│   │   └── Max Number of Steps
│   ├── Tracking Parameters
│   │   ├── Step Length Factor
│   │   ├── Max Number of Steps
│   │   └── Accuracy Control Tolerance
│   ├── Physical Models
│   │   ├── Drag Law (Spherical, Non-Spherical, Stokes-Cunningham)
│   │   ├── Brownian Motion
│   │   ├── Saffman Lift Force
│   │   ├── Magnus Lift Force
│   │   ├── Virtual Mass Force
│   │   ├── Pressure Gradient Force
│   │   ├── Thermophoretic Force
│   │   ├── Erosion/Accretion
│   │   └── Breakup (TAB, Wave, KHRT)
│   ├── Injections
│   │   ├── Injection Type (Surface, Cone, Solid-Cone, Group, File)
│   │   ├── Particle Type (Inert, Droplet, Combusting, Multicomponent)
│   │   ├── Material
│   │   ├── Diameter Distribution (Uniform, Rosin-Rammler, ...)
│   │   ├── Velocity Magnitude / Components
│   │   ├── Temperature
│   │   ├── Flow Rate (kg/s)
│   │   └── Start/Stop Time (비정상)
│   └── Boundary Conditions (per wall/inlet/outlet)
│       ├── Escape
│       ├── Reflect (Normal/Tangent Coefficient)
│       ├── Trap
│       └── Wall-Jet / Wall-Film
│
├── Solidification & Melting
│   ├── Off (기본값)
│   ├── Solidus Temperature
│   ├── Liquidus Temperature
│   ├── Latent Heat (J/kg)
│   ├── Pure Solvent Melting Heat
│   ├── Mushy Zone Parameter (Amushy)
│   └── Pull Velocity (연속 주조)
│
├── Acoustics
│   ├── Off (기본값)
│   ├── Broadband Noise Sources
│   │   ├── Self-Noise
│   │   └── Turbulence-Interaction Noise
│   ├── Ffowcs-Williams & Hawkings (FW-H)
│   │   ├── Source Surfaces
│   │   ├── Receiver Locations (x, y, z)
│   │   ├── Export Frequency
│   │   └── Sound Speed / Reference Acoustic Pressure
│   └── Linearized Euler Equations (LEE)
│
├── Structure (내장 FSI)
│   ├── Off (기본값)
│   ├── Model
│   │   ├── Linear Elastic
│   │   └── Nonlinear (Hyperelastic, Plastic)
│   ├── Material Properties
│   │   ├── Young's Modulus
│   │   ├── Poisson Ratio
│   │   ├── Density
│   │   └── Thermal Expansion Coefficient
│   ├── Coupling Method
│   │   ├── One-Way (유체→구조)
│   │   └── Two-Way (양방향)
│   ├── Mesh Deformation
│   │   ├── Smoothing
│   │   └── Remeshing
│   └── Convergence (인터페이스 잔차)
│
├── Eulerian Wall Film
│   ├── Off (기본값)
│   ├── Film Material
│   ├── Film Thickness Limits
│   ├── Separation Criteria
│   │   ├── Foucart
│   │   └── O'Rourke
│   └── Coupling with DPM
│
└── Potential / Electrochemistry (확장)
    ├── Li-ion Battery Model
    │   ├── Electrochemistry Parameters
    │   ├── E-Chemistry NTGK / Newman
    │   └── Thermal Abuse
    └── Electrolysis / Fuel Cell
```

**JSON 매핑 예시 (Models):**
```json
{
  "setup": {
    "models": {
      "multiphase": { "type": "off" },
      "energy": { "enabled": true, "viscous_dissipation": false },
      "viscous": {
        "model": "k_omega_sst",
        "options": {
          "low_re_corrections": true,
          "production_limiter": true
        },
        "near_wall": "automatic"
      },
      "radiation": {
        "model": "discrete_ordinates",
        "angular_discretization": { "theta": 4, "phi": 4 },
        "solar_load": { "enabled": false }
      },
      "species": { "type": "off" },
      "discrete_phase": { "enabled": false },
      "solidification_melting": { "enabled": false },
      "acoustics": { "type": "off" },
      "structure": { "enabled": false }
    }
  }
}
```

---

#### 1.3 Materials (재료)

```
Materials
├── Fluid Materials
│   ├── 기본 제공: air, water-liquid, ...
│   └── 속성 정의 (각 재료별)
│       ├── Density (kg/m³)
│       │   ├── Constant
│       │   ├── Ideal Gas (이상기체)
│       │   ├── Incompressible Ideal Gas
│       │   ├── Boussinesq (부시네스크 근사)
│       │   ├── Polynomial
│       │   ├── Piecewise-Linear
│       │   └── User-Defined (Expression)
│       ├── Viscosity (Pa·s)
│       │   ├── Constant
│       │   ├── Sutherland (3-coeff)
│       │   ├── Power Law
│       │   ├── Carreau
│       │   ├── Cross
│       │   ├── Herschel-Bulkley
│       │   └── Non-Newtonian Power Law
│       ├── Specific Heat Cp (J/kg·K)
│       │   ├── Constant
│       │   ├── Polynomial
│       │   └── Piecewise-Polynomial
│       ├── Thermal Conductivity (W/m·K)
│       │   ├── Constant
│       │   ├── Polynomial
│       │   └── Piecewise-Linear
│       ├── Molecular Weight (kg/kmol)
│       ├── Absorption Coefficient (복사)
│       ├── Scattering Coefficient (복사)
│       ├── Refractive Index
│       └── Reference Temperature
│
├── Solid Materials
│   ├── 기본 제공: aluminum, steel, copper, ...
│   └── 속성 정의
│       ├── Density (kg/m³)
│       ├── Specific Heat Cp (J/kg·K)
│       ├── Thermal Conductivity (W/m·K)
│       ├── Young's Modulus (Pa) [구조 해석]
│       ├── Poisson Ratio [구조 해석]
│       ├── Yield Stress (Pa) [소성]
│       ├── Thermal Expansion Coefficient (1/K)
│       └── Emissivity (복사)
│
└── Mixture Materials (화학종 사용시)
    ├── Mixture Species List
    ├── Reaction Definitions
    │   ├── Stoichiometric Coefficients
    │   ├── Rate Exponents
    │   ├── Arrhenius Parameters (A, E, β)
    │   └── Third Body Efficiencies
    └── Transport Properties (각 화학종)
        ├── Mass Diffusivity
        ├── Thermal Diffusion Coefficient
        └── Lennard-Jones Parameters
```

---

#### 1.4 Cell Zone Conditions (셀 영역 조건)

```
Cell Zone Conditions
├── Fluid Zones (유체 영역)
│   ├── Zone Name / ID
│   ├── Material
│   ├── Source Terms
│   │   ├── Mass Source (kg/m³·s)
│   │   ├── X-Momentum Source (N/m³)
│   │   ├── Y-Momentum Source (N/m³)
│   │   ├── Z-Momentum Source (N/m³)
│   │   ├── Energy Source (W/m³)
│   │   ├── k Source
│   │   ├── ε / ω Source
│   │   └── Species Source (각 화학종)
│   ├── Fixed Values
│   │   ├── Temperature (고정)
│   │   ├── Velocity Components (고정)
│   │   └── Turbulence Quantities (고정)
│   ├── Porous Zone
│   │   ├── Enabled (true/false)
│   │   ├── Porous Media Model
│   │   │   ├── Ergun (packed bed)
│   │   │   └── Power Law
│   │   ├── Direction Vectors (1, 2)
│   │   ├── Viscous Resistance (1/α, 1/m²)
│   │   ├── Inertial Resistance (C₂, 1/m)
│   │   ├── Porosity (γ)
│   │   ├── Solid Material (열평형)
│   │   └── Heat Transfer Coefficient (비평형)
│   ├── Fan Zone (3D 팬)
│   │   ├── Enabled (true/false)
│   │   ├── Pressure Jump (Pa) / Fan Curve
│   │   ├── Rotation Axis
│   │   └── Rotation Speed (RPM)
│   ├── Motion
│   │   ├── Stationary
│   │   ├── Moving Reference Frame (MRF)
│   │   │   ├── Rotation Axis Origin
│   │   │   ├── Rotation Axis Direction
│   │   │   ├── Rotational Velocity (rad/s)
│   │   │   └── Translational Velocity (m/s)
│   │   └── Sliding Mesh
│   │       ├── Mesh Interface
│   │       └── Rotation Parameters
│   └── Reaction (이 영역 반응 on/off)
│
└── Solid Zones (고체 영역)
    ├── Zone Name / ID
    ├── Material
    ├── Source Terms
    │   └── Energy Source (W/m³)
    ├── Fixed Values
    │   └── Temperature (고정)
    ├── Motion
    │   ├── Stationary
    │   ├── Translational
    │   └── Rotational
    └── Contact Resistance (인접 영역)
```

---

#### 1.5 Boundary Conditions (경계 조건)

```
Boundary Conditions
│
├── Inlet Boundaries (유입)
│   ├── Velocity Inlet
│   │   ├── Velocity Specification Method
│   │   │   ├── Magnitude, Normal to Boundary
│   │   │   ├── Components (Vx, Vy, Vz)
│   │   │   └── Magnitude and Direction
│   │   ├── Velocity Magnitude (m/s) 또는 Profile / Expression
│   │   ├── Turbulence Specification
│   │   │   ├── k and epsilon (직접 입력)
│   │   │   ├── k and omega (직접 입력)
│   │   │   ├── Intensity and Hydraulic Diameter
│   │   │   ├── Intensity and Length Scale
│   │   │   └── Intensity and Viscosity Ratio
│   │   ├── Temperature (에너지 모델 활성시)
│   │   ├── Species Mass Fractions (화학종 활성시)
│   │   ├── DPM Conditions (입자 Escape/Reflect/Trap)
│   │   └── Radiation (External Emissivity, Temperature)
│   │
│   ├── Pressure Inlet
│   │   ├── Gauge Total Pressure (Pa)
│   │   ├── Supersonic/Initial Gauge Pressure (Pa)
│   │   ├── Direction Specification
│   │   ├── Turbulence Specification (위와 동일)
│   │   ├── Temperature
│   │   └── Species
│   │
│   ├── Mass Flow Inlet
│   │   ├── Mass Flow Rate (kg/s) 또는 Mass Flux (kg/s·m²)
│   │   ├── Direction
│   │   ├── Turbulence Specification
│   │   ├── Temperature
│   │   └── Species
│   │
│   └── Far-Field Pressure (압축성 외부 유동)
│       ├── Mach Number
│       ├── Gauge Pressure (Pa)
│       ├── Temperature (K)
│       ├── Flow Direction (X, Y, Z components)
│       └── Turbulence Specification
│
├── Outlet Boundaries (유출)
│   ├── Pressure Outlet
│   │   ├── Gauge Pressure (Pa)
│   │   ├── Backflow Direction Specification
│   │   ├── Backflow Turbulence Specification
│   │   ├── Backflow Temperature
│   │   ├── Backflow Species
│   │   ├── Radial Equilibrium Pressure Distribution
│   │   ├── Target Mass Flow Rate (선택)
│   │   └── Prevent Reverse Flow (on/off)
│   │
│   ├── Outflow
│   │   └── Flow Rate Weighting (다중 출구시 비율)
│   │
│   └── Pressure Far-Field (위 Far-Field와 동일 구조)
│
├── Wall Boundaries (벽면)
│   ├── Wall
│   │   ├── Momentum
│   │   │   ├── No Slip (기본값)
│   │   │   ├── Specified Shear
│   │   │   │   ├── Shear Stress Components (Pa)
│   │   │   │   └── Marangoni Stress
│   │   │   ├── Slip
│   │   │   │   ├── Specular Coefficient
│   │   │   │   └── Slip Length
│   │   │   ├── Moving Wall
│   │   │   │   ├── Absolute / Relative
│   │   │   │   ├── Translational Velocity (m/s)
│   │   │   │   └── Rotational Velocity (rad/s, axis, origin)
│   │   │   └── Roughness
│   │   │       ├── Roughness Height (m)
│   │   │       └── Roughness Constant
│   │   ├── Thermal
│   │   │   ├── Heat Flux (W/m²)
│   │   │   │   └── 0 = adiabatic (단열)
│   │   │   ├── Temperature (K)
│   │   │   ├── Convection
│   │   │   │   ├── Heat Transfer Coefficient (W/m²·K)
│   │   │   │   └── Free Stream Temperature (K)
│   │   │   ├── Radiation
│   │   │   │   ├── External Emissivity
│   │   │   │   └── External Radiation Temperature (K)
│   │   │   ├── Mixed (Convection + Radiation)
│   │   │   └── Coupled (CHT 인터페이스)
│   │   ├── Species
│   │   │   ├── Zero Diffusive Flux
│   │   │   └── Specified Mass Fraction
│   │   ├── DPM
│   │   │   ├── Reflect (반사)
│   │   │   ├── Trap (포획)
│   │   │   ├── Escape (탈출)
│   │   │   ├── Wall-Jet
│   │   │   └── Wall-Film
│   │   ├── Wall Film (Eulerian)
│   │   │   ├── Film Thickness
│   │   │   └── Contact Angle
│   │   └── Radiation Wall
│   │       ├── Internal Emissivity
│   │       ├── Wall Emissivity
│   │       ├── Diffuse Fraction
│   │       └── Irradiation
│   │
│   └── 구조 벽면 (고체 해석)
│       ├── Displacement (고정/자유/지정)
│       ├── Force / Traction
│       └── Contact (접촉 인터페이스)
│
├── Internal Boundaries (내부 경계)
│   ├── Interior (일반 내부면)
│   ├── Fan
│   │   ├── Pressure Jump (Pa) 또는 Fan Curve (ΔP vs Q)
│   │   └── Fan Polynomial Coefficients
│   ├── Radiator
│   │   ├── Loss Coefficient
│   │   └── Heat Transfer per Unit Area
│   ├── Porous Jump
│   │   ├── Face Permeability (m²)
│   │   ├── Porous Medium Thickness (m)
│   │   └── Pressure-Jump Coefficient (1/m)
│   └── Interface (CHT / Mesh Interface)
│       ├── Coupled Wall (유체-고체)
│       ├── Contact Resistance (K·m²/W)
│       └── Mapped Interface
│
├── Symmetry & Periodic
│   ├── Symmetry
│   │   └── (설정 불필요 — 자동 zero-flux)
│   ├── Periodic
│   │   ├── Translational
│   │   │   └── Periodic Direction & Offset
│   │   └── Rotational
│   │       ├── Rotation Axis
│   │       └── Angle
│   └── Axis (축대칭 중심축)
│
└── 특수 경계
    ├── Inlet Vent
    │   ├── Loss Coefficient
    │   ├── Flow Direction
    │   └── Total Pressure
    ├── Intake Fan
    │   ├── Pressure Jump / Fan Curve
    │   └── Total Temperature
    ├── Outlet Vent
    │   ├── Loss Coefficient
    │   └── Ambient Pressure
    ├── Exhaust Fan
    │   ├── Pressure Jump / Fan Curve
    │   └── Ambient Conditions
    └── Non-Reflecting BC (압축성)
        ├── Mach Number
        └── Pressure / Temperature
```

---

#### 1.6 Dynamic Mesh (동적 메시)

```
Dynamic Mesh
├── Enable Dynamic Mesh (true/false)
│
├── Mesh Methods
│   ├── Smoothing
│   │   ├── Spring-Based
│   │   │   ├── Spring Constant Factor
│   │   │   ├── Convergence Tolerance
│   │   │   └── Number of Iterations
│   │   ├── Diffusion-Based
│   │   │   ├── Diffusion Parameter
│   │   │   └── Diffusion Function (Uniform, Inverse-Distance, etc.)
│   │   └── Laplacian
│   │       ├── Relaxation Factor
│   │       └── Number of Iterations
│   │
│   ├── Layering
│   │   ├── Split Factor (기본 1.4)
│   │   ├── Collapse Factor (기본 0.2)
│   │   ├── Height-Based / Ratio-Based
│   │   └── Ideal Cell Height
│   │
│   └── Remeshing
│       ├── Method
│       │   ├── Local Cell
│       │   ├── Local Face
│       │   └── Region Face
│       ├── Minimum Cell Size
│       ├── Maximum Cell Size
│       ├── Maximum Cell Skewness
│       ├── Maximum Face Skewness
│       └── Size Remeshing Interval
│
├── Six DOF Solver (6자유도)
│   ├── Enabled (true/false)
│   ├── Mass (kg)
│   ├── Moment of Inertia (Ixx, Iyy, Izz, Ixy, Ixz, Iyz)
│   ├── Center of Gravity (x, y, z)
│   ├── External Forces / Moments
│   ├── Constraints
│   │   ├── Translation (X/Y/Z lock)
│   │   └── Rotation (X/Y/Z lock)
│   └── Properties
│       ├── Scheme (Direct, Iterative)
│       └── Under-Relaxation Factor
│
├── Motion Zones (영역별 운동 정의)
│   ├── Zone Name
│   ├── Motion Type
│   │   ├── Stationary
│   │   ├── Rigid Body (단일 이동/회전)
│   │   │   ├── Translational Velocity
│   │   │   ├── Rotational Velocity
│   │   │   ├── Center of Rotation
│   │   │   └── Axis of Rotation
│   │   ├── Deforming (변형)
│   │   │   ├── Geometry Definition (geometry motion)
│   │   │   └── Remeshing Parameters
│   │   ├── User-Defined (Expression/UDF)
│   │   └── System Coupling (외부 연성)
│   └── Motion Attributes
│       ├── Profile (시간 프로파일)
│       └── Named Expression 참조
│
└── Events
    ├── Mesh Motion Events (시간 기반 트리거)
    └── Remeshing Events
```

---

#### 1.7 Reference Values (기준값)

```
Reference Values
├── Area (m²)                     # 항력/양력 계수 산정 기준 면적
├── Density (kg/m³)               # 기준 밀도
├── Depth (m)                     # 2D 해석 깊이
├── Enthalpy (J/kg)               # 기준 엔탈피
├── Length (m)                    # 기준 길이 (Re 수 산정)
├── Pressure (Pa)                 # 기준 압력
├── Temperature (K)               # 기준 온도
├── Velocity (m/s)                # 기준 속도
├── Viscosity (Pa·s)              # 기준 점성계수
├── Ratio of Specific Heats (γ)   # 비열비
└── Compute From Zone (자동 계산)  # 특정 영역에서 자동 추출
```

**JSON 매핑:**
```json
{
  "setup": {
    "reference_values": {
      "area": 1.0,
      "density": 1.225,
      "depth": 1.0,
      "length": 1.0,
      "pressure": 0.0,
      "temperature": 288.16,
      "velocity": 1.0,
      "viscosity": 1.7894e-5,
      "ratio_of_specific_heats": 1.4,
      "compute_from": "inlet"
    }
  }
}
```

---

#### 1.8 Named Expressions (사용자 정의 수식)

```
Named Expressions
├── Expression Name (고유 이름)
├── Expression Definition (수식 문자열)
│   ├── 수학 함수: sin, cos, exp, log, sqrt, abs, min, max, pow, ...
│   ├── 조건문: if(condition, true_val, false_val)
│   ├── 필드 변수 참조: $velocity_magnitude, $temperature, $pressure, ...
│   ├── 좌표 참조: $x, $y, $z, $r, $theta
│   ├── 시간 참조: $t, $time_step
│   ├── 상수: $pi, $e
│   └── 다른 Expression 참조: ${expr_name}
├── Units (단위)
├── Type
│   ├── Scalar
│   ├── Vector
│   └── Boolean
└── Used In (사용처 추적)
    ├── Boundary Conditions
    ├── Source Terms
    ├── Material Properties
    ├── Dynamic Mesh Motion
    └── Monitor Definitions
```

**JSON 매핑:**
```json
{
  "setup": {
    "named_expressions": {
      "parabolic_inlet": {
        "definition": "1.5 * (1 - (($y - 0.05) / 0.05)^2)",
        "units": "m/s",
        "type": "scalar"
      },
      "time_varying_temp": {
        "definition": "300 + 50 * sin(2 * $pi * $t / 10)",
        "units": "K",
        "type": "scalar"
      },
      "rotating_velocity": {
        "definition": "if($t < 5, 100 * $t / 5, 100)",
        "units": "rad/s",
        "type": "scalar"
      }
    }
  }
}
```

---

### 12.3 SOLVER (수치 해법 설정) — 상세 구조

#### 2.1 Solution Methods (해법 방법)

```
Solution Methods
│
├── Pressure-Velocity Coupling (압력-속도 커플링)
│   ├── SIMPLE                    # 기본, 정상상태 적합
│   ├── SIMPLEC                   # 빠른 수렴, 비압축
│   ├── PISO                      # 비정상 적합
│   │   ├── Skewness Correction
│   │   └── Neighbor Correction
│   ├── Coupled                   # 압력-속도 동시 풀이
│   │   └── Courant Number (기본: 200)
│   └── Fractional Step (FSM)     # 비반복 시간 전진
│
├── Spatial Discretization (공간 이산화)
│   ├── Gradient Method
│   │   ├── Green-Gauss Cell-Based
│   │   ├── Green-Gauss Node-Based
│   │   └── Least Squares Cell-Based (기본값)
│   │
│   ├── Pressure Interpolation
│   │   ├── Standard
│   │   ├── PRESTO! (Pressure Staggering)
│   │   ├── Linear
│   │   ├── Second Order
│   │   └── Body Force Weighted
│   │
│   ├── Momentum
│   │   ├── First Order Upwind
│   │   ├── Second Order Upwind (기본 권장)
│   │   ├── Power Law
│   │   ├── Central Differencing
│   │   ├── Bounded Central Differencing (LES 권장)
│   │   ├── QUICK
│   │   ├── MUSCL (3rd Order)
│   │   └── TVD Schemes (Van Leer, Van Albada, Minmod, Superbee)
│   │
│   ├── Turbulent Kinetic Energy (k)
│   │   ├── First Order Upwind
│   │   ├── Second Order Upwind
│   │   └── QUICK
│   │
│   ├── Turbulent Dissipation Rate (ε) / Specific Dissipation (ω)
│   │   ├── First Order Upwind
│   │   ├── Second Order Upwind
│   │   └── QUICK
│   │
│   ├── Energy / Temperature
│   │   ├── First Order Upwind
│   │   ├── Second Order Upwind
│   │   └── QUICK
│   │
│   ├── Species
│   │   ├── First Order Upwind
│   │   ├── Second Order Upwind
│   │   └── QUICK
│   │
│   └── Volume Fraction (다상)
│       ├── First Order Upwind (Implicit)
│       ├── Second Order Upwind (Implicit)
│       ├── Compressive (VOF)
│       ├── CICSAM
│       └── Geo-Reconstruct (가장 정밀, Explicit VOF)
│
├── Transient Formulation (비정상 시간 이산화)
│   ├── First Order Implicit
│   ├── Second Order Implicit (기본 권장)
│   ├── Bounded Second Order Implicit
│   └── Non-Iterative Time Advancement (NITA)
│       ├── Fractional Step Method
│       └── Max Corrections per Time Step
│
└── Density-Based Solver Options (밀도 기반 솔버 전용)
    ├── Formulation
    │   ├── Implicit
    │   └── Explicit
    ├── Flux Type
    │   ├── Roe-FDS
    │   ├── AUSM
    │   └── AUSM+
    └── Entropy Fix (Roe)
```

---

#### 2.2 Solution Controls (수렴 제어)

```
Solution Controls
│
├── Under-Relaxation Factors (아래완화 인자)
│   ├── Pressure                 # 기본값: 0.3
│   ├── Density                  # 기본값: 1.0
│   ├── Body Forces              # 기본값: 1.0
│   ├── Momentum                 # 기본값: 0.7
│   ├── Turbulent Kinetic Energy # 기본값: 0.8
│   ├── Turbulent Dissipation    # 기본값: 0.8
│   ├── Specific Dissipation     # 기본값: 0.8
│   ├── Turbulent Viscosity      # 기본값: 1.0
│   ├── Energy                   # 기본값: 1.0
│   ├── Species                  # 기본값: 1.0
│   ├── Discrete Phase Sources   # 기본값: 0.5
│   └── 사용자 정의 스칼라...
│   [참고: Pressure + Momentum = 1.0 권장]
│
├── Courant Number (Coupled 솔버)
│   ├── Flow Courant Number       # 기본값: 200 (정상), 가변 (비정상)
│   └── Solid Courant Number      # 고체 영역
│
├── Pseudo Time Method (가상시간)
│   ├── Local Time Stepping
│   │   ├── Length Scale Method (Aggressive, Conservative, User-Specified)
│   │   └── Verbosity
│   ├── Global Time Stepping
│   │   └── Time Step Size
│   └── Automatic
│
├── Limits (물리량 제한)
│   ├── Minimum Static Temperature (K)    # 기본: 1
│   ├── Maximum Static Temperature (K)    # 기본: 5000
│   ├── Minimum Turb. Kinetic Energy      # 기본: 1e-14
│   ├── Minimum Turb. Dissipation Rate    # 기본: 1e-20
│   ├── Maximum Turb. Viscosity Ratio     # 기본: 1e5
│   └── Minimum Absolute Pressure (Pa)    # 기본: 1
│
└── Equations (활성 방정식 on/off)
    ├── Flow (on/off)
    ├── Turbulence (on/off)       # 유동 수렴 후 활성화 가능
    ├── Energy (on/off)
    ├── Species (on/off)
    ├── Discrete Phase (on/off)
    └── 사용자 정의 스칼라 (on/off)
```

**JSON 매핑:**
```json
{
  "solver": {
    "controls": {
      "under_relaxation": {
        "pressure": 0.3,
        "momentum": 0.7,
        "turbulent_kinetic_energy": 0.8,
        "turbulent_dissipation_rate": 0.8,
        "energy": 1.0
      },
      "courant_number": 200,
      "limits": {
        "min_temperature": 1,
        "max_temperature": 5000,
        "max_turbulent_viscosity_ratio": 1e5
      },
      "equations": {
        "flow": true,
        "turbulence": true,
        "energy": true
      }
    }
  }
}
```

---

#### 2.3 Monitors (모니터링)

```
Monitors
│
├── Residual Monitor
│   ├── Monitored Equations
│   │   ├── Continuity
│   │   ├── X-Velocity
│   │   ├── Y-Velocity
│   │   ├── Z-Velocity
│   │   ├── Energy
│   │   ├── k (난류 운동에너지)
│   │   ├── epsilon / omega
│   │   └── Species-N
│   ├── Convergence Criteria (각 방정식별)
│   │   ├── Absolute Criteria (기본: 1e-3, Energy: 1e-6)
│   │   └── Relative Criteria (선택)
│   ├── Normalization
│   │   ├── Scale (Global / Local)
│   │   └── Normalize by First Iteration
│   ├── Print to Console (on/off)
│   ├── Plot (on/off)
│   └── Window ID
│
├── Report Definitions (리포트 정의)
│   ├── Force Report
│   │   ├── Drag Coefficient (Cd)
│   │   │   ├── Force Direction Vector
│   │   │   ├── Wall Zones
│   │   │   └── Reference Values (Area, Velocity, Density)
│   │   ├── Lift Coefficient (Cl)
│   │   │   ├── Force Direction Vector
│   │   │   └── Wall Zones
│   │   └── Moment Coefficient (Cm)
│   │       ├── Moment Center
│   │       ├── Moment Axis
│   │       └── Wall Zones
│   │
│   ├── Surface Report
│   │   ├── Field Variable (Pressure, Temperature, Velocity, ...)
│   │   ├── Surface Selection
│   │   └── Report Type
│   │       ├── Area-Weighted Average
│   │       ├── Mass-Weighted Average
│   │       ├── Integral
│   │       ├── Flow Rate (Mass/Volume)
│   │       ├── Total Heat Transfer Rate
│   │       ├── Uniformity Index
│   │       ├── Facet Minimum / Maximum
│   │       └── Standard Deviation
│   │
│   ├── Volume Report
│   │   ├── Field Variable
│   │   ├── Cell Zone Selection
│   │   └── Report Type
│   │       ├── Volume Average
│   │       ├── Volume Integral
│   │       ├── Mass Average
│   │       ├── Mass Integral
│   │       ├── Sum
│   │       ├── Minimum / Maximum
│   │       └── Standard Deviation
│   │
│   └── User-Defined Report (Expression 기반)
│
├── Report Plots
│   ├── Data Sources (1개 이상의 Report Definition)
│   ├── X-Axis: Iteration / Time Step / Flow Time
│   ├── Y-Axis: Report Value(s)
│   ├── Plot Window ID
│   └── File Output (선택)
│       ├── File Name
│       └── Format (CSV / JSON)
│
├── Report Files
│   ├── Data Sources (Report Definitions)
│   ├── File Name
│   ├── Format (CSV / JSON)
│   └── Write Frequency (Every Iteration / N Iterations / Time Step)
│
├── Point Monitors (프로브)
│   ├── Location (x, y, z)
│   ├── Monitored Variables (velocity, pressure, temperature, ...)
│   └── Write Frequency
│
└── Surface Monitors
    ├── Surface Name
    ├── Report Type (Average, Integral, Max, ...)
    ├── Field Variable
    └── Write Frequency
```

---

#### 2.4 Initialization (초기화)

```
Initialization
│
├── Standard Initialization
│   ├── Compute From (Reference Zone: inlet, outlet, ...)
│   ├── Initial Values
│   │   ├── Gauge Pressure (Pa)
│   │   ├── X-Velocity (m/s)
│   │   ├── Y-Velocity (m/s)
│   │   ├── Z-Velocity (m/s)
│   │   ├── Temperature (K)
│   │   ├── Turbulent Kinetic Energy (m²/s²)
│   │   ├── Turbulent Dissipation Rate (m²/s³) 또는 Specific Dissipation (1/s)
│   │   ├── Species Mass Fractions
│   │   └── Volume Fractions (다상)
│   └── Initialize
│
├── Hybrid Initialization
│   ├── General Settings
│   │   ├── Number of Iterations (기본: 10)
│   │   └── Turbulence Model Specific
│   ├── Laplace Equation Solve (pressure, velocity)
│   └── Initialize
│
├── FMG Initialization (Full Multigrid)
│   ├── 난이도 높은 수렴을 위한 최강 초기화
│   ├── Coarsening Levels
│   ├── Iterations per Level
│   └── Initialize
│
├── Patch (영역별 초기값 패치)
│   ├── Zone Selection
│   ├── Variable Selection
│   ├── Value 또는 Expression
│   └── Apply
│
└── Read Data / Interpolate
    ├── Read Previous Solution File
    ├── Interpolate from Different Mesh
    └── Data File Format
```

**JSON 매핑:**
```json
{
  "solver": {
    "initialization": {
      "method": "hybrid",
      "hybrid_settings": {
        "num_iterations": 10
      },
      "patch": [
        {
          "zone": "fluid_region",
          "variable": "temperature",
          "value": 500
        }
      ],
      "initial_values": {
        "gauge_pressure": 0,
        "velocity": [0, 0, 0],
        "temperature": 300,
        "k": 0.1,
        "omega": 10
      }
    }
  }
}
```

---

#### 2.5 Calculation Activities (계산 활동)

```
Calculation Activities
│
├── Autosave
│   ├── Save Data File Every N Iterations / Time Steps
│   ├── Save Data File at Last Iteration
│   ├── Maximum Number of Data Files
│   ├── File Name Pattern
│   │   └── e.g., "case_%04d.gfd"
│   ├── Append File Number
│   └── Save Format
│       ├── Native (.gfd)
│       ├── OpenVDB (.vdb)
│       └── VTK (.vtu)
│
├── Automatic Export
│   ├── Solution Data Export
│   │   ├── Export Surfaces
│   │   ├── Export Variables
│   │   ├── File Type (VDB, VTK, CSV, CGNS)
│   │   └── Export Frequency
│   ├── Particle History Data (DPM)
│   └── Custom Export (Expression-based filter)
│
├── Execute Commands (사용자 정의 명령)
│   ├── Command Name
│   ├── Command String (스크립트)
│   ├── Trigger
│   │   ├── Every N Iterations
│   │   ├── Every N Time Steps
│   │   ├── At End
│   │   ├── On Convergence
│   │   └── At Specified Iteration/Time
│   └── Active (on/off)
│
└── Solution Steering (자동 수렴 전략)
    ├── Flow Type (Subsonic / Transonic / Supersonic)
    ├── Initial CFL
    ├── Max CFL
    ├── CFL Growth Rate
    └── Convergence Checks
```

---

### 12.4 RUN CALCULATION (계산 실행) — 상세 구조

#### 3.1 Run Settings (실행 설정)

```
Run Calculation
│
├── Steady-State Settings
│   ├── Number of Iterations
│   ├── Reporting Interval
│   ├── Profile Update Interval
│   └── Run Calculation (Start)
│
├── Transient Settings
│   ├── Time Step Size (s)
│   ├── Number of Time Steps
│   ├── Max Iterations per Time Step
│   ├── Reporting Interval
│   ├── Profile Update Interval
│   ├── Extrapolation Order (0, 1)
│   └── Options
│       ├── Frozen Flux Formulation
│       └── Data Sampling for Time Statistics
│           ├── Sampling Interval
│           └── Sampled Variables (Mean, RMS, ...)
│
├── Adaptive Time Stepping (자동 시간 간격)
│   ├── Enabled (true/false)
│   ├── Truncation Error Tolerance
│   ├── Ending Time
│   ├── Minimum Time Step Size
│   ├── Maximum Time Step Size
│   ├── Minimum Step Change Factor
│   └── Maximum Step Change Factor
│
└── Solution Checks (계산 전 검증)
    ├── Mesh Check
    │   ├── Minimum Orthogonal Quality
    │   ├── Maximum Skewness
    │   ├── Maximum Aspect Ratio
    │   └── Negative Volume Check
    ├── Setup Completeness
    │   ├── All BCs Defined?
    │   ├── All Materials Assigned?
    │   └── Solver Compatibility Check
    └── Estimated Memory / Disk Usage
```

#### 3.2 Schedule / Batch (스케줄 / 배치)

```
Schedule / Batch
│
├── Batch Mode Execution
│   ├── Input JSON File Path
│   ├── Output Directory
│   ├── Log File Path
│   ├── Number of Threads / MPI Processes
│   └── Priority (Normal / Low / High)
│
├── Parameter Study (파라미터 스터디)
│   ├── Design Points
│   │   ├── Parameter Name
│   │   ├── Values List [v1, v2, v3, ...]
│   │   └── Type (Continuous / Discrete)
│   ├── Sweep Method
│   │   ├── Full Factorial
│   │   ├── Latin Hypercube
│   │   └── Custom
│   └── Run Settings per Point
│
├── Queue System (대기열)
│   ├── Max Concurrent Jobs
│   ├── Job Priority
│   └── Job Dependencies
│
└── Remote Execution (원격 실행)
    ├── SSH Host
    ├── Working Directory
    ├── Resource Allocation (CPU, Memory)
    └── File Transfer (Input/Output)
```

**JSON 매핑:**
```json
{
  "run": {
    "type": "steady",
    "iterations": 5000,
    "reporting_interval": 10,
    "check": {
      "mesh_quality": true,
      "setup_completeness": true
    },
    "batch": {
      "threads": 16,
      "log_file": "logs/simulation.log"
    }
  }
}
```

---

### 12.5 RESULTS (결과 후처리) — 상세 구조

#### 4.1 Graphics (그래픽)

```
Graphics
│
├── Mesh Display
│   ├── Surfaces / Zones to Display
│   ├── Display Style
│   │   ├── Wireframe
│   │   ├── Surface
│   │   └── Feature Edges
│   ├── Coloring (Zone / Partition / Material)
│   └── Transparency / Opacity
│
├── Contours (등고선)
│   ├── Field Variable
│   │   ├── Pressure (Static, Dynamic, Total, Absolute, Coefficient)
│   │   ├── Velocity (Magnitude, X/Y/Z, Vorticity, Helicity, Stream Function)
│   │   ├── Temperature (Static, Total)
│   │   ├── Turbulence (k, ε, ω, μt, y+, Wall Shear Stress)
│   │   ├── Density
│   │   ├── Species Mass Fractions
│   │   ├── Volume Fraction (다상)
│   │   ├── Stress (von Mises, Principal, Components)
│   │   ├── Strain (Equivalent, Principal, Components)
│   │   ├── Displacement (Magnitude, X/Y/Z)
│   │   └── Custom Field (Expression)
│   ├── Surfaces
│   ├── Range
│   │   ├── Auto Range
│   │   └── Custom Min/Max
│   ├── Colormap
│   │   ├── Scheme (Rainbow, Jet, Viridis, Coolwarm, ...)
│   │   ├── Levels (등급 수)
│   │   ├── Log Scale (on/off)
│   │   └── Reverse (on/off)
│   ├── Options
│   │   ├── Filled Contours / Lines Only
│   │   ├── Node Values / Cell Values
│   │   └── Draw Mesh
│   └── Clip Planes (절단면)
│       ├── Normal Vector
│       └── Point on Plane
│
├── Vectors (벡터)
│   ├── Vector Field (Velocity, Force, Displacement, ...)
│   ├── Color By (Velocity Magnitude, Pressure, Temperature, ...)
│   ├── Surfaces
│   ├── Scale Factor
│   ├── Skip Factor
│   ├── Style
│   │   ├── Arrow
│   │   ├── Line
│   │   └── Cone
│   └── Uniform Length (on/off)
│
├── Pathlines (경로선)
│   ├── Release From (Surface)
│   ├── Color By (Variable)
│   ├── Steps
│   ├── Step Size
│   ├── Max Time / Max Steps
│   ├── Direction
│   │   ├── Forward
│   │   ├── Backward
│   │   └── Both
│   ├── Options
│   │   ├── Continuous / Pulsed
│   │   ├── Reverse Direction
│   │   └── Relative Pathlines (비정상)
│   └── Width / Style (Line, Ribbon, Tube)
│
├── Streamlines (유선)
│   ├── Release Surface
│   ├── Field (Velocity)
│   ├── Color By
│   ├── Density (Seeds per Surface)
│   └── Style (Line, Tube, Ribbon)
│
├── Iso-Surfaces (등가면)
│   ├── Field Variable
│   ├── Iso-Value(s)
│   ├── Display Options (Color, Transparency)
│   └── Clip to Zone
│
├── Scenes (복합 장면)
│   ├── Multiple Graphics Objects
│   ├── Transparency per Object
│   └── Background Color
│
└── LIC (Line Integral Convolution)
    ├── Surface
    ├── Vector Field
    └── Texture Resolution
```

---

#### 4.2 Plots (플롯)

```
Plots
│
├── XY Plot
│   ├── Plot Type
│   │   ├── Solution (필드 변수 vs 위치)
│   │   └── File (외부 데이터 비교)
│   ├── Y-Axis Variable
│   │   └── (위 Contours와 동일한 변수 목록)
│   ├── X-Axis
│   │   ├── Position (X, Y, Z, Distance)
│   │   └── Variable (다른 필드 변수)
│   ├── Surfaces / Lines
│   ├── Plot Direction (X, Y, Z)
│   ├── Legend
│   ├── Axes Formatting
│   │   ├── Range (Auto / Custom)
│   │   ├── Scale (Linear / Log)
│   │   └── Label
│   └── Export to File (CSV, PNG, SVG)
│
├── Residual Plot
│   ├── Equations to Plot
│   ├── Scale (Log)
│   ├── Iterations Range
│   └── Convergence Lines
│
├── FFT / Power Spectrum (주파수 분석)
│   ├── Monitor Data Source
│   ├── Sampling Rate
│   ├── Window Function (Hanning, Hamming, ...)
│   └── Plot (Frequency vs Amplitude)
│
├── Histogram
│   ├── Field Variable
│   ├── Zone / Surface
│   ├── Number of Bins
│   └── Weighting (None, Area, Volume, Mass)
│
└── Report Plots (Section 2.3에서 정의된 것)
```

---

#### 4.3 Reports (보고서)

```
Reports
│
├── Fluxes
│   ├── Mass Flow Rate (각 경계별)
│   ├── Heat Transfer Rate (각 경계별)
│   ├── Radiation Heat Transfer Rate
│   └── Net / Total Imbalance (질량/에너지 균형 확인)
│
├── Forces
│   ├── Force on Walls
│   │   ├── Pressure Force (Fx, Fy, Fz)
│   │   ├── Viscous Force (Fx, Fy, Fz)
│   │   └── Total Force
│   ├── Coefficients
│   │   ├── Drag Coefficient (Cd)
│   │   ├── Lift Coefficient (Cl)
│   │   └── Moment Coefficient (Cm)
│   └── Center of Pressure
│
├── Surface Integrals
│   ├── Area (m²)
│   ├── Integral (변수 적분)
│   ├── Area-Weighted Average
│   ├── Mass-Weighted Average
│   ├── Flow Rate (질량/체적)
│   ├── Total Heat Transfer Rate (W)
│   ├── Total Pressure (area-avg)
│   ├── Uniformity Index
│   ├── Custom (Expression)
│   ├── Minimum / Maximum / Std-Dev
│   └── Facet Average
│
├── Volume Integrals
│   ├── Volume (m³)
│   ├── Volume Average
│   ├── Volume Integral
│   ├── Mass Average
│   ├── Mass Integral
│   ├── Mass (kg)
│   ├── Sum
│   ├── Minimum / Maximum / Std-Dev
│   └── Custom (Expression)
│
├── y+ Report (벽면 품질 확인)
│   ├── Min / Max / Average y+
│   ├── % Cells in Range
│   │   ├── y+ < 1 (Resolved)
│   │   ├── 1 < y+ < 5
│   │   ├── 5 < y+ < 30 (Buffer)
│   │   └── y+ > 30 (Log-Law)
│   └── Wall Zone Selection
│
├── Reference Values Computation
│   ├── Reynolds Number
│   ├── Mach Number (압축성)
│   ├── Nusselt Number
│   ├── Prandtl Number
│   └── Rayleigh Number (자연대류)
│
└── Summary Report
    ├── Case Information
    ├── Solver Settings
    ├── Convergence Status
    ├── Key Performance Indicators
    └── Export (HTML, PDF, JSON)
```

---

#### 4.4 Animations (애니메이션)

```
Animations
│
├── Solution Animation
│   ├── Graphics Object (Contour, Vector, Pathline, ...)
│   ├── Animation Type
│   │   ├── Per Iteration (정상)
│   │   └── Per Time Step (비정상)
│   ├── Record Frequency
│   ├── Window
│   └── View Angle
│
├── Playback
│   ├── Frame Rate (fps)
│   ├── Loop (on/off)
│   └── Reverse (on/off)
│
└── Export
    ├── Image Sequence (PNG, JPEG, BMP)
    │   ├── Resolution (Width x Height)
    │   └── Directory
    ├── Video (MP4, AVI, GIF)
    │   ├── Codec
    │   ├── Quality / Bitrate
    │   └── Frame Rate
    └── VDB Sequence (시간별 .vdb 파일)
```

---

#### 4.5 Data Export (데이터 내보내기)

```
Data Export
│
├── Solution Data
│   ├── Format
│   │   ├── OpenVDB (.vdb) — 기본
│   │   ├── VTK (.vtu / .vtm / .pvd)
│   │   ├── CGNS (.cgns)
│   │   ├── EnSight (.case + .geo + .scl/vec)
│   │   ├── Tecplot (.plt / .szplt)
│   │   └── CSV (표 형식)
│   ├── Variables to Export
│   ├── Surfaces / Zones
│   ├── Node / Cell Data
│   └── ASCII / Binary
│
├── Tabular Data
│   ├── Probe History → CSV / JSON
│   ├── Report Data → CSV / JSON
│   ├── Residual History → CSV / JSON
│   └── Surface Data → CSV
│
├── Mesh Export
│   ├── Gmsh (.msh)
│   ├── CGNS (.cgns)
│   ├── STL (.stl)
│   └── VTK (.vtu)
│
└── Checkpoint / Restart
    ├── Full State Binary (.gfd-checkpoint)
    ├── Portable (cross-platform)
    └── Compression (on/off)
```

---

### 12.6 전체 JSON 구조 통합 개요

위의 모든 워크플로우를 반영한 최상위 JSON 스키마:

```json
{
  "setup": {
    "general": { "..." },
    "models": {
      "multiphase": { "..." },
      "energy": { "..." },
      "viscous": { "..." },
      "radiation": { "..." },
      "heat_exchanger": { "..." },
      "species": { "..." },
      "discrete_phase": { "..." },
      "solidification_melting": { "..." },
      "acoustics": { "..." },
      "structure": { "..." },
      "eulerian_wall_film": { "..." },
      "electrochemistry": { "..." }
    },
    "materials": {
      "fluids": { "..." },
      "solids": { "..." },
      "mixtures": { "..." }
    },
    "cell_zone_conditions": [ { "..." } ],
    "boundary_conditions": [ { "..." } ],
    "dynamic_mesh": { "..." },
    "reference_values": { "..." },
    "named_expressions": { "..." }
  },
  "solver": {
    "methods": {
      "pressure_velocity_coupling": "...",
      "spatial_discretization": { "..." },
      "transient_formulation": "..."
    },
    "controls": {
      "under_relaxation": { "..." },
      "courant_number": "...",
      "limits": { "..." },
      "equations": { "..." }
    },
    "monitors": {
      "residuals": { "..." },
      "report_definitions": [ { "..." } ],
      "report_plots": [ { "..." } ],
      "point_monitors": [ { "..." } ]
    },
    "initialization": { "..." },
    "calculation_activities": {
      "autosave": { "..." },
      "automatic_export": { "..." },
      "execute_commands": [ { "..." } ]
    }
  },
  "run": {
    "type": "steady | transient",
    "iterations": "...",
    "time_stepping": { "..." },
    "adaptive_time_stepping": { "..." },
    "checks": { "..." },
    "batch": { "..." },
    "parameter_study": { "..." }
  },
  "results": {
    "graphics": { "..." },
    "plots": { "..." },
    "reports": { "..." },
    "animations": { "..." },
    "data_export": { "..." }
  }
}
```

---

### 12.7 Fluent 대비 GFD 확장 사항

| 항목 | Fluent | GFD (추가) |
|------|--------|-----------|
| 고체역학 | FSI 제한적 | 완전한 FEA (탄성/소성/크리프/접촉) |
| 출력 형식 | .cas/.dat 독점 | OpenVDB (오픈) + VTK |
| 입력 형식 | GUI + Journal | JSON (프로그래밍 친화) |
| Expression | Fluent Expression Language | Rust 기반 Expression Engine |
| 병렬화 | MPI + GPU | Rayon (스레드) + MPI + GPU (향후) |
| 스크립팅 | Scheme / Python | JSON + CLI + Rust Plugin |
| 배치 실행 | Fluent Batch | 네이티브 CLI (`gfd run`) |
| 파라미터 스터디 | Workbench DOE | 내장 Sweep Engine |
| 라이선스 | 상용 | 오픈소스 (MIT/Apache) |

---

## 13. 사용자 정의 수학식 편집 시스템 및 SDK 설계

> **핵심 철학:** GFD 솔버의 모든 수학식은 "블랙박스"가 아니라 **투명하게 공개**되어야 한다.
> 사용자는 기본 제공 모델을 그대로 쓸 수도 있고, GUI에서 수학식을 직접 수정하여 자신만의 모델을 만들 수도 있다.
> 이를 위해 **연속형 수학식 편집 → 자동 이산화 → 행렬 조립 → 솔버 실행**의 전 과정을 SDK로 제공한다.

---

### 13.1 전체 아키텍처: 수학식 편집 파이프라인

```
┌─────────────────────────────────────────────────────────────────────┐
│                        사용자 워크플로우                              │
│                                                                     │
│  ① 모델 선택        ② Edit Expression     ③ 검증          ④ 실행    │
│  (k-ω SST 등)  →   (수학식 GUI 편집)   →  (차원/안정성)  →  (솔버)   │
│                          │                                          │
│                    ┌─────┴──────┐                                    │
│                    │  2가지 모드  │                                   │
│               ┌────┴────┐  ┌────┴─────┐                              │
│               │ 연속형   │  │ 이산형    │                             │
│               │ PDE 입력 │  │ 직접 입력 │                             │
│               └────┬────┘  └────┬─────┘                              │
│                    │            │                                     │
│           자동 이산화 엔진       │ (사용자가 이산화된 식 직접 제공)      │
│                    │            │                                     │
│                    └─────┬──────┘                                     │
│                          ▼                                           │
│                   행렬 조립 엔진                                      │
│                          │                                           │
│                          ▼                                           │
│                   선형 솔버 실행                                      │
│                          │                                           │
│                          ▼                                           │
│                      결과 출력                                        │
└─────────────────────────────────────────────────────────────────────┘
```

---

### 13.2 수학식 편집 모드 (2가지)

#### Mode A: 연속형 PDE 편집 (Continuous Mode)

사용자가 **연속 편미분방정식(PDE)** 을 수학 표기로 입력하면, GFD의 **자동 이산화 엔진**이 FVM/FEM으로 변환한다.

```
예: k-ω SST 모델의 k 방정식을 수정하는 경우

[기본 제공 수식]
∂(ρk)/∂t + ∇·(ρuk) = ∇·((μ + σₖμₜ)∇k) + P̃ₖ - β*ρkω

[사용자 수정] — 예: 부력 생산항 Gₖ 추가
∂(ρk)/∂t + ∇·(ρuk) = ∇·((μ + σₖμₜ)∇k) + P̃ₖ + Gₖ - β*ρkω

→ GFD 자동 이산화 엔진이 FVM 이산식으로 변환
→ 행렬 조립에 반영
```

#### Mode B: 이산형 직접 입력 (Discrete Mode)

사용자가 **이미 이산화된 수식**을 셀/면/노드 단위로 직접 입력한다.

```
예: 같은 k 방정식의 이산형을 직접 작성

aₚkₚ = Σ(aₙₖkₙₖ) + Sₖ·V

여기서:
  aₙₖ = D_f·A_f + max(-F_f, 0)        # 인접 셀 계수
  aₚ  = Σ(aₙₖ) + F_out - Sₚ·V + ρ·V/Δt  # 중심 셀 계수
  Sₖ  = (P̃ₖ + Gₖ)                      # 소스항 (사용자 수정)
  Sₚ  = -β*ρω                          # 소스항 음의 기여분
  D_f = (μ + σₖμₜ)/d_f · A_f           # 확산 플럭스
  F_f = ρ·u_f·A_f                      # 대류 플럭스

→ 행렬 조립에 직접 반영 (이산화 엔진 우회)
```

---

### 13.3 GUI에서의 수학식 편집 워크플로우

```
┌──────────────────────────────────────────────────────────────┐
│  Models > Viscous > k-omega SST                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │ ● Use Default Equations                                 │ │
│  │ ○ Edit Equations                                        │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
│  [Edit Expression] 버튼 클릭 시:                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Equation Editor — k-omega SST                          │ │
│  │  ┌───────────────────────────────────────────────────┐  │ │
│  │  │ Equations:                                        │  │ │
│  │  │  ☑ k-equation        [Edit] [Reset to Default]   │  │ │
│  │  │  ☑ ω-equation        [Edit] [Reset to Default]   │  │ │
│  │  │  ☑ Eddy Viscosity    [Edit] [Reset to Default]   │  │ │
│  │  │  ☑ Blending Function [Edit] [Reset to Default]   │  │ │
│  │  │  ☑ Model Constants   [Edit] [Reset to Default]   │  │ │
│  │  └───────────────────────────────────────────────────┘  │ │
│  │                                                         │ │
│  │  [Edit] 클릭 시 → Equation Editor Panel 열림:            │ │
│  │  ┌───────────────────────────────────────────────────┐  │ │
│  │  │ Mode: ● Continuous (PDE)  ○ Discrete (직접)       │  │ │
│  │  │                                                   │  │ │
│  │  │ k-equation (Continuous):                          │  │ │
│  │  │ ┌───────────────────────────────────────────────┐ │  │ │
│  │  │ │ ∂(ρk)/∂t + ∇·(ρuk)                           │ │  │ │
│  │  │ │   = ∇·((μ + σ_k·μ_t)·∇k)                    │ │  │ │
│  │  │ │   + P_k_tilde                                 │ │  │ │
│  │  │ │   - β_star·ρ·k·ω                             │ │  │ │
│  │  │ │   + G_k          ← 사용자가 추가한 항          │ │  │ │
│  │  │ └───────────────────────────────────────────────┘ │  │ │
│  │  │                                                   │  │ │
│  │  │ Sub-expressions:                                  │  │ │
│  │  │  P_k_tilde = min(μ_t·S², 10·β_star·ρ·k·ω)      │  │ │
│  │  │  G_k = -β_g·(μ_t/Pr_t)·(g·∇ρ)    [새로 정의]   │  │ │
│  │  │  β_g = 1.0                          [새 상수]    │  │ │
│  │  │                                                   │  │ │
│  │  │ ┌─────────┐ ┌─────────────┐ ┌──────────────────┐ │  │ │
│  │  │ │ Validate │ │ Preview Disc│ │ Apply & Compile  │ │  │ │
│  │  │ └─────────┘ └─────────────┘ └──────────────────┘ │  │ │
│  │  │                                                   │  │ │
│  │  │ Validation Results:                               │  │ │
│  │  │  ✅ Dimensional consistency: PASS                 │  │ │
│  │  │  ✅ Variable references: PASS                     │  │ │
│  │  │  ⚠️ Stability estimate: Check CFL condition       │  │ │
│  │  │  ✅ Discretization preview: Generated OK          │  │ │
│  │  └───────────────────────────────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

---

### 13.4 수학식 편집 규칙 (Equation Editing Rules)

#### 13.4.1 문법 규칙

```
수학식 문법 (GFD Math Notation — GMN)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

■ 기본 연산자
  +  -  *  /  ^             사칙연산 및 거듭제곱

■ 미분 연산자 (연속형 전용)
  d(φ)/dt                   시간 미분 (∂φ/∂t)
  grad(φ)                   구배 (∇φ)
  div(F)                    발산 (∇·F)
  laplacian(γ, φ)           라플라시안 (∇·(γ∇φ))
  curl(F)                   회전 (∇×F)
  d2(φ)/dx2                 2차 편미분

■ 텐서 연산
  dot(A, B)                 내적 (A·B)
  cross(A, B)               외적 (A×B)
  outer(A, B)               외적 텐서 (A⊗B)
  tr(T)                     대각합 (trace)
  det(T)                    행렬식
  inv(T)                    역행렬
  transpose(T)              전치
  symm(T)                   대칭부 = 0.5*(T + Tᵀ)
  skew(T)                   반대칭부 = 0.5*(T - Tᵀ)
  mag(V)                    크기 |V|
  magSqr(V)                 크기² |V|²

■ 수학 함수
  sin, cos, tan, asin, acos, atan, atan2
  exp, log, log10, sqrt, cbrt, abs
  pow(x, n), min(a, b), max(a, b)
  sign(x), heaviside(x), clamp(x, lo, hi)
  lerp(a, b, t)             선형 보간

■ 조건문
  if(condition, true_val, false_val)
  switch(var, case1, val1, case2, val2, ..., default_val)

■ 필드 변수 참조 ($ 접두사)
  $rho             밀도
  $u, $v, $w       속도 성분
  $U               속도 벡터
  $p               압력
  $T               온도
  $k               난류 운동에너지
  $epsilon          난류 소산율
  $omega           비소산율
  $mu              동점성계수
  $mu_t            난류 점성계수
  $nu_t            난류 동점성계수
  $cp              비열
  $lambda          열전도율
  $Y_i             화학종 질량분율 (i번째)
  $alpha_i         체적분율 (i번째 상)
  $sigma_ij        응력 텐서 성분
  $epsilon_ij      변형률 텐서 성분
  $S               변형률 텐서 크기 |S| = √(2·Sᵢⱼ·Sᵢⱼ)
  $Omega           회전율 텐서 크기

■ 기하학 참조
  $x, $y, $z       좌표
  $r, $theta       원통 좌표
  $V_cell          셀 체적
  $A_face          면 면적
  $n               면 법선벡터
  $d               셀 중심 간 거리
  $delta           벽면 거리 (y)

■ 시간 참조
  $t               현재 시간
  $dt              현재 타임스텝
  $iter            현재 반복 횟수

■ 상수 정의
  const 이름 = 값              # 사용자 상수
  const C_mu = 0.09
  const sigma_k = 0.85
```

#### 13.4.2 편집 제약 규칙 (Safety Rules)

| 규칙 | 설명 | 검증 방법 |
|------|------|----------|
| **R1: 차원 일치** | 등호 좌변과 우변의 물리 차원이 반드시 일치해야 함 | 자동 차원 분석 엔진 |
| **R2: 변수 존재** | 참조하는 모든 변수($xxx)가 현재 활성 모델에 존재해야 함 | 심볼 테이블 검사 |
| **R3: 텐서 랭크 일치** | 스칼라-벡터-텐서 연산의 랭크가 호환되어야 함 | 타입 체커 |
| **R4: 양의 정치성** | 확산 계수(γ)는 양수여야 함 → 행렬 대각 우세 보장 | 런타임 검사 + 경고 |
| **R5: 소스항 선형화** | 소스항 S는 S = Sᶜ + Sᵖ·φ 형태로 분해 필요 (Sᵖ ≤ 0 권장) | 자동 분해 시도 |
| **R6: 경계 호환** | 수정된 방정식이 기존 경계조건과 호환되는지 확인 | 경계 타입 매칭 |
| **R7: 보존성** | FVM 이산형의 경우 ΣFlux_faces = Source 형태 보존 | 플럭스 밸런스 검사 |
| **R8: CFL 안정성** | 명시적 시간 적분 사용 시 CFL 조건 추정 제공 | 최대 고유값 추정 |

#### 13.4.3 이산형 직접 입력 규칙 (Discrete Mode Rules)

```
이산형 입력 문법 (셀 중심 FVM 기준)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

■ 셀/면 인덱스
  _P               현재 셀 (P = Point)
  _N[i]            i번째 인접 셀 (Neighbor)
  _f[i]            i번째 면 (Face)

■ 계수 정의
  a_P  = ...       중심 셀 계수
  a_N[i] = ...     인접 셀 계수
  b_P  = ...       우변 소스 (RHS)

■ 면 플럭스
  F_f[i] = $rho_f[i] * dot($U_f[i], $n_f[i]) * $A_f[i]   # 대류 플럭스
  D_f[i] = $gamma_f[i] * $A_f[i] / $d_f[i]                # 확산 플럭스

■ 대류 스킴 함수 (내장)
  upwind(phi_P, phi_N, F)
  central(phi_P, phi_N)
  quick(phi_UU, phi_P, phi_N, F)
  tvd(phi_P, phi_N, r, limiter)    # limiter: vanLeer, minmod, superbee, ...

■ 시간 이산
  transient_euler(phi, phi_old, rho, V, dt)     # 1차 오일러
  transient_bdf2(phi, phi_old, phi_oldold, ...)  # 2차 BDF

■ 조립 최종 형태
  assemble(a_P, a_N[], b_P)        # 행렬에 계수 삽입
```

---

### 13.5 SDK 체계 — 전체 목록

GFD 솔버의 수학식 편집 및 확장을 위해 **10개의 SDK**를 제공한다.

```
GFD SDK 체계
├── SDK-1:  Expression SDK          수학식 파싱/편집/심볼릭 처리
├── SDK-2:  Discretization SDK      연속 PDE → 이산 대수 방정식 자동 변환
├── SDK-3:  Matrix Assembly SDK     이산 계수 → 희소 행렬 조립
├── SDK-4:  Linear Solver SDK       선형 시스템 풀이 커스터마이징
├── SDK-5:  Turbulence Model SDK    난류 모델 생성/수정
├── SDK-6:  Material Model SDK      재료 물성 모델 정의
├── SDK-7:  Boundary Condition SDK  경계 조건 커스텀 정의
├── SDK-8:  Source Term SDK         소스항 주입 및 수정
├── SDK-9:  Coupling SDK            멀티피직스 연성 커스텀
└── SDK-10: Post-Processing SDK     사용자 정의 후처리 필드 계산
```

---

#### SDK-1: Expression SDK (수학식 편집 엔진)

**목적:** 수학식을 문자열로 입력받아 파싱, 심볼릭 처리, 차원 검증, 코드 생성까지 수행

```
Expression SDK
│
├── Parser (파서)
│   ├── Tokenizer — GMN 문법을 토큰으로 분해
│   ├── AST Builder — Abstract Syntax Tree 생성
│   │   ├── BinaryOp (Add, Sub, Mul, Div, Pow)
│   │   ├── UnaryOp (Neg, Grad, Div, Laplacian, Curl)
│   │   ├── FunctionCall (sin, exp, max, ...)
│   │   ├── FieldRef ($rho, $U, $T, ...)
│   │   ├── Constant (숫자, const 정의)
│   │   ├── Conditional (if, switch)
│   │   └── TensorOp (dot, cross, outer, tr, ...)
│   └── Error Recovery — 구문 오류 시 유용한 에러 메시지
│
├── Symbolic Engine (심볼릭 엔진)
│   ├── Simplification — 수식 정리 (0*x → 0, 1*x → x, ...)
│   ├── Differentiation — 심볼릭 미분 (∂f/∂x 자동 계산)
│   ├── Substitution — 변수 치환
│   ├── Expansion — 수식 전개
│   └── Linearization — 소스항 선형화 (S = Sc + Sp·φ)
│
├── Dimensional Analysis (차원 분석)
│   ├── Unit System — SI 기본 (kg, m, s, K, mol, A, cd)
│   ├── Unit Inference — 변수로부터 차원 추론
│   ├── Unit Check — 등식 양변 차원 일치 검증
│   └── Unit Conversion — 자동 단위 변환
│
├── Validation (검증)
│   ├── Syntax Check — 문법 오류
│   ├── Semantic Check — 의미 오류 (존재하지 않는 변수 등)
│   ├── Dimension Check — 차원 불일치
│   ├── Type Check — 스칼라/벡터/텐서 랭크 불일치
│   ├── Stability Estimate — CFL/von Neumann 안정성 경고
│   └── Boundedness Check — 값의 물리적 범위 경고
│
├── Code Generation (코드 생성)
│   ├── To Rust — AST → 실행 가능한 Rust 코드 (JIT 또는 AOT)
│   ├── To LaTeX — AST → LaTeX 수식 (GUI 렌더링용)
│   ├── To MathML — AST → MathML (웹 GUI용)
│   └── To JSON — AST → 직렬화 (저장/전송)
│
└── API
    ├── parse(str) → Result<AST>
    ├── validate(ast) → Vec<Diagnostic>
    ├── simplify(ast) → AST
    ├── differentiate(ast, var) → AST
    ├── linearize_source(ast, phi) → (Sc, Sp)
    ├── check_dimensions(ast) → Result<Unit>
    ├── to_rust(ast) → String
    ├── to_latex(ast) → String
    └── to_json(ast) → String
```

**Rust crate:** `gfd-expression`

```rust
// 사용 예시
use gfd_expression::{parse, validate, to_latex, to_rust};

let expr = parse("d($rho * $k)/dt + div($rho * $U * $k)
    = laplacian(($mu + sigma_k * $mu_t), $k)
    + P_k_tilde + G_k - beta_star * $rho * $k * $omega")?;

let diagnostics = validate(&expr, &active_model_context)?;
// diagnostics: [Warning: G_k undefined — define as sub-expression]

let latex = to_latex(&expr);
// "\\frac{\\partial(\\rho k)}{\\partial t} + \\nabla\\cdot(\\rho \\mathbf{u} k) = ..."

let rust_code = to_rust(&expr, &discretization_context)?;
// 실행 가능한 Rust 코드 문자열
```

---

#### SDK-2: Discretization SDK (자동 이산화 엔진)

**목적:** 연속형 PDE(AST) → 이산 대수 방정식(계수 + 소스) 자동 변환

```
Discretization SDK
│
├── FVM Discretizer (유한체적법)
│   ├── Convection Term — div(ρuφ)
│   │   ├── Upwind (1차)
│   │   ├── Linear Upwind (2차)
│   │   ├── Central Differencing
│   │   ├── QUICK
│   │   ├── TVD with Limiters
│   │   │   ├── Van Leer
│   │   │   ├── Van Albada
│   │   │   ├── Minmod
│   │   │   ├── Superbee
│   │   │   ├── MUSCL
│   │   │   └── User-Defined Limiter (Expression)
│   │   └── Bounded Central (LES 용)
│   │
│   ├── Diffusion Term — laplacian(γ, φ)
│   │   ├── Central (2차, 직교 메시)
│   │   ├── Non-Orthogonal Correction
│   │   │   ├── Minimum Correction
│   │   │   ├── Orthogonal Correction
│   │   │   ├── Over-Relaxed Correction
│   │   │   └── Limited (0~1 blend)
│   │   └── Deferred Correction
│   │
│   ├── Temporal Term — d(ρφ)/dt
│   │   ├── Euler Implicit (1차)
│   │   ├── BDF2 (2차)
│   │   ├── Crank-Nicolson (2차)
│   │   ├── Euler Explicit (조건부 안정)
│   │   └── RK4 (4차 명시적)
│   │
│   ├── Source Term — S(φ)
│   │   ├── Auto-Linearization — S = Sc + Sp·φ 자동 분해
│   │   ├── Explicit Source (우변에만)
│   │   └── Implicit Source (행렬 대각에 반영)
│   │
│   ├── Gradient Computation
│   │   ├── Green-Gauss Cell-Based
│   │   ├── Green-Gauss Node-Based
│   │   └── Least Squares
│   │
│   └── Face Interpolation
│       ├── Linear (중심차분)
│       ├── Upwind-Biased
│       └── Harmonic Mean (확산 계수)
│
├── FEM Discretizer (유한요소법)
│   ├── Weak Form Generator
│   │   ├── PDE → Weak Form 자동 변환
│   │   │   ├── 시행함수 곱하기 (test function)
│   │   │   ├── 부분 적분 (Green's theorem)
│   │   │   └── 경계 항 추출
│   │   └── 사용자 직접 Weak Form 입력
│   │
│   ├── Element Types
│   │   ├── Lagrange (P1, P2, P3, ...)
│   │   ├── Serendipity (Q8, Q20)
│   │   ├── Hermite
│   │   └── Nedelec / Raviart-Thomas (벡터 요소)
│   │
│   ├── Quadrature (수치 적분)
│   │   ├── Gauss-Legendre
│   │   ├── Gauss-Lobatto
│   │   └── Custom Quadrature Points
│   │
│   ├── Shape Function Library
│   │   ├── 1D: Line2, Line3
│   │   ├── 2D: Tri3, Tri6, Quad4, Quad8, Quad9
│   │   └── 3D: Tet4, Tet10, Hex8, Hex20, Hex27, Wedge6, Pyramid5
│   │
│   └── Assembly Strategy
│       ├── Element-by-Element
│       ├── Edge-Based
│       └── Cell-Based
│
├── DG Discretizer (불연속 갈레르킨, 향후)
│   ├── Numerical Flux (Lax-Friedrichs, Roe, HLLC, ...)
│   ├── Polynomial Basis (Legendre, Nodal)
│   └── Slope Limiter (TVB, WENO, ...)
│
├── Discretization Pipeline
│   ├── Input: AST (연속형 수식)
│   ├── Step 1: 항 분류 (시간항, 대류항, 확산항, 소스항)
│   ├── Step 2: 각 항별 이산화 스킴 적용
│   ├── Step 3: 계수 생성 (a_P, a_N, b)
│   ├── Step 4: 소스항 선형화
│   ├── Step 5: 경계조건 반영
│   └── Output: DiscreteEquation {coefficients, source, bc_modifications}
│
└── API
    ├── discretize_fvm(ast, mesh, schemes) → DiscreteEquation
    ├── discretize_fem(ast, mesh, element, quadrature) → DiscreteEquation
    ├── classify_terms(ast) → TermClassification
    ├── preview_stencil(ast, mesh) → StencilVisualization
    ├── estimate_stability(ast, mesh, dt) → StabilityReport
    └── explain_discretization(ast) → Vec<Step>  # 이산화 과정 설명
```

**Rust crate:** `gfd-discretize`

```rust
use gfd_discretize::{FvmDiscretizer, Schemes, discretize_fvm};

let schemes = Schemes {
    convection: ConvectionScheme::SecondOrderUpwind,
    diffusion: DiffusionScheme::CentralWithCorrection { limit: 0.5 },
    temporal: TemporalScheme::BDF2,
    gradient: GradientMethod::LeastSquares,
};

// 연속형 AST → 이산 방정식
let discrete_eq = discretize_fvm(&ast_k_equation, &mesh, &schemes)?;

// 결과: 각 셀에 대한 a_P, a_N[], b_P 계수
// 이후 Matrix Assembly SDK로 전달
```

---

#### SDK-3: Matrix Assembly SDK (행렬 조립 엔진)

**목적:** 이산 계수(a_P, a_N, b) → 글로벌 희소 행렬 A·x = b 조립

```
Matrix Assembly SDK
│
├── Sparse Matrix Formats
│   ├── CSR (Compressed Sparse Row) — 기본
│   ├── CSC (Compressed Sparse Column)
│   ├── COO (Coordinate) — 조립용
│   ├── BSR (Block Sparse Row) — 연성 시스템
│   └── Conversion 함수들
│
├── Assembly Engine
│   ├── add_coefficient(row, col, value) — 개별 계수 추가
│   ├── add_cell_equation(cell_id, a_P, a_N_pairs, b) — 셀 단위
│   ├── add_element_matrix(elem_id, Ke, fe) — FEM 요소 단위
│   ├── apply_boundary_condition(bc_type, face, value)
│   │   ├── Dirichlet — 행 수정 (a_P = 1, a_N = 0, b = value)
│   │   ├── Neumann — 소스 수정 (b += flux * A_face)
│   │   ├── Robin — 계수 수정 (a_P += h*A, b += h*T_inf*A)
│   │   └── Periodic — 인접 셀 연결
│   └── finalize() → LinearSystem { A, x, b }
│
├── Block Assembly (연성 시스템)
│   ├── 속도-압력 블록 (Navier-Stokes)
│   │   ├── [A_uu  A_up] [u]   [b_u]
│   │   │   [A_pu  A_pp] [p] = [b_p]
│   │   └── 블록 전처리기 (Schur complement 등)
│   ├── 멀티피직스 블록
│   │   └── 유체-열-구조 통합 행렬
│   └── Custom Block Layout
│
├── Modification API (사용자 수정)
│   ├── inspect_row(row) → (indices, values)
│   ├── modify_diagonal(row, new_value)
│   ├── modify_coefficient(row, col, new_value)
│   ├── add_to_source(row, value)
│   ├── insert_equation(row, coefficients, source)
│   └── remove_equation(row) — 특정 행 비활성화
│
├── Diagnostics (진단)
│   ├── check_diagonal_dominance() → Report
│   ├── check_symmetry() → bool
│   ├── compute_condition_number() → f64
│   ├── find_zero_pivots() → Vec<usize>
│   └── matrix_sparsity_pattern() → SparsityVisualization
│
└── API
    ├── new_assembler(mesh, dof_per_cell) → Assembler
    ├── assemble(discrete_equations) → LinearSystem
    ├── modify(system, modifications) → LinearSystem
    ├── diagnose(system) → DiagnosticReport
    └── export_matrix(system, format) → File  # MatrixMarket, PETSc, etc.
```

**Rust crate:** `gfd-matrix`

---

#### SDK-4: Linear Solver SDK (선형 솔버 커스터마이징)

**목적:** A·x = b 풀이 전략 선택 및 커스텀 전처리기/솔버 구현

```
Linear Solver SDK
│
├── Iterative Solvers
│   ├── CG (Conjugate Gradient) — 대칭 양정치
│   ├── BiCGSTAB — 비대칭 범용
│   ├── GMRES(m) — 비대칭, 재시작 파라미터 m
│   ├── FGMRES — 가변 전처리 GMRES
│   ├── TFQMR
│   ├── GCR
│   └── User-Defined Solver (trait 구현)
│
├── Direct Solvers
│   ├── LU (via faer / SuperLU FFI)
│   ├── Cholesky (대칭 양정치)
│   └── QR
│
├── Preconditioners
│   ├── Jacobi (대각)
│   ├── Gauss-Seidel / SOR
│   ├── ILU(k) — 불완전 LU 분해 (fill-in level k)
│   ├── ILUT — threshold 기반 ILU
│   ├── AMG (Algebraic Multigrid)
│   │   ├── Coarsening Strategy (Ruge-Stüben, PMIS, HMIS)
│   │   ├── Interpolation (Classical, Extended+i, ...)
│   │   ├── Smoother (Gauss-Seidel, Chebyshev, ...)
│   │   ├── Cycle Type (V, W, F)
│   │   └── Levels
│   ├── Block Preconditioners (연성 시스템)
│   │   ├── Block Jacobi
│   │   ├── Block Gauss-Seidel
│   │   └── Schur Complement
│   └── User-Defined Preconditioner (trait 구현)
│
├── Solver Control
│   ├── Tolerance (absolute, relative)
│   ├── Max Iterations
│   ├── Verbosity Level
│   ├── Convergence Monitor callback
│   └── Divergence Action (Stop, Reduce dt, Switch solver)
│
└── Extension API (trait)
    pub trait CustomLinearSolver {
        fn solve(&mut self, A: &SparseMatrix, b: &Vector, x: &mut Vector)
            -> Result<SolverStats>;
        fn name(&self) -> &str;
    }

    pub trait CustomPreconditioner {
        fn setup(&mut self, A: &SparseMatrix) -> Result<()>;
        fn apply(&self, r: &Vector, z: &mut Vector) -> Result<()>;
        fn name(&self) -> &str;
    }
```

**Rust crate:** `gfd-linalg`

---

#### SDK-5: Turbulence Model SDK (난류 모델 생성/수정)

**목적:** 기본 난류 모델을 수정하거나, 완전히 새로운 난류 모델을 정의

```
Turbulence Model SDK
│
├── Model Template (모델 정의 구조)
│   ├── Model Name / ID
│   ├── Number of Transport Equations (0, 1, 2, ...)
│   ├── Transport Equations[]
│   │   ├── Variable Name (k, epsilon, omega, nuTilda, ...)
│   │   ├── Equation (연속형 PDE 또는 이산형)
│   │   ├── Diffusion Coefficient Expression
│   │   ├── Source Term (Production) Expression
│   │   ├── Source Term (Destruction) Expression
│   │   └── Boundary Conditions (wall, inlet, outlet 기본값)
│   ├── Eddy Viscosity Definition (μₜ = f(transported vars))
│   ├── Model Constants{}
│   │   ├── Name
│   │   ├── Default Value
│   │   ├── Description
│   │   └── Valid Range [min, max]
│   ├── Auxiliary Relations (blending functions, limiters, ...)
│   ├── Wall Treatment
│   │   ├── Wall Function (Standard, Scalable, Enhanced)
│   │   └── Low-Re formulation
│   └── Realizability Constraints
│
├── Pre-built Models (수정 가능)
│   ├── Spalart-Allmaras
│   ├── k-epsilon Standard
│   ├── k-epsilon RNG
│   ├── k-epsilon Realizable
│   ├── k-omega Standard
│   ├── k-omega BSL
│   ├── k-omega SST
│   ├── RSM Linear Pressure-Strain
│   ├── LES Smagorinsky
│   ├── LES Dynamic Smagorinsky
│   ├── LES WALE
│   └── (각 모델은 위 Template 형식으로 내부 정의)
│
├── Modification Operations
│   ├── modify_equation(model, eq_name, new_ast)
│   ├── modify_constant(model, const_name, new_value)
│   ├── add_source_term(model, eq_name, source_ast)
│   ├── remove_source_term(model, eq_name, term_id)
│   ├── modify_eddy_viscosity(model, new_ast)
│   ├── add_auxiliary_relation(model, name, ast)
│   └── derive_from(base_model) → new_model  # 기존 모델 상속
│
├── Validation
│   ├── Dimensional consistency
│   ├── Realizability (μₜ ≥ 0, k ≥ 0, ε > 0, ω > 0)
│   ├── Wall asymptotic behavior (y → 0 극한)
│   ├── Free-stream behavior (y → ∞ 극한)
│   └── Known benchmark comparison suggestion
│
└── JSON 정의 예시 (사용자 정의 모델)
```

```json
{
  "turbulence_model": {
    "name": "k-omega-SST-buoyancy",
    "base": "k_omega_sst",
    "modifications": [
      {
        "target": "k_equation",
        "action": "add_source",
        "expression": "-$beta_buoy * ($mu_t / $Pr_t) * dot($gravity, grad($rho))",
        "sub_expressions": {
          "beta_buoy": "1.0",
          "Pr_t": "0.85"
        }
      },
      {
        "target": "constants",
        "action": "modify",
        "name": "beta_star",
        "value": 0.09
      }
    ]
  }
}
```

**Rust crate:** `gfd-turbulence` (gfd-expression 의존)

---

#### SDK-6: Material Model SDK (재료 물성 모델)

**목적:** 온도/압력/변형률 등에 따른 비선형 물성치 정의

```
Material Model SDK
│
├── Property Types
│   ├── Scalar Property (밀도, 점성, 비열, 열전도율, ...)
│   ├── Tensor Property (이방성 열전도, 탄성 텐서, ...)
│   └── State-Dependent Property (상변화, 이력 의존, ...)
│
├── Definition Methods
│   ├── Constant Value
│   ├── Expression (Expression SDK 사용)
│   │   └── 예: density = "101325 * 0.029 / (8.314 * $T)"  # 이상기체
│   ├── Polynomial (계수 배열)
│   │   └── 예: cp(T) = a₀ + a₁T + a₂T² + a₃T³
│   ├── Piecewise Linear (테이블)
│   │   └── 예: mu = [(273, 1.72e-5), (373, 2.17e-5), ...]
│   ├── Sutherland (3계수)
│   │   └── μ = μ_ref * (T/T_ref)^(3/2) * (T_ref+S)/(T+S)
│   ├── Non-Newtonian Models
│   │   ├── Power Law: μ = K * γ̇^(n-1)
│   │   ├── Carreau: μ = μ_inf + (μ_0 - μ_inf) * [1 + (λγ̇)²]^((n-1)/2)
│   │   ├── Cross: μ = μ_inf + (μ_0 - μ_inf) / [1 + (λγ̇)^m]
│   │   ├── Herschel-Bulkley: τ = τ_y + K * γ̇^n
│   │   └── User-Defined (Expression)
│   ├── Elasticity Models
│   │   ├── Linear Isotropic (E, ν)
│   │   ├── Linear Orthotropic (E₁,E₂,E₃, ν₁₂,ν₁₃,ν₂₃, G₁₂,G₁₃,G₂₃)
│   │   ├── Hyperelastic (Neo-Hookean, Mooney-Rivlin, Ogden, ...)
│   │   └── User-Defined C_ijkl Tensor (Expression)
│   └── Plasticity Models
│       ├── Yield Function (Expression)
│       ├── Hardening Law (Isotropic, Kinematic, Mixed)
│       │   └── User-Defined (Expression)
│       └── Flow Rule (Associated, Non-Associated)
│
├── Thermodynamic Database
│   ├── NASA 7/9-coefficient polynomials (cp, h, s)
│   ├── JANAF tables
│   └── Custom lookup tables
│
└── API
    pub trait MaterialProperty {
        fn evaluate(&self, state: &MaterialState) -> Result<PropertyValue>;
        fn derivative(&self, state: &MaterialState, wrt: StateVar) -> Result<PropertyValue>;
        fn units(&self) -> Unit;
    }

    pub trait ConstitutiveModel {
        fn stress(&self, strain: &Tensor, state: &MaterialState) -> Result<Tensor>;
        fn tangent(&self, strain: &Tensor, state: &MaterialState) -> Result<Tensor4>;
    }
```

**Rust crate:** `gfd-material`

---

#### SDK-7: Boundary Condition SDK (경계 조건 커스텀)

**목적:** 표준 경계조건 외에 사용자 정의 경계조건 생성

```
Boundary Condition SDK
│
├── Standard BC Types (내장, 수정 가능)
│   ├── Dirichlet (Fixed Value)
│   │   └── value = constant | expression | profile
│   ├── Neumann (Fixed Gradient / Flux)
│   │   └── flux = constant | expression
│   ├── Robin (Mixed)
│   │   └── a·φ + b·∂φ/∂n = c
│   ├── Convective (열전달 h)
│   │   └── q = h·(T - T_inf)
│   ├── Radiative
│   │   └── q = ε·σ·(T⁴ - T_env⁴)
│   └── Periodic / Symmetry / Axis
│
├── Custom BC Definition
│   ├── Target Variable (pressure, velocity, temperature, ...)
│   ├── Face Value Expression — φ_face = f($x, $y, $z, $t, ...)
│   ├── Face Gradient Expression — ∂φ/∂n|_face = g(...)
│   ├── Coefficient Modification — a_P, b 수정 규칙
│   ├── Flux Expression — F_face = custom(...)
│   └── Multi-Variable Coupled BC
│       └── 예: 벽면에서 온도와 농도를 동시에 제어
│
├── Time-Dependent BC
│   ├── Expression with $t
│   ├── Table (time, value) 보간
│   └── Waveform (sin, square, ramp, custom)
│
├── Inlet Profile Generation
│   ├── Synthetic Turbulence Generator
│   │   ├── Vortex Method
│   │   ├── Digital Filter Method
│   │   └── Random Flow Generation (RFG)
│   ├── Mapped Profile (다른 해석 결과 참조)
│   └── Analytic Profile (Poiseuille, 1/7-power law, ...)
│
└── API
    pub trait CustomBoundaryCondition {
        fn apply_coefficients(
            &self,
            face: &Face,
            cell: &Cell,
            field: &Field,
            a_p: &mut f64,
            b: &mut f64,
            state: &SolverState
        ) -> Result<()>;

        fn face_value(
            &self,
            face: &Face,
            state: &SolverState
        ) -> Result<f64>;

        fn description(&self) -> &str;
    }
```

**Rust crate:** `gfd-boundary`

---

#### SDK-8: Source Term SDK (소스항 주입)

**목적:** 임의의 방정식에 사용자 정의 소스항 추가/수정

```
Source Term SDK
│
├── Source Term Definition
│   ├── Target Equation (momentum_x, energy, k, omega, ...)
│   ├── Zone Selection (전체 / 특정 셀 존 / 조건부)
│   │   └── Condition: Expression → bool
│   │       예: "if($x > 0.5 && $x < 1.0, true, false)"
│   ├── Explicit Source (Sc) — 우변에 추가
│   │   └── Expression (W/m³, N/m³, kg/m³·s, ...)
│   ├── Implicit Source (Sp) — φ에 비례하는 부분
│   │   └── Expression (Sp ≤ 0 이면 대각 우세 강화)
│   └── Linearization Strategy
│       ├── Auto (Expression SDK가 자동 분해)
│       ├── Manual (사용자 직접 Sc, Sp 지정)
│       └── Full Explicit (Sp = 0, 전부 Sc)
│
├── 사전 정의 소스
│   ├── Volume Heat Source (Q, W/m³)
│   ├── Momentum Source (Body Force, N/m³)
│   ├── Mass Source (kg/m³·s)
│   ├── MHD Lorentz Force (J × B)
│   ├── Darcy/Forchheimer (다공성 매체)
│   ├── Buoyancy (Boussinesq / Full)
│   └── Coriolis Force (회전계)
│
└── API
    pub trait SourceTerm {
        fn compute(
            &self,
            cell: &Cell,
            state: &SolverState,
        ) -> Result<(f64, f64)>;  // (Sc, Sp)

        fn target_equation(&self) -> EquationId;
        fn zone_filter(&self) -> Option<&ZoneFilter>;
    }
```

**Rust crate:** `gfd-source`

---

#### SDK-9: Coupling SDK (멀티피직스 연성)

**목적:** 물리 모듈 간 데이터 교환 및 커플링 전략 커스터마이징

```
Coupling SDK
│
├── Interface Definition
│   ├── Shared Surface (유체-고체 경계면)
│   ├── Volume Overlap (같은 메시 공유)
│   └── Mapped Interface (다른 메시 간 보간)
│
├── Transfer Operations
│   ├── Conservative Mapping (힘, 열유속 — 적분량 보존)
│   ├── Consistent Mapping (온도, 변위 — 값 보간)
│   ├── Interpolation Methods
│   │   ├── Nearest Neighbor
│   │   ├── Linear (Barycentric)
│   │   ├── Radial Basis Function (RBF)
│   │   └── Mortar Method (FEM)
│   └── User-Defined Mapping (Expression)
│
├── Coupling Strategy
│   ├── One-Way (단방향)
│   ├── Two-Way Partitioned (반복)
│   │   ├── Fixed-Point (Gauss-Seidel)
│   │   ├── Aitken Relaxation (동적 완화)
│   │   ├── IQN-ILS (Quasi-Newton)
│   │   └── Anderson Acceleration
│   ├── Monolithic (단일 행렬)
│   └── User-Defined Strategy
│
├── Custom Coupling Expression
│   └── 예: 유체→구조 전달시 압력에 보정 팩터 적용
│       "force_on_solid = ($p + 0.5*$rho*magSqr($U)) * $n * $A_face * $correction_factor"
│
└── API
    pub trait CouplingInterface {
        fn transfer(
            &self,
            from: &SolverState,
            to: &mut SolverState,
            mapping: &Mapping,
        ) -> Result<()>;

        fn check_convergence(
            &self,
            current: &SolverState,
            previous: &SolverState,
        ) -> Result<f64>;  // coupling residual
    }
```

**Rust crate:** `gfd-coupling`

---

#### SDK-10: Post-Processing SDK (사용자 정의 후처리)

**목적:** 결과 필드로부터 사용자 정의 파생 변수 계산

```
Post-Processing SDK
│
├── Derived Field Definition
│   ├── Name
│   ├── Expression (Expression SDK 사용)
│   │   └── 예: "q_criterion = 0.5*(magSqr($Omega) - magSqr($S))"
│   │   └── 예: "entropy_gen = $mu/$T * (2*magSqr(symm(grad($U))))"
│   ├── Units
│   └── Compute Scope (전체 / 특정 영역)
│
├── Integral Operations (Expression 기반)
│   ├── Surface Integral: "integrate(expr, surface)"
│   ├── Volume Integral: "integrate(expr, zone)"
│   ├── Line Integral: "integrate(expr, line)"
│   └── Time Average: "time_avg(expr, t_start, t_end)"
│
├── Statistical Operations
│   ├── Mean / RMS / Std-Dev (공간 또는 시간)
│   ├── PDF (확률 밀도 함수)
│   ├── Power Spectrum (FFT)
│   ├── Two-Point Correlation
│   └── Structure Function
│
└── API
    pub trait DerivedField {
        fn compute(&self, state: &SolverState) -> Result<Field>;
        fn name(&self) -> &str;
        fn units(&self) -> Unit;
    }
```

**Rust crate:** `gfd-postprocess`

---

### 13.6 SDK 간 의존성 및 데이터 흐름

```
                    사용자 입력 (GUI / JSON)
                           │
                           ▼
              ┌──── Expression SDK (SDK-1) ────┐
              │   파싱 → AST → 검증 → 코드생성    │
              └──────────┬─────────────────────┘
                         │ AST
           ┌─────────────┼──────────────────┐
           ▼             ▼                  ▼
    ┌─────────────┐ ┌──────────┐  ┌──────────────┐
    │ Turbulence  │ │ Material │  │ Boundary     │
    │ Model SDK   │ │ Model SDK│  │ Condition SDK│
    │ (SDK-5)     │ │ (SDK-6)  │  │ (SDK-7)      │
    └──────┬──────┘ └────┬─────┘  └──────┬───────┘
           │             │               │
           └──────┬──────┘               │
                  ▼                      │
    ┌────────────────────────┐           │
    │ Source Term SDK (SDK-8)│           │
    └───────────┬────────────┘           │
                │                        │
                ▼                        │
    ┌──────────────────────────┐         │
    │ Discretization SDK (SDK-2)│◄───────┘
    │ 연속형 PDE → 이산 계수     │
    └───────────┬──────────────┘
                │ a_P, a_N[], b
                ▼
    ┌──────────────────────────┐
    │ Matrix Assembly SDK (SDK-3)│
    │ 계수 → 글로벌 Ax=b         │
    └───────────┬──────────────┘
                │ A, x, b
                ▼
    ┌──────────────────────────┐
    │ Linear Solver SDK (SDK-4) │
    │ 선형 시스템 풀이            │
    └───────────┬──────────────┘
                │ 해 (φ 필드)
        ┌───────┼───────┐
        ▼       ▼       ▼
    ┌────────┐ ┌──────┐ ┌──────────────┐
    │Coupling│ │다음   │ │Post-Process  │
    │SDK     │ │반복   │ │SDK (SDK-10)  │
    │(SDK-9) │ │      │ │              │
    └────────┘ └──────┘ └──────────────┘
```

### 13.7 SDK Rust 크레이트 구조 및 의존성

```
gfd/crates/
├── gfd-expression/       # SDK-1: 수학식 파서/심볼릭 엔진
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── tokenizer.rs       # 토큰 분해
│       ├── parser.rs          # AST 빌더
│       ├── ast.rs             # AST 노드 정의
│       ├── simplify.rs        # 수식 정리
│       ├── differentiate.rs   # 심볼릭 미분
│       ├── linearize.rs       # 소스항 선형화
│       ├── dimension.rs       # 차원 분석
│       ├── validate.rs        # 종합 검증
│       ├── codegen_rust.rs    # Rust 코드 생성
│       ├── codegen_latex.rs   # LaTeX 출력
│       └── codegen_json.rs    # JSON 직렬화
│
├── gfd-discretize/       # SDK-2: 이산화 엔진
│   ├── Cargo.toml        # 의존: gfd-expression, gfd-core
│   └── src/
│       ├── lib.rs
│       ├── fvm/
│       │   ├── convection.rs   # 대류항 이산화
│       │   ├── diffusion.rs    # 확산항 이산화
│       │   ├── temporal.rs     # 시간항 이산화
│       │   ├── source.rs       # 소스항 선형화
│       │   ├── gradient.rs     # 구배 계산
│       │   └── interpolation.rs # 면 보간
│       ├── fem/
│       │   ├── weak_form.rs    # 약형식 변환
│       │   ├── shape_fn.rs     # 형상 함수
│       │   ├── quadrature.rs   # 수치 적분
│       │   └── assembly.rs     # 요소 조립
│       └── pipeline.rs         # 파이프라인 오케스트레이션
│
├── gfd-matrix/           # SDK-3: 행렬 조립
│   ├── Cargo.toml        # 의존: gfd-core
│   └── src/
│       ├── lib.rs
│       ├── sparse.rs          # CSR/CSC/COO
│       ├── assembler.rs       # 셀/요소 → 글로벌 조립
│       ├── block.rs           # 블록 행렬
│       ├── boundary.rs        # BC 반영
│       ├── modify.rs          # 사용자 수정 API
│       └── diagnostics.rs     # 대각 우세, 조건수, etc.
│
├── gfd-linalg/           # SDK-4: 선형 솔버
│   ├── Cargo.toml        # 의존: gfd-matrix, faer
│   └── src/
│       ├── lib.rs
│       ├── iterative/
│       │   ├── cg.rs
│       │   ├── bicgstab.rs
│       │   ├── gmres.rs
│       │   └── fgmres.rs
│       ├── direct/
│       │   ├── lu.rs
│       │   └── cholesky.rs
│       ├── preconditioner/
│       │   ├── jacobi.rs
│       │   ├── ilu.rs
│       │   ├── amg.rs
│       │   └── block.rs
│       └── traits.rs          # CustomLinearSolver, CustomPreconditioner
│
├── gfd-turbulence/       # SDK-5: 난류 모델
│   ├── Cargo.toml        # 의존: gfd-expression, gfd-discretize
│   └── src/
│       ├── lib.rs
│       ├── model_template.rs  # 모델 정의 구조
│       ├── builtin/           # 내장 모델들
│       │   ├── spalart_allmaras.rs
│       │   ├── k_epsilon.rs
│       │   ├── k_omega_sst.rs
│       │   ├── rsm.rs
│       │   └── les/
│       ├── custom.rs          # 사용자 정의 모델 로더
│       ├── wall_functions.rs
│       └── validation.rs      # 모델 검증
│
├── gfd-material/         # SDK-6: 재료 모델
│   ├── Cargo.toml        # 의존: gfd-expression
│   └── src/
│       ├── lib.rs
│       ├── fluid/
│       │   ├── newtonian.rs
│       │   ├── non_newtonian.rs
│       │   └── ideal_gas.rs
│       ├── solid/
│       │   ├── elastic.rs
│       │   ├── hyperelastic.rs
│       │   └── plasticity.rs
│       ├── thermal.rs
│       ├── database.rs        # NASA poly, JANAF
│       └── traits.rs
│
├── gfd-boundary/         # SDK-7: 경계 조건
│   ├── Cargo.toml        # 의존: gfd-expression, gfd-core
│   └── src/
│       ├── lib.rs
│       ├── standard/          # Dirichlet, Neumann, Robin, ...
│       ├── custom.rs          # 사용자 정의 BC
│       ├── profiles.rs        # 시간/공간 프로파일
│       ├── synthetic_turbulence.rs  # 합성 난류 생성
│       └── traits.rs
│
├── gfd-source/           # SDK-8: 소스항
│   ├── Cargo.toml        # 의존: gfd-expression
│   └── src/
│       ├── lib.rs
│       ├── volume_source.rs
│       ├── momentum_source.rs
│       ├── porous.rs
│       ├── buoyancy.rs
│       └── traits.rs
│
├── gfd-coupling/         # SDK-9: 멀티피직스 커플링
│   ├── Cargo.toml        # 의존: gfd-core, gfd-expression
│   └── src/
│       ├── lib.rs
│       ├── interface.rs       # 인터페이스 정의
│       ├── mapping/           # 보간 방법
│       │   ├── nearest.rs
│       │   ├── rbf.rs
│       │   └── mortar.rs
│       ├── strategy/          # 커플링 전략
│       │   ├── fixed_point.rs
│       │   ├── aitken.rs
│       │   ├── iqn_ils.rs
│       │   └── anderson.rs
│       └── traits.rs
│
└── gfd-postprocess/      # SDK-10: 후처리
    ├── Cargo.toml         # 의존: gfd-expression, gfd-core
    └── src/
        ├── lib.rs
        ├── derived_field.rs   # 파생 필드 계산
        ├── integrals.rs       # 적분 연산
        ├── statistics.rs      # 통계 (평균, RMS, FFT, ...)
        └── traits.rs
```

---

### 13.8 GUI 수학식 편집기 상세 설계

#### 13.8.1 편집기 구성 요소

```
Equation Editor GUI
│
├── Equation List Panel (좌측)
│   ├── 현재 모델의 모든 방정식 나열
│   ├── 각 방정식 옆에 상태 아이콘
│   │   ├── 🔵 Default (기본값)
│   │   ├── 🟡 Modified (수정됨)
│   │   └── 🔴 Error (오류)
│   ├── [+ Add Equation] 버튼
│   └── [Reset All to Default] 버튼
│
├── Editor Panel (중앙)
│   ├── Mode Toggle: [Continuous PDE] / [Discrete Form]
│   │
│   ├── Continuous Mode:
│   │   ├── Math Input Area
│   │   │   ├── 텍스트 입력 (GMN 문법)
│   │   │   ├── 실시간 LaTeX 렌더링 (아래쪽)
│   │   │   └── 자동완성 (변수명, 함수명)
│   │   ├── Sub-Expression Panel
│   │   │   ├── 보조 수식 정의 영역
│   │   │   └── 상수 정의 테이블
│   │   └── [Preview Discretization] 버튼
│   │       └── 이산화 결과 미리보기 팝업
│   │
│   └── Discrete Mode:
│       ├── Coefficient Template
│       │   ├── a_P = [입력]
│       │   ├── a_N[i] = [입력]
│       │   ├── b_P = [입력]
│       │   └── Stencil 시각화
│       ├── Face Flux Definition
│       │   ├── Convection Flux = [입력]
│       │   └── Diffusion Flux = [입력]
│       └── Source Term
│           ├── Sc = [입력]
│           └── Sp = [입력]
│
├── Validation Panel (우측)
│   ├── 실시간 검증 결과
│   │   ├── ✅/❌ Syntax
│   │   ├── ✅/❌ Dimensions
│   │   ├── ✅/❌ Variable References
│   │   ├── ✅/❌ Tensor Ranks
│   │   ├── ✅/⚠️ Stability
│   │   └── ✅/⚠️ Boundedness
│   ├── Error Messages (클릭 → 해당 위치 이동)
│   └── Suggestions (자동 수정 제안)
│
├── Toolbar
│   ├── [Validate] — 전체 검증 실행
│   ├── [Preview Disc.] — 이산화 미리보기
│   ├── [Compare Default] — 기본값과 비교 (diff)
│   ├── [Export JSON] — 수정 사항 JSON 저장
│   ├── [Import JSON] — 이전 수정 불러오기
│   ├── [Apply & Compile] — 적용 및 솔버 재컴파일
│   └── [Reset to Default] — 기본 수식으로 복원
│
└── Bottom Status Bar
    ├── "Ready" / "Compiling..." / "Error in k-equation"
    └── 메모리 사용량 / 예상 추가 계산 비용
```

#### 13.8.2 수식 편집 단축키

| 단축키 | 기능 |
|--------|------|
| `Ctrl+Space` | 자동완성 (변수, 함수) |
| `Ctrl+D` | 차원 검사 실행 |
| `Ctrl+Shift+P` | 이산화 미리보기 |
| `Ctrl+Enter` | 적용 & 컴파일 |
| `Ctrl+Z` / `Ctrl+Y` | 실행 취소 / 다시 실행 |
| `Ctrl+/` | 주석 토글 (# 주석) |
| `F2` | 선택 심볼 이름 변경 |
| `F5` | 전체 검증 |
| `Ctrl+B` | 기본값과 비교 보기 |

---

### 13.9 수학식 컴파일 및 실행 전략

```
사용자 수식 편집 → 적용 → 실행 경로:

[전략 1: JIT 컴파일 (기본)]
  1. Expression SDK가 AST 생성
  2. AST → Rust 코드 문자열 생성
  3. cranelift JIT 또는 자체 바이트코드 인터프리터로 실행
  장점: 빠른 적용, 재시작 불필요
  단점: 최적화 제한적

[전략 2: AOT 재컴파일]
  1. Expression SDK가 AST 생성
  2. AST → .rs 파일 생성 (gfd-custom-equations/src/)
  3. cargo build --release 재컴파일
  4. 솔버 재시작
  장점: 최대 성능 (LLVM 최적화)
  단점: 컴파일 시간 (수십 초)

[전략 3: 바이트코드 인터프리터 (경량)]
  1. Expression SDK가 AST 생성
  2. AST → 스택 기반 바이트코드로 변환
  3. 자체 VM에서 실행 (셀 단위 루프)
  장점: 즉시 적용, 크로스 플랫폼
  단점: JIT 대비 2~5x 느림

[권장] 개발 중: 전략 3 (빠른 반복)
       프로덕션: 전략 1 (JIT) 또는 전략 2 (AOT)
```

---

### 13.10 JSON에서의 사용자 정의 수식 저장 형태

```json
{
  "setup": {
    "models": {
      "viscous": {
        "model": "k_omega_sst",
        "custom_equations": {
          "k_equation": {
            "mode": "continuous",
            "equation": "d($rho * $k)/dt + div($rho * $U * $k) = laplacian(($mu + sigma_k * $mu_t), $k) + P_k_tilde + G_k - beta_star * $rho * $k * $omega",
            "sub_expressions": {
              "P_k_tilde": "min($mu_t * $S^2, 10 * beta_star * $rho * $k * $omega)",
              "G_k": "-beta_buoy * ($mu_t / Pr_t) * dot($gravity, grad($rho))"
            },
            "constants": {
              "sigma_k": { "value": 0.85, "description": "k 확산 계수" },
              "beta_star": { "value": 0.09, "description": "k 소산 계수" },
              "beta_buoy": { "value": 1.0, "description": "부력 생산 계수" },
              "Pr_t": { "value": 0.85, "description": "난류 Prandtl 수" }
            },
            "discretization_override": null
          },
          "omega_equation": null,
          "eddy_viscosity": null
        }
      }
    },
    "custom_source_terms": [
      {
        "target_equation": "energy",
        "zone": "heater_zone",
        "expression": "1e6 * sin(2 * $pi * $t / 0.1)",
        "type": "explicit",
        "units": "W/m^3"
      }
    ],
    "custom_boundary_conditions": [
      {
        "name": "pulsating_inlet",
        "patch": "inlet",
        "variable": "velocity",
        "expression": "1.5 * (1 + 0.1 * sin(2 * $pi * 100 * $t)) * (1 - ($r/0.05)^2)",
        "units": "m/s"
      }
    ],
    "custom_material_properties": [
      {
        "material": "blood",
        "property": "viscosity",
        "model": "carreau",
        "expression": "mu_inf + (mu_0 - mu_inf) * pow(1 + pow(lambda * $shear_rate, 2), (n-1)/2)",
        "constants": {
          "mu_inf": 0.00345,
          "mu_0": 0.056,
          "lambda": 3.313,
          "n": 0.3568
        }
      }
    ],
    "custom_derived_fields": [
      {
        "name": "q_criterion",
        "expression": "0.5 * (magSqr($Omega) - magSqr($S))",
        "units": "1/s^2"
      },
      {
        "name": "entropy_generation",
        "expression": "$mu / $T * 2 * magSqr(symm(grad($U))) + $lambda / ($T^2) * magSqr(grad($T))",
        "units": "W/m^3/K"
      }
    ]
  }
}
```

---

### 13.11 SDK 요약 테이블

| SDK | 크레이트명 | 핵심 기능 | 입력 | 출력 | 의존 |
|-----|-----------|----------|------|------|------|
| **SDK-1** Expression | `gfd-expression` | 수식 파싱, 검증, 코드 생성 | GMN 문자열 | AST, LaTeX, Rust 코드 | 없음 (최하위) |
| **SDK-2** Discretization | `gfd-discretize` | PDE → 이산 계수 변환 | AST + Mesh + Schemes | a_P, a_N[], b | SDK-1, gfd-core |
| **SDK-3** Matrix Assembly | `gfd-matrix` | 이산 계수 → Ax=b | 계수 + Mesh | SparseMatrix | gfd-core |
| **SDK-4** Linear Solver | `gfd-linalg` | Ax=b 풀이 | SparseMatrix, b | x (해 벡터) | SDK-3, faer |
| **SDK-5** Turbulence | `gfd-turbulence` | 난류 모델 정의/수정 | 모델 JSON + 수식 | 수정된 Transport Eq | SDK-1, SDK-2 |
| **SDK-6** Material | `gfd-material` | 물성치 비선형 정의 | 수식/테이블 | Property 값 | SDK-1 |
| **SDK-7** Boundary | `gfd-boundary` | 경계조건 커스텀 | 수식 + 면 정보 | 계수 수정 | SDK-1, gfd-core |
| **SDK-8** Source Term | `gfd-source` | 소스항 주입/수정 | 수식 + 영역 | Sc, Sp | SDK-1 |
| **SDK-9** Coupling | `gfd-coupling` | 멀티피직스 연성 | 필드 + 인터페이스 | 교환 데이터 | gfd-core, SDK-1 |
| **SDK-10** Post-Process | `gfd-postprocess` | 파생 변수 계산 | 수식 + 결과 필드 | 새 필드 | SDK-1, gfd-core |

---

## 14. GPU 가속 유동해석 통합 (NVIDIA CUDA)

> GFD 솔버의 유동해석(CFD)을 NVIDIA GPU로 가속화한다.
> CPU SIMPLE 솔버가 동작 완료된 상태(20×20 cavity, 39 iter 수렴)에서,
> 선형 솔버를 GPU로 이전하는 것만으로 **5~10배 전체 가속**이 가능하다.

---

### 14.1 NVIDIA GPU 유동해석 기술 현황

#### 사용 가능한 NVIDIA 라이브러리

| 기술 | 유형 | 라이선스 | 핵심 기능 | GFD 적용 |
|------|------|----------|----------|----------|
| **[AmgX](https://github.com/NVIDIA/AMGX)** | 선형 솔버 | BSD-3 | GPU AMG + Krylov (CG, BiCGSTAB, GMRES) | **1순위 — 압력 Poisson 가속** |
| **[cuSPARSE](https://developer.nvidia.com/cusparse)** | 희소행렬 | CUDA Toolkit | GPU SpMV, 행렬 변환, 삼각 풀이 | **1순위 — 행렬-벡터 곱** |
| **[cuSOLVER](https://developer.nvidia.com/cusolver)** | 직접 솔버 | CUDA Toolkit | GPU LU/Cholesky/QR, 희소 직접 풀이 | 소규모 직접 풀이 |
| **[cuDSS](https://developer.nvidia.com/cudss)** | 직접 희소 솔버 | CUDA Toolkit | GPU 최적화 직접 희소 솔버 (최신) | 대규모 직접 풀이 대안 |
| **[cuFFT](https://developer.nvidia.com/cufft)** | FFT | CUDA Toolkit | GPU Fast Fourier Transform | 스펙트럴 방법, 주기 경계 |
| **[PhysX Flow 2.2](https://github.com/NVIDIA-Omniverse/PhysX)** | 유체 시뮬레이션 | BSD-3 | sparse grid GPU 유체, 500+ CUDA 커널 | 참조 구현/알고리즘 차용 |
| **[PhysicsNeMo](https://github.com/NVIDIA/physicsnemo)** | AI 물리 | Apache-2.0 | PINN, FourCastNet, GraphCast AI CFD | 장기적 AI 가속 |
| **[cuBLAS](https://developer.nvidia.com/cublas)** | 밀집 선형대수 | CUDA Toolkit | GPU BLAS (벡터/행렬 연산) | 내부 벡터 연산 |

#### Rust-CUDA 생태계

| 크레이트 | 상태 | 기능 |
|---------|------|------|
| **[cudarc](https://github.com/coreylowman/cudarc)** | 활발 (2025) | 안전한 CUDA 래퍼, **cuSPARSE/cuSOLVER/cuBLAS 바인딩 포함** |
| **[rust-cuda](https://github.com/Rust-GPU/Rust-CUDA)** | 리부트 (2025.03) | Rust로 CUDA 커널 직접 작성 |
| **[rustacuda](https://crates.io/crates/rustacuda)** | 안정 | CUDA Driver API 래퍼 |

> **핵심 결정:** `cudarc` 크레이트를 주 CUDA 바인딩으로 사용. cuSPARSE/cuSOLVER/cuBLAS 바인딩이 이미 포함.

---

### 14.2 GPU 가속 전략: 3계층 아키텍처

```
┌─────────────────────────────────────────────────────────────────┐
│                     GFD 솔버 메인 루프 (CPU)                      │
│  Setup → Assemble → [GPU Solve] → Correct → Check Convergence  │
└────────────────────────────┬────────────────────────────────────┘
                             │
              ┌──────────────┼──────────────────┐
              ▼              ▼                   ▼
     ┌─────────────┐ ┌────────────┐  ┌──────────────────┐
     │  Level 1    │ │  Level 2   │  │    Level 3       │
     │ 선형 솔버   │ │ 행렬 연산   │  │  커널 레벨       │
     │ GPU 가속    │ │ GPU 가속   │  │  GPU 가속        │
     ├─────────────┤ ├────────────┤  ├──────────────────┤
     │ AmgX        │ │ cuSPARSE   │  │ Custom CUDA      │
     │ (AMG+CG+    │ │ (SpMV)     │  │ Kernels          │
     │  BiCGSTAB)  │ │            │  │ (면 플럭스,      │
     │ cuSOLVER    │ │ cuBLAS     │  │  구배 계산,       │
     │ cuDSS       │ │ (벡터)     │  │  보정 연산)       │
     └─────────────┘ └────────────┘  └──────────────────┘
```

#### Level 1: 선형 솔버 GPU 가속 (최우선 — 최대 효과)

SIMPLE 알고리즘에서 **전체 시간의 60~80%가 선형 시스템 풀이**에 소모됨.
→ 선형 솔버만 GPU로 옮겨도 **5~10x 전체 가속**.

| 대상 방정식 | CPU 현재 | GPU 교체 | 이유 |
|------------|---------|---------|------|
| 압력 Poisson (p') | CG + Jacobi | **AmgX AMG-CG** | SPD, 가장 느림, AMG 최적 |
| 운동량 (u,v,w) | BiCGSTAB | **AmgX BiCGSTAB + ILU** | 비대칭, 대규모 |
| 에너지 (T) | CG | **cuSOLVER CG** 또는 **AmgX** | SPD |
| 난류 (k,ε,ω) | BiCGSTAB | **AmgX BiCGSTAB** | 비대칭 |

#### Level 2: 행렬 연산 GPU 가속

| 연산 | CPU 현재 | GPU 교체 |
|------|---------|---------|
| SpMV (Ax) | gfd-core spmv() | **cuSPARSE cusparseSpMV** |
| 벡터 내적/범수 | 수동 루프 | **cuBLAS cublasDdot/cublasDnrm2** |
| 행렬 포맷 변환 | COO→CSR | **cuSPARSE** format conversion |

#### Level 3: 커스텀 CUDA 커널

| 연산 | 설명 | 병렬화 전략 |
|------|------|-----------|
| 면 플럭스 계산 | F_f = ρ·u_f·n_f·A_f | 1 thread per face |
| 구배 계산 (Green-Gauss) | grad(φ) = (1/V)·Σ(φ_f·A·n) | 1 thread per face, atomic add |
| 속도 보정 | u -= (V/a_P)·grad(p') | 1 thread per cell |
| 압력 보정 | p += α_p·p' | 1 thread per cell |
| Under-relaxation | φ = α·φ_new + (1-α)·φ_old | 1 thread per cell |
| 잔차 계산 | max(\|mass_imbalance\|) | Parallel reduction |

---

### 14.3 기존 코드와의 통합 설계

#### 신규 크레이트: `gfd-gpu`

```
crates/gfd-gpu/                       # GPU 추상화 레이어
├── Cargo.toml
├── kernels/                          # CUDA 커널 소스 (.cu)
│   ├── flux.cu
│   ├── gradient.cu
│   ├── correction.cu
│   └── reduction.cu
└── src/
    ├── lib.rs                        # GpuContext, feature flags
    ├── device.rs                     # GPU 디바이스 감지/선택
    ├── memory.rs                     # GpuVector (cuBLAS 연산)
    ├── sparse.rs                     # GpuSparseMatrix (cuSPARSE SpMV)
    ├── transfer.rs                   # CPU↔GPU 데이터 전송
    ├── solver/
    │   ├── mod.rs                    # GpuLinearSolver trait
    │   ├── amgx.rs                   # AmgX FFI 래퍼
    │   ├── cusolver.rs               # cuSOLVER 래퍼
    │   └── cusparse_cg.rs            # cuSPARSE 기반 GPU CG
    └── kernels/
        ├── mod.rs
        ├── flux.rs                   # 면 플럭스 커널 래퍼
        ├── gradient.rs               # Green-Gauss 커널 래퍼
        ├── correction.rs             # 보정 커널 래퍼
        └── reduction.rs              # 리덕션 커널 래퍼
```

#### Trait 기반 CPU/GPU 투명 교체

```rust
/// 통합 디스패처 — 런타임 CPU/GPU 선택
pub enum SolverBackend {
    Cpu(Box<dyn LinearSolverTrait>),
    Gpu(Box<dyn GpuLinearSolver>),
}

impl SolverBackend {
    pub fn solve(&mut self, system: &mut LinearSystem) -> Result<SolverStats> {
        match self {
            SolverBackend::Cpu(s) => s.solve(&system.a, &system.b, &mut system.x),
            SolverBackend::Gpu(s) => {
                let gpu_a = GpuSparseMatrix::from_cpu(&system.a)?;
                let gpu_b = GpuVector::from_cpu(&system.b)?;
                let mut gpu_x = GpuVector::from_cpu(&system.x)?;
                let stats = s.solve_gpu(&gpu_a, &gpu_b, &mut gpu_x)?;
                gpu_x.to_cpu(&mut system.x)?;
                Ok(stats)
            }
        }
    }
}
```

#### SIMPLE 솔버 GPU 통합 (simple.rs 수정)

```rust
// 현재 (CPU)
let mut solver = BiCgStab::new(1e-6, 1000);
solver.solve(&system.a, &system.b, &mut system.x)?;

// GPU 통합 후
let solver_backend = if gpu_available() {
    SolverBackend::Gpu(Box::new(AmgxSolver::new(AmgxConfig::bicgstab_ilu())))
} else {
    SolverBackend::Cpu(Box::new(BiCgStab::new(1e-6, 1000)))
};
solver_backend.solve(&mut system)?;
```

#### Feature Flag 기반 조건부 컴파일

```toml
# gfd-gpu/Cargo.toml
[features]
default = []
cuda = ["cudarc"]
amgx = ["cuda"]

[dependencies]
gfd-core = { path = "../gfd-core" }
cudarc = { version = "0.12", features = ["cusparse", "cusolver", "cublas"], optional = true }
```

```bash
# CPU 전용 빌드 (기본)
cargo build --release

# GPU 가속 빌드
cargo build --release --features gpu

# GPU + AmgX 빌드
cargo build --release --features gpu-amgx
```

---

### 14.4 AmgX 통합 상세

#### AmgX C API FFI 래핑

```rust
// gfd-gpu/src/solver/amgx.rs
extern "C" {
    fn AMGX_initialize() -> i32;
    fn AMGX_config_create_from_file(cfg: *mut AmgxConfig, path: *const c_char) -> i32;
    fn AMGX_matrix_upload_all(mtx: AmgxMatrix, n: i32, nnz: i32, ...) -> i32;
    fn AMGX_solver_setup(slv: AmgxSolver, mtx: AmgxMatrix) -> i32;
    fn AMGX_solver_solve(slv: AmgxSolver, rhs: AmgxVector, sol: AmgxVector) -> i32;
    fn AMGX_vector_download(vec: AmgxVector, data: *mut f64) -> i32;
    fn AMGX_finalize() -> i32;
}

pub struct AmgxSolver { /* config, resources, solver, matrix, vectors */ }

impl AmgxSolver {
    pub fn pressure_solver() -> Self { /* AMG-CG config */ }
    pub fn momentum_solver() -> Self { /* ILU-BiCGSTAB config */ }
}
```

#### AmgX 최적 구성 (압력 Poisson)

```json
{
  "config_version": 2,
  "solver": {
    "solver": "PCG",
    "preconditioner": {
      "solver": "AMG",
      "smoother": "BLOCK_JACOBI",
      "presweeps": 2, "postsweeps": 2,
      "max_levels": 20, "cycle": "V"
    },
    "max_iters": 500, "tolerance": 1e-8, "norm": "L2"
  }
}
```

---

### 14.5 커스텀 CUDA 커널

#### 면 플럭스 계산

```cuda
__global__ void compute_face_flux(
    const double* vel_x, const double* vel_y, const double* vel_z,
    const double* density,
    const int* face_owner, const int* face_neighbor,
    const double* normal_x, const double* normal_y, const double* normal_z,
    const double* face_area, double* face_flux, int num_faces
) {
    int fid = blockIdx.x * blockDim.x + threadIdx.x;
    if (fid >= num_faces) return;
    int o = face_owner[fid], n = face_neighbor[fid];
    double uf, vf, wf, rho_f;
    if (n >= 0) {
        uf = 0.5*(vel_x[o]+vel_x[n]); vf = 0.5*(vel_y[o]+vel_y[n]);
        wf = 0.5*(vel_z[o]+vel_z[n]); rho_f = 0.5*(density[o]+density[n]);
    } else {
        uf=vel_x[o]; vf=vel_y[o]; wf=vel_z[o]; rho_f=density[o];
    }
    face_flux[fid] = rho_f*(uf*normal_x[fid]+vf*normal_y[fid]+wf*normal_z[fid])*face_area[fid];
}
```

#### Green-Gauss 구배

```cuda
__global__ void green_gauss_gradient(
    const double* phi, const int* face_owner, const int* face_neighbor,
    const double* nx, const double* ny, const double* nz,
    const double* area, double* gx, double* gy, double* gz, int num_faces
) {
    int fid = blockIdx.x * blockDim.x + threadIdx.x;
    if (fid >= num_faces) return;
    int o = face_owner[fid], n = face_neighbor[fid];
    double phi_f = (n >= 0) ? 0.5*(phi[o]+phi[n]) : phi[o];
    double cx = phi_f*area[fid]*nx[fid], cy = phi_f*area[fid]*ny[fid], cz = phi_f*area[fid]*nz[fid];
    atomicAdd(&gx[o], cx); atomicAdd(&gy[o], cy); atomicAdd(&gz[o], cz);
    if (n >= 0) { atomicAdd(&gx[n],-cx); atomicAdd(&gy[n],-cy); atomicAdd(&gz[n],-cz); }
}
```

#### 속도/압력 보정

```cuda
__global__ void correct_velocity(
    double* vx, double* vy, double* vz,
    const double* gpx, const double* gpy, const double* gpz,
    const double* vol, const double* a_p, int n
) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i >= n) return;
    double rA = vol[i] / a_p[i];
    vx[i] -= rA*gpx[i]; vy[i] -= rA*gpy[i]; vz[i] -= rA*gpz[i];
}
```

---

### 14.6 PhysX Flow 2.2 활용 전략

PhysX Flow는 **sparse grid 기반** GPU 유체 시뮬레이션 (BSD-3, 2025.04 완전 오픈소스).
직접 통합보다는 **알고리즘/커널 설계 참조**로 활용.

| PhysX Flow 기술 | GFD 적용 |
|----------------|---------|
| Sparse grid 메모리 관리 | 적응 격자 메모리 최적화 |
| GPU 커널 동기화 패턴 | CUDA 커널 체이닝 설계 |
| Shared memory 활용 | stencil 연산 최적화 |
| Warp-level 프리미티브 | 리덕션 커널 최적화 |

---

### 14.7 GPU 구현 로드맵

| Phase | 내용 | 기간 |
|-------|------|------|
| **A** | GPU 인프라 (cudarc 연동, GpuSparseMatrix, GpuVector, GPU SpMV) | 2주 |
| **B** | GPU 선형 솔버 (GPU CG/BiCGSTAB, Jacobi GPU, 벤치마크) | 3주 |
| **C** | AmgX 통합 (FFI 바인딩, AMG-CG, ILU-BiCGSTAB, SIMPLE 연결) | 2주 |
| **D** | 커스텀 CUDA 커널 (면 플럭스, 구배, 보정, 리덕션) | 2주 |
| **E** | 전체 GPU SIMPLE 루프 (GPU 상주, 전송 최소화, Multi-GPU) | 2주 |

---

### 14.8 기존 코드 수정 요약

| 기존 파일 | 수정 내용 |
|----------|----------|
| `Cargo.toml` (루트) | `gfd-gpu` 의존성, `gpu`/`gpu-amgx` feature flag |
| `gfd-linalg/src/lib.rs` | `#[cfg(feature = "gpu")] pub mod gpu;` |
| `gfd-fluid/incompressible/simple.rs` | `SolverBackend` 디스패처 교체 |
| `gfd-thermal/conduction.rs` | GPU 솔버 옵션 |
| `src/main.rs` | `--gpu` CLI 플래그, GPU 초기화 |
| `gfd-parallel/src/gpu/mod.rs` | 스텁 → cudarc 연동 |

| 신규 파일 | 내용 |
|----------|------|
| `gfd-gpu/Cargo.toml` | cudarc, AmgX FFI |
| `gfd-gpu/src/lib.rs` | GpuContext |
| `gfd-gpu/src/device.rs` | GPU 감지/선택 |
| `gfd-gpu/src/memory.rs` | GpuVector |
| `gfd-gpu/src/sparse.rs` | GpuSparseMatrix + cuSPARSE SpMV |
| `gfd-gpu/src/solver/*.rs` | AmgX, cuSOLVER, GPU CG |
| `gfd-gpu/src/kernels/*.rs` | CUDA 커널 Rust 래퍼 |
| `gfd-gpu/kernels/*.cu` | CUDA 커널 소스 |

---

### 14.9 예상 성능 향상

| 구성 요소 | CPU (현재) | GPU (예상) | 가속비 |
|----------|-----------|-----------|-------|
| 압력 Poisson (CG) | 100% | ~10% (AmgX AMG-CG) | **~10x** |
| 운동량 (BiCGSTAB) | 100% | ~15% (AmgX+ILU) | **~7x** |
| 구배 계산 | 100% | ~5% (CUDA kernel) | **~20x** |
| 면 플럭스 | 100% | ~5% (CUDA kernel) | **~20x** |
| 보정 연산 | 100% | ~2% (CUDA kernel) | **~50x** |
| **전체 SIMPLE step** | **100%** | **~12%** | **~8x** |

> 1M cell: CPU ~300초 → GPU ~40초 (8x)
> 10M cell: GPU 이점 더 증가 (메모리 대역폭 → 계산 병목 전환)

---

## 부록 A: 참고 자료

### 오픈소스 솔버 저장소
- [OpenFOAM](https://github.com/OpenFOAM/OpenFOAM-dev)
- [SU2](https://github.com/su2code/SU2)
- [Code_Saturne](https://github.com/code-saturne/code_saturne)
- [FDS](https://github.com/firemodels/fds)
- [CFL3D](https://github.com/nasa/CFL3D)
- [Nek5000](https://github.com/Nek5000/Nek5000)
- [MFEM](https://github.com/mfem/mfem)
- [deal.II](https://github.com/dealii/dealii)
- [FEniCSx](https://github.com/FEniCS/dolfinx)
- [CalculiX](https://github.com/Dhondtguido/CalculiX)
- [Elmer FEM](https://github.com/ElmerCSC/elmerfem)
- [MOOSE](https://github.com/idaholab/moose)
- [Kratos](https://github.com/KratosMultiphysics/Kratos)
- [preCICE](https://github.com/precice/precice)
- [OpenRadioss](https://github.com/OpenRadioss/OpenRadioss)
- [MFC](https://github.com/MFlowCode/MFC)

### Rust 생태계
- [vdb-rs](https://github.com/Traverse-Research/vdb-rs) — Rust VDB 구현
- [nalgebra](https://nalgebra.org/) — 선형대수
- [faer](https://github.com/sarah-ek/faer-rs) — 고성능 선형대수
- [vtkio](https://crates.io/crates/vtkio) — VTK I/O
- [cudarc](https://github.com/coreylowman/cudarc) — Rust CUDA 바인딩 (cuSPARSE/cuSOLVER/cuBLAS)
- [rust-cuda](https://github.com/Rust-GPU/Rust-CUDA) — Rust CUDA 커널 작성
- [rustacuda](https://crates.io/crates/rustacuda) — CUDA Driver API 래퍼

### GPU / NVIDIA 라이브러리
- [NVIDIA AmgX](https://github.com/NVIDIA/AMGX) — GPU 멀티그리드 선형 솔버 (BSD-3)
- [cuSPARSE](https://developer.nvidia.com/cusparse) — GPU 희소 행렬 연산
- [cuSOLVER](https://developer.nvidia.com/cusolver) — GPU 선형 솔버
- [cuDSS](https://developer.nvidia.com/cudss) — GPU 직접 희소 솔버 (최신)
- [cuFFT](https://developer.nvidia.com/cufft) — GPU Fast Fourier Transform
- [cuBLAS](https://developer.nvidia.com/cublas) — GPU 밀집 선형대수
- [PhysX Flow 2.2](https://github.com/NVIDIA-Omniverse/PhysX) — GPU 유체 시뮬레이션 (BSD-3, 2025 완전 오픈소스)
- [PhysicsNeMo](https://github.com/NVIDIA/physicsnemo) — AI 물리 프레임워크

### 벤치마크 데이터
- [NASA Turbulence Modeling Resource](https://turbmodels.larc.nasa.gov/)
- [Ghia et al. (1982) Lid-Driven Cavity](https://doi.org/10.1016/0021-9991(82)90058-4)
- [De Vahl Davis (1983) Natural Convection](https://doi.org/10.1002/fld.1650030305)

---

> **문서 버전:** v1.3
> **최종 수정:** 2026-03-24
> **변경 이력:**
> - v1.3 — Section 14 (GPU 가속 유동해석 통합, NVIDIA CUDA) 추가, 기술 스택 GPU 항목 추가
> - v1.2 — Section 13 (사용자 정의 수학식 편집 시스템 및 SDK 설계) 추가
> - v1.1 — Section 12 (GFD 솔버 워크플로우 구조, Fluent 기반) 추가
> **다음 단계:** GPU Phase A — gfd-gpu 크레이트 생성 및 cudarc 연동
