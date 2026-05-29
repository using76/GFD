/**
 * The command journal — undo/redo + replay + audit, all in one structure.
 *
 *  - Undo/redo: each entry stores the inverse patch, so undo is exact and cheap.
 *  - Replay: re-dispatching the recorded invocations on a fresh state is
 *    deterministic (used by regression tests).
 *  - Audit: every agent action is recorded with `source: 'agent'`, viewable in
 *    the UI and revertable.
 */

import type { CommandInvocation, CommandOutcome } from './command';
import type { AppliedOp, PatchOp } from './patch';

export interface JournalEntry {
  seq: number;
  invocation: CommandInvocation;
  outcome: CommandOutcome;
  /** Inverse of the applied state patch — applied on undo. */
  inverse: AppliedOp[];
  /** Forward patch the command produced — re-applied on redo. */
  forward: PatchOp[];
  timestamp: number;
  undoable: boolean;
}

export class Journal {
  private entries: JournalEntry[] = [];
  /** Index of the next slot; entries before it are "done", after are "redoable". */
  private cursor = 0;
  private seqCounter = 0;

  record(entry: Omit<JournalEntry, 'seq'>): JournalEntry {
    // A new action invalidates any redo branch.
    if (this.cursor < this.entries.length) {
      this.entries.length = this.cursor;
    }
    const full: JournalEntry = { ...entry, seq: this.seqCounter++ };
    this.entries.push(full);
    this.cursor = this.entries.length;
    return full;
  }

  canUndo(): boolean {
    return this.findPrevUndoable() >= 0;
  }

  canRedo(): boolean {
    return this.findNextUndoable() >= 0;
  }

  /** Move the cursor back to the previous undoable entry and return it. */
  undo(): JournalEntry | null {
    const idx = this.findPrevUndoable();
    if (idx < 0) return null;
    this.cursor = idx;
    return this.entries[idx];
  }

  /** Move the cursor forward over the next undoable entry and return it. */
  redo(): JournalEntry | null {
    const idx = this.findNextUndoable();
    if (idx < 0) return null;
    this.cursor = idx + 1;
    return this.entries[idx];
  }

  /** Full ordered history (for the audit-log panel). */
  history(): readonly JournalEntry[] {
    return this.entries;
  }

  /** Invocations up to the cursor — the deterministic replay sequence. */
  replaySequence(): CommandInvocation[] {
    return this.entries.slice(0, this.cursor).map((e) => ({ ...e.invocation, source: 'replay' }));
  }

  clear(): void {
    this.entries = [];
    this.cursor = 0;
    this.seqCounter = 0;
  }

  private findPrevUndoable(): number {
    for (let i = this.cursor - 1; i >= 0; i--) {
      if (this.entries[i].undoable && this.entries[i].outcome.ok) return i;
    }
    return -1;
  }

  private findNextUndoable(): number {
    for (let i = this.cursor; i < this.entries.length; i++) {
      if (this.entries[i].undoable && this.entries[i].outcome.ok) return i;
    }
    return -1;
  }
}
