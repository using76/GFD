import React, { useState } from 'react';
import { Button, Space, Checkbox, Divider, Statistic, InputNumber, Typography, Select, Tag, message } from 'antd';
import {
  CaretRightOutlined,
  PauseOutlined,
  StopOutlined,
  SettingOutlined,
  DownloadOutlined,
} from '@ant-design/icons';
import { useAppStore } from '../../store/useAppStore';

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
  const physicsModels = useAppStore((s) => s.physicsModels);
  const material = useAppStore((s) => s.material);
  const meshDisplayData = useAppStore((s) => s.meshDisplayData);
  const meshGenerated = useAppStore((s) => s.meshGenerated);
  const boundaries = useAppStore((s) => s.boundaries);

  const [showSummary, setShowSummary] = useState(false);

  const isRunning = solverStatus === 'running';
  const isPaused = solverStatus === 'paused';
  const isIdle = solverStatus === 'idle';

  const lastResidual = residuals.length > 0 ? residuals[residuals.length - 1] : null;

  return (
    <div style={{ padding: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 12, fontSize: 14, borderBottom: '1px solid #303030', paddingBottom: 8 }}>
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

      {/* Iteration speed & ETA */}
      {solverStatus === 'running' && currentIteration > 2 && (
        <div style={{ marginBottom: 12, padding: 6, background: '#1a1a30', borderRadius: 4, fontSize: 11, color: '#778' }}>
          <div>Speed: ~{(currentIteration / ((Date.now() - (performance as any).timeOrigin) * 0.001 || 1) * 50).toFixed(0)} iter/s (50ms/step)</div>
          <div>Remaining: ~{Math.max(0, solverSettings.maxIterations - currentIteration)} iterations</div>
          <div>ETA: ~{((solverSettings.maxIterations - currentIteration) * 0.05).toFixed(1)}s</div>
        </div>
      )}

      <Space style={{ marginBottom: 16 }}>
        <Button
          type="primary"
          icon={<CaretRightOutlined />}
          disabled={isRunning || !meshGenerated}
          onClick={() => {
            if (!meshGenerated) return;
            setShowSummary(false);
            startSolver();
          }}
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

      {!meshGenerated && (
        <div style={{ padding: 8, background: '#2a1a1a', borderRadius: 4, fontSize: 11, color: '#ff8c00', marginBottom: 12 }}>
          Generate a mesh before starting the solver.
        </div>
      )}

      <Divider style={{ margin: '8px 0' }} />

      {/* Time stepping */}
      <div style={{ marginBottom: 8 }}>
        <Typography.Text strong style={{ fontSize: 12 }}>Time</Typography.Text>
        <Select
          size="small"
          value={solverSettings.timeMode}
          onChange={(v) => updateSolverSettings({ timeMode: v })}
          options={[
            { label: 'Steady', value: 'steady' },
            { label: 'Transient', value: 'transient' },
          ]}
          style={{ width: '100%', marginTop: 4 }}
        />
      </div>
      {solverSettings.timeMode === 'transient' && (
        <div style={{ display: 'flex', gap: 8, marginBottom: 8 }}>
          <div style={{ flex: 1 }}>
            <Typography.Text type="secondary" style={{ fontSize: 10 }}>dt (s)</Typography.Text>
            <InputNumber
              size="small"
              value={solverSettings.timeStepSize}
              min={1e-9}
              step={0.0001}
              style={{ width: '100%' }}
              onChange={(v) => updateSolverSettings({ timeStepSize: v ?? 0.001 })}
            />
          </div>
          <div style={{ flex: 1 }}>
            <Typography.Text type="secondary" style={{ fontSize: 10 }}>Total (s)</Typography.Text>
            <InputNumber
              size="small"
              value={solverSettings.totalTime}
              min={0.001}
              step={0.1}
              style={{ width: '100%' }}
              onChange={(v) => updateSolverSettings({ totalTime: v ?? 1.0 })}
            />
          </div>
        </div>
      )}

      {/* Iteration controls */}
      <div style={{ display: 'flex', gap: 8, marginBottom: 8 }}>
        <div style={{ flex: 1 }}>
          <Typography.Text type="secondary" style={{ fontSize: 10 }}>Max Iterations</Typography.Text>
          <InputNumber
            size="small"
            value={solverSettings.maxIterations}
            min={1}
            max={100000}
            step={100}
            style={{ width: '100%' }}
            onChange={(v) => updateSolverSettings({ maxIterations: v ?? 200 })}
          />
        </div>
        <div style={{ flex: 1 }}>
          <Typography.Text type="secondary" style={{ fontSize: 10 }}>Tolerance</Typography.Text>
          <InputNumber
            size="small"
            value={solverSettings.tolerance}
            min={1e-12}
            max={1e-1}
            step={1e-7}
            style={{ width: '100%' }}
            onChange={(v) => updateSolverSettings({ tolerance: v ?? 1e-6 })}
          />
        </div>
      </div>

      <Divider style={{ margin: '8px 0' }} />

      <div style={{ display: 'flex', flexDirection: 'column', gap: 8, marginBottom: 12 }}>
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

      <Divider style={{ margin: '8px 0' }} />

      {/* Config Summary */}
      <div
        onClick={() => setShowSummary(!showSummary)}
        style={{ cursor: 'pointer', color: '#4096ff', fontSize: 11, marginBottom: 4 }}
      >
        <SettingOutlined /> {showSummary ? 'Hide' : 'Show'} Configuration Summary
      </div>
      {showSummary && (
        <div style={{ padding: 8, background: '#1a1a30', borderRadius: 4, fontSize: 11, color: '#aab' }}>
          <div><b>Mesh:</b> {meshDisplayData ? `${meshDisplayData.cellCount.toLocaleString()} cells, ${meshDisplayData.nodeCount.toLocaleString()} nodes` : 'Not generated'}</div>
          <div><b>Flow:</b> {physicsModels.flow} | <b>Turb:</b> {physicsModels.turbulence}</div>
          <div><b>Energy:</b> {physicsModels.energy ? 'On' : 'Off'} | <b>Multiphase:</b> {physicsModels.multiphase}</div>
          <div><b>Radiation:</b> {physicsModels.radiation} | <b>Species:</b> {physicsModels.species}</div>
          <div><b>Material:</b> {material.name} (rho={material.density}, mu={material.viscosity.toExponential(2)})</div>
          <div><b>Solver:</b> {solverSettings.method} | <b>Time:</b> {solverSettings.timeMode}</div>
          <div><b>Discretization:</b> P={solverSettings.pressureScheme}, M={solverSettings.momentumScheme}</div>
          <div><b>URF:</b> p={solverSettings.relaxPressure}, u={solverSettings.relaxVelocity}, k={solverSettings.relaxTurbulence}, e={solverSettings.relaxEnergy}</div>
          <div><b>Boundaries:</b> {boundaries.length} patches</div>
          {boundaries.map((b) => (
            <div key={b.id} style={{ paddingLeft: 12 }}>
              <Tag color={b.type === 'inlet' ? 'blue' : b.type === 'outlet' ? 'red' : b.type === 'symmetry' ? 'gold' : 'green'} style={{ fontSize: 9, padding: '0 3px', margin: '1px 0' }}>
                {b.type}
              </Tag>{' '}
              {b.name}
              {b.type === 'inlet' && ` v=[${b.velocity.join(',')}]`}
              {b.type === 'outlet' && ` p=${b.pressure}`}
            </div>
          ))}
          <div><b>GPU:</b> {useGpu ? 'Enabled' : 'Disabled'} | <b>MPI:</b> {useMpi ? `${mpiCores} cores` : 'Disabled'}</div>
        </div>
      )}

      {/* Export section */}
      {residuals.length > 0 && (
        <>
          <Divider style={{ margin: '8px 0' }} />
          <Space>
            <Button size="small" icon={<DownloadOutlined />} onClick={() => {
              const consoleLines = useAppStore.getState().consoleLines;
              const blob = new Blob([consoleLines.join('\n')], { type: 'text/plain' });
              const url = URL.createObjectURL(blob);
              const a = document.createElement('a');
              a.href = url; a.download = 'gfd_solver_log.txt'; a.click();
              URL.revokeObjectURL(url);
              message.success('Console log exported');
            }}>
              Export Log
            </Button>
            <Button size="small" icon={<DownloadOutlined />} onClick={() => {
              const lines = ['iteration,continuity,x-momentum,y-momentum,energy'];
              residuals.forEach(r => {
                lines.push(`${r.iteration},${r.continuity.toExponential(6)},${r.xMomentum.toExponential(6)},${r.yMomentum.toExponential(6)},${r.energy.toExponential(6)}`);
              });
              const blob = new Blob([lines.join('\n')], { type: 'text/csv' });
              const url = URL.createObjectURL(blob);
              const a = document.createElement('a');
              a.href = url; a.download = 'gfd_residuals.csv'; a.click();
              URL.revokeObjectURL(url);
              message.success('Residual history exported');
            }}>
              Export Residuals
            </Button>
          </Space>
        </>
      )}
    </div>
  );
};

export default RunControls;
