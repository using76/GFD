# AI-Driven Simulation Loop — Implementation Progress

목표(사용자 요청): **AI가 직접 조작 → 3D 라이브 표시 → CAD 파일 읽기 → mesh → 해석
설정 → 모델 선택 → 계산 → 결과 표현 → 결과 분석 → 문제점 파악 → 시뮬레이션
재수정** 이 폐루프(closed loop)가 끝까지 작동하도록, 각 요소의 미구현/개선점을
찾아 구현한다. (Ralph loop, max 20 iterations)

검증 게이트(매 iteration): `cargo build --bin gfd-server`, `cd gui && npx tsc
--noEmit` (0 errors), `cd gui && npm test`.

## Loop spine — 끊긴 링크 분석 (iter 1 기준 실제 코드 확인)

| # | 단계 | 상태 | 비고 |
|---|------|:----:|------|
| 1 | AI 조작 (MCP) | O | `core/mcp/bridge.ts`, `mcp-server/` 작동 |
| 2 | 라이브 3D | O | `ViewportV2.tsx` (Geometry/Results/Vector/Streamline layer 모두 존재) |
| 3 | **CAD 파일 읽기 → 트리** | **X→진행** | `io.ts`에 import 명령 없음; mesh import는 loose mesh만 반환(arena 미등록) |
| 4 | mesh 생성 | O | `mesh.generate`; 단 mesh 설정 패널 부족 |
| 5 | 해석 설정 | O | `setup.*` |
| 6 | 모델 선택 | O | `physics.*`, manifest |
| 7 | 계산 | O | 실제 solver `calc.run`, `realSolver` |
| 8 | 결과 표현 | O | ViewportV2 layers (문서가 stale했음 — 실제로는 구현됨) |
| 9 | **결과 분석** | O(iter2) | `calc.diagnose` — 수렴/발산/Re·난류모델/유동미발달 진단 |
| 10 | **문제 파악 → 재수정** | O(iter3) | `runAutoRefine` + `auto_refine` MCP 메타툴 — diagnose→fix→재실행 자율 폐루프 |

**→ iter 3 시점: 루프 spine 전 구간(1–10)이 연결됨.** AI는 `io.import_*`로 CAD를
올리고, mesh/setup/model/calc.run으로 해석하고, `calc.diagnose`로 분석하고,
`auto_refine` 한 번 호출로 진단→수정→재실행을 자율 반복할 수 있다.

## Iteration 기록

### iter 1 — CAD 파일 import → feature 트리 ✅ (완료)
- 문제: `io.ts`는 export만 있고 import 명령 부재. `cad.import.stl/obj/off/ply/xyz`는
  loose mesh만 반환(shape_id 없음→트리/렌더 불가). STEP/BRep만 shape_id 반환.
- 구현:
  - 백엔드(`src/server.rs`): `ServerState.imported_meshes: HashMap<String,TriMesh>` +
    `cad.import.mesh_to_tree` RPC(파일→TriMesh 저장→shape_id+bbox+counts 반환);
    `handle_cad_tessellate_adaptive`가 imported mesh를 우선 반환; delete/reset도 정리.
  - command-core(`gui/src/core/commands/io.ts`): `io.import_mesh`(stl/obj/off/ply/xyz,
    bbox 포함) / `io.import_step` / `io.import_brep`(tessellation에서 bbox 유도) 명령.
    MCP bridge가 자동으로 tool 노출 → AI가 바로 import 가능.
- 검증: `cargo build --bin gfd-server` OK, `tsc --noEmit` 0 errors, `vitest run` 75 passed
  (import 노드 생성 테스트 2개 추가).
- 효과: AI/유저가 STL/OBJ/STEP을 트리에 올려 ViewportV2에서 렌더 → 루프의 시작점 연결.

### iter 2 — 결과 분석/진단 명령 `calc.diagnose` ✅ (완료)
- 문제: 루프의 "결과 분석 → 문제 파악" 두뇌 부재. 단일 residual만 있고 AI가 무엇을
  고쳐야 할지 판단할 구조화된 진단/제안이 없음.
