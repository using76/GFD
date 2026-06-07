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

export interface ContourParams {
  field?: string;
  colormap?: 'jet' | 'rainbow' | 'grayscale' | 'coolwarm';
}

export interface ContourResult {
  vertices: number[];
  colors: number[];
}

export const resultsContour: CommandDef<ContourParams, ContourResult> = {
  id: 'results.contour',
  category: 'results',
  group: 'Visualize',
  title: 'Contour',
  titleKo: '컨투어',
  description:
    'Build a colored boundary-surface contour of a solved field (vertices + per-vertex RGB) for the viewport. Sets the active field.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: {
      field: { type: 'string' },
      colormap: { type: 'string', enum: ['jet', 'rainbow', 'grayscale', 'coolwarm'] },
    },
  },
  async run(params, ctx) {
    const field = params.field ?? ctx.getState().results?.activeField ?? 'velocity_magnitude';
    const r = await ctx.rpc.request<ContourResult>('field.contour', {
      field,
      colormap: params.colormap ?? 'jet',
    });
    const patch: PatchOp[] = ctx.getState().results
      ? [{ op: 'replace', path: ['results', 'activeField'], value: field }]
      : [];
    return { ok: true, result: r, statePatch: patch };
  },
};

export function registerResultsCommands(registry: CommandRegistry): void {
  registry.register(resultsLoadField);
  registry.register(resultsContour);
}
