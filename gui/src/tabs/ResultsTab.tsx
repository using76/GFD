import React, { useState, useEffect } from 'react';
import {
  HeatMapOutlined,
  ArrowsAltOutlined,
  SwapOutlined,
  FileTextOutlined,
} from '@ant-design/icons';
import SplitLayout from '../components/SplitLayout';
import OutlineTree from '../components/OutlineTree';
import type { TreeItem } from '../components/OutlineTree';
import ContourSettings from './results/ContourSettings';
import VectorSettings from './results/VectorSettings';
import StreamlineSettings from './results/StreamlineSettings';
import IsoSurfaceSettings from './results/IsoSurfaceSettings';
import ReportPanel from './results/ReportPanel';

interface ResultsTabProps {
  viewport: React.ReactNode;
}

type ResultsSection = 'contours' | 'vectors' | 'streamlines' | 'isosurface' | 'reports';

const treeItems: TreeItem[] = [
  {
    key: 'display',
    title: 'Display',
    children: [
      { key: 'contours', title: 'Contours', icon: <HeatMapOutlined />, isLeaf: true },
      { key: 'vectors', title: 'Vectors', icon: <ArrowsAltOutlined />, isLeaf: true },
      { key: 'streamlines', title: 'Streamlines', icon: <SwapOutlined />, isLeaf: true },
      { key: 'isosurface', title: 'Iso-Surface', icon: <SwapOutlined />, isLeaf: true },
    ],
  },
  {
    key: 'reports',
    title: 'Reports',
    icon: <FileTextOutlined />,
    isLeaf: true,
  },
];

const panelMap: Record<ResultsSection, React.ReactNode> = {
  contours: <ContourSettings />,
  vectors: <VectorSettings />,
  streamlines: <StreamlineSettings />,
  isosurface: <IsoSurfaceSettings />,
  reports: <ReportPanel />,
};

const ResultsTab: React.FC<ResultsTabProps> = ({ viewport }) => {
  const [section, setSection] = useState<ResultsSection>('contours');

  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail?.section && ['contours', 'vectors', 'streamlines', 'isosurface', 'reports'].includes(detail.section)) {
        setSection(detail.section as ResultsSection);
      }
    };
    window.addEventListener('gfd-results-section', handler);
    return () => window.removeEventListener('gfd-results-section', handler);
  }, []);

  const leftPanel = (
    <div>
      <div
        style={{
          padding: '8px 12px',
          fontWeight: 600,
          borderBottom: '1px solid #303030',
        }}
      >
        Results
      </div>
      <OutlineTree
        items={treeItems}
        selectedKey={section}
        onSelect={(key) => setSection(key as ResultsSection)}
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

export default ResultsTab;
