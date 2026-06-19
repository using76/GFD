/**
 * Renderer half of the external MCP control channel. The Electron main process
 * relays {id, op, name?, args?} messages from an external MCP client (e.g. the
 * Claude Code CLI, via mcp-server.cjs) over `window.gfdMcp`; here we answer them
 * through the SAME command-core bridge the in-app chat uses — so the CLI drives
 * this running GUI identically, with every action reflected live in the viewport.
 */

import { useEffect } from 'react';
import { createMcpBridge, type JsonObject } from '../core';
import { useCore } from './CoreContext';

interface ControlMsg {
  id: number;
  op: 'listTools' | 'callTool' | string;
  name?: string;
  args?: JsonObject;
}

declare global {
  interface Window {
    gfdMcp?: {
      onControl: (cb: (msg: ControlMsg) => void) => () => void;
      sendResult: (id: number, result: unknown) => void;
    };
  }
}

export function McpControlResponder() {
  const core = useCore();
  useEffect(() => {
    const api = window.gfdMcp;
    if (!api) return; // not running under Electron (browser dev) → no control channel
    const bridge = createMcpBridge(core);
    const off = api.onControl(async (msg) => {
      try {
        if (msg.op === 'listTools') {
          api.sendResult(msg.id, { tools: bridge.listTools() });
        } else if (msg.op === 'callTool') {
          const result = await bridge.callTool(msg.name ?? '', msg.args ?? {});
          api.sendResult(msg.id, result);
        } else {
          api.sendResult(msg.id, { ok: false, content: null, error: `unknown op: ${msg.op}` });
        }
      } catch (e) {
        api.sendResult(msg.id, { ok: false, content: null, error: e instanceof Error ? e.message : String(e) });
      }
    });
    return off;
  }, [core]);
  return null;
}
