/**
 * Calculation commands — run the REAL Rust solver (no more Math.exp simulation).
 *
 * calc.run starts a solve via the realSolver runner, streams residuals as
 * `solver-residual` events, and on completion writes final status + result-field
 * summaries into AppState (field VALUES are fetched on demand via
 * results.load_field so AppState stays small and JSON-friendly).
 *
 * The runner handle is kept in a session registry so calc.stop can halt it.
 */

import type { JsonValue } from '../types';
import type { CommandDef, CommandContext } from '../command';
import type { CommandRegistry } from '../registry';
import type { PatchOp } from '../patch';
import type { ResultsSummary, SolverStatus } from '../state';
import { runRealSolver, type SolverConfig, type SolverHandle } from '../solver/realSolver';

/** Active solver handles, keyed by backend job id. */
const sessions = new Map<string, SolverHandle>();

export interface CalcRunParams {
  maxIterations?: number;
  tolerance?: number;
  /** Backend boundary-condition objects, passed through verbatim. */
  boundaryConditions?: JsonValue[];
  /** Poll cadence for solve.status (ms). */
  pollIntervalMs?: number;
}

function buildConfig(ctx: CommandContext, params: CalcRunParams): SolverConfig {
  const s = ctx.getState();
  const physics = s.physics;
  const isThermal = physics.models.flow === 'none';
  return {
    flow: physics.models.flow,
    turbulence: physics.models.turbulence,
    physics: isThermal ? 'thermal' : 'fluid',
    maxIterations: params.maxIterations ?? s.solver.maxIterations,
    tolerance: params.tolerance ?? 1e-4,
    density: physics.material.density,
    viscosity: physics.material.viscosity,
    conductivity: physics.material.conductivity,
    alphaU: 0.5,
    alphaP: 0.3,
    boundaryConditions: params.boundaryConditions ?? [],
  };
}

function solverPatch(partial: Partial<SolverStatus>): PatchOp[] {
  return Object.entries(partial).map(([key, value]) => ({
    op: 'replace',
    path: ['solver', key],
    value: value as JsonValue,
  }));
}

export const calcRun: CommandDef<CalcRunParams, { jobId: string }> = {
  id: 'calc.run',
  category: 'calc',
  group: 'Run',
  title: 'Run Solver',
  titleKo: '솔버 실행',
  description:
    'Start the real CFD/thermal solver on the current backend mesh and stream residuals until convergence. Requires a generated mesh.',
  capability: 'run-solver',
  undoable: false,
  paramsSchema: {
    type: 'object',
    properties: {
      maxIterations: { type: 'integer', minimum: 1 },
      tolerance: { type: 'number', minimum: 0 },
      boundaryConditions: { type: 'array', items: { type: 'object' } },
      pollIntervalMs: { type: 'integer', minimum: 1 },
    },
  },
  async run(params, ctx) {
    const config = buildConfig(ctx, params);
    // Assigned synchronously below before any async callback can fire.
    let jobId = '';

    const handle = await runRealSolver(
      ctx.rpc,
      config,
      {
        onResidual: (p) => {
          ctx.emit({
            type: 'solver-residual',
            jobId,
            iteration: p.iteration,
            residual: p.residual,
            elapsedMs: p.elapsedMs,
          });
          ctx.update(solverPatch({ iteration: p.iteration, residual: p.residual }));
        },
        onDone: (r) => {
          const status: SolverStatus['status'] = r.status === 'converged' ? 'converged' : 'finished';
          const results: ResultsSummary = {
            availableFields: r.fields.map((f) => f.name),
            activeField: r.fields[0]?.name ?? null,
            fieldStats: Object.fromEntries(r.fields.map((f) => [f.name, { min: f.min, max: f.max, mean: f.mean }])),
          };
          ctx.update([
            ...solverPatch({ status, iteration: r.iteration, residual: r.residual }),
            { op: 'replace', path: ['results'], value: results as unknown as JsonValue },
          ]);
          ctx.emit({ type: 'solver-done', jobId, status: r.status, iteration: r.iteration });
          sessions.delete(jobId);
        },
        onError: (e) => {
          ctx.update(solverPatch({ status: 'error' }));
          ctx.emit({ type: 'solver-error', jobId: jobId || null, message: e.message });
          sessions.delete(jobId);
        },
      },
      { pollIntervalMs: params.pollIntervalMs }
    );

    jobId = handle.jobId;
    sessions.set(handle.jobId, handle);

    return {
      ok: true,
      result: { jobId: handle.jobId },
      statePatch: solverPatch({
        jobId: handle.jobId,
        status: 'running',
        iteration: 0,
        residual: null,
        maxIterations: config.maxIterations,
      }),
    };
  },
};

export interface CalcStopParams {
  jobId?: string;
}

export const calcStop: CommandDef<CalcStopParams, { stopped: boolean }> = {
  id: 'calc.stop',
  category: 'calc',
  group: 'Run',
  title: 'Stop Solver',
  titleKo: '솔버 중지',
  description: 'Halt the running solve. Targets the given job id, or the current one if omitted.',
  capability: 'run-solver',
  undoable: false,
  paramsSchema: {
    type: 'object',
    properties: { jobId: { type: 'string' } },
  },
  async run(params, ctx) {
    const jobId = params.jobId ?? ctx.getState().solver.jobId;
    if (!jobId) {
      return { ok: false, error: { code: 'NO_JOB', message: 'No running solver to stop' } };
    }
    const handle = sessions.get(jobId);
    if (handle) {
      await handle.stop();
      sessions.delete(jobId);
    }
    return { ok: true, result: { stopped: true }, statePatch: solverPatch({ status: 'finished' }) };
  },
};

/** Exposed for tests/inspection. */
export function activeSolverSessions(): ReadonlyMap<string, SolverHandle> {
  return sessions;
}

export function registerCalcCommands(registry: CommandRegistry): void {
  registry.register(calcRun);
  registry.register(calcStop);
}
