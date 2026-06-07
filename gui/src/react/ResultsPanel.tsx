/**
 * Results panel — lists solved fields and provides the user controls for
 * results visualization: contour field selection, and vector/streamline toggles
 * + parameters. Every control dispatches a command (results.contour /
 * results.set_viz), so the AI can drive the exact same controls.
 */

import { useAppState, useDispatch } from './CoreContext';

function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}

function Slider({
  label,
  value,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
}) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 4 }}>
      <span style={{ width: 90, color: '#aaa' }}>{label}</span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{ flex: 1 }}
      />
      <span style={{ width: 36, textAlign: 'right' }}>{value}</span>
    </div>
  );
}

export function ResultsPanel() {
  const state = useAppState();
  const dispatch = useDispatch();
  const results = state.results;
  const viz = state.viz;
  const setViz = (p: Record<string, number | boolean>) => void dispatch('results.set_viz', p);

  if (!results || results.availableFields.length === 0) {
    return <div style={{ padding: 10, color: '#888', fontSize: 12 }}>No results yet — run a solve.</div>;
  }

  const stats = results.activeField ? results.fieldStats[results.activeField] : undefined;

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
      {stats && (
        <div style={{ marginTop: 8, color: '#aaa' }}>
          {results.activeField}: min {stats.min.toExponential(2)} · max {stats.max.toExponential(2)} · mean{' '}
          {stats.mean.toExponential(2)}
        </div>
      )}

      <div style={{ marginTop: 10, borderTop: '1px solid #333', paddingTop: 8, display: 'flex', flexDirection: 'column', gap: 4 }}>
        <Toggle label="Contour" checked={viz.showContour} onChange={(v) => setViz({ showContour: v })} />

        <Toggle label="Vectors" checked={viz.showVectors} onChange={(v) => setViz({ showVectors: v })} />
        {viz.showVectors && (
          <>
            <Slider label="scale" value={viz.vectorScale} min={0.1} max={3} step={0.1} onChange={(v) => setViz({ vectorScale: v })} />
            <Slider label="density" value={viz.vectorStride} min={1} max={16} step={1} onChange={(v) => setViz({ vectorStride: v })} />
          </>
        )}

        <Toggle label="Streamlines" checked={viz.showStreamlines} onChange={(v) => setViz({ showStreamlines: v })} />
        {viz.showStreamlines && (
          <>
            <Slider label="seeds" value={viz.streamlineSeeds} min={1} max={100} step={1} onChange={(v) => setViz({ streamlineSeeds: v })} />
            <Slider label="steps" value={viz.streamlineSteps} min={10} max={1000} step={10} onChange={(v) => setViz({ streamlineSteps: v })} />
          </>
        )}
      </div>
    </div>
  );
}
