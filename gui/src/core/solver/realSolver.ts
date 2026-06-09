/**
 * Phase 1 — real backend solver runner.
 *
 * Drives the ACTUAL Rust solver via the gfd-server JSON-RPC contract instead of
 * the GUI's Math.exp simulation:
 *
 *   solve.start  { flow, turbulence, max_iterations, tolerance, density,
 *                  viscosity, conductivity, alpha_u, alpha_p, physics,
 *                  boundary_conditions }  -> { job_id }
 *   solve.status { job_id } -> { running, iteration, residual, elapsed_ms, status? }
 *   solve.stop   { job_id } -> { stopped }
 *   field.get    { field }  -> { values, min, max, mean }
 *
 * NOTE: the backend reports a SINGLE combined `residual` scalar (not per-equation
 * components), and the fluid solver exposes the fields pressure / vx / vy / vz /
 * velocity_magnitude; the thermal solver exposes temperature. This runner is
 * framework-agnostic (no React/Three) and fully mockable via RpcClient.
 */

import type { JsonObject, JsonValue } from '../types';
import type { RpcClient } from '../transport/rpcClient';

export interface SolverConfig {
  flow: string;
  turbulence: string;
  /** "fluid" | "thermal"; if omitted the backend infers from `flow`. */
  physics?: 'fluid' | 'thermal';
  maxIterations: number;
  tolerance: number;
  density: number;
  viscosity: number;
  conductivity: number;
  alphaU: number;
  alphaP: number;
  boundaryConditions: JsonValue[];
}

export interface SolverResidualPoint {
  iteration: number;
  residual: number;
  elapsedMs: number;
}

export interface SolverFieldResult {
  name: string;
  values: number[];
  min: number;
  max: number;
  mean: number;
}

export interface SolverResult {
  status: string;
  iteration: number;
  residual: number;
  /** Per-equation final update residuals (vx/vy/vz/pressure/continuity), if reported. */
  residualsByEq?: Record<string, number | null>;
  fields: SolverFieldResult[];
}

export interface SolverCallbacks {
  onResidual?(point: SolverResidualPoint): void;
  onLog?(message: string): void;
  onDone?(result: SolverResult): void;
  onError?(error: Error): void;
}

export interface SolverHandle {
  jobId: string;
  stop(): Promise<void>;
}

export interface SolverRunOptions {
  /** Polling cadence for solve.status. Default 250ms. */
  pollIntervalMs?: number;
}

interface StatusResponse {
  running: boolean;
  iteration: number;
  residual: number;
  elapsed_ms: number;
  status?: string;
  residuals?: Record<string, number | null>;
}

function toStartParams(config: SolverConfig): JsonObject {
  const params: JsonObject = {
    flow: config.flow,
    turbulence: config.turbulence,
    max_iterations: config.maxIterations,
    tolerance: config.tolerance,
    density: config.density,
    viscosity: config.viscosity,
    conductivity: config.conductivity,
    alpha_u: config.alphaU,
    alpha_p: config.alphaP,
    boundary_conditions: config.boundaryConditions,
  };
  if (config.physics) params.physics = config.physics;
  return params;
}

function candidateFields(config: SolverConfig): string[] {
  const isThermal = config.physics === 'thermal' || config.flow === 'none';
  return isThermal
    ? ['temperature']
    : ['pressure', 'velocity_magnitude', 'vx', 'vy', 'vz', ...(config.physics === 'fluid' ? [] : ['temperature'])];
}

async function fetchFields(rpc: RpcClient, config: SolverConfig): Promise<SolverFieldResult[]> {
  const results: SolverFieldResult[] = [];
  for (const name of candidateFields(config)) {
    try {
      const r = await rpc.request<{ values: number[]; min: number; max: number; mean: number }>('field.get', {
        field: name,
      });
      if (r && Array.isArray(r.values) && r.values.length > 0) {
        results.push({ name, values: r.values, min: r.min, max: r.max, mean: r.mean });
      }
    } catch {
      // Field not produced by this solve — skip silently.
    }
  }
  return results;
}

function delay(ms: number, cancelled: () => boolean): Promise<void> {
  return new Promise((resolve) => {
    const step = Math.min(ms, 50);
    const tick = () => {
      if (cancelled()) return resolve();
      setTimeout(() => (cancelled() ? resolve() : (ms -= step) > 0 ? tick() : resolve()), step);
    };
    tick();
  });
}

function toError(e: unknown): Error {
  return e instanceof Error ? e : new Error(String(e));
}

/**
 * Start a real solve and stream residuals until the backend reports
 * `running: false`, then fetch result fields. Returns a handle whose `stop()`
 * halts polling and asks the backend to stop the job.
 */
export async function runRealSolver(
  rpc: RpcClient,
  config: SolverConfig,
  callbacks: SolverCallbacks = {},
  options: SolverRunOptions = {}
): Promise<SolverHandle> {
  const startRes = await rpc.request<{ job_id?: string }>('solve.start', toStartParams(config));
  const jobId = startRes?.job_id;
  if (!jobId) {
    throw new Error('solve.start did not return a job_id');
  }

  let stopped = false;
  const interval = options.pollIntervalMs ?? 250;

  const poll = async (): Promise<void> => {
    while (!stopped) {
      let status: StatusResponse;
      try {
        status = await rpc.request<StatusResponse>('solve.status', { job_id: jobId });
      } catch (e) {
        callbacks.onError?.(toError(e));
        return;
      }

      if (typeof status.residual === 'number' && Number.isFinite(status.residual)) {
        callbacks.onResidual?.({
          iteration: status.iteration,
          residual: status.residual,
          elapsedMs: status.elapsed_ms,
        });
      }

      if (!status.running) {
        const fields = await fetchFields(rpc, config);
        callbacks.onDone?.({
          status: status.status ?? 'finished',
          iteration: status.iteration,
          residual: status.residual,
          ...(status.residuals ? { residualsByEq: status.residuals } : {}),
          fields,
        });
        return;
      }

      await delay(interval, () => stopped);
    }
  };

  // Fire-and-forget; consumers observe progress through callbacks.
  void poll().catch((e) => callbacks.onError?.(toError(e)));

  return {
    jobId,
    async stop(): Promise<void> {
      stopped = true;
      try {
        await rpc.request('solve.stop', { job_id: jobId });
      } catch {
        // Best-effort stop.
      }
    },
  };
}
