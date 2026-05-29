/**
 * Mesh commands — wire the REAL backend mesher (mesh.generate).
 *
 * This is a prerequisite for real solving: solve.start fails unless the backend
 * holds a mesh, and only mesh.generate populates ServerState.mesh.
 */

import type { JsonObject } from '../types';
import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';
import type { MeshState } from '../state';
import type { PatchOp } from '../patch';

export interface MeshGenerateParams {
  nx: number;
  ny: number;
  nz: number;
  domain?: { xmin: number; xmax: number; ymin: number; ymax: number; zmin: number; zmax: number };
}

interface MeshGenerateResponse {
  cells: number;
  faces: number;
  nodes: number;
  quality: { min_ortho: number; max_skew: number; max_ar: number; bad_cells: number };
}

export const meshGenerate: CommandDef<MeshGenerateParams, MeshGenerateResponse> = {
  id: 'mesh.generate',
  category: 'mesh',
  group: 'Generate',
  title: 'Generate Mesh',
  titleKo: '메시 생성',
  description:
    'Generate a structured hex mesh on the backend over the given domain. Required before running the solver.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      nx: { type: 'integer', minimum: 1, default: 20 },
      ny: { type: 'integer', minimum: 1, default: 20 },
      nz: { type: 'integer', minimum: 0, default: 0 },
      domain: {
        type: 'object',
        properties: {
          xmin: { type: 'number' }, xmax: { type: 'number' },
          ymin: { type: 'number' }, ymax: { type: 'number' },
          zmin: { type: 'number' }, zmax: { type: 'number' },
        },
      },
    },
    required: ['nx', 'ny', 'nz'],
  },
  async run(params, ctx) {
    const rpcParams: JsonObject = { nx: params.nx, ny: params.ny, nz: params.nz };
    if (params.domain) rpcParams.domain = params.domain;
    const res = await ctx.rpc.request<MeshGenerateResponse>('mesh.generate', rpcParams);

    const meshState: MeshState = {
      generated: true,
      cellCount: res.cells,
      nodeCount: res.nodes,
      quality: {
        minOrthogonality: res.quality.min_ortho,
        maxSkewness: res.quality.max_skew,
        maxAspectRatio: res.quality.max_ar,
      },
    };
    const patch: PatchOp[] = [{ op: 'replace', path: ['mesh'], value: meshState as unknown as JsonObject }];
    return { ok: true, result: res, statePatch: patch };
  },
};

export function registerMeshCommands(registry: CommandRegistry): void {
  registry.register(meshGenerate);
}
