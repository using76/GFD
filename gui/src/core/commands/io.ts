/**
 * I/O commands (Phase 4) — export shapes to interchange formats, backed by the
 * real cad.export.* RPCs. Returns the content string to the caller (download in
 * the UI, or an AI agent saving a file).
 */

import type { CommandDef, CommandContext } from '../command';
import type { CommandRegistry } from '../registry';
import type { GeometryNode } from '../state';
import type { PatchOp } from '../patch';
import type { Vec3, JsonValue } from '../types';

export interface ExportStlParams {
  shape_id: string;
  u_steps?: number;
  v_steps?: number;
}

export const exportStl: CommandDef<ExportStlParams, { content: string }> = {
  id: 'io.export_stl',
  category: 'system',
  group: 'Export',
  title: 'Export STL',
  titleKo: 'STL 내보내기',
  description: 'Tessellate a shape and return its ASCII STL content as a string.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: {
      shape_id: { type: 'string' },
      u_steps: { type: 'integer', minimum: 1 },
      v_steps: { type: 'integer', minimum: 1 },
    },
    required: ['shape_id'],
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ content: string }>('cad.export.stl_string', {
      shape_id: params.shape_id,
      u_steps: params.u_steps ?? 32,
      v_steps: params.v_steps ?? 16,
    });
    return { ok: true, result: r };
  },
};

export interface ExportUsdParams {
  shape_id?: string;
  u_steps?: number;
  v_steps?: number;
}

export const exportUsd: CommandDef<ExportUsdParams, { content: string; shapes: number }> = {
  id: 'io.export_usd',
  category: 'system',
  group: 'Export',
  title: 'Export OpenUSD',
  titleKo: 'OpenUSD 내보내기',
  description:
    'Export the document (or one shape) as OpenUSD ASCII (.usda) — UsdGeomMesh prims, Z-up — for NVIDIA Omniverse / Isaac Sim.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: {
      shape_id: { type: 'string' },
      u_steps: { type: 'integer', minimum: 1 },
      v_steps: { type: 'integer', minimum: 1 },
    },
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ content: string; shapes: number }>('cad.export.usd_string', {
      ...(params.shape_id ? { shape_id: params.shape_id } : {}),
      u_steps: params.u_steps ?? 32,
      v_steps: params.v_steps ?? 16,
    });
    return { ok: true, result: r };
  },
};

export interface ExportVdbParams {
  field: string;
  path: string;
}

export const exportVdb: CommandDef<ExportVdbParams, { ok: boolean; path: string; voxels: number }> = {
  id: 'results.export_vdb',
  category: 'results',
  group: 'Export',
  title: 'Export OpenVDB',
  titleKo: 'OpenVDB 내보내기',
  description: 'Write a solved scalar field to an OpenVDB (.vdb) volume file at the given path.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: { field: { type: 'string' }, path: { type: 'string' } },
    required: ['field', 'path'],
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ ok: boolean; path: string; voxels: number }>('field.export_vdb', {
      field: params.field,
      path: params.path,
    });
    return { ok: true, result: r };
  },
};

// ────────────────────────────────────────────────────────────────────────────
// Import — read a CAD file off disk and add it to the feature tree as a node.
// Closes the "read CAD files into the model" link of the AI simulation loop.
// ────────────────────────────────────────────────────────────────────────────

interface Bbox {
  min: Vec3;
  max: Vec3;
}

/** Axis-aligned bbox over a flat [x,y,z,...] position array (tessellation). */
function bboxFromPositions(positions: number[]): Bbox {
  if (positions.length < 3) return { min: [0, 0, 0], max: [0, 0, 0] };
  const min: Vec3 = [Infinity, Infinity, Infinity];
  const max: Vec3 = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i + 2 < positions.length; i += 3) {
    for (let k = 0; k < 3; k++) {
      const v = positions[i + k];
      if (v < min[k]) min[k] = v;
      if (v > max[k]) max[k] = v;
    }
  }
  return { min, max };
}

