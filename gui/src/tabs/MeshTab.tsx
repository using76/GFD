import React from 'react';
import {
  AppstoreOutlined,
  BorderInnerOutlined,
} from '@ant-design/icons';
import SplitLayout from '../components/SplitLayout';
import OutlineTree from '../components/OutlineTree';
import type { TreeItem } from '../components/OutlineTree';
import MeshSettings from './mesh/MeshSettings';
import QualityPanel from './mesh/QualityPanel';
import { useAppStore } from '../store/useAppStore';

interface MeshTabProps {
  viewport: React.ReactNode;
}

const MeshTab: React.FC<MeshTabProps> = ({ viewport }) => {
  const meshZones = useAppStore((s) => s.meshZones);
  const meshGenerated = useAppStore((s) => s.meshGenerated);

  const treeItems: TreeItem[] = meshGenerated
    ? [
        {
          key: 'volumes',
          title: 'Volumes',
          icon: <AppstoreOutlined />,
          children: meshZones
            .filter((z) => z.kind === 'volume')
            .map((z) => ({
              key: z.id,
              title: z.name,
              isLeaf: true,
            })),
        },
        {
          key: 'surfaces',
          title: 'Surfaces',
          icon: <BorderInnerOutlined />,
          children: meshZones
            .filter((z) => z.kind === 'surface')
            .map((z) => ({
              key: z.id,
              title: z.name,
              isLeaf: true,
            })),
        },
      ]
    : [
        {
          key: 'no-mesh',
          title: 'No mesh generated',
          isLeaf: true,
        },
      ];

  const leftPanel = (
    <div>
      <div
        style={{
          padding: '8px 12px',
          fontWeight: 600,
          borderBottom: '1px solid #303030',
        }}
      >
        Mesh Zones
      </div>
      <OutlineTree items={treeItems} />
    </div>
  );

  const rightPanel = (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <div style={{ flex: 1, overflow: 'auto' }}>
        <MeshSettings />
      </div>
      {meshGenerated && (
        <div style={{ borderTop: '1px solid #303030', overflow: 'auto', maxHeight: '50%' }}>
          <QualityPanel />
        </div>
      )}
    </div>
  );

  return (
    <SplitLayout left={leftPanel} center={viewport} right={rightPanel} />
  );
};

export default MeshTab;
