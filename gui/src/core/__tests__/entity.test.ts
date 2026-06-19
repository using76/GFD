import { describe, it, expect } from 'vitest';
import { createInitialState, type AppState, type GeometryNode } from '../state';
import { createEntityResolver } from '../entity';
import { createMockRpcClient } from '../transport/rpcClient';

function box(id: string, name: string, min: [number, number, number], max: [number, number, number]): GeometryNode {
  return {
    id,
    arenaId: 1,
    name,
    kind: 'box',
    parentId: null,
    featureParams: {},
    transform: { position: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1] },
    bbox: { min, max },
    faceIds: [],
    visible: true,
    tessellationRev: 0,
  };
}

function stateWith(nodes: GeometryNode[]): AppState {
  const s = createInitialState();
  for (const n of nodes) s.doc.geometry.nodes[n.id] = n;
  s.doc.geometry.roots = nodes.map((n) => n.id);
  return s;
}

const rpc = createMockRpcClient(() => ({}));

describe('entity resolver — spatial references', () => {
  it('resolves the nearest shape to a point', async () => {
    const s = stateWith([
      box('a', 'a', [0, 0, 0], [1, 1, 1]),
      box('b', 'b', [10, 0, 0], [11, 1, 1]),
    ]);
    const r = createEntityResolver(() => s, rpc);
    const near = await r.resolve({ by: 'nearest', point: [9, 0.5, 0.5], entity: 'shape' });
    expect(near).toEqual({ entityType: 'shape', ids: ['b'] });
    const near2 = await r.resolve({ by: 'nearest', point: [0.5, 0.5, 0.5], entity: 'shape' });
    expect(near2?.ids).toEqual(['a']);
  });

  it('resolves a nearest FACE via the backend pick on the nearest shape', async () => {
    const s = stateWith([box('a', 'a', [0, 0, 0], [1, 1, 1])]);
    const live = createMockRpcClient((method: string, params: Record<string, unknown>) => {
      if (method === 'cad.pick.face') {
        expect(params.shape_id).toBe('a');
        return { face_id: 7, surface_kind: 'plane', distance: 4, closest_point: [0.5, 0.5, 1], centroid: [0.5, 0.5, 1] };
      }
      return {};
    });
    const r = createEntityResolver(() => s, live);
    const res = await r.resolve({ by: 'nearest', point: [0.5, 0.5, 5], entity: 'face' });
    expect(res).toEqual({ entityType: 'face', ids: ['7'] });
  });

  it('resolves nearest edge and vertex via the backend pick', async () => {
    const s = stateWith([box('a', 'a', [0, 0, 0], [1, 1, 1])]);
    const live = createMockRpcClient((method: string) => {
      if (method === 'cad.pick.edge') return { edge_id: 3, distance: 0.1, midpoint: [0, 0.5, 1] };
      if (method === 'cad.pick.vertex') return { vertex_id: 5, distance: 0.2, point: [1, 1, 1] };
      return {};
    });
    const r = createEntityResolver(() => s, live);
    expect(await r.resolve({ by: 'nearest', point: [0, 0.5, 1], entity: 'edge' })).toEqual({ entityType: 'edge', ids: ['3'] });
    expect(await r.resolve({ by: 'nearest', point: [1, 1, 1], entity: 'vertex' })).toEqual({ entityType: 'vertex', ids: ['5'] });
  });

  it('resolves the directional extreme shape (top/bottom along +Z)', async () => {
    const s = stateWith([
      box('low', 'low', [0, 0, 0], [1, 1, 1]),
      box('high', 'high', [0, 0, 5], [1, 1, 6]),
    ]);
    const r = createEntityResolver(() => s, rpc);
    expect((await r.resolve({ by: 'semantic', hint: 'top' }))?.ids).toEqual(['high']);
    expect((await r.resolve({ by: 'semantic', hint: 'bottom' }))?.ids).toEqual(['low']);
  });

  it('resolves inlet/outlet by name', async () => {
    const s = stateWith([
      box('p1', 'inlet_pipe', [0, 0, 0], [1, 1, 1]),
      box('p2', 'outlet_pipe', [2, 0, 0], [3, 1, 1]),
    ]);
    const r = createEntityResolver(() => s, rpc);
    expect((await r.resolve({ by: 'semantic', hint: 'inlet' }))?.ids).toEqual(['p1']);
    expect((await r.resolve({ by: 'semantic', hint: 'outlet' }))?.ids).toEqual(['p2']);
  });

  it('restricts a directional hint to the `of` shape and approximates largest_face', async () => {
    const s = stateWith([
      box('small', 'small', [0, 0, 0], [1, 1, 1]),
      box('big', 'big', [0, 0, 0], [4, 4, 4]),
    ]);
    const r = createEntityResolver(() => s, rpc);
    // `of` restricts the pool to that shape.
    expect((await r.resolve({ by: 'semantic', hint: 'top', of: 'small' }))?.ids).toEqual(['small']);
    // largest_face approximates with the largest bbox.
    expect((await r.resolve({ by: 'semantic', hint: 'largest_face' }))?.ids).toEqual(['big']);
  });
});
