/**
 * Physics commands (Phase 7) — runtime-pluggable governing equations and
 * constitutive relations, editable by both the human UI and an AI agent.
 *
 * Mutating commands replace the whole manifest in AppState (one replace patch →
 * exact undo). `physics.validate_expression` and `physics.apply_manifest` call
 * the backend (which reuses gfd-expression). Backends without the physics.*
 * namespace will surface a clear error — the GUI contract is forward-compatible.
 */

import type { JsonObject, JsonValue } from '../types';
import type { CommandDef, CommandContext } from '../command';
import type { CommandRegistry } from '../registry';
import type { PatchOp } from '../patch';
import {
  buildDefaultManifest,
  type ConstitutiveImpl,
  type PhysicsManifest,
  type PhysicsTerm,
} from '../physics/manifest';

function getManifest(ctx: CommandContext): PhysicsManifest {
  return ctx.getState().physics.manifest ?? buildDefaultManifest();
}

function manifestPatch(next: PhysicsManifest): PatchOp[] {
  return [{ op: 'replace', path: ['physics', 'manifest'], value: next as unknown as JsonValue }];
}

export interface ValidateExpressionParams {
  expr: string;
  fields?: string[];
}

export const validateExpression: CommandDef<ValidateExpressionParams, { valid: boolean; diagnostics: JsonValue; latex?: string }> = {
  id: 'physics.validate_expression',
  category: 'physics',
  group: 'Edit',
  title: 'Validate Expression',
  titleKo: '식 검증',
  description: 'Validate a GMN/gfd-expression PDE-term string and return diagnostics + LaTeX for live preview.',
  capability: 'read',
  paramsSchema: {
    type: 'object',
    properties: { expr: { type: 'string' }, fields: { type: 'array', items: { type: 'string' } } },
    required: ['expr'],
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<{ valid: boolean; diagnostics: JsonValue; latex?: string }>(
      'physics.validate_expression',
      { expr: params.expr, fields: params.fields ?? [] }
    );
    return { ok: true, result: r };
  },
};

export interface SetConstitutiveParams {
  property: string;
  impl: ConstitutiveImpl;
}

export const setConstitutive: CommandDef<SetConstitutiveParams, PhysicsManifest> = {
  id: 'physics.set_constitutive',
  category: 'physics',
  group: 'Edit',
  title: 'Set Constitutive Relation',
  titleKo: '구성 관계 설정',
  description: 'Set a material/constitutive relation (e.g. viscosity) to a constant or an expression.',
  capability: 'mutate-physics',
  paramsSchema: {
    type: 'object',
    properties: {
      property: { type: 'string' },
      impl: { type: 'object' },
    },
    required: ['property', 'impl'],
  },
  async run(params, ctx) {
    const manifest = structuredClone(getManifest(ctx));
    const idx = manifest.constitutive.findIndex((c) => c.property === params.property);
    if (idx >= 0) manifest.constitutive[idx].impl = params.impl;
    else manifest.constitutive.push({ property: params.property, impl: params.impl });
    return { ok: true, result: manifest, statePatch: manifestPatch(manifest) };
  },
};

export interface SetTermParams {
  equationId: string;
  term: PhysicsTerm;
}

export const setTerm: CommandDef<SetTermParams, PhysicsManifest> = {
  id: 'physics.set_term',
  category: 'physics',
  group: 'Edit',
  title: 'Set Equation Term',
  titleKo: '방정식 항 설정',
  description: 'Add or replace a term (builtin or expression) in a governing equation, addressed by equation id + term id.',
  capability: 'mutate-physics',
  paramsSchema: {
    type: 'object',
    properties: { equationId: { type: 'string' }, term: { type: 'object' } },
    required: ['equationId', 'term'],
  },
  async run(params, ctx) {
    const manifest = structuredClone(getManifest(ctx));
    const eq = manifest.equations.find((e) => e.id === params.equationId);
    if (!eq) {
      return { ok: false, error: { code: 'UNKNOWN_EQUATION', message: `No equation "${params.equationId}"` } };
    }
    const i = eq.terms.findIndex((t) => t.id === params.term.id);
    if (i >= 0) eq.terms[i] = params.term;
    else eq.terms.push(params.term);
    return { ok: true, result: manifest, statePatch: manifestPatch(manifest) };
  },
};

export interface RemoveTermParams {
  equationId: string;
  termId: string;
}

export const removeTerm: CommandDef<RemoveTermParams, PhysicsManifest> = {
  id: 'physics.remove_term',
  category: 'physics',
  group: 'Edit',
  title: 'Remove Equation Term',
  titleKo: '방정식 항 제거',
  description: 'Remove a term from a governing equation by equation id + term id.',
  capability: 'mutate-physics',
  paramsSchema: {
    type: 'object',
    properties: { equationId: { type: 'string' }, termId: { type: 'string' } },
    required: ['equationId', 'termId'],
  },
  async run(params, ctx) {
    const manifest = structuredClone(getManifest(ctx));
    const eq = manifest.equations.find((e) => e.id === params.equationId);
    if (!eq) {
      return { ok: false, error: { code: 'UNKNOWN_EQUATION', message: `No equation "${params.equationId}"` } };
    }
    eq.terms = eq.terms.filter((t) => t.id !== params.termId);
    return { ok: true, result: manifest, statePatch: manifestPatch(manifest) };
  },
};

export const applyManifest: CommandDef<Record<string, never>, { applied: boolean; status?: JsonValue }> = {
  id: 'physics.apply_manifest',
  category: 'physics',
  group: 'Apply',
  title: 'Apply Physics Model',
  titleKo: '물리 모델 적용',
  description: 'Send the current physics manifest to the backend, which validates all expressions and prepares the solver.',
  capability: 'mutate-physics',
  paramsSchema: { type: 'object', properties: {}, additionalProperties: false },
  async run(_params, ctx) {
    const manifest = getManifest(ctx);
    const status = await ctx.rpc.request<JsonValue>('physics.apply_manifest', {
      manifest: manifest as unknown as JsonObject,
    });
    return { ok: true, result: { applied: true, status } };
  },
};

export function registerPhysicsCommands(registry: CommandRegistry): void {
  registry.register(validateExpression);
  registry.register(setConstitutive);
  registry.register(setTerm);
  registry.register(removeTerm);
  registry.register(applyManifest);
}
