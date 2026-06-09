/**
 * Mesh commands — wire the REAL backend mesher (mesh.generate).
 *
 * This is a prerequisite for real solving: solve.start fails unless the backend
 * holds a mesh, and only mesh.generate populates ServerState.mesh.
 */

import type { JsonObject, JsonValue } from '../types';
import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';
import type { AppState, MeshState } from '../state';
import type { PatchOp } from '../patch';

interface MeshDomain {
  xmin: number;
  xmax: number;
  ymin: number;
  ymax: number;
  zmin: number;
  zmax: number;
}

/** Domain that covers the current geometry — the enclosure if one exists, else
 *  the combined bbox of visible shapes (+10% padding). Connects geometry → mesh
 *  so an imported/created part is actually inside the meshed domain. */
function autoDomain(state: AppState): MeshDomain | undefined {
  const nodes = Object.values(state.doc.geometry.nodes).filter((n) => n.visible);
  if (nodes.length === 0) return undefined;
  const enc = nodes.find((n) => n.kind === 'enclosure');
  const pool = enc ? [enc] : nodes;
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  for (const n of pool) {
    for (let i = 0; i < 3; i++) {
      min[i] = Math.min(min[i], n.bbox.min[i]);
      max[i] = Math.max(max[i], n.bbox.max[i]);
    }
  }
  if (!Number.isFinite(min[0])) return undefined;
  const span = Math.max(max[0] - min[0], max[1] - min[1], max[2] - min[2], 1);
  const pad = enc ? 0 : 0.1 * span; // an enclosure is already padded
  return {
    xmin: min[0] - pad, xmax: max[0] + pad,
    ymin: min[1] - pad, ymax: max[1] + pad,
    zmin: min[2] - pad, zmax: max[2] + pad,
  };
}

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
    'Generate a structured hex mesh on the backend. If no domain is given it auto-fits the geometry (enclosure box, or the combined bbox of visible shapes + padding). Required before running the solver.',
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
    const domain = params.domain ?? autoDomain(ctx.getState());
    if (domain) rpcParams.domain = domain as unknown as JsonValue;
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
      badCells: res.quality.bad_cells,
      gen: { nx: params.nx, ny: params.ny, nz: params.nz },
    };
    const patch: PatchOp[] = [{ op: 'replace', path: ['mesh'], value: meshState as unknown as JsonObject }];
    return { ok: true, result: res, statePatch: patch };
  },
};

export function registerMeshCommands(registry: CommandRegistry): void {
  registry.register(meshGenerate);
}
