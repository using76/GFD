/**
 * Entity addressing — how an AI agent (or the UI) refers to scene entities.
 *
 * Solves the "select the top face of the box" / "the inlet face" problem: the
 * agent does not need to know numeric ids. A reference is resolved to concrete
 * ids by the EntityResolver, reusing existing backend measure/raycast RPCs.
 *
 * `id`, `name`, and `bbox` resolve purely from AppState; `ray`/`screen` use a
 * backend raycast RPC. `nearest` (closest shape to a point) and `semantic`
 * (directional extreme / inlet-outlet by name / largest by bbox) resolve at
 * SHAPE granularity from AppState bounding boxes — face/edge/vertex granularity
 * still needs backend tessellation and falls through to null.
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

/** Euclidean distance from a point to an axis-aligned box (0 if inside). */
function pointToBboxDistance(p: Vec3, bbox: GeometryNode['bbox']): number {
  let s = 0;
  for (let k = 0; k < 3; k++) {
    const v = p[k];
    const d = v < bbox.min[k] ? bbox.min[k] - v : v > bbox.max[k] ? v - bbox.max[k] : 0;
    s += d * d;
  }
  return Math.sqrt(s);
}

function bboxVolume(b: GeometryNode['bbox']): number {
  return (b.max[0] - b.min[0]) * (b.max[1] - b.min[1]) * (b.max[2] - b.min[2]);
}

// Directional hint → [axis, takeMax]. Convention: +Z up, so top/bottom are Z,
// left/right are X, front/back are Y.
const DIR_AXIS: Record<string, [number, boolean]> = {
  top: [2, true],
  bottom: [2, false],
  right: [0, true],
  left: [0, false],
  back: [1, true],
  front: [1, false],
};

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
        case 'nearest': {
          const ns = nodes();
          if (!ns.length) return null;
          // Nearest shape by point-to-bbox distance.
          let best = ns[0];
          let bestD = Infinity;
          for (const n of ns) {
            const d = pointToBboxDistance(ref.point, n.bbox);
            if (d < bestD) {
              bestD = d;
              best = n;
            }
          }
          if (ref.entity === 'shape') {
            return { entityType: 'shape', ids: [best.id] };
          }
          // Face granularity: pick the nearest face of the nearest shape via the
          // backend (per-face tessellation). Edge/vertex still need a backend.
          if (ref.entity === 'face' && rpc.isLive()) {
            try {
              const res = await rpc.request<{ face_id?: number } | null>('cad.pick.face', {
                shape_id: best.id,
                point: [...ref.point],
              });
              return res?.face_id !== undefined ? { entityType: 'face', ids: [String(res.face_id)] } : null;
            } catch {
              return null; // no pickable face / shape has no faces → unresolved, not an error
            }
          }
          return null;
        }
        case 'semantic': {
          const ns = nodes();
          if (!ns.length) return null;
          // Flow patches resolve by name (e.g. a shape named "inlet_pipe").
          if (ref.hint === 'inlet' || ref.hint === 'outlet') {
            const matches = ns.filter((n) => n.name.toLowerCase().includes(ref.hint));
            return matches.length ? { entityType: 'shape', ids: matches.map((n) => n.id) } : null;
          }
          // Restrict to a named/id'd shape when `of` is given, else the whole scene.
          const pool = ref.of ? ns.filter((n) => n.id === ref.of || n.name === ref.of) : ns;
          if (!pool.length) return null;
          // Directional extreme: the shape furthest along the requested axis.
          const dir = DIR_AXIS[ref.hint];
          if (dir) {
            const [axis, takeMax] = dir;
            let best = pool[0];
            for (const n of pool) {
              const v = takeMax ? n.bbox.max[axis] : n.bbox.min[axis];
              const bv = takeMax ? best.bbox.max[axis] : best.bbox.min[axis];
              if (takeMax ? v > bv : v < bv) best = n;
            }
            return { entityType: 'shape', ids: [best.id] };
          }
          // largest_face is approximated by the largest-bounding-box shape until
          // backend face geometry is available.
          if (ref.hint === 'largest_face') {
            let best = pool[0];
            for (const n of pool) if (bboxVolume(n.bbox) > bboxVolume(best.bbox)) best = n;
            return { entityType: 'shape', ids: [best.id] };
          }
          return null;
        }
      }
    },
  };
}
