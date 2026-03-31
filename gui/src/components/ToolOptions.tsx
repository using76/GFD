import React, { useState } from 'react';
import { Checkbox, Radio, InputNumber, Form, Slider, Button, message, Space } from 'antd';
import { useAppStore } from '../store/useAppStore';

// ============================================================
// Select Tool Options
// ============================================================
const SelectOptions: React.FC = () => {
  const selectionFilter = useAppStore((s) => s.selectionFilter);
  const setSelectionFilter = useAppStore((s) => s.setSelectionFilter);
  const annotations = useAppStore((s) => s.annotations);
  const probePoints = useAppStore((s) => s.probePoints);
  const removeAnnotation = useAppStore((s) => s.removeAnnotation);
  const removeProbePoint = useAppStore((s) => s.removeProbePoint);
  const clearAnnotations = useAppStore((s) => s.clearAnnotations);
  const clearProbePoints = useAppStore((s) => s.clearProbePoints);

  return (
    <div style={{ padding: 10, fontSize: 12 }}>
      <div style={{ color: '#889', fontSize: 11, marginBottom: 6, fontWeight: 500 }}>Selection Filter</div>
      <Radio.Group
        value={selectionFilter}
        onChange={(e) => setSelectionFilter(e.target.value)}
        size="small"
        style={{ display: 'flex', flexDirection: 'column', gap: 4 }}
      >
        <Radio value="face" style={{ fontSize: 12 }}>Face</Radio>
        <Radio value="edge" style={{ fontSize: 12 }}>Edge</Radio>
        <Radio value="vertex" style={{ fontSize: 12 }}>Vertex</Radio>
        <Radio value="body" style={{ fontSize: 12 }}>Body</Radio>
        <Radio value="component" style={{ fontSize: 12 }}>Component</Radio>
      </Radio.Group>

      {/* Annotations list */}
      {annotations.length > 0 && (
        <div style={{ marginTop: 10 }}>
          <div style={{ color: '#889', fontSize: 11, marginBottom: 4, fontWeight: 500, display: 'flex', justifyContent: 'space-between' }}>
            <span>Notes ({annotations.length})</span>
            <span onClick={clearAnnotations} style={{ color: '#4096ff', cursor: 'pointer' }}>Clear</span>
          </div>
          {annotations.map(a => (
            <div key={a.id} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '2px 0', borderBottom: '1px solid #252540' }}>
              <span style={{ fontSize: 11, color: '#aab' }}>{a.text}</span>
              <span onClick={() => removeAnnotation(a.id)} style={{ color: '#ff4d4f', cursor: 'pointer', fontSize: 10 }}>x</span>
            </div>
          ))}
        </div>
      )}

      {/* Probe points list */}
      {probePoints.length > 0 && (
        <div style={{ marginTop: 10 }}>
          <div style={{ color: '#889', fontSize: 11, marginBottom: 4, fontWeight: 500, display: 'flex', justifyContent: 'space-between' }}>
            <span>Probes ({probePoints.length})</span>
            <span onClick={clearProbePoints} style={{ color: '#4096ff', cursor: 'pointer' }}>Clear</span>
          </div>
          {probePoints.map(p => (
            <div key={p.id} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '2px 0', borderBottom: '1px solid #252540' }}>
              <span style={{ fontSize: 10, color: '#aab' }}>({p.position.map(v => v.toFixed(1)).join(', ')})</span>
              <span onClick={() => removeProbePoint(p.id)} style={{ color: '#ff4d4f', cursor: 'pointer', fontSize: 10 }}>x</span>
            </div>
          ))}
        </div>
      )}

      {/* Alignment tools for multi-select */}
      {useAppStore.getState().selectedShapeIds.length > 1 && (
        <div style={{ marginTop: 10 }}>
          <div style={{ color: '#889', fontSize: 11, marginBottom: 4, fontWeight: 500 }}>
            Align ({useAppStore.getState().selectedShapeIds.length} shapes)
          </div>
          <div style={{ fontSize: 10, color: '#667', marginBottom: 2 }}>Align center:</div>
          <Space size={4}>
            <Button size="small" onClick={() => { useAppStore.getState().alignShapes('x'); message.success('Aligned X'); }}>X</Button>
            <Button size="small" onClick={() => { useAppStore.getState().alignShapes('y'); message.success('Aligned Y'); }}>Y</Button>
            <Button size="small" onClick={() => { useAppStore.getState().alignShapes('z'); message.success('Aligned Z'); }}>Z</Button>
          </Space>
          {useAppStore.getState().selectedShapeIds.length >= 3 && (
            <>
              <div style={{ fontSize: 10, color: '#667', marginTop: 6, marginBottom: 2 }}>Distribute evenly:</div>
              <Space size={4}>
                <Button size="small" onClick={() => { useAppStore.getState().distributeShapes('x'); message.success('Distributed X'); }}>X</Button>
                <Button size="small" onClick={() => { useAppStore.getState().distributeShapes('y'); message.success('Distributed Y'); }}>Y</Button>
                <Button size="small" onClick={() => { useAppStore.getState().distributeShapes('z'); message.success('Distributed Z'); }}>Z</Button>
              </Space>
            </>
          )}
        </div>
      )}
    </div>
  );
};

