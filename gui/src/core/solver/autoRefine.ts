/**
 * Auto-refine — the autonomous closed loop the user asked for:
 *   diagnose results → identify the top problem → apply its fix → re-run →
 *   repeat until healthy (or maxRounds).
 *
 * Commands cannot dispatch other commands (CommandContext has no `dispatch`), so
 * this orchestrator lives one level up with access to `core.dispatcher`. It is
 * framework-agnostic (no React/Three) and fully mockable via the RpcClient, and
 * is exposed to AI agents through the `auto_refine` MCP meta-tool so the model
 * can close the loop in a single call.
 */

import type { Core } from '../index';
import type { JsonObject, JsonValue } from '../types';
import { diagnoseState, type DiagnoseResult, type DiagnoseIssue } from '../commands/calc';

export interface AutoRefineOptions {
  /** Max diagnose→fix→re-run rounds. Default 3. */
  maxRounds?: number;
  /** Override the solver iteration budget for each re-run. */
  maxIterations?: number;
  /** Boundary conditions passed through to calc.run. */
  boundaryConditions?: JsonValue[];
  /** Poll cadence (ms) while waiting for a re-run to finish. Default 25. */
  pollIntervalMs?: number;
  /** Hard cap (ms) on waiting for any single solve. Default 120000. */
  timeoutMs?: number;
  signal?: AbortSignal;
}

export interface AutoRefineRound {
  round: number;
  issueCode: string;
  severity: string;
  fixCommand: string | null;
  fixParams: JsonValue | null;
  beforeResidual: number | null;
  afterResidual: number | null;
  afterStatus: string;
}

export type StoppedReason = 'healthy' | 'max_rounds' | 'aborted' | 'error' | 'no_progress';

export interface AutoRefineResult {
  rounds: AutoRefineRound[];
  finalDiagnosis: DiagnoseResult;
  healthy: boolean;
  stoppedReason: StoppedReason;
}

export type AutoRefineEvent =
  | { type: 'round_start'; round: number; issue: DiagnoseIssue }
  | { type: 'fix_applied'; round: number; command: string }
  | { type: 'solve_done'; round: number; status: string; residual: number | null }
  | { type: 'done'; result: AutoRefineResult };

/** Did a round measurably help? (converged, or residual dropped >1%.) */
function roundImproved(r: AutoRefineRound): boolean {
  if (r.afterStatus === 'converged') return true;
  const a = r.afterResidual;
  const b = r.beforeResidual;
  return a !== null && b !== null && Number.isFinite(a) && Number.isFinite(b) && a < b * 0.99;
}

/** Actionable issues with a fix, ordered: errors → warnings → NO_RESULTS. */
function actionableIssues(issues: DiagnoseIssue[]): DiagnoseIssue[] {
  const withFix = issues.filter((i) => !!i.fix);
  return [
    ...withFix.filter((i) => i.severity === 'error'),
    ...withFix.filter((i) => i.severity === 'warning'),
    ...withFix.filter((i) => i.code === 'NO_RESULTS'),
  ];
}

/** Run calc.diagnose so the analysis is cached in state, and return it. */
async function dispatchDiagnose(core: Core): Promise<DiagnoseResult> {
  const o = await core.dispatcher.dispatch({ commandId: 'calc.diagnose', params: {}, source: 'agent' });
  return (o.ok && o.result ? o.result : diagnoseState(core.store.getState())) as DiagnoseResult;
}

/** Block until the async solver runner flips status away from 'running'. */
async function waitForSolve(core: Core, timeoutMs: number, pollMs: number, signal?: AbortSignal): Promise<void> {
  const start = Date.now();
  for (;;) {
    const status = core.store.getState().solver.status;
    if (status !== 'running') return;
    if (signal?.aborted) return;
    if (Date.now() - start > timeoutMs) return;
    await new Promise((r) => setTimeout(r, pollMs));
  }
}

