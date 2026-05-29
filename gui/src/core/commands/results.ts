/**
 * Results commands — fetch solved field data from the backend on demand.
 *
 * Field VALUE arrays are deliberately kept OUT of AppState (large, not useful to
 * serialize for `get_state`); this command returns them to whoever needs them
 * (the renderer, or an AI agent inspecting a probe).
 */

import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';
import type { PatchOp } from '../patch';

export interface FieldGetParams {
  field: string;
}

export interface FieldGetResult {
  name: string;
  values: number[];
  min: number;
  max: number;
  mean: number;
}

export const resultsLoadField: CommandDef<FieldGetParams, FieldGetResult> = {
  id: 'results.load_field',
  category: 'results',
  group: 'Fields',
  title: 'Load Field',
  titleKo: '필드 불러오기',
  description: 'Fetch a solved scalar field (values + min/max/mean) by name and make it the active field.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: { field: { type: 'string' } },
    required: ['field'],
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ values: number[]; min: number; max: number; mean: number }>('field.get', {
      field: params.field,
    });
    const result: FieldGetResult = { name: params.field, ...r };

    const patch: PatchOp[] = ctx.getState().results
      ? [{ op: 'replace', path: ['results', 'activeField'], value: params.field }]
      : [];
    return { ok: true, result, statePatch: patch };
  },
};

export function registerResultsCommands(registry: CommandRegistry): void {
  registry.register(resultsLoadField);
}
