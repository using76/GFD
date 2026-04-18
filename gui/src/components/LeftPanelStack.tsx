import React, { useState, useEffect } from 'react';
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
  CheckCircleOutlined,
  WarningOutlined,
} from '@ant-design/icons';
import { Tree, Button, message } from 'antd';
import type { TreeDataNode } from 'antd';
import { useAppStore } from '../store/useAppStore';
import type { RepairIssueKind } from '../store/useAppStore';
import ToolOptions from './ToolOptions';
import ShapeProperties from '../tabs/cad/ShapeProperties';
import DefeaturingPanel from '../tabs/cad/DefeaturingPanel';
import CfdPrepPanel from '../tabs/cad/CfdPrepPanel';
import NamedSelectionPanel from '../tabs/cad/NamedSelectionPanel';
import MeshSettings from '../tabs/mesh/MeshSettings';
import QualityPanel from '../tabs/mesh/QualityPanel';
import ModelsPanel from '../tabs/setup/ModelsPanel';
import MaterialPanel from '../tabs/setup/MaterialPanel';
import BoundaryPanel from '../tabs/setup/BoundaryPanel';
import SolverSettingsPanel from '../tabs/setup/SolverSettingsPanel';
import InitialConditionsPanel from '../tabs/setup/InitialConditionsPanel';
import SpeciesPanel from '../tabs/setup/SpeciesPanel';
import RunControls from '../tabs/calc/RunControls';
import ContourSettings from '../tabs/results/ContourSettings';
import VectorSettings from '../tabs/results/VectorSettings';
import StreamlineSettings from '../tabs/results/StreamlineSettings';
import ReportPanel from '../tabs/results/ReportPanel';
import ParametricSweepPanel from '../tabs/results/ParametricSweepPanel';

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
// Saved Views (camera bookmarks)
// ============================================================
const SavedViewsList: React.FC = () => {
  const savedViews = useAppStore((s) => s.savedViews);
  const addSavedView = useAppStore((s) => s.addSavedView);
  const removeSavedView = useAppStore((s) => s.removeSavedView);

  const saveCurrent = () => {
    // Ask the Viewport to capture and respond via gfd-camera-captured
    const onCaptured = (e: Event) => {
      const d = (e as CustomEvent).detail as { position: [number, number, number]; target: [number, number, number] };
      window.removeEventListener('gfd-camera-captured', onCaptured);
      const defaultName = `View ${savedViews.length + 1}`;
      const name = prompt('Name for this view:', defaultName) ?? defaultName;
      if (!name) return;
      addSavedView({ id: `view-${Date.now()}`, name, position: d.position, target: d.target });
      message.success(`Saved view: ${name}`);
    };
    window.addEventListener('gfd-camera-captured', onCaptured);
    window.dispatchEvent(new CustomEvent('gfd-camera-capture'));
  };

  const restoreView = (v: { position: [number, number, number]; target: [number, number, number] }) => {
    window.dispatchEvent(new CustomEvent('gfd-camera-restore', { detail: v }));
  };

  return (
    <div style={{ padding: 8 }}>
      <Button size="small" block type="primary" onClick={saveCurrent} style={{ marginBottom: 6 }}>
        Save Current View
      </Button>
      {savedViews.length === 0 ? (
        <div style={{ padding: 8, color: '#667', fontSize: 11 }}>
          No saved views yet. Click "Save Current View" to bookmark the current camera.
        </div>
      ) : (
        <div style={{ maxHeight: 220, overflow: 'auto' }}>
          {savedViews.map((v) => (
            <div
              key={v.id}
              style={{
                display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                padding: '4px 6px', borderBottom: '1px solid #1a1a30', fontSize: 11,
              }}
            >
              <span
                onClick={() => restoreView(v)}
                style={{ color: '#4096ff', cursor: 'pointer', flex: 1 }}
                title="Click to restore"
              >
                {v.name}
              </span>
              <span style={{ color: '#556', fontSize: 10, marginRight: 6 }}>
                ({v.position.map(n => n.toFixed(1)).join(', ')})
              </span>
              <span
                onClick={() => removeSavedView(v.id)}
                style={{ color: '#ff4d4f', cursor: 'pointer', fontSize: 10, padding: '0 4px' }}
                title="Delete"
              >
                x
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

// ============================================================
// Layers sub-tab: named collections with visibility toggle
// ============================================================
const LayersList: React.FC = () => {
  const layers = useAppStore((s) => s.layers);
  const addLayer = useAppStore((s) => s.addLayer);
  const removeLayer = useAppStore((s) => s.removeLayer);
  const toggleLayerVisibility = useAppStore((s) => s.toggleLayerVisibility);
  const updateLayer = useAppStore((s) => s.updateLayer);
  const selectedShapeIds = useAppStore((s) => s.selectedShapeIds);
  const selectedShapeId = useAppStore((s) => s.selectedShapeId);
  const assignShapeToLayer = useAppStore((s) => s.assignShapeToLayer);
  const shapes = useAppStore((s) => s.shapes);

  const createLayer = () => {
    const name = prompt('Layer name:', `Layer ${layers.length + 1}`);
    if (!name) return;
    const palette = ['#1668dc', '#52c41a', '#fa8c16', '#eb2f96', '#722ed1', '#13c2c2'];
    addLayer({
      id: `layer-${Date.now()}`,
      name,
      visible: true,
      color: palette[layers.length % palette.length],
    });
  };

  const assignSelection = (layerId: string) => {
    const ids = selectedShapeIds.length > 0 ? selectedShapeIds : (selectedShapeId ? [selectedShapeId] : []);
    if (ids.length === 0) {
      message.warning('Select one or more shapes first.');
      return;
    }
    ids.forEach(id => assignShapeToLayer(id, layerId));
    message.success(`Assigned ${ids.length} shape(s) to layer.`);
  };

  return (
    <div style={{ padding: 8 }}>
      <Button size="small" block type="primary" onClick={createLayer} style={{ marginBottom: 6 }}>
        Add Layer
      </Button>
      {layers.length === 0 ? (
        <div style={{ padding: 8, color: '#667', fontSize: 11 }}>
          No layers yet. Layers let you toggle visibility of groups of shapes together.
        </div>
      ) : (
        <div style={{ maxHeight: 240, overflow: 'auto' }}>
          {layers.map((l) => {
            const memberCount = shapes.filter(sh => sh.layerId === l.id).length;
            return (
              <div key={l.id} style={{
                display: 'flex', alignItems: 'center', gap: 4, padding: '4px 6px',
                borderBottom: '1px solid #1a1a30', fontSize: 11,
              }}>
                <span style={{ width: 10, height: 10, borderRadius: 2, background: l.color, flexShrink: 0 }} />
                <span
                  onClick={() => toggleLayerVisibility(l.id)}
                  style={{ color: l.visible ? '#aab' : '#556', cursor: 'pointer', marginRight: 4 }}
                  title="Toggle visibility"
                >
                  {l.visible ? '👁' : '⊘'}
                </span>
                <input
                  value={l.name}
                  onChange={(e) => updateLayer(l.id, { name: e.target.value })}
                  style={{ flex: 1, background: 'transparent', color: '#ccd', border: 'none', fontSize: 11, outline: 'none' }}
                />
                <span style={{ color: '#556', fontSize: 9 }}>({memberCount})</span>
                <span
                  onClick={() => assignSelection(l.id)}
                  style={{ color: '#4096ff', cursor: 'pointer', fontSize: 9, padding: '0 4px' }}
                  title="Assign selected shapes"
                >
                  +sel
                </span>
                <span
                  onClick={() => removeLayer(l.id)}
                  style={{ color: '#ff4d4f', cursor: 'pointer', fontSize: 10, padding: '0 4px' }}
                >
                  ×
                </span>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};

// ============================================================
// Groups sub-tab: persistent selections
// ============================================================
const GroupsList: React.FC = () => {
  const groups = useAppStore((s) => s.customGroups);
  const addGroup = useAppStore((s) => s.addCustomGroup);
  const removeGroup = useAppStore((s) => s.removeCustomGroup);
  const addShapeToGroup = useAppStore((s) => s.addShapeToGroup);
  const selectedShapeIds = useAppStore((s) => s.selectedShapeIds);
  const selectedShapeId = useAppStore((s) => s.selectedShapeId);
  const toggleMultiSelect = useAppStore((s) => s.toggleMultiSelect);
  const clearMultiSelect = useAppStore((s) => s.clearMultiSelect);

  const createGroup = () => {
    const name = prompt('Group name:', `Group ${groups.length + 1}`);
    if (!name) return;
    const ids = selectedShapeIds.length > 0 ? selectedShapeIds : (selectedShapeId ? [selectedShapeId] : []);
    addGroup({ id: `grp-${Date.now()}`, name, shapeIds: [...ids] });
    message.success(`Group "${name}" created with ${ids.length} shape(s).`);
  };

  const selectGroup = (shapeIds: string[]) => {
    clearMultiSelect();
    shapeIds.forEach(id => toggleMultiSelect(id));
    message.info(`Selected ${shapeIds.length} shape(s) from group.`);
  };

  return (
    <div style={{ padding: 8 }}>
      <Button size="small" block type="primary" onClick={createGroup} style={{ marginBottom: 6 }}>
        New Group from Selection
      </Button>
      {groups.length === 0 ? (
        <div style={{ padding: 8, color: '#667', fontSize: 11 }}>
          No groups yet. Select shapes and click "New Group" to save as a persistent selection.
        </div>
      ) : (
        <div style={{ maxHeight: 240, overflow: 'auto' }}>
          {groups.map((g) => (
            <div key={g.id} style={{
              display: 'flex', alignItems: 'center', gap: 4, padding: '4px 6px',
              borderBottom: '1px solid #1a1a30', fontSize: 11,
            }}>
              <span
                onClick={() => selectGroup(g.shapeIds)}
                style={{ flex: 1, color: '#4096ff', cursor: 'pointer' }}
                title="Click to select all shapes in this group"
              >
                {g.name}
              </span>
              <span style={{ color: '#556', fontSize: 9 }}>({g.shapeIds.length})</span>
              <span
                onClick={() => {
                  const ids = selectedShapeIds.length > 0 ? selectedShapeIds : (selectedShapeId ? [selectedShapeId] : []);
                  ids.forEach(id => addShapeToGroup(g.id, id));
                }}
                style={{ color: '#52c41a', cursor: 'pointer', fontSize: 9, padding: '0 4px' }}
                title="Add selected shapes to this group"
              >
                +sel
              </span>
              <span
                onClick={() => removeGroup(g.id)}
                style={{ color: '#ff4d4f', cursor: 'pointer', fontSize: 10, padding: '0 4px' }}
              >
                ×
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

// ============================================================
// Selection sub-tab: current selection details
// ============================================================
const SelectionList: React.FC = () => {
  const selectedShapeIds = useAppStore((s) => s.selectedShapeIds);
  const selectedShapeId = useAppStore((s) => s.selectedShapeId);
  const shapes = useAppStore((s) => s.shapes);
  const clearMultiSelect = useAppStore((s) => s.clearMultiSelect);

  const ids = selectedShapeIds.length > 0 ? selectedShapeIds : (selectedShapeId ? [selectedShapeId] : []);
  if (ids.length === 0) {
    return <div style={{ padding: 12, color: '#667', fontSize: 11 }}>No selection. Click a shape in the viewport or Structure tree to select.</div>;
  }

  return (
    <div style={{ padding: 8 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 6 }}>
        <span style={{ color: '#aab', fontSize: 11, fontWeight: 600 }}>
          {ids.length} selected
        </span>
        <span onClick={clearMultiSelect} style={{ color: '#4096ff', fontSize: 11, cursor: 'pointer' }}>
          Clear
        </span>
      </div>
      {ids.map(id => {
        const sh = shapes.find(s => s.id === id);
        if (!sh) return null;
        return (
          <div key={id} style={{ padding: '2px 6px', fontSize: 11, color: '#aab', borderBottom: '1px solid #1a1a30' }}>
            <b>{sh.name}</b> <span style={{ color: '#556' }}>({sh.kind})</span>
            <div style={{ fontSize: 10, color: '#667', paddingLeft: 8 }}>
              pos: ({sh.position.map(n => n.toFixed(2)).join(', ')})
            </div>
          </div>
        );
      })}
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
      {subTab === 'layers' && <LayersList />}
      {subTab === 'groups' && <GroupsList />}
      {subTab === 'selection' && <SelectionList />}
      {subTab === 'views' && <SavedViewsList />}
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
    { key: 'initial', title: 'Initial Conditions', icon: <LineChartOutlined />, isLeaf: true },
    { key: 'species', title: 'Species / Reactions', icon: <ExperimentOutlined />, isLeaf: true },
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
    { key: 'sweep', title: 'Parametric Sweep', icon: <LineChartOutlined />, isLeaf: true },
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
// Repair Issues Panel (list of 3D repair markers)
// ============================================================
const RepairIssuesPanel: React.FC = () => {
  const repairIssues = useAppStore((s) => s.repairIssues);
  const selectedRepairIssueId = useAppStore((s) => s.selectedRepairIssueId);
  const selectRepairIssue = useAppStore((s) => s.selectRepairIssue);
  const fixRepairIssue = useAppStore((s) => s.fixRepairIssue);
  const clearRepairIssues = useAppStore((s) => s.clearRepairIssues);

  const unfixed = repairIssues.filter(i => !i.fixed);
  const fixed = repairIssues.filter(i => i.fixed);

  const kindColors: Record<RepairIssueKind, string> = {
    missing_face: '#ff8c00',
    extra_edge: '#ffd700',
    gap: '#00e5ff',
    non_manifold: '#ff4d4f',
    self_intersect: '#eb2f96',
  };

  const kindLabels: Record<RepairIssueKind, string> = {
    missing_face: 'Missing Face',
    extra_edge: 'Extra Edge',
    gap: 'Gap',
    non_manifold: 'Non-Manifold',
    self_intersect: 'Self-Intersect',
  };

  if (repairIssues.length === 0) {
    return <div style={{ padding: 12, color: '#667', fontSize: 11 }}>No repair issues. Click "Check" in the ribbon to scan.</div>;
  }

  return (
    <div style={{ padding: 8 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 6 }}>
        <span style={{ color: '#aab', fontSize: 11, fontWeight: 600 }}>
          {unfixed.length} issue{unfixed.length !== 1 ? 's' : ''} remaining
        </span>
        {repairIssues.length > 0 && (
          <span
            onClick={clearRepairIssues}
            style={{ color: '#4096ff', fontSize: 11, cursor: 'pointer' }}
          >
            Clear all
          </span>
        )}
      </div>

      <div style={{ maxHeight: 250, overflow: 'auto' }}>
        {unfixed.map((issue) => (
          <div
            key={issue.id}
            onClick={() => selectRepairIssue(issue.id)}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              padding: '4px 6px',
              fontSize: 11,
              cursor: 'pointer',
              background: selectedRepairIssueId === issue.id ? '#2a2a5a' : 'transparent',
              borderBottom: '1px solid #1a1a30',
              borderLeft: `3px solid ${kindColors[issue.kind]}`,
            }}
            onMouseEnter={(e) => { if (selectedRepairIssueId !== issue.id) e.currentTarget.style.background = '#1a1a3a'; }}
            onMouseLeave={(e) => { if (selectedRepairIssueId !== issue.id) e.currentTarget.style.background = 'transparent'; }}
          >
            <WarningOutlined style={{ color: kindColors[issue.kind], fontSize: 12 }} />
            <div style={{ flex: 1 }}>
              <div style={{ color: '#ccd' }}>{kindLabels[issue.kind]}</div>
              <div style={{ color: '#667', fontSize: 10 }}>{issue.description}</div>
            </div>
            <span
              onClick={(e) => { e.stopPropagation(); fixRepairIssue(issue.id); }}
              style={{
                color: '#52c41a',
                fontSize: 10,
                cursor: 'pointer',
                padding: '1px 4px',
                border: '1px solid #52c41a',
                borderRadius: 3,
              }}
            >
              Fix
            </span>
          </div>
        ))}
      </div>

      {fixed.length > 0 && (
        <div style={{ marginTop: 6 }}>
          <div style={{ color: '#52c41a', fontSize: 10, fontWeight: 600, padding: '2px 0' }}>
            <CheckCircleOutlined /> {fixed.length} fixed
          </div>
        </div>
      )}
    </div>
  );
};

// ============================================================
// Measure Results Panel
// ============================================================
const MeasureResultsPanel: React.FC = () => {
  const measureLabels = useAppStore((s) => s.measureLabels);
  const clearMeasureLabels = useAppStore((s) => s.clearMeasureLabels);

  if (measureLabels.length === 0) {
    return <div style={{ padding: 12, color: '#667', fontSize: 11 }}>No measurements yet. Use distance/angle/area tools.</div>;
  }

  return (
    <div style={{ padding: 8 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 6 }}>
        <span style={{ color: '#aab', fontSize: 11, fontWeight: 600 }}>
          {measureLabels.length} measurement{measureLabels.length !== 1 ? 's' : ''}
        </span>
        <span
          onClick={clearMeasureLabels}
          style={{ color: '#4096ff', fontSize: 11, cursor: 'pointer' }}
        >
          Clear all
        </span>
      </div>

      <div style={{ maxHeight: 200, overflow: 'auto' }}>
        {measureLabels.map((label, i) => (
          <div
            key={label.id}
            style={{
              padding: '4px 6px',
              fontSize: 11,
              color: '#ccd',
              borderBottom: '1px solid #1a1a30',
              borderLeft: '3px solid #4096ff',
              display: 'flex',
              alignItems: 'center',
              gap: 6,
            }}
          >
            <span style={{ color: '#667', fontSize: 10, minWidth: 16 }}>#{i + 1}</span>
            <span style={{ fontWeight: 500 }}>{label.text}</span>
            {label.endPosition && (
              <span style={{ color: '#556', fontSize: 10, marginLeft: 'auto' }}>
                ({label.position[0].toFixed(1)}, {label.position[2].toFixed(1)}) - ({label.endPosition[0].toFixed(1)}, {label.endPosition[2].toFixed(1)})
              </span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
};

// ============================================================
// Display Settings Panel
// ============================================================
const DisplaySettingsPanel: React.FC = () => {
  const renderMode = useAppStore((s) => s.renderMode);
  const setRenderMode = useAppStore((s) => s.setRenderMode);
  const transparencyMode = useAppStore((s) => s.transparencyMode);
  const setTransparencyMode = useAppStore((s) => s.setTransparencyMode);
  const lightingIntensity = useAppStore((s) => s.lightingIntensity);
  const setLightingIntensity = useAppStore((s) => s.setLightingIntensity);
  const backgroundMode = useAppStore((s) => s.backgroundMode);
  const setBackgroundMode = useAppStore((s) => s.setBackgroundMode);
  const sectionPlane = useAppStore((s) => s.sectionPlane);
  const setSectionPlane = useAppStore((s) => s.setSectionPlane);
  const cameraMode = useAppStore((s) => s.cameraMode);
  const setCameraMode = useAppStore((s) => s.setCameraMode);

  return (
    <div style={{ padding: 10, fontSize: 12 }}>
      <div style={{ color: '#889', fontSize: 11, marginBottom: 6, fontWeight: 500 }}>Render Mode</div>
      <div style={{ display: 'flex', gap: 4, marginBottom: 10 }}>
        {(['wireframe', 'solid', 'contour'] as const).map((mode) => (
          <div
            key={mode}
            onClick={() => setRenderMode(mode)}
            style={{
              padding: '3px 8px',
              fontSize: 11,
              cursor: 'pointer',
              borderRadius: 3,
              border: renderMode === mode ? '1px solid #4096ff' : '1px solid #303050',
              background: renderMode === mode ? '#2a2a5a' : '#1a1a30',
              color: renderMode === mode ? '#fff' : '#889',
            }}
          >
            {mode.charAt(0).toUpperCase() + mode.slice(1)}
          </div>
        ))}
      </div>

      <div style={{ color: '#889', fontSize: 11, marginBottom: 4, fontWeight: 500 }}>Options</div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6, marginBottom: 10 }}>
        <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer', color: '#aab', fontSize: 11 }}>
          <input type="checkbox" checked={transparencyMode} onChange={(e) => setTransparencyMode(e.target.checked)} />
          Transparency
        </label>
        <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer', color: '#aab', fontSize: 11 }}>
          <input type="checkbox" checked={sectionPlane.enabled} onChange={(e) => setSectionPlane({ enabled: e.target.checked })} />
          Section Plane
        </label>
      </div>

      <div style={{ color: '#889', fontSize: 11, marginBottom: 4, fontWeight: 500 }}>Lighting: {(lightingIntensity * 100).toFixed(0)}%</div>
      <input
        type="range"
        min={0.25}
        max={2.0}
        step={0.05}
        value={lightingIntensity}
        onChange={(e) => setLightingIntensity(Number(e.target.value))}
        style={{ width: '100%', marginBottom: 10 }}
      />

      <div style={{ color: '#889', fontSize: 11, marginBottom: 4, fontWeight: 500 }}>Background</div>
      <div style={{ display: 'flex', gap: 4, marginBottom: 10 }}>
        {(['dark', 'light', 'gradient'] as const).map((mode) => (
          <div
            key={mode}
            onClick={() => setBackgroundMode(mode)}
            style={{
              padding: '3px 8px',
              fontSize: 11,
              cursor: 'pointer',
              borderRadius: 3,
              border: backgroundMode === mode ? '1px solid #4096ff' : '1px solid #303050',
              background: backgroundMode === mode ? '#2a2a5a' : '#1a1a30',
              color: backgroundMode === mode ? '#fff' : '#889',
            }}
          >
            {mode.charAt(0).toUpperCase() + mode.slice(1)}
          </div>
        ))}
      </div>

      <div style={{ color: '#889', fontSize: 11, marginBottom: 4, fontWeight: 500 }}>Camera</div>
      <div style={{ display: 'flex', gap: 4 }}>
        <div
          onClick={() => setCameraMode({ type: 'perspective' })}
          style={{
            padding: '3px 8px',
            fontSize: 11,
            cursor: 'pointer',
            borderRadius: 3,
            border: cameraMode.type === 'perspective' ? '1px solid #4096ff' : '1px solid #303050',
            background: cameraMode.type === 'perspective' ? '#2a2a5a' : '#1a1a30',
            color: cameraMode.type === 'perspective' ? '#fff' : '#889',
          }}
        >
          Perspective
        </div>
        <div
          onClick={() => setCameraMode({ type: 'orthographic' })}
          style={{
            padding: '3px 8px',
            fontSize: 11,
            cursor: 'pointer',
            borderRadius: 3,
            border: cameraMode.type === 'orthographic' ? '1px solid #4096ff' : '1px solid #303050',
            background: cameraMode.type === 'orthographic' ? '#2a2a5a' : '#1a1a30',
            color: cameraMode.type === 'orthographic' ? '#fff' : '#889',
          }}
        >
          Orthographic
        </div>
      </div>
    </div>
  );
};

// ============================================================
// Prepare Sub-Panel (driven by prepareSubPanel state)
// ============================================================
const PrepareSubPanelContent: React.FC = () => {
  const prepareSubPanel = useAppStore((s) => s.prepareSubPanel);

  if (prepareSubPanel === 'defeaturing') {
    return (
      <CollapsiblePanel title="Defeaturing" defaultOpen>
        <DefeaturingPanel />
      </CollapsiblePanel>
    );
  }

  if (prepareSubPanel === 'named_selection') {
    return (
      <CollapsiblePanel title="Named Selections" defaultOpen>
        <NamedSelectionPanel />
      </CollapsiblePanel>
    );
  }

  // Default: no extra sub-panel
  return null;
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

  // Listen for setup section change events from ribbon buttons
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail?.section) {
        setSetupSection(detail.section);
      }
    };
    window.addEventListener('gfd-setup-section', handler);
    return () => window.removeEventListener('gfd-setup-section', handler);
  }, []);

  // Listen for results section change events from ribbon buttons
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail?.section) {
        setResultsSection(detail.section);
      }
    };
    window.addEventListener('gfd-results-section', handler);
    return () => window.removeEventListener('gfd-results-section', handler);
  }, []);

  const isDesign = activeRibbonTab === 'design';
  const isDisplay = activeRibbonTab === 'display';
  const isMeasure = activeRibbonTab === 'measure';
  const isRepair = activeRibbonTab === 'repair';
  const isPrepare = activeRibbonTab === 'prepare';
  const isMesh = activeRibbonTab === 'mesh';
  const isSetup = activeRibbonTab === 'setup';
  const isCalc = activeRibbonTab === 'calc';
  const isResults = activeRibbonTab === 'results';

  // Setup section panel
  const setupPanelMap: Record<string, React.ReactNode> = {
    models: <ModelsPanel />,
    materials: <MaterialPanel />,
    boundaries: <BoundaryPanel />,
    initial: <InitialConditionsPanel />,
    species: <SpeciesPanel />,
    solver: <SolverSettingsPanel />,
  };

  // Results section panel
  const resultsPanelMap: Record<string, React.ReactNode> = {
    contours: <ContourSettings />,
    vectors: <VectorSettings />,
    streamlines: <StreamlineSettings />,
    reports: <ReportPanel />,
    sweep: <ParametricSweepPanel />,
  };

  return (
    <div style={{ height: '100%', overflow: 'auto', background: '#111122' }}>

      {/* ============ Design tab: Structure + Tool Options + Properties + Defeaturing + CFD Prep ============ */}
      {isDesign && (
        <>
          <CollapsiblePanel title="Structure" defaultOpen>
            <StructureSubTabs />
          </CollapsiblePanel>

          <CollapsiblePanel title={`Options - ${activeTool.charAt(0).toUpperCase() + activeTool.slice(1)}`} defaultOpen>
            <ToolOptions />
          </CollapsiblePanel>

          <CollapsiblePanel title="Properties" defaultOpen>
            <ShapeProperties />
          </CollapsiblePanel>

          <CollapsiblePanel title="Defeaturing" defaultOpen={false}>
            <DefeaturingPanel />
          </CollapsiblePanel>

          <CollapsiblePanel title="CFD Prep" defaultOpen={false}>
            <CfdPrepPanel />
          </CollapsiblePanel>
        </>
      )}

      {/* ============ Display tab: Display Settings ============ */}
      {isDisplay && (
        <>
          <CollapsiblePanel title="Structure" defaultOpen>
            <StructureSubTabs />
          </CollapsiblePanel>

          <CollapsiblePanel title="Display Settings" defaultOpen>
            <DisplaySettingsPanel />
          </CollapsiblePanel>

          <CollapsiblePanel title="Properties" defaultOpen={false}>
            <ShapeProperties />
          </CollapsiblePanel>
        </>
      )}

      {/* ============ Measure tab: Measure tool + Results list ============ */}
      {isMeasure && (
        <>
          <CollapsiblePanel title="Structure" defaultOpen>
            <StructureSubTabs />
          </CollapsiblePanel>

          <CollapsiblePanel title={`Options - ${activeTool.charAt(0).toUpperCase() + activeTool.slice(1)}`} defaultOpen>
            <ToolOptions />
          </CollapsiblePanel>

          <CollapsiblePanel title="Measurement Results" defaultOpen>
            <MeasureResultsPanel />
          </CollapsiblePanel>

          <CollapsiblePanel title="Properties" defaultOpen={false}>
            <ShapeProperties />
          </CollapsiblePanel>
        </>
      )}

      {/* ============ Repair tab: Repair Issues + Defeaturing + Repair Log ============ */}
      {isRepair && (
        <>
          <CollapsiblePanel title="Structure" defaultOpen>
            <StructureSubTabs />
          </CollapsiblePanel>

          <CollapsiblePanel title="Repair Issues" defaultOpen>
            <RepairIssuesPanel />
          </CollapsiblePanel>

          <CollapsiblePanel title="Defeaturing" defaultOpen={false}>
            <DefeaturingPanel />
          </CollapsiblePanel>

          <CollapsiblePanel title="Repair Log" defaultOpen>
            <RepairLogPanel />
          </CollapsiblePanel>
        </>
      )}

      {/* ============ Prepare tab: CFD Prep (Enclosure + Volume Extract) shown directly ============ */}
      {isPrepare && (
        <>
          <CollapsiblePanel title="Structure" defaultOpen>
            <StructureSubTabs />
          </CollapsiblePanel>

          <CollapsiblePanel title="CFD Prep" defaultOpen>
            <CfdPrepPanel />
          </CollapsiblePanel>

          <PrepareSubPanelContent />

          <CollapsiblePanel title="Properties" defaultOpen={false}>
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

          <CollapsiblePanel title="Quality" defaultOpen>
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
