import React from 'react';
import { Select, Typography, Divider, Empty, Form, InputNumber, Tag, Button } from 'antd';
import { PlusOutlined } from '@ant-design/icons';
import { useAppStore } from '../../store/useAppStore';
import type { BoundaryType, WallThermalCondition } from '../../store/useAppStore';

const fluidTypeOptions = [
  { label: 'Wall', value: 'wall' },
  { label: 'Velocity Inlet', value: 'inlet' },
  { label: 'Pressure Outlet', value: 'outlet' },
  { label: 'Symmetry', value: 'symmetry' },
];

const structuralTypeOptions = [
  { label: 'Fixed Support', value: 'fixed' },
  { label: 'Applied Force', value: 'force' },
  { label: 'Free (Wall)', value: 'wall' },
];

const typeColors: Record<BoundaryType, string> = {
  inlet: '#4488ff',
  outlet: '#ff4444',
  wall: '#44cc44',
  symmetry: '#ffcc00',
  fixed: '#aa44ff',
  force: '#ff8800',
};

const BoundaryPanel: React.FC = () => {
  const boundaries = useAppStore((s) => s.boundaries);
  const selectedBoundaryId = useAppStore((s) => s.selectedBoundaryId);
  const selectBoundary = useAppStore((s) => s.selectBoundary);
  const updateBoundary = useAppStore((s) => s.updateBoundary);
  const addBoundary = useAppStore((s) => s.addBoundary);
  const meshGenerated = useAppStore((s) => s.meshGenerated);
  const meshSurfaces = useAppStore((s) => s.meshSurfaces);
  const physicsModels = useAppStore((s) => s.physicsModels);
  const typeOptions = physicsModels.structural ? structuralTypeOptions : fluidTypeOptions;

  const selected = boundaries.find((b) => b.id === selectedBoundaryId);

  const handleAddCustomBC = () => {
    const id = `bc-custom-${Date.now()}`;
    addBoundary({
      id,
      name: `custom-${boundaries.length + 1}`,
      type: 'wall',
      velocity: [0, 0, 0],
      pressure: 0,
      temperature: 300,
      turbulenceIntensity: 0.05,
      wallThermalCondition: 'adiabatic',
      heatFlux: 0,
      movingWallVelocity: [0, 0, 0],
      force: [0, 0, 0],
    });
    selectBoundary(id);
  };

  // Auto-sync: if mesh surfaces exist but no boundaries, create from surfaces
  const handleSyncFromMesh = () => {
    if (meshSurfaces.length === 0) return;
    meshSurfaces.forEach((surf) => {
      const exists = boundaries.find(b => b.id === surf.id);
      if (!exists) {
        const type: BoundaryType = surf.boundaryType === 'inlet' ? 'inlet'
          : surf.boundaryType === 'outlet' ? 'outlet'
          : surf.boundaryType === 'symmetry' ? 'symmetry'
          : 'wall';
        addBoundary({
          id: surf.id,
          name: surf.name,
          type,
          velocity: type === 'inlet' ? [1, 0, 0] : [0, 0, 0],
          pressure: 0,
          temperature: 300,
          turbulenceIntensity: 0.05,
          wallThermalCondition: 'adiabatic',
          heatFlux: 0,
          movingWallVelocity: [0, 0, 0],
          force: [0, 0, 0],
        });
      }
    });
  };

  if (boundaries.length === 0) {
    return (
      <div style={{ padding: 16 }}>
        <Empty description={meshGenerated ? "No boundary patches found." : "Generate mesh first to define boundary conditions."} />
        {meshGenerated && meshSurfaces.length > 0 && (
          <Button type="primary" block style={{ marginTop: 12 }} onClick={handleSyncFromMesh}>
            Create BCs from Mesh Surfaces ({meshSurfaces.length})
          </Button>
        )}
        {meshGenerated && (
          <Button block style={{ marginTop: 8 }} icon={<PlusOutlined />} onClick={handleAddCustomBC}>
            Add Custom BC
          </Button>
        )}
      </div>
    );
  }

  return (
    <div style={{ padding: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 12, fontSize: 14, borderBottom: '1px solid #303030', paddingBottom: 8 }}>
        Boundary Conditions
      </div>

      {/* Boundary list */}
      <div style={{ marginBottom: 12 }}>
        <Typography.Text type="secondary" style={{ fontSize: 12 }}>
          Patches ({boundaries.length})
        </Typography.Text>
        <div style={{ marginTop: 4, maxHeight: 180, overflow: 'auto', border: '1px solid #303050', borderRadius: 4 }}>
          {boundaries.map((b) => (
            <div
              key={b.id}
              onClick={() => selectBoundary(b.id)}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 6,
                padding: '5px 8px',
                cursor: 'pointer',
                background: selectedBoundaryId === b.id ? '#2a2a5a' : 'transparent',
                borderBottom: '1px solid #1a1a30',
                borderLeft: `3px solid ${typeColors[b.type]}`,
                fontSize: 12,
              }}
              onMouseEnter={(e) => { if (selectedBoundaryId !== b.id) e.currentTarget.style.background = '#1a1a3a'; }}
              onMouseLeave={(e) => { if (selectedBoundaryId !== b.id) e.currentTarget.style.background = 'transparent'; }}
            >
              <span style={{ flex: 1, color: '#ccd' }}>{b.name}</span>
              <Tag
                color={typeColors[b.type]}
                style={{ fontSize: 10, lineHeight: '16px', padding: '0 4px', margin: 0, background: 'transparent', borderColor: typeColors[b.type], color: typeColors[b.type] }}
              >
                {b.type}
              </Tag>
            </div>
          ))}
        </div>
        <Button size="small" icon={<PlusOutlined />} block style={{ marginTop: 4 }} onClick={handleAddCustomBC}>
          Add BC
        </Button>
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

          {/* INLET parameters */}
          {selected.type === 'inlet' && (
            <Form layout="vertical" size="small">
              <Form.Item label="Velocity (m/s) [Vx, Vy, Vz]">
                <div style={{ display: 'flex', gap: 4 }}>
                  {(['X', 'Y', 'Z'] as const).map((axis, i) => (
                    <InputNumber
                      key={axis}
                      value={selected.velocity[i]}
                      step={0.1}
                      placeholder={axis}
                      style={{ flex: 1 }}
                      onChange={(v) => {
                        const next = [...selected.velocity] as [number, number, number];
                        next[i] = v ?? 0;
                        updateBoundary(selected.id, { velocity: next });
                      }}
                    />
                  ))}
                </div>
              </Form.Item>
              <Form.Item label="Temperature (K)">
                <InputNumber
                  value={selected.temperature}
                  min={0}
                  step={1}
                  style={{ width: '100%' }}
                  onChange={(v) => updateBoundary(selected.id, { temperature: v ?? 300 })}
                />
              </Form.Item>
              <Form.Item label="Turbulence Intensity">
                <InputNumber
                  value={selected.turbulenceIntensity}
                  min={0}
                  max={1}
                  step={0.01}
                  style={{ width: '100%' }}
                  onChange={(v) => updateBoundary(selected.id, { turbulenceIntensity: v ?? 0.05 })}
                  addonAfter="%"
                />
              </Form.Item>
            </Form>
          )}

          {/* OUTLET parameters */}
          {selected.type === 'outlet' && (
            <Form layout="vertical" size="small">
              <Form.Item label="Gauge Pressure (Pa)">
                <InputNumber
                  value={selected.pressure}
                  step={100}
                  style={{ width: '100%' }}
                  onChange={(v) => updateBoundary(selected.id, { pressure: v ?? 0 })}
                />
              </Form.Item>
              <Form.Item label="Temperature (K)">
                <InputNumber
                  value={selected.temperature}
                  min={0}
                  step={1}
                  style={{ width: '100%' }}
                  onChange={(v) => updateBoundary(selected.id, { temperature: v ?? 300 })}
                />
              </Form.Item>
            </Form>
          )}

          {/* WALL parameters */}
          {selected.type === 'wall' && (
            <Form layout="vertical" size="small">
              <Form.Item label="Thermal Condition">
                <Select
                  value={selected.wallThermalCondition || 'adiabatic'}
                  style={{ width: '100%' }}
                  onChange={(v: WallThermalCondition) => updateBoundary(selected.id, { wallThermalCondition: v })}
                  options={[
                    { label: 'Adiabatic', value: 'adiabatic' },
                    { label: 'Fixed Temperature', value: 'fixed-temp' },
                    { label: 'Heat Flux', value: 'heat-flux' },
                  ]}
                />
              </Form.Item>
              {(selected.wallThermalCondition === 'fixed-temp') && (
                <Form.Item label="Wall Temperature (K)">
                  <InputNumber
                    value={selected.temperature}
                    min={0}
                    step={1}
                    style={{ width: '100%' }}
                    onChange={(v) => updateBoundary(selected.id, { temperature: v ?? 300 })}
                  />
                </Form.Item>
              )}
              {(selected.wallThermalCondition === 'heat-flux') && (
                <Form.Item label="Heat Flux (W/m2)">
                  <InputNumber
                    value={selected.heatFlux ?? 0}
                    step={100}
                    style={{ width: '100%' }}
                    onChange={(v) => updateBoundary(selected.id, { heatFlux: v ?? 0 })}
                  />
                </Form.Item>
              )}
              <Form.Item label="Moving Wall Velocity (m/s) [Vx, Vy, Vz]">
                <div style={{ display: 'flex', gap: 4 }}>
                  {(['X', 'Y', 'Z'] as const).map((axis, i) => (
                    <InputNumber
                      key={axis}
                      value={(selected.movingWallVelocity || [0, 0, 0])[i]}
                      step={0.1}
                      placeholder={axis}
                      style={{ flex: 1 }}
                      onChange={(v) => {
                        const next = [...(selected.movingWallVelocity || [0, 0, 0])] as [number, number, number];
                        next[i] = v ?? 0;
                        updateBoundary(selected.id, { movingWallVelocity: next });
                      }}
                    />
                  ))}
                </div>
              </Form.Item>
            </Form>
          )}

          {/* SYMMETRY parameters */}
          {selected.type === 'symmetry' && (
            <div style={{ padding: 8, color: '#667', fontSize: 11 }}>
              Symmetry boundary: no additional parameters required. Zero normal gradient for all variables.
            </div>
          )}

          {/* FIXED support (structural) */}
          {selected.type === 'fixed' && (
            <div style={{ padding: 8, color: '#667', fontSize: 11 }}>
              Fixed support: zero displacement on all axes. Used as the reaction boundary in structural analysis.
            </div>
          )}

          {/* APPLIED FORCE (structural) */}
          {selected.type === 'force' && (
            <Form layout="vertical" size="small">
              <Form.Item label="Force (N) [Fx, Fy, Fz]">
                <div style={{ display: 'flex', gap: 4 }}>
                  {(['X', 'Y', 'Z'] as const).map((axis, i) => (
                    <InputNumber
                      key={axis}
                      value={(selected.force ?? [0, 0, 0])[i]}
                      step={10}
                      placeholder={axis}
                      style={{ flex: 1 }}
                      onChange={(v) => {
                        const cur = selected.force ?? [0, 0, 0];
                        const next = [...cur] as [number, number, number];
                        next[i] = v ?? 0;
                        updateBoundary(selected.id, { force: next });
                      }}
                    />
                  ))}
                </div>
              </Form.Item>
            </Form>
          )}
        </>
      )}
    </div>
  );
};

export default BoundaryPanel;
