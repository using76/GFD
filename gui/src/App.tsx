import React from 'react';
import { ConfigProvider, theme } from 'antd';
import {
  ToolOutlined,
  AppstoreOutlined,
  ControlOutlined,
  CalculatorOutlined,
  LineChartOutlined,
} from '@ant-design/icons';
import { useAppStore } from './store/useAppStore';
import MenuBar from './components/MenuBar';
import StatusBar from './components/StatusBar';
import Viewport3D from './engine/Viewport3D';
import CadTab from './tabs/CadTab';
import MeshTab from './tabs/MeshTab';
import SetupTab from './tabs/SetupTab';
import CalcTab from './tabs/CalcTab';
import ResultsTab from './tabs/ResultsTab';

const viewport = <Viewport3D />;

// Tab content is rendered dynamically in App component

export default function App() {
  const activeTab = useAppStore((s) => s.activeTab);
  const setActiveTab = useAppStore((s) => s.setActiveTab);

  const HEADER_H = 40;
  const TAB_BAR_H = 40;
  const FOOTER_H = 28;
  const contentH = `calc(100vh - ${HEADER_H + TAB_BAR_H + FOOTER_H}px)`;

  const tabContent: Record<string, React.ReactNode> = {
    cad: <CadTab viewport={viewport} />,
    mesh: <MeshTab viewport={viewport} />,
    setup: <SetupTab viewport={viewport} />,
    calc: <CalcTab />,
    results: <ResultsTab viewport={viewport} />,
  };

  return (
    <ConfigProvider
      theme={{
        algorithm: theme.darkAlgorithm,
        token: { colorPrimary: '#1677ff', borderRadius: 4, fontSize: 13 },
      }}
    >
      <style>{`
        html, body, #root { margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden; background: #0a0a1a; }
      `}</style>

      <div style={{ width: '100vw', height: '100vh', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
        {/* Header */}
        <div style={{ height: HEADER_H, background: '#1a1a2e', borderBottom: '1px solid #303050', padding: '0 12px', lineHeight: `${HEADER_H}px`, flexShrink: 0 }}>
          <MenuBar />
        </div>

        {/* Tab Bar */}
        <div style={{ height: TAB_BAR_H, background: '#16213e', borderBottom: '1px solid #303050', display: 'flex', alignItems: 'center', gap: 0, flexShrink: 0, paddingLeft: 8 }}>
          {[
            { key: 'cad', icon: <ToolOutlined />, label: 'CAD' },
            { key: 'mesh', icon: <AppstoreOutlined />, label: 'Mesh' },
            { key: 'setup', icon: <ControlOutlined />, label: 'Setup' },
            { key: 'calc', icon: <CalculatorOutlined />, label: 'Calculation' },
            { key: 'results', icon: <LineChartOutlined />, label: 'Results' },
          ].map(tab => (
            <div
              key={tab.key}
              onClick={() => setActiveTab(tab.key as any)}
              style={{
                padding: '6px 16px',
                cursor: 'pointer',
                color: activeTab === tab.key ? '#fff' : '#888',
                background: activeTab === tab.key ? '#1a1a2e' : 'transparent',
                borderRadius: '6px 6px 0 0',
                borderBottom: activeTab === tab.key ? '2px solid #1677ff' : '2px solid transparent',
                fontSize: 13,
                display: 'flex',
                alignItems: 'center',
                gap: 6,
                userSelect: 'none',
                transition: 'all 0.15s',
              }}
            >
              {tab.icon} {tab.label}
            </div>
          ))}
        </div>

        {/* Content Area — fixed height, no flex ambiguity */}
        <div style={{ height: contentH, overflow: 'hidden' }}>
          {tabContent[activeTab] || <div>Select a tab</div>}
        </div>

        {/* Footer */}
        <div style={{ height: FOOTER_H, background: '#1a1a2e', borderTop: '1px solid #303050', padding: '0 12px', lineHeight: `${FOOTER_H}px`, flexShrink: 0, fontSize: 12 }}>
          <StatusBar />
        </div>
      </div>
    </ConfigProvider>
  );
}
