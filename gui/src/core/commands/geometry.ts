/**
 * Geometry commands (Phase 2) — AI-editable 2D/3D model via the CAD kernel.
 *
 * Each command wraps an existing cad.* RPC and maintains the GeometryTree in
 * AppState with canonical ids (id === backend shape_id), so an AI agent can
 * create, transform, rename, and delete shapes, and address them by id/name.
 *
 * The CAD feature ops (translate/rotate/scale/mirror) are functional on the
 * backend — they produce a NEW shape — so we add a new node (parentId = source)
 * and hide the source, modelling a simple feature lineage.
 */

import type { JsonObject, JsonValue, Vec3 } from '../types';
import type { CommandDef, CommandContext } from '../command';
import type { CommandRegistry } from '../registry';
import type { GeometryNode } from '../state';
import type { PatchOp } from '../patch';

interface PrimitiveResponse {
  shape_id: string;
  arena_id: number;
  kind: string;
}

function estimateBbox(kind: string, p: Record<string, number | undefined>): { min: Vec3; max: Vec3 } {
  const sym = (x: number, y: number, z: number): { min: Vec3; max: Vec3 } => ({
    min: [-x, -y, -z],
    max: [x, y, z],
  });
  switch (kind) {
    case 'box':
      return sym((p.lx ?? 1) / 2, (p.ly ?? 1) / 2, (p.lz ?? 1) / 2);
    case 'sphere':
      return sym(p.radius ?? 0.5, p.radius ?? 0.5, p.radius ?? 0.5);
    case 'cylinder':
      return sym(p.radius ?? 0.5, (p.height ?? 1) / 2, p.radius ?? 0.5);
    case 'cone':
      return sym(p.r1 ?? 0.5, (p.height ?? 1) / 2, p.r1 ?? 0.5);
    case 'torus': {
      const r = (p.major ?? 0.5) + (p.minor ?? 0.15);
      return sym(r, p.minor ?? 0.15, r);
    }
    default:
      return sym(0, 0, 0);
  }
}

function makeNode(
  resp: PrimitiveResponse,
  name: string,
  featureParams: Record<string, number | string | boolean>,
  bbox: { min: Vec3; max: Vec3 },
  parentId: string | null
): GeometryNode {
  return {
    id: resp.shape_id,
    arenaId: resp.arena_id,
    name,
    kind: resp.kind,
    parentId,
    featureParams,
    transform: { position: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1] },
    bbox,
    faceIds: [],
    visible: true,
    tessellationRev: 0,
  };
}

/** Patch ops that append a node to the tree (nodes map + roots array). */
function addNodePatch(ctx: CommandContext, node: GeometryNode): PatchOp[] {
  const rootCount = ctx.getState().doc.geometry.roots.length;
  return [
    { op: 'add', path: ['doc', 'geometry', 'nodes', node.id], value: node as unknown as JsonValue },
    { op: 'add', path: ['doc', 'geometry', 'roots', rootCount], value: node.id },
  ];
}

const DIM_KEYS = ['lx', 'ly', 'lz', 'radius', 'height', 'r1', 'r2', 'major', 'minor'] as const;

export interface CreatePrimitiveParams {
  kind: 'box' | 'sphere' | 'cylinder' | 'cone' | 'torus';
  name?: string;
  lx?: number; ly?: number; lz?: number;
  radius?: number; height?: number;
  r1?: number; r2?: number;
  major?: number; minor?: number;
}

export const createPrimitive: CommandDef<CreatePrimitiveParams, PrimitiveResponse> = {
  id: 'geometry.create_primitive',
  category: 'geometry',
  group: 'Create',
  title: 'Create Primitive',
  titleKo: '프리미티브 생성',
  description: 'Create a CAD primitive (box, sphere, cylinder, cone, torus) and add it to the geometry tree.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      kind: { type: 'string', enum: ['box', 'sphere', 'cylinder', 'cone', 'torus'] },
      name: { type: 'string' },
      lx: { type: 'number', minimum: 0 }, ly: { type: 'number', minimum: 0 }, lz: { type: 'number', minimum: 0 },
      radius: { type: 'number', minimum: 0 }, height: { type: 'number', minimum: 0 },
      r1: { type: 'number', minimum: 0 }, r2: { type: 'number', minimum: 0 },
      major: { type: 'number', minimum: 0 }, minor: { type: 'number', minimum: 0 },
    },
    required: ['kind'],
  },
  async run(params, ctx) {
    const rpcParams: JsonObject = { kind: params.kind };
    const dims: Record<string, number | undefined> = {};
    for (const k of DIM_KEYS) {
      const v = params[k];
      if (typeof v === 'number') {
        rpcParams[k] = v;
        dims[k] = v;
      }
    }
    const resp = await ctx.rpc.request<PrimitiveResponse>('cad.feature.primitive', rpcParams);
    const name = params.name ?? `${params.kind}_${resp.shape_id}`;
    const node = makeNode(resp, name, { kind: params.kind, ...dims }, estimateBbox(params.kind, dims), null);
    return { ok: true, result: resp, statePatch: addNodePatch(ctx, node) };
  },
};

