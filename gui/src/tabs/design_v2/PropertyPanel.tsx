import React, { useMemo, useState } from 'react';
import { Button, InputNumber, Select, Space, message } from 'antd';
import cadClient from '../../ipc/cadClient';
import { useCadStore } from '../../store/cadStore';

/**
 * Property panel — pick a shape, edit its creation parameters, and re-run
 * the feature. Replaces the shape's buffers in place while keeping the
 * same GUI id so the FeatureTree row doesn't jump around.
 *
 * Iter 19 support: box, sphere, cylinder, cone, torus, chamfer_box,
 * fillet_box. Pad / Revolve / Pocket require a polygon array so they're
 * disabled here (still recreate-only through the main buttons).
 */

const PARAM_LAYOUT: Record<string, { label: string; defaults: Record<string, number> }> = {
  box:         { label: 'Box',         defaults: { lx: 1, ly: 1, lz: 1 } },
  sphere:      { label: 'Sphere',      defaults: { radius: 0.5 } },
  cylinder:    { label: 'Cylinder',    defaults: { radius: 0.3, height: 1 } },
  cone:        { label: 'Cone',        defaults: { r1: 0.4, r2: 0.0, height: 1 } },
  torus:       { label: 'Torus',       defaults: { major: 0.5, minor: 0.15 } },
  chamfer_box: { label: 'Chamfered box', defaults: { lx: 1, ly: 1, lz: 1, distance: 0.25 } },
  fillet_box:  { label: 'Filleted box',  defaults: { lx: 1, ly: 1, lz: 1, radius: 0.25 } },
};

const PropertyPanel: React.FC = () => {
  const shapes = useCadStore((s) => s.shapes);
  const updateBuffers = useCadStore((s) => s.updateBuffers);
  const editableShapes = useMemo(
    () => shapes.filter((s) => s.kind in PARAM_LAYOUT),
    [shapes],
  );
  const [selId, setSelId] = useState<string | null>(editableShapes[0]?.id ?? null);
  const [busy, setBusy] = useState(false);

  if (editableShapes.length === 0) {
    return (
      <div style={{ padding: 12, color: '#667', fontSize: 12 }}>
        No editable shapes. Create a box / sphere / cylinder / cone / torus /
        chamfered box / filleted box and they'll appear here.
      </div>
    );
  }

  const sel = editableShapes.find((s) => s.id === selId) ?? editableShapes[0];
  const layout = PARAM_LAYOUT[sel.kind];
  const currentParams = sel.params ?? layout.defaults;

  const [pendingParams, setPendingParams] = useState<Record<string, number>>(currentParams);
  // Reset pending params when selection changes.
  React.useEffect(() => {
    setPendingParams(sel.params ?? PARAM_LAYOUT[sel.kind].defaults);
  }, [sel.id, sel.kind, sel.params]);

  const apply = async () => {
    if (busy) return;
    setBusy(true);
    try {
      let result: { shape_id: string };
      switch (sel.kind) {
        case 'box':
          result = await cadClient.primitive('box', pendingParams); break;
        case 'sphere':
          result = await cadClient.primitive('sphere', pendingParams); break;
        case 'cylinder':
          result = await cadClient.primitive('cylinder', pendingParams); break;
        case 'cone':
          result = await cadClient.primitive('cone', pendingParams); break;
        case 'torus':
          result = await cadClient.primitive('torus', pendingParams); break;
        case 'chamfer_box':
          result = await cadClient.chamferBox(
            pendingParams.lx, pendingParams.ly, pendingParams.lz, pendingParams.distance,
          ); break;
        case 'fillet_box':
          result = await cadClient.filletBox(
            pendingParams.lx, pendingParams.ly, pendingParams.lz, pendingParams.radius,
          ); break;
        default:
          throw new Error(`unsupported kind: ${sel.kind}`);
      }
      const tess = await cadClient.tessellate(result.shape_id, 16, 8);
      updateBuffers(
        sel.id,
        new Float32Array(tess.positions),
        new Float32Array(tess.normals),
        new Uint32Array(tess.indices),
      );
      // Record new params + new backing shape_id so subsequent tessellations
      // reach the fresh arena entry.
      useCadStore.setState((state) => ({
        shapes: state.shapes.map((s) =>
          s.id === sel.id ? { ...s, params: pendingParams } : s,
        ),
      }));
      message.success(`${sel.id}: re-executed with ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`Apply failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div style={{ padding: 12, color: '#ddd' }}>
      <Space direction="vertical" style={{ width: '100%' }}>
        <Space>
          <span style={{ fontSize: 11, color: '#889' }}>Shape:</span>
          <Select
            size="small"
            style={{ width: 180 }}
            value={sel.id}
            onChange={(v) => setSelId(v)}
            options={editableShapes.map((s) => ({
              value: s.id,
              label: `${s.id} (${PARAM_LAYOUT[s.kind].label})`,
            }))}
          />
        </Space>

        <div style={{
          display: 'grid',
          gridTemplateColumns: '80px 1fr',
          gap: '4px 8px',
          alignItems: 'center',
          background: '#14142a',
          padding: 8,
          borderRadius: 4,
        }}>
          {Object.keys(layout.defaults).map((key) => (
            <React.Fragment key={key}>
              <span style={{ fontSize: 11, color: '#889' }}>{key}</span>
              <InputNumber
                size="small"
                step={0.05}
                value={pendingParams[key] ?? layout.defaults[key]}
                onChange={(v) => setPendingParams({ ...pendingParams, [key]: (v as number) ?? 0 })}
                style={{ width: 100 }}
              />
            </React.Fragment>
          ))}
        </div>

        <Button type="primary" size="small" onClick={apply} disabled={busy}>
          Apply & re-execute
        </Button>
      </Space>
    </div>
  );
};

export default PropertyPanel;
