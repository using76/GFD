/**
 * Headless MCP server entrypoint.
 *
 *   external agent ──MCP stdio──▶ this process ──▶ command-core bridge
 *                                              ──▶ StdioRpcClient ──▶ gfd-server
 *
 * Run:  node dist/mcp-server.js   (after building this entry)
 * Env:  GFD_SERVER_BIN — path to the gfd-server binary
 *       (default: ../target/release/gfd-server)
 *
 * Note: the `screenshot` tool needs the live R3F renderer and is unavailable in
 * this headless server; everything else (geometry, mesh, solve, measure, repair,
 * physics, ...) works against the real backend.
 */

import { createCore } from '../core';
import { createStdioRpcClient } from './stdioRpcClient';
import { createMcpSdkServer } from './sdkServer';

async function main(): Promise<void> {
  const binary = process.env.GFD_SERVER_BIN ?? '../target/release/gfd-server';
  const rpc = createStdioRpcClient(binary);
  const core = createCore({ rpc });
  const { connectStdio } = createMcpSdkServer(core);
  await connectStdio();
}

main().catch((err: unknown) => {
  process.stderr.write(`gfd mcp-server failed: ${err instanceof Error ? err.message : String(err)}\n`);
  process.exit(1);
});
