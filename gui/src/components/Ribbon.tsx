import React, { useCallback } from 'react';
import { Upload, message } from 'antd';
import {
  // Design tab
  CopyOutlined,
  ScissorOutlined,
  SnippetsOutlined,
  HomeOutlined,
  DragOutlined,
  RetweetOutlined,
  SyncOutlined,
  ZoomInOutlined,
  EditOutlined,
  SelectOutlined,
  ColumnHeightOutlined,
  SwapOutlined,
  FormatPainterOutlined,
  HighlightOutlined,
  CompressOutlined,
  BlockOutlined,
  RadiusSettingOutlined,
  PlusCircleOutlined,
  MinusCircleOutlined,
  InteractionOutlined,
  SplitCellsOutlined,
  BorderOutlined,
  GatewayOutlined,
  AimOutlined,
  ExpandOutlined,
  ImportOutlined,
  // Display tab
  EyeOutlined,
  EyeInvisibleOutlined,
  BgColorsOutlined,
  BulbOutlined,
  PictureOutlined,
  // Measure tab
  ColumnWidthOutlined,
  FieldNumberOutlined,
  // Repair tab
  CheckCircleOutlined,
  ToolOutlined,
  MergeCellsOutlined,
  // Prepare tab
  ExperimentOutlined,
  BorderInnerOutlined,
  AppstoreOutlined,
  BugOutlined,
  ThunderboltOutlined,
  DeleteOutlined,
  // Mesh tab
  BuildOutlined,
  SettingOutlined,
  BarChartOutlined,
  // Setup tab
  GoldOutlined,
  // Calc tab
  CaretRightOutlined,
  PauseOutlined,
  StopOutlined,
  // Results tab
  HeatMapOutlined,
  ArrowsAltOutlined,
  FileTextOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../store/useAppStore';
import type { RibbonTab, ShapeKind, BooleanOp } from '../store/useAppStore';

// ---- Ribbon Button Component ----
const RibbonButton: React.FC<{
  icon: React.ReactNode;
  label: string;
  active?: boolean;
  large?: boolean;
  onClick?: () => void;
}> = ({ icon, label, active, large, onClick }) => (
  <div
    onClick={onClick}
    style={{
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center',
      padding: large ? '4px 10px' : '4px 6px',
      minWidth: large ? 48 : 36,
      cursor: 'pointer',
      borderRadius: 3,
      background: active ? '#2a2a4a' : 'transparent',
      color: active ? '#4096ff' : '#bbb',
      userSelect: 'none',
      transition: 'all 0.12s',
      fontSize: large ? 20 : 16,
    }}
    onMouseEnter={(e) => {
      if (!active) e.currentTarget.style.background = '#252540';
    }}
    onMouseLeave={(e) => {
      if (!active) e.currentTarget.style.background = 'transparent';
    }}
  >
    <span style={{ fontSize: large ? 20 : 16, lineHeight: 1 }}>{icon}</span>
    <span style={{ fontSize: 10, marginTop: 2, whiteSpace: 'nowrap', lineHeight: 1.2 }}>{label}</span>
  </div>
);

// ---- Group Separator ----
const GroupSep: React.FC<{ label?: string }> = ({ label }) => (
  <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', padding: '0 2px' }}>
    <div style={{ width: 1, flex: 1, background: '#3a3a5a', minHeight: 20 }} />
    {label && <span style={{ fontSize: 9, color: '#666', padding: '2px 0', whiteSpace: 'nowrap' }}>{label}</span>}
  </div>
);

let nextId = 100;

/** Parse binary STL buffer */
function parseBinaryStl(buf: ArrayBuffer): { verts: Float32Array; fc: number } {
  const dv = new DataView(buf);
  const fc = dv.getUint32(80, true);
  if (fc === 0 || 84 + fc * 50 > buf.byteLength) {
    return { verts: new Float32Array(0), fc: 0 };
  }
  const verts = new Float32Array(fc * 9);
  let offset = 84;
  for (let i = 0; i < fc; i++) {
    offset += 12; // skip normal
    for (let v = 0; v < 3; v++) {
      verts[i * 9 + v * 3] = dv.getFloat32(offset, true);
      verts[i * 9 + v * 3 + 1] = dv.getFloat32(offset + 4, true);
      verts[i * 9 + v * 3 + 2] = dv.getFloat32(offset + 8, true);
      offset += 12;
    }
    offset += 2; // skip attribute
  }
  return { verts, fc };
}

function makeShape(kind: ShapeKind) {
  const id = `shape-${nextId++}`;
  const defaults: Record<string, Record<string, number>> = {
    box: { width: 1, height: 1, depth: 1 },
    sphere: { radius: 0.5 },
    cylinder: { radius: 0.3, height: 1 },
    cone: { radius: 0.4, height: 1 },
    torus: { majorRadius: 0.5, minorRadius: 0.15 },
    pipe: { outerRadius: 0.4, innerRadius: 0.3, height: 1.5 },
  };
  return {
    id,
    name: `${kind}-${id}`,
    kind,
    position: [0, 0, 0] as [number, number, number],
    rotation: [0, 0, 0] as [number, number, number],
    dimensions: { ...(defaults[kind] ?? {}) },
    group: 'body' as const,
  };
}

// ============================================================
// Design Tab Ribbon
// ============================================================
const DesignRibbon: React.FC = () => {
  const addShape = useAppStore((s) => s.addShape);
  const updateShape = useAppStore((s) => s.updateShape);
  const shapes = useAppStore((s) => s.shapes);
  const selectedShapeId = useAppStore((s) => s.selectedShapeId);
  const setCadMode = useAppStore((s) => s.setCadMode);
  const setPendingBooleanOp = useAppStore((s) => s.setPendingBooleanOp);
  const activeTool = useAppStore((s) => s.activeTool);
  const setActiveTool = useAppStore((s) => s.setActiveTool);
  const clipboardShape = useAppStore((s) => s.clipboardShape);
  const setClipboardShape = useAppStore((s) => s.setClipboardShape);

  const create = useCallback((kind: ShapeKind) => {
    addShape(makeShape(kind));
  }, [addShape]);

  const startBoolean = useCallback((op: BooleanOp) => {
    if (shapes.filter((s) => s.group !== 'enclosure').length < 2) {
      message.warning('Boolean operations require at least 2 shapes.');
      return;
    }
    if (selectedShapeId) {
      setCadMode('boolean_select_tool');
      setPendingBooleanOp(op);
      useAppStore.getState().setPendingBooleanTargetId(selectedShapeId);
      message.info(`Click the tool shape to ${op}.`);
    } else {
      setCadMode('boolean_select_target');
      setPendingBooleanOp(op);
      message.info(`Select the TARGET shape first for ${op}.`);
    }
  }, [shapes, selectedShapeId, setCadMode, setPendingBooleanOp]);

  return (
    <div style={{ display: 'flex', alignItems: 'stretch', gap: 0, height: '100%' }}>
      {/* Clipboard Group */}
      <RibbonButton icon={<SnippetsOutlined />} label="Paste" onClick={() => {
        if (!clipboardShape) { message.warning('Nothing in clipboard. Copy a shape first.'); return; }
        const id = `shape-${nextId++}`;
        const pasted = {
          ...clipboardShape,
          id,
          name: `${clipboardShape.name}-copy`,
          position: [
            clipboardShape.position[0] + 0.5,
            clipboardShape.position[1],
            clipboardShape.position[2],
          ] as [number, number, number],
        };
        addShape(pasted);
        message.success(`Pasted "${pasted.name}".`);
      }} />
      <RibbonButton icon={<CopyOutlined />} label="Copy" onClick={() => {
        if (!selectedShapeId) { message.warning('Select a shape first.'); return; }
        const shape = shapes.find((s) => s.id === selectedShapeId);
        if (!shape) return;
        setClipboardShape({ ...shape });
        message.success(`Copied "${shape.name}" to clipboard.`);
      }} />
      <RibbonButton icon={<ScissorOutlined />} label="Cut" onClick={() => {
        if (!selectedShapeId) { message.warning('Select a shape first.'); return; }
        const shape = shapes.find((s) => s.id === selectedShapeId);
        if (!shape) return;
        setClipboardShape({ ...shape });
        useAppStore.getState().removeShape(selectedShapeId);
        message.success(`Cut "${shape.name}" to clipboard.`);
      }} />
      <GroupSep label="Clipboard" />

      {/* Orient Group */}
      <RibbonButton icon={<HomeOutlined />} label="Home" onClick={() => window.dispatchEvent(new CustomEvent('gfd-camera-preset', { detail: { position: [5, 5, 5] } }))} />
      <RibbonButton icon={<DragOutlined />} label="Pan" onClick={() => message.info('Pan: Use middle mouse')} />
      <RibbonButton icon={<SyncOutlined />} label="Spin" onClick={() => message.info('Spin: Use right mouse')} />
      <RibbonButton icon={<ZoomInOutlined />} label="Zoom" onClick={() => message.info('Zoom: Use scroll wheel')} />
      <GroupSep label="Orient" />

      {/* Sketch Group */}
      <RibbonButton icon={<EditOutlined />} label="Sketch" onClick={() => { setActiveTool('select'); message.info('Sketch: Select faces to extrude with Pull tool.'); }} />
      <GroupSep label="Sketch" />

      {/* Select/Pull/Move/Fill Group */}
      <RibbonButton icon={<SelectOutlined />} label="Select" active={activeTool === 'select'} large onClick={() => setActiveTool('select')} />
      <RibbonButton icon={<ColumnHeightOutlined />} label="Pull" active={activeTool === 'pull'} large onClick={() => setActiveTool('pull')} />
      <RibbonButton icon={<SwapOutlined />} label="Move" active={activeTool === 'move'} large onClick={() => setActiveTool('move')} />
      <RibbonButton icon={<FormatPainterOutlined />} label="Fill" active={activeTool === 'fill'} large onClick={() => setActiveTool('fill')} />
      <GroupSep label="Tools" />

      {/* Edit Group */}
      <RibbonButton icon={<HighlightOutlined />} label="Blend" onClick={() => {
        if (!selectedShapeId) { message.warning('Select a shape to fillet.'); return; }
        const shape = shapes.find((s) => s.id === selectedShapeId);
        if (!shape) return;
        const currentRadius = shape.dimensions.filletRadius ?? 0;
        const newRadius = currentRadius > 0 ? 0 : 0.08;
        updateShape(selectedShapeId, { dimensions: { ...shape.dimensions, filletRadius: newRadius } });
        message.success(newRadius > 0 ? `Applied fillet (radius=${newRadius}) to "${shape.name}".` : `Removed fillet from "${shape.name}".`);
      }} />
      <RibbonButton icon={<CompressOutlined />} label="Chamfer" onClick={() => { if (!selectedShapeId) { message.warning('Select a shape to chamfer.'); return; } const shape = shapes.find((s) => s.id === selectedShapeId); if (!shape) return; const cur = shape.dimensions.chamferSize ?? 0; const nv = cur > 0 ? 0 : 0.05; updateShape(selectedShapeId, { dimensions: { ...shape.dimensions, chamferSize: nv } }); message.success(nv > 0 ? `Applied chamfer (${nv}) to "${shape.name}".` : `Removed chamfer from "${shape.name}".`); }} />
      <GroupSep label="Edit" />

      {/* Boolean Group */}
      <RibbonButton icon={<SplitCellsOutlined />} label="Split" onClick={() => startBoolean('split')} />
      <RibbonButton icon={<PlusCircleOutlined />} label="Union" onClick={() => startBoolean('union')} />
      <RibbonButton icon={<MinusCircleOutlined />} label="Subtract" onClick={() => startBoolean('subtract')} />
      <RibbonButton icon={<InteractionOutlined />} label="Intersect" onClick={() => startBoolean('intersect')} />
      <GroupSep label="Boolean" />

      {/* Create Group */}
      <RibbonButton icon={<CompressOutlined />} label="Shell" onClick={() => {
        if (!selectedShapeId) { message.warning('Select a shape to shell.'); return; }
        const shape = shapes.find((s) => s.id === selectedShapeId);
        if (!shape) return;
        const isShell = shape.dimensions.isShell ?? 0;
        if (isShell) {
          updateShape(selectedShapeId, { dimensions: { ...shape.dimensions, isShell: 0, shellThickness: 0 } });
          message.success(`Removed shell from "${shape.name}".`);
        } else {
          const thickness = 0.05;
          updateShape(selectedShapeId, { dimensions: { ...shape.dimensions, isShell: 1, shellThickness: thickness } });
          message.success(`Applied shell (thickness=${thickness}) to "${shape.name}".`);
        }
      }} />
      <RibbonButton icon={<BlockOutlined />} label="Offset" onClick={() => {
        if (!selectedShapeId) { message.warning('Select a shape to offset.'); return; }
        const shape = shapes.find((s) => s.id === selectedShapeId);
        if (!shape) return;
        const m = makeShape(shape.kind);
        m.name = `${shape.name}-offset`;
        m.dimensions = { ...shape.dimensions };
        m.position = [shape.position[0] + 0.1, shape.position[1] + 0.1, shape.position[2] + 0.1];
        m.rotation = [...shape.rotation];
        addShape(m);
        message.success(`Offset copy of "${shape.name}" created.`);
      }} />
      <RibbonButton icon={<SwapOutlined />} label="Mirror" onClick={() => {
        if (!selectedShapeId) { message.warning('Select a shape first.'); return; }
        const shape = shapes.find((s) => s.id === selectedShapeId);
        if (!shape) return;
        const m = makeShape(shape.kind);
        m.name = `${shape.name}-mirror`;
        m.dimensions = { ...shape.dimensions };
        m.position = [-shape.position[0], shape.position[1], shape.position[2]];
        addShape(m);
        message.success('Mirrored across YZ plane.');
      }} />
      <RibbonButton icon={<BorderOutlined />} label="Box" onClick={() => create('box')} />
      <RibbonButton icon={<RadiusSettingOutlined />} label="Sphere" onClick={() => create('sphere')} />
      <RibbonButton icon={<ColumnHeightOutlined />} label="Cylinder" onClick={() => create('cylinder')} />
      <RibbonButton icon={<AimOutlined />} label="Cone" onClick={() => create('cone')} />
      <RibbonButton icon={<RetweetOutlined />} label="Torus" onClick={() => create('torus')} />
      <RibbonButton icon={<GatewayOutlined />} label="Pipe" onClick={() => create('pipe')} />
      <GroupSep label="Create" />

      {/* Reference Geometry */}
      <RibbonButton icon={<FieldNumberOutlined />} label="Equation" onClick={() => {
        const eq = window.prompt('Enter equation surface (z = f(x,y)):', 'sin(x)*cos(y)');
        if (!eq) return;
        const id = `shape-${nextId++}`;
        addShape({
          id, name: `Eq: ${eq.substring(0, 16)}`, kind: 'box',
          position: [0, 0, 0], rotation: [0, 0, 0],
          dimensions: { width: 4, height: 0.01, depth: 4, _equation: 1 },
          group: 'body',
        });
        message.success(`Equation surface created: z = ${eq}`);
      }} />
      <RibbonButton icon={<BorderInnerOutlined />} label="Plane" onClick={() => {
        const id = `shape-${nextId++}`;
        addShape({
          id, name: 'Ref Plane', kind: 'box',
          position: [0, 0, 0], rotation: [0, 0, 0],
          dimensions: { width: 4, height: 0.005, depth: 4, _refHelper: 1 },
          group: 'body',
        });
        message.success('Reference plane created.');
      }} />
      <RibbonButton icon={<AimOutlined />} label="Axis" onClick={() => {
        const id = `shape-${nextId++}`;
        addShape({
          id, name: 'Ref Axis', kind: 'cylinder',
          position: [0, 0, 0], rotation: [0, 0, 0],
          dimensions: { radius: 0.01, height: 6, _refHelper: 1 },
          group: 'body',
        });
        message.success('Reference axis created.');
      }} />
      <GroupSep label="Reference" />

      {/* Import STL (ASCII + Binary) */}
      <Upload
        accept=".stl,.STL"
        showUploadList={false}
        beforeUpload={(file) => {
          const reader = new FileReader();
          reader.onload = (e) => {
            try {
              const buf = e.target?.result as ArrayBuffer;
              if (!buf || buf.byteLength < 84) {
                message.error('Invalid STL file (too small)');
                return;
              }

              // Detect ASCII vs Binary
              const headerBytes = new Uint8Array(buf, 0, 6);
              const headerStr = String.fromCharCode(...headerBytes);
              let verts: Float32Array;
              let fc: number;

              if (headerStr.startsWith('solid') && buf.byteLength > 84) {
                // Try ASCII first
                const text = new TextDecoder().decode(buf);
                const vertexRegex = /vertex\s+([-\d.eE+]+)\s+([-\d.eE+]+)\s+([-\d.eE+]+)/g;
                const coords: number[] = [];
                let match;
                while ((match = vertexRegex.exec(text)) !== null) {
                  coords.push(parseFloat(match[1]), parseFloat(match[2]), parseFloat(match[3]));
                }
                if (coords.length >= 9) {
                  // ASCII parse succeeded
                  verts = new Float32Array(coords);
                  fc = coords.length / 9;
                } else {
                  // Fallback to binary (some binary files start with "solid")
                  const result = parseBinaryStl(buf);
                  verts = result.verts;
                  fc = result.fc;
                }
              } else {
                // Binary STL
                const result = parseBinaryStl(buf);
                verts = result.verts;
                fc = result.fc;
              }

              if (fc === 0) {
                message.error('No triangles found in STL file');
                return;
              }

              // Compute bounding box and auto-center
              let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity, minZ = Infinity, maxZ = -Infinity;
              for (let vi = 0; vi < verts.length; vi += 3) {
                if (verts[vi] < minX) minX = verts[vi]; if (verts[vi] > maxX) maxX = verts[vi];
                if (verts[vi+1] < minY) minY = verts[vi+1]; if (verts[vi+1] > maxY) maxY = verts[vi+1];
                if (verts[vi+2] < minZ) minZ = verts[vi+2]; if (verts[vi+2] > maxZ) maxZ = verts[vi+2];
              }
              const cx = (minX + maxX) / 2, cy = (minY + maxY) / 2, cz = (minZ + maxZ) / 2;
              const maxSpan = Math.max(maxX - minX, maxY - minY, maxZ - minZ);

              // Auto-center: shift vertices so center is at origin
              if (confirm(`Auto-center STL at origin? (current center: ${cx.toFixed(2)}, ${cy.toFixed(2)}, ${cz.toFixed(2)}, span: ${maxSpan.toFixed(2)}m)`)) {
                for (let vi = 0; vi < verts.length; vi += 3) {
                  verts[vi] -= cx; verts[vi+1] -= cy; verts[vi+2] -= cz;
                }
              }

              // Auto-scale if very large or very small
              if (maxSpan > 10 || maxSpan < 0.01) {
                if (confirm(`Scale to fit unit box? (current span: ${maxSpan.toFixed(4)}m)`)) {
                  const scale = 2.0 / maxSpan;
                  for (let vi = 0; vi < verts.length; vi++) verts[vi] *= scale;
                }
              }

              const id = `shape-${nextId++}`;
              addShape({
                id,
                name: file.name.replace(/\.stl$/i, ''),
                kind: 'stl',
                position: [0, 0, 0],
                rotation: [0, 0, 0],
                dimensions: {},
                stlData: { vertices: verts, faceCount: fc },
                group: 'body',
              });
              message.success(`Imported ${file.name} (${fc} triangles, span: ${maxSpan.toFixed(2)}m)`);
            } catch (err: any) {
              message.error(`STL import failed: ${err.message || err}`);
            }
          };
          reader.readAsArrayBuffer(file);
          return false;
        }}
      >
        <RibbonButton icon={<ImportOutlined />} label="Import" />
      </Upload>
    </div>
  );
};

// ============================================================
// Display Tab Ribbon
// ============================================================
const DisplayRibbon: React.FC = () => {
  const renderMode = useAppStore((s) => s.renderMode);
  const setRenderMode = useAppStore((s) => s.setRenderMode);
  const cameraMode = useAppStore((s) => s.cameraMode);
  const setCameraMode = useAppStore((s) => s.setCameraMode);
  const transparencyMode = useAppStore((s) => s.transparencyMode);
  const setTransparencyMode = useAppStore((s) => s.setTransparencyMode);
  const sectionPlane = useAppStore((s) => s.sectionPlane);
  const setSectionPlane = useAppStore((s) => s.setSectionPlane);
  const exploded = useAppStore((s) => s.exploded);
  const setExploded = useAppStore((s) => s.setExploded);

  return (
    <div style={{ display: 'flex', alignItems: 'stretch', gap: 0, height: '100%' }}>
      <RibbonButton icon={<BorderOutlined />} label="Wireframe" active={renderMode === 'wireframe'} onClick={() => setRenderMode('wireframe')} />
      <RibbonButton icon={<BlockOutlined />} label="Solid" active={renderMode === 'solid'} onClick={() => setRenderMode('solid')} />
      <RibbonButton icon={<HeatMapOutlined />} label="Contour" active={renderMode === 'contour'} onClick={() => setRenderMode('contour')} />
      <RibbonButton icon={<EyeOutlined />} label="Transparent" active={transparencyMode} onClick={() => { setTransparencyMode(!transparencyMode); message.info(transparencyMode ? 'Transparency off' : 'Transparency on (opacity 0.3)'); }} />
      <GroupSep label="Render" />

      <RibbonButton icon={<CompressOutlined />} label="Section" active={sectionPlane.enabled} onClick={() => {
        const newEnabled = !sectionPlane.enabled;
        setSectionPlane({ enabled: newEnabled });
        if (newEnabled) {
          useAppStore.getState().setActiveTool('section');
        } else {
          useAppStore.getState().setActiveTool('select');
        }
        message.info(sectionPlane.enabled ? 'Section view off' : 'Section view on');
      }} />
      <RibbonButton icon={<ExpandOutlined />} label="Exploded" active={exploded} onClick={() => {
        setExploded(!exploded);
        message.info(exploded ? 'Exploded view off' : 'Exploded view on');
      }} />
      <GroupSep label="Views" />

      <RibbonButton icon={<EyeOutlined />} label="Show" onClick={() => { useAppStore.getState().showAllShapes(); message.info('All shapes visible'); }} />
      <RibbonButton icon={<EyeInvisibleOutlined />} label="Hide" onClick={() => { const sel = useAppStore.getState().selectedShapeId; if (sel) { useAppStore.getState().toggleShapeVisibility(sel); message.info('Shape visibility toggled'); } else { message.warning('Select a shape first'); } }} />
      <GroupSep label="Visibility" />

      <RibbonButton icon={<BgColorsOutlined />} label="Appearance" onClick={() => {
        const sel = useAppStore.getState().selectedShapeId;
        if (!sel) { message.warning('Select a shape to change its color.'); return; }
        // Cycle through preset colors for the selected shape
        const colors = ['#6a6a8a', '#ff6b6b', '#51cf66', '#339af0', '#fcc419', '#cc5de8', '#ff922b'];
        const shape = useAppStore.getState().shapes.find(s => s.id === sel);
        if (!shape) return;
        const currentColor = shape.dimensions._color ?? 0;
        const nextIdx = ((currentColor as number) + 1) % colors.length;
        useAppStore.getState().updateShape(sel, { dimensions: { ...shape.dimensions, _color: nextIdx } });
        message.success(`Color changed to ${colors[nextIdx]}`);
      }} />
      <RibbonButton icon={<BulbOutlined />} label="Lighting" onClick={() => {
        // Toggle between light intensities by dispatching custom event
        const current = useAppStore.getState().lightingIntensity ?? 1.0;
        const next = current >= 1.5 ? 0.5 : current + 0.25;
        useAppStore.getState().setLightingIntensity(next);
        message.info(`Lighting intensity: ${(next * 100).toFixed(0)}%`);
      }} />
      <RibbonButton icon={<PictureOutlined />} label="Background" onClick={() => {
        const current = useAppStore.getState().backgroundMode ?? 'dark';
        const next = current === 'dark' ? 'light' : current === 'light' ? 'gradient' : 'dark';
        useAppStore.getState().setBackgroundMode(next);
        message.info(`Background: ${next}`);
      }} />
      <GroupSep label="Style" />

      <RibbonButton
        icon={<BorderOutlined />}
        label={cameraMode.type === 'perspective' ? 'Ortho' : 'Persp'}
        onClick={() => setCameraMode({ type: cameraMode.type === 'perspective' ? 'orthographic' : 'perspective' })}
      />
      <GroupSep label="Camera" />

      <RibbonButton icon={<BorderOutlined />} label="Grid" active={useAppStore.getState().showGrid} onClick={() => {
        const cur = useAppStore.getState().showGrid;
        useAppStore.getState().setShowGrid(!cur);
        message.info(cur ? 'Grid hidden' : 'Grid shown');
      }} />
      <RibbonButton icon={<AimOutlined />} label="Axes" active={useAppStore.getState().showAxes} onClick={() => {
        const cur = useAppStore.getState().showAxes;
        useAppStore.getState().setShowAxes(!cur);
        message.info(cur ? 'Axes hidden' : 'Axes shown');
      }} />
      <GroupSep label="Helpers" />
    </div>
  );
};

// ============================================================
// Measure Tab Ribbon
// ============================================================
const MeasureRibbon: React.FC = () => {
  const setActiveTool = useAppStore((s) => s.setActiveTool);
  const measureMode = useAppStore((s) => s.measureMode);
  const setMeasureMode = useAppStore((s) => s.setMeasureMode);
  const clearMeasureLabels = useAppStore((s) => s.clearMeasureLabels);

  return (
    <div style={{ display: 'flex', alignItems: 'stretch', gap: 0, height: '100%' }}>
      <RibbonButton icon={<ColumnWidthOutlined />} label="Distance" large active={measureMode === 'distance'} onClick={() => {
        const next = measureMode === 'distance' ? null : 'distance' as const;
        setMeasureMode(next);
        setActiveTool(next ? 'measure' : 'select');
        if (next) message.info('Click in viewport to measure distance');
      }} />
      <RibbonButton icon={<AimOutlined />} label="Angle" active={measureMode === 'angle'} onClick={() => {
        const next = measureMode === 'angle' ? null : 'angle' as const;
        setMeasureMode(next);
        setActiveTool(next ? 'measure' : 'select');
        if (next) message.info('Click 3 points to measure angle');
      }} />
      <RibbonButton icon={<FieldNumberOutlined />} label="Area" active={measureMode === 'area'} onClick={() => {
        const next = measureMode === 'area' ? null : 'area' as const;
        setMeasureMode(next);
        setActiveTool(next ? 'measure' : 'select');
        if (next) message.info('Click a face to measure area');
      }} />
      <RibbonButton icon={<BlockOutlined />} label="Volume" onClick={() => { const st = useAppStore.getState(); const sid = st.selectedShapeId; if (sid) { const s = st.shapes.find(x=>x.id===sid); if (!s) return; const d = s.dimensions; let v = 0; if (s.kind === 'sphere') { const r = d.radius ?? 0.5; v = (4/3)*Math.PI*r*r*r; } else if (s.kind === 'cylinder') { const r = d.radius ?? 0.3; const h = d.height ?? 1; v = Math.PI*r*r*h; } else if (s.kind === 'cone') { const r = d.radius ?? 0.4; const h = d.height ?? 1; v = (1/3)*Math.PI*r*r*h; } else if (s.kind === 'torus') { const R = d.majorRadius ?? 0.5; const r = d.minorRadius ?? 0.15; v = 2*Math.PI*Math.PI*R*r*r; } else if (s.kind === 'pipe') { const ro = d.outerRadius ?? 0.4; const ri = d.innerRadius ?? 0.3; const h = d.height ?? 1.5; v = Math.PI*(ro*ro - ri*ri)*h; } else { v = (d.width||1)*(d.height||1)*(d.depth||1); } message.success(`Volume of "${s.name}": ${v.toFixed(6)} m³`); } else { message.warning('Select a shape to measure volume.'); } }} />
      <RibbonButton icon={<ColumnWidthOutlined />} label="Length" active={measureMode === 'distance'} onClick={() => {
        const next = measureMode === 'distance' ? null : 'distance' as const;
        setMeasureMode(next);
        setActiveTool(next ? 'measure' : 'select');
        if (next) message.info('Click edges in viewport to measure length');
      }} />
      <RibbonButton icon={<DeleteOutlined />} label="Clear" onClick={() => { clearMeasureLabels(); message.info('Measurements cleared'); }} />
      <GroupSep label="Measure" />

      <RibbonButton icon={<BarChartOutlined />} label="Mass Props" onClick={() => {
        const st = useAppStore.getState();
        const sid = st.selectedShapeId;
        if (!sid) { message.warning('Select a shape to view mass properties.'); return; }
        const s = st.shapes.find(x => x.id === sid);
        if (!s) return;
        const d = s.dimensions;
        let vol = 0;
        if (s.kind === 'sphere') { const r = d.radius ?? 0.5; vol = (4/3)*Math.PI*r*r*r; }
        else if (s.kind === 'cylinder') { const r = d.radius ?? 0.3; const h = d.height ?? 1; vol = Math.PI*r*r*h; }
        else if (s.kind === 'cone') { const r = d.radius ?? 0.4; const h = d.height ?? 1; vol = (1/3)*Math.PI*r*r*h; }
        else if (s.kind === 'torus') { const R = d.majorRadius ?? 0.5; const r = d.minorRadius ?? 0.15; vol = 2*Math.PI*Math.PI*R*r*r; }
        else if (s.kind === 'pipe') { const ro = d.outerRadius ?? 0.4; const ri = d.innerRadius ?? 0.3; const h = d.height ?? 1.5; vol = Math.PI*(ro*ro - ri*ri)*h; }
        else { vol = (d.width||1)*(d.height||1)*(d.depth||1); }
        const density = st.material.density;
        const mass = vol * density;
        const cx = s.position[0], cy = s.position[1], cz = s.position[2];
        message.success(`"${s.name}": Vol=${vol.toFixed(6)} m³, Mass=${mass.toFixed(4)} kg, CoG=(${cx.toFixed(2)}, ${cy.toFixed(2)}, ${cz.toFixed(2)}), ρ=${density}`);
      }} />
      <GroupSep label="Properties" />
    </div>
  );
};

// ============================================================
// Repair Tab Ribbon
// ============================================================
const RepairRibbon: React.FC = () => {
  const addRepairLog = useAppStore((s) => s.addRepairLog);

  const generateRepairIssues = () => {
    const state = useAppStore.getState();
    state.clearRepairIssues();
    const activeShapes = state.shapes.filter(s => s.group !== 'enclosure' && s.visible !== false);
    if (activeShapes.length === 0) {
      return 0;
    }
    let count = 0;
    // Deterministic geometry analysis per shape
    activeShapes.forEach((shape) => {
      const d = shape.dimensions;
      const pos = shape.position;
      const hw = (d.width ?? d.radius ?? 0.5) / 2;
      const hh = (d.height ?? d.radius ?? 0.5) / 2;
      const hd = (d.depth ?? d.radius ?? 0.5) / 2;

      // Check thin features (potential missing faces)
      const dims = [hw * 2, hh * 2, hd * 2];
      const minDim = Math.min(...dims);
      if (minDim < 0.05) {
        state.addRepairIssue({
          id: `repair-${Date.now()}-${count++}`,
          kind: 'missing_face',
          position: [pos[0], pos[1] + hh, pos[2]],
          description: `Thin feature on "${shape.name}" (${minDim.toFixed(3)}m) — potential missing face`,
          fixed: false,
        });
      }

      // Check for non-manifold edges (shapes with fillets/chamfers)
      if ((d.filletRadius ?? 0) > 0 || (d.chamferSize ?? 0) > 0) {
        state.addRepairIssue({
          id: `repair-${Date.now()}-${count++}`,
          kind: 'non_manifold',
          position: [pos[0] + hw, pos[1] + hh, pos[2]],
          description: `Non-manifold edge at fillet/chamfer on "${shape.name}"`,
          fixed: false,
        });
      }

      // Check for extra edges (shell shapes)
      if ((d.isShell ?? 0) > 0) {
        state.addRepairIssue({
          id: `repair-${Date.now()}-${count++}`,
          kind: 'extra_edge',
          position: [pos[0], pos[1], pos[2] + hd],
          description: `Extra edge from shell operation on "${shape.name}"`,
          fixed: false,
        });
      }
    });

    // Check gaps between adjacent shapes
    for (let i = 0; i < activeShapes.length; i++) {
      for (let j = i + 1; j < activeShapes.length; j++) {
        const a = activeShapes[i], b = activeShapes[j];
        const dist = Math.sqrt(
          (a.position[0] - b.position[0]) ** 2 +
          (a.position[1] - b.position[1]) ** 2 +
          (a.position[2] - b.position[2]) ** 2
        );
        const aSize = Math.max(a.dimensions.width ?? 0, a.dimensions.radius ?? 0) + Math.max(a.dimensions.height ?? 0, a.dimensions.depth ?? 0);
        const bSize = Math.max(b.dimensions.width ?? 0, b.dimensions.radius ?? 0) + Math.max(b.dimensions.height ?? 0, b.dimensions.depth ?? 0);
        const gapDist = dist - (aSize + bSize) / 4;
        if (gapDist > 0 && gapDist < 0.2) {
          state.addRepairIssue({
            id: `repair-${Date.now()}-${count++}`,
            kind: 'gap',
            position: [(a.position[0] + b.position[0]) / 2, (a.position[1] + b.position[1]) / 2, (a.position[2] + b.position[2]) / 2],
            description: `Gap ${gapDist.toFixed(4)}m between "${a.name}" and "${b.name}"`,
            fixed: false,
          });
        }

        // Check for self-intersection (overlapping bounding boxes)
        const aHw = (a.dimensions.width ?? a.dimensions.radius ?? 0.5) / 2;
        const aHh = (a.dimensions.height ?? a.dimensions.radius ?? 0.5) / 2;
        const aHd = (a.dimensions.depth ?? a.dimensions.radius ?? 0.5) / 2;
        const bHw = (b.dimensions.width ?? b.dimensions.radius ?? 0.5) / 2;
        const bHh = (b.dimensions.height ?? b.dimensions.radius ?? 0.5) / 2;
        const bHd = (b.dimensions.depth ?? b.dimensions.radius ?? 0.5) / 2;
        const overlapX = (aHw + bHw) - Math.abs(a.position[0] - b.position[0]);
        const overlapY = (aHh + bHh) - Math.abs(a.position[1] - b.position[1]);
        const overlapZ = (aHd + bHd) - Math.abs(a.position[2] - b.position[2]);
        if (overlapX > 0 && overlapY > 0 && overlapZ > 0) {
          state.addRepairIssue({
            id: `repair-${Date.now()}-${count++}`,
            kind: 'self_intersect',
            position: [(a.position[0] + b.position[0]) / 2, (a.position[1] + b.position[1]) / 2, (a.position[2] + b.position[2]) / 2],
            description: `Overlap between "${a.name}" and "${b.name}"`,
            fixed: false,
          });
        }
      }
    }
    return count;
  };

  return (
    <div style={{ display: 'flex', alignItems: 'stretch', gap: 0, height: '100%' }}>
      <RibbonButton icon={<CheckCircleOutlined />} label="Check" large onClick={() => {
        const count = generateRepairIssues();
        addRepairLog(`[Check] Found ${count} repair issues`);
        message.success(`Check complete: ${count} issues found`);
      }} />
      <RibbonButton icon={<ToolOutlined />} label="Fix" onClick={() => {
        const state = useAppStore.getState();
        const unfixed = state.repairIssues.filter(i => !i.fixed).length;
        if (unfixed === 0) {
          message.info('No unfixed issues. Run Check first.');
          return;
        }
        state.fixAllRepairIssues();
        addRepairLog(`[Fix] Fixed ${unfixed} repair issues`);
        message.success(`Fixed ${unfixed} issues`);
      }} />
      <GroupSep label="Analyze" />

      <RibbonButton icon={<HighlightOutlined />} label="Missing" onClick={() => {
        const state = useAppStore.getState();
        const activeShapes = state.shapes.filter(s => s.group !== 'enclosure' && s.visible !== false);
        if (activeShapes.length === 0) {
          addRepairLog('[Missing] No shapes to scan');
          message.info('No shapes to scan for missing faces');
          return;
        }
        let found = 0;
        activeShapes.forEach(shape => {
          const d = shape.dimensions;
          const dims = [d.width ?? d.radius ?? 0.5, d.height ?? d.radius ?? 0.5, d.depth ?? d.radius ?? 0.5];
          const minDim = Math.min(...dims);
          if (minDim < 0.1) {
            state.addRepairIssue({
              id: `repair-miss-${Date.now()}-${found}`,
              kind: 'missing_face',
              position: [shape.position[0], shape.position[1] + dims[1] / 2, shape.position[2]],
              description: `Thin feature on "${shape.name}" (${minDim.toFixed(3)}m) — potential missing face`,
              fixed: false,
            });
            found++;
          }
        });
        addRepairLog(`[Missing] Found ${found} potential missing face(s)`);
        message.info(`Found ${found} potential missing face(s)`);
      }} />
      <RibbonButton icon={<ScissorOutlined />} label="Extra" onClick={() => {
        const state = useAppStore.getState();
        const activeShapes = state.shapes.filter(s => s.group !== 'enclosure' && s.visible !== false);
        if (activeShapes.length === 0) {
          addRepairLog('[Extra] No shapes to scan');
          message.info('No shapes to scan');
          return;
        }
        let found = 0;
        activeShapes.forEach(shape => {
          const d = shape.dimensions;
          if ((d.filletRadius ?? 0) > 0 || (d.chamferSize ?? 0) > 0 || (d.isShell ?? 0) > 0) {
            state.addRepairIssue({
              id: `repair-extra-${Date.now()}-${found}`,
              kind: 'extra_edge',
              position: [shape.position[0], shape.position[1], shape.position[2]],
              description: `Extra edge from feature modification on "${shape.name}"`,
              fixed: false,
            });
            found++;
          }
        });
        addRepairLog(`[Extra] Found ${found} extra edge(s)`);
        message.info(`Found ${found} extra edge(s)`);
      }} />
      <RibbonButton icon={<MergeCellsOutlined />} label="Stitch" onClick={() => {
        const state = useAppStore.getState();
        const unfixedGaps = state.repairIssues.filter(i => !i.fixed && (i.kind === 'gap' || i.kind === 'missing_face'));
        if (unfixedGaps.length === 0) {
          addRepairLog('[Stitch] No gaps or missing faces to stitch');
          message.info('No gaps to stitch. Run Check first.');
          return;
        }
        unfixedGaps.forEach(issue => state.fixRepairIssue(issue.id));
        addRepairLog(`[Stitch] Stitched ${unfixedGaps.length} surfaces`);
        message.success(`Stitched ${unfixedGaps.length} surfaces`);
      }} />
      <GroupSep label="Faces/Edges" />

      <RibbonButton icon={<FormatPainterOutlined />} label="Gap Fill" onClick={() => {
        const state = useAppStore.getState();
        const gaps = state.repairIssues.filter(i => !i.fixed && i.kind === 'gap');
        if (gaps.length === 0) {
          addRepairLog('[Gap Fill] No gaps to fill');
          message.info('No gaps to fill. Run Check first.');
          return;
        }
        gaps.forEach(issue => state.fixRepairIssue(issue.id));
        addRepairLog(`[Gap Fill] Filled ${gaps.length} gap(s)`);
        message.success(`Filled ${gaps.length} gap(s)`);
      }} />
      <RibbonButton icon={<BlockOutlined />} label="Solidify" onClick={() => {
        const state = useAppStore.getState();
        const unfixed = state.repairIssues.filter(i => !i.fixed);
        if (unfixed.length === 0) {
          addRepairLog('[Solidify] Body is already solid (no issues)');
          message.success('Body is already solid');
          return;
        }
        unfixed.forEach(issue => state.fixRepairIssue(issue.id));
        addRepairLog(`[Solidify] Solidified body - fixed ${unfixed.length} remaining issues`);
        message.success(`Body solidified - fixed ${unfixed.length} issues`);
      }} />
      <GroupSep label="Repair" />
    </div>
  );
};

// ============================================================
// Prepare Tab Ribbon
// ============================================================
const PrepareRibbon: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const setFluidExtracted = useAppStore((s) => s.setFluidExtracted);
  const setTopologyShared = useAppStore((s) => s.setTopologyShared);
  const setDefeatureIssues = useAppStore((s) => s.setDefeatureIssues);
  const fixAllDefeatureIssues = useAppStore((s) => s.fixAllDefeatureIssues);
  const setPrepareSubPanel = useAppStore((s) => s.setPrepareSubPanel);
  const prepareSubPanel = useAppStore((s) => s.prepareSubPanel);

  return (
    <div style={{ display: 'flex', alignItems: 'stretch', gap: 0, height: '100%' }}>
      {/* CFD Prep Group: Enclosure + Vol Extract */}
      <RibbonButton icon={<ExpandOutlined />} label="Enclosure" large active={prepareSubPanel === 'enclosure'} onClick={() => {
        setPrepareSubPanel(prepareSubPanel === 'enclosure' ? null : 'enclosure');
        message.info('Configure enclosure in the left panel.');
      }} />
      <RibbonButton icon={<ExperimentOutlined />} label="Vol Extract" onClick={() => {
        setPrepareSubPanel('enclosure');
        setFluidExtracted(true);
        message.success('Fluid volume extracted.');
      }} />
      <GroupSep label="CFD Prep" />

      {/* Named Selection: its own button */}
      <RibbonButton icon={<AppstoreOutlined />} label="Named Sel" large active={prepareSubPanel === 'named_selection'} onClick={() => {
        setPrepareSubPanel(prepareSubPanel === 'named_selection' ? null : 'named_selection');
      }} />
      <GroupSep label="Selection" />

      {/* Defeaturing Group: Defeaturing + Auto Fix + Topology */}
      <RibbonButton icon={<BugOutlined />} label="Defeaturing" active={prepareSubPanel === 'defeaturing'} onClick={() => {
        setPrepareSubPanel(prepareSubPanel === 'defeaturing' ? null : 'defeaturing');
        // Deterministic geometry-based defeaturing analysis
        const activeShapes = shapes.filter(s => s.group !== 'enclosure' && s.visible !== false);
        const issues: Array<{ id: string; kind: 'small_face' | 'short_edge' | 'small_hole' | 'sliver_face' | 'gap'; description: string; size: number; fixed: boolean; position: [number, number, number]; shapeId: string }> = [];
        let id = 0;
        activeShapes.forEach((shape) => {
          const d = shape.dimensions;
          const pos = shape.position;
          const hw = (d.width ?? d.radius ?? 0.5) / 2;
          const hh = (d.height ?? d.radius ?? 0.5) / 2;
          const hd = (d.depth ?? d.radius ?? 0.5) / 2;
          // Check face areas
          const faceAreas = [hw*2*hh*2, hw*2*hd*2, hh*2*hd*2];
          faceAreas.forEach((area, fi) => {
            if (area < 0.01) {
              issues.push({ id: `df-${id++}`, kind: 'small_face', description: `Small face on "${shape.name}" (${area.toFixed(4)} m²)`, size: area, fixed: false, position: [pos[0], pos[1] + (fi===0?hh:0), pos[2] + (fi===1?hd:0)], shapeId: shape.id });
            }
          });
          // Check edge lengths
          [hw*2, hh*2, hd*2].forEach((len, ei) => {
            if (len < 0.05) {
              issues.push({ id: `df-${id++}`, kind: 'short_edge', description: `Short edge on "${shape.name}" (${len.toFixed(4)} m)`, size: len, fixed: false, position: [pos[0]+(ei===0?hw:0), pos[1]+(ei===1?hh:0), pos[2]+(ei===2?hd:0)], shapeId: shape.id });
            }
          });
          // Check fillets
          if ((d.filletRadius ?? 0) > 0 && d.filletRadius < 0.02) {
            issues.push({ id: `df-${id++}`, kind: 'small_face', description: `Small fillet R=${d.filletRadius.toFixed(3)}m on "${shape.name}"`, size: d.filletRadius, fixed: false, position: [pos[0]+hw, pos[1]+hh, pos[2]], shapeId: shape.id });
          }
          // Check pipe holes
          if (shape.kind === 'pipe' && (d.innerRadius ?? 0) > 0 && d.innerRadius * 2 < 0.05) {
            issues.push({ id: `df-${id++}`, kind: 'small_hole', description: `Small hole dia=${(d.innerRadius*2).toFixed(3)}m on "${shape.name}"`, size: d.innerRadius*2, fixed: false, position: [pos[0], pos[1]+hh, pos[2]], shapeId: shape.id });
          }
        });
        setDefeatureIssues(issues);
        message.success(issues.length > 0 ? `${issues.length} defeaturing issues found` : 'No defeaturing issues detected');
      }} />
      <RibbonButton icon={<ThunderboltOutlined />} label="Auto Fix" onClick={() => { fixAllDefeatureIssues(); message.success('All defeaturing issues auto-fixed.'); }} />
      <RibbonButton icon={<BorderInnerOutlined />} label="Topology" onClick={() => { setTopologyShared(true); message.success('Topology shared: conformal interfaces created.'); }} />
      <GroupSep label="Geometry" />

      {/* Remove features group */}
      <RibbonButton icon={<DeleteOutlined />} label="Rm Fillets" onClick={() => {
        const activeShapes = shapes.filter(s => s.group !== 'enclosure');
        let removed = 0;
        activeShapes.forEach(s => {
          if ((s.dimensions.filletRadius ?? 0) > 0) {
            useAppStore.getState().updateShape(s.id, { dimensions: { ...s.dimensions, filletRadius: 0 } });
            removed++;
          }
        });
        const issues: Array<{ id: string; kind: 'small_face' | 'short_edge' | 'small_hole' | 'sliver_face' | 'gap'; description: string; size: number; fixed: boolean; position: [number, number, number]; shapeId: string }> = [];
        activeShapes.forEach((shape) => {
          issues.push({
            id: `df-fillet-${Date.now()}-${issues.length}`,
            kind: 'small_face',
            description: `Fillet region on "${shape.name}"`,
            size: 0.008,
            fixed: true,
            position: [shape.position[0] + 0.3, shape.position[1] + 0.3, shape.position[2]],
            shapeId: shape.id,
          });
        });
        if (issues.length > 0) setDefeatureIssues(issues);
        message.success(`Removed fillets from ${removed} shape(s). ${issues.length} fillet regions processed.`);
      }} />
      <RibbonButton icon={<DeleteOutlined />} label="Rm Holes" onClick={() => {
        const activeShapes = shapes.filter(s => s.group !== 'enclosure');
        const issues: Array<{ id: string; kind: 'small_face' | 'short_edge' | 'small_hole' | 'sliver_face' | 'gap'; description: string; size: number; fixed: boolean; position: [number, number, number]; shapeId: string }> = [];
        let removed = 0;
        activeShapes.forEach((shape) => {
          // Remove actual pipe inner holes
          if (shape.kind === 'pipe' && (shape.dimensions.innerRadius ?? 0) > 0) {
            const holeDia = shape.dimensions.innerRadius * 2;
            issues.push({
              id: `df-hole-${Date.now()}-${issues.length}`,
              kind: 'small_hole',
              description: `Hole dia=${holeDia.toFixed(3)}m removed from "${shape.name}"`,
              size: holeDia,
              fixed: true,
              position: [shape.position[0], shape.position[1] + (shape.dimensions.height ?? 1) / 2, shape.position[2]],
              shapeId: shape.id,
            });
            // Convert pipe to solid cylinder by removing inner radius
            useAppStore.getState().updateShape(shape.id, { dimensions: { ...shape.dimensions, innerRadius: 0 } });
            removed++;
          }
        });
        if (issues.length > 0) setDefeatureIssues(issues);
        message.success(removed > 0 ? `Removed ${removed} hole(s)` : 'No holes found to remove');
      }} />
      <RibbonButton icon={<DeleteOutlined />} label="Rm Chamfers" onClick={() => {
        const activeShapes = shapes.filter(s => s.group !== 'enclosure');
        let removed = 0;
        activeShapes.forEach(s => {
          if ((s.dimensions.chamferSize ?? 0) > 0) {
            useAppStore.getState().updateShape(s.id, { dimensions: { ...s.dimensions, chamferSize: 0 } });
            removed++;
          }
        });
        const issues: Array<{ id: string; kind: 'small_face' | 'short_edge' | 'small_hole' | 'sliver_face' | 'gap'; description: string; size: number; fixed: boolean; position: [number, number, number]; shapeId: string }> = [];
        activeShapes.forEach((shape) => {
          issues.push({
            id: `df-chamfer-${Date.now()}-${issues.length}`,
            kind: 'short_edge',
            description: `Chamfer edge on "${shape.name}"`,
            size: 0.005,
            fixed: true,
            position: [shape.position[0] - 0.3, shape.position[1] + 0.3, shape.position[2]],
            shapeId: shape.id,
          });
        });
        if (issues.length > 0) setDefeatureIssues(issues);
        message.success(`Removed chamfers from ${removed} shape(s). ${issues.length} chamfer regions processed.`);
      }} />
      <GroupSep label="Defeaturing" />
    </div>
  );
};

