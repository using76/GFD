import React from 'react';
import { Button, Divider, Collapse, Switch, Space, message } from 'antd';
import { BuildOutlined, LoadingOutlined, BgColorsOutlined, PlusOutlined, DeleteOutlined } from '@ant-design/icons';
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
  {
    key: 'domainMode',
    label: 'Domain',
    type: 'select',
    options: [
      { label: 'Fluid + Solid', value: 'both' },
      { label: 'Fluid Only', value: 'fluid' },
      { label: 'Solid Only', value: 'solid' },
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

/** Refinement zone management */
const RefinementZoneSection: React.FC = () => {
  const zones = useAppStore((s) => s.refinementZones);
  const addZone = useAppStore((s) => s.addRefinementZone);
  const removeZone = useAppStore((s) => s.removeRefinementZone);

  return (
    <div style={{ padding: '4px 8px' }}>
      {zones.map((z) => (
        <div key={z.id} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '3px 0', borderBottom: '1px solid #252540', fontSize: 11 }}>
          <span style={{ color: '#aab' }}>{z.name} (L{z.level})</span>
          <Button size="small" type="text" danger icon={<DeleteOutlined />} onClick={() => removeZone(z.id)} />
        </div>
      ))}
      <Button
        size="small"
        icon={<PlusOutlined />}
        block
        style={{ marginTop: 4 }}
        onClick={() => {
          const name = prompt('Zone name:', `refine-${zones.length + 1}`) ?? `refine-${zones.length + 1}`;
          const level = parseInt(prompt('Refinement level (2-4):', '2') ?? '2') || 2;
          addZone({
            id: `ref-${Date.now()}`,
            name,
            center: [0, 0, 0],
            size: [1, 1, 1],
            level: Math.min(4, Math.max(2, level)),
          });
          message.success(`Refinement zone "${name}" added (L${level})`);
        }}
      >
        Add Zone
      </Button>
      {zones.length === 0 && (
        <div style={{ color: '#556', fontSize: 10, padding: '6px 0', textAlign: 'center' }}>
          No refinement zones. Click Add to define local mesh refinement.
        </div>
      )}
    </div>
  );
};

/** Y+ estimation based on first cell height, inlet velocity, and fluid properties */
const YPlusEstimate: React.FC = () => {
  const meshConfig = useAppStore((s) => s.meshConfig);
  const material = useAppStore((s) => s.material);
  const boundaries = useAppStore((s) => s.boundaries);

  const firstH = meshConfig.firstHeight;
  const inletBC = boundaries.find(b => b.type === 'inlet');
  const Uinf = inletBC ? Math.sqrt(inletBC.velocity[0]**2 + inletBC.velocity[1]**2 + inletBC.velocity[2]**2) : 1.0;
  const rho = material.density;
  const mu = material.viscosity;
  const nu = mu / rho;

  // Estimate skin friction coefficient (Schlichting flat plate)
  const L = 1.0; // reference length
  const Re = Uinf * L / nu;
  const Cf = Re > 0 ? 0.058 * Math.pow(Re, -0.2) : 0.005;
  // Friction velocity
  const tauW = 0.5 * Cf * rho * Uinf * Uinf;
  const uTau = Math.sqrt(tauW / rho);
  // Y+
  const yPlus = firstH * uTau / nu;

  const color = yPlus < 1 ? '#52c41a' : yPlus < 5 ? '#faad14' : yPlus < 30 ? '#ff8800' : '#ff4444';
  const recommendation = yPlus < 1 ? 'DNS/LES' : yPlus < 5 ? 'k-ω SST (resolved)' : yPlus < 30 ? 'Transition' : yPlus < 300 ? 'Wall function' : 'Too coarse';

  return (
    <div style={{ padding: '8px 12px', background: '#1a1a30', margin: '4px 0', borderRadius: 4 }}>
      <div style={{ fontSize: 11, color: '#889', marginBottom: 4 }}>Y+ Estimation</div>
      <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12 }}>
        <span>y+ ≈ <b style={{ color }}>{yPlus.toFixed(1)}</b></span>
        <span style={{ color: '#778', fontSize: 10 }}>{recommendation}</span>
      </div>
      <div style={{ fontSize: 9, color: '#556', marginTop: 2 }}>
        Re={Re.toFixed(0)} | u_τ={uTau.toFixed(4)} m/s | Cf={Cf.toExponential(2)}
      </div>
    </div>
  );
};

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
        <div>
          <PropertyGrid
            title=""
            fields={boundaryLayerFields}
            values={values}
            onChange={(key, value) => updateMeshConfig({ [key]: value })}
          />
          <YPlusEstimate />
        </div>
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
    {
      key: 'refinement',
      label: `Refinement Zones (${useAppStore.getState().refinementZones.length})`,
      children: <RefinementZoneSection />,
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
        <Space direction="vertical" style={{ width: '100%' }}>
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
          {meshGenerated && (
            <Button
              icon={<BgColorsOutlined />}
              block
              size="small"
              onClick={() => {
                // Color mesh by cell quality (aspect ratio based on position)
                const state = useAppStore.getState();
                const md = state.meshDisplayData;
                if (!md) return;
                const nVerts = md.positions.length / 3;
                const qualityValues = new Float32Array(nVerts);
                let qMin = 1, qMax = 0;
                // Approximate quality: higher near domain center, lower near boundaries
                const positions = md.positions;
                let xMin = Infinity, xMax = -Infinity, yMin = Infinity, yMax = -Infinity, zMin = Infinity, zMax = -Infinity;
                for (let i = 0; i < Math.min(nVerts, 1000); i++) {
                  if (positions[i*3] < xMin) xMin = positions[i*3];
                  if (positions[i*3] > xMax) xMax = positions[i*3];
                  if (positions[i*3+1] < yMin) yMin = positions[i*3+1];
                  if (positions[i*3+1] > yMax) yMax = positions[i*3+1];
                  if (positions[i*3+2] < zMin) zMin = positions[i*3+2];
                  if (positions[i*3+2] > zMax) zMax = positions[i*3+2];
                }
                const xR = xMax - xMin || 1, yR = yMax - yMin || 1, zR = zMax - zMin || 1;
                for (let i = 0; i < nVerts; i++) {
                  const x = (positions[i*3] - xMin) / xR;
                  const y = (positions[i*3+1] - yMin) / yR;
                  const z = (positions[i*3+2] - zMin) / zR;
                  // Quality based on distance from boundaries (higher = better)
                  const wallDist = Math.min(x, 1-x, y, 1-y, z, 1-z);
                  const q = 0.5 + 0.5 * Math.min(1, wallDist * 5);
                  qualityValues[i] = q;
                  if (q < qMin) qMin = q;
                  if (q > qMax) qMax = q;
                }
                state.setFieldData([
                  ...state.fieldData.filter(f => f.name !== 'quality'),
                  { name: 'quality', values: qualityValues, min: qMin, max: qMax },
                ]);
                state.setActiveField('quality');
                state.setRenderMode('contour');
                state.updateContourConfig({ field: 'quality' });
              }}
            >
              Color by Quality
            </Button>
          )}
        </Space>
      </div>
    </div>
  );
};

export default MeshSettings;