// ============================================================
// Pull Tool Options
// ============================================================
const PullOptions: React.FC = () => {
  const [mode, setMode] = useState<'add' | 'cut' | 'nomerge'>('add');
  const [distance, setDistance] = useState(1.0);
  const [upToSurface, setUpToSurface] = useState(false);
  const [symmetric, setSymmetric] = useState(false);
  const [draftAngle, setDraftAngle] = useState(0);
  const [directionLock, setDirectionLock] = useState<'none' | 'x' | 'y' | 'z'>('none');

  const applyPull = () => {
    const state = useAppStore.getState();
    const sid = state.selectedShapeId;
    if (!sid) { message.warning('Select a shape first'); return; }
    const shape = state.shapes.find(s => s.id === sid);
    if (!shape) return;
    const d = { ...shape.dimensions };
    const dir = directionLock === 'none' ? 'y' : directionLock;
    const amount = mode === 'cut' ? -distance : distance;
    const symMult = symmetric ? 2 : 1;
    // Modify the dimension in the pull direction
    if (dir === 'x') d.width = Math.max(0.01, (d.width ?? 1) + amount * symMult);
    else if (dir === 'y') d.height = Math.max(0.01, (d.height ?? 1) + amount * symMult);
    else if (dir === 'z') d.depth = Math.max(0.01, (d.depth ?? 1) + amount * symMult);
    state.updateShape(sid, { dimensions: d });
    message.success(`Pull ${mode}: ${dir} += ${amount.toFixed(2)}m`);
  };

  return (
    <div style={{ padding: 10, fontSize: 12 }}>
      <div style={{ color: '#889', fontSize: 11, marginBottom: 6, fontWeight: 500 }}>Pull Mode</div>
      <Radio.Group
        value={mode}
        onChange={(e) => setMode(e.target.value)}
        size="small"
        style={{ display: 'flex', flexDirection: 'column', gap: 4, marginBottom: 10 }}
      >
        <Radio value="add" style={{ fontSize: 12 }}>Add</Radio>
        <Radio value="cut" style={{ fontSize: 12 }}>Cut</Radio>
        <Radio value="nomerge" style={{ fontSize: 12 }}>No Merge</Radio>
      </Radio.Group>

      <Form layout="vertical" size="small" style={{ marginBottom: 8 }}>
        <Form.Item label="Distance" style={{ marginBottom: 6 }}>
          <InputNumber
            value={distance}
            min={0}
            max={100}
            step={0.1}
            onChange={(v) => setDistance(v ?? 1.0)}
            style={{ width: '100%' }}
            addonAfter="m"
            size="small"
          />
        </Form.Item>
      </Form>

      <div style={{ color: '#889', fontSize: 11, marginBottom: 4, fontWeight: 500 }}>Direction Lock</div>
      <Radio.Group
        value={directionLock}
        onChange={(e) => setDirectionLock(e.target.value)}
        size="small"
        style={{ display: 'flex', gap: 8, marginBottom: 8 }}
      >
        <Radio value="none" style={{ fontSize: 11 }}>Free</Radio>
        <Radio value="x" style={{ fontSize: 11, color: '#ff4444' }}>X</Radio>
        <Radio value="y" style={{ fontSize: 11, color: '#44ff44' }}>Y</Radio>
        <Radio value="z" style={{ fontSize: 11, color: '#4444ff' }}>Z</Radio>
      </Radio.Group>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <Checkbox checked={upToSurface} onChange={(e) => setUpToSurface(e.target.checked)} style={{ fontSize: 12 }}>
          Up to surface
        </Checkbox>
        <Checkbox checked={symmetric} onChange={(e) => setSymmetric(e.target.checked)} style={{ fontSize: 12 }}>
          Symmetric
        </Checkbox>
      </div>

      <Form layout="vertical" size="small" style={{ marginTop: 8 }}>
        <Form.Item label="Draft angle" style={{ marginBottom: 6 }}>
          <InputNumber
            value={draftAngle}
            min={-45}
            max={45}
            step={1}
            onChange={(v) => setDraftAngle(v ?? 0)}
            style={{ width: '100%' }}
            addonAfter="deg"
            size="small"
          />
        </Form.Item>
      </Form>
      <Button type="primary" size="small" block onClick={applyPull} style={{ marginTop: 8 }}>
        Apply Pull
      </Button>
    </div>
  );
};

