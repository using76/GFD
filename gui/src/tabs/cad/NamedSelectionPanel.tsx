import React, { useState, useCallback, useMemo } from 'react';
import { Button, Input, Select, Divider, Tag, Tooltip, message } from 'antd';
import {
  PlusOutlined,
  DeleteOutlined,
  BgColorsOutlined,
  CheckCircleOutlined,
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
  inlet: '\u25B6',
  outlet: '\u25C0',
  wall: '\u2588',
  symmetry: '\u2194',
  interface: '\u21C4',
  custom: '\u2605',
};

const NamedSelectionPanel: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const namedSelections = useAppStore((s) => s.namedSelections);
  const setNamedSelections = useAppStore((s) => s.setNamedSelections);
  const addNamedSelection = useAppStore((s) => s.addNamedSelection);
  const removeNamedSelection = useAppStore((s) => s.removeNamedSelection);
  const enclosureCreated = useAppStore((s) => s.enclosureCreated);
  const hoveredSelectionName = useAppStore((s) => s.hoveredSelectionName);
  const setHoveredSelectionName = useAppStore((s) => s.setHoveredSelectionName);
  const cfdPrepStep = useAppStore((s) => s.cfdPrepStep);
  const setCfdPrepStep = useAppStore((s) => s.setCfdPrepStep);

  const [newSelName, setNewSelName] = useState('');
  const [newSelType, setNewSelType] = useState<NamedSelectionType>('wall');

  const enclosureShape = useMemo(
    () => shapes.find((s) => s.kind === 'enclosure' || s.isEnclosure),
    [shapes]
  );

  const wallCount = namedSelections.filter((ns) => ns.type === 'wall').length;

  // Auto-name by normal direction
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

  return (
    <div style={{ padding: 12, fontSize: 12 }}>
      {/* Header */}
      <div
        style={{
          fontWeight: 600,
          marginBottom: 10,
          fontSize: 13,
          borderBottom: '1px solid #303050',
          paddingBottom: 6,
          display: 'flex',
          alignItems: 'center',
          gap: 6,
          color: '#ccd',
        }}
      >
        <BgColorsOutlined style={{ color: '#1677ff' }} />
        Named Selections
      </div>

      {/* Info text */}
      <div style={{ color: '#888', fontSize: 11, marginBottom: 8 }}>
        Click faces in 3D viewport to name surfaces for boundary conditions.
      </div>

      {/* Existing named selections list */}
      {namedSelections.length > 0 && (
        <div
          style={{
            maxHeight: 200,
            overflow: 'auto',
            marginBottom: 10,
            border: '1px solid #252530',
            borderRadius: 4,
            background: '#111118',
          }}
        >
          {namedSelections.map((ns) => (
            <div
              key={ns.name}
              onMouseEnter={() => setHoveredSelectionName(ns.name)}
              onMouseLeave={() => setHoveredSelectionName(null)}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '4px 6px',
                borderBottom: '1px solid #1a1a30',
                background: hoveredSelectionName === ns.name ? '#1a1a3e' : 'transparent',
                borderRadius: 2,
                cursor: 'pointer',
                transition: 'background 0.15s',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                {/* Color indicator */}
                <span
                  style={{
                    display: 'inline-block',
                    width: 8,
                    height: 8,
                    borderRadius: '50%',
                    background: ns.color,
                    flexShrink: 0,
                  }}
                />
                <span style={{ color: ns.color, fontSize: 13 }}>
                  {selectionTypeIcons[ns.type]}
                </span>
                <div>
                  <div style={{ color: '#ddd', fontSize: 11 }}>{ns.name}</div>
                  <div style={{ color: '#667', fontSize: 10 }}>
                    {ns.faces.length} face{ns.faces.length !== 1 ? 's' : ''}
                  </div>
                </div>
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
        <div style={{ color: '#888', fontSize: 10, marginBottom: 8 }}>
          {namedSelections.length} selections ({wallCount} walls)
        </div>
      )}

      {/* Auto-name button */}
      <Button
        icon={<BgColorsOutlined />}
        onClick={handleAutoNameByNormal}
        block
        size="small"
        style={{ marginBottom: 8 }}
        disabled={!enclosureCreated}
      >
        Auto-Name by Normal
      </Button>

      {/* Add custom selection */}
      <Divider style={{ margin: '8px 0', borderColor: '#252540' }} />
      <div style={{ color: '#889', fontSize: 11, marginBottom: 4, fontWeight: 500 }}>Add custom selection</div>
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

      {namedSelections.length > 0 && (
        <div style={{ color: '#52c41a', fontSize: 11, marginTop: 6, display: 'flex', alignItems: 'center', gap: 4 }}>
          <CheckCircleOutlined /> {namedSelections.length} selection{namedSelections.length !== 1 ? 's' : ''} defined
        </div>
      )}
    </div>
  );
};

export default NamedSelectionPanel;
