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
- **Phase 1 ✅** — real-backend solver runner + `mesh.generate` / `calc.run` / `calc.stop` / `results.load_field` commands wired to gfd-server. Simulation remains only in the legacy store's `!window.gfdAPI` branch until the UI migrates (Phase 2–4).
- **Next** — Phase 2 (command-ify geometry + unify ids), Phase 3 (split renderer), Phase 4 (data-driven ribbon/panels + port 168 features), Phase 5 (MCP/control server), Phase 6 (vision), Phase 7 (pluggable physics), Phase 8 (LLM providers).

See `/root/.claude/plans/gfd-gui-atomic-cocoa.md` for the full plan.

## Test & typecheck

```bash
cd gui
npx tsc --noEmit     # must be 0 errors
npm test             # vitest run — command-core unit + integration tests
```
