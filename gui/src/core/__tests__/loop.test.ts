/**
 * End-to-end integration of the AI simulation loop: import a CAD file → mesh →
 * setup → solve → analyze → (self-correct). Proves the whole spine composes
 * through the real command pipeline + MCP bridge, not just unit by unit.
 */
import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createMockRpcClient } from '../transport/rpcClient';
import { createMcpBridge } from '../mcp/bridge';
import type { JsonObject } from '../types';
import type { DiagnoseResult } from '../commands/calc';
import type { AutoRefineResult } from '../solver/autoRefine';

/** A fake gfd-server covering the whole loop; `vmax` sets the result velocity. */
function loopBackend(vmax = 0.5) {
  let hasMesh = false;
  let statusCalls = 0;
  let shapeCounter = 0;
  return createMockRpcClient((method: string) => {
    switch (method) {
      case 'cad.import.mesh_to_tree':
        shapeCounter += 1;
        return {
          shape_id: `shape_${shapeCounter}`,
          arena_id: 0,
          kind: 'imported_stl',
          triangle_count: 12,
          vertex_count: 8,
          bbox: { min: [-1, -1, -1], max: [1, 1, 1] },
        };
      case 'mesh.generate':
        hasMesh = true;
        return { cells: 400, faces: 840, nodes: 441, quality: { min_ortho: 0.95, max_skew: 0.1, max_ar: 1.0, bad_cells: 0 } };
      case 'solve.start':
        if (!hasMesh) throw new Error('No mesh generated yet. Call mesh.generate first.');
        statusCalls = 0;
        return { job_id: 'job_1' };
      case 'solve.status': {
        statusCalls += 1;
        const running = statusCalls < 2;
        return {
          running,
          iteration: statusCalls,
          residual: 1e-5,
          elapsed_ms: 5,
          ...(running
            ? {}
            : { status: 'converged', residuals: { vx: 5e-6, vy: 1e-6, vz: 1e-6, pressure: 2e-6, continuity: 1e-5 } }),
        };
      }
      case 'field.get':
        return { values: [0.1, 0.2, 0.3], min: 0, max: vmax, mean: vmax / 2 };
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

describe('AI simulation loop — end to end', () => {
  it('imports a CAD file, meshes, sets up, solves, and diagnoses healthy', async () => {
    const core = createCore({ rpc: loopBackend(0.5) });
    const run = (commandId: string, params: JsonObject = {}) =>
      core.dispatcher.dispatch({ commandId, params, source: 'agent' });

    // 1. read a CAD file into the tree
    const imp = await run('io.import_mesh', { path: '/models/part.stl' });
    expect(imp.ok).toBe(true);
    expect(core.store.getState().doc.geometry.nodes['shape_1']).toBeTruthy();

    // 2. mesh
    await run('mesh.generate', { nx: 20, ny: 20, nz: 0 });
    expect(core.store.getState().mesh?.generated).toBe(true);

    // 3. setup (model / material / boundary)
    await run('setup.set_model', { key: 'flow', value: 'incompressible' });
    await run('setup.set_material', { property: 'viscosity', value: 0.1 });
    await run('setup.add_boundary', { patch: 'inlet', type: 'velocity_inlet', parameters: { vx: 1 } });
    // well-posed: an open inlet needs a pressure outlet (else diagnose flags it)
    await run('setup.add_boundary', { patch: 'outlet', type: 'pressure_outlet', parameters: { pressure: 0 } });

    // 4. solve
    const solved = await run('calc.run', { pollIntervalMs: 1 });
    expect(solved.ok).toBe(true);
    await waitFor(() => core.store.getState().solver.status === 'converged');
    expect(core.store.getState().results?.availableFields).toContain('velocity_magnitude');
    // per-equation residuals threaded through to state
    expect(core.store.getState().solver.residualsByEq?.vx).toBeDefined();

    // 5. analyze — converged + no problems, cached in state
    const diag = await run('calc.diagnose', {});
    const d = diag.result as DiagnoseResult;
    expect(d.converged).toBe(true);
    expect(d.issues.some((i) => i.severity === 'error' || i.severity === 'warning')).toBe(false);
    expect(core.store.getState().diagnosis).not.toBeNull();
  });

  it('self-corrects a turbulent-regime/laminar-model setup via auto_refine', async () => {
    const core = createCore({ rpc: loopBackend(5) });
    const run = (commandId: string, params: JsonObject = {}) =>
      core.dispatcher.dispatch({ commandId, params, source: 'agent' });

    await run('io.import_mesh', { path: '/models/part.stl' });
    await run('mesh.generate', { nx: 20, ny: 20, nz: 0 });
    // High Reynolds (water-like, fast) but a laminar model.
    await run('setup.set_material', { property: 'density', value: 1000 });
    await run('setup.set_material', { property: 'viscosity', value: 0.0001 });
    await run('calc.run', { pollIntervalMs: 1 });
    await waitFor(() => core.store.getState().solver.status === 'converged');

    // diagnose flags the turbulence-model mismatch
    const d0 = (await run('calc.diagnose', {})).result as DiagnoseResult;
    expect(d0.flowRegime).toBe('turbulent');
    expect(d0.issues.some((i) => i.code === 'TURBULENCE_MODEL')).toBe(true);

    // auto_refine (via the MCP meta-tool an agent would call) fixes it
    const bridge = createMcpBridge(core);
    const r = await bridge.callTool('auto_refine', { maxRounds: 3 });
    expect(r.ok).toBe(true);
    expect((r.content as unknown as AutoRefineResult).healthy).toBe(true);
    expect(core.store.getState().physics.models.turbulence).toBe('k_epsilon');
  });
});
