/**
 * The Dispatcher — the one code path every command flows through, whether it
 * comes from a human clicking a button or an AI agent calling an MCP tool.
 *
 * Pipeline: resolve → validate → consent → execute → apply → journal → emit.
 * Because human and agent share this pipeline (and the same registry/schemas),
 * the two control planes cannot diverge.
 */

import type { CommandContext, CommandInvocation, CommandOutcome, CoreEvent } from './command';
import { isUndoableByDefault } from './command';
import type { CommandRegistry } from './registry';
import type { StateStore } from './state';
import { Journal } from './journal';
import { ConsentController } from './consent';
import { basicValidator, type Validator } from './schema';
import { toPatchOps } from './patch';
import type { RpcClient } from './transport/rpcClient';
import type { EntityResolver } from './entity';

export interface DispatcherDeps {
  registry: CommandRegistry;
  store: StateStore;
  rpc: RpcClient;
  resolver: EntityResolver;
  journal?: Journal;
  consent?: ConsentController;
  validator?: Validator;
  onEvent?: (event: CoreEvent) => void;
}

export class Dispatcher {
  readonly journal: Journal;
  readonly consent: ConsentController;
  private registry: CommandRegistry;
  private store: StateStore;
  private rpc: RpcClient;
  private resolver: EntityResolver;
  private validator: Validator;
  private onEvent?: (event: CoreEvent) => void;

  constructor(deps: DispatcherDeps) {
    this.registry = deps.registry;
    this.store = deps.store;
    this.rpc = deps.rpc;
    this.resolver = deps.resolver;
    this.journal = deps.journal ?? new Journal();
    this.consent = deps.consent ?? new ConsentController();
    this.validator = deps.validator ?? basicValidator;
    this.onEvent = deps.onEvent;
  }

  async dispatch(invocation: CommandInvocation): Promise<CommandOutcome> {
    const def = this.registry.get(invocation.commandId);
    if (!def) {
      return this.fail('UNKNOWN_COMMAND', `No command "${invocation.commandId}"`);
    }

    // 1. Validate params against the same schema the UI/MCP use.
    const errors = this.validator.validate(def.paramsSchema, invocation.params);
    if (errors.length > 0) {
      return this.fail(
        'INVALID_PARAMS',
        `Invalid params for "${def.id}": ${errors.map((e) => `${e.path} ${e.message}`).join('; ')}`
      );
    }

    // 2. Consent gate (agent-initiated commands only).
    const allowed = await this.consent.authorize(invocation.source, {
      commandId: def.id,
      capability: def.capability,
      agentSessionId: invocation.meta?.agentSessionId,
    });
    if (!allowed) {
      return this.fail('CONSENT_DENIED', `Command "${def.id}" was not authorized for ${invocation.source}`);
    }

    // 3. Execute.
    const ctx: CommandContext = {
      getState: () => this.store.getState(),
      rpc: this.rpc,
      resolveEntity: (ref) => this.resolver.resolve(ref),
      emit: (event) => this.emit(event),
      update: (ops) => {
        if (ops.length === 0) return;
        this.store.applyOps(ops);
        this.emit({ type: 'state-changed', revision: this.store.getState().doc.revision });
      },
    };

    let outcome: CommandOutcome;
    try {
      outcome = await def.run(invocation.params, ctx);
    } catch (err) {
      outcome = {
        ok: false,
        error: { code: 'RUNTIME_ERROR', message: err instanceof Error ? err.message : String(err) },
      };
    }

    // 4. Apply state patch + journal.
    let inverse = undefined as ReturnType<StateStore['applyOps']> | undefined;
    if (outcome.ok && outcome.statePatch && outcome.statePatch.length > 0) {
      inverse = this.store.applyOps(outcome.statePatch);
    }

    const undoable = def.undoable ?? isUndoableByDefault(def.capability);
    this.journal.record({
      invocation,
      outcome,
      inverse: inverse ?? [],
      forward: outcome.statePatch ?? [],
      timestamp: Date.now(),
      undoable: undoable && !!outcome.ok && !!outcome.statePatch?.length,
    });

    this.emit({ type: 'command-completed', commandId: def.id, ok: outcome.ok, source: invocation.source });
    if (inverse) this.emit({ type: 'state-changed', revision: this.store.getState().doc.revision });

    return outcome;
  }

  async undo(): Promise<boolean> {
    const entry = this.journal.undo();
    if (!entry) return false;
    this.store.applyOps(toPatchOps(entry.inverse));
    this.emit({ type: 'state-changed', revision: this.store.getState().doc.revision });
    return true;
  }

  async redo(): Promise<boolean> {
    const entry = this.journal.redo();
    if (!entry) return false;
    this.store.applyOps(entry.forward);
    this.emit({ type: 'state-changed', revision: this.store.getState().doc.revision });
    return true;
  }

  private emit(event: CoreEvent): void {
    this.onEvent?.(event);
  }

  private fail(code: string, message: string): CommandOutcome {
    return { ok: false, error: { code, message } };
  }
}
