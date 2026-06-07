/**
 * Entity addressing — how an AI agent (or the UI) refers to scene entities.
 *
 * Solves the "select the top face of the box" / "the inlet face" problem: the
 * agent does not need to know numeric ids. A reference is resolved to concrete
 * ids by the EntityResolver, reusing existing backend measure/raycast RPCs.
 *
 * Phase 0 implements the `id`, `name`, and `bbox` resolvers (pure, from
 * AppState). Spatial refs (`ray`, `screen`, `nearest`, `semantic`) have a stable
 * interface here and are wired to RPC in Phase 3 (renderer) / later phases.
 */

import type { JsonObject, Vec3 } from './types';
import type { AppState, GeometryNode } from './state';
import type { RpcClient } from './transport/rpcClient';

export type EntityRef =
  | { by: 'id'; id: string }
  | { by: 'name'; name: string; entity?: 'shape' | 'face' | 'named_selection' }
  | { by: 'ray'; origin: Vec3; dir: Vec3 }
  | { by: 'screen'; x: number; y: number }
  | { by: 'nearest'; point: Vec3; entity: 'face' | 'vertex' | 'edge' | 'shape' }
  | { by: 'bbox'; min: Vec3; max: Vec3; mode: 'contains' | 'intersects' }
  | {
      by: 'semantic';
      hint: 'top' | 'bottom' | 'left' | 'right' | 'front' | 'back' | 'largest_face' | 'inlet' | 'outlet';
      of?: string;
    };

export interface ResolvedEntity {
  entityType: 'shape' | 'face' | 'edge' | 'vertex';
  ids: string[];
}

export interface EntityResolver {
  resolve(ref: EntityRef): Promise<ResolvedEntity | null>;
}

function bboxOverlaps(a: GeometryNode['bbox'], min: Vec3, max: Vec3): boolean {
  return (
    a.min[0] <= max[0] && a.max[0] >= min[0] &&
    a.min[1] <= max[1] && a.max[1] >= min[1] &&
    a.min[2] <= max[2] && a.max[2] >= min[2]
  );
}

function bboxContained(a: GeometryNode['bbox'], min: Vec3, max: Vec3): boolean {
  return (
    a.min[0] >= min[0] && a.max[0] <= max[0] &&
    a.min[1] >= min[1] && a.max[1] <= max[1] &&
    a.min[2] >= min[2] && a.max[2] <= max[2]
  );
}

/**
 * Default resolver. `getState` supplies the current AppState; `rpc` is used for
 * spatial queries that require the backend (raycast / nearest).
 */
export function createEntityResolver(getState: () => Readonly<AppState>, rpc: RpcClient): EntityResolver {
  const nodes = () => Object.values(getState().doc.geometry.nodes);

  return {
    async resolve(ref) {
      switch (ref.by) {
        case 'id': {
          const exists = !!getState().doc.geometry.nodes[ref.id];
          return exists ? { entityType: 'shape', ids: [ref.id] } : null;
        }
        case 'name': {
          const matches = nodes().filter((n) => n.name === ref.name);
          return matches.length ? { entityType: 'shape', ids: matches.map((n) => n.id) } : null;
        }
        case 'bbox': {
          const pred = ref.mode === 'contains' ? bboxContained : bboxOverlaps;
          const matches = nodes().filter((n) => pred(n.bbox, ref.min, ref.max));
          return matches.length ? { entityType: 'shape', ids: matches.map((n) => n.id) } : null;
        }
        case 'ray':
        case 'screen': {
          // Server-side raycast against tessellated geometry.
          if (!rpc.isLive()) return null;
          const rayParams: JsonObject =
            ref.by === 'ray'
              ? { origins: [[...ref.origin]], dirs: [[...ref.dir]] }
              : { screen: { x: ref.x, y: ref.y } };
          const res = await rpc.request<{ shape_id?: string } | null>(
            'cad.measure.trimesh_raycast',
            rayParams
          );
          return res?.shape_id ? { entityType: 'shape', ids: [res.shape_id] } : null;
        }
        case 'nearest':
        case 'semantic':
          // Stable interface; backend wiring lands with the renderer (Phase 3+).
          return null;
      }
    },
  };
}
