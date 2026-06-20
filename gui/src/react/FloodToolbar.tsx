/**
 * Floating viewport panel for the 2D flood simulation — field selector, a
 * time-step control (Δt + Run/Run×5), the running time + peak depth readout, a
 * georeferenced raster export, and reset. Every control dispatches the same
 * `flood.*` command-core commands the AI/MCP use, so manual and AI control share
 * one path. Shown only when a flood scenario is loaded.
 */

import { useState } from 'react';
import { useAppState, useDispatch } from './CoreContext';

const wrap: React.CSSProperties = {
  position: 'absolute', top: 10, right: 10, zIndex: 10,
  display: 'flex', flexDirection: 'column', gap: 6,
  padding: 8, background: 'rgba(22,24,28,0.92)', border: '1px solid #333',
  borderRadius: 6, color: '#cdd6df', fontSize: 11, width: 196,
  fontFamily: 'system-ui, sans-serif', userSelect: 'none',
};
const row: React.CSSProperties = { display: 'flex', alignItems: 'center', gap: 6 };
const seg = (active: boolean): React.CSSProperties => ({
  flex: 1, padding: '3px 0', textAlign: 'center', cursor: 'pointer',
  background: active ? '#2f6f8f' : '#262b32', color: active ? '#fff' : '#9aa7b4',
  border: '1px solid #333', borderRadius: 3, fontSize: 10,
});
const btn: React.CSSProperties = {
  flex: 1, padding: '4px 0', textAlign: 'center', cursor: 'pointer',
  background: '#2f6f8f', color: '#fff', border: '1px solid #2a5f7a', borderRadius: 3, fontSize: 10,
};
const label: React.CSSProperties = { color: '#7f8c99', minWidth: 40 };

export function FloodToolbar() {
  const state = useAppState();
  const dispatch = useDispatch();
  const f = state.flood;
  const [dt, setDt] = useState(2);
  const [busy, setBusy] = useState(false);

  if (!f.loaded) return null;

  const field = f.field;
  const run = async (mult: number) => {
    if (busy) return;
    setBusy(true);
    try {
      for (let i = 0; i < mult; i++) {
        await dispatch('flood.run', { t_end: dt, field });
      }
    } finally {
      setBusy(false);
    }
  };
  const setField = (fld: 'depth' | 'max' | 'velocity') => void dispatch('flood.run', { t_end: 0, field: fld });
  const exportRaster = () => void dispatch('flood.export_raster', { path: `flood_${field}.asc`, field });

  return (
    <div style={wrap}>
      <div style={{ ...row, fontWeight: 600, color: '#aeb9c4' }}>Flood 💧</div>

      <div style={row}>
        <span style={label}>Field</span>
        {(['depth', 'max', 'velocity'] as const).map((fld) => (
          <div key={fld} style={seg(field === fld)} onClick={() => setField(fld)}>
            {fld === 'depth' ? 'Depth' : fld === 'max' ? 'Max' : 'Vel'}
          </div>
        ))}
      </div>

      <div style={row}>
        <span style={label}>Δt {dt}s</span>
        <input
          type="range" min={0.2} max={20} step={0.2} value={dt}
          onChange={(e) => setDt(parseFloat(e.target.value))}
          style={{ flex: 1 }}
        />
      </div>

      <div style={row}>
        <div style={btn} onClick={() => void run(1)}>{busy ? '…' : '▶ Run'}</div>
        <div style={btn} onClick={() => void run(5)}>{busy ? '…' : '▶▶ ×5'}</div>
      </div>

      <div style={{ ...row, justifyContent: 'space-between', color: '#9aa7b4' }}>
        <span>t = {f.time.toFixed(1)} s</span>
        <span>{field} max {f.range[1].toFixed(2)}</span>
      </div>

      <div style={row}>
        <div style={{ ...seg(false), background: '#262b32' }} onClick={exportRaster}>Export .asc</div>
        <div style={{ ...seg(false), background: '#3a2630' }} onClick={() => void dispatch('flood.reset', {})}>Reset</div>
      </div>
    </div>
  );
}
