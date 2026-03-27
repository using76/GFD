import React, { useState } from 'react';
import {
  DownOutlined,
  RightOutlined,
  FolderOutlined,
  BorderOutlined,
  RadiusSettingOutlined,
  ColumnHeightOutlined,
  AimOutlined,
  RetweetOutlined,
  GatewayOutlined,
  FileOutlined,
  ExpandOutlined,
  InteractionOutlined,
  AppstoreOutlined,
  BorderInnerOutlined,
  ExperimentOutlined,
  GoldOutlined,
  BlockOutlined,
  SettingOutlined,
  LineChartOutlined,
  MonitorOutlined,
  HeatMapOutlined,
  ArrowsAltOutlined,
  SwapOutlined,
  FileTextOutlined,
} from '@ant-design/icons';
import { Tree } from 'antd';
import type { TreeDataNode } from 'antd';
import { useAppStore } from '../store/useAppStore';
import ToolOptions from './ToolOptions';
import ShapeProperties from '../tabs/cad/ShapeProperties';
import DefeaturingPanel from '../tabs/cad/DefeaturingPanel';
import CfdPrepPanel from '../tabs/cad/CfdPrepPanel';
import MeshSettings from '../tabs/mesh/MeshSettings';
import QualityPanel from '../tabs/mesh/QualityPanel';
import ModelsPanel from '../tabs/setup/ModelsPanel';
import MaterialPanel from '../tabs/setup/MaterialPanel';
import BoundaryPanel from '../tabs/setup/BoundaryPanel';
import SolverSettingsPanel from '../tabs/setup/SolverSettingsPanel';
import RunControls from '../tabs/calc/RunControls';
import ContourSettings from '../tabs/results/ContourSettings';
import VectorSettings from '../tabs/results/VectorSettings';
import ReportPanel from '../tabs/results/ReportPanel';

// ============================================================
// Collapsible Panel
// ============================================================
const CollapsiblePanel: React.FC<{
  title: string;
  defaultOpen?: boolean;
  children: React.ReactNode;
}> = ({ title, defaultOpen = true, children }) => {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <div style={{ borderBottom: '1px solid #252540' }}>
      <div
        onClick={() => setIsOpen(!isOpen)}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 6,
          padding: '6px 10px',
          cursor: 'pointer',
          background: '#161628',
          userSelect: 'none',
          fontSize: 12,
          fontWeight: 600,
          color: '#aab',
          borderBottom: isOpen ? '1px solid #252540' : 'none',
        }}
        onMouseEnter={(e) => { e.currentTarget.style.background = '#1a1a3a'; }}
        onMouseLeave={(e) => { e.currentTarget.style.background = '#161628'; }}
      >
        <span style={{ fontSize: 10, color: '#667' }}>
          {isOpen ? <DownOutlined /> : <RightOutlined />}
        </span>
        {title}
      </div>
      {isOpen && (
        <div style={{ maxHeight: 400, overflow: 'auto' }}>
          {children}
        </div>
      )}
    </div>
  );
};

