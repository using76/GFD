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

export function registerIoCommands(registry: CommandRegistry): void {
  registry.register(exportStl);
}
