# Headless MCP server (Phase 5 transport)

Lets an external AI agent (e.g. Claude Code) drive the full GFD backend over MCP:

```
external agent ──MCP stdio──▶ run.ts ──▶ command-core bridge (auto-mapped tools)
                                     ──▶ StdioRpcClient ──▶ gfd-server (real solver/CAD)
```

- `sdkServer.ts` — wraps `createMcpBridge(core)` with `@modelcontextprotocol/sdk`
  `Server` + `StdioServerTransport`. `ListTools` → every command + meta-tools;
  `CallTool` → dispatched with `source:'agent'` (consent-gated + journaled).
- `stdioRpcClient.ts` — spawns `gfd-server` and speaks line-delimited JSON-RPC.
- `run.ts` — entrypoint. `GFD_SERVER_BIN` overrides the binary path.

## Run

```bash
cargo build --release --bin gfd-server          # from repo root
cd gui && npx vite build --config <node-config>  # or tsx src/mcp-server/run.ts
GFD_SERVER_BIN=../target/release/gfd-server node dist/mcp-server.js
```

Then register it as an MCP server in your agent. The `screenshot` tool needs the
live R3F renderer (the `?v2` Electron app), so it is unavailable in this headless
server; every other tool (geometry / sketch / mesh / setup / calc / results /
measure / repair / prepare / physics / view / selection) works against the real
backend.

## Embedded (Electron) variant

To expose the *running app* (with screenshots) instead, host this MCP server in
the Electron main process and bridge `ListTools`/`CallTool` to the renderer's
`createMcpBridge(core)` over a localhost WebSocket (token-authenticated). The
bridge code is identical; only the transport differs.