// ============================================================
// Structure Tree (CAD objects)
// ============================================================
const StructureTree: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const selectedShapeId = useAppStore((s) => s.selectedShapeId);
  const selectShape = useAppStore((s) => s.selectShape);
  const booleanOps = useAppStore((s) => s.booleanOps);

  const kindColors: Record<string, string> = {
    box: '#1677ff', sphere: '#52c41a', cylinder: '#fa8c16',
    cone: '#eb2f96', torus: '#722ed1', pipe: '#13c2c2',
    stl: '#d4b106', enclosure: '#52c41a',
  };

  function kindIcon(kind: string, isEnclosure?: boolean): React.ReactNode {
    if (isEnclosure || kind === 'enclosure') return <ExpandOutlined style={{ color: kindColors.enclosure }} />;
    const color = kindColors[kind] || '#888';
    const icons: Record<string, React.ReactNode> = {
      box: <BorderOutlined style={{ color }} />,
      sphere: <RadiusSettingOutlined style={{ color }} />,
      cylinder: <ColumnHeightOutlined style={{ color }} />,
      cone: <AimOutlined style={{ color }} />,
      torus: <RetweetOutlined style={{ color }} />,
      pipe: <GatewayOutlined style={{ color }} />,
      stl: <FileOutlined style={{ color }} />,
    };
    return icons[kind] || <BorderOutlined style={{ color }} />;
  }

  const bodies = shapes.filter((s) => s.group !== 'enclosure' && s.group !== 'boolean');
  const enclosures = shapes.filter((s) => s.group === 'enclosure' || s.kind === 'enclosure');

  const treeData: TreeDataNode[] = [
    {
      key: 'bodies',
      title: `Bodies (${bodies.length})`,
      icon: <FolderOutlined />,
      selectable: false,
      children: bodies.map((s) => ({
        key: s.id,
        title: s.name,
        icon: kindIcon(s.kind),
        isLeaf: true,
      })),
    },
  ];

  if (booleanOps.length > 0) {
    treeData.push({
      key: 'booleans',
      title: `Booleans (${booleanOps.length})`,
      icon: <InteractionOutlined style={{ color: '#faad14' }} />,
      selectable: false,
      children: booleanOps.map((op) => ({
        key: `boolop-${op.id}`,
        title: op.name,
        icon: <InteractionOutlined style={{ color: '#faad14' }} />,
        isLeaf: true,
      })),
    });
  }

  if (enclosures.length > 0) {
    treeData.push({
      key: 'enclosures',
      title: `Enclosures (${enclosures.length})`,
      icon: <ExpandOutlined style={{ color: '#52c41a' }} />,
      selectable: false,
      children: enclosures.map((s) => ({
        key: s.id,
        title: s.name,
        icon: kindIcon(s.kind, true),
        isLeaf: true,
      })),
    });
  }

  return (
    <Tree
      showIcon
      defaultExpandAll
      selectedKeys={selectedShapeId ? [selectedShapeId] : []}
      treeData={treeData}
      onSelect={(keys) => {
        if (keys.length > 0) {
          const key = keys[0] as string;
          if (!key.startsWith('boolop-') && key !== 'bodies' && key !== 'booleans' && key !== 'enclosures') {
            selectShape(key);
          }
        }
      }}
      style={{ padding: 6, fontSize: 12 }}
    />
  );
};

// ============================================================
// Sub-tabs for Structure panel
// ============================================================
const StructureSubTabs: React.FC = () => {
  const [subTab, setSubTab] = useState<'structure' | 'layers' | 'groups' | 'selection' | 'views'>('structure');

  const tabs = [
    { key: 'structure', label: 'Structure' },
    { key: 'layers', label: 'Layers' },
    { key: 'groups', label: 'Groups' },
    { key: 'selection', label: 'Selection' },
    { key: 'views', label: 'Views' },
  ] as const;

  return (
    <div>
      <div style={{ display: 'flex', borderBottom: '1px solid #252540', background: '#141428' }}>
        {tabs.map((t) => (
          <div
            key={t.key}
            onClick={() => setSubTab(t.key)}
            style={{
              padding: '4px 8px',
              fontSize: 10,
              cursor: 'pointer',
              color: subTab === t.key ? '#fff' : '#667',
              borderBottom: subTab === t.key ? '2px solid #4096ff' : '2px solid transparent',
              userSelect: 'none',
              flex: 1,
              textAlign: 'center',
            }}
          >
            {t.label}
          </div>
        ))}
      </div>
      {subTab === 'structure' && <StructureTree />}
      {subTab === 'layers' && <div style={{ padding: 12, color: '#667', fontSize: 11 }}>No layers defined.</div>}
      {subTab === 'groups' && <div style={{ padding: 12, color: '#667', fontSize: 11 }}>No groups defined.</div>}
      {subTab === 'selection' && <div style={{ padding: 12, color: '#667', fontSize: 11 }}>No selections active.</div>}
      {subTab === 'views' && <div style={{ padding: 12, color: '#667', fontSize: 11 }}>Saved views will appear here.</div>}
    </div>
  );
};

