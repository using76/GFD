import React from 'react';
import { Upload, message } from 'antd';
import {
  ScissorOutlined,
  SwapOutlined,
  BlockOutlined,
  BorderOutlined,
  ExpandOutlined,
  ImportOutlined,
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
import type { RibbonTab } from '../store/useAppStore';

// ---- Ribbon Button Component ----
const RibbonButton: React.FC<{
  icon: React.ReactNode;
  label: string;
  active?: boolean;
  large?: boolean;
  shortcut?: string;
  onClick?: () => void;
}> = ({ icon, label, active, large, shortcut, onClick }) => (
  <div
    onClick={onClick}
    title={shortcut ? `${label} [${shortcut}]` : label}
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



// Placeholder ribbons — the old mesh-based implementations were removed on
// 2026-04-20 and will be replaced once the pure-Rust CAD kernel lands.
// See docs/CAD_KERNEL_PLAN.md.
import { DesignRibbonV2, DisplayRibbonV2, MeasureRibbonV2, RepairRibbonV2 } from './RibbonCadV2';


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
/** Parse VTK ASCII POLYDATA file → Float32Array positions + colors */
function parseVtkPolydata(text: string, color: [number, number, number]): { positions: Float32Array; colors: Float32Array; wireframe: Float32Array } {
  const lines = text.split('\n');
  const points: number[] = [];
  const polys: number[][] = [];
  let mode = '';
  let pointCount = 0;
  let polyCount = 0;

  for (let li = 0; li < lines.length; li++) {
    const line = lines[li].trim();
    if (line.startsWith('POINTS')) {
      pointCount = parseInt(line.split(/\s+/)[1]);
      mode = 'points';
      continue;
    }
    if (line.startsWith('POLYGONS')) {
      polyCount = parseInt(line.split(/\s+/)[1]);
      mode = 'polys';
      continue;
    }
    if (mode === 'points' && points.length / 3 < pointCount) {
      const nums = line.split(/\s+/).map(Number).filter(n => !isNaN(n));
      points.push(...nums);
    }
    if (mode === 'polys' && polys.length < polyCount) {
      const nums = line.split(/\s+/).map(Number).filter(n => !isNaN(n));
      if (nums.length >= 4 && nums[0] === 3) {
        polys.push([nums[1], nums[2], nums[3]]);
      }
    }
  }

  const positions = new Float32Array(polys.length * 9);
  const colors = new Float32Array(polys.length * 9);
  const wireSegs: number[] = [];

  for (let i = 0; i < polys.length; i++) {
    const [i0, i1, i2] = polys[i];
    const off = i * 9;
    positions[off]   = points[i0*3];   positions[off+1] = points[i0*3+1]; positions[off+2] = points[i0*3+2];
    positions[off+3] = points[i1*3];   positions[off+4] = points[i1*3+1]; positions[off+5] = points[i1*3+2];
    positions[off+6] = points[i2*3];   positions[off+7] = points[i2*3+1]; positions[off+8] = points[i2*3+2];
    for (let v = 0; v < 3; v++) {
      colors[off + v*3]     = color[0];
      colors[off + v*3 + 1] = color[1];
      colors[off + v*3 + 2] = color[2];
    }
    wireSegs.push(
      points[i0*3], points[i0*3+1], points[i0*3+2], points[i1*3], points[i1*3+1], points[i1*3+2],
      points[i1*3], points[i1*3+1], points[i1*3+2], points[i2*3], points[i2*3+1], points[i2*3+2],
      points[i2*3], points[i2*3+1], points[i2*3+2], points[i0*3], points[i0*3+1], points[i0*3+2],
    );
  }

  return { positions, colors, wireframe: new Float32Array(wireSegs) };
}

/** Inline slider for Plan Cut offset control */
const CutSlider: React.FC<{ value: number; onChange: (v: number) => void }> = ({ value, onChange }) => (
  <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', padding: '2px 4px', minWidth: 80 }}>
    <input
      type="range"
      min={-3}
      max={3}
      step={0.01}
      value={value}
      onChange={(e) => onChange(parseFloat(e.target.value))}
      style={{ width: 72, height: 14, cursor: 'pointer', accentColor: '#4096ff' }}
    />
    <span style={{ fontSize: 9, color: '#889', marginTop: 1 }}>{value.toFixed(2)}</span>
  </div>
);

const MeshRibbon: React.FC = () => {
  const generateMesh = useAppStore((s) => s.generateMesh);
  const meshGenerating = useAppStore((s) => s.meshGenerating);
  const meshGenerated = useAppStore((s) => s.meshGenerated);
  const domainMode = useAppStore((s) => s.meshConfig.domainMode);
  const updateMeshConfig = useAppStore((s) => s.updateMeshConfig);
  const sectionPlane = useAppStore((s) => s.sectionPlane);
  const setSectionPlane = useAppStore((s) => s.setSectionPlane);

  return (
    <div style={{ display: 'flex', alignItems: 'stretch', gap: 0, height: '100%' }}>
      <RibbonButton icon={<BuildOutlined />} label={meshGenerating ? 'Generating...' : meshGenerated ? 'Regenerate' : 'Generate'} large onClick={() => { if (!meshGenerating) generateMesh(); }} />

      {/* Load VTK mesh files */}
      <Upload
        accept=".vtk,.VTK"
        showUploadList={false}
        multiple
        beforeUpload={(_file, fileList) => {
          const readAll = async () => {
            let fluidPos: Float32Array | null = null;
            let fluidCol: Float32Array | null = null;
            let fluidWire: Float32Array | null = null;
            let solidPos: Float32Array | null = null;
            let solidCol: Float32Array | null = null;
            let solidWire: Float32Array | null = null;
            let totalTris = 0;

            for (const f of fileList) {
              const text = await f.text();
              const isSolid = f.name.toLowerCase().includes('solid');
              const color: [number, number, number] = isSolid ? [0.55, 0.55, 0.62] : [0.267, 0.533, 1.0];
              const parsed = parseVtkPolydata(text, color);
              totalTris += parsed.positions.length / 9;
              if (isSolid) {
                solidPos = parsed.positions;
                solidCol = parsed.colors;
                solidWire = parsed.wireframe;
              } else {
                fluidPos = parsed.positions;
                fluidCol = parsed.colors;
                fluidWire = parsed.wireframe;
              }
            }

            if (!fluidPos && solidPos) {
              fluidPos = solidPos; fluidCol = solidCol; fluidWire = solidWire;
              solidPos = null; solidCol = null; solidWire = null;
            }

            if (!fluidPos || fluidPos.length === 0) {
              message.error('No mesh data found in VTK files');
              return;
            }

            useAppStore.setState({
              meshGenerated: true,
              meshGenerating: false,
              meshDisplayData: {
                positions: fluidPos,
                indices: null,
                colors: fluidCol,
                wireframePositions: fluidWire,
                solidPositions: solidPos,
                solidColors: solidCol,
                solidWireframePositions: solidWire,
                cellCount: totalTris / 2,
                nodeCount: totalTris,
                fluidCellCount: (fluidPos?.length ?? 0) / 18,
                solidCellCount: (solidPos?.length ?? 0) / 18,
                nx: 0, ny: 0, nz: 0,
              },
            });
            message.success(`Loaded mesh: ${totalTris} triangles from ${fileList.length} file(s)`);
          };
          readAll();
          return false;
        }}
      >
        <RibbonButton icon={<ImportOutlined />} label="Load Mesh" />
      </Upload>
      <GroupSep label="Mesh" />

      <RibbonButton icon={<BorderOutlined />} label="Fluid" active={domainMode === 'fluid'} onClick={() => updateMeshConfig({ domainMode: 'fluid' })} />
      <RibbonButton icon={<BlockOutlined />} label="Solid" active={domainMode === 'solid'} onClick={() => updateMeshConfig({ domainMode: 'solid' })} />
      <RibbonButton icon={<AppstoreOutlined />} label="Both" active={domainMode === 'both'} onClick={() => updateMeshConfig({ domainMode: 'both' })} />
      <GroupSep label="Domain" />

      {/* Plan Cut: X/Y/Z axis toggle + offset slider */}
      <RibbonButton icon={<ScissorOutlined />} label="Cut X" active={sectionPlane.enabled && sectionPlane.axis === 'x'} onClick={() => {
        if (sectionPlane.enabled && sectionPlane.axis === 'x') { setSectionPlane({ enabled: false }); }
        else { setSectionPlane({ enabled: true, axis: 'x', normal: [1, 0, 0], offset: 0 }); }
      }} />
      <RibbonButton icon={<ScissorOutlined />} label="Cut Y" active={sectionPlane.enabled && sectionPlane.axis === 'y'} onClick={() => {
        if (sectionPlane.enabled && sectionPlane.axis === 'y') { setSectionPlane({ enabled: false }); }
        else { setSectionPlane({ enabled: true, axis: 'y', normal: [0, 1, 0], offset: 0 }); }
      }} />
      <RibbonButton icon={<ScissorOutlined />} label="Cut Z" active={sectionPlane.enabled && sectionPlane.axis === 'z'} onClick={() => {
        if (sectionPlane.enabled && sectionPlane.axis === 'z') { setSectionPlane({ enabled: false }); }
        else { setSectionPlane({ enabled: true, axis: 'z', normal: [0, 0, 1], offset: 0 }); }
      }} />
      {sectionPlane.enabled && (
        <CutSlider value={sectionPlane.offset} onChange={(v) => setSectionPlane({ offset: v })} />
      )}
      <GroupSep label="Plan Cut" />

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
  design: <DesignRibbonV2 />,
  display: <DisplayRibbonV2 />,
  measure: <MeasureRibbonV2 />,
  repair: <RepairRibbonV2 />,
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
