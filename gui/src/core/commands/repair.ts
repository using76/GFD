/**
 * Repair commands (Phase 4) — shape healing, backed by the real cad.heal.* RPCs.
 * repair.fix mutates the arena shape, so it bumps the node's tessellationRev to
 * force a re-render.
 */

import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';
import type { JsonValue } from '../types';
import type { PatchOp } from '../patch';

export interface RepairCheckParams {
  shape_id: string;
}

export const repairCheck: CommandDef<RepairCheckParams, { valid: boolean; issues: JsonValue }> = {
  id: 'repair.check',
  category: 'repair',
  group: 'Validate',
  title: 'Check Validity',
  titleKo: '유효성 검사',
  description: 'Check a shape for topological issues (open wires, small edges, duplicates, ...).',
  capability: 'read',
  paramsSchema: { type: 'object', properties: { shape_id: { type: 'string' } }, required: ['shape_id'] },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ valid: boolean; issues: JsonValue }>('cad.heal.check_validity', {
      shape_id: params.shape_id,
    });
    return { ok: true, result: r };
  },
};

export interface RepairFixParams {
  shape_id: string;
  tolerance?: number;
  sew?: boolean;
  fix_wires?: boolean;
  remove_small?: boolean;
  remove_duplicate_faces?: boolean;
}

export const repairFix: CommandDef<RepairFixParams, { log: string[] }> = {
  id: 'repair.fix',
  category: 'repair',
  group: 'Fix',
  title: 'Fix Shape',
  titleKo: '형상 수정',
  description: 'Heal a shape in place (sew vertices, close wires, remove small edges, dedup faces). Returns a log.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      shape_id: { type: 'string' },
      tolerance: { type: 'number', minimum: 0 },
      sew: { type: 'boolean' },
      fix_wires: { type: 'boolean' },
      remove_small: { type: 'boolean' },
      remove_duplicate_faces: { type: 'boolean' },
    },
    required: ['shape_id'],
  },
  async run(params, ctx) {
    const node = ctx.getState().doc.geometry.nodes[params.shape_id];
    if (!node) return { ok: false, error: { code: 'UNKNOWN_SHAPE', message: `No shape "${params.shape_id}"` } };
    const r = await ctx.rpc.request<{ log: string[] }>('cad.heal.fix', {
      shape_id: params.shape_id,
      tolerance: params.tolerance ?? 1e-7,
      sew: params.sew ?? true,
      fix_wires: params.fix_wires ?? false,
      remove_small: params.remove_small ?? true,
      remove_duplicate_faces: params.remove_duplicate_faces ?? false,
    });
    const patch: PatchOp[] = [
      { op: 'replace', path: ['doc', 'geometry', 'nodes', params.shape_id, 'tessellationRev'], value: node.tessellationRev + 1 },
    ];
    return { ok: true, result: r, statePatch: patch };
  },
};

export const repairStats: CommandDef<RepairCheckParams, JsonValue> = {
  id: 'repair.stats',
  category: 'repair',
  group: 'Validate',
  title: 'Shape Stats',
  titleKo: '형상 통계',
  description: 'Return topological counts (vertices/edges/wires/faces/shells/solids) for a shape.',
  capability: 'read',
  paramsSchema: { type: 'object', properties: { shape_id: { type: 'string' } }, required: ['shape_id'] },
  async run(params, ctx) {
    const r = await ctx.rpc.request<JsonValue>('cad.heal.stats', { shape_id: params.shape_id });
    return { ok: true, result: r };
  },
};

export function registerRepairCommands(registry: CommandRegistry): void {
  registry.register(repairCheck);
  registry.register(repairFix);
  registry.register(repairStats);
}
