import React, { useState } from 'react';
import { Button, Space, message, Divider } from 'antd';
import cadClient from '../../ipc/cadClient';
import { useCadStore } from '../../store/cadStore';

type Row = {
  shape_id: string;
  kind: string;
  area: number;
  bboxV: number;
  volume: number;
  com: [number, number, number] | null;
  inertia: [number, number, number] | null;
  bsphereR: number | null;
  edgeRange: [number, number] | null;
  valid: boolean;
  issues: number;
};

/**
 * Measure v2 — exact analytical queries (area / volume / validity) backed by
 * the pure-Rust gfd-cad kernel via JSON-RPC. Iteration 10 covers:
 * - surface_area (Newell + analytic closed surfaces)
 * - bbox_volume (axis-aligned wrap)
 * - heal.check_validity issue count
 */
const MeasureTabV2: React.FC = () => {
  const shapes = useCadStore((s) => s.shapes);
  const [rows, setRows] = useState<Row[]>([]);
  const [busy, setBusy] = useState(false);

  const measureAll = async () => {
    if (busy) return;
    setBusy(true);
    try {
      // Single batched RPC (iter 169). Imported meshes skip the B-Rep path.
      const brepIds = shapes.filter((s) => !s.kind.startsWith('imported_')).map((s) => s.id);
      const batch = brepIds.length > 0
        ? await cadClient.multiShapeSummary(brepIds).catch(() => ({ summaries: [], count: 0 }))
        : { summaries: [], count: 0 };
      const byId = new Map<string, typeof batch.summaries[number]>();
      for (const r of batch.summaries) {
        if (r.shape_id) byId.set(r.shape_id, r);
      }
      const out: Row[] = shapes.map((s) => {
        const r = byId.get(s.id);
        if (!r || r.error) {
          return {
            shape_id: s.id, kind: s.kind,
            area: NaN, bboxV: NaN, volume: NaN,
            com: null, inertia: null, bsphereR: null, edgeRange: null,
            valid: false, issues: 0,
          };
        }
        return {
          shape_id: s.id,
          kind: s.kind,
          area: r.surface_area ?? NaN,
          bboxV: r.bbox_volume ?? NaN,
          volume: r.divergence_volume ?? NaN,
          com: r.center_of_mass ?? null,
          inertia: r.inertia_tensor ? [r.inertia_tensor[0], r.inertia_tensor[1], r.inertia_tensor[2]] : null,
          bsphereR: r.bounding_sphere ? r.bounding_sphere.radius : null,
          edgeRange: r.edge_length_range ?? null,
          valid: r.valid ?? false,
          issues: r.issues ?? 0,
        };
      });
      setRows(out);
      message.success(`Measured ${out.length} shape(s)`);
    } catch (e) {
      message.error(`Measure failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div style={{ padding: 12, color: '#ddd' }}>
      <h4 style={{ color: '#4096ff', marginTop: 0 }}>Measure v2 — B-Rep queries</h4>
      <Space>
        <Button type="primary" disabled={busy || shapes.length === 0} onClick={measureAll}>
          Measure all shapes
        </Button>
        <span style={{ color: '#889', fontSize: 11 }}>
          {shapes.length} shape(s) tracked
        </span>
      </Space>

      <Divider style={{ borderColor: '#303050', margin: '12px 0' }} />

      {rows.length === 0 ? (
        <div style={{ color: '#667', fontSize: 12 }}>
          No measurements yet. Click "Measure all shapes".
        </div>
      ) : (
        <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse' }}>
          <thead>
            <tr style={{ color: '#889', textAlign: 'left' }}>
              <th style={{ padding: '4px 8px' }}>Shape</th>
              <th style={{ padding: '4px 8px' }}>Kind</th>
              <th style={{ padding: '4px 8px' }}>Area</th>
              <th style={{ padding: '4px 8px' }}>Volume</th>
              <th style={{ padding: '4px 8px' }}>bbox-V</th>
              <th style={{ padding: '4px 8px' }}>CoM (x,y,z)</th>
              <th style={{ padding: '4px 8px' }}>I (xx,yy,zz)</th>
              <th style={{ padding: '4px 8px' }}>bsphere R</th>
              <th style={{ padding: '4px 8px' }}>edge ℓ min/max</th>
              <th style={{ padding: '4px 8px' }}>Validity</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.shape_id} style={{ borderTop: '1px solid #303050' }}>
                <td style={{ padding: '4px 8px', color: '#aab' }}>{r.shape_id}</td>
                <td style={{ padding: '4px 8px' }}>{r.kind}</td>
                <td style={{ padding: '4px 8px', fontFamily: 'monospace' }}>
                  {Number.isFinite(r.area) ? r.area.toFixed(4) : '—'}
                </td>
                <td style={{ padding: '4px 8px', fontFamily: 'monospace' }}>
                  {Number.isFinite(r.volume) ? r.volume.toFixed(4) : '—'}
                </td>
                <td style={{ padding: '4px 8px', fontFamily: 'monospace' }}>
                  {Number.isFinite(r.bboxV) ? r.bboxV.toFixed(4) : '—'}
                </td>
                <td style={{ padding: '4px 8px', fontFamily: 'monospace', fontSize: 10 }}>
                  {r.com ? `(${r.com[0].toFixed(2)}, ${r.com[1].toFixed(2)}, ${r.com[2].toFixed(2)})` : '—'}
                </td>
                <td style={{ padding: '4px 8px', fontFamily: 'monospace', fontSize: 10 }}>
                  {r.inertia ? `(${r.inertia[0].toFixed(3)}, ${r.inertia[1].toFixed(3)}, ${r.inertia[2].toFixed(3)})` : '—'}
                </td>
                <td style={{ padding: '4px 8px', fontFamily: 'monospace' }}>
                  {r.bsphereR !== null ? r.bsphereR.toFixed(3) : '—'}
                </td>
                <td style={{ padding: '4px 8px', fontFamily: 'monospace', fontSize: 10 }}>
                  {r.edgeRange ? `${r.edgeRange[0].toFixed(3)} / ${r.edgeRange[1].toFixed(3)}` : '—'}
                </td>
                <td style={{ padding: '4px 8px', color: r.valid ? '#52c41a' : '#faad14' }}>
                  {r.valid ? 'OK' : `${r.issues} issue(s)`}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
};

export default MeasureTabV2;
