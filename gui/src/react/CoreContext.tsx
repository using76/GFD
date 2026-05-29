/**
 * React bridge to the command-core (Phase 3/4).
 *
 * Components NEVER mutate state directly — they read AppState via `useAppState`
 * (a React external-store subscription) and `dispatch` commands via `useDispatch`.
 * This is the same path the MCP/agent uses, so the human UI and AI stay in sync.
 */

import { createContext, useCallback, useContext, useMemo, useSyncExternalStore, type ReactNode } from 'react';
import { createCore, type Core } from '../core';
import type { AppState } from '../core';
import type { CommandOutcome, CommandSource } from '../core';
import type { JsonObject } from '../core';

const CoreCtx = createContext<Core | null>(null);

export function CoreProvider({ core, children }: { core?: Core; children: ReactNode }) {
  const value = useMemo(() => core ?? createCore(), [core]);
  return <CoreCtx.Provider value={value}>{children}</CoreCtx.Provider>;
}

export function useCore(): Core {
  const core = useContext(CoreCtx);
  if (!core) throw new Error('useCore must be used within a <CoreProvider>');
  return core;
}

/** Subscribe to the canonical AppState; re-renders when a command mutates it. */
export function useAppState(): AppState {
  const core = useCore();
  const subscribe = useCallback((cb: () => void) => core.store.subscribe(cb), [core]);
  const getSnapshot = useCallback(() => core.store.getState() as AppState, [core]);
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

export type DispatchFn = (
  commandId: string,
  params?: JsonObject,
  source?: CommandSource
) => Promise<CommandOutcome>;

export function useDispatch(): DispatchFn {
  const core = useCore();
  return useCallback(
    (commandId, params = {}, source = 'human') => core.dispatcher.dispatch({ commandId, params, source }),
    [core]
  );
}

export function useUndoRedo(): { undo: () => Promise<boolean>; redo: () => Promise<boolean> } {
  const core = useCore();
  return useMemo(
    () => ({ undo: () => core.dispatcher.undo(), redo: () => core.dispatcher.redo() }),
    [core]
  );
}