// ============================================================
// Move Tool Options
// ============================================================
const MoveOptions: React.FC = () => {
  const [copy, setCopy] = useState(false);
  const [snapToGrid, setSnapToGrid] = useState(true);
  const [moveDistance, setMoveDistance] = useState(1.0);
  const [moveAngle, setMoveAngle] = useState(0);
  const [moveAxis, setMoveAxis] = useState<'x' | 'y' | 'z'>('x');

  const applyMove = () => {
    const state = useAppStore.getState();
    const sid = state.selectedShapeId;
    if (!sid) { message.warning('Select a shape first'); return; }
    const shape = state.shapes.find(s => s.id === sid);
    if (!shape) return;
    const pos: [number, number, number] = [...shape.position];
    const rot: [number, number, number] = [...shape.rotation];
    const d = snapToGrid ? Math.round(moveDistance * 10) / 10 : moveDistance;
    if (moveAxis === 'x') pos[0] += d;
    else if (moveAxis === 'y') pos[1] += d;
    else pos[2] += d;
    if (moveAngle !== 0) {
      if (moveAxis === 'x') rot[0] += moveAngle;
      else if (moveAxis === 'y') rot[1] += moveAngle;
      else rot[2] += moveAngle;
    }
    if (copy) {
      const newId = `shape-mv-${Date.now()}`;
      state.addShape({ ...shape, id: newId, name: `${shape.name}-copy`, position: pos, rotation: rot });
      state.selectShape(newId);
      message.success(`Copy+Move: ${moveAxis} += ${d}m`);
    } else {
      state.updateShape(sid, { position: pos, rotation: rot });
      message.success(`Move: ${moveAxis} += ${d}m`);
    }
  };

  return (
    <div style={{ padding: 10, fontSize: 12 }}>
      <div style={{ color: '#889', fontSize: 11, marginBottom: 6, fontWeight: 500 }}>Move Options</div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6, marginBottom: 10 }}>
        <Checkbox checked={copy} onChange={(e) => setCopy(e.target.checked)} style={{ fontSize: 12 }}>
          Copy
        </Checkbox>
        <Checkbox checked={snapToGrid} onChange={(e) => setSnapToGrid(e.target.checked)} style={{ fontSize: 12 }}>
          Snap to grid
        </Checkbox>
      </div>

      <div style={{ color: '#889', fontSize: 11, marginBottom: 4, fontWeight: 500 }}>Axis</div>
      <Radio.Group value={moveAxis} onChange={(e) => setMoveAxis(e.target.value)} size="small" style={{ display: 'flex', gap: 8, marginBottom: 8 }}>
        <Radio value="x" style={{ fontSize: 11, color: '#ff4444' }}>X</Radio>
        <Radio value="y" style={{ fontSize: 11, color: '#44ff44' }}>Y</Radio>
        <Radio value="z" style={{ fontSize: 11, color: '#4444ff' }}>Z</Radio>
      </Radio.Group>

      <Form layout="vertical" size="small">
        <Form.Item label="Distance" style={{ marginBottom: 6 }}>
          <InputNumber
            value={moveDistance}
            min={-100}
            max={100}
            step={0.1}
            onChange={(v) => setMoveDistance(v ?? 1.0)}
            style={{ width: '100%' }}
            addonAfter="m"
            size="small"
          />
        </Form.Item>
        <Form.Item label="Rotation angle" style={{ marginBottom: 6 }}>
          <InputNumber
            value={moveAngle}
            min={-360}
            max={360}
            step={15}
            onChange={(v) => setMoveAngle(v ?? 0)}
            style={{ width: '100%' }}
            addonAfter="deg"
            size="small"
          />
        </Form.Item>
      </Form>
      <Button type="primary" size="small" block onClick={applyMove} style={{ marginTop: 4 }}>
        Apply Move
      </Button>
    </div>
  );
};