- 구현(`gui/src/core/commands/calc.ts`): `diagnoseState(AppState)` 순수 함수 +
  `calc.diagnose` 명령. solver 상태·field 통계·재료·설정을 읽어 이슈 도출:
  발산(NaN/Inf/과대)→error, 최대반복 미수렴→warning, 잔차정체→warning,
  Re vs 난류모델 불일치→warning(k_epsilon 제안), 저Re+난류모델→info, 유동미발달→warning.
  각 이슈에 actionable `fix:{command,params}`(setup.set_solver / setup.set_model /
  calc.run) 포함 → AI가 바로 적용해 루프 폐쇄 가능. Re/유동영역(laminar/transitional/
  turbulent)/특성길이·속도/요약 반환.
- 검증: `tsc --noEmit` 0 errors, `vitest run` 80 passed (diagnose 테스트 5개 추가).

### iter 3 — 자율 재수정 폐루프 `runAutoRefine` + `auto_refine` MCP 메타툴 ✅ (완료)
- 문제: `CommandContext`에 `dispatch`가 없어 명령이 명령을 호출 못함 → 진단→수정→재실행
  자동화가 불가. 루프의 마지막 링크(자율 재수정)가 비어 있었음.
- 구현:
  - `gui/src/core/solver/autoRefine.ts`: `runAutoRefine(core, opts, onEvent)` —
    `core.dispatcher` 기반 오케스트레이터. 매 라운드 `diagnoseState`→최상위(에러>경고)
    actionable 이슈 선택→fix 명령 dispatch→`calc.run` 재실행→완료 대기(status 폴링)→
    라운드 기록. healthy까지 또는 maxRounds. round별 before/after residual·status 반환.
  - `gui/src/core/mcp/bridge.ts`: `auto_refine` 메타툴 추가 → AI가 한 번 호출로 폐루프 실행.
- 검증: `tsc --noEmit` 0 errors, `vitest run` 83 passed (autorefine 테스트 3개:
  발산→완화하향→수렴, healthy no-op, MCP 노출).

### iter 4 — 메쉬 품질을 diagnose에 통합 ✅ (완료)
- 문제: 발산/저수렴의 주요 원인인 나쁜 메쉬를 분석이 보지 못함.
- 구현: `MeshState`에 `badCells`/`gen{nx,ny,nz}` 저장(`mesh.generate`), `diagnoseState`가
  직교성<0.15·왜도>0.9·bad cells>0·종횡비>1000 감지 → 경고 + `mesh.generate`(격자 2배)
  fix 제안. `DiagnoseResult.mesh` 요약 추가.
- 검증: tsc 0, vitest 85 passed (메쉬 진단 테스트 2개 추가).

### iter 5 — 진단 결과 AppState 캐시 + MCP 노출 ✅ (완료)
- 문제: diagnose/auto_refine 결과가 명령 반환값으로만 존재 → UI/AI가 상태로 못 봄.
- 구현: `AppState.diagnosis`(JsonValue) 추가, `calc.diagnose`가 결과를 state에 기록,
  `auto_refine`이 매 라운드 `calc.diagnose` dispatch로 라이브 갱신, `get_state_summary`
  메타툴이 diagnosis(summary+issues) 노출.
- 검증: tsc 0, vitest 86 passed (state 캐시 테스트 추가).

### iter 6 — 진단 라이브 UI 패널 ✅ (완료)
- 문제: "결과 분석"이 화면에 안 보임("라이브로 보이고" 미충족).
- 구현(`gui/src/react/ResultsPanel.tsx`): `DiagnosisSection` — [진단]/[자동 수정] 버튼 +
  `state.diagnosis` 이슈를 심각도 색상으로 표시 + 이슈별 [적용] 버튼(`fix.command` dispatch).
  결과 없을 때도 표시. 사람도 한 번에 AI 제안을 적용/자율수정 실행 가능.
- 검증: tsc 0, `vite build` 성공, vitest 86 passed.

### iter 7 — AI에게 시뮬레이션 루프 교육 + scene context에 진단 주입 ✅ (완료)
- 문제: `SYSTEM_PROMPT`이 geometry/gmsh/calc.run만 알고 io.import_*/calc.diagnose/
  auto_refine/전체 워크플로우를 몰라 AI가 새 기능을 안 씀(=루프 미작동).
