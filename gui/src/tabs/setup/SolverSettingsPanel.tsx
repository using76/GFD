import React from 'react';
import PropertyGrid from '../../components/PropertyGrid';
import type { PropertyField } from '../../components/PropertyGrid';
import { useAppStore } from '../../store/useAppStore';

const fields: PropertyField[] = [
  {
    key: 'method',
    label: 'Pressure-Velocity Coupling',
    type: 'select',
    options: [
      { label: 'SIMPLE', value: 'SIMPLE' },
      { label: 'PISO', value: 'PISO' },
      { label: 'SIMPLEC', value: 'SIMPLEC' },
    ],
  },
  {
    key: 'relaxPressure',
    label: 'Pressure URF',
    type: 'number',
    min: 0.01,
    max: 1.0,
    step: 0.05,
  },
  {
    key: 'relaxVelocity',
    label: 'Velocity URF',
    type: 'number',
    min: 0.01,
    max: 1.0,
    step: 0.05,
  },
  {
    key: 'maxIterations',
    label: 'Max Iterations',
    type: 'number',
    min: 1,
    max: 100000,
    step: 100,
  },
  {
    key: 'tolerance',
    label: 'Convergence Tolerance',
    type: 'number',
    min: 1e-12,
    max: 1e-1,
    step: 1e-7,
  },
];

const SolverSettingsPanel: React.FC = () => {
  const solverSettings = useAppStore((s) => s.solverSettings);
  const updateSolverSettings = useAppStore((s) => s.updateSolverSettings);

  const values: Record<string, unknown> = { ...solverSettings };

  return (
    <PropertyGrid
      title="Solver Settings"
      fields={fields}
      values={values}
      onChange={(key, value) => updateSolverSettings({ [key]: value })}
    />
  );
};

export default SolverSettingsPanel;
