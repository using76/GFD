import React from 'react';
import { Form, Select, InputNumber, Slider, Typography, Divider, Button, Space, message } from 'antd';
import { SaveOutlined } from '@ant-design/icons';
import { useAppStore } from '../../store/useAppStore';

const BUILT_IN_PRESETS: Record<string, { method: string; maxIterations: number; tolerance: number; relaxPressure: number; relaxVelocity: number }> = {
  'Default (SIMPLE)': { method: 'SIMPLE', maxIterations: 500, tolerance: 1e-4, relaxPressure: 0.3, relaxVelocity: 0.7 },
  'Fast (SIMPLEC)': { method: 'SIMPLEC', maxIterations: 200, tolerance: 1e-3, relaxPressure: 0.5, relaxVelocity: 0.8 },
  'Accurate (PISO)': { method: 'PISO', maxIterations: 1000, tolerance: 1e-6, relaxPressure: 0.2, relaxVelocity: 0.5 },
};

const SolverSettingsPanel: React.FC = () => {
  const solverSettings = useAppStore((s) => s.solverSettings);
  const updateSolverSettings = useAppStore((s) => s.updateSolverSettings);

  return (
    <div style={{ padding: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 12, fontSize: 14, borderBottom: '1px solid #303030', paddingBottom: 8, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        Solver Settings
        <Space size={4}>
          <Select size="small" placeholder="Presets" style={{ width: 120 }}
            options={Object.keys(BUILT_IN_PRESETS).map(k => ({ label: k, value: k }))}
            onChange={(v) => {
              const p = BUILT_IN_PRESETS[v];
              if (p) { updateSolverSettings(p as never); message.success(`Preset "${v}" applied`); }
            }}
          />
          <Button size="small" icon={<SaveOutlined />} onClick={() => {
            const name = prompt('Preset name:', 'My Config');
            if (!name) return;
            const presets = JSON.parse(localStorage.getItem('gfd-solver-presets') ?? '{}');
            presets[name] = { ...solverSettings };
            localStorage.setItem('gfd-solver-presets', JSON.stringify(presets));
            message.success(`Saved preset "${name}"`);
          }} title="Save preset" />
        </Space>
      </div>

      <Form layout="vertical" size="small">
        {/* Pressure-Velocity Coupling */}
        <Form.Item label="Pressure-Velocity Coupling">
          <Select
            value={solverSettings.method}
            onChange={(v) => updateSolverSettings({ method: v })}
            options={[
              { label: 'SIMPLE', value: 'SIMPLE' },
              { label: 'PISO', value: 'PISO' },
              { label: 'SIMPLEC', value: 'SIMPLEC' },
            ]}
          />
        </Form.Item>

        {/* Time stepping */}
        <Form.Item label="Time">
          <Select
            value={solverSettings.timeMode}
            onChange={(v) => updateSolverSettings({ timeMode: v })}
            options={[
              { label: 'Steady', value: 'steady' },
              { label: 'Transient', value: 'transient' },
            ]}
          />
        </Form.Item>
        {solverSettings.timeMode === 'transient' && (
          <>
            <Form.Item label="Time Step Size (s)">
              <InputNumber
                value={solverSettings.timeStepSize}
                min={1e-9}
                max={100}
                step={0.0001}
                style={{ width: '100%' }}
                onChange={(v) => updateSolverSettings({ timeStepSize: v ?? 0.001 })}
              />
            </Form.Item>
            <Form.Item label="Total Time (s)">
              <InputNumber
                value={solverSettings.totalTime}
                min={0.001}
                max={10000}
                step={0.1}
                style={{ width: '100%' }}
                onChange={(v) => updateSolverSettings({ totalTime: v ?? 1.0 })}
              />
            </Form.Item>
          </>
        )}

        <Divider style={{ margin: '8px 0' }} />
        <Typography.Text strong style={{ fontSize: 12 }}>Spatial Discretization</Typography.Text>

        <Form.Item label="Pressure" style={{ marginTop: 8 }}>
          <Select
            value={solverSettings.pressureScheme}
            onChange={(v) => updateSolverSettings({ pressureScheme: v })}
            options={[
              { label: 'Standard', value: 'standard' },
              { label: 'Second Order', value: 'second-order' },
            ]}
          />
        </Form.Item>

        <Form.Item label="Momentum">
          <Select
            value={solverSettings.momentumScheme}
            onChange={(v) => updateSolverSettings({ momentumScheme: v })}
            options={[
              { label: 'First Order Upwind', value: 'first-order' },
              { label: 'Second Order Upwind', value: 'second-order' },
              { label: 'QUICK', value: 'QUICK' },
            ]}
          />
        </Form.Item>

        <Divider style={{ margin: '8px 0' }} />
        <Typography.Text strong style={{ fontSize: 12 }}>Under-Relaxation Factors</Typography.Text>

        <Form.Item label={`Pressure: ${solverSettings.relaxPressure.toFixed(2)}`} style={{ marginTop: 8, marginBottom: 4 }}>
          <Slider
            min={0.01}
            max={1.0}
            step={0.01}
            value={solverSettings.relaxPressure}
            onChange={(v) => updateSolverSettings({ relaxPressure: v })}
          />
        </Form.Item>

        <Form.Item label={`Momentum: ${solverSettings.relaxVelocity.toFixed(2)}`} style={{ marginBottom: 4 }}>
          <Slider
            min={0.01}
            max={1.0}
            step={0.01}
            value={solverSettings.relaxVelocity}
            onChange={(v) => updateSolverSettings({ relaxVelocity: v })}
          />
        </Form.Item>

        <Form.Item label={`Turbulence: ${solverSettings.relaxTurbulence.toFixed(2)}`} style={{ marginBottom: 4 }}>
          <Slider
            min={0.01}
            max={1.0}
            step={0.01}
            value={solverSettings.relaxTurbulence}
            onChange={(v) => updateSolverSettings({ relaxTurbulence: v })}
          />
        </Form.Item>

        <Form.Item label={`Energy: ${solverSettings.relaxEnergy.toFixed(2)}`} style={{ marginBottom: 4 }}>
          <Slider
            min={0.01}
            max={1.0}
            step={0.01}
            value={solverSettings.relaxEnergy}
            onChange={(v) => updateSolverSettings({ relaxEnergy: v })}
          />
        </Form.Item>

        <Divider style={{ margin: '8px 0' }} />
        <Typography.Text strong style={{ fontSize: 12 }}>Convergence Criteria</Typography.Text>

        <Form.Item label="Max Iterations" style={{ marginTop: 8 }}>
          <InputNumber
            value={solverSettings.maxIterations}
            min={1}
            max={100000}
            step={100}
            style={{ width: '100%' }}
            onChange={(v) => updateSolverSettings({ maxIterations: v ?? 200 })}
          />
        </Form.Item>

        <Form.Item label="Continuity / Momentum Tolerance">
          <InputNumber
            value={solverSettings.tolerance}
            min={1e-12}
            max={1e-1}
            step={1e-7}
            style={{ width: '100%' }}
            onChange={(v) => updateSolverSettings({ tolerance: v ?? 1e-6 })}
          />
        </Form.Item>

        <Form.Item label="Energy Tolerance">
          <InputNumber
            value={solverSettings.toleranceEnergy}
            min={1e-12}
            max={1e-1}
            step={1e-7}
            style={{ width: '100%' }}
            onChange={(v) => updateSolverSettings({ toleranceEnergy: v ?? 1e-6 })}
          />
        </Form.Item>
      </Form>
    </div>
  );
};

export default SolverSettingsPanel;