/** Shared implementation for the functional transform features. */
function transformCommand(
  id: string,
  method: string,
  title: string,
  titleKo: string,
  extraSchema: JsonObject,
  buildRpc: (params: JsonObject) => JsonObject
): CommandDef<JsonObject, PrimitiveResponse> {
  return {
    id,
    category: 'geometry',
    group: 'Transform',
    title,
    titleKo,
    description: `Apply ${title.toLowerCase()} to a shape, producing a new shape in the tree.`,
    capability: 'mutate-scene',
    paramsSchema: {
      type: 'object',
      properties: { shape_id: { type: 'string' }, ...(extraSchema.properties as JsonObject) },
      required: ['shape_id', ...((extraSchema.required as string[]) ?? [])],
    },
    async run(params, ctx) {
      const sourceId = params.shape_id as string;
      const source = ctx.getState().doc.geometry.nodes[sourceId];
      if (!source) {
        return { ok: false, error: { code: 'UNKNOWN_SHAPE', message: `No shape "${sourceId}"` } };
      }
      const resp = await ctx.rpc.request<PrimitiveResponse>(method, buildRpc(params));
      const node = makeNode(resp, `${resp.kind}_${resp.shape_id}`, { source: sourceId }, source.bbox, sourceId);
      return {
        ok: true,
        result: resp,
        statePatch: [
          ...addNodePatch(ctx, node),
          { op: 'replace', path: ['doc', 'geometry', 'nodes', sourceId, 'visible'], value: false },
        ],
      };
    },
  };
}

export const translateShape = transformCommand(
  'geometry.translate',
  'cad.feature.translate',
  'Translate',
  '이동',
  { properties: { tx: { type: 'number' }, ty: { type: 'number' }, tz: { type: 'number' } } },
  (p) => ({ shape_id: p.shape_id, tx: p.tx ?? 0, ty: p.ty ?? 0, tz: p.tz ?? 0 })
);

export const rotateShape = transformCommand(
  'geometry.rotate',
  'cad.feature.rotate',
  'Rotate',
  '회전',
  {
    properties: {
      ax: { type: 'number' }, ay: { type: 'number' }, az: { type: 'number' },
      angle_deg: { type: 'number' },
    },
    required: ['angle_deg'],
  },
  (p) => ({ shape_id: p.shape_id, ax: p.ax ?? 0, ay: p.ay ?? 0, az: p.az ?? 1, angle_deg: p.angle_deg ?? 90 })
);

export const scaleShape = transformCommand(
  'geometry.scale',
  'cad.feature.scale',
  'Scale',
  '크기 조절',
  { properties: { sx: { type: 'number' }, sy: { type: 'number' }, sz: { type: 'number' } } },
  (p) => ({ shape_id: p.shape_id, sx: p.sx ?? 1, sy: p.sy ?? 1, sz: p.sz ?? 1 })
);

export const mirrorShape = transformCommand(
  'geometry.mirror',
  'cad.feature.mirror',
  'Mirror',
  '대칭 복사',
  { properties: { plane: { type: 'string', enum: ['xy', 'yz', 'xz'] } }, required: ['plane'] },
  (p) => ({ shape_id: p.shape_id, plane: p.plane ?? 'xy' })
);

export interface DeleteShapeParams {
  shape_id: string;
}

export const deleteShape: CommandDef<DeleteShapeParams, { deleted: boolean }> = {
  id: 'geometry.delete',
  category: 'geometry',
  group: 'Edit',
  title: 'Delete Shape',
  titleKo: '형상 삭제',
  description: 'Delete a shape from the document and the geometry tree.',
  capability: 'destructive',
  paramsSchema: {
    type: 'object',
    properties: { shape_id: { type: 'string' } },
    required: ['shape_id'],
  },
  async run(params, ctx) {
    const tree = ctx.getState().doc.geometry;
    if (!tree.nodes[params.shape_id]) {
      return { ok: false, error: { code: 'UNKNOWN_SHAPE', message: `No shape "${params.shape_id}"` } };
    }
    await ctx.rpc.request('cad.arena.delete_shape', { shape_id: params.shape_id });
    const rootIdx = tree.roots.indexOf(params.shape_id);
    const patch: PatchOp[] = [{ op: 'remove', path: ['doc', 'geometry', 'nodes', params.shape_id] }];
    if (rootIdx >= 0) patch.push({ op: 'remove', path: ['doc', 'geometry', 'roots', rootIdx] });
    return { ok: true, result: { deleted: true }, statePatch: patch };
  },
};

export interface RenameShapeParams {
  shape_id: string;
  name: string;
}

export const renameShape: CommandDef<RenameShapeParams, { name: string }> = {
  id: 'geometry.rename',
  category: 'geometry',
  group: 'Edit',
  title: 'Rename Shape',
  titleKo: '형상 이름 변경',
  description: 'Give a shape a semantic name (e.g. "inlet_pipe") so it can be addressed by name.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: { shape_id: { type: 'string' }, name: { type: 'string' } },
    required: ['shape_id', 'name'],
  },
  async run(params, ctx) {
    if (!ctx.getState().doc.geometry.nodes[params.shape_id]) {
      return { ok: false, error: { code: 'UNKNOWN_SHAPE', message: `No shape "${params.shape_id}"` } };
    }
    return {
      ok: true,
      result: { name: params.name },
      statePatch: [{ op: 'replace', path: ['doc', 'geometry', 'nodes', params.shape_id, 'name'], value: params.name }],
    };
  },
};

