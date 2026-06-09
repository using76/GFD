/**
 * MCP bridge (Phase 5) — the framework-agnostic core of the external control
 * surface. It auto-maps the command registry to MCP tools (so a new command
 * appears to AI agents with zero MCP maintenance) and adds a small set of
 * meta-tools that don't map 1:1 to a single command.
 *
 * A thin transport layer (stdio/WebSocket, in Electron main) wraps this bridge
 * with the MCP SDK — see mcp/README.md. This module has no SDK dependency so it
 * is fully unit-testable.
 *
 * Every command tool is dispatched with source:'agent', so the ConsentController
 * gates it and the journal records it for audit.
 */

import type { JsonObject, JsonValue } from '../types';
import type { JsonSchema } from '../schema';
import type { Core } from '../index';
import { createEntityResolver, type EntityRef, type EntityResolver } from '../entity';
import { runAutoRefine } from '../solver/autoRefine';

export interface McpToolSchema {
  name: string;
  description: string;
  inputSchema: JsonSchema;
}

export interface McpToolResult {
  ok: boolean;
  content: JsonValue;
  error?: string;
}

export interface McpBridge {
  listTools(): McpToolSchema[];
  callTool(name: string, args: JsonObject): Promise<McpToolResult>;
}

/** MCP tool names disallow dots; map command ids to a safe form and back. */
function toToolName(commandId: string): string {
  return commandId.replace(/\./g, '__');
}
function toCommandId(toolName: string): string {
  return toolName.replace(/__/g, '.');
}

const EMPTY_SCHEMA: JsonSchema = { type: 'object', properties: {} };