// ============================================================
// Mesh Zones Tree
// ============================================================
const MeshZonesPanel: React.FC = () => {
  const meshZones = useAppStore((s) => s.meshZones);
  const meshGenerated = useAppStore((s) => s.meshGenerated);

  if (!meshGenerated) {
    return <div style={{ padding: 12, color: '#667', fontSize: 11 }}>No mesh generated yet.</div>;
  }

  const treeData: TreeDataNode[] = [
    {
      key: 'volumes',
      title: 'Volumes',
      icon: <AppstoreOutlined />,
      selectable: false,
      children: meshZones.filter((z) => z.kind === 'volume').map((z) => ({
        key: z.id, title: z.name, isLeaf: true,
      })),
    },
    {
      key: 'surfaces',
      title: 'Surfaces',
      icon: <BorderInnerOutlined />,
      selectable: false,
      children: meshZones.filter((z) => z.kind === 'surface').map((z) => ({
        key: z.id, title: z.name, isLeaf: true,
      })),
    },
  ];

  return <Tree showIcon defaultExpandAll treeData={treeData} style={{ padding: 6, fontSize: 12 }} />;
};

// ============================================================
// Setup Tree
// ============================================================
const SetupTreePanel: React.FC<{ onSelect: (section: string) => void; selected: string }> = ({ onSelect, selected }) => {
  const treeData: TreeDataNode[] = [
    { key: 'models', title: 'Models', icon: <ExperimentOutlined />, isLeaf: true },
    { key: 'materials', title: 'Materials', icon: <GoldOutlined />, isLeaf: true },
    { key: 'boundaries', title: 'Boundary Conditions', icon: <BlockOutlined />, isLeaf: true },
    { key: 'solver', title: 'Solver Settings', icon: <SettingOutlined />, isLeaf: true },
  ];

  return (
    <Tree
      showIcon
      selectedKeys={[selected]}
      treeData={treeData}
      onSelect={(keys) => { if (keys.length > 0) onSelect(keys[0] as string); }}
      style={{ padding: 6, fontSize: 12 }}
    />
  );
};

// ============================================================
// Calc Tree
// ============================================================
const CalcTreePanel: React.FC<{ onSelect: (view: string) => void; selected: string }> = ({ onSelect, selected }) => {
  const treeData: TreeDataNode[] = [
    {
      key: 'monitors',
      title: 'Monitors',
      selectable: false,
      children: [
        { key: 'residuals', title: 'Residuals', icon: <LineChartOutlined />, isLeaf: true },
        { key: 'console', title: 'Console', icon: <MonitorOutlined />, isLeaf: true },
      ],
    },
  ];

  return (
    <Tree
      showIcon
      defaultExpandAll
      selectedKeys={[selected]}
      treeData={treeData}
      onSelect={(keys) => { if (keys.length > 0) onSelect(keys[0] as string); }}
      style={{ padding: 6, fontSize: 12 }}
    />
  );
};

// ============================================================
// Results Tree
// ============================================================
const ResultsTreePanel: React.FC<{ onSelect: (section: string) => void; selected: string }> = ({ onSelect, selected }) => {
  const treeData: TreeDataNode[] = [
    {
      key: 'display',
      title: 'Display',
      selectable: false,
      children: [
        { key: 'contours', title: 'Contours', icon: <HeatMapOutlined />, isLeaf: true },
        { key: 'vectors', title: 'Vectors', icon: <ArrowsAltOutlined />, isLeaf: true },
        { key: 'streamlines', title: 'Streamlines', icon: <SwapOutlined />, isLeaf: true },
      ],
    },
    { key: 'reports', title: 'Reports', icon: <FileTextOutlined />, isLeaf: true },
  ];

  return (
    <Tree
      showIcon
      defaultExpandAll
      selectedKeys={[selected]}
      treeData={treeData}
      onSelect={(keys) => { if (keys.length > 0) onSelect(keys[0] as string); }}
      style={{ padding: 6, fontSize: 12 }}
    />
  );
};

