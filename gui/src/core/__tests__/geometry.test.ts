import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createEntityResolver } from '../entity';
import { createMockRpcClient } from '../transport/rpcClient';
import type { JsonObject } from '../types';

/** Fake CAD backend: hands out incrementing shape ids. */
function fakeCad() {
  let counter = 0;
  return createMockRpcClient((method: string, _params: JsonObject) => {
    if (method === 'cad.feature.primitive') {
      counter += 1;
      return { shape_id: `shape_${counter}`, arena_id: counter, kind: 'box' };
    }
    if (method.startsWith('cad.feature.')) {
      counter += 1;
      const kind = method.split('.').pop()!;
      return { shape_id: `shape_${counter}`, arena_id: counter, kind };
    }
    if (method === 'cad.arena.delete_shape') return { deleted: true, arena_id: 1 };
    if (method === 'cad.tessellate_adaptive') return { positions: [0, 0, 0], normals: [0, 1, 0], indices: [0] };
    if (method === 'cad.sketch.new') return { sketch_idx: 0 };
    if (method === 'cad.sketch.solve') return { residual: 1e-10, points: [[0, 0]] };
    return {};
  });
}

describe('Phase 2 geometry commands', () => {
  it('creates a primitive and adds it to the geometry tree with canonical id', async () => {
    const core = createCore({ rpc: fakeCad() });
    const r = await core.dispatcher.dispatch({
      commandId: 'geometry.create_primitive',
      params: { kind: 'box', lx: 2, ly: 1, lz: 1, name: 'inlet_box' },
      source: 'agent',
    });
    expect(r.ok).toBe(true);
    const tree = core.store.getState().doc.geometry;
    expect(tree.roots).toContain('shape_1');
    expect(tree.nodes['shape_1'].name).toBe('inlet_box');
    expect(tree.nodes['shape_1'].bbox.max).toEqual([1, 0.5, 0.5]);
  });

  it('resolves a shape by its semantic name via EntityRef', async () => {
    const rpc = fakeCad();
    const core = createCore({ rpc });
    await core.dispatcher.dispatch({
      commandId: 'geometry.create_primitive',
      params: { kind: 'box', name: 'inlet' },
      source: 'agent',
    });
    const resolver = createEntityResolver(() => core.store.getState(), rpc);
    const resolved = await resolver.resolve({ by: 'name', name: 'inlet' });
    expect(resolved).toEqual({ entityType: 'shape', ids: ['shape_1'] });
  });

  it('transforms a shape into a new node and hides the source', async () => {
    const core = createCore({ rpc: fakeCad() });
    await core.dispatcher.dispatch({ commandId: 'geometry.create_primitive', params: { kind: 'box' }, source: 'agent' });
    const t = await core.dispatcher.dispatch({
      commandId: 'geometry.translate',
      params: { shape_id: 'shape_1', tx: 1 },
      source: 'agent',
    });
    expect(t.ok).toBe(true);
    const tree = core.store.getState().doc.geometry;
    expect(tree.nodes['shape_1'].visible).toBe(false);
    expect(tree.nodes['shape_2'].parentId).toBe('shape_1');
  });

  it('renames, tessellates (bumps rev), and deletes', async () => {
    const core = createCore({ rpc: fakeCad() });
    await core.dispatcher.dispatch({ commandId: 'geometry.create_primitive', params: { kind: 'box' }, source: 'agent' });

    await core.dispatcher.dispatch({ commandId: 'geometry.rename', params: { shape_id: 'shape_1', name: 'wing' }, source: 'agent' });
    expect(core.store.getState().doc.geometry.nodes['shape_1'].name).toBe('wing');

    await core.dispatcher.dispatch({ commandId: 'geometry.tessellate', params: { shape_id: 'shape_1' }, source: 'agent' });
    expect(core.store.getState().doc.geometry.nodes['shape_1'].tessellationRev).toBe(1);

    const del = await core.dispatcher.dispatch({ commandId: 'geometry.delete', params: { shape_id: 'shape_1' }, source: 'agent' });
    expect(del.ok).toBe(true);
    expect(core.store.getState().doc.geometry.nodes['shape_1']).toBeUndefined();
    expect(core.store.getState().doc.geometry.roots).not.toContain('shape_1');
  });

  it('undoes a primitive creation', async () => {
    const core = createCore({ rpc: fakeCad() });
    await core.dispatcher.dispatch({ commandId: 'geometry.create_primitive', params: { kind: 'sphere' }, source: 'human' });
    expect(core.store.getState().doc.geometry.roots).toHaveLength(1);
    expect(await core.dispatcher.undo()).toBe(true);
    expect(core.store.getState().doc.geometry.roots).toHaveLength(0);
    expect(core.store.getState().doc.geometry.nodes['shape_1']).toBeUndefined();
  });

  it('rejects transforms on unknown shapes', async () => {
    const core = createCore({ rpc: fakeCad() });
    const r = await core.dispatcher.dispatch({ commandId: 'geometry.rotate', params: { shape_id: 'nope', angle_deg: 45 }, source: 'agent' });
    expect(r.ok).toBe(false);
    expect(r.error?.code).toBe('UNKNOWN_SHAPE');
  });

  it('runs a sketch flow', async () => {
    const core = createCore({ rpc: fakeCad() });
    const s = await core.dispatcher.dispatch({ commandId: 'sketch.new', params: {}, source: 'agent' });
    expect(s.ok).toBe(true);
    expect(core.store.getState().doc.sketchIds).toContain(0);
    const solved = await core.dispatcher.dispatch({ commandId: 'sketch.solve', params: { sketch_idx: 0 }, source: 'agent' });
    expect(solved.ok).toBe(true);
  });
});
