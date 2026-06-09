/**
 * System commands — read-only backend introspection.
 */

import type { CommandDef } from '../command';
import type { CommandRegistry } from '../registry';

interface VersionResult {
  name?: string;
  version?: string;
  [k: string]: unknown;
}

export const systemVersion: CommandDef<Record<string, never>, VersionResult> = {
  id: 'system.version',
  category: 'system',
  title: 'Backend Version',
  titleKo: '백엔드 버전',
  description: 'Return the gfd-server kernel name, version, and capabilities.',
  capability: 'read',
  paramsSchema: { type: 'object', properties: {}, additionalProperties: false },
  async run(_params, ctx) {
    const result = await ctx.rpc.request<VersionResult>('system.version');
    return { ok: true, result };
  },
};

interface LoopStep {
  done: boolean;
  detail: string;
}

export interface LoopStatusResult {
  steps: {
    geometry: LoopStep;
    mesh: LoopStep;
    setup: LoopStep;
    solved: LoopStep;
    analyzed: LoopStep;
  };
  readyToSolve: boolean;
  /** The single recommended next action, or null when the loop is complete. */
  nextAction: { step: string; command: string; hint: string } | null;
  summary: string;
}

export const loopStatus: CommandDef<Record<string, never>, LoopStatusResult> = {
  id: 'system.loop_status',
  category: 'system',
  title: 'Loop Status',
  titleKo: '루프 상태',
  description:
    'Report which simulation-loop steps are complete (geometry → mesh → setup → solve → analyze) and the single recommended next action, so an AI agent can drive the loop in order without skipping a step.',
  capability: 'read',
  paramsSchema: { type: 'object', properties: {} },
  async run(_params, ctx) {
    const s = ctx.getState();
    const nodeCount = Object.keys(s.doc.geometry.nodes).length;
    const gmshShapes = s.gmsh.shapeIds.length;
    const hasGeometry = nodeCount > 0 || gmshShapes > 0;
    const hasMesh = !!s.mesh?.generated || !!s.gmsh.meshStats;
    const boundaryCount = s.setup.boundaries.length;
    const hasSetup = boundaryCount > 0;
    const hasResults = !!s.results && s.results.availableFields.length > 0;
    const solved = hasResults && (s.solver.status === 'converged' || s.solver.status === 'finished');
    const diag = s.diagnosis as null | { issues?: Array<{ severity: string }> };
    const analyzed = !!diag;
    const openProblems = diag ? (diag.issues ?? []).filter((i) => i.severity === 'error' || i.severity === 'warning').length : 0;

    const steps: LoopStatusResult['steps'] = {
      geometry: { done: hasGeometry, detail: hasGeometry ? `${nodeCount} shape(s)${gmshShapes ? `, ${gmshShapes} OCC` : ''}` : 'no shapes' },
      mesh: { done: hasMesh, detail: hasMesh ? (s.mesh ? `${s.mesh.cellCount} cells` : 'gmsh mesh') : 'not generated' },
      setup: { done: hasSetup, detail: hasSetup ? `${boundaryCount} boundary condition(s)` : 'no boundary conditions' },
      solved: { done: solved, detail: solved ? `status=${s.solver.status}, iter=${s.solver.iteration}` : 'not solved' },
      analyzed: { done: analyzed, detail: analyzed ? `${openProblems} open problem(s)` : 'not analyzed' },
    };

    let nextAction: LoopStatusResult['nextAction'] = null;
    if (!hasGeometry) {
      nextAction = { step: 'geometry', command: 'geometry.create_primitive', hint: 'Create a primitive or import a CAD file (io.import_mesh / io.import_step).' };
    } else if (!hasMesh) {
      nextAction = { step: 'mesh', command: 'mesh.generate', hint: 'Generate a mesh over the domain.' };
    } else if (!hasSetup) {
      nextAction = { step: 'setup', command: 'setup.add_boundary', hint: 'Add boundary conditions and set the material/model.' };
    } else if (!solved) {
      nextAction = { step: 'solve', command: 'calc.run', hint: 'Run the solver.' };
    } else if (!analyzed) {
      nextAction = { step: 'analyze', command: 'calc.diagnose', hint: 'Diagnose the result.' };
    } else if (openProblems > 0) {
      nextAction = { step: 'fix', command: 'auto_refine', hint: 'Apply the suggested fixes, or call auto_refine to self-correct.' };
    }

    const readyToSolve = hasGeometry && hasMesh && hasSetup;
    const summary = nextAction ? `Next: ${nextAction.step} — ${nextAction.command}` : 'Loop complete: solved and healthy.';
    return { ok: true, result: { steps, readyToSolve, nextAction, summary } };
  },
};

export function registerSystemCommands(registry: CommandRegistry): void {
  registry.register(systemVersion);
  registry.register(loopStatus);
}
