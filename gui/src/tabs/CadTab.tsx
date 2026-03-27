import React, { useState } from 'react';
import SplitLayout from '../components/SplitLayout';
import PrimitiveToolbar from './cad/PrimitiveToolbar';
import CadTree from './cad/CadTree';
import ShapeProperties from './cad/ShapeProperties';
import DefeaturingPanel from './cad/DefeaturingPanel';
import CfdPrepPanel from './cad/CfdPrepPanel';

interface CadTabProps {
  viewport: React.ReactNode;
}

const rightPanelTabs = [
  { key: 'props', label: 'Properties' },
  { key: 'defeaturing', label: 'Defeaturing' },
  { key: 'cfdprep', label: 'CFD Prep' },
];

const CadTab: React.FC<CadTabProps> = ({ viewport }) => {
  const [rightTab, setRightTab] = useState('props');

  const rightContent = (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      {/* Mini tab bar for the right panel */}
      <div
        style={{
          display: 'flex',
          borderBottom: '1px solid #303030',
          background: '#1a1a1a',
          flexShrink: 0,
        }}
      >
        {rightPanelTabs.map((tab) => (
          <div
            key={tab.key}
            onClick={() => setRightTab(tab.key)}
            style={{
              padding: '6px 10px',
              fontSize: 11,
              cursor: 'pointer',
              color: rightTab === tab.key ? '#fff' : '#777',
              borderBottom:
                rightTab === tab.key
                  ? '2px solid #1677ff'
                  : '2px solid transparent',
              userSelect: 'none',
              whiteSpace: 'nowrap',
            }}
          >
            {tab.label}
          </div>
        ))}
      </div>
      {/* Panel content */}
      <div style={{ flex: 1, overflow: 'auto' }}>
        {rightTab === 'props' && <ShapeProperties />}
        {rightTab === 'defeaturing' && <DefeaturingPanel />}
        {rightTab === 'cfdprep' && <CfdPrepPanel />}
      </div>
    </div>
  );

  return (
    <div style={{ width: '100%', height: '100%', display: 'flex', flexDirection: 'column' }}>
      <PrimitiveToolbar />
      <div style={{ flex: 1, overflow: 'hidden' }}>
        <SplitLayout
          left={<CadTree />}
          center={viewport}
          right={rightContent}
        />
      </div>
    </div>
  );
};

export default CadTab;