/** Build an imported-shape GeometryNode and the patch ops that append it. */
function appendImportedNode(
  ctx: CommandContext,
  shapeId: string,
  arenaId: number,
  name: string,
  kind: string,
  bbox: Bbox,
  featureParams: Record<string, number | string | boolean>
): PatchOp[] {
  const node: GeometryNode = {
    id: shapeId,
    arenaId,
    name,
    kind,
    parentId: null,
    featureParams,
    transform: { position: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1] },
    bbox,
    faceIds: [],
    visible: true,
    tessellationRev: 0,
  };
  const rootCount = ctx.getState().doc.geometry.roots.length;
  return [
    { op: 'add', path: ['doc', 'geometry', 'nodes', node.id], value: node as unknown as JsonValue },
    { op: 'add', path: ['doc', 'geometry', 'roots', rootCount], value: node.id },
  ];
}

const MESH_FORMATS = ['stl', 'obj', 'off', 'ply', 'xyz'] as const;
type MeshFormat = (typeof MESH_FORMATS)[number];

export interface ImportMeshParams {
  path: string;
  format?: MeshFormat;
  name?: string;
}

interface ImportMeshResponse {
  shape_id: string;
  arena_id: number;
  kind: string;
  triangle_count: number;
  vertex_count: number;
  bbox: Bbox;
}

export const importMesh: CommandDef<ImportMeshParams, ImportMeshResponse> = {
  id: 'io.import_mesh',
  category: 'geometry',
  group: 'Import',
  title: 'Import Mesh',
  titleKo: '메쉬 가져오기',
  description:
    'Read a triangle-mesh CAD file (STL/OBJ/OFF/PLY/XYZ) off disk and add it to the geometry tree as a renderable shape (id == backend shape_id). Format is inferred from the extension if omitted.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      path: { type: 'string' },
      format: { type: 'string', enum: [...MESH_FORMATS] },
      name: { type: 'string' },
    },
    required: ['path'],
  },
  async run(params, ctx) {
    const resp = await ctx.rpc.request<ImportMeshResponse>('cad.import.mesh_to_tree', {
      path: params.path,
      ...(params.format ? { format: params.format } : {}),
    });
    const base = params.path.replace(/\\/g, '/').split('/').pop() ?? params.path;
    const name = params.name ?? base;
    const patch = appendImportedNode(ctx, resp.shape_id, resp.arena_id, name, resp.kind, resp.bbox, {
      format: params.format ?? 'mesh',
      triangles: resp.triangle_count,
      vertices: resp.vertex_count,
    });
    return { ok: true, result: resp, statePatch: patch };
  },
};

export interface ImportBrepParams {
  path: string;
  name?: string;
}

/** Shared impl for B-Rep imports (STEP / BRep-JSON) that return an arena shape. */
function brepImportCommand(
  id: string,
  method: string,
  title: string,
  titleKo: string,
  kindLabel: string
): CommandDef<ImportBrepParams, { shape_id: string; arena_id: number }> {
  return {
    id,
    category: 'geometry',
    group: 'Import',
    title,
    titleKo,
    description: `Import a ${kindLabel} file into the geometry tree as a B-Rep shape (bbox computed from its tessellation).`,
    capability: 'mutate-scene',
    paramsSchema: {
      type: 'object',
      properties: { path: { type: 'string' }, name: { type: 'string' } },
      required: ['path'],
    },
    async run(params, ctx) {
      const resp = await ctx.rpc.request<{ shape_id: string | null; arena_id?: number }>(method, {
        path: params.path,
      });
      if (!resp.shape_id) {
        return { ok: false, error: { code: 'IMPORT_EMPTY', message: `${kindLabel} import produced no shape` } };
      }
      // No bbox in the import response — derive it from a coarse tessellation.
      let bbox: Bbox = { min: [0, 0, 0], max: [0, 0, 0] };
      try {
        const tess = await ctx.rpc.request<{ positions: number[] }>('cad.tessellate_adaptive', {
          shape_id: resp.shape_id,
          chord_tolerance: 0.05,
        });
        if (tess.positions && tess.positions.length >= 3) bbox = bboxFromPositions(tess.positions);
      } catch {
        // keep the zero bbox if tessellation is unavailable
      }
      const base = params.path.replace(/\\/g, '/').split('/').pop() ?? params.path;
      const patch = appendImportedNode(
        ctx,
        resp.shape_id,
        resp.arena_id ?? 0,
        params.name ?? base,
        `imported_${kindLabel.toLowerCase()}`,
        bbox,
        { format: kindLabel.toLowerCase() }
      );
      return { ok: true, result: { shape_id: resp.shape_id, arena_id: resp.arena_id ?? 0 }, statePatch: patch };
    },
  };
}

