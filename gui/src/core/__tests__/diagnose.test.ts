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

  it('flags a low-orthogonality mesh and suggests a finer re-mesh', () => {
    const s = baseState();
    s.mesh = {
      generated: true,
      cellCount: 400,
      nodeCount: 500,
      quality: { minOrthogonality: 0.05, maxSkewness: 0.4, maxAspectRatio: 8 },
      badCells: 0,
      gen: { nx: 20, ny: 20, nz: 0 },
    };
    s.solver = { ...s.solver, status: 'converged', iteration: 30, residual: 1e-5 };
    s.results = {
      availableFields: ['velocity_magnitude'],
      activeField: 'velocity_magnitude',
      fieldStats: { velocity_magnitude: { min: 0, max: 0.5, mean: 0.2 } },
    };
    const d = diagnoseState(s);
    expect(d.mesh?.cells).toBe(400);
    const mq = d.issues.find((i) => i.code === 'MESH_ORTHOGONALITY');
    expect(mq?.severity).toBe('warning');
    expect(mq?.fix).toEqual({ command: 'mesh.generate', params: { nx: 40, ny: 40, nz: 0 } });
  });

  it('flags bad cells with priority over other mesh metrics', () => {
    const s = baseState();
    s.mesh = {
      generated: true,
      cellCount: 400,
      nodeCount: 500,
      quality: { minOrthogonality: 0.05, maxSkewness: 0.95, maxAspectRatio: 8 },
      badCells: 7,
      gen: { nx: 16, ny: 16, nz: 4 },
    };
    const d = diagnoseState(s);
    expect(d.issues.find((i) => i.code === 'MESH_BAD_CELLS')?.fix?.params).toEqual({ nx: 32, ny: 32, nz: 8 });
    expect(d.issues.some((i) => i.code === 'MESH_ORTHOGONALITY')).toBe(false);
  });

  it('identifies the dominant non-converged equation and targets its relaxation', () => {
    const s = baseState();
    s.physics.material = { ...s.physics.material, density: 1, viscosity: 0.1 };
    s.solver = {
      ...s.solver,
      status: 'finished',
      iteration: 200,
      residual: 1e-2,
      residualsByEq: { vx: 5e-2, vy: 1e-4, vz: 1e-4, pressure: 2e-4, continuity: 1e-2 },
    };
    s.results = {
      availableFields: ['velocity_magnitude'],
      activeField: 'velocity_magnitude',
      fieldStats: { velocity_magnitude: { min: 0, max: 0.5, mean: 0.2 } },
    };
    const d = diagnoseState(s);
    expect(d.dominantEquation).toBe('vx');
    const dom = d.issues.find((i) => i.code === 'DOMINANT_EQUATION');
    expect(dom).toBeTruthy();
    // vx is a momentum equation → fix targets relaxVelocity, not relaxPressure.
    expect((dom?.fix?.params as { relaxVelocity?: number }).relaxVelocity).toBeDefined();
    expect((dom?.fix?.params as { relaxPressure?: number }).relaxPressure).toBeUndefined();
  });

  it('does not flag a dominant equation once converged', () => {
    const s = baseState();
    s.solver = {
      ...s.solver,
      status: 'converged',
      iteration: 60,
      residual: 1e-6,
      residualsByEq: { vx: 5e-7, vy: 1e-7, vz: 1e-7, pressure: 2e-7, continuity: 1e-6 },
    };
    s.results = {
      availableFields: ['velocity_magnitude'],
      activeField: 'velocity_magnitude',
      fieldStats: { velocity_magnitude: { min: 0, max: 0.5, mean: 0.2 } },
    };
    const d = diagnoseState(s);
    expect(typeof d.dominantEquation).toBe('string'); // largest is still reported…
    expect(d.issues.some((i) => i.code === 'DOMINANT_EQUATION')).toBe(false); // …but not an issue
  });

  it('flags a stalled residual (flat trend above tolerance) with a relaxation fix', () => {
    const s = baseState();
    s.solver = {
      ...s.solver,
      status: 'finished',
      iteration: 120,
      residual: 1e-2,
      // last 6 nearly flat → stalled
      residualHistory: [1.05e-2, 1.02e-2, 1.01e-2, 1.0e-2, 1.0e-2, 1.0e-2],
    };
    s.results = {
      availableFields: ['velocity_magnitude'],
      activeField: 'velocity_magnitude',
      fieldStats: { velocity_magnitude: { min: 0, max: 0.5, mean: 0.2 } },
    };
    const d = diagnoseState(s);
    expect(d.convergenceTrend).toBe('stalled');
    const st = d.issues.find((i) => i.code === 'STALLED');
    expect(st?.severity).toBe('warning');
    expect((st?.fix?.params as { relaxVelocity?: number }).relaxVelocity).toBeDefined();
  });

  it('classifies a dropping residual trace as converging (no STALLED issue)', () => {
    const s = baseState();
    s.solver = {
      ...s.solver,
      status: 'finished',
      iteration: 50,
      residual: 1e-3,
      residualHistory: [1e-1, 5e-2, 2e-2, 1e-2, 3e-3, 1e-3],
    };
    s.results = {
      availableFields: ['velocity_magnitude'],
      activeField: 'velocity_magnitude',
      fieldStats: { velocity_magnitude: { min: 0, max: 0.5, mean: 0.2 } },
    };
    const d = diagnoseState(s);
    expect(d.convergenceTrend).toBe('converging');
    expect(d.issues.some((i) => i.code === 'STALLED')).toBe(false);
  });

  it('flags an inlet without an outlet (ill-posed) and suggests a pressure outlet', () => {
    const s = baseState();
    s.physics.models = { ...s.physics.models, flow: 'incompressible' };
    s.setup.boundaries = [{ patch: 'in', type: 'velocity_inlet', parameters: { vx: 1 } }];
    const d = diagnoseState(s);
    const iss = d.issues.find((i) => i.code === 'INLET_NO_OUTLET');
    expect(iss?.severity).toBe('warning');
    expect(iss?.fix?.command).toBe('setup.add_boundary');
    expect((iss?.fix?.params as { type: string }).type).toBe('pressure_outlet');
  });

  it('does not flag INLET_NO_OUTLET once a pressure outlet exists', () => {
    const s = baseState();
    s.physics.models = { ...s.physics.models, flow: 'incompressible' };
    s.setup.boundaries = [
      { patch: 'in', type: 'velocity_inlet', parameters: { vx: 1 } },
      { patch: 'out', type: 'pressure_outlet', parameters: { pressure: 0 } },
    ];
    expect(diagnoseState(s).issues.some((i) => i.code === 'INLET_NO_OUTLET')).toBe(false);
  });

  it('flags an energy model with no thermal boundary condition', () => {
    const s = baseState();
    s.physics.models = { ...s.physics.models, flow: 'incompressible', energy: true };
    s.setup.boundaries = [{ patch: 'wall', type: 'wall', parameters: {} }];
    expect(diagnoseState(s).issues.some((i) => i.code === 'ENERGY_NO_THERMAL_BC')).toBe(true);
  });

  it('is registered and dispatchable, returning a summary', async () => {
    const core = createCore({ rpc: createMockRpcClient(() => ({})) });
    const r = await core.dispatcher.dispatch({ commandId: 'calc.diagnose', params: {}, source: 'agent' });
    expect(r.ok).toBe(true);
    expect((r.result as DiagnoseResult).summary).toContain('status=');
  });

  it('caches the diagnosis into AppState for the UI / get_state', async () => {
    const core = createCore({ rpc: createMockRpcClient(() => ({})) });
    expect(core.store.getState().diagnosis).toBeNull();
    await core.dispatcher.dispatch({ commandId: 'calc.diagnose', params: {}, source: 'agent' });
    const cached = core.store.getState().diagnosis as DiagnoseResult | null;
    expect(cached).not.toBeNull();
    expect(cached?.summary).toContain('status=');
    expect(Array.isArray(cached?.issues)).toBe(true);
  });
});