export async function runAutoRefine(
  core: Core,
  opts: AutoRefineOptions = {},
  onEvent?: (e: AutoRefineEvent) => void
): Promise<AutoRefineResult> {
  const maxRounds = opts.maxRounds ?? 3;
  const pollMs = opts.pollIntervalMs ?? 25;
  const timeoutMs = opts.timeoutMs ?? 120_000;
  const rounds: AutoRefineRound[] = [];
  let stoppedReason: StoppedReason = 'max_rounds';
  let lastDiag: DiagnoseResult | null = null;
  // Issue codes whose fix was already tried and made no measurable progress —
  // give each distinct fix exactly one shot, then escalate to the next actionable
  // issue (e.g. relaxation didn't help → try re-meshing) before giving up.
  const ineffectiveCodes = new Set<string>();

  for (let round = 1; round <= maxRounds; round++) {
    if (opts.signal?.aborted) {
      stoppedReason = 'aborted';
      break;
    }

    // Diagnose through the command so the analysis is cached in AppState live.
    const diag = await dispatchDiagnose(core);
    lastDiag = diag;
    const candidates = actionableIssues(diag.issues);
    const issue = candidates.find((c) => !ineffectiveCodes.has(c.code)) ?? null;
    if (!issue) {
      // No fresh fix left: healthy if nothing was actionable, else stuck.
      stoppedReason = candidates.length ? 'no_progress' : 'healthy';
      break;
    }
    onEvent?.({ type: 'round_start', round, issue });

    // Apply the corrective fix (unless the fix is itself "just run the solver").
    if (issue.fix && issue.fix.command !== 'calc.run') {
      const fixOut = await core.dispatcher.dispatch({
        commandId: issue.fix.command,
        params: issue.fix.params,
        source: 'agent',
      });
      if (!fixOut.ok) {
        stoppedReason = 'error';
        break;
      }
      onEvent?.({ type: 'fix_applied', round, command: issue.fix.command });
    }

    const beforeResidual = core.store.getState().solver.residual;

    // Re-run the solver with the corrected setup.
    const runParams: JsonObject = {};
    if (opts.maxIterations !== undefined) runParams.maxIterations = opts.maxIterations;
    if (opts.boundaryConditions !== undefined) runParams.boundaryConditions = opts.boundaryConditions;
    const runOut = await core.dispatcher.dispatch({ commandId: 'calc.run', params: runParams, source: 'agent' });
    if (!runOut.ok) {
      stoppedReason = 'error';
      break;
    }
    await waitForSolve(core, timeoutMs, pollMs, opts.signal);

    const after = core.store.getState().solver;
    const thisRound: AutoRefineRound = {
      round,
      issueCode: issue.code,
      severity: issue.severity,
      fixCommand: issue.fix?.command ?? null,
      fixParams: (issue.fix?.params ?? null) as JsonValue,
      beforeResidual,
      afterResidual: after.residual,
      afterStatus: after.status,
    };
    rounds.push(thisRound);
    onEvent?.({ type: 'solve_done', round, status: after.status, residual: after.residual });

    // If this fix didn't help, don't try it again — escalate to a different one
    // next round (or stop once every actionable fix has been exhausted).
    if (!roundImproved(thisRound)) ineffectiveCodes.add(issue.code);
  }

  // Reuse the last in-loop diagnosis when we stopped on it (healthy / no-progress
  // both diagnosed right before breaking); otherwise re-diagnose to capture the
  // post-last-solve state (and cache it).
  const finalDiagnosis =
    lastDiag && (stoppedReason === 'healthy' || stoppedReason === 'no_progress')
      ? lastDiag
      : await dispatchDiagnose(core);
  const healthy = !finalDiagnosis.issues.some((i) => i.severity === 'error' || i.severity === 'warning');
  if (healthy && stoppedReason === 'max_rounds') stoppedReason = 'healthy';
  const result: AutoRefineResult = { rounds, finalDiagnosis, healthy, stoppedReason };
  onEvent?.({ type: 'done', result });
  return result;
}
