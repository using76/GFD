/**
 * Results panel — lists solved fields and provides the user controls for
 * results visualization: contour field selection, and vector/streamline toggles
 * + parameters. Every control dispatches a command (results.contour /
 * results.set_viz), so the AI can drive the exact same controls.
 */

import { useState } from 'react';
import { useAppState, useDispatch, useCore } from './CoreContext';
import { runAutoRefine } from '../core/solver/autoRefine';

interface DiagIssueView {
  code: string;
  severity: 'info' | 'warning' | 'error';
  message: string;
  suggestion?: string;
  fix?: { command: string; params: Record<string, number | string | boolean> };
}
interface DiagView {
  summary?: string;
  converged?: boolean;
  reynolds?: number | null;
  flowRegime?: string;
  residualsByEq?: Record<string, number | null> | null;
  dominantEquation?: string | null;
  issues?: DiagIssueView[];
}

/** Per-equation residual bar; the convergence-limiting equation is highlighted. */
function EqResiduals({ diag }: { diag: DiagView }) {
  const eq = diag.residualsByEq;
  if (!eq) return null;
  const entries = Object.entries(eq).filter(([, v]) => typeof v === 'number') as Array<[string, number]>;
  if (!entries.length) return null;
  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, margin: '4px 0', color: '#bbb' }}>
      <span style={{ color: '#888' }}>방정식 잔차:</span>
      {entries.map(([name, val]) => {
        const dominant = name === diag.dominantEquation;
        return (
          <span key={name} style={{ color: dominant ? '#ffc14a' : '#bbb', fontWeight: dominant ? 600 : 400 }}>
            {name} {val.toExponential(1)}
            {dominant ? ' ◄' : ''}
          </span>
        );
      })}
    </div>
  );
}

const SEV_COLOR: Record<string, string> = { error: '#ff6b6b', warning: '#ffc14a', info: '#7fb3d5' };

/** Live analysis surface: shows calc.diagnose output and one-click fixes. */
function DiagnosisSection() {
  const state = useAppState();
  const dispatch = useDispatch();
  const core = useCore();
  const [refining, setRefining] = useState(false);
  const diag = state.diagnosis as DiagView | null;

  const onAutoRefine = async () => {
    setRefining(true);
    try {
      await runAutoRefine(core, { maxRounds: 3 });
    } finally {
      setRefining(false);
    }
  };

  return (
    <div style={{ marginBottom: 10, borderBottom: '1px solid #333', paddingBottom: 8 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 6 }}>
        <span style={{ fontWeight: 600 }}>진단 / Diagnosis</span>
        <button
          onClick={() => void dispatch('calc.diagnose', {})}
          style={{ padding: '2px 8px', border: '1px solid #3a3a3a', borderRadius: 4, background: '#2a2a2a', color: '#ddd', cursor: 'pointer' }}
        >
          진단
        </button>
        <button
          onClick={() => void onAutoRefine()}
          disabled={refining}
          style={{ padding: '2px 8px', border: '1px solid #3a5a8a', borderRadius: 4, background: refining ? '#333' : '#27406a', color: '#fff', cursor: refining ? 'default' : 'pointer' }}
        >
          {refining ? '자동 수정 중…' : '자동 수정'}
        </button>
      </div>
      {!diag && <div style={{ color: '#888' }}>아직 진단 없음 — "진단"을 누르세요.</div>}
      {diag && (
        <>
          <div style={{ color: '#bbb', marginBottom: 4 }}>{diag.summary}</div>
          <EqResiduals diag={diag} />
          {(diag.issues ?? []).map((iss, idx) => (
            <div key={`${iss.code}_${idx}`} style={{ display: 'flex', alignItems: 'flex-start', gap: 6, marginTop: 4 }}>
              <span style={{ color: SEV_COLOR[iss.severity] ?? '#aaa', fontWeight: 600, minWidth: 54 }}>[{iss.severity}]</span>
              <span style={{ flex: 1, color: '#ddd' }}>
                {iss.message}
                {iss.suggestion && <span style={{ color: '#999' }}> — {iss.suggestion}</span>}
              </span>
              {iss.fix && (
                <button
                  onClick={() => void dispatch(iss.fix!.command, iss.fix!.params)}
                  style={{ padding: '1px 6px', border: '1px solid #3a3a3a', borderRadius: 4, background: '#2a2a2a', color: '#9ad', cursor: 'pointer', whiteSpace: 'nowrap' }}
                >
                  적용
                </button>
              )}
            </div>
          ))}
        </>
      )}
    </div>
  );
}

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
    return (
      <div style={{ padding: 10, color: '#ddd', fontSize: 12 }}>
        <DiagnosisSection />
        <div style={{ color: '#888' }}>No results yet — run a solve.</div>
      </div>
    );
  }

  const stats = results.activeField ? results.fieldStats[results.activeField] : undefined;

  return (
    <div style={{ padding: 10, color: '#ddd', fontSize: 12 }}>
      <DiagnosisSection />
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 6 }}>
        <span style={{ fontWeight: 600 }}>Results — color field</span>
        <button
          onClick={() => void dispatch('results.vorticity', {})}
          title="Compute |∇×u| and add it as a field"
          style={{ padding: '2px 8px', border: '1px solid #3a3a3a', borderRadius: 4, background: '#2a2a2a', color: '#9ad', cursor: 'pointer' }}
        >
          + 와도
        </button>
        <button
          onClick={() => void dispatch('results.qcriterion', {})}
          title="Compute the Q-criterion (vortex cores) and add it as a field"
          style={{ padding: '2px 8px', border: '1px solid #3a3a3a', borderRadius: 4, background: '#2a2a2a', color: '#9ad', cursor: 'pointer' }}
        >
          + Q
        </button>
      </div>
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

        <Toggle label="Isosurface" checked={viz.showIsosurface} onChange={(v) => setViz({ showIsosurface: v })} />
        {viz.showIsosurface && stats && (
          <Slider
            label="isovalue"
            value={viz.isovalue}
            min={stats.min}
            max={stats.max}
            step={(stats.max - stats.min) / 100 || 0.01}
            onChange={(v) => setViz({ isovalue: v })}
          />
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