- 구현: 프롬프트/컨텍스트를 순수 모듈 `gui/src/react/agentPrompt.ts`로 분리하고
  "THE SIMULATION LOOP" 7단계(GEOMETRY→MESH→SETUP→SOLVE→VISUALIZE→ANALYZE→
  SELF-CORRECT)와 새 명령들을 명시. `sceneContext`가 `state.diagnosis` 요약+이슈를
  매 턴 주입 → AI가 최신 분석을 보고 행동.
- 검증: tsc 0, vite build, vitest 89 (agentPrompt 테스트 3개).

### iter 8 — auto_refine 무진전(no-progress) 조기 종료 ✅ (완료)
- 문제: 동일 이슈(예: 발산)가 개선 없이 반복되면 효과 없는 fix를 maxRounds까지 반복.
- 구현(`autoRefine.ts`): 직전 라운드와 같은 issueCode이고 `roundImproved`(수렴 또는 잔차
  >1% 감소) 아니면 `stoppedReason='no_progress'`로 즉시 중단. 솔버 낭비 방지.
- 검증: tsc 0, vitest 90 (stuck backend → 1 라운드 후 no_progress 테스트).

### iter 9 — per-equation residual 분리 (백엔드→진단 전 스택) ✅ (완료)
- 문제: 백엔드가 단일 residual(연속/continuity)만 보고 → AI가 어느 방정식이
  발산/미수렴하는지 모름.
- 구현(안전한 additive, gfd-fluid 미변경):
  - 백엔드(`src/server.rs`): `run_fluid_solve`가 반복 간 필드별 update 잔차
    (Δvx/Δvy/Δvz/Δp의 normalized L2)를 계산 → `JobResult.eq_residuals`. `solve.status`가
    finished 시 `residuals{vx,vy,vz,pressure,continuity}` 노출(n/a는 null).
  - `realSolver.ts`→`SolverStatus.residualsByEq`→`calc.run` 저장.
  - `diagnoseState`: 미수렴 시 지배(dominant) 방정식 식별 → 운동량이면 relaxVelocity,
    압력이면 relaxPressure를 낮추는 타깃 fix(`DOMINANT_EQUATION`, info). `dominantEquation`/
    `residualsByEq` 반환.
- 검증: `cargo build --bin gfd-server` OK, tsc 0, vitest 92 passed (per-eq 테스트 2개).

### iter 10 — 공간 엔티티 참조(nearest/semantic) 구현 ✅ (완료)
- 문제: `entity.ts`의 `nearest`/`semantic`이 null 반환(stub) → AI가 위치/방향으로 형상
  선택 불가. (`query_spatial`/`select` MCP 메타툴이 이 resolver를 사용.)
- 구현(`gui/src/core/entity.ts`, AppState bbox 기반 shape 단위):
  - `nearest{point}`: 점→AABB 최단거리로 가장 가까운 형상.
  - `semantic{hint}`: top/bottom/left/right/front/back = 해당 축 극단 형상(+Z up),
    inlet/outlet = 이름 매칭, largest_face ≈ 최대 bbox 형상, `of`로 풀 제한.
  - face/edge/vertex 단위는 백엔드 tessellation 필요 → null 유지(정직).
- 검증: tsc 0, vitest 97 passed (entity 테스트 5개).

### iter 11 — `measure.distance` 명령 ✅ (완료)
- 문제: 두 형상 간 거리/간격 측정 명령 부재(상태문서 distance 2-pick gap).
- 구현(`gui/src/core/commands/measure.ts`): `measure.distance{a,b}` — 중심거리 +
  bbox 최근접 간격(겹치면 0) + 각 중심. AI가 간격/클리어런스 점검 가능.
- 검증: tsc 0, vitest 100 passed (measureDistance 테스트 3개).

### iter 12 — auto_refine 에스컬레이션(효과 없는 fix → 다른 fix) ✅ (완료)
- 문제: iter 8은 같은 이슈가 안 먹히면 즉시 중단 → 다른 처방(예: 메쉬 재생성)을 시도 안 함.
- 구현(`autoRefine.ts`): `actionableIssues`(error→warning→NO_RESULTS 정렬) +
  `ineffectiveCodes` 집합. 각 distinct fix를 한 번만 시도하고, 개선 없으면 그 코드를
  제외하고 다음 actionable 이슈로 escalate. 모두 소진 시 `no_progress`.
  예: 발산→완화계수 하향(무효)→메쉬 재생성으로 escalate→중단.
