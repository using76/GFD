/**
 * GFD command-core — public surface.
 *
 * Framework-agnostic single source of truth. The React UI, the MCP/control
 * server, and headless tests all build on this. Nothing here imports React,
 * Three.js, or Electron.
 */

export * from './types';
export * from './schema';
export * from './patch';
export * from './state';
export * from './physics/manifest';
export * from './entity';
export * from './command';
export * from './registry';
export * from './journal';
export * from './consent';
export * from './dispatcher';
export * from './transport/rpcClient';
export * from './solver/realSolver';
export * from './mcp';
export * from './llm';

import { CommandRegistry } from './registry';
import { StateStore, createInitialState, type AppState } from './state';
import { Dispatcher } from './dispatcher';
import { ConsentController, type ConsentPolicy } from './consent';
import { createEntityResolver } from './entity';
import { createElectronRpcClient, type RpcClient } from './transport/rpcClient';
import { registerCoreCommands } from './commands';
import type { CoreEvent } from './command';

export * from './commands';

export interface Core {
  registry: CommandRegistry;
  store: StateStore;
  dispatcher: Dispatcher;
  rpc: RpcClient;
}

export interface CreateCoreOptions {
  rpc?: RpcClient;
  initialState?: AppState;
  consentPolicy?: ConsentPolicy;
  onEvent?: (event: CoreEvent) => void;
  /** Register the built-in command catalogue (default true). */
  registerCommands?: boolean;
}

/** Assemble a ready-to-use core. The default RPC targets the Electron backend. */
export function createCore(options: CreateCoreOptions = {}): Core {
  const registry = new CommandRegistry();
  if (options.registerCommands !== false) {
    registerCoreCommands(registry);
  }
  const store = new StateStore(options.initialState ?? createInitialState());
  const rpc = options.rpc ?? createElectronRpcClient();
  const resolver = createEntityResolver(() => store.getState(), rpc);
  const consent = new ConsentController(options.consentPolicy);
  const dispatcher = new Dispatcher({ registry, store, rpc, resolver, consent, onEvent: options.onEvent });
  return { registry, store, dispatcher, rpc };
}
