import React from 'react';
import { Select, Typography, Divider, Empty } from 'antd';
import PropertyGrid from '../../components/PropertyGrid';
import type { PropertyField } from '../../components/PropertyGrid';
import { useAppStore } from '../../store/useAppStore';
import type { BoundaryType } from '../../store/useAppStore';

const typeOptions = [
  { label: 'Wall', value: 'wall' },
  { label: 'Velocity Inlet', value: 'inlet' },
  { label: 'Pressure Outlet', value: 'outlet' },
  { label: 'Symmetry', value: 'symmetry' },
];

const paramFields: Record<BoundaryType, PropertyField[]> = {
  wall: [
    { key: 'temperature', label: 'Temperature (K)', type: 'number', min: 0, step: 1 },
  ],
  inlet: [
    { key: 'velocity', label: 'Velocity (m/s)', type: 'vector3', step: 0.1 },
    { key: 'temperature', label: 'Temperature (K)', type: 'number', min: 0, step: 1 },
  ],
  outlet: [
    { key: 'pressure', label: 'Gauge Pressure (Pa)', type: 'number', step: 100 },
    { key: 'temperature', label: 'Temperature (K)', type: 'number', min: 0, step: 1 },
  ],
  symmetry: [],
};

const BoundaryPanel: React.FC = () => {
  const boundaries = useAppStore((s) => s.boundaries);
  const selectedBoundaryId = useAppStore((s) => s.selectedBoundaryId);
  const selectBoundary = useAppStore((s) => s.selectBoundary);
  const updateBoundary = useAppStore((s) => s.updateBoundary);

  const selected = boundaries.find((b) => b.id === selectedBoundaryId);

  if (boundaries.length === 0) {
    return (
      <div style={{ padding: 16 }}>
        <Empty description="Generate mesh first to define boundary conditions." />
      </div>
    );
  }

  return (
    <div style={{ padding: 12 }}>
      <div
        style={{
          fontWeight: 600,
          marginBottom: 12,
          fontSize: 14,
          borderBottom: '1px solid #303030',
          paddingBottom: 8,
        }}
      >
        Boundary Conditions
      </div>

      <div style={{ marginBottom: 12 }}>
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          Select Patch
        </Typography.Text>
        <Select
          style={{ width: '100%', marginTop: 4 }}
          value={selectedBoundaryId}
          placeholder="Select boundary..."
          options={boundaries.map((b) => ({ label: b.name, value: b.id }))}
          onChange={selectBoundary}
        />
      </div>

      {selected && (
        <>
          <Divider style={{ margin: '8px 0' }} />
          <div style={{ marginBottom: 12 }}>
            <Typography.Text type="secondary" style={{ fontSize: 12 }}>
              Boundary Type
            </Typography.Text>
            <Select
              style={{ width: '100%', marginTop: 4 }}
              value={selected.type}
              options={typeOptions}
              onChange={(v) => updateBoundary(selected.id, { type: v })}
            />
          </div>

          {paramFields[selected.type].length > 0 && (
            <PropertyGrid
              fields={paramFields[selected.type]}
              values={{
                velocity: selected.velocity,
                pressure: selected.pressure,
                temperature: selected.temperature,
              }}
              onChange={(key, value) =>
                updateBoundary(selected.id, { [key]: value })
              }
            />
          )}
        </>
      )}
    </div>
  );
};

export default BoundaryPanel;
