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

function bboxCenter(b: { min: Vec3; max: Vec3 }): Vec3 {
  return [(b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2];
}

function distance(a: Vec3, b: Vec3): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

/** Closest gap between two AABBs (0 if they overlap/touch). */
function bboxGap(a: { min: Vec3; max: Vec3 }, b: { min: Vec3; max: Vec3 }): number {
  let s = 0;
  for (let k = 0; k < 3; k++) {
    const sep = Math.max(0, a.min[k] - b.max[k], b.min[k] - a.max[k]);
    s += sep * sep;
  }
  return Math.sqrt(s);
}

export interface MeasureDistanceParams {
  a: string;
  b: string;
}

export interface MeasureDistanceResult {
  centerDistance: number;
  gap: number;
  centerA: Vec3;
  centerB: Vec3;
}

export const measureDistance: CommandDef<MeasureDistanceParams, MeasureDistanceResult> = {
  id: 'measure.distance',
  category: 'measure',
  group: 'Extent',
  title: 'Distance',
  titleKo: '거리',
  description:
    'Distance between two shapes (by id): center-to-center distance and the closest gap between their bounding boxes (0 if they overlap). Lets the AI check clearances and spacing.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: { a: { type: 'string' }, b: { type: 'string' } },
    required: ['a', 'b'],
  },
  async run(params, ctx) {
    const tree = ctx.getState().doc.geometry.nodes;
    const na = tree[params.a];
    const nb = tree[params.b];
    if (!na || !nb) {
      const missing = !na ? params.a : params.b;
      return { ok: false, error: { code: 'UNKNOWN_SHAPE', message: `No shape "${missing}"` } };
    }
    const centerA = bboxCenter(na.bbox);
    const centerB = bboxCenter(nb.bbox);
    return {
      ok: true,
      result: { centerDistance: distance(centerA, centerB), gap: bboxGap(na.bbox, nb.bbox), centerA, centerB },
    };
  },
};

export interface MeasureAngleParams {
  a: string;
  b?: string;
  direction?: [number, number, number];
}

export interface MeasureAngleResult {
  angle_deg: number;
  axis_a: number[];
  axis_b: number[];
  a: string;
  b: string;
}

export const measureAngle: CommandDef<MeasureAngleParams, MeasureAngleResult> = {
  id: 'measure.angle',
  category: 'measure',
  group: 'Extent',
  title: 'Angle',
  titleKo: '각도',
  description:
    "Angle (0–90°, orientation-free) between two shapes' principal axes, or between shape `a`'s principal axis and an explicit direction [x,y,z]. Works for imported meshes too (PCA on the tessellated vertices). Use it to check part alignment/orientation.",
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: {
      a: { type: 'string' },
      b: { type: 'string' },
      direction: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3 },
    },
    required: ['a'],
  },
  async run(params, ctx) {
    if (!params.b && !params.direction) {
      return { ok: false, error: { code: 'BAD_PARAMS', message: "need shape 'b' or a 'direction'" } };
    }
    const r = await ctx.rpc.request<MeasureAngleResult>('cad.measure.shape_angle', {
      a: params.a,
      ...(params.b ? { b: params.b } : {}),
      ...(params.direction ? { direction: params.direction } : {}),
    });
    return { ok: true, result: r };
  },
};

export function registerMeasureCommands(registry: CommandRegistry): void {
  registry.register(measureVolume);
  registry.register(measureSurfaceArea);
  registry.register(measureCenterOfMass);
  registry.register(measureBoundingBox);
  registry.register(measureDistance);
  registry.register(measureAngle);
}
