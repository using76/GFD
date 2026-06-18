import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createInitialState, type AppState, type GeometryNode } from '../state';
import { createMockRpcClient } from '../transport/rpcClient';
import type { JsonObject } from '../types';

function node(id: string, kind: string, min: [number, number, number], max: [number, number, number]): GeometryNode {
  return {
    id, arenaId: 1, name: id, kind, parentId: null, featureParams: {},
    transform: { position: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1] },
    bbox: { min, max }, faceIds: [], visible: true, tessellationRev: 0,
  };
}

const MESH_RESP = { cells: 400, faces: 840, nodes: 441, quality: { min_ortho: 0.95, max_skew: 0.1, max_ar: 1.0, bad_cells: 0 } };

function coreCapturing(state: AppState, captured: { domain?: JsonObject }) {
  const rpc = createMockRpcClient((method: string, params: JsonObject) => {
    if (method === 'mesh.generate') {
      captured.domain = params.domain as JsonObject | undefined;
      return MESH_RESP;
    }
    return {};
  });
  return createCore({ rpc, initialState: state });
}

describe('mesh.generate auto-domain (geometry → mesh)', () => {
  it('fits the domain to an enclosure box exactly (no extra padding)', async () => {
    const s = createInitialState();
    s.doc.geometry.nodes['enc'] = node('enc', 'enclosure', [-1, -1, -1], [3, 2, 2]);
    s.doc.geometry.roots = ['enc'];
    const captured: { domain?: JsonObject } = {};
    const core = coreCapturing(s, captured);
    await core.dispatcher.dispatch({ commandId: 'mesh.generate', params: { nx: 10, ny: 10, nz: 10 }, source: 'agent' });
    expect(captured.domain).toEqual({ xmin: -1, xmax: 3, ymin: -1, ymax: 2, zmin: -1, zmax: 2 });
  });

  it('pads the combined bbox when there is no enclosure', async () => {
    const s = createInitialState();
    s.doc.geometry.nodes['b'] = node('b', 'box', [0, 0, 0], [10, 1, 1]);
    s.doc.geometry.roots = ['b'];
    const captured: { domain?: JsonObject } = {};
    const core = coreCapturing(s, captured);
    await core.dispatcher.dispatch({ commandId: 'mesh.generate', params: { nx: 10, ny: 10, nz: 10 }, source: 'agent' });
    // span = max(10,1,1,1) = 10 → pad = 1
    expect(captured.domain).toEqual({ xmin: -1, xmax: 11, ymin: -1, ymax: 2, zmin: -1, zmax: 2 });
  });

  it('respects an explicit domain and ignores geometry', async () => {
    const s = createInitialState();
    s.doc.geometry.nodes['b'] = node('b', 'box', [0, 0, 0], [10, 1, 1]);
    s.doc.geometry.roots = ['b'];
    const captured: { domain?: JsonObject } = {};
    const core = coreCapturing(s, captured);
    const explicit = { xmin: 0, xmax: 1, ymin: 0, ymax: 1, zmin: 0, zmax: 1 };
    await core.dispatcher.dispatch({ commandId: 'mesh.generate', params: { nx: 5, ny: 5, nz: 5, domain: explicit }, source: 'agent' });
    expect(captured.domain).toEqual(explicit);
  });

  it('passes no domain when there is no geometry (backend default)', async () => {
    const captured: { domain?: JsonObject } = {};
    const core = coreCapturing(createInitialState(), captured);
    await core.dispatcher.dispatch({ commandId: 'mesh.generate', params: { nx: 5, ny: 5, nz: 0 }, source: 'agent' });
    expect(captured.domain).toBeUndefined();
  });
});

describe('mesh.generate solid masking (immersed boundary)', () => {
  const MASK_RESP = { cells: 400, faces: 840, nodes: 441, solid_cells: 37, fluid_cells: 363, quality: { min_ortho: 0.95, max_skew: 0.1, max_ar: 1.0, bad_cells: 0 } };

  function coreCapturingMask(state: AppState, captured: { params?: JsonObject }) {
    const rpc = createMockRpcClient((method: string, params: JsonObject) => {
      if (method === 'mesh.generate') {
        captured.params = params;
        return MASK_RESP;
      }
      return {};
    });
    return createCore({ rpc, initialState: state });
  }

  it('forwards mask_solids + solid_shape_ids (excluding the enclosure) and stores counts', async () => {
    const s = createInitialState();
    s.doc.geometry.nodes['enc'] = node('enc', 'enclosure', [-1, -1, -1], [3, 3, 3]);
    s.doc.geometry.nodes['part'] = node('part', 'imported_step', [0, 0, 0], [1, 1, 1]);
    s.doc.geometry.roots = ['enc', 'part'];
    const captured: { params?: JsonObject } = {};
    const core = coreCapturingMask(s, captured);
    await core.dispatcher.dispatch({ commandId: 'mesh.generate', params: { nx: 10, ny: 10, nz: 10, maskSolids: true }, source: 'agent' });
    expect(captured.params?.mask_solids).toBe(true);
    // enclosure is excluded; only the part is a solid.
    expect(captured.params?.solid_shape_ids).toEqual(['part']);
    const mesh = core.store.getState().mesh;
    expect(mesh?.solidCells).toBe(37);
    expect(mesh?.fluidCells).toBe(363);
  });

  it('does not forward masking params when maskSolids is omitted', async () => {
    const s = createInitialState();
    s.doc.geometry.nodes['part'] = node('part', 'box', [0, 0, 0], [1, 1, 1]);
    s.doc.geometry.roots = ['part'];
    const captured: { params?: JsonObject } = {};
    const core = coreCapturingMask(s, captured);
    await core.dispatcher.dispatch({ commandId: 'mesh.generate', params: { nx: 5, ny: 5, nz: 0 }, source: 'agent' });
    expect(captured.params?.mask_solids).toBeUndefined();
    expect(captured.params?.solid_shape_ids).toBeUndefined();
  });
});