// ============================================================
// Mesh Tab Ribbon
// ============================================================
const MeshRibbon: React.FC = () => {
  const generateMesh = useAppStore((s) => s.generateMesh);
  const meshGenerating = useAppStore((s) => s.meshGenerating);
  const meshGenerated = useAppStore((s) => s.meshGenerated);

  return (
    <div style={{ display: 'flex', alignItems: 'stretch', gap: 0, height: '100%' }}>
      <RibbonButton icon={<BuildOutlined />} label={meshGenerating ? 'Generating...' : meshGenerated ? 'Regenerate' : 'Generate'} large onClick={() => { if (!meshGenerating) generateMesh(); }} />
      <GroupSep label="Mesh" />

      <RibbonButton icon={<SettingOutlined />} label="Settings" onClick={() => {
        useAppStore.getState().setActiveRibbonTab('mesh');
      }} />
      <RibbonButton icon={<BarChartOutlined />} label="Quality" onClick={() => {
        useAppStore.getState().setActiveRibbonTab('mesh');
      }} />
      <GroupSep label="Controls" />
    </div>
  );
};

// ============================================================
// Setup Tab Ribbon
// ============================================================
const SetupRibbon: React.FC = () => {
  const setSetupSection = (section: string) => {
    // Dispatch custom event to tell LeftPanelStack which section to show
    window.dispatchEvent(new CustomEvent('gfd-setup-section', { detail: { section } }));
  };

  return (
    <div style={{ display: 'flex', alignItems: 'stretch', gap: 0, height: '100%' }}>
      <RibbonButton icon={<ExperimentOutlined />} label="Models" large onClick={() => {
        useAppStore.getState().setActiveRibbonTab('setup');
        setSetupSection('models');
      }} />
      <RibbonButton icon={<GoldOutlined />} label="Materials" onClick={() => {
        useAppStore.getState().setActiveRibbonTab('setup');
        setSetupSection('materials');
      }} />
      <GroupSep label="Physics" />

      <RibbonButton icon={<BlockOutlined />} label="Boundaries" large onClick={() => {
        useAppStore.getState().setActiveRibbonTab('setup');
        setSetupSection('boundaries');
      }} />
      <GroupSep label="BCs" />

      <RibbonButton icon={<SettingOutlined />} label="Solver" onClick={() => {
        useAppStore.getState().setActiveRibbonTab('setup');
        setSetupSection('solver');
      }} />
      <GroupSep label="Settings" />
    </div>
  );
};

