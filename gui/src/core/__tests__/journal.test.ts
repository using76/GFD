import { describe, it, expect } from 'vitest';
import { Journal } from '../journal';
import type { CommandInvocation } from '../command';

function entry(tab: string, undoable = true) {
  const invocation: CommandInvocation = {
    commandId: 'ui.set_tab',
    params: { tab },
    source: 'human',
  };
  return {
    invocation,
    outcome: { ok: true, statePatch: [{ op: 'replace' as const, path: ['ui', 'activeTab'], value: tab }] },
    inverse: [],
    forward: [{ op: 'replace' as const, path: ['ui', 'activeTab'], value: tab }],
    timestamp: Date.now(),
    undoable,
  };
}

describe('Journal', () => {
  it('records entries and tracks undo/redo availability', () => {
    const j = new Journal();
    expect(j.canUndo()).toBe(false);
    j.record(entry('a'));
    expect(j.canUndo()).toBe(true);
    expect(j.canRedo()).toBe(false);
  });

  it('moves the cursor on undo/redo', () => {
    const j = new Journal();
    j.record(entry('a'));
    j.record(entry('b'));

    expect(j.undo()?.invocation.params.tab).toBe('b');
    expect(j.canRedo()).toBe(true);
    expect(j.redo()?.invocation.params.tab).toBe('b');
    expect(j.canRedo()).toBe(false);
  });

  it('invalidates the redo branch when a new action is recorded', () => {
    const j = new Journal();
    j.record(entry('a'));
    j.record(entry('b'));
    j.undo(); // back over b
    j.record(entry('c')); // new branch
    expect(j.canRedo()).toBe(false);
    expect(j.history().map((e) => e.invocation.params.tab)).toEqual(['a', 'c']);
  });

  it('skips non-undoable entries when finding undo targets', () => {
    const j = new Journal();
    j.record(entry('a', true));
    j.record(entry('b', false)); // e.g. a camera move marked non-undoable
    expect(j.undo()?.invocation.params.tab).toBe('a');
  });

  it('produces a replay sequence marked as replay', () => {
    const j = new Journal();
    j.record(entry('a'));
    j.record(entry('b'));
    const seq = j.replaySequence();
    expect(seq).toHaveLength(2);
    expect(seq.every((i) => i.source === 'replay')).toBe(true);
  });
});
