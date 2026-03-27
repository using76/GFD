import React from 'react';
import PropertyGrid from '../../components/PropertyGrid';
import type { PropertyField } from '../../components/PropertyGrid';
import { useAppStore } from '../../store/useAppStore';

const fields: PropertyField[] = [
  { key: 'name', label: 'Material Name', type: 'string' },
  { key: 'density', label: 'Density (kg/m3)', type: 'number', min: 0, step: 0.1 },
  { key: 'viscosity', label: 'Dynamic Viscosity (Pa.s)', type: 'number', min: 0, step: 1e-6 },
  { key: 'cp', label: 'Specific Heat Cp (J/kg.K)', type: 'number', min: 0, step: 1 },
  { key: 'conductivity', label: 'Thermal Conductivity (W/m.K)', type: 'number', min: 0, step: 0.001 },
];

const MaterialPanel: React.FC = () => {
  const material = useAppStore((s) => s.material);
  const updateMaterial = useAppStore((s) => s.updateMaterial);

  const values: Record<string, unknown> = { ...material };

  return (
    <PropertyGrid
      title="Material Properties"
      fields={fields}
      values={values}
      onChange={(key, value) => updateMaterial({ [key]: value })}
    />
  );
};

export default MaterialPanel;