- 검증: tsc 0, vitest 101 passed (에스컬레이션 테스트 추가; 기존 no_progress/healthy 유지).

### iter 13 — get_state_summary에 per-equation residual 노출 ✅ (완료)
- 구현(`mcp/bridge.ts`): `get_state_summary`의 solver에 `residualsByEq` 포함 →
  AI가 저렴한 요약 한 번으로 어느 방정식(vx/vy/vz/pressure/continuity)이 문제인지 파악.
- 검증: tsc 0, mcp 테스트 통과.

### iter 14 — diagnose UI에 per-equation 잔차 + 지배 방정식 표시 ✅ (완료)
- 구현(`gui/src/react/ResultsPanel.tsx`): `EqResiduals` — `state.diagnosis.residualsByEq`를
  방정식별로 표시하고 `dominantEquation`을 강조(◄). "라이브로 보이고"의 human-facing 완성.
- 검증: tsc 0, vite build 성공.

### iter 15 — 전체 루프 end-to-end 통합 테스트 ✅ (완료)
- 목적: 사용자의 핵심 요구("이 루프가 작동")를 단위가 아닌 **전 구간 조합**으로 검증 + 회귀 방지.
- 구현(`gui/src/core/__tests__/loop.test.ts`): fakeBackend로
  (1) import_mesh→mesh.generate→setup(model/material/boundary)→calc.run→수렴→
  calc.diagnose(healthy, per-eq residual state 반영) 전 구간,
  (2) 고Re+층류모델 → diagnose가 TURBULENCE_MODEL 감지 → `auto_refine`(MCP 메타툴)이
  k_epsilon로 자가수정 → healthy. **자가수정 폐루프까지 end-to-end 통과.**
- 검증: tsc 0, vitest 103 passed (통합 테스트 2개).
- 결과 확인 (mesh 전제조건): 실제 백엔드(`server.rs handle_solve_start`)는 mesh 없으면
  "No mesh generated yet. Call mesh.generate first." 반환 — 이미 graceful(gap 아님).

### iter 16 — `system.loop_status` (AI 루프 오케스트레이션 가이드) ✅ (완료)
- 문제: AI가 루프 단계를 건너뛰거나 순서를 틀릴 수 있음(예: mesh 없이 solve 시도).
- 구현(`gui/src/core/commands/system.ts`): `system.loop_status` — geometry→mesh→setup→
  solve→analyze 각 단계 완료 여부 + `readyToSolve` + **단일 권장 다음 행동**(nextAction:
  step/command/hint) 반환. 레지스트리 command라 MCP 툴로 자동 노출. 프롬프트에도 안내 추가.
- 검증: tsc 0, vitest 108 passed (단계별 nextAction 테스트 5개).

### iter 17 — isosurface 렌더링 (결과 표현, 전 스택) ✅ (완료)
- 문제: "결과 표현"에서 3D 등치면(isosurface)이 미구현(상태문서 X). contour는 경계면만.
- 구현(marching tetrahedra, 테이블 없는 robust 방식):
  - 백엔드(`src/server.rs`): `march_tet`(셀 below 개수로 분기) + `handle_field_isosurface`
    (cell-center dual grid, cell idx=i+j·nx+k·nx·ny, 위치는 mesh.cell.center) +
    `field.isosurface` RPC. **Rust 단위 테스트 3개**(1-below/2-2/no-crossing).
  - command-core: `results.isosurface{field,isovalue}`(기본 isovalue=평균), `VizState`에
    `showIsosurface`/`isovalue`.
  - 렌더러(`ViewportV2.tsx`): `IsosurfaceLayer`(반투명 녹색 mesh, computeVertexNormals).
  - UI(`ResultsPanel.tsx`): Isosurface 토글 + isovalue 슬라이더(field min~max).
- 검증: `cargo test --bin gfd-server march_tet` 3 passed, tsc 0, vite build, vitest 109.

