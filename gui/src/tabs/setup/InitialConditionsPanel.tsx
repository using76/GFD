import React from 'react';
import { Form, InputNumber, Checkbox, Typography, Divider, Button, message } from 'antd';
import { useAppStore } from '../../store/useAppStore';

const InitialConditionsPanel: React.FC = () => {
  const ic = useAppStore((s) => s.initialConditions);
  const update = useAppStore((s) => s.updateInitialConditions);
  const boundaries = useAppStore((s) => s.boundaries);
  const material = useAppStore((s) => s.material);
  const physicsModels = useAppStore((s) => s.physicsModels);

  const pullFromInlet = () => {
    const inlet = boundaries.find((b) => b.type === 'inlet');
    if (!inlet) {
      message.warning('No inlet boundary defined.');
      return;
    }
    update({
      velocity: [...inlet.velocity] as [number, number, number],
      temperature: inlet.temperature,
      tke: 1.5 * (Math.sqrt(inlet.velocity[0] ** 2 + inlet.velocity[1] ** 2 + inlet.velocity[2] ** 2) * inlet.turbulenceIntensity) ** 2,
    });
    message.success('Initial values copied from inlet BC.');
  };

  return (
    <div style={{ padding: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 12, fontSize: 14, borderBottom: '1px solid #303030', paddingBottom: 8 }}>
        Initial Conditions
      </div>

      <Form layout="vertical" size="small">
        <Form.Item valuePropName="checked">
          <Checkbox
            checked={ic.initFromInlet}
            onChange={(e) => update({ initFromInlet: e.target.checked })}
          >
            Initialize from inlet BC on solver start
          </Checkbox>
        </Form.Item>

        <Typography.Paragraph style={{ fontSize: 11, color: '#778', marginTop: -4, marginBottom: 8 }}>
          {ic.initFromInlet
            ? 'Fields are initialised to the inlet boundary velocity/temperature/TKE. Values below are ignored.'
            : 'Fields are initialised to the constants below.'}
        </Typography.Paragraph>

        <Divider style={{ margin: '8px 0' }} />

        <Form.Item label="Initial Pressure (Pa)">
          <InputNumber
            value={ic.pressure}
            step={100}
            style={{ width: '100%' }}
            disabled={ic.initFromInlet}
            onChange={(v) => update({ pressure: v ?? 0 })}
          />
        </Form.Item>

        <Form.Item label="Initial Velocity (m/s) [Vx, Vy, Vz]">
          <div style={{ display: 'flex', gap: 4 }}>
            {(['X', 'Y', 'Z'] as const).map((axis, i) => (
              <InputNumber
                key={axis}
                value={ic.velocity[i]}
                step={0.1}
                placeholder={axis}
                style={{ flex: 1 }}
                disabled={ic.initFromInlet}
                onChange={(v) => {
                  const next = [...ic.velocity] as [number, number, number];
                  next[i] = v ?? 0;
                  update({ velocity: next });
                }}
              />
            ))}
          </div>
        </Form.Item>

        {physicsModels.energy && (
          <Form.Item label="Initial Temperature (K)">
            <InputNumber
              value={ic.temperature}
              min={0}
              step={1}
              style={{ width: '100%' }}
              disabled={ic.initFromInlet}
              onChange={(v) => update({ temperature: v ?? 300 })}
            />
          </Form.Item>
        )}

        {physicsModels.turbulence !== 'none' && (
          <Form.Item label="Initial TKE (m²/s²)">
            <InputNumber
              value={ic.tke}
              min={0}
              step={0.001}
              style={{ width: '100%' }}
              disabled={ic.initFromInlet}
              onChange={(v) => update({ tke: v ?? 0.01 })}
            />
          </Form.Item>
        )}

        <Button
          size="small"
          block
          disabled={ic.initFromInlet}
          onClick={pullFromInlet}
          style={{ marginTop: 4 }}
        >
          Copy from Inlet BC
        </Button>
      </Form>

      <Divider style={{ margin: '12px 0' }} />

      <div style={{ padding: 8, background: '#1a1a30', borderRadius: 4, fontSize: 11, color: '#778' }}>
        Reynolds number (inlet): {(() => {
          const inlet = boundaries.find((b) => b.type === 'inlet');
          const v = inlet ? Math.sqrt(inlet.velocity[0] ** 2 + inlet.velocity[1] ** 2 + inlet.velocity[2] ** 2) : 0;
          const nu = material.viscosity / Math.max(material.density, 1e-12);
          const Re = v / Math.max(nu, 1e-18);
          return Number.isFinite(Re) ? Re.toExponential(2) : 'N/A';
        })()}
      </div>
    </div>
  );
};

export default InitialConditionsPanel;
