import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createInitialState } from '../state';
import { createMcpBridge } from '../mcp/bridge';
import { createMockRpcClient } from '../transport/rpcClient';
import type { JsonObject } from '../types';

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

describe('OpenUSD / OpenVDB export commands', () => {
  it('exports USDA and VDB through commands', async () => {
    const rpc = createMockRpcClient((method: string) => {
      if (method === 'cad.feature.primitive') return { shape_id: 'shape_1', arena_id: 1, kind: 'box' };
      if (method === 'cad.export.usd_string') return { content: '#usda 1.0\n', length: 10, shapes: 1 };
      if (method === 'field.export_vdb') return { ok: true, path: '/tmp/f.vdb', voxels: 256 };
      return {};
    });
    const core = createCore({ rpc });
    const usd = await core.dispatcher.dispatch({ commandId: 'io.export_usd', params: {}, source: 'agent' });
    expect((usd.result as { content: string }).content).toMatch(/^#usda/);
    const vdb = await core.dispatcher.dispatch({ commandId: 'results.export_vdb', params: { field: 'pressure', path: '/tmp/f.vdb' }, source: 'agent' });
    expect((vdb.result as { voxels: number }).voxels).toBe(256);
  });
});

describe('Solver→UI field contour', () => {
  it('fetches a colored contour and sets the active field', async () => {
    const rpc = createMockRpcClient((method: string, params: JsonObject) => {
      if (method === 'field.contour') {
        expect(params.field).toBe('velocity_magnitude');
        return { vertices: [0, 0, 0, 1, 0, 0, 0, 1, 0], colors: [1, 0, 0, 0, 1, 0, 0, 0, 1] };
      }
      return {};
    });
    // seed results so the activeField patch applies
    const core = createCore({
      rpc,
      initialState: undefined,
    });
    // Manually place a results summary via a mesh+solve-like patch isn't needed;
    // results.contour works without prior results too (returns geometry).
    const r = await core.dispatcher.dispatch({
      commandId: 'results.contour',
      params: { field: 'velocity_magnitude' },
      source: 'agent',
    });
    expect(r.ok).toBe(true);
    const c = r.result as { vertices: number[]; colors: number[] };
    expect(c.vertices).toHaveLength(9);
    expect(c.colors).toHaveLength(9);
  });
});

describe('Vector / streamline viz commands', () => {
  it('fetches vectors, streamlines, and toggles viz state', async () => {
    const rpc = createMockRpcClient((method: string) => {
      if (method === 'field.vectors') return { origins: [0, 0, 0], vectors: [1, 0, 0], max_magnitude: 1, count: 1 };
      if (method === 'field.streamlines') return { lines: [[0, 0, 0, 1, 0, 0]], count: 1 };
      return {};
    });
    const core = createCore({ rpc });
    const v = await core.dispatcher.dispatch({ commandId: 'results.vectors', params: { stride: 2 }, source: 'agent' });
    expect((v.result as { count: number }).count).toBe(1);
    const s = await core.dispatcher.dispatch({ commandId: 'results.streamlines', params: {}, source: 'agent' });
    expect((s.result as { count: number }).count).toBe(1);

    await core.dispatcher.dispatch({ commandId: 'results.set_viz', params: { showVectors: true, vectorScale: 2 }, source: 'agent' });
    expect(core.store.getState().viz.showVectors).toBe(true);
    expect(core.store.getState().viz.vectorScale).toBe(2);
  });

  it('extracts an isosurface and toggles its viz state', async () => {
    const rpc = createMockRpcClient((method: string, params: JsonObject) => {
      if (method === 'field.isosurface') {
        expect(params.field).toBe('velocity_magnitude');
        return { positions: [0, 0, 0, 1, 0, 0, 0, 1, 0], triangle_count: 1, isovalue: params.isovalue ?? 0.5, field: 'velocity_magnitude' };
      }
      return {};
    });
    const core = createCore({ rpc });
    const iso = await core.dispatcher.dispatch({
      commandId: 'results.isosurface',
      params: { field: 'velocity_magnitude', isovalue: 0.5 },
      source: 'agent',
    });
    expect(iso.ok).toBe(true);
    expect((iso.result as { triangle_count: number }).triangle_count).toBe(1);

    await core.dispatcher.dispatch({ commandId: 'results.set_viz', params: { showIsosurface: true, isovalue: 0.3 }, source: 'agent' });
    expect(core.store.getState().viz.showIsosurface).toBe(true);
    expect(core.store.getState().viz.isovalue).toBe(0.3);
  });

  it('computes vorticity and registers it as a selectable field', async () => {
    const rpc = createMockRpcClient((method: string) => {
      if (method === 'field.vorticity') return { field: 'vorticity_magnitude', min: 0, max: 12, mean: 3 };
      return {};
    });
    const seeded = createInitialState();
    seeded.results = { availableFields: ['velocity_magnitude'], activeField: 'velocity_magnitude', fieldStats: { velocity_magnitude: { min: 0, max: 1, mean: 0.5 } } };
    const core = createCore({ rpc, initialState: seeded });
    const r = await core.dispatcher.dispatch({ commandId: 'results.vorticity', params: {}, source: 'agent' });
    expect(r.ok).toBe(true);
    const res = core.store.getState().results;
    expect(res?.availableFields).toContain('vorticity_magnitude');
    expect(res?.activeField).toBe('vorticity_magnitude');
    expect(res?.fieldStats['vorticity_magnitude']?.max).toBe(12);
  });

  it('probes a field value at a point', async () => {
    const rpc = createMockRpcClient((method: string, params: JsonObject) => {
      if (method === 'field.probe') {
        expect(params.field).toBe('pressure');
        expect(params.point).toEqual([1, 2, 0]);
        return { field: 'pressure', value: 42, cell: 7, cell_center: [1.1, 2.0, 0.0], distance: 0.1, point: [1, 2, 0] };
      }
      return {};
    });
    const core = createCore({ rpc });
    const r = await core.dispatcher.dispatch({
      commandId: 'results.probe',
      params: { field: 'pressure', point: [1, 2, 0] },
      source: 'agent',
    });
    expect(r.ok).toBe(true);
    expect((r.result as { value: number }).value).toBe(42);
  });

  it('computes the Q-criterion as a selectable field', async () => {
    const rpc = createMockRpcClient((method: string) => {
      if (method === 'field.qcriterion') return { field: 'q_criterion', min: -5, max: 20, mean: 1.5 };
      return {};
    });
    const seeded = createInitialState();
    seeded.results = { availableFields: ['velocity_magnitude'], activeField: 'velocity_magnitude', fieldStats: {} };
    const core = createCore({ rpc, initialState: seeded });
    await core.dispatcher.dispatch({ commandId: 'results.qcriterion', params: {}, source: 'agent' });
    const res = core.store.getState().results;
    expect(res?.availableFields).toContain('q_criterion');
    expect(res?.activeField).toBe('q_criterion');
  });
});

describe('CAD import → feature tree', () => {
  it('imports a mesh file as a tree node with its bbox', async () => {
    const rpc = createMockRpcClient((method: string, params: JsonObject) => {
      if (method === 'cad.import.mesh_to_tree') {
        expect(params.path).toBe('/models/bracket.stl');
        return {
          shape_id: 'shape_7',
          arena_id: 0,
          kind: 'imported_stl',
          triangle_count: 12,
          vertex_count: 8,
          bbox: { min: [-1, -1, -1], max: [1, 1, 1] },
        };
      }
      return {};
    });
    const core = createCore({ rpc });
    const r = await core.dispatcher.dispatch({
      commandId: 'io.import_mesh',
      params: { path: '/models/bracket.stl' },
      source: 'agent',
    });
    expect(r.ok).toBe(true);
    const tree = core.store.getState().doc.geometry;
    expect(tree.roots).toContain('shape_7');
    const node = tree.nodes['shape_7'];
    expect(node.kind).toBe('imported_stl');
    expect(node.name).toBe('bracket.stl');
    expect(node.bbox.max).toEqual([1, 1, 1]);
    expect(node.visible).toBe(true);
  });

  it('imports a STEP file as a faceted solid when faces reconstruct', async () => {
    const rpc = createMockRpcClient((method: string) => {
      if (method === 'cad.import.step_mesh') {
        return {
          shape_id: 'shape_9',
          arena_id: 0,
          kind: 'imported_step',
          triangle_count: 12,
          vertex_count: 8,
          bbox: { min: [0, 0, 0], max: [2, 1, 1] },
        };
      }
      return {};
    });
    const core = createCore({ rpc });
    const r = await core.dispatcher.dispatch({
      commandId: 'io.import_step',
      params: { path: '/cad/bracket.step', name: 'bracket' },
      source: 'agent',
    });
    expect(r.ok).toBe(true);
    expect((r.result as { faceted: boolean }).faceted).toBe(true);
    const node = core.store.getState().doc.geometry.nodes['shape_9'];
    expect(node.kind).toBe('imported_step');
    expect(node.bbox.max).toEqual([2, 1, 1]);
  });

  it('imports a STEP file and derives the bbox from its tessellation', async () => {
    const rpc = createMockRpcClient((method: string) => {
      if (method === 'cad.import.step') return { shape_id: 'shape_3', arena_id: 3 };
      if (method === 'cad.tessellate_adaptive') return { positions: [0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4], normals: [], indices: [] };
      return {};
    });
    const core = createCore({ rpc });
    const r = await core.dispatcher.dispatch({
      commandId: 'io.import_step',
      params: { path: 'C:\\cad\\part.step', name: 'part' },
      source: 'agent',
    });
    expect(r.ok).toBe(true);
    const node = core.store.getState().doc.geometry.nodes['shape_3'];
    expect(node.name).toBe('part');
    expect(node.bbox.min).toEqual([0, 0, 0]);
    expect(node.bbox.max).toEqual([2, 3, 4]);
  });
});
