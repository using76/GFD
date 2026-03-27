import React from 'react';
import { Button, Divider } from 'antd';
import { BuildOutlined } from '@ant-design/icons';
import PropertyGrid from '../../components/PropertyGrid';
import type { PropertyField } from '../../components/PropertyGrid';
import { useAppStore } from '../../store/useAppStore';

const fields: PropertyField[] = [
  {
    key: 'type',
    label: 'Mesh Type',
    type: 'select',
    options: [
      { label: 'Cartesian', value: 'cartesian' },
      { label: 'Tetrahedral', value: 'tet' },
      { label: 'Hexahedral', value: 'hex' },
      { label: 'Polyhedral', value: 'poly' },
      { label: 'Cut-Cell', value: 'cutcell' },
    ],
  },
  { key: 'globalSize', label: 'Global Size', type: 'number', min: 0.001, step: 0.01 },
  { key: 'growthRate', label: 'Growth Rate', type: 'number', min: 1.0, max: 2.0, step: 0.05 },
  { key: 'prismLayers', label: 'Prism Layers', type: 'number', min: 0, max: 20, step: 1 },
  { key: 'firstHeight', label: 'First Layer Height', type: 'number', min: 1e-6, step: 0.0001 },
  { key: 'layerRatio', label: 'Layer Growth Ratio', type: 'number', min: 1.0, max: 3.0, step: 0.1 },
];

const MeshSettings: React.FC = () => {
  const meshConfig = useAppStore((s) => s.meshConfig);
  const updateMeshConfig = useAppStore((s) => s.updateMeshConfig);
  const generateMesh = useAppStore((s) => s.generateMesh);
  const meshGenerated = useAppStore((s) => s.meshGenerated);

  const values: Record<string, unknown> = { ...meshConfig };

  return (
    <div>
      <PropertyGrid
        title="Mesh Settings"
        fields={fields}
        values={values}
        onChange={(key, value) => updateMeshConfig({ [key]: value })}
      />
      <Divider style={{ margin: '8px 12px' }} />
      <div style={{ padding: '0 12px 12px' }}>
        <Button
          type="primary"
          icon={<BuildOutlined />}
          block
          onClick={generateMesh}
        >
          {meshGenerated ? 'Regenerate Mesh' : 'Generate Mesh'}
        </Button>
      </div>
    </div>
  );
};

export default MeshSettings;
