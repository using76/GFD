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
| 9 | **결과 분석** | △ | 단일 residual만; 진단/분석 명령 부족 |
| 10 | **문제 파악 → 재수정** | △ | `calc.sensitivity` 있음; 자동 진단→제안 루프 부족 |

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

## 향후 후보 (backlog)
- 결과 진단 명령 `results.diagnose` (발산/재순환/수렴품질/y+ 등 자동 분석) → AI가 읽음.
- per-equation residual 분리(현재 단일 scalar) → AI 수렴 진단.
- 자동 재수정 루프 `calc.auto_refine` (분석→파라미터 제안→재실행).
- mesh 품질/설정 패널(size/prism/quality) command + UI.
- 공간 엔티티 참조(ray/screen/nearest) 구현(현재 stub).
- Distance/angle 2-pick measure.
- import한 STEP의 topology 복원(현재 points-only).
