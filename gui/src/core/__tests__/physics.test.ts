import { describe, it, expect } from 'vitest';
import { createCore } from '../index';
import { createMockRpcClient } from '../transport/rpcClient';
import type { JsonObject } from '../types';
import type { PhysicsTerm } from '../physics/manifest';

function physicsBackend() {
  return createMockRpcClient((method: string, params: JsonObject) => {
    if (method === 'physics.validate_expression') {
      const expr = String(params.expr);
      return { valid: !expr.includes('@'), diagnostics: [], latex: `\\mathrm{${expr}}` };
    }
    if (method === 'physics.apply_manifest') return { ok: true, terms_validated: 4 };
    return {};
  });
}

describe('Phase 7 pluggable physics', () => {
  it('ships a default manifest in initial state', () => {
    const core = createCore({ rpc: physicsBackend() });
    const m = core.store.getState().physics.manifest;
    expect(m?.id).toBe('incompressible_ns');
    expect(m?.equations.find((e) => e.id === 'momentum_x')).toBeDefined();
  });

  it('sets a constitutive relation to an expression and undoes it', async () => {
    const core = createCore({ rpc: physicsBackend() });
    const r = await core.dispatcher.dispatch({
      commandId: 'physics.set_constitutive',
      params: { property: 'viscosity', impl: { kind: 'expression', expr: '$mu0*exp(-b*$T)' } },
      source: 'agent',
    });
    expect(r.ok).toBe(true);
    const visc = core.store.getState().physics.manifest!.constitutive.find((c) => c.property === 'viscosity');
    expect(visc?.impl).toEqual({ kind: 'expression', expr: '$mu0*exp(-b*$T)' });

    expect(await core.dispatcher.undo()).toBe(true);
    const after = core.store.getState().physics.manifest!.constitutive.find((c) => c.property === 'viscosity');
    expect(after?.impl).toEqual({ kind: 'constant', value: 0.01 });
  });

  it('adds, replaces, and removes equation terms', async () => {
    const core = createCore({ rpc: physicsBackend() });
    const term: PhysicsTerm = {
      id: 'momentum_x_buoyancy',
      role: 'source',
      impl: { kind: 'expression', expr: '$rho*$beta*($T-$Tref)*$g', linearizeOver: 'T' },
      zone: { by: 'all' },
      enabled: true,
    };
    await core.dispatcher.dispatch({ commandId: 'physics.set_term', params: { equationId: 'momentum_x', term: term as unknown as JsonObject }, source: 'agent' });
    let eq = core.store.getState().physics.manifest!.equations.find((e) => e.id === 'momentum_x')!;
    expect(eq.terms.some((t) => t.id === 'momentum_x_buoyancy')).toBe(true);

    await core.dispatcher.dispatch({ commandId: 'physics.remove_term', params: { equationId: 'momentum_x', termId: 'momentum_x_buoyancy' }, source: 'agent' });
    eq = core.store.getState().physics.manifest!.equations.find((e) => e.id === 'momentum_x')!;
    expect(eq.terms.some((t) => t.id === 'momentum_x_buoyancy')).toBe(false);
  });

  it('validates expressions through the backend', async () => {
    const core = createCore({ rpc: physicsBackend() });
    const good = await core.dispatcher.dispatch({ commandId: 'physics.validate_expression', params: { expr: 'div($rho*$U)' }, source: 'agent' });
    expect(good.ok).toBe(true);
    expect((good.result as { valid: boolean }).valid).toBe(true);
  });

  it('rejects terms on unknown equations', async () => {
    const core = createCore({ rpc: physicsBackend() });
    const r = await core.dispatcher.dispatch({
      commandId: 'physics.set_term',
      params: { equationId: 'nope', term: { id: 't', role: 'source', impl: { kind: 'builtin', name: 'x' }, zone: { by: 'all' }, enabled: true } },
      source: 'agent',
    });
    expect(r.ok).toBe(false);
    expect(r.error?.code).toBe('UNKNOWN_EQUATION');
  });

  it('applies the manifest to the backend', async () => {
    const core = createCore({ rpc: physicsBackend() });
    const r = await core.dispatcher.dispatch({ commandId: 'physics.apply_manifest', params: {}, source: 'agent' });
    expect(r.ok).toBe(true);
    expect((r.result as { applied: boolean }).applied).toBe(true);
  });
});
