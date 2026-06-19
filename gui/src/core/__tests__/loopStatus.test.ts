import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createInitialState, type AppState, type GeometryNode } from '../state';
import { createMockRpcClient } from '../transport/rpcClient';
import type { LoopStatusResult } from '../commands/system';

function node(id: string): GeometryNode {
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

async function statusOf(mutate: (s: AppState) => void): Promise<LoopStatusResult> {
  const s = createInitialState();
  mutate(s);
  const core = createCore({ rpc: createMockRpcClient(() => ({})), initialState: s });
  const r = await core.dispatcher.dispatch({ commandId: 'system.loop_status', params: {}, source: 'agent' });
  return r.result as LoopStatusResult;
}

function withMesh(s: AppState) {
  s.mesh = { generated: true, cellCount: 400, nodeCount: 441, quality: null };
}

describe('system.loop_status — next-step guidance', () => {
  it('recommends geometry first on an empty document', async () => {
    const r = await statusOf(() => {});
    expect(r.steps.geometry.done).toBe(false);
    expect(r.nextAction?.step).toBe('geometry');
    expect(r.readyToSolve).toBe(false);
  });

  it('recommends meshing once geometry exists', async () => {
    const r = await statusOf((s) => {
      s.doc.geometry.nodes['shape_1'] = node('shape_1');
    });
    expect(r.steps.geometry.done).toBe(true);
    expect(r.nextAction?.command).toBe('mesh.generate');
  });

  it('recommends boundary setup after meshing', async () => {
    const r = await statusOf((s) => {
      s.doc.geometry.nodes['shape_1'] = node('shape_1');
      withMesh(s);
    });
    expect(r.nextAction?.step).toBe('setup');
    expect(r.readyToSolve).toBe(false);
  });

  it('recommends solving once geometry+mesh+boundaries exist', async () => {
    const r = await statusOf((s) => {
      s.doc.geometry.nodes['shape_1'] = node('shape_1');
      withMesh(s);
      s.setup.boundaries = [{ patch: 'inlet', type: 'velocity_inlet', parameters: { vx: 1 } }];
    });
    expect(r.readyToSolve).toBe(true);
    expect(r.nextAction?.command).toBe('calc.run');
  });

  it('recommends diagnosis after a solve, then fixing when problems remain', async () => {
    const solvedMut = (s: AppState) => {
      s.doc.geometry.nodes['shape_1'] = node('shape_1');
      withMesh(s);
      s.setup.boundaries = [{ patch: 'inlet', type: 'velocity_inlet', parameters: { vx: 1 } }];
      s.solver = { ...s.solver, status: 'converged', iteration: 50, residual: 1e-5 };
      s.results = { availableFields: ['velocity_magnitude'], activeField: 'velocity_magnitude', fieldStats: {} };
    };
    const afterSolve = await statusOf(solvedMut);
    expect(afterSolve.nextAction?.command).toBe('calc.diagnose');

    const withProblem = await statusOf((s) => {
      solvedMut(s);
      s.diagnosis = { summary: 'x', issues: [{ severity: 'warning', code: 'TURBULENCE_MODEL', message: 'm' }] };
    });
    expect(withProblem.nextAction?.step).toBe('fix');

    const healthy = await statusOf((s) => {
      solvedMut(s);
      s.diagnosis = { summary: 'ok', issues: [{ severity: 'info', code: 'OK', message: 'ok' }] };
    });
    expect(healthy.nextAction).toBeNull();
    expect(healthy.summary).toContain('complete');
  });
});
