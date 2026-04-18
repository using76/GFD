import React, { useState } from 'react';
import { Form, Select, InputNumber, Button, message, Typography, Divider, Empty } from 'antd';
import { PlayCircleOutlined, DeleteOutlined, DownloadOutlined } from '@ant-design/icons';
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid, Legend } from 'recharts';
import { useAppStore } from '../../store/useAppStore';
import type { SweepParameter } from '../../store/useAppStore';

const paramOptions: { label: string; value: SweepParameter; unit: string; min: number; max: number; step: number }[] = [
  { label: 'Inlet Velocity', value: 'inletVelocity', unit: 'm/s', min: 0, max: 100, step: 0.1 },
  { label: 'Density', value: 'density', unit: 'kg/m³', min: 0.01, max: 20000, step: 0.1 },
  { label: 'Viscosity', value: 'viscosity', unit: 'Pa·s', min: 1e-7, max: 10, step: 1e-5 },
  { label: "Young's Modulus", value: 'youngsModulus', unit: 'Pa', min: 1e3, max: 1e12, step: 1e9 },
];

const ParametricSweepPanel: React.FC = () => {
  const sweepParam = useAppStore((s) => s.sweepParam);
  const setSweepParam = useAppStore((s) => s.setSweepParam);
  const sweepRuns = useAppStore((s) => s.sweepRuns);
  const addSweepRun = useAppStore((s) => s.addSweepRun);
  const clearSweepRuns = useAppStore((s) => s.clearSweepRuns);
  const fieldData = useAppStore((s) => s.fieldData);
  const boundaries = useAppStore((s) => s.boundaries);
  const material = useAppStore((s) => s.material);

  const [from, setFrom] = useState(0.5);
  const [to, setTo] = useState(5.0);
  const [steps, setSteps] = useState(5);
  const [running, setRunning] = useState(false);

  const paramInfo = paramOptions.find(p => p.value === sweepParam)!;

  const runSweep = async () => {
    if (steps < 2 || from === to) {
      message.warning('Need at least 2 steps with distinct from/to values.');
      return;
    }
    setRunning(true);
    clearSweepRuns();
    const state = useAppStore.getState();
    // Remember original values to restore after sweep
    const origMat = { ...state.material };
    const inlet = boundaries.find(b => b.type === 'inlet');
    const origInletVel: [number, number, number] = inlet ? [...inlet.velocity] as [number, number, number] : [1, 0, 0];

    for (let i = 0; i < steps; i++) {
      const val = from + (to - from) * i / (steps - 1);
      // Apply the swept parameter
      if (sweepParam === 'inletVelocity') {
        if (inlet) {
          const dir = Math.hypot(...origInletVel) > 0 ? origInletVel.map(c => c / Math.hypot(...origInletVel)) : [1, 0, 0];
          state.updateBoundary(inlet.id, { velocity: [dir[0] * val, dir[1] * val, dir[2] * val] as [number, number, number] });
        }
      } else if (sweepParam === 'density') {
        state.updateMaterial({ density: val });
      } else if (sweepParam === 'viscosity') {
        state.updateMaterial({ viscosity: val });
      } else if (sweepParam === 'youngsModulus') {
        state.updateMaterial({ youngsModulus: val });
      }

      // Run solver synchronously-ish: start + wait for 'finished'
      await new Promise<void>((resolve) => {
        state.stopSolver();
        setTimeout(() => {
          state.startSolver();
          const poll = setInterval(() => {
            const st = useAppStore.getState();
            if (st.solverStatus === 'finished' || st.solverStatus === 'idle') {
              clearInterval(poll);
              // Record summary
              const p = st.fieldData.find(f => f.name === 'pressure');
              const v = st.fieldData.find(f => f.name === 'velocity');
              const t = st.fieldData.find(f => f.name === 'temperature');
              addSweepRun({
                id: `run-${Date.now()}-${i}`,
                paramValue: val,
                results: {
                  pMin: p?.min ?? 0,
                  pMax: p?.max ?? 0,
                  vMax: v?.max ?? 0,
                  tMax: t?.max ?? 0,
                  cd: 0, // Estimated Cd would require the full integration — kept 0 for demo
                  cl: 0,
                },
              });
              resolve();
            }
          }, 150);
        }, 100);
      });
    }

    // Restore original values
    if (sweepParam === 'inletVelocity' && inlet) {
      state.updateBoundary(inlet.id, { velocity: origInletVel });
    } else {
      state.updateMaterial({ density: origMat.density, viscosity: origMat.viscosity, youngsModulus: origMat.youngsModulus });
    }
    setRunning(false);
    message.success(`Sweep complete: ${steps} runs`);
  };

  const exportCsv = () => {
    const lines: string[] = [`${paramInfo.label} (${paramInfo.unit}),p_min,p_max,v_max,T_max`];
    sweepRuns.forEach(r => {
      lines.push(`${r.paramValue},${r.results.pMin.toFixed(4)},${r.results.pMax.toFixed(4)},${r.results.vMax.toFixed(4)},${r.results.tMax.toFixed(4)}`);
    });
    const blob = new Blob([lines.join('\n')], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url; a.download = 'gfd_sweep.csv'; a.click();
    URL.revokeObjectURL(url);
    message.success('Sweep CSV exported');
  };

  const chartData = sweepRuns.map(r => ({
    param: r.paramValue,
    p_max: r.results.pMax,
    v_max: r.results.vMax,
  }));

  return (
    <div style={{ padding: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 12, fontSize: 14, borderBottom: '1px solid #303030', paddingBottom: 8 }}>
        Parametric Sweep
      </div>

      <Form layout="vertical" size="small">
        <Form.Item label="Parameter">
          <Select
            value={sweepParam}
            onChange={(v) => setSweepParam(v)}
            options={paramOptions.map(p => ({ label: `${p.label} (${p.unit})`, value: p.value }))}
          />
        </Form.Item>

        <div style={{ display: 'flex', gap: 8 }}>
          <Form.Item label="From" style={{ flex: 1 }}>
            <InputNumber value={from} step={paramInfo.step} min={paramInfo.min} max={paramInfo.max} style={{ width: '100%' }}
              onChange={(v) => setFrom(v ?? 0)} />
          </Form.Item>
          <Form.Item label="To" style={{ flex: 1 }}>
            <InputNumber value={to} step={paramInfo.step} min={paramInfo.min} max={paramInfo.max} style={{ width: '100%' }}
              onChange={(v) => setTo(v ?? 0)} />
          </Form.Item>
          <Form.Item label="Steps" style={{ flex: 1 }}>
            <InputNumber value={steps} step={1} min={2} max={20} style={{ width: '100%' }}
              onChange={(v) => setSteps(v ?? 5)} />
          </Form.Item>
        </div>

        <Button
          type="primary"
          icon={<PlayCircleOutlined />}
          block
          loading={running}
          disabled={running || fieldData.length === 0}
          onClick={runSweep}
        >
          {running ? 'Running sweep…' : `Run sweep (${steps} runs)`}
        </Button>
        {fieldData.length === 0 && (
          <Typography.Text type="secondary" style={{ fontSize: 11, display: 'block', marginTop: 4 }}>
            Run the solver at least once before starting a sweep — it needs baseline mesh & fields.
          </Typography.Text>
        )}
      </Form>

      <Divider style={{ margin: '12px 0' }} />

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <Typography.Text strong style={{ fontSize: 12 }}>
          Results ({sweepRuns.length})
        </Typography.Text>
        {sweepRuns.length > 0 && (
          <div style={{ display: 'flex', gap: 4 }}>
            <Button size="small" icon={<DownloadOutlined />} onClick={exportCsv}>CSV</Button>
            <Button size="small" danger icon={<DeleteOutlined />} onClick={clearSweepRuns}>Clear</Button>
          </div>
        )}
      </div>

      {sweepRuns.length === 0 ? (
        <Empty style={{ marginTop: 12 }} description="No sweep runs yet." />
      ) : (
        <>
          {/* Chart */}
          <div style={{ width: '100%', height: 180, marginTop: 8 }}>
            <ResponsiveContainer width="100%" height={180} minWidth={100}>
              <LineChart data={chartData}>
                <CartesianGrid strokeDasharray="3 3" stroke="#333" />
                <XAxis dataKey="param" tick={{ fontSize: 9 }} stroke="#888" label={{ value: paramInfo.label, position: 'insideBottom', offset: -3, fontSize: 10 }} />
                <YAxis tick={{ fontSize: 9 }} stroke="#888" />
                <Tooltip contentStyle={{ background: '#1f1f1f', border: '1px solid #444', fontSize: 10 }} />
                <Legend wrapperStyle={{ fontSize: 10 }} />
                <Line type="monotone" dataKey="p_max" stroke="#1668dc" dot strokeWidth={1.5} isAnimationActive={false} />
                <Line type="monotone" dataKey="v_max" stroke="#52c41a" dot strokeWidth={1.5} isAnimationActive={false} />
              </LineChart>
            </ResponsiveContainer>
          </div>

          <div style={{ maxHeight: 180, overflow: 'auto', fontSize: 11, marginTop: 4 }}>
            <table style={{ width: '100%', color: '#aab' }}>
              <thead>
                <tr style={{ borderBottom: '1px solid #303050' }}>
                  <th style={{ textAlign: 'left', padding: 2 }}>{paramInfo.label}</th>
                  <th style={{ textAlign: 'right', padding: 2 }}>p_max</th>
                  <th style={{ textAlign: 'right', padding: 2 }}>v_max</th>
                  <th style={{ textAlign: 'right', padding: 2 }}>T_max</th>
                </tr>
              </thead>
              <tbody>
                {sweepRuns.map(r => (
                  <tr key={r.id} style={{ borderBottom: '1px solid #252540' }}>
                    <td style={{ padding: 2 }}>{r.paramValue.toFixed(4)}</td>
                    <td style={{ textAlign: 'right', padding: 2 }}>{r.results.pMax.toFixed(2)}</td>
                    <td style={{ textAlign: 'right', padding: 2 }}>{r.results.vMax.toFixed(4)}</td>
                    <td style={{ textAlign: 'right', padding: 2 }}>{r.results.tMax.toFixed(1)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}

      <div style={{ marginTop: 8, padding: 6, background: '#1a1a30', borderRadius: 4, fontSize: 10, color: '#667' }}>
        Baseline (reference): ρ={material.density}, μ={material.viscosity.toExponential(2)}, E={material.youngsModulus.toExponential(2)}
      </div>
    </div>
  );
};

export default ParametricSweepPanel;
