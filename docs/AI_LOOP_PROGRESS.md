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

## 향후 후보 (backlog)
- import한 STEP의 topology 복원(현재 points-only) — 백엔드.
- measure.angle (edge/face 방향 — 백엔드 필요).
- nearest/semantic의 face/edge 단위(백엔드 tessellation 기반).
- diagnose 결과의 residualsByEq를 UI 패널에 표시.
- diagnose 결과를 AppState/UI 패널에 표시(현재 명령 결과로만 반환).
- mesh 품질/설정 패널(size/prism/quality) command + UI.
- 공간 엔티티 참조(ray/screen/nearest) 구현(현재 stub).
- Distance/angle 2-pick measure.
- import한 STEP의 topology 복원(현재 points-only).
