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
| 10 | **문제 파악 → 재수정** | △ | `calc.diagnose`가 actionable fix 제안; `calc.sensitivity` 있음; 자동 적용+재실행 루프는 미구현 |

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

## 향후 후보 (backlog)
- 자동 재수정 루프 `calc.auto_refine` (diagnose→최상위 fix 적용→재실행, 폐루프 자동화).
- per-equation residual 분리(현재 단일 scalar) → AI 수렴 진단 정밀화.
- diagnose 결과를 AppState/UI 패널에 표시(현재 명령 결과로만 반환).
- mesh 품질/설정 패널(size/prism/quality) command + UI.
- 공간 엔티티 참조(ray/screen/nearest) 구현(현재 stub).
- Distance/angle 2-pick measure.
- import한 STEP의 topology 복원(현재 points-only).
