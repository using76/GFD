import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createInitialState, type AppState, type GeometryNode } from '../state';
import { createMockRpcClient } from '../transport/rpcClient';
import { diagnoseState, type DiagnoseResult } from '../commands/calc';

function nodeUnitBox(id: string): GeometryNode {
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

function baseState(): AppState {
  const s = createInitialState();
  s.doc.geometry.nodes['shape_1'] = nodeUnitBox('shape_1');
  s.doc.geometry.roots = ['shape_1'];
  return s;
}

describe('calc.diagnose — analyze → identify → fix', () => {
  it('flags divergence (NaN/Inf field) as an error with a relaxation fix', () => {
    const s = baseState();
    s.solver = { ...s.solver, status: 'finished', iteration: 30, residual: 1e3 };
    s.results = {
      availableFields: ['velocity_magnitude'],
      activeField: 'velocity_magnitude',
      fieldStats: { velocity_magnitude: { min: 0, max: Infinity, mean: NaN } },
    };
    const d: DiagnoseResult = diagnoseState(s);
    const div = d.issues.find((i) => i.code === 'DIVERGENCE');
    expect(div).toBeTruthy();
    expect(div?.severity).toBe('error');
    expect(div?.fix?.command).toBe('setup.set_solver');
    expect(typeof (div?.fix?.params as { relaxVelocity: number }).relaxVelocity).toBe('number');
    expect(d.converged).toBe(false);
  });

  it('detects a turbulent Reynolds regime under a laminar model and suggests k_epsilon', () => {
    const s = baseState();
    // water-like: Re = rho*U*L/mu = 1000*5*1/0.001 = 5e6 → turbulent
    s.physics.material = { ...s.physics.material, density: 1000, viscosity: 0.001 };
    s.physics.models = { ...s.physics.models, flow: 'incompressible', turbulence: 'none' };
    s.solver = { ...s.solver, status: 'converged', iteration: 80, residual: 5e-5 };
    s.results = {
      availableFields: ['velocity_magnitude'],
      activeField: 'velocity_magnitude',
      fieldStats: { velocity_magnitude: { min: 0, max: 5, mean: 2 } },
    };
    const d = diagnoseState(s);
    expect(d.reynolds).not.toBeNull();
    expect(d.flowRegime).toBe('turbulent');
    const turb = d.issues.find((i) => i.code === 'TURBULENCE_MODEL');
    expect(turb?.severity).toBe('warning');
    expect(turb?.fix).toEqual({ command: 'setup.set_model', params: { key: 'turbulence', value: 'k_epsilon' } });
  });

  it('gives a clean bill of health for a converged laminar solve', () => {
    const s = baseState();
    s.physics.material = { ...s.physics.material, density: 1, viscosity: 0.1 };
    s.physics.models = { ...s.physics.models, flow: 'incompressible', turbulence: 'none' };
    s.solver = { ...s.solver, status: 'converged', iteration: 40, residual: 1e-6 };
    s.results = {
      availableFields: ['velocity_magnitude', 'pressure'],
      activeField: 'velocity_magnitude',
      fieldStats: {
        velocity_magnitude: { min: 0, max: 0.5, mean: 0.2 }, // Re = 1*0.5*1/0.1 = 5 → laminar
        pressure: { min: -1, max: 1, mean: 0 },
      },
    };
    const d = diagnoseState(s);
    expect(d.flowRegime).toBe('laminar');
    expect(d.converged).toBe(true);
    expect(d.issues.some((i) => i.severity === 'error' || i.severity === 'warning')).toBe(false);
    expect(d.issues.some((i) => i.code === 'OK')).toBe(true);
  });

  it('reports NO_RESULTS when no solve has run', () => {
    const d = diagnoseState(createInitialState());
    expect(d.issues.some((i) => i.code === 'NO_RESULTS')).toBe(true);
    expect(d.reynolds).toBeNull();
  });

  it('is registered and dispatchable, returning a summary', async () => {
    const core = createCore({ rpc: createMockRpcClient(() => ({})) });
    const r = await core.dispatcher.dispatch({ commandId: 'calc.diagnose', params: {}, source: 'agent' });
    expect(r.ok).toBe(true);
    expect((r.result as DiagnoseResult).summary).toContain('status=');
  });
});
