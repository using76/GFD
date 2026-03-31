import React, { useEffect } from 'react';
import { Divider, Typography, Empty, Card, Statistic, Row, Col, Form, Select, Checkbox, InputNumber, Slider } from 'antd';
import { useAppStore } from '../../store/useAppStore';

const ContourSettings: React.FC = () => {
  const contourConfig = useAppStore((s) => s.contourConfig);
  const updateContourConfig = useAppStore((s) => s.updateContourConfig);
  const fieldData = useAppStore((s) => s.fieldData);
  const setActiveField = useAppStore((s) => s.setActiveField);
  const setRenderMode = useAppStore((s) => s.setRenderMode);
  const solverStatus = useAppStore((s) => s.solverStatus);
  const boundaries = useAppStore((s) => s.boundaries);

  const activeField = useAppStore((s) => s.activeField);
  const renderMode = useAppStore((s) => s.renderMode);

  const hasFields = fieldData.length > 0;

  // Auto-enable contour mode when entering results with field data
  useEffect(() => {
    if (hasFields && activeField && renderMode !== 'contour') {
      setRenderMode('contour');
    }
  }, [hasFields, activeField, renderMode, setRenderMode]);

  if (!hasFields && solverStatus !== 'finished') {
    return (
      <div style={{ padding: 16 }}>
        <Empty description="Run the solver to generate field data for contour display." />
      </div>
    );
  }

  // Build field options from actual solved fields
  const fieldOptions = fieldData.map((f) => ({
    label: f.name.charAt(0).toUpperCase() + f.name.slice(1) + (f.name === 'pressure' ? ' (Pa)' : f.name === 'velocity' ? ' (m/s)' : f.name === 'temperature' ? ' (K)' : ''),
    value: f.name,
  }));

  // Show field range info
  const activeFieldData = fieldData.find((f) => f.name === contourConfig.field);

  // Compute mean
  let meanValue: number | null = null;
  if (activeFieldData) {
    let sum = 0;
    for (let i = 0; i < activeFieldData.values.length; i++) {
      sum += activeFieldData.values[i];
    }
    meanValue = sum / activeFieldData.values.length;
  }

  // Unit labels
  const unitMap: Record<string, string> = {
    pressure: 'Pa',
    velocity: 'm/s',
    temperature: 'K',
  };
  const unit = unitMap[contourConfig.field] || '';

  // Colormap gradients for preview
  const colormapGradients: Record<string, string> = {
    jet: 'linear-gradient(to right, #0000ff, #00ffff, #00ff00, #ffff00, #ff0000)',
    rainbow: 'linear-gradient(to right, #ff0000, #ff8800, #ffff00, #00ff00, #0088ff, #8800ff)',
    grayscale: 'linear-gradient(to right, #000000, #ffffff)',
    coolwarm: 'linear-gradient(to right, #3b4cc0, #6b8df0, #aac7fd, #f0f0f0, #f7b89c, #e8604c, #b40426)',
  };

  // Boundary options for "show on"
  const boundaryOptions = [
    { label: 'All Boundaries', value: 'all' },
    ...boundaries.map((b) => ({ label: b.name, value: b.id })),
  ];

  return (
    <div>
      <div style={{ padding: '12px 12px 0' }}>
        <div style={{ fontWeight: 600, marginBottom: 12, fontSize: 14, borderBottom: '1px solid #303030', paddingBottom: 8 }}>
          Contour Settings
        </div>
      </div>

      <Form layout="vertical" size="small" style={{ padding: '0 12px' }}>
        <Form.Item label="Field">
          <Select
            value={contourConfig.field}
            options={fieldOptions.length > 0 ? fieldOptions : [
              { label: 'Pressure (Pa)', value: 'pressure' },
              { label: 'Velocity Magnitude (m/s)', value: 'velocity' },
              { label: 'Temperature (K)', value: 'temperature' },
              { label: 'Turbulence Kinetic Energy (m²/s²)', value: 'tke' },
              { label: 'VOF Phase Fraction', value: 'vof_alpha' },
              { label: 'Incident Radiation (W/m²)', value: 'radiation_G' },
              { label: 'Species Mass Fraction', value: 'species_Y' },
            ]}
            onChange={(v) => {
              updateContourConfig({ field: v as never });
              setActiveField(v);
              setRenderMode('contour');
            }}
          />
        </Form.Item>

        <Form.Item label="Colormap">
          <Select
            value={contourConfig.colormap}
            options={[
              { label: 'Jet', value: 'jet' },
              { label: 'Rainbow', value: 'rainbow' },
              { label: 'Grayscale', value: 'grayscale' },
              { label: 'Cool-Warm', value: 'coolwarm' },
            ]}
            onChange={(v) => updateContourConfig({ colormap: v as never })}
          />
        </Form.Item>

        <Form.Item label={`Opacity: ${(contourConfig.opacity * 100).toFixed(0)}%`}>
          <Slider
            min={0}
            max={1}
            step={0.05}
            value={contourConfig.opacity}
            onChange={(v) => updateContourConfig({ opacity: v })}
          />
        </Form.Item>

        <Form.Item label="Show On">
          <Select
            value={contourConfig.showOnBoundary || 'all'}
            options={boundaryOptions}
            onChange={(v) => updateContourConfig({ showOnBoundary: v })}
          />
        </Form.Item>

        <Form.Item valuePropName="checked">
          <Checkbox
            checked={contourConfig.autoRange}
            onChange={(e) => updateContourConfig({ autoRange: e.target.checked })}
          >
            Auto Range
          </Checkbox>
        </Form.Item>

        {!contourConfig.autoRange && (
          <div style={{ display: 'flex', gap: 8 }}>
            <Form.Item label="Min" style={{ flex: 1 }}>
              <InputNumber
                value={contourConfig.min}
                step={0.1}
                style={{ width: '100%' }}
                onChange={(v) => updateContourConfig({ min: v ?? 0 })}
              />
            </Form.Item>
            <Form.Item label="Max" style={{ flex: 1 }}>
              <InputNumber
                value={contourConfig.max}
                step={0.1}
                style={{ width: '100%' }}
                onChange={(v) => updateContourConfig({ max: v ?? 1 })}
              />
            </Form.Item>
          </div>
        )}
      </Form>

      {activeFieldData && (
        <>
          <Divider style={{ margin: '4px 12px' }} />
          <div style={{ padding: '4px 12px' }}>
            <Typography.Text strong style={{ fontSize: 12 }}>
              Field Statistics ({contourConfig.field})
            </Typography.Text>
            <Row gutter={[6, 6]} style={{ marginTop: 8 }}>
              <Col span={8}>
                <Card size="small" style={{ background: '#1a1a30' }}>
                  <Statistic
                    title="Min"
                    value={activeFieldData.min}
                    precision={2}
                    suffix={unit}
                    valueStyle={{ fontSize: 12 }}
                  />
                </Card>
              </Col>
              <Col span={8}>
                <Card size="small" style={{ background: '#1a1a30' }}>
                  <Statistic
                    title="Max"
                    value={activeFieldData.max}
                    precision={2}
                    suffix={unit}
                    valueStyle={{ fontSize: 12 }}
                  />
                </Card>
              </Col>
              <Col span={8}>
                <Card size="small" style={{ background: '#1a1a30' }}>
                  <Statistic
                    title="Mean"
                    value={meanValue ?? 0}
                    precision={2}
                    suffix={unit}
                    valueStyle={{ fontSize: 12 }}
                  />
                </Card>
              </Col>
            </Row>
          </div>

          {/* Color bar legend */}
          <div style={{ padding: '8px 12px' }}>
            <Typography.Text type="secondary" style={{ fontSize: 11 }}>
              Color Bar
            </Typography.Text>
            <div style={{
              marginTop: 4,
              height: 16,
              borderRadius: 3,
              background: colormapGradients[contourConfig.colormap] || colormapGradients.jet,
              opacity: contourConfig.opacity,
            }} />
            <div style={{
              display: 'flex',
              justifyContent: 'space-between',
              fontSize: 10,
              color: '#889',
              marginTop: 2,
            }}>
              <span>{activeFieldData.min.toFixed(1)} {unit}</span>
              <span>{((activeFieldData.min + activeFieldData.max) / 2).toFixed(1)}</span>
              <span>{activeFieldData.max.toFixed(1)} {unit}</span>
            </div>
          </div>
        </>
      )}
    </div>
  );
};

export default ContourSettings;
