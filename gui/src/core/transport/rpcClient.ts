/**
 * Transport abstraction over the gfd-server JSON-RPC backend.
 *
 * Every command and the solver runner talk to the backend exclusively through
 * this interface, so they are trivially testable with a mock and never reach
 * for `window` directly.
 *
 * The Electron implementation forwards to `window.gfdAPI.sendRequest`, which the
 * preload bridge pipes to the gfd-server process over stdin/stdout.
 */

import type { JsonObject } from '../types';

export interface RpcClient {
  request<T = unknown>(method: string, params?: JsonObject): Promise<T>;
  /** True when a real backend is reachable (Electron with gfd-server spawned). */
  isLive(): boolean;
}

/** Thrown when an RPC is attempted but no backend is available. */
export class NoBackendError extends Error {
  constructor(method: string) {
    super(`No GFD backend available for "${method}" (running in browser?)`);
    this.name = 'NoBackendError';
  }
}

/**
 * RpcClient backed by the Electron preload bridge (`window.gfdAPI`).
 * `isLive()` is false in the browser, letting callers fall back to simulation.
 */
export function createElectronRpcClient(): RpcClient {
  return {
    isLive(): boolean {
      return typeof window !== 'undefined' && !!window.gfdAPI;
    },
    async request<T>(method: string, params?: JsonObject): Promise<T> {
      if (typeof window === 'undefined' || !window.gfdAPI) {
        throw new NoBackendError(method);
      }
      const res = await window.gfdAPI.sendRequest(method, params);
      // The Electron main process resolves backend failures (and timeouts) as
      // `{ error }`. Surface those as a thrown error so the command fails cleanly
      // instead of building corrupt state (e.g. a node with an undefined id).
      if (res && typeof res === 'object' && 'error' in res && (res as { error?: unknown }).error) {
        throw new Error(`RPC "${method}" failed: ${String((res as { error?: unknown }).error)}`);
      }
      return res as T;
    },
  };
}

/** An RpcClient driven by a user-supplied handler — used in tests and headless runs. */
export function createMockRpcClient(
  handler: (method: string, params: JsonObject) => unknown | Promise<unknown>,
  opts: { live?: boolean } = {}
): RpcClient {
  return {
    isLive: () => opts.live ?? true,
    async request<T>(method: string, params?: JsonObject): Promise<T> {
      return (await handler(method, params ?? {})) as T;
    },
  };
}
