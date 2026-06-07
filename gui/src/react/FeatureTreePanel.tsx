/**
 * Feature tree panel (Phase 3/4). Reads the GeometryTree from AppState and lets
 * the user select / delete shapes — all via commands, so AI and human share the
 * same selection + edit path.
 */

import { useAppState, useDispatch } from './CoreContext';

export function FeatureTreePanel() {
  const state = useAppState();
  const dispatch = useDispatch();
  const nodes = Object.values(state.doc.geometry.nodes);
  const selected = new Set(state.selection.ids);

  return (
    <div style={{ padding: 8, color: '#ddd', fontSize: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 6 }}>Geometry ({nodes.length})</div>
      {nodes.length === 0 && <div style={{ color: '#888' }}>No shapes yet.</div>}
      {nodes.map((n) => (
        <div
          key={n.id}
          onClick={() => dispatch('selection.set', { ids: [n.id] })}
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            padding: '3px 6px',
            borderRadius: 3,
            cursor: 'pointer',
            background: selected.has(n.id) ? '#234' : 'transparent',
            opacity: n.visible ? 1 : 0.5,
          }}
        >
          <span>
            {n.name} <span style={{ color: '#777' }}>({n.kind})</span>
          </span>
          <button
            onClick={(e) => {
              e.stopPropagation();
              void dispatch('geometry.delete', { shape_id: n.id });
            }}
            style={{ background: 'transparent', border: 'none', color: '#f66', cursor: 'pointer' }}
            title="Delete"
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}
