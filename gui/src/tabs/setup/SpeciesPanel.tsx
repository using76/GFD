import React from 'react';
import { Form, Input, InputNumber, Button, Empty, Divider, Typography, Select, message } from 'antd';
import { PlusOutlined, DeleteOutlined } from '@ant-design/icons';
import { useAppStore } from '../../store/useAppStore';
import type { SpeciesDefinition } from '../../store/useAppStore';

const PRESETS: SpeciesDefinition[] = [
  { id: '', name: 'O2', molecularWeight: 32.0, diffusivity: 2.1e-5, inletMassFraction: 0.23, initialMassFraction: 0.23 },
  { id: '', name: 'N2', molecularWeight: 28.0, diffusivity: 2.0e-5, inletMassFraction: 0.77, initialMassFraction: 0.77 },
  { id: '', name: 'CH4', molecularWeight: 16.0, diffusivity: 2.2e-5, inletMassFraction: 1.0, initialMassFraction: 0.0 },
  { id: '', name: 'CO2', molecularWeight: 44.0, diffusivity: 1.6e-5, inletMassFraction: 0.0, initialMassFraction: 0.0 },
  { id: '', name: 'H2O', molecularWeight: 18.0, diffusivity: 2.6e-5, inletMassFraction: 0.0, initialMassFraction: 0.0 },
];