export interface TessellateParams {
  shape_id: string;
  chordTolerance?: number;
}

export interface TessellateResult {
  positions: number[];
  normals: number[];
  indices?: number[];
}

export const tessellateShape: CommandDef<TessellateParams, TessellateResult> = {
  id: 'geometry.tessellate',
  category: 'geometry',
  group: 'Display',
  title: 'Tessellate',
  titleKo: '테셀레이션',
  description: 'Tessellate a shape to a triangle mesh (positions/normals/indices) for rendering. Bumps tessellationRev.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: { shape_id: { type: 'string' }, chordTolerance: { type: 'number', minimum: 0 } },
    required: ['shape_id'],
  },
  async run(params, ctx) {
    const node = ctx.getState().doc.geometry.nodes[params.shape_id];
    if (!node) {
      return { ok: false, error: { code: 'UNKNOWN_SHAPE', message: `No shape "${params.shape_id}"` } };
    }
    const mesh = await ctx.rpc.request<TessellateResult>('cad.tessellate_adaptive', {
      shape_id: params.shape_id,
      chord_tolerance: params.chordTolerance ?? 0.02,
    });
    return {
      ok: true,
      result: mesh,
      statePatch: [
        {
          op: 'replace',
          path: ['doc', 'geometry', 'nodes', params.shape_id, 'tessellationRev'],
          value: node.tessellationRev + 1,
        },
      ],
    };
  },
};

export interface BooleanParams {
  shape_ids: string[];
  name?: string;
}

export const booleanUnion: CommandDef<BooleanParams, PrimitiveResponse> = {
  id: 'geometry.boolean',
  category: 'geometry',
  group: 'Boolean',
  title: 'Boolean Union',
  titleKo: '불리언 합집합',
  description: 'Merge multiple shapes into one compound shape (B-Rep compound_merge).',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: { shape_ids: { type: 'array', items: { type: 'string' }, minItems: 2 }, name: { type: 'string' } },
    required: ['shape_ids'],
  },
  async run(params, ctx) {
    const tree = ctx.getState().doc.geometry;
    for (const id of params.shape_ids) {
      if (!tree.nodes[id]) return { ok: false, error: { code: 'UNKNOWN_SHAPE', message: `No shape "${id}"` } };
    }
    const resp = await ctx.rpc.request<PrimitiveResponse>('cad.boolean.union', { shape_ids: params.shape_ids });
    const node = makeNode(resp, params.name ?? `union_${resp.shape_id}`, { inputs: params.shape_ids.length }, tree.nodes[params.shape_ids[0]].bbox, null);
    const patch: PatchOp[] = [
      ...addNodePatch(ctx, node),
      ...params.shape_ids.map<PatchOp>((id) => ({ op: 'replace', path: ['doc', 'geometry', 'nodes', id, 'visible'], value: false })),
    ];
    return { ok: true, result: resp, statePatch: patch };
  },
};

export interface LinearArrayParams {
  shape_id: string;
  count: number;
  dx?: number;
  dy?: number;
  dz?: number;
}

export const linearArray: CommandDef<LinearArrayParams, PrimitiveResponse> = {
  id: 'geometry.linear_array',
  category: 'geometry',
  group: 'Pattern',
  title: 'Linear Array',
  titleKo: '선형 배열',
  description: 'Repeat a shape `count` times along a (dx,dy,dz) step, producing a new array shape.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      shape_id: { type: 'string' },
      count: { type: 'integer', minimum: 2 },
      dx: { type: 'number' }, dy: { type: 'number' }, dz: { type: 'number' },
    },
    required: ['shape_id', 'count'],
  },
  async run(params, ctx) {
    const source = ctx.getState().doc.geometry.nodes[params.shape_id];
    if (!source) return { ok: false, error: { code: 'UNKNOWN_SHAPE', message: `No shape "${params.shape_id}"` } };
    const resp = await ctx.rpc.request<PrimitiveResponse>('cad.feature.linear_array', {
      shape_id: params.shape_id,
      count: params.count,
      dx: params.dx ?? 2,
      dy: params.dy ?? 0,
      dz: params.dz ?? 0,
    });
    const node = makeNode(resp, `array_${resp.shape_id}`, { source: params.shape_id, count: params.count }, source.bbox, params.shape_id);
    return { ok: true, result: resp, statePatch: addNodePatch(ctx, node) };
  },
};

export function registerGeometryCommands(registry: CommandRegistry): void {
  registry.register(createPrimitive);
  registry.register(translateShape);
  registry.register(rotateShape);
  registry.register(scaleShape);
  registry.register(mirrorShape);
  registry.register(booleanUnion);
  registry.register(linearArray);
  registry.register(deleteShape);
  registry.register(renameShape);
  registry.register(tessellateShape);
}