// ============================================================
// Repair Log Panel
// ============================================================
const RepairLogPanel: React.FC = () => {
  const repairLog = useAppStore((s) => s.repairLog);
  const clearRepairLog = useAppStore((s) => s.clearRepairLog);

  if (repairLog.length === 0) {
    return <div style={{ padding: 12, color: '#667', fontSize: 11 }}>No repair actions performed yet.</div>;
  }

  return (
    <div style={{ padding: 8 }}>
      <div style={{ maxHeight: 200, overflow: 'auto', marginBottom: 6 }}>
        {repairLog.map((msg, i) => (
          <div
            key={i}
            style={{
              padding: '3px 6px',
              fontSize: 11,
              color: msg.includes('Fix') || msg.includes('Stitch') ? '#52c41a' : '#aab',
              borderBottom: '1px solid #1a1a30',
              fontFamily: 'monospace',
            }}
          >
            {msg}
          </div>
        ))}
      </div>
      <div
        onClick={clearRepairLog}
        style={{ color: '#4096ff', fontSize: 11, cursor: 'pointer', padding: '2px 6px' }}
      >
        Clear log
      </div>
    </div>
  );
};

// ============================================================
// Prepare Sub-Tab Panel (Defeaturing or CFD Prep)
// ============================================================
const PrepareSubTabPanel: React.FC = () => {
  const prepareSubTab = useAppStore((s) => s.prepareSubTab);
  const setPrepareSubTab = useAppStore((s) => s.setPrepareSubTab);

  return (
    <>
      <div style={{ display: 'flex', borderBottom: '1px solid #252540', background: '#141428' }}>
        {([
          { key: 'defeaturing' as const, label: 'Defeaturing' },
          { key: 'cfdprep' as const, label: 'CFD Prep' },
        ]).map((t) => (
          <div
            key={t.key}
            onClick={() => setPrepareSubTab(t.key)}
            style={{
              padding: '5px 12px',
              fontSize: 11,
              cursor: 'pointer',
              color: prepareSubTab === t.key ? '#fff' : '#667',
              borderBottom: prepareSubTab === t.key ? '2px solid #4096ff' : '2px solid transparent',
              userSelect: 'none',
              flex: 1,
              textAlign: 'center',
              fontWeight: prepareSubTab === t.key ? 600 : 400,
            }}
          >
            {t.label}
          </div>
        ))}
      </div>
      {prepareSubTab === 'defeaturing' && (
        <CollapsiblePanel title="Defeaturing" defaultOpen>
          <DefeaturingPanel />
        </CollapsiblePanel>
      )}
      {prepareSubTab === 'cfdprep' && (
        <CollapsiblePanel title="CFD Prep" defaultOpen>
          <CfdPrepPanel />
        </CollapsiblePanel>
      )}
    </>
  );
};

