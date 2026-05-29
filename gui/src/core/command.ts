/**
 * The Command model — the single unit of action in GFD.
 *
 * EVERY operation a human can perform (create a primitive, edit a parameter, set
 * a boundary condition, generate a mesh, run the solver, change the view, select
 * an entity, edit a physics term) is a CommandDef. The human UI and external AI
 * agents both dispatch the SAME commands, so they can never drift apart.
 *
 * A CommandDef's `paramsSchema` is the single source for: the UI form, the MCP
 * tool inputSchema, and dispatcher validation.
 */

import type { Capability, CommandCategory, CommandSource, JsonObject } from './types';
import type { JsonSchema } from './schema';
import type { PatchOp } from './patch';
import type { AppState } from './state';
import type { EntityResolver } from './entity';
import type { RpcClient } from './transport/rpcClient';

/** Events emitted during/after command execution (progress, logs, etc.). */
export type CoreEvent =
  | { type: 'command-completed'; commandId: string; ok: boolean; source: CommandSource }
  | { type: 'log'; level: 'info' | 'warn' | 'error'; message: string }
  | { type: 'progress'; commandId: string; fraction: number }
  | { type: 'state-changed'; revision: number }
  | { type: 'solver-residual'; jobId: string; iteration: number; residual: number; elapsedMs: number }
  | { type: 'solver-done'; jobId: string; status: string; iteration: number }
  | { type: 'solver-error'; jobId: string | null; message: string };

export interface CommandContext {
  getState(): Readonly<AppState>;
  rpc: RpcClient;
  resolveEntity: EntityResolver['resolve'];
  emit(event: CoreEvent): void;
  /**
   * Apply a state patch OUTSIDE the journal. For long-running / streaming
   * commands (e.g. the solver) that update state asynchronously after `run`
   * has already returned. These mutations are intentionally not undoable.
   */
  update(ops: PatchOp[]): void;
  /** Aborted when the user cancels or stops the agent. */
  signal?: AbortSignal;
}

export interface CommandOutcome<R = unknown> {
  ok: boolean;
  result?: R;
  error?: { code: string; message: string };
  /** Patches the dispatcher applies to the canonical AppState. */
  statePatch?: PatchOp[];
}

export interface CommandInvocation {
  commandId: string;
  /** Wire-format params (validated against the command's paramsSchema). */
  params: JsonObject;
  source: CommandSource;
  meta?: { agentSessionId?: string; correlationId?: string };
}

export interface CommandDef<P = JsonObject, R = unknown> {
  /** Stable dotted id, e.g. "geometry.create_primitive". */
  id: string;
  category: CommandCategory;
  /** Optional ribbon group within the category's tab. */
  group?: string;
  title: string;
  titleKo?: string;
  /** Becomes the MCP tool description, so write it for an agent audience. */
  description: string;
  /** Single source of truth for UI form + MCP inputSchema + validation. */
  paramsSchema: JsonSchema;
  resultSchema?: JsonSchema;
  capability: Capability;
  /** Whether to expose this as an MCP tool (default true). */
  exposeToAgent?: boolean;
  /** Whether the command participates in undo/redo (default: inferred from capability). */
  undoable?: boolean;
  run(params: P, ctx: CommandContext): Promise<CommandOutcome<R>>;
}

/** Reasonable default for whether a capability should be undoable. */
export function isUndoableByDefault(capability: Capability): boolean {
  return (
    capability === 'mutate-scene' ||
    capability === 'mutate-physics' ||
    capability === 'destructive'
  );
}
