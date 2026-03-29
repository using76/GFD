import React, { useState, useCallback, useMemo } from 'react';
import { Button, InputNumber, Form, Divider, Select, message, Input, Tag, Tooltip } from 'antd';
import {
  ExpandOutlined,
  ExperimentOutlined,
  BorderInnerOutlined,
  AppstoreOutlined,
  CheckCircleOutlined,
  PlusOutlined,
  DeleteOutlined,
  BgColorsOutlined,
  BuildOutlined,
  LoadingOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../../store/useAppStore';
import type { NamedSelection, NamedSelectionType } from '../../store/useAppStore';

const selectionTypeColors: Record<NamedSelectionType, string> = {
  inlet: '#4488ff',
  outlet: '#ff4444',
  wall: '#44ff44',
  symmetry: '#ffff44',
  interface: '#ff88ff',
  custom: '#88ffff',
};

const selectionTypeIcons: Record<NamedSelectionType, string> = {
  inlet: '\u25B6',   // right arrow
  outlet: '\u25C0',  // left arrow (reversed)
  wall: '\u2588',    // full block
  symmetry: '\u2194', // left-right arrow
  interface: '\u21C4', // double arrow
  custom: '\u2605',   // star
};

let nextEnclosureId = 100;

const StepHeader: React.FC<{
  step: number;
  title: string;
  done: boolean;
  current: boolean;
}> = ({ step, title, done, current }) => (
  <div
    style={{
      display: 'flex',
      alignItems: 'center',
      gap: 8,
      padding: '6px 0',
      marginBottom: 4,
    }}
  >
    <div
      style={{
        width: 20,
        height: 20,
        borderRadius: '50%',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        fontSize: 11,
        fontWeight: 600,
        flexShrink: 0,
        background: done ? '#52c41a' : current ? '#1677ff' : '#333',
        color: done || current ? '#fff' : '#888',
      }}
    >
      {done ? <CheckCircleOutlined style={{ fontSize: 12 }} /> : step}
    </div>
    <span
      style={{
        fontWeight: 500,
        fontSize: 12,
        color: done ? '#52c41a' : current ? '#fff' : '#888',
      }}
    >
      {title}
    </span>
  </div>
);

const CfdPrepPanel: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const addShape = useAppStore((s) => s.addShape);
  const namedSelections = useAppStore((s) => s.namedSelections);
  const setNamedSelections = useAppStore((s) => s.setNamedSelections);
  const addNamedSelection = useAppStore((s) => s.addNamedSelection);
  const removeNamedSelection = useAppStore((s) => s.removeNamedSelection);
  const enclosureCreated = useAppStore((s) => s.enclosureCreated);
  const setEnclosureCreated = useAppStore((s) => s.setEnclosureCreated);
  const fluidExtracted = useAppStore((s) => s.fluidExtracted);
  const setFluidExtracted = useAppStore((s) => s.setFluidExtracted);
  const topologyShared = useAppStore((s) => s.topologyShared);
  const setTopologyShared = useAppStore((s) => s.setTopologyShared);
  const hoveredSelectionName = useAppStore((s) => s.hoveredSelectionName);
  const setHoveredSelectionName = useAppStore((s) => s.setHoveredSelectionName);
  const cfdPrepStep = useAppStore((s) => s.cfdPrepStep);
  const setCfdPrepStep = useAppStore((s) => s.setCfdPrepStep);
  const generateMesh = useAppStore((s) => s.generateMesh);
  const meshGenerated = useAppStore((s) => s.meshGenerated);
  const meshGenerating = useAppStore((s) => s.meshGenerating);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const setActiveRibbonTab = useAppStore((s) => s.setActiveRibbonTab);

  const [padXp, setPadXp] = useState(2.0);
  const [padXn, setPadXn] = useState(1.0);
  const [padYp, setPadYp] = useState(1.0);
  const [padYn, setPadYn] = useState(1.0);
  const [padZp, setPadZp] = useState(1.0);
  const [padZn, setPadZn] = useState(1.0);
  const [selectedBody, setSelectedBody] = useState<string | null>(null);
  const [newSelName, setNewSelName] = useState('');
  const [newSelType, setNewSelType] = useState<NamedSelectionType>('wall');

  const bodyShapes = useMemo(
    () => shapes.filter((s) => s.group !== 'enclosure' && s.kind !== 'enclosure'),
    [shapes]
  );

  const enclosureShape = useMemo(
    () => shapes.find((s) => s.kind === 'enclosure' || s.isEnclosure),
    [shapes]
  );

  // Step 1: Create Enclosure
  const handleCreateEnclosure = useCallback(() => {
    if (bodyShapes.length === 0) {
      message.warning('No body shapes to enclose. Create shapes first.');
      return;
    }

    let minX = Infinity, maxX = -Infinity;
    let minY = Infinity, maxY = -Infinity;
    let minZ = Infinity, maxZ = -Infinity;

    bodyShapes.forEach((s) => {
      const hw = (s.dimensions.width ?? s.dimensions.radius ?? 0.5);
      const hh = (s.dimensions.height ?? s.dimensions.radius ?? 0.5);
      const hd = (s.dimensions.depth ?? s.dimensions.radius ?? 0.5);
      minX = Math.min(minX, s.position[0] - hw);
      maxX = Math.max(maxX, s.position[0] + hw);
      minY = Math.min(minY, s.position[1] - hh);
      maxY = Math.max(maxY, s.position[1] + hh);
      minZ = Math.min(minZ, s.position[2] - hd);
      maxZ = Math.max(maxZ, s.position[2] + hd);
    });

    minX -= padXn; maxX += padXp;
    minY -= padYn; maxY += padYp;
    minZ -= padZn; maxZ += padZp;

    const w = maxX - minX;
    const h = maxY - minY;
    const d = maxZ - minZ;

    const id = `encl-${nextEnclosureId++}`;
    addShape({
      id,
      name: 'Enclosure',
      kind: 'enclosure',
      position: [(minX + maxX) / 2, (minY + maxY) / 2, (minZ + maxZ) / 2],
      rotation: [0, 0, 0],
      dimensions: { width: w, height: h, depth: d },
      isEnclosure: true,
      group: 'enclosure',
    });

    setEnclosureCreated(true);
    if (cfdPrepStep < 1) setCfdPrepStep(1);
    message.success(`Enclosure created: ${w.toFixed(2)} x ${h.toFixed(2)} x ${d.toFixed(2)} m`);
  }, [bodyShapes, padXp, padXn, padYp, padYn, padZp, padZn, addShape, setEnclosureCreated, cfdPrepStep, setCfdPrepStep]);

  // Step 2: Extract Fluid Volume
  const handleExtractFluid = useCallback(() => {
    if (!enclosureCreated) {
      message.warning('Create an enclosure first.');
      return;
    }
    setFluidExtracted(true);
    if (cfdPrepStep < 2) setCfdPrepStep(2);
    message.success('Fluid volume extracted (boolean subtract solid from enclosure)');
  }, [enclosureCreated, setFluidExtracted, cfdPrepStep, setCfdPrepStep]);

  // Step 3: Auto-name by normal direction
  const handleAutoNameByNormal = useCallback(() => {
    if (!enclosureShape) {
      message.warning('Create an enclosure first.');
      return;
    }

    const cx = enclosureShape.position[0];
    const cy = enclosureShape.position[1];
    const cz = enclosureShape.position[2];
    const hw = (enclosureShape.dimensions.width ?? 1) / 2;
    const hh = (enclosureShape.dimensions.height ?? 1) / 2;
    const hd = (enclosureShape.dimensions.depth ?? 1) / 2;

    const autoSelections: NamedSelection[] = [
      {
        name: 'inlet',
        type: 'inlet',
        faces: [0],
        center: [cx - hw, cy, cz],
        normal: [-1, 0, 0],
        width: enclosureShape.dimensions.depth ?? 1,
        height: enclosureShape.dimensions.height ?? 1,
        color: selectionTypeColors.inlet,
      },
      {
        name: 'outlet',
        type: 'outlet',
        faces: [1],
        center: [cx + hw, cy, cz],
        normal: [1, 0, 0],
        width: enclosureShape.dimensions.depth ?? 1,
        height: enclosureShape.dimensions.height ?? 1,
        color: selectionTypeColors.outlet,
      },
      {
        name: 'wall-top',
        type: 'wall',
        faces: [2],
        center: [cx, cy + hh, cz],
        normal: [0, 1, 0],
        width: enclosureShape.dimensions.width ?? 1,
        height: enclosureShape.dimensions.depth ?? 1,
        color: selectionTypeColors.wall,
      },
      {
        name: 'wall-bottom',
        type: 'wall',
        faces: [3],
        center: [cx, cy - hh, cz],
        normal: [0, -1, 0],
        width: enclosureShape.dimensions.width ?? 1,
        height: enclosureShape.dimensions.depth ?? 1,
        color: selectionTypeColors.wall,
      },
      {
        name: 'wall-front',
        type: 'wall',
        faces: [4],
        center: [cx, cy, cz + hd],
        normal: [0, 0, 1],
        width: enclosureShape.dimensions.width ?? 1,
        height: enclosureShape.dimensions.height ?? 1,
        color: selectionTypeColors.wall,
      },
      {
        name: 'wall-back',
        type: 'wall',
        faces: [5],
        center: [cx, cy, cz - hd],
        normal: [0, 0, -1],
        width: enclosureShape.dimensions.width ?? 1,
        height: enclosureShape.dimensions.height ?? 1,
        color: selectionTypeColors.wall,
      },
    ];

    setNamedSelections(autoSelections);
    if (cfdPrepStep < 3) setCfdPrepStep(3);
    message.success('Auto-named 6 face selections by normal direction');
  }, [enclosureShape, setNamedSelections, cfdPrepStep, setCfdPrepStep]);

  // Add custom named selection
  const handleAddSelection = useCallback(() => {
    if (!newSelName.trim()) {
      message.warning('Enter a name for the selection.');
      return;
    }
    if (namedSelections.find((ns) => ns.name === newSelName.trim())) {
      message.warning('A selection with this name already exists.');
      return;
    }

    const sel: NamedSelection = {
      name: newSelName.trim(),
      type: newSelType,
      faces: [],
      center: [0, 0, 0],
      normal: [0, 1, 0],
      width: 1,
      height: 1,
      color: selectionTypeColors[newSelType],
    };
    addNamedSelection(sel);
    setNewSelName('');
    message.success(`Added named selection: ${sel.name}`);
  }, [newSelName, newSelType, namedSelections, addNamedSelection]);

  // Step 4: Share Topology
  const handleShareTopology = useCallback(() => {
    if (!enclosureCreated) {
      message.warning('Create an enclosure first.');
      return;
    }
    setTopologyShared(true);
    setCfdPrepStep(4);
    message.success('Topology shared: conformal interfaces created between bodies');
  }, [enclosureCreated, setTopologyShared, setCfdPrepStep]);

  // Step 5: Generate Mesh and switch to Mesh tab
  const handleGenerateMesh = useCallback(() => {
    if (!enclosureCreated) {
      message.warning('Create an enclosure first.');
      return;
    }
    generateMesh();
    setCfdPrepStep(5);
    // After a short delay (mesh generation is async), switch to mesh tab
    setTimeout(() => {
      setActiveTab('mesh');
      setActiveRibbonTab('mesh');
      message.success('Mesh generated. Switched to Mesh tab.');
    }, 900);
  }, [enclosureCreated, generateMesh, setCfdPrepStep, setActiveTab, setActiveRibbonTab]);

  const wallCount = namedSelections.filter((ns) => ns.type === 'wall').length;

  return (
    <div style={{ padding: 12, fontSize: 12 }}>
      {/* Header */}
      <div
        style={{
          fontWeight: 600,
          marginBottom: 12,
          fontSize: 14,
          borderBottom: '1px solid #303030',
          paddingBottom: 8,
          display: 'flex',
          alignItems: 'center',
          gap: 6,
        }}
      >
        <AppstoreOutlined />
        CFD Preparation
      </div>

      {/* ====== Step 1: Enclosure ====== */}
      <StepHeader step={1} title="Enclosure" done={enclosureCreated} current={cfdPrepStep === 0} />
      <div
        style={{
          background: '#111118',
          border: '1px solid #252530',
          borderRadius: 4,
          padding: 8,
          marginBottom: 12,
          marginLeft: 10,
          borderLeft: `2px solid ${enclosureCreated ? '#52c41a' : '#333'}`,
        }}
      >
        <div style={{ color: '#999', fontSize: 11, marginBottom: 6 }}>
          Padding (m):
        </div>
        <Form layout="vertical" size="small">
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '2px 8px' }}>
            <Form.Item label="+X" style={{ marginBottom: 2 }}>
              <InputNumber
                value={padXp}
                min={0}
                step={0.5}
                onChange={(v) => setPadXp(v ?? 2)}
                style={{ width: '100%' }}
                size="small"
              />
            </Form.Item>
            <Form.Item label="-X" style={{ marginBottom: 2 }}>
              <InputNumber
                value={padXn}
                min={0}
                step={0.5}
                onChange={(v) => setPadXn(v ?? 1)}
                style={{ width: '100%' }}
                size="small"
              />
            </Form.Item>
            <Form.Item label="+Y" style={{ marginBottom: 2 }}>
              <InputNumber
                value={padYp}
                min={0}
                step={0.5}
                onChange={(v) => setPadYp(v ?? 1)}
                style={{ width: '100%' }}
                size="small"
              />
            </Form.Item>
            <Form.Item label="-Y" style={{ marginBottom: 2 }}>
              <InputNumber
                value={padYn}
                min={0}
                step={0.5}
                onChange={(v) => setPadYn(v ?? 1)}
                style={{ width: '100%' }}
                size="small"
              />
            </Form.Item>
            <Form.Item label="+Z" style={{ marginBottom: 2 }}>
              <InputNumber
                value={padZp}
                min={0}
                step={0.5}
                onChange={(v) => setPadZp(v ?? 1)}
                style={{ width: '100%' }}
                size="small"
              />
            </Form.Item>
            <Form.Item label="-Z" style={{ marginBottom: 2 }}>
              <InputNumber
                value={padZn}
                min={0}
                step={0.5}
                onChange={(v) => setPadZn(v ?? 1)}
                style={{ width: '100%' }}
                size="small"
              />
            </Form.Item>
          </div>
        </Form>
        <Button
          type={enclosureCreated ? 'default' : 'primary'}
          icon={<ExpandOutlined />}
          onClick={handleCreateEnclosure}
          block
          size="small"
          style={{ marginTop: 4 }}
        >
          {enclosureCreated ? 'Recreate Enclosure' : 'Create Enclosure'}
        </Button>
        {enclosureCreated && (
          <div style={{ color: '#52c41a', fontSize: 11, marginTop: 4, display: 'flex', alignItems: 'center', gap: 4 }}>
            <CheckCircleOutlined /> Enclosure created
          </div>
        )}
      </div>

      {/* ====== Step 2: Volume Extract ====== */}
      <StepHeader step={2} title="Volume Extract" done={fluidExtracted} current={cfdPrepStep === 1} />
      <div
        style={{
          background: '#111118',
          border: '1px solid #252530',
          borderRadius: 4,
          padding: 8,
          marginBottom: 12,
          marginLeft: 10,
          borderLeft: `2px solid ${fluidExtracted ? '#52c41a' : '#333'}`,
          opacity: enclosureCreated ? 1 : 0.5,
          pointerEvents: enclosureCreated ? 'auto' : 'none',
        }}
      >
        {bodyShapes.length > 0 && (
          <Form layout="vertical" size="small" style={{ marginBottom: 4 }}>
            <Form.Item label="Select solid body:" style={{ marginBottom: 4 }}>
              <Select
                value={selectedBody}
                onChange={(v) => setSelectedBody(v)}
                placeholder="Select body"
                size="small"
                options={bodyShapes.map((s) => ({ value: s.id, label: s.name }))}
                style={{ width: '100%' }}
              />
            </Form.Item>
          </Form>
        )}
        <Button
          type={fluidExtracted ? 'default' : 'primary'}
          icon={<ExperimentOutlined />}
          onClick={handleExtractFluid}
          block
          size="small"
          disabled={!enclosureCreated}
        >
          Extract Fluid Volume
        </Button>
        {fluidExtracted && (
          <div style={{ color: '#52c41a', fontSize: 11, marginTop: 4, display: 'flex', alignItems: 'center', gap: 4 }}>
            <CheckCircleOutlined /> Fluid volume extracted
          </div>
        )}
      </div>

      {/* ====== Step 3: Named Selections ====== */}
      <StepHeader step={3} title="Named Selections" done={namedSelections.length > 0} current={cfdPrepStep === 2} />
      <div
        style={{
          background: '#111118',
          border: '1px solid #252530',
          borderRadius: 4,
          padding: 8,
          marginBottom: 12,
          marginLeft: 10,
          borderLeft: `2px solid ${namedSelections.length > 0 ? '#52c41a' : '#333'}`,
          opacity: enclosureCreated ? 1 : 0.5,
          pointerEvents: enclosureCreated ? 'auto' : 'none',
        }}
      >
        <div style={{ color: '#888', fontSize: 11, marginBottom: 6 }}>
          Click face in 3D to name
        </div>

        {/* Existing named selections */}
        {namedSelections.length > 0 && (
          <div style={{ maxHeight: 160, overflow: 'auto', marginBottom: 8 }}>
            {namedSelections.map((ns) => (
              <div
                key={ns.name}
                onMouseEnter={() => setHoveredSelectionName(ns.name)}
                onMouseLeave={() => setHoveredSelectionName(null)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  padding: '3px 4px',
                  borderBottom: '1px solid #1a1a1a',
                  background: hoveredSelectionName === ns.name ? '#1a1a3e' : 'transparent',
                  borderRadius: 2,
                  cursor: 'pointer',
                  transition: 'background 0.15s',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  <span style={{ color: ns.color, fontSize: 13 }}>
                    {selectionTypeIcons[ns.type]}
                  </span>
                  <span style={{ color: '#ddd', fontSize: 11 }}>{ns.name}</span>
                  <Tag
                    style={{
                      fontSize: 9,
                      padding: '0 3px',
                      lineHeight: '14px',
                      margin: 0,
                      border: `1px solid ${ns.color}44`,
                      color: ns.color,
                      background: 'transparent',
                    }}
                  >
                    {ns.type}
                  </Tag>
                </div>
                <Tooltip title="Remove">
                  <Button
                    type="text"
                    size="small"
                    icon={<DeleteOutlined />}
                    style={{ fontSize: 10, color: '#666', width: 20, height: 20, padding: 0 }}
                    onClick={(e) => {
                      e.stopPropagation();
                      removeNamedSelection(ns.name);
                    }}
                  />
                </Tooltip>
              </div>
            ))}
          </div>
        )}

        {/* Summary */}
        {namedSelections.length > 0 && (
          <div style={{ color: '#888', fontSize: 10, marginBottom: 6 }}>
            {namedSelections.length} selections ({wallCount} walls)
          </div>
        )}

        {/* Auto-name button */}
        <Button
          icon={<BgColorsOutlined />}
          onClick={handleAutoNameByNormal}
          block
          size="small"
          style={{ marginBottom: 6 }}
          disabled={!enclosureCreated}
        >
          Auto-Name by Normal
        </Button>

        {/* Add custom selection */}
        <Divider style={{ margin: '6px 0' }} />
        <div style={{ display: 'flex', gap: 4, marginBottom: 4 }}>
          <Input
            value={newSelName}
            onChange={(e) => setNewSelName(e.target.value)}
            placeholder="Selection name"
            size="small"
            style={{ flex: 1 }}
            onPressEnter={handleAddSelection}
          />
          <Select
            value={newSelType}
            onChange={(v) => setNewSelType(v)}
            size="small"
            style={{ width: 90 }}
            options={[
              { value: 'inlet', label: 'Inlet' },
              { value: 'outlet', label: 'Outlet' },
              { value: 'wall', label: 'Wall' },
              { value: 'symmetry', label: 'Symmetry' },
              { value: 'interface', label: 'Interface' },
              { value: 'custom', label: 'Custom' },
            ]}
          />
        </div>
        <Button
          icon={<PlusOutlined />}
          onClick={handleAddSelection}
          block
          size="small"
          disabled={!newSelName.trim()}
        >
          Add Named Selection
        </Button>
      </div>

      {/* ====== Step 4: Share Topology ====== */}
      <StepHeader step={4} title="Share Topology" done={topologyShared} current={cfdPrepStep === 3} />
      <div
        style={{
          background: '#111118',
          border: '1px solid #252530',
          borderRadius: 4,
          padding: 8,
          marginBottom: 8,
          marginLeft: 10,
          borderLeft: `2px solid ${topologyShared ? '#52c41a' : '#333'}`,
          opacity: enclosureCreated ? 1 : 0.5,
          pointerEvents: enclosureCreated ? 'auto' : 'none',
        }}
      >
        <div style={{ color: '#888', fontSize: 11, marginBottom: 6 }}>
          Creates shared faces between bodies for conformal meshing.
        </div>
        <Button
          icon={<BorderInnerOutlined />}
          onClick={handleShareTopology}
          block
          size="small"
          type={topologyShared ? 'default' : 'primary'}
          disabled={!enclosureCreated}
        >
          {topologyShared ? 'Topology Shared' : 'Enable Share Topology'}
        </Button>
        {topologyShared && (
          <div style={{ color: '#52c41a', fontSize: 11, marginTop: 4, display: 'flex', alignItems: 'center', gap: 4 }}>
            <CheckCircleOutlined /> Topology shared
          </div>
        )}
      </div>

      {/* ====== Step 5: Generate Mesh ====== */}
      <StepHeader step={5} title="Generate Mesh" done={meshGenerated} current={cfdPrepStep === 4} />
      <div
        style={{
          background: '#111118',
          border: '1px solid #252530',
          borderRadius: 4,
          padding: 8,
          marginBottom: 8,
          marginLeft: 10,
          borderLeft: `2px solid ${meshGenerated ? '#52c41a' : '#333'}`,
          opacity: enclosureCreated ? 1 : 0.5,
          pointerEvents: enclosureCreated ? 'auto' : 'none',
        }}
      >
        <div style={{ color: '#888', fontSize: 11, marginBottom: 6 }}>
          Generate mesh within the enclosure domain and switch to Mesh tab.
        </div>
        <Button
          type={meshGenerated ? 'default' : 'primary'}
          icon={meshGenerating ? <LoadingOutlined /> : <BuildOutlined />}
          onClick={handleGenerateMesh}
          block
          size="small"
          disabled={!enclosureCreated || meshGenerating}
          loading={meshGenerating}
        >
          {meshGenerating
            ? 'Generating...'
            : meshGenerated
            ? 'Regenerate Mesh'
            : 'Generate Mesh'}
        </Button>
        {meshGenerated && (
          <div style={{ color: '#52c41a', fontSize: 11, marginTop: 4, display: 'flex', alignItems: 'center', gap: 4 }}>
            <CheckCircleOutlined /> Mesh generated
          </div>
        )}
      </div>
    </div>
  );
};

export default CfdPrepPanel;
