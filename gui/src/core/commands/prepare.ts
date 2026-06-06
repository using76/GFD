/**
 * Prepare commands (Phase 4) — CFD pre-processing. Real, backed operations
 * (no fakes): create_enclosure builds an actual box around the model via the CAD
 * kernel; named selections store the geometry→boundary-patch mapping an agent or
 * user assigns.
 */

import type { JsonValue, Vec3 } from '../types';
import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';
import type { GeometryNode, NamedSelection } from '../state';
import type { PatchOp } from '../patch';

interface FeatureResponse {
  shape_id: string;
  arena_id: number;
  kind: string;
}

export interface CreateEnclosureParams {
  padding?: number;
  includeHidden?: boolean;
}

export const createEnclosure: CommandDef<CreateEnclosureParams, FeatureResponse> = {
  id: 'prepare.create_enclosure',
  category: 'prepare',
  group: 'Domain',
  title: 'Create Enclosure',
  titleKo: '인클로저 생성',
  description: 'Create a fluid-domain box enclosing the model (combined bounding box + padding), via the CAD kernel.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      padding: { type: 'number', minimum: 0 },
      includeHidden: { type: 'boolean' },
    },
  },
  async run(params, ctx) {
    const padding = params.padding ?? 0.1;
    const nodes = Object.values(ctx.getState().doc.geometry.nodes).filter(
      (n) => params.includeHidden || n.visible
    );
    if (nodes.length === 0) {
      return { ok: false, error: { code: 'NO_GEOMETRY', message: 'No geometry to enclose' } };
    }

    const min: Vec3 = [Infinity, Infinity, Infinity];
    const max: Vec3 = [-Infinity, -Infinity, -Infinity];
    for (const n of nodes) {
      for (let i = 0; i < 3; i++) {
        min[i] = Math.min(min[i], n.bbox.min[i]);
        max[i] = Math.max(max[i], n.bbox.max[i]);
      }
    }
    const pmin: Vec3 = [min[0] - padding, min[1] - padding, min[2] - padding];
    const pmax: Vec3 = [max[0] + padding, max[1] + padding, max[2] + padding];
    const size: Vec3 = [pmax[0] - pmin[0], pmax[1] - pmin[1], pmax[2] - pmin[2]];
    const center: Vec3 = [(pmin[0] + pmax[0]) / 2, (pmin[1] + pmax[1]) / 2, (pmin[2] + pmax[2]) / 2];

    // Box is centered at origin; create then translate to the model center.
    const box = await ctx.rpc.request<FeatureResponse>('cad.feature.primitive', {
      kind: 'box',
      lx: size[0],
      ly: size[1],
      lz: size[2],
    });
    const enc = await ctx.rpc.request<FeatureResponse>('cad.feature.translate', {
      shape_id: box.shape_id,
      tx: center[0],
      ty: center[1],
      tz: center[2],
    });

    const node: GeometryNode = {
      id: enc.shape_id,
      arenaId: enc.arena_id,
      name: 'enclosure',
      kind: 'enclosure',
      parentId: null,
      featureParams: { padding },
      transform: { position: center, rotation: [0, 0, 0], scale: [1, 1, 1] },
      bbox: { min: pmin, max: pmax },
      faceIds: [],
      visible: true,
      tessellationRev: 0,
    };
    const rootCount = ctx.getState().doc.geometry.roots.length;
    const patch: PatchOp[] = [
      { op: 'add', path: ['doc', 'geometry', 'nodes', node.id], value: node as unknown as JsonValue },
      { op: 'add', path: ['doc', 'geometry', 'roots', rootCount], value: node.id },
    ];
    return { ok: true, result: enc, statePatch: patch };
  },
};

export interface AddNamedSelectionParams {
  name: string;
  shapeId?: string;
  faceIds?: string[];
  bcType?: string;
}

export const addNamedSelection: CommandDef<AddNamedSelectionParams, NamedSelection> = {
  id: 'prepare.add_named_selection',
  category: 'prepare',
  group: 'Named Selections',
  title: 'Add Named Selection',
  titleKo: '이름 있는 선택 추가',
  description: 'Define/replace a named geometry group (faces or a shape) and optionally its boundary type.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      name: { type: 'string' },
      shapeId: { type: 'string' },
      faceIds: { type: 'array', items: { type: 'string' } },
      bcType: { type: 'string' },
    },
    required: ['name'],
  },
  async run(params, ctx) {
    const sel: NamedSelection = {
      name: params.name,
      shapeId: params.shapeId,
      faceIds: params.faceIds ?? [],
      bcType: params.bcType,
    };
    const list = ctx.getState().prepare.namedSelections;
    const idx = list.findIndex((s) => s.name === params.name);
    const patch: PatchOp[] =
      idx >= 0
        ? [{ op: 'replace', path: ['prepare', 'namedSelections', idx], value: sel as unknown as JsonValue }]
        : [{ op: 'add', path: ['prepare', 'namedSelections', list.length], value: sel as unknown as JsonValue }];
    return { ok: true, result: sel, statePatch: patch };
  },
};

export interface RemoveNamedSelectionParams {
  name: string;
}

export const removeNamedSelection: CommandDef<RemoveNamedSelectionParams, { removed: boolean }> = {
  id: 'prepare.remove_named_selection',
  category: 'prepare',
  group: 'Named Selections',
  title: 'Remove Named Selection',
  titleKo: '이름 있는 선택 제거',
  description: 'Remove a named selection by name.',
  capability: 'mutate-scene',
  paramsSchema: { type: 'object', properties: { name: { type: 'string' } }, required: ['name'] },
  async run(params, ctx) {
    const idx = ctx.getState().prepare.namedSelections.findIndex((s) => s.name === params.name);
    if (idx < 0) return { ok: false, error: { code: 'NO_SELECTION', message: `No named selection "${params.name}"` } };
    return {
      ok: true,
      result: { removed: true },
      statePatch: [{ op: 'remove', path: ['prepare', 'namedSelections', idx] }],
    };
  },
};

export function registerPrepareCommands(registry: CommandRegistry): void {
  registry.register(createEnclosure);
  registry.register(addNamedSelection);
  registry.register(removeNamedSelection);
}