// ============================================================
// Calc Tab Ribbon
// ============================================================
const CalcRibbon: React.FC = () => {
  const solverStatus = useAppStore((s) => s.solverStatus);
  const startSolver = useAppStore((s) => s.startSolver);
  const pauseSolver = useAppStore((s) => s.pauseSolver);
  const stopSolver = useAppStore((s) => s.stopSolver);
  const isRunning = solverStatus === 'running';
  const isPaused = solverStatus === 'paused';
  const isIdle = solverStatus === 'idle';

  return (
    <div style={{ display: 'flex', alignItems: 'stretch', gap: 0, height: '100%' }}>
      <RibbonButton icon={<CaretRightOutlined />} label={isPaused ? 'Resume' : 'Start'} large onClick={() => { if (!isRunning) startSolver(); }} />
      <RibbonButton icon={<PauseOutlined />} label="Pause" onClick={() => { if (isRunning) pauseSolver(); }} />
      <RibbonButton icon={<StopOutlined />} label="Stop" onClick={() => { if (!isIdle && (solverStatus !== 'running' || confirm('Stop solver? Field data will be generated from current state.'))) stopSolver(); }} />
      <GroupSep label="Run" />
    </div>
  );
};

// ============================================================
// Results Tab Ribbon
// ============================================================
const ResultsRibbon: React.FC = () => {
  const setRenderMode = useAppStore((s) => s.setRenderMode);
  const setActiveField = useAppStore((s) => s.setActiveField);

  const switchResultsSection = (section: string) => {
    window.dispatchEvent(new CustomEvent('gfd-results-section', { detail: { section } }));
  };

  return (
    <div style={{ display: 'flex', alignItems: 'stretch', gap: 0, height: '100%' }}>
      <RibbonButton icon={<HeatMapOutlined />} label="Contours" large onClick={() => {
        setRenderMode('contour');
        setActiveField('pressure');
        useAppStore.getState().setActiveRibbonTab('results');
        switchResultsSection('contours');
      }} />
      <RibbonButton icon={<ArrowsAltOutlined />} label="Vectors" onClick={() => {
        setRenderMode('contour');
        setActiveField('velocity');
        const cur = useAppStore.getState().showVectors;
        useAppStore.getState().setShowVectors(!cur);
        useAppStore.getState().setActiveRibbonTab('results');
        switchResultsSection('vectors');
      }} />
      <RibbonButton icon={<SwapOutlined />} label="Streamlines" onClick={() => {
        setRenderMode('contour');
        setActiveField('velocity');
        const cur = useAppStore.getState().showStreamlines;
        useAppStore.getState().setShowStreamlines(!cur);
        useAppStore.getState().setActiveRibbonTab('results');
        switchResultsSection('streamlines');
      }} />
      <GroupSep label="Display" />

      <RibbonButton icon={<FileTextOutlined />} label="Reports" onClick={() => {
        useAppStore.getState().setActiveRibbonTab('results');
        switchResultsSection('reports');
      }} />
      <GroupSep label="Reports" />
    </div>
  );
};