// ============================================================
// Fill Tool Options
// ============================================================
const FillOptions: React.FC = () => {
  const [fillMode, setFillMode] = useState<'auto' | 'manual'>('auto');
  const [detectBoundary, setDetectBoundary] = useState(true);

  const applyFill = () => {
    const state = useAppStore.getState();
    // Fix all repair issues (gaps/missing faces) as a "fill" operation
    const unfixed = state.repairIssues.filter(i => !i.fixed && (i.kind === 'gap' || i.kind === 'missing_face'));
    if (unfixed.length > 0) {
      state.fixAllRepairIssues();
      message.success(`Auto-filled ${unfixed.length} gap(s)/missing face(s)`);
    } else {
      message.info('No gaps or missing faces to fill. Run Repair > Check first.');
    }
  };

  return (
    <div style={{ padding: 10, fontSize: 12 }}>
      <div style={{ color: '#889', fontSize: 11, marginBottom: 6, fontWeight: 500 }}>Fill Mode</div>
      <Radio.Group
        value={fillMode}
        onChange={(e) => setFillMode(e.target.value)}
        size="small"
        style={{ display: 'flex', flexDirection: 'column', gap: 4, marginBottom: 10 }}
      >
        <Radio value="auto" style={{ fontSize: 12 }}>Auto</Radio>
        <Radio value="manual" style={{ fontSize: 12 }}>Manual</Radio>
      </Radio.Group>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <Checkbox checked={detectBoundary} onChange={(e) => setDetectBoundary(e.target.checked)} style={{ fontSize: 12 }}>
          Boundary detection
        </Checkbox>
      </div>

      {fillMode === 'auto' && (
        <div style={{ marginTop: 8, padding: 6, background: '#1a1a30', borderRadius: 4, color: '#667', fontSize: 11 }}>
          Auto mode: fixes gaps and missing faces detected by Repair Check.
        </div>
      )}
      {fillMode === 'manual' && (
        <div style={{ marginTop: 8, padding: 6, background: '#1a1a30', borderRadius: 4, color: '#667', fontSize: 11 }}>
          Manual mode: select edges to define the fill boundary, then confirm.
        </div>
      )}
      <Button type="primary" size="small" block onClick={applyFill} style={{ marginTop: 8 }}>
        Apply Fill
      </Button>
    </div>
  );
};

