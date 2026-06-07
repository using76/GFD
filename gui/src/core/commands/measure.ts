/**
 * Measure commands (Phase 4) — exact analytic queries on shapes, backed by the
 * real cad.measure.* RPCs. Read-only; results are returned to the caller (UI HUD
 * or AI agent) and not stored in AppState.
 */

import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';
import type { Vec3 } from '../types';

export interface ShapeIdParams {
  shape_id: string;
}

const shapeIdSchema = {
  type: 'object' as const,
  properties: { shape_id: { type: 'string' as const } },
  required: ['shape_id'],
};

export const measureVolume: CommandDef<ShapeIdParams, { volume: number }> = {
  id: 'measure.volume',
  category: 'measure',
  group: 'Mass',
  title: 'Volume',
  titleKo: '부피',
  description: 'Compute the enclosed volume of a shape (divergence theorem).',
  capability: 'read',
  paramsSchema: shapeIdSchema,
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ volume: number }>('cad.measure.volume', { shape_id: params.shape_id });
    return { ok: true, result: r };
  },
};

export const measureSurfaceArea: CommandDef<ShapeIdParams, { area: number }> = {
  id: 'measure.surface_area',
  category: 'measure',
  group: 'Mass',
  title: 'Surface Area',
  titleKo: '표면적',
  description: 'Compute the total surface area of a shape.',
  capability: 'read',
  paramsSchema: shapeIdSchema,
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ area: number }>('cad.measure.surface_area', { shape_id: params.shape_id });
    return { ok: true, result: r };
  },
};

export const measureCenterOfMass: CommandDef<ShapeIdParams, { x: number; y: number; z: number }> = {
  id: 'measure.center_of_mass',
  category: 'measure',
  group: 'Mass',
  title: 'Center of Mass',
  titleKo: '질량 중심',
  description: 'Compute the center of mass of a shape.',
  capability: 'read',
  paramsSchema: shapeIdSchema,
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ x: number; y: number; z: number }>('cad.measure.center_of_mass', {
      shape_id: params.shape_id,
    });
    return { ok: true, result: r };
  },
};

export const measureBoundingBox: CommandDef<ShapeIdParams, { min: Vec3; max: Vec3 }> = {
  id: 'measure.bounding_box',
  category: 'measure',
  group: 'Extent',
  title: 'Bounding Box',
  titleKo: '경계 상자',
  description: "Return a shape's axis-aligned bounding box from the geometry tree.",
  capability: 'read',
  paramsSchema: shapeIdSchema,
  async run(params, ctx) {
    const node = ctx.getState().doc.geometry.nodes[params.shape_id];
    if (!node) return { ok: false, error: { code: 'UNKNOWN_SHAPE', message: `No shape "${params.shape_id}"` } };
    return { ok: true, result: { min: node.bbox.min, max: node.bbox.max } };
  },
};

export function registerMeasureCommands(registry: CommandRegistry): void {
  registry.register(measureVolume);
  registry.register(measureSurfaceArea);
  registry.register(measureCenterOfMass);
  registry.register(measureBoundingBox);
}
