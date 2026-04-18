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
  ReferenceLine,
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
  const tolerance = useAppStore((s) => s.solverSettings.tolerance);
  const structural = useAppStore((s) => s.physicsModels.structural);

  // In structural mode, the ResidualPoint slots hold different physical quantities.
  const labels = structural
    ? { continuity: 'Δu / u', xMomentum: 'Δu_x', yMomentum: 'Δu_y', energy: 'Stress res.' }
    : { continuity: 'continuity', xMomentum: 'x-Momentum', yMomentum: 'y-Momentum', energy: 'energy' };

  const data = useMemo(
    () =>
      residuals.map((r) => ({
        iteration: r.iteration,
        [labels.continuity]: r.continuity,
        [labels.xMomentum]: r.xMomentum,
        [labels.yMomentum]: r.yMomentum,
        [labels.energy]: r.energy,
      })),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [residuals, structural]
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
        {structural ? 'Structural Convergence (displacement & stress norms)' : 'Residual Convergence'}
      </Typography.Text>
      <div style={{ flex: 1, minHeight: 200 }}>
        <ResponsiveContainer width="100%" height="100%" minWidth={100} minHeight={200}>
          <LineChart data={data}>
            <CartesianGrid strokeDasharray="3 3" stroke="#333" />
            <XAxis
              dataKey="iteration"
              label={{ value: 'Iteration', position: 'insideBottom', offset: -5 }}
              stroke="#888"
            />
            <YAxis
              scale="log"
              domain={['auto', 'auto']}
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
            <ReferenceLine
              y={tolerance}
              stroke="#ff4d4f"
              strokeDasharray="6 3"
              strokeWidth={1}
              label={{ value: `Tol: ${tolerance.toExponential(0)}`, position: 'right', fill: '#ff4d4f', fontSize: 10 }}
            />
            <Line
              type="monotone"
              dataKey={labels.continuity}
              stroke={COLORS.continuity}
              dot={false}
              strokeWidth={1.5}
              isAnimationActive={false}
            />
            <Line
              type="monotone"
              dataKey={labels.xMomentum}
              stroke={COLORS.xMomentum}
              dot={false}
              strokeWidth={1.5}
              isAnimationActive={false}
            />
            <Line
              type="monotone"
              dataKey={labels.yMomentum}
              stroke={COLORS.yMomentum}
              dot={false}
              strokeWidth={1.5}
              isAnimationActive={false}
            />
            <Line
              type="monotone"
              dataKey={labels.energy}
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
