/**
 * Gmsh (OpenCASCADE) command set — the professional CAD → enclosure → mesh tail
 * the AI drives. Each command calls a `gmsh.*` backend RPC, then re-tessellates
 * the whole model so the viewport (GmshSceneLayer) shows the result. All are
 * auto-exposed to the AI by the MCP bridge.
 *
 * Gmsh meshes the whole model (not per shape), so we keep one scene mesh in
 * `state.gmsh` rather than per-node ShapeMeshes.
 */

import type { JsonObject, JsonValue } from '../types';
import type { CommandContext, CommandDef } from '../command';
import type { CommandRegistry } from '../registry';
import type { PatchOp } from '../patch';

interface TessResult {
  positions: number[];
  indices: number[];
  triangle_count: number;
}

/** True if `id` belongs to the Gmsh (OCC) model rather than the pure-Rust CAD
 *  geometry tree — so generic geometry/measure commands can route to gmsh.* . */
export function isGmshShape(state: { gmsh?: { shapeIds?: string[] } }, id: string): boolean {
  return Array.isArray(state.gmsh?.shapeIds) && state.gmsh!.shapeIds!.includes(id);
}

/** Re-tessellate the whole Gmsh model and patch `state.gmsh.{mesh,shapeIds}`. */
export async function refreshScene(ctx: CommandContext, shapeIds: string[]): Promise<PatchOp[]> {
  let mesh: JsonValue = null;
  try {
    const t = await ctx.rpc.request<TessResult>('gmsh.tessellate', {});
    if (t && Array.isArray(t.positions) && t.positions.length > 0) {
      mesh = { positions: t.positions, indices: t.indices, triangleCount: t.triangle_count } as unknown as JsonValue;
    }
  } catch {
    // tessellation is best-effort; the op already succeeded
  }
  return [
    { op: 'replace', path: ['gmsh', 'shapeIds'], value: shapeIds as unknown as JsonValue },
    { op: 'replace', path: ['gmsh', 'mesh'], value: mesh },
  ];
}

function asStrings(v: JsonValue | undefined): string[] {
  return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : [];
}

const DIM_PROPS = {
  x: { type: 'number' }, y: { type: 'number' }, z: { type: 'number' },
  lx: { type: 'number', minimum: 0 }, ly: { type: 'number', minimum: 0 }, lz: { type: 'number', minimum: 0 },
  radius: { type: 'number', minimum: 0 }, height: { type: 'number', minimum: 0 },
  r1: { type: 'number', minimum: 0 }, r2: { type: 'number', minimum: 0 },
} as const;

const gmshPrimitive: CommandDef<JsonObject, { shape_id: string; tag: number }> = {
  id: 'gmsh.primitive',
  category: 'geometry',
  group: 'OCC (Gmsh)',
  title: 'Create Solid (OCC)',
  titleKo: 'OCC 솔리드 생성',
  description:
    'Create a professional B-Rep solid via the OpenCASCADE kernel (box/sphere/cylinder/cone) at an explicit position [x,y,z]. Prefer this for CAD that will be healed, enclosed, or meshed. Returns its shape_id.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: { kind: { type: 'string', enum: ['box', 'sphere', 'cylinder', 'cone'] }, ...DIM_PROPS },
    required: ['kind'],
  },
  async run(params, ctx) {
    const resp = await ctx.rpc.request<{ shape_id: string; tag: number }>('gmsh.primitive', params);
    const ids = [...ctx.getState().gmsh.shapeIds, resp.shape_id];
    return { ok: true, result: resp, statePatch: await refreshScene(ctx, ids) };
  },
};

