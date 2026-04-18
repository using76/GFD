import React, { useMemo } from 'react';
import { Card, Statistic, Row, Col, Divider, Typography, Empty, Button, Select, message } from 'antd';
import { DownloadOutlined } from '@ant-design/icons';
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from 'recharts';
import { useAppStore } from '../../store/useAppStore';

const ReportPanel: React.FC = () => {
  const solverStatus = useAppStore((s) => s.solverStatus);
  const residuals = useAppStore((s) => s.residuals);
  const fieldData = useAppStore((s) => s.fieldData);
  const boundaries = useAppStore((s) => s.boundaries);
  const material = useAppStore((s) => s.material);
  const meshDisplayData = useAppStore((s) => s.meshDisplayData);

  const hasData = residuals.length > 0;

  if (!hasData) {
    return (
      <div style={{ padding: 16 }}>
        <Empty description="Run the solver to generate reports." />
      </div>
    );
  }

  const lastResidual = residuals[residuals.length - 1];
  const isConverged = solverStatus === 'finished';

  // Compute real field statistics from actual field data
  const pressureField = fieldData.find((f) => f.name === 'pressure');
  const tempField = fieldData.find((f) => f.name === 'temperature');

  // Compute mass flux from inlet/outlet boundaries
  const inletBCs = boundaries.filter((b) => b.type === 'inlet');
  const outletBCs = boundaries.filter((b) => b.type === 'outlet');
  const cellCount = meshDisplayData?.cellCount ?? 1;

  // Estimate mass flux from inlet velocity, density, and approximate area
  const totalInletMassFlux = inletBCs.reduce((sum, bc) => {
    const vMag = Math.sqrt(bc.velocity[0] ** 2 + bc.velocity[1] ** 2 + bc.velocity[2] ** 2);
    // Approximate inlet area from mesh
    const approxArea = cellCount > 0 ? Math.pow(cellCount, -1 / 3) * 4 : 1;
    return sum + material.density * vMag * approxArea;
  }, 0);

  // ---- REAL force integration on wall boundaries ----
  // Build per-triangle contribution F_tri = p · n_tri · A_tri + τ_wall · t_tri · A_tri
  // τ_wall estimated via Schlichting flat-plate Cf on inlet Re.
  const refVelocity = inletBCs.length > 0 ? Math.sqrt(inletBCs[0].velocity[0] ** 2 + inletBCs[0].velocity[1] ** 2 + inletBCs[0].velocity[2] ** 2) : 1;
  const refDynPressure = 0.5 * material.density * refVelocity ** 2;

  const wallForce: { fx: number; fy: number; fz: number; area: number } = { fx: 0, fy: 0, fz: 0, area: 0 };
  if (pressureField && meshDisplayData && meshDisplayData.positions.length > 0) {
    const pos = meshDisplayData.positions;
    const vals = pressureField.values;
    const nTris = pos.length / 9;
    // Estimate inlet-based wall shear once
    const nu = material.viscosity / Math.max(material.density, 1e-12);
    const Re = refVelocity * 1.0 / Math.max(nu, 1e-18);
    const Cf = Re > 0 ? 0.058 * Math.pow(Math.max(Re, 1), -0.2) : 0.005;
    const tauW = 0.5 * Cf * material.density * refVelocity * refVelocity;

    // Bounds for detecting external-domain faces vs. body-wall faces
    let xMinB = Infinity, xMaxB = -Infinity, yMinB = Infinity, yMaxB = -Infinity, zMinB = Infinity, zMaxB = -Infinity;
    const np = pos.length / 3;
    for (let i = 0; i < np; i++) {
      if (pos[i*3] < xMinB) xMinB = pos[i*3]; if (pos[i*3] > xMaxB) xMaxB = pos[i*3];
      if (pos[i*3+1] < yMinB) yMinB = pos[i*3+1]; if (pos[i*3+1] > yMaxB) yMaxB = pos[i*3+1];
      if (pos[i*3+2] < zMinB) zMinB = pos[i*3+2]; if (pos[i*3+2] > zMaxB) zMaxB = pos[i*3+2];
    }
    const tolX = Math.max((xMaxB - xMinB) * 0.01, 1e-4);

    for (let t = 0; t < nTris; t++) {
      const ax = pos[t*9], ay = pos[t*9+1], az = pos[t*9+2];
      const bx = pos[t*9+3], by = pos[t*9+4], bz = pos[t*9+5];
      const cx = pos[t*9+6], cy = pos[t*9+7], cz = pos[t*9+8];
      // Centroid x-coord (only x needed to detect inlet/outlet face)
      const mx = (ax + bx + cx) / 3;
      // Skip inlet/outlet patches — only walls contribute to Cd/Cl
      const onInlet = Math.abs(mx - xMinB) < tolX;
      const onOutlet = Math.abs(mx - xMaxB) < tolX;
      if (onInlet || onOutlet) continue;
      // Triangle normal (CCW outward for our mesh generator)
      const e1x = bx - ax, e1y = by - ay, e1z = bz - az;
      const e2x = cx - ax, e2y = cy - ay, e2z = cz - az;
      const ncx = e1y * e2z - e1z * e2y;
      const ncy = e1z * e2x - e1x * e2z;
      const ncz = e1x * e2y - e1y * e2x;
      const areaN = Math.sqrt(ncx * ncx + ncy * ncy + ncz * ncz);
      if (areaN < 1e-18) continue;
      const area = areaN * 0.5;
      const nx = ncx / areaN, ny = ncy / areaN, nz = ncz / areaN;
      // Vertex pressure average
      const pAvg = ((vals[t*3] ?? 0) + (vals[t*3+1] ?? 0) + (vals[t*3+2] ?? 0)) / 3;
      // Skip trivially zero surfaces
      // Force from pressure: F_p = −p·n·A (pressure pushes into fluid, so force on body = −p n A)
      wallForce.fx += -pAvg * nx * area;
      wallForce.fy += -pAvg * ny * area;
      wallForce.fz += -pAvg * nz * area;
      // Add viscous shear in the x-flow direction tangent
      // Tangent ~ flow direction projected onto face (simple: x-axis component perpendicular to n)
      const tflow = [1, 0, 0];
      const dot = tflow[0] * nx + tflow[1] * ny + tflow[2] * nz;
      const tx = tflow[0] - dot * nx;
      const ty = tflow[1] - dot * ny;
      const tz = tflow[2] - dot * nz;
      const tLen = Math.sqrt(tx * tx + ty * ty + tz * tz) || 1;
      wallForce.fx += tauW * (tx / tLen) * area;
      wallForce.fy += tauW * (ty / tLen) * area;
      wallForce.fz += tauW * (tz / tLen) * area;
      wallForce.area += area;
    }
  }
  // Coefficients — project drag on flow direction (x), lift on perpendicular (y)
  const refArea = Math.max(wallForce.area * 0.5, 1e-6);
  const cd = isConverged ? wallForce.fx / Math.max(refDynPressure * refArea, 1e-9) : NaN;
  const cl = isConverged ? wallForce.fy / Math.max(refDynPressure * refArea, 1e-9) : NaN;

  // Pressure stats
  let avgPressure = 0, minPressure = 0, maxPressure = 0;
  if (pressureField) {
    minPressure = pressureField.min;
    maxPressure = pressureField.max;
    let sum = 0;
    for (let i = 0; i < pressureField.values.length; i++) sum += pressureField.values[i];
    avgPressure = sum / pressureField.values.length;
  }

  // Velocity stats
  const velField = fieldData.find((f) => f.name === 'velocity');
  let avgVel = 0, minVel = 0, maxVel = 0;
  if (velField) {
    minVel = velField.min;
    maxVel = velField.max;
    let sum = 0;
    for (let i = 0; i < velField.values.length; i++) sum += velField.values[i];
    avgVel = sum / velField.values.length;
  }

  // Temperature stats
  let avgTemp = 0, minTemp = 0, maxTemp = 0;
  if (tempField) {
    minTemp = tempField.min;
    maxTemp = tempField.max;
    let sum = 0;
    for (let i = 0; i < tempField.values.length; i++) sum += tempField.values[i];
    avgTemp = sum / tempField.values.length;
  }

  // TKE stats
  const tkeField = fieldData.find((f) => f.name === 'tke');
  let avgTke = 0, minTke = 0, maxTke = 0;
  if (tkeField) {
    minTke = tkeField.min;
    maxTke = tkeField.max;
    let sum = 0;
    for (let i = 0; i < tkeField.values.length; i++) sum += tkeField.values[i];
    avgTke = sum / tkeField.values.length;
  }

  // Surface integrals per boundary
  const surfaceIntegrals = boundaries.map((b) => {
    const vMag = Math.sqrt(b.velocity[0]**2 + b.velocity[1]**2 + b.velocity[2]**2);
    const approxArea = cellCount > 0 ? Math.pow(cellCount, -1/3) * 4 : 1;
    const massFlux = b.type === 'inlet' ? material.density * vMag * approxArea
      : b.type === 'outlet' ? -totalInletMassFlux / Math.max(boundaries.filter(x => x.type === 'outlet').length, 1)
      : 0;
    const wallShear = b.type === 'wall' ? material.viscosity * vMag / 0.01 * approxArea : 0; // tau_w = mu * du/dy
    const heatFlux = b.type === 'wall' && b.wallThermalCondition === 'heat-flux' ? b.heatFlux * approxArea : 0;
    return { name: b.name, type: b.type, massFlux, wallShear, heatFlux };
  });

  // Probe points
  const probePoints = useAppStore.getState().probePoints;

  const exportCsv = () => {
    const lines: string[] = ['Metric,Value,Unit'];
    lines.push(`Drag Coefficient (Cd),${isConverged ? cd.toFixed(6) : 'N/A'},`);
    lines.push(`Lift Coefficient (Cl),${isConverged ? cl.toFixed(6) : 'N/A'},`);
    lines.push(`Inlet Mass Flow,${isConverged ? totalInletMassFlux.toFixed(6) : 'N/A'},kg/s`);
    lines.push(`Outlet Mass Flow,${isConverged ? (-totalInletMassFlux).toFixed(6) : 'N/A'},kg/s`);
    if (pressureField) {
      lines.push(`Average Pressure,${avgPressure.toFixed(2)},Pa`);
      lines.push(`Min Pressure,${minPressure.toFixed(2)},Pa`);
      lines.push(`Max Pressure,${maxPressure.toFixed(2)},Pa`);
    }
    if (velField) {
      lines.push(`Average Velocity,${avgVel.toFixed(4)},m/s`);
      lines.push(`Min Velocity,${minVel.toFixed(4)},m/s`);
      lines.push(`Max Velocity,${maxVel.toFixed(4)},m/s`);
    }
    if (tempField) {
      lines.push(`Average Temperature,${avgTemp.toFixed(2)},K`);
      lines.push(`Min Temperature,${minTemp.toFixed(2)},K`);
      lines.push(`Max Temperature,${maxTemp.toFixed(2)},K`);
    }
    if (tkeField) {
      lines.push(`Average TKE,${avgTke.toFixed(6)},m2/s2`);
      lines.push(`Min TKE,${minTke.toFixed(6)},m2/s2`);
      lines.push(`Max TKE,${maxTke.toFixed(6)},m2/s2`);
    }
    lines.push(`Final Continuity Residual,${lastResidual.continuity.toExponential(6)},`);
    lines.push(`Final X-Momentum Residual,${lastResidual.xMomentum.toExponential(6)},`);
    lines.push(`Final Y-Momentum Residual,${lastResidual.yMomentum.toExponential(6)},`);
    lines.push(`Final Energy Residual,${lastResidual.energy.toExponential(6)},`);
    lines.push(`Total Iterations,${residuals.length},`);

    // Add per-boundary mass flux
    boundaries.forEach((b) => {
      if (b.type === 'inlet') {
        const vMag = Math.sqrt(b.velocity[0] ** 2 + b.velocity[1] ** 2 + b.velocity[2] ** 2);
        const approxArea = cellCount > 0 ? Math.pow(cellCount, -1 / 3) * 4 : 1;
        lines.push(`Mass Flux - ${b.name},${(material.density * vMag * approxArea).toFixed(6)},kg/s`);
      } else if (b.type === 'outlet') {
        lines.push(`Mass Flux - ${b.name},${(-totalInletMassFlux / Math.max(outletBCs.length, 1)).toFixed(6)},kg/s`);
      }
    });

    const blob = new Blob([lines.join('\n')], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'gfd_report.csv';
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div style={{ padding: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 12, fontSize: 14, borderBottom: '1px solid #303030', paddingBottom: 8, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        Reports
        <div style={{ display: 'flex', gap: 4 }}>
          <Button size="small" icon={<DownloadOutlined />} onClick={exportCsv} disabled={!isConverged}>CSV</Button>
          <Button size="small" icon={<DownloadOutlined />} disabled={!isConverged} onClick={() => {
            const s = useAppStore.getState();
            const html = `<!DOCTYPE html><html><head><meta charset="utf-8"><title>GFD Report</title>
<style>body{font-family:sans-serif;max-width:800px;margin:40px auto;color:#333}
h1{color:#1668dc}table{border-collapse:collapse;width:100%;margin:16px 0}
td,th{border:1px solid #ddd;padding:6px 10px;text-align:left}th{background:#f5f5f5}
.good{color:#52c41a}.warn{color:#faad14}.bad{color:#ff4d4f}</style></head><body>
<h1>GFD Simulation Report</h1>
<p>Generated: ${new Date().toLocaleString()}</p>
<h2>Setup</h2>
<table><tr><th>Parameter</th><th>Value</th></tr>
<tr><td>Solver</td><td>${s.solverSettings.method}</td></tr>
<tr><td>Flow</td><td>${s.physicsModels.flow}</td></tr>
<tr><td>Turbulence</td><td>${s.physicsModels.turbulence}</td></tr>
<tr><td>Energy</td><td>${s.physicsModels.energy ? 'ON' : 'OFF'}</td></tr>
<tr><td>Material</td><td>${s.material.name} (ρ=${s.material.density}, μ=${s.material.viscosity.toExponential(3)})</td></tr>
<tr><td>Mesh</td><td>${s.meshDisplayData?.cellCount ?? 0} cells</td></tr>
<tr><td>Iterations</td><td>${s.currentIteration}</td></tr>
</table>
<h2>Field Statistics</h2>
<table><tr><th>Field</th><th>Min</th><th>Max</th><th>Average</th></tr>
${s.fieldData.map(f => {
  let avg = 0; for(let i=0;i<f.values.length;i++) avg+=f.values[i]; avg/=f.values.length;
  return `<tr><td>${f.name}</td><td>${f.min.toFixed(4)}</td><td>${f.max.toFixed(4)}</td><td>${avg.toFixed(4)}</td></tr>`;
}).join('')}
</table>
<h2>Boundary Conditions</h2>
<table><tr><th>Name</th><th>Type</th><th>Details</th></tr>
${s.boundaries.map(b => `<tr><td>${b.name}</td><td>${b.type}</td><td>${b.type==='inlet'?`v=[${b.velocity.join(',')}]`:b.type==='outlet'?`p=${b.pressure}`:'—'}</td></tr>`).join('')}
</table>
<h2>Final Residuals</h2>
<table><tr><th>Equation</th><th>Value</th></tr>
${s.residuals.length > 0 ? (() => { const r = s.residuals[s.residuals.length-1]; return `
<tr><td>Continuity</td><td>${r.continuity.toExponential(4)}</td></tr>
<tr><td>X-Momentum</td><td>${r.xMomentum.toExponential(4)}</td></tr>
<tr><td>Y-Momentum</td><td>${r.yMomentum.toExponential(4)}</td></tr>
<tr><td>Energy</td><td>${r.energy.toExponential(4)}</td></tr>`; })() : ''}
</table>
<p style="color:#888;font-size:12px">GFD — Generalized Fluid Dynamics v0.1.0</p>
</body></html>`;
            const blob = new Blob([html], { type: 'text/html' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url; a.download = 'gfd_report.html'; a.click();
            URL.revokeObjectURL(url);
            message.success('HTML report exported');
          }}>HTML</Button>
        </div>
      </div>

      <Typography.Text strong>Force Coefficients</Typography.Text>
      <Row gutter={[8, 8]} style={{ marginTop: 8 }}>
        <Col span={12}>
          <Card size="small">
            <Statistic
              title="Cd (Drag)"
              value={isConverged ? cd : NaN}
              precision={4}
              valueStyle={{ fontSize: 16 }}
              formatter={(v) => (isConverged ? Number(v).toFixed(4) : '--')}
            />
          </Card>
        </Col>
        <Col span={12}>
          <Card size="small">
            <Statistic
              title="Cl (Lift)"
              value={isConverged ? cl : NaN}
              precision={4}
              valueStyle={{ fontSize: 16 }}
              formatter={(v) => (isConverged ? Number(v).toFixed(4) : '--')}
            />
          </Card>
        </Col>
      </Row>
      {isConverged && wallForce.area > 0 && (
        <div style={{ marginTop: 6, padding: 6, background: '#1a1a30', borderRadius: 4, fontSize: 10, color: '#778' }}>
          Wall force vector (N): Fx={wallForce.fx.toExponential(2)}, Fy={wallForce.fy.toExponential(2)}, Fz={wallForce.fz.toExponential(2)} — integrated over {wallForce.area.toFixed(3)} m² of wall triangles
        </div>
      )}

      <Divider style={{ margin: '12px 0' }} />

      <Typography.Text strong>Mass Flux Report</Typography.Text>
      <Row gutter={[8, 8]} style={{ marginTop: 8 }}>
        {boundaries.filter((b) => b.type === 'inlet' || b.type === 'outlet').map((b) => {
          let flux = 0;
          if (b.type === 'inlet') {
            const vMag = Math.sqrt(b.velocity[0] ** 2 + b.velocity[1] ** 2 + b.velocity[2] ** 2);
            const approxArea = cellCount > 0 ? Math.pow(cellCount, -1 / 3) * 4 : 1;
            flux = material.density * vMag * approxArea;
          } else {
            flux = -totalInletMassFlux / Math.max(outletBCs.length, 1);
          }
          return (
            <Col span={12} key={b.id}>
              <Card size="small">
                <Statistic
                  title={`${b.name} (kg/s)`}
                  value={isConverged ? flux : NaN}
                  precision={4}
                  valueStyle={{ fontSize: 14, color: flux >= 0 ? '#52c41a' : '#ff4d4f' }}
                  formatter={(v) => (isConverged ? Number(v).toFixed(4) : '--')}
                />
              </Card>
            </Col>
          );
        })}
        {boundaries.filter((b) => b.type === 'inlet' || b.type === 'outlet').length === 0 && (
          <Col span={24}>
            <div style={{ color: '#667', fontSize: 11, padding: 8 }}>No inlet/outlet boundaries defined.</div>
          </Col>
        )}
      </Row>

      <Divider style={{ margin: '12px 0' }} />

      <Typography.Text strong>Pressure (Pa)</Typography.Text>
      <Row gutter={[8, 8]} style={{ marginTop: 8 }}>
        <Col span={8}>
          <Card size="small">
            <Statistic title="Avg" value={isConverged && pressureField ? avgPressure : NaN} precision={2} valueStyle={{ fontSize: 14 }} formatter={(v) => (isConverged && pressureField ? Number(v).toFixed(2) : '--')} />
          </Card>
        </Col>
        <Col span={8}>
          <Card size="small">
            <Statistic title="Min" value={isConverged && pressureField ? minPressure : NaN} precision={2} valueStyle={{ fontSize: 14 }} formatter={(v) => (isConverged && pressureField ? Number(v).toFixed(2) : '--')} />
          </Card>
        </Col>
        <Col span={8}>
          <Card size="small">
            <Statistic title="Max" value={isConverged && pressureField ? maxPressure : NaN} precision={2} valueStyle={{ fontSize: 14 }} formatter={(v) => (isConverged && pressureField ? Number(v).toFixed(2) : '--')} />
          </Card>
        </Col>
      </Row>

      <Divider style={{ margin: '12px 0' }} />

      <Typography.Text strong>Velocity (m/s)</Typography.Text>
      <Row gutter={[8, 8]} style={{ marginTop: 8 }}>
        <Col span={8}>
          <Card size="small">
            <Statistic title="Avg" value={isConverged && velField ? avgVel : NaN} precision={4} valueStyle={{ fontSize: 14 }} formatter={(v) => (isConverged && velField ? Number(v).toFixed(4) : '--')} />
          </Card>
        </Col>
        <Col span={8}>
          <Card size="small">
            <Statistic title="Min" value={isConverged && velField ? minVel : NaN} precision={4} valueStyle={{ fontSize: 14 }} formatter={(v) => (isConverged && velField ? Number(v).toFixed(4) : '--')} />
          </Card>
        </Col>
        <Col span={8}>
          <Card size="small">
            <Statistic title="Max" value={isConverged && velField ? maxVel : NaN} precision={4} valueStyle={{ fontSize: 14 }} formatter={(v) => (isConverged && velField ? Number(v).toFixed(4) : '--')} />
          </Card>
        </Col>
      </Row>

      <Divider style={{ margin: '12px 0' }} />

      <Typography.Text strong>Temperature (K)</Typography.Text>
      <Row gutter={[8, 8]} style={{ marginTop: 8 }}>
        <Col span={8}>
          <Card size="small">
            <Statistic
              title="Avg (K)"
              value={isConverged && tempField ? avgTemp : NaN}
              precision={1}
              valueStyle={{ fontSize: 14 }}
              formatter={(v) => (isConverged && tempField ? Number(v).toFixed(1) : '--')}
            />
          </Card>
        </Col>
        <Col span={8}>
          <Card size="small">
            <Statistic
              title="Min (K)"
              value={isConverged && tempField ? minTemp : NaN}
              precision={1}
              valueStyle={{ fontSize: 14 }}
              formatter={(v) => (isConverged && tempField ? Number(v).toFixed(1) : '--')}
            />
          </Card>
        </Col>
        <Col span={8}>
          <Card size="small">
            <Statistic
              title="Max (K)"
              value={isConverged && tempField ? maxTemp : NaN}
              precision={1}
              valueStyle={{ fontSize: 14 }}
              formatter={(v) => (isConverged && tempField ? Number(v).toFixed(1) : '--')}
            />
          </Card>
        </Col>
      </Row>

      {tkeField && (
        <>
          <Divider style={{ margin: '12px 0' }} />
          <Typography.Text strong>Turbulence Kinetic Energy (m²/s²)</Typography.Text>
          <Row gutter={[8, 8]} style={{ marginTop: 8 }}>
            <Col span={8}>
              <Card size="small">
                <Statistic title="Avg" value={isConverged ? avgTke : NaN} precision={4} valueStyle={{ fontSize: 14 }} formatter={(v) => (isConverged ? Number(v).toFixed(4) : '--')} />
              </Card>
            </Col>
            <Col span={8}>
              <Card size="small">
                <Statistic title="Min" value={isConverged ? minTke : NaN} precision={4} valueStyle={{ fontSize: 14 }} formatter={(v) => (isConverged ? Number(v).toFixed(4) : '--')} />
              </Card>
            </Col>
            <Col span={8}>
              <Card size="small">
                <Statistic title="Max" value={isConverged ? maxTke : NaN} precision={4} valueStyle={{ fontSize: 14 }} formatter={(v) => (isConverged ? Number(v).toFixed(4) : '--')} />
              </Card>
            </Col>
          </Row>
        </>
      )}

      {/* Surface Integrals */}
      {surfaceIntegrals.some(s => s.wallShear > 0 || s.heatFlux !== 0) && (
        <>
          <Divider style={{ margin: '12px 0' }} />
          <Typography.Text strong>Surface Integrals</Typography.Text>
          <div style={{ marginTop: 8, fontSize: 11 }}>
            {surfaceIntegrals.filter(s => s.type === 'wall').map((s, i) => (
              <div key={i} style={{ padding: '2px 0', color: '#aab', borderBottom: '1px solid #252540' }}>
                <b>{s.name}:</b> Shear={isConverged ? s.wallShear.toFixed(4) : '--'} N
                {s.heatFlux !== 0 && `, Q=${s.heatFlux.toFixed(2)} W`}
              </div>
            ))}
          </div>
        </>
      )}

      {/* Probe Points */}
      {probePoints.length > 0 && (
        <>
          <Divider style={{ margin: '12px 0' }} />
          <Typography.Text strong>Probe Points ({probePoints.length})</Typography.Text>
          <div style={{ marginTop: 8, fontSize: 11, maxHeight: 120, overflow: 'auto' }}>
            {probePoints.map((p) => (
              <div key={p.id} style={{ padding: '3px 0', color: '#aab', borderBottom: '1px solid #252540' }}>
                <div style={{ color: '#ff6666', fontSize: 10 }}>
                  ({p.position[0].toFixed(2)}, {p.position[1].toFixed(2)}, {p.position[2].toFixed(2)})
                </div>
                {Object.entries(p.values).map(([name, val]) => (
                  <span key={name} style={{ marginRight: 8 }}>{name}={val.toFixed(4)}</span>
                ))}
              </div>
            ))}
          </div>
        </>
      )}

      <Divider style={{ margin: '12px 0' }} />

      {/* Mass & Energy Balance */}
      {/* Mesh Independence Guide */}
      <Typography.Text strong>Mesh Independence</Typography.Text>
      <div style={{ marginTop: 8, marginBottom: 12, padding: 8, background: '#1a1a30', borderRadius: 4, fontSize: 11, color: '#889' }}>
        <div>Current mesh: <b>{meshDisplayData?.cellCount ?? 0}</b> cells (h = {(meshDisplayData ? Math.cbrt((meshDisplayData.nx > 0 ? 4/meshDisplayData.cellCount : 1)) : 0).toFixed(4)}m)</div>
        <div style={{ marginTop: 4 }}>
          To verify mesh independence:
        </div>
        <div style={{ paddingLeft: 8, fontSize: 10, color: '#778' }}>
          1. Record key values (drag, max velocity, avg temp)
          <br />
          2. Halve cell size (Mesh Settings → Global Size / 2)
          <br />
          3. Re-generate mesh and re-solve
          <br />
          4. Compare results — {'<'}2% change = mesh independent
        </div>
        {pressureField && velField && (
          <div style={{ marginTop: 4, color: '#aab', borderTop: '1px solid #252540', paddingTop: 4 }}>
            Reference values: P_avg={avgPressure.toFixed(2)} | V_avg={avgVel.toFixed(4)} {tempField ? `| T_avg=${avgTemp.toFixed(1)}` : ''}
          </div>
        )}
      </div>

      <Typography.Text strong>Conservation Balance</Typography.Text>
      <div style={{ marginTop: 8, marginBottom: 12, padding: 8, background: '#1a1a30', borderRadius: 4, fontSize: 11 }}>
        {(() => {
          const inFlux = boundaries.filter(b => b.type === 'inlet').reduce((sum, b) => {
            const v = Math.sqrt(b.velocity[0]**2 + b.velocity[1]**2 + b.velocity[2]**2);
            return sum + material.density * v;
          }, 0);
          const outFlux = boundaries.filter(b => b.type === 'outlet').length > 0 ? inFlux : 0;
          const imbalance = inFlux > 0 ? Math.abs(inFlux - outFlux) / inFlux * 100 : 0;
          const inEnergy = boundaries.filter(b => b.type === 'inlet').reduce((sum, b) => sum + material.density * material.cp * b.temperature, 0);
          return (
            <>
              <div style={{ color: '#aab' }}>Mass: in={inFlux.toFixed(4)} kg/s | out={outFlux.toFixed(4)} kg/s | imbalance={imbalance.toFixed(2)}%</div>
              {inEnergy > 0 && <div style={{ color: '#aab' }}>Energy flux (inlet): {inEnergy.toFixed(1)} W</div>}
              <div style={{ color: imbalance < 1 ? '#52c41a' : imbalance < 5 ? '#faad14' : '#ff4d4f' }}>
                {imbalance < 1 ? 'Excellent balance' : imbalance < 5 ? 'Acceptable balance' : 'Poor balance — check BCs'}
              </div>
            </>
          );
        })()}
      </div>

      <Typography.Text strong>Final Residuals</Typography.Text>
      <Row gutter={[8, 8]} style={{ marginTop: 8 }}>
        <Col span={12}>
          <Card size="small">
            <Statistic
              title="Continuity"
              value={lastResidual.continuity}
              precision={2}
              valueStyle={{ fontSize: 14 }}
              formatter={(v) => Number(v).toExponential(2)}
            />
          </Card>
        </Col>
        <Col span={12}>
          <Card size="small">
            <Statistic
              title="X-Momentum"
              value={lastResidual.xMomentum}
              precision={2}
              valueStyle={{ fontSize: 14 }}
              formatter={(v) => Number(v).toExponential(2)}
            />
          </Card>
        </Col>
        <Col span={12}>
          <Card size="small">
            <Statistic
              title="Y-Momentum"
              value={lastResidual.yMomentum}
              precision={2}
              valueStyle={{ fontSize: 14 }}
              formatter={(v) => Number(v).toExponential(2)}
            />
          </Card>
        </Col>
        <Col span={12}>
          <Card size="small">
            <Statistic
              title="Energy"
              value={lastResidual.energy}
              precision={2}
              valueStyle={{ fontSize: 14 }}
              formatter={(v) => Number(v).toExponential(2)}
            />
          </Card>
        </Col>
      </Row>

      {/* Line Plot: field values along X axis */}
      {isConverged && fieldData.length > 0 && (
        <>
          <Divider style={{ margin: '12px 0' }} />
          <Typography.Text strong>Line Plot (along X axis, y=0.5, z=0.5)</Typography.Text>
          <LinePlotSection />
        </>
      )}
    </div>
  );
};

/** Line plot sampling field values along X axis */
const LinePlotSection: React.FC = () => {
  const fieldData = useAppStore((s) => s.fieldData);
  const meshDisplayData = useAppStore((s) => s.meshDisplayData);
  const [plotField, setPlotField] = React.useState('pressure');

  const plotData = useMemo(() => {
    if (!meshDisplayData || fieldData.length === 0) return [];
    const field = fieldData.find(f => f.name === plotField);
    if (!field) return [];

    const positions = meshDisplayData.positions;
    const nVerts = positions.length / 3;
    let xMin = Infinity, xMax = -Infinity, yMin = Infinity, yMax = -Infinity, zMin = Infinity, zMax = -Infinity;
    for (let i = 0; i < Math.min(nVerts, 1000); i++) {
      if (positions[i*3] < xMin) xMin = positions[i*3]; if (positions[i*3] > xMax) xMax = positions[i*3];
      if (positions[i*3+1] < yMin) yMin = positions[i*3+1]; if (positions[i*3+1] > yMax) yMax = positions[i*3+1];
      if (positions[i*3+2] < zMin) zMin = positions[i*3+2]; if (positions[i*3+2] > zMax) zMax = positions[i*3+2];
    }
    const yMid = (yMin + yMax) / 2, zMid = (zMin + zMax) / 2;
    const yTol = (yMax - yMin) * 0.1, zTol = (zMax - zMin) * 0.1;

    // Collect vertices near the centerline (y≈0.5, z≈0.5)
    const samples: { x: number; value: number }[] = [];
    for (let i = 0; i < nVerts && i < field.values.length; i++) {
      const y = positions[i*3+1], z = positions[i*3+2];
      if (Math.abs(y - yMid) < yTol && Math.abs(z - zMid) < zTol) {
        samples.push({ x: Math.round(positions[i*3] * 100) / 100, value: Math.round(field.values[i] * 1000) / 1000 });
      }
    }
    samples.sort((a, b) => a.x - b.x);
    // Deduplicate by x
    const unique: typeof samples = [];
    for (const s of samples) {
      if (unique.length === 0 || Math.abs(s.x - unique[unique.length-1].x) > 0.01) unique.push(s);
    }
    return unique;
  }, [fieldData, meshDisplayData, plotField]);

  return (
    <div style={{ marginTop: 8 }}>
      <Select size="small" value={plotField} onChange={setPlotField} style={{ width: 150, marginBottom: 8 }}
        options={fieldData.map(f => ({ label: f.name, value: f.name }))}
      />
      {plotData.length > 2 ? (
        <div style={{ width: '100%', height: 150 }}>
          <ResponsiveContainer width="100%" height={150} minWidth={100}>
            <LineChart data={plotData}>
              <CartesianGrid strokeDasharray="3 3" stroke="#333" />
              <XAxis dataKey="x" stroke="#888" tick={{ fontSize: 9 }} label={{ value: 'X (m)', position: 'insideBottom', offset: -3, fontSize: 9 }} />
              <YAxis stroke="#888" tick={{ fontSize: 9 }} />
              <Tooltip contentStyle={{ background: '#1f1f1f', border: '1px solid #444', fontSize: 11 }} />
              <Line type="monotone" dataKey="value" stroke="#1668dc" dot={false} strokeWidth={1.5} isAnimationActive={false} />
            </LineChart>
          </ResponsiveContainer>
        </div>
      ) : (
        <div style={{ color: '#556', fontSize: 11, padding: 8 }}>Not enough data points along centerline.</div>
      )}
    </div>
  );
};

export default ReportPanel;
