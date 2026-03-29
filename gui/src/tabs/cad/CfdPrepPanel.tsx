import React, { useState, useCallback, useMemo, useEffect } from 'react';
import { Button, InputNumber, Form, Select, message } from 'antd';
import {
  ExpandOutlined,
  ExperimentOutlined,
  CheckCircleOutlined,
  AppstoreOutlined,
  ClearOutlined,
  SelectOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../../store/useAppStore';

let nextEnclosureId = 100;

const CfdPrepPanel: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const addShape = useAppStore((s) => s.addShape);
  const enclosureCreated = useAppStore((s) => s.enclosureCreated);
  const setEnclosureCreated = useAppStore((s) => s.setEnclosureCreated);
  const fluidExtracted = useAppStore((s) => s.fluidExtracted);
  const setFluidExtracted = useAppStore((s) => s.setFluidExtracted);
  const cfdPrepStep = useAppStore((s) => s.cfdPrepStep);
  const setCfdPrepStep = useAppStore((s) => s.setCfdPrepStep);
  const setEnclosurePreview = useAppStore((s) => s.setEnclosurePreview);
  const selectedBodiesForEnclosure = useAppStore((s) => s.selectedBodiesForEnclosure);
  const setSelectedBodiesForEnclosure = useAppStore((s) => s.setSelectedBodiesForEnclosure);
  const toggleBodyForEnclosure = useAppStore((s) => s.toggleBodyForEnclosure);

  const [padXp, setPadXp] = useState(2.0);
  const [padXn, setPadXn] = useState(1.0);
  const [padYp, setPadYp] = useState(1.0);
  const [padYn, setPadYn] = useState(1.0);
  const [padZp, setPadZp] = useState(1.0);
  const [padZn, setPadZn] = useState(1.0);
  const [selectedBody, setSelectedBody] = useState<string | null>(null);

  const bodyShapes = useMemo(
    () => shapes.filter((s) => s.group !== 'enclosure' && s.kind !== 'enclosure'),
    [shapes]
  );

  // Compute bounding box of selected bodies
  const boundingBox = useMemo(() => {
    const selected = selectedBodiesForEnclosure.length > 0
      ? bodyShapes.filter((s) => selectedBodiesForEnclosure.includes(s.id))
      : [];
    if (selected.length === 0) return null;

    let minX = Infinity, maxX = -Infinity;
    let minY = Infinity, maxY = -Infinity;
    let minZ = Infinity, maxZ = -Infinity;

    selected.forEach((s) => {
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

    const cx = (minX + maxX) / 2;
    const cy = (minY + maxY) / 2;
    const cz = (minZ + maxZ) / 2;

    return { minX, maxX, minY, maxY, minZ, maxZ, cx, cy, cz };
  }, [bodyShapes, selectedBodiesForEnclosure]);

  // Computed enclosure dimensions
  const enclosureDims = useMemo(() => {
    if (!boundingBox) return null;
    const w = (boundingBox.maxX + padXp) - (boundingBox.minX - padXn);
    const h = (boundingBox.maxY + padYp) - (boundingBox.minY - padYn);
    const d = (boundingBox.maxZ + padZp) - (boundingBox.minZ - padZn);
    return { w, h, d };
  }, [boundingBox, padXp, padXn, padYp, padYn, padZp, padZn]);

  // Update enclosure preview when paddings or selected bodies change
  useEffect(() => {
    if (!boundingBox) {
      setEnclosurePreview(null);
      return;
    }
    setEnclosurePreview({
      center: [boundingBox.cx, boundingBox.cy, boundingBox.cz],
      padXp, padXn, padYp, padYn, padZp, padZn,
    });
  }, [boundingBox, padXp, padXn, padYp, padYn, padZp, padZn, setEnclosurePreview]);

  // Clear preview when component unmounts
  useEffect(() => {
    return () => {
      setEnclosurePreview(null);
    };
  }, [setEnclosurePreview]);

  // Select all bodies
  const handleSelectAll = useCallback(() => {
    setSelectedBodiesForEnclosure(bodyShapes.map((s) => s.id));
  }, [bodyShapes, setSelectedBodiesForEnclosure]);

  // Clear selection
  const handleClearSelection = useCallback(() => {
    setSelectedBodiesForEnclosure([]);
  }, [setSelectedBodiesForEnclosure]);

  // Create Enclosure
  const handleCreateEnclosure = useCallback(() => {
    if (!boundingBox || selectedBodiesForEnclosure.length === 0) {
      message.warning('Select at least one body to enclose.');
      return;
    }

    const minX = boundingBox.minX - padXn;
    const maxX = boundingBox.maxX + padXp;
    const minY = boundingBox.minY - padYn;
    const maxY = boundingBox.maxY + padYp;
    const minZ = boundingBox.minZ - padZn;
    const maxZ = boundingBox.maxZ + padZp;

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
    setEnclosurePreview(null);
    if (cfdPrepStep < 1) setCfdPrepStep(1);
    message.success(`Enclosure created: ${w.toFixed(2)} x ${h.toFixed(2)} x ${d.toFixed(2)} m`);
  }, [boundingBox, selectedBodiesForEnclosure, padXp, padXn, padYp, padYn, padZp, padZn, addShape, setEnclosureCreated, setEnclosurePreview, cfdPrepStep, setCfdPrepStep]);

  // Extract Fluid Volume: hide solid, mark enclosure as fluid domain
  const handleExtractFluid = useCallback(() => {
    if (!enclosureCreated) {
      message.warning('Create an enclosure first.');
      return;
    }
    if (!selectedBody) {
      message.warning('Select a solid body to subtract from enclosure.');
      return;
    }

    const state = useAppStore.getState();
    const solidShape = state.shapes.find((s) => s.id === selectedBody);
    const enclosureShape = state.shapes.find((s) => s.kind === 'enclosure' || s.isEnclosure);

    if (!solidShape || !enclosureShape) {
      message.error('Could not find solid or enclosure shape.');
      return;
    }

    // Store the solid's info in the enclosure for rendering the "hole"
    // For STL shapes, also store the vertex data so CadScene can render the cutout
    state.updateShape(enclosureShape.id, {
      dimensions: {
        ...enclosureShape.dimensions,
        subtractedSolidId: solidShape.id,
        subtractedSolidKind: solidShape.kind,
        subtractedSolidPos: solidShape.position,
        subtractedSolidDims: solidShape.dimensions,
        subtractedSolidRotation: solidShape.rotation,
      },
      stlData: solidShape.kind === 'stl' && solidShape.stlData
        ? solidShape.stlData  // Pass STL vertices for 3D cutout rendering
        : enclosureShape.stlData,
    });

    // Hide the original solid (don't delete — mark as hidden so cutout stays)
    state.updateShape(selectedBody, { group: 'extracted_solid' } as any);

    setFluidExtracted(true);
    if (cfdPrepStep < 2) setCfdPrepStep(2);

    // Add console log
    const lines = state.consoleLines || [];
    state.setConsoleLines([
      ...lines,
      `[CFD Prep] Extracted fluid volume: Enclosure minus "${solidShape.name}"`,
      `[CFD Prep] Solid body "${solidShape.name}" removed from scene (subtracted)`,
      `[CFD Prep] Fluid domain is now the enclosure with internal cutout`,
    ]);

    message.success(`Fluid volume extracted: Enclosure - "${solidShape.name}"`);
  }, [enclosureCreated, selectedBody, setFluidExtracted, cfdPrepStep, setCfdPrepStep]);

  const inputStyle = { width: '100%' };
  const labelStyle: React.CSSProperties = { marginBottom: 2 };

  return (
    <div style={{ padding: 12, fontSize: 12 }}>
      {/* ====== Enclosure Section ====== */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 6,
          fontWeight: 600,
          fontSize: 13,
          marginBottom: 8,
          paddingBottom: 6,
          borderBottom: '1px solid #303050',
          color: '#ccd',
        }}
      >
        <ExpandOutlined style={{ color: '#52c41a' }} />
        Enclosure
      </div>

      <div
        style={{
          background: '#111118',
          border: '1px solid #252530',
          borderRadius: 4,
          padding: 10,
          marginBottom: 14,
          borderLeft: `2px solid ${enclosureCreated ? '#52c41a' : '#4096ff'}`,
        }}
      >
        {/* Selected bodies count */}
        <div style={{ color: '#aab', fontSize: 11, marginBottom: 6, display: 'flex', alignItems: 'center', gap: 4 }}>
          <SelectOutlined style={{ fontSize: 12 }} />
          Selected bodies: <strong style={{ color: '#fff' }}>{selectedBodiesForEnclosure.length}</strong>
        </div>

        {/* Select All / Clear buttons */}
        <div style={{ display: 'flex', gap: 4, marginBottom: 8 }}>
          <Button size="small" onClick={handleSelectAll} style={{ flex: 1, fontSize: 11 }}>
            Select All
          </Button>
          <Button size="small" icon={<ClearOutlined />} onClick={handleClearSelection} style={{ flex: 1, fontSize: 11 }}>
            Clear
          </Button>
        </div>

        {/* Body list for selection */}
        {bodyShapes.length > 0 && (
          <div style={{ maxHeight: 100, overflow: 'auto', marginBottom: 8, border: '1px solid #1a1a30', borderRadius: 3 }}>
            {bodyShapes.map((s) => {
              const isSelected = selectedBodiesForEnclosure.includes(s.id);
              return (
                <div
                  key={s.id}
                  onClick={() => toggleBodyForEnclosure(s.id)}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 6,
                    padding: '3px 6px',
                    fontSize: 11,
                    cursor: 'pointer',
                    background: isSelected ? '#1a2a4a' : 'transparent',
                    color: isSelected ? '#4096ff' : '#889',
                    borderBottom: '1px solid #1a1a30',
                  }}
                  onMouseEnter={(e) => { if (!isSelected) e.currentTarget.style.background = '#161630'; }}
                  onMouseLeave={(e) => { if (!isSelected) e.currentTarget.style.background = 'transparent'; }}
                >
                  <input
                    type="checkbox"
                    checked={isSelected}
                    onChange={() => {}}
                    style={{ margin: 0, cursor: 'pointer' }}
                  />
                  <span>{s.name}</span>
                </div>
              );
            })}
          </div>
        )}

        {/* Center (read-only) */}
        {boundingBox && (
          <div style={{ color: '#889', fontSize: 11, marginBottom: 8 }}>
            Center: ({boundingBox.cx.toFixed(2)}, {boundingBox.cy.toFixed(2)}, {boundingBox.cz.toFixed(2)})
          </div>
        )}

        {/* Padding inputs */}
        <div style={{ color: '#999', fontSize: 11, marginBottom: 4 }}>Padding:</div>
        <Form layout="vertical" size="small">
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '2px 8px' }}>
            <Form.Item label="+X" style={labelStyle}>
              <InputNumber value={padXp} min={0} step={0.5} onChange={(v) => setPadXp(v ?? 2)} style={inputStyle} size="small" />
            </Form.Item>
            <Form.Item label="-X" style={labelStyle}>
              <InputNumber value={padXn} min={0} step={0.5} onChange={(v) => setPadXn(v ?? 1)} style={inputStyle} size="small" />
            </Form.Item>
            <Form.Item label="+Y" style={labelStyle}>
              <InputNumber value={padYp} min={0} step={0.5} onChange={(v) => setPadYp(v ?? 1)} style={inputStyle} size="small" />
            </Form.Item>
            <Form.Item label="-Y" style={labelStyle}>
              <InputNumber value={padYn} min={0} step={0.5} onChange={(v) => setPadYn(v ?? 1)} style={inputStyle} size="small" />
            </Form.Item>
            <Form.Item label="+Z" style={labelStyle}>
              <InputNumber value={padZp} min={0} step={0.5} onChange={(v) => setPadZp(v ?? 1)} style={inputStyle} size="small" />
            </Form.Item>
            <Form.Item label="-Z" style={labelStyle}>
              <InputNumber value={padZn} min={0} step={0.5} onChange={(v) => setPadZn(v ?? 1)} style={inputStyle} size="small" />
            </Form.Item>
          </div>
        </Form>

        {/* Computed size */}
        {enclosureDims && (
          <div style={{ color: '#aab', fontSize: 11, marginTop: 2, marginBottom: 6, fontWeight: 500 }}>
            Size: {enclosureDims.w.toFixed(1)} x {enclosureDims.h.toFixed(1)} x {enclosureDims.d.toFixed(1)} m
          </div>
        )}

        {/* Create Enclosure button */}
        <Button
          type={enclosureCreated ? 'default' : 'primary'}
          icon={enclosureCreated ? <CheckCircleOutlined /> : <ExpandOutlined />}
          onClick={handleCreateEnclosure}
          block
          size="small"
          style={{ marginTop: 4 }}
          disabled={selectedBodiesForEnclosure.length === 0}
        >
          {enclosureCreated ? 'Recreate Enclosure' : 'Create Enclosure'}
        </Button>
        {enclosureCreated && (
          <div style={{ color: '#52c41a', fontSize: 11, marginTop: 4, display: 'flex', alignItems: 'center', gap: 4 }}>
            <CheckCircleOutlined /> Enclosure created
          </div>
        )}
      </div>

      {/* ====== Volume Extract Section ====== */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 6,
          fontWeight: 600,
          fontSize: 13,
          marginBottom: 8,
          paddingBottom: 6,
          borderBottom: '1px solid #303050',
          color: enclosureCreated ? '#ccd' : '#556',
        }}
      >
        <ExperimentOutlined style={{ color: enclosureCreated ? '#1677ff' : '#444' }} />
        Volume Extract
      </div>

      <div
        style={{
          background: '#111118',
          border: '1px solid #252530',
          borderRadius: 4,
          padding: 10,
          marginBottom: 8,
          borderLeft: `2px solid ${fluidExtracted ? '#52c41a' : '#333'}`,
          opacity: enclosureCreated ? 1 : 0.5,
          pointerEvents: enclosureCreated ? 'auto' : 'none',
        }}
      >
        <div style={{ color: '#889', fontSize: 11, marginBottom: 4, display: 'flex', alignItems: 'center', gap: 4 }}>
          Enclosure: {enclosureCreated ? (
            <span style={{ color: '#52c41a' }}><CheckCircleOutlined /> Created</span>
          ) : (
            <span style={{ color: '#ff4d4f' }}>Not created</span>
          )}
        </div>

        {bodyShapes.length > 0 && (
          <Form layout="vertical" size="small" style={{ marginBottom: 4 }}>
            <Form.Item label="Solid body:" style={{ marginBottom: 4 }}>
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
    </div>
  );
};

export default CfdPrepPanel;
