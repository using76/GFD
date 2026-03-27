import React from 'react';
import { Tooltip } from 'antd';
import {
  SelectOutlined,
  DragOutlined,
  SyncOutlined,
  ColumnWidthOutlined,
  ScissorOutlined,
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

const MiniToolbar: React.FC = () => {
  const activeTool = useAppStore((s) => s.activeTool);
  const setActiveTool = useAppStore((s) => s.setActiveTool);

  return (
    <div
      style={{
        position: 'absolute',
        top: 12,
        left: 12,
        display: 'flex',
        flexDirection: 'column',
        gap: 2,
        zIndex: 10,
        background: 'rgba(20, 20, 40, 0.85)',
        borderRadius: 6,
        padding: 3,
        border: '1px solid #303050',
        backdropFilter: 'blur(4px)',
      }}
    >
      {tools.map((tool) => {
        const isActive = activeTool === tool.key;
        return (
          <Tooltip key={tool.key} title={tool.label} placement="right">
            <div
              onClick={() => setActiveTool(tool.key)}
              style={{
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
              }}
              onMouseEnter={(e) => {
                if (!isActive) {
                  e.currentTarget.style.background = '#222244';
                  e.currentTarget.style.color = '#bbc';
                }
              }}
              onMouseLeave={(e) => {
                if (!isActive) {
                  e.currentTarget.style.background = 'transparent';
                  e.currentTarget.style.color = '#889';
                }
              }}
            >
              {tool.icon}
            </div>
          </Tooltip>
        );
      })}
    </div>
  );
};

export default MiniToolbar;
