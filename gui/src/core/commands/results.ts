/**
 * Results commands — fetch solved field data from the backend on demand.
 *
 * Field VALUE arrays are deliberately kept OUT of AppState (large, not useful to
 * serialize for `get_state`); this command returns them to whoever needs them
 * (the renderer, or an AI agent inspecting a probe).
 */

import type { JsonValue } from '../types';
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

export interface DerivedFieldResult {
  field: string;
  min: number;
  max: number;
  mean: number;
}

/** A backend-computed derived field (vorticity, Q-criterion, …) that registers
 *  itself as a selectable, activatable field for contour/isosurface. */
function makeDerivedFieldCommand(
  id: string,
  method: string,
  title: string,
  titleKo: string,
  description: string
): CommandDef<Record<string, never>, DerivedFieldResult> {
  return {
    id,
    category: 'results',
    group: 'Fields',
    title,
    titleKo,
    description,
    capability: 'read',
    paramsSchema: { type: 'object', properties: {} },
    async run(_params, ctx) {
      const r = await ctx.rpc.request<DerivedFieldResult>(method, {});
      const results = ctx.getState().results;
      const patch: PatchOp[] = [];
      if (results) {
        const availableFields = results.availableFields.includes(r.field)
          ? results.availableFields
          : [...results.availableFields, r.field];
        const fieldStats = { ...results.fieldStats, [r.field]: { min: r.min, max: r.max, mean: r.mean } };
        patch.push({ op: 'replace', path: ['results', 'availableFields'], value: availableFields });
        patch.push({ op: 'replace', path: ['results', 'fieldStats'], value: fieldStats as unknown as JsonValue });
        patch.push({ op: 'replace', path: ['results', 'activeField'], value: r.field });
      }
      return { ok: true, result: r, statePatch: patch };
    },
  };
}

export const resultsVorticity = makeDerivedFieldCommand(
  'results.vorticity',
  'field.vorticity',
  'Compute Vorticity',
  '와도 계산',
  'Compute the vorticity magnitude |∇×u| of the solved velocity field and register it as a selectable field (use it with contour or isosurface to see vortices).'
);

export const resultsQCriterion = makeDerivedFieldCommand(
  'results.qcriterion',
  'field.qcriterion',
  'Compute Q-criterion',
  'Q-기준 계산',
  'Compute the Q-criterion ½(‖Ω‖²−‖S‖²) of the solved velocity field and register it as a selectable field. Q>0 isosurfaces mark vortex cores — the standard vortex-identification method.'
);

export interface IsosurfaceParams {
  field?: string;
  isovalue?: number;
}

export interface IsosurfaceResult {
  positions: number[];
  triangle_count: number;
  isovalue: number;
  field: string;
}

export const resultsIsosurface: CommandDef<IsosurfaceParams, IsosurfaceResult> = {
  id: 'results.isosurface',
  category: 'results',
  group: 'Visualize',
  title: 'Isosurface',
  titleKo: '등치면',
  description:
    'Extract an isosurface (marching tetrahedra) of a solved scalar field at an isovalue (default: the field mean). Returns a triangle mesh for the viewport — the standard way to see a 3D field level set.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: { field: { type: 'string' }, isovalue: { type: 'number' } },
  },
  async run(params, ctx) {
    const field = params.field ?? ctx.getState().results?.activeField ?? 'velocity_magnitude';
    const r = await ctx.rpc.request<IsosurfaceResult>('field.isosurface', {
      field,
      ...(params.isovalue !== undefined ? { isovalue: params.isovalue } : {}),
    });
    return { ok: true, result: r };
  },
};

export interface VectorsResult {
  origins: number[];
  vectors: number[];
  max_magnitude: number;
  count: number;
}

export const resultsVectors: CommandDef<{ stride?: number }, VectorsResult> = {
  id: 'results.vectors',
  category: 'results',
  group: 'Visualize',
  title: 'Vectors',
  titleKo: '벡터',
  description: 'Fetch per-cell velocity glyphs (origins + vectors) for vector visualization.',
  capability: 'read',
  paramsSchema: { type: 'object', properties: { stride: { type: 'integer', minimum: 1 } } },
  async run(params, ctx) {
    const r = await ctx.rpc.request<VectorsResult>('field.vectors', { stride: params.stride ?? 4 });
    return { ok: true, result: r };
  },
};

export interface StreamlinesResult {
  lines: number[][];
  count: number;
}

export const resultsStreamlines: CommandDef<{ n_seeds?: number; max_steps?: number }, StreamlinesResult> = {
  id: 'results.streamlines',
  category: 'results',
  group: 'Visualize',
  title: 'Streamlines',
  titleKo: '유선',
  description: 'Integrate streamlines (RK2) through the solved velocity field and return polylines.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: { n_seeds: { type: 'integer', minimum: 1 }, max_steps: { type: 'integer', minimum: 2 } },
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<StreamlinesResult>('field.streamlines', {
      n_seeds: params.n_seeds ?? 20,
      max_steps: params.max_steps ?? 200,
    });
    return { ok: true, result: r };
  },
};

export interface SetVizParams {
  showContour?: boolean;
  showVectors?: boolean;
  vectorScale?: number;
  vectorStride?: number;
  showStreamlines?: boolean;
  streamlineSeeds?: number;
  streamlineSteps?: number;
  showIsosurface?: boolean;
  isovalue?: number;
}

export const resultsSetViz: CommandDef<SetVizParams, Record<string, never>> = {
  id: 'results.set_viz',
  category: 'results',
  group: 'Visualize',
  title: 'Visualization Options',
  titleKo: '시각화 옵션',
  description: 'Toggle/configure results visualization (contour/vectors/streamlines, scale, density, seeds).',
  capability: 'view-only',
  undoable: false,
  paramsSchema: {
    type: 'object',
    properties: {
      showContour: { type: 'boolean' },
      showVectors: { type: 'boolean' },
      vectorScale: { type: 'number', minimum: 0 },
      vectorStride: { type: 'integer', minimum: 1 },
      showStreamlines: { type: 'boolean' },
      streamlineSeeds: { type: 'integer', minimum: 1 },
      streamlineSteps: { type: 'integer', minimum: 2 },
      showIsosurface: { type: 'boolean' },
      isovalue: { type: 'number' },
    },
  },
  async run(params) {
    const patch: PatchOp[] = Object.entries(params).map(([key, value]) => ({
      op: 'replace',
      path: ['viz', key],
      value: value as JsonValue,
    }));
    return { ok: true, result: {}, statePatch: patch };
  },
};

export function registerResultsCommands(registry: CommandRegistry): void {
  registry.register(resultsLoadField);
  registry.register(resultsContour);
  registry.register(resultsVectors);
  registry.register(resultsStreamlines);
  registry.register(resultsIsosurface);
  registry.register(resultsVorticity);
  registry.register(resultsQCriterion);
  registry.register(resultsSetViz);
}
