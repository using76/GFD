import React from 'react';
import { Button, Divider, Collapse, Switch } from 'antd';
import { BuildOutlined, LoadingOutlined } from '@ant-design/icons';
import PropertyGrid from '../../components/PropertyGrid';
import type { PropertyField } from '../../components/PropertyGrid';
import { useAppStore } from '../../store/useAppStore';

const meshTypeFields: PropertyField[] = [
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
];

const sizeFields: PropertyField[] = [
  { key: 'globalSize', label: 'Global Size', type: 'number', min: 0.01, step: 0.01 },
  { key: 'minCellSize', label: 'Min Cell Size', type: 'number', min: 0.001, step: 0.005 },
  { key: 'growthRate', label: 'Growth Rate', type: 'number', min: 1.0, max: 2.0, step: 0.05 },
  { key: 'cellsPerFeature', label: 'Cells Per Feature', type: 'number', min: 1, max: 10, step: 1 },
];

const boundaryLayerFields: PropertyField[] = [
  { key: 'prismLayers', label: 'Number of Layers', type: 'number', min: 0, max: 20, step: 1 },
  { key: 'firstHeight', label: 'First Layer Height', type: 'number', min: 1e-6, step: 0.0001 },
  { key: 'layerRatio', label: 'Layer Growth Ratio', type: 'number', min: 1.0, max: 3.0, step: 0.1 },
  { key: 'layerTotalThickness', label: 'Total Thickness', type: 'number', min: 0.001, step: 0.001 },
];

const qualityLimitFields: PropertyField[] = [
  { key: 'maxSkewness', label: 'Max Skewness', type: 'number', min: 0.1, max: 1.0, step: 0.05 },
  { key: 'minOrthogonality', label: 'Min Orthogonality', type: 'number', min: 0, max: 1.0, step: 0.05 },
  { key: 'maxAspectRatio', label: 'Max Aspect Ratio', type: 'number', min: 1.0, max: 100, step: 1 },
];

const MeshSettings: React.FC = () => {
  const meshConfig = useAppStore((s) => s.meshConfig);
  const updateMeshConfig = useAppStore((s) => s.updateMeshConfig);
  const generateMesh = useAppStore((s) => s.generateMesh);
  const meshGenerated = useAppStore((s) => s.meshGenerated);
  const meshGenerating = useAppStore((s) => s.meshGenerating);

  const values: Record<string, unknown> = { ...meshConfig };

  const collapseItems = [
    {
      key: 'type',
      label: 'Mesh Type',
      children: (
        <PropertyGrid
          title=""
          fields={meshTypeFields}
          values={values}
          onChange={(key, value) => updateMeshConfig({ [key]: value })}
        />
      ),
    },
    {
      key: 'sizing',
      label: 'Sizing',
      children: (
        <div>
          <PropertyGrid
            title=""
            fields={sizeFields}
            values={values}
            onChange={(key, value) => updateMeshConfig({ [key]: value })}
          />
          <div style={{ padding: '4px 12px', display: 'flex', alignItems: 'center', gap: 8 }}>
            <Switch
              size="small"
              checked={meshConfig.curvatureRefine}
              onChange={(checked) => updateMeshConfig({ curvatureRefine: checked })}
            />
            <span style={{ fontSize: 12, color: '#aab' }}>Curvature Refinement</span>
          </div>
        </div>
      ),
    },
    {
      key: 'boundary_layer',
      label: 'Boundary Layers',
      children: (
        <PropertyGrid
          title=""
          fields={boundaryLayerFields}
          values={values}
          onChange={(key, value) => updateMeshConfig({ [key]: value })}
        />
      ),
    },
    {
      key: 'quality',
      label: 'Quality Limits',
      children: (
        <PropertyGrid
          title=""
          fields={qualityLimitFields}
          values={values}
          onChange={(key, value) => updateMeshConfig({ [key]: value })}
        />
      ),
    },
  ];

  return (
    <div>
      <div
        style={{
          padding: '8px 12px',
          fontWeight: 600,
          borderBottom: '1px solid #303030',
          color: '#ccd',
          fontSize: 13,
        }}
      >
        Mesh Settings
      </div>
      <Collapse
        defaultActiveKey={['type', 'sizing']}
        size="small"
        bordered={false}
        items={collapseItems}
        style={{ background: 'transparent' }}
      />
      <Divider style={{ margin: '8px 12px' }} />
      <div style={{ padding: '0 12px 12px' }}>
        <Button
          type="primary"
          icon={meshGenerating ? <LoadingOutlined /> : <BuildOutlined />}
          block
          onClick={generateMesh}
          disabled={meshGenerating}
          loading={meshGenerating}
        >
          {meshGenerating
            ? 'Generating...'
            : meshGenerated
            ? 'Regenerate Mesh'
            : 'Generate Mesh'}
        </Button>
      </div>
    </div>
  );
};

export default MeshSettings;
