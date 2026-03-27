import React from 'react';
import { Tooltip } from 'antd';
import {
  SelectOutlined,
  DragOutlined,
  SyncOutlined,
  ColumnWidthOutlined,
  ScissorOutlined,
  CompressOutlined,
  BorderOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../store/useAppStore';
import type { ActiveTool } from '../store/useAppStore';

interface ToolButton {
  key: ActiveTool;
  icon: React.ReactNode;
  label: string;
}

const tools: ToolButton[] = [
  { key: 'select', icon: <SelectOutlined />, label: 'Select' },
  { key: 'move', icon: <DragOutlined />, label: 'Move' },
  { key: 'pull', icon: <SyncOutlined />, label: 'Rotate / Pull' },
  { key: 'measure', icon: <ColumnWidthOutlined />, label: 'Measure' },
  { key: 'section', icon: <ScissorOutlined />, label: 'Section' },
];

interface ViewPreset {
  label: string;
  abbr: string;
  position: [number, number, number];
}

const VIEW_PRESETS: ViewPreset[] = [
  { label: 'Front', abbr: 'F', position: [0, 0, 8] },
  { label: 'Back', abbr: 'Bk', position: [0, 0, -8] },
  { label: 'Top', abbr: 'T', position: [0, 8, 0.01] },
  { label: 'Bottom', abbr: 'Bt', position: [0, -8, 0.01] },
  { label: 'Left', abbr: 'L', position: [-8, 0, 0] },
  { label: 'Right', abbr: 'R', position: [8, 0, 0] },
  { label: 'Iso', abbr: 'I', position: [5, 5, 5] },
];

function dispatchCameraEvent(position: [number, number, number]) {
  window.dispatchEvent(
    new CustomEvent('gfd-camera-preset', { detail: { position } })
  );
}

const MiniToolbar: React.FC = () => {
  const activeTool = useAppStore((s) => s.activeTool);
  const setActiveTool = useAppStore((s) => s.setActiveTool);
  const cameraMode = useAppStore((s) => s.cameraMode);
  const setCameraMode = useAppStore((s) => s.setCameraMode);

  const btnStyle = (isActive: boolean): React.CSSProperties => ({
    width: 32,
    height: 32,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    borderRadius: 4,
    cursor: 'pointer',
    background: isActive ? '#2a3a6a' : 'transparent',
    color: isActive ? '#4096ff' : '#889',
    fontSize: 16,
    transition: 'all 0.12s',
  });

  const hoverHandlers = (isActive: boolean) => ({
    onMouseEnter: (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isActive) { e.currentTarget.style.background = '#222244'; e.currentTarget.style.color = '#bbc'; }
    },
    onMouseLeave: (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isActive) { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = '#889'; }
    },
  });

  return (
    <div style={{
      position: 'absolute',
      top: 12,
      left: 12,
      display: 'flex',
      flexDirection: 'column',
      gap: 2,
      zIndex: 10,
    }}>
      {/* Tool buttons */}
      <div style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 2,
        background: 'rgba(20, 20, 40, 0.85)',
        borderRadius: 6,
        padding: 3,
        border: '1px solid #303050',
        backdropFilter: 'blur(4px)',
      }}>
        {tools.map((tool) => {
          const isActive = activeTool === tool.key;
          return (
            <Tooltip key={tool.key} title={tool.label} placement="right">
              <div onClick={() => setActiveTool(tool.key)} style={btnStyle(isActive)} {...hoverHandlers(isActive)}>
                {tool.icon}
              </div>
            </Tooltip>
          );
        })}
      </div>

      {/* View preset buttons */}
      <div style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 1,
        background: 'rgba(20, 20, 40, 0.85)',
        borderRadius: 6,
        padding: 3,
        border: '1px solid #303050',
        backdropFilter: 'blur(4px)',
        marginTop: 4,
      }}>
        {VIEW_PRESETS.map((preset) => (
          <Tooltip key={preset.label} title={preset.label} placement="right">
            <div
              onClick={() => dispatchCameraEvent(preset.position)}
              style={{
                width: 32,
                height: 22,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                borderRadius: 3,
                cursor: 'pointer',
                color: '#778',
                fontSize: 10,
                fontWeight: 600,
                letterSpacing: 0.5,
              }}
              onMouseEnter={(e) => { e.currentTarget.style.background = '#222244'; e.currentTarget.style.color = '#bbc'; }}
              onMouseLeave={(e) => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = '#778'; }}
            >
              {preset.abbr}
            </div>
          </Tooltip>
        ))}

        {/* Separator */}
        <div style={{ height: 1, background: '#303050', margin: '2px 4px' }} />

        {/* Fit All */}
        <Tooltip title="Fit All" placement="right">
          <div
            onClick={() => dispatchCameraEvent([5, 5, 5])}
            style={{
              width: 32,
              height: 24,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              borderRadius: 3,
              cursor: 'pointer',
              color: '#778',
              fontSize: 13,
            }}
            onMouseEnter={(e) => { e.currentTarget.style.background = '#222244'; e.currentTarget.style.color = '#bbc'; }}
            onMouseLeave={(e) => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = '#778'; }}
          >
            <CompressOutlined />
          </div>
        </Tooltip>

        {/* Camera toggle */}
        <Tooltip title={cameraMode.type === 'perspective' ? 'Switch to Orthographic' : 'Switch to Perspective'} placement="right">
          <div
            onClick={() => setCameraMode({ type: cameraMode.type === 'perspective' ? 'orthographic' : 'perspective' })}
            style={{
              width: 32,
              height: 24,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              borderRadius: 3,
              cursor: 'pointer',
              color: '#778',
              fontSize: 13,
            }}
            onMouseEnter={(e) => { e.currentTarget.style.background = '#222244'; e.currentTarget.style.color = '#bbc'; }}
            onMouseLeave={(e) => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = '#778'; }}
          >
            <BorderOutlined />
          </div>
        </Tooltip>
      </div>
    </div>
  );
};

export default MiniToolbar;