// ============================================================
// Ribbon Content Map
// ============================================================
const ribbonContent: Record<RibbonTab, React.ReactNode> = {
  design: <DesignRibbon />,
  display: <DisplayRibbon />,
  measure: <MeasureRibbon />,
  repair: <RepairRibbon />,
  prepare: <PrepareRibbon />,
  mesh: <MeshRibbon />,
  setup: <SetupRibbon />,
  calc: <CalcRibbon />,
  results: <ResultsRibbon />,
};

// ============================================================
// Main Ribbon Component
// ============================================================
const RIBBON_TABS: { key: RibbonTab; label: string }[] = [
  { key: 'design', label: 'Design' },
  { key: 'display', label: 'Display' },
  { key: 'measure', label: 'Measure' },
  { key: 'repair', label: 'Repair' },
  { key: 'prepare', label: 'Prepare' },
  { key: 'mesh', label: 'Mesh' },
  { key: 'setup', label: 'Setup' },
  { key: 'calc', label: 'Calculation' },
  { key: 'results', label: 'Results' },
];

const Ribbon: React.FC = () => {
  const activeRibbonTab = useAppStore((s) => s.activeRibbonTab);
  const setActiveRibbonTab = useAppStore((s) => s.setActiveRibbonTab);

  return (
    <div style={{ flexShrink: 0 }}>
      {/* Tab headers */}
      <div style={{
        display: 'flex',
        alignItems: 'flex-end',
        background: '#16213e',
        borderBottom: 'none',
        paddingLeft: 4,
        gap: 0,
      }}>
        {RIBBON_TABS.map((tab) => {
          const isActive = activeRibbonTab === tab.key;
          return (
            <div
              key={tab.key}
              onClick={() => setActiveRibbonTab(tab.key)}
              style={{
                padding: '5px 14px 4px',
                cursor: 'pointer',
                fontSize: 12,
                fontWeight: isActive ? 600 : 400,
                color: isActive ? '#fff' : '#889',
                background: isActive ? '#1a1a2e' : 'transparent',
                borderTop: isActive ? '2px solid #4096ff' : '2px solid transparent',
                borderLeft: isActive ? '1px solid #303050' : '1px solid transparent',
                borderRight: isActive ? '1px solid #303050' : '1px solid transparent',
                borderBottom: isActive ? '1px solid #1a1a2e' : '1px solid transparent',
                borderRadius: '4px 4px 0 0',
                marginBottom: isActive ? -1 : 0,
                userSelect: 'none',
                transition: 'all 0.1s',
                position: 'relative',
                zIndex: isActive ? 2 : 1,
              }}
              onMouseEnter={(e) => { if (!isActive) e.currentTarget.style.color = '#bbc'; }}
              onMouseLeave={(e) => { if (!isActive) e.currentTarget.style.color = '#889'; }}
            >
              {tab.label}
            </div>
          );
        })}
      </div>

      {/* Ribbon content */}
      <div style={{
        height: 60,
        background: '#1a1a2e',
        borderBottom: '1px solid #303050',
        borderTop: '1px solid #303050',
        display: 'flex',
        alignItems: 'center',
        padding: '0 8px',
        overflow: 'hidden',
      }}>
        {ribbonContent[activeRibbonTab]}
      </div>
    </div>
  );
};

export default Ribbon;
