import { describe, it, expect } from 'vitest';
import { applyPatch, toPatchOps, type PatchOp } from '../patch';

describe('applyPatch', () => {
  it('replaces a nested value immutably and inverts exactly', () => {
    const state = { a: { b: 1 }, list: [10, 20] };
    const ops: PatchOp[] = [{ op: 'replace', path: ['a', 'b'], value: 42 }];
    const { next, inverse } = applyPatch(state, ops);

    expect(next.a.b).toBe(42);
    expect(state.a.b).toBe(1); // original untouched

    const back = applyPatch(next, toPatchOps(inverse)).next;
    expect(back).toEqual(state);
  });

  it('adds and removes object keys with correct inverse', () => {
    const state: { obj: Record<string, number> } = { obj: { x: 1 } };
    const { next, inverse } = applyPatch(state, [{ op: 'add', path: ['obj', 'y'], value: 2 }]);
    expect(next.obj).toEqual({ x: 1, y: 2 });

    const back = applyPatch(next, toPatchOps(inverse)).next;
    expect(back).toEqual(state);
  });

  it('splices array elements and inverts', () => {
    const state = { list: [1, 2, 3] };
    const { next, inverse } = applyPatch(state, [{ op: 'remove', path: ['list', 1] }]);
    expect(next.list).toEqual([1, 3]);

    const back = applyPatch(next, toPatchOps(inverse)).next;
    expect(back.list).toEqual([1, 2, 3]);
  });

  it('applies multiple ops and inverts them in reverse', () => {
    const state = { n: 0, s: 'a' };
    const ops: PatchOp[] = [
      { op: 'replace', path: ['n'], value: 5 },
      { op: 'replace', path: ['s'], value: 'b' },
    ];
    const { next, inverse } = applyPatch(state, ops);
    expect(next).toEqual({ n: 5, s: 'b' });
    expect(applyPatch(next, toPatchOps(inverse)).next).toEqual(state);
  });
});