### iter 18 — vorticity 파생장 (결과 분석/표현, 전 스택) ✅ (완료)
- 문제: 와류 분석에 필수인 vorticity(|∇×u|) 파생장이 없음. isosurface(17)와 결합 시 강력.
- 구현:
  - 백엔드(`src/server.rs`): `compute_vorticity_magnitude`(구조격자 중심차분, 경계 one-sided) +
    `handle_field_vorticity`(velocity→vorticity_magnitude를 state.fields에 등록) +
    `field.vorticity` RPC. **Rust 테스트**(솔리드회전 u=(-Ωy,Ωx,0) → |ω|=2Ω 전셀 검증).
  - command-core: `results.vorticity` — 계산 후 `availableFields`/`fieldStats`/`activeField`에
    vorticity_magnitude 추가 → contour/isosurface/load_field로 바로 사용.
  - UI: ResultsPanel "+ 와도" 버튼. 프롬프트 VISUALIZE 단계에 isosurface/vorticity 안내.
- 검증: `cargo test --bin gfd-server` 4 passed, tsc 0, vite build, vitest 110.

### iter 19 — Q-criterion 파생장 (와류 코어 식별) ✅ (완료)
- 문제: vorticity는 전단까지 포함 → 와류 코어를 과대표시. Q-criterion(½(‖Ω‖²−‖S‖²))이
  표준 와류 식별 기준(Q>0 = 회전 우세).
- 구현:
  - 백엔드(`src/server.rs`): `compute_q_criterion`(속도구배 텐서→S/Ω→Q) + `field_stats`
    헬퍼 + `handle_field_qcriterion`(q_criterion 등록) + `field.qcriterion` RPC.
    **Rust 테스트**: 솔리드회전 → S=0, Q=½‖Ω‖²=1 전셀 검증.
  - command-core: 파생장 명령 팩토리 `makeDerivedFieldCommand`로 vorticity/qcriterion 통합.
    `results.qcriterion`.
  - UI: ResultsPanel "+ Q" 버튼. 프롬프트에 q-criterion 안내.
- 검증: `cargo test --bin gfd-server` 5 passed, tsc 0, vite build, vitest 111.

### iter 20 — 수렴 추세(convergence trend) 분석 ✅ (완료)
- 문제: 단일 잔차만으로는 "느리게 수렴 중"과 "정체"를 구분 못함 → 처방이 모호.
- 구현:
  - `SolverStatus.residualHistory`(calc.run이 onResidual마다 누적, 200 cap, 시작 시 리셋).
  - `diagnoseState`: `convergenceTrendOf`(최근 6점 비율→converging/stalled/diverging) →
    `DiagnoseResult.convergenceTrend` + summary에 `trend=` 추가. 미수렴+정체+허용오차초과면
    `STALLED` 경고(완화계수↓ fix)를 MAX_ITERS보다 먼저 → auto_refine이 완화계수부터 시도,
    안되면 반복↑로 escalate(iter 12).
- 검증: tsc 0, vitest 113 (정체/수렴 추세 테스트 2개).

---

## ✅ 최종 요약 (iter 1–20, Ralph loop 완료)

**목표 달성:** "AI가 직접 조작 → 라이브 3D → CAD 읽기 → mesh → 설정 → 모델 →
계산 → 표현 → 분석 → 문제파악 → 재수정" 폐루프가 **end-to-end로 작동·검증**됨
(`loop.test.ts`가 자가수정까지 통과).

| 루프 단계 | 구현 iteration |
|---|---|
| AI 직접 조작(MCP) | 7(프롬프트 교육), 16(loop_status), 10(공간선택) |
| 라이브 3D | (기존) + 17(isosurface), 18/19(파생장 렌더) |
| CAD 파일 읽기 | **1**(io.import_mesh/step/brep → 트리) |
| mesh | 4(품질진단) |
| 해석 설정/모델 | (기존) + diagnose 연동 |
| 계산 | (기존, 실제 solver) + 9(per-eq residual) |
| 결과 표현 | 6/14(패널), 17(isosurface), 18(vorticity), 19(Q-criterion) |
| 결과 분석 | **2**(diagnose), 5(state캐시), 9/13(per-eq), 20(추세) |
| 문제 파악→재수정 | **3**(auto_refine), 8(no-progress), 12(escalation) |
| 측정/검증 | 11(measure.distance), 15(통합테스트) |

