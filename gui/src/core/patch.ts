/**
 * Dependency-free immutable state patches.
 *
 * Commands return a list of `PatchOp`s describing how they change the AppState.
 * Applying a patch produces a NEW state (the previous one is never mutated) plus
 * the inverse patch, which the journal stores so undo/redo is exact and cheap.
 *
 * Paths are arrays of property keys / array indices, e.g. ["doc", "geometry",
 * "nodes", "shape_1", "visible"].
 */

import type { JsonValue } from './types';

export type PathSeg = string | number;

export type PatchOp =
  | { op: 'replace'; path: PathSeg[]; value: JsonValue }
  | { op: 'add'; path: PathSeg[]; value: JsonValue }
  | { op: 'remove'; path: PathSeg[] };

/** A patch op with the previous value captured, enabling exact inversion. */
export type AppliedOp =
  | { op: 'replace'; path: PathSeg[]; value: JsonValue; prev: JsonValue }
  | { op: 'add'; path: PathSeg[]; value: JsonValue }
  | { op: 'remove'; path: PathSeg[]; prev: JsonValue };

function clone<T>(value: T): T {
  return structuredClone(value);
}

function getContainer(root: unknown, path: PathSeg[]): { parent: unknown; key: PathSeg } {
  let node: unknown = root;
  for (let i = 0; i < path.length - 1; i++) {
    const seg = path[i];
    if (node === null || typeof node !== 'object') {
      throw new Error(`patch: cannot descend into non-object at "${path.slice(0, i + 1).join('/')}"`);
    }
    node = (node as Record<PathSeg, unknown>)[seg];
  }
  return { parent: node, key: path[path.length - 1] };
}

function readAt(parent: unknown, key: PathSeg): JsonValue {
  if (parent === null || typeof parent !== 'object') {
    throw new Error('patch: parent is not a container');
  }
  return (parent as Record<PathSeg, JsonValue>)[key];
}

/**
 * Apply `ops` to `state`, returning a fresh state and the inverse ops.
 * The input `state` is not modified.
 */
export function applyPatch<T>(state: T, ops: PatchOp[]): { next: T; inverse: AppliedOp[] } {
  const next = clone(state);
  const inverse: AppliedOp[] = [];

  for (const op of ops) {
    if (op.path.length === 0) {
      throw new Error('patch: empty path is not allowed');
    }
    const { parent, key } = getContainer(next, op.path);
    if (parent === null || typeof parent !== 'object') {
      throw new Error(`patch: invalid container at "${op.path.join('/')}"`);
    }
    const container = parent as Record<PathSeg, JsonValue>;

    switch (op.op) {
      case 'replace': {
        const prev = readAt(parent, key);
        container[key] = clone(op.value);
        inverse.push({ op: 'replace', path: op.path, value: prev, prev: clone(op.value) });
        break;
      }
      case 'add': {
        if (Array.isArray(parent) && typeof key === 'number') {
          (parent as JsonValue[]).splice(key, 0, clone(op.value));
          inverse.push({ op: 'remove', path: op.path, prev: clone(op.value) });
        } else {
          const existed = key in container;
          if (existed) {
            const prev = container[key];
            container[key] = clone(op.value);
            inverse.push({ op: 'replace', path: op.path, value: prev, prev: clone(op.value) });
          } else {
            container[key] = clone(op.value);
            inverse.push({ op: 'remove', path: op.path, prev: clone(op.value) });
          }
        }
        break;
      }
      case 'remove': {
        const prev = readAt(parent, key);
        if (Array.isArray(parent) && typeof key === 'number') {
          (parent as JsonValue[]).splice(key, 1);
        } else {
          delete container[key];
        }
        inverse.push({ op: 'add', path: op.path, value: clone(prev) });
        break;
      }
    }
  }

  // The inverse must be applied in reverse order to undo correctly.
  inverse.reverse();
  return { next, inverse };
}

/** Convert captured AppliedOps back into plain PatchOps (e.g. for re-applying). */
export function toPatchOps(applied: AppliedOp[]): PatchOp[] {
  return applied.map((op) => {
    switch (op.op) {
      case 'replace':
        return { op: 'replace', path: op.path, value: op.value };
      case 'add':
        return { op: 'add', path: op.path, value: op.value };
      case 'remove':
        return { op: 'remove', path: op.path };
    }
  });
}
