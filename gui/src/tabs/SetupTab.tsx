import React, { useState } from 'react';
import {
  ExperimentOutlined,
  GoldOutlined,
  BlockOutlined,
  SettingOutlined,
} from '@ant-design/icons';
import SplitLayout from '../components/SplitLayout';
import OutlineTree from '../components/OutlineTree';
import type { TreeItem } from '../components/OutlineTree';
import ModelsPanel from './setup/ModelsPanel';
import MaterialPanel from './setup/MaterialPanel';
import BoundaryPanel from './setup/BoundaryPanel';
import SolverSettingsPanel from './setup/SolverSettingsPanel';

interface SetupTabProps {
  viewport: React.ReactNode;
}

type SetupSection = 'models' | 'materials' | 'boundaries' | 'solver';

const treeItems: TreeItem[] = [
  {
    key: 'models',
    title: 'Models',
    icon: <ExperimentOutlined />,
    children: [
      { key: 'models', title: 'Viscous / Energy / Multiphase', isLeaf: true },
    ],
  },
  {
    key: 'materials',
    title: 'Materials',
    icon: <GoldOutlined />,
    isLeaf: true,
  },
  {
    key: 'boundaries',
    title: 'Boundary Conditions',
    icon: <BlockOutlined />,
    isLeaf: true,
  },
  {
    key: 'solver',
    title: 'Solver Settings',
    icon: <SettingOutlined />,
    isLeaf: true,
  },
];

const panelMap: Record<SetupSection, React.ReactNode> = {
  models: <ModelsPanel />,
  materials: <MaterialPanel />,
  boundaries: <BoundaryPanel />,
  solver: <SolverSettingsPanel />,
};

const SetupTab: React.FC<SetupTabProps> = ({ viewport }) => {
  const [section, setSection] = useState<SetupSection>('models');

  const leftPanel = (
    <div>
      <div
        style={{
          padding: '8px 12px',
          fontWeight: 600,
          borderBottom: '1px solid #303030',
        }}
      >
        Setup
      </div>
      <OutlineTree
        items={treeItems}
        selectedKey={section}
        onSelect={(key) => setSection(key as SetupSection)}
      />
    </div>
  );

  return (
    <SplitLayout
      left={leftPanel}
      center={viewport}
      right={panelMap[section] ?? <div />}
    />
  );
};

export default SetupTab;
