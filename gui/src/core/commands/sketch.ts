/**
 * Sketch commands (Phase 2) — 2D constraint sketches via the CAD sketcher.
 *
 * Minimal surface to create a sketch, add line/circle entities, and solve it.
 * Sketch indices are tracked in AppState.doc.sketchIds so the UI/agent can list
 * them. Point creation (add_point) returns ids used by line/circle entities.
 */

import type { JsonValue } from '../types';
import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';
import type { PatchOp } from '../patch';

export const sketchNew: CommandDef<Record<string, never>, { sketch_idx: number }> = {
  id: 'sketch.new',
  category: 'geometry',
  group: 'Sketch',
  title: 'New Sketch',
  titleKo: '새 스케치',
  description: 'Create a new empty 2D sketch and return its index.',
  capability: 'mutate-scene',
  paramsSchema: { type: 'object', properties: {}, additionalProperties: false },
  async run(_params, ctx) {
    const resp = await ctx.rpc.request<{ sketch_idx: number }>('cad.sketch.new');
    const count = ctx.getState().doc.sketchIds.length;
    const patch: PatchOp[] = [
      { op: 'add', path: ['doc', 'sketchIds', count], value: resp.sketch_idx as JsonValue },
    ];
    return { ok: true, result: resp, statePatch: patch };
  },
};

export interface SketchAddLineParams {
  sketch_idx: number;
  a: number;
  b: number;
}

export const sketchAddLine: CommandDef<SketchAddLineParams, { entity_id: number }> = {
  id: 'sketch.add_line',
  category: 'geometry',
  group: 'Sketch',
  title: 'Add Line',
  titleKo: '선 추가',
  description: 'Add a line between two existing sketch point ids.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      sketch_idx: { type: 'integer', minimum: 0 },
      a: { type: 'integer', minimum: 0 },
      b: { type: 'integer', minimum: 0 },
    },
    required: ['sketch_idx', 'a', 'b'],
  },
  async run(params, ctx) {
    const resp = await ctx.rpc.request<{ entity_id: number }>('cad.sketch.add_line', {
      sketch_idx: params.sketch_idx,
      a: params.a,
      b: params.b,
    });
    return { ok: true, result: resp };
  },
};

export interface SketchAddCircleParams {
  sketch_idx: number;
  center: number;
  radius: number;
}

export const sketchAddCircle: CommandDef<SketchAddCircleParams, { entity_id: number }> = {
  id: 'sketch.add_circle',
  category: 'geometry',
  group: 'Sketch',
  title: 'Add Circle',
  titleKo: '원 추가',
  description: 'Add a circle at a sketch point with the given radius.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      sketch_idx: { type: 'integer', minimum: 0 },
      center: { type: 'integer', minimum: 0 },
      radius: { type: 'number', minimum: 0 },
    },
    required: ['sketch_idx', 'center', 'radius'],
  },
  async run(params, ctx) {
    const resp = await ctx.rpc.request<{ entity_id: number }>('cad.sketch.add_circle', {
      sketch_idx: params.sketch_idx,
      center: params.center,
      radius: params.radius,
    });
    return { ok: true, result: resp };
  },
};

export interface SketchSolveParams {
  sketch_idx: number;
  tolerance?: number;
  max_iters?: number;
}

export const sketchSolve: CommandDef<SketchSolveParams, { residual: number; points: number[][] }> = {
  id: 'sketch.solve',
  category: 'geometry',
  group: 'Sketch',
  title: 'Solve Sketch',
  titleKo: '스케치 해석',
  description: 'Solve the sketch constraint system and return the residual and resolved point positions.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      sketch_idx: { type: 'integer', minimum: 0 },
      tolerance: { type: 'number', minimum: 0 },
      max_iters: { type: 'integer', minimum: 1 },
    },
    required: ['sketch_idx'],
  },
  async run(params, ctx) {
    const resp = await ctx.rpc.request<{ residual: number; points: number[][] }>('cad.sketch.solve', {
      sketch_idx: params.sketch_idx,
      tolerance: params.tolerance ?? 1e-8,
      max_iters: params.max_iters ?? 100,
    });
    return { ok: true, result: resp };
  },
};

export function registerSketchCommands(registry: CommandRegistry): void {
  registry.register(sketchNew);
  registry.register(sketchAddLine);
  registry.register(sketchAddCircle);
  registry.register(sketchSolve);
}
