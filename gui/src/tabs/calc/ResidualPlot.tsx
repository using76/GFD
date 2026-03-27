import React, { useMemo } from 'react';
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  Legend,
  ResponsiveContainer,
  CartesianGrid,
} from 'recharts';
import { Typography, Empty } from 'antd';
import { useAppStore } from '../../store/useAppStore';

const COLORS = {
  continuity: '#1668dc',
  xMomentum: '#ff4d4f',
  yMomentum: '#52c41a',
  energy: '#fa8c16',
};

const ResidualPlot: React.FC = () => {
  const residuals = useAppStore((s) => s.residuals);

  const data = useMemo(
    () =>
      residuals.map((r) => ({
        iteration: r.iteration,
        continuity: r.continuity,
        xMomentum: r.xMomentum,
        yMomentum: r.yMomentum,
        energy: r.energy,
      })),
    [residuals]
  );

  if (data.length === 0) {
    return (
      <div
        style={{
          width: '100%',
          height: '100%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <Empty description="Start the solver to see residual convergence." />
      </div>
    );
  }

  return (
    <div
      style={{
        width: '100%',
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        padding: 16,
      }}
    >
      <Typography.Text strong style={{ marginBottom: 8 }}>
        Residual Convergence
      </Typography.Text>
      <div style={{ flex: 1, minHeight: 0 }}>
        <ResponsiveContainer width="100%" height="100%">
          <LineChart data={data}>
            <CartesianGrid strokeDasharray="3 3" stroke="#333" />
            <XAxis
              dataKey="iteration"
              label={{ value: 'Iteration', position: 'insideBottom', offset: -5 }}
              stroke="#888"
            />
            <YAxis
              scale="log"
              domain={[1e-6, 1]}
              allowDataOverflow
              label={{
                value: 'Residual',
                angle: -90,
                position: 'insideLeft',
              }}
              stroke="#888"
              tickFormatter={(v: number) => v.toExponential(0)}
            />
            <Tooltip
              formatter={((value: number) => value.toExponential(3)) as never}
              contentStyle={{ background: '#1f1f1f', border: '1px solid #444' }}
            />
            <Legend />
            <Line
              type="monotone"
              dataKey="continuity"
              stroke={COLORS.continuity}
              dot={false}
              strokeWidth={1.5}
              isAnimationActive={false}
            />
            <Line
              type="monotone"
              dataKey="xMomentum"
              stroke={COLORS.xMomentum}
              dot={false}
              strokeWidth={1.5}
              isAnimationActive={false}
            />
            <Line
              type="monotone"
              dataKey="yMomentum"
              stroke={COLORS.yMomentum}
              dot={false}
              strokeWidth={1.5}
              isAnimationActive={false}
            />
            <Line
              type="monotone"
              dataKey="energy"
              stroke={COLORS.energy}
              dot={false}
              strokeWidth={1.5}
              isAnimationActive={false}
            />
          </LineChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
};

export default ResidualPlot;