const gmshImport: CommandDef<JsonObject, { imported: string; shape_ids: string[]; count?: number }> = {
  id: 'gmsh.import',
  category: 'geometry',
  group: 'OCC (Gmsh)',
  title: 'Import CAD (OCC)',
  titleKo: 'CAD 임포트 (OCC)',
  description:
    'Import a STEP/IGES/BREP (or STL) file BY PATH into the OCC model. Dirty CAD is auto-healed and huge-unit models are auto-normalized. Returns imported solid ids; the viewport updates. Then enclose / mesh as usual. (In the chat use the 📎 Import button to pick a file.)',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: { path: { type: 'string' }, kind: { type: 'string', enum: ['step', 'stl'] } },
    required: ['path'],
  },
  async run(params, ctx) {
    const resp = await ctx.rpc.request<{ imported: string; shape_ids: string[]; count?: number }>('gmsh.import', params);
    const ids = [...ctx.getState().gmsh.shapeIds, ...(resp.shape_ids ?? [])];
    return { ok: true, result: resp, statePatch: await refreshScene(ctx, ids) };
  },
};

const gmshBoolean: CommandDef<JsonObject, { shape_ids: string[] }> = {
  id: 'gmsh.boolean',
  category: 'geometry',
  group: 'OCC (Gmsh)',
  title: 'Boolean (OCC)',
  titleKo: '불리언 (OCC)',
  description:
    'Real B-Rep boolean via OpenCASCADE. op="cut" (objects − tools), "fuse" (union), or "common" (intersection). objects/tools are arrays of shape_ids. Removes inputs by default.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      op: { type: 'string', enum: ['cut', 'fuse', 'common'] },
      objects: { type: 'array', items: { type: 'string' } },
      tools: { type: 'array', items: { type: 'string' } },
      remove: { type: 'boolean' },
    },
    required: ['op', 'objects', 'tools'],
  },
  async run(params, ctx) {
    const resp = await ctx.rpc.request<{ shape_ids: string[] }>('gmsh.boolean', params);
    const consumed = new Set([...asStrings(params.objects), ...asStrings(params.tools)]);
    const remove = params.remove !== false;
    const prev = ctx.getState().gmsh.shapeIds;
    const ids = [...(remove ? prev.filter((s) => !consumed.has(s)) : prev), ...resp.shape_ids];
    return { ok: true, result: resp, statePatch: await refreshScene(ctx, ids) };
  },
};

const gmshHeal: CommandDef<JsonObject, { healed: number; shape_ids: string[] }> = {
  id: 'gmsh.heal',
  category: 'repair',
  group: 'OCC (Gmsh)',
  title: 'Heal (OCC)',
  titleKo: '형상 치유 (OCC)',
  description:
    'Heal/repair a solid via OpenCASCADE ShapeFix: fix degenerate & small edges/faces, sew faces, make solids. Pass shape_id to heal one (omit to heal all). Tune with tolerance + flags.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      shape_id: { type: 'string' },
      tolerance: { type: 'number', minimum: 0 },
      fix_degenerated: { type: 'boolean' },
      fix_small_edges: { type: 'boolean' },
      fix_small_faces: { type: 'boolean' },
      sew_faces: { type: 'boolean' },
      make_solids: { type: 'boolean' },
    },
  },
  async run(params, ctx) {
    const resp = await ctx.rpc.request<{ healed: number; shape_ids: string[] }>('gmsh.heal', params);
    const target = typeof params.shape_id === 'string' ? params.shape_id : null;
    const prev = ctx.getState().gmsh.shapeIds;
    const ids = [...(target ? prev.filter((s) => s !== target) : prev), ...resp.shape_ids];
    return { ok: true, result: resp, statePatch: await refreshScene(ctx, ids) };
  },
};

const gmshEnclosure: CommandDef<JsonObject, { shape_id: string; model_bbox: number[] }> = {
  id: 'gmsh.enclosure',
  category: 'prepare',
  group: 'OCC (Gmsh)',
  title: 'Enclosure (OCC)',
  titleKo: 'Enclosure 생성 (OCC)',
  description:
    'Create a padded box enclosure around all current solids (the would-be fluid domain bounds). padding adds margin on every side. Then use gmsh.extract_fluid to cut the solids out.',
  capability: 'mutate-scene',
  paramsSchema: { type: 'object', properties: { padding: { type: 'number', minimum: 0 } } },
  async run(params, ctx) {
    const resp = await ctx.rpc.request<{ shape_id: string; model_bbox: number[] }>('gmsh.enclosure', params);
    const ids = [...ctx.getState().gmsh.shapeIds, resp.shape_id];
    return { ok: true, result: resp, statePatch: await refreshScene(ctx, ids) };
  },
};