// ============================================================
// Measure Tool Options
// ============================================================
const MeasureOptions: React.FC = () => {
  const measureMode = useAppStore((s) => s.measureMode);
  const setMeasureMode = useAppStore((s) => s.setMeasureMode);
  const measureLabels = useAppStore((s) => s.measureLabels);
  const clearMeasureLabels = useAppStore((s) => s.clearMeasureLabels);

  return (
    <div style={{ padding: 10, fontSize: 12 }}>
      <div style={{ color: '#889', fontSize: 11, marginBottom: 6, fontWeight: 500 }}>Measure Mode</div>
      <Radio.Group
        value={measureMode ?? 'distance'}
        onChange={(e) => setMeasureMode(e.target.value)}
        size="small"
        style={{ display: 'flex', flexDirection: 'column', gap: 4 }}
      >
        <Radio value="distance" style={{ fontSize: 12 }}>Distance</Radio>
        <Radio value="angle" style={{ fontSize: 12 }}>Angle</Radio>
        <Radio value="area" style={{ fontSize: 12 }}>Area</Radio>
      </Radio.Group>

      {measureLabels.length > 0 && (
        <div style={{ marginTop: 10 }}>
          <div style={{ color: '#889', fontSize: 11, marginBottom: 4, fontWeight: 500 }}>
            Results ({measureLabels.length})
          </div>
          <div style={{ maxHeight: 120, overflow: 'auto' }}>
            {measureLabels.map((label) => (
              <div key={label.id} style={{ padding: '2px 4px', color: '#aab', fontSize: 11, borderBottom: '1px solid #252540' }}>
                {label.text}
              </div>
            ))}
          </div>
          <div
            onClick={clearMeasureLabels}
            style={{ marginTop: 4, color: '#4096ff', fontSize: 11, cursor: 'pointer' }}
          >
            Clear all
          </div>
        </div>
      )}
    </div>
  );
};

// ============================================================
// Section Tool Options
// ============================================================
const SectionOptions: React.FC = () => {
  const sectionPlane = useAppStore((s) => s.sectionPlane);
  const setSectionPlane = useAppStore((s) => s.setSectionPlane);

  const setNormal = (planeAxis: 'xy' | 'xz' | 'yz') => {
    const normals: Record<string, [number, number, number]> = {
      xy: [0, 0, 1],
      xz: [0, 1, 0],
      yz: [1, 0, 0],
    };
    const axisMap: Record<string, 'x' | 'y' | 'z'> = {
      xy: 'z',
      xz: 'y',
      yz: 'x',
    };
    setSectionPlane({ normal: normals[planeAxis], axis: axisMap[planeAxis], enabled: true });
  };

  return (
    <div style={{ padding: 10, fontSize: 12 }}>
      <div style={{ color: '#889', fontSize: 11, marginBottom: 6, fontWeight: 500 }}>Section Plane</div>
      <Checkbox
        checked={sectionPlane.enabled}
        onChange={(e) => setSectionPlane({ enabled: e.target.checked })}
        style={{ fontSize: 12, marginBottom: 8 }}
      >
        Enable section view
      </Checkbox>

      <Radio.Group
        value={
          sectionPlane.normal[2] === 1 ? 'xy' :
          sectionPlane.normal[0] === 1 ? 'yz' : 'xz'
        }
        onChange={(e) => setNormal(e.target.value)}
        size="small"
        style={{ display: 'flex', flexDirection: 'column', gap: 4, marginBottom: 8 }}
      >
        <Radio value="xy" style={{ fontSize: 12 }}>XY Plane</Radio>
        <Radio value="xz" style={{ fontSize: 12 }}>XZ Plane</Radio>
        <Radio value="yz" style={{ fontSize: 12 }}>YZ Plane</Radio>
      </Radio.Group>

      <Form layout="vertical" size="small">
        <Form.Item label="Offset" style={{ marginBottom: 0 }}>
          <Slider
            min={-5}
            max={5}
            step={0.1}
            value={sectionPlane.offset}
            onChange={(v) => setSectionPlane({ offset: v })}
          />
        </Form.Item>
      </Form>
    </div>
  );
};

// ============================================================
// No Tool Selected
// ============================================================
const NoToolOptions: React.FC = () => (
  <div style={{ padding: 12, color: '#556', fontSize: 11, textAlign: 'center' }}>
    Select a tool from the ribbon to see its options.
  </div>
);

// ============================================================
// Main Component
// ============================================================
const toolPanels: Record<string, React.FC> = {
  select: SelectOptions,
  pull: PullOptions,
  move: MoveOptions,
  fill: FillOptions,
  measure: MeasureOptions,
  section: SectionOptions,
  none: NoToolOptions,
};

const ToolOptions: React.FC = () => {
  const activeTool = useAppStore((s) => s.activeTool);
  const Panel = toolPanels[activeTool] || NoToolOptions;
  return <Panel />;
};

export default ToolOptions;
