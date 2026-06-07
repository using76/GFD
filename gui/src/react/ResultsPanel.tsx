/**
 * Results panel — lists solved fields and lets the user color the viewport by
 * one (dispatches results.contour → ResultsFieldLayer renders it). The
 * solver→UI visual link.
 */

import { useAppState, useDispatch } from './CoreContext';

export function ResultsPanel() {
  const state = useAppState();
  const dispatch = useDispatch();
  const results = state.results;
  if (!results || results.availableFields.length === 0) {
    return <div style={{ padding: 10, color: '#888', fontSize: 12 }}>No results yet — run a solve.</div>;
  }
  return (
    <div style={{ padding: 10, color: '#ddd', fontSize: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 6 }}>Results — color field</div>
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
        {results.availableFields.map((f) => {
          const active = results.activeField === f;
          return (
            <button
              key={f}
              onClick={() => void dispatch('results.contour', { field: f })}
              style={{
                padding: '3px 8px',
                border: '1px solid #3a3a3a',
                borderRadius: 4,
                background: active ? '#4096ff' : '#2a2a2a',
                color: active ? '#fff' : '#ddd',
                cursor: 'pointer',
              }}
            >
              {f}
            </button>
          );
        })}
      </div>
      {results.activeField && results.fieldStats[results.activeField] && (
        <div style={{ marginTop: 8, color: '#aaa' }}>
          {results.activeField}: min {results.fieldStats[results.activeField].min.toExponential(2)} · max{' '}
          {results.fieldStats[results.activeField].max.toExponential(2)} · mean{' '}
          {results.fieldStats[results.activeField].mean.toExponential(2)}
        </div>
      )}
    </div>
  );
}
