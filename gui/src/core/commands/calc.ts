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

import type { JsonValue, JsonObject } from '../types';
import type { CommandDef, CommandContext } from '../command';
import type { AppState } from '../state';
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
  const setup = s.setup;
  const isThermal = physics.models.flow === 'none';
  return {
    flow: physics.models.flow,
    turbulence: physics.models.turbulence,
    physics: isThermal ? 'thermal' : 'fluid',
    maxIterations: params.maxIterations ?? setup.solver.maxIterations,
    tolerance: params.tolerance ?? setup.solver.tolerance,
    density: physics.material.density,
    viscosity: physics.material.viscosity,
    conductivity: physics.material.conductivity,
    alphaU: setup.solver.relaxVelocity,
    alphaP: setup.solver.relaxPressure,
    boundaryConditions: params.boundaryConditions ?? (setup.boundaries as unknown as JsonValue[]),
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

export interface CalcSensitivityParams {
  parameter?: 'viscosity' | 'density';
  objective?: 'kinetic_energy' | 'max_velocity' | 'mean_pressure';
  delta?: number;
  maxIterations?: number;
  boundaryConditions?: JsonValue[];
}

export interface SensitivityResult {
  parameter: string;
  objective: string;
  delta: number;
  objective_base: number;
  objective_perturbed: number;
  gradient: number;
  method: string;
}

export const calcSensitivity: CommandDef<CalcSensitivityParams, SensitivityResult> = {
  id: 'calc.sensitivity',
  category: 'calc',
  group: 'Optimize',
  title: 'Sensitivity (∂obj/∂param)',
  titleKo: '민감도 (∂목적/∂파라미터)',
  description:
    'Finite-difference gradient of an objective (kinetic_energy/max_velocity/mean_pressure) w.r.t. a parameter (viscosity/density). Enables AI gradient-based optimization. Requires a mesh.',
  capability: 'run-solver',
  undoable: false,
  paramsSchema: {
    type: 'object',
    properties: {
      parameter: { type: 'string', enum: ['viscosity', 'density'] },
      objective: { type: 'string', enum: ['kinetic_energy', 'max_velocity', 'mean_pressure'] },
      delta: { type: 'number', minimum: 0 },
      maxIterations: { type: 'integer', minimum: 1 },
      boundaryConditions: { type: 'array', items: { type: 'object' } },
    },
  },
  async run(params, ctx) {
    const s = ctx.getState();
    const r = await ctx.rpc.request<SensitivityResult>('solve.sensitivity', {
      parameter: params.parameter ?? 'viscosity',
      objective: params.objective ?? 'kinetic_energy',
      viscosity: s.physics.material.viscosity,
      density: s.physics.material.density,
      max_iterations: params.maxIterations ?? 150,
      ...(params.delta !== undefined ? { delta: params.delta } : {}),
      boundary_conditions: params.boundaryConditions ?? (s.setup.boundaries as unknown as JsonValue[]),
    });
    return { ok: true, result: r };
  },
};

export interface CalcLbmParams {
  nx?: number;
  ny?: number;
  steps?: number;
  viscosity?: number;
  lidVelocity?: number;
}

export interface LbmResult {
  solver: string;
  nx: number;
  ny: number;
  steps: number;
  u_max: number;
  u_mean: number;
  reynolds: number;
}

export const calcLbm: CommandDef<CalcLbmParams, LbmResult> = {
  id: 'calc.lbm',
  category: 'calc',
  group: 'Run',
  title: 'Run LBM (D2Q9)',
  titleKo: 'LBM 실행 (D2Q9)',
  description:
    'Run a Lattice Boltzmann D2Q9 lid-driven cavity (mesoscopic solver, independent of the FVM mesh). Returns u_max/u_mean/Re and stores velocity fields.',
  capability: 'run-solver',
  undoable: false,
  paramsSchema: {
    type: 'object',
    properties: {
      nx: { type: 'integer', minimum: 4, maximum: 512 },
      ny: { type: 'integer', minimum: 4, maximum: 512 },
      steps: { type: 'integer', minimum: 1 },
      viscosity: { type: 'number', minimum: 0 },
      lidVelocity: { type: 'number' },
    },
  },
  async run(params, ctx) {
    const r = await ctx.rpc.request<LbmResult>('solve.lbm', {
      nx: params.nx ?? 64,
      ny: params.ny ?? 64,
      steps: params.steps ?? 2000,
      viscosity: params.viscosity ?? 0.05,
      lid_velocity: params.lidVelocity ?? 0.1,
    });
    const results: ResultsSummary = {
      availableFields: ['velocity_magnitude', 'vx', 'vy'],
      activeField: 'velocity_magnitude',
      fieldStats: { velocity_magnitude: { min: 0, max: r.u_max, mean: r.u_mean } },
    };
    return {
      ok: true,
      result: r,
      statePatch: [
        { op: 'replace', path: ['solver', 'status'], value: 'finished' },
        { op: 'replace', path: ['results'], value: results as unknown as JsonValue },
      ],
    };
  },
};

// ────────────────────────────────────────────────────────────────────────────
// Diagnose — the "analyze results → identify problems → suggest a fix" brain of
// the AI simulation loop. Pure analysis over AppState (no backend RPC): reads
// the solver status, field stats, material and setup, then emits structured
// issues. Each issue carries an actionable { command, params } `fix` so an AI
// agent (or an auto-refine loop) can apply the correction and re-run — closing
// the loop.
// ────────────────────────────────────────────────────────────────────────────

export type Severity = 'info' | 'warning' | 'error';

export interface DiagnoseIssue {
  code: string;
  severity: Severity;
  message: string;
  suggestion: string;
  /** A command + params an agent can dispatch to act on this issue. */
  fix?: { command: string; params: JsonObject };
}

export type FlowRegime = 'laminar' | 'transitional' | 'turbulent' | 'unknown';

export interface DiagnoseResult {
  status: string;
  converged: boolean;
  iteration: number;
  residual: number | null;
  tolerance: number;
  reynolds: number | null;
  characteristicLength: number;
  characteristicVelocity: number;
  flowRegime: FlowRegime;
  fieldStats: Record<string, { min: number; max: number; mean: number }>;
  issues: DiagnoseIssue[];
  summary: string;
}

/** Not-finite or absurdly large → a blow-up signature. */
function isBad(x: number): boolean {
  return !Number.isFinite(x) || Math.abs(x) > 1e8;
}

/** Largest overall bbox extent across all geometry nodes (estimate of L). */
function characteristicLength(state: AppState): number {
  const nodes = Object.values(state.doc.geometry.nodes);
  if (nodes.length === 0) return 1.0;
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  for (const n of nodes) {
    for (let k = 0; k < 3; k++) {
      if (n.bbox.min[k] < min[k]) min[k] = n.bbox.min[k];
      if (n.bbox.max[k] > max[k]) max[k] = n.bbox.max[k];
    }
  }
  const ext = [max[0] - min[0], max[1] - min[1], max[2] - min[2]].filter((e) => Number.isFinite(e) && e > 0);
  return ext.length ? Math.max(...ext) : 1.0;
}

/** Max prescribed velocity magnitude across boundary conditions. */
function boundaryVelocity(state: AppState): number {
  let u = 0;
  for (const bc of state.setup.boundaries) {
    const p = bc.parameters ?? {};
    const mag = Math.hypot(p.vx ?? 0, p.vy ?? 0, p.vz ?? 0);
    if (mag > u) u = mag;
    if (typeof p.velocity === 'number' && p.velocity > u) u = p.velocity;
  }
  return u;
}

function regimeOf(re: number | null): FlowRegime {
  if (re === null) return 'unknown';
  if (re < 2300) return 'laminar';
  if (re < 4000) return 'transitional';
  return 'turbulent';
}

export function diagnoseState(state: AppState): DiagnoseResult {
  const solver = state.solver;
  const results = state.results;
  const ss = state.setup.solver;
  const fieldStats = results?.fieldStats ?? {};
  const issues: DiagnoseIssue[] = [];

  const L = characteristicLength(state);
  const vm = fieldStats['velocity_magnitude'];
  const resultVel = vm && Number.isFinite(vm.max) && vm.max > 0 ? vm.max : 0;
  const bcVel = boundaryVelocity(state);
  const U = resultVel > 0 ? resultVel : bcVel;
  const { density: rho, viscosity: mu } = state.physics.material;
  const isFlow = state.physics.models.flow !== 'none';
  const reynolds = isFlow && U > 0 && mu > 0 ? (rho * U * L) / mu : null;
  const flowRegime = regimeOf(reynolds);

  const lowRelaxV = Math.max(0.05, Math.min(0.3, ss.relaxVelocity * 0.5));
  const lowRelaxP = Math.max(0.02, Math.min(0.1, ss.relaxPressure * 0.5));

  // (a) No results yet.
  const hasResults = !!results && results.availableFields.length > 0;
  if (!hasResults) {
    issues.push({
      code: 'NO_RESULTS',
      severity: 'info',
      message: '아직 솔브 결과가 없습니다.',
      suggestion: 'calc.run으로 솔버를 먼저 실행하세요.',
      fix: { command: 'calc.run', params: {} },
    });
  }

  // (b) Divergence / blow-up: any field stat NaN/Inf/huge.
  const badFields = Object.entries(fieldStats)
    .filter(([, s]) => isBad(s.min) || isBad(s.max) || isBad(s.mean))
    .map(([n]) => n);
  if (badFields.length > 0) {
    issues.push({
      code: 'DIVERGENCE',
      severity: 'error',
      message: `발산(blow-up) 감지: [${badFields.join(', ')}] 필드에 비정상 값(NaN/Inf/과대).`,
      suggestion: `완화계수를 낮추고(relaxVelocity≈${lowRelaxV}, relaxPressure≈${lowRelaxP}) 메쉬를 점검한 뒤 재실행하세요.`,
      fix: { command: 'setup.set_solver', params: { relaxVelocity: lowRelaxV, relaxPressure: lowRelaxP } },
    });
  }

  // (c) Hit the iteration cap without converging.
  const hitCap = ss.maxIterations > 0 && solver.iteration >= Math.floor(ss.maxIterations * 0.98);
  if (solver.status !== 'converged' && hitCap && badFields.length === 0 && hasResults) {
    issues.push({
      code: 'MAX_ITERS',
      severity: 'warning',
      message: `최대 반복 ${ss.maxIterations}회에 도달했지만 수렴하지 않았습니다 (status=${solver.status}).`,
      suggestion: '반복 횟수를 늘리거나(2배) 완화계수를 조정한 뒤 재실행하세요.',
      fix: { command: 'setup.set_solver', params: { maxIterations: ss.maxIterations * 2 } },
    });
  }

  // (d) Residual stalled well above tolerance.
  if (
    solver.residual !== null &&
    Number.isFinite(solver.residual) &&
    solver.residual > ss.tolerance * 10 &&
    badFields.length === 0 &&
    !hitCap
  ) {
    issues.push({
      code: 'HIGH_RESIDUAL',
      severity: 'warning',
      message: `잔차 ${solver.residual.toExponential(2)}가 허용오차 ${ss.tolerance.toExponential(1)}의 10배를 초과합니다.`,
      suggestion: '추가 반복 또는 완화계수 하향이 필요합니다.',
      fix: { command: 'setup.set_solver', params: { relaxVelocity: lowRelaxV, relaxPressure: lowRelaxP } },
    });
  }

  // (e) Turbulent/transitional regime but a laminar model is selected.
  if (reynolds !== null && reynolds > 2300 && isFlow && state.physics.models.turbulence === 'none') {
    issues.push({
      code: 'TURBULENCE_MODEL',
      severity: 'warning',
      message: `Re≈${Math.round(reynolds)}로 천이/난류 영역이지만 난류 모델이 'none'(층류)입니다.`,
      suggestion: 'k-ε 또는 k-ω SST 난류 모델을 활성화하세요.',
      fix: { command: 'setup.set_model', params: { key: 'turbulence', value: 'k_epsilon' } },
    });
  }

  // (f) Low-Re flow carrying a turbulence model unnecessarily.
  if (reynolds !== null && reynolds < 2300 && state.physics.models.turbulence !== 'none') {
    issues.push({
      code: 'LAMINAR_REGIME',
      severity: 'info',
      message: `Re≈${Math.round(reynolds)}는 층류 영역입니다. 난류 모델이 불필요할 수 있습니다.`,
      suggestion: '계산 비용 절감을 위해 난류 모델 비활성화를 고려하세요.',
      fix: { command: 'setup.set_model', params: { key: 'turbulence', value: 'none' } },
    });
  }

  // (g) Flow never developed despite a driving boundary velocity.
  if (vm && Number.isFinite(vm.max) && vm.max < 1e-9 && bcVel > 1e-6) {
    issues.push({
      code: 'NO_FLOW',
      severity: 'warning',
      message: `경계조건은 속도 ${bcVel.toPrecision(3)}를 지시하지만 결과 속도장이 거의 0입니다 — 유동 미발달.`,
      suggestion: '경계조건(입구/벽) 설정과 메쉬 연결성을 점검하세요.',
    });
  }

  // (h) Clean bill of health.
  const hasProblem = issues.some((i) => i.severity !== 'info');
  if (!hasProblem && solver.status === 'converged') {
    issues.push({
      code: 'OK',
      severity: 'info',
      message: '솔브가 정상 수렴했고 명백한 문제가 없습니다.',
      suggestion: '결과를 시각화(results.contour)하거나 민감도(calc.sensitivity)로 최적화를 진행하세요.',
    });
  }

  const worst: Severity = issues.some((i) => i.severity === 'error')
    ? 'error'
    : issues.some((i) => i.severity === 'warning')
      ? 'warning'
      : 'info';
  const head = worst === 'error' ? '심각한 문제' : worst === 'warning' ? '주의 필요' : '정상';
  const summary = `[${head}] status=${solver.status}, iter=${solver.iteration}/${ss.maxIterations}, residual=${
    solver.residual === null ? 'n/a' : solver.residual.toExponential(2)
  }, Re=${reynolds === null ? 'n/a' : Math.round(reynolds)} (${flowRegime}), issues=${issues.length}`;

  return {
    status: solver.status,
    converged: solver.status === 'converged',
    iteration: solver.iteration,
    residual: solver.residual,
    tolerance: ss.tolerance,
    reynolds,
    characteristicLength: L,
    characteristicVelocity: U,
    flowRegime,
    fieldStats,
    issues,
    summary,
  };
}

export const calcDiagnose: CommandDef<Record<string, never>, DiagnoseResult> = {
  id: 'calc.diagnose',
  category: 'calc',
  group: 'Analyze',
  title: 'Diagnose Results',
  titleKo: '결과 진단',
  description:
    'Analyze the current solve (convergence, field blow-up, Reynolds regime vs turbulence model, undeveloped flow) and return structured issues, each with an actionable fix command. The brain of the AI analyze→fix→re-run loop.',
  capability: 'read',
  undoable: false,
  paramsSchema: { type: 'object', properties: {} },
  async run(_params, ctx) {
    return { ok: true, result: diagnoseState(ctx.getState()) };
  },
};

export function registerCalcCommands(registry: CommandRegistry): void {
  registry.register(calcRun);
  registry.register(calcStop);
  registry.register(calcSensitivity);
  registry.register(calcLbm);
  registry.register(calcDiagnose);
}
