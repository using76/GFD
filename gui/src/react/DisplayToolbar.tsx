/**
 * Floating viewport toolbar for view controls — render mode, opacity, layer
 * show/hide, and the section (clipping) plane. Each control dispatches the same
 * `display.*` command-core commands the AI uses, so manual and AI control stay
 * in sync and share one code path.
 */

import type { JsonObject } from '../core';
import { useAppState, useDispatch } from './CoreContext';

const wrap: React.CSSProperties = {
  position: 'absolute', top: 10, left: 10, zIndex: 10,
  display: 'flex', flexDirection: 'column', gap: 6,
  padding: 8, background: 'rgba(22,24,28,0.92)', border: '1px solid #333',
  borderRadius: 6, color: '#cdd6df', fontSize: 11, width: 188,
  fontFamily: 'system-ui, sans-serif', userSelect: 'none',
};
const row: React.CSSProperties = { display: 'flex', alignItems: 'center', gap: 6 };
const seg = (active: boolean): React.CSSProperties => ({
  flex: 1, padding: '3px 0', textAlign: 'center', cursor: 'pointer',
  background: active ? '#3b6ea5' : '#262b32', color: active ? '#fff' : '#9aa7b4',
  border: '1px solid #333', borderRadius: 3, fontSize: 10,
});
const label: React.CSSProperties = { color: '#7f8c99', minWidth: 46 };

/** Union extent of visible geometry along an axis, for the offset slider range. */
function axisExtent(state: ReturnType<typeof useAppState>, axis: 0 | 1 | 2): [number, number] {
  let lo = Infinity, hi = -Infinity;
  for (const n of Object.values(state.doc.geometry.nodes)) {
    if (!n.visible) continue;
    lo = Math.min(lo, n.bbox.min[axis]);
    hi = Math.max(hi, n.bbox.max[axis]);
  }
  const gm = state.gmsh.mesh;
  if (gm && gm.positions.length) {
    for (let i = axis; i < gm.positions.length; i += 3) {
      lo = Math.min(lo, gm.positions[i]);
      hi = Math.max(hi, gm.positions[i]);
    }
  }
  if (!Number.isFinite(lo) || hi <= lo) return [-10, 10];
  const pad = (hi - lo) * 0.05 + 1e-6;
  return [lo - pad, hi + pad];
}

export function DisplayToolbar() {
  const state = useAppState();
  const dispatch = useDispatch();
  const d = state.display;
  const set = (id: string, params: JsonObject) => void dispatch(id, params);

  const axisIdx = d.sectionPlane.axis === 'y' ? 1 : d.sectionPlane.axis === 'z' ? 2 : 0;
  const [omin, omax] = axisExtent(state, axisIdx as 0 | 1 | 2);

  return (
    <div style={wrap}>
      <div style={{ ...row, fontWeight: 600, color: '#aeb9c4' }}>Display</div>

      {/* Render mode */}
      <div style={row}>
        {(['shaded', 'shaded_edges', 'wireframe'] as const).map((m) => (
          <div key={m} style={seg(d.renderMode === m)} onClick={() => set('display.set_render_mode', { mode: m })}>
            {m === 'shaded' ? 'Solid' : m === 'shaded_edges' ? 'Edges' : 'Wire'}
          </div>
        ))}
      </div>

      {/* Opacity */}
      <div style={row}>
        <span style={label}>Opacity</span>
        <input
          type="range" min={0.1} max={1} step={0.05} value={d.opacity}
          onChange={(e) => set('display.set_opacity', { opacity: parseFloat(e.target.value) })}
          style={{ flex: 1 }}
        />
        <span style={{ width: 26, textAlign: 'right' }}>{d.opacity.toFixed(2)}</span>
      </div>

      {/* Layers */}
      <div style={row}>
        <span style={label}>Show</span>
        {(['geometry', 'mesh', 'results'] as const).map((ly) => (
          <div key={ly} style={seg(d.layers[ly])} onClick={() => set('display.set_layer', { layer: ly, visible: !d.layers[ly] })}>
            {ly === 'geometry' ? 'Geom' : ly === 'mesh' ? 'Mesh' : 'Res'}
          </div>
        ))}
      </div>

      {/* Section plane */}
      <div style={row}>
        <div style={seg(d.sectionPlane.enabled)} onClick={() => set('display.set_section_plane', { enabled: !d.sectionPlane.enabled })}>
          {d.sectionPlane.enabled ? 'Section ON' : 'Section OFF'}
        </div>
      </div>
      {d.sectionPlane.enabled && (
        <>
          <div style={row}>
            {(['x', 'y', 'z'] as const).map((ax) => (
              <div key={ax} style={seg(d.sectionPlane.axis === ax)} onClick={() => set('display.set_section_plane', { axis: ax })}>
                {ax.toUpperCase()}
              </div>
            ))}
          </div>
          <div style={row}>
            <span style={label}>Offset</span>
            <input
              type="range" min={omin} max={omax} step={(omax - omin) / 200}
              value={Math.min(omax, Math.max(omin, d.sectionPlane.offset))}
              onChange={(e) => set('display.set_section_plane', { offset: parseFloat(e.target.value) })}
              style={{ flex: 1 }}
            />
          </div>
        </>
      )}
    </div>
  );
}
