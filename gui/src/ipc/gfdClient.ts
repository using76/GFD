/**
 * GFD IPC Client
 *
 * Communicates with the Rust backend via Electron IPC (when running in Electron)
 * or provides realistic simulated responses for browser-only development.
 */

type EventCallback = (data: unknown) => void;

const eventListeners: Set<EventCallback> = new Set();

// Solver simulation state
let simIteration = 0;
let simRunning = false;

/**
 * Send a JSON-RPC request to the GFD Rust backend.
 */
export async function sendRequest(
  method: string,
  params?: Record<string, unknown>
): Promise<unknown> {
  if (window.gfdAPI) {
    return window.gfdAPI.sendRequest(method, params);
  }

  // Browser simulation mode
  return simulateResponse(method, params ?? {});
}

/**
 * Simulate realistic responses for browser-only development.
 */
function simulateResponse(
  method: string,
  params: Record<string, unknown>
): unknown {
  switch (method) {
    case 'system.version':
      return { name: 'GFD', version: '0.1.0', mode: 'browser-simulation' };

    case 'mesh.generate': {
      const nx = (params.nx as number) || 20;
      const ny = (params.ny as number) || 20;
      return {
        cells: nx * ny,
        nodes: (nx + 1) * (ny + 1),
        faces: 2 * nx * ny + nx + ny,
        quality: {
          min_ortho: 0.92 + Math.random() * 0.05,
          max_skew: 0.08 + Math.random() * 0.05,
          max_aspect: 1.0,
        },
      };
    }

    case 'mesh.get_display_data': {
      const nx = (params.nx as number) || 20;
      const ny = (params.ny as number) || 20;
      return generateMeshDisplayData(nx, ny);
    }

    case 'mesh.quality':
      return {
        minOrthogonality: 0.92 + Math.random() * 0.05,
        maxSkewness: 0.08 + Math.random() * 0.05,
        maxAspectRatio: 1.0 + Math.random() * 0.5,
        histogram: Array.from({ length: 10 }, (_, i) =>
          i >= 8 ? 0.3 + Math.random() * 0.2 : 0.02 + Math.random() * 0.05
        ),
      };

    case 'solve.start': {
      simIteration = 0;
      simRunning = true;
      return { job_id: `sim-${Date.now()}` };
    }

    case 'solve.status': {
      if (!simRunning) {
        return {
          running: false,
          iteration: simIteration,
          converged: simIteration > 0,
          residuals: null,
        };
      }
      simIteration++;
      const decay = Math.exp(-simIteration * 0.005);
      const residual = {
        continuity: 1e-1 * decay * (0.8 + 0.4 * Math.random()),
        xMomentum: 5e-2 * decay * (0.8 + 0.4 * Math.random()),
        yMomentum: 5e-2 * decay * (0.8 + 0.4 * Math.random()),
        energy: 1e-2 * decay * (0.8 + 0.4 * Math.random()),
      };
      const maxIter = (params.maxIterations as number) || 1000;
      if (simIteration >= maxIter) {
        simRunning = false;
      }
      return {
        running: simRunning,
        iteration: simIteration,
        converged: !simRunning && simIteration >= maxIter,
        residuals: residual,
        log: `[Iter ${simIteration}] continuity=${residual.continuity.toExponential(3)}`,
      };
    }

    case 'solve.stop': {
      simRunning = false;
      return { stopped: true };
    }

    case 'field.get': {
      const field = (params.field as string) || 'pressure';
      return simulateFieldData(field, (params.nx as number) || 20, (params.ny as number) || 20);
    }

    default:
      console.warn(`[gfdClient] Unknown method: ${method}`, params);
      return { stub: true, method, message: 'Running in browser mode (no backend)' };
  }
}

/**
 * Generate a simple grid mesh display data that Three.js can render.
 */
function generateMeshDisplayData(nx: number, ny: number) {
  const nodeCount = (nx + 1) * (ny + 1);
  const positions: number[] = [];
  const dx = 1.0 / nx;
  const dy = 1.0 / ny;

  for (let j = 0; j <= ny; j++) {
    for (let i = 0; i <= nx; i++) {
      positions.push(i * dx, 0, j * dy);
    }
  }

  const indices: number[] = [];
  for (let j = 0; j < ny; j++) {
    for (let i = 0; i < nx; i++) {
      const n0 = j * (nx + 1) + i;
      const n1 = n0 + 1;
      const n2 = n0 + (nx + 1);
      const n3 = n2 + 1;
      // Two triangles per quad
      indices.push(n0, n1, n2);
      indices.push(n1, n3, n2);
    }
  }

  return {
    positions,
    indices,
    nodeCount,
    cellCount: nx * ny,
  };
}

/**
 * Generate a scalar field with a gradient pattern.
 */
function simulateFieldData(fieldName: string, nx: number, ny: number) {
  const values: number[] = [];
  let min = Infinity;
  let max = -Infinity;

  for (let j = 0; j <= ny; j++) {
    for (let i = 0; i <= nx; i++) {
      const x = i / nx;
      const z = j / ny;
      let v: number;

      switch (fieldName) {
        case 'pressure':
          v = 100 * (1 - x) + 20 * Math.sin(Math.PI * z) + 5 * Math.random();
          break;
        case 'velocity': {
          const vx = Math.sin(Math.PI * x) * Math.cos(Math.PI * z);
          const vz = -Math.cos(Math.PI * x) * Math.sin(Math.PI * z);
          v = Math.sqrt(vx * vx + vz * vz);
          break;
        }
        case 'temperature':
          v = 400 - 100 * x + 15 * Math.sin(2 * Math.PI * z) + 3 * Math.random();
          break;
        default:
          v = x + z;
      }

      values.push(v);
      if (v < min) min = v;
      if (v > max) max = v;
    }
  }

  return { name: fieldName, values, min, max };
}

/**
 * Subscribe to events from the GFD backend (solver progress, convergence, etc.).
 * Returns an unsubscribe function.
 */
export function onEvent(callback: EventCallback): () => void {
  eventListeners.add(callback);

  if (window.gfdAPI) {
    const unsub = window.gfdAPI.onEvent((data) => {
      callback(data);
    });
    return () => {
      eventListeners.delete(callback);
      unsub();
    };
  }

  // Browser-only: no events to listen to
  return () => {
    eventListeners.delete(callback);
  };
}

/**
 * Check if the GFD backend is running.
 */
export async function getBackendStatus(): Promise<{ running: boolean }> {
  if (window.gfdAPI) {
    return window.gfdAPI.getStatus();
  }
  return { running: false };
}

/**
 * Convenience: load a simulation JSON file.
 */
export async function loadSimulation(filePath: string): Promise<unknown> {
  return sendRequest('load', { path: filePath });
}

/**
 * Convenience: start the solver.
 */
export async function startSolver(): Promise<unknown> {
  return sendRequest('solve.start', {});
}

/**
 * Convenience: stop the solver.
 */
export async function stopSolver(): Promise<unknown> {
  return sendRequest('solve.stop', {});
}

/**
 * Convenience: generate mesh.
 */
export async function generateMesh(params: Record<string, unknown>): Promise<unknown> {
  return sendRequest('mesh.generate', params);
}
