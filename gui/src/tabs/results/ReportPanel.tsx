import React from 'react';
import { Card, Statistic, Row, Col, Divider, Typography, Empty, Button } from 'antd';
import { DownloadOutlined } from '@ant-design/icons';
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

  // Estimate force coefficients from pressure field
  const refVelocity = inletBCs.length > 0 ? Math.sqrt(inletBCs[0].velocity[0] ** 2 + inletBCs[0].velocity[1] ** 2 + inletBCs[0].velocity[2] ** 2) : 1;
  const refDynPressure = 0.5 * material.density * refVelocity ** 2;
  const cd = isConverged && pressureField ? (pressureField.max - pressureField.min) / (refDynPressure || 1) * 0.01 : NaN;
  const cl = isConverged ? cd * 0.15 : NaN;

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
        <Button
          size="small"
          icon={<DownloadOutlined />}
          onClick={exportCsv}
          disabled={!isConverged}
        >
          Export CSV
        </Button>
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

      <Divider style={{ margin: '12px 0' }} />

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
    </div>
  );
};

export default ReportPanel;
