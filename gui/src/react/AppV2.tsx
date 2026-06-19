/**
 * AppV2 — the new command-core-driven shell (Phases 3/4).
 *
 * Layout: data-driven ribbon (from the registry) + feature tree + schema-driven
 * command form panel + the R3F ViewportV2. The legacy App remains the default
 * entry; this shell is opt-in (see main.tsx ?v2) until feature parity (Phase 4
 * completion) is reached.
 */

import { useState } from 'react';
import type { Core } from '../core';
import { CoreProvider, useAppState, useUndoRedo } from './CoreContext';
import { RibbonFromRegistry } from './RibbonFromRegistry';
import { CommandFormPanel } from './CommandFormPanel';
import { FeatureTreePanel } from './FeatureTreePanel';
import { ResultsPanel } from './ResultsPanel';
import { ViewportV2 } from './engine/ViewportV2';
import { DisplayToolbar } from './DisplayToolbar';

function StatusBar() {
  const s = useAppState();
  return (
    <div style={{ display: 'flex', gap: 16, padding: '4px 10px', background: '#1a1a1a', color: '#aaa', fontSize: 11, borderTop: '1px solid #333' }}>
      <span>rev {s.doc.revision}</span>
      <span>shapes {Object.keys(s.doc.geometry.nodes).length}</span>
      <span>mesh {s.mesh ? `${s.mesh.cellCount} cells` : '—'}</span>
      <span>solver {s.solver.status}{s.solver.residual !== null ? ` (res ${s.solver.residual.toExponential(2)})` : ''}</span>
      <span>fields {s.results?.availableFields.join(', ') || '—'}</span>
    </div>
  );
}

function Shell() {
  const [activeCommand, setActiveCommand] = useState<string | null>(null);
  const { undo, redo } = useUndoRedo();

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', fontFamily: 'system-ui, sans-serif', background: '#101216' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '4px 10px', background: '#161616', color: '#ddd', borderBottom: '1px solid #333' }}>
        <strong style={{ color: '#4096ff' }}>GFD</strong>
        <span style={{ fontSize: 11, color: '#888' }}>command-core workbench (v2)</span>
        <div style={{ flex: 1 }} />
        <button onClick={() => void undo()} style={btn}>Undo</button>
        <button onClick={() => void redo()} style={btn}>Redo</button>
      </div>

      <RibbonFromRegistry onPick={setActiveCommand} />

      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <div style={{ width: 300, borderRight: '1px solid #333', background: '#161616', overflowY: 'auto', display: 'flex', flexDirection: 'column' }}>
          <FeatureTreePanel />
          <div style={{ borderTop: '1px solid #333' }}>
            {activeCommand ? (
              <CommandFormPanel commandId={activeCommand} />
            ) : (
              <div style={{ padding: 10, color: '#888', fontSize: 12 }}>Pick a ribbon command to edit its parameters.</div>
            )}
          </div>
          <div style={{ borderTop: '1px solid #333' }}>
            <ResultsPanel />
          </div>
        </div>
        <div style={{ flex: 1, minWidth: 0, position: 'relative' }}>
          <DisplayToolbar />
          <ViewportV2 />
        </div>
      </div>

      <StatusBar />
    </div>
  );
}

const btn: React.CSSProperties = {
  padding: '3px 10px',
  background: '#2a2a2a',
  color: '#ddd',
  border: '1px solid #3a3a3a',
  borderRadius: 4,
  cursor: 'pointer',
  fontSize: 12,
};

export function AppV2({ core }: { core?: Core }) {
  return (
    <CoreProvider core={core}>
      <Shell />
    </CoreProvider>
  );
}
