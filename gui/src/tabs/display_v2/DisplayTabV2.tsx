import React from 'react';
import { Slider, Switch, Button, ColorPicker, Space, Select, message } from 'antd';
import type { Color } from 'antd/es/color-picker';
import { useCadStore } from '../../store/cadStore';
import type { RenderMode } from '../../store/cadStore';
import cadClient from '../../ipc/cadClient';

/**
 * Display v2 — rendering controls for the gfd-cad kernel shapes.
 *
 * Iteration 10 scope: per-shape visibility toggle and RGB color picker.
 * Section view clipping plane, transparency, and rendering modes (wireframe
 * / hidden line) are wired with TODO hooks and will go live once the
 * CadKernelLayer grows the corresponding material uniforms.
 */
const DisplayTabV2: React.FC = () => {
  const shapes = useCadStore((s) => s.shapes);
  const setVisible = useCadStore((s) => s.setVisible);
  const setMode = useCadStore((s) => s.setMode);
  const setOpacity = useCadStore((s) => s.setOpacity);
  const removeShape = useCadStore((s) => s.removeShape);

  const rgbTo01 = (c: Color) => {
    const hex = typeof c === 'string' ? c : c.toHexString();
    const h = hex.replace('#', '');
    return [
      parseInt(h.substring(0, 2), 16) / 255,
      parseInt(h.substring(2, 4), 16) / 255,
      parseInt(h.substring(4, 6), 16) / 255,
    ] as [number, number, number];
  };
  const rgb01ToHex = (rgb: [number, number, number]) =>
    '#' + rgb.map((v) => Math.round(v * 255).toString(16).padStart(2, '0')).join('');

  return (
    <div style={{ padding: 12, color: '#ddd' }}>
      <h4 style={{ color: '#4096ff', marginTop: 0 }}>Display v2</h4>

      {shapes.length === 0 ? (
        <div style={{ color: '#667', fontSize: 12 }}>
          No CAD shapes yet. Create some in the Design tab.
        </div>
      ) : (
        <div>
          <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse' }}>
            <thead>
              <tr style={{ color: '#889', textAlign: 'left' }}>
                <th style={{ padding: '4px 8px' }}>Shape</th>
                <th style={{ padding: '4px 8px' }}>Color</th>
                <th style={{ padding: '4px 8px' }}>Mode</th>
                <th style={{ padding: '4px 8px' }}>Opacity</th>
                <th style={{ padding: '4px 8px' }}>Visible</th>
                <th style={{ padding: '4px 8px' }}>—</th>
              </tr>
            </thead>
            <tbody>
              {shapes.map((s) => (
                <tr key={s.id} style={{ borderTop: '1px solid #303050' }}>
                  <td style={{ padding: '4px 8px', color: '#aab' }}>
                    {s.id}
                    <span style={{ color: '#667', fontSize: 10, marginLeft: 6 }}>({s.kind})</span>
                  </td>
                  <td style={{ padding: '4px 8px' }}>
                    <ColorPicker
                      value={rgb01ToHex(s.color)}
                      size="small"
                      onChange={(c) => {
                        useCadStore.setState((state) => ({
                          shapes: state.shapes.map((x) => x.id === s.id ? { ...x, color: rgbTo01(c) } : x),
                        }));
                      }}
                    />
                  </td>
                  <td style={{ padding: '4px 8px' }}>
                    <Select
                      size="small"
                      style={{ width: 110 }}
                      value={s.mode ?? (s.wireframe ? 'wireframe' : 'shaded')}
                      onChange={(v: RenderMode) => setMode(s.id, v)}
                      options={[
                        { value: 'shaded', label: 'Shaded' },
                        { value: 'shaded_edges', label: 'Shaded+Edges' },
                        { value: 'wireframe', label: 'Wireframe' },
                        { value: 'hidden_line', label: 'Hidden-line' },
                      ]}
                    />
                  </td>
                  <td style={{ padding: '4px 8px', minWidth: 100 }}>
                    <Slider
                      min={0.1}
                      max={1}
                      step={0.05}
                      value={s.opacity ?? 1}
                      onChange={(v) => setOpacity(s.id, v as number)}
                    />
                  </td>
                  <td style={{ padding: '4px 8px' }}>
                    <Switch
                      size="small"
                      checked={s.visible}
                      onChange={(v) => setVisible(s.id, v)}
                    />
                  </td>
                  <td style={{ padding: '4px 8px' }}>
                    <Button
                      size="small"
                      danger
                      onClick={async () => {
                        if (!s.kind.startsWith('imported_')) {
                          try { await cadClient.deleteShape(s.id); }
                          catch { /* shape may not exist in arena (e.g., boolean result) */ }
                        }
                        removeShape(s.id);
                      }}
                    >
                      ×
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <hr style={{ borderColor: '#303050', margin: '16px 0' }} />
      <h4 style={{ color: '#ccd', margin: '4px 0' }}>Section plane</h4>
      <Space direction="vertical" style={{ width: '100%' }}>
        <Space>
          <span style={{ fontSize: 11, color: '#889' }}>Enabled</span>
          <Switch
            size="small"
            checked={useCadStore.getState().section.enabled}
            onChange={(v) => useCadStore.getState().setSection({ enabled: v })}
          />
          <span style={{ fontSize: 11, color: '#889' }}>Axis</span>
          {(['x', 'y', 'z'] as const).map((ax) => (
            <Button
              key={ax}
              size="small"
              onClick={() => {
                const n: [number, number, number] = ax === 'x' ? [1, 0, 0] : ax === 'y' ? [0, 1, 0] : [0, 0, 1];
                useCadStore.getState().setSection({ normal: n });
              }}
            >
              {ax.toUpperCase()}
            </Button>
          ))}
        </Space>
        <div>
          <div style={{ fontSize: 11, color: '#889' }}>Offset (along normal)</div>
          <Slider
            min={-5}
            max={5}
            step={0.05}
            value={useCadStore((s) => s.section.offset)}
            onChange={(v) => useCadStore.getState().setSection({ offset: v as number })}
          />
        </div>
        <Button
          size="small"
          onClick={() => {
            shapes.forEach((s) => setVisible(s.id, true));
          }}
        >
          Show all
        </Button>
        <Button
          size="small"
          onClick={() => {
            shapes.forEach((s) => setVisible(s.id, false));
          }}
        >
          Hide all
        </Button>
        <Button
          size="small"
          onClick={async () => {
            // Re-tessellate every shape with adaptive chord tolerance 0.005.
            let count = 0;
            try {
              for (const s of shapes) {
                if (s.kind === 'imported_stl' || s.kind.startsWith('boolean_')) continue;
                const mesh = await cadClient.tessellateAdaptive(s.id, 0.005);
                useCadStore.getState().updateBuffers(
                  s.id,
                  new Float32Array(mesh.positions),
                  new Float32Array(mesh.normals),
                  new Uint32Array(mesh.indices),
                );
                count += mesh.triangle_count;
              }
              message.success(`Adaptive retessellate: ${count} triangles across ${shapes.length} shape(s)`);
            } catch (e) {
              message.error(`Retessellate failed: ${(e as Error).message}`);
            }
          }}
        >
          High-quality retessellate
        </Button>
      </Space>

      <hr style={{ borderColor: '#303050', margin: '16px 0' }} />
      <h4 style={{ color: '#ccd', margin: '4px 0' }}>Download last shape (browser Blob)</h4>
      <Space wrap>
        {(
          [
            ['.stl',  'stl'],
            ['.obj',  'obj'],
            ['.ply',  'ply'],
            ['.stp',  'step'],
            ['.brep', 'brep'],
            ['.vtk',  'vtk'],
            ['.wrl',  'wrl'],
            ['.dxf',  'dxf'],
            ['.xyz',  'xyz'],
          ] as const
        ).map(([ext, kind]) => (
          <Button
            key={`dl-${kind}`}
            size="small"
            disabled={shapes.length === 0}
            onClick={async () => {
              const s = shapes[shapes.length - 1];
              try {
                const r =
                  kind === 'stl'  ? await cadClient.exportStlString(s.id)  :
                  kind === 'obj'  ? await cadClient.exportObjString(s.id)  :
                  kind === 'ply'  ? await cadClient.exportPlyString(s.id)  :
                  kind === 'step' ? await cadClient.exportStepString(s.id) :
                  kind === 'brep' ? await cadClient.exportBrepString(s.id) :
                  kind === 'vtk'  ? await cadClient.exportVtkString(s.id)  :
                  kind === 'wrl'  ? await cadClient.exportWrlString(s.id)  :
                  kind === 'dxf'  ? await cadClient.exportDxfString(s.id)  :
                                    await cadClient.exportXyzString(s.id);
                const blob = new Blob([r.content], { type: 'text/plain' });
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = `${s.id}${ext}`;
                a.click();
                URL.revokeObjectURL(url);
                message.success(`Downloaded ${s.id}${ext} (${r.length} bytes)`);
              } catch (e) {
                message.error(`Download failed: ${(e as Error).message}`);
              }
            }}
          >
            Download {ext}
          </Button>
        ))}
      </Space>

      <hr style={{ borderColor: '#303050', margin: '16px 0' }} />
      <h4 style={{ color: '#ccd', margin: '4px 0' }}>Export last shape (disk path)</h4>
      <Space wrap>
        {(
          [
            ['STL ASCII',   'stl_ascii'],
            ['STL binary',  'stl_binary'],
            ['OBJ',         'obj'],
            ['OFF',         'off'],
            ['PLY',         'ply'],
            ['WRL',         'wrl'],
            ['STEP',        'step'],
            ['BRep-JSON',   'brep'],
            ['XYZ',         'xyz'],
            ['VTK',         'vtk'],
            ['DXF',         'dxf'],
          ] as const
        ).map(([label, kind]) => (
          <Button
            key={kind}
            size="small"
            disabled={shapes.length === 0}
            onClick={async () => {
              const s = shapes[shapes.length - 1];
              const path = window.prompt(`Save ${label} — path?`, `${s.id}.${kind.split('_')[0]}`);
              if (!path) return;
              try {
                let res: { path: string; [k: string]: unknown };
                switch (kind) {
                  case 'stl_ascii':  res = await cadClient.exportStl(s.id, path, false); break;
                  case 'stl_binary': res = await cadClient.exportStl(s.id, path, true); break;
                  case 'obj':        res = await cadClient.exportObj(s.id, path); break;
                  case 'off':        res = await cadClient.exportOff(s.id, path); break;
                  case 'ply':        res = await cadClient.exportPly(s.id, path); break;
                  case 'wrl':        res = await cadClient.exportWrl(s.id, path); break;
                  case 'step':       res = await cadClient.exportStep(s.id, path); break;
                  case 'brep':       res = await cadClient.exportBrep(path, s.id); break;
                  case 'xyz':        res = await cadClient.exportXyz(s.id, path); break;
                  case 'vtk':        res = await cadClient.exportVtk(s.id, path); break;
                  case 'dxf':        res = await cadClient.exportDxf(s.id, path); break;
                }
                message.success(`Exported ${s.id} → ${res.path}`);
              } catch (e) {
                message.error(`${label} export failed: ${(e as Error).message}`);
              }
            }}
          >
            {label}
          </Button>
        ))}
      </Space>
    </div>
  );
};

export default DisplayTabV2;
