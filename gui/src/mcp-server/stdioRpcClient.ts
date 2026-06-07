/**
 * Node RpcClient that talks to the gfd-server binary over stdin/stdout
 * (line-delimited JSON-RPC). Used by the headless MCP server so an external AI
 * agent can drive the full backend without the Electron renderer.
 */

import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import type { JsonObject } from '../core';
import type { RpcClient } from '../core';

interface Pending {
  resolve: (value: unknown) => void;
  reject: (err: Error) => void;
}

export interface StdioRpcClient extends RpcClient {
  dispose(): void;
}

export function createStdioRpcClient(binaryPath: string): StdioRpcClient {
  const child = spawn(binaryPath, [], { stdio: ['pipe', 'pipe', 'inherit'] });
  const pending = new Map<number, Pending>();
  let nextId = 1;
  let alive = true;

  child.on('exit', () => {
    alive = false;
    for (const p of pending.values()) p.reject(new Error('gfd-server exited'));
    pending.clear();
  });

  if (child.stdout) {
    const rl = createInterface({ input: child.stdout });
    rl.on('line', (line: string) => {
      const trimmed = line.trim();
      if (!trimmed) return;
      let msg: { id?: number; result?: unknown; error?: string | null };
      try {
        msg = JSON.parse(trimmed);
      } catch {
        return; // non-JSON log line on stdout — ignore
      }
      if (typeof msg.id !== 'number') return;
      const p = pending.get(msg.id);
      if (!p) return;
      pending.delete(msg.id);
      if (msg.error) p.reject(new Error(msg.error));
      else p.resolve(msg.result);
    });
  }

  return {
    isLive: () => alive,
    request<T>(method: string, params?: JsonObject): Promise<T> {
      const id = nextId++;
      return new Promise<T>((resolve, reject) => {
        if (!alive || !child.stdin) {
          reject(new Error('gfd-server is not running'));
          return;
        }
        pending.set(id, { resolve: (v) => resolve(v as T), reject });
        child.stdin.write(`${JSON.stringify({ id, method, params })}\n`);
      });
    },
    dispose() {
      alive = false;
      child.kill();
    },
  };
}
