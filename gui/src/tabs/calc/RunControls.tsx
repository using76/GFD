import React from 'react';
import { Button, Space, Checkbox, Divider, Statistic, InputNumber, Typography } from 'antd';
import {
  CaretRightOutlined,
  PauseOutlined,
  StopOutlined,
} from '@ant-design/icons';
import PropertyGrid from '../../components/PropertyGrid';
import type { PropertyField } from '../../components/PropertyGrid';
import { useAppStore } from '../../store/useAppStore';

const iterFields: PropertyField[] = [
  {
    key: 'maxIterations',
    label: 'Max Iterations',
    type: 'number',
    min: 1,
    max: 100000,
    step: 100,
  },
  {
    key: 'tolerance',
    label: 'Convergence Tolerance',
    type: 'number',
    min: 1e-12,
    max: 1e-1,
    step: 1e-7,
  },
];

const RunControls: React.FC = () => {
  const solverStatus = useAppStore((s) => s.solverStatus);
  const currentIteration = useAppStore((s) => s.currentIteration);
  const residuals = useAppStore((s) => s.residuals);
  const solverSettings = useAppStore((s) => s.solverSettings);
  const updateSolverSettings = useAppStore((s) => s.updateSolverSettings);
  const startSolver = useAppStore((s) => s.startSolver);
  const pauseSolver = useAppStore((s) => s.pauseSolver);
  const stopSolver = useAppStore((s) => s.stopSolver);
  const useGpu = useAppStore((s) => s.useGpu);
  const useMpi = useAppStore((s) => s.useMpi);
  const mpiCores = useAppStore((s) => s.mpiCores);
  const setUseGpu = useAppStore((s) => s.setUseGpu);
  const setUseMpi = useAppStore((s) => s.setUseMpi);
  const setMpiCores = useAppStore((s) => s.setMpiCores);

  const isRunning = solverStatus === 'running';
  const isPaused = solverStatus === 'paused';
  const isIdle = solverStatus === 'idle';

  const lastResidual = residuals.length > 0 ? residuals[residuals.length - 1] : null;

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
        Run Controls
      </div>

      <Statistic
        title="Current Iteration"
        value={currentIteration}
        suffix={`/ ${solverSettings.maxIterations}`}
        valueStyle={{ fontSize: 18 }}
        style={{ marginBottom: 8 }}
      />

      {lastResidual && (
        <div style={{ marginBottom: 12, fontSize: 11, color: '#889' }}>
          <Typography.Text style={{ fontSize: 11, color: '#889' }}>
            Continuity: {lastResidual.continuity.toExponential(3)}
          </Typography.Text>
          <br />
          <Typography.Text style={{ fontSize: 11, color: '#889' }}>
            X-Momentum: {lastResidual.xMomentum.toExponential(3)}
          </Typography.Text>
          <br />
          <Typography.Text style={{ fontSize: 11, color: '#889' }}>
            Y-Momentum: {lastResidual.yMomentum.toExponential(3)}
          </Typography.Text>
          <br />
          <Typography.Text style={{ fontSize: 11, color: '#889' }}>
            Energy: {lastResidual.energy.toExponential(3)}
          </Typography.Text>
        </div>
      )}

      <Space style={{ marginBottom: 16 }}>
        <Button
          type="primary"
          icon={<CaretRightOutlined />}
          disabled={isRunning}
          onClick={startSolver}
        >
          {isPaused ? 'Resume' : 'Start'}
        </Button>
        <Button
          icon={<PauseOutlined />}
          disabled={!isRunning}
          onClick={pauseSolver}
        >
          Pause
        </Button>
        <Button
          danger
          icon={<StopOutlined />}
          disabled={isIdle}
          onClick={stopSolver}
        >
          Stop
        </Button>
      </Space>

      <Divider style={{ margin: '8px 0' }} />

      <PropertyGrid
        fields={iterFields}
        values={{
          maxIterations: solverSettings.maxIterations,
          tolerance: solverSettings.tolerance,
        }}
        onChange={(key, value) => updateSolverSettings({ [key]: value })}
      />

      <Divider style={{ margin: '8px 0' }} />

      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        <Checkbox checked={useGpu} onChange={(e) => setUseGpu(e.target.checked)}>
          GPU Acceleration (CUDA)
        </Checkbox>
        <Checkbox checked={useMpi} onChange={(e) => setUseMpi(e.target.checked)}>
          MPI Parallel
        </Checkbox>
        {useMpi && (
          <div style={{ paddingLeft: 24, display: 'flex', alignItems: 'center', gap: 8 }}>
            <Typography.Text style={{ fontSize: 12, color: '#889' }}>Cores:</Typography.Text>
            <InputNumber
              size="small"
              min={1}
              max={128}
              value={mpiCores}
              onChange={(v) => setMpiCores(v ?? 4)}
              style={{ width: 70 }}
            />
          </div>
        )}
      </div>
    </div>
  );
};

export default RunControls;