const gmshExtractFluid: CommandDef<JsonObject, { fluid_shape_ids: string[] }> = {
  id: 'gmsh.extract_fluid',
  category: 'prepare',
  group: 'OCC (Gmsh)',
  title: 'Extract Fluid (OCC)',
  titleKo: '유체 영역 추출 (OCC)',
  description:
    'Extract the fluid domain = enclosure − solids (real B-Rep boolean cut). enclosure is a shape_id (from gmsh.enclosure), solids is an array of shape_ids. Returns the watertight fluid solid id(s) to mesh.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: { enclosure: { type: 'string' }, solids: { type: 'array', items: { type: 'string' } } },
    required: ['enclosure', 'solids'],
  },
  async run(params, ctx) {
    const resp = await ctx.rpc.request<{ fluid_shape_ids: string[] }>('gmsh.extract_fluid', params);
    const consumed = new Set([String(params.enclosure), ...asStrings(params.solids)]);
    const ids = [...ctx.getState().gmsh.shapeIds.filter((s) => !consumed.has(s)), ...resp.fluid_shape_ids];
    return { ok: true, result: resp, statePatch: await refreshScene(ctx, ids) };
  },
};

const gmshMesh: CommandDef<JsonObject, { nodes: number; tets: number }> = {
  id: 'gmsh.mesh',
  category: 'mesh',
  group: 'OCC (Gmsh)',
  title: 'Mesh (Gmsh)',
  titleKo: '메시 생성 (Gmsh)',
  description:
    'Generate a tetrahedral volume mesh of the current model (the fluid domain) with Gmsh and install it for the solver. size = target element size (omit for auto). flow_axis (x/y/z, default x) names the boundary patches (axis-min→inlet, axis-max→outlet, rest→wall). For a near-wall boundary layer, set bl_first (first layer height); bl_ratio (growth, default 1.2) and bl_thickness (total, default 8×first) tune it. Then calc.run solves on it.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      size: { type: 'number', minimum: 0 },
      flow_axis: { type: 'string', enum: ['x', 'y', 'z'] },
      bl_first: { type: 'number', minimum: 0 },
      bl_ratio: { type: 'number', minimum: 1 },
      bl_thickness: { type: 'number', minimum: 0 },
    },
  },
  async run(params, ctx) {
    const resp = await ctx.rpc.request<{ nodes: number; tets: number; cells: number; faces: number; boundary_patches: string[] }>('gmsh.mesh', params);
    const patch = await refreshScene(ctx, ctx.getState().gmsh.shapeIds);
    patch.push({ op: 'replace', path: ['gmsh', 'meshStats'], value: { nodes: resp.nodes, tets: resp.tets } as unknown as JsonValue });
    // Install in AppState.mesh too so the calc flow (solve.start) is enabled and
    // the status bar reflects the fluid-domain mesh.
    patch.push({
      op: 'replace',
      path: ['mesh'],
      value: {
        generated: true,
        cellCount: resp.cells ?? resp.tets,
        nodeCount: resp.nodes,
        quality: { minOrthogonality: 1, maxSkewness: 0, maxAspectRatio: 1 },
      } as unknown as JsonValue,
    });
    return { ok: true, result: resp, statePatch: patch };
  },
};

export function registerGmshCommands(registry: CommandRegistry): void {
  registry.register(gmshPrimitive);
  registry.register(gmshImport);
  registry.register(gmshBoolean);
  registry.register(gmshHeal);
  registry.register(gmshEnclosure);
  registry.register(gmshExtractFluid);
  registry.register(gmshMesh);
}
