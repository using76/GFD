import React, { useState, useCallback, useRef } from 'react';
import { ConfigProvider, theme, message } from 'antd';
import {
  UndoOutlined,
  RedoOutlined,
  SaveOutlined,
  MenuOutlined,
  FileOutlined,
  FolderOpenOutlined,
  ExportOutlined,
  SettingOutlined,
  InfoCircleOutlined,
  QuestionCircleOutlined,
} from '@ant-design/icons';
import { useAppStore } from './store/useAppStore';
import Ribbon from './components/Ribbon';
import LeftPanelStack from './components/LeftPanelStack';
import MiniToolbar from './components/MiniToolbar';
import StatusBar from './components/StatusBar';
import Viewport3D from './engine/Viewport3D';
import ResidualPlot from './tabs/calc/ResidualPlot';
import ConsoleOutput from './tabs/calc/ConsoleOutput';

// ============================================================
// Application Menu (Blue circle button)
// ============================================================
const AppMenu: React.FC = () => {
  const [open, setOpen] = useState(false);

  const menuItems = [
    { key: 'new', icon: <FileOutlined />, label: 'New Project', action: () => { if (confirm('Create a new project?')) window.location.reload(); } },
    { key: 'open', icon: <FolderOpenOutlined />, label: 'Open...', action: () => message.info('Open project (simulated)') },
    { key: 'save', icon: <SaveOutlined />, label: 'Save', action: () => message.success('Project saved (simulated)') },
    { key: 'saveas', icon: <SaveOutlined />, label: 'Save As...', action: () => message.info('Save As (simulated)') },
    { key: 'div1', divider: true },
    { key: 'import', icon: <FolderOpenOutlined />, label: 'Import Mesh...', action: () => message.info('Import mesh (simulated)') },
    { key: 'export', icon: <ExportOutlined />, label: 'Export VTK...', action: () => message.info('Export VTK (simulated)') },
    { key: 'div2', divider: true },
    { key: 'settings', icon: <SettingOutlined />, label: 'Settings', action: () => message.info('Settings (simulated)') },
    { key: 'about', icon: <InfoCircleOutlined />, label: 'About GFD', action: () => message.info('GFD - Generalized Fluid Dynamics v0.1.0') },
  ];

  return (
    <div style={{ position: 'relative' }}>
      <div
        onClick={() => setOpen(!open)}
        style={{
          width: 30,
          height: 30,
          borderRadius: '50%',
          background: 'linear-gradient(135deg, #2060cc, #1040aa)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          cursor: 'pointer',
          boxShadow: '0 1px 4px rgba(0,0,0,0.4)',
          flexShrink: 0,
        }}
      >
        <MenuOutlined style={{ color: '#fff', fontSize: 12 }} />
      </div>

      {open && (
        <>
          {/* Backdrop */}
          <div
            onClick={() => setOpen(false)}
            style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0, zIndex: 999 }}
          />
          {/* Menu dropdown */}
          <div style={{
            position: 'absolute',
            top: 34,
            left: 0,
            width: 220,
            background: '#1a1a2e',
            border: '1px solid #303050',
            borderRadius: 6,
            padding: '4px 0',
            zIndex: 1000,
            boxShadow: '0 4px 16px rgba(0,0,0,0.5)',
          }}>
            {menuItems.map((item) => {
              if ((item as any).divider) {
                return <div key={item.key} style={{ height: 1, background: '#303050', margin: '4px 8px' }} />;
              }
              return (
                <div
                  key={item.key}
                  onClick={() => {
                    setOpen(false);
                    (item as any).action?.();
                  }}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 8,
                    padding: '6px 12px',
                    cursor: 'pointer',
                    color: '#bbc',
                    fontSize: 12,
                  }}
                  onMouseEnter={(e) => { e.currentTarget.style.background = '#252540'; }}
                  onMouseLeave={(e) => { e.currentTarget.style.background = 'transparent'; }}
                >
                  <span style={{ fontSize: 13, width: 16, textAlign: 'center', color: '#889' }}>
                    {(item as any).icon}
                  </span>
                  {item.label}
                </div>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
};

// ============================================================
// Quick Access Toolbar
// ============================================================
const QuickAccess: React.FC = () => (
  <div style={{ display: 'flex', alignItems: 'center', gap: 2 }}>
    {[
      { icon: <UndoOutlined />, tip: 'Undo', action: () => message.info('Undo (simulated)') },
      { icon: <RedoOutlined />, tip: 'Redo', action: () => message.info('Redo (simulated)') },
      { icon: <SaveOutlined />, tip: 'Save', action: () => message.success('Saved (simulated)') },
    ].map((btn, i) => (
      <div
        key={i}
        onClick={btn.action}
        title={btn.tip}
        style={{
          width: 24,
          height: 24,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          borderRadius: 3,
          cursor: 'pointer',
          color: '#889',
          fontSize: 13,
        }}
        onMouseEnter={(e) => { e.currentTarget.style.background = '#252540'; e.currentTarget.style.color = '#bbc'; }}
        onMouseLeave={(e) => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = '#889'; }}
      >
        {btn.icon}
      </div>
    ))}
  </div>
);

// ============================================================
// Resizable Left Panel Wrapper
// ============================================================
const LEFT_MIN = 200;
const LEFT_MAX = 500;
const LEFT_DEFAULT = 270;

const ResizableLeftPanel: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [width, setWidth] = useState(LEFT_DEFAULT);
  const dragging = useRef(false);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    dragging.current = true;
    const startX = e.clientX;
    const startW = width;

    const onMove = (ev: MouseEvent) => {
      if (!dragging.current) return;
      const newW = Math.max(LEFT_MIN, Math.min(LEFT_MAX, startW + ev.clientX - startX));
      setWidth(newW);
    };
    const onUp = () => {
      dragging.current = false;
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }, [width]);

  return (
    <div style={{ display: 'flex', flexShrink: 0, height: '100%' }}>
      <div style={{ width, minWidth: LEFT_MIN, height: '100%', overflow: 'hidden', borderRight: '1px solid #252540' }}>
        {children}
      </div>
      <div
        onMouseDown={onMouseDown}
        style={{ width: 4, cursor: 'col-resize', background: '#1a1a30', flexShrink: 0 }}
        onMouseEnter={(e) => { e.currentTarget.style.background = '#303060'; }}
        onMouseLeave={(e) => { e.currentTarget.style.background = '#1a1a30'; }}
      />
    </div>
  );
};

