import React from 'react';
import PropertyGrid from '../../components/PropertyGrid';
import type { PropertyField } from '../../components/PropertyGrid';
import { useAppStore } from '../../store/useAppStore';

const fields: PropertyField[] = [
  {
    key: 'flow',
    label: 'Flow',
    type: 'select',
    options: [
      { label: 'Incompressible', value: 'incompressible' },
      { label: 'Compressible', value: 'compressible' },
    ],
  },
  {
    key: 'turbulence',
    label: 'Turbulence',
    type: 'select',
    options: [
      { label: 'None (Laminar)', value: 'none' },
      { label: 'k-epsilon', value: 'k-epsilon' },
      { label: 'k-omega SST', value: 'k-omega-sst' },
      { label: 'Spalart-Allmaras', value: 'sa' },
      { label: 'LES', value: 'les' },
    ],
  },
  {
    key: 'energy',
    label: 'Energy Equation',
    type: 'checkbox',
  },
  {
    key: 'multiphase',
    label: 'Multiphase',
    type: 'select',
    options: [
      { label: 'None', value: 'none' },
      { label: 'VOF', value: 'vof' },
      { label: 'Euler-Euler', value: 'euler' },
      { label: 'Mixture', value: 'mixture' },
      { label: 'DPM', value: 'dpm' },
    ],
  },
];

const ModelsPanel: React.FC = () => {
  const physicsModels = useAppStore((s) => s.physicsModels);
  const updatePhysicsModels = useAppStore((s) => s.updatePhysicsModels);

  const values: Record<string, unknown> = { ...physicsModels };

  return (
    <PropertyGrid
      title="Physics Models"
      fields={fields}
      values={values}
      onChange={(key, value) => updatePhysicsModels({ [key]: value })}
    />
  );
};

export default ModelsPanel;
