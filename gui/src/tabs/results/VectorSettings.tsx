import React from 'react';
import PropertyGrid from '../../components/PropertyGrid';
import type { PropertyField } from '../../components/PropertyGrid';
import { useAppStore } from '../../store/useAppStore';

const fields: PropertyField[] = [
  { key: 'scale', label: 'Scale Factor', type: 'number', min: 0.01, max: 10, step: 0.1 },
  { key: 'density', label: 'Density', type: 'number', min: 0.1, max: 5, step: 0.1 },
  {
    key: 'colorField',
    label: 'Color By',
    type: 'select',
    options: [
      { label: 'Pressure', value: 'pressure' },
      { label: 'Velocity Magnitude', value: 'velocity' },
      { label: 'Temperature', value: 'temperature' },
    ],
  },
];

const VectorSettings: React.FC = () => {
  const vectorConfig = useAppStore((s) => s.vectorConfig);
  const updateVectorConfig = useAppStore((s) => s.updateVectorConfig);

  const values: Record<string, unknown> = { ...vectorConfig };

  return (
    <PropertyGrid
      title="Vector Settings"
      fields={fields}
      values={values}
      onChange={(key, value) => updateVectorConfig({ [key]: value })}
    />
  );
};

export default VectorSettings;