// ============================================================
// Center Content (viewport or calc views)
// ============================================================
const CenterContent: React.FC = () => {
  const activeRibbonTab = useAppStore((s) => s.activeRibbonTab);

  // For Calculation tab, show residual plot or console instead of viewport
  if (activeRibbonTab === 'calc') {
    return (
      <div style={{ width: '100%', height: '100%', display: 'flex', flexDirection: 'column' }}>
        <div style={{ flex: 1, minHeight: 0 }}>
          <ResidualPlot />
        </div>
        <div style={{ height: 200, borderTop: '1px solid #252540', flexShrink: 0 }}>
          <ConsoleOutput />
        </div>
      </div>
    );
  }

  // For all other tabs, show 3D viewport with mini toolbar
  return (
    <div style={{ width: '100%', height: '100%', position: 'relative' }}>
      <Viewport3D />
      <MiniToolbar />
    </div>
  );
};

// ============================================================
// Main App
// ============================================================
export default function App() {
  const TITLE_BAR_H = 36;
  const STATUS_BAR_H = 28;

  return (
    <ConfigProvider
      theme={{
        algorithm: theme.darkAlgorithm,
        token: { colorPrimary: '#4096ff', borderRadius: 4, fontSize: 12 },
      }}
    >
      <style>{`
        html, body, #root { margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden; background: #0a0a18; }
        ::-webkit-scrollbar { width: 6px; height: 6px; }
        ::-webkit-scrollbar-track { background: #111122; }
        ::-webkit-scrollbar-thumb { background: #333355; border-radius: 3px; }
        ::-webkit-scrollbar-thumb:hover { background: #444466; }
        .ant-tree { background: transparent !important; }
        .ant-tree .ant-tree-node-content-wrapper { color: #aab !important; font-size: 12px !important; }
        .ant-tree .ant-tree-node-content-wrapper:hover { background: #1a1a3a !important; }
        .ant-tree .ant-tree-node-content-wrapper.ant-tree-node-selected { background: #2a2a5a !important; color: #fff !important; }
        .ant-form-item-label > label { color: #889 !important; font-size: 11px !important; }
        .ant-select-selector { background: #1a1a30 !important; border-color: #303050 !important; }
        .ant-input-number, .ant-input { background: #1a1a30 !important; border-color: #303050 !important; }
      `}</style>

      <div style={{ width: '100vw', height: '100vh', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>

        {/* ============ Title Bar ============ */}
        <div style={{
          height: TITLE_BAR_H,
          background: '#12122a',
          borderBottom: '1px solid #252540',
          display: 'flex',
          alignItems: 'center',
          padding: '0 10px',
          gap: 10,
          flexShrink: 0,
        }}>
          <AppMenu />
          <QuickAccess />
          <div style={{ flex: 1 }} />
          <span style={{ fontSize: 12, color: '#556', fontWeight: 500, letterSpacing: 0.5 }}>
            GFD - Generalized Fluid Dynamics
          </span>
          <div style={{ flex: 1 }} />
          <QuestionCircleOutlined style={{ color: '#445', cursor: 'pointer', fontSize: 14 }} onClick={() => message.info('Help: see PROJECT_PLAN.md')} />
        </div>

        {/* ============ Ribbon ============ */}
        <Ribbon />

        {/* ============ Main Content: Left Panel + Center ============ */}
        <div style={{ flex: 1, display: 'flex', overflow: 'hidden', minHeight: 0 }}>

          {/* Left Panel Stack */}
          <ResizableLeftPanel>
            <LeftPanelStack />
          </ResizableLeftPanel>

          {/* Center: Viewport / Calc content */}
          <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden', minWidth: 200 }}>
            <div style={{ flex: 1, minHeight: 0, overflow: 'hidden' }}>
              <CenterContent />
            </div>
          </div>
        </div>

        {/* ============ Status Bar ============ */}
        <div style={{
          height: STATUS_BAR_H,
          background: '#12122a',
          borderTop: '1px solid #252540',
          flexShrink: 0,
        }}>
          <StatusBar />
        </div>
      </div>
    </ConfigProvider>
  );
}
