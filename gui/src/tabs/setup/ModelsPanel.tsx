import React from 'react';
import { Form, Select, Checkbox, Typography } from 'antd';
import { useAppStore } from '../../store/useAppStore';

const descriptions: Record<string, Record<string, string>> = {
  flow: {
    incompressible: 'Constant density flow. Suitable for low-speed (Ma < 0.3) liquid or gas flows.',
    compressible: 'Variable density flow. Required for high-speed gas dynamics (Ma > 0.3), shock waves.',
  },
  turbulence: {
    none: 'Laminar flow assumption. Valid for low Reynolds number flows.',
    'k-epsilon': 'Standard k-epsilon model. Good general-purpose RANS model for industrial flows.',
    'k-omega-sst': 'k-omega SST model. Excellent for boundary layer flows, adverse pressure gradients.',
    sa: 'Spalart-Allmaras model. One-equation model, efficient for aerodynamic flows.',
    les: 'Large Eddy Simulation. Resolves large turbulent structures; requires fine mesh and small time steps.',
  },
  multiphase: {
    none: 'Single-phase flow.',
    vof: 'Volume of Fluid. Tracks immiscible fluid interfaces (e.g., free surface, sloshing).',
    euler: 'Euler-Euler model. For interpenetrating phases with significant volume fractions.',
    mixture: 'Mixture model. Simplified multiphase for dispersed flows with slip velocity.',
    dpm: 'Discrete Phase Model. Lagrangian tracking of particles, droplets, or bubbles.',
  },
  radiation: {
    none: 'No radiation modeling.',
    p1: 'P-1 radiation model. Fast, suitable for optically thick media.',
    dom: 'Discrete Ordinates Model. Accurate for all optical thicknesses; higher computational cost.',
  },
  species: {
    none: 'No species transport.',
    'species-transport': 'Species Transport. Solves convection-diffusion equations for chemical species.',
    combustion: 'Combustion model. Species transport with chemical reaction source terms.',
  },
};

const ModelsPanel: React.FC = () => {
  const physicsModels = useAppStore((s) => s.physicsModels);
  const updatePhysicsModels = useAppStore((s) => s.updatePhysicsModels);

  const descStyle: React.CSSProperties = {
    fontSize: 11,
    color: '#778',
    padding: '4px 8px',
    background: '#1a1a30',
    borderRadius: 4,
    marginTop: -4,
    marginBottom: 8,
  };

  return (
    <div style={{ padding: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 12, fontSize: 14, borderBottom: '1px solid #303030', paddingBottom: 8 }}>
        Physics Models
      </div>
      <Form layout="vertical" size="small">
        <Form.Item label="Flow">
          <Select
            value={physicsModels.flow}
            onChange={(v) => updatePhysicsModels({ flow: v })}
            options={[
              { label: 'Incompressible', value: 'incompressible' },
              { label: 'Compressible', value: 'compressible' },
            ]}
          />
        </Form.Item>
        <div style={descStyle}>{descriptions.flow[physicsModels.flow]}</div>

        <Form.Item label="Turbulence">
          <Select
            value={physicsModels.turbulence}
            onChange={(v) => updatePhysicsModels({ turbulence: v })}
            options={[
              { label: 'None (Laminar)', value: 'none' },
              { label: 'k-epsilon', value: 'k-epsilon' },
              { label: 'k-omega SST', value: 'k-omega-sst' },
              { label: 'Spalart-Allmaras', value: 'sa' },
              { label: 'LES', value: 'les' },
            ]}
          />
        </Form.Item>
        <div style={descStyle}>{descriptions.turbulence[physicsModels.turbulence]}</div>

        <Form.Item valuePropName="checked">
          <Checkbox
            checked={physicsModels.energy}
            onChange={(e) => updatePhysicsModels({ energy: e.target.checked })}
          >
            Energy Equation
          </Checkbox>
        </Form.Item>
        {physicsModels.energy && (
          <div style={descStyle}>Solves the energy equation for temperature distribution and heat transfer.</div>
        )}

        <Form.Item label="Multiphase">
          <Select
            value={physicsModels.multiphase}
            onChange={(v) => updatePhysicsModels({ multiphase: v })}
            options={[
              { label: 'None', value: 'none' },
              { label: 'VOF', value: 'vof' },
              { label: 'Euler-Euler', value: 'euler' },
              { label: 'Mixture', value: 'mixture' },
              { label: 'DPM', value: 'dpm' },
            ]}
          />
        </Form.Item>
        <div style={descStyle}>{descriptions.multiphase[physicsModels.multiphase]}</div>

        <Form.Item label="Radiation">
          <Select
            value={physicsModels.radiation}
            onChange={(v) => updatePhysicsModels({ radiation: v })}
            options={[
              { label: 'None', value: 'none' },
              { label: 'P-1', value: 'p1' },
              { label: 'Discrete Ordinates (DO)', value: 'dom' },
            ]}
          />
        </Form.Item>
        <div style={descStyle}>{descriptions.radiation[physicsModels.radiation]}</div>

        <Form.Item label="Species">
          <Select
            value={physicsModels.species}
            onChange={(v) => updatePhysicsModels({ species: v })}
            options={[
              { label: 'None', value: 'none' },
              { label: 'Species Transport', value: 'species-transport' },
              { label: 'Combustion', value: 'combustion' },
            ]}
          />
        </Form.Item>
        <div style={descStyle}>{descriptions.species[physicsModels.species]}</div>
      </Form>
    </div>
  );
};

export default ModelsPanel;
