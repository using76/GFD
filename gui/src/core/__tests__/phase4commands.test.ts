import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createMockRpcClient } from '../transport/rpcClient';
import type { JsonObject } from '../types';

function backend() {
  let counter = 0;
  return createMockRpcClient((method: string, params: JsonObject) => {
    if (method === 'cad.feature.primitive') {
      counter += 1;
      return { shape_id: `shape_${counter}`, arena_id: counter, kind: 'box' };
    }
    if (method === 'cad.measure.volume') return { volume: 1.0 };
    if (method === 'cad.measure.surface_area') return { area: 6.0 };
    if (method === 'cad.measure.center_of_mass') return { x: 0, y: 0, z: 0 };
    if (method === 'cad.heal.check_validity') return { valid: false, issues: [{ kind: 'open_wire' }] };
    if (method === 'cad.heal.fix') return { log: ['sewed 2 vertices'] };
    if (method === 'cad.heal.stats') return { faces: 6, edges: 12, vertices: 8 };
    if (method === 'mesh.generate') return { cells: 100, faces: 200, nodes: 121, quality: { min_ortho: 0.9, max_skew: 0.1, max_ar: 1, bad_cells: 0 } };
    if (method === 'solve.start') return { job_id: 'job_x' };
    if (method === 'solve.status') return { running: false, iteration: 50, residual: 1e-5, elapsed_ms: 5, status: 'converged' };
    if (method === 'field.get') return params.field === 'pressure' ? { values: [1], min: 1, max: 1, mean: 1 } : (() => { throw new Error('nf'); })();
    return {};
  });
}

async function makeBox(core: ReturnType<typeof createCore>) {
  await core.dispatcher.dispatch({ commandId: 'geometry.create_primitive', params: { kind: 'box' }, source: 'agent' });
}

describe('Phase 4 measure/repair/display/setup commands', () => {
  it('measures volume / area / center of mass / bbox', async () => {
    const core = createCore({ rpc: backend() });
    await makeBox(core);
    const vol = await core.dispatcher.dispatch({ commandId: 'measure.volume', params: { shape_id: 'shape_1' }, source: 'agent' });
    expect((vol.result as { volume: number }).volume).toBe(1.0);
    const bbox = await core.dispatcher.dispatch({ commandId: 'measure.bounding_box', params: { shape_id: 'shape_1' }, source: 'agent' });
    expect(bbox.ok).toBe(true);
  });

  it('checks and fixes a shape (fix bumps tessellationRev)', async () => {
    const core = createCore({ rpc: backend() });
    await makeBox(core);
    const check = await core.dispatcher.dispatch({ commandId: 'repair.check', params: { shape_id: 'shape_1' }, source: 'agent' });
    expect((check.result as { valid: boolean }).valid).toBe(false);
    const rev0 = core.store.getState().doc.geometry.nodes['shape_1'].tessellationRev;
    await core.dispatcher.dispatch({ commandId: 'repair.fix', params: { shape_id: 'shape_1' }, source: 'agent' });
    expect(core.store.getState().doc.geometry.nodes['shape_1'].tessellationRev).toBe(rev0 + 1);
  });

  it('sets display render mode, visibility, and section plane', async () => {
    const core = createCore({ rpc: backend() });
    await makeBox(core);
    await core.dispatcher.dispatch({ commandId: 'display.set_render_mode', params: { mode: 'wireframe' }, source: 'agent' });
    expect(core.store.getState().display.renderMode).toBe('wireframe');
    await core.dispatcher.dispatch({ commandId: 'display.set_visibility', params: { shape_id: 'shape_1', visible: false }, source: 'agent' });
    expect(core.store.getState().doc.geometry.nodes['shape_1'].visible).toBe(false);
    await core.dispatcher.dispatch({ commandId: 'display.set_section_plane', params: { enabled: true, axis: 'z', offset: 0.5 }, source: 'agent' });
    expect(core.store.getState().display.sectionPlane).toEqual({ enabled: true, axis: 'z', offset: 0.5 });
  });

  it('configures setup and feeds it into a real solve', async () => {
    const core = createCore({ rpc: backend() });
    await core.dispatcher.dispatch({ commandId: 'setup.set_model', params: { key: 'turbulence', value: 'k-epsilon' }, source: 'agent' });
    expect(core.store.getState().physics.models.turbulence).toBe('k-epsilon');

    await core.dispatcher.dispatch({ commandId: 'setup.set_material', params: { property: 'viscosity', value: 0.001 }, source: 'agent' });
    expect(core.store.getState().physics.material.viscosity).toBe(0.001);

    await core.dispatcher.dispatch({ commandId: 'setup.add_boundary', params: { patch: 'inlet', type: 'velocity_inlet', parameters: { vx: 1 } }, source: 'agent' });
    await core.dispatcher.dispatch({ commandId: 'setup.add_boundary', params: { patch: 'inlet', type: 'velocity_inlet', parameters: { vx: 2 } }, source: 'agent' });
    expect(core.store.getState().setup.boundaries).toHaveLength(1); // replaced, not duplicated
    expect(core.store.getState().setup.boundaries[0].parameters.vx).toBe(2);

    await core.dispatcher.dispatch({ commandId: 'setup.set_solver', params: { maxIterations: 500, method: 'PISO' }, source: 'agent' });
    expect(core.store.getState().setup.solver.maxIterations).toBe(500);

    // calc.run reads setup → solver.maxIterations reflects setup, not the default 200.
    await core.dispatcher.dispatch({ commandId: 'mesh.generate', params: { nx: 5, ny: 5, nz: 0 }, source: 'agent' });
    const run = await core.dispatcher.dispatch({ commandId: 'calc.run', params: { pollIntervalMs: 1 }, source: 'agent' });
    expect(run.ok).toBe(true);
    expect(core.store.getState().solver.maxIterations).toBe(500);
  });

  it('removes a boundary', async () => {
    const core = createCore({ rpc: backend() });
    await core.dispatcher.dispatch({ commandId: 'setup.add_boundary', params: { patch: 'outlet', type: 'pressure_outlet' }, source: 'agent' });
    const r = await core.dispatcher.dispatch({ commandId: 'setup.remove_boundary', params: { patch: 'outlet' }, source: 'agent' });
    expect(r.ok).toBe(true);
    expect(core.store.getState().setup.boundaries).toHaveLength(0);
  });
});
