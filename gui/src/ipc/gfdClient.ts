/**
 * GFD IPC Client
 *
 * Communicates with the Rust backend via Electron IPC (when running in Electron)
 * or provides stub responses for browser-only development.
 */

type EventCallback = (data: unknown) => void;

const eventListeners: Set<EventCallback> = new Set();

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

  // Browser-only stub: simulate responses for development
  console.warn(`[gfdClient] No Electron IPC available. Stubbing: ${method}`, params);
  return { stub: true, method, message: 'Running in browser mode (no backend)' };
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
  return sendRequest('solve', {});
}

/**
 * Convenience: stop the solver.
 */
export async function stopSolver(): Promise<unknown> {
  return sendRequest('stop', {});
}

/**
 * Convenience: generate mesh.
 */
export async function generateMesh(params: Record<string, unknown>): Promise<unknown> {
  return sendRequest('mesh.generate', params);
}
