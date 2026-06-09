import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createInitialState, type AppState, type GeometryNode } from '../state';
import { createMockRpcClient } from '../transport/rpcClient';
import { createMcpBridge } from '../mcp/bridge';
import { runAutoRefine, type AutoRefineResult } from '../solver/autoRefine';

function unitBox(id: string): GeometryNode {
  return {
    id,
    arenaId: 1,
    name: id,
    kind: 'box',
    parentId: null,
    featureParams: {},
    transform: { position: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1] },
    bbox: { min: [0, 0, 0], max: [1, 1, 1] },
    faceIds: [],
    visible: true,
    tessellationRev: 0,
  };
}

/** A converged-but-diverged-field seed: a solve ran but blew up (Inf/NaN). */
function divergedState(): AppState {
  const s = createInitialState();
  s.doc.geometry.nodes['shape_1'] = unitBox('shape_1');
  s.doc.geometry.roots = ['shape_1'];
  s.physics.material = { ...s.physics.material, density: 1, viscosity: 0.1 };
  s.physics.models = { ...s.physics.models, flow: 'incompressible', turbulence: 'none' };
  s.setup.solver = { ...s.setup.solver, maxIterations: 100, relaxVelocity: 0.7, relaxPressure: 0.3 };
  s.solver = { ...s.solver, status: 'finished', iteration: 100, residual: 1e3 };
  s.results = {
    availableFields: ['velocity_magnitude'],
    activeField: 'velocity_magnitude',
    fieldStats: { velocity_magnitude: { min: 0, max: Infinity, mean: NaN } },
  };
  return s;
}

/** Mock backend whose re-runs always converge to a healthy laminar field. */
function healthyBackend() {
  return createMockRpcClient((method: string) => {
    if (method === 'solve.start') return { job_id: 'job1' };
    if (method === 'solve.status') return { running: false, iteration: 60, residual: 5e-5, status: 'converged' };
    if (method === 'field.get') return { values: [0.1, 0.2, 0.3], min: 0, max: 0.5, mean: 0.25 };
    return {};
  });
}

describe('auto-refine — autonomous diagnose→fix→re-run loop', () => {
  it('detects divergence, lowers relaxation, re-runs, and reaches a healthy state', async () => {
    const core = createCore({ rpc: healthyBackend(), initialState: divergedState() });
    const result: AutoRefineResult = await runAutoRefine(core, { maxRounds: 3, pollIntervalMs: 5 });

    expect(result.rounds.length).toBe(1);
    expect(result.rounds[0].issueCode).toBe('DIVERGENCE');
    expect(result.rounds[0].fixCommand).toBe('setup.set_solver');
    // The corrective fix actually lowered the velocity relaxation factor.
    expect(core.store.getState().setup.solver.relaxVelocity).toBeLessThan(0.7);
    // The re-run converged and the field is no longer blown up.
    expect(result.rounds[0].afterStatus).toBe('converged');
    expect(result.healthy).toBe(true);
    expect(result.stoppedReason).toBe('healthy');
    expect(result.finalDiagnosis.converged).toBe(true);
  });

  it('stops immediately (healthy) when there is nothing to fix', async () => {
    const s = createInitialState();
    s.doc.geometry.nodes['shape_1'] = unitBox('shape_1');
    s.doc.geometry.roots = ['shape_1'];
    s.physics.material = { ...s.physics.material, density: 1, viscosity: 0.1 };
    s.solver = { ...s.solver, status: 'converged', iteration: 40, residual: 1e-6 };
    s.results = {
      availableFields: ['velocity_magnitude'],
      activeField: 'velocity_magnitude',
      fieldStats: { velocity_magnitude: { min: 0, max: 0.5, mean: 0.2 } },
    };
    const core = createCore({ rpc: healthyBackend(), initialState: s });
    const result = await runAutoRefine(core, { maxRounds: 3, pollIntervalMs: 5 });
    expect(result.rounds.length).toBe(0);
    expect(result.healthy).toBe(true);
    expect(result.stoppedReason).toBe('healthy');
  });

  it('is exposed as the auto_refine MCP meta-tool', async () => {
    const core = createCore({ rpc: healthyBackend(), initialState: divergedState() });
    const bridge = createMcpBridge(core);
    expect(bridge.listTools().some((t) => t.name === 'auto_refine')).toBe(true);
    const r = await bridge.callTool('auto_refine', { maxRounds: 2 });
    expect(r.ok).toBe(true);
    expect((r.content as unknown as AutoRefineResult).healthy).toBe(true);
  });
});
