import React from 'react';
import { Checkbox, Slider, Typography, Form } from 'antd';
import { useAppStore } from '../../store/useAppStore';

const StreamlineSettings: React.FC = () => {
  const showStreamlines = useAppStore((s) => s.showStreamlines);
  const setShowStreamlines = useAppStore((s) => s.setShowStreamlines);
  const vectorConfig = useAppStore((s) => s.vectorConfig);
  const updateVectorConfig = useAppStore((s) => s.updateVectorConfig);
  const solverStatus = useAppStore((s) => s.solverStatus);

  const hasData = solverStatus === 'finished';

  return (
    <div style={{ padding: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 12, fontSize: 14, borderBottom: '1px solid #303030', paddingBottom: 8 }}>
        Streamline Settings
      </div>

      <Checkbox
        checked={showStreamlines}
        onChange={(e) => setShowStreamlines(e.target.checked)}
        style={{ marginBottom: 12 }}
        disabled={!hasData}
      >
        Show Streamlines
      </Checkbox>

      {!hasData && (
        <Typography.Text type="secondary" style={{ display: 'block', marginBottom: 12, fontSize: 11 }}>
          Run the solver to visualize streamlines.
        </Typography.Text>
      )}

      <Form layout="vertical" size="small">
        <Form.Item label={`Seed Density: ${vectorConfig.density.toFixed(1)}`} style={{ marginBottom: 8 }}>
          <Slider
            min={0.5}
            max={3}
            step={0.1}
            value={vectorConfig.density}
            onChange={(v) => updateVectorConfig({ density: v })}
            disabled={!hasData}
          />
        </Form.Item>

        <Form.Item label={`Scale: ${vectorConfig.scale.toFixed(1)}`} style={{ marginBottom: 8 }}>
          <Slider
            min={0.1}
            max={5}
            step={0.1}
            value={vectorConfig.scale}
            onChange={(v) => updateVectorConfig({ scale: v })}
            disabled={!hasData}
          />
        </Form.Item>
      </Form>

      <div style={{ marginTop: 12, padding: 8, background: '#1a1a30', borderRadius: 4, fontSize: 11, color: '#778' }}>
        Streamlines trace fluid particle paths using RK4 integration.
        Seed points are placed at the inlet face (x-min) and integrated downstream.
        Density controls the number of seed lines; scale affects visual thickness.
      </div>
    </div>
  );
};

export default StreamlineSettings;
