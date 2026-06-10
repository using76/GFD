/**
 * I/O commands (Phase 4) — export shapes to interchange formats, backed by the
 * real cad.export.* RPCs. Returns the content string to the caller (download in
 * the UI, or an AI agent saving a file).
 */

import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';

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
  registry.register(exportNvdb);
}