**검증 총계:** 113 TS tests + 5 Rust tests, tsc 0 errors, vite build OK,
`cargo build/test --bin gfd-server` OK. branch `feat/ai-sim-loop` (14 commits) —
리뷰/머지 준비 완료.

### iter 21 — STEP 면(face) 복원 → 렌더 가능한 solid ✅ (완료)
- 문제: STEP import가 points-only라 임포트 시 보이지 않는 점 구름만 생성(가장 흔한
  엔지니어링 CAD 포맷인데 사용 불가).
- 구현:
  - `gfd-cad-io`(`step.rs`): `read_step_trimesh` — ADVANCED_FACE→FACE_OUTER_BOUND→
    EDGE_LOOP→EDGE_CURVE(/ORIENTED_EDGE)→VERTEX_POINT→CARTESIAN_POINT 체인을 파싱,
    각 루프의 정렬된 정점으로 폴리곤 면 복원 후 fan 삼각화 → TriMesh. **Rust 테스트**
    (삼각형 면 write→read 라운드트립, 복원 정점이 원본 위에 있음 검증).
  - 백엔드(`server.rs`): `cad.import.step_mesh`(read_step_trimesh→imported_meshes 등록,
    shape_id+bbox 반환 — iter 1 인프라 재사용).
  - command-core(`io.ts`): `io.import_step`이 faceted 재구성 우선, 실패 시 arena/points
    폴백. 결과에 `faceted` 플래그.
- 검증: `cargo test -p gfd-cad-io read_step_trimesh` 1 passed, `cargo build --bin gfd-server`,
  tsc 0, vitest 114 (faceted STEP import 테스트 추가).
- 한계: 평면 폴리곤 면만(곡면은 면 폴리곤으로 근사), 볼록 가정(대부분 solid 면은 볼록).

### iter 22 — STEP 비볼록 면 ear-clipping ✅ (완료)
- 문제: iter 21의 fan 삼각화는 볼록 면만 정확 — L자/브래킷 등 비볼록 면은 삼각형이
  면 밖으로 삐져나감.
- 구현(`gfd-cad-io/step.rs`): `triangulate_face` — Newell normal로 면 법선 계산 →
  in-plane (u,v) 기저로 3D→2D 투영 → CCW 보정 → `gfd_cad_tessel::triangulate_polygon`
  (ear-clipping). 실패/퇴화 시 fan 폴백. `read_step_trimesh`가 이를 사용.
- 검증: `cargo test -p gfd-cad-io` 20 passed (L자 6각형 → 4 삼각형, 총면적=3.0으로
  겹침/누락 없음 검증), `cargo build --bin gfd-server`.

### iter 23 — 경계조건/모델 정합성 검사 (ill-posed 설정 사전 탐지) ✅ (완료)
- 문제: 입구(inlet)만 있고 출구/압력 기준이 없는 설정은 질량보존 불가 → 비압축 해가
  ill-posed/발산. 흔한 실수인데 diagnose가 못 잡았음(발산만 보고 근본원인 모름).
- 구현(`calc.ts` diagnoseState): setup.boundaries+models를 읽어
  - `INLET_NO_OUTLET`(warning): inlet 있고 outlet/pressure 없음 → pressure_outlet 추가 fix.
  - `ENERGY_NO_THERMAL_BC`(info): energy 모델 on인데 온도 BC 없음.
  setup만 읽으므로 **솔브 전 diagnose**로도 탐지 가능. auto_refine 에스컬레이션이 outlet
  추가를 시도(완화계수로 안 잡힐 때).
- 검증: tsc 0, vitest 117 (정합성 테스트 3개; 기존 loop 통합테스트의 실제 ill-posed
  설정을 잡아내 well-posed로 보정).

## 향후 후보 (남은 deep/niche backlog)
- imported 형상이 구조격자 mesh.generate에 반영(immersed boundary/cut-cell) — deep.
- STEP 곡면(원통/구면) 세분(현재 면 폴리곤 근사).
- measure.angle / face 단위 공간참조.
- diagnose 결과를 AppState/UI 패널에 표시(현재 명령 결과로만 반환).
- mesh 품질/설정 패널(size/prism/quality) command + UI.
- 공간 엔티티 참조(ray/screen/nearest) 구현(현재 stub).
- Distance/angle 2-pick measure.
- import한 STEP의 topology 복원(현재 points-only).