const SpeciesPanel: React.FC = () => {
  const species = useAppStore((s) => s.species);
  const reactions = useAppStore((s) => s.reactions);
  const addSpecies = useAppStore((s) => s.addSpecies);
  const updateSpecies = useAppStore((s) => s.updateSpecies);
  const removeSpecies = useAppStore((s) => s.removeSpecies);
  const addReaction = useAppStore((s) => s.addReaction);
  const updateReaction = useAppStore((s) => s.updateReaction);
  const removeReaction = useAppStore((s) => s.removeReaction);
  const physicsModels = useAppStore((s) => s.physicsModels);

  const isCombustion = physicsModels.species === 'combustion';
  const isSpeciesOn = physicsModels.species !== 'none';

  const addPreset = (preset: SpeciesDefinition) => {
    const id = `sp-${Date.now()}-${preset.name}`;
    addSpecies({ ...preset, id });
    message.success(`Added ${preset.name}`);
  };

  if (!isSpeciesOn) {
    return (
      <div style={{ padding: 16 }}>
        <Empty
          description={
            <>
              Species transport is disabled.
              <br />
              Enable "Species Transport" or "Combustion" in Models to define species.
            </>
          }
        />
      </div>
    );
  }

  return (
    <div style={{ padding: 12 }}>
      <div style={{ fontWeight: 600, marginBottom: 12, fontSize: 14, borderBottom: '1px solid #303030', paddingBottom: 8 }}>
        Species ({species.length})
      </div>

      <div style={{ marginBottom: 8 }}>
        <Typography.Text type="secondary" style={{ fontSize: 11 }}>Quick add:</Typography.Text>
        <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap', marginTop: 4 }}>
          {PRESETS.map((p) => (
            <Button key={p.name} size="small" onClick={() => addPreset(p)}>{p.name}</Button>
          ))}
          <Button
            size="small"
            icon={<PlusOutlined />}
            onClick={() => {
              const id = `sp-${Date.now()}`;
              addSpecies({
                id,
                name: `species-${species.length + 1}`,
                molecularWeight: 28.0,
                diffusivity: 2.0e-5,
                inletMassFraction: 0.0,
                initialMassFraction: 0.0,
              });
            }}
          >
            Custom
          </Button>
        </div>
      </div>

      {species.length === 0 ? (
        <Empty description="No species defined." style={{ marginTop: 12 }} />
      ) : (
        <div style={{ maxHeight: 260, overflow: 'auto' }}>
          {species.map((sp) => (
            <div key={sp.id} style={{ padding: 6, marginBottom: 4, border: '1px solid #303050', borderRadius: 4 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 4 }}>
                <Input
                  size="small"
                  value={sp.name}
                  onChange={(e) => updateSpecies(sp.id, { name: e.target.value })}
                  style={{ flex: 1, fontWeight: 600 }}
                />
                <Button size="small" danger icon={<DeleteOutlined />} onClick={() => removeSpecies(sp.id)} />
              </div>
              <Form layout="horizontal" size="small">
                <Form.Item label="MW (kg/kmol)" style={{ marginBottom: 2 }}>
                  <InputNumber size="small" value={sp.molecularWeight} step={1} style={{ width: '100%' }} onChange={(v) => updateSpecies(sp.id, { molecularWeight: v ?? 28 })} />
                </Form.Item>
                <Form.Item label="D (m²/s)" style={{ marginBottom: 2 }}>
                  <InputNumber size="small" value={sp.diffusivity} step={1e-6} style={{ width: '100%' }} onChange={(v) => updateSpecies(sp.id, { diffusivity: v ?? 2e-5 })} />
                </Form.Item>
                <Form.Item label="Y_inlet" style={{ marginBottom: 2 }}>
                  <InputNumber size="small" value={sp.inletMassFraction} min={0} max={1} step={0.01} style={{ width: '100%' }} onChange={(v) => updateSpecies(sp.id, { inletMassFraction: v ?? 0 })} />
                </Form.Item>
                <Form.Item label="Y_init" style={{ marginBottom: 0 }}>
                  <InputNumber size="small" value={sp.initialMassFraction} min={0} max={1} step={0.01} style={{ width: '100%' }} onChange={(v) => updateSpecies(sp.id, { initialMassFraction: v ?? 0 })} />
                </Form.Item>
              </Form>
            </div>
          ))}
        </div>
      )}

      {/* Mass-fraction sanity check */}
      {species.length > 0 && (
        <div style={{ marginTop: 6, padding: 4, background: '#1a1a30', borderRadius: 4, fontSize: 10, color: '#778' }}>
          ΣY_inlet = {species.reduce((s, sp) => s + sp.inletMassFraction, 0).toFixed(3)} (should be ≈ 1.0)
        </div>
      )}

      {/* Reactions (only if combustion) */}
      {isCombustion && (
        <>
          <Divider style={{ margin: '12px 0' }} />
          <div style={{ fontWeight: 600, marginBottom: 8, fontSize: 13 }}>
            Reactions ({reactions.length})
          </div>
          <Button
            size="small"
            icon={<PlusOutlined />}
            block
            disabled={species.length < 2}
            onClick={() => {
              const id = `rx-${Date.now()}`;
              addReaction({
                id,
                name: `reaction-${reactions.length + 1}`,
                reactants: species.slice(0, 1).map(sp => ({ speciesId: sp.id, stoich: 1 })),
                products: species.slice(-1).map(sp => ({ speciesId: sp.id, stoich: 1 })),
                arrheniusA: 1e10,
                activationEnergy: 1.6e8,
                beta: 0,
              });
            }}
          >
            Add Reaction
          </Button>
          {species.length < 2 && (
            <Typography.Text type="secondary" style={{ fontSize: 11, display: 'block', marginTop: 4 }}>
              Define at least two species to create a reaction.
            </Typography.Text>
          )}

          {reactions.length > 0 && (
            <div style={{ marginTop: 8, maxHeight: 240, overflow: 'auto' }}>
              {reactions.map((r) => (
                <div key={r.id} style={{ padding: 6, marginBottom: 4, border: '1px solid #303050', borderRadius: 4 }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginBottom: 4 }}>
                    <Input
                      size="small"
                      value={r.name}
                      onChange={(e) => updateReaction(r.id, { name: e.target.value })}
                      style={{ flex: 1, fontWeight: 600 }}
                    />
                    <Button size="small" danger icon={<DeleteOutlined />} onClick={() => removeReaction(r.id)} />
                  </div>
                  <Form layout="vertical" size="small">
                    <Form.Item label="Reactant" style={{ marginBottom: 2 }}>
                      <Select size="small" value={r.reactants[0]?.speciesId}
                        options={species.map(sp => ({ label: sp.name, value: sp.id }))}
                        onChange={(v) => updateReaction(r.id, { reactants: [{ speciesId: v, stoich: r.reactants[0]?.stoich ?? 1 }] })}
                      />
                    </Form.Item>
                    <Form.Item label="Product" style={{ marginBottom: 2 }}>
                      <Select size="small" value={r.products[0]?.speciesId}
                        options={species.map(sp => ({ label: sp.name, value: sp.id }))}
                        onChange={(v) => updateReaction(r.id, { products: [{ speciesId: v, stoich: r.products[0]?.stoich ?? 1 }] })}
                      />
                    </Form.Item>
                    <Form.Item label="A (pre-exp)" style={{ marginBottom: 2 }}>
                      <InputNumber size="small" value={r.arrheniusA} step={1e9} style={{ width: '100%' }} onChange={(v) => updateReaction(r.id, { arrheniusA: v ?? 1e10 })} />
                    </Form.Item>
                    <Form.Item label="Ea (J/kmol)" style={{ marginBottom: 2 }}>
                      <InputNumber size="small" value={r.activationEnergy} step={1e7} style={{ width: '100%' }} onChange={(v) => updateReaction(r.id, { activationEnergy: v ?? 1e8 })} />
                    </Form.Item>
                    <Form.Item label="β" style={{ marginBottom: 0 }}>
                      <InputNumber size="small" value={r.beta} step={0.1} style={{ width: '100%' }} onChange={(v) => updateReaction(r.id, { beta: v ?? 0 })} />
                    </Form.Item>
                  </Form>
                  <div style={{ fontSize: 10, color: '#778', marginTop: 4 }}>
                    k = A · T^β · exp(−Ea/RT)
                  </div>
                </div>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
};

export default SpeciesPanel;
