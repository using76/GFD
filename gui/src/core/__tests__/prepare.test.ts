import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createMockRpcClient } from '../transport/rpcClient';
import type { JsonObject } from '../types';

function backend() {
  let counter = 0;
  return createMockRpcClient((method: string) => {
    if (method === 'cad.feature.primitive') {
      counter += 1;
      return { shape_id: `shape_${counter}`, arena_id: counter, kind: 'box' };
    }
    if (method === 'cad.feature.translate') {
      counter += 1;
      return { shape_id: `shape_${counter}`, arena_id: counter, kind: 'translate' };
    }
    return {};
  });
}

describe('Phase 4 prepare commands', () => {
  it('creates an enclosure around existing geometry', async () => {
    const core = createCore({ rpc: backend() });
    // Box centered at origin with extents 2×1×1 → bbox [-1,-0.5,-0.5]..[1,0.5,0.5]
    await core.dispatcher.dispatch({ commandId: 'geometry.create_primitive', params: { kind: 'box', lx: 2, ly: 1, lz: 1 }, source: 'agent' });

    const enc = await core.dispatcher.dispatch({ commandId: 'prepare.create_enclosure', params: { padding: 0.5 }, source: 'agent' });
    expect(enc.ok).toBe(true);

    const nodes = core.store.getState().doc.geometry.nodes;
    const enclosure = Object.values(nodes).find((n) => n.kind === 'enclosure');
    expect(enclosure).toBeDefined();
    // padded bbox: [-1.5,-1,-1]..[1.5,1,1]
    expect(enclosure!.bbox.min).toEqual([-1.5, -1, -1]);
    expect(enclosure!.bbox.max).toEqual([1.5, 1, 1]);
  });

  it('errors when there is no geometry to enclose', async () => {
    const core = createCore({ rpc: backend() });
    const enc = await core.dispatcher.dispatch({ commandId: 'prepare.create_enclosure', params: {}, source: 'agent' });
    expect(enc.ok).toBe(false);
    expect(enc.error?.code).toBe('NO_GEOMETRY');
  });

  it('adds, replaces, and removes named selections', async () => {
    const core = createCore({ rpc: backend() });
    await core.dispatcher.dispatch({ commandId: 'prepare.add_named_selection', params: { name: 'inlet', bcType: 'velocity_inlet' }, source: 'agent' });
    await core.dispatcher.dispatch({ commandId: 'prepare.add_named_selection', params: { name: 'inlet', bcType: 'pressure_outlet' }, source: 'agent' });
    let sels = core.store.getState().prepare.namedSelections;
    expect(sels).toHaveLength(1); // replaced
    expect(sels[0].bcType).toBe('pressure_outlet');

    const rm = await core.dispatcher.dispatch({ commandId: 'prepare.remove_named_selection', params: { name: 'inlet' }, source: 'agent' });
    expect(rm.ok).toBe(true);
    sels = core.store.getState().prepare.namedSelections;
    expect(sels).toHaveLength(0);
  });

  it('exposes prepare commands as MCP tools', () => {
    const core = createCore({ rpc: backend() });
    expect(core.registry.has('prepare.create_enclosure')).toBe(true);
  });

  // Keep the unused JsonObject import meaningful for the linter.
  it('ignores extra params gracefully', async () => {
    const core = createCore({ rpc: backend() });
    const extra: JsonObject = { name: 'wall', foo: 'bar' };
    const r = await core.dispatcher.dispatch({ commandId: 'prepare.add_named_selection', params: extra, source: 'agent' });
    expect(r.ok).toBe(true);
  });
});
