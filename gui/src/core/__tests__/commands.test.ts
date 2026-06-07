import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createMockRpcClient } from '../transport/rpcClient';
import type { JsonObject } from '../types';
import type { CoreEvent } from '../command';

/** A fake gfd-server: needs a mesh before it will "solve". */
function fakeBackend() {
  let hasMesh = false;
  let statusCalls = 0;
  return createMockRpcClient((method: string, params: JsonObject) => {
    switch (method) {
      case 'mesh.generate':
        hasMesh = true;
        return { cells: 400, faces: 840, nodes: 441, quality: { min_ortho: 0.95, max_skew: 0.1, max_ar: 1.0, bad_cells: 0 } };
      case 'solve.start':
        if (!hasMesh) throw new Error('No mesh generated yet. Call mesh.generate first.');
        statusCalls = 0;
        return { job_id: 'job_1' };
      case 'solve.status': {
        statusCalls += 1;
        const running = statusCalls < 3;
        return { running, iteration: statusCalls, residual: 0.1 / statusCalls, elapsed_ms: statusCalls * 5, ...(running ? {} : { status: 'converged' }) };
      }
      case 'field.get':
        return params.field === 'pressure'
          ? { values: [1, 2, 3], min: 1, max: 3, mean: 2 }
          : (() => { throw new Error('not found'); })();
      default:
        return {};
    }
  });
}

function waitFor(predicate: () => boolean, timeoutMs = 2000): Promise<void> {
  return new Promise((resolve, reject) => {
    const start = Date.now();
    const tick = () => {
      if (predicate()) return resolve();
      if (Date.now() - start > timeoutMs) return reject(new Error('timeout'));
      setTimeout(tick, 5);
    };
    tick();
  });
}

describe('Phase 1 real-backend commands', () => {
  it('registers the core command catalogue', () => {
    const core = createCore({ rpc: createMockRpcClient(() => ({})) });
    for (const id of ['system.version', 'mesh.generate', 'calc.run', 'calc.stop', 'results.load_field']) {
      expect(core.registry.has(id)).toBe(true);
    }
  });

  it('runs the full mesh → solve → results flow against a fake backend', async () => {
    const events: CoreEvent[] = [];
    const core = createCore({ rpc: fakeBackend(), onEvent: (e) => events.push(e) });

    const mesh = await core.dispatcher.dispatch({ commandId: 'mesh.generate', params: { nx: 20, ny: 20, nz: 0 }, source: 'agent' });
    expect(mesh.ok).toBe(true);
    expect(core.store.getState().mesh?.cellCount).toBe(400);

    const run = await core.dispatcher.dispatch({ commandId: 'calc.run', params: { pollIntervalMs: 1 }, source: 'agent' });
    expect(run.ok).toBe(true);
    expect(core.store.getState().solver.status).toBe('running');

    await waitFor(() => core.store.getState().solver.status === 'converged');

    const state = core.store.getState();
    expect(state.solver.iteration).toBeGreaterThanOrEqual(3);
    expect(state.results?.availableFields).toContain('pressure');
    expect(events.some((e) => e.type === 'solver-residual')).toBe(true);
    expect(events.some((e) => e.type === 'solver-done')).toBe(true);

    const field = await core.dispatcher.dispatch({ commandId: 'results.load_field', params: { field: 'pressure' }, source: 'agent' });
    expect(field.ok).toBe(true);
    expect(state.results?.activeField).toBe('pressure');
  });

  it('surfaces a backend error when solving without a mesh', async () => {
    const core = createCore({ rpc: fakeBackend() });
    const run = await core.dispatcher.dispatch({ commandId: 'calc.run', params: { pollIntervalMs: 1 }, source: 'agent' });
    expect(run.ok).toBe(false);
    expect(run.error?.message).toMatch(/mesh/i);
  });
});
