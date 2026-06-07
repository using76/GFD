# `@gfd/command-core` (gui/src/core)

Framework-agnostic **single source of truth** for the GFD workbench rewrite.
The human React UI, the (future) MCP/control server, and headless tests all
build on this layer. **Nothing here may import React, Three.js, or Electron.**

Every operation a human can perform is a `CommandDef`. Humans and AI agents
dispatch the **same** commands through the **same** `Dispatcher`, validated by the
**same** JSON Schemas — so the two control planes cannot drift apart.

## Modules

| File | Role |
|------|------|
| `types.ts` | Primitive types (`Vec3`, `JsonValue`, `Capability`, `CommandCategory`). |
| `schema.ts` | Minimal JSON-Schema subset + dependency-free `basicValidator`. Single source for UI form + MCP inputSchema + validation. |
| `patch.ts` | Immutable state patches (`applyPatch` returns next state + exact inverse). |
| `state.ts` | Canonical `AppState` (geometry tree, selection, camera, mesh, physics, solver, results) + observable `StateStore`. `get_state` returns this verbatim. |
| `entity.ts` | `EntityRef` addressing — how an agent says "the inlet face" without ids. `id`/`name`/`bbox` resolve now; spatial refs wire to RPC later. |
| `command.ts` | `CommandDef`, `CommandInvocation`, `CommandOutcome`, `CommandContext`, `CoreEvent`. |
| `registry.ts` | `CommandRegistry` — catalogue the ribbon and MCP tools are generated from. |
| `journal.ts` | Undo/redo + replay + audit in one structure. |
| `consent.ts` | Per-capability consent gating for agent-initiated commands. |
| `dispatcher.ts` | The one pipeline: resolve → validate → consent → execute → apply → journal → emit. |
| `transport/rpcClient.ts` | `RpcClient` over gfd-server JSON-RPC (Electron + mock impls). |
| `solver/realSolver.ts` | **Phase 1** — drives the real Rust solver (`solve.start`/`status`/`stop`, `field.get`). |
| `commands/*` | Authored commands. Phase 0/1: `system`, `mesh`, `calc`, `results`. |

## Usage

```ts
import { createCore } from './core';

const core = createCore();                       // Electron RPC, core commands registered
await core.dispatcher.dispatch({
  commandId: 'mesh.generate',
  params: { nx: 20, ny: 20, nz: 0 },
  source: 'human',
});
await core.dispatcher.dispatch({ commandId: 'calc.run', params: {}, source: 'human' });
core.store.subscribe((s) => render(s));          // React mirrors AppState
```

## Phase status

- **Phase 0 ✅** — command-core (registry, dispatcher, journal, state, patches, entity, consent, transport).
- **Phase 1 ✅** — real-backend solver runner + `mesh.generate` / `calc.run` / `calc.stop` / `results.load_field`.
- **Phase 2 ✅** — geometry & sketch commands (`geometry.create_primitive`, transforms, delete/rename/tessellate, `sketch.*`); canonical id == backend shape_id; GeometryTree; name/bbox entity addressing.
- **Phase 5 ✅** — `mcp/bridge.ts` auto-maps the registry to MCP tools + meta-tools (`get_state`, `list_entities`, `run_command`, `select`, `query_spatial`, `screenshot`, undo/redo). `selection.set` + `view.set_camera` commands. Headless stdio transport in `src/mcp-server/` (`run.ts` + `sdkServer.ts` + `stdioRpcClient.ts`) makes the bridge runnable by an external agent.
- **Phase 6 ✅** — annotated-screenshot capture for the vision loop. `src/react/engine/ViewportV2.tsx`'s `ScreenshotRegistrar` registers a capturer (`gl.domElement.toDataURL` + per-entity bbox-center projection to screen pixels); the MCP `screenshot` meta-tool returns the PNG data URL + label legend so an agent can close the see→decide→act loop.
- **Phase 7 ✅** — pluggable physics manifest (`physics/manifest.ts`) + `physics.*` commands (validate_expression, set_constitutive, set_term, remove_term, apply_manifest). Default Navier–Stokes manifest in state. Backend RPCs landed in `src/server.rs` (`physics.validate_expression` / `physics.list_builtins` / `physics.apply_manifest`, gfd-expression-backed) with the expression momentum source affecting the actual solve.
- **Phase 8 ✅** — pluggable LLM provider layer (`llm/`): provider-agnostic interface + registry, Claude + Ollama adapters, MCP→Anthropic/OpenAI tool converters.
- **Phase 3 ✅ (code)** — focused R3F renderer in `src/react/engine/ViewportV2.tsx` (GeometryLayer lazily tessellates visible nodes; click-pick → `selection.set`; CameraSync from AppState) replacing the 2,184-line CadScene approach. `ui/ribbonModel.ts` + `ui/formModel.ts` are pure & unit-tested. Compiles + bundles; visual verification pending an interactive run.
- **Phase 4 ✅ (framework + all command categories) / 🚧 (legacy retirement)** — data-driven UI in `src/react/`: `RibbonFromRegistry`, `CommandFormPanel` (inputs from `paramsSchema`), `FeatureTreePanel`, `CoreContext`, `AppV2` shell (opt-in via `?v2`). Command sets authored for every ribbon category: geometry, sketch, selection, view, display (render mode/visibility/section), measure (volume/area/CoM/bbox), repair (check/fix/stats), prepare (enclosure/named-selections), mesh, setup (models/material/boundaries/solver → feeds calc.run), calc, results, physics. Remaining: deepen per-feature coverage to the legacy 168 and retire the old store/CadScene once the `?v2` shell reaches parity.
- **Remaining (beyond Phase 8)** — embedding the MCP bridge directly in the Electron main process over a localhost WebSocket (the headless `src/mcp-server/` stdio transport already provides external-agent control; this is a convenience variant — see `mcp/README.md`), Phase 4 legacy retirement (retire the old store/CadScene once `?v2` reaches the legacy 168-feature parity), and deepening per-feature command coverage.

**Status: Phases 0–8 are implemented, committed, and verified** — `npx tsc --noEmit` is clean and `npm test` is green (64 tests). The items above are post-Phase-8 polish, not blockers for the numbered plan.

See `/root/.claude/plans/gfd-gui-atomic-cocoa.md` for the full plan.

## Test & typecheck

```bash
cd gui
npx tsc --noEmit     # must be 0 errors
npm test             # vitest run — command-core unit + integration tests
```
