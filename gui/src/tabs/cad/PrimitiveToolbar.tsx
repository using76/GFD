import React, { useCallback } from 'react';
import { Button, Space, Upload, Dropdown, message } from 'antd';
import type { MenuProps } from 'antd';
import {
  BorderOutlined,
  RadiusSettingOutlined,
  ColumnHeightOutlined,
  ImportOutlined,
  PlusCircleOutlined,
  MinusCircleOutlined,
  BlockOutlined,
  SlidersOutlined,
  ScissorOutlined,
  ToolOutlined,
  ExperimentOutlined,
  AimOutlined,
  DownOutlined,
  RetweetOutlined,
  SwapOutlined,
  DragOutlined,
  GroupOutlined,
  CompressOutlined,
  ExpandOutlined,
  GatewayOutlined,
  InteractionOutlined,
  SplitCellsOutlined,
  BugOutlined,
  ThunderboltOutlined,
  BorderInnerOutlined,
  AppstoreOutlined,
  HighlightOutlined,
  PartitionOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../../store/useAppStore';
import type { ShapeKind, BooleanOp } from '../../store/useAppStore';

let nextId = 1;

function makeShape(kind: ShapeKind, extraName?: string) {
  const id = `shape-${nextId++}`;
  const defaults: Record<string, Record<string, number>> = {
    box: { width: 1, height: 1, depth: 1 },
    sphere: { radius: 0.5 },
    cylinder: { radius: 0.3, height: 1 },
    cone: { radius: 0.4, height: 1 },
    torus: { majorRadius: 0.5, minorRadius: 0.15 },
    pipe: { outerRadius: 0.4, innerRadius: 0.3, height: 1.5 },
  };
  const group = kind === 'enclosure' ? 'enclosure' as const : 'body' as const;
  return {
    id,
    name: extraName ?? `${kind}-${id}`,
    kind,
    position: [0, 0, 0] as [number, number, number],
    rotation: [0, 0, 0] as [number, number, number],
    dimensions: { ...(defaults[kind] ?? {}) },
    group,
  };
}

/** Parse ASCII STL text into a flat Float32Array of vertex positions and face count. */
function parseStlAscii(text: string): { vertices: Float32Array; faceCount: number } {
  const vertexRegex = /vertex\s+([-\d.eE+]+)\s+([-\d.eE+]+)\s+([-\d.eE+]+)/gi;
  const verts: number[] = [];
  let match: RegExpExecArray | null;
  while ((match = vertexRegex.exec(text)) !== null) {
    verts.push(parseFloat(match[1]), parseFloat(match[2]), parseFloat(match[3]));
  }
  return {
    vertices: new Float32Array(verts),
    faceCount: Math.floor(verts.length / 9),
  };
}

/** Parse binary STL buffer into a flat Float32Array of vertex positions and face count. */
function parseStlBinary(buffer: ArrayBuffer): { vertices: Float32Array; faceCount: number } {
  const dv = new DataView(buffer);
  const faceCount = dv.getUint32(80, true);
  const verts = new Float32Array(faceCount * 9);
  let offset = 84;
  for (let i = 0; i < faceCount; i++) {
    // skip normal (12 bytes)
    offset += 12;
    for (let v = 0; v < 3; v++) {
      verts[i * 9 + v * 3] = dv.getFloat32(offset, true);
      verts[i * 9 + v * 3 + 1] = dv.getFloat32(offset + 4, true);
      verts[i * 9 + v * 3 + 2] = dv.getFloat32(offset + 8, true);
      offset += 12;
    }
    // skip attribute byte count
    offset += 2;
  }
  return { vertices: verts, faceCount };
}

const PrimitiveToolbar: React.FC = () => {
  const addShape = useAppStore((s) => s.addShape);
  const shapes = useAppStore((s) => s.shapes);
  const selectedShapeId = useAppStore((s) => s.selectedShapeId);
  const setCadMode = useAppStore((s) => s.setCadMode);
  const setPendingBooleanOp = useAppStore((s) => s.setPendingBooleanOp);
  const cadMode = useAppStore((s) => s.cadMode);

  const create = useCallback(
    (kind: ShapeKind) => {
      const shape = makeShape(kind);
      addShape(shape);
    },
    [addShape]
  );

  const handleStlUpload = useCallback(
    (file: File) => {
      const reader = new FileReader();
      reader.onload = (e) => {
        const result = e.target?.result;
        if (!result) return;

        let parsed: { vertices: Float32Array; faceCount: number };
        // Try to detect if ASCII or binary
        if (typeof result === 'string') {
          parsed = parseStlAscii(result);
        } else {
          // Check if ASCII by looking at first bytes
          const headerBytes = new Uint8Array(result as ArrayBuffer, 0, 5);
          const headerStr = String.fromCharCode(...headerBytes);
          if (headerStr === 'solid') {
            // Might be ASCII, try text decode
            const text = new TextDecoder().decode(result as ArrayBuffer);
            if (text.includes('vertex')) {
              parsed = parseStlAscii(text);
            } else {
              parsed = parseStlBinary(result as ArrayBuffer);
            }
          } else {
            parsed = parseStlBinary(result as ArrayBuffer);
          }
        }

        if (parsed.faceCount === 0) {
          message.error('Could not parse STL file: no triangles found.');
          return;
        }

        const id = `shape-${nextId++}`;
        addShape({
          id,
          name: file.name.replace(/\.stl$/i, ''),
          kind: 'stl',
          position: [0, 0, 0],
          rotation: [0, 0, 0],
          dimensions: {},
          stlData: {
            vertices: parsed.vertices,
            faceCount: parsed.faceCount,
          },
          group: 'body',
        });
        message.success(`Imported ${file.name} (${parsed.faceCount} triangles)`);
      };
      reader.readAsArrayBuffer(file);
      return false; // prevent Ant Upload from auto-uploading
    },
    [addShape]
  );

  const startBoolean = useCallback(
    (op: BooleanOp) => {
      if (shapes.filter((s) => s.group !== 'enclosure').length < 2) {
        message.warning('Boolean operations require at least 2 shapes.');
        return;
      }
      if (selectedShapeId) {
        // Use selected shape as target, ask for tool
        setCadMode('boolean_select_tool');
        setPendingBooleanOp(op);
        useAppStore.getState().setPendingBooleanTargetId(selectedShapeId);
        message.info(`Click the tool shape to ${op} with "${shapes.find((s) => s.id === selectedShapeId)?.name}".`);
      } else {
        setCadMode('boolean_select_target');
        setPendingBooleanOp(op);
        message.info(`Select the TARGET shape first, then the TOOL shape for ${op}.`);
      }
    },
    [shapes, selectedShapeId, setCadMode, setPendingBooleanOp]
  );

  const cancelBoolean = useCallback(() => {
    setCadMode('select');
    setPendingBooleanOp(null);
    useAppStore.getState().setPendingBooleanTargetId(null);
    message.info('Boolean operation cancelled.');
  }, [setCadMode, setPendingBooleanOp]);

  // ---- Create dropdown ----
  const createMenuItems: MenuProps['items'] = [
    { key: 'box', icon: <BorderOutlined />, label: 'Box', onClick: () => create('box') },
    { key: 'sphere', icon: <RadiusSettingOutlined />, label: 'Sphere', onClick: () => create('sphere') },
    { key: 'cylinder', icon: <ColumnHeightOutlined />, label: 'Cylinder', onClick: () => create('cylinder') },
    { key: 'cone', icon: <AimOutlined />, label: 'Cone', onClick: () => create('cone') },
    { key: 'torus', icon: <RetweetOutlined />, label: 'Torus', onClick: () => create('torus') },
    { key: 'pipe', icon: <GatewayOutlined />, label: 'Pipe', onClick: () => create('pipe') },
  ];

  // ---- Sketch dropdown ----
  const sketchMenuItems: MenuProps['items'] = [
    {
      key: 'extrude',
      icon: <DragOutlined />,
      label: 'Extrude',
      onClick: () => {
        useAppStore.getState().setActiveTool('pull');
        message.info('Extrude: Select a face and use Pull tool to extrude.');
      },
    },
    {
      key: 'revolve',
      icon: <RetweetOutlined />,
      label: 'Revolve',
      onClick: () => {
        if (!selectedShapeId) { message.warning('Select a shape to revolve.'); return; }
        const shape = shapes.find(s => s.id === selectedShapeId);
        if (!shape) return;
        const rev = makeShape('torus', `${shape.name}-revolve`);
        rev.position = [...shape.position] as [number, number, number];
        addShape(rev);
        message.success(`Revolved "${shape.name}" into torus.`);
      },
    },
    {
      key: 'sweep',
      icon: <SwapOutlined />,
      label: 'Sweep',
      onClick: () => {
        if (!selectedShapeId) { message.warning('Select a shape to sweep.'); return; }
        const shape = shapes.find(s => s.id === selectedShapeId);
        if (!shape) return;
        const swept = makeShape('cylinder', `${shape.name}-sweep`);
        swept.position = [...shape.position] as [number, number, number];
        swept.dimensions = { radius: (shape.dimensions.radius || shape.dimensions.width || 0.3) * 0.5, height: 2 };
        addShape(swept);
        message.success(`Swept "${shape.name}" along axis.`);
      },
    },
    {
      key: 'loft',
      icon: <GroupOutlined />,
      label: 'Loft',
      onClick: () => {
        if (!selectedShapeId) { message.warning('Select a shape to loft.'); return; }
        const shape = shapes.find(s => s.id === selectedShapeId);
        if (!shape) return;
        const lofted = makeShape('cone', `${shape.name}-loft`);
        lofted.position = [...shape.position] as [number, number, number];
        lofted.dimensions = { radius: shape.dimensions.radius || shape.dimensions.width || 0.4, height: 1.5 };
        addShape(lofted);
        message.success(`Lofted "${shape.name}" into cone.`);
      },
    },
  ];

  // ---- Edit dropdown ----
  const editMenuItems: MenuProps['items'] = [
    {
      key: 'mirror',
      icon: <SwapOutlined />,
      label: 'Mirror',
      onClick: () => {
        if (!selectedShapeId) {
          message.warning('Select a shape first to mirror.');
          return;
        }
        const shape = shapes.find((s) => s.id === selectedShapeId);
        if (!shape) return;
        const mirrored = makeShape(shape.kind, `${shape.name}-mirror`);
        mirrored.dimensions = { ...shape.dimensions };
        mirrored.position = [-shape.position[0], shape.position[1], shape.position[2]];
        mirrored.rotation = [...shape.rotation] as [number, number, number];
        addShape(mirrored);
        message.success(`Mirrored "${shape.name}" across YZ plane.`);
      },
    },
    {
      key: 'pattern_linear',
      icon: <PartitionOutlined />,
      label: 'Linear Pattern',
      onClick: () => {
        if (!selectedShapeId) {
          message.warning('Select a shape first.');
          return;
        }
        const shape = shapes.find((s) => s.id === selectedShapeId);
        if (!shape) return;
        for (let i = 1; i <= 3; i++) {
          const clone = makeShape(shape.kind, `${shape.name}-lp${i}`);
          clone.dimensions = { ...shape.dimensions };
          clone.position = [
            shape.position[0] + i * 1.5,
            shape.position[1],
            shape.position[2],
          ];
          clone.rotation = [...shape.rotation] as [number, number, number];
          addShape(clone);
        }
        message.success(`Created linear pattern of 3 copies from "${shape.name}".`);
      },
    },
    {
      key: 'pattern_circular',
      icon: <RetweetOutlined />,
      label: 'Circular Pattern',
      onClick: () => {
        if (!selectedShapeId) {
          message.warning('Select a shape first.');
          return;
        }
        const shape = shapes.find((s) => s.id === selectedShapeId);
        if (!shape) return;
        const count = 5;
        for (let i = 1; i <= count; i++) {
          const angle = (i * 2 * Math.PI) / (count + 1);
          const r = 2.0;
          const clone = makeShape(shape.kind, `${shape.name}-cp${i}`);
          clone.dimensions = { ...shape.dimensions };
          clone.position = [
            shape.position[0] + r * Math.cos(angle),
            shape.position[1],
            shape.position[2] + r * Math.sin(angle),
          ];
          clone.rotation = [...shape.rotation] as [number, number, number];
          addShape(clone);
        }
        message.success(`Created circular pattern of ${count} copies from "${shape.name}".`);
      },
    },
    {
      key: 'shell',
      icon: <CompressOutlined />,
      label: 'Shell',
      onClick: () => {
        if (!selectedShapeId) {
          message.warning('Select a shape first to shell.');
          return;
        }
        const shape = shapes.find(s => s.id === selectedShapeId);
        if (!shape) return;
        const isShell = shape.dimensions.isShell ?? 0;
        if (isShell) {
          useAppStore.getState().updateShape(selectedShapeId, { dimensions: { ...shape.dimensions, isShell: 0, shellThickness: 0 } });
          message.success(`Removed shell from "${shape.name}".`);
        } else {
          const thickness = 0.05;
          useAppStore.getState().updateShape(selectedShapeId, { dimensions: { ...shape.dimensions, isShell: 1, shellThickness: thickness } });
          message.success(`Applied shell (thickness=${thickness}) to "${shape.name}".`);
        }
      },
    },
  ];

  // ---- Boolean dropdown ----
  const booleanMenuItems: MenuProps['items'] = [
    {
      key: 'union',
      icon: <PlusCircleOutlined />,
      label: 'Union',
      onClick: () => startBoolean('union'),
    },
    {
      key: 'subtract',
      icon: <MinusCircleOutlined />,
      label: 'Subtract',
      onClick: () => startBoolean('subtract'),
    },
    {
      key: 'intersect',
      icon: <InteractionOutlined />,
      label: 'Intersect',
      onClick: () => startBoolean('intersect'),
    },
    {
      key: 'split',
      icon: <SplitCellsOutlined />,
      label: 'Split',
      onClick: () => startBoolean('split'),
    },
  ];

  // ---- Defeaturing dropdown ----
  const defeaturingMenuItems: MenuProps['items'] = [
    {
      key: 'analyze',
      icon: <BugOutlined />,
      label: 'Analyze',
      onClick: () => {
        // Simulate defeaturing analysis with 3D positions
        const issues = [
          { id: 'df-1', kind: 'small_face' as const, description: 'Face area 0.001 mm^2 on box-shape-1', size: 0.001, fixed: false, position: [0.5, 0.3, 0.1] as [number, number, number], shapeId: 'shape-1' },
          { id: 'df-2', kind: 'short_edge' as const, description: 'Edge length 0.05 mm on cylinder-shape-2', size: 0.05, fixed: false, position: [-0.3, 0.5, 0.2] as [number, number, number], shapeId: 'shape-2' },
          { id: 'df-3', kind: 'small_hole' as const, description: 'Hole diameter 0.2 mm on box-shape-1', size: 0.2, fixed: false, position: [0.4, -0.2, 0.5] as [number, number, number], shapeId: 'shape-1' },
          { id: 'df-4', kind: 'sliver_face' as const, description: 'Sliver face AR=50 on sphere-shape-3', size: 50, fixed: false, position: [-0.1, 0.4, -0.3] as [number, number, number], shapeId: 'shape-3' },
          { id: 'df-5', kind: 'gap' as const, description: 'Gap 0.01 mm between box-shape-1 and cylinder-shape-2', size: 0.01, fixed: false, position: [0.6, 0.0, 0.0] as [number, number, number], shapeId: 'shape-1' },
        ];
        useAppStore.getState().setDefeatureIssues(issues);
        message.success(`Defeaturing analysis complete: ${issues.length} issues found.`);
      },
    },
    {
      key: 'autofix',
      icon: <ThunderboltOutlined />,
      label: 'Auto Fix All',
      onClick: () => {
        useAppStore.getState().fixAllDefeatureIssues();
        message.success('All defeaturing issues fixed.');
      },
    },
    {
      key: 'remove_small_faces',
      icon: <ScissorOutlined />,
      label: 'Remove Small Faces',
      onClick: () => {
        useAppStore.getState().setActiveRibbonTab('prepare');
        useAppStore.getState().setPrepareSubPanel('defeaturing');
        const state = useAppStore.getState();
        const issues = state.defeatureIssues.filter(i => !i.fixed && i.kind === 'small_face');
        if (issues.length > 0) {
          issues.forEach(i => useAppStore.getState().fixDefeatureIssue(i.id));
          message.success(`Removed ${issues.length} small face(s).`);
        } else {
          message.info('No small faces found. Run Defeaturing scan first.');
        }
      },
    },
    {
      key: 'fill_holes',
      icon: <HighlightOutlined />,
      label: 'Fill Holes',
      onClick: () => {
        useAppStore.getState().setActiveRibbonTab('prepare');
        useAppStore.getState().setPrepareSubPanel('defeaturing');
        const state = useAppStore.getState();
        const issues = state.defeatureIssues.filter(i => !i.fixed && i.kind === 'small_hole');
        if (issues.length > 0) {
          issues.forEach(i => useAppStore.getState().fixDefeatureIssue(i.id));
          message.success(`Filled ${issues.length} hole(s).`);
        } else {
          message.info('No small holes found. Run Defeaturing scan first.');
        }
      },
    },
  ];

  // ---- CFD Prep dropdown ----
  const cfdPrepMenuItems: MenuProps['items'] = [
    {
      key: 'enclosure',
      icon: <ExpandOutlined />,
      label: 'Create Enclosure',
      onClick: () => {
        // Calculate bounding box of all shapes and add an enclosure
        let minX = -2, maxX = 2, minY = -2, maxY = 2, minZ = -2, maxZ = 2;
        const bodyShapes = shapes.filter((s) => s.group !== 'enclosure');
        if (bodyShapes.length > 0) {
          minX = Math.min(...bodyShapes.map((s) => s.position[0])) - 2;
          maxX = Math.max(...bodyShapes.map((s) => s.position[0])) + 2;
          minY = Math.min(...bodyShapes.map((s) => s.position[1])) - 2;
          maxY = Math.max(...bodyShapes.map((s) => s.position[1])) + 2;
          minZ = Math.min(...bodyShapes.map((s) => s.position[2])) - 2;
          maxZ = Math.max(...bodyShapes.map((s) => s.position[2])) + 2;
        }
        const w = maxX - minX;
        const h = maxY - minY;
        const d = maxZ - minZ;
        const id = `shape-${nextId++}`;
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
        message.success('Created enclosure around all bodies.');
      },
    },
    {
      key: 'extract_fluid',
      icon: <ExperimentOutlined />,
      label: 'Extract Fluid Domain',
      onClick: () => {
        useAppStore.getState().setFluidExtracted(true);
        useAppStore.getState().setActiveRibbonTab('prepare');
        useAppStore.getState().setPrepareSubPanel('enclosure');
        message.success('Fluid domain extracted from enclosure.');
      },
    },
    {
      key: 'symmetry_cut',
      icon: <BorderInnerOutlined />,
      label: 'Symmetry Cut',
      onClick: () => {
        useAppStore.getState().setCadMode('symmetry_cut');
        useAppStore.getState().setSectionPlane({ enabled: true, axis: 'x', normal: [1, 0, 0], offset: 0 });
        message.info('Symmetry cut: enabled section plane at YZ (x=0). Adjust offset in Display > Section.');
      },
    },
    {
      key: 'name_regions',
      icon: <AppstoreOutlined />,
      label: 'Name Regions',
      onClick: () => {
        useAppStore.getState().setActiveRibbonTab('prepare');
        useAppStore.getState().setPrepareSubPanel('named_selection');
        message.info('Switched to Named Selections panel. Define inlet, outlet, wall regions there.');
      },
    },
  ];

  const isBooleanMode = cadMode === 'boolean_select_target' || cadMode === 'boolean_select_tool';

  return (
    <div
      style={{
        padding: '6px 12px',
        borderBottom: '1px solid #303030',
        background: '#1f1f1f',
        display: 'flex',
        alignItems: 'center',
        gap: 4,
        flexWrap: 'wrap',
      }}
    >
      {isBooleanMode ? (
        <Space>
          <span style={{ color: '#faad14', fontSize: 12 }}>
            {cadMode === 'boolean_select_target'
              ? 'Click TARGET shape...'
              : 'Click TOOL shape...'}
          </span>
          <Button size="small" onClick={cancelBoolean}>
            Cancel
          </Button>
        </Space>
      ) : (
        <Space wrap size={4}>
          {/* Create */}
          <Dropdown menu={{ items: createMenuItems }} trigger={['click']}>
            <Button icon={<BlockOutlined />} size="small">
              Create <DownOutlined style={{ fontSize: 10 }} />
            </Button>
          </Dropdown>

          {/* Sketch */}
          <Dropdown menu={{ items: sketchMenuItems }} trigger={['click']}>
            <Button icon={<SlidersOutlined />} size="small">
              Sketch <DownOutlined style={{ fontSize: 10 }} />
            </Button>
          </Dropdown>

          {/* Edit */}
          <Dropdown menu={{ items: editMenuItems }} trigger={['click']}>
            <Button icon={<ToolOutlined />} size="small">
              Edit <DownOutlined style={{ fontSize: 10 }} />
            </Button>
          </Dropdown>

          {/* Boolean */}
          <Dropdown menu={{ items: booleanMenuItems }} trigger={['click']}>
            <Button icon={<InteractionOutlined />} size="small">
              Boolean <DownOutlined style={{ fontSize: 10 }} />
            </Button>
          </Dropdown>

          {/* Defeaturing */}
          <Dropdown menu={{ items: defeaturingMenuItems }} trigger={['click']}>
            <Button icon={<ScissorOutlined />} size="small">
              Defeaturing <DownOutlined style={{ fontSize: 10 }} />
            </Button>
          </Dropdown>

          {/* CFD Prep */}
          <Dropdown menu={{ items: cfdPrepMenuItems }} trigger={['click']}>
            <Button icon={<ExperimentOutlined />} size="small">
              CFD Prep <DownOutlined style={{ fontSize: 10 }} />
            </Button>
          </Dropdown>

          {/* Import STL */}
          <Upload
            accept=".stl"
            showUploadList={false}
            beforeUpload={(file) => {
              handleStlUpload(file);
              return false;
            }}
          >
            <Button icon={<ImportOutlined />} size="small">
              Import STL
            </Button>
          </Upload>
        </Space>
      )}
    </div>
  );
};

export default PrimitiveToolbar;
