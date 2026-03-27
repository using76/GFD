import React from 'react';
import {
  LineChartOutlined,
  MonitorOutlined,
} from '@ant-design/icons';
import SplitLayout from '../components/SplitLayout';
import OutlineTree from '../components/OutlineTree';
import type { TreeItem } from '../components/OutlineTree';
import RunControls from './calc/RunControls';
import ResidualPlot from './calc/ResidualPlot';
import ConsoleOutput from './calc/ConsoleOutput';

const treeItems: TreeItem[] = [
  {
    key: 'monitors',
    title: 'Monitors',
    children: [
      {
        key: 'residuals',
        title: 'Residuals',
        icon: <LineChartOutlined />,
        isLeaf: true,
      },
      {
        key: 'console',
        title: 'Console',
        icon: <MonitorOutlined />,
        isLeaf: true,
      },
    ],
  },
];

const CalcTab: React.FC = () => {
  const [view, setView] = React.useState<'residuals' | 'console'>('residuals');

  const centerContent =
    view === 'residuals' ? <ResidualPlot /> : <ConsoleOutput />;

  const leftPanel = (
    <div>
      <div
        style={{
          padding: '8px 12px',
          fontWeight: 600,
          borderBottom: '1px solid #303030',
        }}
      >
        Monitors
      </div>
      <OutlineTree
        items={treeItems}
        selectedKey={view}
        onSelect={(key) => setView(key as 'residuals' | 'console')}
      />
    </div>
  );

  return (
    <SplitLayout
      left={leftPanel}
      center={centerContent}
      right={<RunControls />}
    />
  );
};

export default CalcTab;
