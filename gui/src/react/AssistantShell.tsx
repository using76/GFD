/**
 * AssistantShell — the AI-only workbench.
 *
 * No ribbon, no parameter forms, no clickable feature tree. The human operates
 * the workbench by talking to the AI (ChatPanel); the AI drives the same command
 * registry through the agent loop; and the screen shows only the *result* (the
 * 3D ViewportV2, subscribed to AppState) and the *process* (the action log in the
 * chat). A thin read-only status strip reflects the current model state.
 *
 * Opt in with ?ai (see main.tsx). This is the realization of the "discard the
 * manual tabs, control by AI, show results graphically" direction.
 */

import { useEffect, useMemo } from 'react';
import { createCore, createInitialState, type Core, type ViewDefaults } from '../core';
import { CoreProvider, useAppState } from './CoreContext';
import { ChatPanel } from './ChatPanel';
import { ViewportV2 } from './engine/ViewportV2';
import { McpControlResponder } from './McpControlResponder';

const DEFAULTS_KEY = 'gfd.viewDefaults.v1';

function loadSavedDefaults(): ViewDefaults | null {
  try {
    if (typeof window === 'undefined' || !window.localStorage) return null;
    const raw = window.localStorage.getItem(DEFAULTS_KEY);
    return raw ? (JSON.parse(raw) as ViewDefaults) : null;
  } catch {
    return null;
  }
}

function persistDefaults(d: ViewDefaults): void {
  try {
    if (typeof window === 'undefined' || !window.localStorage) return;
    window.localStorage.setItem(DEFAULTS_KEY, JSON.stringify(d));
  } catch {
    // ignore quota / availability errors
  }
}

/** Build a core whose view (and saved defaults) is hydrated from localStorage. */
function hydratedCore(): Core {
  const state = createInitialState();
  const saved = loadSavedDefaults();
  if (saved) {
    state.viewDefaults = saved;
    state.camera = JSON.parse(JSON.stringify(saved.camera));
    state.display = JSON.parse(JSON.stringify(saved.display));
    state.viz = JSON.parse(JSON.stringify(saved.viz));
  }
  return createCore({ initialState: state });
}

function StatusStrip() {
  const s = useAppState();
  const stat = (label: string, value: string) => (
    <span>
      <span style={{ color: '#666' }}>{label} </span>
      <span style={{ color: '#bbb' }}>{value}</span>
    </span>
  );
  return (
    <div style={strip}>
      {stat('rev', String(s.doc.revision))}
      {stat('shapes', String(Object.keys(s.doc.geometry.nodes).length))}
      {stat('mesh', s.mesh ? `${s.mesh.cellCount} cells` : '—')}
      {stat('solver', `${s.solver.status}${s.solver.residual !== null ? ` (res ${s.solver.residual.toExponential(2)})` : ''}`)}
      {stat('fields', s.results?.availableFields.join(', ') || '—')}
    </div>
  );
}

function Shell() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', fontFamily: 'system-ui, sans-serif', background: '#0b0c10' }}>
      <div style={topbar}>
        <strong style={{ color: '#4096ff' }}>GFD</strong>
        <span style={{ fontSize: 11, color: '#888' }}>AI workbench</span>
      </div>
      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <div style={{ width: 380, borderRight: '1px solid #23262d', minWidth: 0 }}>
          <ChatPanel />
        </div>
        <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
          <div style={{ flex: 1, minHeight: 0 }}>
            <ViewportV2 />
          </div>
          <StatusStrip />
        </div>
      </div>
    </div>
  );
}

const topbar: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: 8,
  padding: '5px 12px',
  background: '#121317',
  color: '#ddd',
  borderBottom: '1px solid #23262d',
};

const strip: React.CSSProperties = {
  display: 'flex',
  gap: 18,
  padding: '4px 12px',
  background: '#121317',
  fontSize: 11,
  borderTop: '1px solid #23262d',
};

export function AssistantShell({ core }: { core?: Core }) {
  const ownCore = useMemo(() => core ?? hydratedCore(), [core]);

  // Persist the user's saved view defaults across sessions whenever they change.
  useEffect(() => {
    let last = JSON.stringify(ownCore.store.getState().viewDefaults);
    return ownCore.store.subscribe(() => {
      const cur = JSON.stringify(ownCore.store.getState().viewDefaults);
      if (cur !== last) {
        last = cur;
        persistDefaults(ownCore.store.getState().viewDefaults);
      }
    });
  }, [ownCore]);

  return (
    <CoreProvider core={ownCore}>
      <McpControlResponder />
      <Shell />
    </CoreProvider>
  );
}
