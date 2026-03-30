import React from 'react';
import { Form, InputNumber, Select, Typography, Divider } from 'antd';
import { useAppStore } from '../../store/useAppStore';
import type { Material } from '../../store/useAppStore';

const materialPresets: Record<string, Material> = {
  Air: { name: 'Air', density: 1.225, viscosity: 1.789e-5, cp: 1006.43, conductivity: 0.0242 },
  Water: { name: 'Water', density: 998.2, viscosity: 1.003e-3, cp: 4182.0, conductivity: 0.6 },
  'Engine Oil': { name: 'Engine Oil', density: 884, viscosity: 0.486, cp: 1909, conductivity: 0.145 },
  Glycerin: { name: 'Glycerin', density: 1261, viscosity: 1.412, cp: 2427, conductivity: 0.286 },
  Mercury: { name: 'Mercury', density: 13546, viscosity: 1.526e-3, cp: 139.3, conductivity: 8.54 },
  Ethanol: { name: 'Ethanol', density: 789, viscosity: 1.2e-3, cp: 2440, conductivity: 0.171 },
  'Natural Gas': { name: 'Natural Gas', density: 0.668, viscosity: 1.087e-5, cp: 2222, conductivity: 0.0332 },
  Steam: { name: 'Steam', density: 0.5977, viscosity: 1.34e-5, cp: 2010, conductivity: 0.0261 },
  Blood: { name: 'Blood', density: 1060, viscosity: 3.5e-3, cp: 3617, conductivity: 0.52 },
  Steel: { name: 'Steel', density: 7850, viscosity: 0, cp: 434, conductivity: 60.5 },
  Aluminum: { name: 'Aluminum', density: 2719, viscosity: 0, cp: 871, conductivity: 202.4 },
  Copper: { name: 'Copper', density: 8933, viscosity: 0, cp: 385, conductivity: 401 },
  Titanium: { name: 'Titanium', density: 4506, viscosity: 0, cp: 523, conductivity: 21.9 },
  Custom: { name: 'Custom', density: 1.0, viscosity: 1e-5, cp: 1000, conductivity: 0.1 },
};

const MaterialPanel: React.FC = () => {
  const material = useAppStore((s) => s.material);
  const updateMaterial = useAppStore((s) => s.updateMaterial);

  const handlePresetChange = (presetName: string) => {
    const preset = materialPresets[presetName];
    if (preset) {
      updateMaterial(preset);
    }
  };

  return (
    <div style={{ padding: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 12, fontSize: 14, borderBottom: '1px solid #303030', paddingBottom: 8 }}>
        Material Properties
      </div>

      <Form layout="vertical" size="small">
        <Form.Item label="Material Preset">
          <Select
            value={material.name}
            onChange={handlePresetChange}
            options={Object.keys(materialPresets).map((k) => ({ label: k, value: k }))}
          />
        </Form.Item>

        <Divider style={{ margin: '8px 0' }} />

        <Form.Item label="Material Name">
          <Typography.Text style={{ fontSize: 12, color: '#ccd' }}>{material.name}</Typography.Text>
        </Form.Item>

        <Form.Item label="Density (kg/m3)">
          <InputNumber
            value={material.density}
            min={0}
            step={0.1}
            style={{ width: '100%' }}
            onChange={(v) => updateMaterial({ density: v ?? 1.0, name: 'Custom' })}
          />
        </Form.Item>

        <Form.Item label="Dynamic Viscosity (Pa.s)">
          <InputNumber
            value={material.viscosity}
            min={0}
            step={1e-6}
            style={{ width: '100%' }}
            onChange={(v) => updateMaterial({ viscosity: v ?? 1e-5, name: 'Custom' })}
          />
        </Form.Item>

        <Form.Item label="Specific Heat Cp (J/kg.K)">
          <InputNumber
            value={material.cp}
            min={0}
            step={1}
            style={{ width: '100%' }}
            onChange={(v) => updateMaterial({ cp: v ?? 1000, name: 'Custom' })}
          />
        </Form.Item>

        <Form.Item label="Thermal Conductivity (W/m.K)">
          <InputNumber
            value={material.conductivity}
            min={0}
            step={0.001}
            style={{ width: '100%' }}
            onChange={(v) => updateMaterial({ conductivity: v ?? 0.1, name: 'Custom' })}
          />
        </Form.Item>
      </Form>

      {/* Info card */}
      <div style={{ padding: 8, background: '#1a1a30', borderRadius: 4, fontSize: 11, color: '#778', marginTop: 8 }}>
        {material.name === 'Air' && 'Standard air at 20 C and 1 atm.'}
        {material.name === 'Water' && 'Liquid water at 20 C and 1 atm.'}
        {material.name === 'Steel' && 'Structural carbon steel (AISI 1020). Viscosity = 0 for solid.'}
        {material.name === 'Aluminum' && 'Pure aluminum (Al 6061). Viscosity = 0 for solid.'}
        {material.name === 'Custom' && 'Custom material properties. Edit values above.'}
      </div>
    </div>
  );
};

export default MaterialPanel;
