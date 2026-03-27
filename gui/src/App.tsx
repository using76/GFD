import { ConfigProvider, Layout, Tabs, theme } from 'antd';
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

const { Header, Footer } = Layout;

const viewport = <Viewport3D />;

const TAB_ITEMS = [
  {
    key: 'cad',
    label: (
      <span>
        <ToolOutlined /> CAD
      </span>
    ),
    children: <CadTab viewport={viewport} />,
  },
  {
    key: 'mesh',
    label: (
      <span>
        <AppstoreOutlined /> Mesh
      </span>
    ),
    children: <MeshTab viewport={viewport} />,
  },
  {
    key: 'setup',
    label: (
      <span>
        <ControlOutlined /> Setup
      </span>
    ),
    children: <SetupTab viewport={viewport} />,
  },
  {
    key: 'calc',
    label: (
      <span>
        <CalculatorOutlined /> Calculation
      </span>
    ),
    children: <CalcTab />,
  },
  {
    key: 'results',
    label: (
      <span>
        <LineChartOutlined /> Results
      </span>
    ),
    children: <ResultsTab viewport={viewport} />,
  },
];

export default function App() {
  const activeTab = useAppStore((s) => s.activeTab);
  const setActiveTab = useAppStore((s) => s.setActiveTab);

  return (
    <ConfigProvider
      theme={{
        algorithm: theme.darkAlgorithm,
        token: {
          colorPrimary: '#1677ff',
          borderRadius: 4,
          fontSize: 13,
        },
      }}
    >
      <Layout style={{ height: '100vh', overflow: 'hidden' }}>
        {/* Top Menu */}
        <Header
          style={{
            height: 40,
            lineHeight: '40px',
            padding: '0 12px',
            background: '#1a1a2e',
            borderBottom: '1px solid #303050',
          }}
        >
          <MenuBar />
        </Header>

        {/* Tab Bar + Content: each tab owns its own left/center/right split */}
        <div
          style={{
            flex: 1,
            display: 'flex',
            flexDirection: 'column',
            overflow: 'hidden',
          }}
        >
          <Tabs
            activeKey={activeTab}
            onChange={setActiveTab}
            type="card"
            size="small"
            items={TAB_ITEMS}
            style={{
              flex: 1,
              display: 'flex',
              flexDirection: 'column',
            }}
            tabBarStyle={{
              margin: 0,
              paddingLeft: 12,
              background: '#16213e',
              borderBottom: '1px solid #303050',
            }}
          />
        </div>

        {/* Bottom Status Bar */}
        <Footer
          style={{
            height: 28,
            lineHeight: '28px',
            padding: '0 12px',
            background: '#1a1a2e',
            borderTop: '1px solid #303050',
          }}
        >
          <StatusBar />
        </Footer>
      </Layout>
    </ConfigProvider>
  );
}
