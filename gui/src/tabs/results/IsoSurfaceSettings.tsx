import React from 'react';
import { Checkbox, Slider, Select, Typography, Form } from 'antd';
import { useAppStore } from '../../store/useAppStore';

const IsoSurfaceSettings: React.FC = () => {
  const enabled = useAppStore((s) => s.isoSurfaceEnabled);
  const field = useAppStore((s) => s.isoSurfaceField);
  const value = useAppStore((s) => s.isoSurfaceValue);
  const setIsoSurface = useAppStore((s) => s.setIsoSurface);
  const fieldData = useAppStore((s) => s.fieldData);
  const solverStatus = useAppStore((s) => s.solverStatus);

  const hasData = solverStatus === 'finished' && fieldData.length > 0;
  const activeFieldData = fieldData.find(f => f.name === field);
  const fMin = activeFieldData?.min ?? 0;
  const fMax = activeFieldData?.max ?? 100;

  return (
    <div style={{ padding: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 12, fontSize: 14, borderBottom: '1px solid #303030', paddingBottom: 8 }}>
        Iso-Surface Settings
      </div>

      <Checkbox
        checked={enabled}
        onChange={(e) => setIsoSurface(e.target.checked)}
        disabled={!hasData}
        style={{ marginBottom: 12 }}
      >
        Show Iso-Surface
      </Checkbox>

      {!hasData && (
        <Typography.Text type="secondary" style={{ display: 'block', marginBottom: 12, fontSize: 11 }}>
          Run the solver to generate iso-surfaces.
        </Typography.Text>
      )}

      <Form layout="vertical" size="small">
        <Form.Item label="Field" style={{ marginBottom: 8 }}>
          <Select
            value={field}
            onChange={(v) => setIsoSurface(enabled, v)}
            disabled={!hasData}
            options={fieldData.map(f => ({ label: f.name, value: f.name }))}
          />
        </Form.Item>

        <Form.Item label={`Value: ${value.toFixed(2)} (${fMin.toFixed(1)} - ${fMax.toFixed(1)})`} style={{ marginBottom: 8 }}>
          <Slider
            min={fMin}
            max={fMax}
            step={(fMax - fMin) / 100 || 0.1}
            value={value}
            onChange={(v) => setIsoSurface(enabled, field, v)}
            disabled={!hasData}
          />
        </Form.Item>
      </Form>

      <div style={{ marginTop: 8, padding: 8, background: '#1a1a30', borderRadius: 4, fontSize: 11, color: '#778' }}>
        Iso-surface renders a 3D surface at constant field value.
        Orange semi-transparent mesh shows where the field equals the specified threshold.
      </div>
    </div>
  );
};

export default IsoSurfaceSettings;
