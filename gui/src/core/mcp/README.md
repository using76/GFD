# MCP control surface (Phase 5)

`bridge.ts` is the framework-agnostic heart of the external-agent control plane.
It auto-maps the command registry to MCP tools and adds meta-tools. It has **no
MCP SDK dependency** and is fully unit-tested (`__tests__/mcp.test.ts`).

## Tool surface

- **Command tools** — one per agent-exposed `CommandDef`, named `<category>__<id>`
  (dots → `__`), `inputSchema` = the command's `paramsSchema`. Dispatched with
  `source: 'agent'` (consent-gated + journaled).
- **Meta-tools** — `get_state`, `get_state_summary`, `list_entities`,
  `run_command`, `query_spatial`, `select`, `screenshot` (Phase 6),
  `undo`, `redo`.

## Wiring the real server (Electron main)

The bridge is wrapped by a transport in the Electron main process:

```
External agent ──MCP(stdio/WS)──▶ @modelcontextprotocol/sdk Server
                                       │  ListTools  → bridge.listTools()
                                       │  CallTool   → bridge.callTool(name,args)
                                       ▼
                                  WebSocket ──▶ renderer Dispatcher (one journal)
```

Steps to land the transport (kept out of this PR to avoid a hard SDK dep):
1. `npm i @modelcontextprotocol/sdk` (+ `ws` for the localhost control channel).
2. In `electron/main.js`, after spawning gfd-server, start a `ControlServer`
   (localhost WS, per-session token shown in the UI / written to a known file).
3. The renderer creates the `Core` + `createMcpBridge(core)` and answers control
   messages; main hosts the MCP `Server` and forwards `ListTools`/`CallTool` to
   the bridge over the WS.
4. Consent: agent capability gating is already enforced in the dispatcher; expose
   a "stop agent" toggle that flips the `ConsentController` policy to `read-only`.

Until then, `createMcpBridge(core)` can be driven directly (e.g. from tests or a
future in-process agent).
