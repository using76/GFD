import React, { useState } from 'react';
import { Checkbox, Radio, InputNumber, Form } from 'antd';
import { useAppStore } from '../store/useAppStore';

// ============================================================
// Select Tool Options
// ============================================================
const SelectOptions: React.FC = () => {
  const selectionFilter = useAppStore((s) => s.selectionFilter);
  const setSelectionFilter = useAppStore((s) => s.setSelectionFilter);

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
    </div>
  );
};

// ============================================================
// Pull Tool Options
// ============================================================
const PullOptions: React.FC = () => {
  const [mode, setMode] = useState<'add' | 'cut' | 'nomerge'>('add');
  const [upToSurface, setUpToSurface] = useState(false);
  const [symmetric, setSymmetric] = useState(false);
  const [draftAngle, setDraftAngle] = useState(0);

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

      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <Checkbox checked={upToSurface} onChange={(e) => setUpToSurface(e.target.checked)} style={{ fontSize: 12 }}>
          Up to surface
        </Checkbox>
        <Checkbox checked={symmetric} onChange={(e) => setSymmetric(e.target.checked)} style={{ fontSize: 12 }}>
          Symmetric
        </Checkbox>
      </div>

      <Form layout="vertical" size="small" style={{ marginTop: 8 }}>
        <Form.Item label="Draft angle" style={{ marginBottom: 0 }}>
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
    </div>
  );
};

// ============================================================
// Move Tool Options
// ============================================================
const MoveOptions: React.FC = () => {
  const [copy, setCopy] = useState(false);
  const [snapToGrid, setSnapToGrid] = useState(true);

  return (
    <div style={{ padding: 10, fontSize: 12 }}>
      <div style={{ color: '#889', fontSize: 11, marginBottom: 6, fontWeight: 500 }}>Move Options</div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <Checkbox checked={copy} onChange={(e) => setCopy(e.target.checked)} style={{ fontSize: 12 }}>
          Copy
        </Checkbox>
        <Checkbox checked={snapToGrid} onChange={(e) => setSnapToGrid(e.target.checked)} style={{ fontSize: 12 }}>
          Snap to grid
        </Checkbox>
      </div>
    </div>
  );
};

// ============================================================
// Fill Tool Options
// ============================================================
const FillOptions: React.FC = () => {
  const [fillMode, setFillMode] = useState('delete_fill');

  return (
    <div style={{ padding: 10, fontSize: 12 }}>
      <div style={{ color: '#889', fontSize: 11, marginBottom: 6, fontWeight: 500 }}>Fill Mode</div>
      <Radio.Group
        value={fillMode}
        onChange={(e) => setFillMode(e.target.value)}
        size="small"
        style={{ display: 'flex', flexDirection: 'column', gap: 4 }}
      >
        <Radio value="delete_fill" style={{ fontSize: 12 }}>Delete and Fill</Radio>
        <Radio value="delete_patch" style={{ fontSize: 12 }}>Delete and Patch</Radio>
        <Radio value="smooth" style={{ fontSize: 12 }}>Smooth</Radio>
      </Radio.Group>
    </div>
  );
};

// ============================================================
// Measure Tool Options
// ============================================================
const MeasureOptions: React.FC = () => (
  <div style={{ padding: 10, fontSize: 12 }}>
    <div style={{ color: '#889', fontSize: 11, marginBottom: 6, fontWeight: 500 }}>Measure Mode</div>
    <Radio.Group defaultValue="distance" size="small" style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
      <Radio value="distance" style={{ fontSize: 12 }}>Distance</Radio>
      <Radio value="angle" style={{ fontSize: 12 }}>Angle</Radio>
      <Radio value="area" style={{ fontSize: 12 }}>Area</Radio>
    </Radio.Group>
  </div>
);

// ============================================================
// Section Tool Options
// ============================================================
const SectionOptions: React.FC = () => (
  <div style={{ padding: 10, fontSize: 12 }}>
    <div style={{ color: '#889', fontSize: 11, marginBottom: 6, fontWeight: 500 }}>Section Plane</div>
    <Radio.Group defaultValue="xy" size="small" style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
      <Radio value="xy" style={{ fontSize: 12 }}>XY Plane</Radio>
      <Radio value="xz" style={{ fontSize: 12 }}>XZ Plane</Radio>
      <Radio value="yz" style={{ fontSize: 12 }}>YZ Plane</Radio>
    </Radio.Group>
  </div>
);

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