// ============================================================
// Main LeftPanelStack
// ============================================================
const LeftPanelStack: React.FC = () => {
  const activeRibbonTab = useAppStore((s) => s.activeRibbonTab);
  const activeTool = useAppStore((s) => s.activeTool);

  const [setupSection, setSetupSection] = useState('models');
  const [calcView, setCalcView] = useState('residuals');
  const [resultsSection, setResultsSection] = useState('contours');

  // Determine which panels to show based on active ribbon tab
  const isDesignGroup = activeRibbonTab === 'design' || activeRibbonTab === 'display' || activeRibbonTab === 'measure' || activeRibbonTab === 'repair' || activeRibbonTab === 'prepare';
  const isMesh = activeRibbonTab === 'mesh';
  const isSetup = activeRibbonTab === 'setup';
  const isCalc = activeRibbonTab === 'calc';
  const isResults = activeRibbonTab === 'results';

  // Setup section panel
  const setupPanelMap: Record<string, React.ReactNode> = {
    models: <ModelsPanel />,
    materials: <MaterialPanel />,
    boundaries: <BoundaryPanel />,
    solver: <SolverSettingsPanel />,
  };

  // Results section panel
  const resultsPanelMap: Record<string, React.ReactNode> = {
    contours: <ContourSettings />,
    vectors: <VectorSettings />,
    streamlines: <div style={{ padding: 16, color: '#667' }}>Streamlines will be available in a future release.</div>,
    reports: <ReportPanel />,
  };

  return (
    <div style={{ height: '100%', overflow: 'auto', background: '#111122' }}>

      {/* ============ Design group panels ============ */}
      {isDesignGroup && (
        <>
          <CollapsiblePanel title="Structure" defaultOpen>
            <StructureSubTabs />
          </CollapsiblePanel>

          <CollapsiblePanel title={`Options - ${activeTool.charAt(0).toUpperCase() + activeTool.slice(1)}`} defaultOpen>
            <ToolOptions />
          </CollapsiblePanel>

          {(activeRibbonTab === 'prepare') && (
            <PrepareSubTabPanel />
          )}

          {(activeRibbonTab === 'repair') && (
            <>
              <CollapsiblePanel title="Defeaturing" defaultOpen>
                <DefeaturingPanel />
              </CollapsiblePanel>
              <CollapsiblePanel title="Repair Log" defaultOpen={false}>
                <RepairLogPanel />
              </CollapsiblePanel>
            </>
          )}

          <CollapsiblePanel title="Properties" defaultOpen>
            <ShapeProperties />
          </CollapsiblePanel>
        </>
      )}

      {/* ============ Mesh panels ============ */}
      {isMesh && (
        <>
          <CollapsiblePanel title="Mesh Zones" defaultOpen>
            <MeshZonesPanel />
          </CollapsiblePanel>

          <CollapsiblePanel title="Mesh Settings" defaultOpen>
            <MeshSettings />
          </CollapsiblePanel>

          <CollapsiblePanel title="Quality" defaultOpen={false}>
            <QualityPanel />
          </CollapsiblePanel>
        </>
      )}

      {/* ============ Setup panels ============ */}
      {isSetup && (
        <>
          <CollapsiblePanel title="Setup" defaultOpen>
            <SetupTreePanel selected={setupSection} onSelect={setSetupSection} />
          </CollapsiblePanel>

          <CollapsiblePanel title={setupSection.charAt(0).toUpperCase() + setupSection.slice(1)} defaultOpen>
            {setupPanelMap[setupSection] ?? <div />}
          </CollapsiblePanel>
        </>
      )}

      {/* ============ Calc panels ============ */}
      {isCalc && (
        <>
          <CollapsiblePanel title="Monitors" defaultOpen>
            <CalcTreePanel selected={calcView} onSelect={setCalcView} />
          </CollapsiblePanel>

          <CollapsiblePanel title="Run Controls" defaultOpen>
            <RunControls />
          </CollapsiblePanel>
        </>
      )}

      {/* ============ Results panels ============ */}
      {isResults && (
        <>
          <CollapsiblePanel title="Results" defaultOpen>
            <ResultsTreePanel selected={resultsSection} onSelect={setResultsSection} />
          </CollapsiblePanel>

          <CollapsiblePanel title={resultsSection.charAt(0).toUpperCase() + resultsSection.slice(1)} defaultOpen>
            {resultsPanelMap[resultsSection] ?? <div />}
          </CollapsiblePanel>
        </>
      )}
    </div>
  );
};

export default LeftPanelStack;
