/**
 * Setup commands (Phase 4) — physics models, material, boundary conditions, and
 * solver settings. These populate the state that calc.run reads to drive the
 * real solver, so an AI agent can fully configure a simulation via commands.
 */

import type { JsonValue } from '../types';
import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';
import type { BoundaryCondition, SolverSettings } from '../state';
import type { PatchOp } from '../patch';

export interface SetModelParams {
  key: 'flow' | 'turbulence' | 'energy' | 'multiphase' | 'radiation';
  value: JsonValue;
}

export const setModel: CommandDef<SetModelParams, { key: string; value: JsonValue }> = {
  id: 'setup.set_model',
  category: 'setup',
  group: 'Models',
  title: 'Set Physics Model',
  titleKo: '물리 모델 설정',
  description: 'Set a physics model selection (flow, turbulence, energy, multiphase, radiation).',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      key: { type: 'string', enum: ['flow', 'turbulence', 'energy', 'multiphase', 'radiation'] },
      value: {},
    },
    required: ['key', 'value'],
  },
  async run(params) {
    const value = params.key === 'energy' ? Boolean(params.value) : params.value;
    return {
      ok: true,
      result: { key: params.key, value },
      statePatch: [{ op: 'replace', path: ['physics', 'models', params.key], value }],
    };
  },
};

export interface SetMaterialParams {
  property: 'name' | 'density' | 'viscosity' | 'cp' | 'conductivity';
  value: JsonValue;
}

export const setMaterial: CommandDef<SetMaterialParams, { property: string; value: JsonValue }> = {
  id: 'setup.set_material',
  category: 'setup',
  group: 'Material',
  title: 'Set Material Property',
  titleKo: '재료 물성 설정',
  description: 'Set a fluid material property (name, density, viscosity, cp, conductivity).',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      property: { type: 'string', enum: ['name', 'density', 'viscosity', 'cp', 'conductivity'] },
      value: {},
    },
    required: ['property', 'value'],
  },
  async run(params) {
    return {
      ok: true,
      result: { property: params.property, value: params.value },
      statePatch: [{ op: 'replace', path: ['physics', 'material', params.property], value: params.value }],
    };
  },
};

export interface AddBoundaryParams {
  patch: string;
  type: string;
  parameters?: Record<string, number>;
}

export const addBoundary: CommandDef<AddBoundaryParams, BoundaryCondition> = {
  id: 'setup.add_boundary',
  category: 'setup',
  group: 'Boundaries',
  title: 'Add Boundary Condition',
  titleKo: '경계 조건 추가',
  description: 'Add or replace a boundary condition on a patch (wall/velocity_inlet/pressure_outlet/symmetry...).',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      patch: { type: 'string' },
      type: { type: 'string' },
      parameters: { type: 'object' },
    },
    required: ['patch', 'type'],
  },
  async run(params, ctx) {
    const bc: BoundaryCondition = { patch: params.patch, type: params.type, parameters: params.parameters ?? {} };
    const list = ctx.getState().setup.boundaries;
    const idx = list.findIndex((b) => b.patch === params.patch);
    const patch: PatchOp[] =
      idx >= 0
        ? [{ op: 'replace', path: ['setup', 'boundaries', idx], value: bc as unknown as JsonValue }]
        : [{ op: 'add', path: ['setup', 'boundaries', list.length], value: bc as unknown as JsonValue }];
    return { ok: true, result: bc, statePatch: patch };
  },
};

export interface RemoveBoundaryParams {
  patch: string;
}

export const removeBoundary: CommandDef<RemoveBoundaryParams, { removed: boolean }> = {
  id: 'setup.remove_boundary',
  category: 'setup',
  group: 'Boundaries',
  title: 'Remove Boundary Condition',
  titleKo: '경계 조건 제거',
  description: 'Remove the boundary condition on a patch.',
  capability: 'mutate-scene',
  paramsSchema: { type: 'object', properties: { patch: { type: 'string' } }, required: ['patch'] },
  async run(params, ctx) {
    const idx = ctx.getState().setup.boundaries.findIndex((b) => b.patch === params.patch);
    if (idx < 0) return { ok: false, error: { code: 'NO_BOUNDARY', message: `No boundary on "${params.patch}"` } };
    return {
      ok: true,
      result: { removed: true },
      statePatch: [{ op: 'remove', path: ['setup', 'boundaries', idx] }],
    };
  },
};

export interface SetSolverParams {
  method?: SolverSettings['method'];
  maxIterations?: number;
  tolerance?: number;
  relaxVelocity?: number;
  relaxPressure?: number;
}

export const setSolver: CommandDef<SetSolverParams, SolverSettings> = {
  id: 'setup.set_solver',
  category: 'setup',
  group: 'Solver',
  title: 'Set Solver Settings',
  titleKo: '솔버 설정',
  description: 'Configure the pressure–velocity coupling, iteration limit, tolerance, and relaxation factors.',
  capability: 'mutate-scene',
  paramsSchema: {
    type: 'object',
    properties: {
      method: { type: 'string', enum: ['SIMPLE', 'PISO', 'SIMPLEC'] },
      maxIterations: { type: 'integer', minimum: 1 },
      tolerance: { type: 'number', minimum: 0 },
      relaxVelocity: { type: 'number', minimum: 0, maximum: 1 },
      relaxPressure: { type: 'number', minimum: 0, maximum: 1 },
    },
  },
  async run(params, ctx) {
    const cur = ctx.getState().setup.solver;
    const next: SolverSettings = {
      method: params.method ?? cur.method,
      maxIterations: params.maxIterations ?? cur.maxIterations,
      tolerance: params.tolerance ?? cur.tolerance,
      relaxVelocity: params.relaxVelocity ?? cur.relaxVelocity,
      relaxPressure: params.relaxPressure ?? cur.relaxPressure,
    };
    return {
      ok: true,
      result: next,
      statePatch: [{ op: 'replace', path: ['setup', 'solver'], value: next as unknown as JsonValue }],
    };
  },
};

export function registerSetupCommands(registry: CommandRegistry): void {
  registry.register(setModel);
  registry.register(setMaterial);
  registry.register(addBoundary);
  registry.register(removeBoundary);
  registry.register(setSolver);
}
