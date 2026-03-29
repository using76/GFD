import React from 'react';
import { Card, Statistic, Row, Col, Typography } from 'antd';
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';
import { useAppStore } from '../../store/useAppStore';

const QualityPanel: React.FC = () => {
  const quality = useAppStore((s) => s.meshQuality);

  if (!quality) {
    return (
      <div style={{ padding: 16, color: '#888' }}>
        <Typography.Text type="secondary">
          Generate a mesh to view quality statistics.
        </Typography.Text>
      </div>
    );
  }

  const histogramData = quality.histogram.map((value, i) => ({
    bin: `${(i * 0.1).toFixed(1)}-${((i + 1) * 0.1).toFixed(1)}`,
    fraction: +(value * 100).toFixed(1),
  }));

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
        Mesh Quality
      </div>

      <Row gutter={[8, 8]}>
        <Col span={8}>
          <Card size="small">
            <Statistic
              title="Cells"
              value={quality.cellCount}
              valueStyle={{ fontSize: 14 }}
            />
          </Card>
        </Col>
        <Col span={8}>
          <Card size="small">
            <Statistic
              title="Faces"
              value={quality.faceCount}
              valueStyle={{ fontSize: 14 }}
            />
          </Card>
        </Col>
        <Col span={8}>
          <Card size="small">
            <Statistic
              title="Nodes"
              value={quality.nodeCount}
              valueStyle={{ fontSize: 14 }}
            />
          </Card>
        </Col>
        <Col span={12}>
          <Card size="small">
            <Statistic
              title="Min Orthogonality"
              value={quality.minOrthogonality}
              precision={3}
              valueStyle={{ fontSize: 14, color: quality.minOrthogonality > 0.8 ? '#52c41a' : '#faad14' }}
            />
          </Card>
        </Col>
        <Col span={12}>
          <Card size="small">
            <Statistic
              title="Max Skewness"
              value={quality.maxSkewness}
              precision={3}
              valueStyle={{ fontSize: 14, color: quality.maxSkewness < 0.3 ? '#52c41a' : '#faad14' }}
            />
          </Card>
        </Col>
        <Col span={12}>
          <Card size="small">
            <Statistic
              title="Max Aspect Ratio"
              value={quality.maxAspectRatio}
              precision={2}
              valueStyle={{ fontSize: 14, color: quality.maxAspectRatio < 5 ? '#52c41a' : '#faad14' }}
            />
          </Card>
        </Col>
      </Row>

      <div style={{ marginTop: 16 }}>
        <Typography.Text strong>Quality Histogram</Typography.Text>
        <div style={{ width: '100%', height: 160, marginTop: 8 }}>
          <ResponsiveContainer width="100%" height={150} minWidth={100}>
            <BarChart data={histogramData}>
              <XAxis dataKey="bin" tick={{ fontSize: 9 }} />
              <YAxis tick={{ fontSize: 10 }} unit="%" />
              <Tooltip />
              <Bar dataKey="fraction" fill="#1668dc" />
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>
    </div>
  );
};

export default QualityPanel;
