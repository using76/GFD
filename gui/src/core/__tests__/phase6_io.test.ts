import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createMcpBridge } from '../mcp/bridge';
import { createMockRpcClient } from '../transport/rpcClient';

function backend() {
  let counter = 0;
  return createMockRpcClient((method: string) => {
    if (method === 'cad.feature.primitive') {
      counter += 1;
      return { shape_id: `shape_${counter}`, arena_id: counter, kind: 'box' };
    }
    if (method === 'cad.boolean.union') {
      counter += 1;
      return { shape_id: `shape_${counter}`, arena_id: counter, kind: 'compound' };
    }
    if (method === 'cad.feature.linear_array') {
      counter += 1;
      return { shape_id: `shape_${counter}`, arena_id: counter, kind: 'linear_array' };
    }
    if (method === 'cad.export.stl_string') return { content: 'solid s\nendsolid s\n' };
    return {};
  });
}

describe('Phase 4 parity commands (boolean / array / export)', () => {
  it('unions two shapes into a compound and hides the inputs', async () => {
    const core = createCore({ rpc: backend() });
    await core.dispatcher.dispatch({ commandId: 'geometry.create_primitive', params: { kind: 'box' }, source: 'agent' });
    await core.dispatcher.dispatch({ commandId: 'geometry.create_primitive', params: { kind: 'sphere' }, source: 'agent' });
    const u = await core.dispatcher.dispatch({ commandId: 'geometry.boolean', params: { shape_ids: ['shape_1', 'shape_2'] }, source: 'agent' });
    expect(u.ok).toBe(true);
    const tree = core.store.getState().doc.geometry;
    expect(tree.nodes['shape_1'].visible).toBe(false);
    expect(tree.nodes['shape_2'].visible).toBe(false);
    expect(tree.nodes['shape_3'].kind).toBe('compound');
  });

  it('creates a linear array node', async () => {
    const core = createCore({ rpc: backend() });
    await core.dispatcher.dispatch({ commandId: 'geometry.create_primitive', params: { kind: 'box' }, source: 'agent' });
    const a = await core.dispatcher.dispatch({ commandId: 'geometry.linear_array', params: { shape_id: 'shape_1', count: 4, dx: 1.5 }, source: 'agent' });
    expect(a.ok).toBe(true);
    expect(core.store.getState().doc.geometry.nodes['shape_2'].kind).toBe('linear_array');
  });

  it('exports STL string', async () => {
    const core = createCore({ rpc: backend() });
    await core.dispatcher.dispatch({ commandId: 'geometry.create_primitive', params: { kind: 'box' }, source: 'agent' });
    const e = await core.dispatcher.dispatch({ commandId: 'io.export_stl', params: { shape_id: 'shape_1' }, source: 'agent' });
    expect(e.ok).toBe(true);
    expect((e.result as { content: string }).content).toContain('solid');
  });
});

describe('Phase 6 screenshot bridge', () => {
  it('fails when no renderer capturer is registered', async () => {
    const bridge = createMcpBridge(createCore({ rpc: backend() }));
    const r = await bridge.callTool('screenshot', {});
    expect(r.ok).toBe(false);
    expect(r.error).toMatch(/renderer/i);
  });

  it('returns image + labels when the renderer registers a capturer', async () => {
    const core = createCore({ rpc: backend() });
    core.screenshot.register(async () => ({
      image: 'data:image/png;base64,AAAA',
      labels: [{ id: 'shape_1', name: 'box', screenXY: [100, 120] }],
      width: 800,
      height: 600,
    }));
    const bridge = createMcpBridge(core);
    const r = await bridge.callTool('screenshot', {});
    expect(r.ok).toBe(true);
    const shot = r.content as { image: string; labels: Array<{ id: string }> };
    expect(shot.image).toMatch(/^data:image\/png/);
    expect(shot.labels[0].id).toBe('shape_1');
  });
});
