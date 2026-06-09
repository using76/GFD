import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createInitialState, type AppState, type GeometryNode } from '../state';
import { createMockRpcClient } from '../transport/rpcClient';
import type { MeasureDistanceResult } from '../commands/measure';

function box(id: string, min: [number, number, number], max: [number, number, number]): GeometryNode {
  return {
    id,
    arenaId: 1,
    name: id,
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

function coreWith(nodes: GeometryNode[]) {
  const s: AppState = createInitialState();
  for (const n of nodes) s.doc.geometry.nodes[n.id] = n;
  s.doc.geometry.roots = nodes.map((n) => n.id);
  return createCore({ rpc: createMockRpcClient(() => ({})), initialState: s });
}

describe('measure.distance', () => {
  it('computes center distance and bbox gap for separated shapes', async () => {
    const core = coreWith([box('a', [0, 0, 0], [1, 1, 1]), box('b', [4, 0, 0], [5, 1, 1])]);
    const r = await core.dispatcher.dispatch({ commandId: 'measure.distance', params: { a: 'a', b: 'b' }, source: 'agent' });
    expect(r.ok).toBe(true);
    const d = r.result as MeasureDistanceResult;
    expect(d.centerDistance).toBeCloseTo(4); // centers at x=0.5 and x=4.5
    expect(d.gap).toBeCloseTo(3); // gap between x=1 and x=4
  });

  it('reports a zero gap when shapes overlap', async () => {
    const core = coreWith([box('a', [0, 0, 0], [2, 2, 2]), box('b', [1, 1, 1], [3, 3, 3])]);
    const r = await core.dispatcher.dispatch({ commandId: 'measure.distance', params: { a: 'a', b: 'b' }, source: 'agent' });
    expect((r.result as MeasureDistanceResult).gap).toBe(0);
  });

  it('errors on an unknown shape id', async () => {
    const core = coreWith([box('a', [0, 0, 0], [1, 1, 1])]);
    const r = await core.dispatcher.dispatch({ commandId: 'measure.distance', params: { a: 'a', b: 'ghost' }, source: 'agent' });
    expect(r.ok).toBe(false);
    expect(r.error?.code).toBe('UNKNOWN_SHAPE');
  });
});
