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

export type StoppedReason = 'healthy' | 'max_rounds' | 'aborted' | 'error';

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

/** Highest-severity issue that carries an actionable fix (errors before warnings). */
function pickActionable(issues: DiagnoseIssue[]): DiagnoseIssue | null {
  const withFix = issues.filter((i) => !!i.fix);
  return (
    withFix.find((i) => i.severity === 'error') ??
    withFix.find((i) => i.severity === 'warning') ??
    withFix.find((i) => i.code === 'NO_RESULTS') ??
    null
  );
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

  for (let round = 1; round <= maxRounds; round++) {
    if (opts.signal?.aborted) {
      stoppedReason = 'aborted';
      break;
    }

    const diag = diagnoseState(core.store.getState());
    const issue = pickActionable(diag.issues);
    if (!issue) {
      stoppedReason = 'healthy';
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
    rounds.push({
      round,
      issueCode: issue.code,
      severity: issue.severity,
      fixCommand: issue.fix?.command ?? null,
      fixParams: (issue.fix?.params ?? null) as JsonValue,
      beforeResidual,
      afterResidual: after.residual,
      afterStatus: after.status,
    });
    onEvent?.({ type: 'solve_done', round, status: after.status, residual: after.residual });
  }

  const finalDiagnosis = diagnoseState(core.store.getState());
  const healthy = !finalDiagnosis.issues.some((i) => i.severity === 'error' || i.severity === 'warning');
  if (healthy && stoppedReason === 'max_rounds') stoppedReason = 'healthy';
  const result: AutoRefineResult = { rounds, finalDiagnosis, healthy, stoppedReason };
  onEvent?.({ type: 'done', result });
  return result;
}
