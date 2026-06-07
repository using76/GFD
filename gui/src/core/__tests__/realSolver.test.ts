import { describe, it, expect } from 'vitest';
import { runRealSolver, type SolverConfig, type SolverResidualPoint, type SolverResult } from '../solver/realSolver';
import { createMockRpcClient } from '../transport/rpcClient';
import type { JsonObject } from '../types';

const config: SolverConfig = {
  flow: 'incompressible',
  turbulence: 'none',
  physics: 'fluid',
  maxIterations: 100,
  tolerance: 1e-4,
  density: 1.0,
  viscosity: 0.01,
  conductivity: 1.0,
  alphaU: 0.5,
  alphaP: 0.3,
  boundaryConditions: [],
};

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

describe('runRealSolver', () => {
  it('streams residuals, stops when running:false, and fetches fields', async () => {
    let statusCalls = 0;
    const rpc = createMockRpcClient((method: string, params: JsonObject) => {
      switch (method) {
        case 'solve.start':
          return { job_id: 'job_1' };
        case 'solve.status': {
          statusCalls += 1;
          const running = statusCalls < 3;
          return {
            running,
            iteration: statusCalls,
            residual: 1e-1 / statusCalls,
            elapsed_ms: statusCalls * 10,
            ...(running ? {} : { status: 'converged' }),
          };
        }
        case 'field.get': {
          if (params.field === 'pressure') {
            return { values: [1, 2, 3], min: 1, max: 3, mean: 2 };
          }
          throw new Error(`Field '${String(params.field)}' not found`);
        }
        default:
          return {};
      }
    });

    const residuals: SolverResidualPoint[] = [];
    let done: SolverResult | null = null;

    await runRealSolver(
      rpc,
      config,
      {
        onResidual: (p) => residuals.push(p),
        onDone: (r) => (done = r),
      },
      { pollIntervalMs: 1 }
    );

    await waitFor(() => done !== null);

    const result = done as SolverResult | null;
    expect(result).not.toBeNull();
    expect(residuals.length).toBeGreaterThanOrEqual(3);
    expect(result!.status).toBe('converged');
    expect(result!.fields.map((f) => f.name)).toContain('pressure');
    expect(result!.fields).toHaveLength(1); // only pressure resolved; others "not found"
  });

  it('throws if solve.start returns no job_id', async () => {
    const rpc = createMockRpcClient(() => ({}));
    await expect(runRealSolver(rpc, config, {}, { pollIntervalMs: 1 })).rejects.toThrow(/job_id/);
  });

  it('stop() halts polling and calls solve.stop', async () => {
    let stopCalled = false;
    const rpc = createMockRpcClient((method: string) => {
      if (method === 'solve.start') return { job_id: 'job_2' };
      if (method === 'solve.status') return { running: true, iteration: 1, residual: 0.5, elapsed_ms: 1 };
      if (method === 'solve.stop') {
        stopCalled = true;
        return { stopped: true };
      }
      return {};
    });

    const handle = await runRealSolver(rpc, config, {}, { pollIntervalMs: 1 });
    await handle.stop();
    expect(stopCalled).toBe(true);
  });
});
