/**
 * MCP server adapter — wraps the framework-agnostic command-core MCP bridge with
 * the official @modelcontextprotocol/sdk Server over a stdio transport. An
 * external agent (e.g. Claude Code) launches this and gets every command as a
 * tool plus the meta-tools, all consent-gated and journaled by the dispatcher.
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { CallToolRequestSchema, ListToolsRequestSchema } from '@modelcontextprotocol/sdk/types.js';
import type { Core, JsonObject } from '../core';
import { createMcpBridge } from '../core';

export interface McpSdkServer {
  server: Server;
  connectStdio(): Promise<void>;
}

export function createMcpSdkServer(core: Core): McpSdkServer {
  const bridge = createMcpBridge(core);
  const server = new Server(
    { name: 'gfd-workbench', version: '0.1.0' },
    { capabilities: { tools: {} } }
  );

  server.setRequestHandler(ListToolsRequestSchema, async () => ({
    tools: bridge.listTools().map((t) => ({
      name: t.name,
      description: t.description,
      // The bridge's inputSchema is a JSON Schema object compatible with MCP.
      inputSchema: t.inputSchema as { type: 'object' } & Record<string, unknown>,
    })),
  }));

  server.setRequestHandler(CallToolRequestSchema, async (req) => {
    const name = req.params.name;
    const args = (req.params.arguments ?? {}) as JsonObject;
    const result = await bridge.callTool(name, args);
    return {
      content: [{ type: 'text' as const, text: JSON.stringify(result.content ?? null) }],
      isError: !result.ok,
    };
  });

  return {
    server,
    async connectStdio() {
      await server.connect(new StdioServerTransport());
    },
  };
}
