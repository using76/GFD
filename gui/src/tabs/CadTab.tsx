import React from 'react';
import SplitLayout from '../components/SplitLayout';
import PrimitiveToolbar from './cad/PrimitiveToolbar';
import CadTree from './cad/CadTree';
import ShapeProperties from './cad/ShapeProperties';

interface CadTabProps {
  viewport: React.ReactNode;
}

const CadTab: React.FC<CadTabProps> = ({ viewport }) => {
  return (
    <div style={{ width: '100%', height: '100%', display: 'flex', flexDirection: 'column' }}>
      <PrimitiveToolbar />
      <div style={{ flex: 1, overflow: 'hidden' }}>
        <SplitLayout
          left={<CadTree />}
          center={viewport}
          right={<ShapeProperties />}
        />
      </div>
    </div>
  );
};

export default CadTab;