/** STEP import: reconstruct planar faces into a renderable solid when possible,
 *  else fall back to the arena (points-only) representation. */
export const importStep: CommandDef<ImportBrepParams, { shape_id: string; arena_id: number; faceted: boolean }> = {
  id: 'io.import_step',
  category: 'geometry',
  group: 'Import',
  title: 'Import STEP',
  titleKo: 'STEP 가져오기',
  description:
    'Import a STEP file into the tree. Reconstructs planar faces into a renderable, meshable solid when possible; otherwise falls back to the arena (points) shape.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: { path: { type: 'string' }, name: { type: 'string' } },
    required: ['path'],
  },
  async run(params, ctx) {
    const base = params.path.replace(/\\/g, '/').split('/').pop() ?? params.path;
    const name = params.name ?? base;
    // 1. Faceted reconstruction (renders as a solid).
    try {
      const resp = await ctx.rpc.request<ImportMeshResponse>('cad.import.step_mesh', { path: params.path });
      if (resp.shape_id) {
        const patch = appendImportedNode(ctx, resp.shape_id, resp.arena_id, name, resp.kind, resp.bbox, {
          format: 'step',
          triangles: resp.triangle_count,
        });
        return { ok: true, result: { shape_id: resp.shape_id, arena_id: resp.arena_id, faceted: true }, statePatch: patch };
      }
    } catch {
      // fall through to the arena/points path
    }
    // 2. Fallback: arena shape (points-only), bbox from a coarse tessellation.
    const resp = await ctx.rpc.request<{ shape_id: string | null; arena_id?: number }>('cad.import.step', {
      path: params.path,
    });
    if (!resp.shape_id) {
      return { ok: false, error: { code: 'IMPORT_EMPTY', message: 'STEP import produced no shape' } };
    }
    let bbox: Bbox = { min: [0, 0, 0], max: [0, 0, 0] };
    try {
      const tess = await ctx.rpc.request<{ positions: number[] }>('cad.tessellate_adaptive', {
        shape_id: resp.shape_id,
        chord_tolerance: 0.05,
      });
      if (tess.positions && tess.positions.length >= 3) bbox = bboxFromPositions(tess.positions);
    } catch {
      // keep the zero bbox
    }
    const patch = appendImportedNode(ctx, resp.shape_id, resp.arena_id ?? 0, name, 'imported_step', bbox, { format: 'step' });
    return { ok: true, result: { shape_id: resp.shape_id, arena_id: resp.arena_id ?? 0, faceted: false }, statePatch: patch };
  },
};

export const importBrep = brepImportCommand('io.import_brep', 'cad.import.brep', 'Import BRep', 'BRep 가져오기', 'BRep');

export interface ExportVtkParams {
  path: string;
}

export const exportVtk: CommandDef<ExportVtkParams, { ok: boolean; path: string; fields: number; cells: number }> = {
  id: 'results.export_vtk',
  category: 'results',
  group: 'Export',
  title: 'Export VTK',
  titleKo: 'VTK 내보내기',
  description:
    'Write the solved mesh + all fields to a VTK Legacy (.vtk) unstructured-grid file for ParaView / professional post-processing.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: { path: { type: 'string' } },
    required: ['path'],
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ ok: boolean; path: string; fields: number; cells: number }>('field.export_vtk', {
      path: params.path,
    });
    return { ok: true, result: r };
  },
};

export const exportNvdb: CommandDef<ExportVdbParams, { ok: boolean; path: string; voxels: number; dims: number[] }> = {
  id: 'results.export_nvdb',
  category: 'results',
  group: 'Export',
  title: 'Export NanoVDB',
  titleKo: 'NanoVDB 내보내기',
  description:
    'Write a solved scalar field to a NanoVDB (.nvdb) volume (header-compatible dense container) for NVIDIA Omniverse / Isaac Sim.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: { field: { type: 'string' }, path: { type: 'string' } },
    required: ['field', 'path'],
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ ok: boolean; path: string; voxels: number; dims: number[] }>('field.export_nvdb', {
      field: params.field,
      path: params.path,
    });
    return { ok: true, result: r };
  },
};

export function registerIoCommands(registry: CommandRegistry): void {
  registry.register(exportStl);
  registry.register(exportUsd);
  registry.register(exportVdb);
  registry.register(exportVtk);
  registry.register(exportNvdb);
  registry.register(importMesh);
  registry.register(importStep);
  registry.register(importBrep);
}