export function createMcpBridge(core: Core): McpBridge {
  const resolver: EntityResolver = createEntityResolver(() => core.store.getState(), core.rpc);

  const ok = (content: JsonValue): McpToolResult => ({ ok: true, content });
  const fail = (error: string): McpToolResult => ({ ok: false, content: null, error });

  async function resolveRefs(refs: EntityRef[]): Promise<string[]> {
    const ids: string[] = [];
    for (const ref of refs) {
      const r = await resolver.resolve(ref);
      if (r) ids.push(...r.ids);
    }
    return ids;
  }

  const metaTools: Record<string, { schema: McpToolSchema; handler: (args: JsonObject) => Promise<McpToolResult> }> = {
    get_state: {
      schema: { name: 'get_state', description: 'Return the full serializable AppState (with doc.revision).', inputSchema: EMPTY_SCHEMA },
      handler: async () => ok(core.store.getState() as unknown as JsonValue),
    },
    get_state_summary: {
      schema: { name: 'get_state_summary', description: 'Token-cheap summary: entity counts/names, mesh, solver, results.', inputSchema: EMPTY_SCHEMA },
      handler: async () => {
        const s = core.store.getState();
        const diag = s.diagnosis as null | { summary?: unknown; issues?: Array<{ code: string; severity: string; message: string }> };
        return ok({
          revision: s.doc.revision,
          shapes: Object.values(s.doc.geometry.nodes).map((n) => ({ id: n.id, name: n.name, kind: n.kind, visible: n.visible })),
          selection: s.selection as unknown as JsonValue,
          mesh: s.mesh ? { cells: s.mesh.cellCount, nodes: s.mesh.nodeCount, badCells: s.mesh.badCells ?? 0 } : null,
          solver: { status: s.solver.status, iteration: s.solver.iteration, residual: s.solver.residual },
          results: s.results ? { fields: s.results.availableFields, active: s.results.activeField } : null,
          diagnosis: diag
            ? {
                summary: diag.summary ?? null,
                issues: (diag.issues ?? []).map((i) => ({ code: i.code, severity: i.severity, message: i.message })),
              }
            : null,
        } as JsonValue);
      },
    },
    list_entities: {
      schema: { name: 'list_entities', description: 'List geometry entities with ids, semantic names, kinds, and bounding boxes.', inputSchema: EMPTY_SCHEMA },
      handler: async () => {
        const nodes = Object.values(core.store.getState().doc.geometry.nodes);
        return ok(nodes.map((n) => ({ id: n.id, name: n.name, kind: n.kind, bbox: n.bbox, visible: n.visible })) as unknown as JsonValue);
      },
    },
    run_command: {
      schema: {
        name: 'run_command',
        description: 'Escape hatch: dispatch any registered command by id with params.',
        inputSchema: {
          type: 'object',
          properties: { commandId: { type: 'string' }, params: { type: 'object' } },
          required: ['commandId'],
        },
      },
      handler: async (args) => {
        const commandId = args.commandId as string;
        const params = (args.params as JsonObject) ?? {};
        const outcome = await core.dispatcher.dispatch({ commandId, params, source: 'agent' });
        return outcome.ok ? ok((outcome.result ?? null) as JsonValue) : fail(outcome.error?.message ?? 'command failed');
      },
    },
    query_spatial: {
      schema: {
        name: 'query_spatial',
        description: 'Resolve an entity reference (id/name/bbox/ray/screen/nearest/semantic) to concrete entity ids.',
        inputSchema: { type: 'object', properties: { ref: { type: 'object' } }, required: ['ref'] },
      },
      handler: async (args) => {
        const resolved = await resolver.resolve(args.ref as unknown as EntityRef);
        return resolved ? ok(resolved as unknown as JsonValue) : fail('reference did not resolve');
      },
    },
    select: {
      schema: {
        name: 'select',
        description: 'Select entities by reference list (id/name/bbox/...) or explicit ids.',
        inputSchema: {
          type: 'object',
          properties: { refs: { type: 'array', items: { type: 'object' } }, ids: { type: 'array', items: { type: 'string' } } },
        },
      },
      handler: async (args) => {
        const ids = Array.isArray(args.ids)
          ? (args.ids as string[])
          : await resolveRefs((args.refs as unknown as EntityRef[]) ?? []);
        const outcome = await core.dispatcher.dispatch({ commandId: 'selection.set', params: { ids }, source: 'agent' });
        return outcome.ok ? ok((outcome.result ?? null) as JsonValue) : fail(outcome.error?.message ?? 'select failed');
      },
    },
    screenshot: {
      schema: {
        name: 'screenshot',
        description: 'Capture an annotated viewport PNG (data URL) plus a legend of entity ids → screen positions.',
        inputSchema: { type: 'object', properties: { width: { type: 'integer' }, height: { type: 'integer' } } },
      },
      handler: async (args) => {
        if (!core.screenshot.isAvailable()) {
          return fail('screenshot requires the renderer to be mounted (open the ?v2 workbench)');
        }
        try {
          const shot = await core.screenshot.capture({
            width: typeof args.width === 'number' ? args.width : undefined,
            height: typeof args.height === 'number' ? args.height : undefined,
          });
          return ok(shot as unknown as JsonValue);
        } catch (e) {
          return fail(e instanceof Error ? e.message : String(e));
        }
      },
    },
    auto_refine: {
      schema: {
        name: 'auto_refine',
        description:
          'Autonomous closed loop: diagnose the current solve, apply the highest-severity fix (relaxation / turbulence model / iterations), re-run the solver, and repeat up to maxRounds until healthy. Returns per-round history + final diagnosis. Requires a mesh + setup so calc.run can execute.',
        inputSchema: {
          type: 'object',
          properties: {
            maxRounds: { type: 'integer', minimum: 1, maximum: 10 },
            maxIterations: { type: 'integer', minimum: 1 },
          },
        },
      },
      handler: async (args) => {
        const result = await runAutoRefine(core, {
          maxRounds: typeof args.maxRounds === 'number' ? args.maxRounds : undefined,
          maxIterations: typeof args.maxIterations === 'number' ? args.maxIterations : undefined,
        });
        return ok(result as unknown as JsonValue);
      },
    },
    undo: {
      schema: { name: 'undo', description: 'Undo the last undoable command.', inputSchema: EMPTY_SCHEMA },
      handler: async () => ok({ undone: await core.dispatcher.undo() }),
    },
    redo: {
      schema: { name: 'redo', description: 'Redo the last undone command.', inputSchema: EMPTY_SCHEMA },
      handler: async () => ok({ redone: await core.dispatcher.redo() }),
    },
  };

  return {
    listTools() {
      const commandTools: McpToolSchema[] = core.registry.listAgentExposed().map((cmd) => ({
        name: toToolName(cmd.id),
        description: cmd.description,
        inputSchema: cmd.paramsSchema,
      }));
      const meta = Object.values(metaTools).map((m) => m.schema);
      return [...meta, ...commandTools];
    },

    async callTool(name, args) {
      const meta = metaTools[name];
      if (meta) return meta.handler(args);

      const commandId = toCommandId(name);
      if (core.registry.has(commandId)) {
        const outcome = await core.dispatcher.dispatch({ commandId, params: args, source: 'agent' });
        return outcome.ok ? ok((outcome.result ?? null) as JsonValue) : fail(outcome.error?.message ?? 'command failed');
      }
      return fail(`unknown tool: ${name}`);
    },
  };
}
